use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Filter,
    Close,
    Completed,
    Account,
    Stats,
    Settings,
    Help,
    SignOut,
    Expand,
    Comment,
    Done,
    Link,
    Add,
    Tick,
    Edit,
    Trash,
    Reopen,
    Bot,
}

impl Icon {
    pub fn path(self) -> SharedString {
        SharedString::new_static(match self {
            Icon::Filter => "icons/filter.svg",
            Icon::Close => "icons/close.svg",
            Icon::Completed => "icons/completed.svg",
            Icon::Account => "icons/account.svg",
            Icon::Stats => "icons/stats.svg",
            Icon::Settings => "icons/settings.svg",
            Icon::Help => "icons/help.svg",
            Icon::SignOut => "icons/sign-out.svg",
            Icon::Expand => "icons/expand.svg",
            Icon::Comment => "icons/comment.svg",
            Icon::Done => "icons/done.svg",
            Icon::Link => "icons/link.svg",
            Icon::Add => "icons/add.svg",
            Icon::Tick => "icons/tick.svg",
            Icon::Edit => "icons/edit.svg",
            Icon::Trash => "icons/trash.svg",
            Icon::Reopen => "icons/reopen.svg",
            Icon::Bot => "icons/bot.svg",
        })
    }
}

fn line(width: &str, body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
    )
}

fn svg_for(path: &str) -> Option<String> {
    Some(match path {
        "icons/filter.svg" => line(
            "2",
            r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5 20 20"/>"#,
        ),
        "icons/close.svg" => line("2", r#"<path d="M7 7l10 10M17 7 7 17"/>"#),
        "icons/completed.svg" => line(
            "2",
            r#"<rect x="3.5" y="3.5" width="17" height="17" rx="5"/><path d="m8.5 12.5 2.5 2.5 4.5-5"/>"#,
        ),
        "icons/account.svg" => line(
            "2",
            r#"<circle cx="12" cy="8.5" r="3.5"/><path d="M5.5 19.5c1.2-3 3.7-4.5 6.5-4.5s5.3 1.5 6.5 4.5"/>"#,
        ),
        "icons/stats.svg" => line("2", r#"<path d="M5.5 19.5v-5M12 19.5v-9M18.5 19.5v-14"/>"#),
        "icons/settings.svg" => line(
            "2",
            r#"<path d="M4 8h16M4 16h16"/><circle cx="15" cy="8" r="2" fill="black"/><circle cx="9" cy="16" r="2" fill="black"/>"#,
        ),
        "icons/help.svg" => line(
            "2",
            r#"<rect x="3.5" y="3.5" width="17" height="17" rx="5"/><path d="M9.8 9.6a2.3 2.3 0 1 1 3.1 2.1c-.6.3-.9.8-.9 1.4v.3"/><circle cx="12" cy="16.4" r=".6" fill="black"/>"#,
        ),
        "icons/sign-out.svg" => line(
            "2",
            r#"<path d="M10 4H7.5A3.5 3.5 0 0 0 4 7.5v9A3.5 3.5 0 0 0 7.5 20H10M15 8l4 4-4 4M19 12h-9"/>"#,
        ),
        "icons/expand.svg" => line("2.2", r#"<path d="M9.5 6.5 15 12l-5.5 5.5"/>"#),
        "icons/comment.svg" => line(
            "2",
            r#"<path d="M8 17.5h8.5A3.5 3.5 0 0 0 20 14V8a3.5 3.5 0 0 0-3.5-3.5h-9A3.5 3.5 0 0 0 4 8v12.5z"/>"#,
        ),
        "icons/done.svg" => line("2", r#"<rect x="4" y="4" width="16" height="16" rx="5"/>"#),
        "icons/link.svg" => line("2.6", r#"<path d="M8 16 16 8M10 8h6v6"/>"#),
        "icons/add.svg" => line("2", r#"<path d="M12 5.5v13M5.5 12h13"/>"#),
        "icons/tick.svg" => line("3.6", r#"<path d="M5 12l5 5 9-10"/>"#),
        "icons/edit.svg" => line("2", r#"<path d="M14.5 5.5l4 4L9 19H5v-4z"/>"#),
        "icons/trash.svg" => line("2", r#"<path d="M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12"/>"#),
        "icons/reopen.svg" => line(
            "3",
            r#"<path d="M4.5 12a7.5 7.5 0 1 0 2.2-5.3M4.5 4v4h4"/>"#,
        ),
        "icons/bot.svg" => line(
            "2",
            r#"<rect x="5" y="8.5" width="14" height="10.5" rx="3"/><path d="M12 8.5V5"/><circle cx="9.5" cy="13.5" r=".9" fill="black"/><circle cx="14.5" cy="13.5" r=".9" fill="black"/>"#,
        ),
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
