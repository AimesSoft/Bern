**English** · [简体中文](../zh-CN/Platforms/Web.md)

# Web (WASM App & WebUI)

Bern compiles browser targets (`platform = "browser"`, `ui = "web"`) to lightweight **DOM + CSS** paired with your Rust engine. Because a browser target can run the engine either inside the browser or on a native host, every Web target explicitly chooses a `web_mode`:

- **`web_mode = "app"` (WASM App)**: Compiles the Rust engine to WebAssembly and runs everything in the browser tab. Ideal for static hosting and zero-install web apps.
- **`web_mode = "ui"` (WebUI)**: Generates a browser UI that connects over WebSocket JSON to a native `<app>-host` server (or directly into a running native desktop/mobile app via `web_ui`). Ideal when the engine needs full native I/O, local filesystem access, or a companion CLI/Skill.

Web targets can be built on macOS, Linux, or Windows.

## Prerequisites

- **Rust 1.99+** (`rustup update`).
- Add the WebAssembly Rust target:

```bash
rustup target add wasm32-unknown-unknown
bern doctor
```

If your project or a module pulls in `wasm-bindgen`, install a matching `wasm-bindgen-cli` on `PATH` (or point `BERN_WASM_BINDGEN` at the exact executable).

## Configuration in `Bern.toml`

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[app.icon]
default = "assets/icon.png"
web = "assets/web-icons"         # Optional: PNG, SVG, or folder of favicons

# Option 1: Standalone WebAssembly app
[targets.web_app]
platform = "browser"
ui = "web"
web_mode = "app"
# javascript = true              # Optional: runs bern::script inside a Web Worker

# Option 2: WebUI backed by a native engine host
[targets.web_ui]
platform = "browser"
ui = "web"
web_mode = "ui"
```

### Embedding WebUI inside a native app

Any native target can bundle a `web_mode = "ui"` target inside its resources (`__bern_web_ui/`) so users can open the native app's live UI from any browser on the LAN:

```toml
[targets.macos]
platform = "macos"
ui = "swiftui"
web_ui = "web_ui"                # References [targets.web_ui] above
```

Inside your Rust engine, start or stop the embedded server at runtime (typically behind a user setting):

```rust
use bern::web_ui::{self, Options};

// Start serving WebUI (and optional custom HTTP Routes) on port 8787
web_ui::set(Some(Options { port: 8787, routes: None }));

// Inspect state (Stopped / Running { port } / Failed(String))
let status = web_ui::status();

// Stop serving
web_ui::set(None);
```

## Build, run, and debug

```bash
# Build static WASM app or WebUI + native host
bern build --target web_app
bern build --target web_ui

# Start local dev server and open in your browser
# Passing --port keeps browser localStorage/origin consistent across restarts
bern run --target web_app --port 8080
bern run --target web_ui --port 8081
```

During `bern run --target web_app`, Bern runs a local IPC bridge so the generated CLI can still inspect and drive the live browser tab during development:

```bash
target/bern/web_app/bin/tasks --help
target/bern/web_app/bin/tasks window screenshot --output web.png
```

For `web_mode = "ui"`, the standalone native host (`<app>-host`) listens on `127.0.0.1`, validates HTTP `Origin`/`Host` headers, and authenticates the WebSocket session using a per-launch fragment token—while still exposing the generated app CLI and Skill for automation.

## Platform behavior and adaptations

- **Shared self-drawn visual system (`bern-surface`)**: Emits `--bern-*` theme tokens and control rules from `bern-surface` over `bern-theme` snapshots (shared with GPUI), delivering consistent translucent surfaces, spacing, typography, and perpendicular white capsule slider thumbs while preserving semantic HTML/CSS, native IME, and WAI-ARIA and respecting `prefers-color-scheme`, `WindowSet(appearance:)`, and `Window(idle_enabled:)`.
- **Windows and dialogs**: Secondary windows and `Dialog` views render as accessible virtual dialogs inside the viewport.
- **Files and storage**: Local persistence uses same-origin `localStorage` in `web_mode = "app"` and native file storage in `web_mode = "ui"`. Browser file pickers expose `blob:` URLs that plug straight into `MediaSource`.
- **Orientation & TV**: `Window(landscape: true)` calls the Screen Orientation API where supported by mobile browsers; TV browsers activate `Layout(size: tv)` and `font(tv:)`.
- **Animated SVG & JavaScript**: Animated `Svg` compiles to `BSVG` bytecode rendered by pure-Rust `bern-svg` (no heavy runtime library). `javascript = true` is supported on `web_mode = "app"` via an isolated Web Worker.
- **Web extensions & native views**: Mount custom DOM/Web Component views via `NativeView(web: "...")` ([Native views](../Topics/Native-Views.md)), or bundle third-party JS/WASM assets with `web_extension` ([Web extensions](../Topics/Web-Extensions.md)).
- **Not applicable**: `MenuBar`, `StatusItem`, and `Switch(login_item: true)` are accepted cleanly as no-ops.

## Packaging and signing

```bash
bern package --target web_app
bern package --target web_ui
```

Packaging outputs in `target/bern/dist/`:

| Mode | Output | Contents |
| --- | --- | --- |
| `web_mode = "app"` | Static `.zip` and `.tar.gz` | `index.html`, compiled `.wasm`, JS glue, CSS, and resources—ready to deploy to any static web host or CDN (no CLI or Skill bundled). |
| `web_mode = "ui"` | Platform `.zip` / `.tar.gz` | Native `<app>-host` binary, static WebUI page, generated app CLI (`bin/<app>`), and Skill (`skills/<app>/SKILL.md`). |
