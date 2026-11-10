mod ui;

use std::sync::Arc;

use gpui::{
    App, AppContext, Application, Bounds, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, point, px, size,
};

use nestodo_desktop::api::parse_base_url;
use nestodo_desktop::prefs;
use nestodo_desktop::store::SystemStore;

use ui::icons::Assets;
use ui::outline_view::ToggleShowDone;
use ui::root::{
    Quit, Root, ShowShortcuts, ShowStats, ShowTodos, ask_before_leaving_unsaved_changes,
};
use ui::text_field::{Copy, Cut, Paste, SelectAll};

fn base_url() -> Result<url::Url, String> {
    let value = std::env::var("NESTODO_BASE_URL")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| option_env!("NESTODO_BASE_URL").map(str::to_string))
        .ok_or_else(|| "No server is set for this app.".to_string())?;
    parse_base_url(&value)
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys(ui::text_field::bindings());
    cx.bind_keys(ui::move_picker::bindings());
    cx.bind_keys(ui::outline_view::bindings());
    cx.bind_keys(ui::root::bindings());
}

fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu {
            name: "Nestodo".into(),
            items: vec![MenuItem::action("Quit Nestodo", Quit)],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Select All", SelectAll),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Todos", ShowTodos),
                MenuItem::action("Stats", ShowStats),
                MenuItem::separator(),
                MenuItem::action("Show Completed", ToggleShowDone),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![MenuItem::action("Keyboard Shortcuts", ShowShortcuts)],
        },
    ]);
}

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        bind_keys(cx);
        set_menus(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(900.), px(820.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Nestodo".into()),
                        appears_transparent: false,
                        traffic_light_position: Some(point(px(12.), px(12.))),
                    }),
                    window_min_size: Some(size(px(360.), px(400.))),
                    app_id: Some("io.github.yagipass.nestodo".into()),
                    ..Default::default()
                },
                |window, cx| {
                    let store = Arc::new(SystemStore::new());
                    let path = prefs::default_path();
                    cx.new(|cx| Root::new(base_url(), store, path, window, cx))
                },
            )
            .expect("the window opens");

        ask_before_leaving_unsaved_changes(window, cx);
        cx.activate(true);
    });
}
