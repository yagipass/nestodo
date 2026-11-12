use gpui::{
    AnyElement, AnyView, App, BoxShadow, Div, ElementId, Hsla, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Point, Render, SharedString, Stateful, Styled, Svg,
    Window, anchored, deferred, div, point, prelude::*, px, svg,
};

use crate::ui::icons::Icon;
use crate::ui::theme::{Theme, theme};

pub const PAGE_WIDTH: f32 = 728.;
pub const MIN_WINDOW_WIDTH: f32 = 360.;
pub const TITLE_BAR_HEIGHT: f32 = 52.;
pub const TRAFFIC_LIGHTS_WIDTH: f32 = 84.;
pub const NAMED_TITLE_BAR_WIDTH: f32 = 480.;

pub fn icon(icon: Icon, size: f32, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .size(px(size))
        .flex_none()
        .text_color(color)
}

pub fn icon_button(
    id: impl Into<ElementId>,
    glyph: Icon,
    glyph_size: f32,
    cx: &App,
) -> Stateful<Div> {
    let colors = theme(cx);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded(px(9.))
        .text_color(colors.muted)
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .child(icon(glyph, glyph_size, colors.muted))
}

pub fn kbd(text: impl Into<SharedString>, cx: &App) -> Div {
    let colors = theme(cx);
    div()
        .flex_none()
        .min_w(px(22.))
        .px(px(6.))
        .py(px(1.))
        .border_1()
        .border_b_2()
        .border_color(colors.border)
        .rounded(px(6.))
        .bg(colors.surface)
        .text_color(colors.text)
        .text_size(px(11.5))
        .line_height(px(16.))
        .flex()
        .justify_center()
        .child(text.into())
}

pub fn shadow(colors: &Theme, large: bool) -> Vec<BoxShadow> {
    let (blur, y, alpha) = match (large, colors.dark) {
        (false, false) => (32., 12., 0.14),
        (true, false) => (56., 24., 0.24),
        (false, true) => (40., 16., 0.45),
        (true, true) => (56., 24., 0.5),
    };
    let mut base = colors.shadow;
    base.a = alpha;
    let mut edge = colors.shadow;
    edge.a = if colors.dark { 0.2 } else { 0.08 };
    vec![
        BoxShadow {
            color: base,
            offset: point(px(0.), px(y)),
            blur_radius: px(blur),
            spread_radius: px(0.),
            inset: false,
        },
        BoxShadow {
            color: edge,
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        },
    ]
}

pub fn sheet_shadow(colors: &Theme) -> Vec<BoxShadow> {
    if colors.dark {
        return Vec::new();
    }
    let mut near = colors.shadow;
    near.a = 0.06;
    let mut far = colors.shadow;
    far.a = 0.07;
    vec![
        BoxShadow {
            color: near,
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        },
        BoxShadow {
            color: far,
            offset: point(px(0.), px(12.)),
            blur_radius: px(32.),
            spread_radius: px(0.),
            inset: false,
        },
    ]
}

pub fn modal(
    width: Pixels,
    content: AnyElement,
    on_backdrop: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &App,
) -> impl IntoElement {
    let colors = theme(cx);
    let viewport = window.viewport_size();
    let max_height = viewport.height * 0.8;
    let width = width.min(viewport.width - px(32.));
    deferred(
        anchored().position(Point::default()).child(
            div()
                .occlude()
                .w(viewport.width)
                .h(viewport.height)
                .bg(colors.backdrop)
                .flex()
                .flex_col()
                .items_center()
                .pt(viewport.height * 0.1)
                .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, window, cx| {
                    on_backdrop(window, cx);
                })
                .child(
                    div()
                        .w(width)
                        .max_h(max_height)
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .border_1()
                        .border_color(colors.border)
                        .rounded(px(16.))
                        .bg(colors.surface)
                        .text_color(colors.text)
                        .shadow(shadow(&colors, true))
                        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        })
                        .child(content),
                ),
        ),
    )
    .with_priority(1)
}

pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    pub fn text(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
        let text = text.into();
        move |_, cx| {
            let text = text.clone();
            cx.new(|_| Tooltip { text }).into()
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let colors = theme(cx);
        div().pl(px(8.)).pt(px(6.)).child(
            div()
                .px(px(8.))
                .py(px(4.))
                .rounded(px(8.))
                .bg(colors.text)
                .text_color(colors.bg)
                .text_size(px(12.))
                .whitespace_nowrap()
                .child(self.text.clone()),
        )
    }
}
