**English** · [简体中文](../zh-CN/Guides/Quick-Start.md)

# Quick Start

This guide takes you from installing `bern` to creating, checking, running, and packaging a native Rust application.

## Prerequisites

- **Rust 1.99** (`rustup` installs it automatically when `rust-toolchain.toml` is present).
- The platform toolchain for whichever target you want to build (Xcode for Apple platforms, Visual Studio C++ Build Tools for Windows, GTK 4 + libadwaita dev packages for Linux, Android Studio SDK/NDK + Gradle for Android, or `wasm32-unknown-unknown` for Web). See the [Platforms](../Platforms/macOS.md) manuals for exact installation steps.

## 1. Install `bern-cli`

Install the prebuilt binary with [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):

```sh
cargo binstall bern-cli
```

Or compile from source:

```sh
cargo install bern-cli
```

Verify what this machine can build right now:

```sh
bern doctor
```

`bern doctor` checks Rust, SDKs, simulators, and Rust cross-compilation targets, and prints exact install instructions for anything missing.

## 2. Create a new project

```sh
bern new my_app
cd my_app
```

Optional flags:

- `--bundle-id com.example.myapp` — reverse-DNS identifier (default: `com.example.<name>`).
- `--languages en,zh-Hans` — comma-separated supported languages, default first (template includes `en`, `zh-Hans`, `zh-Hant`, `ja`).
- `--bern-path /path/to/Bern` — point the generated workspace at a local Bern checkout instead of crates.io.

The generated Cargo workspace has a clear three-crate layout:

```text
my_app/
├── Bern.toml            # App metadata, languages, resources & targets
├── Cargo.toml           # Workspace manifest
├── my_app-protocol/     # Value types, read-only state & command traits
├── my_app-core/         # Plain Rust business engine (zero UI dependency)
└── my_app-ui/           # Declarative #[component] + ui! views
```

## 3. Check, run, and build

### Compile-time check

```sh
bern check
```

`bern check` parses `Bern.toml`, lowers `ui!` declarations into IR, checks translation completeness (`B102`/`B103`), verifies adaptive feature contracts (`B201`–`B204`), and checks target capabilities without running a full native link. Pass `--target <name>` to check one target.

### Run locally

```sh
bern run
```

Without `--target`, `bern run` builds and launches your host OS native target (SwiftUI on macOS, WinUI 3 on Windows, GTK 4 on Linux). Pass `--target` to run any other target declared in `Bern.toml`:

```sh
bern run --target web_app           # Opens in browser (use --port 8080 to keep storage)
bern run --target ios               # Installs & launches in the booted iOS simulator
bern run --target android           # Installs & launches on USB-connected device or emulator
bern run --target gpui_macos        # Runs on the optional GPUI desktop renderer
```

Useful `bern run` flags:

- `--release` — optimized build.
- `--port <1-65535>` — fixed local HTTP port for browser targets (preserves browser storage across runs).
- `--trace <RULES>` — enable performance tracing (for example `--trace engine,ui,slow:16ms`; see [Tracing](../Reference/Tracing.md)).
- `--exclude-module <ids>` — omit optional modules at compile time (see [Module Builds](../Topics/Module-Builds.md)).

### Build without launching

```sh
bern build                          # Builds every target this machine can build
bern build --target macos --release # Builds one target in release mode
```

Build outputs land in `target/bern/<target>/`, including the generated native app, its companion CLI under `bin/`, `features.json`, and `skills/<app>/SKILL.md` when IPC is enabled.

## 4. Package for release

```sh
bern package                        # Packages every target this machine can build
bern package --target macos         # Packages one target
```

Release artifacts are written to `target/bern/dist/`:

| Platform | Artifacts in `target/bern/dist/` |
| --- | --- |
| macOS | `<App>-<version>-macos-<arch>.dmg` |
| iOS / watchOS / tvOS | `<App>-<version>-<platform>.ipa` |
| Windows | `<App>-<version>-windows-<arch>-setup.exe` and portable `.zip` |
| Linux | `<app>_<version>_<arch>.deb` and `.tar.gz` |
| Android / Wear OS | `.apk` and Play Store `.aab` |
| Web (`app` / `ui`) | Static site `.zip` / `.tar.gz` (plus host & CLI for `web_ui`) |

Signing credentials are read from environment variables (never committed to your repo); see each platform manual under [Platforms](../Platforms/macOS.md) for signing details.

## Next steps

- Learn how the three crates connect in [Protocol & Engine](Protocol-and-Engine.md).
- Explore [Layouts, Navigation & Feature Contracts](Layouts-and-Navigation.md).
- Browse working multi-page examples in [examples/](../../examples/) (`showcase`, `planner`, `gallery`, `native-controls`, `wearos`, `svg-animation`).
