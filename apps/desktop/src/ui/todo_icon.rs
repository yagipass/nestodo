use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AnyElement, Image, Img, div, img, prelude::*, px};

use nestodo_desktop::icon::{ImageIcon, Shown, shown_icon};

const ROW_GLYPH: f32 = 17.;
const ROW_IMAGE: f32 = 22.;
const ROW_RADIUS: f32 = 6.;

#[derive(Clone, Debug, Default)]
pub struct Icons {
    pub list: Vec<ImageIcon>,
    pub pictures: HashMap<String, Arc<Image>>,
}

pub fn todo_icon(value: &str, glyph: f32, muted: bool, icons: &Icons) -> Option<AnyElement> {
    match shown_icon(value, &icons.list)? {
        Shown::Emoji(text) => Some(
            div()
                .flex_none()
                .text_size(px(glyph))
                .line_height(px(glyph * 1.25))
                .when(muted, |glyph| glyph.opacity(0.5))
                .child(text.to_owned())
                .into_any_element(),
        ),
        Shown::Image(icon) => icons.pictures.get(&icon.id).map(|picture| {
            image_square(picture.clone(), glyph * ROW_IMAGE / ROW_GLYPH, muted).into_any_element()
        }),
    }
}

pub fn image_square(picture: Arc<Image>, side: f32, muted: bool) -> Img {
    img(picture)
        .flex_none()
        .size(px(side))
        .rounded(px(side * ROW_RADIUS / ROW_IMAGE))
        .when(muted, |picture| picture.opacity(0.5))
}
