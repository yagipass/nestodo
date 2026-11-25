use gpui::{
    Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, Subscription,
    Window, actions, div, prelude::*, px,
};

use nestodo_desktop::icon::{ICON_CHOICES, is_icon};

use crate::ui::text_field::{FieldEvent, TextField};
use crate::ui::theme::theme;
use crate::ui::todo_icon::todo_icon;
use crate::ui::widgets::text_button;

actions!(icon_picker, [ApplyOther, CloseIconPicker]);

const PER_ROW: usize = 8;

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("enter", ApplyOther, Some("IconOther")),
        KeyBinding::new("escape", CloseIconPicker, Some("IconPicker")),
    ]
}

pub enum IconEvent {
    Chosen(Option<String>),
    Dismissed,
}

pub struct IconPicker {
    current: Option<String>,
    field: Entity<TextField>,
    refused: bool,
    _subscription: Subscription,
}

impl EventEmitter<IconEvent> for IconPicker {}

impl Focusable for IconPicker {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl IconPicker {
    pub fn new(current: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field =
            cx.new(|cx| TextField::new("IconOther", cx).placeholder("Type or paste one emoji"));
        let subscription = cx.subscribe(&field, |this: &mut Self, _, event: &FieldEvent, cx| {
            if let FieldEvent::Changed { .. } = event
                && this.refused
            {
                this.refused = false;
                cx.notify();
            }
        });
        window.focus(&field.focus_handle(cx), cx);
        Self {
            current,
            field,
            refused: false,
            _subscription: subscription,
        }
    }

    fn apply_other(&mut self, _: &ApplyOther, _: &mut Window, cx: &mut Context<Self>) {
        let field = self.field.read(cx);
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
}

impl Render for IconPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(cx);
        let rows = ICON_CHOICES
            .chunks(PER_ROW)
            .enumerate()
            .map(|(row, choices)| {
                div()
                    .flex()
                    .gap(px(4.))
                    .children(choices.iter().enumerate().map(|(column, &choice)| {
                        let index = row * PER_ROW + column;
                        let selected = self.current.as_deref() == Some(choice);
                        div()
                            .id(ElementId::NamedInteger("icon-choice".into(), index as u64))
                            .debug_selector(move || format!("icon-choice-{index}"))
                            .relative()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(40.))
                            .rounded(px(10.))
                            .cursor_pointer()
                            .when(selected, |cell| {
                                cell.bg(colors.accent_soft).child(
                                    div()
                                        .debug_selector(|| "icon-choice-selected".into())
                                        .absolute()
                                        .inset_0()
                                        .border_2()
                                        .border_color(colors.accent)
                                        .rounded(px(10.)),
                                )
                            })
                            .when(!selected, |cell| {
                                cell.hover(move |style| style.bg(colors.hover))
                            })
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(IconEvent::Chosen(Some(choice.to_owned())));
                            }))
                            .children(todo_icon(choice, 22., false))
                    }))
            });
        let other = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .h(px(40.))
            .px(px(12.))
            .border_1()
            .border_color(if self.refused {
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
                    .child("Other"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(15.))
                    .line_height(px(20.))
                    .text_color(colors.text)
                    .child(self.field.clone()),
            );
        let refusal = self.refused.then(|| {
            div()
                .debug_selector(|| "icon-refused".into())
                .text_size(px(13.))
                .text_color(colors.danger)
                .child("Enter one emoji")
        });
        let remove = self.current.is_some().then(|| {
            div().flex().child(
                text_button("remove-icon", "Remove icon", true, true, &colors)
                    .debug_selector(|| "remove-icon".into())
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(IconEvent::Chosen(None)))),
            )
        });
        div()
            .key_context("IconPicker")
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .on_action(cx.listener(Self::apply_other))
            .on_action(cx.listener(|_, _: &CloseIconPicker, _, cx| {
                cx.emit(IconEvent::Dismissed);
            }))
            .child(div().flex().flex_col().gap(px(4.)).children(rows))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(other)
                    .children(refusal),
            )
            .children(remove)
    }
}
