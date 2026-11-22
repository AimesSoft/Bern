**English** · [简体中文](../zh-CN/Platforms/watchOS.md)

# watchOS

Bern compiles the watchOS target (`platform = "watchos"`) to a standalone Watch-Only **SwiftUI** application targeting **watchOS 10 or newer**. watchOS targets must be built on a Mac.

## Prerequisites

- **macOS host** with **Rust 1.99+** and **Xcode 26 or newer** (with the watchOS platform SDK and Apple Watch simulators installed).
- Add the Rust targets for the watchOS simulator and Apple Watch hardware:

```bash
# On Apple Silicon Macs (use x86_64-apple-watchos-sim on Intel Macs)
rustup target add aarch64-apple-watchos-sim aarch64-apple-watchos
bern doctor
```

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[targets.watchos]
platform = "watchos"
ui = "swiftui"
```

### Mandatory `watch` layout (`B401`)

Because watch screens are tiny and scroll vertically with the Digital Crown, Bern requires every reachable `Adaptive` block in a watchOS target to provide an explicit `Layout(size: watch)` branch. If an `Adaptive` block lacks `Layout(size: watch)`, `bern check --target watchos` fails with diagnostic **`B401`**.

At compile time, Bern prunes all non-`watch` branches (`compact`, `medium`, `expanded`, `tv`) so code and views for larger screens are never generated into the watch binary.

```rust
Adaptive {
    Layout(size: watch) {
        ScrollView {
            Column(gap: 8) {
                Text(tasks.title, style: title)
                ForEach(tasks.items) |item| {
                    Button(item.title) => toggle(id: item.id)
                }
            }
        }
    }
    Layout(size: compact) { /* pruned for watchOS */ }
}
```

## Build, run, and debug

```bash
# Check declarations (including B401 watch-layout coverage)
bern check --target watchos

# Build for the watchOS simulator
bern build --target watchos

# Launch on an Apple Watch simulator with IPC connected
bern run --target watchos
```

Drive the running simulator app via the generated app CLI using `--engine watchos`:

```bash
target/bern/watchos/bin/tasks --engine watchos window screenshot --output watch.png
```

Useful debugging environment variables:

| Variable | Effect |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | Overrides appearance for inspection. |
| `BERN_MOTION=reduce\|slow` | Reduces motion or slows spring animations 10×. |
| `BERN_TRACE=<rules>` | Enables performance tracing. |
| `BERN_SNAPSHOT_DIR=<dir>` | Saves automatic window snapshots. |

## Platform behavior and adaptations

- **Watch-native controls**: `Menu` and `Picker` render as crown-scrollable native watch sheets (`BernWatchMenu` / `BernWatchPicker`). `Table` lowers to a vertical `List`. `Form` avoids nested row plates so stacked cards stay legible on small screens.
- **Navigation and modals**: Secondary windows and `Modal` views open as full-screen watch sheets; `TabView` uses vertical page tabs.
- **Animated SVG & JavaScript**: Pure-Rust animated `Svg` (`BSVG` bytecode) and opt-in in-process `javascript = true` both run directly on watchOS with no WebView dependency. Static SVGs rasterize to PNGs at build time.
- **Unavailable capabilities**: `WebPage`, `file_pick`, `paste:`, `MenuBar`, `StatusItem`, `ShortcutInput`, and pointer-hover interactions are not available on watchOS. Declarations that keep those desktop/phone controls inside non-`watch` branches of `Adaptive` compile cleanly because unused branches are pruned before target checking.
- **Native views**: Co-located WatchKit/SwiftUI views can be embedded with `NativeView(watchos: "...")`. See [Native views](../Topics/Native-Views.md).

## Packaging and signing

```bash
bern package --target watchos
```

`bern package --target watchos` builds a release Watch-Only `.ipa` (`arm64` + `arm64_32` where applicable) under `target/bern/dist/`. Re-sign the `.ipa` with your Apple distribution certificate and Watch provisioning profile before installing on a paired Apple Watch or uploading to App Store Connect.
