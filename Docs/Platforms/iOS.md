**English** · [简体中文](../zh-CN/Platforms/iOS.md)

# iOS & iPadOS

Bern compiles the iOS target (`platform = "ios"`) to a native **SwiftUI + UIKit** app bundle built on macOS, supporting both **iPhone** and **iPad**. By default, Bern ships dual frontends inside one app bundle so devices on **iOS 17+** use the modern SwiftUI frontend while **iOS 16** devices seamlessly fall back to the iOS 16 frontend.

## Prerequisites

- **macOS host** with **Rust 1.99+** and **Xcode 26 or newer** (including the iOS platform SDK and simulators).
- Add the Rust targets for the iOS simulator and physical devices:

```bash
# On Apple Silicon Macs (use x86_64-apple-ios on Intel Macs)
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
bern doctor
```

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
files_folder = true          # Expose app Documents directory in the iOS Files app
background_audio = false     # Enable AVAudioSession background playback
photos = false               # Request photo library save permission when saving images
local_network = false        # Request local network permission (NSLocalNetworkUsageDescription)

[app.icon]
default = "assets/icon.png"
ios = "assets/AppIcon.icon"  # Optional: PNG, .appiconset, or Icon Composer .icon

[targets.ios]
platform = "ios"
ui = "swiftui"
# Default dual frontends; omit unless you want to restrict versions or split packages:
# frontends = ["swiftui", "swiftui-16"]
# packages = "one"           # "one" (default, single .ipa) or "separate"
```

### Dual frontends (`swiftui` + `swiftui-16`)

| `frontends` | Minimum iOS | Packaging (`packages`) | Behavior |
| --- | --- | --- | --- |
| `["swiftui", "swiftui-16"]` *(default)* | iOS 16.0 | `"one"` (single `.ipa`) or `"separate"` (`-ios.ipa` + `-ios16.ipa`) | Shared `BernEngine.framework` (Rust) + `BernUI17.framework` loaded via `dlopen` on iOS 17+; iOS 16 falls back to the built-in iOS 16 frontend. |
| `["swiftui"]` | iOS 17.0 | Single `.ipa` | Drops the iOS 16 fallback for smaller bundles and full iOS 17+ APIs only. |

## Build, run, and debug

```bash
# Build for the iOS simulator
bern build --target ios

# Boot/reuse an iOS simulator, install the app, and launch it with local IPC wired up
bern run --target ios
```

When launched via `bern run --target ios`, Bern forwards `SIMCTL_CHILD_BERN_IPC_DIR` into the simulator so your host machine can drive the simulator app directly with the generated app CLI:

```bash
target/bern/ios/bin/tasks --engine ios window screenshot --output ios.png
```

Useful debugging environment variables (pass through `SIMCTL_CHILD_*` when launching manually via `xcrun simctl`):

| Variable | Effect |
| --- | --- |
| `BERN_FRONT_END=16` | Forces the iOS 16 fallback frontend on an iOS 17+ simulator so you can verify iOS 16 layout and behavior without a separate iOS 16 runtime. |
| `BERN_APPEARANCE=light\|dark` | Overrides system appearance. |
| `BERN_MOTION=reduce\|slow` | Forces reduced motion or slows springs 10×. |
| `BERN_TRACE=<rules>` | Enables tracing (or use `bern run --target ios --trace ...`). |
| `BERN_SNAPSHOT_DIR=<dir>` | Writes window snapshots into the specified directory. |

## Platform behavior and adaptations

- **iPhone vs. iPad windows**: On iPhone, secondary windows open as virtual sheets and compact navigation uses a tab bar (`TabAccessory` maps to the system accessory above the tab bar on iOS 26.1+). On iPadOS (iOS 17+), Bern uses native multi-window `WindowGroup` scenes and exposes the Settings window via `⌘,`.
- **Orientation locking**: `Window(landscape: true)` locks orientation on iPhone when a screen (such as a video player) requests landscape.
- **Icons and alternate icons**: `[app.icon].ios` accepts a PNG, `.appiconset`, or Icon Composer `.icon` (compiled to light/dark/tinted Liquid Glass `Assets.car`). `[app.alternate_icons]` maps to `CFBundleAlternateIcons` and switches icons at runtime via `WindowSet(icon:)`.
- **Files and sharing**: `file_pick` opens the system document picker; `bern::platform::open_path` presents the system share sheet (`UIActivityViewController`), `bern::platform::export_path` copies a local file or folder into a picked security-scoped Files folder, and setting `files_folder = true` sets `UIFileSharingEnabled` and `LSSupportsOpeningDocumentsInPlace` so the app's documents appear in the Files app.
- **Not applicable on iOS**: `MenuBar`, `StatusItem`, and `Switch(login_item: true)` are accepted cleanly as no-ops so shared desktop/mobile declarations compile without `#[cfg]` guards.
- **iOS 16 fallback differences**: When running on iOS 16 (`swiftui-16`), scroll paging (`ScrollView(snap:)`), `scroll_scale` / `scroll_opacity`, hardware keyboard `onKeyPress`, iPad multi-window scenes, and HDR images are gracefully reduced (inspector opens as a sheet, iPad uses virtual windows, images render in SDR).
- **Native views**: Co-located SwiftUI/UIKit views mount through `NativeView(ios: "...")`. See [Native views](../Topics/Native-Views.md).

## Packaging and signing

```bash
bern package --target ios
```

`bern package --target ios` builds the release arm64 device bundle and writes an ad-hoc `.ipa` under `target/bern/dist/` (or two `.ipa` files when `packages = "separate"`).

Because Apple requires a distribution or development provisioning profile and signing certificate for installation on physical devices or TestFlight/App Store uploads, re-sign the generated `.ipa` with your team certificate and provisioning profile before deploying to devices or submitting via Xcode / `xcrun altool` / Transporter.
