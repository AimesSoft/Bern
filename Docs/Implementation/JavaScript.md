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

`javascript` is off by default, and it is all an application says: the host of each target is Bern's to pick, and `bern build` refuses a package that sets features on `bern-script`. A test, which has no target to run in, may turn on `boa` as a dev-dependency. Every native target (`macos`, `ios`, `tvos`, `watchos`, `windows`, `linux`, `android`, `wearos`, and GPUI desktop targets) enables `bern-script/boa` in-process so plugins never spawn hidden WebView processes (such as `msedgewebview2.exe`, `WKWebView` XPC processes, or `WebKitWebProcess`); only `platform = "browser"` (`web_mode = "app"`), which already runs inside a browser, uses a Web Worker. The script feature is currently used for targets where the engine and the UI run in the same process; the Web's remote `web_mode = "ui"` does not yet provide remote script routing.

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

On the Web (`platform = "browser"`), each context uses one Worker and asynchronous results come back through the host bridge; requests time out after 30 seconds, and releasing a context or stopping the host ends pending requests. The browser host provides a monotonic clock, so WebAssembly does not depend on `std::time::Instant`.

On native targets, Boa uses a lightweight worker thread for each context and limits loop iterations and recursion depth. It supports plain expressions, JSON, and Promises that can complete within the current task queue; Promises that need an external event to complete return an error. Plugin APIs for networking, storage, players and the like are not yet defined at this layer. This layer is script execution infrastructure; a plugin SDK can add a unified host interface on top of it.

The UI can use `if window.webview { ... } else { ... }`. This is a compile-time condition: the branch not chosen does not enter target checks or code generation. For the automatic static fallback of `Svg(static_source: ...)`, see [Animated SVG](Animated-SVG.md).

Verification: `cargo test -p bern-script`, `cargo test -p bern-script --features boa`, `cargo test -p bern-cli --test svg_targets`, `node --test crates/bern-script/tests/browser.test.mjs`.

Verified on 2026-11-14: compiler / IR / target API unit tests, generation and fallback tests across all platforms, Boa persistent-variable / context-isolation / JSON / Promise / exception / loop-limit tests, and browser Worker tests all passed; all native targets generate `bern-script` with `features = ["boa"]` without spawning hidden WebView script hosts.
