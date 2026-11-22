**English** · [简体中文](../zh-CN/Topics/Eject.md)

# Ejecting to standalone native projects (`bern eject`)

Updated: 2026-11-22. `bern eject` emits standalone, standard native IDE projects that no longer depend on `bern-cli`, the Bern compiler, or the original workspace. It acts as a zero-lock-in escape hatch: start with a single Rust UI declaration in Bern, and at any point export clean platform-native projects to open, edit, sign, extend, or hand off directly in Xcode, Android Studio, Visual Studio, Cargo, or any static Web server.

The original Bern project is never modified. Ejected projects are written to `ejected/<target>` by default (or `--out <dir>`).

## Usage

```sh
# Eject every declared target with an offline self-contained Rust workspace (cross-platform from any host)
bern eject

# Eject one target to a chosen directory
bern eject --target macos --out ./native-macos

# Eject with a pre-compiled engine binary so the native project builds without Rust installed
bern eject --target ios --engine prebuilt --release --out ./native-ios

# Keep the application CLI, IPC control channel, and SKILL.md
bern eject --target macos --with-cli

# Replace a non-empty destination directory
bern eject --target windows --out ./native-win --force
```

### Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--manifest PATH` | Nearest `Bern.toml` | Application manifest to eject |
| `--target NAME` | All targets | Target name from `[targets.<name>]` |
| `--out DIR` | `<app>/ejected/<target>` | Output directory (multi-target eject writes `<DIR>/<target>`) |
| `--engine source\|prebuilt` | `source` | `source` vendors an offline Rust workspace wired into the native IDE build; `prebuilt` compiles and embeds the engine binary |
| `--release` | Debug | Build optimized binaries in `--engine prebuilt` mode |
| `--with-cli` | Off | Retain the IPC channel, application CLI source, and `SKILL.md` |
| `--exclude-module LIST` | None | Omit optional module ids, matching `bern build --exclude-module` |
| `--force` | Off | Allow overwriting a non-empty output directory |

## Two engine delivery modes

### 1. Offline self-contained Rust source (`--engine source`, default)

Every local crate reachable from the application's engine — `protocol`, `engine`, enabled `modules/*`, local workspace helper crates, and the exact subset of Bern runtime crates used by that target — is copied into the ejected project. All `workspace = true` inheritances are expanded, all local paths are rewritten to relative paths inside the ejected tree, `Cargo.lock` and `rust-toolchain.toml` are carried along, and external `dev-dependencies` are pruned.

The native build system invokes `cargo` automatically when building in the IDE:

- **Apple (`.xcodeproj`)**: a **Build Rust Engine** Run Script phase runs `scripts/build-rust.sh` for Xcode's active SDK, architecture(s), and Debug/Release configuration.
- **Android / Wear OS (Gradle)**: `app/build.gradle.kts` registers `buildRustEngineDebug` / `buildRustEngineRelease` before `merge*JniLibFolders`, using Android Studio's configured NDK.
- **Windows (Visual Studio `.sln` + `.vcxproj`)**: MSBuild runs `scripts\build-rust.cmd` before linking, matching the active `x64`/`ARM64` platform and `Debug`/`Release` configuration.
- **Linux (GTK 4) & Desktop (GPUI)**: the ejected folder itself is a standalone Cargo workspace (`cargo run`).
- **Web (DOM + WASM)**: `scripts/build-wasm.sh` and `scripts/build-wasm.cmd` compile `rust/page` to `app.wasm`.

Because `--engine source` does not invoke target platform compilers during ejection, any host OS can eject projects for every platform (for example, ejecting Windows, Linux, Android, Apple, and Web projects on macOS).

### 2. Prebuilt engine binary (`--engine prebuilt`)

For non-Rust front ends (Apple SwiftUI, Android / Wear OS Compose, Windows WinUI 3 / Islands, Web), `--engine prebuilt` compiles the Rust engine during ejection, embeds only the resulting native binary, and discards all temporary Rust staging files. The ejected project opens and builds in Xcode, Android Studio, Visual Studio, or a browser with **no Rust toolchain installed**.

Targets whose UI front end is itself written in Rust (`gtk + linux` and `gpui + *`) require Cargo regardless of the engine and therefore only support `--engine source`.

## Project structure per platform

Generated UI sources are split by concern and stripped of `Do not edit` banners so each component and window lives in its own file while keeping 100% behavioral parity with Bern. Co-located `NativeView` files and declared resources are copied directly into the ejected project with zero references back to the original workspace.

| Platform | Native project entry | Split UI layout | Engine (`source` / `prebuilt`) |
| --- | --- | --- | --- |
| **macOS / iOS / tvOS / watchOS** | `<Product>.xcodeproj` + shared `.xcscheme` | `Sources/{App,Models,Styles,Views,Windows,Native,Runtime}/` | `rust/` + `scripts/build-rust.sh` / `Frameworks/BernEngine.xcframework` |
| **Android / Wear OS** | Root Gradle project (`settings.gradle.kts`, `app/build.gradle.kts`, `gradle-wrapper.properties`) | `app/src/main/kotlin/<pkg>/{MainActivity,Models,Styles,AppState}.kt`, `views/`, `windows/`, `dev/bern/runtime/` | `rust/` + Gradle NDK task / `app/src/main/jniLibs/<abi>/libbern_app.so` |
| **Windows (WinUI 3 / Islands)** | `<Product>.sln` + `<Product>.vcxproj` + `.vcxproj.filters` | `src/catalog/`, `src/ui/{app_ui,styles}.inl`, `src/ui/{views,windows}/`, `src/native/`, `src/runtime/` | `rust/` + `scripts\build-rust.cmd` / `lib\<arch>\bern_app.lib` |
| **Linux (GTK 4)** | Standalone `Cargo.toml` | `src/main.rs`, `src/ui/{mod,styles,strings}.rs`, `src/ui/{views,windows,native}/` | Self-contained Cargo workspace (`source` only) |
| **Desktop (GPUI)** | Standalone `Cargo.toml` (with vendored GPUI patches) | `src/main.rs`, `src/ui/{mod,styles,strings}.rs`, `src/ui/{views,windows,native}/` | Self-contained Cargo workspace (`source` only) |
| **Web (DOM + WASM)** | Zero-bundler `index.html` + native ES modules | `src/{app,models,styles}.js`, `src/{views,windows,native}/`, `runtime/`, `styles/` | `rust/` + `scripts/build-wasm.{sh,cmd}` / `app.wasm` |

## Clean defaults & protocol evolution

- **Stripped dev scaffolding by default**: `bern eject` omits the IPC control channel, application CLI binaries, `SKILL.md`, `build.json`, and `features.json` unless `--with-cli` is passed.
- **Relaxed protocol fingerprint check**: `bern_ffi::start_app` accepts `fingerprint == 0` to bypass strict catalog hash validation. Ejected projects pass `0` so you can freely add fields, commands, or native extensions in the ejected Rust backend without hitting a catalog mismatch error at launch.
- **Windows SDK preparation**: `scripts\prepare-sdk.cmd` uses Windows' built-in `curl.exe`, `tar.exe`, and `cppwinrt.exe` (no PowerShell) to populate `.sdk-cache\` on first Visual Studio build.

## Verification scope

- **Workspace unit & integration checks (verified 2026-11-22)**: `bern-ffi`, `bern-target-api`, all six backend crates (`bern-backend-swiftui`, `bern-backend-compose`, `bern-backend-winui3`, `bern-backend-gtk`, `bern-backend-gpui`, `bern-backend-web`), `bern-cli`, and `bern --test docs` pass (`fmt`, `clippy -D warnings`, offline bundling, zero leaked paths, zero `Do not edit`, zero PowerShell on Windows, unique Swift/Kotlin basenames across split files).
- **macOS Xcode builds (verified 2026-11-22)**: ejected `examples/hello` (`--engine source` and `--engine prebuilt` built with `xcodebuild` under a minimal `PATH` without Rust/Cargo) and `examples/showcase` (`--engine source`, multi-component split + nested `media/` & `images/` folder references + localizations) built end-to-end into runnable `.app` bundles with `xcodebuild`.
- **Not yet verified on native hosts**: opening and compiling ejected projects on iOS/tvOS/watchOS Xcode simulators, Android Studio / Gradle, Visual Studio 2022 on Windows, Linux GTK 4, GPUI standalone Cargo builds, and live browser serving.
