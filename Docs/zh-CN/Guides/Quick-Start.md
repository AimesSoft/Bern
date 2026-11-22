[English](../../Guides/Quick-Start.md) · **简体中文**

# 快速开始

本指南带你完成安装 `bern`、创建工作区、编译期检查、多端运行到打包发布的完整流程。

## 前置准备

- **Rust 1.99**（目录下有 `rust-toolchain.toml` 时，`rustup` 会自动安装）。
- 你要构建的目标平台对应的原生工具链（Apple 平台需 Xcode、Windows 需 Visual Studio C++ 生成工具、Linux 需 GTK 4 + libadwaita 开发包、Android 需 Android Studio SDK/NDK + Gradle、Web 需 `wasm32-unknown-unknown`）。具体安装命令见[平台操作手册](../Platforms/macOS.md)。

## 1. 安装 `bern-cli`

推荐使用 [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) 直接下载预编译二进制：

```sh
cargo binstall bern-cli
```

或者从源码编译安装：

```sh
cargo install bern-cli
```

检查当前电脑已经具备哪些平台的构建条件：

```sh
bern doctor
```

`bern doctor` 会逐项检查 Rust、原生 SDK、模拟器与交叉编译目标，并给出缺失项的准确安装指引。

## 2. 创建新项目

```sh
bern new my_app
cd my_app
```

常用可选参数：

- `--bundle-id com.example.myapp` — 反向域名包标识（默认 `com.example.<name>`）。
- `--languages en,zh-Hans` — 逗号分隔的支持语言列表，第一项为默认语言（模板内置 `en`、`zh-Hans`、`zh-Hant`、`ja`）。
- `--bern-path /path/to/Bern` — 让生成的工程指向本地 Bern 源码克隆，而非 crates.io。

生成的 Cargo 工作区默认拆分为清晰的三个 crate：

```text
my_app/
├── Bern.toml            # 应用元数据、多语言、资源与构建目标
├── Cargo.toml           # 工作区配置
├── my_app-protocol/     # 值类型、只读状态与命令 trait
├── my_app-core/         # 纯普通 Rust 业务引擎（零 UI 依赖）
└── my_app-ui/           # 声明式 #[component] + ui! 界面
```

## 3. 检查、运行与构建

### 编译期静态检查

```sh
bern check
```

`bern check` 读取 `Bern.toml`，把 `ui!` 编译为共享 IR，校验多语言翻译完整性（`B102`/`B103`）、自适应功能契约（`B201`–`B204`）与目标能力要求，无需等待完整原生链接。可通过 `--target <名称>` 只检查单个目标。

### 本地运行

```sh
bern run
```

不传 `--target` 时，`bern run` 默认构建并启动当前操作系统的原生目标（macOS 为 SwiftUI，Windows 为 WinUI 3，Linux 为 GTK 4）。传入 `--target` 可直接运行 `Bern.toml` 中声明的任意其他目标：

```sh
bern run --target web_app           # 在浏览器中打开（可用 --port 8080 固定端口保留本地存储）
bern run --target ios               # 安装并启动到正在运行的 iOS 模拟器
bern run --target android           # 安装并启动到 USB 真机或 Android 模拟器
bern run --target gpui_macos        # 使用可选 GPUI 桌面渲染后端运行
```

常用 `bern run` 选项：

- `--release` — 开启优化编译。
- `--port <1-65535>` — 为浏览器目标指定固定本地端口（重启后不丢失浏览器存储）。
- `--trace <RULES>` — 开启性能追踪（如 `--trace engine,ui,slow:16ms`，详见[性能追踪](../Reference/Tracing.md)）。
- `--exclude-module <ids>` — 编译期剔除可选模块（详见[按模块编译](../Topics/Module-Builds.md)）。

### 仅构建不启动

```sh
bern build                          # 构建当前电脑能构建的全部目标
bern build --target macos --release # 以 release 模式构建单个目标
```

构建产物统一位于 `target/bern/<目标>/`，包含原生应用、同级 `bin/` 下的应用 CLI、`features.json`，以及开启 IPC 时生成的 `skills/<应用>/SKILL.md`。

## 4. 打包发布

```sh
bern package                        # 为本机能构建的全部目标打出发布包
bern package --target macos         # 只打包单个目标
```

发布产物输出到 `target/bern/dist/`：

| 平台 | `target/bern/dist/` 中的产物 |
| --- | --- |
| macOS | `<App>-<version>-macos-<arch>.dmg` |
| iOS / watchOS / tvOS | `<App>-<version>-<platform>.ipa` |
| Windows | `<App>-<version>-windows-<arch>-setup.exe` 与免安装 `.zip` |
| Linux | `<app>_<version>_<arch>.deb` 与 `.tar.gz` |
| Android / Wear OS | `.apk` 与上架用 `.aab` |
| Web（`app` / `ui`） | 静态站点 `.zip` / `.tar.gz`（`web_ui` 另含引擎宿主与 CLI） |

代码签名凭据全部通过环境变量传入（不进代码仓库），具体配置见对应平台的[平台操作手册](../Platforms/macOS.md)。

## 下一步

- 深入理解三层如何配合：[协议与纯 Rust 引擎](Protocol-and-Engine.md)。
- 学习界面组织与多端适配：[布局、导航与功能契约](Layouts-and-Navigation.md)。
- 参考仓库中的完整多页示例：[examples/](../../../examples/)（`showcase`、`planner`、`gallery`、`native-controls`、`wearos`、`svg-animation`）。
