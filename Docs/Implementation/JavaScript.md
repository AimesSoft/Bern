**English** · [简体中文](../zh-CN/Implementation/JavaScript.md)

# JavaScript and `bern.cli`

Updated: 2026-11-21. Bern's `bern::script` crate provides opt-in, isolated, persistent JavaScript contexts with a built-in in-process `bern.cli(...)` bridge to the application's command catalog. Apps that do not enable scripting link zero JavaScript runtime.

## Enabling JavaScript

Enable `javascript = true` per target in `Bern.toml`:

```toml
[targets.macos]
platform = "macos"
javascript = true

[targets.windows]
platform = "windows"
javascript = true

# Optional: drop WebView dependencies for SVG/WebPage while keeping JS scripting.
[targets.linux_plain]
platform = "linux"
webview = false
javascript = true
```

| Target | JS backend |
| --- | --- |
| macOS / iOS / tvOS / watchOS | Boa 0.22, pure Rust compiled in process |
| Windows / Linux / Android / Wear OS | Boa 0.22, pure Rust compiled in process |
| Desktop GPUI (macOS / Windows / Linux) | Boa 0.22, pure Rust compiled in process |
| Web (`web_mode = "app"`) | Browser Web Worker |

Key build rules:

- `javascript` is off by default. An application only toggles `javascript = true` in `Bern.toml`; Bern selects the backend per target, and `bern build` rejects packages that set features directly on `bern-script`.
- Standalone unit tests that run outside a target may enable `bern-script/boa` under `[dev-dependencies]`.
- Every native target enables `bern-script/boa` in process, so script execution never spawns hidden system WebView processes (`msedgewebview2.exe`, `WKWebView` XPC helpers or `WebKitWebProcess`). Only `platform = "browser"` (`web_mode = "app"`) uses a Web Worker.
- Scripting targets run the engine and script host in the same process. Remote `web_mode = "ui"` pages do not perform remote script routing; instead, the standalone WebUI engine host runs scripts in process on Boa and routes `bern.cli` session calls to connected pages through `Sessions`.

## Script context basics

```rust
async fn example() -> Result<(), bern::script::Error> {
    let js = bern::script::Context::new()?;
    js.eval("var count = 1").await?;
    let result = js.eval("++count").await?;
    assert_eq!(result, 2);
    Ok(())
}
```

- Keep each `Context` alive for the lifetime of the plugin or script session; global variables are isolated per context.
- `eval` returns an awaitable `Evaluation` (and `try_result` polls without blocking). Never block-wait on the UI thread.
- Values cross the boundary as JSON: `undefined` maps to `null`, and unhandled JavaScript exceptions return `bern::script::Error`.

## In-process `bern.cli` interface

When JavaScript is enabled, every context exposes `globalThis.bern.cli(...)`, routing directly to the running application's command catalog (`Host`, `MediaHub` and `Sessions`) in process without opening an IPC socket. Engine commands invoked from JavaScript arrive with `Entry::Script`.

```js
// 1. CLI command line (same syntax as the application CLI, with quote escaping)
await bern.cli("media seek --player-id main --position-ms 90000");

// 2. Pre-split argument array (avoids quoting for paths or free text)
await bern.cli(["media", "open", "--player-id", "main", "--source", url]);

// 3. Canonical command id (or CLI path) with a JSON parameter object and optional session
await bern.cli("media.seek", { player_id: "main", position_ms: 90000 });
await bern.cli("ui.inspect", {}, { session: sessionId });

// Metadata targets work through the same entry
const snapshot = await bern.cli("state");
const catalog = await bern.cli("catalog");
const sessions = await bern.cli("sessions");
const helpText = await bern.cli("help");
```

`bern.cli` always returns a `Promise`, so the same script runs identically on native Boa and browser Web Workers. Failures reject with a JavaScript `Error` carrying `.code` (such as `permission_denied`, `invalid_params`, `unknown_command`, `cli_timeout` or `cli_unavailable`) and `.message`.

## Allowlist and denylist policies

`Context::new()` defaults to `CliPolicy::allow_all()`, giving scripts full access to the application's CLI. Applications running third-party scripts or permissioned plugins construct contexts with `Context::with_policy(...)` and can update policies live with `Context::set_policy(...)`. Every check runs on the Rust host side against the resolved canonical target id before execution:

```rust
use bern::script::{CliPolicy, Context};

// Denylist mode: full access except specific namespaces
let power = Context::with_policy(
    CliPolicy::allow_all()
        .deny("account.*")
        .deny("trace.*")
        .with_label("automation"),
)?;

// Allowlist mode: deny by default, grant selected namespaces or patterns
let plugin = Context::with_policy(
    CliPolicy::allow_only(["media.*", "@queries", "state", "catalog", "help"])
        .deny("media.bind")
        .deny_state("account.*")
        .with_label("example.plugin"),
)?;
```

| Pattern | Matches |
| --- | --- |
| `*` | Every command, query, media command, session command and metadata target |
| `<namespace>.*` | Every canonical id under that namespace (for example `media.*`, `ui.*`, `window.*`, `trace.*`) |
| `@queries` | All read-only engine queries |
| `@engine` | All engine commands and queries |
| `@media` | All media commands |
| `@session` | All UI, window and trace session commands |
| `@meta` | `state`, `sessions`, `catalog` and `help` |
| Exact id | That exact target (for example `media.seek`, `ui.activate`, `state`) |

- Deny rules always override allow rules.
- By default `catalog` and `help` are filtered to entries allowed by the context's policy (`filter_catalog(true)`), and `state` snapshots honor `allow_state`, `allow_state_only` and `deny_state`.
- For parameter-level or dynamic runtime checks, attach a Rust interceptor with `CliPolicy::with_filter(...)` to inspect resolved arguments or live permission state before execution.

## Execution model and timeouts

- **Native targets (Boa)**: each context runs on a lightweight worker thread with an event loop and enforces loop iteration and recursion limits. It supports plain expressions, JSON, chained `await bern.cli(...)` calls and async callbacks scheduled during evaluation; Promises that depend on an uninstalled external host return an error.
- **Web (`platform = "browser"`)**: each context uses a dedicated Web Worker and exchanges asynchronous results (including `bern.cli` requests) over the host bridge. Script evaluations time out after 30 seconds, `bern.cli` calls follow `CliPolicy::with_timeout_ms` (default 10 seconds), and dropping a context or stopping the host cancels pending requests. The browser host supplies a monotonic clock so WebAssembly does not depend on `std::time::Instant`.

## Compile-time `window.webview` branches

UI declarations can branch on `if window.webview { ... } else { ... }`. This condition is resolved at compile time: the unselected branch is excluded from target capability checks and code generation. For the automatic static fallback of `Svg(static_source: ...)`, see [Animated SVG](Animated-SVG.md).

## Verification

Commands: `cargo test -p bern-script`, `cargo test -p bern-script --features boa`, `cargo test -p bern-adapter-cli`, `cargo test -p bern-cli --test svg_targets`, `node --test crates/bern-script/tests/browser.test.mjs`.

Verified on 2026-11-21: workspace tests, `bern-script --features boa` tests, `bern-adapter-cli::script` tests and `browser.test.mjs` Worker tests passed in CI, covering in-process `bern.cli` invocation forms, `CliPolicy` allowlist/denylist/state/catalog filtering, dynamic filters, timeouts, Boa persistent contexts/event loops and browser Worker bridging across native and Web targets.
