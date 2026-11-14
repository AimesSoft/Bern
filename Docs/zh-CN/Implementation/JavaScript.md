[English](../../Implementation/JavaScript.md) · **简体中文**

# JavaScript 与目标能力

Bern 的 `bern::script` 提供独立、持久的 JavaScript 上下文。目标配置启用脚本执行；插件调用接口在各平台保持一致。

```toml
[targets.macos]
platform = "macos"
javascript = true

[targets.windows]
platform = "windows"
javascript = true

# 可选：需要去掉 SVG/WebPage 的 WebView 依赖的发行版。
[targets.linux_plain]
platform = "linux"
webview = false
javascript = true
```

| 目标 | JS 后端 |
| --- | --- |
| macOS / iOS / tvOS / watchOS | Boa 0.22，纯 Rust 随应用编译 |
| Windows / Linux / Android / Wear OS | Boa 0.22，纯 Rust 随应用编译 |
| Web，web_mode = "app" | 浏览器 Worker |

`javascript` 默认关闭，应用只需要声明这一项：每个目标用什么宿主由 Bern 决定，包在 `bern-script` 上设置特性时 `bern build` 会报错。测试没有目标可运行，可以在 dev-dependencies 里打开 `boa`。所有原生目标（`macos`、`ios`、`tvos`、`watchos`、`windows`、`linux`、`android`、`wearos` 及 GPUI 桌面目标）统一在进程内启用 `bern-script/boa`，运行脚本不再拉起隐藏的系统 WebView 进程群（如 `msedgewebview2.exe`、`WKWebView` XPC 进程或 `WebKitWebProcess`）；仅本身运行在浏览器内的 `platform = "browser"`（`web_mode = "app"`）使用浏览器 Worker。脚本功能目前用于引擎与 UI 同进程的目标；Web 的远程 `web_mode = "ui"` 暂不提供远程脚本路由。

```rust
async fn example() -> Result<(), bern::script::Error> {
    let js = bern::script::Context::new()?;
    js.eval("var count = 1").await?;
    let result = js.eval("++count").await?;
    assert_eq!(result, 2);
    Ok(())
}
```

`Context` 应在插件存活期间保留；各上下文的全局变量独立。`eval` 返回可 await 的 `Evaluation`，也可使用 `try_result` 非阻塞读取。不要在 UI 线程阻塞等待。返回值使用 JSON，undefined 对应 null，脚本异常返回 `Error`。

Web 端（`platform = "browser"`）每个上下文使用独立 Worker，异步结果通过宿主桥回传；请求超时 30 秒，释放上下文和停止宿主会结束待处理请求。浏览器宿主提供单调时钟，因此 WebAssembly 不依赖 `std::time::Instant`。

原生目标上，Boa 每个上下文使用轻量独立工作线程，并限制循环次数与递归深度。支持普通表达式、JSON 和能在当前任务队列中完成的 Promise；需要外部事件才能完成的 Promise 返回错误。网络、存储、播放器等插件 API 尚未在此层定义。此层是脚本执行基础设施，插件 SDK 可在其上增加统一宿主接口。

UI 可用 `if window.webview { ... } else { ... }`。这是编译期条件，未选分支不进入目标检查和代码生成。`Svg(static_source: ...)` 的自动静态回退见 [动画 SVG](Animated-SVG.md)。

验证：`cargo test -p bern-script`、`cargo test -p bern-script --features boa`、`cargo test -p bern-cli --test svg_targets`、`node --test crates/bern-script/tests/browser.test.mjs`。

2026-11-14 验证：编译器／IR／目标 API 单元测试、全平台生成与回退测试、Boa 的持久变量／上下文隔离／JSON／Promise／异常／循环限制测试及浏览器 Worker 测试全部通过；所有原生目标统一生成带 `features = ["boa"]` 的进程内脚本引擎，不再创建隐藏 WebView 脚本宿主。
