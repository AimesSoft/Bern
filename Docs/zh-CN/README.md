[English](../README.md) · **简体中文**

# 文档

第一次使用 Bern，请从 **开发指南** 开始学习如何用 Rust 编写应用；配置、调试或打包具体操作系统时请查 **平台操作手册**；需要核对 `ui!` 语法、组件属性或平台能力支持表时，请查阅 **接口参考**。

## 按任务速查

| 你想做什么 | 对应文档 |
| --- | --- |
| 从零安装 `bern` 并跑通第一个应用 | [快速上手](Guides/Quick-Start.md) |
| 用 Rust 定义状态、命令、查询与错误处理 | [协议与引擎](Guides/Protocol-and-Engine.md) |
| 编写自适应布局、导航、弹层、电视与手表界面 | [布局与导航](Guides/Layouts-and-Navigation.md) |
| 接入多语言翻译（`tr!`）、主题与样式（`style!`） | [本地化与样式](Guides/Localization-and-Styling.md) |
| 配置多窗口、图标、系统托盘、文件与媒体播放 | [窗口、资源与文件](Guides/Windows-Resources-and-Files.md) |
| 用配套应用 CLI 与 AI Skill 自动化操控和截图 | [应用 CLI 与 Skill](Guides/App-CLI-and-Skills.md) |
| 导出独立纯原生 IDE 工程，解除框架锁定 | [导出为独立纯原生项目](Topics/Eject.md) |
| 配置特定平台工具链、调试、签名与打包 | [平台操作手册](#平台操作手册) |
| 查阅组件属性清单或平台能力支持矩阵 | [接口参考](#接口参考) |
| 了解编译流水线与框架架构不变量 | [架构总览](Architecture.md) |

## 总览

- [架构总览](Architecture.md) — 编译流水线（`#[bern::protocol]` / `ui!` → IR → 原生目标）、引擎边界与核心架构不变量。

## 开发指南

- [快速上手](Guides/Quick-Start.md) — 安装 `bern`、使用 `bern new` 创建项目、理解三 crate 结构并跑通 `check` / `run` / `package` 循环。
- [协议与引擎](Guides/Protocol-and-Engine.md) — `#[bern::protocol]`、`#[bern::session]`、`#[bern::value]`、`Query`、后台任务、本地存储与凭据管理。
- [布局与导航](Guides/Layouts-and-Navigation.md) — `Adaptive` 四档尺寸与 TV/Watch 布局、`SplitView`、`TabView`、`Modal`、`Dialog`、`Shortcut` 与 `default_focus`。
- [本地化与样式](Guides/Localization-and-Styling.md) — `tr!`、`strings/*.toml`、复数与日期格式化、语义色、`style!` 与运行时深浅色/色板切换。
- [窗口、资源与文件](Guides/Windows-Resources-and-Files.md) — 多窗口场景、`MenuBar`、`StatusItem`、`[app.icon]`、`[[app.opens]]`、文件选择与 `MediaSource` / `Video`。
- [应用 CLI 与 Skill](Guides/App-CLI-and-Skills.md) — 使用生成的配套应用 CLI、IPC 路由、`window screenshot`、`ui.*` 控制命令与自动生成的 `SKILL.md`。

## 平台操作手册

面向每个平台的实战手册，涵盖环境准备、`Bern.toml` 配置、构建调试、平台适配要点以及打包与签名：

- [macOS](Platforms/macOS.md) — SwiftUI + AppKit（`.app`、`.dmg` 与 Apple 公证）
- [iOS 与 iPadOS](Platforms/iOS.md) — SwiftUI + UIKit、双前端回退（`swiftui` + `swiftui-16`）、模拟器与 `.ipa`
- [watchOS](Platforms/watchOS.md) — 独立 Watch-Only SwiftUI、`Layout(size: watch)`（`B401`）与 `.ipa`
- [tvOS](Platforms/tvOS.md) — Apple TV SwiftUI、Siri Remote 焦点控制、Top Shelf 图标与 `.ipa`
- [Windows](Platforms/Windows.md) — C++/WinRT 双前端（WinUI 3 + XAML Islands）、Direct2D 安装包与 `.zip`
- [Linux](Platforms/Linux.md) — GTK 4 + libadwaita、StatusNotifierItem、`.deb` 与 `.tar.gz`
- [Android 与 Wear OS](Platforms/Android.md) — Jetpack Compose Material 3 与 Wear Compose、SAF、`.apk` 与 `.aab`
- [GPUI](Platforms/GPUI.md) — 可选的跨桌面统一 GPU 渲染后端（`ui = "gpui"`）
- [Web](Platforms/Web.md) — 纯前端 WASM 应用（`web_mode = "app"`）、原生引擎 WebUI（`web_mode = "ui"`）与内嵌 `web_ui`

## 专题扩展

- [导出为独立纯原生项目](Topics/Eject.md) — `bern eject` 可随时导出标准 `.xcodeproj`、Gradle、Visual Studio `.sln`/`.vcxproj`、独立 Cargo 与零打包器 ES 模块工程（支持 `--engine source` 与 `--engine prebuilt`）。
- [原生视图](Topics/Native-Views.md) — 与 Rust 源码同仓的跨平台原生视图（`NativeView`），含类型化 props、`tr!` 校验、CLI 入口与 `Fallback`。
- [JavaScript 与 `bern.cli`](Topics/JavaScript.md) — 按目标开启的进程内 JavaScript（`bern::script`）、`bern.cli(...)` 与宿主 `CliPolicy` 策略。
- [动画 SVG](Topics/Animated-SVG.md) — 编译期 `BSVG` 字节码与全平台纯 Rust 动画渲染。
- [按模块构建](Topics/Module-Builds.md) — 使用 `bern new --module` 与 `--exclude-module` 拆分独立功能模块。
- [Web 扩展](Topics/Web-Extensions.md) — 通过 `web_extension` 集成自定义 JS/WASM 浏览器扩展。

## 接口参考

- [UI 声明参考](Reference/UI.md) — `ui!` 语法、表达式、导航、格式化、动效与自动骨架屏。
- [组件属性表](Reference/Component-Properties.md) — 所有内置组件的属性、子节点与动作清单。
- [平台能力与验证范围](Reference/Platforms.md) — 各目标特性标记支持矩阵与当前验证说明。
- [性能追踪](Reference/Tracing.md) — `BERN_TRACE`、`bern run --trace`、`trace.*` CLI 命令与时间线输出格式。

## 更新规则

- 每当组件、属性、目标选项或 `Bern.toml` 配置项发生变化时，在同一改动中同步更新对应的开发指南、平台手册与[接口参考](#接口参考)。
- 保持 `Docs/` 与 `Docs/zh-CN/` 的目录和文档结构一一对应（标题层级、代码块数与表格行数一致），并通过 `cargo test -p bern --test docs` 校验。
- 文档内容以框架当前实际行为为准，不再保留过期的历史设计草稿或私有应用名称。
