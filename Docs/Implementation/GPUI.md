**English** · [简体中文](../zh-CN/Implementation/GPUI.md)

# GPUI implementation notes

- Basis: [the GPUI section of the design document](../Design.md#gpui-optional-desktop-target)
- Status: first version (2026-10-30). On macOS, RustSend and a large media app both pass the check, build and run on this target; only the components listed below exist. Windows and Linux have never been built.

## Selection and configuration

The GPUI target is used only when chosen explicitly; it is not the default for any platform:

```toml
[targets.gpui_macos]
platform = "macos"       # or windows, linux
ui = "gpui"
channels = ["ipc"]
```

The registered targets are `gpui-macos`, `gpui-windows` and `gpui-linux`, each built only on the system of the same name. Colors come from `BernTheme`.

## Dependencies

| Dependency | Version | Notes |
| --- | --- | --- |
| `gpui-pre` (referenced as `gpui`) | `=0.3.10` | Weekly snapshots of Zed's main branch published by longbridge; the official `gpui` on crates.io stopped at 0.2.2 (2025-10-22) |
| `gpui-pre-macos`, `gpui-pre-linux`, `gpui-pre-windows` | `=0.3.10`, optional | Platform crates, see below |
| `gpui-base` | `=0.8.1` | The unstyled layer of GPUI Kit: editing, selection and input methods for text inputs |
| `gpui-pre-reqwest-client`, `gpui-pre-http-client` | `=0.3.10` | http(s) images in state; downloads for image average colors |
| `image` | 0.25 (png, jpeg, webp, gif) | Tinting template images, image average colors |
| `objc2`, `objc2-app-kit`, `objc2-quartz-core`, `objc2-core-graphics`, `objc2-foundation`, `raw-window-handle` | macOS only | App icon, hiding the pointer, Reduce Motion, the About panel, window commands, the Metal layer for video |
| `bern-ffi`, `bern-theme` | Workspace | In-process interface; theme snapshot and control recipes |
| `sys-locale`, `url`, `smallvec`, `futures` | — | Preferred languages, file URLs, frames, event channels |

- **Platform crates enabled by feature**: `bern-gpui` has three features, `macos`, `linux` and `windows`, each enabling only the corresponding GPUI platform crate, and the generated app turns on only the one it is built for. GPUI's `gpui-pre-platform` is not used: during dependency resolution it brings in wgpu 30 for the Web platform, which requires `js-sys >= 0.3.104`, while the app's own dependencies may pin an earlier version (a large media app's media core pins `=0.3.103`, and its build fails already while resolving dependencies). Cargo's dependency resolution includes the dependencies of all platforms, but not optional dependencies that are not enabled.
- **Features of the macOS platform crate**: `runtime_shaders` compiles the Metal shaders at startup, so the build does not need Xcode's Metal toolchain (the Xcode on this Mac does not have it installed); the cost is that the app compiles the shaders the first time it starts on a given machine. `font-kit`: without it GPUI uses an empty text system, and no text is drawn at all.
- The versions are a pinned set; GPUI Kit's `gpui-component` 0.8.1 also depends on `gpui-pre =0.3.10`. Upgrading is a separate change, and the checks in these notes are run again.

## Structure

- `crates/bern-backend-gpui` (a workspace member that only generates code and does not depend on GPUI): target descriptor, check, generation, build plan. `bern package` does not yet support this target.
- `crates/bern-gpui` (not in the workspace; like `bern-gtk`, it is compiled only by the generated app):
  - `host`: starts the engine through `bern_ffi::start_app`. Callbacks run on the engine and channel threads; the bytes are copied into a channel and processed in arrival order by a task on the GPUI main thread.
  - `windows`: one native window per `Window`; opening and closing are both reported to the session runtime; when the last window closes the app quits, stopping the engine first.
  - Elements: `layout` (rows and columns, stacks, grids, scrolling, tabs, preview lists, accordions), `content` (text, labels, images), `controls` (buttons, switches, pickers, text inputs), `overlay` (popovers, menus, dialogs, split buttons), `search` (search, the search panel, the toolbar), `scroll` (snapping and positioning), `query` (asynchronous queries), `media` (video), `spin` (spinning images), `navigation`, `form`, `scene`, `canvas`, `behaviors`, `menus`; `look` is the `style!` rules, plus `theme`, `text`, `session` (external entry points) and `native` (platform interfaces).
- The generated app has the same layout as on other targets: on Windows and Linux it is `<product>/<program>` (with `resources/` next to it), on macOS `<product>.app` (see below); the app CLI and MCP server are in `bin/`, the agent skill in `skills/`.
- **macOS app bundle**: as with the SwiftUI target, `Info.plist` holds the name, version, identifier and `CFBundleIconFile`, and the name in each language is in `<language>.lproj/InfoPlist.strings`; the icon is an `.icns` made from the declared PNG (or the Bern logo), which the system crops to its shape and puts on the icon plate; a declared `.icns` is used as is, and Icon Composer documents are not supported (an error is reported); the program is in `Contents/MacOS`, the resources in `Contents/Resources/resources`, the CLI and MCP server in `Contents/Helpers` and the skill in `Contents/Resources/skills`; these are signed first, then the whole bundle is ad-hoc signed, and then `touch` and `lsregister -f` make Finder and the Dock read the new icon; `bin/` holds links to the programs inside the bundle. Inside a bundle, the runtime no longer uses `setApplicationIconImage` to set the app's own icon (that way the Dock would show the original image without a plate); it sets it only when `WindowSet(icon:)` switches to a different icon.

Window content is rebuilt every time GPUI renders: the generated code gives each window and component a `fn(&Ctx) -> El` that reads the engine state directly. After the engine state changes, each window's root view receives `notify` and redraws on the next frame. So this target does not need per-field reactive bindings like those of the GTK runtime.

## Implemented

Updated: 2026-10-31. The generator generates only the following components and properties; everything else is reported as an error at build time, pointing at the declaration.

| Category | Components |
| --- | --- |
| Windows | `WindowSet` (`accent`, `appearance` and `icon` chosen at runtime), `Window` (size, minimum size, `chrome: unified`, drafts, `idle_after`, `idle_enabled`, `immersive`) |
| Layout | `Column`, `Row` (including `align: baseline`), `Flow`, `Grid`, `Stack` (`style!` background, foreground, corner radius, shadow, `enabled`, `under_bars`), `ScrollView` (vertical, horizontal, `snap`, `position`, `open_at`, `open_center`), `Spacer`, `Divider`, `Adaptive`/`Layout`, `Form`, `Section`, `Accordion`, `PreviewList`, `FocusRegion` |
| Scene | `ScaleBox`, `Positioned`, `Rail`, `Canvas` (`Shape` rectangles, rounded rectangles, capsules, ellipses, strokes, dashes, fades and `fill_from` image average color, `Series`, `Pattern`) |
| Navigation | `Navigation`/`NavLink`/`Destination`, `SplitView`/`Sidebar`/`SidebarItem`/`SidebarHeader`/`SidebarFooter`/`Detail`, `Tabs`/`Tab` (`display: icon`)/`TabAccessory`, `Toolbar` (items before `Spacer` go at the leading end) |
| Content | `Text`, `Label`, `Image` (bundled resources, files and http(s) images from state, dark images, corner radius, `fit`, `aspect`, `rendering: template`), `Progress`, `Toast`, `Video` (see below) |
| Controls | `Button` (shape, height, `style!` rules, `pick`, keyboard shortcuts, with content), `IconButton`, `Switch`, `SplitButton`, `Picker`, `LanguagePicker`, `TextInput`, `Search` (search fields in a page, the sidebar or the toolbar with results below, and the search panel of `style: palette`), `SearchItem`, `Slider`, `Menu`/`MenuItem`, `PopoverButton`, `ControlGroup`, `ShortcutInput`, `Drop` (including `over`), `Pinch` |
| Behaviors | `ForEach`, `if`/`if let`/`match`, components with parameters, `Query`, `Modal`, `Shortcut`, `Timer`, `Watch`, `reveal: hover`, media command actions |

`style!` rules: the panels of `Stack`, `Form`, `Sidebar`, `Detail` and `PopoverButton` take background and foreground (`Stack` also corner radius and shadow); `Button`, `IconButton` and `SidebarItem` take the resting look and the hover, pressed and disabled states (sidebar items also selected), including `scale`.

Other capabilities:

- **Colors**: `bern-theme` generates snapshots by appearance and seed; the seed is the accent color chosen at runtime or `[theme].brand_seed`; `appearance: dark` makes a section of content dark. `fill.subtle`, `fill.strong`, `selection.background` and `selection.foreground` are derived from other tokens, as on the Web target; roles that the theme lacks are drawn in magenta so they are easy to spot. Text and icons without a specified color take the foreground of where they are: the foreground of the button style or the `Stack` style, or the primary text color when there is none.
- **Sizes**: the same type scale and control sizes as the Web design guidelines; `text { … }` in `style!` changes the font weight and size. All lengths inside a `ScaleBox` (canvases included) scale with the scene. Rows, columns and text do not exceed the width of their container, so multi-line text wraps within the container: GPUI measures the minimum width of text by its single-line width, and flex shrinking alone does not wrap it.
- **Scale states**: `scale` in `style!` redraws the button scaled on hover or press, above an invisible unscaled copy; the copy holds the layout, so the surroundings do not move; there is no animation.
- **Motion**: only `spin`; appear, disappear, transition and similar properties are accepted, but changes happen immediately, and the shared check gives a warning about this.
- **Spinning**: only an `Image` from resources spins; other elements and images from state do not spin, and the check gives a warning. GPUI's color images cannot carry a transform on the GPU, but monochrome graphics can: when all opaque parts of the image (opacity above 96) are the same color, or with `rendering: template`, the image is wrapped in an SVG with an embedded bitmap and drawn by `paint_svg` as a monochrome graphic with a rotation matrix; GPUI caches its mask by path and size, and spinning changes only the matrix; multi-color images are rotated frame by frame on a background thread, at device pixels, into a square the size of the diagonal (bilinear sampling), the main thread draws the newest frame, and used frames are removed from the atlas. While spinning, every frame requests the next one, so the whole window is rebuilt every frame. Reduce Motion is taken from the system's accessibility setting and from `BERN_MOTION=reduce`.
- **Menu bar** (macOS): the system's standard menus, with names and keyboard shortcuts as on macOS: the app menu (About, Services, Hide, Hide Others, Show All, Quit), File (Close ⌘W), Edit (Undo through Select All), Window (Minimize, Zoom, Bring All to Front). The Edit menu uses the text input's (`gpui-base`) own actions, and is available when a text input has focus and grayed out otherwise; Close, Minimize and Zoom call the window's own `performClose:` and so on, so closing goes through the window's close callback just like clicking the close button. The About panel is the system's standard panel, with the app name and version. After the UI language is switched, the menus change along with it. The app's declared `MenuBar` is not shown yet. On Windows and Linux there is only Quit.
- **Buttons with content**: as on the Web target, apart from the capsule shape they have no border or background of their own; their look comes only from `style!` rules, whose corner radius clips the content; they get slightly darker when pressed.
- **Search panel**: the entry in the toolbar shows the prompt and the keyboard shortcut; once opened, the panel covers the window, its input field takes the keyboard, the up and down arrow keys move between navigable `SearchItem`s, Return executes, and Esc or a click outside the panel closes it.
- **Video**: the app's media core plays all players (`bern_media_kernel_attach`). On macOS each `Video` is a native view in the window's content view, below the GPUI view, hosting a `CAMetalLayer` that is handed to the core for drawing, with its position and size following the layout; the window background is made transparent, and the backgrounds of the window and of each layer leave a hole where the video is, so controls placed over the video are still drawn above it. The frame clock is a thread that calls `bern_media_surface_frame` at 60 Hz, with time from `CACurrentMediaTime`. Without a media core no player can play: this target does not yet have its own media executor; Windows and Linux do not provide surfaces to the core. The standard control bar of `Video(controls: standard)` is not implemented and is reported as an error at build time.
- **Language**: chosen from the system's preferred languages; switched at runtime through `LanguagePicker` or `ui.set_language`.
- **External entry points**: `ui.inspect`, `ui.activate`, `ui.edit`, `ui.submit`, `ui.set_bool`, `ui.select`, `ui.language`, `ui.set_language`; other session commands return `ui_unavailable` immediately. The control list is obtained by building the window content once more when the request arrives, so it reflects the current state even when the window is covered; listing does not open query subscriptions.

### `BERN_WINDOW_OFFSCREEN`

For development checks. The window opens outside all displays (-10000, -10000), the app is set to accessory (not in the Dock), and the window does not take focus. The purpose is checking on a computer that someone is using: the window covers nothing and does not steal the keyboard; and since it cannot be covered by other windows, it keeps producing frames and can be captured with `screencapture -l <window number>`.

On macOS, when a window is completely covered by other windows, GPUI stops its display link: it neither redraws nor handles clicks, and only catches up on drawing when it is uncovered again. This is why, during the first on-screen check, the window looked unchanged after the state was changed through the CLI.

## Measured

### A large media app (2026-10-30)

The same declaration, unchanged; the test copy sets its own `bundle_id`, and at runtime `HOME` points to an empty directory, so the data directory is separate from the real app's.

- Check errors dropped from 1749 to 0 (only warnings, such as for motion and `MenuBar`, remain).
- Build: the app, CLI, MCP server and skill are all produced; the engine is compiled together with its own media core.
- Run under `BERN_WINDOW_OFFSCREEN` with screenshots, operated through the app CLI:
  - the four steps of the first-launch onboarding dialog: the welcome page (checkered pattern, vector mascot, wordmark, wrapped description), choosing a starting point (option cards, selected stroke and checkmark, module switch capsules, a small window preview that changes along), appearance (four theme color dots, light/dark segments), and ready;
  - the main window's sidebar and toolbar (search panel entry, "Open" button), today's updates on the home page (http posters, the timeline and the "now" line, horizontal scrolling opened by `open_center`), the new-season page's weekly accordion with a preview list for each day, the library page (segments, filter capsules, sliders, sort menu, empty state), and the downloads page;
  - the search panel: it opens, and after typing lists results by group, with the first item highlighted.
- Problems fixed during the check: stack containers only checked whether the first layer filled the space (the window content filled only the upper half); horizontal scroll views also filled the height; scroll content was compressed; dialogs were laid out by their declaration site instead of the window (squeezed into a vertical line); rows and text did not wrap; roles such as `fill.subtle` were drawn in magenta; buttons with content had an extra ring of button look; the button style's foreground was not passed to icons; sidebar items and search in disabled layers were still reported as enabled.

### RustSend: spinning logo, light mode, menus and icon (2026-10-31)

The test copy replaces the receive page's `Svg` (which used the document engine only for spinning) with a bitmap plus `spin: app.ready && !app.pending, spin_period: 15`; the logo is monochrome and goes through the GPU. Built as an `.app` bundle and run under `BERN_WINDOW_OFFSCREEN`:

- In two window screenshots taken two seconds apart, 4366 pixels differ in the logo area and none in the text area; while spinning, the whole process uses about 12–15% of one core (debug build), mostly for rebuilding the whole window every frame.
- `ui select` changes the appearance in settings to "Light"; the window turns light and the settings file gets `light`; after changing back to "Follow System" it follows the system (this Mac is dark).
- Reading the menu bar through System Events: Apple menu, RustSend, File, Edit, Window; every item's name, enabled state and keyboard shortcut (⌘Q, ⌥⌘H, ⌘W, ⌘C, ⌘Z, ⌘M) are correct, and the Edit menu is grayed out when no text input has focus.
- The bundle's icon drawn with `NSWorkspace` (copied to a new path and registered again): the logo sits on the system's rounded icon plate.

### RustSend (2026-10-30)

With RustSend 1.0's UI declarations unchanged and the `gpui_macos` target added, the check passes, and the debug build takes 2 min 41 s (fresh target directory). After the fixes for the app above, it was rebuilt with the same runtime, and the screenshots of the receive and settings pages are correct. The test copy sets its own `bundle_id` and uses `RUSTSEND_DATA_DIR` to point to an empty data directory (its engine reads and writes `~/Library/Application Support/localsend-bern` by default, regardless of `bundle_id`). Run under `BERN_WINDOW_OFFSCREEN` with screenshots:

- the four pages Receive, Send, Transfer history and Settings display in the compact layout, with the logo (the animated SVG drawn into an image at build time), icons, segmented picker, popup button, switches, progress bar and form sections all present;
- `ui select` switches pages and settings categories, `ui activate` opens the network info popover, `ui set-bool` turns on quick save, after `ui edit` changes the name "Save" becomes enabled, and after saving with `ui activate` the name in the engine changes accordingly;
- `ui set-language` switches to English, and the dark appearance is switched on through the engine state; the monochrome accent color is correct in both appearances;
- with the compact breakpoint temporarily set to 400, the desktop layout (title row, large page titles) displays as well.

### Minimal app (2026-10-30)

Apple M4, 16 GB, macOS 27.2, Rust 1.99.

**Minimal app**: the counter generated by `bern new`, with the `gpui_macos` target added:

- `bern check --target gpui_macos` passes;
- debug build (fresh target directory, including compiling GPUI) in 50 seconds;
- run under `BERN_WINDOW_OFFSCREEN`, with window screenshots:
  - the title, the count and the two buttons display in Simplified Chinese;
  - after `counter add-one` is run three times through the app CLI it shows 3, and after `counter reset` it shows 0;
  - `ui inspect` lists `main.add_one` and `main.reset`, with the labels "加一" and "重置";
  - after `ui activate --target main.add_one` twice, both the engine state and the window show 2;
  - a nonexistent target returns `target_not_found`;
  - throughout the check the frontmost app never changed, and the test window never took focus.

Release build of the same app (LTO, `codegen-units = 1`, stripped):

| Item | Result |
| --- | --- |
| Compile time | 3 min 27 s, cold build, including GPUI |
| App program | 6.6 MB |
| CLI, MCP server | 0.9 MB each |
| Memory (`footprint`) | 73 MB on first launch (while compiling shaders), 29 MB afterwards |

**Probes**: a counter window using only GPUI and a window with `gpui-component` added, both LTO release builds compiled without `font-kit`, so they draw no text, and neither size nor memory includes the text system.

| Probe | Compile time | Size | First frame | Memory |
| --- | --- | --- | --- | --- |
| GPUI only | 1 min 47 s (cold build) | 5.3 MB | 1.8 s on first launch, about 0.2 s afterwards | 70 MB the first time, 26 MB afterwards |
| With `gpui-component` | another 3 min 59 s on top of the previous row | 14.0 MB | about 0.14 s | 27 MB |

The slow first launch and the high memory come from compiling the Metal shaders (`runtime_shaders`).

## Not verified and not implemented

- **Video playback**: no media has actually been opened; the core's drawing, the backgrounds left open and the frame clock are all not verified.
- **Real pointer and keyboard**: no clicks were measured; button clicks and `ui.activate` share the same closure, but have not gone through real mouse events. Clicks sent to the process with `CGEventPostToPid` during the check were not handled while the app was not frontmost. Scale states, hover highlights, the search panel's arrow keys, `over` for drag and drop, pinch and `FocusRegion` are only compiled, not measured.
- **Other platforms**: never compiled on Windows and Linux; neither the generated app nor `gpui-pre` has been built for these two platforms; embedded devices have not been tried.
- **Appearance**: the system accent color is not wired in; the contrast preference has not been looked at; switching under "Follow System" when the system appearance changes at runtime has not been looked at (the checks do not change system settings); switching the app icon has not been looked at, and the icon switched in is shown through `setApplicationIconImage`, without the system's icon plate.
- **Spinning**: the frame-by-frame rotation of multi-color images on a background thread is only compiled, not measured.
- **Windows**: the frontend does not report `bern_window_facts`; the window's `role`, `presentation` and close confirmation are not implemented; `immersive` only hides the toolbar and the pointer when idle, and the title bar buttons remain; the app's declared `MenuBar` is not shown (only the system's standard menus).
- **External entry points**: `ui.select` matches options by their display text, while the app CLI's help describes enum variant names; multiple instances of the same component (for example the four option cards) have the same control identifiers.
- **macOS outputs**: `bern package` does not support this target, and there is no release signing or notarization; the icon in the Dock has not been checked in a screenshot (an app running offscreen is not in the Dock).
- **Other**: accessibility (GPUI depends on AccessKit, but the controls do not provide roles and names yet), the standard control bar of `Video`, a platform media executor, and motion other than `spin` are not implemented.
