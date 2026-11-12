use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, Entity, FocusHandle, Focusable, Font,
    FontFallbacks, FontWeight, KeyBinding, MouseButton, MouseDownEvent, Pixels, PromptLevel,
    SharedString, Subscription, WeakEntity, Window, WindowHandle, actions, canvas, deferred, div,
    img, prelude::*, px,
};
use url::Url;

use nestodo_desktop::access::{self, AccessApp};
use nestodo_desktop::api::{self, Client, Connection};
use nestodo_desktop::error::Error;
use nestodo_desktop::http::Http;
use nestodo_desktop::prefs::Prefs;
use nestodo_desktop::store::TokenStore;
use nestodo_desktop::wire::Todo;

use crate::ui::backend::Backend;
use crate::ui::icons::{APP_ICON, Icon};
use crate::ui::outline_view::{OutlineEvent, OutlineView, ToggleShowDone};
use crate::ui::settings_view::{SettingsEvent, SettingsView};
use crate::ui::shortcuts::shortcuts_dialog;
use crate::ui::stats_view::StatsView;
use crate::ui::text_field::{FieldEvent, TextField};
use crate::ui::theme::{FIGTREE, sync_theme, theme};
use crate::ui::widgets::{
    NAMED_TITLE_BAR_WIDTH, TITLE_BAR_HEIGHT, TRAFFIC_LIGHTS_WIDTH, icon, icon_button, modal, shadow,
};

actions!(
    nestodo,
    [
        ShowShortcuts,
        ShowStats,
        ShowSettings,
        ShowTodos,
        CloseDialog,
        CloseMenu,
        SubmitServer,
        CancelServerChange,
        Quit
    ]
);

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("escape", CloseDialog, Some("Dialog")),
        KeyBinding::new("escape", CloseMenu, Some("AccountMenu")),
        KeyBinding::new("enter", SubmitServer, Some("ServerForm")),
        KeyBinding::new("escape", CancelServerChange, Some("ServerForm")),
        KeyBinding::new("cmd-q", Quit, None),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Todos,
    Stats,
    Settings,
}

struct Workspace {
    client: Arc<Client>,
    page: Page,
    outline: Entity<OutlineView>,
    stats: Option<Entity<StatsView>>,
    settings: Option<(Entity<SettingsView>, Subscription)>,
    _subscription: Subscription,
}

struct ServerForm {
    field: Entity<TextField>,
    error: Option<String>,
    checking: bool,
    _subscription: Subscription,
}

enum Screen {
    NoServer(ServerForm),
    Connecting,
    Unavailable(String),
    SignedOut {
        app: AccessApp,
        error: Option<String>,
    },
    Waiting {
        app: AccessApp,
        cancel: Arc<AtomicBool>,
    },
    Ready(Workspace),
}

pub struct Root {
    base: Option<Url>,
    store: Arc<dyn TokenStore>,
    prefs_path: Option<PathBuf>,
    prefs: Prefs,
    screen: Screen,
    focus_handle: FocusHandle,
    menu_focus: FocusHandle,
    dialog_focus: FocusHandle,
    menu_open: bool,
    shortcuts_open: bool,
    account_bounds: Rc<Cell<Bounds<Pixels>>>,
    moving_window: bool,
    _subscriptions: Vec<Subscription>,
}

fn sentence(message: String) -> String {
    let mut chars = message.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => message,
    }
}

fn leave_when_saved(
    root: WeakEntity<Root>,
    window: &mut Window,
    cx: &mut App,
    leave: impl FnOnce(&mut Window, &mut App) + 'static,
) -> bool {
    let Some(entity) = root.upgrade() else {
        return true;
    };
    if !entity.read(cx).has_unsaved_changes(cx) {
        return true;
    }
    entity.update(cx, Root::flush);
    window
        .spawn(cx, async move |cx| {
            for _ in 0..50 {
                let (pending, failed) = cx
                    .update(|_, cx| {
                        root.upgrade().map_or((false, false), |root| {
                            let root = root.read(cx);
                            (root.has_unsaved_changes(cx), root.save_failed(cx))
                        })
                    })
                    .unwrap_or((false, false));
                if !pending {
                    cx.update(|window, cx| leave(window, cx)).ok();
                    return;
                }
                if failed {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
            let answer = cx
                .update(|window, cx| {
                    window.prompt(
                        PromptLevel::Warning,
                        "Some changes haven\u{2019}t been saved.",
                        Some("Leave anyway and lose them?"),
                        &["Stay", "Leave Anyway"],
                        cx,
                    )
                })
                .ok();
            if let Some(answer) = answer
                && answer.await.ok() == Some(1)
            {
                cx.update(|window, cx| leave(window, cx)).ok();
            }
        })
        .detach();
    false
}

pub fn ask_before_leaving_unsaved_changes(window: WindowHandle<Root>, cx: &mut App) {
    guard_leaving(window, cx, |cx| cx.quit());
}

fn guard_leaving(
    window: WindowHandle<Root>,
    cx: &mut App,
    quit: impl Fn(&mut App) + Clone + 'static,
) {
    window
        .update(cx, |_, window, cx| {
            let root = cx.entity().downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                leave_when_saved(root.clone(), window, cx, |window, _| {
                    window.remove_window();
                })
            });
        })
        .ok();
    cx.on_action(move |_: &Quit, cx| {
        let quit = quit.clone();
        cx.defer(move |cx| {
            let leave = quit.clone();
            let left = window
                .entity(cx)
                .and_then(|root| {
                    let root = root.downgrade();
                    AnyWindowHandle::from(window).update(cx, |_, window, cx| {
                        leave_when_saved(root, window, cx, move |_, cx| leave(cx))
                    })
                })
                .unwrap_or(true);
            if left {
                quit(cx);
            }
        });
    });
}

type Loaded = (Client, Vec<Todo>);

fn load(base: Url, store: Arc<dyn TokenStore>) -> Result<Result<Loaded, AccessApp>, Error> {
    let connected = Client::connect(base.clone(), store.clone(), api::now())?;
    let client = match connected {
        Connection::Ready(client) => *client,
        Connection::SignedOut(app) => return Ok(Err(app)),
    };
    match client.list() {
        Ok(todos) => Ok(Ok((client, todos))),
        Err(Error::NotSignedIn) => {
            api::logout(store.as_ref(), base.host_str().unwrap_or_default())?;
            match Client::connect(base, store, api::now())? {
                Connection::SignedOut(app) => Ok(Err(app)),
                Connection::Ready(_) => Err(Error::NotSignedIn),
            }
        }
        Err(error) => Err(error),
    }
}

fn rejection(error: &Error) -> String {
    match error {
        Error::Network { .. } => {
            "Can\u{2019}t reach this server. Check the address and your connection.".into()
        }
        Error::Storage(_) => sentence(error.to_string()),
        _ => "Couldn\u{2019}t find Nestodo at this address.".into(),
    }
}

impl Root {
    pub fn new(
        store: Arc<dyn TokenStore>,
        prefs_path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        sync_theme(window, cx);
        let prefs = Prefs::load(prefs_path.as_deref());
        let subscriptions = vec![
            cx.observe_window_appearance(window, |_, window, cx| {
                sync_theme(window, cx);
            }),
            cx.on_focus_lost(window, Self::restore_focus),
        ];
        let base = prefs
            .base_url
            .as_deref()
            .and_then(|saved| api::base_url_of(saved).ok());
        let mut root = Self {
            base,
            store,
            prefs_path,
            prefs,
            screen: Screen::Connecting,
            focus_handle: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            dialog_focus: cx.focus_handle(),
            menu_open: false,
            shortcuts_open: false,
            account_bounds: Rc::new(Cell::new(Bounds::default())),
            moving_window: false,
            _subscriptions: subscriptions,
        };
        window.focus(&root.focus_handle, cx);
        if root.base.is_some() {
            root.connect(window, cx);
        } else {
            let saved = root.prefs.base_url.clone().unwrap_or_default();
            root.ask_for_server(&saved, window, cx);
        }
        root
    }

    fn ask_for_server(&mut self, address: &str, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            let mut field = TextField::new("Server", cx).placeholder("nestodo.example.com");
            field.set_text(address, address.len(), cx);
            field
        });
        let subscription = cx.subscribe(&field, |_, _, _: &FieldEvent, cx| cx.notify());
        window.focus(&field.focus_handle(cx), cx);
        window.set_window_title("Nestodo");
        self.menu_open = false;
        self.screen = Screen::NoServer(ServerForm {
            field,
            error: None,
            checking: false,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn submit_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Screen::NoServer(form) = &mut self.screen else {
            return;
        };
        let field = form.field.read(cx);
        if form.checking || field.is_composing() || field.text().trim().is_empty() {
            return;
        }
        let base = match api::base_url_of(field.text()) {
            Ok(base) => base,
            Err(message) => {
                form.error = Some(message);
                cx.notify();
                return;
            }
        };
        form.error = None;
        form.checking = true;
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let checked = base.clone();
            let result = cx
                .background_spawn(async move { load(checked, store) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.server_checked(base, result, window, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn server_checked(
        &mut self,
        base: Url,
        result: Result<Result<Loaded, AccessApp>, Error>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::NoServer(form) = &mut self.screen else {
            return;
        };
        form.checking = false;
        cx.notify();
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(error) => {
                form.error = Some(rejection(&error));
                return;
            }
        };
        let previous = self.prefs.base_url.replace(base.to_string());
        if self.prefs.save(self.prefs_path.as_deref()).is_err() {
            self.prefs.base_url = previous;
            form.error = Some("Couldn\u{2019}t save the server. Try again.".into());
            return;
        }
        let left = self
            .base
            .take()
            .filter(|old| old.host_str() != base.host_str());
        self.base = Some(base);
        match left {
            Some(old) => {
                form.checking = true;
                self.sign_out_then_open(&old, loaded, window, cx);
            }
            None => self.open_checked(loaded, window, cx),
        }
    }

    fn open_checked(
        &mut self,
        loaded: Result<Loaded, AccessApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match loaded {
            Ok((client, todos)) => self.open_workspace(client, &todos, window, cx),
            Err(app) => {
                self.screen = Screen::SignedOut { app, error: None };
                window.focus(&self.focus_handle, cx);
            }
        }
    }

    fn sign_out_then_open(
        &self,
        old: &Url,
        loaded: Result<Loaded, AccessApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let host = old.host_str().unwrap_or_default().to_string();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { api::logout(store.as_ref(), &host) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.open_checked(loaded, window, cx);
                cx.notify();
                if let Err(error) = result {
                    let message = error.to_string();
                    drop(window.prompt(
                        PromptLevel::Critical,
                        "Couldn\u{2019}t sign out of the old server",
                        Some(&message),
                        &["OK"],
                        cx,
                    ));
                }
            })
            .ok();
        })
        .detach();
    }

    fn change_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_then(Self::edit_server, window, cx);
    }

    fn edit_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = &self.base else {
            return;
        };
        let address = api::label(base);
        self.ask_for_server(&address, window, cx);
    }

    fn cancel_server_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(&self.screen, Screen::NoServer(form) if !form.checking) {
            self.connect(window, cx);
        }
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = self.base.clone() else {
            return;
        };
        self.screen = Screen::Connecting;
        self.menu_open = false;
        self.shortcuts_open = false;
        window.set_window_title("Nestodo");
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { load(base, store) }).await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(Ok((client, todos))) => this.open_workspace(client, &todos, window, cx),
                    Ok(Err(app)) => this.screen = Screen::SignedOut { app, error: None },
                    Err(error) => this.screen = Screen::Unavailable(sentence(error.to_string())),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn open_workspace(
        &mut self,
        client: Client,
        todos: &[Todo],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let client = Arc::new(client);
        let backend: Arc<dyn Backend> = client.clone();
        let show_done = self.prefs.show_completed;
        let outline = cx.new(|cx| OutlineView::new(backend, todos, show_done, window, cx));
        let subscription =
            cx.subscribe_in(&outline, window, |this, _, event, window, cx| match event {
                OutlineEvent::ShowDoneChanged(shown) => {
                    this.prefs.show_completed = *shown;
                    let _ = this.prefs.save(this.prefs_path.as_deref());
                }
                OutlineEvent::ReloadRequested | OutlineEvent::SignedOut => this.connect(window, cx),
            });
        window.focus(&outline.focus_handle(cx), cx);
        window.set_window_title("Todos | Nestodo");
        self.screen = Screen::Ready(Workspace {
            client,
            page: Page::Todos,
            outline,
            stats: None,
            settings: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn sign_in(&mut self, app: AccessApp, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = self.base.clone() else {
            return;
        };
        let pending = access::start_login(&base, &app);
        cx.open_url(pending.url.as_str());
        let cancel = Arc::new(AtomicBool::new(false));
        self.screen = Screen::Waiting {
            app: app.clone(),
            cancel: cancel.clone(),
        };
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let flag = cancel.clone();
            let failed_app = app.clone();
            let result = cx
                .background_spawn(async move {
                    let http = Http::new();
                    access::finish_login(&http, &pending, store.as_ref(), api::now(), &flag)?;
                    let client = Client::signed_in(base, app, store, api::now())?;
                    let todos = client.list()?;
                    Ok::<_, Error>((client, todos))
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match result {
                    Ok((client, todos)) => this.open_workspace(client, &todos, window, cx),
                    Err(Error::Cancelled) => {}
                    Err(error) => {
                        this.screen = Screen::SignedOut {
                            app: failed_app,
                            error: Some(sentence(error.to_string())),
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if let Screen::Waiting { app, cancel } = &self.screen {
            cancel.store(true, Ordering::Relaxed);
            self.screen = Screen::SignedOut {
                app: app.clone(),
                error: None,
            };
            cx.notify();
        }
    }

    fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_then(Self::forget_login, window, cx);
    }

    fn leave_then(
        &mut self,
        then: fn(&mut Self, &mut Window, &mut Context<Self>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(window, cx);
        let root = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let leave = {
                let root = root.clone();
                move |window: &mut Window, cx: &mut App| {
                    root.update(cx, |this, cx| then(this, window, cx)).ok();
                }
            };
            if leave_when_saved(root, window, cx, leave.clone()) {
                leave(window, cx);
            }
        });
        cx.notify();
    }

    fn forget_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Screen::Ready(workspace) = &self.screen else {
            return;
        };
        let client = workspace.client.clone();
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { client.sign_out(store.as_ref()) })
                .await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(()) => this.connect(window, cx),
                Err(error) => {
                    let message = error.to_string();
                    drop(window.prompt(
                        PromptLevel::Critical,
                        "Couldn\u{2019}t sign out",
                        Some(&message),
                        &["OK"],
                        cx,
                    ));
                }
            })
            .ok();
        })
        .detach();
    }

    fn show_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = false;
        let Screen::Ready(workspace) = &mut self.screen else {
            return;
        };
        if workspace.page == page {
            self.restore_focus(window, cx);
            cx.notify();
            return;
        }
        workspace.page = page;
        match page {
            Page::Todos => {
                window.set_window_title("Todos | Nestodo");
                window.focus(&workspace.outline.focus_handle(cx), cx);
                workspace
                    .outline
                    .update(cx, super::outline_view::OutlineView::refresh);
            }
            Page::Stats => {
                window.set_window_title("Stats | Nestodo");
                workspace
                    .outline
                    .update(cx, super::outline_view::OutlineView::flush_now);
                let backend: Arc<dyn Backend> = workspace.client.clone();
                match &workspace.stats {
                    Some(stats) => stats.update(cx, super::stats_view::StatsView::reload),
                    None => workspace.stats = Some(cx.new(|cx| StatsView::new(backend, cx))),
                }
                window.focus(&self.focus_handle, cx);
            }
            Page::Settings => {
                window.set_window_title("Settings | Nestodo");
                workspace
                    .outline
                    .update(cx, super::outline_view::OutlineView::flush_now);
                if let Some((settings, _)) = &workspace.settings {
                    settings.update(cx, SettingsView::reload);
                } else {
                    let backend: Arc<dyn Backend> = workspace.client.clone();
                    let settings = cx.new(|cx| SettingsView::new(backend, cx));
                    let subscription = cx.subscribe_in(
                        &settings,
                        window,
                        |this, _, event: &SettingsEvent, window, cx| match event {
                            SettingsEvent::SignedOut => this.connect(window, cx),
                        },
                    );
                    workspace.settings = Some((settings, subscription));
                }
                window.focus(&self.focus_handle, cx);
            }
        }
        cx.notify();
    }

    fn open_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.shortcuts_open = true;
        window.focus(&self.dialog_focus, cx);
        cx.notify();
    }

    fn close_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shortcuts_open = false;
        self.restore_focus(window, cx);
        cx.notify();
    }

    fn restore_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.screen {
            Screen::Ready(workspace) if workspace.page == Page::Todos => {
                window.focus(&workspace.outline.focus_handle(cx), cx);
            }
            Screen::NoServer(form) => window.focus(&form.field.focus_handle(cx), cx),
            _ => window.focus(&self.focus_handle, cx),
        }
    }

    fn toggle_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        if self.menu_open {
            window.focus(&self.menu_focus, cx);
        } else {
            self.restore_focus(window, cx);
        }
        cx.notify();
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.restore_focus(window, cx);
        cx.notify();
    }

    fn on_close_menu(&mut self, _: &CloseMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
    }

    fn on_close_dialog(&mut self, _: &CloseDialog, window: &mut Window, cx: &mut Context<Self>) {
        self.close_shortcuts(window, cx);
    }

    fn modal_open(&self, cx: &App) -> bool {
        self.shortcuts_open
            || matches!(&self.screen, Screen::Ready(workspace)
                if workspace.page == Page::Todos && workspace.outline.read(cx).has_modal())
    }

    fn on_show_shortcuts(
        &mut self,
        _: &ShowShortcuts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.modal_open(cx) {
            self.open_shortcuts(window, cx);
        }
    }

    fn on_show_stats(&mut self, _: &ShowStats, window: &mut Window, cx: &mut Context<Self>) {
        if !self.modal_open(cx) {
            self.show_page(Page::Stats, window, cx);
        }
    }

    fn on_show_settings(&mut self, _: &ShowSettings, window: &mut Window, cx: &mut Context<Self>) {
        if !self.modal_open(cx) {
            self.show_page(Page::Settings, window, cx);
        }
    }

    fn on_show_todos(&mut self, _: &ShowTodos, window: &mut Window, cx: &mut Context<Self>) {
        if !self.modal_open(cx) {
            self.show_page(Page::Todos, window, cx);
        }
    }

    fn on_toggle_show_done(
        &mut self,
        _: &ToggleShowDone,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal_open(cx) {
            return;
        }
        if let Screen::Ready(workspace) = &self.screen
            && workspace.page == Page::Todos
        {
            workspace
                .outline
                .clone()
                .update(cx, |outline, cx| outline.toggle_show_done(window, cx));
        }
    }

    pub fn has_unsaved_changes(&self, cx: &App) -> bool {
        match &self.screen {
            Screen::Ready(workspace) => workspace.outline.read(cx).has_unsaved_changes(),
            _ => false,
        }
    }

    pub fn save_failed(&self, cx: &App) -> bool {
        match &self.screen {
            Screen::Ready(workspace) => workspace.outline.read(cx).save_failed(),
            _ => false,
        }
    }

    pub fn flush(&mut self, cx: &mut Context<Self>) {
        if let Screen::Ready(workspace) = &self.screen {
            workspace
                .outline
                .update(cx, super::outline_view::OutlineView::flush_now);
        }
    }

    fn render_menu(
        &mut self,
        workspace_email: Option<String>,
        page: Page,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(cx);
        let protected =
            matches!(&self.screen, Screen::Ready(workspace) if workspace.client.is_protected());
        let item = |id: &'static str, glyph: Icon, label: &'static str, current: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(10.))
                .w_full()
                .h(px(36.))
                .px(px(10.))
                .rounded(px(9.))
                .text_size(px(14.))
                .text_color(colors.text)
                .cursor_pointer()
                .when(current, |item| item.font_weight(FontWeight::SEMIBOLD))
                .hover(move |style| style.bg(colors.hover))
                .child(icon(
                    glyph,
                    18.,
                    if current { colors.accent } else { colors.muted },
                ))
                .child(label)
        };
        let separator = || div().h(px(1.)).mx(px(6.)).my(px(4.)).bg(colors.border);
        let account_bounds = self.account_bounds.clone();
        deferred(
            div()
                .id("account-menu")
                .key_context("AccountMenu")
                .track_focus(&self.menu_focus)
                .on_action(cx.listener(Self::on_close_menu))
                .absolute()
                .top(px(38.))
                .right_0()
                .w(px(236.))
                .p(px(6.))
                .border_1()
                .border_color(colors.border)
                .rounded(px(14.))
                .bg(colors.surface)
                .shadow(shadow(&colors, false))
                .occlude()
                .on_mouse_down_out(
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if !account_bounds.get().contains(&event.position) {
                            this.menu_open = false;
                            if this.menu_focus.is_focused(window) {
                                this.restore_focus(window, cx);
                            }
                            cx.notify();
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .px(px(10.))
                        .py(px(8.))
                        .text_size(px(14.))
                        .text_color(colors.text)
                        .children(workspace_email.map(|email| div().truncate().child(email)))
                        .children(self.base.as_ref().map(|base| {
                            div()
                                .truncate()
                                .text_size(px(12.))
                                .text_color(colors.muted)
                                .child(api::label(base))
                        })),
                )
                .child(separator())
                .child(
                    item("menu-stats", Icon::Stats, "Stats", page == Page::Stats).on_click(
                        cx.listener(|this, _, window, cx| this.show_page(Page::Stats, window, cx)),
                    ),
                )
                .child(
                    item(
                        "menu-settings",
                        Icon::Settings,
                        "Settings",
                        page == Page::Settings,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_page(Page::Settings, window, cx);
                    })),
                )
                .child(
                    item("menu-help", Icon::Help, "Help", false).on_click(
                        cx.listener(|this, _, window, cx| this.open_shortcuts(window, cx)),
                    ),
                )
                .child(separator())
                .child(
                    item("menu-change-server", Icon::Server, "Change server", false).on_click(
                        cx.listener(|this, _, window, cx| this.change_server(window, cx)),
                    ),
                )
                .when(protected, |menu| {
                    menu.child(
                        item("menu-sign-out", Icon::SignOut, "Sign out", false)
                            .on_click(cx.listener(|this, _, window, cx| this.sign_out(window, cx))),
                    )
                }),
        )
        .with_priority(2)
        .into_any_element()
    }

    fn render_title_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let bar = div()
            .id("title-bar")
            .debug_selector(|| "title-bar".into())
            .flex_none()
            .w_full()
            .h(px(TITLE_BAR_HEIGHT))
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(if window.is_fullscreen() {
                16.
            } else {
                TRAFFIC_LIGHTS_WIDTH
            }))
            .pr(px(12.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.moving_window = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.moving_window = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.moving_window = false),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if std::mem::take(&mut this.moving_window) {
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            });
        let Screen::Ready(workspace) = &self.screen else {
            return bar.into_any_element();
        };
        let page = workspace.page;
        let email = workspace.client.email();
        let outline = workspace.outline.clone();
        let tools = (page == Page::Todos)
            .then(|| outline.update(cx, |outline, cx| outline.render_tools(window, cx)));
        let menu = self.menu_open.then(|| self.render_menu(email, page, cx));
        let bounds = self.account_bounds.clone();
        let open = self.menu_open;
        let named = window.viewport_size().width >= px(NAMED_TITLE_BAR_WIDTH);
        bar.child(
            div()
                .id("brand")
                .debug_selector(|| "brand".into())
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, window, cx| {
                    this.show_page(Page::Todos, window, cx);
                }))
                .child(img(APP_ICON).size(px(24.)).flex_none())
                .when(named, |brand| {
                    brand.child(
                        div()
                            .font_family(FIGTREE)
                            .text_size(px(17.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(colors.text)
                            .child("Nestodo"),
                    )
                }),
        )
        .child(div().flex_1())
        .children(tools)
        .child(
            div()
                .relative()
                .flex()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    icon_button("account", Icon::Account, 19., cx)
                        .debug_selector(|| "account".into())
                        .when(open, |button| button.bg(colors.hover))
                        .child(
                            canvas(
                                move |bounds_now, _, _| bounds.set(bounds_now),
                                |_, (), _, _| {},
                            )
                            .absolute()
                            .size_full(),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_menu(window, cx);
                        })),
                )
                .children(menu),
        )
        .into_any_element()
    }

    fn render_server_line(&self, cx: &Context<Self>) -> AnyElement {
        let colors = theme(cx);
        div()
            .max_w_full()
            .flex()
            .items_center()
            .gap(px(4.))
            .children(self.base.as_ref().map(|base| {
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.))
                    .text_color(colors.muted)
                    .child(api::label(base))
            }))
            .child(
                div()
                    .id("change-server")
                    .flex_none()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(10.))
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.accent)
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.hover))
                    .on_click(cx.listener(|this, _, window, cx| this.change_server(window, cx)))
                    .child("Change server"),
            )
            .into_any_element()
    }

    fn render_login(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let hint = |text: &'static str| {
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .text_size(px(14.))
                .text_color(colors.muted)
                .children(text.split('\n').map(|line| div().child(line)))
        };
        let wide = |id: &'static str, label: &'static str, enabled: bool| {
            div()
                .id(id)
                .w_full()
                .h(px(52.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(16.))
                .bg(colors.accent)
                .text_color(colors.on_accent)
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .when(enabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.accent_strong))
                })
                .when(!enabled, |button| button.opacity(0.5))
                .child(label)
        };
        let link = |id: &'static str, label: &'static str, enabled: bool| {
            div()
                .id(id)
                .px(px(12.))
                .py(px(8.))
                .rounded(px(10.))
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.accent)
                .when(enabled, |link| {
                    link.cursor_pointer()
                        .hover(move |style| style.bg(colors.hover))
                })
                .when(!enabled, |link| link.opacity(0.5))
                .child(label)
        };
        let (error, actions): (Option<String>, AnyElement) = match &self.screen {
            Screen::NoServer(form) => {
                let ready = !form.checking && !form.field.read(cx).text().trim().is_empty();
                let cancel = self.base.is_some().then(|| {
                    link("cancel-server-change", "Cancel", !form.checking).when(
                        !form.checking,
                        |link| {
                            link.on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_server_change(window, cx);
                            }))
                        },
                    )
                });
                let label = if form.checking {
                    "Checking\u{2026}"
                } else {
                    "Continue"
                };
                (
                    None,
                    div()
                        .key_context("ServerForm")
                        .on_action(cx.listener(|this, _: &SubmitServer, window, cx| {
                            this.submit_server(window, cx);
                        }))
                        .on_action(cx.listener(|this, _: &CancelServerChange, window, cx| {
                            this.cancel_server_change(window, cx);
                        }))
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(44.))
                                        .flex()
                                        .items_center()
                                        .px(px(14.))
                                        .border_1()
                                        .border_color(if form.error.is_some() {
                                            colors.danger
                                        } else {
                                            colors.border
                                        })
                                        .rounded(px(10.))
                                        .bg(colors.surface)
                                        .text_size(px(16.))
                                        .line_height(px(22.))
                                        .child(div().flex_1().min_w_0().child(form.field.clone())),
                                )
                                .children(form.error.clone().map(|error| {
                                    div()
                                        .text_size(px(14.))
                                        .text_color(colors.danger)
                                        .child(error)
                                })),
                        )
                        .child(wide("continue", label, ready).when(ready, |button| {
                            button.on_click(cx.listener(|this, _, window, cx| {
                                this.submit_server(window, cx);
                            }))
                        }))
                        .child(hint("The address you open the web app at."))
                        .children(cancel)
                        .into_any_element(),
                )
            }
            Screen::Connecting | Screen::Ready(_) => (
                None,
                div()
                    .text_size(px(14.))
                    .text_color(colors.muted)
                    .child("Connecting\u{2026}")
                    .into_any_element(),
            ),
            Screen::Unavailable(message) => (
                Some(message.clone()),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        wide("retry", "Try again", true)
                            .on_click(cx.listener(|this, _, window, cx| this.connect(window, cx))),
                    )
                    .child(self.render_server_line(cx))
                    .into_any_element(),
            ),
            Screen::SignedOut { app, error } => {
                let app = app.clone();
                (
                    error.clone(),
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            wide("sign-in", "Sign in", true).on_click(cx.listener(
                                move |this, _, window, cx| this.sign_in(app.clone(), window, cx),
                            )),
                        )
                        .child(hint(
                            "Opens the Cloudflare Access sign-in in your browser.\nUse the same account as the web app.",
                        ))
                        .child(self.render_server_line(cx))
                        .into_any_element(),
                )
            }
            Screen::Waiting { .. } => (
                None,
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(16.))
                    .child(wide("signing-in", "Signing in\u{2026}", false))
                    .child(hint(
                        "In the browser, sign in and approve.\nCloudflare calls this app a command-line tool.",
                    ))
                    .child(
                        link("cancel-sign-in", "Cancel", true)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_sign_in(cx))),
                    )
                    .into_any_element(),
            ),
        };
        div()
            .size_full()
            .flex()
            .justify_center()
            .px(px(24.))
            .child(
                div()
                    .w_full()
                    .max_w(px(420.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .pb(px(40.))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(14.))
                            .child(img(APP_ICON).size(px(88.)))
                            .child(
                                div()
                                    .font_family(FIGTREE)
                                    .text_size(px(38.))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .child("Nestodo"),
                            )
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .text_color(colors.muted)
                                    .child("A todo list for everyone in the family"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(16.))
                            .children(error.map(|error| {
                                div()
                                    .w_full()
                                    .flex()
                                    .justify_center()
                                    .text_size(px(14.))
                                    .text_color(colors.danger)
                                    .child(SharedString::from(error))
                            }))
                            .child(actions),
                    ),
            )
            .into_any_element()
    }
}

fn body_font() -> Font {
    let mut body = gpui::font(".SystemUIFont");
    body.fallbacks = Some(FontFallbacks::from_fonts(vec!["Hiragino Sans".to_owned()]));
    body
}

impl Focusable for Root {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(cx);
        let content: AnyElement = match &self.screen {
            Screen::Ready(workspace) => {
                match (workspace.page, &workspace.stats, &workspace.settings) {
                    (Page::Stats, Some(stats), _) => stats.clone().into_any_element(),
                    (Page::Settings, _, Some((settings, _))) => settings.clone().into_any_element(),
                    _ => workspace.outline.clone().into_any_element(),
                }
            }
            _ => self.render_login(cx),
        };
        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_title_bar(window, cx))
            .child(div().flex_1().min_h_0().child(content));
        let dialog = self.shortcuts_open.then(|| {
            let close = cx.entity().downgrade();
            let content = div()
                .key_context("Dialog")
                .track_focus(&self.dialog_focus)
                .on_action(cx.listener(Self::on_close_dialog))
                .flex()
                .flex_col()
                .min_h_0()
                .child(shortcuts_dialog(
                    {
                        let close = close.clone();
                        move |window, cx| {
                            close
                                .update(cx, |this, cx| this.close_shortcuts(window, cx))
                                .ok();
                        }
                    },
                    cx,
                ))
                .into_any_element();
            modal(
                px(640.),
                content,
                move |window, cx| {
                    close
                        .update(cx, |this, cx| this.close_shortcuts(window, cx))
                        .ok();
                },
                window,
                cx,
            )
        });
        div()
            .id("root")
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_show_shortcuts))
            .on_action(cx.listener(Self::on_show_stats))
            .on_action(cx.listener(Self::on_show_settings))
            .on_action(cx.listener(Self::on_show_todos))
            .on_action(cx.listener(Self::on_toggle_show_done))
            .size_full()
            .bg(colors.bg)
            .text_color(colors.text)
            .font(body_font())
            .line_height(gpui::relative(1.25))
            .text_size(px(16.))
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .child(body)
            .children(dialog)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use gpui::{TestAppContext, VisualTestContext};
    use mockito::{Server, ServerGuard};
    use nestodo_desktop::store::Kind;

    use super::*;
    use crate::ui::widgets::MIN_WINDOW_WIDTH;

    #[derive(Default)]
    struct NoTokens {
        deleted: Mutex<Vec<String>>,
    }

    impl TokenStore for NoTokens {
        fn load(&self, _: &str, _: Kind) -> Result<Option<String>, Error> {
            Ok(None)
        }

        fn save(&self, _: &str, _: Kind, _: &str) -> Result<(), Error> {
            Ok(())
        }

        fn delete(&self, host: &str, _: Kind) -> Result<(), Error> {
            self.deleted.lock().unwrap().push(host.to_string());
            Ok(())
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "nestodo-desktop-root-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn settings(&self) -> PathBuf {
            self.0.join("settings.json")
        }

        fn saved_server(&self) -> Option<String> {
            Prefs::load(Some(&self.settings())).base_url
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Form {
        text: String,
        error: Option<String>,
        checking: bool,
    }

    fn refusing_saves() -> ServerGuard {
        let mut server = Server::new();
        server.mock("HEAD", "/").create();
        server
            .mock("GET", "/api/v1/todos")
            .with_body(
                r#"{"todos":[{"id":1,"parentId":null,"position":0,"title":"Milk","note":"","completedAt":null,"collapsed":false}]}"#,
            )
            .create();
        server
    }

    fn site(todos_status: usize) -> ServerGuard {
        let mut server = Server::new();
        server.mock("HEAD", "/").create();
        server
            .mock("GET", "/api/v1/todos")
            .with_status(todos_status)
            .with_body(r#"{"todos":[]}"#)
            .create();
        server
    }

    fn closed_port() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    }

    fn unreachable_localhost() -> String {
        closed_port().replace("127.0.0.1", "localhost")
    }

    fn launch(
        cx: &mut TestAppContext,
        prefs: PathBuf,
        store: Arc<NoTokens>,
    ) -> (Entity<Root>, &mut VisualTestContext) {
        cx.update(|cx| {
            cx.bind_keys(crate::ui::text_field::bindings());
            cx.bind_keys(crate::ui::outline_view::bindings());
            cx.bind_keys(crate::ui::settings_view::bindings());
            cx.bind_keys(bindings());
        });
        let (root, cx) = cx.add_window_view(|window, cx| Root::new(store, Some(prefs), window, cx));
        cx.run_until_parked();
        (root, cx)
    }

    fn remember(server: &ServerGuard, prefs: &TempDir) {
        remember_address(&server.url(), prefs);
    }

    fn remember_address(address: &str, prefs: &TempDir) {
        Prefs {
            show_completed: true,
            base_url: Some(address.to_string()),
        }
        .save(Some(&prefs.settings()))
        .unwrap();
    }

    fn open<'a>(
        cx: &'a mut TestAppContext,
        server: &ServerGuard,
        prefs: &TempDir,
    ) -> (Entity<Root>, Entity<OutlineView>, &'a mut VisualTestContext) {
        remember(server, prefs);
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());
        let outline = root.read_with(cx, |root, _| match &root.screen {
            Screen::Ready(workspace) => workspace.outline.clone(),
            _ => panic!("the workspace did not open"),
        });
        (root, outline, cx)
    }

    fn form(root: &Entity<Root>, cx: &mut VisualTestContext) -> Option<Form> {
        root.read_with(cx, |root, cx| match &root.screen {
            Screen::NoServer(form) => Some(Form {
                text: form.field.read(cx).text().to_string(),
                error: form.error.clone(),
                checking: form.checking,
            }),
            _ => None,
        })
    }

    fn enter_server(address: &str, cx: &mut VisualTestContext) {
        cx.simulate_input(address);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
    }

    fn edit_note(outline: &Entity<OutlineView>, cx: &mut VisualTestContext) {
        outline.update_in(cx, |outline, window, cx| {
            outline.open_note("t1", window, cx);
        });
        cx.run_until_parked();
        cx.simulate_input("x");
    }

    fn ask_before_leaving(cx: &mut VisualTestContext) -> Rc<Cell<bool>> {
        let window = cx.window_handle().downcast::<Root>().unwrap();
        let quit = Rc::new(Cell::new(false));
        let quitting = quit.clone();
        cx.cx
            .update(|cx| guard_leaving(window, cx, move |_| quitting.set(true)));
        quit
    }

    fn wait(cx: &mut VisualTestContext) {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn cmd_q_right_after_an_edit_the_server_refuses_asks_before_quitting_so_the_edit_is_not_lost_silently(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (_, outline, cx) = open(cx, &server, &prefs);
        let quit = ask_before_leaving(cx);
        edit_note(&outline, cx);

        cx.simulate_keystrokes("cmd-q");
        wait(cx);
        assert!(cx.has_pending_prompt());
        assert!(!quit.get());

        cx.simulate_prompt_answer("Leave Anyway");
        cx.run_until_parked();
        assert!(quit.get());
    }

    #[gpui::test]
    fn closing_the_window_right_after_an_edit_the_server_refuses_asks_before_closing_so_the_edit_is_not_lost_silently(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (_, outline, cx) = open(cx, &server, &prefs);
        ask_before_leaving(cx);
        edit_note(&outline, cx);

        let closed = cx.simulate_close();
        wait(cx);

        assert!(!closed);
        assert!(cx.has_pending_prompt());
    }

    #[gpui::test]
    fn the_view_menu_leaves_the_page_behind_the_shortcuts_dialog_alone_so_escape_still_closes_it(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (root, _, cx) = open(cx, &server, &prefs);
        cx.dispatch_action(ShowShortcuts);

        cx.dispatch_action(ShowStats);
        cx.dispatch_action(ShowTodos);

        root.update_in(cx, |root, window, _| {
            assert!(root.shortcuts_open);
            assert!(
                matches!(&root.screen, Screen::Ready(workspace) if workspace.page == Page::Todos)
            );
            assert!(root.dialog_focus.is_focused(window));
        });
        cx.simulate_keystrokes("escape");
        root.read_with(cx, |root, _| assert!(!root.shortcuts_open));
    }

    #[gpui::test]
    fn leaving_the_account_menu_through_any_item_gives_the_keyboard_back_to_the_page(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (root, outline, cx) = open(cx, &server, &prefs);

        root.update_in(cx, Root::toggle_menu);
        root.update_in(cx, Root::close_menu);
        outline.update_in(cx, |outline, window, cx| {
            assert!(outline.focus_handle(cx).contains_focused(window, cx));
        });

        root.update_in(cx, Root::toggle_menu);
        root.update_in(cx, |root, window, cx| {
            root.show_page(Page::Settings, window, cx);
        });
        root.update_in(cx, |root, window, _| {
            assert!(!root.menu_open);
            assert!(root.focus_handle.is_focused(window));
        });

        cx.dispatch_action(ShowStats);
        root.update_in(cx, Root::toggle_menu);
        root.update_in(cx, |root, window, cx| {
            root.show_page(Page::Stats, window, cx);
        });
        root.update_in(cx, |root, window, _| {
            assert!(root.focus_handle.is_focused(window));
        });
    }

    #[gpui::test]
    fn settings_open_in_the_app_like_stats_instead_of_sending_people_to_sign_in_again_in_the_browser(
        cx: &mut TestAppContext,
    ) {
        let mut server = refusing_saves();
        let webhook = server
            .mock("GET", "/api/v1/webhook")
            .with_body(r#"{"available":true,"webhook":null}"#)
            .create();
        let analytics = server
            .mock("GET", "/api/v1/analytics")
            .with_body(r#"{"available":true,"enabled":false}"#)
            .create();
        let prefs = TempDir::new();
        let (root, _, cx) = open(cx, &server, &prefs);

        cx.dispatch_action(ShowSettings);
        cx.run_until_parked();

        assert_eq!(cx.opened_url(), None);
        root.read_with(cx, |root, _| {
            assert!(
                matches!(&root.screen, Screen::Ready(workspace) if workspace.page == Page::Settings)
            );
        });
        assert!(cx.debug_bounds("usage-analytics").is_some());
        webhook.assert();
        analytics.assert();
    }

    fn press_with_a_wobble(selector: &'static str, cx: &mut VisualTestContext) {
        let at = cx
            .debug_bounds(selector)
            .expect("the control was drawn")
            .center();
        let moved = at + gpui::point(px(2.), px(1.));
        cx.simulate_mouse_down(at, MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_move(moved, MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_up(moved, MouseButton::Left, gpui::Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn a_title_bar_control_pressed_with_a_shaky_hand_still_works_instead_of_dragging_the_window(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (root, _, cx) = open(cx, &server, &prefs);
        cx.dispatch_action(ShowStats);
        cx.run_until_parked();

        press_with_a_wobble("brand", cx);
        root.read_with(cx, |root, _| {
            assert!(
                matches!(&root.screen, Screen::Ready(workspace) if workspace.page == Page::Todos)
            );
        });

        press_with_a_wobble("show-completed", cx);
        root.read_with(cx, |root, _| assert!(!root.prefs.show_completed));

        press_with_a_wobble("account", cx);
        root.read_with(cx, |root, _| assert!(root.menu_open));
    }

    #[gpui::test]
    fn a_title_bar_press_released_below_the_bar_does_not_move_the_window_when_the_mouse_next_passes_over_it(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (_, _, cx) = open(cx, &server, &prefs);
        let bar = cx
            .debug_bounds("title-bar")
            .expect("the title bar was drawn");
        let brand = cx.debug_bounds("brand").expect("the brand was drawn");
        let empty = gpui::point(brand.right() + px(40.), bar.center().y);
        let below = gpui::point(empty.x, bar.bottom() + px(40.));

        cx.simulate_mouse_down(empty, MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_move(below, MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_up(below, MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_move(empty, None, gpui::Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn the_title_bar_controls_neither_overlap_nor_leave_the_narrowest_window_while_the_filter_is_open(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (_, outline, cx) = open(cx, &server, &prefs);
        cx.simulate_resize(gpui::size(px(MIN_WINDOW_WIDTH), px(400.)));
        outline.update_in(cx, OutlineView::open_filter);
        cx.run_until_parked();

        let mut edge = px(0.);
        for control in ["brand", "filter", "show-completed", "account"] {
            let bounds = cx.debug_bounds(control).expect("the control was drawn");
            assert!(
                bounds.left() >= edge,
                "{control} starts at {:?}, over what ends at {edge:?}",
                bounds.left()
            );
            edge = bounds.right();
        }
        assert!(
            edge <= px(MIN_WINDOW_WIDTH),
            "the account button ends at {edge:?}, past the window's {MIN_WINDOW_WIDTH}px"
        );
    }

    #[gpui::test]
    fn a_reload_that_ends_on_an_error_screen_keeps_the_keyboard_in_the_window_so_the_help_menu_still_works(
        cx: &mut TestAppContext,
    ) {
        let mut server = refusing_saves();
        let prefs = TempDir::new();
        let (root, _, cx) = open(cx, &server, &prefs);
        server.reset();

        root.update_in(cx, Root::connect);
        cx.run_until_parked();

        root.update_in(cx, |root, window, cx| {
            assert!(matches!(root.screen, Screen::Unavailable(_)));
            assert!(window.is_action_available(&ShowShortcuts, cx));
        });
    }

    #[gpui::test]
    fn staying_after_sign_out_finds_unsaved_changes_gives_the_keyboard_back_to_the_outline(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (root, outline, cx) = open(cx, &server, &prefs);
        edit_note(&outline, cx);
        wait(cx);
        root.update_in(cx, Root::toggle_menu);

        root.update_in(cx, Root::sign_out);
        wait(cx);
        cx.simulate_prompt_answer("Stay");
        cx.run_until_parked();

        outline.update_in(cx, |outline, window, cx| {
            assert!(outline.focus_handle(cx).contains_focused(window, cx));
        });
    }

    #[gpui::test]
    fn a_first_launch_asks_for_the_server_with_the_keyboard_in_the_address_field(
        cx: &mut TestAppContext,
    ) {
        let prefs = TempDir::new();
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());

        root.update_in(cx, |root, window, cx| {
            let Screen::NoServer(form) = &root.screen else {
                panic!("the app did not ask for the server");
            };
            assert!(form.field.focus_handle(cx).is_focused(window));
        });
    }

    #[gpui::test]
    fn an_address_copied_from_the_browser_is_saved_as_the_servers_root_and_opens_with_the_todos_it_was_checked_with(
        cx: &mut TestAppContext,
    ) {
        let mut server = Server::new();
        server.mock("HEAD", "/").create();
        let todos = server
            .mock("GET", "/api/v1/todos")
            .with_body(r#"{"todos":[]}"#)
            .expect(1)
            .create();
        let prefs = TempDir::new();
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());

        enter_server(
            &format!("{}/today?from=https://example.com/", server.url()),
            cx,
        );

        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
        assert_eq!(prefs.saved_server(), Some(format!("{}/", server.url())));
        todos.assert();
    }

    #[gpui::test]
    fn an_address_that_cant_be_reached_is_not_saved_so_the_next_launch_asks_again_instead_of_failing_on_a_typo(
        cx: &mut TestAppContext,
    ) {
        let prefs = TempDir::new();
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());
        let address = closed_port();

        enter_server(&address, cx);

        let form = form(&root, cx).expect("the address form stays up");
        assert_eq!(form.text, address);
        assert_eq!(
            form.error.as_deref(),
            Some("Can\u{2019}t reach this server. Check the address and your connection.")
        );
        assert_eq!(prefs.saved_server(), None);
    }

    #[gpui::test]
    fn a_site_that_answers_without_a_todo_list_is_not_saved_since_the_outline_could_never_load_from_it(
        cx: &mut TestAppContext,
    ) {
        let server = site(404);
        let prefs = TempDir::new();
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());

        enter_server(&server.url(), cx);

        assert_eq!(
            form(&root, cx).and_then(|form| form.error).as_deref(),
            Some("Couldn\u{2019}t find Nestodo at this address.")
        );
        assert_eq!(prefs.saved_server(), None);
    }

    #[gpui::test]
    fn text_that_is_not_a_web_address_is_explained_under_the_field_without_trying_to_connect(
        cx: &mut TestAppContext,
    ) {
        let prefs = TempDir::new();
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());

        enter_server("my server", cx);

        let form = form(&root, cx).expect("the address form stays up");
        assert!(!form.checking);
        assert_eq!(
            form.error.as_deref(),
            Some("Enter an address such as nestodo.example.com.")
        );
    }

    #[gpui::test]
    fn the_address_form_stays_up_while_the_server_is_checked_and_a_second_enter_does_not_check_it_again(
        cx: &mut TestAppContext,
    ) {
        let mut server = Server::new();
        let discovery = server.mock("HEAD", "/").expect(1).create();
        server
            .mock("GET", "/api/v1/todos")
            .with_body(r#"{"todos":[]}"#)
            .create();
        let prefs = TempDir::new();
        let (root, cx) = launch(cx, prefs.settings(), Arc::default());
        cx.simulate_input(&server.url());

        root.update_in(cx, |root, window, cx| {
            root.submit_server(window, cx);
            root.submit_server(window, cx);
        });
        assert!(form(&root, cx).is_some_and(|form| form.checking));
        cx.run_until_parked();

        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
        discovery.assert();
    }

    #[gpui::test]
    fn a_server_the_mac_fails_to_save_is_reported_instead_of_opened_so_the_next_launch_does_not_ask_again_without_warning(
        cx: &mut TestAppContext,
    ) {
        let server = site(200);
        let prefs = TempDir::new();
        fs::create_dir_all(&prefs.0).unwrap();
        let blocker = prefs.0.join("not-a-folder");
        fs::write(&blocker, "").unwrap();
        let (root, cx) = launch(cx, blocker.join("settings.json"), Arc::default());

        enter_server(&server.url(), cx);

        assert_eq!(
            form(&root, cx).and_then(|form| form.error).as_deref(),
            Some("Couldn\u{2019}t save the server. Try again.")
        );
    }

    #[gpui::test]
    fn clicking_change_server_by_mistake_keeps_the_server_and_its_sign_in_so_escape_goes_back_without_signing_in_again(
        cx: &mut TestAppContext,
    ) {
        let server = site(200);
        let prefs = TempDir::new();
        remember(&server, &prefs);
        let store = Arc::new(NoTokens::default());
        let (root, cx) = launch(cx, prefs.settings(), store.clone());
        root.update_in(cx, Root::toggle_menu);

        root.update_in(cx, Root::change_server);
        cx.run_until_parked();
        assert_eq!(
            form(&root, cx).map(|form| form.text),
            Some(server.url()),
            "the form starts from the current address"
        );
        assert_eq!(prefs.saved_server(), Some(server.url()));

        cx.simulate_keystrokes("escape");
        cx.run_until_parked();

        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
        assert!(store.deleted.lock().unwrap().is_empty());
    }

    #[gpui::test]
    fn entering_another_server_signs_out_of_the_old_one_so_its_sign_in_is_not_left_on_the_mac(
        cx: &mut TestAppContext,
    ) {
        let server = site(200);
        let prefs = TempDir::new();
        remember_address(&unreachable_localhost(), &prefs);
        let store = Arc::new(NoTokens::default());
        let (root, cx) = launch(cx, prefs.settings(), store.clone());

        root.update_in(cx, Root::change_server);
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-a");
        enter_server(&server.url(), cx);

        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
        assert_eq!(prefs.saved_server(), Some(format!("{}/", server.url())));
        assert_eq!(*store.deleted.lock().unwrap(), ["localhost", "localhost"]);
    }

    #[gpui::test]
    fn the_new_server_opens_only_after_the_old_sign_in_is_deleted_so_switching_straight_back_cannot_lose_a_new_sign_in(
        cx: &mut TestAppContext,
    ) {
        let server = site(200);
        let prefs = TempDir::new();
        remember_address(&unreachable_localhost(), &prefs);
        let store = Arc::new(NoTokens::default());
        let (root, cx) = launch(cx, prefs.settings(), store.clone());
        let opened_early = Rc::new(Cell::new(false));
        cx.update(|_, cx| {
            let opened_early = opened_early.clone();
            let store = store.clone();
            cx.observe(&root, move |root, cx| {
                if matches!(root.read(cx).screen, Screen::Ready(_))
                    && store.deleted.lock().unwrap().is_empty()
                {
                    opened_early.set(true);
                }
            })
            .detach();
        });

        root.update_in(cx, Root::change_server);
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-a");
        enter_server(&server.url(), cx);

        assert!(
            !opened_early.get(),
            "the new server opened while the old sign-in was still saved"
        );
        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
    }

    #[gpui::test]
    fn entering_the_same_server_again_keeps_its_sign_in_instead_of_signing_out_of_it(
        cx: &mut TestAppContext,
    ) {
        let server = site(200);
        let prefs = TempDir::new();
        remember(&server, &prefs);
        let store = Arc::new(NoTokens::default());
        let (root, cx) = launch(cx, prefs.settings(), store.clone());

        root.update_in(cx, Root::change_server);
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
        assert!(store.deleted.lock().unwrap().is_empty());
    }

    #[gpui::test]
    fn the_server_can_be_changed_from_the_error_screen_when_it_stops_answering(
        cx: &mut TestAppContext,
    ) {
        let mut server = refusing_saves();
        let prefs = TempDir::new();
        let (root, _, cx) = open(cx, &server, &prefs);
        server.reset();
        root.update_in(cx, Root::connect);
        cx.run_until_parked();

        root.update_in(cx, Root::change_server);
        cx.run_until_parked();

        assert!(form(&root, cx).is_some());
    }

    #[gpui::test]
    fn another_server_the_mac_fails_to_save_keeps_the_current_one_and_its_sign_in_so_a_restart_does_not_return_to_a_server_already_signed_out_of(
        cx: &mut TestAppContext,
    ) {
        let server = site(200);
        let prefs = TempDir::new();
        remember_address(&unreachable_localhost(), &prefs);
        let store = Arc::new(NoTokens::default());
        let (root, cx) = launch(cx, prefs.settings(), store.clone());
        fs::remove_file(prefs.settings()).unwrap();
        fs::create_dir(prefs.settings()).unwrap();

        root.update_in(cx, Root::change_server);
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-a");
        enter_server(&server.url(), cx);

        assert_eq!(
            form(&root, cx).and_then(|form| form.error).as_deref(),
            Some("Couldn\u{2019}t save the server. Try again.")
        );
        assert!(store.deleted.lock().unwrap().is_empty());
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        root.read_with(cx, |root, _| {
            assert!(
                matches!(root.screen, Screen::Unavailable(_)),
                "going back reconnects to the old server, not the one that failed to save"
            );
        });
    }

    #[gpui::test]
    fn changing_the_server_right_after_an_edit_the_server_refuses_asks_first_so_the_edit_is_not_lost_silently(
        cx: &mut TestAppContext,
    ) {
        let server = refusing_saves();
        let prefs = TempDir::new();
        let (root, outline, cx) = open(cx, &server, &prefs);
        edit_note(&outline, cx);

        root.update_in(cx, Root::change_server);
        wait(cx);
        cx.simulate_prompt_answer("Stay");
        cx.run_until_parked();

        root.read_with(cx, |root, _| {
            assert!(matches!(root.screen, Screen::Ready(_)));
        });
        assert_eq!(prefs.saved_server(), Some(server.url()));
    }

    #[test]
    fn shows_lower_case_error_messages_as_sentences_on_the_sign_in_screen() {
        assert_eq!(
            sentence("could not reach http://127.0.0.1:5999/".into()),
            "Could not reach http://127.0.0.1:5999/"
        );
        assert_eq!(sentence(String::new()), "");
    }
}
