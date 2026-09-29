# macfan

SMC fan control and a general AppleSMC client for macOS, in Rust. Two small
binaries, zero dependencies, no privileged helper.

| binary  | what it does |
|---------|--------------|
| `macfan` | read fan speeds, temperatures and mode; force fans to max / min / a given rpm; hand them back to the system |
| `rsmc`   | general-purpose SMC client with the same command line as the classic `smc` tool (`-f`, `-t`, `-l`, `-k`, `-r`, `-w`), so it drops into existing scripts |

```console
$ macfan
Total fans in system: 1

Fan #0:
    Current speed : 4128
    Minimum speed: 1199
    Maximum speed: 7199
    Safe speed   : 0
    Target speed : 4128
    Mode         : auto

$ macfan max
fan(s) -> MAX (forced, 1 fan(s)). verify:
Total fans in system: 1

Fan #0:
    Current speed : 5190
    Minimum speed: 1199
    Maximum speed: 7199
    Safe speed   : 0
    Target speed : 7199
    Mode         : forced

$ macfan temps | head -4
  TB0T    34.6 °C
  TB1T    34.6 °C
  TB2T    32.2 °C
  TCHP    52.1 °C

$ macfan auto        # give the fan back to the system
```

- **One binary per architecture** — the release binaries are universal
  (arm64 + x86_64), so the same file runs on Apple Silicon and Intel Macs.
  No Homebrew, no Python, no Perl, no recompiling on each machine.
- **Reads need no privileges.** Writes need root, and `macfan` re-runs itself
  through `sudo` (one prompt, then cached). `SUDO_PASSWORD` in the environment
  enables unattended use; `--no-sudo` disables escalation entirely.
- **Verified against the classic tool**, not just "looks right": every SMC key
  this machine exposes is compared, mismatches are re-checked, and the two
  places where the classic tool is wrong are documented below.

## Install

From source (needs a Rust toolchain):

```sh
git clone https://github.com/greatyingzi/macfan
cd macfan
cargo build --release            # target/release/{macfan,rsmc}
scripts/install.sh target/release
```

Universal binary (no toolchain needed):

```sh
curl -LO https://github.com/greatyingzi/macfan/releases/latest/download/macfan-universal.tar.gz
tar xzf macfan-universal.tar.gz
cd macfan-universal && ./install.sh
```

The same release also carries the two binaries on their own
(`macfan-<version>-macos-universal`, `rsmc-<version>-macos-universal`) and a
`SHA256SUMS` file. GitHub does not preserve file modes for release assets, so a
standalone download needs `chmod +x` first; the tarball keeps its modes.

`install.sh` puts the binaries in `~/.local/bin` and prints the `PATH` line if
that directory isn't on it yet.

Requirements: macOS 11 or newer, any architecture. Reading the SMC (including
fan control) uses the `AppleSMC` IOKit user client, which is present on every
Mac.

## Usage

### `macfan` — fans

| command | effect |
|---------|--------|
| `macfan` / `macfan status` | fan speeds, mode, and the SMC's min/max/safe/target for every fan |
| `macfan max` | force every fan to its **own** maximum |
| `macfan min` | force every fan to its own minimum (quiet — watch the temperatures) |
| `macfan set <rpm>` | force every fan to `<rpm>`, capped at each fan's maximum |
| `macfan auto` | hand the fans back to system control |
| `macfan temps` | every readable temperature sensor in °C |
| `macfan list` | raw fan dump (alias of `status`) |

Options: `--no-sudo` (report the permission error instead of escalating),
`-V`/`--version`, `-h`/`--help`.

A forced fan is **temporary**: rebooting, sleeping or closing the lid returns
it to system control, and `macfan auto` does it immediately.

### `macfan temps` — temperatures by family

"Temperature" is not one number. A MacBook Pro exposes 126 readable sensors, and
they measure different places at different scales: a CPU hot spot at 92 °C and a
battery cell at 36 °C are both true at the same moment, and their average means
nothing.

```console
$ macfan temps
  group              n        max       mean   state
  CPU               30     92.3 °C     79.4 °C   warm
  CPU die            2     92.3 °C     89.1 °C   warm  (secondary)
  GPU                6     67.4 °C     64.3 °C   ok
  Heatsink          16     68.1 °C     63.5 °C   warm  (secondary)
  SSD (proximity)   20     66.2 °C     54.0 °C   warm  (secondary)
  Memory             4     63.7 °C     38.8 °C   warm  (secondary)
  Battery            3     36.1 °C     35.4 °C   ok
  Charger            1     56.3 °C     56.3 °C   HOT   (secondary)
  Wireless           1     41.1 °C     41.1 °C   ok    (secondary)

$ macfan temps gpu        # one family
$ macfan temps --all      # every sensor, named
```

`state` compares the family's **hot spot** against that family's own thresholds —
45 °C is worth flagging on a battery and unremarkable on a CPU rail. Sensors
reading zero are skipped: they are unwired, and counting them would drag a mean
down for no reason.

#### Which readings are trustworthy

Each family was loaded on purpose and watched, so a family is only claimed when
its sensors moved for the right reason (M2 13" MacBook Pro, 2026-09):

| family | keys | measured response |
|--------|------|-------------------|
| CPU | `Tp0*`, `Tp1*`, `Te0*` | +24…29 °C under 8 CPU threads |
| CPU die | `TCMz` (max), `TCMb` (average) | the SMC's own die aggregates, same ramp |
| GPU | `Tg0*`, `Tg1*` | +11…12 °C under a WebGL shader burn, while the CPU moved +2 |
| Heatsink | `Th0*` | +7.5 °C under GPU load, slower and smoother than the die |
| Battery | `TB*T` | 36.4 °C, flat under every load — a real probe that is simply inert |
| Charger | `TCHP` | rises while charging |
| Memory | `TMVR`, `TVM*` | memory rail and its regulator, 44…64 °C |
| Wireless | `TW0*` | +0.7 °C: small but real |

**Not trustworthy: the SSD family.** Keys `Ts0*`, `Tsx*` and `TH0*` are labelled
SSD/NAND in the community key databases, but 90 s of sustained writes with
`fsync` moved them by 3.7 °C — exactly what the die sensors moved, i.e. they
track board heat rather than flash activity. macOS exposes no NVMe temperature
either (`system_profiler SPNVMeDataType` is empty, the controller in `ioreg` has
no temperature properties, and `smartctl` cannot talk to Apple's storage). They
are listed as **SSD (proximity)** and never presented as a disk temperature.

The SMC's derived values (`TVS*`, `TVD*`, `TVA*`, `TAO`) repeat other readings
rather than measuring a place of their own, so they appear only in
`macfan temps --all` and are never summarised or alarmed about.

#### What is displayed, and how

- **Hot spot, not mean.** Per-core sensors park and drop out of the readable
  set, so a mean over a changing sample size jumps; the maximum is stable, and
  it is also what drives fan ramp and throttling.
- **Smoothed over a 15 s time constant** (an exponential moving average that
  uses the real interval between samples). Raw readings move ±3…5 °C between two
  reads, which is unreadable at a glance; 15 s steadies the number while still
  showing a workload ramp within about twenty seconds. The peak of the window is
  shown next to it when it is 3 °C or more above, because a spike that has
  already passed is what someone watching wants to know.
- The menu bar app puts the same families in a submenu under its temperature
  row, each judged against its own thresholds.

### `rsmc` — raw SMC access

| flag | meaning |
|------|---------|
| `-f` | decoded fan information |
| `-t` | list temperatures (`sp78` sensors, as the classic tool does) |
| `-l` | list every key with its value |
| `-k <key>` | key to operate on, e.g. `-k F0Ac` |
| `-r` | read the key given with `-k` |
| `-w <hex>` | write raw bytes to the key given with `-k` |
| `-v`, `-h` | version, help |

```console
$ rsmc -k F0Ac -r
  F0Ac  [flt ]  4128 (bytes 00 40 81 45)

$ rsmc -k F0Tg -r                      # target speed as a raw f32
  F0Tg  [flt ]  4128 (bytes 00 40 81 45)

$ sudo rsmc -k F0Tg -w 00006042        # 56.0 rpm target, written by hand
```

SMC keys are 4 characters, space padded (`FS!` works as well as `FS! `).

## Menu bar app

`menubar/` is a menu bar item built on the same library (AppKit through
`objc2` — no Swift, no Xcode project, no Electron):

```sh
cargo run --manifest-path menubar/Cargo.toml
```

The status bar shows the current speed (`7199 ⚡ rpm` while a fan is under manual
control, every fan's speed on multi-fan Macs). The menu has a live line per fan,
then:

| menu item | what it does |
|-----------|--------------|
| Force maximum / minimum speed | force every fan to its own limit |
| Automatic (system control) | hand the fans back to the system |
| Set speed… | dialog with an input field, validated and clamped to the machine's range |
| Set 3000 / 4500 / 6000 rpm | one-click presets |
| Launch at login | tick to install a per-user LaunchAgent |
| Preset speeds | three editable fields, applied with the Apply button — the menu and its ticks follow them |
| Menu bar shows | icon + speed / speed only / icon + temperature (the highest of a few core sensors) |
| Language | follows the system by default; the picker overrides it on the next launch |
| Refresh | re-read the fans now (otherwise every 2 s) |
| Quit | |

The menu also says **which mode is in force**: a `Current: …` line at the top,
a tick on the matching entry (automatic, maximum, minimum, or a preset), and a
`Custom <rpm> rpm` entry when the speed in force is not one of the presets. The
ticks come from the measured state — every fan's target compared against its
own limits — not from the last click, so if another tool or the SMC itself moves
the fans, the menu follows within its two-second refresh. Fans that end up with
different targets report a mixed state instead of pretending.

**Languages.** The UI follows the system language — English, 简体中文, 繁體中文
and 日本語 ship today, anything else falls back to English. `MACFAN_LANG=zh-Hans`
overrides it (useful in scripts and in the self test). The CLI stays English on
purpose: its output is compared byte for byte against the classic `smc` tool.

**Writes and privileges.** Writing to the SMC needs root. Inside the app: if it
already runs as root, or `SUDO_PASSWORD` is set, it asks the `macfan` CLI to do
the write (the CLI handles sudo itself); otherwise the write goes through
`osascript … with administrator privileges`, so macOS shows its normal
authorisation dialog. No privileged helper, no code signing, no daemon. After
the helper returns, the app re-reads the SMC and warns if the fans did not end
up in the requested state — a write is asynchronous, so an exit code of 0 does
not by itself mean it landed.

**Launch at login** writes `~/Library/LaunchAgents/com.macfan.menubar.plist`
(validated with `plutil -lint`) and leaves it unloaded, so ticking the box does
not spawn a second copy of the app on the spot; launchd picks it up at the next
login. `SMAppService` would be the modern route, but it wants a signed bundle,
so that can wait until there is one. The same thing is scriptable:

```sh
macfan-menubar --login-status    # installed / not installed
macfan-menubar --login-install
macfan-menubar --login-remove
```

**Verification.** `macfan-menubar --selftest` builds the whole UI on the main
thread, reads it back and exits: status item title, menu items and how many are
wired to an action, the launch-agent round trip in a throwaway directory, and
the speed-input parsing and clamping. Combine it with `MACFAN_LANG` to check
every language. A screenshot is not available to an agent shell (no
screen-recording permission), and "it started without crashing" would not have
proven the wiring.

### App bundle and DMG

```sh
menubar/scripts/make-app.sh          # -> dist/macfan.app   (ad-hoc signed)
menubar/scripts/make-dmg.sh          # -> dist/macfan-0.1.0.dmg
```

The bundle is what makes the icon, the app menu name and the alert artwork
correct: a bare binary inherits the terminal's icon and `NSAlert` has no app
icon to show. `make-app.sh` also copies `macfan` and `rsmc` into
`Contents/MacOS/`, which is where the GUI looks for the CLI it drives.

The icon comes from `menubar/assets/icon.svg` (rendered master:
`assets/icon-1024.png`, regenerate with `scripts/make-icon.py`). Geometry
follows the Big Sur grid — 824x824 artwork on a 1024 canvas with a 185.5 corner
radius, transparency outside — so it sits correctly next to stock icons. The
glyph is deliberately simple (four thick petals and a hub on a blue gradient)
so it survives 16 px.

**Installing.** Drag `macfan.app` into `/Applications`. That location matters:
Launchpad indexes `/Applications` and `~/Applications`, not the folder you
unpacked the DMG in, so an app left in `~/Downloads` will not show up there.

**Gatekeeper.** Without a Developer ID the bundle is only ad-hoc signed, so a
downloaded copy carries the quarantine flag and macOS refuses the first launch.
Measured on macOS 15: launching a quarantined copy does nothing at all; after
clearing the flag it starts normally. Clear it once:

```sh
xattr -dr com.apple.quarantine /Applications/macfan.app
open /Applications/macfan.app
```

The `-r` matters: the flag also sits on the binaries inside the bundle, and
clearing it from the `.app` directory alone is not enough. The GUI equivalent is
right-click → Open, and if macOS only offers "Done" then System Settings →
Privacy & Security → "Open Anyway". A build from source never gets the flag at
all. The DMG carries an install note with these same steps.

A Homebrew cask is published — `brew install --cask greatyingzi/tap/macfan` —
which makes install, upgrade (`brew upgrade --cask macfan`) and removal
(`brew uninstall --cask --zap macfan`) one command each, and verifies the
checksum on the way in.

It does **not** remove the first-launch approval: measured on Homebrew 7, the
cask deliberately applies the quarantine attribute to what it downloads (the
old `--no-quarantine` flag no longer exists), so Gatekeeper still asks once. The
same `xattr -dr` line above clears it. Only a Developer ID would remove that step
for real.

## How it works

The SMC is reachable through the `AppleSMC` IOKit service. `rsmc` opens a
connection and issues struct calls (selector `2`) carrying a `KeyData` struct:
the key as a big-endian `u32`, a command byte (`5` read bytes, `6` write bytes,
`8` read key by index, `9` read key info), the key's size/type, and a 32-byte
payload. Reads are two calls — read key info to learn the payload size, then
read the bytes; the size/type of a key never changes, so `rsmc` caches it.

Forcing a fan takes two writes: switch the fan to manual (`F0Md = 1`), then set
the target speed (`F0Tg = <rpm as a 4-byte little-endian f32>`). `macfan auto`
writes `F0Md = 0`. Fan count comes from `FNum`, and every per-fan key is
addressed as `F<index><suffix>`, so machines with one, two or more fans all
work and each fan is capped by its own maximum.

The SMC applies a target-speed write a moment later (200–400 ms in practice),
so writing and immediately reading back shows the *old* value. `macfan`
therefore polls until the fans report the requested state before printing the
status, and says so if the request never lands.

## Verification

Parity work is in `scripts/parity-check.sh` (it runs the classic `smc` tool and
`rsmc` side by side on live hardware):

```
  [1/4] -f fan dump        : identical (8 lines)
  [2/4] -l key+type column : identical (1645 keys)
  [3/4] -l payloads        : 1328 keys held still, 317 moved, 17 known deviations (4 si16 sign, 13 unreadable), 0 to re-verify
  [4/4] re-verification    : 0 reproducible, 0 stable-and-equal, 0 moving/uncomparable
  PASS — no unexpected differences
```

What that means, and how it was measured:

- **Fan dump**: byte-identical, apart from the live current-speed line.
- **Key enumeration**: same 1645 keys in the same order, and the key + type
  columns are identical for all of them.
- **Values**: only keys that hold still through the measurement window are
  compared (the reference tool's two dumps bracket ours). Every remaining
  mismatch is then re-read 5 times with each tool, and only counts as a real
  difference when each tool's readings are themselves stable and the two values
  differ. Both steps are needed: hundreds of keys drift between two dumps, and
  keys like `aP70` — an internal counter that oscillates in a narrow band —
  mismatch on virtually every single read without being a difference at all.
  Anything that won't hold still is reported as uncomparable rather than being
  quietly counted as a pass.
- **Writes**: a real round trip was verified against the reference tool
  (`macfan set 3000` → `smc -f` reports target 3000 → `macfan max` restores the
  previous state), and idempotent writes were checked too.
- **Both slices** of the universal binary run: arm64 natively, x86_64 under
  Rosetta, each byte-identical to the reference on `-f`.

Hardware this was developed and checked on: MacBook Pro 13" M2 (macOS 15,
1 fan) — full parity, writes, both slices; MacBook Pro 14" M1 Pro (macOS 26,
2 fans) — fan decoding verified read-only (both fans, per-fan limits).

## Intentional differences from the classic `smc` tool

Four of these are deliberate; two fix defects in the classic tool.

1. **`si16` keys keep their sign.** The classic tool casts the payload to a
   host-order `int16`, byte-swaps it with `ntohs`, and prints the result as
   unsigned, so `ff 32` prints as `65330` instead of `-206`. `rsmc` prints
   `-206`.
2. **Unreadable payloads say `unreadable`.** When a key's payload read fails,
   the classic tool prints the key's type and then `data_size` bytes — but
   `data_size` can exceed the 32-byte buffer, so it prints adjacent stack
   memory (one key here claims 117 bytes). `rsmc` prints the type and
   `unreadable` instead of inventing bytes.
3. **`-v` reports this program's version** (`0.1.0`), not `0.01`.
4. **A failure exits non-zero.** The classic tool prints `Error: ...` and still
   exits 0, which scripts cannot detect.
5. **Short keys are padded** (`-k FS!` == `-k FS! `), and `-kFNum` (attached
   argument) works like `-k FNum`.

`macfan` is additive: the classic tool has no fan-control commands, and its
`-t` prints nothing at all on Apple Silicon (no sensor there uses `sp78`),
which is why `macfan temps` decodes the `flt` sensors Apple Silicon actually
publishes.

## Caveats

- Writing to the SMC is writing to a system interface that Apple does not
  document or support. Nothing here changes firmware or persistent state, and
  a reboot always restores system fan control, but the usual warning applies:
  use it at your own risk.
- `macfan min` pins a fan at its lowest speed. Under sustained load that means
  higher temperatures; `macfan auto` or a reboot gives control back.
- SMC keys are machine-specific. Keys the code doesn't know (or that a given
  Mac doesn't expose) are reported as `no data` / `unreadable` rather than
  guessed at.

## License and provenance

MIT OR Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE`.

This is an independent implementation. The IOKit selector numbers, command
codes and the `KeyData` layout are interface facts required to talk to the
kernel driver, and everything else — the client, the decoders, the CLI, the
fan logic — was written for this crate; no code was copied or translated from
another SMC project. For parity testing it compares its output against the
classic `smc` tool (GPL-2.0, from hholtmann/smcFanControl) when that binary is
present on the machine being tested; no GPL code or binary is included here.

---

## 中文说明

macOS 上控制风扇 + 通用 SMC 工具，Rust 实现，两个二进制、零依赖：

- `macfan`：看风扇转速/模式/温度；`max` 满速、`min` 最低、`set <rpm>` 指定转速
  （各自按该风扇上限封顶）、`auto` 交还系统、`temps` 列温度传感器。
- `rsmc`：通用 SMC 客户端，命令行与经典 `smc` 工具一致（`-f`/`-t`/`-l`/`-k`/`-r`/`-w`），
  可直接替换原有脚本里的 smc。

特点：单文件通用二进制（arm64 + x86_64 都能跑，Intel 机不用重编）；读不用权限、写自动
`sudo` 重新执行（也可用 `SUDO_PASSWORD` 免交互）；输出与经典工具逐字节对齐，并把经典工具
的**两个 bug**（`si16` 丢符号、读失败的键会打印越界栈内存）修正并写在这里。强制转速是临时的
——重启/睡眠/合盖即回到系统自动。
