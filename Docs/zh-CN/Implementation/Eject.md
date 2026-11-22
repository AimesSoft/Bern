[English](../../Implementation/Eject.md) · **简体中文**

# 导出为独立纯原生项目（`bern eject`）

更新：2026-11-22。`bern eject` 将 Bern 应用编译导出为标准的各平台原生 IDE 源代码项目，不再依赖 `bern-cli`、Bern 编译器或原始工作区。它作为选型 Bern 时的“无锁定保险”：你可以用一份 Rust 声明快速启动多端开发，并在任意时刻导出干净的原生工程，直接用 Xcode、Android Studio、Visual Studio、Cargo 或静态 HTTP 服务打开、修改、签名、扩展或交接。

原 Bern 项目本身不会被修改。导出产物默认写入 `ejected/<target>`（也可用 `--out <dir>` 指定路径）。

## 用法

```sh
# 导出 Bern.toml 中的全部目标，并附带完全离线自包含的 Rust 后端源码（任意宿主机可跨平台导出）
bern eject

# 将单个目标导出到指定目录
bern eject --target macos --out ./native-macos

# 预编译 Rust 引擎二进制并嵌入原生工程，目标机器无需安装 Rust 即可直接构建
bern eject --target ios --engine prebuilt --release --out ./native-ios

# 保留应用 CLI、IPC 控制通道与 SKILL.md
bern eject --target macos --with-cli

# 覆盖非空的目标目录
bern eject --target windows --out ./native-win --force
```

### 选项说明

| 选项 | 默认值 | 说明 |
| --- | --- | --- |
| `--manifest PATH` | 向上查找最近的 `Bern.toml` | 指定要导出的应用配置 |
| `--target NAME` | 全部目标 | 指定 `[targets.<name>]` 中的目标名 |
| `--out DIR` | `<app>/ejected/<target>` | 输出目录（导出多目标时写入 `<DIR>/<target>`） |
| `--engine source\|prebuilt` | `source` | `source` 附带离线自包含 Rust 工作区并与原生 IDE 构建联动；`prebuilt` 预编译并内嵌引擎二进制 |
| `--release` | Debug | 在 `--engine prebuilt` 模式下编译 release 优化版引擎二进制 |
| `--with-cli` | 关闭 | 保留 IPC 控制通道、应用 CLI 源码与 `SKILL.md` |
| `--exclude-module LIST` | 无 | 按模块排除编译，语义与 `bern build --exclude-module` 一致 |
| `--force` | 关闭 | 允许覆盖已存在的非空输出目录 |

## 两种引擎交付模式

### 1. 离线自包含 Rust 源码模式（`--engine source`，默认）

导出器会沿着引擎依赖图收集应用自身的所有本地 crate（`protocol`、`engine`、已启用的 `modules/*`、工作区内辅助 crate）以及该目标实际用到的 Bern 运行时 crate 子集，全部打包进导出目录。所有 `workspace = true` 继承会被展开，所有本地路径重写为相对路径，一并携带 `Cargo.lock` 与 `rust-toolchain.toml`，并清理外部 `dev-dependencies`。

原生 IDE 构建系统会自动驱动 `cargo` 编译引擎：

- **Apple（`.xcodeproj`）**：内置 **Build Rust Engine** Run Script 构建阶段，自动执行 `scripts/build-rust.sh`，按 Xcode 当前选中的 SDK、架构与 Debug/Release 配置编译并链接。
- **Android / Wear OS（Gradle）**：`app/build.gradle.kts` 在 `merge*JniLibFolders` 前挂接 `buildRustEngineDebug` / `buildRustEngineRelease` 任务，自动使用 Android Studio 配置的 NDK 编译 `.so`。
- **Windows（Visual Studio `.sln` + `.vcxproj`）**：MSBuild 在链接前自动执行 `scripts\build-rust.cmd`，匹配当前 `x64`/`ARM64` 平台与 `Debug`/`Release` 配置。
- **Linux（GTK 4）与桌面端（GPUI）**：导出目录本身就是独立的自包含 Cargo 工作区（直接 `cargo run`）。
- **Web（DOM + WASM）**：附带 `scripts/build-wasm.sh` 与 `scripts/build-wasm.cmd`，一键编译 `rust/page` 生成 `app.wasm`。

因为 `--engine source` 在导出阶段不需要调用目标平台的本地编译器，任意操作系统都可以跨平台导出全部平台的原生源码项目（例如在 macOS 上一次导出 Windows、Linux、Android、Apple 与 Web 项目）。

### 2. 预编译引擎二进制模式（`--engine prebuilt`）

对非 Rust 前端目标（Apple SwiftUI、Android / Wear OS Compose、Windows WinUI 3 / Islands、Web），`--engine prebuilt` 会在导出时编译 Rust 引擎，将产物二进制直接集成到原生工程，并清理所有临时 Rust 中间文件。导出后的项目可在**完全未安装 Rust 工具链**的机器上直接用 Xcode、Android Studio、Visual Studio 或浏览器打开构建。

前端本身使用 Rust 编写的目标（`gtk + linux` 与 `gpui + *`）无论引擎形态都需要 Cargo，因此仅支持 `--engine source`。

## 各平台导出目录结构

生成的界面代码按职责拆分为清晰的多文件结构，并移除 `Do not edit` 提示，让每个组件和窗口各自独立成文件，同时与 Bern 运行时保持 100% 行为一致。紧邻的 `NativeView` 原生文件与声明资源也会直接拷入导出项目，不残留任何指向原工作区的外部路径。

| 平台 | 原生工程入口 | 拆分后的 UI 结构 | 引擎集成（`source` / `prebuilt`） |
| --- | --- | --- | --- |
| **macOS / iOS / tvOS / watchOS** | `<Product>.xcodeproj` + 共享 `.xcscheme` | `Sources/{App,Models,Styles,Views,Windows,Native,Runtime}/` | `rust/` + `scripts/build-rust.sh` / `Frameworks/BernEngine.xcframework` |
| **Android / Wear OS** | 根目录 Gradle 工程（`settings.gradle.kts`、`app/build.gradle.kts`、`gradle-wrapper.properties`） | `app/src/main/kotlin/<pkg>/{MainActivity,Models,Styles,AppState}.kt`、`views/`、`windows/`、`dev/bern/runtime/` | `rust/` + Gradle NDK 任务 / `app/src/main/jniLibs/<abi>/libbern_app.so` |
| **Windows（WinUI 3 / Islands）** | `<Product>.sln` + `<Product>.vcxproj` + `.vcxproj.filters` | `src/catalog/`、`src/ui/{app_ui,styles}.inl`、`src/ui/{views,windows}/`、`src/native/`、`src/runtime/` | `rust/` + `scripts\build-rust.cmd` / `lib\<arch>\bern_app.lib` |
| **Linux（GTK 4）** | 独立 `Cargo.toml` | `src/main.rs`、`src/ui/{mod,styles,strings}.rs`、`src/ui/{views,windows,native}/` | 自包含 Cargo 工作区（仅 `source`） |
| **桌面端（GPUI）** | 独立 `Cargo.toml`（含 vendored GPUI 补丁） | `src/main.rs`、`src/ui/{mod,styles,strings}.rs`、`src/ui/{views,windows,native}/` | 自包含 Cargo 工作区（仅 `source`） |
| **Web（DOM + WASM）** | 零打包器 `index.html` + 浏览器原生 ES 模块 | `src/{app,models,styles}.js`、`src/{views,windows,native}/`、`runtime/`、`styles/` | `rust/` + `scripts/build-wasm.{sh,cmd}` / `app.wasm` |

## 精简默认配置与协议演进

- **默认剥离开发期脚手架**：未传 `--with-cli` 时，`bern eject` 会剥离 IPC 控制通道、应用 CLI 可执行目标、`SKILL.md`、`build.json` 与 `features.json`，产出干净的原生工程。
- **放宽协议指纹强校验**：`bern_ffi::start_app` 约定传入 `fingerprint == 0` 时跳过目录哈希强校验。导出项目统一传入 `0`，方便在脱离 Bern 后直接修改 Rust 后端协议（新增字段或命令）而不会在启动时被目录不匹配拦截。
- **Windows SDK 准备脚本**：`scripts\prepare-sdk.cmd` 仅调用 Windows 自带的 `curl.exe`、`tar.exe` 与 `cppwinrt.exe`（不使用 PowerShell），在 Visual Studio 首次构建时自动准备 `.sdk-cache\`。

## 验证范围

- **工作区单元测试与集成检查（2026-11-22 已验证）**：`bern-ffi`、`bern-target-api`、全部六个后端 crate（`bern-backend-swiftui`、`bern-backend-compose`、`bern-backend-winui3`、`bern-backend-gtk`、`bern-backend-gpui`、`bern-backend-web`）、`bern-cli` 与 `bern --test docs` 全部通过（覆盖 `fmt`、`clippy -D warnings`、离线依赖打包、零外部绝对路径、零 `Do not edit` 残留、Windows 零 PowerShell、拆分后 Swift/Kotlin 跨目录文件名唯一性）。
- **macOS Xcode 实机构建（2026-11-22 已验证）**：使用真实 `xcodebuild` 端到端编出可运行的 `.app` 包，覆盖 `examples/hello`（`--engine source` 与移除 Rust/Cargo 的极简 `PATH` 下的 `--engine prebuilt`）与 `examples/showcase`（`--engine source`，含多组件拆分、`media/` 与 `images/` 嵌套目录 Folder Reference 以及多语言表）。
- **尚未在对应宿主机实测**：iOS/tvOS/watchOS 模拟器 Xcode 构建、Android Studio / Gradle 构建、Windows Visual Studio 2022 构建、Linux GTK 4 与 GPUI 独立 Cargo 构建，以及浏览器实时伺服运行。
