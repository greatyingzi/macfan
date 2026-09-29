# macfan

[![Release](https://img.shields.io/github/v/release/greatyingzi/macfan?sort=semver&color=blue)](https://github.com/greatyingzi/macfan/releases/latest)
[![CI](https://github.com/greatyingzi/macfan/actions/workflows/ci.yml/badge.svg)](https://github.com/greatyingzi/macfan/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Platform](https://img.shields.io/badge/platform-macOS%2011%2B-lightgrey.svg)](#install)
[![Homebrew](https://img.shields.io/badge/Homebrew-greatyingzi%2Ftap%2Fmacfan-orange.svg)](#install)

**English** · [简体中文](README.zh-CN.md)

Fan control and an AppleSMC client for macOS, in Rust. A menu bar app and two
small command-line binaries. Free, no telemetry, no accounts, no privileged helper.

```console
$ macfan status          # one of the CLI's commands; the app does this on screen
Fan #0:
    Current speed : 4128
    Minimum speed: 1199
    Maximum speed: 7199
    Target speed : 4128
    Mode         : auto
```

## Install

```sh
brew install --cask greatyingzi/tap/macfan
```

macfan is ad-hoc signed rather than notarised, so the first launch needs one
approval:

```sh
xattr -dr com.apple.quarantine /Applications/macfan.app
```

Without Homebrew, `macfan-universal.tar.gz` from the
[latest release](https://github.com/greatyingzi/macfan/releases/latest) unpacks
into `./install.sh`; from source it is `cargo build --release`. macOS 11 or newer,
any architecture.

## Support

macfan is free and meant to stay that way: no telemetry, no account, no feature
behind a payment. If it earns its place and you want to say thanks, there is a
[Ko-fi page](https://ko-fi.com/yingzi62662) — the **Sponsor** button at the top of
this page points at the same place. Tips are one-off and cover the project's real
costs, not anyone's income: a Developer ID would end the first-launch approval for
every user, and second-hand hardware would widen testing, which today is a single
M2 MacBook Pro. Donating unlocks nothing.

What helps more than money: **report what does not work** — a machine where
`macfan status` prints the wrong speed, a key that reads `unreadable`, fans that
never settle — or try it on hardware this project has never seen. Intel Macs and
desktops are untested.

## Menu bar app

The app is the point of the project: fan control and temperatures without a terminal.

```sh
cargo run --manifest-path menubar/Cargo.toml    # or install the released .app
```

The status bar shows what you choose — fan speed (`7219`, or `7219/2400` on a
two-fan Mac) or the CPU hot spot (`79°C`) — with an optional glyph (a fan or a
thermometer), and nothing else: no mode marker, no unit on a speed.

| menu entry | what it does |
|------------|--------------|
| `Current: …` | live state line; the tick marks the mode in force, read from the SMC rather than remembered from the last click |
| Force maximum / minimum speed | every fan to its own limit |
| Automatic (system control) | hand the fans back |
| Set speed… | dialog, validated and clamped to this machine's range |
| Set 3000 / 4500 / 6000 rpm | one-click presets (editable in Settings) |
| Launch at login | per-user LaunchAgent, no daemon |
| Show / Icon in the title | what the title reports — `Speed` or `Temperature` — and whether a glyph sits beside it |
| Language | follows the system: English, 简体中文, 繁體中文, 日本語 |
| Refresh / Quit | re-read now (otherwise every 2 s) / quit |

**Temperatures.** "Temperature" is not one number: 126 sensors measure different
places, and a CPU hot spot at 92 °C and a battery at 36 °C are both true at once.
The app's temperature row opens a submenu of eight families, each smoothed over
15 s and coloured against its own thresholds, because 45 °C means different things
on a battery and on a CPU rail:

| family | warm | hot | family | warm | hot |
|--------|------|-----|--------|------|-----|
| CPU | 85 °C | 100 °C | Memory | 55 °C | 70 °C |
| CPU die | 80 °C | 95 °C | Battery | 40 °C | 45 °C |
| GPU | 75 °C | 90 °C | Charger | 45 °C | 55 °C |
| Heatsink | 65 °C | 80 °C | Wireless | 50 °C | 60 °C |
| SSD (proximity) | 60 °C | 75 °C | Power delivery | 70 °C | 85 °C |

The number is a *hot spot*, not a mean — per-core sensors park and drop out, so a
mean over a changing sample set jumps, while the maximum is stable and is what
drives fan ramp and throttling. It is smoothed over a 15 s time constant (raw
readings move ±3…5 °C between two reads); when the window's peak is 3 °C or more
above the average, it is shown next to it.

**Settings.** The window covers the title, the language, launch-at-login, the three
presets, and **restore the last speed at launch** (off by default). Sleeping,
rebooting and closing the lid all hand the fans back to the system, so that option
puts your setting back when the app starts. It skips the write when nothing would
change, so the authorisation prompt appears only when something actually moves.

**Privileges.** Reading needs none. Writing needs root: the app asks the `macfan`
CLI to do it (which handles `sudo` itself), or falls back to the standard macOS
authorisation dialog. No privileged helper, no daemon, no kext. After a write it
re-reads the SMC and warns if the fans did not end up in the requested state.

**Packaging.** `menubar/scripts/make-app.sh` builds `dist/macfan.app` (ad-hoc
signed, icon rendered from `assets/icon.svg`), `make-dmg.sh` wraps it in a DMG.
Drag the app into `/Applications` — Launchpad only indexes `/Applications` and
`~/Applications` — and clear the quarantine flag as shown under Install.

## Command line

Two binaries, zero dependencies, universal (arm64 + x86_64).

| binary | what it does |
|--------|--------------|
| `macfan` | fan speeds and modes; force max / min / a given rpm; hand control back |
| `rsmc` | general AppleSMC client with the classic `smc` command line (`-f -t -l -k -r -w`), so it drops into existing scripts |

### `macfan`

| command | effect |
|---------|--------|
| `macfan` / `status` / `list` | speeds, mode, and the SMC's min/max/safe/target for every fan |
| `macfan max` / `min` | force every fan to its own maximum / minimum |
| `macfan set <rpm>` | force to `<rpm>`, capped per fan |
| `macfan auto` | hand the fans back to the system |
| `macfan temps` | temperatures by family (below) |

`--no-sudo` reports the permission error instead of escalating. A forced fan is
temporary: a reboot, sleep or a closed lid returns it to system control.

### `macfan temps`

```console
$ macfan temps
  group              n        max       mean   state
  CPU               30     70.3 °C     50.4 °C   ok
  CPU die            2     68.0 °C     64.3 °C   ok  (secondary)
  GPU                6     45.2 °C     42.8 °C   ok
  Heatsink          16     47.2 °C     41.2 °C   ok  (secondary)
  SSD (proximity)   20     50.5 °C     40.3 °C   ok  (secondary)
  Memory             4     38.8 °C     26.0 °C   ok  (secondary)
  Battery            3     31.2 °C     31.0 °C   ok
  Charger            1     38.4 °C     38.4 °C   ok  (secondary)
  Wireless           1     32.4 °C     32.4 °C   ok  (secondary)
  Power delivery    20     48.2 °C     43.5 °C   ok  (secondary)
  System             2     41.0 °C     36.8 °C   ok  (secondary)
  Other              3     34.4 °C     16.7 °C   ok  (secondary)
```

`macfan temps <family>` narrows it to one, `--all` names every sensor. `state` uses
the same warm/hot thresholds as the app; sensors reading zero are skipped, being
unwired.

Which families can be trusted was measured, not assumed — each was loaded on
purpose (M2 13" MacBook Pro):

| family | keys | measured response |
|--------|------|-------------------|
| CPU | `Tp0*`, `Tp1*`, `Te0*` | +24…29 °C under 8 CPU threads |
| CPU die | `TCMz`, `TCMb` | the SMC's own die max / average, same ramp |
| GPU | `Tg0*`, `Tg1*` | +11…12 °C under a WebGL burn, CPU +2 |
| Heatsink | `Th0*` | +7.5 °C under GPU load, slower and smoother |
| Battery | `TB*T` | 36.4 °C, flat under every load — real, just inert |
| Charger / memory / wireless | `TCHP`, `TMVR`/`TVM*`, `TW0*` | charger rises on charge; memory rail 44…64 °C; wireless +0.7 °C |

Two caveats worth one line each. The **SSD family is not a disk temperature**:
90 s of sustained writes moved `Ts0*` by 3.7 °C — the same as the die sensors, so
they track board heat — and macOS exposes no NVMe temperature at all. They are
labelled *SSD (proximity)* and never presented as storage. The SMC's derived values
(`TVS*`, `TVD*`, `TVA*`, `TAO`) repeat other readings, so they appear only in
`--all`.

### `rsmc`

| flag | meaning |
|------|---------|
| `-f` | decoded fan information |
| `-t` | temperatures (`sp78` sensors, as the classic tool does) |
| `-l` | every key with its value |
| `-k <key>` `-r` `-w <hex>` | operate on a key: read it, or write raw bytes |
| `-v`, `-h` | version, help |

```console
$ rsmc -k F0Ac -r
  F0Ac  [flt ]  4128 (bytes 00 40 81 45)
```

Keys are 4 characters, space padded (`FS!` works as well as `FS! `).

# Technical

## How it works

The SMC is reachable through the `AppleSMC` IOKit service. `rsmc` opens a
connection and issues struct calls (selector `2`) carrying a `KeyData` struct: the
key as a big-endian `u32`, a command byte (`5` read bytes, `6` write bytes, `8`
key by index, `9` key info), the key's size and type, and a 32-byte payload. A read
is two calls — key info for the size, then the bytes — and the size never changes,
so it is cached.

Forcing a fan is two writes: `F0Md = 1` (manual), then `F0Tg` (target as a 4-byte
little-endian f32); `auto` writes `F0Md = 0`. Fan count comes from `FNum` and
per-fan keys are `F<index><suffix>`, so one, two or more fans all work, each capped
by its own maximum. The SMC applies the write 200–400 ms later, so `macfan` polls
until the fans report the requested state instead of trusting the exit code.

## Verification

`scripts/parity-check.sh` runs the classic `smc` tool and `rsmc` side by side on
live hardware:

```
  [1/4] -f fan dump        : identical (8 lines)
  [2/4] -l key+type column : identical (1645 keys)
  [3/4] -l payloads        : 1328 keys held still, 317 moved, 17 known deviations (4 si16 sign, 13 unreadable), 0 to re-verify
  [4/4] re-verification    : 0 reproducible, 0 stable-and-equal, 0 moving/uncomparable
  PASS — no unexpected differences
```

Only keys that hold still through the measurement window are compared; each
remaining mismatch is then read five times by both tools and counts as a real
difference only if both readings are stable and differ. Anything that will not hold
still is reported as uncomparable rather than counted as a pass. Writes were
verified as a real round trip, and both slices of the universal binary were run
(arm64 natively, x86_64 under Rosetta). Hardware used: MacBook Pro 13" M2 (macOS
15, one fan; full parity, writes, both slices) and MacBook Pro 14" M1 Pro (macOS
26, two fans; read-only fan decoding).

The GUI has its own check: `macfan-menubar --selftest` builds the whole interface
on the main thread, reads it back and exits — status item title, menu items and how
many are wired to actions, the launch-agent round trip, the speed parsing, and the
title/temperature wiring. `MACFAN_LANG` runs it in each language.

## Differences from the classic `smc` tool

Three are deliberate; two fix defects.

1. **`si16` keys keep their sign.** The classic tool prints `ff 32` as `65330`
   instead of `-206` (a host-order cast plus `ntohs`). `rsmc` prints `-206`.
2. **Unreadable payloads say `unreadable`.** On a failed payload read the classic
   tool prints `data_size` bytes from a buffer that can be smaller, i.e. adjacent
   stack memory (one key here claims 117 bytes). `rsmc` prints the type and
   `unreadable`.
3. **`-v` reports this program's version**, not `0.01`.
4. **A failure exits non-zero**, which scripts can act on.
5. **Short keys are padded** (`-k FS!` == `-k FS! `), and `-kFNum` works.

`macfan` is additive: the classic tool has no fan control, and its `-t` prints
nothing on Apple Silicon because no sensor there uses `sp78` — which is why
`macfan temps` decodes the `flt` sensors Apple Silicon actually publishes.

## Caveats

- Writing to the SMC writes to an interface Apple does not document or support.
  Nothing here changes firmware or persistent state, and a reboot always restores
  system fan control — but the usual warning applies: use it at your own risk.
- `macfan min` pins a fan at its lowest speed; under sustained load that means
  higher temperatures. `macfan auto` or a reboot gives control back.
- SMC keys are machine-specific. Unknown or absent keys are reported as
  `no data` / `unreadable` rather than guessed at.

## License

MIT OR Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE`.

An independent implementation: the IOKit selector numbers, command codes and
`KeyData` layout are interface facts needed to talk to the driver, and everything
else was written for this crate. No code was copied or translated from another SMC
project. Parity testing compares output against the classic `smc` tool (GPL-2.0,
from hholtmann/smcFanControl) when that binary is present; no GPL code or binary is
included here.
