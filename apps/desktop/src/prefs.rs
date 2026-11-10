use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub show_completed: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_completed: true,
        }
    }
}

#[must_use]
pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("nestodo-desktop").join("settings.json"))
}

impl Prefs {
    #[must_use]
    pub fn load(path: Option<&Path>) -> Self {
        path.and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: Option<&Path>) {
        let Some(path) = path else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_completed_todos_until_the_user_hides_them_like_the_web_cookie() {
        assert!(Prefs::default().show_completed);
        let dir =
            std::env::temp_dir().join(format!("nestodo-desktop-prefs-{}", std::process::id()));
        let path = dir.join("settings.json");
        assert!(Prefs::load(Some(&path)).show_completed);
        Prefs {
            show_completed: false,
        }
        .save(Some(&path));
        assert!(!Prefs::load(Some(&path)).show_completed);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn falls_back_to_the_defaults_when_the_file_is_damaged() {
        let dir =
            std::env::temp_dir().join(format!("nestodo-desktop-prefs-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, "{not json").unwrap();
        assert_eq!(Prefs::load(Some(&path)), Prefs::default());
        let _ = fs::remove_dir_all(dir);
    }
}
