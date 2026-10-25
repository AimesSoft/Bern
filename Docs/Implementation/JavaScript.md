# JavaScript 与目标能力

Bern 的 `bern::script` 提供独立、持久的 JavaScript 上下文。目标配置选择执行宿主；插件调用接口保持一致。

```toml
[targets.macos]
platform = "macos"
javascript = true

[targets.tvos]
platform = "tvos"
javascript = true

# 可选：需要去掉 WebView 依赖的发行版。
[targets.linux_plain]
platform = "linux"
webview = false
javascript = true
```

| 目标 | JS 后端 |
| --- | --- |
| macOS / iOS | WKWebView |
| Android | Android WebView |
| Windows | WebView2 |
| Linux | WebKitGTK 6 |
| Web，web_mode = "app" | 浏览器 Worker |
| tvOS、webview = false | Boa 0.22，纯 Rust 随应用编译 |

`javascript` 默认关闭。生成的 Cargo 工程只在无 WebView 时启用 `bern-script/boa`，系统宿主构建不编译 Boa。Linux 的 `bern-gtk/script` 引入 WebKitGTK；无 WebView 的 Linux 目标同时把 SVG 转为图片，不再引入该依赖。脚本功能目前用于引擎与 UI 同进程的目标；Web 的远程 `web_mode = "ui"` 暂不提供远程脚本路由。

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

系统 WebView 在第一次执行脚本时创建，用私有路由文档与每个上下文的 iframe，Web 用每个上下文的 Worker，异步结果通过宿主桥回传；请求超时 30 秒，释放上下文和停止宿主会结束待处理请求。宿主提供单调时钟，因此 WebAssembly 不依赖 std::time::Instant。

Boa 每个上下文使用独立工作线程，并限制循环次数与递归深度。支持普通表达式、JSON 和能在当前任务队列中完成的 Promise；需要外部事件才能完成的 Promise 返回错误。网络、存储、播放器等插件 API 尚未在此层定义。此层是脚本执行基础设施，插件 SDK 可在其上增加统一宿主接口。

UI 可用 `if window.webview { ... } else { ... }`。这是编译期条件，未选分支不进入目标检查和代码生成。`Svg(static_source: ...)` 的自动静态回退见 [动画 SVG](Animated-SVG.md)。

验证：`cargo test -p bern-script`、`cargo test -p bern-script --features boa`、`cargo test -p bern-cli --test svg_targets`、`node --test crates/bern-script/tests/browser.test.mjs`。macOS 的 `bern-script` 测试还会编译 Swift 宿主并实测 WKWebView。

2026-10-22 验证：编译器／IR／目标 API 的 114 项单元测试、七平台生成与回退测试、GTK 生成和打包测试通过。Boa 的持久变量、独立上下文、JSON、Promise、异常与循环限制测试通过；WKWebView 真正执行与浏览器 Worker 测试通过。一个实际应用的 tvOS 模拟器整包构建成功，二进制未链接 WebKit；启用脚本的 SVG 示例完成 iOS 模拟器、Android APK、Web 整包构建，iOS 依赖树不含 Boa。Windows、Linux 的原生宿主尚未在对应系统上编译运行。
