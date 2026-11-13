**English** · [简体中文](../zh-CN/Implementation/watchOS-SwiftUI.md)

# watchOS SwiftUI implementation notes

For the current component interface see the [component reference](../Reference/UI.md); for per-item support and the recent verification scope see the [platform capability table](../Reference/Platforms.md); for the official basis on Apple platforms see the [Apple adaptation guidelines](../Adaptation/Apple.md).

- Date: 2026-11-14
- Basis: [Bern architecture design](../Design.md)
- Toolchain: Rust 1.99.0 (`aarch64-apple-watchos-sim`, `aarch64-apple-watchos`); Xcode 27.2, watchOS 27.2 simulator and device SDKs
- Verification environment: Apple Watch Series 12 (46mm) simulator, watchOS 27.2

This document records what is implemented and verified for the `swiftui + watchos` target (`Platform::Watchos`). It shares the SwiftUI generator and runtime with macOS, iOS and tvOS, using WatchKit and SwiftUI watchOS controls in `BernWatchKit.swift`, and shares `Layout(size: watch)` and `window.watch` with `compose-wearos`.

## Completion status

| Design requirement | Status | Notes |
| --- | --- | --- |
| Platform resolution | Done | `platform = "watchos"` defaults to `ui = "swiftui"`; `bern doctor`, `bern check`, `bern build`, `bern run` and `bern package` recognize this target |
| Explicit watch layout & contracts | Done | Every reachable `Adaptive` must declare `Layout(size: watch)` (`B401`); `Application::on_watch` prunes non-watch branches before checking, and `#[bern::features]` proves `watch` coverage separately |
| Generated code & simulator build | Done | `bern build --target watchos` compiles the Rust staticlib for `aarch64-apple-watchos-sim`, the CLI and MCP tools for the Mac, and the Swift 6 watchOS app bundle (`WKApplication`, `WKWatchOnly`, `UIDeviceFamily = 4`) |
| Watch-native controls | Done | `BernWatchPicker` and `BernWatchMenu` present Crown-scrollable watch sheets where `.menu` / `.segmented` pickers and `SwiftUI.Menu` do not exist on watchOS; `Form` rows avoid nested card platters |
| Engine capabilities | Done | `bern-command` provides `Application Support` / `Caches` storage, Keychain `secrets`, `MPNowPlayingInfoCenter` `now_playing`, and Boa JavaScript when `javascript = true` |
| App icon | Done | `[app.icon].watchos` accepts a square PNG, an `.appiconset` folder or an Icon Composer `.icon` folder |
| `bern run --target watchos` | Done | Installs into a booted watchOS simulator, passes `SIMCTL_CHILD_BERN_IPC_DIR`, and connects via `--engine watchos` |
| Watch-Only `.ipa` packaging | Partial | `bern package --target watchos` builds `aarch64-apple-watchos` and writes `<App>-<version>-watchos.ipa` (`ITSWatchOnlyContainer` stub wrapping `Watch/<App>.app`, signed ad hoc); re-signing and installing on a physical watch are not verified |

## Project structure

| Location | Contents |
| --- | --- |
| `crates/bern-backend-swiftui/src/watchos.rs` | `SwiftUiWatchos`: target descriptor, `B401` watch layout check, code generation, simulator build plan and Watch-Only `.ipa` packaging |
| `crates/bern-backend-swiftui/templates/BernWatchKit.swift` | WatchKit screen bounds, virtual window sheet host, `BernSheetChrome`, `BernWatchPicker`, `BernWatchMenu`, `BernSearchInput`, `AVKit.VideoPlayer` presenter |
| `crates/bern-ir/src/television.rs` | `Application::on_watch` and `Application::without_watch` compile-time pruning |
| `crates/bern-backend-swiftui/tests/watchos.rs` | Generation, `B401`, `#[bern::features]` `watch` coverage, branch pruning and Swift 6 SDK type-check tests |

## Differences from iOS

- **Scenes and windows**: one `WindowGroup` hosting `BernVirtualHost`. Secondary windows open as native watchOS sheets; windows without their own navigation wrap in `BernSheetChrome`, whose `.cancellationAction` replaces the sheet's top-leading `×` button so closing always runs through the `confirm_close` protocol without adding a duplicate trailing `Done` button.
- **Form platter hygiene**: watchOS `Form` already wraps each row in a rounded card platter. Inside a `Form`, secondary `Button`s use the row's own platter directly, while prominent/icon/flat buttons and `Slider`s clear `listRowBackground` so controls that bring their own Liquid Glass capsule or slider platter do not nest inside a second card.
- **Controls unavailable in watchOS SwiftUI**:
  - `SwiftUI.Menu` and `.pickerStyle(.menu)` / `.pickerStyle(.segmented)` are unavailable in watchOS; `BernWatchMenu` and `BernWatchPicker` present native watchOS sheets whose `List` rows scroll with the Digital Crown and dismiss on selection.
  - `SwiftUI.Table` is unavailable in watchOS and lowers to a `List` of row buttons; `.inspector` lowers to `.sheet`; `ControlGroup` in `SplitButton` uses a compact `HStack`; `PopoverButton` presents a sheet.
  - `BernRubyText` renders via CoreText into a SwiftUI `Canvas` (`GraphicsContext.withCGContext`), and `BernLitText` uses SwiftUI `TextRenderer`, avoiding `UIViewRepresentable` (unavailable on watchOS).
  - `WebPage`, dynamic WebView `Svg` (static SVGs compile to PNGs), `file_pick` and `paste` are not available on watchOS.

## Build and run

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

## Verification log

2026-11-14, Apple Watch Series 12 (46mm) simulator, watchOS 27.2, Xcode 27.2.

- **Build and tests**: `crates/bern-backend-swiftui/tests/watchos.rs` type-checks generated Swift 6 sources against `WatchSimulator27.2.sdk` and verifies `B401`, `#[bern::features]` `watch` coverage, and `window.watch` pruning; `ios16_front_end` and `television` regression tests pass; `bern build --target watchos` and `bern package --target watchos` produce `WearDemo.app` and `WearDemo-0.1.0-watchos.ipa` with zero compiler warnings.
- **Simulator interaction and screenshots**: `examples/wearos` installed and launched on Apple Watch Series 12 (46mm) with `SIMCTL_CHILD_BERN_IPC_DIR`; verified over `--engine watchos` IPC and `simctl io screenshot`: `Adaptive` reporting `watch`, `counter.add` updating engine state, `NavigationStack` pushing `History` and returning via `ui back`, `Settings` sheet opening with a single Liquid Glass `×` button, `Toggle`, `Slider`, `BernWatchPicker` (`Daily` → `Weekly`), `TextField` draft editing, runtime `ui set-language`, and unsaved-draft close confirmation `Alert` (`ConfirmationRequired` → `Cancel` keeping the sheet, `Discard` closing and detaching `settings#1`).
- **Not verified**: physical Apple Watch hardware, re-signing and installing `-watchos.ipa` onto a paired device, video playback, `Table`, `Tabs`, `SplitView`, hardware Digital Crown physical detents, and Keychain / Now Playing on watch.
