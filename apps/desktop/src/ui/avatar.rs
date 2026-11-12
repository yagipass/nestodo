use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, ElementId, Image, ImageFormat, Stateful, div, img, prelude::*, px,
};

use nestodo_desktop::profile::Profile;

use crate::ui::backend::Backend;
use crate::ui::icons::Icon;
use crate::ui::theme::{Theme, theme};
use crate::ui::widgets::icon;

#[derive(Clone, Debug)]
pub struct Avatar {
    pub url: String,
    pub image: Arc<Image>,
}

impl Avatar {
    #[must_use]
    pub fn of(url: String, bytes: Vec<u8>) -> Option<Self> {
        let format = match image::guess_format(&bytes).ok()? {
            image::ImageFormat::Png => ImageFormat::Png,
            image::ImageFormat::Jpeg => ImageFormat::Jpeg,
            image::ImageFormat::WebP => ImageFormat::Webp,
            _ => return None,
        };
        Some(Self {
            url,
            image: Arc::new(Image::from_bytes(format, bytes)),
        })
    }
}

pub fn fetch_avatar(
    backend: &dyn Backend,
    profile: &Profile,
    current: Option<&Avatar>,
) -> Option<Avatar> {
    let url = profile.avatar_url.as_ref()?;
    if let Some(current) = current.filter(|current| &current.url == url) {
        return Some(current.clone());
    }
    Avatar::of(url.clone(), backend.avatar(url).ok()?)
}

pub fn avatar_circle(avatar: Option<&Avatar>, size: f32, colors: &Theme) -> AnyElement {
    match avatar {
        Some(avatar) => img(avatar.image.clone())
            .size(px(size))
            .flex_none()
            .rounded_full()
            .into_any_element(),
        None => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(colors.hover)
            .child(icon(Icon::Account, size * 0.5, colors.muted))
            .into_any_element(),
    }
}

pub fn avatar_button(id: impl Into<ElementId>, avatar: &Avatar, cx: &App) -> Stateful<Div> {
    let colors = theme(cx);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded(px(9.))
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .child(avatar_circle(Some(avatar), 22., &colors))
}
