# macfan

[![Release](https://img.shields.io/github/v/release/greatyingzi/macfan?sort=semver&color=blue)](https://github.com/greatyingzi/macfan/releases/latest)
[![CI](https://github.com/greatyingzi/macfan/actions/workflows/ci.yml/badge.svg)](https://github.com/greatyingzi/macfan/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可)
[![Platform](https://img.shields.io/badge/platform-macOS%2011%2B-lightgrey.svg)](#安装)
[![Homebrew](https://img.shields.io/badge/Homebrew-greatyingzi%2Ftap%2Fmacfan-orange.svg)](#安装)

[English](README.md) · **简体中文**

macOS 上的风扇控制 + AppleSMC 客户端，Rust 实现。一个菜单栏应用和两个小的命令行二进制。
免费、无遥测、不需要账号、不需要特权助手。

```console
$ macfan status          # 命令行的一条命令；应用把这些显示在界面上
Fan #0:
    Current speed : 4128
    Minimum speed: 1199
    Maximum speed: 7199
    Target speed : 4128
    Mode         : auto
```

## 安装

```sh
brew install --cask greatyingzi/tap/macfan
```

macfan 是 ad-hoc 签名（没有公证），首次启动需要放行一次：

```sh
xattr -dr com.apple.quarantine /Applications/macfan.app
```

不用 Homebrew 的话，从[最新 release](https://github.com/greatyingzi/macfan/releases/latest)
下载 `macfan-universal.tar.gz`，解开后跑 `./install.sh`；从源码则是 `cargo build --release`。
要求 macOS 11 及以上，任意架构。

## 支持这个项目

macfan 免费，并且打算一直免费：无遥测、无账号、不把功能锁在付费后面。如果它在你机器上留下来了，
想表示一下：这里有 [Ko-fi 页面](https://ko-fi.com/yingzi62662) —— 页面右上角的 **Sponsor**
按钮指向同一处。打赏是一次性的，用在项目实际开销上，不变成谁的收入：一个 Apple Developer ID
能让所有用户免掉首次启动的放行；二手机器能扩大机型验证 —— 目前只有一台 M2 MacBook Pro。
**打赏不解锁任何东西。**

比钱更有用的事：**报告不工作的地方** —— 某台机器上 `macfan status` 转速不对、某个键读出
`unreadable`、风扇始终落不到目标值；或者在没被验证过的硬件上试一下，Intel Mac 和台式机都还没测。

## 菜单栏应用

应用是这个项目的主体：不用终端就能看转速、改转速、看温度。

```sh
cargo run --manifest-path menubar/Cargo.toml    # 或直接装发布的 .app
```

状态栏显示你选的内容 —— 风扇转速（`7219`，双风扇为 `7219/2400`）或 CPU 热点（`79°C`）——
外加可选的图标（风扇 / 温度计），仅此而已：没有模式标记，转速后面也没有单位。

| 菜单项 | 作用 |
|--------|------|
| `当前: …` | 实时状态行；打勾表示当前生效的模式，读的是 SMC 实测状态而不是上次点了什么 |
| 强制最高 / 最低转速 | 每台风扇压到各自极限 |
| 自动（系统控制） | 交还系统 |
| 设定转速… | 对话框输入，校验并按本机范围封顶 |
| 设定 3000 / 4500 / 6000 rpm | 一键预设（可在设置里改） |
| 开机时启动 | 用户级 LaunchAgent，没有守护进程 |
| 显示内容 / 标题带图标 | 标题报告 `转速` 还是 `温度`，以及旁边放不放图标 |
| 语言 | 跟随系统：英文、简体中文、繁體中文、日本語 |
| 刷新 / 退出 | 立即重读（否则每 2 秒）/ 退出 |

**温度。** "温度"不是一个数：126 个传感器测的位置各不相同，CPU 热点 92 °C 和电池 36 °C
在同一时刻都为真。应用的温度行展开后是八个族的子菜单，每族按 15 秒平滑、按**各自阈值**着色 ——
45 °C 对电池和对 CPU 供电完全是两回事：

| 族 | 温 | 热 | 族 | 温 | 热 |
|----|----|-----|----|----|-----|
| CPU | 85 °C | 100 °C | 内存 | 55 °C | 70 °C |
| CPU 芯片 | 80 °C | 95 °C | 电池 | 40 °C | 45 °C |
| GPU | 75 °C | 90 °C | 充电器 | 45 °C | 55 °C |
| 散热片 | 65 °C | 80 °C | 无线 | 50 °C | 60 °C |
| 固态（邻近） | 60 °C | 75 °C | 供电 | 70 °C | 85 °C |

数字取的是**热点而不是均值** —— 单核传感器会 park、会掉出可读集合，样本数一变均值就跳，
而最大值稳定，而且它才是驱动风扇提速和降频的量。它在 15 秒时间常数上做平滑（裸读数两次之间
会动 ±3…5 °C）；窗口峰值比平滑值高 3 °C 以上时会一起显示。

**设置。** 窗口里涵盖标题显示、语言、开机启动、三个预设，以及**启动时恢复上次转速**（默认关闭）。
睡眠、重启、合盖都会把风扇交还系统，这个选项在应用启动时把设定放回去；如果机器已经处于该状态
它会跳过写入，所以授权框只在真会改变时出现。

**权限。** 读不需要权限；写需要 root —— 应用让 `macfan` CLI 去写（CLI 自己处理 sudo），
或退回 macOS 标准的授权对话框。没有特权助手、没有守护进程、没有 kext。写完之后它会重读 SMC，
如果风扇没落到请求状态会警告。

**打包。** `menubar/scripts/make-app.sh` 生成 `dist/macfan.app`（ad-hoc 签名，图标由
`assets/icon.svg` 渲染），`make-dmg.sh` 打成 DMG。把 app 拖进 `/Applications`（启动台只索引
`/Applications` 和 `~/Applications`），并按「安装」一节清掉隔离标记。

## 命令行

两个二进制、零依赖、通用版（arm64 + x86_64）。

| 二进制 | 作用 |
|--------|------|
| `macfan` | 看转速和模式；强制最高 / 最低 / 指定转速；交还控制 |
| `rsmc` | 通用 AppleSMC 客户端，命令行与经典 `smc` 一致（`-f -t -l -k -r -w`），可直接替换脚本 |

### `macfan`

| 命令 | 作用 |
|------|------|
| `macfan` / `status` / `list` | 每台风扇的转速、模式，以及 SMC 报的 min/max/safe/target |
| `macfan max` / `min` | 各自压到最高 / 最低 |
| `macfan set <rpm>` | 强制到指定转速，按每台风扇上限封顶 |
| `macfan auto` | 交还系统 |
| `macfan temps` | 按族看温度（见下） |

`--no-sudo` 会直接报权限错误而不是提权。强制转速是临时的：重启、睡眠、合盖都会回到系统控制。

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

`macfan temps <族>` 只看一族，`--all` 列出每个传感器并带名字。`state` 用的是和应用一样的
温/热阈值；读数为 0 的传感器跳过（那是没接线的）。

哪些族可信是**测出来的**，不是猜的 —— 每族都特意加过负载（M2 13" MacBook Pro）：

| 族 | 键 | 实测响应 |
|----|----|----------|
| CPU | `Tp0*`、`Tp1*`、`Te0*` | 8 线程下 +24…29 °C |
| CPU 芯片 | `TCMz`、`TCMb` | SMC 自带的芯片最大 / 平均，同步上升 |
| GPU | `Tg0*`、`Tg1*` | WebGL 燃烧 +11…12 °C，同期 CPU 只 +2 |
| 散热片 | `Th0*` | GPU 负载 +7.5 °C，更慢更平滑 |
| 电池 | `TB*T` | 36.4 °C，各种负载纹丝不动 —— 真实，只是惰性 |
| 充电器 / 内存 / 无线 | `TCHP`、`TMVR`/`TVM*`、`TW0*` | 充电器充电时上升；内存轨 44…64 °C；无线 +0.7 °C |

两个各值得一句话的提醒。**固态那一族不是磁盘温度**：90 秒持续写入只让 `Ts0*` 动 3.7 °C，
和芯片传感器一样，说明它跟的是机板热；而且 macOS 根本没有 NVMe 温度通道。所以它标成
*SSD (proximity)*，从不当作存储温度展示。SMC 的推算值（`TVS*`、`TVD*`、`TVA*`、`TAO`）
是别的读数的重复，只在 `--all` 里出现。

### `rsmc`

| 参数 | 含义 |
|------|------|
| `-f` | 解码后的风扇信息 |
| `-t` | 温度（`sp78` 传感器，与经典工具一致） |
| `-l` | 所有键及其值 |
| `-k <key>` `-r` `-w <hex>` | 操作指定键：读，或写原始字节 |
| `-v`、`-h` | 版本、帮助 |

```console
$ rsmc -k F0Ac -r
  F0Ac  [flt ]  4128 (bytes 00 40 81 45)
```

键是 4 个字符，不足补空格（`FS!` 与 `FS! ` 等价）。

# 技术细节

## 工作原理

SMC 通过 `AppleSMC` IOKit 服务访问。`rsmc` 建连接后发 struct call（selector `2`），
携带 `KeyData` 结构：键名（大端 `u32`）、命令字节（`5` 读字节、`6` 写字节、`8` 按索引取键、
`9` 读键信息）、键的大小与类型、32 字节 payload。一次读是两次调用 —— 先取大小再读字节 ——
大小永不改变，所以会缓存。

强制风扇是两次写：`F0Md = 1`（手动），再写 `F0Tg`（目标转速，4 字节小端 f32）；`auto` 写
`F0Md = 0`。风扇数量来自 `FNum`，每台风扇的键是 `F<索引><后缀>`，所以单风扇、双风扇乃至更多
都能工作，且各自按自己的上限封顶。SMC 会在 200–400 ms 后才应用写入，所以 `macfan` 会轮询到
风扇报出请求状态为止，而不是相信退出码。

## 验证

`scripts/parity-check.sh` 在真机上把经典 `smc` 和 `rsmc` 并排跑：

```
  [1/4] -f fan dump        : identical (8 lines)
  [2/4] -l key+type column : identical (1645 keys)
  [3/4] -l payloads        : 1328 keys held still, 317 moved, 17 known deviations (4 si16 sign, 13 unreadable), 0 to re-verify
  [4/4] re-verification    : 0 reproducible, 0 stable-and-equal, 0 moving/uncomparable
  PASS — no unexpected differences
```

只比较在测量窗口内保持不动的键；剩下的每处不一致都用两个工具各重读五次，只有双方读数各自稳定
且数值确实不同才算真差异。稳不下来的一律报"不可比"，不会算成通过。写入验证过真实往返，
通用二进制的两个切片（arm64 原生、x86_64 走 Rosetta）都跑过。所用硬件：MacBook Pro 13" M2
（macOS 15，单风扇；完整对齐、写入、两个切片）与 MacBook Pro 14" M1 Pro（macOS 26，双风扇；
风扇解码只读验证）。

GUI 有自己的检查：`macfan-menubar --selftest` 在主线程上建出整个界面再读回去然后退出 ——
状态栏标题、菜单项及接线数量、启动项往返、转速解析与封顶，以及标题/温度的接线。
`MACFAN_LANG` 可逐语言跑。

## 与经典 `smc` 工具的差异

三条是刻意的，两条修的是缺陷。

1. **`si16` 键保留符号。** 经典工具把 `ff 32` 打成 `65330` 而不是 `-206`（本机序强转 + `ntohs`）。
   `rsmc` 打 `-206`。
2. **读不到的 payload 报 `unreadable`。** 经典工具在读取失败时会从一个可能更小的缓冲区里
   打印 `data_size` 个字节，也就是相邻栈内存（本机有个键自称 117 字节）。`rsmc` 打印类型和
   `unreadable`。
3. **`-v` 报本程序版本**，不是 `0.01`。
4. **失败时退出码非零**，脚本能据此判断。
5. **短键补空格**（`-k FS!` 等价 `-k FS! `），`-kFNum` 也能用。

`macfan` 是加法：经典工具没有风扇控制，它的 `-t` 在 Apple Silicon 上什么都不打印
（那上面没有传感器用 `sp78`），所以 `macfan temps` 解码 Apple Silicon 实际发布的 `flt` 传感器。

## 注意事项

- 写 SMC 就是在写一个 Apple 不公开、不支持的接口。这里没有任何操作会改固件或持久状态，
  重启总能恢复系统风扇控制 —— 但老规矩：**风险自负**。
- `macfan min` 把风扇钉在最低速，持续负载下意味着更高温度；`macfan auto` 或重启即可交还。
- SMC 键因机而异。不认识的键（或本机没有的键）会报 `no data` / `unreadable`，而不是去猜。

## 许可

MIT OR Apache-2.0 —— 见 `LICENSE-MIT` 与 `LICENSE-APACHE`。

独立实现：IOKit 的 selector 编号、命令码与 `KeyData` 布局是与驱动通讯所必需的接口事实，
其余一切为本 crate 所写。没有从任何其它 SMC 项目复制或翻译代码。对齐测试会在被测机器上存在
经典 `smc` 工具（GPL-2.0，来自 hholtmann/smcFanControl）时与它比对输出；本仓库不含任何
GPL 代码或二进制。
