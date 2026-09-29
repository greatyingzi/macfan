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

/// What the menu bar title shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TitleStyle {
    /// Glyph plus the current speed (default).
    #[default]
    IconRpm,
    /// Speed only, as text.
    RpmOnly,
    /// Glyph plus the highest sampled core temperature.
    IconTemp,
}

impl TitleStyle {
    /// Name used in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            TitleStyle::IconRpm => "icon_rpm",
            TitleStyle::RpmOnly => "rpm_only",
            TitleStyle::IconTemp => "icon_temp",
        }
    }

    /// Parse a settings-file value, falling back to the default.
    pub fn parse(value: &str) -> TitleStyle {
        match value.trim() {
            "rpm_only" => TitleStyle::RpmOnly,
            "icon_temp" => TitleStyle::IconTemp,
            _ => TitleStyle::IconRpm,
        }
    }

    /// All styles, in menu order.
    pub fn all() -> [TitleStyle; 3] {
        [
            TitleStyle::IconRpm,
            TitleStyle::RpmOnly,
            TitleStyle::IconTemp,
        ]
    }
}

/// The speeds offered as menu presets when the settings have not been changed.
pub const DEFAULT_PRESETS: [u32; 3] = [3000, 4500, 6000];

/// Sanity bounds for a preset the user types in.
pub const PRESET_MIN: u32 = 100;
pub const PRESET_MAX: u32 = 20_000;

/// Everything the app remembers between launches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Show the menu bar icon (status item).
    pub show_status_item: bool,
    /// Show the app in the Dock.
    pub show_dock_icon: bool,
    /// Start without opening the settings window (used by the login agent).
    pub start_minimized: bool,
    /// The three speeds offered in the menu.
    pub presets: [u32; 3],
    /// What the menu bar title shows.
    pub title_style: TitleStyle,
    /// Language tag override; `None` follows the system.
    pub language: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_status_item: true,
            show_dock_icon: true,
            start_minimized: false,
            presets: DEFAULT_PRESETS,
            title_style: TitleStyle::default(),
            language: None,
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
            let raw_value = value;
            let value = value.trim().eq_ignore_ascii_case("true");
            match key {
                "show_status_item" => settings.show_status_item = value,
                "show_dock_icon" => settings.show_dock_icon = value,
                "start_minimized" => settings.start_minimized = value,
                "title_style" => settings.title_style = TitleStyle::parse(raw_value),
                "language" => {
                    settings.language = match raw_value.trim() {
                        "system" | "" => None,
                        tag => Some(tag.to_string()),
                    }
                }
                "preset1" | "preset2" | "preset3" => {
                    if let Ok(rpm) = raw_value.trim().parse::<u32>() {
                        let index =
                            key.chars().last().unwrap().to_digit(10).unwrap_or(1) as usize - 1;
                        if index < 3 {
                            settings.presets[index] = rpm;
                        }
                    }
                }
                _ => {}
            }
        }
        settings
    }

    /// Serialise for the settings file.
    pub fn render(&self) -> String {
        format!(
            "# macfan settings\nshow_status_item = {}\nshow_dock_icon = {}\nstart_minimized = {}\n\
             preset1 = {}\npreset2 = {}\npreset3 = {}\ntitle_style = {}\nlanguage = {}\n",
            self.show_status_item,
            self.show_dock_icon,
            self.start_minimized,
            self.presets[0],
            self.presets[1],
            self.presets[2],
            self.title_style.key(),
            self.language.as_deref().unwrap_or("system"),
        )
    }

    /// Hiding both the menu bar icon and the Dock icon would leave no way back
    /// into the app, so at least one of them stays visible.
    pub fn normalise(&mut self) -> bool {
        let mut changed = false;
        if !self.show_status_item && !self.show_dock_icon {
            self.show_status_item = true;
            changed = true;
        }
        // A preset has to be a usable speed; junk from the settings file, or a
        // field left empty in the window, falls back to the default.
        for (index, slot) in self.presets.iter_mut().enumerate() {
            if *slot < PRESET_MIN || *slot > PRESET_MAX {
                *slot = DEFAULT_PRESETS[index];
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// `MACFAN_SETTINGS` lives in the process environment, so the tests that
    /// set it cannot run concurrently.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

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
        let _guard = env_guard();
        let path =
            std::env::temp_dir().join(format!("macfan-settings-test-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        std::env::set_var("MACFAN_SETTINGS", &path);

        assert_eq!(Settings::load(), Settings::default());
        let wanted = Settings {
            show_status_item: false,
            show_dock_icon: true,
            start_minimized: true,
            ..Settings::default()
        };
        wanted.save().expect("save writes the file");
        assert_eq!(Settings::load(), wanted);

        std::env::remove_var("MACFAN_SETTINGS");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn presets_styles_and_language_round_trip() {
        let _guard = env_guard();
        let path =
            std::env::temp_dir().join(format!("macfan-settings-extra-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        std::env::set_var("MACFAN_SETTINGS", &path);

        let wanted = Settings {
            presets: [2500, 5000, 7000],
            title_style: TitleStyle::IconTemp,
            language: Some("ja".to_string()),
            ..Settings::default()
        };
        wanted.save().expect("save writes the file");
        assert_eq!(Settings::load(), wanted);

        // Unknown style falls back to the default; "system" clears the override.
        let parsed = Settings::parse("title_style = nonsense\nlanguage = system\npreset1 = 3300\n");
        assert_eq!(parsed.title_style, TitleStyle::IconRpm);
        assert_eq!(parsed.language, None);
        assert_eq!(parsed.presets[0], 3300);
        assert_eq!(parsed.presets[1], DEFAULT_PRESETS[1]);

        std::env::remove_var("MACFAN_SETTINGS");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn unusable_presets_fall_back() {
        let mut s = Settings {
            presets: [0, 50, 99_999],
            ..Settings::default()
        };
        assert!(s.normalise());
        assert_eq!(s.presets, DEFAULT_PRESETS);
    }

    #[test]
    fn hiding_everything_is_corrected() {
        let mut s = Settings {
            show_status_item: false,
            show_dock_icon: false,
            start_minimized: true,
            ..Settings::default()
        };
        assert!(s.normalise());
        assert!(s.show_status_item);
        // A settings set that is already fine is left alone.
        let mut ok = Settings::default();
        assert!(!ok.normalise());
    }
}
