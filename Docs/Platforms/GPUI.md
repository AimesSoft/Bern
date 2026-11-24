**English** · [简体中文](../zh-CN/Platforms/GPUI.md)

# GPUI (Optional Unified Desktop)

Bern includes an optional **GPUI** backend (`ui = "gpui"`) that renders a unified GPU-accelerated desktop interface on **macOS, Windows, and Linux** using Bern's own theme algorithm (`BernTheme`). It is never the default for any platform—you opt into it explicitly in `Bern.toml` when you want identical GPU-rendered chrome across all three desktop systems alongside (or instead of) the native backends.

## Prerequisites

- **Rust 1.99+** (`rustup update`).
- Platform GPU/windowing build dependencies on the host you build for:
  - **macOS**: Standard Xcode Command Line Tools (no separate Metal compiler toolchain required; shaders compile at runtime via `runtime_shaders`). `WebKit`, `AVFoundation`, `CoreMedia`, and `ServiceManagement` are built into macOS.
  - **Windows**: Visual Studio C++ build tools + Windows 11 SDK (`rc.exe` / `fxc.exe`) and Rust `stable-msvc`. `WebPage` uses the installed Microsoft Edge WebView2 Runtime; built-in media playback uses Windows Media Foundation (`IMFMediaEngine`).
  - **Linux**: X11/Wayland, font, and GPU development headers plus a Vulkan or OpenGL ES driver (or Mesa software rendering). At runtime, `WebPage` dynamically loads WebKitGTK (`libwebkit2gtk-4.1.so.0` + `libgtk-3.so.0`) and built-in media playback dynamically loads GStreamer (`libgstreamer-1.0.so.0` + `libgstapp-1.0.so.0`) with zero link-time requirement when unused:

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

Because `bern-gpui` links the same Rust engine in-process and exposes the same IPC channel as the native desktop targets, the generated app CLI, UI session commands (`back`, `focus`, `set_file`, `window.*`, close confirmation protocol), `BERN_TRACE` UI spans, and Skill work identically:

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

- **Full desktop IR parity**: Supports all desktop Bern components, features, styles, and motion tokens—including `Table`/`TableColumn`/`TableMenu`, `List`, `Inspector`, `Marquee`, `WindowButtons`, `Slot`, `Sidebar(fallback:)`, `Drop(opens:)`, `WindowRole::Settings` and `WindowRole::Dialog` (modal blocking), `Window(confirm_close:)`, `WindowSet(palette:)`, `Text(lit*)`, `TextInput(current:)`, `paste:`, `Rail(overhang:)`, `idle_area`, `glow_opacity`, `Stack` border/hover styles, pure-Rust animated `Svg` (`BSVG`), and opt-in in-process `javascript = true`.
- **Unified visual system**: Uses `BernTheme` (high-contrast neutral palette, runtime semantic palette overrides, consistent spacing, rounded corners, and spring physics) across macOS, Windows, and Linux.
- **Native system integration**:
  - **Menu bar**: Native macOS `MenuBar` (`platform = "macos"`).
  - **Status item (`StatusItem`)**: Native tray icon and dynamic menu on all three platforms (macOS `NSStatusBar`, Windows `Shell_NotifyIconW`, Linux D-Bus `StatusNotifierItem` + `com.canonical.dbusmenu`), keeping the app alive after the last window closes and launching silently with `--at-login`.
  - **Open at Login (`Switch(login_item: true)`)**: macOS `SMAppService` (with LaunchAgent fallback when unbundled), Windows `HKCU\...\Run` registry, and Linux XDG Autostart `.desktop`.
- **Native embedded `WebPage`**: Uses native platform WebViews (`WKWebView` on macOS, `WebView2` on Windows, `WebKitGTK` on Linux) supporting `cookies_file`, `cookie_domain`, `user_agent`, and `close_on`, plus native WebKit `content_mode` on macOS (Windows and Linux emit the standard build warning and keep the engine default mode, matching WinUI 3 and GTK).
- **Media playback (`Video`)**: Ships built-in native media executors on all three platforms (macOS `AVFoundation`, Windows `IMFMediaEngine`, Linux `GStreamer`) along with GPUI's standard `Video(controls:)` playback control bar. When an application supplies its own `MediaKernel`, the application kernel continues to take priority automatically.
- **Native views**: Co-located GPUI views can be mounted directly via `NativeView(gpui: "...")` (keeping `Fallback` for non-GPUI targets). See [Native views](../Topics/Native-Views.md).

## Packaging and signing

`bern package` produces distributable artifacts for all three GPUI desktop targets into `target/bern/dist/`:

```bash
# macOS: signed (and optionally notarized) .dmg disk image
bern package --target gpui_macos

# Windows: portable .zip (with optional signtool signing via BERN_WINDOWS_CERTIFICATE)
bern package --target gpui_windows

# Linux: portable .tar.gz plus .deb package (when dpkg-deb is available)
bern package --target gpui_linux
```

Signing environment variables (`BERN_MACOS_SIGNING_IDENTITY`, `BERN_MACOS_NOTARY_PROFILE`, `BERN_WINDOWS_CERTIFICATE`, `BERN_WINDOWS_PASSWORD`, `BERN_WINDOWS_TIMESTAMP`) match the native desktop targets.
