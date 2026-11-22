[English](../Architecture.md) · **简体中文**

# 架构与核心不变量

本文浓缩 Bern 编译器、运行时与各后端共同遵守的架构边界与硬约束。日常使用 Bern 开发应用，请直接从[开发指南](Guides/Quick-Start.md)、[平台操作手册](Platforms/macOS.md)与[接口参考](Reference/UI.md)开始。

## 1. 构建期生成原生界面

Bern 是面向 Rust 的跨平台原生 UI 框架。开发者在 `.rs` 文件中编写协议、业务逻辑与声明式 `ui!` 界面；构建时 `bern build` 将 `ui!` 编译为带版本的中间表示（`bern-ir`），再交给彼此独立的目标后端（`bern-target-api`）生成各平台自己的原生 UI 代码：

- **Apple（SwiftUI）**：macOS、iOS / iPadOS、watchOS、tvOS
- **Windows（C++/WinRT）**：WinUI 3，无运行时环境自动回退系统 XAML（`xaml-islands`）
- **Linux**：GTK 4 + libadwaita
- **Android 与 Wear OS**：Kotlin + Jetpack Compose（Material 3 / Wear Compose）
- **桌面 GPU（可选）**：GPUI（macOS、Windows、Linux）
- **Web**：DOM + WebAssembly（`web_mode = "app"` 或 `"ui"`）

Bern 不为渲染界面内嵌浏览器或 JS 引擎，也不会把任意 Rust 代码转译成 Swift、C++ 或 Kotlin。扩展新后端不会改动语法解析器、公共 IR 或应用业务逻辑。

## 2. 三层应用边界

每个 Bern 应用都严格划分为三层：

| 分层 | 职责 | 硬约束 |
| --- | --- | --- |
| **协议（`#[bern::protocol]`）** | 值类型、只读状态快照、类型化命令、查询与事件 | 状态结构与命令目录的唯一真相源 |
| **引擎（普通 Rust）** | 实现命令/查询 trait，持有业务状态 | 不依赖任何窗口、控件或传输层，可直接用 `cargo test` 无界面测试 |
| **界面（`#[component]` + `ui!`）** | 声明界面结构、窗口局部草稿（`#[bern::session]`）与数据绑定 | 受约束的可静态分析语法；不能直接修改引擎状态，也不能给按钮挂任意 Rust 闭包 |

## 3. 以命令为中心的多入口运行时

原生界面、自动生成的应用 CLI、本地 JSON/IPC 控制通道、WebUI 会话以及按需启用的进程内 JavaScript（`bern.cli`），全部经由同一套类型化命令目录驱动同一个业务引擎：

- **所有可操作控件必须绑定命令**（引擎命令或生成的 UI 会话命令）；漏绑命令的按钮直接编译报错（`B301`）。
- **Command 执行动作**、**Query 保持只读**、**Event 通知事实**——读写与副作用严格区分。
- **拒绝自动化黑盒**：即使是紧邻嵌入平台代码的 `NativeView`，也必须声明可被 CLI 发现与调用的控件入口，确保工具和 AI Agent 始终可以按语义操控所有界面。

## 4. 平台原生保真与自动主题

- **默认使用系统原生控件**：在系统原生目标（SwiftUI、WinUI 3、GTK 4、Compose）上，生成代码直接使用平台标准容器、控件、字体层级、输入法、无障碍与系统深浅色。自绘渲染仅用于缺少系统设计语言的后端（Web、GPUI）或显式声明的 `Canvas` / `style!`。
- **显式目标选择**：省略 `ui` 时解析该平台的默认原生技术。默认后端不可用或缺少必需能力时直接报错，绝不静默降级为 Web 或自绘后端。
- **零配置深浅色**：每个控件与主题角色默认跟随系统外观。原生目标直接消费操作系统语义色；Web 与 GPUI 消费 `bern-theme` 按对比度算法推导的主题快照。

## 5. 编译期静态保障与诊断

其他框架常留到运行时才暴露的遗漏，在 Bern 中由 `bern check` 与 `rustc` 直接拦截：

- **类型安全**：`ui!` 展开为 Rust 类型检查代码，由 `rustc` 验证每处状态读取、草稿绑定与命令参数。
- **本地化完整性**：用户可见文本通过 `tr!` 声明；缺少语言（`B102`）与硬编码展示字面量（`B103`，严格模式默认开启）直接编译失败。
- **自适应功能覆盖**：`#[bern::features]` 契约要求编译器逐个尺寸档证明每项功能都有可达入口（`B201`–`B204`），并输出每个目标的 `features.json` 覆盖报告。
- **编译期分支裁剪**：构建期已知的设备与模块条件（`window.tv`、`window.watch`、`window.webview`、`--exclude-module`）会在能力检查和代码生成前直接裁剪不可达分支。

## 6. 统一多窗口与交付边界

- **声明式多窗口**：`WindowSet` 与 `Window` 只需写一次。原生多窗口平台（macOS、Windows、Linux、iPadOS）生成原生系统窗口；单场景平台（iPhone、Android、Web、watchOS、tvOS）自动映射为符合平台习惯的虚拟窗口（sheet、全屏覆盖或对话框），并保持完全一致的窗口身份、关闭确认协议与 CLI 命令。
- **按需脚本引擎**：未配置 `javascript = true` 的应用零 JS 运行时依赖。开启后原生端统一使用进程内 Boa（Web 端使用 Web Worker），并受 Rust 宿主侧 `CliPolicy` 黑白名单约束。
- **自包含资源与文件抽象**：随包资源在 `Bern.toml`（`[resources]`）中显式声明并经 `Resources` 按名解析，不依赖特定平台包目录布局；`MediaSource` 统一抽象本地路径、随包资源、Android `content://` URI 与 Web `blob:` URL。
