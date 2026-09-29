//! Decoding of SMC values into printable form.
//!
//! The SMC stores numbers in a handful of fixed-point and integer formats,
//! keyed by the 4-char data type of each key. This module knows the formats
//! and renders them the way the classic `smc` tool does, so the two can be
//! diffed against each other.

/// A decoded SMC number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decoded {
    /// Fixed-point / float with a fixed number of printed decimals.
    Float {
        /// Decoded value.
        value: f64,
        /// Digits after the decimal point.
        precision: usize,
    },
    /// Unsigned integer.
    Uint(u64),
    /// Signed integer.
    Int(i64),
    /// PWM duty cycle, printed as a percentage.
    Pwm(f64),
}

impl std::fmt::Display for Decoded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decoded::Float { value, precision } => write!(f, "{:.*}", precision, value),
            Decoded::Uint(v) => write!(f, "{}", v),
            Decoded::Int(v) => write!(f, "{}", v),
            Decoded::Pwm(v) => write!(f, "{:.1}%", v),
        }
    }
}

fn be_u16(b: &[u8]) -> u16 {
    if b.len() < 2 {
        return 0;
    }
    u16::from_be_bytes([b[0], b[1]])
}

/// Big-endian unsigned assembly over exactly `size` bytes.
///
/// The payload length matters: a `ui8` key carries one byte, and assembling
/// four bytes out of its buffer would return `1 << 24` instead of `1`.
fn be_uint(b: &[u8], size: usize) -> u64 {
    let mut out = 0u64;
    for byte in b.iter().take(size.min(4)) {
        out = (out << 8) | u64::from(*byte);
    }
    out
}

/// Unsigned big-endian fixed point: `fpXX` types, `scale` = 2^fractional bits.
fn fixed_unsigned(b: &[u8], scale: f64, precision: usize) -> Decoded {
    Decoded::Float {
        value: be_u16(b) as f64 / scale,
        precision,
    }
}

/// Signed big-endian fixed point: `spXX` types.
fn fixed_signed(b: &[u8], scale: f64, precision: usize) -> Decoded {
    Decoded::Float {
        value: f64::from(be_u16(b) as i16) / scale,
        precision,
    }
}

/// Decode a temperature payload into °C plus a short name for its encoding.
///
/// Apple Silicon publishes temperatures as 4-byte little-endian floats; Intel
/// Macs use 16-bit `sp78` fixed point (1/256 °C steps, signed).
pub fn temperature(data_type: &str, data_size: u32, bytes: &[u8]) -> Option<(f64, &'static str)> {
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

/// Decode a payload according to its data type, or `None` if the type/size
/// combination isn't a number we know how to print.
pub fn decode(data_type: &str, data_size: u32, bytes: &[u8]) -> Option<Decoded> {
    let b = bytes;
    Some(match (data_type, data_size) {
        ("flt ", 4) => Decoded::Float {
            value: f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            precision: 0,
        },
        ("fp1f", 2) => fixed_unsigned(b, 32768.0, 5),
        ("fp4c", 2) => fixed_unsigned(b, 4096.0, 5),
        ("fp5b", 2) => fixed_unsigned(b, 2048.0, 5),
        ("fp6a", 2) => fixed_unsigned(b, 1024.0, 4),
        ("fp79", 2) => fixed_unsigned(b, 512.0, 4),
        ("fp88", 2) => fixed_unsigned(b, 256.0, 3),
        ("fpa6", 2) => fixed_unsigned(b, 64.0, 2),
        ("fpc4", 2) => fixed_unsigned(b, 16.0, 2),
        ("fpe2", 2) => fixed_unsigned(b, 4.0, 2),
        ("sp1e", 2) => fixed_signed(b, 16384.0, 5),
        ("sp3c", 2) => fixed_signed(b, 4096.0, 5),
        ("sp4b", 2) => fixed_signed(b, 2048.0, 4),
        ("sp5a", 2) => fixed_signed(b, 1024.0, 4),
        ("sp69", 2) => fixed_signed(b, 512.0, 3),
        ("sp78", 2) => fixed_signed(b, 256.0, 3),
        ("sp87", 2) => fixed_signed(b, 128.0, 3),
        ("sp96", 2) => fixed_signed(b, 64.0, 2),
        ("spb4", 2) => fixed_signed(b, 16.0, 2),
        ("spf0", 2) => Decoded::Float {
            value: f64::from(be_u16(b)),
            precision: 0,
        },
        ("si8 ", 1) => Decoded::Int(i64::from(b[0] as i8)),
        ("si16", 2) => Decoded::Int(i64::from(be_u16(b) as i16)),
        ("ui8 ", 1) | ("ui16", 2) | ("ui32", 4) => Decoded::Uint(be_uint(b, data_size as usize)),
        ("{pwm", 2) => Decoded::Pwm(f64::from(be_u16(b)) * 100.0 / 65536.0),
        _ => return None,
    })
}

/// Render a key whose payload could not be read (key info succeeded, the data
/// read did not). The classic tool prints the type here but then dumps
/// `data_size` bytes out of a 32-byte buffer, i.e. adjacent stack memory; we
/// show the type and say plainly that there is nothing to show.
pub fn format_unreadable(key: &str, data_type: &str) -> String {
    format!("  {:<4}  [{:<4}]  unreadable\n", key, data_type)
}

/// Render one key/value pair the way the reference tool does:
///
/// ```text
///   F0Ac  [flt ]  7165 (bytes b3 eb df 45)
///   TC0P  [    ]  no data
/// ```
pub fn format_line(key: &str, data_type: &str, data_size: u32, bytes: &[u8]) -> String {
    let mut out = format!("  {:<4}  [{:<4}]  ", key, data_type);
    if data_size > 0 {
        if let Some(d) = decode(data_type, data_size, bytes) {
            // The reference tool always prints one space after a value.
            out.push_str(&format!("{} ", d));
        }
        out.push_str("(bytes");
        for byte in bytes.iter().take(data_size as usize) {
            out.push_str(&format!(" {:02x}", byte));
        }
        out.push_str(")\n");
    } else {
        out.push_str("no data\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_decoding_covers_both_encodings() {
        // flt: little-endian float, what Apple Silicon uses.
        assert_eq!(
            temperature("flt ", 4, &35.0f32.to_le_bytes()),
            Some((35.0, "flt"))
        );
        // sp78: signed fixed point, 1/256 °C steps, what Intel Macs use.
        assert_eq!(temperature("sp78", 2, &[0x1e, 0x80]), Some((30.5, "sp78")));
        // Negative temperatures keep their sign.
        let (value, _) = temperature("sp78", 2, &[0xe1, 0x80]).expect("negative");
        assert!(value < 0.0, "{value}");
        // Anything else is not a temperature.
        assert_eq!(temperature("ui16", 2, &[0x00, 0x10]), None);
        assert_eq!(temperature("flt ", 2, &[0, 0]), None);
    }

    #[test]
    fn renders_like_the_reference_tool() {
        // Real hardware trace: 7165.46 rpm, i.e. these four bytes, printed by
        // `smc -k F0Ac -r` as the line asserted below.
        let mut b = [0u8; 32];
        b[..4].copy_from_slice(&[0xb3, 0xeb, 0xdf, 0x45]);
        assert_eq!(
            format_line("F0Ac", "flt ", 4, &b),
            "  F0Ac  [flt ]  7165 (bytes b3 eb df 45)\n"
        );

        // ui8 fan count.
        let mut b = [0u8; 32];
        b[0] = 1;
        assert_eq!(
            format_line("FNum", "ui8 ", 1, &b),
            "  FNum  [ui8 ]  1 (bytes 01)\n"
        );

        // A key whose payload could not be read.
        assert_eq!(
            format_unreadable("ATP0", "hex_"),
            "  ATP0  [hex_]  unreadable\n"
        );
        assert_eq!(format_unreadable("BAD", ""), "  BAD   [    ]  unreadable\n");

        // A key that exists but carries nothing.
        assert_eq!(
            format_line("TC0P", "", 0, &[0u8; 32]),
            "  TC0P  [    ]  no data\n"
        );
    }

    #[test]
    fn number_formats() {
        // sp78: signed 8.8 fixed point, one byte integer + one fractional.
        assert_eq!(
            decode("sp78", 2, &[0x1e, 0x80]),
            Some(Decoded::Float {
                value: 30.5,
                precision: 3
            })
        );
        // Negative sp78.
        assert_eq!(
            decode("sp78", 2, &[0xe1, 0x80]),
            Some(Decoded::Float {
                value: -30.5,
                precision: 3
            })
        );
        // fpe2 on Intel Macs: /4.
        assert_eq!(
            decode("fpe2", 2, &[0x0b, 0x80]),
            Some(Decoded::Float {
                value: 736.0,
                precision: 2
            })
        );
        // Integer payloads are assembled over exactly `data_size` bytes: the
        // buffer handed in is the full 32-byte SMC slot, so trailing zeros
        // must not be folded into the number.
        assert_eq!(decode("ui8 ", 1, &[1, 0, 0, 0]), Some(Decoded::Uint(1)));
        assert_eq!(decode("ui16", 2, &[1, 0, 0, 0]), Some(Decoded::Uint(256)));
        assert_eq!(
            decode("ui32", 4, &[0x00, 0x0f, 0x42, 0x40]),
            Some(Decoded::Uint(1_000_000))
        );
        // ui16 big-endian.
        assert_eq!(decode("ui16", 2, &[0x01, 0x00]), Some(Decoded::Uint(256)));
        // si8.
        assert_eq!(decode("si8 ", 1, &[0xff]), Some(Decoded::Int(-1)));
        // si16 is signed: 0xff32 == -206. The reference tool casts the raw
        // buffer to a host-order int16 and byte-swaps it, which yields the
        // unsigned 65330 instead; we keep the sign.
        assert_eq!(decode("si16", 2, &[0xff, 0x32]), Some(Decoded::Int(-206)));
        assert_eq!(decode("si16", 2, &[0xd7, 0x08]), Some(Decoded::Int(-10488)));
        // {pwm: 16-bit duty cycle.
        assert_eq!(
            decode("{pwm", 2, &[0x80, 0x00, 0, 0]),
            Some(Decoded::Pwm(50.0))
        );
        // Unknown type / size mismatch.
        assert_eq!(decode("hex_", 4, &[1, 2, 3, 4]), None);
        assert_eq!(decode("flt ", 2, &[1, 2]), None);
    }

    #[test]
    fn pwm_renders_with_percent_sign() {
        assert_eq!(Decoded::Pwm(50.0).to_string(), "50.0%");
    }
}
