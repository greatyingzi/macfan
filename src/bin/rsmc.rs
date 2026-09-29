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
use macfan::json::{self, Obj};
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
    println!("    --json     : machine-readable output instead of text");
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

/// One key and its payload as JSON.
fn value_json(v: &macfan::smc::Value) -> String {
    let mut obj = Obj::new();
    obj.str_field("key", &v.key)
        .str_field("type", &v.data_type)
        .raw_field("size", json::number(f64::from(v.data_size)));
    let bytes: Vec<String> = v
        .payload()
        .iter()
        .map(|b| json::number(f64::from(*b)))
        .collect();
    obj.array_field("bytes", &bytes);
    obj.raw_field("uint", json::number(v.as_uint() as f64));
    // The same interpretation the text output uses. A `float` field only
    // appears when the key's type really is a float — reinterpreting a ui32's
    // bytes as f32 produces nonsense.
    if let Some(decoded) = macfan::decode::decode(&v.data_type, v.data_size, v.payload()) {
        obj.str_field("value", &decoded.to_string());
        if let macfan::decode::Decoded::Float { value, .. } = decoded {
            obj.raw_field("float", json::number(value));
        }
    }
    obj.render()
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv.first().cloned().unwrap_or_else(|| "rsmc".into());

    let mut op = Op::None;
    let mut key = String::new();
    let mut value: Vec<u8> = Vec::new();
    let mut json_out = false;

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
        if arg == "--json" {
            json_out = true;
            i += 1;
            continue;
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
            if json_out {
                let mut obj = Obj::new();
                obj.str_field("error", &err.to_string());
                println!("{}", obj.render());
            } else {
                println!("{err}");
            }
            return ExitCode::from(1);
        }
    };

    let mut ok = true;
    match op {
        Op::Fan => match fan::read_fans(&smc) {
            Ok(fans) if json_out => {
                let items: Vec<String> = fans
                    .iter()
                    .map(|f| {
                        let mut obj = Obj::new();
                        obj.raw_field("index", json::number(f.index as f64))
                            .raw_field(
                                "id",
                                f.id.as_deref()
                                    .map(json::string)
                                    .unwrap_or_else(|| "null".into()),
                            )
                            .raw_field("current", json::number(f.current))
                            .raw_field("minimum", json::number(f.min))
                            .raw_field("maximum", json::number(f.max))
                            .raw_field("safe", json::number(f.safe))
                            .raw_field("target", json::number(f.target))
                            .bool_field("forced", f.forced);
                        obj.render()
                    })
                    .collect();
                let mut obj = Obj::new();
                obj.array_field("fans", &items);
                println!("{}", obj.render());
            }
            Ok(fans) => print!("{}", fan::render(&fans)),
            Err(err) => {
                print_call_error("SMCPrintFans", &err);
                ok = false;
            }
        },
        Op::Temps if json_out => match fan::temperature_readings(&smc) {
            Ok(readings) => {
                let items: Vec<String> = readings
                    .iter()
                    .map(|r| {
                        let mut obj = Obj::new();
                        obj.str_field("key", &r.key)
                            .str_field("kind", r.kind)
                            .raw_field("celsius", json::number(r.celsius));
                        obj.render()
                    })
                    .collect();
                println!("{}", json::array(&items));
            }
            Err(err) => {
                let mut obj = Obj::new();
                obj.str_field("error", &err.to_string());
                println!("{}", obj.render());
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
                let mut json_items: Vec<String> = Vec::with_capacity(keys.len());
                for k in keys {
                    match smc.read(&k) {
                        Ok(v) if json_out => json_items.push(value_json(&v)),
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
                            if json_out {
                                let mut obj = Obj::new();
                                obj.str_field("key", k.trim_end())
                                    .str_field("type", ty.trim_end())
                                    .bool_field("unreadable", true);
                                json_items.push(obj.render());
                            } else {
                                print!("{}", format_unreadable(&k, &ty));
                            }
                        }
                    }
                }
                if json_out {
                    println!("{}", json::array(&json_items));
                }
            }
            Err(err) => {
                if json_out {
                    let mut obj = Obj::new();
                    obj.str_field("error", &err.to_string());
                    println!("{}", obj.render());
                } else {
                    print_call_error("SMCPrintAll", &err);
                }
                ok = false;
            }
        },
        Op::Read => {
            if key.is_empty() {
                println!("Error: specify a key to read");
                ok = false;
            } else {
                match smc.read(&key) {
                    Ok(v) if json_out => println!("{}", value_json(&v)),
                    Ok(v) => print!(
                        "{}",
                        format_line(&v.key, &v.data_type, v.data_size, &v.bytes)
                    ),
                    Err(err) => {
                        if json_out {
                            let mut obj = Obj::new();
                            obj.str_field("error", &err.to_string());
                            println!("{}", obj.render());
                        } else {
                            println!("{err}");
                        }
                        ok = false;
                    }
                }
            }
        }
        Op::Write => {
            if key.is_empty() {
                println!("Error: specify a key to write");
                ok = false;
            } else {
                match smc.write(&key, &value) {
                    Ok(()) if json_out => {
                        let mut obj = Obj::new();
                        obj.str_field("key", &macfan::smc::normalize_key(&key))
                            .raw_field("bytes", json::number(value.len() as f64))
                            .bool_field("written", true);
                        println!("{}", obj.render());
                    }
                    Ok(()) => {}
                    Err(err) => {
                        if json_out {
                            let mut obj = Obj::new();
                            obj.str_field("error", &err.to_string());
                            println!("{}", obj.render());
                        } else {
                            println!("{err}");
                        }
                        ok = false;
                    }
                }
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
