**English** · [简体中文](../zh-CN/Implementation/JavaScript.md)

# JavaScript and target capabilities

Bern's `bern::script` provides independent, persistent JavaScript contexts. The target configuration enables script execution; the interface plugins call stays the same across platforms.

```toml
[targets.macos]
platform = "macos"
javascript = true

[targets.windows]
platform = "windows"
javascript = true

# Optional: a distribution that needs to drop the WebView dependency for SVG/WebPage.
[targets.linux_plain]
platform = "linux"
webview = false
javascript = true
```

| Target | JS backend |
| --- | --- |
| macOS / iOS / tvOS / watchOS | Boa 0.22, pure Rust compiled with the app |
| Windows / Linux / Android / Wear OS | Boa 0.22, pure Rust compiled with the app |
| Web, web_mode = "app" | Browser Worker |

`javascript` is off by default, and it is all an application says: the host of each target is Bern's to pick, and `bern build` refuses a package that sets features on `bern-script`. A test, which has no target to run in, may turn on `boa` as a dev-dependency. Every native target (`macos`, `ios`, `tvos`, `watchos`, `windows`, `linux`, `android`, `wearos`, and GPUI desktop targets) enables `bern-script/boa` in-process so plugins never spawn hidden WebView processes (such as `msedgewebview2.exe`, `WKWebView` XPC processes, or `WebKitWebProcess`); only `platform = "browser"` (`web_mode = "app"`), which already runs inside a browser, uses a Web Worker. The script feature is currently used for targets where the engine and the UI run in the same process; the Web's remote `web_mode = "ui"` does not yet provide remote script routing, while the standalone WebUI engine host runs scripts in-process on Boa and routes `bern.cli` to connected pages through `Sessions`.

```rust
async fn example() -> Result<(), bern::script::Error> {
    let js = bern::script::Context::new()?;
    js.eval("var count = 1").await?;
    let result = js.eval("++count").await?;
    assert_eq!(result, 2);
    Ok(())
}
```

A `Context` should be kept for as long as the plugin is alive; the global variables of each context are independent. `eval` returns an awaitable `Evaluation`, and `try_result` can also read it without blocking. Do not block and wait on the UI thread. Return values use JSON, undefined maps to null, and script exceptions return `Error`.

## In-process `bern.cli` interface

When JavaScript is enabled, every context exposes `globalThis.bern.cli(...)`, routing directly to the running application's command catalog (`Host`, `MediaHub`, and `Sessions`) in-process without opening an IPC socket. Engine commands invoked this way arrive with `Entry::Script`.

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

`bern.cli` always returns a `Promise` so the same script runs identically on native Boa and browser Web Workers. Failures reject with a JavaScript `Error` carrying `.code` (such as `permission_denied`, `invalid_params`, `unknown_command`, `cli_timeout`, or `cli_unavailable`) and `.message`.

## Allowlist and denylist policies

`Context::new()` defaults to `CliPolicy::allow_all()`, giving scripts full access to the application's CLI. Applications that run untrusted scripts or permissioned plugins construct contexts with `Context::with_policy(...)` and can update permissions at runtime with `Context::set_policy(...)`. All rules are enforced on the Rust host side against the resolved canonical target id before execution:

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
| `*` | Every command, query, media command, session command, and metadata target |
| `<namespace>.*` | Every canonical id under that namespace (for example `media.*`, `ui.*`, `window.*`, `trace.*`) |
| `@queries` | All read-only engine queries |
| `@engine` | All engine commands and queries |
| `@media` | All media commands |
| `@session` | All UI, window, and trace session commands |
| `@meta` | `state`, `sessions`, `catalog`, and `help` |
| Exact id | That exact target (for example `media.seek`, `ui.activate`, `state`) |

Deny rules always override allow rules. By default `catalog` and `help` are filtered to entries allowed by the context's policy (`filter_catalog(true)`), and `state` snapshots honor `allow_state` / `allow_state_only` / `deny_state`. Applications can also attach a dynamic Rust interceptor with `CliPolicy::with_filter(...)` to inspect resolved parameters or live permission state before a call executes.

On the Web (`platform = "browser"`), each context uses one Worker and asynchronous results (including `bern.cli` calls) come back through the host bridge; evaluations time out after 30 seconds, `bern.cli` calls follow `CliPolicy::with_timeout_ms` (default 10 seconds), and releasing a context or stopping the host ends pending requests. The browser host provides a monotonic clock, so WebAssembly does not depend on `std::time::Instant`.

On native targets, Boa uses a lightweight worker thread with an event loop for each context and limits loop iterations and recursion depth. It supports plain expressions, JSON, `await bern.cli(...)` chains, and background async callbacks triggered during synchronous evaluations; Promises that depend on an uninstalled external host still return an error.

The UI can use `if window.webview { ... } else { ... }`. This is a compile-time condition: the branch not chosen does not enter target checks or code generation. For the automatic static fallback of `Svg(static_source: ...)`, see [Animated SVG](Animated-SVG.md).

Verification: `cargo test -p bern-script`, `cargo test -p bern-script --features boa`, `cargo test -p bern-adapter-cli`, `cargo test -p bern-cli --test svg_targets`, `node --test crates/bern-script/tests/browser.test.mjs`.

Verified on 2026-11-14: compiler / IR / target API unit tests, generation and fallback tests across all platforms, Boa persistent-variable / context-isolation / JSON / Promise / exception / loop-limit tests, and browser Worker tests all passed; all native targets generate `bern-script` with `features = ["boa"]` without spawning hidden WebView script hosts.
