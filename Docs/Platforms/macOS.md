**English** · [简体中文](../zh-CN/Platforms/macOS.md)

# macOS

Bern compiles the macOS target (`platform = "macos"`) to a native **SwiftUI + AppKit** `.app` bundle targeting **macOS 14 or newer**, linking the Rust engine in-process as a static library. macOS targets must be built on a Mac.

## Prerequisites

- **Rust 1.99+** (`rustup update`).
- **Xcode 26 or newer** (macOS SDK 26.0+) installed from the App Store, with active developer directory set to Xcode:

```bash
sudo xcode-select --switch /Applications/Xcode.app
bern doctor
```

`bern doctor` checks the active macOS SDK and `swiftc` version.

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[app.icon]
default = "assets/icon.png"
macos = "assets/AppIcon.icon"   # Optional: PNG, .icns, or Icon Composer .icon

[targets.macos]
platform = "macos"
ui = "swiftui"
```

Common macOS options in `Bern.toml`:

| Setting | Purpose |
| --- | --- |
| `[app.icon].macos` | Accepts a square PNG, `.icns`, or an Icon Composer `.icon` bundle (compiled via `actool` into Liquid Glass `Assets.car` + fallback `.icns` for macOS 14–15). |
| `[app.alternate_icons]` | Declares runtime-switchable Dock icons bound via `WindowSet(icon:)`. |
| `[[app.opens]]` | Registers document file types in `Info.plist` so Finder "Open With" and drag-to-Dock launch or route files into the engine. |
| `webview = true` | Enables `WebPage` (`WKWebView`). Disabled by default to avoid linking WebKit when unused. |
| `javascript = true` | Enables in-process JavaScript (`bern::script`) with zero hidden WebView processes. See [JavaScript and `bern.cli`](../Topics/JavaScript.md). |
| `web_ui = "<target>"` | Bundles a `web_mode = "ui"` target inside the app's resources (`__bern_web_ui/`) so the app can serve a browser UI via `bern::web_ui::set(...)`. |

## Build, run, and debug

```bash
# Build the .app bundle, generated app CLI, and Skill
bern build --target macos

# Build and launch the app directly
bern run --target macos

# Launch with performance tracing enabled
bern run --target macos --trace engine,ui,slow:16ms
```

Build outputs under `target/bern/macos/`:

- `<App>.app` — native application bundle (`Contents/MacOS/<app>`, `Contents/Helpers/<app>` for the app CLI, `Contents/Resources/skills/<app>/SKILL.md` for the agent Skill).
- `<App>.app.dSYM` — debug symbols beside the bundle.
- `bin/<app>` — symlink to the generated app CLI, ready to run while the app is open:

```bash
target/bern/macos/bin/tasks --help
target/bern/macos/bin/tasks window screenshot --output shot.png
```

Useful debugging environment variables:

| Variable | Effect |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | Overrides the system light/dark appearance for testing. |
| `BERN_MOTION=reduce\|slow` | Forces reduced motion, or slows spring animations by 10× for frame-by-frame inspection. |
| `BERN_TRACE=<rules>` | Enables structured tracing (same syntax as `bern run --trace`). See [Performance tracing](../Reference/Tracing.md). |
| `BERN_SNAPSHOT_DIR=<dir>` | Saves automatic window snapshots for layout checks. |
| `BERN_RENDER=active` | Draws controls in every window with active-window styling even when unfocused. |
| `BERN_IPC_DIR=<dir>` | Overrides the local IPC socket directory shared between the app and its CLI. |

## Platform behavior and adaptations

- **Windows and scenes**: Supports native multi-window (`WindowGroup`), Settings window (`⌘,`), `Dialog` sheets, and `Modal` overlays.
- **Menu bar and tray**: `MenuBar`, `CommandMenu`, and `CommandItem` populate the native macOS menu bar. `StatusItem` renders a `MenuBarExtra` tray icon and menu; when all windows close while a `StatusItem` is active, the app stays running in the menu bar and hides its Dock icon. `Switch(login_item: true)` integrates with macOS Login Items.
- **Native views**: Co-located SwiftUI/AppKit views can be mounted via `NativeView(macos: "...")`. See [Native views](../Topics/Native-Views.md).
- **Custom media core**: `Video` provides a `CAMetalLayer` and `CADisplayLink` frame clock when the engine registers a custom `MediaKernel`.
- **Files and drag-and-drop**: Standard `NSOpenPanel`/`NSSavePanel` file picking (`file_pick`), clipboard paste (`paste:`), and Finder file associations (`[[app.opens]]`) are all supported natively.

## Packaging and signing

```bash
bern package --target macos
```

`bern package` builds a release `.app` and creates an LZMA-compressed (`ULMO`) `.dmg` disk image with a clean Finder drag-to-Applications layout under `target/bern/dist/` (no GUI Finder session required).

Without signing environment variables, the app is ad-hoc signed for local testing only—Gatekeeper on other Macs will reject it. Set these environment variables to sign and notarize for distribution:

```bash
export BERN_MACOS_SIGNING_IDENTITY="Developer ID Application: Example Inc (TEAMID)"
export BERN_MACOS_NOTARY_PROFILE="bern-notary"   # created via `xcrun notarytool store-credentials`
bern package --target macos
```

`BERN_MACOS_NOTARY_PROFILE` requires `BERN_MACOS_SIGNING_IDENTITY`. When both are set, Bern signs the bundle and CLI helper with hardened runtime, builds and signs the `.dmg`, submits it to Apple's notarization service, and staples the ticket automatically.
