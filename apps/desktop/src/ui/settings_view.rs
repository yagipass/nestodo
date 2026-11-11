use std::sync::Arc;

use gpui::{
    AnyElement, Context, Div, ElementId, Entity, EventEmitter, FontWeight, KeyBinding, PromptLevel,
    Stateful, Subscription, Window, actions, div, prelude::*, px,
};

use nestodo_desktop::error::Error;
use nestodo_desktop::webhook::{Analytics, Webhook, WebhookForm, WebhookHeader, Webhooks};

use crate::ui::backend::Backend;
use crate::ui::icons::Icon;
use crate::ui::text_field::{FieldEvent, TextField};
use crate::ui::theme::{Theme, theme};
use crate::ui::widgets::{PAGE_WIDTH, icon, icon_button};

actions!(settings, [SaveWebhook, CancelWebhook]);

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("enter", SaveWebhook, Some("WebhookForm")),
        KeyBinding::new("escape", CancelWebhook, Some("WebhookForm")),
    ]
}

const HIDDEN: &str = "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}";
const NEW_URL: &str = "https://example.com/hook";
const KEEP_URL: &str = "Leave blank to keep the saved URL";

pub enum SettingsEvent {
    SignedOut,
}

struct Loaded {
    webhooks: Webhooks,
    analytics: Analytics,
}

enum Load {
    Loading,
    Failed(String),
    Ready(Loaded),
}

struct HeaderFields {
    name: Entity<TextField>,
    value: Entity<TextField>,
}

struct Editor {
    form: WebhookForm,
    url: Entity<TextField>,
    token: Entity<TextField>,
    headers: Vec<HeaderFields>,
    project: Entity<TextField>,
    errors: Vec<(String, String)>,
    failure: Option<String>,
    saving: bool,
    subscriptions: Vec<Subscription>,
}

impl Editor {
    fn error(&self, field: &str) -> Option<String> {
        self.errors
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, message)| message.clone())
    }

    fn fields(&self) -> impl Iterator<Item = &Entity<TextField>> {
        [&self.url, &self.token, &self.project].into_iter().chain(
            self.headers
                .iter()
                .flat_map(|header| [&header.name, &header.value]),
        )
    }

    fn dirty(&self) -> bool {
        self.form != WebhookForm::of(self.form.existing.as_ref())
    }

    fn set_saving(&mut self, saving: bool, cx: &mut Context<SettingsView>) {
        self.saving = saving;
        for field in self.fields() {
            field.update(cx, |field, cx| field.set_read_only(saving, cx));
        }
    }
}

pub struct SettingsView {
    backend: Arc<dyn Backend>,
    load: Load,
    editor: Option<Editor>,
    analytics_saving: bool,
    analytics_failed: bool,
    generation: u64,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

fn field(text: &str, masked: bool, cx: &mut Context<SettingsView>) -> Entity<TextField> {
    cx.new(|cx| {
        let mut field = TextField::new("Settings", cx);
        field.set_masked(masked, cx);
        field.set_text(text, text.len(), cx);
        field
    })
}

fn watch(field: &Entity<TextField>, cx: &mut Context<SettingsView>) -> Subscription {
    cx.subscribe(field, |this, _, event: &FieldEvent, cx| {
        if matches!(event, FieldEvent::Changed { .. }) {
            this.sync(cx);
        }
    })
}

impl SettingsView {
    pub fn new(backend: Arc<dyn Backend>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            backend,
            load: Load::Loading,
            editor: None,
            analytics_saving: false,
            analytics_failed: false,
            generation: 0,
        };
        view.reload(cx);
        view
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.load = Load::Loading;
        self.editor = self.editor.take().filter(Editor::dirty);
        if let Some(editor) = &mut self.editor {
            editor.set_saving(false, cx);
        }
        self.analytics_saving = false;
        self.analytics_failed = false;
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    Ok::<_, Error>(Loaded {
                        webhooks: backend.webhooks()?,
                        analytics: backend.analytics()?,
                    })
                })
                .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(loaded) => this.show(loaded, cx),
                    Err(Error::NotSignedIn) => cx.emit(SettingsEvent::SignedOut),
                    Err(error) => this.load = Load::Failed(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn show(&mut self, loaded: Loaded, cx: &mut Context<Self>) {
        let empty = loaded.webhooks.available && loaded.webhooks.webhook.is_none();
        self.load = Load::Ready(loaded);
        if self.editor.as_ref().is_some_and(Editor::dirty) {
            return;
        }
        self.editor = None;
        if empty {
            self.open_editor(None, cx);
        }
    }

    fn webhook(&self) -> Option<&Webhook> {
        match &self.load {
            Load::Ready(loaded) => loaded.webhooks.webhook.as_ref(),
            _ => None,
        }
    }

    fn open_editor(&mut self, webhook: Option<&Webhook>, cx: &mut Context<Self>) {
        let form = WebhookForm::of(webhook);
        let url = field(&form.url, form.url_secret, cx);
        let token = field("", true, cx);
        let headers: Vec<HeaderFields> = form
            .headers
            .iter()
            .map(|header| HeaderFields {
                name: field(&header.name, false, cx),
                value: field(&header.value, header.secret, cx),
            })
            .collect();
        let project = field(&form.project, false, cx);
        let mut editor = Editor {
            form,
            url,
            token,
            headers,
            project,
            errors: Vec::new(),
            failure: None,
            saving: false,
            subscriptions: Vec::new(),
        };
        let fields: Vec<Entity<TextField>> = editor.fields().cloned().collect();
        editor.subscriptions = fields.iter().map(|field| watch(field, cx)).collect();
        self.editor = Some(editor);
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.form.url = editor.url.read(cx).text().to_string();
        let token = editor.token.read(cx).text().to_string();
        if editor.form.token.is_some() || !token.is_empty() {
            editor.form.token = Some(token);
        }
        for (header, fields) in editor.form.headers.iter_mut().zip(&editor.headers) {
            header.name = fields.name.read(cx).text().to_string();
            header.value = fields.value.read(cx).text().to_string();
        }
        editor.form.project = editor.project.read(cx).text().to_string();
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn refresh_placeholders(&self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let form = &editor.form;
        let url = if form.keeps_hidden_url() {
            KEEP_URL
        } else {
            NEW_URL
        };
        editor
            .url
            .update(cx, |field, cx| field.set_placeholder(url, cx));
        let token = if form.token_kept() {
            HIDDEN
        } else {
            "Optional"
        };
        editor
            .token
            .update(cx, |field, cx| field.set_placeholder(token, cx));
        for (header, fields) in form.headers.iter().zip(&editor.headers) {
            fields
                .name
                .update(cx, |field, cx| field.set_placeholder("Name", cx));
            let value = if form.value_kept(header) {
                HIDDEN
            } else {
                "Value"
            };
            fields
                .value
                .update(cx, |field, cx| field.set_placeholder(value, cx));
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>) {
        let webhook = self.webhook().cloned();
        if webhook.is_some() {
            self.open_editor(webhook.as_ref(), cx);
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.webhook().is_none() || self.editor.as_ref().is_some_and(|editor| editor.saving) {
            return;
        }
        self.editor = None;
        cx.notify();
    }

    fn toggle_url_secret(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.saving {
            return;
        }
        editor.form.url_secret = !editor.form.url_secret;
        let masked = editor.form.url_secret;
        editor
            .url
            .update(cx, |field, cx| field.set_masked(masked, cx));
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn remove_token(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.saving {
            return;
        }
        editor.form.token = None;
        editor
            .token
            .update(cx, |field, cx| field.set_text("", 0, cx));
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn add_header(&mut self, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_none_or(|editor| editor.saving) {
            return;
        }
        let fields = HeaderFields {
            name: field("", false, cx),
            value: field("", true, cx),
        };
        let watched = [watch(&fields.name, cx), watch(&fields.value, cx)];
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.form.headers.push(WebhookHeader {
            name: String::new(),
            value: String::new(),
            secret: true,
        });
        editor.headers.push(fields);
        editor.subscriptions.extend(watched);
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn toggle_header_secret(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.saving {
            return;
        }
        let (Some(header), Some(fields)) = (
            editor.form.headers.get_mut(index),
            editor.headers.get(index),
        ) else {
            return;
        };
        header.secret = !header.secret;
        let masked = header.secret;
        fields
            .value
            .update(cx, |field, cx| field.set_masked(masked, cx));
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn remove_header(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.saving {
            return;
        }
        if index < editor.headers.len() {
            editor.form.headers.remove(index);
            editor.headers.remove(index);
        }
        self.refresh_placeholders(cx);
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if editor.saving || editor.fields().any(|field| field.read(cx).is_composing()) {
            return;
        }
        editor.set_saving(true, cx);
        editor.errors.clear();
        editor.failure = None;
        let edit = editor.form.edit();
        let backend = self.backend.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { backend.save_webhook(&edit) })
                .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(webhooks) => this.saved(webhooks, cx),
                    Err(Error::NotSignedIn) => cx.emit(SettingsEvent::SignedOut),
                    Err(error) => {
                        if let Some(editor) = &mut this.editor {
                            editor.set_saving(false, cx);
                            match error {
                                Error::Refused(fields) => editor.errors = fields,
                                error => {
                                    editor.failure =
                                        Some(format!("Couldn\u{2019}t save the webhook: {error}"));
                                }
                            }
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

    fn saved(&mut self, webhooks: Webhooks, cx: &mut Context<Self>) {
        let Load::Ready(loaded) = &mut self.load else {
            return;
        };
        let empty = webhooks.available && webhooks.webhook.is_none();
        loaded.webhooks = webhooks;
        self.editor = None;
        if empty {
            self.open_editor(None, cx);
        }
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_none_or(|editor| editor.saving) {
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "Remove this webhook?",
            Some("Its URL, token and headers cannot be shown again."),
            &["Remove", "Cancel"],
            cx,
        );
        let backend = self.backend.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            if answer.await.ok() != Some(0) {
                return;
            }
            let started = this
                .update(cx, |this, cx| {
                    let Some(editor) = &mut this.editor else {
                        return false;
                    };
                    if this.generation != generation || editor.saving {
                        return false;
                    }
                    editor.set_saving(true, cx);
                    editor.failure = None;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !started {
                return;
            }
            let result = cx
                .background_spawn(async move { backend.remove_webhook() })
                .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(()) => {
                        if let Load::Ready(loaded) = &mut this.load {
                            loaded.webhooks.webhook = None;
                        }
                        this.open_editor(None, cx);
                    }
                    Err(Error::NotSignedIn) => cx.emit(SettingsEvent::SignedOut),
                    Err(error) => {
                        if let Some(editor) = &mut this.editor {
                            editor.set_saving(false, cx);
                            editor.failure =
                                Some(format!("Couldn\u{2019}t remove the webhook: {error}"));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_analytics(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.analytics_saving || !matches!(self.load, Load::Ready(_)) {
            return;
        }
        self.analytics_saving = true;
        self.analytics_failed = false;
        let backend = self.backend.clone();
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { backend.set_analytics(enabled) })
                .await;
            this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.analytics_saving = false;
                match result {
                    Ok(enabled) => {
                        if let Load::Ready(loaded) = &mut this.load {
                            loaded.analytics.enabled = enabled;
                        }
                    }
                    Err(Error::NotSignedIn) => cx.emit(SettingsEvent::SignedOut),
                    Err(_) => this.analytics_failed = true,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn on_save(&mut self, _: &SaveWebhook, _: &mut Window, cx: &mut Context<Self>) {
        self.save(cx);
    }

    fn on_cancel(&mut self, _: &CancelWebhook, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel(cx);
    }
}

fn card(colors: &Theme) -> Div {
    div()
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .border_1()
        .border_color(colors.border)
        .rounded(px(8.))
        .bg(colors.surface)
}

fn heading(text: &'static str) -> Div {
    div()
        .flex_1()
        .text_size(px(16.))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

fn note(text: impl Into<gpui::SharedString>, colors: &Theme) -> Div {
    div()
        .text_size(px(13.))
        .text_color(colors.muted)
        .child(text.into())
}

fn problem(text: impl Into<gpui::SharedString>, colors: &Theme) -> Div {
    div()
        .text_size(px(13.))
        .text_color(colors.danger)
        .child(text.into())
}

fn label(text: &'static str) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn field_box(field: &Entity<TextField>, invalid: bool, colors: &Theme) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .h(px(34.))
        .flex()
        .items_center()
        .px(px(10.))
        .border_1()
        .border_color(if invalid {
            colors.danger
        } else {
            colors.border
        })
        .rounded(px(6.))
        .bg(colors.bg)
        .text_size(px(14.))
        .line_height(px(20.))
        .child(div().flex_1().min_w_0().child(field.clone()))
}

fn pill(
    id: impl Into<ElementId>,
    text: &'static str,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let strong = colors.accent_strong;
    div()
        .id(id)
        .flex_none()
        .h(px(32.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(16.))
        .bg(colors.accent)
        .text_color(gpui::white())
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |pill| {
            pill.cursor_pointer().hover(move |style| style.bg(strong))
        })
        .when(!enabled, |pill| pill.opacity(0.5))
        .child(text)
}

fn text_button(
    id: impl Into<ElementId>,
    text: &'static str,
    danger: bool,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.hover;
    div()
        .id(id)
        .flex_none()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(6.))
        .text_size(px(14.))
        .text_color(if danger { colors.danger } else { colors.accent })
        .when(enabled, |button| {
            button.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .when(!enabled, |button| button.opacity(0.5))
        .child(text)
}

fn checkbox(
    id: impl Into<ElementId>,
    text: &'static str,
    checked: bool,
    colors: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .cursor_pointer()
        .child(
            div()
                .size(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(if checked {
                    colors.accent
                } else {
                    colors.border
                })
                .bg(if checked { colors.accent } else { colors.bg })
                .when(checked, |check| {
                    check.child(icon(Icon::Check, 12., gpui::white()))
                }),
        )
        .child(div().text_size(px(13.)).child(text))
}

impl SettingsView {
    fn render_status(webhook: &Webhook, colors: &Theme) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(
                webhook
                    .last_sent_at
                    .as_ref()
                    .map(|at| note(format!("Last sent {at} UTC."), colors)),
            )
            .children(
                webhook
                    .last_error
                    .as_ref()
                    .map(|error| problem(format!("The last send failed: {error}."), colors)),
            )
            .when(webhook.unreadable, |status| {
                status.child(problem(
                    "The saved URL, token and headers can no longer be read. Edit this webhook to enter them again.",
                    colors,
                ))
            })
    }

    fn render_view(webhook: &Webhook, colors: &Theme) -> Div {
        let row = |name: &'static str, value: String| {
            div()
                .flex()
                .gap(px(12.))
                .text_size(px(14.))
                .child(
                    div()
                        .w(px(80.))
                        .flex_none()
                        .text_color(colors.muted)
                        .child(name),
                )
                .child(div().flex_1().min_w_0().child(value))
        };
        let headers = if webhook.headers.is_empty() {
            "None".to_string()
        } else {
            webhook
                .headers
                .iter()
                .map(|header| {
                    let value = if header.secret {
                        HIDDEN
                    } else {
                        header.value.as_str()
                    };
                    format!("{}: {value}", header.name)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        div()
            .debug_selector(|| "webhook-view".into())
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(row(
                "URL",
                webhook.url.clone().unwrap_or_else(|| HIDDEN.to_string()),
            ))
            .child(row(
                "Token",
                if webhook.has_token { HIDDEN } else { "None" }.to_string(),
            ))
            .child(row("Headers", headers))
            .child(row("Project", webhook.project.clone()))
    }

    fn render_form(editor: &Editor, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let form = &editor.form;
        let saving = editor.saving;
        let url_error = editor.error("url");
        let token_error = editor.error("token");
        let project_error = editor.error("project");
        let headers = editor
            .headers
            .iter()
            .zip(&form.headers)
            .enumerate()
            .map(|(index, (fields, header))| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(field_box(&fields.name, false, &colors))
                    .child(field_box(&fields.value, false, &colors))
                    .child(
                        checkbox(("header-secret", index), "Secret", header.secret, &colors)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_header_secret(index, cx);
                            })),
                    )
                    .child(
                        icon_button(("remove-header", index), Icon::Close, 14., cx).on_click(
                            cx.listener(move |this, _, _, cx| this.remove_header(index, cx)),
                        ),
                    )
            })
            .collect::<Vec<_>>();
        let replacing = form.replacing();
        div()
            .key_context("WebhookForm")
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_cancel))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(label("URL"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(field_box(&editor.url, url_error.is_some(), &colors))
                            .child(
                                checkbox("url-secret", "Secret", form.url_secret, &colors)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_url_secret(cx);
                                    })),
                            ),
                    )
                    .children(url_error.map(|error| problem(error, &colors)))
                    .when(replacing, |field| {
                        field.child(note(
                            "A new URL drops the saved token and asks for secret header values again, so they never reach another server.",
                            &colors,
                        ))
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(label("Token"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(field_box(&editor.token, token_error.is_some(), &colors))
                            .when(form.token_kept(), |row| {
                                row.child(
                                    icon_button("remove-token", Icon::Close, 14., cx).on_click(
                                        cx.listener(|this, _, _, cx| this.remove_token(cx)),
                                    ),
                                )
                            }),
                    )
                    .children(token_error.map(|error| problem(error, &colors)))
                    .child(note(
                        "Sent as a bearer token and never shown again.",
                        &colors,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(label("Headers"))
                    .children(headers)
                    .child(
                        div().flex().child(
                            text_button("add-header", "Add header", false, true, &colors)
                                .on_click(cx.listener(|this, _, _, cx| this.add_header(cx))),
                        ),
                    )
                    .children(
                        editor
                            .error("headers")
                            .map(|error| problem(error, &colors)),
                    )
                    .child(note(
                        if replacing {
                            "A secret value is never shown again. Leave it blank to keep it."
                        } else {
                            "A secret value is never shown again."
                        },
                        &colors,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(label("Project"))
                    .child(div().flex().child(field_box(
                        &editor.project,
                        project_error.is_some(),
                        &colors,
                    )))
                    .children(project_error.map(|error| problem(error, &colors)))
                    .child(note(
                        "The title of a top-level todo. Only todos under it are sent.",
                        &colors,
                    )),
            )
            .children(editor.failure.clone().map(|failure| problem(failure, &colors)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        pill("save-webhook", "Save", !saving, &colors)
                            .when(!saving, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| this.save(cx)))
                            }),
                    )
                    .when(form.existing.is_some(), |row| {
                        row.child(
                            text_button("cancel-webhook", "Cancel", false, !saving, &colors)
                                .when(!saving, |button| {
                                    button.on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
                                }),
                        )
                        .child(
                            text_button("remove-webhook", "Remove", true, !saving, &colors)
                                .when(!saving, |button| {
                                    button.on_click(cx.listener(|this, _, window, cx| {
                                        this.remove(window, cx);
                                    }))
                                }),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_webhook(&self, loaded: &Loaded, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let available = loaded.webhooks.available;
        let webhook = loaded.webhooks.webhook.as_ref();
        let viewing = available && webhook.is_some() && self.editor.is_none();
        let content: AnyElement = if available {
            let shown = match (&self.editor, webhook) {
                (Some(editor), _) => Self::render_form(editor, cx),
                (None, Some(webhook)) => Self::render_view(webhook, &colors).into_any_element(),
                (None, None) => div().into_any_element(),
            };
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .children(webhook.map(|webhook| Self::render_status(webhook, &colors)))
                .child(shown)
                .into_any_element()
        } else {
            problem(
                "Webhooks are not available because WEBHOOK_SECRET_KEY is not set on this server.",
                &colors,
            )
            .into_any_element()
        };
        card(&colors)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(heading("Webhook"))
                    .when(viewing, |row| {
                        row.child(
                            pill("edit-webhook", "Edit", true, &colors)
                                .on_click(cx.listener(|this, _, _, cx| this.edit(cx))),
                        )
                    }),
            )
            .child(note(
                "Within 13 minutes of adding a todo with no note or only a link, its id, title and note are sent to this URL.",
                &colors,
            ))
            .child(content)
            .into_any_element()
    }

    fn render_analytics(&self, analytics: Analytics, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let saving = self.analytics_saving;
        card(&colors)
            .debug_selector(|| "usage-analytics".into())
            .child(div().flex().child(heading("Usage analytics")))
            .child(note(
                "This setting belongs to your account and applies to the web app on every device.",
                &colors,
            ))
            .when(self.analytics_failed, |card| {
                card.child(problem(
                    "The setting could not be saved. Try again.",
                    &colors,
                ))
            })
            .child(
                div().flex().child(
                    checkbox(
                        "send-usage-data",
                        "Send usage data",
                        analytics.enabled,
                        &colors,
                    )
                    .when(saving, |checkbox| checkbox.opacity(0.5))
                    .when(!saving, |checkbox| {
                        checkbox.on_click(cx.listener(move |this, _, _, cx| {
                            this.set_analytics(!analytics.enabled, cx);
                        }))
                    }),
                ),
            )
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(cx);
        let body: AnyElement = match &self.load {
            Load::Loading => div()
                .text_size(px(14.))
                .text_color(colors.muted)
                .child("Loading\u{2026}")
                .into_any_element(),
            Load::Failed(message) => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(14.))
                .text_color(colors.danger)
                .child(format!("Couldn\u{2019}t load the settings: {message}"))
                .child(
                    div()
                        .id("retry-settings")
                        .text_color(colors.accent)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                        .child("Try again"),
                )
                .into_any_element(),
            Load::Ready(loaded) => div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(self.render_webhook(loaded, cx))
                .when(loaded.analytics.available, |page| {
                    page.child(self.render_analytics(loaded.analytics, cx))
                })
                .into_any_element(),
        };
        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .px(px(16.))
            .pb(px(24.))
            .text_color(colors.text)
            .child(
                div()
                    .w_full()
                    .max_w(px(PAGE_WIDTH))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .font_weight(FontWeight::BOLD)
                            .line_height(px(36.))
                            .child("Settings"),
                    )
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Mutex;

    use gpui::{Focusable, TestAppContext, VisualTestContext};
    use nestodo_desktop::api::StatsResponse;
    use nestodo_desktop::webhook::WebhookEdit;
    use nestodo_desktop::wire::{Reply, Request, Todo};

    use super::*;
    use crate::ui::theme::Theme;

    struct State {
        webhooks: Webhooks,
        analytics: Analytics,
        saved: Vec<WebhookEdit>,
        removed: usize,
        refuse: Option<Vec<(String, String)>>,
        busy: bool,
        signed_out: bool,
    }

    struct Server(Mutex<State>);

    impl Server {
        fn with(webhook: Option<Webhook>) -> Arc<Self> {
            Arc::new(Self(Mutex::new(State {
                webhooks: Webhooks {
                    available: true,
                    webhook,
                },
                analytics: Analytics {
                    available: true,
                    enabled: false,
                },
                saved: Vec::new(),
                removed: 0,
                refuse: None,
                busy: false,
                signed_out: false,
            })))
        }

        fn state(&self) -> std::sync::MutexGuard<'_, State> {
            self.0.lock().unwrap()
        }

        fn answer(&self) -> Result<(), Error> {
            let state = self.state();
            if state.signed_out {
                return Err(Error::NotSignedIn);
            }
            if state.busy {
                return Err(Error::Server {
                    status: 503,
                    message: "The database is busy".into(),
                });
            }
            Ok(())
        }
    }

    impl Backend for Server {
        fn list(&self) -> Result<Vec<Todo>, Error> {
            Ok(Vec::new())
        }

        fn request(&self, _: &Request) -> Result<Reply, Error> {
            Err(Error::Invalid("not used".into()))
        }

        fn stats(&self, _: &str) -> Result<StatsResponse, Error> {
            Err(Error::Invalid("not used".into()))
        }

        fn webhooks(&self) -> Result<Webhooks, Error> {
            self.answer()?;
            Ok(self.state().webhooks.clone())
        }

        fn save_webhook(&self, edit: &WebhookEdit) -> Result<Webhooks, Error> {
            self.state().saved.push(edit.clone());
            self.answer()?;
            let mut state = self.state();
            if let Some(fields) = state.refuse.take() {
                return Err(Error::Refused(fields));
            }
            state.webhooks.webhook = Some(Webhook {
                url: (!edit.url_secret).then(|| edit.url.clone()),
                has_token: edit.token.as_deref().is_some_and(|token| !token.is_empty()),
                unreadable: false,
                headers: edit
                    .headers
                    .iter()
                    .map(|header| WebhookHeader {
                        value: if header.secret {
                            String::new()
                        } else {
                            header.value.clone()
                        },
                        ..header.clone()
                    })
                    .collect(),
                project: edit.project.clone(),
                last_sent_at: None,
                last_error: None,
            });
            Ok(state.webhooks.clone())
        }

        fn remove_webhook(&self) -> Result<(), Error> {
            self.answer()?;
            let mut state = self.state();
            state.removed += 1;
            state.webhooks.webhook = None;
            Ok(())
        }

        fn analytics(&self) -> Result<Analytics, Error> {
            self.answer()?;
            Ok(self.state().analytics)
        }

        fn set_analytics(&self, enabled: bool) -> Result<bool, Error> {
            self.answer()?;
            self.state().analytics.enabled = enabled;
            Ok(enabled)
        }
    }

    fn saved_webhook() -> Webhook {
        Webhook {
            url: None,
            has_token: true,
            unreadable: false,
            headers: vec![WebhookHeader {
                name: "X-Api-Key".into(),
                value: String::new(),
                secret: true,
            }],
            project: "Side projects".into(),
            last_sent_at: None,
            last_error: None,
        }
    }

    fn open(
        cx: &mut TestAppContext,
        server: Arc<Server>,
    ) -> (Entity<SettingsView>, &mut VisualTestContext) {
        cx.update(|cx| {
            cx.set_global(Theme::light());
            cx.bind_keys(crate::ui::text_field::bindings());
            cx.bind_keys(bindings());
        });
        let (view, cx) = cx.add_window_view(|_, cx| SettingsView::new(server, cx));
        cx.run_until_parked();
        (view, cx)
    }

    fn field_of(
        view: &Entity<SettingsView>,
        cx: &mut VisualTestContext,
        pick: impl Fn(&Editor) -> Entity<TextField>,
    ) -> Entity<TextField> {
        view.read_with(cx, |view, _| {
            pick(view.editor.as_ref().expect("the form is open"))
        })
    }

    fn type_into(
        view: &Entity<SettingsView>,
        cx: &mut VisualTestContext,
        pick: impl Fn(&Editor) -> Entity<TextField>,
        text: &str,
    ) {
        let field = field_of(view, cx, pick);
        cx.update(|window, cx| window.focus(&field.focus_handle(cx), cx));
        cx.simulate_input(text);
        cx.run_until_parked();
    }

    fn placeholder(field: &Entity<TextField>, cx: &mut VisualTestContext) -> String {
        field.read_with(cx, |field, _| field.placeholder_text().to_string())
    }

    #[gpui::test]
    fn a_webhook_set_up_in_the_app_is_sent_as_typed_and_then_shown_saved(cx: &mut TestAppContext) {
        let server = Server::with(None);
        let (view, cx) = open(cx, server.clone());

        type_into(
            &view,
            cx,
            |editor| editor.url.clone(),
            "https://example.com/hook",
        );
        type_into(&view, cx, |editor| editor.token.clone(), "e2e-token");
        type_into(&view, cx, |editor| editor.project.clone(), "Work");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        assert_eq!(
            server.state().saved,
            [WebhookEdit {
                url: "https://example.com/hook".into(),
                url_secret: true,
                token: Some("e2e-token".into()),
                headers: Vec::new(),
                project: "Work".into(),
            }]
        );
        assert!(view.read_with(cx, |view, _| view.editor.is_none()));
        assert!(cx.debug_bounds("webhook-view").is_some());
    }

    #[gpui::test]
    fn a_new_webhook_is_typed_behind_dots_until_secret_is_turned_off_so_the_url_can_be_checked(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(None);
        let (view, cx) = open(cx, server.clone());
        let url = field_of(&view, cx, |editor| editor.url.clone());
        assert!(url.read_with(cx, |field, _| field.is_masked()));

        view.update(cx, SettingsView::toggle_url_secret);
        type_into(
            &view,
            cx,
            |editor| editor.url.clone(),
            "https://example.com/hook",
        );
        type_into(&view, cx, |editor| editor.project.clone(), "Work");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        assert!(!url.read_with(cx, |field, _| field.is_masked()));
        assert!(!server.state().saved[0].url_secret);
    }

    #[gpui::test]
    fn a_value_the_server_refuses_stays_in_the_form_with_the_message_under_its_field(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(None);
        server.state().refuse = Some(vec![("url".into(), "Enter an http or https URL".into())]);
        let (view, cx) = open(cx, server.clone());

        type_into(&view, cx, |editor| editor.url.clone(), "ftp://example.com");
        type_into(&view, cx, |editor| editor.project.clone(), "Work");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        let (error, typed, saving) = view.read_with(cx, |view, cx| {
            let editor = view.editor.as_ref().expect("the form stays open");
            (
                editor.error("url"),
                editor.url.read(cx).text().to_string(),
                editor.saving,
            )
        });
        assert_eq!(error.as_deref(), Some("Enter an http or https URL"));
        assert_eq!(typed, "ftp://example.com");
        assert!(!saving);
    }

    #[gpui::test]
    fn a_save_that_does_not_reach_the_server_keeps_the_form_and_says_why(cx: &mut TestAppContext) {
        let server = Server::with(None);
        let (view, cx) = open(cx, server.clone());
        server.state().busy = true;

        type_into(
            &view,
            cx,
            |editor| editor.url.clone(),
            "https://example.com/hook",
        );
        type_into(&view, cx, |editor| editor.project.clone(), "Work");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        let failure = view.read_with(cx, |view, _| {
            view.editor
                .as_ref()
                .and_then(|editor| editor.failure.clone())
        });
        assert!(failure.is_some_and(|failure| failure.contains("The database is busy")));
    }

    #[gpui::test]
    fn editing_keeps_the_saved_secrets_behind_dots_and_says_a_blank_keeps_them(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = open(cx, Server::with(Some(saved_webhook())));
        assert!(cx.debug_bounds("webhook-view").is_some());

        view.update(cx, SettingsView::edit);
        cx.run_until_parked();

        let url = field_of(&view, cx, |editor| editor.url.clone());
        let token = field_of(&view, cx, |editor| editor.token.clone());
        let name = field_of(&view, cx, |editor| editor.headers[0].name.clone());
        let value = field_of(&view, cx, |editor| editor.headers[0].value.clone());
        assert!(url.read_with(cx, |field, _| field.is_masked() && field.text().is_empty()));
        assert!(token.read_with(cx, |field, _| field.is_masked()));
        assert!(value.read_with(cx, |field, _| field.is_masked()));
        assert!(!name.read_with(cx, |field, _| field.is_masked()));
        assert_eq!(placeholder(&url, cx), KEEP_URL);
        assert_eq!(placeholder(&token, cx), HIDDEN);
        assert_eq!(placeholder(&value, cx), HIDDEN);
    }

    #[gpui::test]
    fn typing_a_new_url_stops_promising_to_keep_the_saved_token_so_it_never_reaches_another_server(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = open(cx, Server::with(Some(saved_webhook())));
        view.update(cx, SettingsView::edit);

        type_into(
            &view,
            cx,
            |editor| editor.url.clone(),
            "https://other.example/hook",
        );

        let token = field_of(&view, cx, |editor| editor.token.clone());
        let value = field_of(&view, cx, |editor| editor.headers[0].value.clone());
        assert_eq!(placeholder(&token, cx), "Optional");
        assert_eq!(placeholder(&value, cx), "Value");
    }

    #[gpui::test]
    fn removing_the_saved_token_sends_no_token_instead_of_keeping_it(cx: &mut TestAppContext) {
        let server = Server::with(Some(saved_webhook()));
        let (view, cx) = open(cx, server.clone());
        view.update(cx, SettingsView::edit);

        view.update(cx, SettingsView::remove_token);
        type_into(&view, cx, |editor| editor.project.clone(), " 2");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        assert_eq!(server.state().saved[0].token, None);
    }

    #[gpui::test]
    fn escape_leaves_the_saved_webhook_as_it_was(cx: &mut TestAppContext) {
        let server = Server::with(Some(saved_webhook()));
        let (view, cx) = open(cx, server.clone());
        view.update(cx, SettingsView::edit);

        type_into(&view, cx, |editor| editor.project.clone(), " 2");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();

        assert!(view.read_with(cx, |view, _| view.editor.is_none()));
        assert_eq!(server.state().saved, []);
    }

    #[gpui::test]
    fn removing_asks_first_and_then_leaves_an_empty_form_for_the_next_webhook(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(Some(saved_webhook()));
        let (view, cx) = open(cx, server.clone());
        view.update(cx, SettingsView::edit);

        view.update_in(cx, SettingsView::remove);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert_eq!(server.state().removed, 0);

        view.update_in(cx, SettingsView::remove);
        cx.simulate_prompt_answer("Remove");
        cx.run_until_parked();

        assert_eq!(server.state().removed, 1);
        let form = view.read_with(cx, |view, _| {
            view.editor.as_ref().map(|editor| editor.form.clone())
        });
        assert_eq!(form, Some(WebhookForm::of(None)));
    }

    #[gpui::test]
    fn the_usage_data_switch_saves_the_choice_and_a_failure_leaves_it_as_it_was(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(None);
        let (view, cx) = open(cx, server.clone());

        view.update(cx, |view, cx| view.set_analytics(true, cx));
        cx.run_until_parked();
        assert!(server.state().analytics.enabled);

        server.state().busy = true;
        view.update(cx, |view, cx| view.set_analytics(false, cx));
        cx.run_until_parked();

        let (enabled, failed) = view.read_with(cx, |view, _| match &view.load {
            Load::Ready(loaded) => (loaded.analytics.enabled, view.analytics_failed),
            _ => panic!("the settings were loaded"),
        });
        assert!(enabled);
        assert!(failed);
    }

    #[gpui::test]
    fn the_usage_data_switch_is_left_out_on_a_server_that_sends_no_usage_data(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(None);
        server.state().analytics.available = false;

        let (_, cx) = open(cx, server);

        assert!(cx.debug_bounds("usage-analytics").is_none());
    }

    #[gpui::test]
    fn an_expired_sign_in_asks_the_window_to_sign_in_again(cx: &mut TestAppContext) {
        let server = Server::with(None);
        let (view, cx) = open(cx, server.clone());
        let signed_out = Rc::new(Cell::new(false));
        let seen = signed_out.clone();
        cx.update(|_, cx| {
            cx.subscribe(&view, move |_, event: &SettingsEvent, _| {
                if matches!(event, SettingsEvent::SignedOut) {
                    seen.set(true);
                }
            })
            .detach();
        });
        server.state().signed_out = true;

        type_into(
            &view,
            cx,
            |editor| editor.url.clone(),
            "https://example.com/hook",
        );
        type_into(&view, cx, |editor| editor.project.clone(), "Work");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        assert!(signed_out.get());
    }

    #[gpui::test]
    fn leaving_settings_and_coming_back_keeps_a_half_typed_webhook(cx: &mut TestAppContext) {
        let (view, cx) = open(cx, Server::with(None));
        type_into(
            &view,
            cx,
            |editor| editor.url.clone(),
            "https://example.com/hook",
        );

        view.update(cx, SettingsView::reload);
        cx.run_until_parked();

        let url = field_of(&view, cx, |editor| editor.url.clone());
        assert_eq!(
            url.read_with(cx, |field, _| field.text().to_string()),
            "https://example.com/hook"
        );
    }

    #[gpui::test]
    fn coming_back_to_an_untouched_form_shows_what_changed_on_the_server_meanwhile(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(Some(saved_webhook()));
        let (view, cx) = open(cx, server.clone());
        view.update(cx, SettingsView::edit);
        server.state().webhooks.webhook = Some(Webhook {
            project: "Work".into(),
            ..saved_webhook()
        });

        view.update(cx, SettingsView::reload);
        cx.run_until_parked();

        let (editing, project) = view.read_with(cx, |view, _| {
            (
                view.editor.is_some(),
                view.webhook().map(|webhook| webhook.project.clone()),
            )
        });
        assert!(!editing);
        assert_eq!(project.as_deref(), Some("Work"));
    }

    #[gpui::test]
    fn the_form_is_locked_while_saving_so_nothing_changed_during_the_request_is_silently_dropped(
        cx: &mut TestAppContext,
    ) {
        let server = Server::with(None);
        server.state().refuse = Some(vec![("url".into(), "Enter a URL".into())]);
        let (view, cx) = open(cx, server);
        type_into(&view, cx, |editor| editor.project.clone(), "Work");

        let (locked, still_secret) = view.update(cx, |view, cx| {
            view.save(cx);
            view.toggle_url_secret(cx);
            let editor = view.editor.as_ref().expect("the form is open");
            (
                editor.fields().all(|field| field.read(cx).is_read_only()),
                editor.form.url_secret,
            )
        });
        assert!(locked);
        assert!(still_secret);

        cx.run_until_parked();
        let unlocked = view.read_with(cx, |view, cx| {
            let editor = view.editor.as_ref().expect("the refused form stays open");
            editor.fields().all(|field| !field.read(cx).is_read_only())
        });
        assert!(unlocked);
    }
}
