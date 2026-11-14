use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    Light,
    Dark,
    #[default]
    #[serde(other)]
    System,
}

impl Appearance {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub show_completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_id: Option<String>,
    pub appearance: Appearance,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_completed: true,
            base_url: None,
            usage_id: None,
            appearance: Appearance::System,
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
        let written = path.with_extension("json.tmp");
        let result = fs::write(&written, serde_json::to_vec_pretty(self)?)
            .and_then(|()| fs::rename(&written, path));
        if result.is_err() {
            let _ = fs::remove_file(&written);
        }
        result
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
            usage_id: None,
            ..Prefs::default()
        }
        .save(Some(&path))
        .unwrap();
        assert!(!Prefs::load(Some(&path)).show_completed);
    }

    #[test]
    fn keeps_the_usage_data_id_across_launches_so_this_mac_is_counted_once() {
        let dir = TempDir::new("usage");
        let path = dir.0.join("settings.json");
        let prefs = Prefs {
            usage_id: Some("0f8fad5b-d9cb-469f-a165-70867728950e".into()),
            ..Prefs::default()
        };
        prefs.save(Some(&path)).unwrap();
        assert_eq!(Prefs::load(Some(&path)), prefs);
    }

    #[test]
    fn keeps_the_chosen_appearance_across_launches_so_the_window_opens_in_it() {
        let dir = TempDir::new("appearance");
        let path = dir.0.join("settings.json");
        let prefs = Prefs {
            appearance: Appearance::Dark,
            ..Prefs::default()
        };
        prefs.save(Some(&path)).unwrap();
        assert_eq!(Prefs::load(Some(&path)).appearance, Appearance::Dark);
    }

    #[test]
    fn an_appearance_this_version_does_not_know_follows_the_system_and_keeps_the_server() {
        let dir = TempDir::new("unknown-appearance");
        fs::create_dir_all(&dir.0).unwrap();
        let path = dir.0.join("settings.json");
        fs::write(
            &path,
            r#"{"showCompleted": false, "baseUrl": "https://nestodo.example.com/", "appearance": "sepia"}"#,
        )
        .unwrap();
        assert_eq!(
            Prefs::load(Some(&path)),
            Prefs {
                show_completed: false,
                base_url: Some("https://nestodo.example.com/".into()),
                usage_id: None,
                appearance: Appearance::System,
            }
        );
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
            usage_id: None,
            ..Prefs::default()
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
                usage_id: None,
                ..Prefs::default()
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

    #[test]
    fn a_save_that_fails_leaves_the_previous_file_whole_so_the_saved_server_survives_a_restart() {
        let dir = TempDir::new("whole");
        let path = dir.0.join("settings.json");
        let saved = Prefs {
            show_completed: true,
            base_url: Some("https://nestodo.example.com/".into()),
            usage_id: None,
            ..Prefs::default()
        };
        saved.save(Some(&path)).unwrap();
        fs::create_dir(path.with_extension("json.tmp")).unwrap();

        let failed = Prefs {
            show_completed: false,
            base_url: None,
            usage_id: None,
            ..Prefs::default()
        }
        .save(Some(&path));

        assert!(failed.is_err());
        assert_eq!(Prefs::load(Some(&path)), saved);
    }
}
