//! AppleSMC user-client access over IOKit.
//!
//! # What this module is
//!
//! An independent Rust client for the `AppleSMC` IOKit user client. The
//! selector numbers and the `KeyData` field layout are the kernel ABI you have
//! to match to talk to the SMC at all — interface facts, fixed by Apple's
//! driver, not creative expression. Everything here (error handling, caching,
//! key encoding, the value model) is written from scratch for this crate.
//!
//! # Protocol, in one paragraph
//!
//! Open an `IOServiceOpen` connection to the `AppleSMC` service, then issue
//! struct calls with selector `KERNEL_INDEX_SMC`. A call carries a `KeyData`
//! struct: `key` (4 ASCII chars as a big-endian u32), `data8` (the command:
//! read bytes / write bytes / read key info / read key by index), `key_info`
//! (data size + data type, filled in by the read-key-info command) and
//! `bytes` (payload). Reads need a preceding key-info call to learn the size;
//! writes need the same size as the existing key or the SMC rejects them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::fmt;

/// `kern_return_t` from mach.
pub type KernReturn = i32;

/// `KERN_SUCCESS`.
pub const KERN_SUCCESS: KernReturn = 0;
/// `kIOReturnNotPrivileged` — the SMC rejects fan writes from non-root callers.
pub const K_IO_RETURN_NOT_PRIVILEGED: KernReturn = 0xe000_02c1u32 as i32;
/// `kIOReturnError` — generic failure.
pub const K_IO_RETURN_ERROR: KernReturn = 0xe000_02bcu32 as i32;

/// IOKit struct-call selector for the SMC user client.
const KERNEL_INDEX_SMC: u32 = 2;

/// Command: read the bytes of a key.
pub const CMD_READ_BYTES: u8 = 5;
/// Command: write the bytes of a key.
pub const CMD_WRITE_BYTES: u8 = 6;
/// Command: get the key that lives at a given index (enumerate all keys).
pub const CMD_READ_INDEX: u8 = 8;
/// Command: read a key's size + type.
pub const CMD_READ_KEYINFO: u8 = 9;

/// Key holding the number of fans.
pub const KEY_FAN_COUNT: &str = "FNum";
/// Key holding the total number of SMC keys.
pub const KEY_KEY_COUNT: &str = "#KEY";
/// Legacy forced-mode bitmask key (`FS! `); absent data on Apple Silicon.
pub const KEY_MODE_LEGACY: &str = "FS! ";
/// Highest fan index we are willing to address.
pub const MAX_FANS: usize = 10;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, owning_task: u32, kind: u32, conn: *mut u32) -> KernReturn;
    fn IOServiceClose(conn: u32) -> KernReturn;
    fn IOConnectCallStructMethod(
        conn: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> KernReturn;
}

extern "C" {
    /// `mach_task_self_` — the current task port (macOS exports it as data and
    /// `mach_task_self()` is a macro over it).
    static mach_task_self_: u32;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vers {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PLimit {
    version: u16,
    length: u16,
    cpu: u32,
    gpu: u32,
    mem: u32,
}

/// Size + type + attributes of an SMC key, as reported by the SMC.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct KeyInfo {
    /// Payload size in bytes (0 = the key exists but carries no data).
    pub data_size: u32,
    /// Four-character data type packed big-endian (e.g. `flt `, `ui8 `).
    pub data_type: u32,
    /// Attribute bits.
    pub data_attributes: u8,
}

/// The struct exchanged with the SMC user client.
///
/// Field order, sizes and the resulting padding are part of the ABI; the unit
/// tests below pin every offset so a refactor can't silently shift them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyData {
    key: u32,
    vers: Vers,
    plimit: PLimit,
    key_info: KeyInfo,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

impl Default for KeyData {
    fn default() -> Self {
        // All-zero input is what the protocol expects for unused fields.
        unsafe { std::mem::zeroed() }
    }
}

/// A key's payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    /// Key as supplied (normalised to 4 chars).
    pub key: String,
    /// Payload size in bytes.
    pub data_size: u32,
    /// Data type as a 4-char string (`"flt "`, `"ui8 "`, …).
    pub data_type: String,
    /// Raw payload.
    pub bytes: [u8; 32],
}

impl Value {
    /// Payload bytes actually used by this key.
    pub fn payload(&self) -> &[u8] {
        &self.bytes[..(self.data_size as usize).min(self.bytes.len())]
    }

    /// Interpret the payload as the unsigned integer the SMC stores.
    ///
    /// The SMC stores integers big-endian; `ui8 FNum = 1` arrives as `01`.
    pub fn as_uint(&self) -> u64 {
        let mut out: u64 = 0;
        for b in self.payload() {
            out = (out << 8) | u64::from(*b);
        }
        out
    }

    /// Interpret the payload as an `f32` (`flt ` type), little-endian.
    pub fn as_f32(&self) -> Option<f32> {
        if self.data_size == 4 {
            let mut b = [0u8; 4];
            b.copy_from_slice(&self.bytes[..4]);
            Some(f32::from_le_bytes(b))
        } else {
            None
        }
    }

    /// Value of a key the way the fan code needs it: `flt`, `fpe2`, `ui16` or
    /// `ui8`, else `-1.0`.
    pub fn as_fan_number(&self) -> f64 {
        let b = self.payload();
        match (self.data_type.trim(), self.data_size) {
            ("flt", 4) => self.as_f32().map(f64::from).unwrap_or(-1.0),
            ("fpe2", 2) => be_u16(b) as f64 / 4.0,
            ("ui16", 2) => be_u16(b) as f64,
            ("ui8", 1) => b[0] as f64,
            _ => -1.0,
        }
    }
}

fn be_u16(b: &[u8]) -> u16 {
    if b.len() < 2 {
        return 0;
    }
    u16::from_be_bytes([b[0], b[1]])
}

/// Errors from the SMC layer.
#[derive(Debug)]
pub enum Error {
    /// No `AppleSMC` service in the I/O registry.
    ServiceNotFound,
    /// A call failed; carries the raw `kern_return_t`.
    Call {
        /// What was being attempted.
        what: &'static str,
        /// Raw result code to be printed as `%08x`.
        kr: KernReturn,
    },
    /// The key name isn't 1..=4 bytes.
    BadKey(String),
    /// Write payload length doesn't match the key's existing size.
    SizeMismatch {
        /// Size the key has.
        expected: usize,
        /// Size we were asked to write.
        got: usize,
    },
    /// A hex string was malformed.
    BadHex(String),
}

impl Error {
    /// Is this a "needs root" failure?
    pub fn is_not_privileged(&self) -> bool {
        matches!(self, Error::Call { kr, .. } if *kr == K_IO_RETURN_NOT_PRIVILEGED)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ServiceNotFound => write!(f, "Error: no SMC found"),
            Error::Call { what, kr } => write!(f, "Error: {}() = {:08x}", what, kr),
            Error::BadKey(k) => write!(f, "Error: key {:?} is not 1..=4 bytes", k),
            Error::SizeMismatch { expected, got } => write!(
                f,
                "Error: value has {} byte(s), key expects {}",
                got, expected
            ),
            Error::BadHex(h) => write!(f, "Error: {:?} is not valid hex", h),
        }
    }
}

impl std::error::Error for Error {}

/// Connection to the SMC, plus a key-info cache (the size/type of a key never
/// changes, and every read would otherwise cost two struct calls).
pub struct Smc {
    conn: u32,
    info_cache: RefCell<HashMap<u32, KeyInfo>>,
}

impl Smc {
    /// Open a connection to the SMC.
    pub fn open() -> Result<Smc, Error> {
        unsafe {
            let name = b"AppleSMC\0";
            let matching = IOServiceMatching(name.as_ptr() as *const c_char);
            let service = IOServiceGetMatchingService(0, matching);
            if service == 0 {
                return Err(Error::ServiceNotFound);
            }
            let mut conn: u32 = 0;
            let kr = IOServiceOpen(service, mach_task_self_, 0, &mut conn);
            // The service object is only needed until the connection is open.
            extern "C" {
                fn IOObjectRelease(obj: u32) -> KernReturn;
            }
            IOObjectRelease(service);
            if kr != KERN_SUCCESS {
                return Err(Error::Call {
                    what: "IOServiceOpen",
                    kr,
                });
            }
            Ok(Smc {
                conn,
                info_cache: RefCell::new(HashMap::new()),
            })
        }
    }

    fn call(&self, input: &KeyData) -> Result<KeyData, Error> {
        let mut output = KeyData::default();
        let mut out_size = std::mem::size_of::<KeyData>();
        let kr = unsafe {
            IOConnectCallStructMethod(
                self.conn,
                KERNEL_INDEX_SMC,
                input as *const KeyData as *const c_void,
                std::mem::size_of::<KeyData>(),
                &mut output as *mut KeyData as *mut c_void,
                &mut out_size,
            )
        };
        if kr != KERN_SUCCESS {
            return Err(Error::Call {
                what: "IOConnectCallStructMethod",
                kr,
            });
        }
        Ok(output)
    }

    /// Size + type of a key (cached).
    pub fn key_info(&self, key: &str) -> Result<KeyInfo, Error> {
        let code = key_code(key)?;
        if let Some(hit) = self.info_cache.borrow().get(&code) {
            return Ok(*hit);
        }
        let input = KeyData {
            key: code,
            data8: CMD_READ_KEYINFO,
            ..KeyData::default()
        };
        let out = self.call(&input)?;
        let info = out.key_info;
        self.info_cache.borrow_mut().insert(code, info);
        Ok(info)
    }

    /// Read a key's payload.
    pub fn read(&self, key: &str) -> Result<Value, Error> {
        let code = key_code(key)?;
        let info = self.key_info(key)?;
        let input = KeyData {
            key: code,
            key_info: KeyInfo {
                data_size: info.data_size,
                ..KeyInfo::default()
            },
            data8: CMD_READ_BYTES,
            ..KeyData::default()
        };
        let out = self.call(&input).map_err(|e| match e {
            Error::Call { kr, .. } => Error::Call {
                what: "SMCReadKey",
                kr,
            },
            other => other,
        })?;
        Ok(Value {
            key: normalize_key(key),
            data_size: info.data_size,
            data_type: type_string(info.data_type),
            bytes: out.bytes,
        })
    }

    /// Write a key's payload. The length must match the key's existing size,
    /// as the SMC refuses resizes.
    pub fn write(&self, key: &str, bytes: &[u8]) -> Result<(), Error> {
        let code = key_code(key)?;
        let info = self.key_info(key)?;
        if info.data_size as usize != bytes.len() {
            return Err(Error::SizeMismatch {
                expected: info.data_size as usize,
                got: bytes.len(),
            });
        }
        let mut input = KeyData {
            key: code,
            key_info: KeyInfo {
                data_size: info.data_size,
                ..KeyInfo::default()
            },
            data8: CMD_WRITE_BYTES,
            ..KeyData::default()
        };
        input.bytes[..bytes.len()].copy_from_slice(bytes);
        self.call(&input).map_err(|e| match e {
            Error::Call { kr, .. } => Error::Call {
                what: "SMCWriteKey",
                kr,
            },
            other => other,
        })?;
        Ok(())
    }

    /// Write a `flt ` key from an `f32` (this is how fan target speed is set).
    pub fn write_f32(&self, key: &str, value: f32) -> Result<(), Error> {
        self.write(key, &value.to_le_bytes())
    }

    /// Total number of keys the SMC knows.
    pub fn key_count(&self) -> Result<u32, Error> {
        Ok(self.read(KEY_KEY_COUNT)?.as_uint() as u32)
    }

    /// Key at a given index (used to enumerate every key).
    pub fn key_at(&self, index: u32) -> Result<String, Error> {
        let input = KeyData {
            data8: CMD_READ_INDEX,
            data32: index,
            ..KeyData::default()
        };
        let out = self.call(&input).map_err(|e| match e {
            Error::Call { kr, .. } => Error::Call {
                what: "SMCReadIndex",
                kr,
            },
            other => other,
        })?;
        Ok(code_to_string(out.key))
    }

    /// Every key the SMC exposes, in index order.
    pub fn all_keys(&self) -> Result<Vec<String>, Error> {
        let total = self.key_count()?;
        let mut keys = Vec::with_capacity(total as usize);
        for i in 0..total {
            match self.key_at(i) {
                Ok(k) => keys.push(k),
                // A bad index is skipped rather than fatal (matches the
                // reference tool's behaviour).
                Err(_) => continue,
            }
        }
        Ok(keys)
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        unsafe {
            IOServiceClose(self.conn);
        }
    }
}

/// 4 ASCII chars -> big-endian u32, as the SMC wants them.
pub fn key_code(key: &str) -> Result<u32, Error> {
    let raw = key.as_bytes();
    if raw.is_empty() || raw.len() > 4 {
        return Err(Error::BadKey(key.to_string()));
    }
    let norm = normalize_key(key);
    let b = norm.as_bytes();
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Pad a key to the SMC's fixed 4-byte width with spaces (so `FS!` works as
/// well as `FS! `), truncating anything longer.
pub fn normalize_key(key: &str) -> String {
    let mut s = String::with_capacity(4);
    for b in key.bytes().take(4) {
        s.push(b as char);
    }
    while s.len() < 4 {
        s.push(' ');
    }
    s
}

/// big-endian u32 -> 4-char key string.
pub fn code_to_string(code: u32) -> String {
    let b = code.to_be_bytes();
    let mut s = String::with_capacity(4);
    for byte in b {
        // Non-printable bytes show up as spaces; keys are ASCII in practice.
        if byte.is_ascii_graphic() || byte == b' ' {
            s.push(byte as char);
        } else {
            s.push(' ');
        }
    }
    s
}

/// Packed data type -> 4-char string ("ui8 ", "flt ", …).
///
/// The SMC packs the type big-endian, and an absent type (`0`) renders as
/// spaces — the reference tool relies on `printf("%-4s")` for that.
pub fn type_string(data_type: u32) -> String {
    code_to_string(data_type)
}

/// Parse a hex string ("00f8e045") into bytes.
pub fn parse_hex(s: &str) -> Result<Vec<u8>, Error> {
    if s.is_empty() || s.len() % 2 != 0 {
        return Err(Error::BadHex(s.to_string()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char).to_digit(16);
        let lo = (pair[1] as char).to_digit(16);
        match (hi, lo) {
            (Some(h), Some(l)) => out.push(((h << 4) | l) as u8),
            _ => return Err(Error::BadHex(s.to_string())),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_data_layout_matches_the_abi() {
        use std::mem::{offset_of, size_of};
        // Total size is what the kernel expects for both input and output.
        assert_eq!(size_of::<KeyData>(), 80);
        assert_eq!(size_of::<KeyInfo>(), 12);
        assert_eq!(size_of::<Vers>(), 6);
        assert_eq!(size_of::<PLimit>(), 16);
        // Offsets: key@0, vers@4, plimit@12, key_info@28, key_info.data_type@32,
        // result@40, status@41, data8@42, data32@44, bytes@48.
        assert_eq!(offset_of!(KeyData, key), 0);
        assert_eq!(offset_of!(KeyData, vers), 4);
        assert_eq!(offset_of!(KeyData, plimit), 12);
        assert_eq!(offset_of!(KeyData, key_info), 28);
        assert_eq!(
            offset_of!(KeyData, key_info) + offset_of!(KeyInfo, data_type),
            32
        );
        assert_eq!(offset_of!(KeyData, result), 40);
        assert_eq!(offset_of!(KeyData, status), 41);
        assert_eq!(offset_of!(KeyData, data8), 42);
        assert_eq!(offset_of!(KeyData, data32), 44);
        assert_eq!(offset_of!(KeyData, bytes), 48);
    }

    #[test]
    fn key_encoding_is_big_endian_and_space_padded() {
        assert_eq!(key_code("F0Ac").unwrap(), 0x4630_4163);
        assert_eq!(key_code("FS!").unwrap(), key_code("FS! ").unwrap());
        assert_eq!(normalize_key("FS!"), "FS! ");
        assert_eq!(normalize_key("FNum"), "FNum");
        // Too long and too short are rejected / truncated the way the SMC wants.
        assert!(key_code("").is_err());
        assert!(key_code("F0Acc").is_err());
        assert_eq!(code_to_string(0x4630_4163), "F0Ac");
        assert_eq!(type_string(0x666c_7420), "flt ");
        assert_eq!(type_string(0), "    ");
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("00f8e045").unwrap(), vec![0x00, 0xf8, 0xe0, 0x45]);
        assert_eq!(parse_hex("FF").unwrap(), vec![0xff]);
        assert!(parse_hex("abc").is_err());
        assert!(parse_hex("zz").is_err());
        assert!(parse_hex("").is_err());
    }

    #[test]
    fn value_decoding() {
        let v = Value {
            key: "F0Tg".into(),
            data_size: 4,
            data_type: "flt ".into(),
            bytes: {
                let mut b = [0u8; 32];
                b[..4].copy_from_slice(&7199.0f32.to_le_bytes());
                b
            },
        };
        assert_eq!(v.as_f32(), Some(7199.0));
        assert_eq!(v.as_fan_number(), 7199.0);

        let fnum = Value {
            key: "FNum".into(),
            data_size: 1,
            data_type: "ui8 ".into(),
            bytes: {
                let mut b = [0u8; 32];
                b[0] = 2;
                b
            },
        };
        assert_eq!(fnum.as_uint(), 2);
        assert_eq!(fnum.as_fan_number(), 2.0);

        // fpe2 (older Intel Macs): unsigned big-endian fixed point, /4.
        let fpe2 = Value {
            key: "F0Ac".into(),
            data_size: 2,
            data_type: "fpe2".into(),
            bytes: {
                let mut b = [0u8; 32];
                b[0] = 0x0b;
                b[1] = 0x80; // 0x0b80 = 2944 -> 736 rpm
                b
            },
        };
        assert_eq!(fpe2.as_fan_number(), 736.0);
    }
}
