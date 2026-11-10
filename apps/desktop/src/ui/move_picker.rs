use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, HighlightStyle, KeyBinding,
    ScrollHandle, StyledText, Subscription, Window, actions, div, prelude::*, px,
};

use nestodo_desktop::outline::{Destination, Key, Outline, destinations};
use nestodo_desktop::title::tokens;

use crate::ui::icons::Icon;
use crate::ui::text_field::{FieldEvent, TextField};
use crate::ui::theme::theme;
use crate::ui::widgets::icon;

actions!(
    move_picker,
    [PickerUp, PickerDown, PickerConfirm, PickerCancel]
);

pub fn bindings() -> Vec<KeyBinding> {
    let c = Some("Picker");
    vec![
        KeyBinding::new("up", PickerUp, c),
        KeyBinding::new("ctrl-p", PickerUp, c),
        KeyBinding::new("down", PickerDown, c),
        KeyBinding::new("ctrl-n", PickerDown, c),
        KeyBinding::new("enter", PickerConfirm, c),
        KeyBinding::new("escape", PickerCancel, c),
    ]
}

pub enum PickerEvent {
    Picked(Key),
    Dismissed,
}

pub struct MovePicker {
    outline: Outline,
    moving: Key,
    show_done: bool,
    field: Entity<TextField>,
    active: usize,
    scroll: ScrollHandle,
    _subscription: Subscription,
}

impl EventEmitter<PickerEvent> for MovePicker {}

impl Focusable for MovePicker {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl MovePicker {
    pub fn new(
        outline: Outline,
        moving: Key,
        show_done: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = Self::name_of(&outline, &moving);
        let field = cx.new(|cx| {
            TextField::new("Picker", cx)
                .placeholder(format!("Move \u{201c}{name}\u{201d} to\u{2026}"))
        });
        let subscription = cx.subscribe(&field, |this: &mut Self, _, event: &FieldEvent, cx| {
            if let FieldEvent::Changed { .. } = event {
                this.active = 0;
                this.scroll.scroll_to_item(0);
                cx.notify();
            }
        });
        field.read(cx).focus(window);
        Self {
            outline,
            moving,
            show_done,
            field,
            active: 0,
            scroll: ScrollHandle::new(),
            _subscription: subscription,
        }
    }

    fn name_of(outline: &Outline, key: &str) -> String {
        outline
            .get(key)
            .map(|item| item.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "empty item".to_string())
    }

    fn found(&self, cx: &gpui::App) -> Vec<Destination> {
        destinations(
            &self.outline,
            &self.moving,
            self.show_done,
            self.field.read(cx).text(),
        )
    }

    fn step(&mut self, step: isize, cx: &mut Context<Self>) {
        let found = self.found(cx);
        if found.is_empty() {
            return;
        }
        let last = found.len() - 1;
        let current = self.active.min(last);
        self.active = if step < 0 {
            current.saturating_sub(step.unsigned_abs())
        } else {
            current.saturating_add(step.unsigned_abs()).min(last)
        };
        self.scroll.scroll_to_item(self.active);
        cx.notify();
    }

    fn up(&mut self, _: &PickerUp, _: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, cx);
    }

    fn down(&mut self, _: &PickerDown, _: &mut Window, cx: &mut Context<Self>) {
        self.step(1, cx);
    }

    fn confirm(&mut self, _: &PickerConfirm, _: &mut Window, cx: &mut Context<Self>) {
        if self.field.read(cx).is_composing() {
            return;
        }
        let found = self.found(cx);
        if let Some(selected) = found.get(self.active.min(found.len().saturating_sub(1))) {
            cx.emit(PickerEvent::Picked(selected.key.clone()));
        }
    }
}

impl Render for MovePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(cx);
        let query = self.field.read(cx).text().to_string();
        let found = self.found(cx);
        let selected = self.active.min(found.len().saturating_sub(1));
        let empty = found.is_empty();
        let list = div()
            .id("move-to-options")
            .flex()
            .flex_col()
            .min_h_0()
            .px(px(6.))
            .pb(px(6.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children(found.into_iter().enumerate().map(|(index, destination)| {
                let parts = tokens(&destination.title, &query);
                let highlights: Vec<_> = parts
                    .iter()
                    .filter(|part| part.matched)
                    .map(|part| {
                        (
                            part.from..part.to,
                            HighlightStyle {
                                background_color: Some(colors.matched),
                                ..Default::default()
                            },
                        )
                    })
                    .collect();
                let key = destination.key.clone();
                div()
                    .id(gpui::ElementId::Name(
                        format!("move-to-{}", destination.key).into(),
                    ))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .text_size(px(14.))
                    .cursor_pointer()
                    .when(index == selected, |row| row.bg(colors.accent_soft))
                    .when(index != selected, |row| {
                        row.hover(|style| style.bg(colors.hover))
                    })
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(PickerEvent::Picked(key.clone()));
                    }))
                    .child(div().child(
                        StyledText::new(destination.title.clone()).with_highlights(highlights),
                    ))
                    .when(!destination.path.is_empty(), |row| {
                        row.child(
                            div()
                                .text_size(px(12.))
                                .text_color(colors.muted)
                                .truncate()
                                .child(destination.path.join(" \u{203a} ")),
                        )
                    })
            }));
        div()
            .key_context("Picker")
            .flex()
            .flex_col()
            .min_h_0()
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(|_, _: &PickerCancel, _, cx| {
                cx.emit(PickerEvent::Dismissed);
            }))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6.))
                    .h(px(40.))
                    .m(px(12.))
                    .px(px(10.))
                    .border_1()
                    .border_color(colors.accent)
                    .rounded(px(6.))
                    .bg(colors.surface)
                    .child(icon(Icon::Search, 16., colors.muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(colors.text)
                            .child(self.field.clone()),
                    ),
            )
            .when(empty, |panel| {
                panel.child(
                    div()
                        .px(px(22.))
                        .pb(px(18.))
                        .text_size(px(13.))
                        .text_color(colors.muted)
                        .child("No matches."),
                )
            })
            .when(!empty, |panel| panel.child(list))
    }
}
