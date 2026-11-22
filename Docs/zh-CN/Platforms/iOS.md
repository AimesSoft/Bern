[English](../../Platforms/iOS.md) · **简体中文**

# iOS 与 iPadOS

Bern 在 macOS 上将 iOS 目标（`platform = "ios"`）编译为原生的 **SwiftUI + UIKit** 应用包，同时支持 **iPhone** 与 **iPad**。默认情况下，Bern 会在单个应用包内同时携带双前端：**iOS 17+** 设备运行现代 SwiftUI 前端，**iOS 16** 设备则无缝回退到 iOS 16 前端。

## 环境准备

- **macOS 宿主机**，安装 **Rust 1.99+** 与 **Xcode 26 或更新版本**（需包含 iOS 平台 SDK 与模拟器）。
- 安装 iOS 模拟器与真机的 Rust 编译目标：

```bash
# Apple Silicon Mac（Intel Mac 请将模拟器目标替换为 x86_64-apple-ios）
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
bern doctor
```

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
files_folder = true          # 在 iOS“文件”App 中公开应用 Documents 目录
background_audio = false     # 开启 AVAudioSession 后台音频播放
photos = false               # 保存图片到系统相册时申请相册权限

[app.icon]
default = "assets/icon.png"
ios = "assets/AppIcon.icon"  # 可选：PNG、.appiconset 或 Icon Composer 的 .icon

[targets.ios]
platform = "ios"
ui = "swiftui"
# 默认启用双前端；仅在需要限制最低系统版本或拆分打包时显式指定：
# frontends = ["swiftui", "swiftui-16"]
# packages = "one"           # "one"（默认，单 .ipa）或 "separate"（拆分两个 .ipa）
```

### 双前端策略（`swiftui` + `swiftui-16`）

| `frontends` | 最低系统版本 | 打包方式（`packages`） | 运行行为 |
| --- | --- | --- | --- |
| `["swiftui", "swiftui-16"]` *（默认）* | iOS 16.0 | `"one"`（单个 `.ipa`）或 `"separate"`（`-ios.ipa` 与 `-ios16.ipa`） | 共享 `BernEngine.framework`（Rust 引擎），在 iOS 17+ 通过 `dlopen` 加载 `BernUI17.framework`；iOS 16 回退到内置 iOS 16 前端。 |
| `["swiftui"]` | iOS 17.0 | 单个 `.ipa` | 不包含 iOS 16 回退前端，包体积更小且仅面向 iOS 17+ API。 |

## 构建、运行与调试

```bash
# 为 iOS 模拟器构建
bern build --target ios

# 启动/复用 iOS 模拟器、安装应用并自动打通本地 IPC
bern run --target ios
```

通过 `bern run --target ios` 启动时，Bern 会将 `SIMCTL_CHILD_BERN_IPC_DIR` 透传到模拟器中，宿主机可直接用配套应用 CLI 操控模拟器里的应用：

```bash
target/bern/ios/bin/tasks --engine ios window screenshot --output ios.png
```

常用调试环境变量（若手动通过 `xcrun simctl` 启动，需加上 `SIMCTL_CHILD_` 前缀）：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_FRONT_END=16` | 在 iOS 17+ 模拟器上强制切换为 iOS 16 回退前端，无需额外下载 iOS 16 模拟器运行时即可验证降级表现。 |
| `BERN_APPEARANCE=light\|dark` | 覆盖系统浅色/深色外观。 |
| `BERN_MOTION=reduce\|slow` | 强制减弱动态效果或将弹簧动画放慢 10 倍。 |
| `BERN_TRACE=<rules>` | 开启性能追踪（也可直接用 `bern run --target ios --trace ...`）。 |
| `BERN_SNAPSHOT_DIR=<dir>` | 将窗口截图输出到指定目录。 |

## 平台行为与适配要点

- **iPhone 与 iPad 窗口**：iPhone 上副窗口以虚拟 Sheet 呈现，紧凑导航使用标签栏（在 iOS 26.1+ 上 `TabAccessory` 映射为标签栏上方的系统附属栏）。iPadOS（iOS 17+）支持原生多窗口 `WindowGroup`，并通过 `⌘,` 打开设置窗口。
- **横竖屏锁定**：当播放器等页面需要横屏时，可通过 `Window(landscape: true)` 在手机上切换方向。
- **图标与动态换图标**：`[app.icon].ios` 支持 PNG、`.appiconset` 或 Icon Composer 的 `.icon`（编译为含浅色/深色/着色三种外观的 Liquid Glass `Assets.car`）。`[app.alternate_icons]` 映射为 `CFBundleAlternateIcons`，可在运行时通过 `WindowSet(icon:)` 切换。
- **文件与共享**：`file_pick` 调用系统文件选择器；配置 `files_folder = true` 会写入 `UIFileSharingEnabled` 与 `LSSupportsOpeningDocumentsInPlace`，让应用文档出现在系统“文件”App 中。
- **iOS 上不适用的桌面能力**：`MenuBar`、`StatusItem` 与 `Switch(login_item: true)` 会被安全视作不适用而不报错，方便与桌面端共用同一份 UI 声明。
- **iOS 16 回退差异**：运行在 `swiftui-16` 前端时，滚动吸附（`ScrollView(snap:)`）、`scroll_scale` / `scroll_opacity`、外接键盘 `onKeyPress`、iPad 原生多窗口与 HDR 图像会平稳降级（侧边检查器以 Sheet 打开、iPad 使用虚拟窗口、图像按 SDR 绘制）。
- **原生视图**：可通过 `NativeView(ios: "...")` 挂载与项目同仓的 SwiftUI/UIKit 视图。详见 [原生视图](../Topics/Native-Views.md)。

## 打包与签名

```bash
bern package --target ios
```

`bern package --target ios` 会编译 arm64 真机 Release 包，并在 `target/bern/dist/` 下生成 ad-hoc 签名的 `.ipa`（若配置 `packages = "separate"`，则生成两个 `.ipa`）。

由于安装到真机或上架 TestFlight / App Store 需要 Apple 开发者证书与描述文件（Provisioning Profile），对外分发或安装到真机前请使用团队证书与描述文件对生成的 `.ipa` 重新签名，再通过 Xcode 或 Transporter 提交。
