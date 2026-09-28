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
