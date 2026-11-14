use std::borrow::Cow;

use gpui::{App, Global, Hsla, Rgba, Window, WindowAppearance, rgb};
use nestodo_desktop::prefs::Appearance;

pub const FIGTREE: &str = "Figtree";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub bg: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
    pub hover_on_bg: Hsla,
    pub hover_strong_on_bg: Hsla,
    pub guide: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub matched: Hsla,
    pub grid: Hsla,
    pub accent_strong: Hsla,
    pub accent_soft: Hsla,
    pub accent_soft_on_bg: Hsla,
    pub added: Hsla,
    pub added_strong: Hsla,
    pub heat: [Hsla; 5],
    pub selection: Hsla,
    pub backdrop: Hsla,
    pub shadow: Hsla,
}

fn hsla(color: u32) -> Hsla {
    rgb(color).into()
}

fn rgba(color: u32) -> Hsla {
    let [r, g, b, a] = color.to_be_bytes().map(|channel| f32::from(channel) / 255.);
    Rgba { r, g, b, a }.into()
}

impl Theme {
    pub fn light() -> Self {
        Self {
            dark: false,
            bg: hsla(0xf1edfa),
            surface: hsla(0xffffff),
            text: hsla(0x1e1a2b),
            muted: hsla(0x665f78),
            border: hsla(0xe2dbf1),
            hover: hsla(0xf4f0fc),
            hover_on_bg: hsla(0xe6dff5),
            hover_strong_on_bg: hsla(0xd4cbea),
            guide: hsla(0xcdbcfa),
            accent: hsla(0x53419b),
            on_accent: hsla(0xffffff),
            danger: hsla(0xc4372b),
            matched: hsla(0xfbe3a1),
            grid: hsla(0xefebf7),
            accent_strong: hsla(0x3b2c7a),
            accent_soft: hsla(0xece5fd),
            accent_soft_on_bg: hsla(0xdcd2f8),
            added: hsla(0xe07a2e),
            added_strong: hsla(0xb05a1c),
            heat: [
                hsla(0xeee9f8),
                hsla(0xdcd2f8),
                hsla(0xae99ee),
                hsla(0x7c61d2),
                hsla(0x4a3a8c),
            ],
            selection: rgba(0x53419b40),
            backdrop: rgba(0x1e1a2b57),
            shadow: rgba(0x2a2150ff),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: hsla(0x17122a),
            surface: hsla(0x211a36),
            text: hsla(0xf3eeff),
            muted: hsla(0xa69ec0),
            border: hsla(0x382f55),
            hover: hsla(0x2b2345),
            hover_on_bg: hsla(0x2b2345),
            hover_strong_on_bg: hsla(0x382f55),
            guide: hsla(0x54469a),
            accent: hsla(0xcdbdff),
            on_accent: hsla(0x2a2150),
            danger: hsla(0xf07468),
            matched: hsla(0x6b5310),
            grid: hsla(0x2e2648),
            accent_strong: hsla(0xece5ff),
            accent_soft: hsla(0x33295a),
            accent_soft_on_bg: hsla(0x33295a),
            added: hsla(0xe8894a),
            added_strong: hsla(0xf6b585),
            heat: [
                hsla(0x2b2345),
                hsla(0x3f3470),
                hsla(0x5e4da6),
                hsla(0x8f79e0),
                hsla(0xcdbdff),
            ],
            selection: rgba(0xcdbdff55),
            backdrop: rgba(0x0806108c),
            shadow: rgba(0x000000ff),
        }
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
        }
    }

    pub fn heat_color(&self, level: u8) -> Hsla {
        self.heat[usize::from(level.min(4))]
    }
}

impl Global for Theme {}

pub fn load_fonts(cx: &App) {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("fonts/Figtree-Bold.ttf").as_slice()),
            Cow::Borrowed(include_bytes!("fonts/Figtree-ExtraBold.ttf").as_slice()),
        ])
        .expect("the bundled fonts load");
}

pub fn theme(cx: &App) -> Theme {
    *cx.global::<Theme>()
}

pub fn sync_theme(appearance: Appearance, window: &Window, cx: &mut App) {
    let next = match appearance {
        Appearance::System => Theme::for_appearance(window.appearance()),
        Appearance::Light => Theme::light(),
        Appearance::Dark => Theme::dark(),
    };
    if !cx.has_global::<Theme>() || *cx.global::<Theme>() != next {
        cx.set_global(next);
        cx.refresh_windows();
    }
}

pub fn apply_appearance(appearance: Appearance, window: &Window, cx: &mut App) {
    cx.set_window_appearance(match appearance {
        Appearance::System => None,
        Appearance::Light => Some(WindowAppearance::Light),
        Appearance::Dark => Some(WindowAppearance::Dark),
    });
    sync_theme(appearance, window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Hsla) -> f32 {
        let Rgba { r, g, b, .. } = color.to_rgb();
        let linear = |channel: f32| {
            if channel <= 0.040_45 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn the_outline_without_a_card_highlights_rows_as_visibly_as_the_card_did() {
        for theme in [Theme::light(), Theme::dark()] {
            let mode = if theme.dark { "dark" } else { "light" };
            assert!(
                contrast(theme.bg, theme.hover_on_bg) >= contrast(theme.surface, theme.hover),
                "the row being edited fades into the page in {mode}"
            );
            assert!(
                contrast(theme.hover_on_bg, theme.hover_strong_on_bg)
                    >= contrast(theme.hover, theme.border),
                "a row's buttons lose their hover in {mode}"
            );
            assert!(
                contrast(theme.bg, theme.accent_soft_on_bg)
                    >= contrast(theme.surface, theme.accent_soft),
                "a collapsed parent's bullet fades into the page in {mode}"
            );
            assert!(
                contrast(theme.hover_on_bg, theme.accent_soft_on_bg)
                    >= contrast(theme.hover, theme.accent_soft),
                "a collapsed parent's bullet fades into its highlighted row in {mode}"
            );
        }
    }
}
