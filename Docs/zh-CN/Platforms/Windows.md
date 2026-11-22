[English](../../Platforms/Windows.md) · **简体中文**

# Windows

Bern 将 Windows 目标（`platform = "windows"`）编译为原生的 **C++/WinRT** 可执行文件，在进程内链接 Rust 引擎，最低支持 **Windows 10 2004（Build 19041）**，无需捆绑 .NET 或 VC++ 运行库。Windows 目标必须在 Windows 上构建。

默认情况下，Bern 生成**双前端可执行文件**：当用户机器安装了 **Windows App Runtime 2** 时，使用 **WinUI 3**（Fluent 2 与 Mica 背景材质）绘制；未安装时则在启动瞬间无缝回退到系统内置的 **XAML Islands** 前端。

## 环境准备

- **Windows 10/11 宿主机**（x64 或 arm64）。
- **Rust 1.99+** 且使用 MSVC 工具链（`rustup default stable-msvc`）。
- **Visual Studio 2022 或更新版本**，勾选 **“使用 C++ 的桌面开发”** 工作负载（仅安装 Visual Studio Build Tools 即可）。
- **Windows 11 SDK**（包含 `cppwinrt.exe`），可在 Visual Studio Installer 中勾选安装。

构建前先运行环境自检：

```powershell
bern doctor
```

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
publisher = "Example Inc"

[app.icon]
default = "assets/icon.png"
windows = "assets/app.ico"   # 可选：PNG 或多尺寸 .ico

[targets.windows]
platform = "windows"
ui = "winui3"
# 默认启用双前端；仅在需要单一前端时显式指定：
# frontends = ["winui3", "xaml-islands"]
```

### 前端策略（`frontends`）

| `frontends` | 运行时依赖 | 运行行为 |
| --- | --- | --- |
| `["winui3", "xaml-islands"]` *（默认）* | 无（Windows 10 2004+ 开箱即用） | 系统存在 Windows App Runtime 2 时启用 WinUI 3，否则无缝回退至系统内置 XAML Islands。 |
| `["winui3"]` | Windows App Runtime 2 | 体积更小；若目标机器缺失运行时则引导用户安装 Windows App Runtime。 |
| `["xaml-islands"]` | 无（系统内置 Windows 10 2004+） | 始终使用系统 XAML Islands，不链接 WinUI 3 头文件。 |

## 构建、运行与调试

```powershell
# 构建原生程序、配套应用 CLI 与 Skill
bern build --target windows

# 构建并直接启动应用
bern run --target windows
```

**首次构建说明**：首次编译时，Bern 会调用 `cppwinrt.exe` 在 `target/bern-winrt/` 下生成 C++/WinRT 投影头文件并编译预编译头（每个构建配置约占 2.4 GB 磁盘空间）。后续增量构建会直接复用预编译头，通常数秒即可完成。

应用启动后可通过 `target/bern/windows/bin/` 中的配套 CLI 进行控制：

```powershell
target\bern\windows\bin\tasks.exe --help
target\bern\windows\bin\tasks.exe window screenshot --output win.png
```

常用调试环境变量：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_FORCE_ISLANDS=1` | 在已安装 Windows App Runtime 2 的机器上强制走 XAML Islands 回退前端，便于验证兼容性。 |
| `BERN_APP_RUNTIME=installed\|inbox\|none` | 限制启动时探测运行时的来源，用于测试回退路径。 |
| `BERN_APPEARANCE=light\|dark` | 覆盖系统浅色/深色主题。 |
| `BERN_FRAME_RATE=1` | 在所有窗口上显示 WinUI XAML 帧率计数器。 |
| `BERN_TRACE=<rules>` | 开启性能追踪（也可通过 `bern run --trace` 传入）。 |
| `BERN_SNAPSHOT_DIR=<dir>` | 将窗口截图输出到 `<dir>`。 |

## 平台行为与适配要点

- **标题栏与侧边栏**：Fluent 自定义标题栏已展示应用图标（取自 `.ico` 位图）与标题，因此 `SidebarHeader(role: title)` 在 Windows 上会自动省略，避免重复。
- **系统托盘与开机自启**：`StatusItem` 在 Windows 通知区域创建托盘图标与系统弹出菜单，关闭全部窗口后进程继续常驻托盘。`Switch(login_item: true)` 直接管理当前用户的开机启动项。
- **自定义媒体内核**：当引擎注册自定义 `MediaKernel` 时，`Video` 会提供 `SwapChainPanel` 与垂直同步（vblank）帧时钟。
- **网页视图**：在目标中开启 `webview = true` 后，`WebPage` 使用系统 WebView2。
- **行内 Markdown**：`Text(markdown)` 在 Windows 上会去除标记并以纯文本显示。
- **不适用能力**：macOS 专属的顶部 `MenuBar` 会被安全忽略。
- **原生视图**：可通过 `NativeView(windows: "...")` 挂载同仓的 WinUI 3 / C++ 视图。详见 [原生视图](../Topics/Native-Views.md)。

## 打包与签名

```powershell
bern package --target windows
```

`bern package --target windows` 会执行 Release 构建，并在 `target\bern\dist\` 下生成两份分发包：

- `<app>-<version>-<arch>-setup.exe` — 用户级 Direct2D + LZMS 安装包（无需管理员权限或 UAC 弹窗；支持 `/S` 静默安装与静默卸载）。
- `<app>-<version>-<arch>.zip` — 绿色便携压缩包，内含主程序、配套 CLI 与 Skill。

未签名时 Windows SmartScreen 会对新程序弹出警告。可通过配置用户证书存储区的 SHA-1 指纹或 `.pfx` 证书路径，让 Bern 自动对主程序与安装包执行带时间戳的 SHA-256 签名：

```powershell
# 使用当前用户证书存储区中已安装的证书
$env:BERN_WINDOWS_CERTIFICATE = "0123456789ABCDEF..."
bern package --target windows

# 或使用 .pfx 证书文件
$env:BERN_WINDOWS_CERTIFICATE = "C:\certs\release.pfx"
$env:BERN_WINDOWS_CERTIFICATE_PASSWORD = "your-password"
bern package --target windows
```
