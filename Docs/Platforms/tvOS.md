**English** · [简体中文](../zh-CN/Platforms/tvOS.md)

# tvOS

Bern compiles the tvOS target (`platform = "tvos"`) to a native **SwiftUI** Apple TV application targeting **tvOS 18 or newer**. tvOS targets must be built on a Mac.

## Prerequisites

- **macOS host** with **Rust 1.99+** and **Xcode 26 or newer** (with the tvOS platform SDK and Apple TV simulators installed).
- Add the Rust targets for the tvOS simulator and Apple TV hardware:

```bash
# On Apple Silicon Macs (use x86_64-apple-tvos on Intel Macs)
rustup target add aarch64-apple-tvos-sim aarch64-apple-tvos
bern doctor
```

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[app.icon]
default = "assets/icon.png"
tvos = "assets/AppIcon.brandassets"  # Optional: .brandassets or square PNG

[targets.tvos]
platform = "tvos"
ui = "swiftui"
```

When `[app.icon].tvos` is omitted (or points to a square PNG), Bern automatically synthesizes the layered tvOS home screen app icon and Top Shelf image from the default icon.

## Build, run, and debug

```bash
# Check tvOS declarations
bern check --target tvos

# Build for the tvOS simulator
bern build --target tvos

# Launch on an Apple TV simulator with IPC connected
bern run --target tvos
```

Drive the running simulator app with the generated app CLI using `--engine tvos`:

```bash
target/bern/tvos/bin/tasks --engine tvos window screenshot --output tvos.png
```

Useful debugging environment variables:

| Variable | Effect |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | Overrides appearance for inspection. |
| `BERN_MOTION=reduce\|slow` | Reduces motion or slows spring animations 10×. |
| `BERN_TRACE=<rules>` | Enables performance tracing. |
| `BERN_SNAPSHOT_DIR=<dir>` | Saves automatic window snapshots. |

## Platform behavior and adaptations

- **Compile-time TV specialization**: `window.tv` is statically `true` on tvOS. Bern keeps only `Layout(size: tv)` branches in `Adaptive`, applies `font(tv:)` typography sizes from `style!` blocks, and prunes desktop/phone/watch layouts at compile time.
- **Siri Remote & Focus Engine**:
  - Set `default_focus: true` on the primary `Button` or `IconButton` in a view so focus lands reliably when a screen or modal opens.
  - `Shortcut` maps remote buttons without needing a visible control: `escape` maps to Menu/Back, `space` maps to Play/Pause, and arrow keys fire when focus cannot move further in that direction.
  - On full-screen media screens where controls are hidden, a full-screen focus target lets directional arrows act as seek/skip shortcuts.
- **Windows and overlays**: Secondary windows open as `fullScreenCover`; `Popover` opens as a sheet; `Table` lowers to a single-column list of focusable row buttons.
- **Media and SVG**: Custom `MediaKernel` receives a `CAMetalLayer` and `CADisplayLink` frame clock. Animated `Svg` compiles to pure-Rust `BSVG` bytecode (no WebKit dependency); static SVGs rasterize to PNGs.
- **Unavailable capabilities**: `file_pick`, `paste:`, `WebPage`, `MenuBar`, `StatusItem`, `wheel`, `slider_hover`, `scroll_anchor`, `scroll_browsing`, `bound_blur`, and runtime alternate icons are not available on tvOS. Keep pointer-only UI inside non-`tv` `Adaptive` branches so `bern check --target tvos` stays clean.
- **Native views**: Co-located SwiftUI/UIKit tvOS views mount via `NativeView(tvos: "...")`. See [Native views](../Topics/Native-Views.md).

## Packaging and signing

```bash
bern package --target tvos
```

`bern package --target tvos` builds the release arm64 bundle and writes an ad-hoc `.ipa` to `target/bern/dist/`. Re-sign with your Apple distribution certificate and tvOS provisioning profile before installing on an Apple TV or submitting to App Store Connect.
