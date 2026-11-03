**English** · [简体中文](../zh-CN/Adaptation/Apple.md)

# Apple platform adaptive UI

- Checked on: 2026-10-09 (Asia/Shanghai); the sources for section 9 were checked on 2026-10-10.
- Bern interface notes updated: 2026-10-17 (containers, grouping and fallbacks for older system versions).
- Scope: macOS, iOS / iPhone, iPadOS / iPad, tvOS / Apple TV, and the announced iPhone Duo.
- Related: [Bern framework design](../Design.md). This document is the basis for backend design and acceptance; it does not mean the backends concerned are already implemented.
- **Official basis** states rules or capabilities Apple has published; **Bern decision** states how this project maps them. The two are not interchangeable.
- The source status "Full text" means the official body text or the video transcript was read; some HIG body text was obtained through the official page data. "Search results only" must not on its own support a hard requirement.
- System versions in this document qualify specific capabilities and do not constitute a commitment to a Bern minimum system version; the minimum version, the SDK and availability branches are locked separately by the target module.

## 1. Adapt to the current scene's space and capabilities

**Official basis.** Apple requires layouts to respond to the actual space, size classes, safe areas, text size and localization changes; orientation or the device name cannot stand in for these inputs. Layout adjustments should keep features reachable. [Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout)

**Bern decision.** The Apple adapter provides at least the following independent pieces of environment information; the field names are design concepts, not yet implemented APIs.

| Input | Purpose | Forbidden simplification |
| --- | --- | --- |
| Logical bounds of the current scene / window | Choosing the number of columns, the reading width and the primary–secondary relationship | Laying out by the size of the whole display |
| Horizontal and vertical size classes | Plugging into the layout semantics of native containers | Replacing them with a single `isTablet` |
| System safe areas | Protecting body text, interaction and subtitles | Fixed top and bottom margins on every device |
| Reserved, separating and occluded regions | Expressing folds and irregular usable space | Recording only the four edge insets |
| Text scaling and accessibility preferences | Reflow, contrast and interaction feedback | Only scaling the whole canvas proportionally |
| Current input capabilities | Touch, pointer, keyboard, remote | Fixing one kind of input by product name |
| Appearance and system material context | Generating the theme and handling native regions | Forcibly overriding all system colors |
| Scene creation and media capabilities | Choosing native / virtual presentation | Writing capabilities as permanent platform constants |

A size change is a trusted environment fact, not a business command. Reflow must not search again, reset drafts or create a second player.

Layout may change how navigation is presented, but it must preserve the selected entity, the route, the scroll anchor, editing drafts, the logical window ID and `player_id`.

## 2. macOS: desktop windows and keyboard workflows

**Official basis.** SwiftUI window scenes support setting size, position and restoration behavior by purpose; a window title, even when visually hidden, still affects accessibility and the Window menu. The placement of video windows should also take the visible area of the screen into account. [Tailor macOS windows with SwiftUI, WWDC24](https://developer.apple.com/videos/play/wwdc2024/10148/)

The talk covers new APIs from the macOS Sequoia era. If Bern supports earlier versions, it should use corresponding alternative implementations and verify them separately; they cannot simply be treated as capabilities of every version.

**Bern decision.** macOS defaults to SwiftUI; AppKit supplements the necessary menu, window and host integration.

| Dimension | Bern adaptation decision |
| --- | --- |
| Window | `Window(presentation: auto)` prefers a native scene; system windows are bound per logical instance, and the title does not serve as the ID |
| Navigation | Wide windows can place the sidebar and the detail side by side; narrow windows keep the current detail and a usable way back |
| Menus | The app menu, the Window menu and keyboard shortcuts reference the same command catalog; the enabled state comes from the current session |
| Size | Provide sensible defaults and a minimum usable layout; the system's final position and size and screen changes are synchronized through events |
| Input | Support keyboard traversal, keyboard shortcuts, the pointer and contextual actions; dragging cannot be the only way to complete something |
| Theme | Use system semantic colors, materials and standard control styles, with the brand color mapped to the accent color; Bern uses the shared theme algorithm only for self-drawn parts |
| Video | The player is independent of the window; closing a presentation should not wrongly destroy an app-level player |

**Official basis.** Keyboard navigation on macOS and iPad is affected by system settings; focus semantics change with the input type and the scene. SwiftUI focus APIs also have system version limits. [The SwiftUI cookbook for focus, WWDC23](https://developer.apple.com/videos/play/wwdc2023/10162/)

**Bern acceptance.**

- After the window is repeatedly shrunk and enlarged, content and actions remain reachable; no key fixed action depends on the bottom edge of the screen.
- When the main window and the settings window exist at the same time, keyboard shortcuts and menus act on the correct session.
- Window restoration, reopening and repeated opening all keep the instance policy, and temporary info windows are not restored by mistake.
- When closing is requested through the system's close entry point, verify the handling of dirty drafts, cancellation and invalidated asynchronous confirmation.
- Verify focus, accessible names and activation with Keyboard Navigation turned on and with it turned off.
- When the display changes or the space taken by the menu bar / Dock changes, windows can still return to the visible area.

## 3. iOS / iPhone: touch and navigation continuity

**Official basis.** Top-level destinations, hierarchical navigation and modal tasks carry different responsibilities for organizing information. [Explore navigation design for iOS, WWDC22](https://developer.apple.com/videos/play/wwdc2022/10001/)

This source can support navigation semantics, but its 2022 visual examples cannot dictate that navigation on all future iPhones is fixed at the bottom; for adapting to iPhone Duo's side controls, see section 6.

**Official basis.** Dynamic Type uses scalable text styles; fixed heights easily cause truncation, so wrapping, growth and reflow should be allowed. [Get started with Dynamic Type, WWDC24](https://developer.apple.com/videos/play/wwdc2024/10074/)

**Bern decision.** iOS defaults to SwiftUI; the phone layout first ensures that the current task is operable, rather than shrinking the desktop interface.

| Dimension | Bern adaptation decision |
| --- | --- |
| Window | Query the current scene creation capability; when it is unavailable, use a full-area active virtual window plus a switcher |
| Navigation | Declared by destination, hierarchy and task role; the native target decides where navigation goes in the current environment |
| Back | The back gesture, the visible back control and external entry points use the same session navigation semantics |
| Size | Read the current scene, the keyboard and the safe areas; with the low height in landscape, multiple columns cannot be chosen by width alone |
| Input | Touch target size is separate from visual icon size; allow the extra input that external keyboards and pointers bring |
| Theme | Changes in system appearance, contrast and materials can update Bern's control recipes |
| Video | Full screen, picture-in-picture and foreground/background playback negotiate capabilities separately; support is not inferred from the presence of `Video` |

**Bern acceptance.**

- In the smallest supported window and in landscape, forms, subtitles, error messages and primary actions are not clipped.
- When the keyboard appears or disappears or the input method is switched, the item being edited stays visible and drafts are not lost.
- At the largest supported text size, check wrapping, scrolling, label association and the order of actions.
- After a back interaction is cancelled, the route, focus and command result agree, and navigation is not performed twice.
- After a window switch, the same detail and playback session continue; background restrictions are explicitly reflected in state.
- VoiceOver can identify every icon-only action, and the reading order matches the current layout.

## 4. iPadOS / iPad: resizable windows and mixed input

**Official basis.** The iPadOS 26 design includes resizable windows, window controls, a menu bar and pointer interaction; sidebars and tab bars need to adjust to the space, and window changes should keep the user's current context. Where a document workflow applies, a new document can open in its own window with a clear title. [Elevate the design of your iPad app, WWDC25](https://developer.apple.com/videos/play/wwdc2025/208/)

These specific window and appearance capabilities are bounded by iPadOS 26 and should not be projected onto all earlier versions.

**Bern decision.** iPad reuses the business protocol and the declarations, not "an enlarged fixed phone layout".

| Dimension | Bern adaptation decision |
| --- | --- |
| Window | Map to multiple scenes when the native capability is available; each window has its own navigation, focus and drafts |
| Navigation | Primary and secondary columns can sit side by side, collapse and be restored; features in a collapsed area remain reachable through an entry point |
| Menus | The menu bar, keyboard shortcuts and contextual entry points are bound to the same commands, with no separately maintained business execution path |
| Size | Reflow by the actual window bounds; leave room for the system window controls so that self-drawn toolbars do not cover them |
| Input | Touch operation is complete, and pointer hover and the keyboard are supported as well; hover cannot reveal the only entry point |
| Theme | Native materials can be kept within their bounds |
| Video | A wide space shows the video together with supplementary content; when shrinking, the player identity and the current control focus are kept |

On the same iPad, a small window, full screen and different external display scenes may need different layouts; "iPad" is not a fixed breakpoint.

The system allowing multiple windows does not mean Bern can force arbitrary desktop-style positions. A native creation request can fail, and a failure must not leave behind an "opened" ghost instance with no system handle.

**Bern acceptance.**

- Shrinking continuously from a wide window to a narrow one and restoring it preserves the selected content, the back relationship and drafts.
- Toolbars avoid the window controls; popup menus and auxiliary windows can still be closed after shrinking.
- Touch, keyboard and pointer each complete the same primary Feature, without relying on hover hints.
- Two document windows have distinguishable titles, and command arguments contain the correct window / session target.
- After the system switches scenes, closes a window or restores a window, subscriptions and resource ownership are correct.
- iPadOS 26 and the minimum supported version are accepted separately; behavior that exists only in the new version has a visible alternative path.

## 5. tvOS / Apple TV: remote focus first

**Official basis.** tvOS supports directional navigation with the remote through focus; focus movement and keyboard Tab traversal are not the same mechanism. [The SwiftUI cookbook for focus](https://developer.apple.com/videos/play/wwdc2023/10162/)

**Official basis.** The Layout HIG gives safe area guidance for TV of 60 pt from the top / bottom and 80 pt on the left and right for primary content, and asks that the space needed for focus enlargement be taken into account. [Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout)

**Bern decision.** These values are a design baseline for TV content and need to be checked against the actual safe area provided by the system container; the same margins must not be added again outside a container that has already applied the safe area.

| Dimension | Bern adaptation decision |
| --- | --- |
| Window | By default, logical windows are presented as a single active content plus a navigable switcher; multiple small floating desktop windows are not simulated |
| Navigation | The directional focus graph, the back hierarchy and the initial focus are explicit; returning from an overlay restores focus to a valid control |
| Size | Redo the text hierarchy, content density and spacing for viewing from a distance, instead of enlarging the phone interface as a whole |
| Input | The remote completes playback, seeking, selection and back; optional input devices use the same semantic actions |
| Theme | Check light and dark appearance, video backgrounds and focus states together; focus cannot rely only on subtle color differences |
| Accessibility | Expose name, role, state and actions; a hidden control bar must not leave behind actionable ghost nodes |
| Video | Study the AVKit system experience first, then verify Bern's command interception requirements |

**Official basis.** Apple explicitly recommends that tvOS use the system media player, to suit inputs such as the remote and keep a familiar playback experience. [Create a great video playback experience, WWDC22](https://developer.apple.com/videos/play/wwdc2022/10147/)

**Bern decision.** The official recommendation of AVKit does not mean it already satisfies "every Bern UI action dispatches a command before the side effect happens". Its action interception points must be verified in a prototype; observing `rate` or the playback state after the fact cannot serve as evidence that a command was executed.

If some of the system player's built-in controls cannot satisfy the contract, the backend must clearly present the boundary and the available options. It cannot claim both "fully native controls" and "everything can be intercepted in advance" without verifying each item.

**Bern acceptance.**

- Using only the remote, no pointer or touchscreen is needed at any point from launch to completing the primary Feature.
- Every focus state is clear; scaling or animation at container edges does not clip the currently focused content.
- Long lists, empty states, load failures, overlays and focus after closing all have a definite landing place.
- The control bar can be brought back after it disappears; back does not both close an overlay and exit the detail.
- Check the command execution order and capability errors separately for play, pause, seek, speed and subtitles.
- Check readability and safe areas on actual TV output, not only on desktop simulator screenshots.

**Bern implementation (2026-10-20).** The `swiftui + tvos` target is implemented; for details and verification records, see the [tvOS implementation notes](../Implementation/tvOS-SwiftUI.md). By each dimension of the table above:

| Dimension | Current implementation | Verification |
| --- | --- | --- |
| Window | One scene that fills the screen; the main window fills it, other windows (including settings) open as full-screen pages, and the back button closes them | Settings page measured in the simulator |
| Navigation | The system focus engine; each page of a tab view is a focus section, and `default_focus` gives the preferred focus; `Shortcut(key: "escape")` is the back button, and without it back is left to the system | Focus movement and initial focus measured in the simulator; back button not verified |
| Size | `Layout(size: tv)` and `window.tv` let an app give TV its own layout; a scene drawn at a design size is scaled up to the screen with `ScaleBox`, and the controls in the scene are drawn at the design size | The TV layout of a real app measured in the simulator |
| Input | `Shortcut` and `shortcut` map to back, play/pause and touch surface directions; a slider is a focusable track that steps by direction | Directions measured in the simulator; back, play/pause and a real remote not verified |
| Theme | The focus effect of system controls (lift, turning white, shadow); capsule and circular buttons in a scaled scene show black text on white when focused | Dark appearance measured in the simulator; light appearance not looked at in practice |
| Accessibility | Names and roles of system controls; a focused slider is an adjustable element; `Pinch` and `Swipe` keep only the accessibility adjustment | Not measured with VoiceOver |
| Video | Same as iOS: an AVFoundation executor presented with `AVPlayerLayer`, with the control layer driven by Bern commands, so actions go through a command before they change playback; AVKit's system player is not used, and its interception points still await a prototype | Playback, rewind and fast-forward measured in the simulator; HDR not verified |

The acceptance item "check readability and safe areas on actual TV output" has not been done yet; so far there are only simulator screenshots.

## 6. iPhone Duo: dynamic adaptation for an announced device

### Official status and version boundaries

As of the check date, iPhone Duo **has been officially announced by Apple but is not yet on sale**. The official product page lists preorders starting 2026-10-16 and availability on 2026-10-23; as of this date it cannot be written up as a rumor, nor can it be described as already verified on a real device. [Product page](https://www.apple.com/iphone-duo/), [official press release](https://www.apple.com/newsroom/2026/09/apple-unveils-iphone-duo/)

The official preparation guide requires the iOS 27.1 SDK / Xcode 27.1 for the full edge layout experience, and introduces opening and closing, rotation and fold testing in Device Hub; running in compatibility mode does not mean being fully adapted. [Prepare your app for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111461/)

### Size, folding and navigation

**Official basis.** The outer and inner displays have different base size classes, and layouts should read the actual size class and scene geometry. Features and state are continuous across displays and postures; an unfolded iPhone app cannot simply be equated with an iPad app. [Designing for iPhone Duo HIG](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo), [Prepare your app](https://developer.apple.com/videos/play/tech-talks/111461/)

System navigation and toolbars can move to the side, and Split View also involves placement on the outer side; pinned picture-in-picture and partial folding change the remaining content space. [Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/)

Fold adaptation should recognize reserved regions and use the arrangement / region layout mechanisms to avoid the crease and the camera area, rather than guessing physical angles and pixel boundaries. [Strike a pose with adaptive layouts on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111463/)

**Bern decision.**

- The shared layout expresses "the relationship between primary and secondary content", and the Apple target maps it onto the region mechanisms.
- Safe areas and internal separations are modeled separately; it is not enough to add padding around the window.
- A bottom tab bar is not written into the shared navigation semantics; the accessible order needed for presentation on the side is provided.
- No new app session is created because of the inner display's size; the player, navigation and editing state keep their original identity.
- When posture information is missing, adapt to the actual usable space, and avoid fabricating fake crease regions from the device model.
- Do not presuppose screen sizes, fold angle thresholds or camera occlusion coordinates; these are not Bern cross-platform constants.

### Boundaries of multiple scenes and multiple displays

**Official basis.** iPhone Duo supports multiple UI scenes of the same app, but new scenes can only be created on the inner display, not on the outer one; availability changes, and request failures must be handled. Hinge data is suited to interaction and effects, while layout uses the region mechanisms. Scene accessories are also subject to dynamic availability and are not a permission for arbitrary dual-screen windows. [Leverage multiple displays and scenes on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111464/)

**Bern decision.** `native_window_creation` is a capability of the current environment; it cannot be permanently set to false by `platform == iOS`, nor permanently set to true for the outer display because the Duo inner display supports it.

`Window(presentation: auto)` reads the current capability before it starts creation, and handles the race condition in which the capability changes after the check. When an accepted creation request fails, the virtual fallback is performed according to the declared contract only if the capability is insufficient and the partially created native objects have been cleaned up; other failures keep their actual error category, and the logical instance must not be created twice.

`native_required` returns a structured capability error when the capability is insufficient; other creation failures, such as resource or configuration failures, keep their actual error category. Switching to the outer display is not the user requesting that existing logical windows be closed; scene destruction and the content retention policy must be handled separately.

A scene accessory is an accessory presentation capability; it cannot be automatically mapped to every `Window` being able to run independently on the outer display. The specific conditions of camera-related accessories also cannot be generalized into a multi-display capability for ordinary video.

**Bern acceptance.**

- Going from the outer display to the inner display and back preserves the current detail, drafts, the focus restoration position and `player_id`.
- Both sides of Split View on the inner display are tested separately; navigation and toolbars do not land on an inoperable inner edge.
- After pinned picture-in-picture appears / disappears, the low-height layout can still complete the current task.
- Under combinations of partial folding and rotation, primary controls, body text and subtitles avoid the effective reserved regions.
- Requesting a new native scene on the outer display gets an explicit capability result; a creation failure on the inner display does not lose instance state either.
- Switch between the inner and outer displays during a creation request to verify the handling of races, fallbacks and duplicate events.
- When accessory availability changes, the entry point state and the presentation stay in sync; there is no activatable action without content.
- SDK / simulated environment checks and the final real-device acceptance are recorded separately; "passed on a real device" is not filled in before release.

## 7. Theme, accessibility and icon buttons

**Official basis.** System semantic colors depend on appearance, contrast and materials; brand colors also affect users' judgment of what is interactive. [Color HIG](https://developer.apple.com/design/human-interface-guidelines/color)

**Official basis.** An accessible interface needs to support larger text, sufficient contrast, cues that do not rely on color alone, and VoiceOver; self-drawn or rebuilt controls need to keep label, role and action semantics. [Accessibility HIG](https://developer.apple.com/design/human-interface-guidelines/accessibility), [Catch up on accessibility in SwiftUI, WWDC24](https://developer.apple.com/videos/play/wwdc2024/10073/)

**Bern decision.** Apple platforms have a complete system of light and dark mode and semantic colors. The SwiftUI target uses system semantic colors, materials and standard control styles, and by default does not layer custom background colors on system surfaces such as windows, sidebars and forms, except for style overrides the app declares; a brand color declared by the app is mapped to tint or AccentColor, and when none is declared, the system accent color the user chose is used. The shared Rust theme algorithm is used only for controls and surfaces that Bern draws itself.

IconButton uses the platform's standard icon button style by default, and in system-managed presentation locations it follows the platform rules, for example the background that a toolbar provides uniformly for toolbar items. `shape: capsule` / `height` on `Button`, `Menu`, `Picker`, `Switch` and `SplitButton`, and `ControlGroup`, express containers, sizes and functional grouping; from macOS / iOS 26 the system `glassEffect` can be used, and older systems fall back to a capsule of system material. A container the app explicitly declares must not be removed for the sake of keeping the default style; ordinary icon actions can explicitly use plain. For the current interface, see [component usage](../Reference/UI.md).

Using standard components does not make accessibility automatically compliant; the clarity of macOS keyboard focus and tvOS remote focus still has to be verified separately.

**Bern shared acceptance.** Check names, roles, states and actions with the actual accessibility tree and real operations; cover larger text, light and dark mode, Increase Contrast and Reduce Motion as well. Comparing colors in screenshots alone is not enough to prove usability.

## 8. Commands, system facts and media boundaries

**Bern decision.** The navigation, menus, icons, focus movement and playback controls that Bern itself provides must be bound to registered commands; UI, CLI, JSON and MCP drive the same engine and an explicit target session.

Window size changes, system focus transfers, media decoding and lifecycle changes that the system has already carried out are received as trusted events. A "user command" must not be fabricated after the fact, and the same operation must not be performed again in response to the event, which would form a loop.

Whether a native interaction can be intercepted before its side effect has to be checked control by control. For gesture navigation, system window closing and AVKit controls, tests should distinguish the request, allow / deny, system completion and the event report.

The media backend is independent of the SwiftUI target. `Video(source: …)` and `Video(player: …)` both use the same media protocol; a layout change only reattaches the presentation and does not quietly change resource ownership.

The system's external media entry points, the app's own control bar and the system player's built-in controls each record their source and support boundaries separately; the three must not be lumped together as "native playback".

**Bern decision (progress and volume in the standard control bar).** Progress and volume in `controls: standard` use a dedicated media progress bar and do not reuse the look of the `Slider` component: the played part is a neutral `.primary` and does not use the app accent color. On macOS 26 and later the system `Slider` is used directly, with `.tint(.primary)` to get a neutral played part, and the thumb keeps the system look; below macOS 26 a fallback drawn by Bern is used, shaped after the macOS 26 system slider: a 6 pt rounded track (the unplayed part is `quaternaryLabelColor`) and a 20 × 16 pt white rounded thumb (7 pt corner radius) with a soft shadow and a thin outline. The fallback reports itself to accessibility as a system `Slider` and accepts arrow-key adjustment; dragging only updates the preview, and releasing commits a single seek, the same as the end of editing in the system `Slider`. These sizes come from the actual rendering of the system `Slider` on macOS 27.0.1 on the local machine (2026-10-10), not from a specification Apple has published.

## 9. Bundled resources

**Official basis.** A macOS app bundle puts content that is not code in `Contents/Resources/`; executable code, frameworks, plug-ins and so on each have a fixed location, and a wrong location may only surface as a signing problem at notarization. The same document explains that the signing tool treats directories whose names contain a dot as bundles (stated for subdirectories of nested code). [Placing content in a bundle](https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle)

**Official basis.** `Bundle.resourceURL` returns the file URL of the subdirectory in the bundle that holds resources, and is available since macOS 10.6; non-standard bundle formats may have no usable path. [Bundle.resourceURL](https://developer.apple.com/documentation/foundation/bundle/resourceurl)

**Bern decision.** For resource declarations and the engine's resolution rules, see "Bundled resources" in the design document; the mapping for the macOS SwiftUI target is as follows.

| Dimension | Bern adaptation decision |
| --- | --- |
| Location | The resource `a/b.ext` is copied to `Contents/Resources/a/b.ext`; `<language>.lproj` is left to the generated localization tables |
| Name | Directory segments cannot contain `.` (file names can), which avoids both `.lproj` and the signing tool's bundle detection; a violation makes the build check report an error that points at the `Bern.toml` entry |
| Copying | Only file contents are copied, without extended attributes; copying finishes before signing; old resources that are no longer declared are deleted, and generated localization files are kept |
| Resolution | The Swift host passes `Bundle.main.resourceURL` when it starts the engine, the generated Rust glue carries the catalog of declared names, and the two are assembled before the engine is constructed; the engine gets file URLs inside the bundle and does not depend on paths on the build machine |
| Scope | Only data files are published; executable code, frameworks and plug-ins need their own locations and signing, and are not supported as resources in the first version |

The bundle structure of other Apple platforms is mapped separately according to the official documentation when the corresponding target is implemented; the app's resource declarations do not change.

**Bern acceptance.**

- With the app copied outside the build directory and the resource files moved out of the source tree, declared resources still load from inside the bundle.
- An undeclared name returns `undeclared_resource` even if a file of the same name exists in the bundle.
- After a resource is removed from the manifest and the app is rebuilt, no old file remains in the bundle, and signature verification passes.

## 10. Official sources and reading status

All of the following entries were accessed on the check date; "Full text" only means the evidence in the material was read, not that Bern has implemented it or passed tests.

| Official source | Status | Applicable scope |
| --- | --- | --- |
| [Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout) · [official body text data](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/layout.json) | Full text | Across Apple platforms; page change log 2026-09-09 |
| [Accessibility HIG](https://developer.apple.com/design/human-interface-guidelines/accessibility) · [official body text data](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/accessibility.json) | Full text | Across Apple platforms; page change log 2025-06-09 |
| [Color HIG](https://developer.apple.com/design/human-interface-guidelines/color) · [official body text data](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/color.json) | Full text | Across Apple platforms; how materials look depends on the OS and the context |
| [Tailor macOS windows](https://developer.apple.com/videos/play/wwdc2024/10148/) | Full text | WWDC24; new window APIs are checked against macOS Sequoia and availability |
| [SwiftUI cookbook for focus](https://developer.apple.com/videos/play/wwdc2023/10162/) | Full text | macOS, iPadOS, tvOS; some APIs from iOS 17 / macOS Sonoma |
| [Explore navigation design for iOS](https://developer.apple.com/videos/play/wwdc2022/10001/) | Full text | WWDC22; navigation semantics can be referenced, the old visual examples are not rules for all current devices |
| [Get started with Dynamic Type](https://developer.apple.com/videos/play/wwdc2024/10074/) | Full text | WWDC24; text scaling and layout verification |
| [Catch up on accessibility in SwiftUI](https://developer.apple.com/videos/play/wwdc2024/10073/) | Full text | WWDC24; SwiftUI accessibility semantics |
| [Elevate the design of your iPad app](https://developer.apple.com/videos/play/wwdc2025/208/) | Full text | Windows, menus and appearance in iPadOS 26 |
| [Create a great video playback experience](https://developer.apple.com/videos/play/wwdc2022/10147/) | Full text | iOS, iPadOS, tvOS; the system player and input, does not prove Bern's interception capability |
| [iPhone Duo product](https://www.apple.com/iphone-duo/) · [press release](https://www.apple.com/newsroom/2026/09/apple-unveils-iphone-duo/) | Full text | Announced in September 2026; not yet on sale when checked |
| [Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo) · [official body text data](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/designing-for-iphone-duo.json) | Full text | Guide added 2026-09-09; iPhone Duo |
| [Prepare your app for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111461/) | Full text | Preparation and simulated testing with the iOS 27.1 SDK / Xcode 27.1 |
| [Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/) | Full text | Duo postures, side controls and content space |
| [Strike a pose with adaptive layouts](https://developer.apple.com/videos/play/tech-talks/111463/) | Full text | Duo region and arrangement mechanisms; specific symbol versions are rechecked at implementation time |
| [Leverage multiple displays and scenes](https://developer.apple.com/videos/play/tech-talks/111464/) | Full text | Duo hinge, dynamic multiple scenes and accessory presentation |
| [Placing content in a bundle](https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle) · [official body text data](https://developer.apple.com/tutorials/data/documentation/bundleresources/placing-content-in-a-bundle.json) | Full text | Checked 2026-10-10; where each kind of content goes in a macOS bundle and the effect on signing |
| [Bundle.resourceURL](https://developer.apple.com/documentation/foundation/bundle/resourceurl) · [official body text data](https://developer.apple.com/tutorials/data/documentation/foundation/bundle/resourceurl.json) | Full text | Checked 2026-10-10; macOS 10.6+ and other platforms |
| [Designing for macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos/) | Search results only | The HTML requires JavaScript; this document's hard conclusions for macOS are supported by material that was read |
| [Designing for iOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-ios) | Search results only | The HTML requires JavaScript; the index was not treated as the complete body text |
| [Designing for iPadOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-ipados) | Search results only | The HTML requires JavaScript; specific new capabilities are taken from the WWDC25 full text |
| [Designing for tvOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-tvos) | Search results only | The HTML requires JavaScript; focus, safe areas and video are taken from full texts that were read |

## 11. Project decisions that must be settled before implementation

- The SwiftUI target lists versions and capabilities separately for macOS, iOS, iPadOS and tvOS, and does not use one master switch to represent Apple support.
- The environment protocol keeps irregular regions and dynamic scene capabilities; Apple type mappings stay in the target module, and the shared IR is not bound to UIKit symbols.
- Native multiple scenes, virtual windows and accessory presentation each verify identity, closing, restoration and resource ownership separately.
- First verify navigation gestures, window closing and the player command order in a prototype, then commit to supporting the corresponding standard controls.
- Turn each platform's acceptance items in this document into a backend scenario list; devices without completed real-device verification are explicitly labeled with their evidence level.
