**English** · [简体中文](../zh-CN/Implementation/Animated-SVG.md)

# Animated SVG

Bern has a built-in `bern-svg` document runtime and an `Svg` component. It keeps the original SVG file's vector graphics, CSS keyframes, SMIL, gradients, filters, masks, text and in-document JavaScript. Targets that support WebView render it with the platform's SVG/DOM engine; targets without WebView convert it into a static PNG at build time.

The document engine is heavy (one WebView per `Svg`, with its process and memory), so it is used only for documents that actually move or run code (2026-10-26). The compiler reads every declared SVG resource (`bern_svg::picture`): a document without SMIL or CSS animations, transitions, pointer states, scripts, links, embedded web pages and media is a picture; on every target it is drawn into a PNG at build time, `Svg` is lowered to `Image`, and no document engine is started; one that uses only `currentColor` and no other color is drawn as a template image (`rendering: template`) and follows the color of the surrounding text. One that changes color with the appearance (`@media (prefers-color-scheme: dark|light)` rules, `light-dark()`) is drawn twice: resvg understands neither of these, so at build time the document is first expanded into what it looks like in each appearance (`bern_svg::in_appearance`: the rules for this appearance take effect directly, those for the other appearance are deleted, and `light-dark(light, dark)` is replaced by the corresponding value), one light `<file>.png` and one dark `<file>.dark.png`; `Image` gets an extra internal property `dark_source` that points to the dark one, and the frontend shows the one that matches the control's actual appearance (whether it comes from the system, is forced by the app's `WindowSet(appearance:)`, or is fixed by a part's `appearance:`), and switches when the appearance changes (2026-10-26). The only documents that still use the document engine are those with media queries other than appearance, CSS variables, or `currentColor` used together with other colors or with appearance rules.

How each frontend gets the appearance: SwiftUI reads the environment's `colorScheme`; WinUI 3 and XAML Islands read the frame's `ActualTheme` (it is unknown before being placed in the UI tree, so the image is chosen on `Loaded` and `ActualThemeChanged`); Compose uses `bernDark()`; GTK uses `theme::part_of` (the appearance of the part it is in, or the window's when it is not in a part) and chooses again after being mapped to the window and when the appearance changes; the Web chooses by `getComputedStyle(…).colorScheme` after the image is laid out in the page (the first `ResizeObserver` callback), and chooses again when the theme changes (`themeTick`).

```rust
Svg(
    source: "assets/logo.svg",
    static_source: "assets/logo-final.svg",
    width: 104,
    height: 104,
    label: tr! { "zh-Hans" => "应用图标", "en" => "App icon" },
    playing: app.logo_playing,
    speed: app.logo_speed,
    restart: app.logo_revision
)
```

Resources must be listed in `[resources].paths` of `Bern.toml`. The dynamic path checks the XML and SVG namespaces; the static path is parsed by resvg; missing resources, malformed files and external entities are reported as errors. `Image` remains the component for bitmaps; `Svg` explicitly denotes a vector resource with its own document execution environment.

## Interface

| Property | Meaning |
| --- | --- |
| `source` | A declared `.svg` file, fixed at build time |
| `static_source` | Optional, a declared SVG of the final state; used for static compilation without WebView; when omitted, the base graphics of source are rasterized |
| `width`, `height` | Required, logical size, can be bound to numeric state; the vector graphics scale with the viewport |
| `label` | Name for assistive technologies; when omitted, the content is decorative |
| `fit` | `contain` (default), `cover`, `fill`, corresponding to SVG's meet, slice, none |
| `playing` | Boolean state, default true; pauses/resumes CSS, Web Animations and SMIL timelines |
| `speed` | Positive numeric state, default 1; a single rate for CSS/Web Animations playbackRate and SMIL; invalid values fall back or keep the valid rate |
| `restart` | Numeric state; when the value changes, existing CSS/Web Animations and SMIL timelines are reset to zero; scripts are not reloaded and the scripts' own business state is not reset |
| `interactive` | Build-time boolean, default true; controls mouse/touch interaction inside the SVG |

Double clicks, clicks, pointer events and so on are handled by the original SVG's scripts. The animation controls do not take over JavaScript's `Date.now()`, `setTimeout()` or custom RAF clocks. CSS and SMIL timelines are controlled through a unified interface; WAAPI animations are synchronized when the control state updates, when a CSS animation starts, or when the SVG subtree/class changes; when a script alone creates a new WAAPI animation without any of these events, immediate takeover is not guaranteed. CSS `prefers-reduced-motion` keeps its browser/operating system semantics, and animation authors should provide reduced-motion styles. The document is released when the component exits; platform lifecycle and document visibility notifications are used to pause the standard animation timelines, and the Web also detects whether the iframe enters the viewport.

## Presentation layer

| Target | SVG document engine | Dependency |
| --- | --- | --- |
| macOS / iOS | Transparent `WKWebView`, wrapped as a SwiftUI view | System WebKit, non-persistent data store |
| Android | Transparent `WebView` in a Compose `AndroidView` | System Android System WebView |
| Windows (WinUI 3 and XAML Islands) | WebView2 Win32 presenter (child window) following the XAML placeholder area | Microsoft Edge WebView2 Runtime; native child windows have the HWND airspace limitation |
| Linux / GTK 4 | WebKitGTK 6 `WebView` | `bern-gtk/svg` is enabled only when `Svg` is used; development package `libwebkitgtk-6.0-dev`, Debian runtime dependency `libwebkitgtk-6.0-4` |
| Web | Transparent sandbox iframe with a separate origin | Host browser |
| tvOS or `webview = false` | resvg → PNG at build time, Image at runtime | The app does not pull in the SVG WebView presenter |

This is a wrapper around the standard SVG capabilities of browsers; rendering details and the supported versions of the standards still depend on the version of the platform engine. It does not claim to reimplement a "full-spec" Rust SVG/JavaScript engine. Apps that do not use `Svg` create none of these document views; GTK does not pull in the WebKit dependency, and SwiftUI does not generate WebKit presentation code.

## Resource boundaries

An SVG is a packaged, self-contained app resource: embed images/fonts as data URIs, and keep styles and scripts inside the file. The document does not expose the Bern engine, native commands or the host DOM; CSP restricts external subresources and network requests, the runtime blocks link clicks, and the native hosts restrict navigation and popups. The Web iframe uses a separate-origin `sandbox="allow-scripts"`, but this cannot stop scripts from navigating their own document, so only SVGs provided by the app itself are accepted, and it is not a general-purpose sandbox for untrusted scripts.

Android, Windows and Linux need the corresponding WebView runtime environment from the table. When a distribution target does not provide WebView, configure `webview = false` in the corresponding `[targets.*]`, and the build converts the SVG; on Linux, WebKitGTK is a dynamically linked dependency once the `svg` feature is enabled and must be installed with the system. WebKitGTK's web process sandbox (bwrap) needs to create user namespaces: since Ubuntu 24.04, by default only programs with an AppArmor profile may do so, and without a profile WebKitGTK aborts the whole app. When `Svg` is used, Bern's `.deb` ships this profile (`/etc/apparmor.d/<bundle_id>`, the same as the one Ubuntu provides for WebKitGTK apps such as epiphany: it adds no restrictions and only allows `userns`) and loads it right after installation when AppArmor is enabled; `.tar.gz` and development builds need it added manually, see "Packaging" in the [Linux GTK implementation notes](Linux-GTK.md). On Windows both frontends use WebView2's Win32 presenter: the WinUI 3 `WebView2` control puts a layer of theme background color under the transparent page (white in the light theme; Microsoft's documentation also states that it does not support a transparent background), so it is not used. The presenter is a child HWND above the window; after layout it follows the placeholder area's position and scale frame by frame (until it has not moved for half a second), and it is clipped by the outer scroll view; it does not follow its ancestors' other clipping or opacity changes, and it is always above the other XAML layers (such as flyout menus or pages covering it); so static SVGs do not go through it but are XAML images, covered by upper layers like any other content. The WebView2 cache is in the user's temporary directory (`BernSvgWebView2`), not next to the executable.

## Verification

`examples/svg-animation` is the example shared by all targets: a CSS rotation, a SMIL radius animation, gradients, a script that changes color on double click, and native pause, speed and replay buttons.

```sh
cargo test -p bern-svg -p bern-compiler
node --test crates/bern-svg/tests/runtime.test.mjs
cargo test -p bern-cli --test svg_targets
cd examples/svg-animation
../../target/debug/bern build --target macos
```

`svg_targets` checks code generation, resource validation, static fallback, script backends and runtime packaging for seven targets, and puts the generated output in `target/svg-generation-test/` for the corresponding platform toolchains to check. Passing code generation does not replace a build and a visual check on the target device.

Verified on 2026-10-17:

- The Bern compiler, presentation backend and SVG document tests pass; the 6 Node tests of the shared animation runtime pass. The six-target generation test also checks that the Android build cleanup keeps `BernSvg.kt` and that resources are loaded from `bern/resources/`.
- Full builds of a real app for macOS and Web pass, and the example's macOS build, iOS simulator bundle, Android APK and Web build pass. iOS and Android got no on-device interaction acceptance in this round; Windows and Linux got neither a native compile nor device acceptance.
- On macOS, that app's transparent logo on the onboarding and About pages displays correctly, and the double-click action was measured. On the Web, measured: onboarding at sizes 130/200, the About page at size 104, the entrance, and the blink/head-pat/smirk actions; after leaving onboarding the iframe count drops to zero.
- In the Web example, measured: after pausing, the rotation matrix stays the same across two samples, and replaying while paused returns it to the identity matrix; after changing the speed and resuming, the motion continues, with no console errors. The rate and SMIL clock calculations are covered by Node tests.

Addendum 2026-10-18 (Linux): the same real app builds natively on an Ubuntu 24.04 virtual machine (GTK runtime with `svg` enabled; the virtual machine has only the WebKitGTK 6 runtime libraries, so the build links with hand-written pkg-config files). Fixed an ambiguous call to `settings()` between `WidgetExt` and `WebViewExt` in the GTK presenter (it had never been compiled on Linux before). Measured through Broadway: the size-200 logo on the onboarding page is centered over the checkered illustration; the virtual machine restricts unprivileged user namespaces, so the app must run with `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1`, otherwise it aborts at startup. On the same day an AppArmor profile was added to the `.deb`: the package for `examples/svg-animation` contains the profile and the maintainer scripts, and the profile parses on the virtual machine's AppArmor 4.0; under the system's own profile of the same form, the same program's WebKitGTK sandbox is set up successfully and displays the SVG normally. Loading this profile itself needs root, and installation was not measured. Interactions such as double click, and pause and speed, were not measured.

Addendum 2026-10-19 (Windows): the same real app builds natively on a Windows 11 PC. Fixed the Windows presenter, which had never been compiled before: the runtime was not placed in the `B` namespace (`Len`, `BindLen` and others could not be found); the WinUI 3 side let through only navigations to `about:blank`, so the `NavigateToString` navigation was blocked and the document did not load (it showed as a white block); only after switching to the Win32 presenter was the background transparent. The size-200 logo on the onboarding page is centered transparently over the checkered illustration under both WinUI 3 and XAML Islands (the app's own window snapshot); the position offset during the pop-up animation is corrected by following frame by frame. Interactions such as double click, pause and speed, and the About page (clipping in the scroll view) were not measured.

Addendum 2026-10-21 (Android): a `WebView` that leaves the screen is not destroyed right away; it is switched to a blank page (which releases the document) and one is kept per document and activity; when the same document is shown again within 30 seconds, the view is taken back and the document is reloaded, and scripts and animations still start from the beginning, the same as with a new view; it is released after 30 seconds, when the activity stops, or when the render process exits. The kept view handles `onRenderProcessGone`: while it is invisible the system may reclaim the render process, and without handling this the app would be terminated along with it. On a MIX 2S (Android 15), creating a `WebView` on the main thread takes about 50 ms, and in the same real app the page-switch frame when opening the "About" page again drops from 97–99 ms to 38–39 ms; the cost is that the render process lives up to 30 seconds longer after leaving (about 21 MB of private dirty pages). Drawing on RenderThread in the frame where the icon appears (9–22 ms) is unchanged. For the measurements, see "Frames decided by RenderThread after a page switch" in the [Android Compose implementation notes](Android-Compose.md). The other platforms were not changed, and the time they take to create a document view was not measured either.

## Build-time static fallback

`window.webview` is a target capability constant; before checking and generating, Bern prunes conditional branches and unused components, and folds conditional expressions in properties. It is always off on tvOS; on macOS, iOS, Android, Windows and Linux it is on by default, and `webview = false` can also generate a release without WebView. The Web uses the browser's capability.

Without WebView (and, on any target, for the documents above that neither move nor run code), `Svg` is lowered to `Image`, keeping the size, fit and accessibility name, and removing the playback, speed, replay, interaction, data and external link properties. The PNG is generated in the build directory's `__bern_static_svg/` and packaged with the resources; resvg is linked only into the build tool. The image is rendered for the largest display side it declares: 6 pixels per point (a 3x screen, then double sampling), for example 276 pixels for a 46-point avatar; one without fixed sizes is rendered at twice its intrinsic size. Each side is capped at 4096 pixels. Images are double-sampled when displayed: decoded or drawn at twice the physical pixels they occupy, then scaled down by half. Declared images with fixed sizes (every given side is a number and the image does not fly, `bern_target_api::svg::one_size`) do this on every frontend, instead of letting the compositor scale directly from a much larger image, which would break edges into jaggies: Windows decodes on the thread pool at twice the display size times the window scale (`BitmapDecoder` with a `BitmapTransform` using the Fant filter, placed into a `SoftwareBitmapSource`), and the compositor scales it down by half; images from files are handled the same way; SwiftUI computes the occupied pixels from the environment's `displayScale`, decodes at twice that with ImageIO thumbnails (`kCGImageSourceThumbnailMaxPixelSize`), and caches each size separately; Compose first decodes with power-of-two integer subsampling to no less than twice, then scales to exactly twice with `Bitmap.createScaledBitmap` (bilinear), and the GPU halves it when drawing; GTK, after being mapped to the window, computes pixels from the surface scale (`GdkSurface::scale`, including fractional scaling), scales to twice with gdk-pixbuf area averaging, and decodes again when the scale changes; the Web leaves it to the browser's own downscaling filter and does nothing extra. HDR images on GTK and Windows go through their respective HDR drawing and skip this step; SwiftUI's thumbnails are still decoded as HDR.

`static_source` specifies the image after the animation has finished, and is especially suited to entrance animations and SVGs whose graphics are generated by scripts; the builder does not run animation scripts to find the final frame. This file should also be listed in `[resources].paths`. A real app's logo uses the final vector artwork, so the version without WebView shows the complete logo directly.
