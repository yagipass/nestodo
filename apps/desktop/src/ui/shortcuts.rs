use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::ui::icons::Icon;
use crate::ui::theme::theme;
use crate::ui::widgets::{icon_button, kbd};

type Shortcut = (&'static str, &'static str, Option<&'static str>);

const SHORTCUTS: &[(&str, &[Shortcut])] = &[
    (
        "Editing",
        &[
            ("New item", "Enter", None),
            ("Complete / reopen", "Ctrl/\u{2318} Enter", None),
            ("Join with item above", "\u{232b}", Some("at start")),
            (
                "Delete with children",
                "Ctrl/\u{2318} \u{21e7} \u{232b}",
                None,
            ),
            ("Undo", "Ctrl/\u{2318} Z", None),
        ],
    ),
    (
        "Notes",
        &[
            ("Write a note under the item", "\u{21e7} Enter", None),
            (
                "Back to the title",
                "\u{21e7} Enter / Esc",
                Some("in a note"),
            ),
            ("New line", "Enter", Some("in a note")),
        ],
    ),
    (
        "Structure",
        &[
            ("Indent / outdent", "Tab / \u{21e7} Tab", None),
            ("Move up / down", "Alt \u{21e7} \u{2191} / \u{2193}", None),
            (
                "Move up / down",
                "Ctrl/\u{2318} \u{21e7} \u{2191} / \u{2193}",
                None,
            ),
            ("Move under another item", "Ctrl/\u{2318} \u{21e7} M", None),
            (
                "Collapse / expand",
                "Ctrl/\u{2318} \u{2191} / \u{2193}",
                None,
            ),
            ("Toggle collapse", "Ctrl Space", None),
        ],
    ),
    (
        "Navigation & view",
        &[
            ("Previous / next item", "\u{2191} / \u{2193}", None),
            ("Previous / next item", "Ctrl P / Ctrl N", None),
            ("Show / hide completed", "Ctrl/\u{2318} O", None),
            ("Filter", "/", Some("outside an item")),
        ],
    ),
];

pub fn shortcuts_dialog(
    on_close: impl Fn(&mut gpui::Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let colors = theme(cx);
    let groups = SHORTCUTS.iter().map(|(name, shortcuts)| {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(240.))
            .child(
                div()
                    .mb(px(6.))
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted)
                    .child(name.to_uppercase()),
            )
            .children(shortcuts.iter().map(|(action, keys, note)| {
                let mut chips = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(12.))
                    .text_color(colors.muted);
                for (index, key) in keys.split(' ').enumerate() {
                    if key == "/" && index > 0 {
                        chips = chips.child("/");
                    } else {
                        chips = chips.child(kbd(key.to_string(), cx));
                    }
                }
                if let Some(note) = note {
                    chips = chips.child(note.to_string());
                }
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .min_h(px(34.))
                    .border_t_1()
                    .border_color(colors.grid)
                    .text_size(px(14.))
                    .child(action.to_string())
                    .child(chips)
            }))
    });
    div()
        .id("shortcuts")
        .key_context("Dialog")
        .flex()
        .flex_col()
        .min_h_0()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .pt(px(10.))
                .pr(px(10.))
                .pb(px(4.))
                .pl(px(20.))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("Keyboard shortcuts"),
                )
                .child(
                    icon_button("close-shortcuts", Icon::Close, 16., colors.hover, cx)
                        .on_click(move |_, window, cx| on_close(window, cx)),
                ),
        )
        .child(
            div()
                .id("shortcut-groups")
                .overflow_y_scroll()
                .flex()
                .flex_wrap()
                .gap_x(px(32.))
                .gap_y(px(20.))
                .px(px(20.))
                .pt(px(4.))
                .pb(px(22.))
                .children(groups),
        )
        .into_any_element()
}
