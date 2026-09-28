//! User settings, stored as a small key = value file.
//!
//! A plain file rather than `NSUserDefaults` because the app also runs
//! unbundled (a bare binary has no bundle identifier to hang defaults on), and
//! because a readable file is easier to test, inspect and script.
//!
//! `MACFAN_SETTINGS` overrides the path, which is how the self test and the
//! crate tests avoid touching the real settings.

use std::fs;
use std::path::PathBuf;

/// Everything the app remembers between launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Show the menu bar icon (status item).
    pub show_status_item: bool,
    /// Show the app in the Dock.
    pub show_dock_icon: bool,
    /// Start without opening the settings window (used by the login agent).
    pub start_minimized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_status_item: true,
            show_dock_icon: true,
            start_minimized: false,
        }
    }
}

impl Settings {
    /// Where the settings live.
    pub fn path() -> PathBuf {
        if let Some(path) = std::env::var_os("MACFAN_SETTINGS") {
            return PathBuf::from(path);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        home.join("Library/Application Support/macfan/settings.conf")
    }

    /// Read the settings, falling back to defaults for anything missing or
    /// unreadable (never fails: a broken file must not stop the app).
    pub fn load() -> Settings {
        let Ok(text) = fs::read_to_string(Settings::path()) else {
            return Settings::default();
        };
        Settings::parse(&text)
    }

    /// Write the settings, creating the directory if needed.
    pub fn save(&self) -> Result<(), String> {
        let path = Settings::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        fs::write(&path, self.render()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Parse `key = value` lines; unknown keys and junk lines are ignored.
    pub fn parse(text: &str) -> Settings {
        let mut settings = Settings::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().eq_ignore_ascii_case("true");
            match key {
                "show_status_item" => settings.show_status_item = value,
                "show_dock_icon" => settings.show_dock_icon = value,
                "start_minimized" => settings.start_minimized = value,
                _ => {}
            }
        }
        settings
    }

    /// Serialise for the settings file.
    pub fn render(&self) -> String {
        format!(
            "# macfan settings\nshow_status_item = {}\nshow_dock_icon = {}\nstart_minimized = {}\n",
            self.show_status_item, self.show_dock_icon, self.start_minimized
        )
    }

    /// Hiding both the menu bar icon and the Dock icon would leave no way back
    /// into the app, so at least one of them stays visible.
    pub fn normalise(&mut self) -> bool {
        if !self.show_status_item && !self.show_dock_icon {
            self.show_status_item = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_visible_and_not_minimised() {
        let s = Settings::default();
        assert!(s.show_status_item);
        assert!(s.show_dock_icon);
        assert!(!s.start_minimized);
    }

    #[test]
    fn parse_ignores_junk_and_unknown_keys() {
        let s = Settings::parse(
            "# comment\n\nshow_status_item = false\nnonsense\nshow_dock_icon=TRUE\nunknown = false\n",
        );
        assert!(!s.show_status_item);
        assert!(s.show_dock_icon);
        // Missing keys keep their default.
        assert!(!s.start_minimized);
    }

    #[test]
    fn round_trip_through_a_file() {
        let path =
            std::env::temp_dir().join(format!("macfan-settings-test-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        std::env::set_var("MACFAN_SETTINGS", &path);

        assert_eq!(Settings::load(), Settings::default());
        let wanted = Settings {
            show_status_item: false,
            show_dock_icon: true,
            start_minimized: true,
        };
        wanted.save().expect("save writes the file");
        assert_eq!(Settings::load(), wanted);

        std::env::remove_var("MACFAN_SETTINGS");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn hiding_everything_is_corrected() {
        let mut s = Settings {
            show_status_item: false,
            show_dock_icon: false,
            start_minimized: true,
        };
        assert!(s.normalise());
        assert!(s.show_status_item);
        // A settings set that is already fine is left alone.
        let mut ok = Settings::default();
        assert!(!ok.normalise());
    }
}
