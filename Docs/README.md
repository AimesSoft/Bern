**English** · [简体中文](zh-CN/README.md)

# Bern documentation

Updated: 2026-10-25. To get started, use the [quick start in the root README](../README.md#quick-start); buildable examples are in [examples](../examples/). Look up the current interfaces and platform status starting from the reference documents below.

## Current interfaces

- [Component usage](Reference/UI.md): text and formatting, the player coordinate scene, control groups and sizes, controlled input, images and the masonry layout, Hero, onboarding, search, the idle state, keyboard and keyboard shortcuts, the menu bar, and storage.
- [Complete component property table](Reference/Component-Properties.md): all built-in components, property types, required properties, children and action rules, exported from the current `bern-ir::spec`.
- [Platform capabilities and verification scope](Reference/Platforms.md): support listed by backend feature flag, keeping declared support, build verification and interaction verification apart.
- [Performance tracing](Reference/Tracing.md): the rules of `BERN_TRACE` and `trace.*`, what each segment means, the terminal summary, `trace.report` and the timeline file.

## Architecture and design

- [Architecture and semantics](Design.md): component declarations, modules, theme, commands, windows, media, app icons and clearly marked future designs.
- [Platform adaptation guidelines](Adaptation/README.md): Apple, Android, Windows / Linux, the Web, and platforms under research; an adaptation goal does not mean it is implemented today.
- [React Native research](Research/ReactNative.md): a comparison of the runtime-bridge and build-time-generation architectures.

## Implementation and topics

- [macOS](Implementation/macOS-SwiftUI.md), [iOS](Implementation/iOS-SwiftUI.md), [tvOS](Implementation/tvOS-SwiftUI.md), [Android](Implementation/Android-Compose.md), [Windows](Implementation/Windows-WinUI3.md), [Linux](Implementation/Linux-GTK.md), [Web](Implementation/Web.md): implementation details, past verifications and limitations.
- [GPUI](Implementation/GPUI.md): the self-drawn desktop target that is chosen explicitly, first version (macOS, Windows, Linux): dependency versions, structure, window frames and video surfaces, implemented components and measurements.
- [Animated SVG](Implementation/Animated-SVG.md): the optional component's interface, resources, dependencies, static fallback and verification scope.
- [JavaScript and target capabilities](Implementation/JavaScript.md): conditional builds for WebView / browser and Boa, the script context and capability branches.
- [Building by module](Implementation/Module-Builds.md): `--exclude-module`, compile-time UI conditions and optional Cargo dependencies.
- [The app's own browser extensions](Implementation/Web-Extensions.md): `web_extension` for the browser target, the app's own media executor, WASM imports and the file preparation hook.

## Update rules

When a component or property changes, update the property table and the usage guide; when a backend gains a feature, update the capability table; acceptance records state the date, target, scenario and what was not verified. Successful code generation, a successful native build and passing interaction on a real device are three different conclusions. After adding a record, also check the older "not yet implemented" and "not verified" entries, so that the same capability does not have conflicting states across the documentation.

The interfaces come from the [bern-ir component definitions](../crates/bern-ir/src/spec.rs), and the capabilities come from [bern-target-api](../crates/bern-target-api/src/lib.rs) (the check rules are in [check/](../crates/bern-target-api/src/check/)) and each backend's TargetDescriptor. The implementation logs keep historical context and cannot override the interface facts in the current reference tables.

The documentation is in two languages. English is the default version, in `Docs/` and in `README.md` and `CONTRIBUTING.md` at the root; Simplified Chinese is in `Docs/zh-CN/` (with the same file names as the English) and in `README.zh-CN.md` and `CONTRIBUTING.zh-CN.md`. A change to one page changes both languages in the same commit: headings, table rows and code blocks correspond one to one, and relative links and anchors resolve. `cargo test -p bern --test docs` checks this.
