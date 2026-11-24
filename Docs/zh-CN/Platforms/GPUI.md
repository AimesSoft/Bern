[English](../../Platforms/GPUI.md) · **简体中文**

# GPUI（可选统一 GPU 桌面端）

Bern 提供可选的 **GPUI** 后端（`ui = "gpui"`），使用 Bern 自身的主题算法（`BernTheme`）在 **macOS、Windows 与 Linux** 上以 GPU 加速绘制统一的桌面界面。它不是任何平台的默认后端——当你希望三端桌面呈现完全一致的视觉风格（或与系统原生后端并存）时，可在 `Bern.toml` 中显式启用。

## 环境准备

- **Rust 1.99+**（`rustup update`）。
- 目标桌面宿主机的 GPU 与窗口构建依赖：
  - **macOS**：标准 Xcode Command Line Tools 即可（内置 `runtime_shaders` 运行时编译着色器，无需额外安装 Metal 编译器工具链）；`WebKit`、`AVFoundation`、`CoreMedia` 与 `ServiceManagement` 均为系统内置框架。
  - **Windows**：Visual Studio C++ 构建工具、Windows 11 SDK（含 `rc.exe` / `fxc.exe`）与 Rust `stable-msvc`。`WebPage` 复用系统已安装的 Microsoft Edge WebView2 Runtime，内置视频播放使用 Windows Media Foundation（`IMFMediaEngine`）。
  - **Linux**：X11/Wayland、字体与图形开发头文件，以及 Vulkan / OpenGL ES 驱动（或 Mesa 软件渲染）。运行时 `WebPage` 动态加载 WebKitGTK（`libwebkit2gtk-4.1.so.0` + `libgtk-3.so.0`），内置媒体播放动态加载 GStreamer（`libgstreamer-1.0.so.0` + `libgstapp-1.0.so.0`），未使用时零链接期依赖：

```bash
# Debian / Ubuntu
sudo apt install pkg-config libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev
```

## `Bern.toml` 配置

按桌面操作系统分别配置 `ui = "gpui"` 目标：

```toml
[targets.gpui_macos]
platform = "macos"
ui = "gpui"

[targets.gpui_windows]
platform = "windows"
ui = "gpui"

[targets.gpui_linux]
platform = "linux"
ui = "gpui"
```

GPUI 目标可以与默认的系统原生目标（`swiftui`、`winui3`、`gtk`）共存于同一份 `Bern.toml`，构建和运行时用 `--target` 指定即可。

## 构建、运行与调试

```bash
# 检查 GPUI 目标的能力覆盖情况
bern check --target gpui_macos

# 构建 GPUI 程序、配套应用 CLI 与 Skill
bern build --target gpui_macos

# 构建并启动 GPUI 应用
bern run --target gpui_macos
```

`bern-gpui` 在进程内链接同一份 Rust 引擎并暴露相同的本地 IPC 通道，因此配套应用 CLI、全部 UI 会话命令（`back`、`focus`、`set_file`、`window.*`、关闭确认协议）、`BERN_TRACE` 界面线程追踪打点与 Skill 的用法与原生桌面目标完全一致：

```bash
target/bern/gpui_macos/bin/tasks --help
target/bern/gpui_macos/bin/tasks window screenshot --output gpui.png
```

常用调试环境变量：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_WINDOW_OFFSCREEN=1` | 将新打开的 GPUI 窗口移到屏幕外，便于在正在使用的电脑上执行自动化 UI/CLI 检查而不抢占焦点。 |
| `BERN_APPEARANCE=light\|dark` | 覆盖系统浅色/深色外观。 |
| `BERN_MOTION=reduce\|slow` | 关闭弹簧动画或放慢 10 倍以逐帧检查。 |
| `BERN_MOTION_LOG=1` | 向标准错误输出当前活跃的弹簧动画日志。 |
| `BERN_MEDIA_LOG=1` | 输出视频帧向内核传递的诊断信息。 |
| `BERN_TRACE=<rules>` | 开启性能追踪（也可使用 `bern run --trace ...`）。 |

## 平台行为与适配要点

- **完整桌面 IR 能力对齐**：覆盖桌面端全部组件、特性、样式与动效能力——包括 `Table`/`TableColumn`/`TableMenu`、`List`、`Inspector`、`Marquee`、`WindowButtons`、`Slot`、`Sidebar(fallback:)`、`Drop(opens:)`、`WindowRole::Settings` 与 `WindowRole::Dialog`（含模态阻塞）、`Window(confirm_close:)`、`WindowSet(palette:)`、`Text(lit*)`、`TextInput(current:)`、`paste:`、`Rail(overhang:)`、`idle_area`、`glow_opacity`、`Stack` 边框与悬停态、纯 Rust 动态 `Svg`（`BSVG`）以及进程内 `javascript = true`。
- **统一视觉体系**：在 macOS、Windows 与 Linux 上均采用 `BernTheme`（高对比度中性色板、运行时语义调色板覆盖、统一间距、圆角与弹簧物理曲线）。
- **原生系统集成**：
  - **菜单栏**：`platform = "macos"` 时原生呈现 macOS `MenuBar`。
  - **系统托盘（`StatusItem`）**：三端均提供原生托盘图标与动态菜单（macOS `NSStatusBar`、Windows `Shell_NotifyIconW`、Linux D-Bus `StatusNotifierItem` + `com.canonical.dbusmenu`），支持关窗常驻与 `--at-login` 静默启动。
  - **开机自启（`Switch(login_item: true)`）**：macOS `SMAppService`（非 bundle 场景回退 LaunchAgent）、Windows `HKCU\...\Run` 注册表与 Linux XDG Autostart `.desktop`。
- **原生内嵌 `WebPage`**：三端使用系统原生 WebView（macOS `WKWebView`、Windows `WebView2`、Linux `WebKitGTK`），支持 `cookies_file`、`cookie_domain`、`user_agent` 与 `close_on`，并在 macOS 上支持 WebKit 原生 `content_mode`（Windows 与 Linux 与 WinUI 3 / GTK 一致，保留引擎默认模式并在构建时提示警告）。
- **媒体播放（`Video`）**：三端内置原生媒体执行器（macOS `AVFoundation`、Windows `IMFMediaEngine`、Linux `GStreamer`）并提供 GPUI 标准 `Video(controls:)` 播放控制栏；若应用注册了自定义 `MediaKernel`，仍会自动优先使用应用内核。
- **原生视图**：可通过 `NativeView(gpui: "...")` 直接嵌入同仓 GPUI 视图（同时保留 `Fallback` 给其他目标）。详见 [原生视图](../Topics/Native-Views.md)。

## 打包与签名

`bern package` 已支持为全部三个 GPUI 桌面目标生成可分发安装包，输出到 `target/bern/dist/`：

```bash
# macOS：签名（并支持公证）的 .dmg 磁盘镜像
bern package --target gpui_macos

# Windows：便携 .zip（设置 BERN_WINDOWS_CERTIFICATE 时经 signtool 签名）
bern package --target gpui_windows

# Linux：便携 .tar.gz 及 .deb 安装包（宿主机存在 dpkg-deb 时自动生成）
bern package --target gpui_linux
```

签名与公证环境变量（`BERN_MACOS_SIGNING_IDENTITY`、`BERN_MACOS_NOTARY_PROFILE`、`BERN_WINDOWS_CERTIFICATE`、`BERN_WINDOWS_PASSWORD`、`BERN_WINDOWS_TIMESTAMP`）与系统原生桌面目标一致。
