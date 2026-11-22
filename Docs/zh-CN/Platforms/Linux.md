[English](../../Platforms/Linux.md) · **简体中文**

# Linux

Bern 将 Linux 目标（`platform = "linux"`）编译为原生的 **GTK 4 + libadwaita** Rust 可执行文件，并在进程内链接引擎，面向提供 **GTK 4.14+** 与 **libadwaita 1.5+** 的发行版（如 Ubuntu 24.04+、Fedora 40+、Arch、SteamOS 等）。Linux 目标必须在 Linux 上构建。

## 环境准备

- **Rust 1.99+**（`rustup update`）。
- **pkg-config**、**GTK 4.14+**、**libadwaita 1.5+** 与 **GStreamer**（用于媒体播放）：

```bash
# Debian / Ubuntu 24.04+
sudo apt install pkg-config libgtk-4-dev libadwaita-1-dev gstreamer1.0-libav

# Fedora 40+
sudo dnf install pkgconf-pkg-config gtk4-devel libadwaita-devel
```

如果目标开启了 `webview = true`，还需安装 WebKitGTK 6 开发包（例如 Ubuntu/Debian 上的 `libwebkitgtk-6.0-dev` 或 Fedora 上的 `webkitgtk6.0-devel`）。

构建前先运行环境自检：

```bash
bern doctor
```

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
publisher = "Example Inc"   # 写入 .deb 的 Maintainer 元数据

[app.icon]
default = "assets/icon.png"
linux = "assets/icons/hicolor"   # 可选：正方形 PNG 或 hicolor 图标主题目录

[targets.linux]
platform = "linux"
ui = "gtk"
```

## 构建、运行与调试

```bash
# 构建原生程序、.desktop 入口、配套应用 CLI 与 Skill
bern build --target linux

# 构建并启动应用
bern run --target linux
```

构建产物位于 `target/bern/linux/<Product>/`：

- `<app>` — 原生 GTK 4 应用主程序。
- `resources/` — 资源文件、翻译及可选的 `__bern_web_ui/`。
- `<bundle_id>.desktop` — freedesktop 桌面启动器文件。
- `bin/<app>` — 配套应用 CLI。
- `skills/<app>/SKILL.md` — 配套 AI Skill。

应用运行后可通过配套 CLI 操控：

```bash
target/bern/linux/Tasks/bin/tasks --help
target/bern/linux/Tasks/bin/tasks window screenshot --output linux.png
```

常用调试环境变量：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | 覆盖系统浅色/深色主题。 |
| `BERN_MOTION=reduce` | 关闭弹簧动画并强制减弱动态效果。 |
| `BERN_MOTION_LOG=1` | 向标准错误输出元素的动效变换日志。 |
| `BERN_TRACE=<rules>` | 开启性能追踪（也可通过 `bern run --trace` 传入）。 |
| `BERN_SNAPSHOT_DIR=<dir>` | 自动将窗口截图保存到 `<dir>`。 |

## 平台行为与适配要点

- **顶栏与侧边栏**：由于 libadwaita 的侧边栏顶栏已展示应用名称（KDE 环境下也会在左端显示应用图标），`SidebarHeader(role: title)` 在 Linux 上会自动省略。
- **系统托盘与开机自启**：`StatusItem` 通过 **StatusNotifierItem** 与 **dbusmenu** 注册托盘图标与菜单（兼容带 AppIndicator 的 GNOME、KDE Plasma 与 Wayland 顶栏），关闭全部窗口后进程保持常驻。`Switch(login_item: true)` 管理 XDG autostart `.desktop` 条目。
- **自定义媒体内核**：当引擎安装自定义 `MediaKernel` 时，`Video` 会提供 `FrameSink` 与 GTK 帧时钟。
- **WebPage 与 Ubuntu 24.04+ AppArmor**：开启 `webview = true` 时，`WebPage` 使用 WebKitGTK 6。在 Ubuntu 24.04+ 上，AppArmor 默认限制未配置 profile 的程序创建非特权用户命名空间。`bern package` 生成的 `.deb` 会自动安装所需的 AppArmor profile；若本地运行开发构建或解压 `.tar.gz`，请手动安装对应 profile，或在本地开发测试时临时设置 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1`。
- **文件关联与图标**：`[[app.opens]]` 会向 `.desktop` 写入 `MimeType=` 条目；`[app.icon].linux` 支持 PNG 或完整的 `hicolor` 图标主题目录。
- **原生视图**：可通过 `NativeView(linux: "...")` 挂载同仓的 GTK 4 控件。详见 [原生视图](../Topics/Native-Views.md)。

## 打包与签名

```bash
bern package --target linux
```

`bern package --target linux` 会构建 Release 程序，并在 `target/bern/dist/` 下生成两份分发包：

- `<app>_<version>_<arch>.deb` — Debian/Ubuntu 安装包，安装至 `/opt/<bundle_id>`，将命令软链到 `/usr/bin/<app>`，并自动安装 `.desktop`、hicolor 多尺寸图标以及（开启 `webview = true` 时的）WebKitGTK AppArmor profile。
- `<app>-<version>-linux-<arch>.tar.gz` — 绿色便携归档包，内含主程序、资源、`.desktop`、配套 CLI 与 Skill。
