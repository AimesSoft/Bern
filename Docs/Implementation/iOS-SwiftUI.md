**English** · [简体中文](../zh-CN/Implementation/iOS-SwiftUI.md)

# iOS SwiftUI implementation notes

For the current component interface see the [component reference](../Reference/UI.md); for per-item support and the recent verification scope see the [platform capability table](../Reference/Platforms.md). The implementation and acceptance records of each phase are kept below.

- Date: 2026-10-12
- Basis: [Bern architecture design v0.24](../Design.md)
- Toolchain: Rust 1.99.0 (`aarch64-apple-ios-sim` and `aarch64-apple-ios`); Xcode 26.5, iOS 26.5 SDK, Swift 6.3.2; the second half of the iPad windowed mode work used Xcode 27.0, the iOS 27.0 simulator SDK, Swift 6.4
- Verification environment: iPhone 17 Pro and iPad Pro 13-inch (M5) simulators, iOS 26.5 (M4 Mac)

This document records what was actually completed for the `swiftui + ios` target. The same app bundle runs on iPhone and iPad; how windows are presented is decided at runtime by device (see "iPad"). The declaration, protocol, business core and IR of the showcase app (`examples/showcase`) are unchanged, with only `[targets.ios]` added; the generator is shared with macOS and differs by platform in windows, scenes and a few control styles. This version builds a simulator app; the device target compiles and links, but signing, installation and packaging are not done yet.

## Completion status

| Design requirement | Status | Notes |
| --- | --- | --- |
| Generated code builds | Done | `bern build --target ios` generates Swift and the glue crate, and builds the simulator app bundle plus the CLI and MCP server on the Mac; the generation tests type-check the generated Swift against the iOS simulator SDK |
| Windows, navigation, menus and lifecycle differ by platform | Done | The runtime is split into the shared `BernRuntime.swift` and `BernMedia.swift` and the platform files `BernAppKit.swift` (macOS) and `BernUIKit.swift` (iOS); a build compiles only the one for its own platform |
| Virtual windows when native multiple windows are not supported | Done | iPhone: one scene, the main window fills it, other windows are sheets stacked on top; window facts report `Virtual` |
| Environments that support multiple windows choose by verified native capability | Done | iPad: every window kind except settings is an independent native window (a `WindowGroup` scene), reported as `Native`; settings is a sheet on the main window |
| Foreground and background lifecycle | Done | On entering the background windows report hidden and lose focus, and are restored on returning to the foreground; media reports AVFoundation's actual state |
| Screen rotation, keyboard, safe areas | Partial | Keyboard input, safe areas and iPad window resizing (stopping at the minimum size) measured; rotation is handled by SwiftUI, not measured |
| Subscribable state; commands with parameters and error returns | Done | The same runtime as macOS: engine state, session drafts, queries, close protocol |
| Native Video presentation | Done | AVFoundation executor and `AVPlayerLayer` presentation, command-driven control bar |
| Device build and packaging | Not done | The engine and UI compile and link for `aarch64-apple-ios`; signing, provisioning profiles, installation and `bern package` not done |

## Project structure

| Location | Contents |
| --- | --- |
| `crates/bern-backend-swiftui/src/ios.rs` | iOS target: target description, checks, generation and build steps |
| `crates/bern-backend-swiftui/src/swift/` | Generator shared by both platforms; `Platform` decides the differences |
| `templates/BernUIKit.swift` | iOS platform part: virtual window host, sheets, window facts, development snapshots, video view |
| `templates/BernAppKit.swift` | macOS platform part, split out of the original `BernRuntime.swift`, behavior unchanged |

The generated app bundle has the flat iOS structure: the executable, `Info.plist`, each language's `.lproj` and the bundled resources are all at the top level of the bundle. `actool` compiles the 1024-pixel PNG icon into `Assets.car` (with no icon declared it is a square version of the Bern logo, and a smaller declared icon is first enlarged with `sips`; when `[app.icon].ios` is an `.appiconset` folder it is compiled as is, and its dark and tinted variants take effect with it; when it is an Icon Composer `.icon` folder it is handed to `actool` as `AppIcon.icon` together with the asset catalog, iOS 26 and later show the Liquid Glass icon, and earlier systems use the light, dark and tinted flat images drawn from it). `Info.plist` declares a launch screen (otherwise the app is shown in compatibility mode and does not fill the screen), multiple scenes (multiple windows on iPad; iPhone always has only one scene), iPhone and iPad, all four orientations on iPad, and a minimum of iOS 17.0.

Minimum iOS 17: the same generation as macOS 14; Observation, `ContentUnavailableView` and other APIs the runtime uses are available from this version on.

## Differences from macOS

The generator differs by platform only in the following places:

- **Scenes**: macOS has one `Window` or `Settings` scene per window kind; iOS has neither of these scenes, so every window kind other than the main window and settings is a `WindowGroup` of its own, and the main window's scene content is `BernVirtualHost` (on iPhone it hosts all virtual windows, on iPad it hosts settings).
- **Window size**: the declared width, height and minimum size are used only for windows people can resize, that is, on iPad; on iPhone the screen and the sheet decide.
- **Sidebar**: on iOS the split view collapses into a navigation stack on narrow screens; the sidebar is the first screen, titled with the window title (`.navigationTitle(bernWindow.title)`). The sidebar control is registered on the whole split view, so external entry points can switch pages even when only the detail is shown (the same effect on macOS).
- **Text input**: iOS forms show only an input field's placeholder, not its label. Single-line and password inputs become `LabeledContent` (label on the left, input on the right, as in the Settings app); multi-line inputs put the label above.
- **Control styles**: the `checkbox` style of `Switch` is a toggle switch on iOS (iOS has no checkboxes); the `radio` style of `Picker` is an inline list with checkmarks on iOS; `chips` is the same as on macOS, 30-point system glass capsule buttons (see "Tag pickers and wrapping toolbars" in the macOS document).

## Windows

iPhone can have only one scene, so per the design document it uses virtual windows, in forms that follow iOS conventions (the settings window on iPad works the same way):

- **The main window** fills the scene. After it is closed by a command, the scene shows the app name and an Open "…" button; tapping it reopens the window (a new instance generation).
- **Other windows** are sheets, stacked one on another in the order they were opened: the navigation bar shows the window title, and a "Done" button at the top right requests closing through the close protocol. The settings window's tabs are a tab bar at the bottom of the sheet.
- **Swipe down to close**: with no pending changes to confirm, a sheet can be swiped down to close, reported as a system fact (`changed_by: System`); when the close check needs confirmation, swiping down is disabled and only the "Done" button works, which then presents Save, Don't Save and Cancel.
- **Window actions**: only the topmost window can be activated; it and the main window can be closed. A window covered by a sheet cannot be brought to the front (on iOS the ones above it can only be closed first), so `window.activate` and `window.close` return `target_unavailable`; `minimize`, `restore`, `maximize`, `move` and `resize` likewise return `target_unavailable` and list the available actions.
- **Window facts**: visibility follows the scene (hidden in the background), the frame is the position of the window's root view in the scene (in points), and only the topmost window in the active scene has focus.

## iPad

On iPad the same app bundle handles windows as Apple's documentation describes:

- **Whether multiple windows can be opened**: a SwiftUI app on iPadOS can open multiple windows when it declares `UIApplicationSupportsMultipleScenes` (per the documentation of `supportsMultipleWindows`); the same declaration has no effect on iPhone. Before starting the engine the runtime decides by "iPad and multiple scenes declared" and tells the engine through `bern_app_native_windows`, so the presentation in window facts matches reality: `Native` on iPad, `Virtual` on iPhone.
- **Windows**: iPadOS has no `Window` (single window) scene as on macOS, only `WindowGroup`. Each window kind is one `WindowGroup(id:for:)`, opened with the window kind as the value (`openWindow(id:value:)`); for the same value SwiftUI does not open a new window but brings the existing one to the front, which matches Bern's rule of one instance per window kind by default. Closing uses `dismissWindow(id:value:)`; activating is opening again. The window title is the declared title, which the system's window list and menu bar use to tell windows apart.
- **Settings**: iPadOS has no `Settings` scene; the settings window is a form sheet on the main window, the same as on iPhone. The Human Interface Guidelines require ⌘, to open settings when a keyboard is attached; the generated `.commands` replace the "Settings" item in the iPadOS menu bar with one that opens the settings window, with the shortcut ⌘,.
- **Appearance of secondary windows**: content without its own navigation is placed in a navigation stack with an inline title. WWDC25's "Elevate the design of your iPad app" requires the toolbar to wrap the window controls, otherwise the system uses a safe area that exists only for compatibility; with a toolbar, in windowed mode the window controls sit at the leading edge of the title bar.
- **Size**: TN3192 requires supporting resizable scenes (a launch screen, all four orientations on iPad, no `UIRequiresFullScreen`), and SwiftUI expresses a minimum size with `.windowResizability(.contentMinSize)` and a minimum `frame` on the content. The generated code adds both to windows that declare a minimum size (the `frame` takes effect only on iPad; iPhone is not affected), and `.defaultSize` to windows that declare a width and height.
- **Window facts**: each window's visibility comes from the phase of its own scene (in full-screen mode, after switching to another window the previous window is hidden), and the frame is the position of the window's root view on the screen. In windowed mode the scenes of several windows are active at the same time and each system window is also the key window of its own scene, yet only the frontmost window "appears active", so focus also depends on SwiftUI's `appearsActive` (iOS 18 and later); iOS 17 has no such value, so there focus depends only on the scene phase and key status. A main window covered by the settings sheet cannot be activated, the same as on iPhone. There is no public API to minimize, move or resize iPad windows, so these commands return `target_unavailable`.
- **Navigation**: at regular width the split view shows the sidebar and the detail side by side, with the window title at the top of the sidebar; `Adaptive` chooses its layout by content width, and the video page uses the expanded layout on iPad.

## Local control channel

The app in the simulator is a process on the Mac and can share the file system with the Mac, so the CLI, the JSON client and the MCP server (built for the Mac) can connect to it over the IPC channel. The app's own temporary directory is in a deep container path, and the socket path exceeds the length limit of Unix sockets; `bern run` therefore hands the Mac's temporary directory to the app through `BERN_IPC_DIR` (via `simctl`'s `SIMCTL_CHILD_` variables), and the channel creates its socket there. On iOS the channel listens only when this variable is set: when the app is launched by tapping it on the Home Screen or is installed on a device, no other process can connect to it, so the channel does not start.

`BERN_IPC_DIR` works on other platforms too: it replaces the temporary directory as the parent of each app's socket directory.

When the app in the simulator has the same bundle identifier as the macOS version, the engines of both are in the same directory, and the CLI lists both engines and requires `--engine` to pick one.

iOS suspends apps that are fully in the background, and a suspended engine does not respond. The client's handshake therefore has a 5-second timeout; on timeout it reports "engine not responding" instead of waiting forever; commands after the handshake are not subject to this timeout.

## Media

The executor is shared with macOS (`BernMedia.swift`). The video view is a presentation-only `UIView` that shows the player with `AVPlayerLayer`; it does not handle touches and never changes playback on its own. When the first player is created the audio session is set to `.playback` (movie mode), so video still has sound when the Silent switch is on, as in video apps.

If a player that has finished leaves the end without having received a seek command, it is no longer "ended": iOS moves a finished player back to the start when the app enters the background (the current time AVFoundation reports becomes 0); it is then reported as paused at position 0, and playing again starts from the beginning. This handling is in the executor shared by both platforms; macOS is not affected.

## Build and run

```bash
rustup target add aarch64-apple-ios-sim
bern build --target ios
open -a Simulator
bern run --target ios
```

`bern run --target ios` installs the app into the running iOS simulator and launches it; the app's output appears in the terminal until the app exits; `BERN_*` environment variables (for example `BERN_APPEARANCE=dark`) are passed to the app as well. When no simulator is running it reports an error and explains how to start one. `bern run` without `--target` prefers the target matching the host system, so on a Mac it still launches the macOS version. `bern doctor` checks the iOS simulator SDK and Rust's simulator target; whether a runnable simulator exists affects only `bern run`, not builds (after an Xcode update the simulator devices take a while to finish loading, and builds should not fail in the meantime).

Build steps: build the engine static library for the simulator (`IPHONEOS_DEPLOYMENT_TARGET=17.0`) and the CLI and MCP server for the Mac; mirror the bundled resources to the top level of the bundle; compile the icon with `actool`; compile the UI with `xcrun --sdk iphonesimulator swiftc`; move the dSYM next to the bundle; sign ad hoc.

## Verification log

2026-10-12, iPhone 17 Pro simulator (iOS 26.5), driven by the generated CLI and simulator screenshots:

- **Build and tests**: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` pass; new iOS generation tests (type check against the iOS simulator SDK, Info.plist, build steps, resource naming rules). Device: the engine builds for `aarch64-apple-ios` and the UI compiles and links for `arm64-apple-ios17.0`, producing an arm64 executable with platform iOS and minimum 17.0; not signed, not installed.
- **Appearance**: iOS 26 standard controls and the Liquid Glass navigation bar, search bar and tab bar; the sidebar has a search bar at the bottom; the toolbar buttons on the buttons page, the segmented and menu pickers on the switches and sliders page, and the icons at both ends of the sliders are all system controls. With `BERN_APPEARANCE=dark` everything is dark.
- **Navigation**: at launch the detail is shown directly (title and text), and going back shows the sidebar; tapping a sidebar item enters the corresponding page; the CLI's `ui select --target main_window.pages` switches pages even when only the detail is shown; on the list page tapping "Movies" pushes the item detail.
- **Input**: after tapping the name field and typing "Ada" on the keyboard, the preview immediately shows "Hello, Ada".
- **Windows**: "Open Settings…" opens the settings window as a sheet, and the main window loses focus and has only the close action left; after entering a name, tapping "Done" brings up the save confirmation, and choosing "Save" writes the name into the engine and closes; with no changes, swiping down closes (`changed_by: System`), and with unsaved changes swiping down has no effect; closing from the CLI returns `ConfirmationRequired`, during which controls are `blocked_by_modal`, and after `Discard` it closes; `minimize`, `resize` and activating the covered main window return `target_unavailable`; after the main window is closed the scene shows Open "Bern Showcase", and tapping it opens the main window as `main#2`; after `ui set-language --language en` the whole UI switches to English.
- **Lifecycle**: after pressing the Home button the main window is `Hidden` with no focus, and the CLI can still read state; after returning to the foreground it is `Visible` with focus again.
- **Media**: the preview player plays the color bars clip, seeks and ends (`Ended 6000 / 6000`); after leaving the video page and coming back the state is unchanged; when the app enters the background during playback it keeps playing to the end in the simulator; entering the background after playback has finished reports `Paused 0`, and playing again starts from the beginning. On narrow screens the clip selection on the video page is a "Change Clip" menu.
- **Images**: bundled images and Grid work; PQ PNGs are tone-mapped. JPEGs with a gain map decode as HDR in the simulator into all-black pixels (drawing the decoded result into an 8-bit context reads back 0), so simulator builds decode only the SDR version and show the SDR image the author provided.
- **MCP**: `showcase-mcp` completes `initialize`, and `window_list` returns the `Virtual` windows of the app in the simulator.
- **macOS unaffected**: after the runtime split the macOS version was rebuilt and run; the settings window, the close protocol, minimize and restore were measured through the CLI and work.

iPad, same day, iPad Pro 13-inch (M5) simulator (iOS 26.5, full-screen mode). The showcase app has only a main window and a settings window, so an ordinary "Inspector" window was added temporarily to test multiple windows (not committed):

- The main window is `Native`, visible and focused (frame 1032 × 1324); the split view shows the sidebar and the detail side by side, with the iPadOS 26 window resize handle at the bottom right.
- `window open --kind inspector` opens an independent window (`Native`, with an "Inspector" title bar); in full-screen mode the system switches to it and the main window is `Hidden`; `window activate --window main#1` switches back to the main window; opening the inspector again yields the same instance `inspector#1`; `window close` closes the inspector, the system returns to the Home Screen, and the main window is still in the background.
- Settings is shown as a form sheet on the main window (`Virtual`, width 580); meanwhile the main window has no focus and the inspector is in the background.
- At iPad width the video page uses the expanded layout, and the preview player plays normally.
- When the whole app entered the background it was suspended, and the existing CLI kept waiting at the handshake; with the handshake timeout added it now reports that the engine is not responding (new test).
- iPhone regression: all three windows are `Virtual`, the inspector is a sheet with a "Done" button, and while it is covered by settings `window activate` returns `target_unavailable`.

iPad windowed mode, same day, the same simulator with "Windowed Apps" selected in "Settings > Multitasking & Gestures"; the second half was rebuilt with Xcode 27.0:

- Windows carry the iPadOS 26 window controls: the main window's controls sit at the leading edge of the sidebar toolbar, and the inspector's at the leading edge of its title bar; dragging the bottom-right corner to resize stops at the declared minimum size of 720 × 480; the main window and the inspector can be shown at the same time and partly cover each other.
- Focus: at first it was reported as "scene active and system window is key", and both windows reported focus at the same time — in windowed mode both scenes are active and both system windows are key. It was then changed to remember the window most recently brought to the front by a command or a scene activation; switching by command was correct, but when the window behind was tapped both scenes stayed active without any phase change, so the remembered window went stale; and when a new window was opened, the scene of the window behind first became inactive and then active again, and was taken as having "come to the front". Finally it switched to `appearsActive`: switching back and forth with `window open` and `window activate` and tapping the two windows, each time only the frontmost window reports focus.
- On pressing the Home button iPadOS pushes the windows to the edge of the screen to reveal the Home Screen; the app is not suspended (the CLI gets an answer immediately), the scenes are still active, and the windows still report visible, with the main window focused; nothing changes after coming back through the Dock icon.
- On relaunching the app iPadOS restored only the inspector that was frontmost last time; the main window stayed among the "hidden windows" and the engine saw only `inspector#1`; `window open --kind main` brings the main window back.
- `window close` closes the inspector and the main window gets focus; on iPad settings opens as a sheet (`Virtual`), and after it closes the main window regains focus.
- Once after reinstalling, the app window was all black with no control channel (no IPC thread in the process, no `BERN_IPC_DIR` in the environment), and a crash report was left from reading bundle information during the preparation stage of `UIApplicationMain` at launch, which looked like installation and launch overlapping; after running again it was fine and did not recur.
- iPhone 17 Pro regression: the inspector and settings stack as sheets in turn, the covered window has no available actions, and after tapping "Done" to close the inspector the main window regains focus.

The showcase app was adjusted accordingly: the two rows of images on the images page changed from `Row` to `Grid`; the original four 120-wide images did not fit on iPhone and were clipped on both sides, and now they wrap automatically on narrow screens and stay on one row in wide windows.

### Tag pickers and wrapping toolbars (2026-10-20)

iPhone 17 simulator (iOS 26.1), Settings › Plugins opened through the CLI: the first row is the segmented control "Market / Installed 3" with "All", "Danmaku" and "Picture" aligned right, and "Tools" and "Appearance" wrap to the second row aligned left, matching the Apple phone layout of the prototype; the chips are 30-point system glass capsules, and the selected "All" is in the theme color. The first-run onboarding buttons were mounted twice at the same time on iOS (`ui activate` reported `target_ambiguous`), unrelated to this change and not handled at the time (since fixed, see the next section).

### Control registrations leave no stale entries (2026-10-20)

iPhone 17 simulator (iOS 26.1); a real app shows a full-screen onboarding flow (`Modal(cover: compact)`) on first launch; checked with the generated CLI:

- **Symptom**: `ui inspect` listed the onboarding footer's `skip` and `start` twice each, and `ui activate --target onboarding_footer.skip` reported `target_ambiguous`; the home page covered by the onboarding also had many controls repeated 2–5 times, 125 entries in the window in total. After the onboarding was completed, `skip` and `start` were still in the list, and the playback-only page still listed desktop buttons that are shown only on wide screens; opening the onboarding again after launch added another copy of those desktop buttons. On the Mac and on Android each target is listed once.
- **Cause**: the width is not yet known at the first layout. With a window width of 0 the window does not count as compact, and `Adaptive` with a `nil` width picks its layout as if infinitely wide, so the wide layout is mounted first (the modal is first laid out as a floating panel), then replaced by the phone layout once 402 is measured. Controls used to register in `.task(id:)` and unregister in `onDisappear`; on iOS 26.1 the task starts only after this update, so these momentary controls were unregistered first and registered afterwards, and stayed in the list from then on. On macOS 27 the task runs immediately as the view appears, which is why this does not show on the Mac: printing the order of appear, task and disappear in an offscreen window, the task always came before appear, also when compiled with a macOS 15 deployment target.
- **Fix**: `BernControl` registers in `onAppear`, registers again when the window, target, label or enabled state changes, and unregisters in `onDisappear`, all synchronously. Compose (`SideEffect` and `DisposableEffect`), Web (`onCleanup`), GTK and WinUI 3 (scope cleanup) were already paired synchronously and did not have this problem. The first layout still builds the wide layout once before replacing it, it just no longer leaves registrations behind (since fixed, see the next section).
- **Tests**: `tests/control_registry.rs` runs the real SwiftUI modifiers in an offscreen window: modal-style floating first and then filling, and `Adaptive`-style wide first and then narrow; each target is listed only once and `find` finds it, the list is cleared after removal and restored after reinsertion; another test checks that registration is not in a task, because a late-starting task cannot be reproduced at runtime on macOS 27.
- **Verification**: after the fix, on first launch `ui inspect` has 32 entries in total, `onboarding_footer.skip` and `.start` once each, and the desktop `stage.*` and `timeline.schedule` no longer appear; `ui activate --target onboarding_footer.skip` jumps to the last step and `ui activate --target onboarding_footer.finish` closes the onboarding, after which only the 9 controls of the phone playback-only page and `main_shell.shell` remain, and all onboarding targets are gone; when the onboarding is opened again after launch, each target appears once and the desktop controls no longer appear.

### The first layout is the phone layout (2026-10-20)

- **Problem**: after the fix in the previous section, on phones every newly mounted subtree — at app launch, or an `Adaptive` newly created by a conditional switch — still built the wide layout once first, and `Modal(cover: compact)` was still first laid out as a floating panel, switching only after the width was measured. The wasted builds brought side effects such as entrance animations, shared-element counts and player bindings.
- **Fix**: until the window's width is measured, size classes such as `window.compact` and `window.short` are computed from the size the platform gives (`BernWindowContext.layoutWidth`, `layoutHeight`): with only one scene it takes the scene's size; where independent windows can be opened (iPad) it takes the screen's size, because a new window may open at any size. Both are no smaller than the size the window actually gets, so a wide window does not first build a narrow layout. Until `Adaptive` has measured its own available width it takes the width of its window, as the available width cannot be wider than the window. macOS does not know the window size before the first layout, so it still starts from the widest class.
- **Tests**: `tests/adaptive_first_layout.rs` runs the real `BernAdaptive` in an offscreen window and records how many times each layout is built: in a window 390 wide whose platform size is a phone, both an `Adaptive` present at launch and one inserted later build only `compact`, and a covering modal builds only the filling form; in a window 1000 wide, whether the platform size is a tablet screen or unknown (macOS), only `expanded` is built. The same test fails with the runtime from before the change: at launch `expanded` and the floating panel are each built once.
- **Verification**: iPhone 17 simulator (iOS 26.1), a minimal app with `Adaptive(id: start)` and `Modal(cover: compact)` on the first screen that inserts `Adaptive(id: inserted)` once a switch is turned on; the chosen size was printed temporarily where the layouts and the modal are built (removed after verification):
  - Before: at launch `start` first built `expanded` and the modal first built the floating panel, and after the width was measured they changed to `compact` and filling respectively; `inserted`, inserted by `ui set-bool --target home.more --value true`, also first built `expanded`.
  - After: at launch the window width is still 0 and is estimated from the scene width of 402; `start` is `compact` on its first build and the modal fills on its first build; the inserted `inserted` is `compact` on its first build (its own width unknown, window width 402). `expanded` was never built during the whole run, and in `ui inspect` both adaptives are `compact` from the start.
  - The gallery example on the phone, switched to the film page (`ui select --target main_window.pages --option Film`): `film.film_controls` likewise builds only `compact`.

### Style variants written directly in a window body (2026-10-20)

The generator test `a_style_variant_in_a_window_is_defined` generates `variant: stage` in a window body for iOS; `BernStyles` has `buttonStage`, and the type check against the simulator SDK passes (for the reason see the same-day entry in the WinUI 3 implementation notes). No iOS device was booted in the simulator at the time, so the screen was not looked at.

A `Drop(opens: true)` written directly in a window body was previously not added to the document types and imported types in Info.plist; for the cause and the fix see the same-day entry in the macOS implementation notes; the generator test `a_drop_in_a_window_lists_its_document_types` also asserts the imported types for iOS.

### Placement of the window body (2026-10-21)

When the window body is smaller than the window it is laid out from the top-leading corner of the content area (for the rule and the fix see the same-day entry in the macOS implementation notes; iOS shares the same generation code). iPhone 17 Pro simulator (iOS 26.5), a probe app switching bodies through the CLI, screenshots with `simctl io screenshot`: before the change the default column as a whole was in the middle of the screen (centered both horizontally and vertically); after the change the default column is laid out from the top-leading corner of the safe area with buttons keeping their own width, a `Column(align: center)` written in a component is centered horizontally and at the top, and a `Stack` with only one small layer is in the middle of the content area; icon buttons, menus and popover buttons keep their own width in the default column. The generator test `content_in_a_window_starts_at_its_top_leading_corner` type-checks the iOS generation against the simulator SDK.

### Value types with the same name as framework types (2026-10-21)

In generated Swift a value type is written as `App_` plus the declared name; for the reason and the form see the same-day entry in the macOS implementation notes (iOS shares the same generation code). `tests/framework_names.rs` generates the example `examples/framework-names` for iOS and type-checks it with swiftc against the iOS 27.0 simulator SDK, which passes; the UIKit runtime `BernUIKit.swift` is included in the check as well. Not run in the simulator.

### Components and sessions with the same name as framework types (2026-10-21)

A component is written as `AppView_` plus the declared name, a session class as `App_` plus the declared name, and the variable through which a view reads a session as `session_` plus the declared name; for the reason and the measurements see the same-day entry in the macOS implementation notes (iOS shares the same generation code). The example `examples/framework-names`, with these components and sessions added, type-checks with swiftc against the iOS 27.0 simulator SDK, which passes; not run in the simulator.

## Staggering, proportional tracks, popover buttons and four size classes

2026-10-14, v0.30, iPhone 17 Pro simulator (iOS 26.5), sharing the same SwiftUI implementation with macOS; launched with `SIMCTL_CHILD_BERN_MOTION=slow` to slow motion down.

- **Staggering**: recording the simulator screen (`simctl io recordVideo`, the simulator only) and stepping through it frame by frame, when going back from the film page to the patterns page the cards fade in and rise one after another in the order Gingham, Tartan, Madras.
- **Proportional tracks**: the film page's timeline, ticks and names are spaced proportionally on narrow screens; with `film.level` set to 0.8 the vertical line and the background move to 80%, and the background is as tall as the panel. The names use `anchor: start`; on narrow screens the last two names squeeze each other, which comes from the interface declaration.
- **Popover buttons**: on the frame page the controls hide after it sits idle; a background loop waits for `frame.volume` to mount, a tap on the picture in the simulator then wakes the controls, and `ui activate` runs immediately after mounting: the button value is `open`, and the volume slider in the popover mounts with the value 0.7. In the screenshot the iPhone shows a bubble pointing at the button (`presentationCompactAdaptation(.popover)`), and its appearance is dark along with the frame page.
- **Four size classes**: with the iPhone in portrait, the film page's `Adaptive` is `compact` and shows two icon buttons. iPad and landscape not tested.

## Tables, controlled text and the clipboard (2026-10-29)

The generator is shared with macOS (see the macOS implementation notes): `Table` is SwiftUI's `Table`, on a compact-width iPhone the system shows only the first column, and the row menu is a long-press context menu; `paste:` reads `UIPasteboard.general`, and the first time the system asks the user whether to allow pasting; the engine's `copy_text` writes `UIPasteboard.general`. Verification: the generated Swift type-checks against the iOS SDK, and the gallery's iOS build succeeds; not run in the simulator or on a device.

## Actions that follow values, and text color (2026-10-29)

`Watch` and `Text(color:)` are the same as on macOS (`BernWatch`, `foregroundStyle`). iOS has no login items, so `Switch(login_item: true)` generates `EmptyView` (the capability `login_switch` does not apply). Verification: the `tables` test type-checks against the iOS SDK; not run in the simulator.

## Incomplete work and known limitations

- Only simulator apps are built. Devices need developer signing and provisioning profiles; installation and `bern package` are not implemented.
- HDR: the simulator cannot display HDR, and in the simulator gain maps decode only the SDR version; on a device images decode as HDR and video is shown by `AVPlayerLayer`, with HDR not verified for either.
- No background audio is declared, so on a device the system pauses playback after the app enters the background; this was only observed in the simulator (where playback continues).
- The "Settings" item and ⌘, in the iPadOS menu bar are only type-checked, not measured (the simulator offers no menu bar operation with an attached keyboard).
- iOS 17 has no `appearsActive`, so when several windows are shown at once they may all report focus; no iOS 17 simulator is at hand, not verified.
- When iPadOS relaunches the app it restores only the windows it remembers, so the main window does not necessarily open at launch; Bern reports the windows the system restores.
- In full-screen mode, after the current window is closed the system returns to the Home Screen and does not switch to the app's other windows; those windows are still in the background and can be reached from the Dock or the App Switcher.
- Screen rotation not measured.
- A window covered by a sheet cannot be activated or closed on its own; the windows above it must be closed first.
- When a window's content has its own split view or navigation stack, it has no "Done" button when opened as a sheet (nested navigation stacks cause problems), and can only be closed by swiping down or by a command.

## Phone shell and pixel comparison (2026-10-18)

On an iPhone 17e simulator (iOS 26.5, 390 × 844 points, screenshots 1170 × 2532), a real app's phone home page was compared with its web prototype. The prototype was rendered at 3x into an image of the same size with headless Chrome, and the content boundaries on both sides were found row by row and column by column with the same tool and compared; after the changes below, every element on the home page is within 1 point of the prototype in position and size (for Chinese glyphs see below).

- **Search panel**: `Search(style: palette)` and `SearchItem` also generate the same panel on iOS (previously only macOS accepted them). The input field is replaced with a system `TextField`; tapping the scrim closes the panel, and a hardware keyboard's arrow keys, Return and Esc still work; macOS keeps the AppKit input field and the window click monitor. The generator regression test generates and type-checks the same fixture for iOS.
- **`Pages` in `Tabs`**: at compile time the module pages are expanded into `Tab`s whose content is the page component, from the same source as `Pages` in the sidebar. The selected tab takes the app's accent color (`.bernAccentTint()` on the `TabView`); on iOS plain glass buttons and round icon buttons use the primary text color instead (`bernPrimaryLabel`), so they are not tinted by the tab bar's accent color.
- **`Adaptive` measures the available width**: it used to measure the width of the chosen layout itself, and `frame(maxWidth:)` without a minimum is no smaller than its content, so once the wide layout had been chosen first it kept measuring wide. Now with `minWidth: 0` it measures the width the parent offers.
- **Fixed-size `Stack` placed by `align`**: the `.frame` of `Stack(align:, width:, height:)` carries the same alignment, so the content is no longer always in the center.
- **Surfaces without rounded corners fill the safe area**: the style background of a `Stack` without `corner` uses SwiftUI's shape-style background, which extends under the status bar and the tab bar.
- **`Button(width: fill)`**: the system button stretches with its label to fill the container width (capability `fill_buttons`, SwiftUI and Web).
- **`IconButton(height:)`**: a system glass round button with the given diameter, done the same way as capsule controls with a given height; native iOS round buttons come in only two sizes, about 31 and 47 points. Capsule controls take their font size from the design size class: 15-point semibold at 42 points and above, 13-point medium otherwise.
- **`Row` lays out by each item's own width**: `HStack` divides space among the items evenly, from the least to the most flexible, so even when a row fits it may first give a short truncatable text a share smaller than its width and truncate it with an ellipsis (measured: a row of three items in which the tag group got only 81 points but needed 88). Now `Row` generates `BernRow` (a `Layout`) with the same rules as rows on the other targets: when everything fits each item takes its own width, and the remaining space is shared as evenly as possible among the items that stretch (`Spacer`, filling frames); when it does not fit, the other items shrink in proportion to their own widths, no smaller than their minimum widths. A `Spacer` in a row carries the `BernRowSpacer` marker, so it stretches only along the row and does not take part in the row height; a `Divider` in a row is wrapped in an `HStack` to keep its vertical orientation. A `Spacer` in a row or a column generates `Spacer(minLength: 0)`, and `BernRow` does not take its minimum width either: the minimum length of the system `Spacer()` differs by platform (measured 8 points on macOS, 60 points on tvOS), and `Spacer` on the other targets has no minimum length.
- **Fixed-width or fixed-height `Column` / `Row` lay out content from the start edge**: previously `.frame(width:)` put narrower content in the center, so the text in `Column(width: 58)` sat off to the right. Now a column starts from the top and follows `align` horizontally (leading by default), and a row starts from the leading edge and follows `align` vertically, consistent with the other targets.
- **Phone dropdown capsules**: on iOS `Menu(shape: capsule)` is laid out per design section 6.3: the symbol in a 16-point frame, the arrow in a 12-point frame at 60 % opacity, spacing 6, and 11 points from the edge on the arrow side. The button of `Picker(style: menu, shape: capsule)` carries the same arrow (iOS menu buttons do not draw an arrow themselves).
- **Capsule split buttons**: the toggle part of `SplitButton(shape: capsule)` does not use the system `Label`; the symbol is in a 16-point frame, 6 from the text, 14 from the end of the capsule and 10 from the divider; the dropdown part is 30 wide. macOS and iOS do it the same way.
- **Immersive windows fill the screen**: when `immersive` is true, iOS window content extends under the safe area (`ignoresSafeArea`), and on a phone in landscape an 844 × 390 picture fills it at 1x; when it is false the content returns inside the safe area.
- **Tab bar accessory**: from iOS 26.1 on, `TabAccessory` generates `tabViewBottomAccessory(isEnabled:)`, with the content built only once (`AnyView`), and when `shown` is false the system collapses the accessory; earlier iOS versions embed a material capsule inside the safe area at the bottom of each page. Icon buttons in the accessory are symbols on the bar (environment `bernOnBar`): they no longer each carry glass, and `IconButton(height:)` only gives the size of the tap area. Measured on an iPhone 17e simulator (iOS 26.5): the accessory appears above the tab bar when playback starts and collapses after the player is closed.
- **Internal layout of capsule buttons**: `Button(shape: capsule)` with an icon no longer uses the system `Label` (its spacing differs between the two platforms); instead the symbol is in a 16-point frame (14 points for 30-point-tall tag capsules), 6 from the text; the horizontal padding follows the size class: 22 at 42 points and above, 13 at 30 points and below, 14 otherwise. macOS and iOS do it the same way.

Chinese glyphs: with the same 30-point system font, the advance width of PingFang Han characters rendered by Chrome is 1 em, while native iOS rendering gives about 0.947 em, so Chinese titles look about 5% smaller. Layout is aligned by points and glyph rendering is left to the system; the font size was not enlarged to compensate.

## Shortcuts and rebinding that follow state (2026-10-24)

Generation is shared with macOS: when `shortcut:` and `Shortcut(key:)` are bound to text, `BernShortcuts.keyboard(…)` is generated. On iOS `ShortcutInput` waits for a key press on a hardware keyboard (`focusable` plus `onKeyPress`); without a hardware keyboard it only shows the shortcut. `MenuBar` is not generated on iOS (the iPad menu bar is not hooked up; the capability `menu_bar` does not apply). Verification: `tests/menu_bar.rs` type-checks against the iOS simulator SDK; not run in the simulator or on a device.

## Scroll bars shown only for the mouse (2026-10-27)

`ScrollView(indicators: mouse)` shares `.bernMouseIndicators()` with macOS: only a pointer can hover, a finger cannot, so on iPad, once a pointer has hovered over the scroll view, it uses `.scrollIndicators(.automatic)`, and before that, as well as when only fingers are used, `.never`. Verification: the generator test type-checks the Swift for iOS; not measured on an iPad or in the simulator.
