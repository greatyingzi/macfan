//! `rsmc` — an SMC client with the classic `smc` command line.
//!
//! Option set and output format deliberately match the long-standing `smc`
//! tool so it can replace it in scripts: `-f` fan dump, `-t` temperatures,
//! `-l` every key, `-k <key>` with `-r`/`-w`, `-v`, `-h`.
//!
//! Two intentional differences are documented in the README: `-v` reports this
//! program's version, and a failure exits non-zero (the original prints an
//! error but still exits 0).

use std::process::ExitCode;

use macfan::decode::{format_line, format_unreadable};
use macfan::fan;
use macfan::smc::{parse_hex, Error, Smc};
use macfan::VERSION;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Op {
    None,
    List,
    Read,
    Fan,
    Write,
    Temps,
}

fn usage(prog: &str) {
    println!("rsmc {VERSION} — AppleSMC client (Rust)");
    println!("Usage:");
    println!("{prog} [options]");
    println!("    -f         : fan info decoded");
    println!("    -t         : list all temperatures");
    println!("    -h         : help");
    println!("    -k <key>   : key to manipulate");
    println!("    -l         : list all keys and values");
    println!("    -r         : read the value of a key");
    println!("    -w <value> : write the specified value to a key");
    println!("    -v         : version");
    println!();
}

/// Print an error the way the reference tool does when a top-level operation
/// fails, keeping the `Error: NAME() = 1234abcd` shape scripts may match.
fn print_call_error(what: &str, err: &Error) {
    match err {
        Error::Call { kr, .. } => println!("Error: {what}() = {kr:08x}"),
        other => println!("{other}"),
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv.first().cloned().unwrap_or_else(|| "rsmc".into());

    let mut op = Op::None;
    let mut key = String::new();
    let mut value: Vec<u8> = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let arg = argv[i].clone();
        if arg == "--help" {
            usage(&prog);
            return ExitCode::from(1);
        }
        if arg == "--version" {
            println!("{VERSION}");
            return ExitCode::SUCCESS;
        }
        if !arg.starts_with('-') || arg.len() < 2 {
            // The reference tool ignores bare words.
            i += 1;
            continue;
        }
        let chars: Vec<char> = arg.chars().skip(1).collect();
        let mut j = 0;
        while j < chars.len() {
            let c = chars[j];
            match c {
                'f' => op = Op::Fan,
                't' => op = Op::Temps,
                'l' => op = Op::List,
                'r' => op = Op::Read,
                'v' => {
                    println!("{VERSION}");
                    return ExitCode::SUCCESS;
                }
                'h' | '?' => {
                    usage(&prog);
                    return ExitCode::from(1);
                }
                'k' | 'w' => {
                    // getopt semantics: the argument may be attached (-kFNum)
                    // or the next word (-k FNum).
                    let attached: String = chars[j + 1..].iter().collect();
                    let arg_value = if !attached.is_empty() {
                        attached
                    } else {
                        i += 1;
                        match argv.get(i) {
                            Some(v) => v.clone(),
                            None => {
                                println!("Error: option -{c} requires an argument");
                                return ExitCode::from(1);
                            }
                        }
                    };
                    if c == 'k' {
                        key = arg_value;
                    } else {
                        match parse_hex(&arg_value) {
                            Ok(bytes) => {
                                value = bytes;
                                op = Op::Write;
                            }
                            Err(_) => {
                                println!("Error: value is not valid");
                                return ExitCode::from(1);
                            }
                        }
                    }
                    j = chars.len();
                }
                other => {
                    println!("Error: unknown option -{other}");
                    usage(&prog);
                    return ExitCode::from(1);
                }
            }
            j += 1;
        }
        i += 1;
    }

    if op == Op::None {
        usage(&prog);
        return ExitCode::from(1);
    }

    let smc = match Smc::open() {
        Ok(smc) => smc,
        Err(err) => {
            println!("{err}");
            return ExitCode::from(1);
        }
    };

    let mut ok = true;
    match op {
        Op::Fan => match fan::read_fans(&smc) {
            Ok(fans) => print!("{}", fan::render(&fans)),
            Err(err) => {
                print_call_error("SMCPrintFans", &err);
                ok = false;
            }
        },
        Op::Temps => match fan::legacy_temperatures(&smc) {
            Ok(text) => print!("{text}"),
            Err(err) => {
                print_call_error("SMCPrintTemps", &err);
                ok = false;
            }
        },
        Op::List => match smc.all_keys() {
            Ok(keys) => {
                for k in keys {
                    match smc.read(&k) {
                        Ok(v) => print!(
                            "{}",
                            format_line(&v.key, &v.data_type, v.data_size, &v.bytes)
                        ),
                        // Key info is known, the payload is not: say so
                        // instead of dumping bytes we do not have.
                        Err(_) => {
                            let ty = smc
                                .key_info(&k)
                                .map(|info| macfan::smc::type_string(info.data_type))
                                .unwrap_or_default();
                            print!("{}", format_unreadable(&k, &ty));
                        }
                    }
                }
            }
            Err(err) => {
                print_call_error("SMCPrintAll", &err);
                ok = false;
            }
        },
        Op::Read => {
            if key.is_empty() {
                println!("Error: specify a key to read");
                ok = false;
            } else {
                match smc.read(&key) {
                    Ok(v) => print!(
                        "{}",
                        format_line(&v.key, &v.data_type, v.data_size, &v.bytes)
                    ),
                    Err(err) => {
                        println!("{err}");
                        ok = false;
                    }
                }
            }
        }
        Op::Write => {
            if key.is_empty() {
                println!("Error: specify a key to write");
                ok = false;
            } else if let Err(err) = smc.write(&key, &value) {
                println!("{err}");
                ok = false;
            }
        }
        Op::None => unreachable!(),
    }

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
