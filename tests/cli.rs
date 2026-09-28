//! Command-line behaviour tests: argument handling, exit codes, help text.
//!
//! Nothing here touches the SMC, so this runs anywhere (including CI).

use std::process::Command;

fn rsmc(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rsmc"))
        .args(args)
        .output()
        .expect("rsmc runs")
}

fn macfan(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_macfan"))
        .args(args)
        .output()
        .expect("macfan runs")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[test]
fn rsmc_version_prints_version() {
    let out = rsmc(&["-v"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim(), env!("CARGO_PKG_VERSION"));
    let out = rsmc(&["--version"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn rsmc_without_operation_prints_usage_and_fails() {
    let out = rsmc(&[]);
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(text.contains("Usage:"));
    assert!(text.contains("-f         : fan info decoded"));
}

#[test]
fn rsmc_help_behaves_like_the_reference_tool() {
    // The reference tool returns 1 for -h (it sets no operation).
    let out = rsmc(&["-h"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("-k <key>   : key to manipulate"));
}

#[test]
fn rsmc_key_alone_is_not_an_operation() {
    let out = rsmc(&["-k", "FNum"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("Usage:"));
}

#[test]
fn rsmc_rejects_a_bad_hex_value() {
    let out = rsmc(&["-k", "F0Tg", "-w", "zz"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("Error: value is not valid"));
}

#[test]
fn rsmc_rejects_an_odd_length_hex_value() {
    let out = rsmc(&["-k", "F0Tg", "-w", "abc"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("Error: value is not valid"));
}

#[test]
fn rsmc_accepts_attached_and_separate_option_arguments() {
    // Neither form should print a usage error; both need hardware to finish,
    // so only the argument handling is asserted here.
    for args in [vec!["-h"], vec!["-v"]] {
        let out = rsmc(&args);
        assert!(!stdout(&out).contains("requires an argument"));
    }
    let out = rsmc(&["-kFNum", "-r"]);
    assert!(!stdout(&out).contains("requires an argument"));
}

#[test]
fn rsmc_rejects_an_unknown_option() {
    let out = rsmc(&["-Z"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("Error: unknown option -Z"));
}

#[test]
fn macfan_help_succeeds() {
    let out = macfan(&["--help"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("set <rpm>"));
    assert!(text.contains("--no-sudo"));
}

#[test]
fn macfan_version_succeeds() {
    let out = macfan(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out).trim(),
        format!("macfan {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn macfan_rejects_unknown_command() {
    let out = macfan(&["bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn macfan_rejects_a_non_numeric_speed() {
    let out = macfan(&["set", "fast"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not a positive integer"));
}

#[test]
fn macfan_set_without_a_value_explains_itself() {
    let out = macfan(&["set"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("set needs a speed"));
}

#[test]
fn macfan_rejects_unknown_options() {
    let out = macfan(&["--wat"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown option --wat"));
}
