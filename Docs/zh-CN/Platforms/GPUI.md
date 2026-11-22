[English](../../Platforms/GPUI.md) · **简体中文**

# GPUI（可选统一 GPU 桌面端）

Bern 提供可选的 **GPUI** 后端（`ui = "gpui"`），使用 Bern 自身的主题算法（`BernTheme`）在 **macOS、Windows 与 Linux** 上以 GPU 加速绘制统一的桌面界面。它不是任何平台的默认后端——当你希望三端桌面呈现完全一致的视觉风格（或与系统原生后端并存）时，可在 `Bern.toml` 中显式启用。

## 环境准备

- **Rust 1.99+**（`rustup update`）。
- 目标桌面宿主机的 GPU 与窗口构建依赖：
  - **macOS**：标准 Xcode Command Line Tools 即可（内置 `runtime_shaders` 运行时编译着色器，无需额外安装 Metal 编译器工具链）。
  - **Windows**：Visual Studio C++ 构建工具、Windows 11 SDK（含 `rc.exe` / `fxc.exe`）与 Rust `stable-msvc`。
  - **Linux**：X11/Wayland、字体与图形开发头文件，以及 Vulkan / OpenGL ES 驱动（或 Mesa 软件渲染）：

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

`bern-gpui` 在进程内链接同一份 Rust 引擎并暴露相同的本地 IPC 通道，因此配套应用 CLI 与 Skill 的用法与原生桌面目标完全一致：

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

- **统一视觉体系**：在 macOS、Windows 与 Linux 上均采用 `BernTheme`（高对比度中性色板、统一间距、圆角与弹簧物理曲线）。
- **macOS 菜单栏**：当 `platform = "macos"` 时，GPUI 目标同样声明并呈现系统原生的 macOS `MenuBar`。
- **标准控件与动效**：支持分栏/标签导航、表单、表格、Sheet/Modal、搜索命令面板、自动骨架屏、纯 Rust 动态 `Svg`（`BSVG`）与进程内 `javascript = true`。
- **原生视图**：可通过 `NativeView(gpui: "...")` 直接嵌入同仓 GPUI 视图（同时保留 `Fallback` 给其他目标）。详见 [原生视图](../Topics/Native-Views.md)。
- **编译期能力校验**：若声明用到了 `bern-backend-gpui` 尚未声明的 IR 能力，`bern check --target <gpui_target>` 会在编译前直接报错并定位到具体声明。

## 打包与签名

GPUI 目标暂未接入 `bern package` 安装包制作。发布二进制请执行：

```bash
bern build --target gpui_macos --release
```

然后将 `target/bern/<target>/` 下生成的可执行文件连同 `resources/`、`bin/` 与 `skills/` 目录一并分发。
