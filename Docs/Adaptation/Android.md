**English** · [简体中文](../zh-CN/Adaptation/Android.md)

# Android, foldable and Android TV adaptive UI

- Checked on: 2026-10-09.
- Nature of the document: a basis for Bern's architecture preparation and backend acceptance; interfaces, layout strategies and tests that are not yet implemented are all design requirements.
- Scope: Android phones, tablets, foldables, resizable windows and external input, and Android TV / Google TV.
- Related: [framework design](../Design.md). Jetpack Compose remains the default frontend technology on Android; the media backend is chosen separately.
- Implementation status: the first version of `compose + android` is implemented and verified on a phone (the media backend is Media3); see the [Android Compose implementation notes](../Implementation/Android-Compose.md). The rules for tablets, foldables, windowing and Android TV are not yet implemented or verified, and those parts of this document are still design requirements.
- Evidence boundary: "Official basis" below describes facts from the sources; "Bern decision" is the framework's mapping of those facts and does not pose as Google's guidelines.

## 1. The input to adaptation is the current window

**Official basis.** Android divides the window currently available to the app into size classes, and looks at width and height separately; the classes change with rotation, split screen and folding. `isTablet`, the whole physical screen or the device model cannot be used in their place. [Window size classes](https://developer.android.com/develop/adaptive-apps/guides/use-window-size-classes)

| Dimension | Current official class | Bounds |
| --- | --- | --- |
| Width | Compact | `< 600 dp` |
| Width | Medium | `600 ≤ width < 840 dp` |
| Width | Expanded | `840 ≤ width < 1200 dp` |
| Width | Large | `1200 ≤ width < 1600 dp` |
| Width | Extra-large | `width ≥ 1600 dp` |
| Height | Compact | `< 480 dp` |
| Height | Medium | `480 ≤ height < 900 dp` |
| Height | Expanded | `height ≥ 900 dp` |

The values in the table come from the official page above; the "≥ 840 dp is all Expanded" of older material can no longer be used as the current complete classification. When the width is sufficient but the height is very low, whether multiple columns are usable still has to be evaluated.

**Bern decision.** The Android adapter reports the window's logical size, density, font scale, system occlusions and posture; the shared layout rules consume these capabilities and sizes. The breakpoints above belong to the Android strategy and do not become hard-coded thresholds for all platforms.

The default strategy first chooses the layout relationship, then computes column widths and margins. A larger screen should add information, place related content side by side or improve the reading width, not stretch the same phone form indefinitely.

The current Android documentation shows both `currentWindowAdaptiveInfo(supportLargeAndXLargeWidth = true)` and the newer `currentWindowAdaptiveInfoV2()`. The backend implementation has to pin the actual dependency and its stability, and cannot promise the latest example name on the website directly as Bern's minimum library version. [Size API](https://developer.android.com/develop/adaptive-apps/guides/use-window-size-classes), [Posture API](https://developer.android.com/develop/adaptive-apps/guides/foldables/make-your-app-fold-aware)

## 2. Layout relationships, navigation and content continuity

**Official basis.** The official canonical layouts include list–detail, content grid, and main content–supporting pane. List–detail places the two side by side when there is enough width; when the window shrinks, it keeps the detail the user is currently viewing and shows the list again after back. [Canonical layouts](https://developer.android.com/develop/adaptive-apps/guides/canonical-layouts)

**Bern decision.** Provide semantic adaptation for these relationships, without requiring the business code to write a "phone page" and a "tablet page" separately:

| Content relationship | Narrow window | Wide window | Must be kept |
| --- | --- | --- | --- |
| Media list–detail | Current page and back stack | List and detail side by side | Selected ID, list anchor, detail route |
| Video–playlist | Video first, list opened on demand | Main player plus supporting pane | The same `player_id`, current episode, playback position |
| Poster content feed | A few readable columns | More columns according to the minimum card width | Stable item keys, filters and scroll anchor |
| Settings and forms | Bounded reading width, natural scrolling | Categories and form side by side | Edit drafts, validation results, input focus |

A relayout must not trigger business commands such as "search again" or "reload the current movie". A change in system size is an environment fact and produces a new layout; only the user selecting content runs the corresponding business command.

The navigation entry points of narrow and wide screens may change position or presentation, but must refer to the same command and Feature. The choice among bottom navigation, navigation rail and sidebar belongs to the target strategy; a change of appearance must not be mistaken for a change of app route.

## 3. Foldables, dual screens and hinges

**Official basis.** `FoldingFeature` provides the bounds, orientation, fold state, occlusion type and whether it separates the window; a half-open horizontal fold corresponds to tabletop, and a half-open vertical fold to book. The interface does not provide the exact hinge angle. [Make your app fold aware](https://developer.android.com/develop/adaptive-apps/guides/foldables/make-your-app-fold-aware)

**Bern decision.** The environment protocol records separating regions and occlusion geometry, not a boolean for a brand or for "foldable phone":

- Convert the hinge region using the window coordinate system; native pixels are first converted to the target layout coordinates and are not used directly as `dp`.
- A region with `occlusionType = FULL` cannot carry body text, buttons, subtitles or drag handles.
- When `isSeparating` is true, prefer placing the natural divide between the regions; do not look only at whether the occlusion width is zero.
- When fold information is missing, use the ordinary layout for the current size; do not claim from this that the device has no hinge.
- The input structure supports multiple display features; layouts do not depend on "there is always only the first fold".

| Posture | Bern's initial adaptation strategy |
| --- | --- |
| Narrow outer screen / folded | Single-column navigation; keep the current detail, drafts and player identity |
| Flat large screen | Choose one column or several according to the window and the separating regions |
| Tabletop | Video on the upper part; command-based playback controls and supporting content on the lower part |
| Book / dual screens separated by a hinge | Place relationships such as list–detail in the usable regions on the two sides |

The same visual "flat" does not mean the same occlusion: a dual-screen hinge may still form a separation when flat. User controls should stay away from fold edges that are hard to reach. [Foldables and dual screens](https://developer.android.com/develop/adaptive-apps/guides/foldables/make-your-app-fold-aware)

The table above is Bern's initial strategy; apps can declare primary and supporting content roles. Folding or unfolding cannot create a second player, nor reset the current progress to the page's initial value.

## 4. System versions and multi-window boundaries

| Official behavior | Bern adaptation requirement | Official source |
| --- | --- | --- |
| Android 16 with target API 36 ignores orientation, aspect ratio and resizability restrictions in applicable environments whose smallest display width is `sw ≥ 600 dp`; there are officially listed exceptions and a temporary opt-out mechanism | Resizable by default; do not rely on a locked orientation to keep the UI correct | [Orientation, aspect ratio and resizability](https://developer.android.com/develop/adaptive-apps/guides/app-orientation-aspect-ratio-resizability) |
| Android 17 with target API 37 removes the temporary developer opt-out above | New targets cannot treat the opt-out switch as a long-term adaptation; the build matrix records the running system and the target SDK | [Android 17 behavior changes](https://developer.android.com/about/versions/17/behavior-changes-17) |
| The system provides different modes such as split screen, picture-in-picture and desktop windowing, and actual support depends on the environment | Declare the capabilities separately, rather than summing them all up with one `multi_window = true` | [Multi-window mode](https://developer.android.com/develop/adaptive-apps/guides/support-multi-window-mode) |

The `sw` condition of the system behavior here is not the same quantity as the "current app window width" in Section 1. A small window can run on a large display; the layout still goes by its own window space.

**Bern decision.** `Window(presentation: auto)` maps to a native window only when the host can actually create the required independent window and keep Bern's window contract. Otherwise virtual windows are provided inside the app; narrow screens use an active window plus a switcher, and do not simulate small desktop floating windows that are hard to operate.

System split-screen support does not mean that any app can create a second free-floating window at any time. Across windows, the same engine and the owning UI session are reused; each window keeps its own window scope, back stack, focus and drafts. Size and lifecycle changes that have already happened to native windows are reported as facts, without looping back into running commands of the same name.

The official documentation says that launching an adjacent Activity is a best-effort request rather than a guarantee of where it is created; multiple instances are also different from a multi-pane layout in one window. Bern must check the actual result of the creation before reporting that the window is open. [Multi-window and multi-instance](https://developer.android.com/develop/adaptive-apps/guides/support-multi-window-mode)

## 5. System edges, keyboard and back

**Official basis.** On Android 15 and later, once the app targets SDK 35, edge-to-edge is enforced; the adapter has to handle the actual insets such as the status bar, navigation bar, desktop caption bar, display cutout and IME. `safeDrawing`, `safeGestures` and `safeContent` protect different ranges. [Window insets](https://developer.android.com/develop/ui/compose/system/insets)

**Bern decision.** Backgrounds and video may fill the window; body text, actionable entry points and subtitles respect the corresponding safe areas. System gesture regions and pure drawing occlusions are kept separately; a fixed top or bottom margin must not be added uniformly to the whole UI.

When the IME appears, prefer keeping the current editing position and the submit entry point visible; prevent parent and child containers from consuming the same inset twice. Inside a virtual window, the host's system constraints are still inherited. Text composition, candidate selection and submission are modeled separately; each composition update during Chinese input must not be treated as a business search.

Official predictive back lets the user preview the back destination, and connects to app navigation through the system back interface. [Predictive back](https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture)

**Bern decision.** Samples of the back gesture only update the interactive preview, and on completion one typed back command is run; on cancel the preview is restored and the persistent route does not change. The back hierarchy of modals, the input method, windows and the app is coordinated by the host, and two levels are never popped at once.

## 6. Keyboard, mouse, touch and accessibility

**Official basis.** The large-screen guidelines call for keyboard Tab/arrow-key navigation, common keyboard shortcuts, the right mouse button and the scroll wheel. Complex scrolling collections need explicit focus groups and cannot rely only on the default spatial inference. [Input compatibility](https://developer.android.com/develop/ui/compose/touch-input/input-compatibility-on-large-screens)

**Bern decision.** Input methods can coexist and switch at runtime; detecting a touch screen must not hide all keyboard capabilities:

- Tap, Enter, the remote's confirm button and accessibility activation share the same action command.
- Tab order, directional focus and screen reader order are different relationships; reachability is checked for each.
- List virtualization and reordering restore focus by item ID, never to a native object that has already been recycled.
- Scroll wheel, touch scrolling and directional jumps share the scroll state; continuous input may be merged, and the final semantics are observable.
- Keyboard shortcuts such as save, undo and copy follow platform conventions; while a TextField is being edited, the text system's semantics take priority.
- Right-click menus, long press and custom accessibility actions all have to keep the feature's entry point; necessary features must not be discoverable only by hovering.

**Official basis.** Android Compose's accessibility defaults recommend interactive touch targets of at least `48 × 48 dp`, and require semantic descriptions for images or icons that cannot be understood from text. [Accessibility API defaults](https://developer.android.com/develop/ui/compose/accessibility/api-defaults)

**Bern decision.** The minimum hit area is separate from the graphic size. The Compose target uses Material's standard icon button style and provides touch feedback by platform rules; a transparent hit area ensures reachability, and focus feedback must be clear.

Controls output name, role, value, selected/disabled state and actions; decorative images are not announced redundantly. The adapter consumes font scaling and language changes, and text may wrap and grow naturally in height; text must not be shrunk back to the default size to squeeze into a fixed height.

## 7. Light and dark mode and platform themes

**Official basis.** Material 3 uses semantic color roles; dynamic color is available on Android 12 and later, and when it is unavailable, a custom light and dark scheme is the fallback. [Material 3 in Compose](https://developer.android.com/develop/ui/compose/designsystems/material3)

**Bern decision.** Compose has a complete light and dark system, so following "native first" it uses Material 3 colors: dynamic color can be used, and when it is unavailable, a Material light and dark scheme generated from the brand color or a default seed is used. Bern's theme algorithm is used only for the parts Bern draws itself; app authors do not need to write a dark branch for each control.

System bar icons, popups, selection states and input controls follow the window's resolved theme. Video pixels, posters and subtitle content are not inverted globally; text and controls above media stay legible through a controlled backing surface.

TV media apps may explicitly choose a dark strategy suited to the viewing environment, but the framework must still generate a usable light scheme. In the default system mode, when there is no appearance signal, the light fallback specified by the Bern core is used; a system dark state must not be faked.

## 8. Android TV / Google TV

### 8.1 Layout and safe areas

**Official basis.** TV is for viewing at a distance and landscape display; the development guide recommends `dp`/`sp` and placing key content inside an overscan safe area of about 5%, while backgrounds may fill the screen. The common `960 × 540` design baseline corresponds to `48 dp` left and right and `27 dp` top and bottom. [Build TV layouts](https://developer.android.com/training/tv/playback/compose/layouts)

**Conflicting sources.** Another official design page has examples of `48/27 dp`, `58/28 dp`, and `27` top and bottom rounded to `24`, side by side; these numbers cannot all be written down as one uniform mandatory standard. Bern's first version uses a safe strategy of about 5%, calibrated by testing on real TVs, rather than hard-coding one set of numbers for all resolutions. [TV Layouts original](https://developer.android.com/design/ui/tv/guides/styles/layouts)

**Bern decision.** TV is an independent interaction configuration, not the tablet UI scaled up proportionally. Main browsing uses predictable horizontal content rows or grids; text density per screen is limited, and every necessary feature can be completed with the remote. Swiping, hovering, double-clicking or fine dragging must not be the only entry point.

### 8.2 Focus and back

**Official basis.** TV navigation centers on the D-pad and the back button, with clear paths and recognizable focus; back should not get stuck in a menu loop, nor block leaving with an ordinary exit confirmation. The remote already provides back, so a duplicate on-screen back button is usually unnecessary. [TV navigation](https://developer.android.com/training/tv/get-started/navigation)

**Bern decision.** Every page, list row and popup declares a focus scope. Opening a detail remembers the source card, and back restores it first; when the source has been removed, an explicit fallback item is chosen. Once a popup has focus, the layer beneath cannot be activated by mistake. Continuous movement on long press must follow item identity, and must not make focus jump because of asynchronous poster loading.

Ordinarily leaving the app does not pop up "Exit?"; when there really are unsaved edits, the specific draft confirmation of the close contract is used. After a virtual window closes, the previous active window comes back, without relying on the remote to operate a desktop-style title bar.

The Google TV / Android TV Live tab partner integration has extra direct-back requirements, which differ from ordinary deep links; the corresponding source markers and back rules are implemented only when the app enables this integration, and are not imposed on all pages. [Live tab navigation](https://developer.android.com/training/tv/get-started/navigation#live-tab-navigation)

### 8.3 Playback controls

**Official basis.** In playback, the center button plays/pauses, a single press left or right skips backward or forward, and a long press left or right seeks continuously; up and down show progress and other information without pausing because of it. The skip duration is defined by the app; the official table uses `N`, not a uniform 10 seconds. [Playback controls on TV](https://developer.android.com/training/tv/playback/controls)

**Bern decision.** The TV recipe of `Video(controls: standard)` automatically produces the command bindings above, and distinguishes the focus contexts "controlling playback directly" and "navigating within the control bar". One key press must not both move focus and additionally fast-forward.

Seeking, subtitles, audio tracks and playback speed need focusable semantic entry points; live or non-seekable media explicitly disable the corresponding actions and report the reason. After the controls hide automatically, a definite restore position is kept, and subtitles do not fall outside the safe area because the control bar appears.

**Bern implementation (2026-11-04).** An application with a `Layout(size: tv)` is declared for Android TV: the manifest adds the leanback and touchscreen features as not required, `LEANBACK_LAUNCHER` beside the launcher category, and a banner made from the app icon. `Tabs` is a side rail, as the [navigation drawer guidance](https://developer.android.com/design/ui/tv/guides/components/navigation-drawer) describes: icons, with labels while the remote stands in it, the page following the tab the remote moves to; a tab the focus only falls onto does not change the page. The focused control grows inside a white outline of its shape. Back reaches the application as a callback and never as a key, so `Shortcut(key: "escape")` is the system's Back on a TV as on a phone. `default_focus` waits for a control that is not enabled yet. A TV counts 960 points across at 1080p where tvOS counts 1920: `window.medium` tells them apart. Measured on an Android TV 16 emulator.

**Bern implementation (2026-10-20).** The Compose backend detects a TV by the system's television mode (`UI_MODE_TYPE_TELEVISION`) or the leanback feature: `window.tv` is true, and `Adaptive` selects `Layout(size: tv)`; on a TV, `Shortcut` maps to the remote (the back button is `escape`, the play/pause button is `space`), and the arrow keys move focus first and are handed to shortcuts only when focus cannot move; `default_focus` gives the starting focus with `FocusRequester`. All of the above has only generator tests and is not verified on an Android TV device or emulator; the TV focus scopes, restoring the source card on back and the other decisions above are not yet implemented.

## 9. Player, lifecycle and system entry points

**Official basis.** After a configuration change, the app should keep its context and state, and in particular must not lose input or interrupt a continuous media experience. [Configuration and continuity](https://developer.android.com/guide/topics/large-screens/configuration-and-continuity)

**Bern decision.** The Rust engine, the player owner, the Activity, the window session and the video presentation mount have different lifecycles. Rotating, folding and resizing only remount the view; when the system actually releases resources, state can be restored, but it must not be claimed that playback continues after the process is killed.

Visible, having input focus and resumed are different states; video must not be stopped across the board whenever any `onPause` arrives. Visible players in multi-window are handled according to the product's playback policy and the system's resource constraints. [Multi-window lifecycle](https://developer.android.com/develop/adaptive-apps/guides/support-multi-window-mode)

**Official basis.** Media3's background playback approach manages the Player and Session through `MediaSessionService`, and the UI connects with a `MediaController`; the playback foreground service needs the corresponding manifest permission and the `mediaPlayback` type. [Background playback](https://developer.android.com/media/media3/session/background-playback)

**Bern decision.** System notifications, Bluetooth media keys, remote playback buttons and the app UI enter the same player command entry point. Media3 handles Controller commands directly by default, so the adapter must hook Bern's command routing in at the actual player call boundary, not merely write a command log after playback.

Buffering, ending, audio track changes and system-forced pauses observed by the player are fact events; these state updates must not send playback commands back the other way and form a loop. The system's audio focus policy may take effect before the app, and the framework should synchronize the actual state faithfully.

**Version constraints.** With target API 35 and above, requesting audio focus requires being the top app or running a foreground service. Android 17 tightens background audio further, and target API 37 should be validated against the current while-in-use and other requirements; having a resident Rust engine does not mean having authorization for background playback. [Audio focus](https://developer.android.com/media/optimize/audio-focus), [Android 17 background audio](https://developer.android.com/about/versions/17/changes/bg-audio)

**Bern decision.** The UI, CLI, JSON and MCP receive the same structured results, such as "foreground interaction/permission required" and "playback capability unavailable". Background restrictions, device loss or user refusal must not be downgraded to "command succeeded".

Some Android 17 background audio APIs fail silently, so the backend must first validate the lifecycle conditions that can be known, then distinguish "request accepted" from "state confirmed"; when audio output cannot be confirmed, no evidence of success may be faked. Diagnostic tests should cover the officially provided audio hardening test mode. [Silent failures and verification methods](https://developer.android.com/about/versions/17/changes/bg-audio)

## 10. Media server connections and implementation boundaries

**Official basis.** For Android 17 with target API 37, local network access requires the applicable permission or a system-mediated path; this affects discovering and connecting to LAN servers. [Android 17 local network permission](https://developer.android.com/about/versions/17/behavior-changes-17)

**Bern decision.** "Server not found" in apps such as Emby/Jellyfin must distinguish unauthorized, address unreachable, TLS failure, authentication failure and server not supported. The system permission prompt is triggered by the platform host; remote commands return a pending-authorization state and must not bypass the user's choice.

The framework provides generic connection states, permission capabilities, focus and player semantics; the media library model, account authentication, server selection, transcoding and protocol compatibility are implemented by app/protocol modules. API permission changes should not force the business code to rewrite every layout.

## 11. Acceptance checklist

The following are backend acceptance requirements still to be carried out; this round only completed the research and the documentation:

- [ ] Check the width strategy on both sides of 599/600, 839/840, 1199/1200 and 1599/1600 dp; verify short windows as well.
- [ ] Keep the selected item, drafts, scroll anchor and player identity on phones, tablets, inward- and outward-folding foldables, split screen and resizable windows.
- [ ] Inject horizontal/vertical hinges, FULL occlusion, separating but zero-width, and no posture data; important content and hit areas are all reachable.
- [ ] Test each combination of running system and target SDK for API 35/36/37; do not rely on a locked orientation to hide problems.
- [ ] Cover gesture navigation, three-button navigation, display cutouts, desktop caption bars and IME animation; no duplicated insets and no covered submit entry points.
- [ ] Test both completing and cancelling predictive back; popups, windows and app routes each handle it only once.
- [ ] Key flows can be completed with enlarged fonts, long translations, RTL, TalkBack, keyboard Tab/arrow keys, and mouse scroll wheel/right click.
- [ ] All themes and control states pass Bern's existing recipe checks; transparent icon hit areas do not overlap each other.
- [ ] On TV, sign-in, browsing, search, playback, subtitle/audio track selection and back are completed with the remote alone; no focus dead ends.
- [ ] On TV, test overscan, different resolutions and continuous navigation on long press; text and focus are clear at a real viewing distance.
- [ ] Each playback button triggers only one expected command; the system media entry points and the four app entry points share the actual player state.
- [ ] Test page switches, rotation, Activity recreation, going to the background, audio focus loss, service termination and state restoration.
- [ ] When LAN permission is refused or background playback is restricted, the real reason is shown, and command results match the UI.

## 12. Source reading status and items to verify

| Source group | Reading status this time | Scope adopted / limits |
| --- | --- | --- |
| Android window sizes, canonical layouts, fold-aware guide | Opened the original and read the relevant passages | Size classes, layout relationships, posture and API semantics |
| Android 16 orientation/aspect ratio/resizability guide; Android 17 behavior changes and the dedicated page | Opened the original | The running version and target SDK conditions must be kept; the dedicated page writes the threshold as `>600` and the overview as `≥600`, and Bern tests both at the boundary |
| WindowInsets, external input, accessibility defaults, Material 3 in Compose | Opened the original and read the relevant passages | System occlusions, input, hit areas, dynamic color; no claim to cover all accessibility standards |
| TV Layouts, Build TV layouts, TV navigation, Playback controls | Opened the original and read the relevant passages | The conflict in safe-area numbers is recorded explicitly in the text |
| Configuration continuity, MediaSessionService, audio focus, Android 17 background audio | Opened the original | Lifecycle and system control entry points; the actual dependency combination still needs prototype acceptance |
| Window size page on the Material website | Tried to open it, but the reading tool returned an error | No unread facts were introduced on that basis; the corresponding official Android guide is used |
| Android multi-window mode guide | Opened the original and read the passages on modes, lifecycle and multiple instances | An adjacent window request is not treated as a guarantee of success; the fine-grained Activity launch strategy still needs device verification |

Before backend implementation, the pinned versions of Compose/Adaptive/WindowManager/Media3, the minimum Android version, the scope of the native multi-window implementation and the requirements of TV distribution channels still have to be decided. This document does not treat documentation research as equivalent to having passed device certification.
