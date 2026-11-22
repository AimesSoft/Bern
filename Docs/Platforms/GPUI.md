**English** · [简体中文](../zh-CN/Platforms/GPUI.md)

# GPUI (Optional Unified Desktop)

Bern includes an optional **GPUI** backend (`ui = "gpui"`) that renders a unified GPU-accelerated desktop interface on **macOS, Windows, and Linux** using Bern's own theme algorithm (`BernTheme`). It is never the default for any platform—you opt into it explicitly in `Bern.toml` when you want identical GPU-rendered chrome across all three desktop systems alongside (or instead of) the native backends.

## Prerequisites

- **Rust 1.99+** (`rustup update`).
- Platform GPU/windowing build dependencies on the host you build for:
  - **macOS**: Standard Xcode Command Line Tools (no separate Metal compiler toolchain required; shaders compile at runtime via `runtime_shaders`).
  - **Windows**: Visual Studio C++ build tools + Windows 11 SDK (`rc.exe` / `fxc.exe`) and Rust `stable-msvc`.
  - **Linux**: X11/Wayland, font, and GPU development headers plus a Vulkan or OpenGL ES driver (or Mesa software rendering):

```bash
# Debian / Ubuntu
sudo apt install pkg-config libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev
```

## Configuration in `Bern.toml`

Give each desktop OS its own target entry with `ui = "gpui"`:

```toml
[targets.gpui_macos]
platform = "macos"
ui = "gpui"

[targets.gpui_windows]
platform = "windows"
ui = "gpui"

[targets.gpui_linux]
platform = "linux"
ui = "gpui"
```

GPUI targets can coexist in the same `Bern.toml` with the default native targets (`swiftui`, `winui3`, `gtk`). Use `--target` to select which one to build or run.

## Build, run, and debug

```bash
# Check feature coverage for the GPUI target
bern check --target gpui_macos

# Build the GPUI binary, generated app CLI, and Skill
bern build --target gpui_macos

# Build and launch the GPUI app
bern run --target gpui_macos
```

Because `bern-gpui` links the same Rust engine in-process and exposes the same IPC channel as the native desktop targets, the generated app CLI and Skill work identically:

```bash
target/bern/gpui_macos/bin/tasks --help
target/bern/gpui_macos/bin/tasks window screenshot --output gpui.png
```

Useful debugging environment variables:

| Variable | Effect |
| --- | --- |
| `BERN_WINDOW_OFFSCREEN=1` | Moves newly opened GPUI windows off-screen—useful for running automated UI/CLI checks without stealing focus on an active desktop. |
| `BERN_APPEARANCE=light\|dark` | Overrides system light/dark appearance. |
| `BERN_MOTION=reduce\|slow` | Disables springs or slows them 10× for frame-by-frame inspection. |
| `BERN_MOTION_LOG=1` | Logs active spring animations to stderr. |
| `BERN_MEDIA_LOG=1` | Logs video-to-kernel media handoff diagnostics. |
| `BERN_TRACE=<rules>` | Enables performance tracing (or use `bern run --trace ...`). |

## Platform behavior and adaptations

- **Unified visual system**: Uses `BernTheme` (high-contrast neutral palette, consistent spacing, rounded corners, and spring physics) across macOS, Windows, and Linux.
- **macOS menu bar**: When built with `platform = "macos"`, the GPUI target also declares and renders the native macOS `MenuBar`.
- **Standard controls & animations**: Supports split/tab navigation, forms, tables, sheets/modals, search palettes, automatic skeleton screens, pure-Rust animated `Svg` (`BSVG`), and opt-in in-process `javascript = true`.
- **Native views**: Co-located GPUI views can be mounted directly via `NativeView(gpui: "...")` (keeping `Fallback` for non-GPUI targets). See [Native views](../Topics/Native-Views.md).
- **Capability checking**: Any IR feature not yet declared by `bern-backend-gpui` is caught at compile time by `bern check --target <gpui_target>` with a clear diagnostic pointing to the declaration.

## Packaging and signing

`bern package` does not generate standalone installers for GPUI targets yet. For release binaries, run:

```bash
bern build --target gpui_macos --release
```

and distribute the compiled binary alongside its generated `resources/`, `bin/`, and `skills/` artifacts from `target/bern/<target>/`.
