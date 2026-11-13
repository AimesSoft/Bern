[English](../../Implementation/watchOS-SwiftUI.md) · **简体中文**

# watchOS SwiftUI 实现说明

当前组件接口见[组件参考](../Reference/UI.md)，逐项支持与近期验证范围见[平台能力表](../Reference/Platforms.md)，Apple 平台官方依据见 [Apple 适配规范](../Adaptation/Apple.md)。

- 日期：2026-11-14
- 依据：[Bern 架构设计](../Design.md)
- 工具链：Rust 1.99.0（`aarch64-apple-watchos-sim`、`aarch64-apple-watchos`）；Xcode 27.2，watchOS 27.2 模拟器与真机 SDK
- 验证环境：Apple Watch Series 12 (46mm) 模拟器，watchOS 27.2

本文记录 `swiftui + watchos` 目标（`Platform::Watchos`）已实现和已验证的内容。它与 macOS、iOS、tvOS 共用 SwiftUI 生成器与运行时，平台层由 `BernWatchKit.swift` 以 WatchKit 与 watchOS 原生 SwiftUI 控件实现，并与 `compose-wearos` 共用 `Layout(size: watch)` 和 `window.watch`。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 平台解析 | 完成 | `platform = "watchos"` 默认对应 `ui = "swiftui"`；`bern doctor`、`bern check`、`bern build`、`bern run` 与 `bern package` 均识别该目标 |
| 显式手表布局与功能契约 | 完成 | 所有可达的 `Adaptive` 必须声明 `Layout(size: watch)`（`B401`）；`Application::on_watch` 在检查前裁剪非手表分支，`#[bern::features]` 单独证明 `watch` 覆盖 |
| 生成代码与模拟器构建 | 完成 | `bern build --target watchos` 编译 `aarch64-apple-watchos-sim` 静态库、Mac 端 CLI 与 MCP 工具，以及 Swift 6 watchOS 应用包（`WKApplication`、`WKWatchOnly`、`UIDeviceFamily = 4`） |
| 手表原生控件适配 | 完成 | 针对 watchOS 不提供 `SwiftUI.Menu` 与 `.menu` / `.segmented` 选择器的限制，由 `BernWatchMenu` 与 `BernWatchPicker` 以支持表冠滚动的原生 sheet 呈现；`Form` 行避免双层底板嵌套 |
| 引擎平台能力 | 完成 | `bern-command` 在 watchOS 上支持 `Application Support` / `Caches` 存储、钥匙串 `secrets`、`MPNowPlayingInfoCenter` `now_playing`，以及 `javascript = true` 时的 Boa 引擎 |
| 应用图标 | 完成 | `[app.icon].watchos` 支持方形 PNG、`.appiconset` 目录与 Icon Composer `.icon` 目录 |
| `bern run --target watchos` | 完成 | 安装到已启动的 watchOS 模拟器，传入 `SIMCTL_CHILD_BERN_IPC_DIR`，由 `--engine watchos` 通过 IPC 控制 |
| Watch-Only `.ipa` 打包 | 部分 | `bern package --target watchos` 编译 `aarch64-apple-watchos` 并生成 `<App>-<version>-watchos.ipa`（`ITSWatchOnlyContainer` 壳包裹 `Watch/<App>.app`，ad hoc 签名）；真机重签与安装未验证 |

## 工程结构

| 位置 | 内容 |
| --- | --- |
| `crates/bern-backend-swiftui/src/watchos.rs` | `SwiftUiWatchos`：目标描述、`B401` 手表布局检查、代码生成、模拟器构建计划与 Watch-Only `.ipa` 打包 |
| `crates/bern-backend-swiftui/templates/BernWatchKit.swift` | WatchKit 屏幕尺寸、虚拟窗口 sheet 宿主、`BernSheetChrome`、`BernWatchPicker`、`BernWatchMenu`、`BernSearchInput`、`AVKit.VideoPlayer` 呈现层 |
| `crates/bern-ir/src/television.rs` | `Application::on_watch` 与 `Application::without_watch` 编译期裁剪 |
| `crates/bern-backend-swiftui/tests/watchos.rs` | 代码生成、`B401`、`#[bern::features]` `watch` 覆盖、分支裁剪及 Swift 6 SDK 类型检查测试 |

## 与 iOS 的差异

- **场景与窗口**：单个 `WindowGroup` 承载 `BernVirtualHost`。次级窗口作为原生 watchOS sheet 打开；未自带导航容器的窗口包裹 `BernSheetChrome`，以 `.cancellationAction` 替换 sheet 左上角自带的 `×` 按钮，既不额外挤占右上角空间显示重复的 `Done` 胶囊，又保证关闭动作始终经过 `confirm_close` 协议。
- **表单底板去重**：watchOS 的 `Form` 默认已为每行包上一层圆角卡片底板。在 `Form` 内，普通次要 `Button` 直接复用行自身的卡片底板，而 `prominent` / `icon` / `flat` 按钮以及自带圆角滑轨底板的 `Slider` 会清除外层 `listRowBackground`，避免出现“卡片里再套胶囊”的双层嵌套。
- **watchOS SwiftUI 缺失控件的原生替代**：
  - watchOS 没有 `SwiftUI.Menu` 及 `.pickerStyle(.menu)` / `.pickerStyle(.segmented)`，改由 `BernWatchMenu` 与 `BernWatchPicker` 打开支持数码表冠滚动的原生 sheet 列表，点选即生效并收起。
  - `SwiftUI.Table` 在 watchOS 上不可用，降级为行按钮 `List`；`.inspector` 降级为 `.sheet`；`SplitButton` 在无 `ControlGroup` 时使用紧凑 `HStack`；`PopoverButton` 以 sheet 弹出。
  - `BernRubyText` 通过 CoreText 在 SwiftUI `Canvas`（`GraphicsContext.withCGContext`）上排版绘制，`BernLitText` 直接使用 SwiftUI `TextRenderer`，不依赖 watchOS 不提供的 `UIViewRepresentable`。
  - watchOS 无 WebKit、文档选择器与系统剪贴板，因此不提供 `WebPage`、动态 `Svg`（静态 SVG 仍在构建期栅格化为 PNG）、`file_pick` 与 `paste`。

## 构建与运行

```bash
rustup target add aarch64-apple-watchos aarch64-apple-watchos-sim
```

```bash
bern build --target watchos
```

```bash
bern run --target watchos
```

```bash
bern package --target watchos
```

## 验证记录

2026-11-14，Apple Watch Series 12 (46mm) 模拟器，watchOS 27.2，Xcode 27.2。

- **构建与测试**：`crates/bern-backend-swiftui/tests/watchos.rs` 对生成代码执行 `WatchSimulator27.2.sdk` Swift 6 类型检查，并验证 `B401`、`#[bern::features]` `watch` 覆盖与 `window.watch` 分支裁剪；`ios16_front_end` 与 `television` 回归测试通过；`bern build --target watchos` 与 `bern package --target watchos` 零警告产出 `WearDemo.app` 与 `WearDemo-0.1.0-watchos.ipa`。
- **模拟器交互与截图检查**：将 `examples/wearos` 安装至 Apple Watch Series 12 (46mm) 并传入 `SIMCTL_CHILD_BERN_IPC_DIR` 启动，经 `--engine watchos` IPC 与 `simctl io screenshot` 逐屏核对：`Adaptive` 报告 `watch`、`counter.add` 更新引擎计数、`NavigationStack` 推入 `History` 与 `ui back` 返回、`Settings` sheet 仅保留单个左上角 Liquid Glass `×` 关闭按钮、`Toggle`、`Slider`、`BernWatchPicker`（`Daily` → `Weekly`）、`TextField` 草稿编辑、运行时 `ui set-language` 切换中英文，以及未保存草稿关闭 `Alert`（`ConfirmationRequired` → `Cancel` 留在页面、`Discard` 关闭并分离 `settings#1`）。
- **未验证**：Apple Watch 物理真机、`-watchos.ipa` 重签名安装到配对设备、视频播放、`Table`、`Tabs`、`SplitView`、物理表冠阻尼手感，以及手表上的钥匙串与系统播放中心实机表现。
