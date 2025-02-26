use actix_cors::Cors;
use actix_files::{Files, NamedFile};
use actix_http::{
    header::{self, HeaderMap},
    ContentEncoding,
};
use actix_web::http::header::{CacheControl, CacheDirective, ContentType, ETag, EntityTag};
use actix_web::{
    body::MessageBody,
    dev::{ServiceFactory, ServiceRequest, ServiceResponse},
    error::{self, Error},
    http::StatusCode,
    middleware::{Compat, Condition, ErrorHandlers, Logger},
    web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use cargo_metadata::MetadataCommand;
use regex::Regex;
use rustls::{Certificate, PrivateKey, ServerConfig as RustlsServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufReader};
use std::net::SocketAddr;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

pub use actix_cors;
pub use actix_files;
pub use actix_http;
pub use actix_web;
pub use apply::{Also, Apply};
pub use async_trait::async_trait;
pub use chashmap;
pub use enclose::enc as clone;
pub use futures;
pub use futures_signals_ext::{self, *};
pub use lang::Lang;
pub use mime;
pub use mime_guess;
pub use moon_entry_macros::{main, test};
pub use moonlight::{self, *};
pub use once_cell::{self, sync::Lazy};
pub use parking_lot;
pub use rustls;
pub use rustls_pemfile;
pub use serde;
pub use tokio;
pub use tokio_stream;
pub use trait_set::trait_set;
pub use uuid;

mod actor;
pub mod config;
pub mod error_handler;
mod from_env_vars;
mod frontend;
mod lazy_message_writer;
mod not;
mod redirect;
mod sse;
mod up_msg_request;

use config::CONFIG;
use lazy_message_writer::LazyMessageWriter;
use sse::{ShareableSSE, ShareableSSEMethods, SSE};

pub use actor::{
    sessions::{self, SessionActor},
    ActorId, ActorInstance, Index, PVar,
};
pub use from_env_vars::FromEnvVars;
pub use frontend::Frontend;
pub use not::not;
pub use redirect::Redirect;
pub use up_msg_request::UpMsgRequest;

// @TODO make it configurable
// const MAX_UP_MSG_BYTES: usize = 2 * 1_048_576;
const MAX_UP_MSG_BYTES: usize = usize::MAX;

#[derive(Copy, Clone)]
struct SharedData {
    backend_build_id: u128,
    frontend_build_id: u128,
    cache_busting: bool,
    compressed_pkg: bool,
    pkg_path: &'static str,
}

#[derive(Clone)]
struct ReloadSSE(ShareableSSE);

impl Deref for ReloadSSE {
    type Target = ShareableSSE;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct MessageSSE(ShareableSSE);

impl Deref for MessageSSE {
    type Target = ShareableSSE;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// trait aliases
trait_set! {
    pub trait FrontBuilderOutput = Future<Output = Frontend> + 'static;
    pub trait FrontBuilder<FRBO: FrontBuilderOutput> = Fn() -> FRBO + Send + Sync + 'static;

    pub trait UpHandlerOutput = Future<Output = ()> + 'static;
    pub trait UpHandler<UPHO: UpHandlerOutput, UMsg> = Fn(UpMsgRequest<UMsg>) -> UPHO + Send + Sync + 'static;
}

// ------ ------
//     Start
// ------ ------

pub async fn start<FRB, FRBO, UPH, UPHO, UMsg>(
    frontend: FRB,
    up_msg_handler: UPH,
    service_config: impl Fn(&mut web::ServiceConfig) + Send + Sync + 'static,
) -> io::Result<()>
where
    FRB: FrontBuilder<FRBO>,
    FRBO: FrontBuilderOutput,
    UPH: UpHandler<UPHO, UMsg>,
    UPHO: UpHandlerOutput,
    UMsg: 'static + DeserializeOwned,
{
    let app = || {
        let redirect = Redirect::new()
            .http_to_https(CONFIG.https)
            .port(CONFIG.redirect.port, CONFIG.port);

        App::new()
            .wrap(Condition::new(
                CONFIG.redirect.enabled,
                Compat::new(redirect),
            ))
            // https://docs.rs/actix-web/4.0.0-beta.8/actix_web/middleware/struct.Logger.html
            .wrap(Logger::new(r#""%r" %s %b "%{Referer}i" %T"#))
            .wrap(Cors::default().allowed_origin_fn(move |origin, _| {
                if CONFIG.cors.origins.contains("*") {
                    return true;
                }
                let origin = match origin.to_str() {
                    Ok(origin) => origin,
                    // Browsers should always send a valid Origin.
                    // We don't care about invalid Origin sent from non-browser clients.
                    Err(_) => return false,
                };
                CONFIG.cors.origins.contains(origin)
            }))
            .wrap(
                ErrorHandlers::new()
                    .handler(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        error_handler::internal_server_error,
                    )
                    .handler(StatusCode::NOT_FOUND, error_handler::not_found),
            )
    };
    start_with_app(frontend, up_msg_handler, app, service_config).await
}

pub async fn start_with_app<FRB, FRBO, UPH, UPHO, UMsg, AT, AB, ABE>(
    frontend: FRB,
    up_msg_handler: UPH,
    app: impl Fn() -> App<AT> + Send + Sync + 'static,
    service_config: impl Fn(&mut web::ServiceConfig) + Send + Sync + 'static,
) -> io::Result<()>
where
    FRB: FrontBuilder<FRBO>,
    FRBO: FrontBuilderOutput,
    UPH: UpHandler<UPHO, UMsg>,
    UPHO: UpHandlerOutput,
    UMsg: 'static + DeserializeOwned,
    AT: ServiceFactory<
            ServiceRequest,
            Config = (),
            Response = ServiceResponse<AB>,
            Error = Error,
            InitError = (),
        > + 'static,
    AB: MessageBody<Error = ABE> + 'static,
    ABE: std::error::Error + 'static,
{
    // ------ Init ------

    println!("Moon config: {:?}", *CONFIG);

    env_logger::builder()
        .filter_level(CONFIG.backend_log_level)
        .init();

    let shared_data = SharedData {
        backend_build_id: backend_build_id().await,
        frontend_build_id: Frontend::build_id().await,
        cache_busting: CONFIG.cache_busting,
        compressed_pkg: CONFIG.compressed_pkg,
        pkg_path: "frontend/pkg",
    };
    let reload_sse = ReloadSSE(SSE::start());
    let message_sse = MessageSSE(SSE::start());
    let address = SocketAddr::from(([0, 0, 0, 0], CONFIG.port));

    let mut lazy_message_writer = LazyMessageWriter::new();

    let service_config = Arc::new(service_config);
    let service_config = move |config: &mut web::ServiceConfig| (service_config.clone())(config);

    let data_frontend = web::Data::new(frontend);
    let data_up_msg_handler = web::Data::new(up_msg_handler);
    let data_reload_sse = web::Data::new(reload_sse);
    let data_message_sse = web::Data::new(message_sse);

    let app = Arc::new(app);

    // ------ App ------

    let mut server = HttpServer::new(move || {
        (app.clone())()
            .app_data(web::Data::new(shared_data))
            .app_data(data_frontend.clone())
            .app_data(data_up_msg_handler.clone())
            .app_data(data_reload_sse.clone())
            .app_data(data_message_sse.clone())
            .configure(service_config.clone())
            .service(
                Files::new("_api/public", "public").default_handler(web::to(|| async {
                    HttpResponse::NotFound().reason("File Not Found").finish()
                })),
            )
            .service(
                web::scope("_api")
                    .route(
                        "up_msg_handler",
                        web::post().to(up_msg_handler_responder::<UPH, UPHO, UMsg>),
                    )
                    .route("reload", web::post().to(reload_responder))
                    .route("pkg/{file:.*}", web::get().to(pkg_responder))
                    .route(
                        "web_workers/{crate_name}/pkg/{file:.*}",
                        web::get().to(web_workers_responder),
                    )
                    .route(
                        "message_sse/{session_id}",
                        web::get().to(message_sse_responder),
                    )
                    .route("reload_sse", web::get().to(reload_sse_responder))
                    .route("ping", web::to(|| async { "pong" }))
                    .route(
                        "{path:.*}",
                        web::to(|| async {
                            HttpResponse::NotFound().reason("API Not Found").finish()
                        }),
                    ),
            )
            .default_service(web::get().to(frontend_responder::<FRB, FRBO>))
    });

    // ------ Bind ------

    server = if CONFIG.https {
        server.bind_rustls_021(address, rustls_server_config()?)?
    } else {
        server.bind(address)?
    };
    lazy_message_writer.server_is_running(&address, &CONFIG)?;

    server = if CONFIG.redirect.enabled {
        let address = SocketAddr::from(([0, 0, 0, 0], CONFIG.redirect.port));
        lazy_message_writer.redirect_from(&address, &CONFIG)?;
        server.bind(address)?
    } else {
        server
    };

    // ------ Run ------

    let server = server.run();
    if not(CONFIG.frontend_dist) {
        lazy_message_writer.write_all()?;
    }
    server.await?;

    Ok(println!("Stop Moon"))
}

async fn backend_build_id() -> u128 {
    fs::read_to_string("backend/private/build_id")
        .await
        .ok()
        .and_then(|uuid| uuid.parse().ok())
        .unwrap_or_default()
}

fn rustls_server_config() -> io::Result<RustlsServerConfig> {
    let key_file = &mut BufReader::new(File::open("backend/private/private.pem")?);
    let key = pkcs8_private_keys(key_file)
        .map(|key| {
            PrivateKey(
                key.expect("private key parsing failed")
                    .secret_pkcs8_der()
                    .to_vec(),
            )
        })
        .next()
        .expect("private key file has to contain at least one key");

    let cert_file = &mut BufReader::new(File::open("backend/private/public.pem")?);
    let certificates = certs(cert_file)
        .map(|cert| Certificate(cert.expect("certificate parsing failed").to_vec()))
        .collect();

    let config = RustlsServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .expect("private key is invalid");
    Ok(config)
}

// ------ ------
//  Responders
// ------ ------

// ------ up_msg_handler_responder ------

async fn up_msg_handler_responder<UPH, UPHO, UMsg>(
    req: HttpRequest,
    payload: web::Payload,
    up_msg_handler: web::Data<UPH>,
) -> Result<HttpResponse, Error>
where
    UPH: UpHandler<UPHO, UMsg>,
    UPHO: UpHandlerOutput,
    UMsg: DeserializeOwned,
{
    let headers = req.headers();

    let up_msg_request = UpMsgRequest {
        up_msg: parse_up_msg(payload).await?,
        session_id: parse_session_id(headers)?,
        cor_id: parse_cor_id(headers)?,
        auth_token: parse_auth_token(headers)?,
    };
    up_msg_handler.get_ref()(up_msg_request).await;
    Ok(HttpResponse::Ok().finish())
}

#[cfg(feature = "serde")]
async fn parse_up_msg<UMsg: DeserializeOwned>(mut payload: web::Payload) -> Result<UMsg, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_UP_MSG_BYTES {
            Err(error::JsonPayloadError::Overflow {
                limit: MAX_UP_MSG_BYTES,
            })?
        }
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&body).map_err(error::JsonPayloadError::Deserialize)?)
}

fn parse_session_id(headers: &HeaderMap) -> Result<SessionId, Error> {
    headers
        .get("X-Session-ID")
        .ok_or_else(|| error::ErrorBadRequest("header 'X-Session-ID' is missing"))?
        .to_str()
        .map_err(error::ErrorBadRequest)?
        .parse()
        .map_err(error::ErrorBadRequest)
}

fn parse_cor_id(headers: &HeaderMap) -> Result<CorId, Error> {
    headers
        .get("X-Correlation-ID")
        .ok_or_else(|| error::ErrorBadRequest("header 'X-Correlation-ID' is missing"))?
        .to_str()
        .map_err(error::ErrorBadRequest)?
        .parse()
        .map_err(error::ErrorBadRequest)
}

fn parse_auth_token(headers: &HeaderMap) -> Result<Option<AuthToken>, Error> {
    if let Some(auth_token) = headers.get("X-Auth-Token") {
        let auth_token = auth_token
            .to_str()
            .map_err(error::ErrorBadRequest)
            .map(AuthToken::new)?;
        return Ok(Some(auth_token));
    }
    Ok(None)
}

// ------ reload_responder ------

async fn reload_responder(sse: web::Data<ReloadSSE>) -> impl Responder {
    let _ = sse.broadcast("reload", "");
    HttpResponse::Ok()
}

// ------ pkg_responder ------

async fn pkg_responder(
    req: HttpRequest,
    file: web::Path<String>,
    shared_data: web::Data<SharedData>,
) -> impl Responder {
    if file.contains("..") {
        Err(error::ErrorForbidden(
            "It is not allowed to use '..' in the requested path",
        ))?;
    }

    let mime = mime_guess::from_path(file.as_str()).first_or_octet_stream();
    let (named_file, encoding) = named_file_and_encoding(&req, &file, &shared_data)?;

    let named_file = named_file
        .set_content_type(mime)
        .prefer_utf8(true)
        .use_etag(false)
        .use_last_modified(false)
        .disable_content_disposition()
        .customize();

    let mut responder = if shared_data.cache_busting {
        named_file.insert_header(CacheControl(vec![CacheDirective::MaxAge(31536000)]))
    } else {
        named_file.insert_header(ETag(EntityTag::new(
            false,
            shared_data.frontend_build_id.to_string(),
        )))
    };

    if let Some(encoding) = encoding {
        responder = responder.insert_header(encoding);
    }
    Ok::<_, Error>(responder)
}

fn named_file_and_encoding(
    req: &HttpRequest,
    file: &web::Path<String>,
    shared_data: &web::Data<SharedData>,
) -> Result<(NamedFile, Option<ContentEncoding>), Error> {
    let mut file = format!("{}/{}", shared_data.pkg_path, file);
    if !shared_data.compressed_pkg {
        return Ok((NamedFile::open(file)?, None));
    }
    let accept_encodings = req
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|accept_encoding| accept_encoding.to_str().ok())
        .map(|accept_encoding| accept_encoding.split(", ").collect::<BTreeSet<_>>())
        .unwrap_or_default();

    if accept_encodings.contains(ContentEncoding::Brotli.as_str()) {
        file.push_str(".br");
        let named_file = NamedFile::open(&file);
        if named_file.is_err() {
            eprintln!("Cannot load '{file}'. Consider to set `ENV COMPRESSED_PKG false` or build with `mzoon build -r`.");
        }
        return Ok((named_file?, Some(ContentEncoding::Brotli)));
    }
    if accept_encodings.contains(ContentEncoding::Gzip.as_str()) {
        file.push_str(".gz");
        let named_file = NamedFile::open(&file);
        if named_file.is_err() {
            eprintln!("Cannot load '{file}'. Consider to set `ENV COMPRESSED_PKG false` or build with `mzoon build -r`.");
        }
        return Ok((named_file?, Some(ContentEncoding::Gzip)));
    }
    Ok((NamedFile::open(file)?, None))
}

// ------ web_workers_responder ------

async fn web_workers_responder(
    req: HttpRequest,
    path_parameters: web::Path<(String, String)>,
    shared_data: web::Data<SharedData>,
) -> impl Responder {
    let (crate_name, file) = path_parameters.into_inner();

    // @TODO is it a proper solution? Or check whether it starts with `../` and `..\\`?
    if file.contains("..") {
        Err(error::ErrorForbidden(
            "It is not allowed to use '..' in the requested path",
        ))?;
    }

    let mime = mime_guess::from_path(&file).first_or_octet_stream();
    let (named_file, encoding) =
        web_worker_named_file_and_encoding(&req, &file, &shared_data, &crate_name)?;

    // @TODO set different cache headers because
    // it's a problem to clear Web Worker cache in Firefox?
    let named_file = named_file
        .set_content_type(mime)
        .prefer_utf8(true)
        .use_etag(false)
        .use_last_modified(false)
        .disable_content_disposition()
        .customize();

    let mut responder = if shared_data.cache_busting {
        named_file.insert_header(CacheControl(vec![CacheDirective::MaxAge(31536000)]))
    } else {
        named_file.insert_header(ETag(EntityTag::new(
            false,
            shared_data.frontend_build_id.to_string(),
        )))
    };

    if let Some(encoding) = encoding {
        responder = responder.insert_header(encoding);
    }
    Ok::<_, Error>(responder)
}

fn web_worker_named_file_and_encoding(
    req: &HttpRequest,
    file: &str,
    shared_data: &web::Data<SharedData>,
    crate_name: &str,
) -> Result<(NamedFile, Option<ContentEncoding>), Error> {
    let WorkspaceMember { mut path, .. } = web_worker_workspace_members()?
        .into_iter()
        .find(|member| member.name == crate_name)
        .ok_or_else(|| {
            error::ErrorNotFound(format!(
                "Failed to find Web Worker '{crate_name}' in the project workspace"
            ))
        })?;
    path.push("pkg");
    path.push(file);

    if !shared_data.compressed_pkg {
        return Ok((NamedFile::open(path)?, None));
    }
    let accept_encodings = req
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|accept_encoding| accept_encoding.to_str().ok())
        .map(|accept_encoding| accept_encoding.split(", ").collect::<BTreeSet<_>>())
        .unwrap_or_default();

    if accept_encodings.contains(ContentEncoding::Brotli.as_str()) {
        path.as_mut_os_string().push(".br");
        let named_file = NamedFile::open(&path);
        if named_file.is_err() {
            let path = path.display();
            eprintln!("Cannot load '{path}'. Consider to set `ENV COMPRESSED_PKG false` or build with `mzoon build -r`.");
        }
        return Ok((named_file?, Some(ContentEncoding::Brotli)));
    }
    if accept_encodings.contains(ContentEncoding::Gzip.as_str()) {
        path.as_mut_os_string().push(".gz");
        let named_file = NamedFile::open(&path);
        if named_file.is_err() {
            let path = path.display();
            eprintln!("Cannot load '{path}'. Consider to set `ENV COMPRESSED_PKG false` or build with `mzoon build -r`.");
        }
        return Ok((named_file?, Some(ContentEncoding::Gzip)));
    }
    Ok((NamedFile::open(path)?, None))
}

#[derive(Debug)]
struct WorkspaceMember {
    name: String,
    #[allow(dead_code)]
    version: String,
    path: PathBuf,
}

fn web_worker_workspace_members() -> Result<Vec<WorkspaceMember>, Error> {
    let package_repr_regex = Regex::new(
        r"^(?P<name>\S+)\s(?P<version>\S+)\s\(path\+file://(?P<path>\S+)\)$",
    )
    .map_err(|err| {
        eprintln!("Failed to create Regex for 'PackageId::repr': {err:#}");
        error::ErrorInternalServerError("Failed to create Regex for 'PackageId::repr'")
    })?;

    MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|err| {
            eprintln!("Failed to parse workspace Cargo metadata: {err:#}");
            error::ErrorInternalServerError("Failed to parse workspace Cargo metadata")
        })?
        .workspace_members
        .into_iter()
        .filter_map(|package_id| {
            let Some(captures) = package_repr_regex.captures(&package_id.repr) else {
                let error_message = format!("Failed to parse workspace member with {package_id:?}");
                eprintln!("{error_message}");
                return Some(Err(error::ErrorInternalServerError(error_message)));
            };
            let name = &captures["name"];
            name.ends_with("web_worker")
                .then(|| WorkspaceMember {
                    name: name.to_owned(),
                    version: captures["version"].to_owned(),
                    path: PathBuf::from(&captures["path"]),
                })
                .map(Ok)
        })
        .collect::<Result<_, _>>()
}

// ------ reload_sse_responder ------

async fn reload_sse_responder(
    sse: web::Data<ReloadSSE>,
    shared_data: web::Data<SharedData>,
) -> impl Responder {
    let (connection, event_stream) = sse.new_connection(None);
    let backend_build_id = shared_data.backend_build_id.to_string();

    if connection
        .send("backend_build_id", &backend_build_id)
        .is_err()
    {
        return HttpResponse::InternalServerError()
            .reason("sending backend_build_id failed")
            .finish();
    }

    HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_EVENT_STREAM))
        .insert_header(CacheControl(vec![CacheDirective::NoCache]))
        .streaming(event_stream)
}

// ------ message_sse_responder ------

async fn message_sse_responder(
    session_id: web::Path<String>,
    sse: web::Data<MessageSSE>,
) -> Result<HttpResponse, Error> {
    let session_id = session_id.parse().map_err(error::ErrorBadRequest)?;
    let (_, event_stream) = sse.new_connection(Some(session_id));
    SessionActor::create(session_id, MessageSSE::clone(&sse));

    Ok(HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_EVENT_STREAM))
        .insert_header(CacheControl(vec![CacheDirective::NoCache]))
        .streaming(event_stream))
}

// ------ frontend_responder ------

async fn frontend_responder<FRB, FRBO>(frontend: web::Data<FRB>) -> impl Responder
where
    FRB: FrontBuilder<FRBO>,
    FRBO: FrontBuilderOutput,
{
    let mut responder = HttpResponse::Ok();
    responder.content_type(ContentType::html());

    if CONFIG.frontend_multithreading {
        // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer#security_requirements
        responder
            .insert_header(("Cross-Origin-Opener-Policy", "same-origin"))
            .insert_header(("Cross-Origin-Embedder-Policy", "require-corp"));
    }

    responder.body(frontend.get_ref()().await.into_html().await)
}

// ====== ====== TESTS ====== ======

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{body, rt as actix_rt, test, web::Data};
    use const_format::concatcp;

    const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
    const FIXTURES_DIR: &str = concatcp!(MANIFEST_DIR, "/tests/fixtures");

    #[actix_rt::test]
    async fn test_uncompressed() {
        // ------ ARRANGE ------
        let css_content = include_str!("../tests/fixtures/index.css");

        let shared_data = SharedData {
            frontend_build_id: u128::default(),
            backend_build_id: u128::default(),
            cache_busting: bool::default(),
            compressed_pkg: false,
            pkg_path: FIXTURES_DIR,
        };
        let app = test::init_service(
            App::new()
                .app_data(Data::new(shared_data))
                .route("_api/pkg/{file:.*}", web::get().to(pkg_responder)),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/_api/pkg/index.css")
            .to_request();

        // ------ ACT ------
        let resp = test::call_service(&app, req).await;

        // ------ ASSERT ------
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            mime::TEXT_CSS_UTF_8.to_string()
        );
        assert_eq!(
            body::to_bytes(resp.into_body()).await.unwrap(),
            css_content.as_bytes()
        );
    }

    #[actix_rt::test]
    async fn test_brotli_compressed() {
        // ------ ARRANGE ------
        let css_content = web::Bytes::from_static(include_bytes!("../tests/fixtures/index.css.br"));

        let shared_data = SharedData {
            frontend_build_id: u128::default(),
            backend_build_id: u128::default(),
            cache_busting: bool::default(),
            compressed_pkg: true,
            pkg_path: FIXTURES_DIR,
        };
        let app = test::init_service(
            App::new()
                .app_data(Data::new(shared_data))
                .route("_api/pkg/{file:.*}", web::get().to(pkg_responder)),
        )
        .await;
        let req = test::TestRequest::get()
            .insert_header((header::ACCEPT_ENCODING, ContentEncoding::Br.as_str()))
            .uri("/_api/pkg/index.css")
            .to_request();

        // ------ ACT ------
        let resp = test::call_service(&app, req).await;

        // ------ ASSERT ------
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            mime::TEXT_CSS_UTF_8.to_string()
        );
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_ENCODING)
                .unwrap()
                .to_str()
                .unwrap(),
            ContentEncoding::Br.as_str()
        );
        assert_eq!(body::to_bytes(resp.into_body()).await.unwrap(), css_content,);
    }

    #[actix_rt::test]
    async fn test_gzip_compressed() {
        // ------ ARRANGE ------
        let css_content = web::Bytes::from_static(include_bytes!("../tests/fixtures/index.css.gz"));

        let shared_data = SharedData {
            frontend_build_id: u128::default(),
            backend_build_id: u128::default(),
            cache_busting: bool::default(),
            compressed_pkg: true,
            pkg_path: FIXTURES_DIR,
        };
        let app = test::init_service(
            App::new()
                .app_data(Data::new(shared_data))
                .route("_api/pkg/{file:.*}", web::get().to(pkg_responder)),
        )
        .await;
        let req = test::TestRequest::get()
            .insert_header((header::ACCEPT_ENCODING, ContentEncoding::Gzip.as_str()))
            .uri("/_api/pkg/index.css")
            .to_request();

        // ------ ACT ------
        let resp = test::call_service(&app, req).await;

        // ------ ASSERT ------
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            mime::TEXT_CSS_UTF_8.to_string()
        );
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_ENCODING)
                .unwrap()
                .to_str()
                .unwrap(),
            ContentEncoding::Gzip.as_str()
        );
        assert_eq!(body::to_bytes(resp.into_body()).await.unwrap(), css_content,);
    }
}
