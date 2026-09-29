//! `macfan-menubar` — a menu bar item for fan control.
//!
//! It reuses the `macfan` library for everything SMC-related (the same code the
//! CLI uses and that the parity checks cover); this crate only adds the AppKit
//! shell: a status item that shows the current speed and a menu with the
//! commands.
//!
//! Privileges: writing to the SMC needs root. When this process already runs as
//! root, or `SUDO_PASSWORD` is set in the environment (unattended use), the
//! bundled `macfan` CLI is asked to do the write and it handles sudo itself.
//! Otherwise the write is handed to `osascript` with administrator privileges,
//! which shows the standard macOS authorisation dialog — no privileged helper
//! and no code signing required.
//!
//! The UI follows the system language (see `i18n`); the CLI deliberately does
//! not, because its output is compared byte for byte with the classic tool.

#![allow(non_snake_case)]

mod i18n;
mod login_item;
mod settings;
mod window;

use std::cell::RefCell;
use std::process::Command;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSApplication, NSApplicationActivationPolicy,
    NSApplicationDelegate, NSButton, NSControlStateValueOff, NSControlStateValueOn, NSImage,
    NSMenu, NSMenuItem, NSPopUpButton, NSStatusBar, NSStatusItem, NSTextField,
    NSVariableStatusItemLength, NSWindow, NSWindowDelegate,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
    NSTimer,
};

use macfan::fan::{self, Action};
use macfan::smc::Smc;

use i18n::{Lang, Strings};
use settings::{Settings, TitleStyle, DEFAULT_PRESETS};

const REFRESH_SECONDS: f64 = 2.0;

// Menu item tags: one selector handles them all.
const TAG_MAX: isize = 1;
const TAG_MIN: isize = 2;
const TAG_AUTO: isize = 3;
const TAG_SET_DIALOG: isize = 4;
const TAG_SET_3000: isize = 5;
const TAG_SET_4500: isize = 6;
const TAG_SET_6000: isize = 7;
const TAG_LOGIN: isize = 8;
const TAG_REFRESH: isize = 9;
const TAG_QUIT: isize = 10;
const TAG_SETTINGS: isize = 11;
const TAG_CUSTOM: isize = 12;
/// First of the three preset entries (the others follow).
const TAG_PRESET_0: isize = TAG_SET_3000;
const TAG_STATE: isize = 13;
const TAG_INFO: isize = 99;

// Settings window checkboxes.
const TAG_SET_STATUS_ITEM: isize = 20;
const TAG_SET_DOCK_ICON: isize = 21;
const TAG_SET_LAUNCH_LOGIN: isize = 22;
const TAG_SET_START_MINIMIZED: isize = 23;

#[derive(Default)]
struct Ivars {
    item: RefCell<Option<Retained<NSStatusItem>>>,
    info: RefCell<Vec<Retained<NSMenuItem>>>,
    login: RefCell<Option<Retained<NSMenuItem>>>,
    lang: RefCell<Lang>,
    settings: RefCell<Settings>,
    window: RefCell<Option<Retained<NSWindow>>>,
    checks: RefCell<Vec<Retained<NSButton>>>,
    /// True when the status item carries a glyph, so the title can stay short.
    compact_title: RefCell<bool>,
    /// Action items that take a tick when they match the current state.
    actions: RefCell<Vec<(isize, Retained<NSMenuItem>)>>,
    /// Item that shows a custom speed, hidden unless one is in force.
    custom_item: RefCell<Option<Retained<NSMenuItem>>>,
    /// The "current state" line at the top of the menu.
    state_item: RefCell<Option<Retained<NSMenuItem>>>,
    /// The status item glyph, when the running system has one.
    symbol: RefCell<Option<Retained<NSImage>>>,
    /// The three preset fields in the settings window.
    preset_fields: RefCell<Vec<Retained<NSTextField>>>,
    /// The menu bar title radio buttons.
    style_buttons: RefCell<Vec<Retained<NSButton>>>,
    /// The language picker.
    language_popup: RefCell<Option<Retained<NSPopUpButton>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and Controller does not
    // implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Controller;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for Controller {}

    // SAFETY: NSApplicationDelegate has no safety requirements.
    unsafe impl NSApplicationDelegate for Controller {
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _sender: &NSApplication, _has_visible: bool) -> bool {
            // Clicking the Dock icon with no window open has to bring the
            // settings window back: with the menu bar icon switched off, that
            // is the only way into the app.
            self.show_window();
            true
        }
    }

    // SAFETY: NSWindowDelegate has no safety requirements.
    unsafe impl NSWindowDelegate for Controller {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            // Closing the window hides it; the app keeps running in the menu bar.
            self.hide_window();
            false
        }
    }

    impl Controller {
        #[unsafe(method(handleAction:))]
        fn handle_action(&self, sender: Option<&NSMenuItem>) {
            let tag = sender.map(|s| s.tag()).unwrap_or(0);
            match tag {
                TAG_MAX => self.write(Action::Max),
                TAG_MIN => self.write(Action::Min),
                TAG_AUTO => self.write(Action::Auto),
                TAG_SET_DIALOG => self.prompt_for_speed(),
                TAG_SET_3000 => self.write(Action::Set(3000)),
                TAG_SET_4500 => self.write(Action::Set(4500)),
                TAG_SET_6000 => self.write(Action::Set(6000)),
                TAG_LOGIN => self.toggle_login(),
                TAG_SETTINGS => self.show_window(),
                TAG_REFRESH => self.refresh(),
                TAG_QUIT => {
                    let app = NSApplication::sharedApplication(self.mtm());
                    app.terminate(None);
                }
                _ => {}
            }
        }

        #[unsafe(method(toggleSetting:))]
        fn toggle_setting(&self, sender: Option<&NSButton>) {
            let Some(sender) = sender else { return };
            let tag = sender.tag();
            let on = sender.state() == NSControlStateValueOn;
            {
                let mut settings = self.ivars().settings.borrow_mut();
                match tag {
                    TAG_SET_STATUS_ITEM => settings.show_status_item = on,
                    TAG_SET_DOCK_ICON => settings.show_dock_icon = on,
                    TAG_SET_LAUNCH_LOGIN => {
                        // Derived from the plist, not stored: this one is not a
                        // preference, it is the login agent's existence.
                        let result = if on {
                            login_item::enable().map(|_| ())
                        } else {
                            login_item::disable()
                        };
                        if let Err(err) = result {
                            let s = self.strings();
                            let alert = NSAlert::new(self.mtm());
                            alert.setMessageText(&NSString::from_str(&s.login_error_message(&err)));
                            alert.addButtonWithTitle(&NSString::from_str(s.ok));
                            alert.runModal();
                        }
                    }
                    TAG_SET_START_MINIMIZED => settings.start_minimized = on,
                    _ => return,
                }
            }
            self.apply_settings();
        }

        #[unsafe(method(applyPresets:))]
        fn apply_presets(&self, _sender: Option<&NSButton>) {
            let typed: Vec<String> = self
                .ivars()
                .preset_fields
                .borrow()
                .iter()
                .map(|f| f.stringValue().to_string())
                .collect();
            match parse_presets(&typed) {
                Some(presets) => {
                    self.ivars().settings.borrow_mut().presets = presets;
                    self.apply_settings();
                    self.retitle_presets();
                    self.refresh();
                }
                None => {
                    let s = self.strings();
                    let alert = NSAlert::new(self.mtm());
                    alert.setMessageText(&NSString::from_str(s.invalid_number));
                    alert.addButtonWithTitle(&NSString::from_str(s.ok));
                    alert.runModal();
                }
            }
        }

        #[unsafe(method(titleStyleChanged:))]
        fn title_style_changed(&self, sender: Option<&NSButton>) {
            let Some(sender) = sender else { return };
            let styles = TitleStyle::all();
            if let Some(style) = styles.get(sender.tag().max(0) as usize) {
                self.ivars().settings.borrow_mut().title_style = *style;
                self.apply_settings();
                self.refresh();
            }
        }

        #[unsafe(method(languageChanged:))]
        fn language_changed(&self, sender: Option<&NSPopUpButton>) {
            let Some(sender) = sender else { return };
            let index = sender.indexOfSelectedItem().max(0) as usize;
            if let Some(tag) = window::language_tag(index) {
                self.ivars().settings.borrow_mut().language = tag.map(str::to_string);
                self.apply_settings();
            }
        }

        #[unsafe(method(refreshTimer:))]
        fn refresh_timer(&self, _timer: Option<&NSTimer>) {
            self.refresh();
        }
    }
);

impl Controller {
    fn strings(&self) -> &'static Strings {
        i18n::strings(*self.ivars().lang.borrow())
    }

    fn refresh(&self) {
        let s = self.strings();
        let settings = self.ivars().settings.borrow().clone();
        let text = match status_line(s, &settings) {
            Ok(line) => line,
            Err(err) => format!("{}: {err}", s.smc_unavailable),
        };
        if let Some(item) = self.ivars().item.borrow().as_ref() {
            if let Some(button) = item.button(self.mtm()) {
                button.setTitle(&NSString::from_str(&text));
                let glyph = match settings.title_style {
                    TitleStyle::RpmOnly => None,
                    _ => self.ivars().symbol.borrow().clone(),
                };
                button.setImage(glyph.as_deref());
            }
        }
        for (menu_item, line) in self.ivars().info.borrow().iter().zip(detail_lines(s)) {
            menu_item.setTitle(&NSString::from_str(&line));
        }
        self.refresh_state_ticks();
        self.refresh_login_state();
    }

    /// Menu titles of the presets follow the settings.
    fn retitle_presets(&self) {
        let s = self.strings();
        let presets = self.ivars().settings.borrow().presets;
        for (tag, item) in self.ivars().actions.borrow().iter() {
            if let Some(index) = preset_index(*tag) {
                item.setTitle(&NSString::from_str(&s.preset(presets[index])));
            }
        }
    }

    /// Make the window's controls say what the settings say.
    fn seed_controls(&self) {
        let settings = self.ivars().settings.borrow().clone();
        for (field, rpm) in self
            .ivars()
            .preset_fields
            .borrow()
            .iter()
            .zip(settings.presets)
        {
            field.setStringValue(&NSString::from_str(&rpm.to_string()));
        }
        for button in self.ivars().style_buttons.borrow().iter() {
            let index = TitleStyle::all()
                .iter()
                .position(|style| *style == settings.title_style);
            let on = index == Some(button.tag().max(0) as usize);
            button.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        if let Some(popup) = self.ivars().language_popup.borrow().as_ref() {
            let index = match settings.language.as_deref() {
                None => 0,
                Some(tag) => Lang::all()
                    .iter()
                    .position(|lang| lang.code() == tag)
                    .map(|i| i + 1)
                    .unwrap_or(0),
            };
            popup.selectItemAtIndex(index as isize);
        }
    }

    /// Show the settings window (and bring the app forward so it is usable).
    fn show_window(&self) {
        let mtm = self.mtm();
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        if let Some(window) = self.ivars().window.borrow().as_ref() {
            window.makeKeyAndOrderFront(None);
        }
    }

    /// Hide the settings window without quitting.
    fn hide_window(&self) {
        if let Some(window) = self.ivars().window.borrow().as_ref() {
            window.orderOut(None);
        }
    }

    /// Push the current settings into the running app and persist them.
    fn apply_settings(&self) {
        let mut settings = self.ivars().settings.borrow().clone();
        if settings.normalise() {
            // Hiding every entry point would leave no way back in; the checkbox
            // state below gets corrected to match.
            let alert = NSAlert::new(self.mtm());
            let s = self.strings();
            alert.setMessageText(&NSString::from_str(s.keep_one_visible));
            alert.addButtonWithTitle(&NSString::from_str(s.ok));
            alert.runModal();
        }
        *self.ivars().settings.borrow_mut() = settings.clone();

        let app = NSApplication::sharedApplication(self.mtm());
        app.setActivationPolicy(if settings.show_dock_icon {
            NSApplicationActivationPolicy::Regular
        } else {
            NSApplicationActivationPolicy::Accessory
        });

        if let Some(item) = self.ivars().item.borrow().as_ref() {
            item.setVisible(settings.show_status_item);
        }

        for check in self.ivars().checks.borrow().iter() {
            let on = match check.tag() {
                TAG_SET_STATUS_ITEM => settings.show_status_item,
                TAG_SET_DOCK_ICON => settings.show_dock_icon,
                TAG_SET_LAUNCH_LOGIN => login_item::is_enabled(),
                TAG_SET_START_MINIMIZED => settings.start_minimized,
                _ => continue,
            };
            check.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }

        if let Err(err) = settings.save() {
            eprintln!("macfan-menubar: settings not saved: {err}");
        }
    }

    /// Show which mode the fans are in: a status line plus a tick on the
    /// matching entry (or a custom-speed entry when the value is not a preset).
    fn refresh_state_ticks(&self) {
        let s = self.strings();
        let mode = current_mode(
            &read_fans_or_empty(),
            &self.ivars().settings.borrow().presets,
        );

        if let Some(item) = self.ivars().state_item.borrow().as_ref() {
            let text = match mode {
                Mode::Max => s.force_max.to_string(),
                Mode::Min => s.force_min.to_string(),
                Mode::Auto => s.automatic.to_string(),
                Mode::Preset(rpm) => s.preset(rpm),
                Mode::Custom(rpm) => s.custom_message(rpm),
                Mode::Mixed => s.state_mixed.to_string(),
            };
            item.setTitle(&NSString::from_str(&format!("{}: {}", s.state_label, text)));
        }

        // The custom entry only exists when a non-preset speed is in force.
        if let Some(item) = self.ivars().custom_item.borrow().as_ref() {
            match mode {
                Mode::Custom(rpm) => {
                    item.setTitle(&NSString::from_str(&s.custom_message(rpm)));
                    item.setHidden(false);
                    item.setState(NSControlStateValueOn);
                }
                _ => {
                    item.setHidden(true);
                    item.setState(NSControlStateValueOff);
                }
            }
        }

        for (tag, item) in self.ivars().actions.borrow().iter() {
            let on = match mode {
                Mode::Auto => *tag == TAG_AUTO,
                Mode::Max => *tag == TAG_MAX,
                Mode::Min => *tag == TAG_MIN,
                Mode::Preset(rpm) => {
                    self.ivars()
                        .settings
                        .borrow()
                        .presets
                        .iter()
                        .position(|p| *p == rpm)
                        .map(|i| TAG_PRESET_0 + i as isize)
                        == Some(*tag)
                }
                Mode::Custom(_) => *tag == TAG_CUSTOM,
                Mode::Mixed => false,
            };
            item.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
    }

    /// Keep the "launch at login" menu tick in sync with what is on disk.
    fn refresh_login_state(&self) {
        let state = if login_item::is_enabled() {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        };
        if let Some(item) = self.ivars().login.borrow().as_ref() {
            item.setState(state);
        }
        for check in self.ivars().checks.borrow().iter() {
            if check.tag() == TAG_SET_LAUNCH_LOGIN {
                check.setState(state);
            }
        }
    }

    fn write(&self, action: Action) {
        match action {
            Action::Max => self.apply(action, &["max"]),
            Action::Min => self.apply(action, &["min"]),
            Action::Auto => self.apply(action, &["auto"]),
            Action::Set(rpm) => self.write_set(rpm),
        }
    }

    fn write_set(&self, rpm: u32) {
        let arg = rpm.to_string();
        self.apply(Action::Set(rpm), &["set", &arg]);
    }

    /// Run the command, then check the SMC actually followed: the write is
    /// asynchronous, so "the helper exited 0" alone means very little.
    fn apply(&self, action: Action, args: &[&str]) {
        let before = read_fans_or_empty();
        if let Err(err) = run_cli(args) {
            let s = self.strings();
            let alert = NSAlert::new(self.mtm());
            alert.setMessageText(&NSString::from_str(&s.action_failed_message(&err)));
            alert.addButtonWithTitle(&NSString::from_str(s.ok));
            alert.runModal();
        }
        self.refresh();
        let after = read_fans_or_empty();
        if !before.is_empty() && !after.is_empty() && !fan::satisfied(action, &before, &after) {
            let s = self.strings();
            let alert = NSAlert::new(self.mtm());
            alert.setMessageText(&NSString::from_str(s.not_settled));
            alert.addButtonWithTitle(&NSString::from_str(s.ok));
            alert.runModal();
        }
    }

    /// "Set speed…": ask for a number, sanity check it, then hand it to the CLI.
    fn prompt_for_speed(&self) {
        let s = self.strings();
        let (min, max, current) = self.range_and_current();
        let mtm = self.mtm();

        let app = NSApplication::sharedApplication(mtm);
        // `activate` is macOS 14+, this crate targets 11+, so keep the older
        // call and accept the deprecation.
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str(s.dialog_title));
        alert.setInformativeText(&NSString::from_str(&s.speed_message(min, max)));

        let field = NSTextField::new(mtm);
        field.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(180.0, 24.0),
        ));
        field.setStringValue(&NSString::from_str(&current.to_string()));
        alert.setAccessoryView(Some(&field));
        alert.addButtonWithTitle(&NSString::from_str(s.ok));
        alert.addButtonWithTitle(&NSString::from_str(s.cancel));

        if alert.runModal() != NSAlertFirstButtonReturn {
            return;
        }
        match parse_rpm(&field.stringValue().to_string(), min, max) {
            Ok(rpm) => self.write_set(rpm),
            Err(_) => {
                let alert = NSAlert::new(mtm);
                alert.setMessageText(&NSString::from_str(s.invalid_number));
                alert.addButtonWithTitle(&NSString::from_str(s.ok));
                alert.runModal();
            }
        }
    }

    /// First fan's range and current target, used to seed the speed dialog.
    fn range_and_current(&self) -> (u32, u32, u32) {
        let Ok(smc) = Smc::open() else {
            return (0, 0, 0);
        };
        let Ok(fans) = fan::read_fans(&smc) else {
            return (0, 0, 0);
        };
        match fans.first() {
            Some(f) => (
                f.min.max(0.0) as u32,
                f.max.max(0.0) as u32,
                f.target.max(0.0) as u32,
            ),
            None => (0, 0, 0),
        }
    }

    /// Tick/untick "launch at login".
    fn toggle_login(&self) {
        let s = self.strings();
        let result = if login_item::is_enabled() {
            login_item::disable()
        } else {
            login_item::enable().map(|_| ())
        };
        if let Err(err) = result {
            let alert = NSAlert::new(self.mtm());
            alert.setMessageText(&NSString::from_str(&s.login_error_message(&err)));
            alert.addButtonWithTitle(&NSString::from_str(s.ok));
            alert.runModal();
        }
        self.refresh_login_state();
    }
}

/// Status item glyph: an SF Symbol, as a template image so it follows the menu
/// bar's light/dark appearance. Falls back to nothing on systems whose symbol
/// set predates `fanblades` (macOS 12).
fn status_item_image() -> Option<Retained<NSImage>> {
    for name in ["fanblades", "fanblades.fill", "wind"] {
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(name),
            None,
        );
        if let Some(image) = image {
            image.setTemplate(true);
            return Some(image);
        }
    }
    None
}

/// What the fans are doing right now, as a menu state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Auto,
    Max,
    Min,
    Preset(u32),
    Custom(u32),
    Mixed,
}

/// Classify the current fan state so the menu can tick the matching entry.
///
/// Comparison is by value, not by "what was last clicked": if another tool (or
/// the SMC itself) moves the fans, the ticks follow. One rpm of slack absorbs
/// the SMC's rounding.
fn current_mode(fans: &[fan::Fan], presets: &[u32; 3]) -> Mode {
    let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
    if fans.is_empty() {
        return Mode::Mixed;
    }
    if fans.iter().all(|f| !f.forced) {
        return Mode::Auto;
    }
    if fans.iter().any(|f| !f.forced) {
        return Mode::Mixed;
    }
    if fans.iter().all(|f| f.max > 0.0 && near(f.target, f.max)) {
        return Mode::Max;
    }
    if fans.iter().all(|f| f.min > 0.0 && near(f.target, f.min)) {
        return Mode::Min;
    }
    for preset in presets.iter().copied() {
        if fans.iter().all(|f| near(f.target, f64::from(preset))) {
            return Mode::Preset(preset);
        }
    }
    let first = fans[0].target;
    if fans.iter().all(|f| near(f.target, first)) && first > 0.0 {
        return Mode::Custom(first.round() as u32);
    }
    Mode::Mixed
}

/// Title for the menu bar, per the chosen style.
///
/// `IconRpm` relies on the glyph to say "speed", so it drops the " rpm" suffix;
/// `RpmOnly` has no glyph and keeps it; `IconTemp` shows the highest sampled
/// core temperature instead of the fan speed.
fn status_line(s: &Strings, settings: &Settings) -> Result<String, macfan::smc::Error> {
    let smc = Smc::open()?;
    if settings.title_style == TitleStyle::IconTemp {
        return Ok(match fan::sampled_max_temp(&smc) {
            Some(celsius) => format!("{celsius:.0}°C"),
            None => s.no_fans.to_string(),
        });
    }
    let fans = fan::read_fans(&smc)?;
    if fans.is_empty() {
        return Ok(s.no_fans.to_string());
    }
    let speeds: Vec<String> = fans.iter().map(|f| format!("{:.0}", f.current)).collect();
    let forced = fans.iter().any(|f| f.forced);
    let suffix = match settings.title_style {
        TitleStyle::RpmOnly => " rpm",
        _ => "",
    };
    Ok(format!(
        "{}{}{suffix}",
        speeds.join("/"),
        if forced { " ⚡" } else { "" }
    ))
}

/// Index of a preset menu tag, if it is one.
fn preset_index(tag: isize) -> Option<usize> {
    let index = tag - TAG_PRESET_0;
    (0..3).contains(&index).then_some(index as usize)
}

/// Parse the three preset fields; `None` when any of them is unusable.
fn parse_presets(fields: &[String]) -> Option<[u32; 3]> {
    if fields.len() < 3 {
        return None;
    }
    let mut out = [0u32; 3];
    for (index, text) in fields.iter().take(3).enumerate() {
        let value: u32 = text.trim().parse().ok()?;
        if !(settings::PRESET_MIN..=settings::PRESET_MAX).contains(&value) {
            return None;
        }
        out[index] = value;
    }
    Some(out)
}

/// Detail lines shown at the top of the menu, one per fan.
fn detail_lines(s: &Strings) -> Vec<String> {
    let Ok(smc) = Smc::open() else {
        return vec![s.smc_unavailable.to_string()];
    };
    let Ok(fans) = fan::read_fans(&smc) else {
        return vec![s.smc_unavailable.to_string()];
    };
    if fans.is_empty() {
        return vec![s.no_fans.to_string()];
    }
    fans.iter()
        .map(|f| s.fan_line(f.index, f.current, f.target, f.forced))
        .collect()
}

/// Parse what the user typed in the speed dialog.
///
/// The value is clamped to the machine's range, because a target outside
/// `[min, max]` is not something the SMC will honour anyway — asking for 800 rpm
/// on a fan whose floor is 1199 can only end at the floor.
fn parse_rpm(input: &str, min: u32, max: u32) -> Result<u32, String> {
    let value: u32 = input
        .trim()
        .parse()
        .map_err(|_| format!("{input:?} is not a whole number"))?;
    if value == 0 {
        return Err("0 rpm is not a fan speed".to_string());
    }
    let low = min.max(1);
    let high = if max >= low { max } else { u32::MAX };
    Ok(value.clamp(low, high))
}

/// Fans as they are right now, or nothing if the SMC cannot be read.
fn read_fans_or_empty() -> Vec<fan::Fan> {
    let Ok(smc) = Smc::open() else {
        return Vec::new();
    };
    fan::read_fans(&smc).unwrap_or_default()
}

/// Locate the macfan CLI: next to this executable first, then `macfan` on PATH.
fn cli_path() -> Option<String> {
    if let Some(explicit) = std::env::var_os("MACFAN_CLI") {
        return Some(explicit.to_string_lossy().into_owned());
    }
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.with_file_name("macfan");
        if sibling.is_file() {
            return Some(sibling.to_string_lossy().into_owned());
        }
    }
    Some("macfan".to_string())
}

/// Run a fan command. Root (or an unattended password) means the CLI can write
/// directly; otherwise ask macOS for authorisation.
fn run_cli(args: &[&str]) -> Result<(), String> {
    let Some(cli) = cli_path() else {
        return Err("the macfan command was not found (bundle or PATH)".to_string());
    };
    run_cli_at(&cli, args)
}

/// Run a fan command through a specific CLI path. Separate from `run_cli` so
/// the failure path can be exercised without a GUI.
fn run_cli_at(cli: &str, args: &[&str]) -> Result<(), String> {
    let unattended = std::env::var("SUDO_PASSWORD").is_ok_and(|v| !v.is_empty());
    let root = unsafe { geteuid() == 0 };
    let result = if root || unattended {
        Command::new(cli).args(args).output()
    } else {
        let mut command = shell_quote(cli);
        for a in args {
            command.push(' ');
            command.push_str(&shell_quote(a));
        }
        Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "do shell script \"{}\" with administrator privileges",
                command.replace('\\', "\\\\").replace('"', "\\\"")
            ))
            .output()
    };

    match result {
        Ok(out) if out.status.success() => Ok(()),
        // Cancelling the authorisation dialog lands here too, with osascript's
        // "User canceled." on stderr — say so instead of doing nothing.
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let msg = if stderr.is_empty() {
                format!("exit status {}", out.status)
            } else {
                stderr
            };
            Err(msg)
        }
        Err(err) => Err(format!("could not run {cli}: {err}")),
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

extern "C" {
    fn geteuid() -> u32;
}

/// Read back what we just built: proves the AppKit wiring without needing a
/// screen recorder (which an agent shell typically cannot get).
fn selftest(controller: &Controller, menu: &NSMenu) {
    let lang = *controller.ivars().lang.borrow();
    let title = controller
        .ivars()
        .item
        .borrow()
        .as_ref()
        .and_then(|i| i.button(controller.mtm()))
        .map(|b| b.title().to_string())
        .unwrap_or_default();

    let mut wired = 0;
    let mut labels = Vec::new();
    let mut ticked = 0;
    for index in 0..menu.numberOfItems() {
        if let Some(item) = menu.itemAtIndex(index) {
            if item.target().is_some() && item.action().is_some() {
                wired += 1;
            }
            if item.state() == NSControlStateValueOn {
                ticked += 1;
            }
            labels.push(item.title().to_string());
        }
    }

    println!("selftest: language = {lang:?} ({})", lang.code());
    let supported: Vec<&str> = Lang::all().iter().map(|l| l.code()).collect();
    println!("selftest: supported languages = {supported:?}");
    println!(
        "selftest: status item glyph = {}, title = {title:?}",
        if *controller.ivars().compact_title.borrow() {
            "SF Symbol"
        } else {
            "none (text only)"
        }
    );
    println!(
        "selftest: menu items = {}, wired to an action = {wired}, ticked = {ticked}",
        labels.len()
    );
    println!("selftest: labels = {labels:?}");

    // Launch-at-login plumbing, exercised in a throwaway directory.
    let dir = std::env::temp_dir().join(format!("macfan-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::env::set_var("MACFAN_AGENT_DIR", &dir);
    let login_ok = match login_item::enable() {
        Ok(path) => {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let ok = login_item::is_enabled()
                && text.contains("com.macfan.menubar")
                && text.contains("<key>RunAtLoad</key>")
                && login_item::disable().is_ok()
                && !login_item::is_enabled();
            println!(
                "selftest: launch agent round trip = {}",
                if ok { "ok" } else { "FAILED" }
            );
            ok
        }
        Err(err) => {
            println!("selftest: launch agent round trip = FAILED ({err})");
            false
        }
    };
    std::env::remove_var("MACFAN_AGENT_DIR");
    let _ = std::fs::remove_dir_all(&dir);

    // Speed parsing / clamping.
    let cases = [
        ("4500", 1199, 7199, Some(4500)),
        (" 3000 ", 1199, 7199, Some(3000)),
        ("99999", 1199, 7199, Some(7199)),
        ("800", 1199, 7199, Some(1199)),
        ("abc", 1199, 7199, None),
        ("0", 1199, 7199, None),
        ("-200", 1199, 7199, None),
    ];
    let mut passed = 0;
    for (input, min, max, expected) in cases {
        if parse_rpm(input, min, max).ok() == expected {
            passed += 1;
        } else {
            println!(
                "selftest: parse_rpm({input:?}) = {:?}, expected {expected:?}",
                parse_rpm(input, min, max)
            );
        }
    }
    let parse_ok = passed == cases.len();
    println!("selftest: parse_rpm = {passed}/{} cases", cases.len());

    // Settings window: four checkboxes, wired to the four settings.
    let checks = controller.ivars().checks.borrow();
    let check_titles: Vec<String> = checks.iter().map(|c| c.title().to_string()).collect();
    let settings_window_ok = controller.ivars().window.borrow().is_some()
        && checks.len() == 4
        && checks
            .iter()
            .all(|c| c.target().is_some() && c.action().is_some());
    let live = controller.ivars().settings.borrow().clone();
    let ticked: Vec<String> = (0..labels.len())
        .filter_map(|index| {
            menu.itemAtIndex(index as isize)
                .filter(|item| item.state() == NSControlStateValueOn && !item.isHidden())
                .map(|_| labels[index].clone())
        })
        .collect();
    println!(
        "selftest: mode = {:?}, ticked = {ticked:?}",
        current_mode(&read_fans_or_empty(), &live.presets)
    );
    println!(
        "selftest: settings window = {}, checkboxes = {} {check_titles:?}",
        if settings_window_ok { "ok" } else { "FAILED" },
        checks.len()
    );
    println!(
        "selftest: settings = status_item={} dock_icon={} start_minimized={}",
        live.show_status_item, live.show_dock_icon, live.start_minimized
    );

    // Settings file round trip, again in a throwaway location.
    let settings_file = std::env::temp_dir().join(format!(
        "macfan-settings-selftest-{}.conf",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&settings_file);
    std::env::set_var("MACFAN_SETTINGS", &settings_file);
    let wanted = Settings {
        show_status_item: false,
        show_dock_icon: true,
        start_minimized: true,
        ..Settings::default()
    };
    let settings_ok = wanted.save().is_ok() && Settings::load() == wanted;
    let mut both_hidden = Settings {
        show_status_item: false,
        show_dock_icon: false,
        start_minimized: false,
        ..Settings::default()
    };
    let guard_ok = both_hidden.normalise() && both_hidden.show_status_item;
    println!(
        "selftest: settings file round trip = {}, hide-everything guard = {}",
        if settings_ok { "ok" } else { "FAILED" },
        if guard_ok { "ok" } else { "FAILED" }
    );
    std::env::remove_var("MACFAN_SETTINGS");
    let _ = std::fs::remove_file(&settings_file);

    // A missing CLI must come back as an error, not as silence.
    let bogus = run_cli_at("/nonexistent/macfan", &["max"]);
    println!(
        "selftest: failing command surfaces an error = {}",
        if bogus.is_err() { "ok" } else { "FAILED" }
    );
    let bogus_ok = bogus.is_err();

    // The login item is rewritten when it points at something else.
    let dir = std::env::temp_dir().join(format!("macfan-heal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok();
    std::env::set_var("MACFAN_AGENT_DIR", &dir);
    let stale = dir.join("com.macfan.menubar.plist");
    std::fs::write(
        &stale,
        login_item::plist_contents(std::path::Path::new("/tmp/gone")),
    )
    .ok();
    let healed = login_item::heal_if_stale().is_some()
        && login_item::installed_target()
            == std::env::current_exe()
                .ok()
                .map(|p| p.display().to_string());
    println!(
        "selftest: stale login item self-heals = {}",
        if healed { "ok" } else { "FAILED" }
    );
    std::env::remove_var("MACFAN_AGENT_DIR");
    let _ = std::fs::remove_dir_all(&dir);

    // Preset parsing.
    let preset_cases: [(&[&str], bool); 4] = [
        (&["3000", "4500", "6000"], true),
        (&["2500", " 5000 ", "7000"], true),
        (&["3000", "4500", "0"], false),
        (&["3000", "abc", "6000"], false),
    ];
    let presets_ok = preset_cases.iter().all(|(fields, expected)| {
        let owned: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
        parse_presets(&owned).is_some() == *expected
    });
    println!(
        "selftest: preset parsing = {}/{} cases",
        preset_cases
            .iter()
            .filter(|(fields, expected)| {
                let owned: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
                parse_presets(&owned).is_some() == *expected
            })
            .count(),
        preset_cases.len()
    );

    // Menu titles follow the settings, and the title style reaches the status
    // item: set a custom preset triple, retitle, and read the menu back.
    let original = controller.ivars().settings.borrow().clone();
    controller.ivars().settings.borrow_mut().presets = [2000, 3333, 4444];
    controller.retitle_presets();
    let retitled: Vec<String> = (0..menu.numberOfItems())
        .filter_map(|index| menu.itemAtIndex(index as isize))
        .map(|item| item.title().to_string())
        .filter(|title| title.contains("2000") || title.contains("3333") || title.contains("4444"))
        .collect();
    let retitle_ok = retitled.len() == 3;
    println!("selftest: preset titles follow the settings = {retitled:?}");

    let mut style_ok = true;
    for style in TitleStyle::all() {
        controller.ivars().settings.borrow_mut().title_style = style;
        controller.refresh();
        let shown = controller
            .ivars()
            .item
            .borrow()
            .as_ref()
            .and_then(|item| item.button(controller.mtm()))
            .map(|button| (button.title().to_string(), button.image().is_some()))
            .unwrap_or_default();
        let good = match style {
            TitleStyle::RpmOnly => !shown.1 && shown.0.contains("rpm"),
            TitleStyle::IconRpm => shown.1 && !shown.0.contains("rpm"),
            TitleStyle::IconTemp => shown.1 && shown.0.contains("°C"),
        };
        println!(
            "selftest: title style {:?} -> {:?} (glyph={}) {}",
            style,
            shown.0,
            shown.1,
            if good { "ok" } else { "FAILED" }
        );
        style_ok &= good;
    }
    // The empty status item case: on a machine with no fan, IconTemp falls back
    // to a message rather than an empty title.
    let languages_ok = (0..6).filter_map(window::language_tag).count() == 5
        && window::language_tag(0) == Some(None);
    println!("selftest: language picker maps 5 entries = {languages_ok}");
    *controller.ivars().settings.borrow_mut() = original;
    controller.retitle_presets();
    controller.refresh();

    let ok = !title.is_empty()
        && presets_ok
        && retitle_ok
        && style_ok
        && languages_ok
        && bogus_ok
        && healed
        && labels.len() >= 15
        && wired >= 11
        && login_ok
        && parse_ok
        && settings_window_ok
        && settings_ok
        && guard_ok;
    println!("selftest: {}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}

/// `--login-status|--login-install|--login-remove`: control the login agent
/// from the command line (used by tests, and handy in a setup script).
fn login_item_command(args: &[String]) -> Option<i32> {
    let action = args.iter().find(|a| a.starts_with("--login-"))?;
    let result = match action.as_str() {
        "--login-status" => {
            let path = login_item::plist_path();
            println!(
                "login agent: {} ({})",
                if login_item::is_enabled() {
                    "installed"
                } else {
                    "not installed"
                },
                path.display()
            );
            return Some(0);
        }
        "--login-install" => login_item::enable().map(|path| {
            println!("login agent installed: {}", path.display());
        }),
        "--login-remove" => login_item::disable().map(|()| {
            println!("login agent removed");
        }),
        _ => return None,
    };
    Some(match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("login agent: {err}");
            1
        }
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = login_item_command(&args) {
        std::process::exit(code);
    }

    let mtm = MainThreadMarker::new().expect("menu bar apps start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let lang = Lang::detect();
    let s = i18n::strings(lang);
    let settings = Settings::load();
    let start_minimized = settings.start_minimized || args.iter().any(|a| a == "--minimized");

    let controller: Retained<Controller> = {
        let this = mtm.alloc::<Controller>();
        let this = this.set_ivars(Ivars {
            lang: RefCell::new(lang),
            settings: RefCell::new(settings),
            ..Ivars::default()
        });
        unsafe { msg_send![super(this), init] }
    };

    {
        let delegate: &ProtocolObject<dyn NSApplicationDelegate> =
            ProtocolObject::from_ref(&*controller);
        app.setDelegate(Some(delegate));
    }

    let settings_window = window::build(mtm, s, env!("CARGO_PKG_VERSION"), &*controller);
    {
        let delegate: &ProtocolObject<dyn NSWindowDelegate> =
            ProtocolObject::from_ref(&*controller);
        settings_window.window.setDelegate(Some(delegate));
    }
    *controller.ivars().window.borrow_mut() = Some(settings_window.window);
    *controller.ivars().checks.borrow_mut() = settings_window.checks;
    *controller.ivars().preset_fields.borrow_mut() = settings_window.preset_fields;
    *controller.ivars().style_buttons.borrow_mut() = settings_window.style_buttons;
    *controller.ivars().language_popup.borrow_mut() = Some(settings_window.language_popup);

    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);
    let symbol = status_item_image();
    *controller.ivars().symbol.borrow_mut() = symbol.clone();
    if let Some(button) = status_item.button(mtm) {
        button.setTitle(ns_string!("macfan"));
        if let Some(image) = symbol.as_ref() {
            button.setImage(Some(image));
        }
    }
    *controller.ivars().compact_title.borrow_mut() = symbol.is_some();

    let menu = NSMenu::new(mtm);

    // "Current: ..." line, disabled so it reads as information.
    let state_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(s.state_label),
            None,
            ns_string!(""),
        )
    };
    state_item.setTag(TAG_STATE);
    state_item.setEnabled(false);
    menu.addItem(&state_item);
    *controller.ivars().state_item.borrow_mut() = Some(state_item);

    for (index, line) in detail_lines(s).iter().enumerate() {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str(line),
                None,
                ns_string!(""),
            )
        };
        item.setTag(TAG_INFO + index as isize);
        item.setEnabled(false);
        menu.addItem(&item);
        controller.ivars().info.borrow_mut().push(item);
    }
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let add = |title: &str, tag: isize, key: &str| {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str(title),
                Some(sel!(handleAction:)),
                &NSString::from_str(key),
            )
        };
        item.setTag(tag);
        unsafe { item.setTarget(Some(&controller)) };
        menu.addItem(&item);
        item
    };

    let collect = |item: Retained<NSMenuItem>, tag: isize| {
        controller.ivars().actions.borrow_mut().push((tag, item));
    };
    collect(add(s.force_max, TAG_MAX, ""), TAG_MAX);
    collect(add(s.force_min, TAG_MIN, ""), TAG_MIN);
    collect(add(s.automatic, TAG_AUTO, ""), TAG_AUTO);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    for (index, tag) in [TAG_SET_3000, TAG_SET_4500, TAG_SET_6000]
        .iter()
        .enumerate()
    {
        collect(add(&s.preset(DEFAULT_PRESETS[index]), *tag, ""), *tag);
    }
    // The custom entry belongs with the presets: it is the same kind of choice
    // (a fixed speed), and placing it after the dialog put the tick one row
    // below the entry the user had just used.
    let custom_item = add(&s.custom_message(0), TAG_CUSTOM, "");
    custom_item.setHidden(true);
    *controller.ivars().custom_item.borrow_mut() = Some(custom_item);
    collect(add(s.set_speed, TAG_SET_DIALOG, ""), TAG_SET_DIALOG);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    let login_entry = add(s.launch_at_login, TAG_LOGIN, "");
    *controller.ivars().login.borrow_mut() = Some(login_entry);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add(s.settings_menu, TAG_SETTINGS, ",");
    add(s.refresh, TAG_REFRESH, "r");
    add(s.quit, TAG_QUIT, "q");

    status_item.setMenu(Some(&menu));
    *controller.ivars().item.borrow_mut() = Some(status_item);

    if login_item::heal_if_stale().is_some() {
        eprintln!("macfan-menubar: the login item pointed elsewhere; rewritten for this app");
    }

    controller.apply_settings();
    controller.seed_controls();
    controller.retitle_presets();
    controller.refresh();

    if std::env::args().any(|a| a == "--selftest") {
        selftest(&controller, &menu);
        return;
    }

    if !start_minimized {
        controller.show_window();
    }

    unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            REFRESH_SECONDS,
            &controller,
            sel!(refreshTimer:),
            None,
            true,
        );
    }

    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quoting_survives_awkward_paths() {
        assert_eq!(
            shell_quote("/usr/local/bin/macfan"),
            "'/usr/local/bin/macfan'"
        );
        assert_eq!(
            shell_quote("/Applications/Mac Fan.app/macfan"),
            "'/Applications/Mac Fan.app/macfan'"
        );
        // A quote in the path must not end the quoted string.
        assert_eq!(shell_quote("/tmp/it's here"), "'/tmp/it'\\''s here'");
    }

    #[test]
    fn speed_input_is_validated_and_clamped() {
        assert_eq!(parse_rpm("4500", 1199, 7199), Ok(4500));
        assert_eq!(parse_rpm("  3000  ", 1199, 7199), Ok(3000));
        // Above the fan's ceiling: clamp down.
        assert_eq!(parse_rpm("99999", 1199, 7199), Ok(7199));
        // Below the floor: clamp up.
        assert_eq!(parse_rpm("800", 1199, 7199), Ok(1199));
        // Never acceptable.
        assert!(parse_rpm("", 1199, 7199).is_err());
        assert!(parse_rpm("abc", 1199, 7199).is_err());
        assert!(parse_rpm("0", 1199, 7199).is_err());
        assert!(parse_rpm("-200", 1199, 7199).is_err());
        assert!(parse_rpm("45.5", 1199, 7199).is_err());
        // Unknown range (SMC gave us nothing): only the zero check applies.
        assert_eq!(parse_rpm("4500", 0, 0), Ok(4500));
    }
}
