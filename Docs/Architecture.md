**English** · [简体中文](zh-CN/Architecture.md)

# Architecture & Core Invariants

This document summarizes the architectural boundaries and invariants that govern Bern's compiler, runtime, and backends. For day-to-day application development, start with the [Guides](Guides/Quick-Start.md), [Platform manuals](Platforms/macOS.md), and [Reference](Reference/UI.md).

## 1. Compile-time native UI generation

Bern is a cross-platform native UI framework for Rust. Developers write their protocol, business engine, and declarative `ui!` views in `.rs` files. At build time, `bern build` compiles `ui!` declarations into a versioned intermediate representation (`bern-ir`) and hands that IR to independent backend modules (`bern-target-api`), each generating native UI code for its platform:

- **Apple (SwiftUI)**: macOS, iOS / iPadOS, watchOS, tvOS
- **Windows (C++/WinRT)**: WinUI 3 with automatic system XAML (`xaml-islands`) fallback
- **Linux**: GTK 4 + libadwaita
- **Android & Wear OS**: Kotlin + Jetpack Compose (Material 3 / Wear Compose)
- **Desktop GPU (optional)**: GPUI (macOS, Windows, Linux)
- **Web**: DOM + WebAssembly (`web_mode = "app"` or `"ui"`)

Bern does not embed a browser or JS runtime for UI rendering, and does not transpile arbitrary Rust into Swift, C++, or Kotlin. Adding a new backend never touches the parser, IR, or application logic.

## 2. Three-layer application boundary

Every Bern application separates into three distinct layers:

| Layer | Responsibility | Hard invariant |
| --- | --- | --- |
| **Protocol** (`#[bern::protocol]`) | Value types, read-only state snapshot, typed commands, queries, and events | Single source of truth for state schema and command catalog |
| **Engine** (plain Rust) | Implements command/query traits and owns business state | Never depends on windows, controls, or transports; runs headlessly under `cargo test` |
| **Interface** (`#[component]` + `ui!`) | Declarative UI structure, local window drafts (`#[bern::session]`), and bindings | Restricted analyzable syntax; cannot mutate engine state directly or attach arbitrary Rust closures to buttons |

## 3. Command-centric multi-entry runtime

UI controls, the generated app CLI, the local JSON/IPC control channel, WebUI sessions, and opt-in in-process JavaScript (`bern.cli`) all drive the exact same engine through one typed command catalog:

- **Every actionable control must bind a command** (engine command or generated UI session command); unbound buttons fail compilation (`B301`).
- **Commands execute actions**, **Queries remain read-only**, and **Events report facts** — side effects and reads are never mixed.
- **No automation black boxes**: even co-located platform code (`NativeView`) must declare typed CLI-operable controls so external tools and AI agents can inspect and drive every interactive surface semantically.

## 4. Platform fidelity & native-first theming

- **Native controls by default**: on system targets (SwiftUI, WinUI 3, GTK 4, Compose), generated code uses standard platform containers, controls, typography, IME, accessibility, and system light/dark colors. Self-drawn UI is reserved for backends without a system design language (Web, GPUI) or explicit `Canvas` / `style!` declarations.
- **Explicit target selection**: omitting `ui` selects the platform default. If the default backend is unavailable or missing a required capability, builds fail loudly rather than silently falling back to another UI stack.
- **Zero-setup light and dark mode**: every control and theme token adapts to system appearance automatically. Native targets consume OS semantic colors; Web and GPUI consume snapshots derived by `bern-theme` with checked contrast ratios.

## 5. Static guarantees & diagnostics

Omissions that normally leak to runtime are enforced at compile time by `bern check` and `rustc`:

- **Type safety**: `ui!` expands into type-checked Rust constraints so `rustc` verifies every state read, draft binding, and command argument.
- **Localization**: user-visible text uses `tr!`; missing languages (`B102`) and hardcoded display literals (`B103`, strict mode on by default) fail compilation.
- **Adaptive feature coverage**: `#[bern::features]` contracts require the compiler to prove that every declared feature has a reachable entry across required size classes (`B201`–`B204`) and write per-target `features.json` coverage reports.
- **Compile-time pruning**: device-fixed conditions (`window.tv`, `window.watch`, `window.webview`, `--exclude-module`) prune unreachable branches before capability checks and code generation.

## 6. Unified multi-window & deployment rules

- **Declarative multi-window**: `WindowSet` and `Window` declare windows once. Platforms with native multi-window (macOS, Windows, Linux, iPadOS) create native OS windows; single-scene platforms (iPhone, Android, Web, watchOS, tvOS) map secondary windows to platform-idiomatic virtual windows (sheets, full-screen covers, dialogs) while preserving identical window identity, close confirmation, and CLI commands.
- **Opt-in scripting**: applications without `javascript = true` link zero JS engine. Enabling scripting embeds in-process Boa on native targets (Web Worker on Web) gated by host-side `CliPolicy` rules.
- **Self-contained media & assets**: bundled resources are explicitly declared in `Bern.toml` (`[resources]`) and resolved via `Resources` without assuming OS bundle layouts; `MediaSource` unifies local paths, bundled resources, Android `content://` URIs, and Web `blob:` URLs.
