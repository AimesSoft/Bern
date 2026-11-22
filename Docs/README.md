**English** · [简体中文](zh-CN/README.md)

# Documentation

Start with the **Guides** to build an application in Rust, check the **Platform guides** when configuring or packaging a specific OS, and use **Reference** when looking up exact DSL syntax, component properties, or platform capability coverage.

## Quick directory by task

| Task | Where to read |
| --- | --- |
| Build your first Bern app from scratch | [Quick start](Guides/Quick-Start.md) |
| Define state, commands, queries, and errors in Rust | [Protocol and engine](Guides/Protocol-and-Engine.md) |
| Build responsive layouts, navigation, modals, and TV/Watch views | [Layouts and navigation](Guides/Layouts-and-Navigation.md) |
| Add translations (`tr!`), themes, and custom styles (`style!`) | [Localization and styling](Guides/Localization-and-Styling.md) |
| Manage multi-window apps, icons, tray items, files, and media | [Windows, resources, and files](Guides/Windows-Resources-and-Files.md) |
| Automate, screenshot, and drive your app via CLI & AI Skills | [App CLI and Skills](Guides/App-CLI-and-Skills.md) |
| Eject to standalone native IDE projects with zero Bern lock-in | [Ejecting to standalone native projects](Topics/Eject.md) |
| Set up toolchains, debug, sign, and package for a target OS | [Platform guides](#platforms) |
| Look up exact component properties or platform feature flags | [Reference](#reference) |
| Understand the compiler pipeline and framework invariants | [Architecture overview](Architecture.md) |

## Overview

- [Architecture overview](Architecture.md) — Core compiler pipeline (`#[bern::protocol]` / `ui!` → IR → native targets), engine boundary, and framework invariants.

## Guides

- [Quick start](Guides/Quick-Start.md) — Install `bern`, scaffold a project with `bern new`, understand the 3-crate layout, and run the `check` / `run` / `package` loop.
- [Protocol and engine](Guides/Protocol-and-Engine.md) — `#[bern::protocol]`, `#[bern::session]`, `#[bern::value]`, `Query`, background tasks, storage, and secrets.
- [Layouts and navigation](Guides/Layouts-and-Navigation.md) — `Adaptive` size classes (`compact`, `medium`, `expanded`, `tv`, `watch`), `SplitView`, `TabView`, `Modal`, `Dialog`, `Shortcut`, and `default_focus`.
- [Localization and styling](Guides/Localization-and-Styling.md) — `tr!`, `strings/*.toml`, plural/date formatting, semantic colors, `style!`, and runtime light/dark or palette switching.
- [Windows, resources, and files](Guides/Windows-Resources-and-Files.md) — Multi-window scenes, `MenuBar`, `StatusItem`, `[app.icon]`, `[[app.opens]]`, file picking, and `MediaSource` / `Video`.
- [App CLI and Skills](Guides/App-CLI-and-Skills.md) — Using the generated app CLI, IPC targeting, `window screenshot`, `ui.*` commands, and auto-generated `SKILL.md` files.

## Platforms

Operational manuals covering prerequisites, `Bern.toml` configuration, building/debugging, platform adaptations, and signing/packaging:

- [macOS](Platforms/macOS.md) — SwiftUI + AppKit (`.app`, `.dmg`, notarization)
- [iOS & iPadOS](Platforms/iOS.md) — SwiftUI + UIKit, dual frontends (`swiftui` + `swiftui-16`), simulator & `.ipa`
- [watchOS](Platforms/watchOS.md) — Watch-Only SwiftUI, `Layout(size: watch)` (`B401`), `.ipa`
- [tvOS](Platforms/tvOS.md) — Apple TV SwiftUI, Siri Remote focus, Top Shelf icons, `.ipa`
- [Windows](Platforms/Windows.md) — C++/WinRT dual frontends (WinUI 3 + XAML Islands), Direct2D installer & `.zip`
- [Linux](Platforms/Linux.md) — GTK 4 + libadwaita, StatusNotifierItem, `.deb` & `.tar.gz`
- [Android & Wear OS](Platforms/Android.md) — Jetpack Compose Material 3 & Wear Compose, SAF, `.apk` & `.aab`
- [GPUI](Platforms/GPUI.md) — Optional unified GPU-rendered desktop backend (`ui = "gpui"`)
- [Web](Platforms/Web.md) — Standalone WASM App (`web_mode = "app"`), native-backed WebUI (`web_mode = "ui"`), and embedded `web_ui`

## Topics

- [Ejecting to standalone native projects](Topics/Eject.md) — `bern eject` emits standalone `.xcodeproj`, Gradle, Visual Studio `.sln`/`.vcxproj`, Cargo, and zero-bundler ES module projects (`--engine source` or `--engine prebuilt`).
- [Native views](Topics/Native-Views.md) — Co-located per-platform native views (`NativeView`) with typed props, `tr!` checks, CLI entries, and `Fallback`.
- [JavaScript and `bern.cli`](Topics/JavaScript.md) — Opt-in in-process JavaScript (`bern::script`), `bern.cli(...)`, and host-side `CliPolicy` rules.
- [Animated SVG](Topics/Animated-SVG.md) — Build-time `BSVG` bytecode compilation and pure-Rust animation playback across all targets.
- [Module builds](Topics/Module-Builds.md) — Splitting features into independent modules (`bern new --module`, `--exclude-module`).
- [Web extensions](Topics/Web-Extensions.md) — Integrating custom JS/WASM browser extensions with `web_extension`.

## Reference

- [UI declaration reference](Reference/UI.md) — `ui!` syntax, expressions, navigation, formatting, motion, and automatic skeleton screens.
- [Component property table](Reference/Component-Properties.md) — Exhaustive property, child, and action reference for every built-in component.
- [Platform capabilities and verification scope](Reference/Platforms.md) — Feature-flag support matrix across targets and current verification notes.
- [Performance tracing](Reference/Tracing.md) — `BERN_TRACE`, `bern run --trace`, `trace.*` CLI commands, and timeline output format.

## Update rules

- Whenever a component, property, target option, or `Bern.toml` key changes, update the matching guide, platform manual, and [Reference](#reference) page in the same change.
- Keep `Docs/` and `Docs/zh-CN/` synchronized 1-to-1 in structure (headings, code blocks, and tables) and verify with `cargo test -p bern --test docs`.
- Keep operational guides grounded in current framework behavior rather than historical design plans or private app names.
