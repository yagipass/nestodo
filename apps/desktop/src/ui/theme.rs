use gpui::{App, Global, Hsla, Rgba, Window, WindowAppearance, rgb};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub bg: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
    pub bullet: Hsla,
    pub accent: Hsla,
    pub danger: Hsla,
    pub matched: Hsla,
    pub grid: Hsla,
    pub accent_strong: Hsla,
    pub accent_soft: Hsla,
    pub added: Hsla,
    pub added_strong: Hsla,
    pub heat: [Hsla; 4],
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
            bg: hsla(0xf6f6f4),
            surface: hsla(0xffffff),
            text: hsla(0x1c1c1a),
            muted: hsla(0x6b6b66),
            border: hsla(0xe2e2dd),
            hover: hsla(0xebebe7),
            bullet: hsla(0x3d3d39),
            accent: hsla(0x2f6fdb),
            danger: hsla(0xc4372b),
            matched: hsla(0xfbe3a1),
            grid: hsla(0xecece8),
            accent_strong: hsla(0x1d4fa8),
            accent_soft: hsla(0xeaf1fc),
            added: hsla(0xeb6834),
            added_strong: hsla(0xb8461a),
            heat: [
                hsla(0xb7d3f6),
                hsla(0x6da7ec),
                hsla(0x2a78d6),
                hsla(0x184f95),
            ],
            selection: rgba(0x2f6fdb40),
            backdrop: rgba(0x14141361),
            shadow: rgba(0x0000002e),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: hsla(0x141413),
            surface: hsla(0x1e1e1c),
            text: hsla(0xecece8),
            muted: hsla(0x9a9a93),
            border: hsla(0x33332f),
            hover: hsla(0x2a2a27),
            bullet: hsla(0xd6d6d0),
            accent: hsla(0x6b9cf0),
            danger: hsla(0xf07468),
            matched: hsla(0x6b5310),
            grid: hsla(0x2c2c29),
            accent_strong: hsla(0xa3c2f7),
            accent_soft: hsla(0x23304a),
            added: hsla(0xd95926),
            added_strong: hsla(0xfea382),
            heat: [
                hsla(0x184f95),
                hsla(0x256abf),
                hsla(0x3987e5),
                hsla(0x86b6ef),
            ],
            selection: rgba(0x6b9cf055),
            backdrop: rgba(0x14141361),
            shadow: rgba(0x00000066),
        }
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
        }
    }

    pub fn heat_color(&self, level: u8) -> Hsla {
        match level {
            0 => self.hover,
            level => self.heat[(level.min(4) - 1) as usize],
        }
    }
}

impl Global for Theme {}

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
