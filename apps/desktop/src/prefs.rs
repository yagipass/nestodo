use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub show_completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_completed: true,
            base_url: None,
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

    pub fn save(&self, path: Option<&Path>) -> io::Result<()> {
        let path = path.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "this Mac has no folder for settings",
            )
        })?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "nestodo-desktop-prefs-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn shows_completed_todos_until_the_user_hides_them_like_the_web_cookie() {
        assert!(Prefs::default().show_completed);
        let dir = TempDir::new("shown");
        let path = dir.0.join("settings.json");
        assert!(Prefs::load(Some(&path)).show_completed);
        Prefs {
            show_completed: false,
            base_url: None,
        }
        .save(Some(&path))
        .unwrap();
        assert!(!Prefs::load(Some(&path)).show_completed);
    }

    #[test]
    fn falls_back_to_the_defaults_when_the_file_is_damaged() {
        let dir = TempDir::new("bad");
        fs::create_dir_all(&dir.0).unwrap();
        let path = dir.0.join("settings.json");
        fs::write(&path, "{not json").unwrap();
        assert_eq!(Prefs::load(Some(&path)), Prefs::default());
    }

    #[test]
    fn remembers_the_server_so_the_next_launch_connects_without_asking() {
        let dir = TempDir::new("server");
        let path = dir.0.join("settings.json");
        let prefs = Prefs {
            show_completed: true,
            base_url: Some("https://nestodo.example.com/".into()),
        };
        prefs.save(Some(&path)).unwrap();
        assert_eq!(Prefs::load(Some(&path)), prefs);
    }

    #[test]
    fn a_settings_file_from_before_the_server_was_asked_for_keeps_its_settings_and_has_no_server() {
        let dir = TempDir::new("old");
        fs::create_dir_all(&dir.0).unwrap();
        let path = dir.0.join("settings.json");
        fs::write(&path, r#"{"showCompleted": false}"#).unwrap();
        assert_eq!(
            Prefs::load(Some(&path)),
            Prefs {
                show_completed: false,
                base_url: None,
            }
        );
    }

    #[test]
    fn a_save_that_does_not_reach_the_disk_is_reported_so_a_server_is_never_forgotten_silently() {
        let dir = TempDir::new("blocked");
        fs::create_dir_all(&dir.0).unwrap();
        let blocker = dir.0.join("not-a-folder");
        fs::write(&blocker, "").unwrap();
        assert!(
            Prefs::default()
                .save(Some(&blocker.join("settings.json")))
                .is_err()
        );
        assert!(Prefs::default().save(None).is_err());
    }
}
