**English** · [简体中文](../zh-CN/Platforms/Android.md)

# Android & Wear OS

Bern compiles Android (`platform = "android"`) to native **Jetpack Compose (Material 3)** targeting **Android 8.0 (API 26) or newer** across phones, tablets, and Android TV. It compiles Wear OS (`platform = "wearos"`) to **Wear Compose Material 3**, also targeting **Android 8.0 (API 26) or newer by default** (configurable via `android.min_sdk`, e.g., `30` for Google Play Wear OS 3.0+ releases). Both targets can be built from macOS, Linux, or Windows.

## Prerequisites

- **Rust 1.99+** (`rustup update`).
- **Android Studio** with the **Android SDK** and **Android NDK** (via SDK Manager → SDK Tools). Set `ANDROID_HOME` (or `ANDROID_SDK_ROOT`) if installed outside the default location.
- **Gradle 9.4 or newer** on `PATH` (or override via `BERN_GRADLE`; override JDK via `BERN_JAVA_HOME` if needed).
- Install the Rust target(s) matching the ABIs you build:

```bash
# Required default ABI (arm64-v8a)
rustup target add aarch64-linux-android

# Required for Wear OS default release packaging (32-bit armeabi-v7a) and optional for Android
rustup target add armv7-linux-androideabi

# Optional x86_64 ABI if listed in `abis = [...]`
rustup target add x86_64-linux-android

bern doctor
```

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"
cleartext_http = false       # Allow plain HTTP (adds networkSecurityConfig)
background_audio = false     # Foreground media playback service
overlays = false             # Enable SYSTEM_ALERT_WINDOW overlay support

[app.icon]
default = "assets/icon.png"
android = "assets/android/res"  # Optional: PNG or Android res/ mipmap folder

[targets.android]
platform = "android"
ui = "compose"
abis = ["arm64-v8a"]            # Valid values: "arm64-v8a", "armeabi-v7a", "x86_64"
android = { network = true, multicast = false, audio_settings = false, all_files_access = false, cxx_runtime = false, min_sdk = 26 }

[targets.wearos]
platform = "wearos"
ui = "compose"
# Omit `abis` to package both 64-bit and 32-bit watches (`["arm64-v8a", "armeabi-v7a"]`) by default
abis = ["arm64-v8a", "armeabi-v7a"]
# Defaults to `min_sdk = 26` (Android 8.0); raise to `30` (Wear OS 3.0) or higher for Google Play
android = { standalone = true, min_sdk = 26 }
```

### Android target options (`android = { ... }`)

| Option | Target | Purpose |
| --- | --- | --- |
| `network` | Android / Wear OS | Adds `INTERNET` and `ACCESS_NETWORK_STATE` permissions. |
| `multicast` | Android / Wear OS | Adds `CHANGE_WIFI_MULTICAST_STATE` for LAN discovery (mDNS/SSDP). |
| `audio_settings` | Android / Wear OS | Adds `MODIFY_AUDIO_SETTINGS`. |
| `all_files_access` | Android | Declares `MANAGE_EXTERNAL_STORAGE` (prompted at runtime via `bern::platform::ask_all_files`). |
| `cxx_runtime` | Android / Wear OS | Bundles `libc++_shared.so` from the NDK when native C++ dependencies require it. |
| `standalone` | Wear OS | Sets `com.google.android.wearable.standalone` metadata (defaults to `true`). |
| `min_sdk` | Android / Wear OS | Sets `minSdk` in Gradle and the NDK target API level (`26`–`36`, defaults to `26` on both Android and Wear OS; set `30` or higher for Google Play Wear OS submissions). |

## Build, run, and debug

```bash
# Check declarations (Wear OS enforces B401 Layout(size: watch) coverage)
bern check --target android
bern check --target wearos

# Build APK and Gradle project
bern build --target android
bern build --target wearos

# Install and launch on a connected USB device or emulator with adb IPC forwarding
bern run --target android
bern run --target wearos
```

`bern run` installs the APK over `adb`, launches the activity, and sets up `adb forward` so you can drive the phone, tablet, TV, or watch directly from your development machine with the generated app CLI:

```bash
# Drive the running Android app
target/bern/android/bin/tasks window screenshot --output android.png

# Drive the running Wear OS app separately
target/bern/wearos/bin/tasks --engine wearos window screenshot --output wear.png
```

## Platform behavior and adaptations

- **Material 3 & dynamic color**: Follows Material 3 design language, system dynamic color (Android 12+), and light/dark mode automatically.
- **Storage Access Framework (`content://`)**: File picking returns Android SAF `content://` URIs. Pass them directly to `MediaSource` or inspect folders with `bern::files::list` / `bern::files::walk`—Bern resolves SAF descriptors internally without forcing copies.
- **Alternate app icons**: `[app.alternate_icons]` generates `<activity-alias>` entries so `WindowSet(icon:)` swaps the launcher icon at runtime.
- **Orientation locking**: `Window(landscape: true)` locks the activity to landscape on phones and tablets.
- **Android TV**: Detected at runtime from the system TV UI mode / leanback feature. Automatically activates `Layout(size: tv)`, `window.tv`, `font(tv:)`, D-pad focus (`default_focus: true`), and remote `Shortcut` keys (`escape` → Back, `space` → Play/Pause, D-pad arrows).
- **Wear OS**: Every reachable `Adaptive` block must provide `Layout(size: watch)` (`B401`); non-watch branches are pruned at compile time. Supports rotary input, swipe-to-dismiss, pure-Rust animated `Svg` (`BSVG`), and in-process `javascript = true`.
- **Native views**: Co-located Jetpack Compose / Android views mount via `NativeView(android: "...")` (or `wearos: "..."`). See [Native views](../Topics/Native-Views.md).
- **Not applicable**: `MenuBar`, `StatusItem`, and `Switch(login_item: true)` are accepted cleanly as no-ops.

## Packaging and signing

```bash
bern package --target android
bern package --target wearos
```

`bern package` produces both a release **`.apk`** (sideloading/testing) and an **`.aab`** (Android App Bundle for Google Play) under `target/bern/dist/`. When a release builds more than one ABI—including `wearos` by default (`arm64-v8a` and `armeabi-v7a`)—it writes one per-ABI APK (`<app>-<version>-<platform>-<abi>.apk`) alongside the multi-ABI `.aab`. Debug `bern build` and `bern run` still compile only the first ABI in the list.

Without signing variables, the packages are debug-signed (installable for local testing, rejected by app stores). Set these environment variables for release signing:

```bash
export BERN_ANDROID_KEYSTORE="/path/to/release.jks"
export BERN_ANDROID_KEYSTORE_PASSWORD="store-password"
export BERN_ANDROID_KEY_ALIAS="release"
# Optional: defaults to BERN_ANDROID_KEYSTORE_PASSWORD when omitted
export BERN_ANDROID_KEY_PASSWORD="key-password"

bern package --target android
```
