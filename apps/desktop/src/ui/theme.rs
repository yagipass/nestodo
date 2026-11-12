use std::borrow::Cow;

use gpui::{App, Global, Hsla, Rgba, Window, WindowAppearance, rgb};

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
    pub guide: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub matched: Hsla,
    pub grid: Hsla,
    pub accent_strong: Hsla,
    pub accent_soft: Hsla,
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
            guide: hsla(0xcdbcfa),
            accent: hsla(0x53419b),
            on_accent: hsla(0xffffff),
            danger: hsla(0xc4372b),
            matched: hsla(0xfbe3a1),
            grid: hsla(0xefebf7),
            accent_strong: hsla(0x3b2c7a),
            accent_soft: hsla(0xece5fd),
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
            guide: hsla(0x54469a),
            accent: hsla(0xcdbdff),
            on_accent: hsla(0x2a2150),
            danger: hsla(0xf07468),
            matched: hsla(0x6b5310),
            grid: hsla(0x2e2648),
            accent_strong: hsla(0xece5ff),
            accent_soft: hsla(0x33295a),
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

pub fn sync_theme(window: &Window, cx: &mut App) {
    let next = Theme::for_appearance(window.appearance());
    if !cx.has_global::<Theme>() || *cx.global::<Theme>() != next {
        cx.set_global(next);
        cx.refresh_windows();
    }
}
