# macfan

[![Release](https://img.shields.io/github/v/release/greatyingzi/macfan?sort=semver&color=blue)](https://github.com/greatyingzi/macfan/releases/latest)
[![CI](https://github.com/greatyingzi/macfan/actions/workflows/ci.yml/badge.svg)](https://github.com/greatyingzi/macfan/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可与来源)
[![Platform](https://img.shields.io/badge/platform-macOS%2011%2B-lightgrey.svg)](#安装)
[![Homebrew](https://img.shields.io/badge/Homebrew-greatyingzi%2Ftap%2Fmacfan-orange.svg)](#homebrew)
[![Downloads](https://img.shields.io/github/downloads/greatyingzi/macfan/total.svg)](https://github.com/greatyingzi/macfan/releases)

[English](README.md) · **简体中文**

macOS 上的 SMC 风扇控制 + 通用 AppleSMC 客户端，Rust 实现。两个小二进制，零依赖，不需要特权助手。

| 二进制 | 作用 |
|--------|------|
| `macfan` | 读风扇转速/模式/温度；把风扇强制到最高 / 最低 / 指定转速；交还系统控制 |
| `rsmc` | 通用 SMC 客户端，命令行与经典 `smc` 工具一致（`-f` `-t` `-l` `-k` `-r` `-w`），可直接替换脚本里的 smc |

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
...
    Target speed : 7199
    Mode         : forced

$ macfan temps | head -4
  TB0T    34.6 °C
  TB1T    34.6 °C
  TB2T    32.2 °C
  TCHP    52.1 °C

$ macfan auto        # 把风扇还给系统
```

- **一份二进制跨架构** —— 发布版本是通用二进制（arm64 + x86_64），同一个文件在 Apple Silicon
  和 Intel Mac 上都能跑。不需要 Homebrew、Python、Perl，也不用每台机器重编。
- **读不需要权限。** 写需要 root，`macfan` 会自己通过 `sudo` 重新执行一次（只弹一次授权，之后有缓存）。
  环境变量 `SUDO_PASSWORD` 可免交互；`--no-sudo` 完全关闭提权。
- **与经典工具逐字节对齐**，不是"看着对"：本机暴露的每个 SMC 键都做了对比，不一致的地方会重测，
  经典工具的**两个真 bug** 在下面写清楚了。

---

## 功能

**菜单栏应用** —— 是个正经 app，不是脚本：

- 菜单里直接看转速、改转速：强制最高、强制最低、三档预设、或手输任意转速（每台风扇按各自上限封顶）。
- **按族看温度**，不是一个数：CPU 热点、CPU 芯片、GPU、散热片、固态（邻近）、内存、电池、无线。
  每族按 15 秒窗口平滑，并用**各自的阈值**判断冷热 —— 45 °C 对电池要提醒，对 CPU 供电毫无意义。
  （详见 `macfan temps` 一节）
- **启动时恢复上次设定**（默认关闭）：睡眠、重启、合盖都会把风扇交还系统，这个选项会把你的设定放回去。
- 设置窗口带语言选择；界面自带**英文 / 简体中文 / 繁體中文 / 日本語**，默认跟随系统。
- 以常规 `.app`（通用二进制 + DMG）分发，并有一个 Homebrew cask。

**命令行** —— 两个小二进制、零依赖：

- `macfan` —— 风扇状态、`max` / `min` / `set <rpm>` / `auto`，以及按族看温度的 `macfan temps`。
- `rsmc` —— 通用 AppleSMC 客户端，保留经典 `smc` 的命令行（`-f -t -l -k -r -w`），
  可直接替换脚本里的 smc。逐键对齐由 `scripts/parity-check.sh` 检查。

**实现层面** —— 读无需权限，写只走一次 `sudo` 授权。没有 kext、没有 launch daemon、
没有特权助手。发布二进制为通用版（arm64 + x86_64），支持 macOS 11 及以上。

## 安装

### Homebrew

```sh
brew install --cask greatyingzi/tap/macfan
```

cask 会校验 DMG 的校验和；`brew upgrade --cask macfan` 跟新版本。macfan 是 ad-hoc 签名
（没有公证），所以 macOS 会给下载物打隔离标记，首次启动需要放行一次 —— 一行命令清掉：

```sh
xattr -dr com.apple.quarantine /Applications/macfan.app
```

更多细节见「.app 打包与 DMG」。

### 通用二进制（不需要工具链）

```sh
curl -LO https://github.com/greatyingzi/macfan/releases/latest/download/macfan-universal.tar.gz
tar xzf macfan-universal.tar.gz
cd macfan-universal && ./install.sh
```

同一个 release 里也单独放了两个二进制（`macfan-<版本>-macos-universal`、
`rsmc-<版本>-macos-universal`）和一份 `SHA256SUMS`。GitHub 不保留 release 资产的可执行位，
所以单独下载的要先 `chmod +x`；tar 包保留权限位。

`install.sh` 会把二进制放到 `~/.local/bin`，如果这个目录不在 `PATH` 里会提示该加哪一行。

### 从源码（需要 Rust 工具链）

```sh
git clone https://github.com/greatyingzi/macfan
cd macfan
cargo build --release            # target/release/{macfan,rsmc}
scripts/install.sh target/release
```

系统要求：macOS 11 及以上，任意架构。读 SMC（含风扇控制）走 `AppleSMC` IOKit user client，
每台 Mac 都有。

## 支持这个项目

macfan 是业余时间写的个人项目，定位是"任何人都能用"：免费、无遥测、不需要账号、
不做付费才有的功能 —— 应用能做的事，所有用户都能做。

如果它在你菜单栏上留下来了，想表示一下：这里有 [Ko-fi 页面](https://ko-fi.com/yingzi62662)
（页面右上角的 **Sponsor** 按钮指向同一处，收到的是同一个 PayPal 账户）。打赏是一次性的，
会用在项目实际的开销上，而不是变成谁的收入：一个 Apple Developer ID 可以让所有用户
免掉"首次启动要放行一次"这一步；添置二手机器可以扩大机型验证 —— 目前风扇控制路径
只在一台 M2 MacBook Pro 上验证过。国内通道（爱发电）还在准备中。

打赏不解锁任何东西：没有 issue 优先，没有额外功能，也不要求你署名。不打赏才是常态，
而且不影响任何事情。

比钱更有用的事，大致按价值排序：

1. **报告不工作的地方** —— 某台机器上 `macfan status` 转速不对、某个键读出 `unreadable`、
   或者风扇没有落到目标转速，这些是最有价值的信息（*验证* 一节写了已经覆盖了什么，
   缺口因此是可见的）
2. **在没被验证过的硬件上试一下** —— Intel Mac、台式机、双风扇笔记本目前都还没测过
3. **点个 Star，或在合适的地方带上它**

支持者不会被列在任何地方（除非你自己要求），也没有 newsletter、没有 Discord、
没有需要维护的会员档位。

## 用法

### `macfan` —— 风扇

| 命令 | 作用 |
|------|------|
| `macfan` / `macfan status` | 每台风扇的转速、模式，以及 SMC 报的 min/max/safe/target |
| `macfan max` | 每台风扇强制到**各自**的最高速 |
| `macfan min` | 每台风扇强制到各自的最低速（安静 —— 但要盯温度） |
| `macfan set <rpm>` | 强制到 `<rpm>`，按每台风扇各自上限封顶 |
| `macfan auto` | 交还系统控制 |
| `macfan temps` | 列出所有可读温度传感器（°C） |
| `macfan list` | 原始风扇信息（`status` 的别名） |

选项：`--no-sudo`（报权限错误而不是提权）、`-V`/`--version`、`-h`/`--help`。

强制转速是**临时的**：重启、睡眠、合盖都会回到系统控制，`macfan auto` 立即交还。

### `macfan temps` —— 按族看温度

"温度"不是一个数。一台 MacBook Pro 有 126 个可读传感器，测的位置和量级都不同：
CPU 热点 92 °C 和电池 36 °C 在同一时刻都为真，两者取平均没有任何意义。

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
  (per-sensor list: macfan temps --all; one family: macfan temps gpu)
```

`state` 用的是该族的**热点**对比该族**自己的阈值** —— 45 °C 对电池要提醒，对 CPU 供电毫无意义。
读数为 0 的传感器一律跳过：那是没接线的，算进均值只会白白拉低。

#### 哪些读数可信

每个族都是**特意加负载观测过**的，只有"因为正确的原因而动"才被算作一个族
（M2 13" MacBook Pro，2026-09）：

| 族 | 键 | 实测响应 |
|----|----|----------|
| CPU | `Tp0*`、`Tp1*`、`Te0*` | 8 线程 CPU 负载下 +24…29 °C |
| CPU 芯片 | `TCMz`（最大）、`TCMb`（平均） | SMC 自己算的芯片聚合值，同步上升 |
| GPU | `Tg0*`、`Tg1*` | WebGL 着色器燃烧 +11…12 °C，同期 CPU 只动 +2 |
| 散热片 | `Th0*` | GPU 负载 +7.5 °C，比芯片慢而平滑 |
| 电池 | `TB*T` | 36.4 °C，各种负载下纹丝不动 —— 真实但惰性的探针 |
| 充电器 | `TCHP` | 充电时上升 |
| 内存 | `TMVR`、`TVM*` | 内存供电轨与稳压器，44…64 °C |
| 无线 | `TW0*` | +0.7 °C：小，但是真的 |

**不可信的是固态那一族。** `Ts0*`、`Tsx*`、`TH0*` 在社区键库里被标成 SSD/NAND，
但 90 秒 `fsync` 持续写入只让它们动了 3.7 °C —— 和芯片传感器动的幅度一样，说明跟的是
机板热而不是闪存活动。macOS 也没有任何 NVMe 温度通道（`system_profiler SPNVMeDataType` 是空的、
`ioreg` 里控制器没有温度属性、`smartctl` 读不了 Apple 的存储）。所以它们被列为
**SSD (proximity)**，从不当作磁盘温度展示。

SMC 的推算值（`TVS*`、`TVD*`、`TVA*`、`TAO`）是别的读数的重复，不是独立测点，
所以只出现在 `macfan temps --all` 里，不进汇总、不报警。

#### 显示出来的是什么，怎么算的

- **取热点，不取均值。** 每个核心的传感器会 park、会从可读集合里掉出去，样本数一变均值就跳；
  最大值稳定，而且它就是驱动风扇提速和降频的那个量。
- **按 15 秒时间常数平滑**（用真实采样间隔算的指数移动平均）。裸读数在两次读之间会动 ±3…5 °C，
  一眼看过去没法读；15 秒既稳住数字，又能在二十秒内反映一次负载爬升。窗口峰值比平滑值高
  3 °C 以上时会一起显示 —— 已经过去的尖峰，才是盯着看的人想知道的。
- 菜单栏应用把同样的族放在温度行的子菜单里，每族按各自阈值着色。

#### 菜单栏里显示什么

应用展示的是同一批族，但有两点不同。它的温度行报告**一个**族（默认 CPU，由标题的
「显示内容」设置决定），展开那一行会列出**八个**族：CPU、CPU 芯片、GPU、散热片、
固态（邻近）、内存、电池、无线。其余几个（充电器、供电、系统、其他）留在 CLI 里 ——
它们不会随负载变化。

每族的数字是同一个 15 秒移动平均，**按各自阈值着色**。变色的只是颜色，所以 41 °C 的电池和
41 °C 的 CPU 看起来不一样，而两个数字都没有说谎：

| 族 | 温（warm） | 热（hot） |
|----|-----------|----------|
| CPU | 85 °C | 100 °C |
| CPU 芯片 | 80 °C | 95 °C |
| GPU | 75 °C | 90 °C |
| 散热片 | 65 °C | 80 °C |
| 固态（邻近） | 60 °C | 75 °C |
| 内存 | 55 °C | 70 °C |
| 电池 | 40 °C | 45 °C |
| 充电器 | 45 °C | 55 °C |
| 无线 | 50 °C | 60 °C |
| 供电 | 70 °C | 85 °C |
| 系统、其他 | 70 °C | 85 °C |

这两个数字与 CLI `state` 列用的是同一套，所以窗口和终端永远不会互相矛盾。

### 菜单栏标题

两个设置，一个维度一个：

| 设置 | 取值 | 作用 |
|------|------|------|
| 显示内容 | `转速` / `温度` | 那个数字是什么 |
| 标题带图标 | 开 / 关 | 旁边是否放一个图标 |

转速是裸的 —— `7219`，双风扇机器是 `7219/2400`。温度保留度号（`79°C`），这是区分两者的唯一标记。
图标只表示读数类型（转速=风扇图标，温度=温度计），不承载任何状态：当前模式由菜单第一行用文字说清。

0.2.2 及更早的配置文件在读取时自动迁移：`title_style = rpm_only` → 转速无图标，
`icon_rpm` → 转速带图标，`icon_temp` → 温度带图标。

### `rsmc` —— 原始 SMC 访问

| 参数 | 含义 |
|------|------|
| `-f` | 解码后的风扇信息 |
| `-t` | 列温度（`sp78` 传感器，与经典工具行为一致） |
| `-l` | 列出所有键及其值 |
| `-k <key>` | 指定要操作的键，如 `-k F0Ac` |
| `-r` | 读 `-k` 指定的键 |
| `-w <hex>` | 向 `-k` 指定的键写原始字节 |
| `-v`、`-h` | 版本、帮助 |

```console
$ rsmc -k F0Ac -r
  F0Ac  [flt ]  4128 (bytes 00 40 81 45)

$ rsmc -k F0Tg -r                      # 目标转速，原始 f32
  F0Tg  [flt ]  4128 (bytes 00 40 81 45)

$ sudo rsmc -k F0Tg -w 00006042        # 手写 56.0 rpm
```

SMC 键固定 4 个字符，不足补空格（`FS!` 与 `FS! ` 等价）。

## 菜单栏应用

`menubar/` 是基于同一个库写的菜单栏项目（通过 `objc2` 用 AppKit —— 没有 Swift、没有 Xcode 工程、
没有 Electron）：

```sh
cargo run --manifest-path menubar/Cargo.toml
```

状态栏显示你选的内容 —— 风扇转速（`7219`，双风扇为 `7219/2400`）或 CPU 热点（`79°C`）——
外加可选的图标，仅此而已：没有模式标记，转速后面也没有单位。见「菜单栏标题」。
菜单里每台风扇一行实时状态，然后是：

| 菜单项 | 作用 |
|--------|------|
| 强制最高 / 最低转速 | 把每台风扇压到各自极限 |
| 自动（系统控制） | 交还系统 |
| 设定转速… | 带输入框的对话框，校验并按本机范围封顶 |
| 设定 3000 / 4500 / 6000 rpm | 一键预设 |
| 开机时启动 | 勾选后安装用户级 LaunchAgent |
| 预设转速 | 三个可编辑输入框，用「应用」按钮提交 —— 菜单和勾选会跟着变 |
| 显示内容 | 标题报告哪个量：`转速` 或 `温度` |
| 标题带图标 | 旁边是否放图标（风扇 / 温度计） |
| 语言 | 默认跟随系统；选择器在下次启动生效 |
| 刷新 | 立即重读（否则每 2 秒一次） |
| 退出 | |

菜单还会说明**当前生效的是哪种模式**：顶部一行 `当前: …`、对应项打勾（自动/最高/最低/预设），
以及当生效转速不属于任何预设时的一条 `自定义 <rpm> rpm`。勾选状态来自**实测状态** ——
每台风扇的目标值对比它自己的上下限 —— 而不是上次点了什么，所以别的工具或 SMC 本身改了转速，
菜单会在两秒内跟上。如果各风扇最终目标不一致，会如实报"混合"状态，不会假装统一。

**语言。** 界面跟随系统语言 —— 目前提供英文、简体中文、繁體中文、日本語，其它语言回落到英文。
`MACFAN_LANG=zh-Hans` 可强制指定（脚本和自测里有用）。命令行刻意保持英文：
它的输出要与经典 `smc` 工具逐字节对比。

**写入与权限。** 写 SMC 需要 root。应用内：如果它本身以 root 运行，或设了 `SUDO_PASSWORD`，
它就让 `macfan` CLI 去写（CLI 自己处理 sudo）；否则走 `osascript … with administrator privileges`，
也就是 macOS 标准的授权对话框。没有特权助手、没有签名要求、没有 daemon。帮助进程返回后，
应用会重新读 SMC，如果风扇没有落到请求的状态会警告 —— 写入是异步的，退出码 0 本身不代表写成功。

**开机时启动** 会写 `~/Library/LaunchAgents/com.macfan.menubar.plist`（用 `plutil -lint` 校验过）
并保持未加载状态，所以勾选不会当场起第二个副本；launchd 在下次登录时接管。现代的
`SMAppService` 需要已签名的 bundle，所以等有了签名再说。同一件事也能脚本化：

```sh
macfan-menubar --login-status    # 已安装 / 未安装
macfan-menubar --login-install
macfan-menubar --login-remove
```

**验证。** `macfan-menubar --selftest` 在主线程上把整个界面建出来再读回去然后退出：
状态栏标题、菜单项及其中多少个接了动作、启动项在临时目录里的往返、转速输入的解析与封顶。
配合 `MACFAN_LANG` 可以逐语言检查。agent 环境里拿不到截图（没有屏幕录制权限），
而"启动没崩"并不能证明这些接线是对的。

### 启动时恢复

默认情况下应用启动时不碰风扇：写 SMC 需要 root，意味着会弹授权框，不该在登录时突然扑到人脸上。

**启动时恢复上次设定**（默认关闭）会记住上次**应用过的**设定，并在应用启动时放回去。
这条重要的原因是：睡眠、重启、合盖都会把风扇交还系统控制，否则强制转速只能撑到机器喘口气为止。

两个细节值得知道：

- 它记的是**本应用应用过的**最后一次设定，不是启动时 SMC 报的状态。启动时那一定是自动，
  按它记会把要恢复的东西本身抹掉。
- 如果机器已经处于该状态，恢复会**跳过写入**，所以授权框只在真会改变时出现。写需要 root：
  实测无权限写入会失败并报 `SMCWriteKey() = e00002c1`（not privileged）。

在 M2 上端到端实测过：把风扇交还系统（`Mode: auto`）→ 打开开关 → 重启应用 →
`Mode: forced`、目标 7199。

### .app 打包与 DMG

```sh
menubar/scripts/make-app.sh          # -> dist/macfan.app   （ad-hoc 签名）
menubar/scripts/make-dmg.sh          # -> dist/macfan-<版本>.dmg
```

打成 bundle 才能让图标、应用菜单名和弹窗配图正确：裸二进制会继承终端的图标，
`NSAlert` 也没有 app 图标可用。`make-app.sh` 还会把 `macfan` 和 `rsmc` 复制进
`Contents/MacOS/`，GUI 就是去那里找它驱动的 CLI。

图标源文件是 `menubar/assets/icon.svg`（渲染成品 `assets/icon-1024.png`，
用 `scripts/make-icon.py` 重新生成）。几何按 Big Sur 网格 —— 1024 画布上 824×824 图形、
185.5 圆角、外部透明 —— 所以放在系统图标旁边不违和。图形刻意简单（四片粗叶 + 中心轴 + 蓝色渐变），
保证 16 px 下不糊。

**安装。** 把 `macfan.app` 拖进 `/Applications`。位置是有讲究的：启动台只索引 `/Applications`
和 `~/Applications`，你在哪里解开的 DMG 不算，所以留在 `~/Downloads` 的 app 不会出现在启动台。

**Gatekeeper。** 没有 Developer ID 的 bundle 只有 ad-hoc 签名，所以下载来的副本会带隔离标记，
macOS 会拒绝首次启动。在 macOS 15 上实测：带标记的副本启动后毫无反应；清掉标记就正常启动。清一次：

```sh
xattr -dr com.apple.quarantine /Applications/macfan.app
open /Applications/macfan.app
```

`-r` 很关键：隔离标记也挂在 bundle 内部的二进制上，只清 `.app` 目录本身不够。
图形界面的等价做法是右键 → 打开；如果 macOS 只给"完成"，就去系统设置 → 隐私与安全性 → "仍要打开"。
从源码构建的副本根本不会有这个标记。DMG 里附带的安装说明写着同样的步骤。

已发布 Homebrew cask —— `brew install --cask greatyingzi/tap/macfan` —— 安装、升级
（`brew upgrade --cask macfan`）、卸载（`brew uninstall --cask --zap macfan`）各一条命令，
并在下载时校验校验和。

**cask 如何保持最新。** 两条路，确保它不会悄悄落后 —— 曾经落后过一次：tap 停在 0.2.0，
而 release 已经发到 0.2.5，期间所有用户的 `brew upgrade` 都显示"已是最新"。

- **发布时自动更新**：release 工作流用 deploy key（只对该仓库有写权限，不是个人 token）
  克隆 tap，并跑和人工相同的 `scripts/update-cask.sh` —— 它给**实际发布的** DMG 算哈希、
  用 `scripts/cask-template.rb` 渲染、再回读校验。
- **定时任务兜底**：tap 仓库自己每天跑一次同步，完全不需要任何密钥（仓库自带的 token 就能写自己）。
  如果同步不了，它会报错退出，而不是假装成功。

两条都用"故意把 cask 弄落后"验证过：定时任务把 0.2.0 拉回 0.2.6，release 工作流把 0.2.1 拉回 0.2.6。

它**不会**免掉首次启动的放行：在 Homebrew 7 上实测，cask 会刻意给下载物打上隔离属性
（旧的 `--no-quarantine` 参数已不存在），所以 Gatekeeper 仍会问一次。上面那条 `xattr -dr` 清掉即可。
只有 Developer ID 才能真正免掉这一步。

---

# 技术细节

## 工作原理

SMC 通过 `AppleSMC` IOKit 服务访问。`rsmc` 建立连接后发 struct call（selector `2`），
携带一个 `KeyData` 结构：键名（大端 `u32`）、命令字节（`5` 读字节、`6` 写字节、`8` 按索引取键、
`9` 读键信息）、键的大小/类型，以及 32 字节 payload。读是两次调用 —— 先读键信息拿到 payload 大小，
再读字节；键的大小和类型永不改变，所以 `rsmc` 会缓存。

强制风扇要写两次：先把风扇切到手动（`F0Md = 1`），再设目标转速（`F0Tg = <rpm 的 4 字节小端 f32>`）。
`macfan auto` 写 `F0Md = 0`。风扇数量来自 `FNum`，每台风扇的键按 `F<索引><后缀>` 寻址，
所以单风扇、双风扇乃至更多风扇的机器都能工作，且每台风扇按自己的上限封顶。

SMC 会在稍后（实测 200–400 ms）才应用转速写入，所以写完立刻读回看到的是**旧值**。
因此 `macfan` 会轮询直到风扇报出请求的状态再打印结果，如果始终没落到就如实说明。

## 验证

对齐工作都在 `scripts/parity-check.sh` 里（在真机上把经典 `smc` 和 `rsmc` 并排跑）：

```
  [1/4] -f fan dump        : identical (8 lines)
  [2/4] -l key+type column : identical (1645 keys)
  [3/4] -l payloads        : 1328 keys held still, 317 moved, 17 known deviations (4 si16 sign, 13 unreadable), 0 to re-verify
  [4/4] re-verification    : 0 reproducible, 0 stable-and-equal, 0 moving/uncomparable
  PASS — no unexpected differences
```

这些数字的含义与测法：

- **风扇信息**：逐字节相同，只有实时转速那一行例外。
- **键枚举**：同样的 1645 个键、同样的顺序，键名+类型两列全部一致。
- **取值**：只比较在测量窗口内**保持不动**的键（参考工具两次 dump 把我们的夹在中间）。
  剩下的每处不一致都用两个工具各重读 5 次，只有双方读数各自稳定**且**数值确实不同才算真差异。
  两步都必要：两次 dump 之间会有几百个键漂移，而像 `aP70` 这种在窄区间里来回振荡的内部计数器，
  几乎每次读都不一致，却根本不是差异。稳不下来的一律报为"不可比"，不会悄悄算成通过。
- **写入**：对着参考工具验证过真实往返（`macfan set 3000` → `smc -f` 报 target 3000 →
  `macfan max` 恢复原状态），幂等写入也检查过。
- **通用二进制两个切片**都能跑：arm64 原生、x86_64 走 Rosetta，在 `-f` 上都与参考逐字节一致。

开发与验证所用硬件：MacBook Pro 13" M2（macOS 15，单风扇）—— 完整对齐、写入、两个切片；
MacBook Pro 14" M1 Pro（macOS 26，双风扇）—— 风扇解码只读验证（双风扇、各自上下限）。

## 与经典 `smc` 工具的已知差异

其中四条是刻意的，两条是对经典工具缺陷的修正。

1. **`si16` 键保留符号。** 经典工具把 payload 强转成本机序 `int16`、用 `ntohs` 换字节序、
   再按无符号打印，于是 `ff 32` 打成 `65330` 而不是 `-206`。`rsmc` 打 `-206`。
2. **读不到的 payload 报 `unreadable`。** 当某个键的 payload 读取失败，经典工具会打出键类型
   然后打印 `data_size` 个字节 —— 但 `data_size` 可以超过 32 字节缓冲区，于是它把相邻的栈内存
   打了出来（本机有个键自称 117 字节）。`rsmc` 打印类型和 `unreadable`，不编造字节。
3. **`-v` 报本程序版本**（crate 版本），不是 `0.01`。
4. **失败时退出码非零。** 经典工具打印 `Error: ...` 但退出码仍是 0，脚本无法感知。
5. **短键会补空格**（`-k FS!` 等价 `-k FS! `），`-kFNum`（参数贴着写）与 `-k FNum` 等效。

`macfan` 是加法：经典工具没有风扇控制命令，它的 `-t` 在 Apple Silicon 上什么都不打印
（那上面没有传感器用 `sp78`），所以 `macfan temps` 解码 Apple Silicon 实际发布的 `flt` 传感器。

## 注意事项

- 写 SMC 就是在写一个 Apple 不公开、不支持的接口。这里没有任何操作会改固件或持久状态，
  重启总能恢复系统风扇控制 —— 但老规矩还是要说：**风险自负**。
- `macfan min` 会把风扇钉在最低速。持续负载下这意味着更高的温度；`macfan auto` 或重启即可交还控制。
- SMC 键因机而异。代码不认识的键（或某台 Mac 没有的键）会报 `no data` / `unreadable`，
  而不是去猜。

## 许可与来源

MIT OR Apache-2.0 —— 见 `LICENSE-MIT` 与 `LICENSE-APACHE`。

这是独立实现。IOKit 的 selector 编号、命令码与 `KeyData` 布局是与内核驱动通讯所必需的接口事实，
其余一切 —— 客户端、解码、命令行、风扇逻辑 —— 都是为本 crate 写的；没有从任何其它 SMC 项目
复制或翻译代码。做对齐测试时，它会在被测机器上存在经典 `smc` 工具（GPL-2.0，来自
hholtmann/smcFanControl）的情况下与其输出比对；本仓库不包含任何 GPL 代码或二进制。
