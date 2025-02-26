use crate::{
    calc_page,
    header::header,
    login_page, report_page,
    router::{previous_route, router, Route},
};
use zoon::*;

// ------ ------
//     Types
// ------ ------

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum PageId {
    Report,
    Login,
    Calc,
    Home,
    Unknown,
}

// ------ ------
//    States
// ------ ------

#[static_ref]
pub fn logged_user() -> &'static Mutable<Option<String>> {
    Mutable::new(None)
}

#[static_ref]
fn page_id() -> &'static Mutable<PageId> {
    Mutable::new(PageId::Unknown)
}

// ------ ------
//    Helpers
// ------ ------

pub fn is_user_logged() -> bool {
    logged_user().map(Option::is_some)
}

// ------ ------
//   Commands
// ------ ------

pub fn set_page_id(new_page_id: PageId) {
    page_id().set_neq(new_page_id);
}

pub fn log_in(name: String) {
    logged_user().set(Some(name));
    router().go(previous_route().unwrap_or(Route::Root));
}

pub fn log_out() {
    logged_user().take();
    router().go(Route::Root);
}

// ------ ------
//     View
// ------ ------

pub fn root() -> impl Element {
    Column::new()
        .s(Padding::all(20))
        .s(Gap::both(20))
        .item(header())
        .item(page())
}

fn page() -> impl Element {
    El::new().child_signal(page_id().signal().map(|page_id| match page_id {
        PageId::Report => report_page::page().into_raw(),
        PageId::Login => login_page::page().into_raw(),
        PageId::Calc => calc_page::page().into_raw(),
        PageId::Home => El::new().child("Welcome Home!").into_raw(),
        PageId::Unknown => El::new().child("404").into_raw(),
    }))
}
