//! Privilege handling for SMC writes.
//!
//! Reading the SMC works for any user, but writing fan keys to the SMC is
//! rejected with `kIOReturnNotPrivileged` unless the process is root. We have
//! no interest in a setuid helper or a launchd daemon, so the pragmatic route
//! is the standard Unix one: re-run ourselves through `sudo`, once.

use std::io::Write;
use std::process::{Command, Stdio};

extern "C" {
    fn geteuid() -> u32;
    fn isatty(fd: i32) -> i32;
}

/// Are we already running as root?
pub fn is_root() -> bool {
    unsafe { geteuid() == 0 }
}

/// Is stdin a terminal (i.e. can `sudo` prompt the user)?
pub fn stdin_is_tty() -> bool {
    unsafe { isatty(0) == 1 }
}

/// Non-interactive password source, if the environment provides one. Kept
/// opt-in: nothing is read from disk, and an unset variable changes nothing.
pub fn password_from_env() -> Option<String> {
    std::env::var("SUDO_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty())
}

/// Re-run this program through `sudo` with the same arguments.
///
/// Returns the exit code to use. `stdin` mode: when a password comes from the
/// environment we pipe it to `sudo -S` (so it works from scripts and CI with
/// no terminal); otherwise we hand the terminal over (sudo prompts normally,
/// and reuses its cached credential afterwards).
pub fn rerun_with_sudo(args: &[String]) -> Result<i32, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;

    if let Some(password) = password_from_env() {
        let mut child = Command::new("sudo")
            .arg("-S")
            .arg("-p")
            .arg("")
            .arg(&exe)
            .args(args)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run sudo: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(password.as_bytes());
            let _ = stdin.write_all(b"\n");
            // Dropping stdin closes the pipe so sudo sees end-of-input.
        }
        let status = child
            .wait()
            .map_err(|e| format!("sudo did not finish: {e}"))?;
        return Ok(status.code().unwrap_or(1));
    }

    if !stdin_is_tty() {
        return Err(
            "writing fan settings needs root: run `sudo macfan …`, or set SUDO_PASSWORD for \
             non-interactive use, or pass --no-sudo"
                .to_string(),
        );
    }

    // Hand over the process: no double output, and the exit code is sudo's.
    use std::os::unix::process::CommandExt;
    let err = Command::new("sudo").arg(&exe).args(args).exec();
    Err(format!("cannot run sudo: {err}"))
}
