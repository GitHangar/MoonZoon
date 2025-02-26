use crate::*;
use shared::{DownMsg, UpMsg};
use zoon::println;

#[static_ref]
pub fn connection() -> &'static Connection<UpMsg, DownMsg> {
    Connection::new(|down_msg, cor_id| {
        println!("DownMsg received: {:?}", down_msg);

        app::unfinished_mutations().update_mut(|cor_ids| {
            cor_ids.remove(&cor_id);
        });
        match down_msg {
            // ------ Auth ------
            DownMsg::LoginError(error) => login_page::set_login_error(error),
            DownMsg::LoggedIn(user) => login_page::set_and_store_logged_user(user),
            DownMsg::LoggedOut => app::on_logged_out_msg(),
            DownMsg::AccessDenied => (),
            // ------ Page data ------
            DownMsg::ClientsAndProjectsClients(clients) => {
                clients_and_projects_page::convert_and_set_clients(clients)
            }
            DownMsg::TimeBlocksClients(clients) => {
                time_blocks_page::convert_and_set_clients(clients)
            }
            DownMsg::TimeTrackerClients(clients) => {
                time_tracker_page::convert_and_set_clients(clients)
            }
            _ => (),
        }
    })
    .auth_token_getter(app::auth_token)
}
