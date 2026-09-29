//! `macfan` — a dependency-free Rust toolkit for the macOS SMC.
//!
//! Two binaries share this library:
//!
//! * `rsmc` — an SMC client with the same command line as the classic `smc`
//!   tool (`-f`, `-t`, `-l`, `-k`, `-r`, `-w`), so it can be used as a drop-in
//!   replacement in scripts.
//! * `macfan` — fan control: `status`, `max`, `min`, `set <rpm>`, `auto`.
//!
//! Nothing here links against third-party crates, and nothing was ported from
//! another SMC implementation: the IOKit ABI is rebuilt from scratch (see
//! `smc` for the details) and the value decoders are written for this crate.

pub mod decode;
pub mod fan;
pub mod json;
pub mod smc;
pub mod sudo;

/// Version string reported by `--version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
