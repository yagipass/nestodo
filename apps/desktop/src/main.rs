mod ui;

use std::sync::Arc;

use gpui::{
    App, AppContext, Bounds, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, point,
    px, size,
};

use nestodo_desktop::prefs;
use nestodo_desktop::store::SystemStore;

use ui::icons::Assets;
use ui::outline_view::ToggleShowDone;
use ui::root::{
    Quit, Root, ShowShortcuts, ShowStats, ShowTodos, ask_before_leaving_unsaved_changes,
};
use ui::text_field::{Copy, Cut, Paste, SelectAll};
use ui::widgets::MIN_WINDOW_WIDTH;

fn bind_keys(cx: &mut App) {
    cx.bind_keys(ui::text_field::bindings());
    cx.bind_keys(ui::move_picker::bindings());
    cx.bind_keys(ui::outline_view::bindings());
    cx.bind_keys(ui::root::bindings());
}

fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu::new("Nestodo").items([MenuItem::action("Quit Nestodo", Quit)]),
        Menu::new("Edit").items([
            MenuItem::action("Cut", Cut),
            MenuItem::action("Copy", Copy),
            MenuItem::action("Paste", Paste),
            MenuItem::action("Select All", SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Todos", ShowTodos),
            MenuItem::action("Stats", ShowStats),
            MenuItem::separator(),
            MenuItem::action("Show Completed", ToggleShowDone),
        ]),
        Menu::new("Help").items([MenuItem::action("Keyboard Shortcuts", ShowShortcuts)]),
    ]);
}

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            bind_keys(cx);
            set_menus(cx);
            cx.on_window_closed(|cx, _| {
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
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(20.), px(19.))),
                        }),
                        app_owns_titlebar_drag: true,
                        window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(400.))),
                        app_id: Some("io.github.yagipass.nestodo".into()),
                        ..Default::default()
                    },
                    |window, cx| {
                        let store = Arc::new(SystemStore::new());
                        let path = prefs::default_path();
                        cx.new(|cx| Root::new(store, path, window, cx))
                    },
                )
                .expect("the window opens");

            ask_before_leaving_unsaved_changes(window, cx);
            cx.activate(true);
        });
}
