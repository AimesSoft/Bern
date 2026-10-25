# Bern 文档

更新：2026-10-25。入门使用根目录 [README 的快速开始](../README.md#快速开始)，可构建示例在 [examples](../examples/)。当前接口与平台状态从下面的参考文档查起。

## 当前接口

- [组件用法](Reference/UI.md)：文本与格式化、播放器坐标场景、控件分组与尺寸、受控输入、图片与瀑布流、Hero、引导、搜索、空闲状态、键盘与快捷键、菜单栏及存储。
- [完整组件属性表](Reference/Component-Properties.md)：从当前 `bern-ir::spec` 导出的全部内置组件、属性类型、必填项、子节点和动作规则。
- [平台能力与验证范围](Reference/Platforms.md)：按后端功能标记列出支持情况，将声明支持、构建与交互验证分开。
- [性能追踪](Reference/Tracing.md)：`BERN_TRACE` 与 `trace.*` 的规则、各段的含义、终端汇总、`trace.report` 与时间线文件。

## 架构与设计

- [架构与语义规范](Design.md)：组件声明、模块、主题、命令、窗口、媒体、应用图标和明确标注的后续设计。
- [平台适配规范](Adaptation/README.md)：Apple、Android、Windows / Linux、Web 以及研究中的平台；适配目标不代表当前已实现。
- [React Native 研究](Research/ReactNative.md)：运行时桥接与构建期生成的架构比较。

## 实现与专题

- [macOS](Implementation/macOS-SwiftUI.md)、[iOS](Implementation/iOS-SwiftUI.md)、[tvOS](Implementation/tvOS-SwiftUI.md)、[Android](Implementation/Android-Compose.md)、[Windows](Implementation/Windows-WinUI3.md)、[Linux](Implementation/Linux-GTK.md)、[Web](Implementation/Web.md)：实现细节、历次验证与限制。
- [动画 SVG](Implementation/Animated-SVG.md)：可选组件接口、资源、依赖、静态回退和验证范围。
- [JavaScript 与目标能力](Implementation/JavaScript.md)：WebView／浏览器与 Boa 的条件构建、脚本上下文和能力分支。
- [按模块编译](Implementation/Module-Builds.md)：`--exclude-module`、编译期 UI 条件和可选 Cargo 依赖。
- [应用自己的浏览器扩展](Implementation/Web-Extensions.md)：浏览器目标的 `web_extension`，应用自带的媒体执行端、WASM 导入与文件准备钩子。

## 更新规则

组件或属性变化时同步属性表与用法；后端新增功能时同步能力表；验收记录写清日期、目标、场景及未验证范围。代码生成成功、原生构建成功和实际设备交互通过是三个不同结论。新增记录后同时检查旧“尚未实现”“未验证”条目，避免同一能力在文档中有互相冲突的状态。

接口来源为 [bern-ir 组件定义](../crates/bern-ir/src/spec.rs)，能力来源为 [bern-target-api](../crates/bern-target-api/src/lib.rs)（检查规则在 [check/](../crates/bern-target-api/src/check/)）与各后端 TargetDescriptor。实现日志保留历史上下文，不能覆盖当前参考表中的接口事实。
