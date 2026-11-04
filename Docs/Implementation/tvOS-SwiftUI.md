**English** · [简体中文](../zh-CN/Implementation/tvOS-SwiftUI.md)

# tvOS SwiftUI implementation notes

For the current component interface see the [component reference](../Reference/UI.md); for per-item support and the recent verification scope see the [platform capability table](../Reference/Platforms.md); for the official basis on Apple platforms see the [Apple adaptation guidelines](../Adaptation/Apple.md).

- Date: 2026-10-20
- Basis: [Bern architecture design v0.33](../Design.md)
- Toolchain: Rust 1.99.0 (`aarch64-apple-tvos-sim`); Xcode 27.0, tvOS 27.0 simulator SDK
- Verification environment: Apple TV 4K (3rd generation) (1080p) simulator, tvOS 26.5 (M4 Mac)

This document records what was actually completed for the `swiftui + tvos` target. It shares the generator and the runtime with iOS (`Platform::Tvos`); on a TV it is a single scene that fills the screen, operated through the focus engine and the Siri Remote, with no touch and no pointer. This version builds a simulator app; the device target is not done.

## Completion status

| Design requirement | Status | Notes |
| --- | --- | --- |
| Platform resolution | Done | `platform = "tvos"` defaults to `ui = "swiftui"`; `bern doctor`, `bern build` and `bern run` recognize this target |
| Generated code builds | Done | `bern build --target tvos` generates Swift and the glue crate, and builds the Apple TV simulator app bundle plus the CLI and MCP server on the Mac; the generation tests type-check against the tvOS simulator SDK |
| Build only the part the TV shows | Done | When `Adaptive` has a `Layout(size: tv)`, only that one is kept, `window.tv` is true, and the window size class is determined by the screen; branches and components that are never shown are not generated and do not take part in the capability check (see "TV layout") |
| Focus and remote navigation | Done | The system focus engine and system focus effects; `default_focus` sets the starting focus; a slider is a focusable track |
| Remote buttons | Partial | `Shortcut` and a control's `shortcut:` map to back, play/pause and the edges of the touch surface; the arrow keys were measured in the simulator, back and play/pause are not verified (see "Verification log") |
| App icon | Done | `[app.icon].tvos` accepts a `.brandassets` folder or a square PNG; when none is declared, a layered icon and the Top Shelf image are generated from the default icon |
| `bern run --target tvos` | Done | Installs into the running tvOS simulator and launches; the generated CLI controls it over IPC |
| Device build and packaging | Partial | `bern package --target tvos` builds the engine for `aarch64-apple-tvos` and writes an unsigned `<app>-<version>-tvos.ipa` to `target/bern/dist/`, to be signed and installed by whoever has the profile; signing itself is not done |

## Toolchain

With Rust 1.99.0, rustup provides prebuilt standard libraries for `aarch64-apple-tvos` and `aarch64-apple-tvos-sim` (tier 2, no host tools), so after `rustup target add aarch64-apple-tvos-sim` the stable version pinned by the workspace builds it. Nightly's `-Zbuild-std` is therefore not used: it would require switching to a nightly toolchain and recompiling the standard library on every build, inconsistent with the 1.99.0 used by the other targets, and there is no problem here that it would solve and the prebuilt standard library would not. The corresponding simulator target on an Intel Mac is `x86_64-apple-tvos`.

Minimum tvOS 18.0: Observation, the switch-style `Toggle` and the new tab API that the runtime uses are all available from this version on; tvOS 18 supports the same devices as tvOS 17 (Apple TV HD and every Apple TV 4K).

## Project structure

| Location | Contents |
| --- | --- |
| `crates/bern-backend-swiftui/src/tvos.rs` | tvOS target: target description, checks, generation and build steps |
| `crates/bern-ir/src/television.rs` | `Application::on_television`: the app as shown on a TV |
| `crates/bern-target-api/src/brand_assets.rs` | Layered icons and Top Shelf images: checking, generation |
| `templates/BernUIKit.swift` | Platform part shared with iOS; inside `#if os(tvOS)` are the remote buttons, the focus slider and the windows' remote commands |
| `templates/BernStyle.swift` | Focus effects, control appearance in scaled scenes, default focus |

As on iOS, the app bundle has a flat structure. `Info.plist` declares `UIDeviceFamily` 3, `AppleTVSimulator`, a single scene, the icon name `App Icon` and the Top Shelf image `TVTopShelfImage`.

## Differences from iOS

- **Scenes and windows**: one `WindowGroup` whose content is `BernVirtualHost`, with the main window filling the screen; other windows (including settings) open as a `fullScreenCover` that fills the screen, with no title bar and no "Done" button (tvOS's own full-screen pages have none either), and are closed with the Menu button (back); when a close button is needed, the window content draws it itself. A tvOS sheet is a small card in the middle of the screen, too small for forms and split views. Window facts report `Virtual`. There are no iOS orientations, status bar, iPad multiple windows or ⌘, menu. A TV is always landscape, so `Window(landscape:)` does not apply: the capability table declares `window_landscape`, the runtime does nothing, and the check no longer warns (2026-10-24).
- **Body text size**: tvOS body text is 29 points, and the generated default font size follows it at 29 (17 on iOS). When the app gives a TV size in a `text` block (`font(size: 13, tv: 16)`), tvOS takes the TV size.
- **Controls**: all are tvOS system controls, which lift, turn white and gain a shadow under focus. Controls that tvOS does not have are replaced the way the system does it:
  - `Slider` is `BernFocusSlider`: a focusable track; while it has focus, each press left or right on the touch surface moves one step (one twentieth of the range when there is no `step`), a run of consecutive presses counts as one edit (ending 0.5 seconds after the last one), and for accessibility it is an adjustable element. The progress bar of `Video` works the same way.
  - The content of `PopoverButton` is shown as a sheet; `Switch(style: button)` is an ordinary switch (tvOS has no button-style switch); `TextInput` and search fields use the system text field, and typing goes through the system keyboard and dictation.
  - `Pinch` and `Swipe` keep only the accessibility adjustment, with no gesture; `Drop` has no drag and drop; content with `reveal: hover` is always shown (the check gives a warning), and focus reaches it like any other control.
  - `Sidebar(width:)` has no effect (the tvOS sidebar is a floating panel of system width), and the check gives a warning; the sidebar's search field is in the system's default position.
  - The tab item of `Tab(display: icon)` holds only a symbol image (its `accessibilityLabel` is `label`), so the tab bar shows only the symbol, for example the magnifying glass for search; when selected, the symbol sits on the system's selection background.
- **Focus effects**: system buttons use the system focus effect. Buttons that their style draws are drawn with the prototype's Apple TV focus (`bernTelevisionFocus`): those whose style has a background only on hover are list rows, which under focus turn white, with the content turning dark, and scale to 1.03; those with their own background (such as the keys of the on-screen keyboard) turn white under focus and scale to 1.1; the rest are images (posters, frames, thumbnails), which scale to 1.08 with a shadow. Nothing is drawn without focus. The `hoverEffect(.highlight)` used at first draws a plate and an outline on tvOS 26 even without focus, which does not match the prototype, and has been replaced. When a style writes `focus: none`, no focus effect is shown, for example for a button that fills the picture and exists only to receive the remote.
- **Scaled scenes**: `ScaleBox` enlarges a scene drawn at the design size to the screen (for example 1280 × 720 enlarged 1.5 times to 1920 × 1080). tvOS controls are designed for 1080p and would become half as large again after enlarging, so the controls in a scene are drawn at the design size:
  - Capsule and round buttons: the design height (46 when no height is written, 52 for large), white background with black text under focus, scaled to 1.1, with a shadow; a `prominence: plain` button is just text (16 points, secondary color) and is wrapped in a white capsule under focus.
  - System text with no font specified takes the app's TV body size (`bernSceneBodySize`); the section headers of forms take the prototype's 12.5-point semibold secondary color, not tvOS's large headings.
  - Text fields and search fields use tvOS's own text field (it draws the plate and the focus), with only the design height and font size set.
  - Menu `Picker`: the tvOS menu picker shows only the current value; on tvOS it becomes a row instead: the label on the left, and at the end of the row a capsule menu with a drop-down arrow (46 points).
  - Segmented `Picker`: the tvOS segmented control cannot be resized, so in a scene it is drawn as the prototype's segments (40 points, 15-point text, the selected segment lifted, the focused segment white and enlarged); outside a scene it is still tvOS's own.
  - Switch: tvOS has no switch, only a button reading "On" or "Off"; it is replaced by a drawn switch (a 64 × 28 capsule filled with the accent color when on, with a white knob), toggled with the remote's select button, with an outline around it and scaled to 1.1 under focus. Outside a scene it is drawn at 1.5 times.
  - Slider: a thin track filled with the accent color up to a white capsule thumb (40 × 24), with an outline around it under focus; a long slider is not enlarged, so that it does not cover the time next to it.
  - `Modal` is lifted to the window root for display and enlarged by the scale factor of the scene it is in.
- **No keyboard shortcuts**: tvOS has no `keyboardShortcut`; `shortcut:` and `Shortcut` become remote buttons instead (see below).

`Svg` is not available on tvOS: animated SVG is played by WebKit, and the tvOS SDK has no WebKit (known; it is a compile error). The checker reports where it is used; the app uses a static image instead in its TV layout.

## TV layout

A TV window is always the whole screen, so window-related conditions are already known at build time. `Application::on_television` rewrites the app before tvOS checking and generation:

- When `Adaptive` has a `Layout(size: tv)`, only that layout is kept; when it has none, a layout is chosen by size class as usual (a TV gets its size class from a width of 1920 points and the breakpoints in `[layouts]`, commonly `large`).
- `window.tv` is true; size classes such as `window.compact`, and `window.short`, are derived from the screen's 1920 × 1080 points and the breakpoints; `!`, `&&` and `||` are simplified using the known parts; conditions known only at runtime, such as `window.idle`, are kept.
- A branch whose condition is known to be false is removed, and a branch known to be true replaces the whole condition; components placed only by the removed parts are not generated.

So when a phone or desktop layout uses a capability the TV does not have (for example `pick:` to pick files), the tvOS check does not report an error; it checks only what the TV actually shows. On other targets, `window.tv` is true on Android TV and in TV web browsers (see the [platform capability table](../Reference/Platforms.md); not verified on TV devices) and false on desktop and iOS, where the `tv` layout is not selected. The remote buttons on Android TV are the same as on tvOS: the back button maps to `escape`, the play/pause button to `space`, and the arrow keys move focus first.

On tvOS, `Stack(under_bars: true)` ignores all container safe areas, and the content extends to the edges of the screen; the TV overscan margins (HIG: 60 points top and bottom, 80 points left and right) are left inside the scene by the design, to avoid leaving the margin a second time outside containers that already have safe areas.

## Remote

On tvOS, `Shortcut(key:)` and a control's `shortcut:` map to buttons on the remote:

| Key | Siri Remote | SwiftUI |
| --- | --- | --- |
| `escape` | Back (Menu) | `onExitCommand` |
| `space` | Play/Pause | `onPlayPauseCommand` |
| `left`, `right`, `up`, `down` | Directions on the touch surface, when focus has nowhere to move | `onMoveCommand` |

Combinations with modifier keys and other keys have no corresponding button on a TV and are not generated. Each window's root view hands these commands to the shortcuts currently shown and enabled in that window; when several have the same key, the last one shown takes it, and disabled ones do not take part; when there is no shortcut for the back button, back goes to the system (closing the full-screen page, returning to the Home screen). The arrow keys reach shortcuts only when focus cannot move, so a design such as "left and right skip back and forward when there is no focusable control on the screen" uses a button that fills the picture with `focus: none` to catch focus. A directional press that no shortcut catches counts as input and sets `window.idle` back to false.

`default_focus: true` is written on a `Button` or `IconButton`, and focus starts from it when the interface appears. On tvOS it is the preferred focus of its focus region (`prefersDefaultFocus`), and each tab is a focus region, so each page under the tab bar has its own starting focus; on other platforms it is keyboard focus (see the capability table).

## App icon

The `tvos` key of `[app.icon]`:

- A `.brandassets` folder is compiled as is: the check confirms that the assets listed in `Contents.json` are all present, that each image stack has 2 to 5 layers, that each layer's image exists, and that there is a 400 × 240 primary app icon.
- A square PNG, or the default icon when none is declared: a layered icon is generated. The back layer is the most common color among the icon's opaque pixels (quantized to 5 bits), and the front layer is the icon on a transparent background; when the icon itself is a full-bleed square it fills the height, and when it has transparent edges a margin is left according to the safe area. This generates the 400 × 240 (@1x, @2x) Home screen icon, the 1280 × 768 App Store icon, and the 1920 × 720 and 2320 × 720 (@1x, @2x) Top Shelf images.

`actool` compiles with `--platform appletvsimulator --target-device tv --app-icon "App Icon & Top Shelf Image"`, and the icon and Top Shelf image keys of `Info.plist` come from its partial plist.

## Local control channel

Same as iOS: `bern run` hands the Mac's temporary directory to the app through `SIMCTL_CHILD_BERN_IPC_DIR`, and the CLI, JSON client and MCP server on the Mac connect over IPC to the app in the simulator; when the app is launched from the Home screen, the channel does not listen. When a macOS, iOS or Android engine with the same bundle identifier is running at the same time, pick one with `--engine`: `--engine tvos` picks the simulator's.

## Build and run

```bash
rustup target add aarch64-apple-tvos-sim
```

```bash
bern build --target tvos
```

```bash
bern run --target tvos
```

`bern run --target tvos` looks only for a running tvOS simulator (one whose runtime identifier contains `.tvOS-`); when there is none, it reports an error with a hint on how to start one. Build steps: build the engine static library for the simulator (`TVOS_DEPLOYMENT_TARGET=18.0`) and the CLI and MCP server for the Mac; mirror the bundled resources to the top level of the bundle; compile the icon with `actool`; compile the UI with `xcrun --sdk appletvsimulator swiftc` for `arm64-apple-tvos18.0-simulator`; move the dSYM next to the bundle; sign ad hoc.

## Verification log

2026-10-20, Apple TV 4K (3rd generation) (1080p) simulator, tvOS 26.5, Xcode 27.0. Only the simulator screen was captured (`simctl io screenshot`); no screen recording and no desktop-wide synthetic input.

Input in the simulator: the Simulator app's remote window has to be clicked on the desktop, so a small tool that sends HID events only to this simulator was used instead, to simulate game controller D-pad and button input. The D-pad moves focus and produces `onMoveCommand`; the controller's A button did not trigger the button action on system buttons, buttons with a custom style or buttons drawn purely by a style (confirmed in a probe app containing only these three kinds of buttons), the B button sent the app to the Home screen without reaching the probe's `onExitCommand`, and X and Y did not produce play/pause. So button actions were triggered through the CLI's `ui activate`, and the back and play/pause mappings are **not verified**; the behavior of `onExitCommand` and `onPlayPauseCommand` on a real remote is as given in Apple's documentation (known), and Bern's mapping has not been measured on a real remote.

- **Build and tests**: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` pass; new tvOS generation tests (Info.plist, the generated layered icon passing the same check as a declared icon, build steps and `actool` arguments, type-checking against the tvOS simulator SDK), generation tests for TV layout pruning and remote shortcuts (the same app generates keyboard shortcuts on macOS and is type-checked), and unit tests for layered icons.
- **New code on other targets**: `examples/gallery` uses the TV layout, `Shortcut` and `default_focus`; GTK passes Clippy and type-checking of the generated app in an Ubuntu 24.04 VM, WinUI 3 compiles with MSVC on the Windows PC, the Android gallery builds successfully with Gradle (Kotlin compilation), and the Web runtime tests cover shortcuts yielding keys. None of them were run interactively.
- **Showcase app**: built, installed, launched; the Home screen shows the generated layered icon, and when it is selected the Top Shelf shows the generated shelf image.
- **A real app's TV layout** (the prototype's 1280 × 720 scene enlarged 1.5 times):
  - At the top is the system tab bar (search first), and at the top right are "Open", settings and account; the arrow keys move focus between the tab bar and the content and among the content's controls, and the focus effect is the system lift and whitening.
  - The onboarding pages fill the screen, with the starting focus on "Start" and then on "Continue" at each step; the CLI reads `tv` as the layout selected by `Adaptive`.
  - The "Open link" modal is enlarged with the scene, and focus stays inside the modal.
  - Player: when the control bar is shown, the starting focus is on the pause button; when the control bar is hidden, a button filling the picture catches focus, and left and right skip back and forward 10 seconds without waking the control bar (position 558.65 → 549.15, 558.65 → 569.4), while up wakes the control bar; the mini player plays in the bottom-right corner.
  - The seasonal, media library, search (with results), detail and settings (`fullScreenCover`, tvOS forms and sidebar) pages display correctly.
- **Not verified**: a real remote; the back and play/pause mappings; "select" other than the controller's A button (button actions in the simulator were triggered through the CLI); a real device; HDR and Dolby Vision display; dynamic Top Shelf content (not implemented).

A second round on the same day: corrections after measuring screen by screen against the prototype's TV mode (`design/app.html` with "TV" selected). In the first round the real app reused the desktop components and sizes, without the set of overrides the prototype gives the TV (font sizes, page margins, button heights); problems such as missing row labels on the settings page, search results that were too small and switches shown as "On/Off" text all came from this. Corrections:

- Bern: TV font sizes for text styles (`font(tv:)`, six targets); the text session actions `push_str` and `pop` (six targets, used by the on-screen keyboard); `Search(suggest: true)` (six targets); the control drawing in the "Scaled scenes" section above; TV focus for self-drawn buttons; on a TV other windows get no title bar and no "Done".
- The real app: font sizes, 56-point page margins, a 560 × 315 home stage, a 500-high seasonal page, a six-column poster grid and 40-point filters in the media library, detail, full-screen onboarding, an on-screen keyboard search page, full-screen settings (a 300-point page panel, page titles, a close button), the order at the top right.
- Simulator screenshots compared: home, seasonal, media library, detail, search (including typing a first letter and deleting), settings' General / Appearance / Playback / Danmaku / Optional features / About, the four onboarding steps, the player, Open link, the mini player; positions are within a few points of the prototype.
- A problem observed (worked around in the app at the time, later fixed in Bern): the header of a collapsed weekday column on the seasonal page, `Row { Canvas(width: 16); Column { Row { Text(lines: 1); Spacer } … } }`, had its text squeezed into a thin sliver of the first character. It was unrelated to buttons: the same header outside a button at 86 points wide was squeezed the same way, and at 200 points wide it was fine both inside and outside a button. The cause is that on tvOS the system `Spacer()` has a minimum length of 60 points (8 points on macOS, measured the same way), and `BernRow` took that as the minimum width of `Spacer`, so a 70-point-wide row left only 10 points for the 45-point-wide weekday name, and the two pieces of text on the second row got only about 2 points. Now `Spacer` in `BernRow` has no minimum width, and every generated `Spacer` is `Spacer(minLength: 0)` (in columns too), the same as on the other targets.
  - Verification: on the Apple TV simulator, a small project placed the same header outside a button (200 and 86 points), in a styled button (86 and 200 points), in an unstyled button, and in a button wrapped in an extra `Column(width: 86)`; before the fix all four at 86 points were squeezed, after the fix all six display in full; in a copy of the real app with the collapsed column switched back to the original header, the seven column headers on the seasonal page match the expanded column. Regression test: `tests/row_layout.rs` lays this header out on macOS into an 86-point-wide `BernRow` whose `Spacer` has a 60-point minimum; before the fix the weekday name got only 11 points (it needs 45); generation tests check the form of `Spacer` in rows and columns.

2026-10-21, placement of the window body (the rules and the fix are in the same-day entry of the macOS implementation notes; tvOS shares the same generation code): Apple TV 4K (3rd generation) (1080p) simulator, tvOS 26.5, a probe app switching the body through the CLI, only the simulator screen captured. After the change, the default column is laid out from the top left and buttons keep their own width; a `Column(align: center)` written in a component is centered horizontally and sits at the top; only a `Stack` with a single small layer is in the middle of the screen. Before the change, no screenshots were taken on the TV.

A same-day addition: search in the prototype's TV tab bar is only a magnifying glass, so `Tab(display: icon)` (six targets) was added for it. On another temporary simulator of the same model (tvOS 26.5), search in the real app's tab bar shows only the magnifying glass; after it is selected with the CLI's `ui select`, the magnifying glass sits on the selection background, and the selected name read by `ui inspect` is "Search". Screen reader speech was not measured.

2026-10-21, value types with the same name as framework types: the generated Swift writes value types as `App_` plus the declared name (for the reason and the form see the same-day entry of the macOS implementation notes; tvOS shares the same generation code). `tests/framework_names.rs` generates the example `examples/framework-names` for tvOS and type-checks it with swiftc against the tvOS 27.0 simulator SDK: passes; not run in the simulator.

2026-10-21, components and sessions with the same name as framework types: components are written as `AppView_` plus the declared name, session classes as `App_` plus the declared name, and the variables through which views read sessions as `session_` plus the declared name (for the reason and the measurements see the same-day entry of the macOS implementation notes; tvOS shares the same generation code). The example with these components and sessions added is type-checked with swiftc against the tvOS 27.0 simulator SDK: passes; not run in the simulator.

## Tables and controlled text (2026-10-29)

tvOS has no `Table`: it is generated as a `List` with one button per row, with the cells of the columns laid out horizontally inside the button (spacing 24); a press sets the selection to only this row and runs the primary action, and a long press opens the row menu (`contextMenu`), whose items likewise first select only this row. The header and the sort indicators are not shown. `TextInput(current:)` is the same as on macOS (`BernHeldText`). tvOS has no clipboard: the capability `paste` is not declared (the check reports it as unavailable), `BernClipboard` is an empty implementation, and the engine's `copy_text` returns `platform_unavailable`. Verification: the generated Swift type-checks against the tvOS SDK; not run in the simulator.

## Actions that follow a value and text color (2026-10-29)

`Watch` and `Text(color:)` are the same as on macOS. tvOS has no login items, so `Switch(login_item: true)` generates `EmptyView`. Verification: the `tables` test type-checks against the tvOS SDK; not run in the simulator.

## Not done and known limitations

- Only simulator apps are built; a real device needs signing and a provisioning profile, and installation and `bern package` are not implemented.
- `Svg` is not available (tvOS has no WebKit); an animated logo uses a static image on the TV.
- Picking files (`pick:`), drag and drop and reveal on hover have no corresponding system parts on a TV; these capabilities can appear only in layouts the TV does not show.
- When the search field in the settings window's sidebar gets focus, the system keyboard row covers the top of the detail (tvOS system behavior).
- Input in the simulator is limited by the tools; see above.

## Remote buttons that follow state (2026-10-24)

When `Shortcut(key: app.keys.x)` and a control's `shortcut` are bound to text, the remote button is read by `BernShortcuts.remote(…)` when the view is evaluated: when the text is `escape`, `space` or an arrow key, it maps to back, play/pause and the touch surface directions, and other text has no button; when the text changes, the button is registered again. On a TV, `ShortcutInput` only displays the shortcut, and `MenuBar` is not generated. Verification: `tests/menu_bar.rs` type-checks against the tvOS simulator SDK; not run in the simulator.
