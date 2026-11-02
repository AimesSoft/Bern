**English** · [简体中文](../zh-CN/Implementation/JavaScript.md)

# JavaScript and target capabilities

Bern's `bern::script` provides independent, persistent JavaScript contexts. The target configuration chooses the execution host; the interface plugins call stays the same.

```toml
[targets.macos]
platform = "macos"
javascript = true

[targets.tvos]
platform = "tvos"
javascript = true

# Optional: a distribution that needs to drop the WebView dependency.
[targets.linux_plain]
platform = "linux"
webview = false
javascript = true
```

| Target | JS backend |
| --- | --- |
| macOS / iOS | WKWebView |
| Android | Android WebView |
| Windows | WebView2 |
| Linux | WebKitGTK 6 |
| Web, web_mode = "app" | Browser Worker |
| tvOS, webview = false | Boa 0.22, pure Rust compiled with the app |

`javascript` is off by default. The generated Cargo project enables `bern-script/boa` only when there is no WebView; builds with a system host do not compile Boa. On Linux, `bern-gtk/script` pulls in WebKitGTK; a Linux target without a WebView also turns SVG into images and no longer pulls in that dependency. The script feature is currently used for targets where the engine and the UI run in the same process; the Web's remote `web_mode = "ui"` does not yet provide remote script routing.

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

The system WebView is created the first time a script runs, with a private routing document and one iframe per context; the Web uses one Worker per context, and asynchronous results come back through the host bridge. Requests time out after 30 seconds, and releasing a context or stopping the host ends pending requests. The host provides a monotonic clock, so WebAssembly does not depend on std::time::Instant.

Boa uses a separate worker thread for each context and limits loop iterations and recursion depth. It supports plain expressions, JSON, and Promises that can complete within the current task queue; Promises that need an external event to complete return an error. Plugin APIs for networking, storage, players and the like are not yet defined at this layer. This layer is script execution infrastructure; a plugin SDK can add a unified host interface on top of it.

The UI can use `if window.webview { ... } else { ... }`. This is a compile-time condition: the branch not chosen does not enter target checks or code generation. For the automatic static fallback of `Svg(static_source: ...)`, see [Animated SVG](Animated-SVG.md).

Verification: `cargo test -p bern-script`, `cargo test -p bern-script --features boa`, `cargo test -p bern-cli --test svg_targets`, `node --test crates/bern-script/tests/browser.test.mjs`. On macOS the `bern-script` tests also compile the Swift host and test WKWebView for real.

Verified on 2026-10-22: 114 unit tests of the compiler / IR / target API, generation and fallback tests for seven platforms, and GTK generation and packaging tests passed. Boa tests for persistent variables, independent contexts, JSON, Promises, exceptions and loop limits passed; tests of real WKWebView execution and of the browser Worker passed. A full-package tvOS simulator build of a real app succeeded, and the binary did not link WebKit; the script-enabled SVG example completed full-package builds for the iOS simulator, an Android APK and the Web, and the iOS dependency tree contained no Boa. The native hosts for Windows and Linux have not yet been compiled and run on those systems.
