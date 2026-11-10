use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Search,
    Close,
    ShowCompleted,
    Person,
    Stats,
    Settings,
    Help,
    SignOut,
    Server,
    Chevron,
    Note,
    Check,
}

impl Icon {
    pub fn path(self) -> SharedString {
        SharedString::new_static(match self {
            Icon::Search => "icons/search.svg",
            Icon::Close => "icons/close.svg",
            Icon::ShowCompleted => "icons/show-completed.svg",
            Icon::Person => "icons/person.svg",
            Icon::Stats => "icons/stats.svg",
            Icon::Settings => "icons/settings.svg",
            Icon::Help => "icons/help.svg",
            Icon::SignOut => "icons/sign-out.svg",
            Icon::Server => "icons/server.svg",
            Icon::Chevron => "icons/chevron.svg",
            Icon::Note => "icons/note.svg",
            Icon::Check => "icons/check.svg",
        })
    }
}

fn stroked(width: &str, linejoin: bool, body: &str) -> String {
    let join = if linejoin {
        r#" stroke-linejoin="round""#
    } else {
        ""
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="{width}" stroke-linecap="round"{join}>{body}</svg>"#
    )
}

fn svg_for(path: &str) -> Option<String> {
    Some(match path {
        "icons/search.svg" => stroked(
            "2",
            false,
            r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/>"#,
        ),
        "icons/close.svg" => stroked("2", false, r#"<path d="M6 6l12 12M18 6L6 18"/>"#),
        "icons/show-completed.svg" => stroked(
            "2",
            true,
            r#"<circle cx="12" cy="12" r="9"/><path d="M8.5 12.5l2.5 2.5 4.5-5"/>"#,
        ),
        "icons/person.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="black"><path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/></svg>"#.to_string(),
        "icons/stats.svg" => stroked("2", false, r#"<path d="M5 20v-8M12 20V4M19 20V9"/>"#),
        "icons/settings.svg" => stroked(
            "2",
            false,
            r#"<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>"#,
        ),
        "icons/help.svg" => stroked(
            "2",
            true,
            r#"<circle cx="12" cy="12" r="9"/><path d="M9.6 9.2a2.5 2.5 0 1 1 3.4 2.3c-.6.3-1 .9-1 1.6v.4M12 17h.01"/>"#,
        ),
        "icons/sign-out.svg" => stroked(
            "2",
            true,
            r#"<path d="M9 4H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h4M16 17l5-5-5-5M21 12H9"/>"#,
        ),
        "icons/server.svg" => stroked(
            "2",
            true,
            r#"<rect x="4" y="4" width="16" height="6" rx="1.5"/><rect x="4" y="14" width="16" height="6" rx="1.5"/><path d="M8 7h.01M8 17h.01"/>"#,
        ),
        "icons/chevron.svg" => stroked("2.4", true, r#"<path d="M9 6l6 6-6 6"/>"#),
        "icons/note.svg" => stroked(
            "2",
            true,
            r#"<path d="M6 3h9l4 4v14H6zM15 3v4h4M9 12h7M9 16h5"/>"#,
        ),
        "icons/check.svg" => stroked("2.4", true, r#"<path d="M5 12l5 5 9-10"/>"#),
        APP_ICON => include_str!("../../../web/public/icon.svg").to_string(),
        _ => return None,
    })
}

pub const APP_ICON: &str = "icons/app.svg";

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(svg_for(path).map(|svg| Cow::Owned(svg.into_bytes())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
