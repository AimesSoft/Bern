[English](../../Platforms/tvOS.md) · **简体中文**

# tvOS

Bern 将 tvOS 目标（`platform = "tvos"`）编译为原生的 **SwiftUI** Apple TV 应用，最低支持 **tvOS 18**。tvOS 目标必须在 Mac 上构建。

## 环境准备

- **macOS 宿主机**，安装 **Rust 1.99+** 与 **Xcode 26 或更新版本**（需包含 tvOS 平台 SDK 与 Apple TV 模拟器）。
- 安装 tvOS 模拟器与真机的 Rust 编译目标：

```bash
# Apple Silicon Mac（Intel Mac 请将模拟器目标替换为 x86_64-apple-tvos）
rustup target add aarch64-apple-tvos-sim aarch64-apple-tvos
bern doctor
```

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[app.icon]
default = "assets/icon.png"
tvos = "assets/AppIcon.brandassets"  # 可选：.brandassets 或正方形 PNG

[targets.tvos]
platform = "tvos"
ui = "swiftui"
```

当未单独指定 `[app.icon].tvos`（或指向正方形 PNG）时，Bern 会自动从默认图标合成 tvOS 主屏分层图标与 Top Shelf 横幅图。

## 构建、运行与调试

```bash
# 检查 tvOS 声明
bern check --target tvos

# 为 tvOS 模拟器构建
bern build --target tvos

# 启动 Apple TV 模拟器运行应用并打通 IPC
bern run --target tvos
```

通过配套应用 CLI 配合 `--engine tvos` 操控模拟器中的应用：

```bash
target/bern/tvos/bin/tasks --engine tvos window screenshot --output tvos.png
```

常用调试环境变量：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | 覆盖外观模式以方便检查。 |
| `BERN_MOTION=reduce\|slow` | 强制减弱动态效果或将弹簧动画放慢 10 倍。 |
| `BERN_TRACE=<rules>` | 开启性能追踪。 |
| `BERN_SNAPSHOT_DIR=<dir>` | 自动保存窗口截图。 |

## 平台行为与适配要点

- **编译期 TV 特化**：`window.tv` 在 tvOS 上恒为 `true`。Bern 在编译期仅保留 `Adaptive` 中的 `Layout(size: tv)` 分支，自动应用 `style!` 中的 `font(tv:)` 电视字号，并裁剪桌面/手机/手表分支。
- **Siri Remote 与焦点引擎**：
  - 在页面或弹窗的首选 `Button` / `IconButton` 上标记 `default_focus: true`，确保切换画面时焦点稳定落位。
  - `Shortcut` 无需可见控件即可绑定遥控器按键：`escape` 对应返回/菜单键，`space` 对应播放/暂停键，方向键在焦点无法继续移动时触发。
  - 在隐藏控制条的全屏播放页面中，可利用全屏焦点承接方向键作为快进/快退快捷键。
- **窗口与浮层**：副窗口以 `fullScreenCover` 打开；`Popover` 转为 Sheet；`Table` 降级为单列可聚焦按钮列表。
- **媒体与 SVG**：自定义 `MediaKernel` 会获得 `CAMetalLayer` 与 `CADisplayLink` 帧时钟。动态 `Svg` 编译为纯 Rust `BSVG` 字节码运行（不依赖 WebKit）；静态 SVG 编译期栅格化为 PNG。
- **不可用能力**：`file_pick`、`paste:`、`WebPage`、`MenuBar`、`StatusItem`、`wheel`、`slider_hover`、`scroll_anchor`、`scroll_browsing`、`bound_blur` 与运行时切换图标在 tvOS 上不可用。将这些键鼠专属 UI 留在非 `tv` 分支即可无警告通过 `bern check --target tvos`。
- **原生视图**：可通过 `NativeView(tvos: "...")` 挂载同仓的 SwiftUI/UIKit tvOS 视图。详见 [原生视图](../Topics/Native-Views.md)。

## 打包与签名

```bash
bern package --target tvos
```

`bern package --target tvos` 会编译 arm64 真机 Release 包并在 `target/bern/dist/` 下生成 ad-hoc `.ipa`。安装到 Apple TV 真机或提交 App Store Connect 前，请使用 Apple 发布证书与 tvOS 描述文件重新签名。
