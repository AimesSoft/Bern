**English** · [简体中文](../zh-CN/Adaptation/HarmonyOS.md)

# HarmonyOS adaptive UI

- Document version: v0.1
- Official sources checked on: 2026-10-09
- Scope: HarmonyOS phone, foldable, tablet and PC form factors; smart screen capabilities are verified separately.
- Status: preparing the adaptation design. The ArkUI target, the Rust bridge and media adaptation are not yet implemented or accepted.
- Governing constraints: [architecture design](../Design.md) and [general adaptation principles](README.md).

HarmonyOS needs its own target support matrix and ArkUI adaptation; the fact that some Android logic can be reused does not mean the Android UI backend can cover HarmonyOS directly. The official basis of this document comes from Huawei developer materials; OpenHarmony versions and device capabilities cannot unconditionally stand in for a commercial HarmonyOS environment.

## Official design basis

### Responsive structure and adaptive sizing

Huawei's multi-device best practices distinguish two kinds of capability: responsive layout changes the structure according to window characteristics, for example from one column to two columns; adaptive layout handles changes within a region such as stretching, equal distribution and wrapping. They also list window modes, input interaction and SysCap capability checks, rather than choosing the interface by device name alone. [HarmonyOS multi-device development best practices](https://developer.huawei.com/consumer/cn/best-practices/multidevice/).

This corresponds to Bern's two levels: component layout computes the available size, and Adaptive selects declaration branches with different structures. The device type can provide styles and default strategies; the actual window space decides whether a given structure fits.

### Continuous use on foldables

The official Pura X Max adaptation page explicitly addresses the wider and shorter outer screen, the unfolded inner screen and the folded/unfolded/hover states, and lists problems with keyboard occlusion, content truncation on short screens, split layouts and state restoration after switching. These are official design inputs for that device; they cannot be used to infer that all foldables have the same size or form. [Pura X Max official adaptation guide](https://developer.huawei.com/consumer/cn/newdevice/pura-x-max/).

Bern should model window size, available height and fold posture separately. A posture change can change the relationship between content and the control area, but a "folded" boolean does not directly decide between one column and two. During rotation, folding and unfolding, and window resizing, the current item, drafts, navigation and player identity are preserved.

### Lifecycle of window changes and orientation strategies

This time the body of the official English archived practice was actually read: the page was updated on 2025-01-15 and is under `best-practices-V5`. It explains that `setPreferredOrientation` changes the orientation of the whole window, and the setting may persist after navigating to another page; a temporary video page needs to restore it on exit according to the app's strategy. The example listens to the current window's width and height through `windowSizeChange`, converts pixels to vp to adjust the layout, and removes the listener when the page exits. [Switching Between Landscape and Portrait Modes](https://developer.huawei.com/consumer/en/doc/best-practices-V5/bpta-landscape-and-portrait-development-V5).

**Bern mapping.** Orientation requests go into session commands, and window size observation goes into environment events; orientation strategies and listeners owned by a page are restored or released with the page scope. Physical window pixels and logical UI sizes are passed separately, and the current content width and height cannot be inferred directly from the sensor orientation. This is an adaptation rule formed from the practice that was read; the archive path does not prove the minimum API of the latest SDK, freeform window restrictions or the behavior of all device models.

### Large screens and media scenarios

The Huawei design portal provides separate entries for phones/foldables/tablets, PCs and smart screens, and shows media scenarios such as playback in a side-by-side layout, picture-in-picture and in-app split screen. These examples support adjusting the structure by task; they do not mean that every window should be stretched into the same desktop layout. [HarmonyOS Design](https://developer.huawei.com/consumer/cn/design/).

The assets, materials and example layouts there are HarmonyOS's design context. Bern's command rules do not change; theming follows the main design document's "native first, algorithm as fallback": when ArkUI has a light and dark mode system, platform colors and standard control styles are used. When departing from the platform's recommended visuals is necessary, the backend must record the difference and verify usability; it cannot claim to follow all platform styles pixel for pixel at the same time.

## Bern's platform mapping decisions

This section states Bern's design requirements; the specific ArkUI APIs, the minimum system version and the SDK version are determined in the implementation prototype.

| Adaptation subject | Bern rule | Platform adapter responsibility |
| --- | --- | --- |
| Window space | Use the current content area; both width and height take part in the decision | Report window changes; do not substitute the whole screen resolution |
| Split view | Single-column navigation when compact; wide windows show list-detail or an auxiliary area as the content needs | Map analyzable layouts to target containers |
| Folding and hover | Posture, layout regions, occlusion and orientation are expressed independently; unknown values stay unknown | Report only information that is actually available; do not extrapolate undisclosed hinge regions |
| System avoid areas | Extending backgrounds and keeping key content clear are handled separately | Sync changes of the status bar, navigation area, window control area, cutouts and keyboard |
| Input | Touch, keyboard and mouse, pen, and remote control are capabilities that can coexist | Normalize into semantic commands such as activate, edit, back and focus |
| Fonts | Keep system text scaling and readability; do not shrink the whole page to fit text | Verify the metrics of Chinese, Latin, mixed text and long titles |
| Theme | Follow the system; when the platform has a light and dark mode system use platform colors and standard control styles, and Bern's self-drawn parts follow Bern's control recipes | Listen for environment changes; handle contrast and material degradation |
| Accessibility | Complete names, roles, states, actions and focus order | Verify actual reading and operation with the target's assistive features |
| Lifecycle | A hidden page does not mean the task has ended; a closed window does not mean the engine exits | Sync the actual lifecycle by app, window and media scope |

Do not hardcode any one device model's resolution in the common IR, and for now do not prescribe a set of numeric breakpoints "uniform across all HarmonyOS devices". Breakpoints should be determined jointly by the minimum usable content width, platform component behavior and the actual target guidelines; specific official values must be checked against the body of the locked SDK documentation before they go into the repository.

### Multiple windows

The auto strategy of Window remains: use native windows when the required native window capability is available, otherwise provide virtual windows in a valid UI host. System split screen, freeform windows, app sub-windows and Bern logical windows are not the same concept; the adapter must state which semantics each kind of native object can satisfy.

Creating, activating, requesting close, restoring and setting size go through unified session commands. Changes of size, focus or destruction that the system has already made are merged as fact events. If an ordinary native close cannot satisfy the confirmation check the app requires, choose a virtual presentation that can satisfy the contract, or let native_required fail explicitly.

The main window, multiple auxiliary windows and in-app virtual windows share the engine and each maintain their own focus and navigation. Virtual windows must adapt to switching to narrow screens; a phone must not show several tiny desktop floating windows that cannot be operated.

### Video and platform services

The app syntax of Video does not change. The HarmonyOS media backend handles actual playback, and the ArkUI presentation adapter handles mounting and compositing. Player state, UI controls and server reports continue to use the same player_id and source version rules.

The prototype needs to verify separately:

- The containers, audio and video formats, subtitles, track selection and hardware capabilities supported by the playback facility.
- The layering and input hit-testing of video together with virtual windows, danmaku, subtitles and menus.
- Whether native playback controls can enter the command protocol before acting; when they cannot, use Bern's standard control bar.
- The actual permissions and lifecycle of picture-in-picture, system playback controls, background execution and audio interruptions.
- Whether the player keeps working after its view is temporarily detached when folding or switching windows.
- The input, theme and media capabilities of smart screens. Support on smart screens cannot be inferred from the phone ArkUI implementation.

Cross-device continuation, casting and distributed capabilities are separate service extensions, not features gained automatically by enabling responsive layout. Concrete media transport and server-side business logic remain in the media or app adapters.

## Target description before implementation

Every supported combination records at least the following fields; a combination with missing items must not be marked as done:

| Item | Information that must be recorded |
| --- | --- |
| System and device | Commercial HarmonyOS version, device form factor, actual test device |
| Toolchain | DevEco Studio, compile SDK, minimum API, Rust and ArkUI bridging approach |
| Windows | Main window, sub-windows, multiple instances, freeform windows, close checks and virtual fallback capability |
| Input and accessibility | Touch, keyboard and mouse, pen or remote control, and the actual accessibility verification results |
| Media | Backend, presentation method, overlays, picture-in-picture and background strategy |
| Theme | Reading the system mode, font scaling, increased contrast and special device limitations |
| Evidence | Direct links to official documentation, version or API qualifications, test results and dates |

## Acceptance checklist

- [ ] The same app can complete the required Features on the short outer screen, the unfolded inner screen, tablet split screen and resizable windows.
- [ ] When the keyboard appears, input fields and confirmation actions stay visible; after the keyboard hides, the layout is restored without losing drafts.
- [ ] Folding, rotation and window changes do not reset the selected item, the navigation stack, the list position or playback progress.
- [ ] The content structure changes with the available area; different window sizes in the same posture do not wrongly use the same layout.
- [ ] The status bar, cutouts, navigation area and window control area do not cover key actions.
- [ ] Both touch and keyboard and mouse can complete the key paths; switching peripherals does not rebuild the UI without reason.
- [ ] An ordinary native close and a virtual close run the same confirmation protocol, and forced destruction by the system is cleaned up correctly.
- [ ] Light mode, dark mode, enlarged text, long text and accessibility are verified item by item.
- [ ] Video overlays are readable and operable, and window switching neither duplicates the player nor fakes continued playback.
- [ ] Virtual windows are used correctly when native capabilities are insufficient; a clear error is returned when there is no UI host.
- [ ] After an SDK upgrade, window capabilities, bridging, media layering and input behavior are verified again, rather than only checking that the build compiles.

## Official sources and reading boundaries

All checks were made on 2026-10-09. The portals whose body text was read and one English archived practice support the design direction above; other detailed documents loaded dynamically or timed out, so API parameters, values and version conditions that were not read are not written down as confirmed rules. Search results can return some detailed passages, but they are recorded separately from body text obtained by actually opening the page.

| Official source | Reading status this time and use |
| --- | --- |
| [HarmonyOS multi-device development best practices](https://developer.huawei.com/consumer/cn/best-practices/multidevice/) | Body read: responsive and adaptive layout, windows, input, SysCap and the entries for each device |
| [Pura X Max official adaptation guide](https://developer.huawei.com/consumer/cn/newdevice/pura-x-max/) | Body read: short screen, keyboard, folding and unfolding, split layout and state restoration scenarios |
| [HarmonyOS Design](https://developer.huawei.com/consumer/cn/design/) | Body read: device design categories and media layout examples |
| [English archived practice on switching between landscape and portrait](https://developer.huawei.com/consumer/en/doc/best-practices-V5/bpta-landscape-and-portrait-development-V5) | Body and examples read: window orientation persisting across pages, listening to the current window's width and height, px/vp conversion and releasing the listener; page updated 2025-01-15, archived material |
| [Multi-device general adaptation guide](https://developer.huawei.com/consumer/cn/multidevice/adaptive-apps/) | Only the title and the search summary were obtained reliably; kept as an entry point for later checks |
| [Responsive layout](https://developer.huawei.com/consumer/cn/doc/best-practices/bpta-multi-device-responsive-layout) | Documentation shell or timeout; the browser redirected to the new documentation center, but body extraction did not complete |
| [Foldable app development](https://developer.huawei.com/consumer/cn/doc/best-practices/bpta-foldable-guide) | Only the table-of-contents shell was obtained this time; the specific API rules await a check of the body |
| [Freeform windows](https://developer.huawei.com/consumer/cn/doc/HarmonyOS-Guides/freeform-window) | Title and table of contents visible in search, reading the body timed out; the minimum API cannot be confirmed from it |
| [PC and tablet adaptation](https://developer.huawei.com/consumer/cn/multidevice/pc/adapt) | Body extraction unstable; details not taken as verified guidelines |
| [Immersive window capability](https://developer.huawei.com/consumer/cn/doc/doccenter-capabilities/immersive-window-feature) | Related content visible in search, reading the body unstable; API qualifications await a check |
| [Multi-window layout adaptation, English V14](https://developer.huawei.com/consumer/en/doc/harmonyos-guides-V14/multi-window-layout-adapt-V14) | Search returned detailed passages; actually opening it still gave a documentation shell or a timeout, so no device ratios or API qualifications were written down from it |
| [Official Tabs immersive FAQ](https://developer.huawei.com/consumer/cn/doc/doccenter-dev-faq/faqs-arkui-1584) | Search returned the approach and version descriptions; actually opening it gave only the site navigation shell, so the version conditions are not counted as verified from the body |

Before the HarmonyOS implementation starts, the body text and version conditions of the window, avoid area and responsive APIs of the current version should still be checked. This chapter settles the adaptation goals and acceptance scenarios; it cannot replace a review of the SDK APIs or testing on real devices.
