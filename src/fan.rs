//! Fan discovery and control.
//!
//! All of this is built on the raw SMC keys; the fiddly parts are the key
//! names (`F0Ac`, `F0Mn`, … indexed by fan) and the fact that forcing a fan
//! takes two writes: flip the fan to manual mode, then set the target speed.

use crate::smc::{Error, Smc, Value, KEY_FAN_COUNT, KEY_MODE_LEGACY, MAX_FANS};

/// Characters used for fan indices in SMC keys: `F0`, … `FJ`.
const FAN_CHARS: &str = "0123456789ABCDEFGHIJ";

/// SMC value that puts a fan back under system control.
pub const MODE_AUTO: u8 = 0;
/// SMC value that puts a fan under manual control.
pub const MODE_FORCED: u8 = 1;

/// One fan and everything the SMC reports about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fan {
    /// Fan index (0-based).
    pub index: usize,
    /// Descriptive fan ID, when the machine exposes one.
    pub id: Option<String>,
    /// Current speed in rpm.
    pub current: f64,
    /// Minimum speed in rpm.
    pub min: f64,
    /// Maximum speed in rpm.
    pub max: f64,
    /// "Safe" speed in rpm (0 on Apple Silicon).
    pub safe: f64,
    /// Speed the SMC is currently targeting, in rpm.
    pub target: f64,
    /// True when the fan is under manual control.
    pub forced: bool,
}

impl Fan {
    /// SMC index character (`F0` -> `'0'`).
    pub fn ch(&self) -> char {
        fan_char(self.index)
    }
}

/// Index character for fan `i`.
pub fn fan_char(i: usize) -> char {
    FAN_CHARS.chars().nth(i).unwrap_or('0')
}

/// How many fans this machine has.
pub fn fan_count(smc: &Smc) -> Result<usize, Error> {
    let v = smc.read(KEY_FAN_COUNT)?;
    Ok((v.as_uint() as usize).min(MAX_FANS))
}

fn read_num(smc: &Smc, key: &str) -> f64 {
    smc.read(key).map(|v| v.as_fan_number()).unwrap_or(-1.0)
}

/// Fan ID lives in the tail of the `FnID` payload, as a NUL-terminated string.
fn fan_id(v: &Value) -> Option<String> {
    if v.data_size <= 4 {
        return None;
    }
    let tail = &v.bytes[4..(v.data_size as usize).min(v.bytes.len())];
    let end = tail.iter().position(|b| *b == 0).unwrap_or(tail.len());
    Some(String::from_utf8_lossy(&tail[..end]).to_string())
}

/// Read every fan on this machine.
pub fn read_fans(smc: &Smc) -> Result<Vec<Fan>, Error> {
    let count = fan_count(smc)?;
    let mut fans = Vec::with_capacity(count);
    for index in 0..count {
        let c = fan_char(index);
        let id = smc.read(&format!("F{c}ID")).ok().as_ref().and_then(fan_id);
        // Forced mode: prefer the legacy bitmap key; Apple Silicon doesn't
        // expose it, so fall back to the per-fan mode key.
        let forced = match smc.read(KEY_MODE_LEGACY) {
            Ok(v) if v.data_size > 0 => (v.as_uint() & (1 << index)) != 0,
            _ => read_num(smc, &format!("F{c}Md")) != 0.0,
        };
        fans.push(Fan {
            index,
            id,
            current: read_num(smc, &format!("F{c}Ac")),
            min: read_num(smc, &format!("F{c}Mn")),
            max: read_num(smc, &format!("F{c}Mx")),
            safe: read_num(smc, &format!("F{c}Sf")),
            target: read_num(smc, &format!("F{c}Tg")),
            forced,
        });
    }
    Ok(fans)
}

/// Render fans the way the reference tool prints `-f`, byte for byte (the odd
/// label alignment included: the fan ID and min/max/safe/target labels are one
/// space narrower than the others).
pub fn render(fans: &[Fan]) -> String {
    let mut out = format!("Total fans in system: {}\n", fans.len());
    for f in fans {
        out.push_str(&format!("\nFan #{}:\n", f.index));
        if let Some(id) = &f.id {
            out.push_str(&format!("    Fan ID       : {}\n", id));
        }
        out.push_str(&format!("    Current speed : {:.0}\n", f.current));
        out.push_str(&format!("    Minimum speed: {:.0}\n", f.min));
        out.push_str(&format!("    Maximum speed: {:.0}\n", f.max));
        out.push_str(&format!("    Safe speed   : {:.0}\n", f.safe));
        out.push_str(&format!("    Target speed : {:.0}\n", f.target));
        out.push_str(&format!(
            "    Mode         : {}\n",
            if f.forced { "forced" } else { "auto" }
        ));
    }
    out
}

/// What the user asked the fans to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// All fans to their own maximum.
    Max,
    /// All fans to their own minimum.
    Min,
    /// All fans to a specific rpm, capped at each fan's own maximum.
    Set(u32),
    /// Hand the fans back to the system.
    Auto,
}

impl Action {
    /// Human-readable summary line printed before the verification dump.
    pub fn headline(&self, fans: &[Fan]) -> String {
        let n = fans.len();
        match self {
            Action::Max => format!("fan(s) -> MAX (forced, {n} fan(s)). verify:"),
            Action::Min => format!("fan(s) -> MIN (forced, {n} fan(s)). verify:"),
            Action::Set(rpm) => {
                format!("fan(s) -> {rpm} rpm (forced, capped per-fan, {n} fan(s)). verify:")
            }
            Action::Auto => format!("fan(s) -> AUTO (system-controlled, {n} fan(s)). verify:"),
        }
    }
}

/// One SMC write: a key and the exact bytes to put in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Write {
    /// SMC key.
    pub key: String,
    /// Payload (length must equal the key's existing size).
    pub bytes: Vec<u8>,
}

/// Target rpm for one fan under a given action.
pub fn target_for(action: Action, fan: &Fan) -> Option<f64> {
    match action {
        Action::Auto => None,
        Action::Max => Some(fan.max),
        Action::Min => Some(fan.min),
        Action::Set(rpm) => {
            let rpm = f64::from(rpm);
            // Never ask for more than the fan can do.
            Some(if fan.max > 0.0 && rpm > fan.max {
                fan.max
            } else {
                rpm
            })
        }
    }
}

/// Build the exact write sequence for an action: `F{n}Md` first (switch to
/// manual), then `F{n}Tg` (target speed as an IEEE-754 `flt`).
pub fn plan(action: Action, fans: &[Fan]) -> Vec<Write> {
    let mut writes = Vec::new();
    for fan in fans {
        let c = fan.ch();
        match target_for(action, fan) {
            None => writes.push(Write {
                key: format!("F{c}Md"),
                bytes: vec![MODE_AUTO],
            }),
            Some(target) => {
                writes.push(Write {
                    key: format!("F{c}Md"),
                    bytes: vec![MODE_FORCED],
                });
                writes.push(Write {
                    key: format!("F{c}Tg"),
                    bytes: (target as f32).to_le_bytes().to_vec(),
                });
            }
        }
    }
    writes
}

/// Would this reading mean the write has landed?
///
/// The expectation is computed from the pre-write snapshot, because a capped
/// `Set` depends on each fan's own maximum at the time of the request.
fn satisfies_with(action: Action, after: &Fan, before: &Fan) -> bool {
    match action {
        Action::Auto => !after.forced,
        Action::Max | Action::Min | Action::Set(_) => match target_for(action, before) {
            Some(want) => after.forced && (after.target - want).abs() <= 1.0,
            None => false,
        },
    }
}

/// Did the fans end up where `action` asked, given where they started?
///
/// Shared by the CLI (which polls with it) and the menu bar app (which checks
/// once after the helper has run).
pub fn satisfied(action: Action, before: &[Fan], after: &[Fan]) -> bool {
    !after.is_empty()
        && before.iter().all(|b| {
            after
                .get(b.index)
                .is_some_and(|a| satisfies_with(action, a, b))
        })
}

/// Read the fans back until they show what the action asked for.
///
/// The SMC applies a target-speed write a moment later (a few hundred
/// milliseconds in practice), so a read issued straight after the write still
/// reports the previous value. Poll briefly and report whether it landed.
pub fn wait_until_settled(
    smc: &Smc,
    action: Action,
    before: &[Fan],
    timeout: std::time::Duration,
) -> (Vec<Fan>, bool) {
    let deadline = std::time::Instant::now() + timeout;
    let mut latest = read_fans(smc).unwrap_or_default();
    loop {
        let settled = satisfied(action, before, &latest);
        if settled || std::time::Instant::now() >= deadline {
            return (latest, settled);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        if let Ok(fans) = read_fans(smc) {
            latest = fans;
        }
    }
}

/// One temperature sensor.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// SMC key.
    pub key: String,
    /// Sensor reading in degrees Celsius.
    pub celsius: f64,
    /// Encoding the sensor uses: `"flt"` on Apple Silicon, `"sp78"` on Intel.
    pub kind: &'static str,
}

/// Decode a temperature payload.
///
/// Apple Silicon publishes temperatures as 4-byte little-endian floats; Intel
/// Macs use 16-bit `sp78` fixed point (1/256 °C steps, signed).
fn decode_temperature(
    data_type: &str,
    data_size: u32,
    bytes: &[u8],
) -> Option<(f64, &'static str)> {
    match (data_type, data_size) {
        ("flt ", 4) => Some((
            f64::from(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
            "flt",
        )),
        ("sp78", 2) => Some((
            f64::from(i16::from_be_bytes([bytes[0], bytes[1]])) / 256.0,
            "sp78",
        )),
        _ => None,
    }
}

/// Every readable temperature sensor on this machine, sorted by key.
pub fn temperature_readings(smc: &Smc) -> Result<Vec<Reading>, Error> {
    let mut out = Vec::new();
    for key in smc.all_keys()? {
        if !key.starts_with('T') {
            continue;
        }
        let Ok(v) = smc.read(&key) else { continue };
        if let Some((celsius, kind)) = decode_temperature(&v.data_type, v.data_size, v.payload()) {
            out.push(Reading {
                key: v.key,
                celsius,
                kind,
            });
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// Temperature output in the classic tool's `-t` format: `sp78` keys only.
///
/// Kept for drop-in compatibility — note that on Apple Silicon this prints
/// nothing, because no sensor uses `sp78` there (`macfan temps` shows the real
/// readings).
pub fn legacy_temperatures(smc: &Smc) -> Result<String, Error> {
    let mut out = String::new();
    for r in temperature_readings(smc)? {
        if r.kind == "sp78" {
            out.push_str(&format!("{:<4} {:.3} \n", r.key, r.celsius));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fan(index: usize, min: f64, max: f64) -> Fan {
        Fan {
            index,
            id: None,
            current: min,
            min,
            max,
            safe: 0.0,
            target: min,
            forced: false,
        }
    }

    fn two_fans() -> Vec<Fan> {
        vec![fan(0, 1199.0, 7199.0), fan(1, 1200.0, 6241.0)]
    }

    #[test]
    fn max_and_min_use_each_fans_own_limits() {
        let fans = two_fans();
        let w = plan(Action::Max, &fans);
        assert_eq!(w[0].key, "F0Md");
        assert_eq!(w[0].bytes, vec![1]);
        assert_eq!(w[1].key, "F0Tg");
        assert_eq!(w[1].bytes, 7199.0f32.to_le_bytes().to_vec());
        assert_eq!(w[2].key, "F1Md");
        assert_eq!(w[3].key, "F1Tg");
        assert_eq!(w[3].bytes, 6241.0f32.to_le_bytes().to_vec());

        let w = plan(Action::Min, &fans);
        assert_eq!(w[1].bytes, 1199.0f32.to_le_bytes().to_vec());
        assert_eq!(w[3].bytes, 1200.0f32.to_le_bytes().to_vec());
    }

    #[test]
    fn set_caps_per_fan_and_encodes_ieee754() {
        let fans = two_fans();
        // 4500 fits both fans.
        let w = plan(Action::Set(4500), &fans);
        assert_eq!(w[1].bytes, 4500.0f32.to_le_bytes().to_vec());
        assert_eq!(w[3].bytes, 4500.0f32.to_le_bytes().to_vec());
        // 7199 is capped to fan 1's own maximum (6241).
        let w = plan(Action::Set(7199), &fans);
        assert_eq!(w[1].bytes, 7199.0f32.to_le_bytes().to_vec());
        assert_eq!(w[3].bytes, 6241.0f32.to_le_bytes().to_vec());
        // The classic reference encoding: 7199 rpm -> 00 f8 e0 45.
        assert_eq!(w[1].bytes, vec![0x00, 0xf8, 0xe0, 0x45]);
    }

    #[test]
    fn auto_only_writes_the_mode_key() {
        let fans = two_fans();
        let w = plan(Action::Auto, &fans);
        assert_eq!(w.len(), 2);
        assert_eq!(
            w[0],
            Write {
                key: "F0Md".into(),
                bytes: vec![0]
            }
        );
        assert_eq!(
            w[1],
            Write {
                key: "F1Md".into(),
                bytes: vec![0]
            }
        );
    }

    #[test]
    fn renders_reference_fan_format() {
        let fans = vec![Fan {
            index: 0,
            id: None,
            current: 7165.0,
            min: 1199.0,
            max: 7199.0,
            safe: 0.0,
            target: 7199.0,
            forced: true,
        }];
        assert_eq!(
            render(&fans),
            "Total fans in system: 1\n\nFan #0:\n    Current speed : 7165\n    Minimum speed: 1199\n    Maximum speed: 7199\n    Safe speed   : 0\n    Target speed : 7199\n    Mode         : forced\n"
        );
    }

    #[test]
    fn satisfied_compares_before_and_after_snapshots() {
        let before = two_fans();
        let mut after = before.clone();
        // Still under system control: the write has not landed.
        assert!(!satisfied(Action::Max, &before, &after));
        for fan in after.iter_mut() {
            fan.forced = true;
            fan.target = fan.max;
        }
        assert!(satisfied(Action::Max, &before, &after));
        // A capped Set is judged against each fan's own maximum.
        assert!(satisfied(Action::Set(99999), &before, &after));
        // Auto only cares about the mode.
        assert!(!satisfied(Action::Auto, &before, &after));
        for fan in after.iter_mut() {
            fan.forced = false;
        }
        assert!(satisfied(Action::Auto, &before, &after));
        // An empty reading is never "satisfied".
        assert!(!satisfied(Action::Auto, &before, &[]));
    }

    #[test]
    fn temperature_decoding_covers_both_encodings() {
        // Apple Silicon: 4-byte little-endian float, 35.0 °C.
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&35.0f32.to_le_bytes());
        assert_eq!(decode_temperature("flt ", 4, &b), Some((35.0, "flt")));
        // Intel: sp78 fixed point, 30.5 °C.
        assert_eq!(
            decode_temperature("sp78", 2, &[0x1e, 0x80]),
            Some((30.5, "sp78"))
        );
        // Negative sp78.
        assert_eq!(
            decode_temperature("sp78", 2, &[0xe1, 0x80]),
            Some((-30.5, "sp78"))
        );
        // Anything else is not a temperature.
        assert_eq!(decode_temperature("ui16", 2, &[0x00, 0x10]), None);
        assert_eq!(decode_temperature("flt ", 2, &[0, 0]), None);
    }

    #[test]
    fn settlement_check_uses_the_pre_write_limits() {
        let before = two_fans();
        let mut after = before.clone();
        // Still under system control: not settled yet.
        assert!(!satisfies_with(Action::Max, &after[0], &before[0]));
        after[0].forced = true;
        after[0].target = 7199.0;
        assert!(satisfies_with(Action::Max, &after[0], &before[0]));
        // A capped Set is compared against the fan's own maximum.
        after[1].forced = true;
        after[1].target = 6241.0;
        assert!(satisfies_with(Action::Set(99999), &after[1], &before[1]));
        after[1].target = 5000.0;
        assert!(!satisfies_with(Action::Set(99999), &after[1], &before[1]));
        // Auto only cares about the mode.
        assert!(!satisfies_with(Action::Auto, &after[0], &before[0]));
        let mut released = after[0].clone();
        released.forced = false;
        assert!(satisfies_with(Action::Auto, &released, &before[0]));
    }

    #[test]
    fn fan_id_is_decoded_from_the_payload_tail() {
        let mut bytes = [0u8; 32];
        bytes[4..11].copy_from_slice(b"Left si");
        let v = Value {
            key: "F0ID".into(),
            data_size: 16,
            data_type: "ch8*".into(),
            bytes,
        };
        assert_eq!(fan_id(&v).as_deref(), Some("Left si"));
        // No payload -> no ID.
        let empty = Value {
            key: "F0ID".into(),
            data_size: 0,
            data_type: "    ".into(),
            bytes: [0u8; 32],
        };
        assert_eq!(fan_id(&empty), None);
    }
}
