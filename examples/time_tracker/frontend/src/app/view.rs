use crate::{
    router::Route,
    theme::{self, theme, toggle_theme, Theme},
};
use zoon::*;

pub fn root() -> impl Element {
    Column::new()
        .s(Height::screen())
        .item(header())
        .item(content())
}

pub fn content() -> impl Element {
    Column::new()
        .s(Height::fill())
        .s(Background::new().color_signal(theme::background_0()))
        .s(Scrollbars::y_and_clip_x())
        .on_viewport_size_change(super::on_viewport_size_change)
        .item_signal(super::page_id().signal().map(page))
}

fn header() -> impl Element {
    let hamburger_id = "hamburger";
    Row::with_tag(Tag::Nav)
        .s(Height::exact(64))
        .s(Background::new().color_signal(theme::background_1()))
        .s(Font::new().color_signal(theme::font_1()))
        .s(Shadows::with_signal(
            theme::shadow_2().map(|color| [Shadow::new().y(8).blur(16).color(color)]),
        ))
        .s(LayerIndex::new(1))
        .item(logo())
        .item_signal(
            super::wide_screen().map_true(|| Row::new().s(Height::fill()).items(menu_links(false))),
        )
        .item_signal(super::saving().signal().map_true(|| "Saving..."))
        .item(theme_button())
        .item_signal(super::wide_screen().map_true(auth_controls))
        .item_signal(super::wide_screen().map_false(|| hamburger(hamburger_id)))
        .element_below(
            El::new().child_signal(super::show_menu_panel().map_true(|| menu_panel(hamburger_id))),
        )
}

fn logo() -> impl Element {
    Link::new()
        .s(Height::fill())
        .s(Font::new().weight(FontWeight::Bold).size(32))
        .s(Padding::new().x(12))
        .to(Route::Root)
        .label(Row::new().s(Height::fill()).item("TT"))
}

fn theme_button() -> impl Element {
    let (hovered, hovered_signal) = Mutable::new_and_signal(false);
    El::new().s(Padding::new().x(10)).child(
        Button::new()
            .s(Font::new().color_signal(theme::font_5()))
            .s(
                Background::new().color_signal(hovered_signal.map_bool_signal(
                    || theme::background_5_highlighted(),
                    || theme::background_5(),
                )),
            )
            .s(Padding::all(5))
            .s(RoundedCorners::all_max())
            .on_hovered_change(move |is_hovered| hovered.set_neq(is_hovered))
            .on_press(toggle_theme)
            .label_signal(theme().signal().map(|theme| match theme {
                Theme::Light => "Dark",
                Theme::Dark => "Light",
            })),
    )
}

fn hamburger(id: &str) -> impl Element {
    let (hovered, hovered_signal) = Mutable::new_and_signal(false);
    Button::new()
        .id(id)
        .s(Height::fill())
        .s(Align::new().right())
        .s(
            Background::new().color_signal(hovered_signal.map_bool_signal(
                || theme::background_1_highlighted(),
                || theme::transparent(),
            )),
        )
        .s(Font::new().size(25))
        .s(Padding::new().bottom(4))
        .s(Width::exact(64))
        .on_press(super::toggle_menu)
        .on_hovered_change(move |is_hovered| hovered.set_neq(is_hovered))
        .label(
            Row::new()
                .s(Height::fill())
                .item_signal(super::menu_opened().signal().map_bool(|| "✕", || "☰")),
        )
}

fn menu_panel(hamburger_id: &str) -> impl Element {
    Column::new()
        .s(Background::new().color_signal(theme::background_0()))
        .s(Font::new().color_signal(theme::font_0()))
        .s(Height::exact(250))
        .s(Align::new().right())
        .s(Padding::all(15))
        .s(Shadows::with_signal(
            theme::shadow().map(|color| [Shadow::new().y(8).blur(16).color(color)]),
        ))
        .s(RoundedCorners::new().bottom(10))
        .on_click_outside_with_ids(super::close_menu, [hamburger_id])
        .after_remove(|_| super::close_menu())
        .items(menu_links(true))
        .item(El::new().s(Height::exact(10)))
        .item(auth_controls())
}

fn menu_links(in_menu_panel: bool) -> impl IntoIterator<Item = impl Element> {
    [
        menu_link(
            Route::TimeTracker,
            "Time Tracker",
            super::PageId::TimeTracker,
            in_menu_panel,
        ),
        menu_link(
            Route::ClientsAndProjects,
            "Clients & Projects",
            super::PageId::ClientsAndProjects,
            in_menu_panel,
        ),
        menu_link(
            Route::TimeBlocks,
            "Time Blocks",
            super::PageId::TimeBlocks,
            in_menu_panel,
        ),
    ]
}

fn menu_link(
    route: Route,
    label: &str,
    page_id: super::PageId,
    in_menu_panel: bool,
) -> impl Element {
    let (hovered, hovered_signal) = Mutable::new_and_signal(false);
    let hovered_or_selected = map_ref! {
        let hovered = hovered_signal,
        let current_page_id = super::page_id().signal() => move {
            *hovered || *current_page_id == page_id
        }
    };
    Link::new()
        .s(Height::fill())
        .s(Padding::new().x(12))
        .s(
            Background::new().color_signal(hovered_or_selected.map_true_signal(move || {
                if in_menu_panel {
                    theme::background_2_highlighted().left_either()
                } else {
                    theme::background_1_highlighted().right_either()
                }
            })),
        )
        .on_hovered_change(move |is_hovered| hovered.set_neq(is_hovered))
        .to(route)
        .label(Row::new().s(Height::fill()).item(label))
}

fn auth_controls() -> impl Element {
    Row::new()
        .s(Align::new().right())
        .s(Padding::new().x(12))
        .item_signal(super::is_user_logged_signal().map_false(login_button))
        .item_signal(super::is_user_logged_signal().map_true(logout_button))
}

fn login_button() -> impl Element {
    let (hovered, hovered_signal) = Mutable::new_and_signal(false);
    Link::new()
        .s(
            Background::new().color_signal(hovered_signal.map_bool_signal(
                || theme::background_3_highlighted(),
                || theme::background_3(),
            )),
        )
        .s(Font::new()
            .color_signal(theme::font_3())
            .weight(FontWeight::Bold))
        .s(Padding::new().x(15).y(10))
        .s(RoundedCorners::all(4))
        .on_hovered_change(move |is_hovered| hovered.set_neq(is_hovered))
        .to(Route::Login)
        .label("Log in")
}

fn logout_button() -> impl Element {
    let (hovered, hovered_signal) = Mutable::new_and_signal(false);
    Button::new()
        .s(
            Background::new().color_signal(hovered_signal.map_bool_signal(
                || theme::background_2_highlighted(),
                || theme::background_2(),
            )),
        )
        .s(Font::new().color_signal(theme::font_2()))
        .s(Padding::new().x(15).y(10))
        .s(RoundedCorners::all(4))
        .on_hovered_change(move |is_hovered| hovered.set_neq(is_hovered))
        .on_press(super::log_out)
        .label(
            Row::new()
                .item(
                    El::new()
                        .s(Font::new().weight(FontWeight::SemiBold))
                        .child("Log out "),
                )
                .item(super::logged_user_name()),
        )
}

fn page(page_id: super::PageId) -> impl Element {
    match page_id {
        super::PageId::Login => crate::login_page::view(),
        super::PageId::ClientsAndProjects => crate::clients_and_projects_page::view(),
        super::PageId::TimeTracker => crate::time_tracker_page::view(),
        super::PageId::TimeBlocks => crate::time_blocks_page::view(),
        super::PageId::Home => crate::home_page::view(),
        super::PageId::Unknown => El::new().child(404).into_raw(),
    }
}
