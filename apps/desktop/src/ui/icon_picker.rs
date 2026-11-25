use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use gpui::{
    AnyElement, Context, Div, ElementId, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    Image, KeyBinding, MouseButton, PathPromptOptions, SharedString, Stateful, Subscription,
    Window, actions, div, prelude::*, px,
};

use nestodo_desktop::error::Error;
use nestodo_desktop::icon::{
    ICON_CHOICES, ImageIcon, MAX_ICON_NAME_LENGTH, icon_name_from, image_id, is_icon,
};
use nestodo_desktop::profile::icon_png;

use crate::ui::avatar::decode_image;
use crate::ui::backend::Backend;
use crate::ui::icons::Icon;
use crate::ui::text_field::{FieldEvent, TextField};
use crate::ui::theme::{Theme, theme};
use crate::ui::todo_icon::{Icons, image_square, todo_icon};
use crate::ui::widgets::{Tooltip, bullet_mark, danger_pill, icon, pill, text_button};

actions!(icon_picker, [ApplyOther, SaveImageName, CloseIconPicker]);

const PER_ROW: usize = 8;
const TILE: f32 = 40.;
const LARGE: f32 = 72.;
const UNREADABLE: &str = "The image could not be read.";
const NAMELESS: &str = "Enter a name";

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("enter", ApplyOther, Some("IconOther")),
        KeyBinding::new("enter", SaveImageName, Some("ImageName")),
        KeyBinding::new("escape", CloseIconPicker, Some("IconPicker")),
    ]
}

pub enum IconEvent {
    Chosen(Option<String>),
    Added {
        image: ImageIcon,
        picture: Arc<Image>,
        waiting: bool,
    },
    Renamed(ImageIcon),
    Deleted(String),
    Dismissed,
}

enum View {
    Grid,
    Add { png: Vec<u8>, picture: Arc<Image> },
    Edit(ImageIcon),
    Delete(ImageIcon),
}

pub struct IconPicker {
    backend: Arc<dyn Backend>,
    current: Option<String>,
    title: String,
    icons: Icons,
    uses: HashMap<String, usize>,
    view: View,
    other: Entity<TextField>,
    name: Entity<TextField>,
    focus_handle: FocusHandle,
    refused: bool,
    unreadable: bool,
    busy: bool,
    adding: u64,
    problem: Option<String>,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<IconEvent> for IconPicker {}

impl Focusable for IconPicker {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        match self.view {
            View::Grid => self.other.focus_handle(cx),
            View::Add { .. } | View::Edit(_) => self.name.focus_handle(cx),
            View::Delete(_) => self.focus_handle.clone(),
        }
    }
}

#[must_use]
pub fn deletion_warning(uses: usize) -> String {
    match uses {
        0 => "Deleting it cannot be undone.".to_string(),
        1 => "It is the icon of 1 todo. Deleting it removes the icon from that todo and cannot be undone."
            .to_string(),
        uses => format!(
            "It is the icon of {uses} todos. Deleting it removes the icon from them and cannot be undone."
        ),
    }
}

fn read_image(path: &Path) -> Option<(String, Vec<u8>, Arc<Image>)> {
    let png = icon_png(&std::fs::read(path).ok()?).ok()?;
    let picture = decode_image(png.clone())?;
    Some((icon_name_from(path), png, picture))
}

fn told(error: &Error, otherwise: &str) -> String {
    match error {
        Error::Refused(_) => error.to_string(),
        _ => otherwise.to_string(),
    }
}

fn rows_of(tiles: Vec<AnyElement>) -> Vec<Div> {
    let mut tiles = tiles.into_iter().peekable();
    let mut rows = Vec::new();
    while tiles.peek().is_some() {
        rows.push(
            div()
                .flex()
                .gap(px(4.))
                .children(tiles.by_ref().take(PER_ROW)),
        );
    }
    rows
}

fn tile(id: ElementId, selected: bool, colors: &Theme) -> Stateful<Div> {
    let (accent, hover) = (colors.accent, colors.hover);
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(TILE))
        .rounded(px(10.))
        .cursor_pointer()
        .when(selected, |cell| {
            cell.bg(colors.accent_soft).child(
                div()
                    .debug_selector(|| "icon-choice-selected".into())
                    .absolute()
                    .inset_0()
                    .border_2()
                    .border_color(accent)
                    .rounded(px(10.)),
            )
        })
        .when(!selected, |cell| cell.hover(move |style| style.bg(hover)))
}

fn section(text: &'static str, colors: &Theme) -> Div {
    div()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.muted)
        .child(text)
}

fn problem_line(selector: &'static str, text: impl Into<SharedString>, colors: &Theme) -> Div {
    div()
        .debug_selector(move || selector.into())
        .text_size(px(13.))
        .text_color(colors.danger)
        .child(text.into())
}

fn field_box(label: &'static str, field: &Entity<TextField>, invalid: bool, colors: &Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .h(px(40.))
        .px(px(12.))
        .border_1()
        .border_color(if invalid {
            colors.danger
        } else {
            colors.accent
        })
        .rounded(px(10.))
        .bg(colors.surface)
        .child(
            div()
                .flex_none()
                .text_size(px(13.))
                .text_color(colors.muted)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(15.))
                .line_height(px(20.))
                .text_color(colors.text)
                .child(field.clone()),
        )
}

impl IconPicker {
    pub fn new(
        current: Option<String>,
        title: String,
        backend: Arc<dyn Backend>,
        icons: Icons,
        uses: HashMap<String, usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let other =
            cx.new(|cx| TextField::new("IconOther", cx).placeholder("Type or paste one emoji"));
        let name = cx.new(|cx| TextField::new("ImageName", cx).max_len(MAX_ICON_NAME_LENGTH));
        let subscriptions = [
            cx.subscribe(&other, |this: &mut Self, _, event: &FieldEvent, cx| {
                if let FieldEvent::Changed { .. } = event
                    && this.refused
                {
                    this.refused = false;
                    cx.notify();
                }
            }),
            cx.subscribe(&name, |this: &mut Self, _, event: &FieldEvent, cx| {
                if let FieldEvent::Changed { .. } = event
                    && this.problem.is_some()
                {
                    this.problem = None;
                    cx.notify();
                }
            }),
        ];
        window.focus(&other.focus_handle(cx), cx);
        Self {
            backend,
            current,
            title,
            icons,
            uses,
            view: View::Grid,
            other,
            name,
            focus_handle: cx.focus_handle(),
            refused: false,
            unreadable: false,
            busy: false,
            adding: 0,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn follow(&mut self, icons: Icons, uses: HashMap<String, usize>, cx: &mut Context<Self>) {
        self.icons = icons;
        self.uses = uses;
        cx.notify();
    }

    #[must_use]
    pub fn warning(&self) -> Option<String> {
        let View::Delete(image) = &self.view else {
            return None;
        };
        let uses = self.uses.get(&image.value()).copied().unwrap_or(0);
        Some(deletion_warning(uses))
    }

    #[cfg(test)]
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    #[cfg(test)]
    pub fn name_field(&self) -> Entity<TextField> {
        self.name.clone()
    }

    fn apply_other(&mut self, _: &ApplyOther, _: &mut Window, cx: &mut Context<Self>) {
        let field = self.other.read(cx);
        if field.is_composing() {
            return;
        }
        let typed = field.text().trim().to_owned();
        if is_icon(&typed) {
            cx.emit(IconEvent::Chosen(Some(typed)));
        } else {
            self.refused = true;
            cx.notify();
        }
    }

    fn show_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let leaving_add = matches!(self.view, View::Add { .. });
        if self.busy && !leaving_add {
            return;
        }
        if leaving_add {
            self.adding += 1;
            self.set_busy(false, cx);
        }
        self.view = View::Grid;
        self.problem = None;
        window.focus(&self.other.focus_handle(cx), cx);
        cx.notify();
    }

    fn saved_to_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = Focusable::focus_handle(self, cx).is_focused(window);
        self.view = View::Grid;
        self.problem = None;
        if focused {
            window.focus(&self.other.focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn show_name(
        &mut self,
        view: View,
        name: &str,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view = view;
        self.problem = None;
        self.unreadable = false;
        self.name.update(cx, |field, cx| {
            field.set_text(name, name.len(), cx);
            if select {
                field.select_all_text(cx);
            }
        });
        window.focus(&self.name.focus_handle(cx), cx);
        cx.notify();
    }

    fn choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.unreadable = false;
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let read = cx.background_spawn(async move { read_image(&path) }).await;
            this.update_in(cx, |this, window, cx| {
                if let Some((name, png, picture)) = read {
                    this.show_name(View::Add { png, picture }, &name, true, window, cx);
                } else {
                    this.unreadable = true;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn edit(&mut self, image: &ImageIcon, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.show_name(View::Edit(image.clone()), &image.name, false, window, cx);
    }

    fn typed_name(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let field = self.name.read(cx);
        if self.busy || field.is_composing() {
            return None;
        }
        let name = field.text().trim().to_owned();
        if name.is_empty() {
            self.problem = Some(NAMELESS.into());
            cx.notify();
            return None;
        }
        Some(name)
    }

    fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        if busy {
            self.problem = None;
        }
        self.name
            .update(cx, |field, cx| field.set_read_only(busy, cx));
        cx.notify();
    }

    fn on_save_name(&mut self, _: &SaveImageName, window: &mut Window, cx: &mut Context<Self>) {
        match self.view {
            View::Add { .. } => self.add(cx),
            View::Edit(_) => self.rename(window, cx),
            View::Grid | View::Delete(_) => {}
        }
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let View::Add { png, picture } = &self.view else {
            return;
        };
        let (png, picture) = (png.clone(), picture.clone());
        let Some(name) = self.typed_name(cx) else {
            return;
        };
        self.adding += 1;
        let ticket = self.adding;
        self.set_busy(true, cx);
        let backend = self.backend.clone();
        let picker = cx.entity();
        cx.spawn(async move |_, cx| {
            let added = cx
                .background_spawn(async move { backend.add_icon(&name, &png) })
                .await;
            picker.update(cx, |this, cx| {
                let waiting = this.adding == ticket && matches!(this.view, View::Add { .. });
                if waiting {
                    this.set_busy(false, cx);
                }
                match added {
                    Ok(image) => cx.emit(IconEvent::Added {
                        image,
                        picture,
                        waiting,
                    }),
                    Err(error) if waiting => {
                        this.problem = Some(told(&error, "Could not add the image."));
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let View::Edit(image) = &self.view else {
            return;
        };
        let id = image.id.clone();
        let Some(name) = self.typed_name(cx) else {
            return;
        };
        self.set_busy(true, cx);
        let backend = self.backend.clone();
        let picker = cx.entity();
        cx.spawn_in(window, async move |_, cx| {
            let renamed = cx
                .background_spawn(async move { backend.rename_icon(&id, &name) })
                .await;
            picker
                .update_in(cx, |this, window, cx| {
                    this.set_busy(false, cx);
                    match renamed {
                        Ok(image) => {
                            if let Some(known) = this
                                .icons
                                .list
                                .iter_mut()
                                .find(|known| known.id == image.id)
                            {
                                known.clone_from(&image);
                            }
                            this.saved_to_grid(window, cx);
                            cx.emit(IconEvent::Renamed(image));
                        }
                        Err(error) => {
                            this.problem = Some(told(&error, "Could not save the image."));
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn ask_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let View::Edit(image) = &self.view else {
            return;
        };
        if self.busy {
            return;
        }
        self.view = View::Delete(image.clone());
        self.problem = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn back_to_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let View::Delete(image) = &self.view else {
            return;
        };
        if self.busy {
            return;
        }
        self.view = View::Edit(image.clone());
        self.problem = None;
        window.focus(&self.name.focus_handle(cx), cx);
        cx.notify();
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let View::Delete(image) = &self.view else {
            return;
        };
        if self.busy {
            return;
        }
        let id = image.id.clone();
        self.set_busy(true, cx);
        let backend = self.backend.clone();
        let picker = cx.entity();
        cx.spawn_in(window, async move |_, cx| {
            let gone = id.clone();
            let deleted = cx
                .background_spawn(async move { backend.delete_icon(&gone) })
                .await;
            picker
                .update_in(cx, |this, window, cx| {
                    this.set_busy(false, cx);
                    match deleted {
                        Ok(()) => {
                            this.icons.list.retain(|known| known.id != id);
                            if this.current.as_deref().and_then(image_id) == Some(id.as_str()) {
                                this.current = None;
                            }
                            this.saved_to_grid(window, cx);
                            cx.emit(IconEvent::Deleted(id));
                        }
                        Err(error) => {
                            this.problem = Some(told(&error, "Could not delete the image."));
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn render_grid(&self, cx: &mut Context<Self>) -> Div {
        let colors = theme(cx);
        let hover = colors.hover;
        let mut mine: Vec<AnyElement> = Vec::new();
        for (index, image) in self.icons.list.iter().enumerate() {
            let value = image.value();
            let selected = self.current.as_deref() == Some(value.as_str());
            let drawn = todo_icon(&value, 22., false, &self.icons);
            let edited = image.clone();
            mine.push(
                tile(
                    ElementId::NamedInteger("my-icon".into(), index as u64),
                    selected,
                    &colors,
                )
                .debug_selector(move || format!("my-icon-{index}"))
                .tooltip(Tooltip::text(image.name.clone()))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(IconEvent::Chosen(Some(value.clone())));
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, _, window, cx| this.edit(&edited, window, cx)),
                )
                .children(drawn)
                .into_any_element(),
            );
        }
        mine.push(
            div()
                .id("add-image")
                .debug_selector(|| "add-image".into())
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(TILE))
                .rounded(px(10.))
                .border_1()
                .border_color(colors.border)
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .tooltip(Tooltip::text("Add image"))
                .on_click(cx.listener(|this, _, window, cx| this.choose_file(window, cx)))
                .child(icon(Icon::Add, 20., colors.muted))
                .into_any_element(),
        );
        let emoji: Vec<AnyElement> = ICON_CHOICES
            .iter()
            .enumerate()
            .map(|(index, &choice)| {
                let selected = self.current.as_deref() == Some(choice);
                tile(
                    ElementId::NamedInteger("icon-choice".into(), index as u64),
                    selected,
                    &colors,
                )
                .debug_selector(move || format!("icon-choice-{index}"))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(IconEvent::Chosen(Some(choice.to_owned())));
                }))
                .children(todo_icon(choice, 22., false, &self.icons))
                .into_any_element()
            })
            .collect();
        let unreadable = self
            .unreadable
            .then(|| problem_line("image-unreadable", UNREADABLE, &colors));
        let refusal = self
            .refused
            .then(|| problem_line("icon-refused", "Enter one emoji", &colors));
        let remove = self.current.is_some().then(|| {
            div().flex().child(
                text_button("remove-icon", "Remove icon", true, true, &colors)
                    .debug_selector(|| "remove-icon".into())
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(IconEvent::Chosen(None)))),
            )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(section("My icons", &colors))
                    .child(div().flex().flex_col().gap(px(4.)).children(rows_of(mine)))
                    .children(unreadable),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(section("Emoji", &colors))
                    .child(div().flex().flex_col().gap(px(4.)).children(rows_of(emoji))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(field_box("Other", &self.other, self.refused, &colors))
                    .children(refusal),
            )
            .children(remove)
    }

    fn render_header(title: &'static str, cx: &mut Context<Self>) -> Div {
        let colors = theme(cx);
        let hover = colors.hover;
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .id("image-back")
                    .debug_selector(|| "image-back".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .tooltip(Tooltip::text("Back"))
                    .on_click(cx.listener(|this, _, window, cx| this.show_grid(window, cx)))
                    .child(icon(Icon::Back, 18., colors.muted)),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
    }

    fn render_large(picture: Option<Arc<Image>>, colors: &Theme) -> Div {
        div().flex().justify_center().child(match picture {
            Some(picture) => image_square(picture, LARGE, false).into_any_element(),
            None => div()
                .size(px(LARGE))
                .rounded(px(LARGE * 6. / 22.))
                .bg(colors.hover)
                .into_any_element(),
        })
    }

    fn render_name(&self, colors: &Theme) -> Div {
        let problem = self
            .problem
            .clone()
            .map(|problem| problem_line("image-problem", problem, colors));
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(field_box(
                "Name",
                &self.name,
                self.problem.is_some(),
                colors,
            ))
            .children(problem)
    }

    fn render_preview(&self, picture: &Arc<Image>, done: bool, colors: &Theme) -> Div {
        div()
            .flex()
            .items_center()
            .h(px(34.))
            .px(px(6.))
            .rounded(px(9.))
            .bg(colors.bg)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .child(bullet_mark(done, colors)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .child(image_square(picture.clone(), 22., done)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(4.))
                    .truncate()
                    .text_size(px(15.))
                    .text_color(if done { colors.muted } else { colors.text })
                    .when(done, Styled::line_through)
                    .child(self.title.clone()),
            )
    }

    fn render_add(&self, picture: &Arc<Image>, cx: &mut Context<Self>) -> Div {
        let colors = theme(cx);
        div()
            .debug_selector(|| "image-add-view".into())
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(Self::render_header("Add image", cx))
            .child(Self::render_large(Some(picture.clone()), &colors))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(section("In the outline", &colors))
                    .child(self.render_preview(picture, false, &colors))
                    .child(self.render_preview(picture, true, &colors)),
            )
            .child(self.render_name(&colors))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        text_button("image-cancel", "Back", false, true, &colors)
                            .debug_selector(|| "image-cancel".into())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_grid(window, cx)),
                            ),
                    )
                    .child(
                        pill("add-image-confirm", "Add and use", !self.busy, &colors)
                            .debug_selector(|| "add-image-confirm".into())
                            .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
                    ),
            )
    }

    fn render_edit(&self, image: &ImageIcon, cx: &mut Context<Self>) -> Div {
        let colors = theme(cx);
        div()
            .debug_selector(|| "image-edit-view".into())
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(Self::render_header("Edit image", cx))
            .child(Self::render_large(
                self.icons.pictures.get(&image.id).cloned(),
                &colors,
            ))
            .child(self.render_name(&colors))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        text_button("delete-image", "Delete image", true, !self.busy, &colors)
                            .debug_selector(|| "delete-image".into())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.ask_delete(window, cx)),
                            ),
                    )
                    .child(
                        pill("save-image", "Save", !self.busy, &colors)
                            .debug_selector(|| "save-image".into())
                            .on_click(cx.listener(|this, _, window, cx| this.rename(window, cx))),
                    ),
            )
    }

    fn render_delete(&self, image: &ImageIcon, cx: &mut Context<Self>) -> Div {
        let colors = theme(cx);
        let problem = self
            .problem
            .clone()
            .map(|problem| problem_line("image-problem", problem, &colors));
        div()
            .debug_selector(|| "image-delete-view".into())
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Delete \u{201c}{}\u{201d}?", image.name)),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(colors.text)
                    .children(self.warning()),
            )
            .children(problem)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        text_button("delete-image-back", "Back", false, !self.busy, &colors)
                            .debug_selector(|| "delete-image-back".into())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.back_to_edit(window, cx)),
                            ),
                    )
                    .child(
                        danger_pill("delete-image-confirm", "Delete", !self.busy, &colors)
                            .debug_selector(|| "delete-image-confirm".into())
                            .on_click(cx.listener(|this, _, window, cx| this.delete(window, cx))),
                    ),
            )
    }
}

impl Render for IconPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.view {
            View::Grid => self.render_grid(cx),
            View::Add { picture, .. } => self.render_add(picture, cx),
            View::Edit(image) => self.render_edit(image, cx),
            View::Delete(image) => self.render_delete(image, cx),
        };
        let deleting = matches!(self.view, View::Delete(_));
        div()
            .id("icon-picker")
            .key_context("IconPicker")
            .when(deleting, |picker| picker.track_focus(&self.focus_handle))
            .flex()
            .flex_col()
            .min_h_0()
            .p(px(16.))
            .overflow_y_scroll()
            .on_action(cx.listener(Self::apply_other))
            .on_action(cx.listener(Self::on_save_name))
            .on_action(cx.listener(|_, _: &CloseIconPicker, _, cx| {
                cx.emit(IconEvent::Dismissed);
            }))
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delete_question_says_how_many_todos_lose_the_icon_in_words_that_fit_the_count() {
        assert_eq!(deletion_warning(0), "Deleting it cannot be undone.");
        assert_eq!(
            deletion_warning(1),
            "It is the icon of 1 todo. Deleting it removes the icon from that todo and cannot be undone."
        );
        assert_eq!(
            deletion_warning(3),
            "It is the icon of 3 todos. Deleting it removes the icon from them and cannot be undone."
        );
    }
}
