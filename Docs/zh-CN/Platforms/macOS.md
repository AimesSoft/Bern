[English](../../Platforms/macOS.md) · **简体中文**

# macOS

Bern 将 macOS 目标（`platform = "macos"`）编译为原生的 **SwiftUI + AppKit** `.app` 应用包，最低支持 **macOS 14**，并将 Rust 引擎作为静态库链接进同一进程。macOS 目标必须在 Mac 上构建。

## 环境准备

- **Rust 1.99+**（`rustup update`）。
- 从 App Store 安装 **Xcode 26 或更新版本**（macOS SDK 26.0+），并将激活的开发者目录指向 Xcode：

```bash
sudo xcode-select --switch /Applications/Xcode.app
bern doctor
```

`bern doctor` 会检查当前激活的 macOS SDK 与 `swiftc` 版本。

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[app.icon]
default = "assets/icon.png"
macos = "assets/AppIcon.icon"   # 可选：PNG、.icns 或 Icon Composer 的 .icon

[targets.macos]
platform = "macos"
ui = "swiftui"
```

常用的 macOS 相关配置项：

| 配置项 | 用途 |
| --- | --- |
| `[app.icon].macos` | 支持正方形 PNG、`.icns` 或 Icon Composer 的 `.icon` 包（经由 `actool` 编译为 macOS 26+ 的 Liquid Glass `Assets.car`，并自动生成适配 macOS 14–15 的 `.icns`）。 |
| `[app.alternate_icons]` | 声明可在运行时通过 `WindowSet(icon:)` 动态切换的 Dock 图标。 |
| `[[app.opens]]` | 在 `Info.plist` 中注册可打开的文件类型，使访达“打开方式”或拖放到 Dock 图标时将文件传入引擎。 |
| `webview = true` | 启用 `WebPage`（`WKWebView`）。默认关闭，避免未用到时链接 WebKit。 |
| `javascript = true` | 启用进程内 JavaScript（`bern::script`），不启动隐藏 WebView。详见 [JavaScript 与 `bern.cli`](../Topics/JavaScript.md)。 |
| `web_ui = "<target>"` | 将一个 `web_mode = "ui"` 目标打包进应用资源目录（`__bern_web_ui/`），运行时通过 `bern::web_ui::set(...)` 提供局域网网页端。 |

## 构建、运行与调试

```bash
# 构建 .app 应用包、配套应用 CLI 与 Skill
bern build --target macos

# 构建并直接启动应用
bern run --target macos

# 启动并开启性能追踪
bern run --target macos --trace engine,ui,slow:16ms
```

构建产物位于 `target/bern/macos/`：

- `<App>.app` — 原生应用包（主程序在 `Contents/MacOS/<app>`，配套 CLI 在 `Contents/Helpers/<app>`，AI Skill 在 `Contents/Resources/skills/<app>/SKILL.md`）。
- `<App>.app.dSYM` — 位于应用包旁的调试符号。
- `bin/<app>` — 指向配套应用 CLI 的符号链接，应用启动后可直接调用：

```bash
target/bern/macos/bin/tasks --help
target/bern/macos/bin/tasks window screenshot --output shot.png
```

常用调试环境变量：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | 覆盖系统浅色/深色外观以方便测试。 |
| `BERN_MOTION=reduce\|slow` | 强制减弱动态效果，或将弹簧动画放慢 10 倍以逐帧检查。 |
| `BERN_TRACE=<rules>` | 开启结构化性能追踪（规则同 `bern run --trace`）。详见 [性能追踪](../Reference/Tracing.md)。 |
| `BERN_SNAPSHOT_DIR=<dir>` | 自动输出窗口截图，用于布局核对。 |
| `BERN_RENDER=active` | 即使窗口未聚焦也按激活态绘制控件样式。 |
| `BERN_IPC_DIR=<dir>` | 指定应用与配套 CLI 共用的本地 IPC 套接字目录。 |

## 平台行为与适配要点

- **窗口与场景**：完整支持原生多窗口（`WindowGroup`）、设置窗口（`⌘,`）、`Dialog` 表单与 `Modal` 浮层。
- **菜单栏与托盘**：`MenuBar`、`CommandMenu` 与 `CommandItem` 映射为系统原生菜单栏。`StatusItem` 使用 `MenuBarExtra` 提供顶部状态栏图标与菜单；存在 `StatusItem` 时关闭最后一个窗口不会退出应用，而是驻留状态栏并隐藏 Dock 图标。`Switch(login_item: true)` 直接接入 macOS 登录项。
- **原生视图**：可通过 `NativeView(macos: "...")` 挂载与项目同仓的 SwiftUI/AppKit 视图。详见 [原生视图](../Topics/Native-Views.md)。
- **自定义媒体内核**：若引擎注册了自定义 `MediaKernel`，`Video` 会直接提供 `CAMetalLayer` 与 `CADisplayLink` 帧时钟。
- **文件与剪贴板**：原生支持 `NSOpenPanel`/`NSSavePanel` 文件选择（`file_pick`）、剪贴板粘贴（`paste:`）与访达文件关联（`[[app.opens]]`）。

## 打包与签名

```bash
bern package --target macos
```

`bern package` 会编译 Release 版 `.app`，并在 `target/bern/dist/` 下生成 LZMA 压缩（`ULMO`）的 `.dmg` 磁盘映像，内置拖入 Applications 的访达窗口布局（无需图形界面的 Finder 会话即可生成）。

未配置签名环境变量时，产物仅做本机 ad-hoc 签名，其他 Mac 的 Gatekeeper 会拒绝打开。对外分发请设置以下环境变量以完成签名与公证：

```bash
export BERN_MACOS_SIGNING_IDENTITY="Developer ID Application: Example Inc (TEAMID)"
export BERN_MACOS_NOTARY_PROFILE="bern-notary"   # 通过 `xcrun notarytool store-credentials` 创建
bern package --target macos
```

`BERN_MACOS_NOTARY_PROFILE` 依赖 `BERN_MACOS_SIGNING_IDENTITY`。两者同时设置时，Bern 会自动启用 Hardened Runtime 对应用包及 CLI 助手签名、构建并签名 `.dmg`、提交 Apple 公证服务并自动装订公证票据（staple）。
