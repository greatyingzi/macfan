//! "Launch at login" through a per-user LaunchAgent.
//!
//! `SMAppService` (macOS 13+) is the modern route, but it wants a bundled,
//! signed app; a LaunchAgent works for a plain binary, for a bundle and on
//! macOS 11+, so that is what this uses. Installing writes the plist only —
//! launchd picks it up at the next login, which also avoids starting a second
//! copy of the app the moment the menu item is ticked.
//!
//! `MACFAN_AGENT_DIR` overrides the directory, which is how the self test
//! exercises install/remove without touching the real login items.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// launchd label / plist file name.
pub const LABEL: &str = "com.macfan.menubar";

/// Where the agent plist lives (or would live).
pub fn plist_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("MACFAN_AGENT_DIR") {
        return PathBuf::from(dir).join(format!("{LABEL}.plist"));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// The plist for a given executable path.
pub fn plist_contents(exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
        label = LABEL,
        exe = xml_escape(&exe.display().to_string()),
    )
}

/// Escape the characters that are not valid in plist XML text.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Is the agent installed?
pub fn is_enabled() -> bool {
    plist_path().exists()
}

/// Where the installed agent points, if it is installed and readable.
pub fn installed_target() -> Option<String> {
    let text = fs::read_to_string(plist_path()).ok()?;
    let start = text.find("<key>ProgramArguments</key>")?;
    let rest = &text[start..];
    let open = rest.find("<string>")? + "<string>".len();
    let close = rest[open..].find("</string>")? + open;
    Some(unescape_xml(&rest[open..close]))
}

/// Rewrite the agent when it points somewhere else — the app may have been
/// moved (a build directory to /Applications, for instance), and a login item
/// aimed at a path that no longer exists fails silently at login.
///
/// Returns the new path when a rewrite happened.
pub fn heal_if_stale() -> Option<PathBuf> {
    if !is_enabled() {
        return None;
    }
    let current = std::env::current_exe().ok()?.display().to_string();
    if installed_target().as_deref() == Some(current.as_str()) {
        return None;
    }
    enable().ok()
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Install the agent. Takes effect at the next login.
pub fn enable() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = plist_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::write(&path, plist_contents(&exe)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Remove the agent, and unload it if it happens to be loaded right now.
pub fn disable() -> Result<(), String> {
    let path = plist_path();
    if path.exists() {
        unload(&path);
        fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// `launchctl bootout` the agent, falling back to the older `unload`.
fn unload(path: &Path) {
    let Some(path) = path.to_str() else { return };
    if command_ok("launchctl", &["bootout", &domain(), path]) {
        return;
    }
    let _ = command_ok("launchctl", &["unload", "-w", path]);
}

fn domain() -> String {
    let uid = unsafe { geteuid() };
    format!("gui/{uid}")
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

extern "C" {
    fn geteuid() -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_is_well_formed_and_escapes_paths() {
        let text = plist_contents(Path::new("/Applications/Mac & Fan.app/macfan-menubar"));
        assert!(text.contains("<string>com.macfan.menubar</string>"));
        assert!(text.contains("<key>RunAtLoad</key>"));
        // The ampersand must be escaped, otherwise launchd rejects the plist.
        assert!(text.contains("/Applications/Mac &amp; Fan.app/macfan-menubar"));
        assert!(!text.contains("Mac & Fan"));
        assert_eq!(text.matches("<plist").count(), 1);
    }

    #[test]
    fn enable_and_disable_round_trip_in_a_temp_dir() {
        let dir = std::env::temp_dir().join(format!("macfan-agent-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        std::env::set_var("MACFAN_AGENT_DIR", &dir);

        assert!(!is_enabled());
        assert_eq!(plist_path(), dir.join(format!("{LABEL}.plist")));
        let path = enable().expect("enable writes the plist");
        assert!(is_enabled());
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("com.macfan.menubar"));
        disable().expect("disable removes it");
        assert!(!is_enabled());

        std::env::remove_var("MACFAN_AGENT_DIR");
        let _ = fs::remove_dir_all(&dir);
    }
}
