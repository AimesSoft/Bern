**English** · [简体中文](../zh-CN/Adaptation/WindowsLinux.md)

# Windows and Linux: adaptive basis and Bern adaptation design

- Checked on: 2026-10-09 (Asia/Shanghai).
- Nature of this document: verification of official sources and design recommendations; not a list of implemented capabilities.
- Windows scope: WinUI 3, the Windows App SDK and Microsoft's Windows app design guidelines.
- Linux scope: the GNOME route of GTK 4/libadwaita is the default subject of study, compared against the KDE HIG.
- The "official basis" sections of this document summarize the body text that was read; the "Bern mapping" sections are design decisions proposed by this project.
- API names are used to delimit adaptation responsibilities; the specific Rust wrappers, minimum versions and generated code still have to be verified in a prototype.

## Source scope and reading record

Microsoft's windowing overview is explicitly aimed at WinUI and the Windows App SDK; the UWP `Windows.UI.WindowManagement.AppWindow` of the same name cannot be taken directly as a WinUI 3 API. The windowing basis in this document uses `Microsoft.UI.Windowing`. [Windowing overview](https://learn.microsoft.com/en-us/windows/apps/develop/ui/windowing-overview)

The GNOME HIG is explicitly scoped to the recent GNOME platform, especially GTK 4 and libadwaita; the KDE HIG centers on the usage conventions of KDE Plasma while also considering other environments. It follows that "Linux native" cannot point to a single HIG common to all desktops. [GNOME HIG platform definition](https://developer.gnome.org/hig/), [KDE HIG scope](https://develop.kde.org/hig/)

| Source group | Body reading status | Version and limitations |
| --- | --- | --- |
| Microsoft layout, navigation, input, accessibility | Read the subsections cited in this document | Continuously updated Windows app documentation; Bern's minimum OS version is not derived from it |
| Windows window APIs | Read Closing, Cancel and the windowing overview | API links are pinned to the `windows-app-sdk-1.8` view; that version is not claimed to be the latest |
| Windows media and composition | Read the playback, control binding, resource ownership and optimization subsections | Only settles the adaptation direction; does not prove that all codecs, DRM or GPU environments are available |
| GNOME HIG | Read the body text on scaling, pointer, keyboard, windows and accessibility | The HIG does not uniformly mark release versions for all the rules this document covers |
| libadwaita Adaptive Layouts | Read the breakpoints, Clamp, Dialog and View Switcher subsections | The `/doc/1.8/` page shows Library Version 1.8.8; not a Bern dependency commitment |
| libadwaita Styles & Appearance | Read the light and dark mode and high contrast subsections of `main` | The page shows 1.11.alpha; only a basis for direction, and must be checked against the version the project pins before use |
| GTK/GDK API | Read the body text on scaling, closing, minimizing and video | Floating GTK 4 documentation; only the 4.12 starting version of `get_scale` has been confirmed in the body text |
| KDE | Read the HIG scope and the layout and navigation body text | A comparison with the KDE route; does not mean Bern already has a Qt/Kirigami backend |

Reading anomalies are also recorded: some Microsoft Learn pages showed an authorization prompt at the top, but the body was actually readable; no sign-in or permission bypass was done this time. libadwaita `/doc/1.8/styles-and-appearance.html` returned a read error; on another occasion `/doc/1.5/` returned a page whose header showed 1.10.0, so the content version is not inferred from the URL path. The theme citations finally use `main`, which is explicitly labeled as the development version, and this part is not described as API verified on 1.8.

## Windows: official basis

### W1 Window size and logical units

Microsoft recommends setting breakpoints according to the window space available to the app rather than by physical display size. XAML layout uses effective pixels; scaled device pixels should not be used directly as layout widths. [Screen sizes and breakpoints](https://learn.microsoft.com/en-us/windows/apps/design/layout/screen-sizes-and-breakpoints-for-responsive-design)

The opening and the table of the official general page word the 640 boundary slightly differently; this document adopts the default behavior that NavigationView lists explicitly: minimal navigation at widths up to 640, compact navigation from 641 to 1007, and expanded at 1008 and above. These are the control's default strategy, and the thresholds can be changed. [NavigationView: Adaptive behavior](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/navigationview#adaptive-behavior)

### W2 Navigation is not automatic business routing

NavigationView supports top and side navigation and by default can adapt to width among minimal, compact and expanded sidebars; the control itself does not maintain the app's forward and back business flow. `ItemInvoked` represents an invocation, while `SelectionChanged` can also come from a programmatic write-back; the two cannot indiscriminately trigger the same navigation action. [NavigationView: Display modes and Navigation](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/navigationview)

### W3 Input and hit areas

WinUI controls have a foundation of keyboard behavior; the official guidance requires a predictable Tab order, a sensible initial focus and arrow-key navigation within groups. Static labels should not all become Tab stops for the sake of readability. [Keyboard interactions](https://learn.microsoft.com/en-us/windows/apps/develop/input/keyboard-interactions)

The recommended touch target is usually about 7.5mm square, and the documentation gives the example of 40×40 pixels at a specified density and scale; the size also has to take into account the consequences of mis-taps and the frequency of use. This is not a requirement that all visible icons be drawn as fixed 40px squares. [Guidelines for touch targets](https://learn.microsoft.com/en-us/windows/apps/develop/input/guidelines-for-targeting)

### W4 Text scaling, themes and accessibility

Windows text size and overall display scaling are different settings. WinUI text controls usually support system text scaling, and the official guidance advises against disabling `IsTextScaleFactorEnabled` broadly; static text and editable text should also keep the correct UI Automation roles. [Accessible text requirements](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessible-text-requirements)

Contrast themes and light and dark mode are not the same dimension. Theme resources should follow the system at runtime; HighContrast resources use system semantic colors and correct foreground–background pairings, and brand colors must not forcibly override the user's custom contrast theme. [Contrast themes](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/high-contrast-themes)

### W5 Windows, closing and system facts

WinUI's XAML Window hosts content and lifecycle, and AppWindow provides a window management abstraction over the top-level HWND; the system window and Bern's logical window identity are not the same object. [Windowing overview](https://learn.microsoft.com/en-us/windows/apps/develop/ui/windowing-overview)

`AppWindow.Closing` targets the close entry points the system provides; calling `Destroy` directly does not raise this event. `Cancel=true` cancels this system close when the Closing handler exits; this event alone cannot be relied on to intercept all destruction uniformly. [Closing](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.appwindow.closing?view=windows-app-sdk-1.8), [ClosingEventArgs.Cancel](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.appwindowclosingeventargs.cancel?view=windows-app-sdk-1.8)

### W6 Video presentation and composition

MediaPlayerElement can bind an existing MediaPlayer or create its own player; the two approaches carry different disposal responsibilities. The official guidance supports turning off the built-in transport controls and warns against alternately writing equivalent properties on the control and on the underlying player, otherwise the state may become inconsistent. [Play audio and video with MediaPlayer](https://learn.microsoft.com/en-us/windows/apps/develop/media-playback/play-audio-and-video-with-mediaplayer)

WinUI provides an `IsFullWindow` optimization path for full-window video; overlaying XAML content affects media composition efficiency. Subtitles and temporary controls still have legitimate uses, and the official guidance recommends collapsing them when they are not needed; this is not a functional restriction forbidding overlaid controls. [Optimize animations, media, and images](https://learn.microsoft.com/en-us/windows/apps/develop/performance/optimize-animations-and-media#optimize-media-resources)

## Windows: Bern mapping

The following is a design yet to be implemented, not a Microsoft mandate.

- **Size**: the target adapter reports the logical size of the window content area, the display scale and the text scale; the shared IR does not accept HWND frame pixels as the page's available width.
- **Breakpoints**: Windows navigation presets can refer to 640/1008; the shared Adaptive allows redefining them by content. The actual comparison rules must cover fractional widths and leave no unclassified range.
- **State preservation**: the compact sidebar and the expanded sidebar reference the same routes, commands and Features; changing the width preserves the current page, selection, drafts and scroll identity.
- **Navigation**: native invocation events are converted into `ui.navigate`/`ui.back` session commands; selection notifications from programmatic write-backs only synchronize state and do not execute the command again.
- **Input**: control activation from the mouse, touch, keyboard and UI Automation ultimately references the same action; text input continues to use the native input method editing session.
- **Hit areas**: transparent hot zones can be larger than the icon; IconButton by default uses WinUI's standard icon button style, and the accessible name, focus indicator and sufficient hit space must not be removed.
- **Theme**: following "native first", WinUI theme resources and the platform appearance are used, and brand colors are mapped to accent color resources; high-contrast resources map to system colors, and theme changes do not rebuild the business engine or the window identity.
- **Text**: text scaling affects measurement and reflow; scaling only the glyphs while keeping the original fixed height is not acceptable. Icon resources and video buffers are handled separately according to their rendered size.
- **Closing**: a cancelable close entry point first cancels the native default action and then runs the unified close flow; after confirmation, the window is destroyed through a controlled path, preventing duplicate confirmation or duplicate release.
- **System facts**: moves, resizes, activation or destruction that have already happened are merged through typed observation events; commands are not fabricated after the fact to claim the OS was controlled beforehand.
- **Video**: the control bindings of the standard Video go through Bern commands; player ownership and presentation control ownership are separate, and switching layouts should not unexpectedly Dispose an engine-level player.
- **Composition**: overlaying subtitles, danmaku, menus and virtual windows remains an allowed capability; the target must measure the composition cost separately and cannot promise that every combination gets the lowest-power path.

## GNOME/GTK: official basis

### G1 Adapting to space and content

GNOME recommends starting from the most constrained space and input conditions and using breakpoints to reorganize the UI; large windows need a maximum content width so that text lines do not become too long and related controls are not too far apart. The device form factor cannot reliably indicate whether a keyboard or touch is available. [Scaling & Adaptiveness](https://developer.gnome.org/hig/guidelines/adaptive.html)

The HIG currently recommends 1024×600 as the lower bound of supported desktop display sizes; apps suited to the phone form factor should be able to shrink to 360×294. The former is a recommendation about the display environment and cannot be read as requiring every app to enforce a minimum window of 1024×600. [Scaling & Adaptiveness: Small Size Handling](https://developer.gnome.org/hig/guidelines/adaptive.html#small-size-handling)

### G2 Structures available in libadwaita

The libadwaita 1.8 documentation that was read provides Clamp to limit the maximum content width, Breakpoint to change the layout, top/bottom View Switcher switching, and Dialog, which changes its presentation with the parent window's size. A host that uses breakpoints needs to declare the minimum size it supports; it cannot expect the content to supply all constraints on its own. [Adaptive Layouts](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1.8/adaptive-layouts.html)

### G3 Pointer, touch and keyboard

GNOME does not allow key actions to appear only on hover; actions that can be done with a pointer should also be possible with the keyboard. Three- and four-finger gestures and gestures from the top and bottom screen edges are reserved for the system, and apps should not take them over. [Pointer & Touch](https://developer.gnome.org/hig/guidelines/pointer-touch.html)

The keyboard guidelines require the entire interface to be keyboard-navigable and the standard keyboard shortcuts to be followed; Super is reserved for the system, and conflicts among Alt access keys have to be handled together with the localized text. [Keyboard](https://developer.gnome.org/hig/guidelines/keyboard.html)

### G4 Scaling and resource pixels

GTK's `Widget.get_scale_factor()` returns an integer; for fractional scaling, the documentation recommends using `Gdk.Surface.get_scale()` when the actual ratio is needed. The latter is available since 4.12 and can change during the Surface's lifetime. [Gtk.Widget.get_scale_factor](https://docs.gtk.org/gtk4/method.Widget.get_scale_factor.html), [Gdk.Surface.get_scale](https://docs.gtk.org/gdk4/method.Surface.get_scale.html)

### G5 Light and dark mode, high contrast and assistive technologies

libadwaita's appearance documentation models the system preference, forced light and forced dark separately; high contrast is independent of light and dark mode, standard controls already support it, and custom drawing still has to handle it. This citation reads the development branch, and Bern is not required on that basis to adopt new CSS features immediately. [Styles & Appearance](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/styles-and-appearance.html)

The GNOME HIG requires descriptive accessible names and actually testing high contrast, large text, keyboard only, screen readers and the on-screen keyboard; using GTK controls does not replace app-level acceptance. [Accessibility](https://developer.gnome.org/hig/guidelines/accessibility.html)

### G6 Window relationships and closing

Main windows should be resizable; multiple main windows are independent of each other. Secondary windows belong to a main window and close along with it when the main window closes; the standard close shortcut is Ctrl+W, and modal windows should also support Esc. [Windows](https://developer.gnome.org/hig/patterns/containers/windows.html)

GNOME's "main window" here means an independent top-level window, of which there can be several; it is not the same as the unique primary role in Bern's WindowSet. Bern's default session ownership differs from GNOME's secondary window convention: an app that needs a secondary window that closes with its parent window should declare the owner and the closing relationship explicitly, and the backend does not quietly change the shared lifecycle.

GTK's `close-request` is emitted when the user clicks the close button, and a handler can return true to stop further propagation. On the other hand, GDK states explicitly that the window system may ignore a minimize request; sending the request does not guarantee that the actual window state has changed. [Gtk.Window::close-request](https://docs.gtk.org/gtk4/signal.Window.close-request.html), [Gdk.Toplevel.minimize](https://docs.gtk.org/gdk4/method.Toplevel.minimize.html)

### G7 Video is not just a mapping to GtkVideo

GtkVideo is aimed at previews, embedded animations and simple media controls, and the official documentation explicitly lists features it lacks, such as video overlays, multichannel audio and device selection; a full player can use GdkPaintable together with a media framework such as GStreamer. Presentation without controls can go through Paintable/GtkPicture. [Gtk.Video](https://docs.gtk.org/gtk4/class.Video.html)

## GNOME/GTK: Bern mapping

The following is a design yet to be implemented; it does not extend GNOME conventions into mandatory guidelines for all Linux desktops.

- **Layout**: the GTK target can map to the supported containers of libadwaita; capabilities are declared per the version the target pins, and the shared IR does not expose Adw objects.
- **Thresholds**: breakpoints are chosen from the navigation items, the translated labels and the minimum content width; the Windows constants 640/1008 are not copied.
- **Large windows**: forms and reading content use a maximum width constraint; expandable regions such as video and charts are allocated space by their own content rules.
- **Narrow windows**: navigation can move to a page stack, a bottom switcher or an on-demand panel, and Feature and command identity stay the same; features that do not fit must not simply be deleted.
- **Input**: touch does not depend on hover or right-click; keyboard shortcuts, menus and accessible actions bind the same command and do not take over GNOME's reserved gestures.
- **Scaling**: layout, window size and image/video buffer pixels are kept separate; after moving across displays the resource resolution is chosen again, and the integer scale must not be treated as the real fractional ratio.
- **Theme**: following "native first", the light and dark styles of GTK and of supported desktop themes are used; when the chosen environment cannot provide full light and dark mode, Bern's semantic role mapping is used instead according to the capability description; high contrast takes priority. For now, development-branch CSS media queries are not listed as the minimum usable implementation.
- **Accessibility**: GTK's accessibility tree, editing roles and focus behavior are kept; generated custom controls must supply the same protocol.
- **Windows**: dedicated window declarations explicitly distinguish main windows, secondary windows, parents and modal scopes; closing one of several main windows does not destroy the others along with it.
- **Close confirmation**: the close button, Ctrl+W, the CLI and MCP use the same `window.request_close`; the unsaved-changes check can return a pending-confirmation result, rather than only showing a dialog and leaving the external caller waiting forever.
- **Window system**: requests and observed results are recorded separately; minimize, activate, size and position capabilities are verified per compositor/backend, and Linux cannot be treated as one uniform window manager.
- **Video**: a simple preview and a full player are different capability tiers; the latter has its own acceptance for the media core, the Paintable/texture path, subtitle overlays, color and hardware decoding.

## KDE comparison: Linux has no single HIG

**Official basis.** KDE requires windows to adapt smoothly in tiled, maximized and narrow states; its layout table recommends moving toolbars/navigation bars toward the bottom in narrow forms, with sidebars shown on demand or moved into a page stack. Kirigami's layout units and GlobalDrawer/PageStack are its concrete implementation system. [KDE Layout and navigation](https://develop.kde.org/hig/layout_and_nav/)

**Bern mapping.** The first default Linux target can still be GTK, but the target description should state "GNOME/GTK conventions" explicitly; running a GTK app under Plasma does not mean the KDE HIG has been adopted. A future Qt/Kirigami target plugs in through a separate generator and reuses the window, command, state and theme semantics; it does not need to copy GTK's control shapes, and it cannot bypass Bern's full feature requirements either.

**Capability boundary.** Report the desktop environment, window system, GTK/libadwaita version and verified combinations separately. The KDE materials serve as a comparison and do not constitute a Qt implementation in this round, release compatibility, or certification for all Linux distributions.

## Cross-platform window and command boundaries

The following are Bern protocol requirements; they cannot be inferred to be implemented from the existence of a single native API.

| Scenario | Rule Bern needs |
| --- | --- |
| Native multi-window capability available | Window declarations map to native windows; each instance has its own logical identity and lifecycle |
| Only a single host surface | The in-app virtual window is used automatically; semantic commands do not change, and the actual presentation mode can be queried |
| Virtual window limits | Does not claim a system taskbar entry, independent windows across displays, or system-wide always-on-top |
| External close request | UI, CLI, JSON and MCP share the close check and confirmation protocol; a confirmation is tied to the instance and the state version |
| Facts that already happened in the OS | Trusted adapters report observation events; a command cannot be faked as having been executed first, and external state injection cannot be allowed |
| Host forcibly terminated | Child windows disappear with the host; relies on the recovery strategy and cannot guarantee a chance to show the close confirmation every time |
| Engine sharing | Multiple windows bind the same engine_id; closing a window only releases the resources it owns, and whether playback stops is decided by ownership |
| Stale targets | window_id is validated together with the instance generation; after destruction, old commands cannot hit a newly created window with the same name |

Automatic fallback resolves differences in presentation capability and does not change the target's feature contract; features that need real system capabilities should report that they are unsupported or use a declared alternative.

## Actionable acceptance checklist

The following scenarios must be run on real targets. For the first round of results for the GNOME/GTK part, see the [Linux GTK implementation notes](../Implementation/Linux-GTK.md); for the Windows/WinUI 3 part, see the [Windows WinUI 3 implementation notes](../Implementation/Windows-WinUI3.md).

### Windows acceptance

- [ ] Continuously resize the content area width around 640 and 1008, including the boundary values; navigation keeps the same route and no features are missing.
- [ ] Repeat the tests at the 100%, 125%, 150% and 200% display scales the target supports; after moving to a display with a different scale, hit areas still correspond to what is drawn.
- [ ] Increase the system text size on its own; buttons, menus, titles and forms can be read in full, without clipping in place of reflow.
- [ ] Open media, search, select, play and go back with the mouse, touch and keyboard separately; each action produces exactly one command.
- [ ] Traverse pages with the keyboard only; the initial focus and the focus restoration after a modal closes are sensible; static labels have no extra Tab stops.
- [ ] Switch light and dark mode and the system contrast theme at runtime, and test contrast colors the user has modified; no hard-coded unreadable states.
- [ ] Use Narrator to check names, roles, values, disabled states and navigation order; native write-backs do not dispatch commands again.
- [ ] Test closing from the title bar, closing programmatically, closing from an external CLI, and late messages after destruction, separately; close confirmation and resource release each run once.
- [ ] Switch video between embedded and full-window, show/hide subtitles and the transport controls, and check the actual playback state, composition performance and release.

### GNOME/GTK acceptance

- [ ] Verify all required features in a 1024×600 desktop environment and in narrow windows; add the 360×294 scenario only when the app declares phone support.
- [ ] Under combinations of long translations, large text, a collapsed sidebar and an expanded window, content does not overflow; switching navigation does not lose drafts or selections.
- [ ] Operate with the mouse, touch, touchpad and keyboard only, separately; important features do not depend on hover, right-click or reserved gestures.
- [ ] Ctrl+W, Esc, standard keyboard shortcuts and localized access keys do not conflict; modal scopes and focus restoration match the declaration.
- [ ] In the declared integer/fractional scaling environments and after moving across displays, check that text, icons, video and pointer hits are consistent.
- [ ] Verify light and dark mode, high contrast, large text, the screen reader and the on-screen keyboard item by item, and record the accessibility tree results.
- [ ] Two independent top-level windows share the engine, and closing one keeps the other; secondary windows whose subordinate relationship is declared explicitly are handled by the owner close protocol, without wrongly destroying the app-level player.
- [ ] When the window system rejects a request, on an external close, and when the host is lost, the actual state is returned; a request having been sent must not be recorded as already completed.
- [ ] For the full player, separately verify subtitle/control overlays, media backend failure, source switching, release, and the codec combinations actually supported.

### Cross-capability acceptance

- [ ] Drive the same engine instance from the same fixture with the UI, CLI, JSON and MCP separately; business state and versions stay continuous and consistent.
- [ ] Run the same declaration with native and virtual windows; the conclusions on window identity, close confirmation, navigation and feature coverage are consistent, and system capability differences can be queried clearly.
- [ ] Record results for GNOME and Plasma environments separately; passing in one environment cannot sign off support for another.
- [ ] After pinning the toolchain, record the OS, desktop environment, window system, SDK/GTK/libadwaita and media backend versions, and only then decide the scope of release support.

The acceptance report should distinguish "declaration compiles", "operable with real input", "native API request succeeded", "actual state confirmed" and "performance measurements passed"; no one of them can replace all the conclusions.
