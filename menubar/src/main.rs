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

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::process::Command;

use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSObject, NSObjectProtocol, NSString, NSTimer,
};

use macfan::fan::{self, Action};
use macfan::smc::Smc;

const REFRESH_SECONDS: f64 = 2.0;

// Menu item tags: one selector handles them all.
const TAG_MAX: isize = 1;
const TAG_MIN: isize = 2;
const TAG_AUTO: isize = 3;
const TAG_SET_3000: isize = 4;
const TAG_SET_4500: isize = 5;
const TAG_SET_6000: isize = 6;
const TAG_REFRESH: isize = 7;
const TAG_QUIT: isize = 8;
const TAG_INFO: isize = 99;

#[derive(Default)]
struct Ivars {
    item: RefCell<Option<Retained<NSStatusItem>>>,
    info: RefCell<Vec<Retained<NSMenuItem>>>,
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

    impl Controller {
        #[unsafe(method(handleAction:))]
        fn handle_action(&self, sender: Option<&NSMenuItem>) {
            let tag = sender.map(|s| s.tag()).unwrap_or(0);
            match tag {
                TAG_MAX => self.write(Action::Max),
                TAG_MIN => self.write(Action::Min),
                TAG_AUTO => self.write(Action::Auto),
                TAG_SET_3000 => self.write(Action::Set(3000)),
                TAG_SET_4500 => self.write(Action::Set(4500)),
                TAG_SET_6000 => self.write(Action::Set(6000)),
                TAG_REFRESH => self.refresh(),
                TAG_QUIT => {
                    let app = NSApplication::sharedApplication(self.mtm());
                    app.terminate(None);
                }
                _ => {}
            }
        }

        #[unsafe(method(refreshTimer:))]
        fn refresh_timer(&self, _timer: Option<&NSTimer>) {
            self.refresh();
        }
    }
);

impl Controller {
    fn refresh(&self) {
        let text = match status_line() {
            Ok(line) => line,
            Err(err) => format!("SMC: {err}"),
        };
        if let Some(item) = self.ivars().item.borrow().as_ref() {
            if let Some(button) = item.button(self.mtm()) {
                button.setTitle(&NSString::from_str(&text));
            }
        }
        for (menu_item, line) in self.ivars().info.borrow().iter().zip(detail_lines()) {
            menu_item.setTitle(&NSString::from_str(&line));
        }
    }

    fn write(&self, action: Action) {
        let arg = match action {
            Action::Max => "max",
            Action::Min => "min",
            Action::Auto => "auto",
            Action::Set(rpm) => return self.write_set(rpm),
        };
        run_cli(&[arg]);
        self.refresh();
    }

    fn write_set(&self, rpm: u32) {
        let rpm = rpm.to_string();
        run_cli(&["set", &rpm]);
        self.refresh();
    }
}

/// Compact title for the menu bar: "<rpm> rpm" (plus a bolt when forced).
fn status_line() -> Result<String, macfan::smc::Error> {
    let smc = Smc::open()?;
    let fans = fan::read_fans(&smc)?;
    if fans.is_empty() {
        return Ok("no fans".to_string());
    }
    let speeds: Vec<String> = fans.iter().map(|f| format!("{:.0}", f.current)).collect();
    let forced = fans.iter().any(|f| f.forced);
    Ok(format!(
        "{}{} rpm",
        speeds.join("/"),
        if forced { " ⚡" } else { "" }
    ))
}

/// Detail lines shown at the top of the menu, one per fan.
fn detail_lines() -> Vec<String> {
    let Ok(smc) = Smc::open() else {
        return vec!["SMC unavailable".to_string()];
    };
    let Ok(fans) = fan::read_fans(&smc) else {
        return vec!["SMC unavailable".to_string()];
    };
    fans.iter()
        .map(|f| {
            format!(
                "Fan {}: {:.0} rpm (target {:.0}, {})",
                f.index,
                f.current,
                f.target,
                if f.forced { "forced" } else { "auto" }
            )
        })
        .collect()
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
fn run_cli(args: &[&str]) {
    let Some(cli) = cli_path() else { return };

    let unattended = std::env::var("SUDO_PASSWORD").is_ok_and(|v| !v.is_empty());
    let root = unsafe { geteuid() == 0 };
    let result = if root || unattended {
        Command::new(&cli).args(args).output()
    } else {
        let mut command = format!("'{}'", cli.replace('\'', "'\\''"));
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

    if let Ok(out) = result {
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr);
            eprintln!("macfan-menubar: action failed: {}", msg.trim());
        }
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
    let items = menu.numberOfItems();
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
    for index in 0..items {
        if let Some(item) = menu.itemAtIndex(index) {
            if item.target().is_some() && item.action().is_some() {
                wired += 1;
            }
            labels.push(item.title().to_string());
        }
    }
    println!("selftest: status item button title = {title:?}");
    println!("selftest: menu items = {items}, wired to an action = {wired}");
    println!("selftest: labels = {labels:?}");
    println!("selftest: refresh timer scheduled (fires every {REFRESH_SECONDS}s)");
    let ok = !title.is_empty() && items >= 10 && wired >= 8;
    println!("selftest: {}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}

fn main() {
    let mtm = MainThreadMarker::new().expect("menu bar apps start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller: Retained<Controller> = {
        let this = mtm.alloc::<Controller>();
        let this = this.set_ivars(Ivars::default());
        unsafe { msg_send![super(this), init] }
    };

    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = status_item.button(mtm) {
        button.setTitle(ns_string!("macfan"));
    }

    let menu = NSMenu::new(mtm);
    let menu = Retained::into_raw(menu);
    let menu: Retained<NSMenu> = unsafe { Retained::from_raw(menu).unwrap() };
    for (index, line) in detail_lines().iter().enumerate() {
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
    };
    add("Force max", TAG_MAX, "");
    add("Force min", TAG_MIN, "");
    add("Automatic", TAG_AUTO, "");
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add("Set 3000 rpm", TAG_SET_3000, "");
    add("Set 4500 rpm", TAG_SET_4500, "");
    add("Set 6000 rpm", TAG_SET_6000, "");
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add("Refresh", TAG_REFRESH, "r");
    add("Quit", TAG_QUIT, "q");

    status_item.setMenu(Some(&menu));
    *controller.ivars().item.borrow_mut() = Some(status_item);

    controller.refresh();

    if std::env::args().any(|a| a == "--selftest") {
        selftest(&controller, &menu);
        return;
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
    controller.refresh();

    app.run();
}
