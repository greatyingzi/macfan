//! Tiny UI localisation.
//!
//! The menu bar app follows the system language (that is what
//! `NSLocale.preferredLanguages()` reports). `MACFAN_LANG` overrides it, which
//! is what the self test uses to exercise every language.
//!
//! Only the GUI is localised: the CLI keeps its English output, because that
//! output is compared byte for byte against the classic `smc` tool.

/// Languages the UI ships with.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    /// English (also the fallback for languages we do not translate)
    #[default]
    En,
    /// Simplified Chinese
    ZhHans,
    /// Traditional Chinese
    ZhHant,
    /// Japanese
    Ja,
}

impl Lang {
    /// System language, or `MACFAN_LANG` when set.
    pub fn detect() -> Lang {
        if let Ok(tag) = std::env::var("MACFAN_LANG") {
            if !tag.is_empty() {
                return Lang::from_tag(&tag);
            }
        }
        Lang::from_tag(&system_tag())
    }

    /// Map an Apple language tag (`zh-Hans-CN`, `en-US`, `ja-JP`, …) to a
    /// supported language.
    pub fn from_tag(tag: &str) -> Lang {
        let tag = tag.to_ascii_lowercase();
        if tag.starts_with("zh") {
            // Traditional regions/scripts; everything else Chinese is simplified.
            if tag.contains("hant")
                || tag.contains("tw")
                || tag.contains("hk")
                || tag.contains("mo")
            {
                Lang::ZhHant
            } else {
                Lang::ZhHans
            }
        } else if tag.starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }

    /// Canonical tag of this language.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::ZhHans => "zh-Hans",
            Lang::ZhHant => "zh-Hant",
            Lang::Ja => "ja",
        }
    }

    /// All supported languages, for tests and future menus.
    pub fn all() -> [Lang; 4] {
        [Lang::En, Lang::ZhHans, Lang::ZhHant, Lang::Ja]
    }
}

#[cfg(target_os = "macos")]
fn system_tag() -> String {
    objc2_foundation::NSLocale::preferredLanguages()
        .firstObject()
        .map(|tag| tag.to_string())
        .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn system_tag() -> String {
    std::env::var("LANG").unwrap_or_default()
}

/// Every string the menu bar UI shows.
pub struct Strings {
    /// `true` for languages that use CJK punctuation.
    cjk: bool,
    /// Menu item: force maximum speed.
    pub force_max: &'static str,
    /// Menu item: force minimum speed.
    pub force_min: &'static str,
    /// Menu item: hand the fans back to the system.
    pub automatic: &'static str,
    /// Menu item that opens the speed dialog.
    pub set_speed: &'static str,
    /// Menu item: install/uninstall the login agent.
    pub launch_at_login: &'static str,
    /// Menu item: re-read the fans.
    pub refresh: &'static str,
    /// Menu item: quit.
    pub quit: &'static str,
    /// Dialog title for the speed prompt.
    pub dialog_title: &'static str,
    /// Dialog body, `{min}`/`{max}`/`{current}` are substituted.
    pub dialog_message: &'static str,
    /// Dialog: confirm button.
    pub ok: &'static str,
    /// Dialog: cancel button.
    pub cancel: &'static str,
    /// Dialog: the input was not a usable number.
    pub invalid_number: &'static str,
    /// Alert body: the SMC did not report the requested speed yet.
    pub not_settled: &'static str,
    /// Menu line: word for a fan.
    fan_word: &'static str,
    /// Menu line: word for the target speed.
    target_word: &'static str,
    /// Menu line: the fan is under manual control.
    forced_word: &'static str,
    /// Menu line: the fan is under system control.
    auto_word: &'static str,
    /// Preset item, `{rpm}` is substituted.
    preset_fmt: &'static str,
    /// Shown when no fan is reported.
    pub no_fans: &'static str,
    /// Shown when the SMC cannot be opened.
    pub smc_unavailable: &'static str,
    /// Prefix for login-agent errors, `{err}` is substituted.
    pub login_error: &'static str,
}

const EN: Strings = Strings {
    cjk: false,
    force_max: "Force maximum speed",
    force_min: "Force minimum speed",
    automatic: "Automatic (system control)",
    set_speed: "Set speed…",
    launch_at_login: "Launch at login",
    refresh: "Refresh",
    quit: "Quit",
    dialog_title: "Set fan speed",
    dialog_message: "Target speed in rpm (this Mac reports {min}–{max} rpm).",
    ok: "Set",
    cancel: "Cancel",
    invalid_number: "Enter a whole number of rpm.",
    not_settled: "The SMC has not reported the requested speed yet.",
    fan_word: "Fan",
    target_word: "target",
    forced_word: "forced",
    auto_word: "auto",
    preset_fmt: "Set {rpm} rpm",
    no_fans: "no fans",
    smc_unavailable: "SMC unavailable",
    login_error: "Launch at login: {err}",
};

const ZH_HANS: Strings = Strings {
    cjk: true,
    force_max: "强制最高转速",
    force_min: "强制最低转速",
    automatic: "自动（系统控制）",
    set_speed: "设定转速…",
    launch_at_login: "开机时启动",
    refresh: "刷新",
    quit: "退出",
    dialog_title: "设置风扇转速",
    dialog_message: "目标转速（rpm），本机范围 {min}–{max}。",
    ok: "设定",
    cancel: "取消",
    invalid_number: "请输入整数 rpm。",
    not_settled: "SMC 尚未报告请求的转速。",
    fan_word: "风扇",
    target_word: "目标",
    forced_word: "强制",
    auto_word: "自动",
    preset_fmt: "设定 {rpm} rpm",
    no_fans: "没有风扇",
    smc_unavailable: "SMC 不可用",
    login_error: "开机启动：{err}",
};

const ZH_HANT: Strings = Strings {
    cjk: true,
    force_max: "強制最高轉速",
    force_min: "強制最低轉速",
    automatic: "自動（系統控制）",
    set_speed: "設定轉速…",
    launch_at_login: "開機時啟動",
    refresh: "重新整理",
    quit: "結束",
    dialog_title: "設定風扇轉速",
    dialog_message: "目標轉速（rpm），本機範圍 {min}–{max}。",
    ok: "設定",
    cancel: "取消",
    invalid_number: "請輸入整數 rpm。",
    not_settled: "SMC 尚未回報請求的轉速。",
    fan_word: "風扇",
    target_word: "目標",
    forced_word: "強制",
    auto_word: "自動",
    preset_fmt: "設定 {rpm} rpm",
    no_fans: "沒有風扇",
    smc_unavailable: "SMC 無法使用",
    login_error: "開機啟動：{err}",
};

const JA: Strings = Strings {
    cjk: true,
    force_max: "最高回転数に固定",
    force_min: "最低回転数に固定",
    automatic: "自動（システム制御）",
    set_speed: "回転数を設定…",
    launch_at_login: "ログイン時に起動",
    refresh: "更新",
    quit: "終了",
    dialog_title: "ファン回転数を設定",
    dialog_message: "目標回転数（rpm）。この Mac の範囲は {min}–{max} です。",
    ok: "設定",
    cancel: "キャンセル",
    invalid_number: "整数の rpm を入力してください。",
    not_settled: "SMC はまだ要求した回転数を報告していません。",
    fan_word: "ファン",
    target_word: "目標",
    forced_word: "強制",
    auto_word: "自動",
    preset_fmt: "{rpm} rpm に設定",
    no_fans: "ファンがありません",
    smc_unavailable: "SMC を利用できません",
    login_error: "ログイン時に起動：{err}",
};

/// Strings for a language.
pub fn strings(lang: Lang) -> &'static Strings {
    match lang {
        Lang::En => &EN,
        Lang::ZhHans => &ZH_HANS,
        Lang::ZhHant => &ZH_HANT,
        Lang::Ja => &JA,
    }
}

impl Strings {
    /// One-line summary for a fan, e.g. `Fan 0: 7169 rpm (target 7199, forced)`.
    pub fn fan_line(&self, index: usize, current: f64, target: f64, forced: bool) -> String {
        let mode = if forced {
            self.forced_word
        } else {
            self.auto_word
        };
        if self.cjk {
            format!(
                "{} {}：{:.0} rpm（{} {:.0}，{}）",
                self.fan_word, index, current, self.target_word, target, mode
            )
        } else {
            format!(
                "{} {}: {:.0} rpm ({} {:.0}, {})",
                self.fan_word, index, current, self.target_word, target, mode
            )
        }
    }

    /// A preset menu item label, e.g. `Set 4500 rpm`.
    pub fn preset(&self, rpm: u32) -> String {
        self.preset_fmt.replace("{rpm}", &rpm.to_string())
    }

    /// The dialog body with the machine's range filled in.
    pub fn speed_message(&self, min: u32, max: u32) -> String {
        self.dialog_message
            .replace("{min}", &min.to_string())
            .replace("{max}", &max.to_string())
    }

    /// A login-agent error message.
    pub fn login_error_message(&self, err: &str) -> String {
        self.login_error.replace("{err}", err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_languages_from_apple_tags() {
        assert_eq!(Lang::from_tag("en-US"), Lang::En);
        assert_eq!(Lang::from_tag("zh-Hans-CN"), Lang::ZhHans);
        assert_eq!(Lang::from_tag("zh-CN"), Lang::ZhHans);
        assert_eq!(Lang::from_tag("zh-Hant-TW"), Lang::ZhHant);
        assert_eq!(Lang::from_tag("zh-HK"), Lang::ZhHant);
        assert_eq!(Lang::from_tag("ja-JP"), Lang::Ja);
        // Unsupported languages fall back to English rather than showing nothing.
        assert_eq!(Lang::from_tag("de-DE"), Lang::En);
        assert_eq!(Lang::from_tag(""), Lang::En);
    }

    #[test]
    fn every_language_has_all_strings_filled_in() {
        for lang in Lang::all() {
            let s = strings(lang);
            for value in [
                s.force_max,
                s.force_min,
                s.automatic,
                s.set_speed,
                s.launch_at_login,
                s.refresh,
                s.quit,
                s.dialog_title,
                s.dialog_message,
                s.ok,
                s.cancel,
                s.invalid_number,
                s.not_settled,
                s.fan_word,
                s.target_word,
                s.forced_word,
                s.auto_word,
                s.preset_fmt,
                s.no_fans,
                s.smc_unavailable,
                s.login_error,
            ] {
                assert!(!value.trim().is_empty(), "{lang:?} has an empty string");
            }
            assert!(s.preset(4500).contains("4500"));
            assert!(s.speed_message(1199, 7199).contains("1199"));
        }
    }

    #[test]
    fn fan_lines_use_language_appropriate_punctuation() {
        let en = strings(Lang::En).fan_line(0, 7169.4, 7199.0, true);
        assert_eq!(en, "Fan 0: 7169 rpm (target 7199, forced)");
        let zh = strings(Lang::ZhHans).fan_line(0, 7169.4, 7199.0, true);
        assert_eq!(zh, "风扇 0：7169 rpm（目标 7199，强制）");
        let ja = strings(Lang::Ja).fan_line(1, 2488.0, 2502.0, false);
        assert_eq!(ja, "ファン 1：2488 rpm（目標 2502，自動）");
    }
}
