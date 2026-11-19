**English** · [简体中文](../zh-CN/Implementation/Native-Views.md)

# Co-located native views (`NativeView`)

Updated: 2026-11-20. When an application needs a platform-specific control or surface that Bern's standard component set does not expose (such as AppKit `NSLevelIndicator` / `NSDatePicker`, WinUI 3 `NumberBox` / `CalendarView`, Android `AndroidView(RatingBar / CalendarView)`, GTK 4 `LevelBar` / `Calendar`, or a custom GPUI / Web canvas), `NativeView` embeds co-located native source files right beside the `.rs` UI file that declares it.

Unlike runtime bridge escape hatches, `NativeView` keeps all semantics, localized strings, CLI entries and fallback behavior in the Rust declaration:

- **Co-located files**: Backend paths (`swiftui`, `winui3`, `compose`, `gtk`, `gpui`, `web`) resolve relative to the declaring `.rs` file. Directory scanning (`sources = "pkg/src"`) skips `*.gtk.rs`, `*.gpui.rs` and `*.native.rs` automatically.
- **No runtime internals exposed**: Each backend generates narrow, strongly typed `Props`, `Actions` and `Theme` structs for the view instead of exposing `BernRuntime` internals.
- **Mandatory CLI entries**: Every `NativeView` must declare at least one `NativeAction`, `NativeNumber`, `NativeBool` or `NativeInput` child. All six backends register those entries automatically under `<component>.<view>.<entry>` so `ui inspect`, `ui activate`, `ui set-number`, `ui set-bool`, `ui edit` and `ui submit` work without piercing native widget trees.
- **Default full-width layout**: Without an explicit `width:` prop, `NativeView` fills the container's available width across all six backends so native panels align cleanly inside `Column` layouts. Pass `width:` for a fixed width.
- **Localization and linting**: Pass user-visible text via `NativeText(name:, content: tr! { ... })` or entry `label:`s. The compiler also runs a lightweight lint over co-located native files and warns on obvious hardcoded display strings (suppress individual lines with `// bern-ignore-l10n`).
- **Progressive cross-platform coverage**: Provide `Fallback { ... }` for targets without a native file. Targets that have neither a matching backend file nor a `Fallback` fail at `bern check` / `bern build`.

## Declaration structure

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

## Generated types per backend

For each `NativeView`, the backend exposes:

- Every `NativeValue(name:, value:)` and `NativeText(name:, content:)` as a read-only field in `Props`.
- Every `NativeDraft(name:, value:)` as a read-only field in `Props` and a `set_<name>` / `set<Name>` callback in `Actions`.
- Every entry (`NativeAction`, `NativeNumber`, `NativeBool`, `NativeInput`) with its localized `<id>_label` / `<id>Label` in `Props`, current value (`level`, `boosted`, `date`, plus `min` / `max` / optional `step` for `NativeNumber`), and its typed callback in `Actions` (`reset`, `set_level` / `setLevel`, `set_boosted` / `setBoosted`, `set_date` / `setDate`).
- A read-only `Theme` with `dark` / `is_dark` and semantic colors (`accent`, `surface`, `text_primary` / `textPrimary`, `text_secondary` / `textSecondary`, `border` mapped to `control.border`).

| Backend | File suffix | Entry point & naming | `Props` read model | Build integration |
| --- | --- | --- | --- | --- |
| SwiftUI | `.swift` | `struct <IdPascal>NativeView: View { let props: <IdPascal>NativeProps; let actions: <IdPascal>NativeActions; let theme: BernNativeTheme }` (`camelCase` fields) | Direct values (SwiftUI tracks `@Observable` reads automatically) | Passed directly by original path to `swiftc` (or rewritten into `Sources/` for `swiftui-16`) |
| WinUI 3 / Islands | `.cpp` / `.inl` | `inline xaml::UIElement build(Props const& props, Actions const& actions, Theme const& theme)` inside `namespace native_<sym>` (`snake_case` fields) | Reactive getters `std::function<T()>` read inside `Reactive::Run(...)` | `#include "<abs_path>"` inside `app_ui.inl` (compiled for both `bern::winui` with `BERN_WINUI=1` and `bern::islands` with `BERN_WINUI=0`) |
| Compose | `.kt` | `@Composable fun NativeContent(props: Props, actions: Actions, theme: Theme)` in `<pkg>.native_<sym>` (`camelCase` fields) | Direct immutable `data class` values on each recomposition | Emitted into `<pkg>/native_<sym>/NativeContent.kt` and preserved in Gradle `Mirror` |
| GTK 4 | `.gtk.rs` | `pub fn build(props: Props, actions: Actions, theme: Theme) -> impl IsA<gtk::Widget>` inside `mod native_<sym>` (`snake_case` fields) | Reactive getters `Rc<dyn Fn() -> T>` read inside `bern_gtk::reactive::effect(...)` | `#[path = "<abs_path>"] mod impl_;` inside generated `app/src/ui.rs` |
| GPUI | `.gpui.rs` | `pub fn render(props: Props, actions: Actions, theme: Theme) -> impl IntoElement` inside `mod native_<sym>` (`snake_case` fields) | Direct snapshot values per render frame | `#[path = "<abs_path>"] mod impl_;` inside generated `app/src/ui.rs` |
| Web | `.js` | `export function mount({ host, props, actions, theme, effect, onCleanup })` (`camelCase` fields) | Reactive getters `() => val` read inside the passed `effect(...)`; dispose in `onCleanup(...)` | Emitted as ES module `native/<sym>.js`, statically imported by `app.js` and included in `bern package` |

### Scalar & draft type mapping

| Rust type | SwiftUI | WinUI 3 / Islands | Compose | GTK 4 / GPUI | Web |
| --- | --- | --- | --- | --- | --- |
| `bool` | `Bool` | `bool` | `Boolean` | `bool` | `boolean` |
| `i64` | `Int64` | `int64_t` | `Long` | `i64` | `number` |
| `f64` | `Double` | `double` | `Double` | `f64` | `number` |
| `String` | `String` | `winrt::hstring` | `String` | `String` | `string` |
| `Option<f64>` | `Double?` | `std::optional<double>` | `Double?` | `Option<f64>` | `number \| null` |

## Platform notes

- **Apple (`swiftui`)**: One `.swift` file serves macOS, iOS, tvOS and watchOS; branch platform-specific controls with `#if os(macOS) ... #else ... #endif`.
- **Windows (`winui3`)**: `app_ui.inl` compiles once in `bern::winui` (`BERN_WINUI == 1`) and once in `bern::islands` (`BERN_WINUI == 0`). Use the shared `xaml::` / `controls::` / `media::` aliases and guard WinUI 3-only controls (such as `controls::NumberBox`) with `#if BERN_WINUI`. Capture WinRT handles by value in `Reactive::Run([=]() mutable { ... })`.
- **Android (`compose`)**: Use standard Compose Material 3 composables or embed classic Android widgets through `androidx.compose.ui.viewinterop.AndroidView`. Omit `package` declarations in `*.compose.kt`; the generator hoists your `import` lines under the per-view subpackage.
- **Linux (`gtk`)**: `bern-gtk` re-exports `gtk`, `adw`, `gio` and `glib`. Read `props` inside `bern_gtk::reactive::effect(...)` and guard widget signal callbacks against re-entrant sync updates.
- **Web (`web`)**: Use the `effect` and `onCleanup` callbacks passed into `mount(...)` instead of re-importing `./bern.js` directly. Browser-native ESM URL imports work directly without a bundler.

## Verification (`examples/native-controls`)

Complete six-backend showcase in [examples/native-controls](../../examples/native-controls/) (`meter` and `calendar` views). Verified on 2026-11-19 / 2026-11-20:

- **macOS (SwiftUI + AppKit)**: Built and run on real macOS (`NSLevelIndicator` + `Gauge` + `Stepper` + `NSDatePicker`); full CLI end-to-end verification (`ui inspect`, `set-number`, `set-bool`, `edit`, `set-language`, `activate`) passed.
- **Windows 11 (WinUI 3)**: Built and run on real Windows 11 (`ProgressBar` + `NumberBox` + `ToggleSwitch` + `CalendarView`); full-width layout and full CLI end-to-end verification passed.
- **Windows 10 VM (XAML Islands)**: Run on Windows 10 Pro 22H2 (`ProgressBar` + `-`/`+` stepper + `ToggleSwitch` + `CalendarView`); full-width layout and full CLI end-to-end verification passed.
- **Ubuntu Linux VM (GTK 4 / libadwaita)**: Built and run on GNOME Wayland (`gtk::LevelBar` + `gtk::SpinButton` + `gtk::ToggleButton` + `gtk::Calendar`); full-width layout and full CLI end-to-end verification passed.
- **iOS 26.5 Simulator (iPhone 17 Pro)**: Built and run (`Gauge` + `ProgressView` + `Stepper` + graphical `DatePicker`); full-width layout and full CLI end-to-end verification passed.
- **Android Emulator (Pixel 9 Pro XL)**: Built APK, installed and launched (`CircularProgressIndicator` + `AndroidView(RatingBar)` + `FilterChip` + `AndroidView(CalendarView)`); full-width rendering verified visually. Remote Android CLI over `adb forward` was not separately verified.
- **Web & GPUI**: Passed `bern check`, unit tests and code generation; not yet run interactively in a browser or GPUI window.
