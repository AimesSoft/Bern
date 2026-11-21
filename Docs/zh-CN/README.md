[English](../README.md) · **简体中文**

# Bern 文档

更新：2026-11-21。安装 `bern` 并运行第一个应用请从根目录 [README 的快速开始](../../README.zh-CN.md#快速开始)入手；完整可构建示例在 [examples/](../../examples/)。

## 按场景快速查阅

| 你想做什么 | 建议先读 |
| --- | --- |
| 用标准控件、自适应布局、默认骨架图或 `NativeView` 编写界面 | [当前组件用法](Reference/UI.md)与[完整组件属性表](Reference/Component-Properties.md) |
| 核对各后端支持范围、构建证据与真机验证状态 | [平台能力与验证范围](Reference/Platforms.md) |
| 启用 JavaScript 脚本或用 `bern.cli` 与黑白名单做插件系统 | [JavaScript 与 `bern.cli`](Implementation/JavaScript.md) |
| 在 Rust 声明旁紧邻嵌入各平台原生控件或自绘视图 | [紧邻原生视图](Implementation/Native-Views.md) |
| 选用 GPUI 后端或排查特定平台细节 | [GPUI](Implementation/GPUI.md)或下方各平台实现记录 |
| 了解整体架构、协议、主题、窗口与命令模型 | [架构与语义规范](Design.md) |

## 当前接口

- [当前组件用法](Reference/UI.md)：文本与 ICU 格式化、坐标场景、控件容器、引擎受控输入、图片与瀑布流、Hero 过渡、引导、搜索、空闲状态、键盘快捷键、菜单栏、存储、默认骨架图、`NativeView`，以及完整标准控件体系（`TreeView`、`SplitPane`、`Reorderable`、`BottomSheet`、`Refreshable`、`DatePicker`、`TimePicker`、`NumberInput`、`Alert`、`Banner`、`Disclosure`、`Tooltip`、`EmptyState`、`Badge`、`Avatar`）。
- [完整组件属性表](Reference/Component-Properties.md)：从当前 `bern-ir::spec` 导出的全部内置组件、属性类型、必填项、子节点和动作规则。
- [平台能力与验证范围](Reference/Platforms.md)：按后端功能标记与跨平台能力列出支持范围，将声明支持、构建验证与实机交互验证分开记录。
- [性能追踪](Reference/Tracing.md)：`BERN_TRACE` 与 `trace.*` 规则、各分段含义、终端汇总、`trace.report` 与时间线导出。

## 架构与设计

- [架构与语义规范](Design.md)：组件声明、模块、主题、命令、窗口、媒体、应用图标和明确标注的后续设计。
- [平台适配规范](Adaptation/README.md)：Apple、Android、Windows / Linux、Web 以及研究中的平台；适配目标不代表当前已实现后端。
- [React Native 研究](Research/ReactNative.md)：运行时桥接与构建期生成两种架构的比较。

## 实现与专题

### 平台后端

- **Apple**：[macOS（SwiftUI）](Implementation/macOS-SwiftUI.md) · [iOS / iPadOS（SwiftUI）](Implementation/iOS-SwiftUI.md) · [watchOS（SwiftUI）](Implementation/watchOS-SwiftUI.md) · [tvOS（SwiftUI）](Implementation/tvOS-SwiftUI.md)
- **桌面与移动**：[Windows（WinUI 3 与 XAML Islands）](Implementation/Windows-WinUI3.md) · [Linux（GTK 4）](Implementation/Linux-GTK.md) · [Android 与 Wear OS（Jetpack Compose）](Implementation/Android-Compose.md)
- **可选 GPU 与 Web**：[GPUI（macOS、Windows、Linux）](Implementation/GPUI.md) · [Web 应用与 WebUI（DOM + WebAssembly）](Implementation/Web.md)

### 专题文档

- [紧邻原生视图](Implementation/Native-Views.md)：在 Rust 声明同目录放置各平台原生 UI 文件的 `NativeView`，含强类型 `Props`/`Actions`/`Theme`、强制 CLI 入口、`tr!` 多语言检查与 `Fallback`。
- [JavaScript 与 `bern.cli`](Implementation/JavaScript.md)：按需启用的 `javascript = true`（所有原生目标统一进程内 Boa，Web 目标为浏览器 Worker）、持久脚本上下文、进程内 `globalThis.bern.cli(...)`、Rust 侧黑白名单 `CliPolicy` 与 `window.webview` 编译期分支。
- [动画 SVG](Implementation/Animated-SVG.md)：零 WebView 依赖的纯 Rust `BSVG` 字节码渲染、资源声明、静态回退与验证范围。
- [按模块编译](Implementation/Module-Builds.md)：`--exclude-module`、编译期 UI 条件和可选 Cargo 依赖。
- [应用自己的浏览器扩展](Implementation/Web-Extensions.md)：浏览器目标的 `web_extension`、应用自带媒体执行端、WASM 导入与文件准备钩子。

## 更新规则

组件或属性变化时同步属性表与用法；后端新增功能时同步能力表；验收记录写清日期、目标、场景及未验证范围。代码生成成功、原生构建成功和实际设备交互通过是三个不同结论。新增记录后同时检查旧“尚未实现”“未验证”条目，避免同一能力在文档中有互相冲突的状态。

接口来源为 [bern-ir 组件定义](../../crates/bern-ir/src/spec.rs)，能力来源为 [bern-target-api](../../crates/bern-target-api/src/lib.rs)（检查规则在 [check/](../../crates/bern-target-api/src/check/)）与各后端 `TargetDescriptor`。实现日志保留历史上下文，不能覆盖当前参考表中的接口事实。

文档同步维护英文与简体中文两种语言。英文为默认版本，位于 `Docs/`、`README.md` 与 `CONTRIBUTING.md`；简体中文位于 `Docs/zh-CN/`（文件名与英文一一对应）、`README.zh-CN.md` 与 `CONTRIBUTING.zh-CN.md`。任何修改都在同一次提交中同步两边：标题层级、表格行与代码块一一对应，相对链接与锚点均有效，由 `cargo test -p bern --test docs` 自动校验。
