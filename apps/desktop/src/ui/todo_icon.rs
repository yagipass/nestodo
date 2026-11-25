use gpui::{Div, ParentElement, Styled, div, prelude::*, px};

use nestodo_desktop::icon::shown_icon;

pub fn todo_icon(value: &str, size: f32, muted: bool) -> Option<Div> {
    let shown = shown_icon(Some(value))?;
    Some(
        div()
            .flex_none()
            .text_size(px(size))
            .line_height(px(size * 1.25))
            .when(muted, |glyph| glyph.opacity(0.5))
            .child(shown.to_owned()),
    )
}
