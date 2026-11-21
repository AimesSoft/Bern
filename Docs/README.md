**English** · [简体中文](zh-CN/README.md)

# Bern documentation

Updated: 2026-11-21. To install `bern` and build your first app, start from the [quick start in the root README](../README.md#quick-start); complete buildable apps live in [examples/](../examples/).

## Where to start

| Goal | Read first |
| --- | --- |
| Build a UI with standard controls, layouts, skeletons or `NativeView` | [Component usage](Reference/UI.md) and [Component property table](Reference/Component-Properties.md) |
| Check what each backend supports and how it has been verified | [Platform capabilities and verification scope](Reference/Platforms.md) |
| Embed JavaScript plugins or script automation with `bern.cli` | [JavaScript and `bern.cli`](Implementation/JavaScript.md) |
| Embed platform-specific native controls beside Rust UI files | [Co-located native views](Implementation/Native-Views.md) |
| Target GPUI or a specific OS backend | [GPUI](Implementation/GPUI.md) or the platform notes below |
| Understand the architecture, protocol, theming and command model | [Architecture and semantics](Design.md) |

## Current interfaces

- [Component usage](Reference/UI.md): text and ICU formatting, coordinate scenes, control groups, engine-controlled inputs, images and masonry grids, Hero transitions, onboarding, search, idle state, keyboard shortcuts, menu bar, storage, automatic skeletons, `NativeView`, and the full standard control catalog (`TreeView`, `SplitPane`, `Reorderable`, `BottomSheet`, `Refreshable`, `DatePicker`, `TimePicker`, `NumberInput`, `Alert`, `Banner`, `Disclosure`, `Tooltip`, `EmptyState`, `Badge`, `Avatar`).
- [Complete component property table](Reference/Component-Properties.md): all built-in components, property types, required properties, children and action rules, exported from `bern-ir::spec`.
- [Platform capabilities and verification scope](Reference/Platforms.md): support listed by backend feature flag and cross-platform capability, keeping declared support, build verification and real-device interaction verification apart.
- [Performance tracing](Reference/Tracing.md): `BERN_TRACE` and `trace.*` rules, segment definitions, terminal summaries, `trace.report` and timeline exports.

## Architecture and design

- [Architecture and semantics](Design.md): component declarations, modules, theme, commands, windows, media, app icons and clearly marked future designs.
- [Platform adaptation guidelines](Adaptation/README.md): Apple, Android, Windows / Linux, the Web and platforms under research; an adaptation goal does not mean a backend is implemented today.
- [React Native research](Research/ReactNative.md): a comparison of runtime-bridge and build-time-generation architectures.

## Implementation and topics

### Platform backends

- **Apple**: [macOS (SwiftUI)](Implementation/macOS-SwiftUI.md) · [iOS / iPadOS (SwiftUI)](Implementation/iOS-SwiftUI.md) · [watchOS (SwiftUI)](Implementation/watchOS-SwiftUI.md) · [tvOS (SwiftUI)](Implementation/tvOS-SwiftUI.md)
- **Desktop & mobile**: [Windows (WinUI 3 & XAML Islands)](Implementation/Windows-WinUI3.md) · [Linux (GTK 4)](Implementation/Linux-GTK.md) · [Android & Wear OS (Jetpack Compose)](Implementation/Android-Compose.md)
- **Optional GPU & Web**: [GPUI (macOS, Windows, Linux)](Implementation/GPUI.md) · [Web app & WebUI (DOM + WebAssembly)](Implementation/Web.md)

### Focused topics

- [Co-located native views](Implementation/Native-Views.md): `NativeView` for placing per-backend native UI files beside Rust declarations with typed `Props`/`Actions`/`Theme`, mandatory CLI entries, `tr!` localization linting and `Fallback`.
- [JavaScript and `bern.cli`](Implementation/JavaScript.md): opt-in `javascript = true` targets (in-process Boa on all native targets, Web Worker on the Web), persistent script contexts, in-process `globalThis.bern.cli(...)`, host-side allowlist/denylist `CliPolicy` rules and `window.webview` branches.
- [Animated SVG](Implementation/Animated-SVG.md): pure-Rust `BSVG` bytecode rendering with zero WebView dependency, resource declarations, static fallback and verification scope.
- [Building by module](Implementation/Module-Builds.md): `--exclude-module`, compile-time UI conditions and optional Cargo dependencies.
- [The app's own browser extensions](Implementation/Web-Extensions.md): `web_extension` for the browser target, custom media executors, WASM imports and file preparation hooks.

## Update rules

When a component or property changes, update the property table and the usage guide; when a backend gains a feature, update the capability table; acceptance records state the date, target, scenario and what was not verified. Successful code generation, a successful native build and passing interaction on a real device are three different conclusions. After adding a record, also check older "not yet implemented" and "not verified" entries so the same capability does not have conflicting states across the documentation.

The interfaces come from [bern-ir component definitions](../crates/bern-ir/src/spec.rs), and the capabilities come from [bern-target-api](../crates/bern-target-api/src/lib.rs) (check rules in [check/](../crates/bern-target-api/src/check/)) and each backend's `TargetDescriptor`. Implementation logs preserve historical context and cannot override the interface facts in the current reference tables.

The documentation is maintained in two languages. English is the default under `Docs/`, `README.md` and `CONTRIBUTING.md`; Simplified Chinese lives under `Docs/zh-CN/` (with matching file names), `README.zh-CN.md` and `CONTRIBUTING.zh-CN.md`. Every change updates both languages in the same commit with matching headings, table rows, code blocks, links and anchors, verified by `cargo test -p bern --test docs`.
