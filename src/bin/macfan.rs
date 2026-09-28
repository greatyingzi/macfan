//! `macfan` — control the fans of a Mac through the SMC.
//!
//! Reads need no privileges. Writes do, so when the SMC rejects a write as
//! unprivileged this program re-runs itself through `sudo` (see `sudo`), which
//! keeps everything in one small binary instead of a privileged helper.

use std::process::ExitCode;
use std::time::Duration;

use macfan::fan::{self, Action, Fan, Write};
use macfan::smc::{Error, Smc};
use macfan::{sudo, VERSION};

fn help(prog: &str) -> String {
    format!(
        "macfan {VERSION} — control Mac fans through the SMC

Usage:
  {prog}                 show current fan speeds and mode
  {prog} status          same as above
  {prog} max             force all fans to their own maximum speed
  {prog} min             force all fans to their own minimum speed
  {prog} set <rpm>       force all fans to <rpm> (capped per fan)
  {prog} auto            hand the fans back to the system
  {prog} temps           list temperature sensors (°C)
  {prog} list            raw fan dump (alias of status)
  {prog} -V, --version   print version
  {prog} -h, --help      print this help

Options:
  --no-sudo              never re-run through sudo; report the error instead

Notes:
  - Writes go through sudo (one password prompt, then cached). For unattended
    use set SUDO_PASSWORD in the environment.
  - A forced fan is temporary: reboot, sleep or closing the lid returns it to
    system control. Use `auto` to give it back immediately.
"
    )
}

fn open_smc() -> Result<Smc, ExitCode> {
    Smc::open().map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })
}

fn read_fans(smc: &Smc) -> Result<Vec<Fan>, ExitCode> {
    fan::read_fans(smc).map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })
}

fn show_status() -> ExitCode {
    let smc = match open_smc() {
        Ok(smc) => smc,
        Err(code) => return code,
    };
    match read_fans(&smc) {
        Ok(fans) => {
            print!("{}", fan::render(&fans));
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

fn show_temps() -> ExitCode {
    let smc = match open_smc() {
        Ok(smc) => smc,
        Err(code) => return code,
    };
    match fan::temperature_readings(&smc) {
        Ok(readings) => {
            if readings.is_empty() {
                println!("this Mac exposes no readable temperature sensors");
            } else {
                for r in &readings {
                    println!("  {:<4}  {:>6.1} °C", r.key, r.celsius);
                }
                println!("  {} sensors", readings.len());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn apply(smc: &Smc, writes: &[Write]) -> Result<(), Error> {
    for w in writes {
        smc.write(&w.key, &w.bytes)?;
    }
    Ok(())
}

fn control(action: Action, argv: &[String], no_sudo: bool) -> ExitCode {
    let smc = match open_smc() {
        Ok(smc) => smc,
        Err(code) => return code,
    };
    let fans = match read_fans(&smc) {
        Ok(fans) => fans,
        Err(code) => return code,
    };
    if fans.is_empty() {
        eprintln!("Error: the SMC reports no fans");
        return ExitCode::from(1);
    }

    let writes = fan::plan(action, &fans);
    match apply(&smc, &writes) {
        Ok(()) => {
            println!("{}", action.headline(&fans));
            // The SMC applies a speed change a moment later, so poll until the
            // fans report what we asked for instead of printing a stale value.
            let (after, settled) =
                fan::wait_until_settled(&smc, action, &fans, Duration::from_millis(1200));
            print!("{}", fan::render(&after));
            if !settled {
                eprintln!(
                    "note: the SMC has not reported the requested speed yet; \
                     run `macfan status` to check again"
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) if err.is_not_privileged() && !sudo::is_root() && !no_sudo => {
            // Writes need root: re-run this exact command through sudo.
            match sudo::rerun_with_sudo(argv) {
                Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
                Err(msg) => {
                    eprintln!("Error: {msg}");
                    ExitCode::from(1)
                }
            }
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv.first().cloned().unwrap_or_else(|| "macfan".into());
    let args = &argv[1..];

    let mut command: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut no_sudo = false;

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{}", help(&prog));
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("macfan {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--no-sudo" => no_sudo = true,
            other if other.starts_with("--") => {
                eprintln!("Error: unknown option {other}");
                return ExitCode::from(2);
            }
            other if command.is_none() => command = Some(other.to_string()),
            other => rest.push(other.to_string()),
        }
    }

    let command = command.unwrap_or_else(|| "status".to_string());
    match command.as_str() {
        "status" | "list" => show_status(),
        "temps" | "temperature" | "temperatures" => show_temps(),
        "max" => control(Action::Max, args, no_sudo),
        "min" => control(Action::Min, args, no_sudo),
        "auto" => control(Action::Auto, args, no_sudo),
        "set" => {
            let Some(rpm) = rest.first() else {
                eprintln!("Error: set needs a speed, e.g. `{prog} set 4500`");
                return ExitCode::from(2);
            };
            match rpm.parse::<u32>() {
                Ok(rpm) if rpm > 0 => control(Action::Set(rpm), args, no_sudo),
                _ => {
                    eprintln!("Error: {rpm:?} is not a positive integer rpm value");
                    ExitCode::from(2)
                }
            }
        }
        other => {
            eprintln!("Error: unknown command {other:?}");
            eprint!("{}", help(&prog));
            ExitCode::from(2)
        }
    }
}
