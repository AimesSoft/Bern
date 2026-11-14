**English** · [简体中文](../zh-CN/Implementation/Animated-SVG.md)

# Animated SVG

Bern ships a pure-Rust animated SVG compiler and player (`bern-svg`) behind the `Svg` component. Neither `Svg` nor `bern-svg` uses a WebView or JavaScript engine on any target: static SVGs lower to build-time PNG `Image`s, while animated and interactive SVGs compile at build time into compact binary `BSVG` scenes (`__bern_anim_svg/<source>.bsvg`) and play in-process through `tiny-skia` SIMD rasterization across SwiftUI (macOS, iOS, tvOS, watchOS), Compose (Android, Wear OS), WinUI 3 and XAML Islands, GTK 4, GPUI, and WebAssembly (`<canvas>`).

## Quick start

1. Register the `.svg` file in `[resources].paths` of `Bern.toml`:

```toml
[resources]
paths = ["assets/logo-animated.svg"]
```

2. Place `Svg` in `ui!`:

```rust
Svg(
    source: "assets/logo-animated.svg",
    width: 120,
    height: 120,
    label: tr! { "zh-Hans" => "应用徽标", "en" => "Application logo" },
    playing: app.logo_playing,
    speed: app.logo_speed,
    restart: app.logo_revision
)
```

3. Author animations with standard CSS `@keyframes`, CSS `transition`, or SMIL `<animate>` / `<animateTransform>`, and declare click or double-click state changes directly on `<svg>` with `data-bern-*` attributes (no JavaScript required):

```xml
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="0 0 200 200"
     data-bern-ready="1.8s"
     data-bern-dblclick="random(do-wink: 1.5s, do-nod: 2.2s, do-wave: 2.0s)">
  <style>
    .mark { transform-box: fill-box; transform-origin: 50% 50%; animation: pop .6s cubic-bezier(.3, .7, .4, 1) both; }
    .do-wink .eye-left { transform-box: fill-box; transform-origin: 50% 75%; animation: wink 1.5s ease-in-out both; }
    .do-nod  .head     { transform-box: fill-box; transform-origin: 50% 90%; animation: nod 2.2s ease-in-out both; }
    .do-wave .hand     { transform-box: fill-box; transform-origin: 20% 90%; animation: wave 2.0s ease-in-out both; }
    @keyframes pop  { 0% { transform: scale(0); } 65% { transform: scale(1.06); } 100% { transform: none; } }
    @keyframes wink { 0%, 100% { transform: scaleY(1); } 15%, 80% { transform: scaleY(0.05); } }
    @keyframes nod  { 0%, 100% { transform: rotate(0); } 35% { transform: rotate(6deg); } 70% { transform: rotate(-3deg); } }
    @keyframes wave { 0%, 100% { transform: rotate(0); } 30% { transform: rotate(-18deg); } 60% { transform: rotate(14deg); } }
    @media (prefers-reduced-motion: reduce) { .mark { animation: none; } }
  </style>
  <g class="mark">
    <g class="head">
      <circle cx="100" cy="100" r="72" fill="#3E5BD8"/>
      <g class="eye-left"><ellipse cx="76" cy="94" rx="8" ry="14" fill="#DCE6FF"/></g>
      <ellipse cx="124" cy="94" rx="8" ry="14" fill="#DCE6FF"/>
    </g>
    <g class="hand"><circle cx="162" cy="138" r="14" fill="#F48BA6"/></g>
  </g>
</svg>
```

## Component interface

| Property | Meaning |
| --- | --- |
| `source` | Required `.svg` resource declared in `Bern.toml`, validated and compiled at build time |
| `static_source` | Optional `.svg` resource retained for source compatibility; animated SVGs run natively on every target |
| `width`, `height` | Required logical size in points (may bind numeric state); rasterized at exact physical display scale |
| `label` | Accessibility label; when omitted, the element is marked decorative |
| `fit` | `contain` (default), `cover`, `fill`, matching SVG `meet`, `slice`, `none` |
| `playing` | Boolean state, default `true`; pauses or resumes CSS, transition, SMIL, and timed interaction clocks |
| `speed` | Positive numeric state, default `1`; scales timeline progression; non-positive or non-finite values keep the valid rate |
| `restart` | Numeric state; when its value changes, timelines reset to `0` while preserving toggled classes |
| `interactive` | Build-time boolean, default `true`; enables pointer events (`click`, `dblclick`, `:hover`, `:active`) |
| `data`, `external_links` | Retained optional properties for compatibility; native `BSVG` scenes never execute scripts or navigate |

## Declarative interactions

Instead of spinning up a browser DOM and JavaScript VM to toggle classes on pointer input, `bern-svg` compiles declarative attributes on the root `<svg>` element directly into the `BSVG` state machine:

| Attribute | Syntax and behavior |
| --- | --- |
| `data-bern-ready` | Optional guard duration such as `"2.3s"` or `"500ms"`. Clicks and double-clicks before the entrance timeline reaches this timestamp are ignored; when the user enables Reduce Motion, interactions are ready immediately at `0s` |
| `data-bern-click` | Action run on single click / tap (or TV remote select) |
| `data-bern-dblclick` | Action run on double-click / double-tap |
| `toggle(class)` | Adds `class` to `<svg>` if absent, or removes it if present; pairs naturally with CSS `transition` (for example `data-bern-dblclick="toggle(alternate)"`) |
| `trigger(class: 1.5s)` | Applies `class` to `<svg>` from `t = 0` of its `@keyframes` and automatically removes it once `1.5s` of animation time elapses (duration is optional) |
| `cycle(a: 1.2s, b: 1.2s, none)` | Advances through the listed states in order on each trigger; `none` clears the active state class |
| `random(a: 1.5s, b: 2.6s, c: 2.8s)` | Picks one state at random without repeating the previous pick back-to-back (when two or more entries exist), restarts its animations from `0%`, and removes the class after its duration |

CSS `:hover` and `:active` pseudo-classes on `<svg>` or descendant shapes also respond automatically while `interactive` is `true`.

**Sharing an `.svg` with standalone web previews:** if an `.svg` file is also embedded via `<object>` in a standalone HTML design page, you may keep its browser `<script>` alongside `data-bern-*` on `<svg>`. When `data-bern-*` is present, `bern-svg` compiles the declarative interaction and strips `<script>` cleanly; if `<script>` or `on*` attributes appear *without* `data-bern-*`, `bern check` and `bern build` report a compile-time error guiding you to `data-bern-click` / `data-bern-dblclick`.

## Supported SVG, CSS, and SMIL subset

`bern-svg` is purpose-built for animated icons, brand marks, status indicators, and interactive vector illustrations:

- **Geometry and structure**: `<svg>`, `<g>`, `<defs>`, `<use>`, `<path>` (all SVG 1.1/2 commands `M/m`, `L/l`, `H/h`, `V/v`, `C/c`, `S/s`, `Q/q`, `T/t`, `A/a`, `Z/z`), `<rect>` (including `rx`/`ry`), `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`.
- **Paints, strokes, and clipping**: solid colors (`#hex`, `rgb()`/`rgba()`, named colors), `currentColor`, `light-dark(light, dark)`, `<linearGradient>` and `<radialGradient>` (`objectBoundingBox` and `userSpaceOnUse`, `gradientTransform`), `fill-opacity`, `fill-rule` (`nonzero`, `evenodd`), `stroke`, `stroke-width`, `stroke-opacity`, `stroke-linecap`, `stroke-linejoin`, `stroke-dasharray`, `stroke-dashoffset`, `<clipPath id="...">` (`clip-path="url(#...)"`), and isolated group `<g opacity="...">` compositing.
- **CSS cascade and media queries**: tag, `#id`, `.class`, compound (`.eye.l`), child (`>`), and descendant (`.do-wink .rx`) selectors, `:hover`, `:active`, `!important`, `@media (prefers-color-scheme: dark|light)`, and `@media (prefers-reduced-motion: reduce)`.
- **CSS `@keyframes` and `transition`**: `animation` shorthand and longhands (`animation-name`, `animation-duration`, `animation-delay`, `animation-timing-function`, `animation-iteration-count`, `animation-direction`, `animation-fill-mode`), independent per-property keyframe stops, `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(...)`, `steps(...)`, `transform-box: fill-box | view-box` (with exact Bezier tight-bounds `getBBox()` precomputed at build time), percentage and pixel `transform-origin`, prefix-matched transform interpolation with W3C 2D polar matrix decomposition fallback, and CSS `transition` on `fill`, `stroke`, `opacity`, `transform`, `stroke-width`, and `stroke-dashoffset`.
- **SMIL timelines**: `<animate>`, `<set>`, and `<animateTransform>` (`values`, `from`/`to`, `keyTimes`, `keySplines`, `calcMode`, `begin`, `dur`, `repeatCount`, `fill="freeze"`).

Not supported in `bern-svg` (use static SVGs lowered via `resvg` or native Bern `Text` / `WebPage` instead): `<text>` inside animated SVGs, `<filter>` (`feGaussianBlur`, etc.), `<foreignObject>`, external network subresources, and arbitrary JavaScript execution.

## Two-stage architecture and zero-tick idle sleep

1. **Build-time classification and compilation (`bern-target-api::svg`)**:
   - **Static SVGs (`bern_svg::picture`)**: documents with no animations, transitions, `:hover`/`:active`, `<script>`, or `data-bern-*` attributes are rasterized once at build time via `resvg` into `__bern_static_svg/<source>.png` (plus `.dark.png` for appearance-adaptive SVGs, or `rendering: template` for pure `currentColor` icons) and lowered to `Image`.
   - **Animated SVGs (`bern_svg::compile`)**: compiled at build time into `__bern_anim_svg/<source>.bsvg`. XML parsing, CSS selector parsing, SVG path arc conversion, and exact Bezier `fill-box` bounding boxes happen at build time; shipped native and WASM binaries link `bern-svg` with `default-features = false`, carrying zero XML/CSS parser code.
2. **In-process shared cache and idle sleep (`bern-ffi::svg`)**:
   - Decoded `Scene`s (with pre-built `tiny_skia::Path` geometry) are cached once per resource key in an `Arc<Scene>` across the process; opening a page again reuses the cached scene immediately.
   - Each `bern_svg_render` call returns two bits: `changed` (skip texture upload when pixel state is identical) and `active` (whether any CSS animation, CSS transition, SMIL track, or timed interaction is still in flight).
   - As soon as finite entrance or click animations settle (`active == false`), every frontend pauses its frame clock (`TimelineView` on Apple platforms, `withFrameNanos` on Android/Wear OS, `CompositionTarget::Rendering` on Windows, `add_tick_callback` on GTK 4, `request_animation_frame` on GPUI, `requestAnimationFrame` on Web), dropping idle CPU usage to **0%** until an interaction or state binding wakes it.

## Presentation across targets

| Target | Native presenter | Runtime dependency |
| --- | --- | --- |
| macOS / iOS / tvOS / watchOS | SwiftUI `TimelineView` + `CGImage` (`RgbaPremul`) | In-process `bern-ffi` (`bern-svg`), zero `WebKit` |
| Android / Wear OS | Compose `Canvas` + JNI `DirectByteBuffer` → `Bitmap` | In-process `libbern_app.so`, zero `android.webkit.WebView` |
| Windows (WinUI 3 and XAML Islands) | XAML `Image` + `WriteableBitmap` (`BgraPremul`) | In-process `bern-ffi`, zero `WebView2` or child HWND airspace issues |
| Linux / GTK 4 | `GtkPicture` + `gdk::MemoryTexture` (`R8g8b8a8Premultiplied`) | In-process `bern-gtk`, zero `libwebkitgtk-6.0-4` or AppArmor `userns` profile |
| GPUI (macOS / Windows / Linux) | GPUI `canvas` + `RenderImage` (`BgraStraight`) | In-process `bern-gpui` |
| Web (`web_app` and `web_ui`) | `<canvas class="bern-svg">` + `ImageData` (`RgbaStraight`) | `app.wasm` exports, zero `<iframe>` |

## Standalone Rust crate usage

`bern-svg` can also be used directly in Rust outside the Bern UI macro:

```rust
use bern_svg::{Config, Fit, PixelFormat, Player, PointerEvent, Scene};
use std::sync::Arc;

let bsvg_bytes = bern_svg::compile(svg_source)?;
let scene = Arc::new(Scene::from_bytes(&bsvg_bytes)?);
let mut player = Player::new(scene);

player.configure(Config {
    playing: true,
    speed: 1.0,
    fit: Fit::Contain,
    dark: false,
    ..Config::default()
});

let (step, rgba_premul) = player.render(0.0, 256, 256, PixelFormat::RgbaPremul);
if step.active {
    // Schedule next frame; pause frame loop once `step.active` becomes false.
}
let wake = player.pointer(PointerEvent::DoubleClick(0.5, 0.5));
```

## Verification

`examples/svg-animation` exercises CSS rotation, SMIL radius pulsing, radial gradients, declarative `data-bern-dblclick="toggle(alternate)"` with CSS color transition, and native play/pause, speed, and restart bindings across all targets:

```sh
cargo test -p bern-svg -p bern-ffi -p bern-compiler
cargo test -p bern-cli --test svg_targets
cargo test -p bern --test docs
```
