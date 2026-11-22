[English](../../Topics/Native-Views.md) · **简体中文**

# 紧邻原生视图（`NativeView`）

更新：2026-11-20。当应用需要 Bern 标准组件集未直接暴露的平台原生控件或表面（例如 AppKit 的 `NSLevelIndicator` / `NSDatePicker`、WinUI 3 的 `NumberBox` / `CalendarView`、Android 的 `AndroidView(RatingBar / CalendarView)`、GTK 4 的 `LevelBar` / `Calendar`，或自定义 GPUI / Web 画布）时，`NativeView` 可直接嵌入放在声明 `.rs` 文件同目录下的原生源码文件。

与运行时桥接逃生舱不同，`NativeView` 将所有业务语义、多语言文案、CLI 功能入口与回退行为收口在 Rust 声明处：

- **紧邻文件放置**：各后端文件路径（`swiftui`、`winui3`、`compose`、`gtk`、`gpui`、`web`）相对声明所在的 `.rs` 文件解析。目录扫描（`sources = "pkg/src"`）会自动忽略 `*.gtk.rs`、`*.gpui.rs` 和 `*.native.rs`。
- **不暴露运行时内部**：各后端为每个视图生成窄且强类型的 `Props`、`Actions` 和 `Theme` 结构体，不将 `BernRuntime` 内部实现暴露给原生代码。
- **强制规范化 CLI 入口**：每个 `NativeView` 必须至少声明一个 `NativeAction`、`NativeNumber`、`NativeBool` 或 `NativeInput` 子节点。六个后端都会自动以 `<component>.<view>.<entry>` 将它们注册到控件表，`ui inspect`、`ui activate`、`ui set-number`、`ui set-bool`、`ui edit` 与 `ui submit` 无需穿透原生视图树即可直接工作。
- **默认左右铺满**：未显式传入 `width:` 属性时，`NativeView` 在全部六个后端默认占满容器可用宽度，确保原生面板在 `Column` 布局中对齐整齐；需要固定宽度时传入 `width:` 即可。
- **多语言与静态检查**：所有展示文案经由 `NativeText(name:, content: tr! { ... })` 或入口的 `label:` 传入。编译器还会对紧邻原生文件做轻量静态检查，遇到明显的硬编码展示字面量时发出警告（可用 `// bern-ignore-l10n` 按行豁免）。
- **跨平台渐进实现**：为尚未提供原生文件的目标平台编写 `Fallback { ... }`；若某目标既没有对应后端文件也没有 `Fallback`，将在 `bern check` / `bern build` 阶段报错。

## 声明结构

```rust
NativeView(
    id: meter,
    label: tr! { "zh-Hans" => "原生电平仪表", "en" => "Native Level Meter" },
    swiftui: "meter.swiftui.swift",
    winui3: "meter.winui3.cpp",
    compose: "meter.compose.kt",
    gtk: "meter.gtk.rs",
    gpui: "meter.gpui.rs",
    web: "meter.web.js",
) {
    NativeText(name: heading, content: tr! { "zh-Hans" => "原生电平与步进器", "en" => "Native Meter & Stepper" })
    NativeDraft(name: hover, value: draft.hover_level)
    NativeNumber(id: level, label: tr! { "zh-Hans" => "电平", "en" => "Level" }, current: app.level, min: 0, max: 10, step: 1) => app.set_level(app.level)
    NativeBool(id: boosted, label: tr! { "zh-Hans" => "增强模式", "en" => "Boost Mode" }, checked: app.boosted) => app.set_boosted(app.boosted)
    NativeAction(id: reset, label: tr! { "zh-Hans" => "重置", "en" => "Reset" }) => app.reset()
    Fallback {
        Slider(id: level_fallback, label: tr! { "zh-Hans" => "电平", "en" => "Level" }, current: app.level, min: 0, max: 10) => app.set_level(app.level)
    }
}
```

## 各后端生成接口

每个 `NativeView` 会按声明自动暴露：

- 每个 `NativeValue(name:, value:)` 和 `NativeText(name:, content:)` 作为 `Props` 中的只读字段。
- 每个 `NativeDraft(name:, value:)` 在 `Props` 中提供只读当前值，并在 `Actions` 中生成 `set_<name>` / `set<Name>` 写回回调。
- 每个功能入口（`NativeAction`、`NativeNumber`、`NativeBool`、`NativeInput`）在 `Props` 中暴露多语言 `<id>_label` / `<id>Label` 及当前值（`level`、`boosted`、`date`，`NativeNumber` 另附 `min` / `max` / 可选 `step`），并在 `Actions` 中暴露类型化回调（`reset`、`set_level` / `setLevel`、`set_boosted` / `setBoosted`、`set_date` / `setDate`）。
- 只读 `Theme`，提供 `dark` / `is_dark` 与语义色（`accent`、`surface`、`text_primary` / `textPrimary`、`text_secondary` / `textSecondary`、映射到 `control.border` 的 `border`）。

| 后端 | 文件后缀 | 入口签名与字段风格 | `Props` 读取模型 | 构建集成方式 |
| --- | --- | --- | --- | --- |
| SwiftUI | `.swift` | `struct <IdPascal>NativeView: View { let props: <IdPascal>NativeProps; let actions: <IdPascal>NativeActions; let theme: BernNativeTheme }`（`camelCase` 字段） | 直接值（SwiftUI 自动追踪 `@Observable` 读取） | 原文件绝对路径直接传给 `swiftc`（`swiftui-16` 经 `ios16::rewrite` 写入 `Sources/`） |
| WinUI 3 / Islands | `.cpp` / `.inl` | `inline xaml::UIElement build(Props const& props, Actions const& actions, Theme const& theme)`，位于 `namespace native_<sym>`（`snake_case` 字段） | 响应式 getter `std::function<T()>`，在 `Reactive::Run(...)` 内读取 | 在 `app_ui.inl` 中 `#include "<abs_path>"`（同时编入 `BERN_WINUI=1` 与 `BERN_WINUI=0`） |
| Compose | `.kt` | `@Composable fun NativeContent(props: Props, actions: Actions, theme: Theme)`，位于 `<pkg>.native_<sym>`（`camelCase` 字段） | 每次重组传入不可变 `data class` 直接值 | 发射到 `<pkg>/native_<sym>/NativeContent.kt` 并加入 Gradle `Mirror` 保留列表 |
| GTK 4 | `.gtk.rs` | `pub fn build(props: Props, actions: Actions, theme: Theme) -> impl IsA<gtk::Widget>`，位于 `mod native_<sym>`（`snake_case` 字段） | 响应式 getter `Rc<dyn Fn() -> T>`，在 `bern_gtk::reactive::effect(...)` 内读取 | 在生成的 `app/src/ui.rs` 中以 `#[path = "<abs_path>"] mod impl_;` 挂接 |
| GPUI | `.gpui.rs` | `pub fn render(props: Props, actions: Actions, theme: Theme) -> impl IntoElement`，位于 `mod native_<sym>`（`snake_case` 字段） | 每帧即时渲染快照值 | 在生成的 `app/src/ui.rs` 中以 `#[path = "<abs_path>"] mod impl_;` 挂接 |
| Web | `.js` | `export function mount({ host, props, actions, theme, effect, onCleanup })`（`camelCase` 字段） | 响应式 getter `() => val`，在传入的 `effect(...)` 内读取，在 `onCleanup(...)` 释放 | 发射为独立 ES 模块 `native/<sym>.js`，由 `app.js` 静态导入并随 `bern package` 打包 |

### 标量与草稿类型映射

| Rust 类型 | SwiftUI | WinUI 3 / Islands | Compose | GTK 4 / GPUI | Web |
| --- | --- | --- | --- | --- | --- |
| `bool` | `Bool` | `bool` | `Boolean` | `bool` | `boolean` |
| `i64` | `Int64` | `int64_t` | `Long` | `i64` | `number` |
| `f64` | `Double` | `double` | `Double` | `f64` | `number` |
| `String` | `String` | `winrt::hstring` | `String` | `String` | `string` |
| `Option<f64>` | `Double?` | `std::optional<double>` | `Double?` | `Option<f64>` | `number \| null` |

## 各平台编写要点

- **Apple（`swiftui`）**：单个 `.swift` 文件同时覆盖 macOS、iOS、tvOS 与 watchOS，平台差异用标准 `#if os(macOS) ... #else ... #endif` 条件编译。
- **Windows（`winui3`）**：`app_ui.inl` 会在 `bern::winui`（`BERN_WINUI == 1`）和 `bern::islands`（`BERN_WINUI == 0`）中各编译一次。请使用共享的 `xaml::` / `controls::` / `media::` 别名，并用 `#if BERN_WINUI` 隔离仅 WinUI 3 存在的控件（如 `controls::NumberBox`）。`Reactive::Run([=]() mutable { ... })` 中请按值捕获 WinRT 对象句柄。
- **Android（`compose`）**：可直接写 Material 3 Compose 组件，也可用 `androidx.compose.ui.viewinterop.AndroidView` 嵌入传统 Android `View`。`*.compose.kt` 中无需写 `package` 行，生成器会自动将你的 `import` 提升到独立子包声明下方。
- **Linux（`gtk`）**：`bern-gtk` 已重导出 `gtk`、`adw`、`gio` 与 `glib`。在 `bern_gtk::reactive::effect(...)` 中读取 `props` 并刷新控件，并用同步标记避免信号回调重入。
- **Web（`web`）**：请使用传入 `mount(...)` 的 `effect` 与 `onCleanup`，不要在原生模块中重新导入无版本号的 `./bern.js`。浏览器原生 HTTPS ESM 导入无需打包器即可直接使用。

## 实测验证（`examples/native-controls`）

完整六后端示例见 [examples/native-controls](../../../examples/native-controls/)（含 `meter` 与 `calendar` 两个原生视图）。验证日期 2026-11-19 / 2026-11-20：

- **macOS（SwiftUI + AppKit）**：真机构建并运行（`NSLevelIndicator` + `Gauge` + `Stepper` + `NSDatePicker`），全套 CLI 端到端验证（`ui inspect`、`set-number`、`set-bool`、`edit`、`set-language`、`activate`）通过。
- **Windows 11（WinUI 3）**：真机构建并运行（`ProgressBar` + `NumberBox` + `ToggleSwitch` + `CalendarView`），默认左右铺满与全套 CLI 端到端验证通过。
- **Windows 10 虚拟机（XAML Islands）**：Windows 10 Pro 22H2 实机运行（`ProgressBar` + `-`/`+` 步进按钮 + `ToggleSwitch` + `CalendarView`），默认左右铺满与全套 CLI 端到端验证通过。
- **Ubuntu Linux 虚拟机（GTK 4 / libadwaita）**：GNOME Wayland 实机构建并运行（`gtk::LevelBar` + `gtk::SpinButton` + `gtk::ToggleButton` + `gtk::Calendar`），默认左右铺满与全套 CLI 端到端验证通过。
- **iOS 26.5 模拟器（iPhone 17 Pro）**：构建并运行（`Gauge` + `ProgressView` + `Stepper` + 图形化 `DatePicker`），默认左右铺满与全套 CLI 端到端验证通过。
- **Android 模拟器（Pixel 9 Pro XL）**：构建 APK 并安装启动（`CircularProgressIndicator` + `AndroidView(RatingBar)` + `FilterChip` + `AndroidView(CalendarView)`），默认左右铺满视觉验证通过；经 `adb forward` 的 Android 远端 CLI 未单独验证。
- **GPUI（macOS）**：离屏构建并运行（`meter.gpui.rs` 分段电平条 + `calendar.gpui.rs` 月历网格），默认左右铺满与 CLI 驱动（`ui inspect`、`set-number`、`set-bool`）验证通过。
- **Web**：通过 `bern check`、单元测试与代码生成检查；尚未在浏览器中实跑交互。
