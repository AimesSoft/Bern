[English](../../Implementation/JavaScript.md) · **简体中文**

# JavaScript 与 `bern.cli`

更新：2026-11-21。Bern 的 `bern::script` 提供按需启用、彼此隔离、持久存活的 JavaScript 上下文，并默认内建进程内 `bern.cli(...)` 接口直通应用命令目录。未启用脚本的应用不链接任何 JavaScript 运行时。

## 启用 JavaScript

在 `Bern.toml` 中按目标配置 `javascript = true`：

```toml
[targets.macos]
platform = "macos"
javascript = true

[targets.windows]
platform = "windows"
javascript = true

# 可选：在保留 JS 脚本能力的同时去掉 SVG/WebPage 的 WebView 依赖。
[targets.linux_plain]
platform = "linux"
webview = false
javascript = true
```

| 目标 | JS 后端 |
| --- | --- |
| macOS / iOS / tvOS / watchOS | Boa 0.22，纯 Rust 进程内编译 |
| Windows / Linux / Android / Wear OS | Boa 0.22，纯 Rust 进程内编译 |
| 桌面端 GPUI（macOS / Windows / Linux） | Boa 0.22，纯 Rust 进程内编译 |
| Web（`web_mode = "app"`） | 浏览器 Web Worker |

关键构建规则：

- `javascript` 默认关闭。应用只需在 `Bern.toml` 里打开 `javascript = true`，具体宿主由 Bern 按目标选定；如果在包里手动为 `bern-script` 配置 Cargo feature，`bern build` 会直接拒绝。
- 没有构建目标上下文的独立单元测试可以在 `[dev-dependencies]` 中启用 `bern-script/boa`。
- 所有原生目标统一在进程内启用 `bern-script/boa`，执行脚本不会拉起隐藏的系统 WebView 进程群（如 `msedgewebview2.exe`、`WKWebView` XPC 进程或 `WebKitWebProcess`）；仅运行在浏览器内的 `platform = "browser"`（`web_mode = "app"`）使用 Web Worker。
- 脚本上下文要求引擎与脚本宿主位于同一进程。远程 `web_mode = "ui"` 页面本身不做远程脚本路由；独立 WebUI 引擎宿主在进程内以 Boa 运行脚本，并通过 `Sessions` 将 `bern.cli` 会话命令路由到已连接的页面。

## 脚本上下文基础

```rust
async fn example() -> Result<(), bern::script::Error> {
    let js = bern::script::Context::new()?;
    js.eval("var count = 1").await?;
    let result = js.eval("++count").await?;
    assert_eq!(result, 2);
    Ok(())
}
```

- 在插件或脚本会话存活期间保持 `Context` 实例；不同上下文之间的全局变量完全隔离。
- `eval` 返回可 `await` 的 `Evaluation`，也可用 `try_result` 非阻塞轮询。请勿在 UI 线程上阻塞等待。
- 跨边界返回值采用 JSON：`undefined` 映射为 `null`，未捕获的脚本异常返回 `bern::script::Error`。

## 进程内 `bern.cli` 接口

启用 JavaScript 后，每个上下文默认注入 `globalThis.bern.cli(...)`，在进程内直接对接运行中应用的统一命令目录（`Host`、`MediaHub` 与 `Sessions`），无需开启 IPC 套接字。由此触发的引擎命令携带 `Entry::Script` 来源标识。

```js
// 1. CLI 命令行字符串（与外部应用 CLI 语法一致，支持引号转义）
await bern.cli("media seek --player-id main --position-ms 90000");

// 2. 参数数组（含路径或自由文本时免转义）
await bern.cli(["media", "open", "--player-id", "main", "--source", url]);

// 3. 规范命令 ID（或 CLI 路径）+ JSON 参数对象 + 可选会话选项
await bern.cli("media.seek", { player_id: "main", position_ms: 90000 });
await bern.cli("ui.inspect", {}, { session: sessionId });

// 元命令同样通过同一入口调用
const snapshot = await bern.cli("state");
const catalog = await bern.cli("catalog");
const sessions = await bern.cli("sessions");
const helpText = await bern.cli("help");
```

`bern.cli` 始终返回 `Promise`，从而保证同一份脚本在原生 Boa 与浏览器 Web Worker 上行为一致。调用失败时 reject 为携带 `.code`（如 `permission_denied`、`invalid_params`、`unknown_command`、`cli_timeout` 或 `cli_unavailable`）与 `.message` 的 JavaScript `Error`。

## 黑白名单策略

`Context::new()` 默认使用 `CliPolicy::allow_all()`，向脚本全量开放应用 CLI。运行第三方脚本或权限化插件的应用可通过 `Context::with_policy(...)` 配置策略，并随时通过 `Context::set_policy(...)` 在运行期热更新权限。所有规则均在 Rust 宿主侧基于解析后的规范目标 ID 执行校验，脚本无法绕过：

```rust
use bern::script::{CliPolicy, Context};

// 黑名单模式：默认全开，仅拦截指定命名空间
let power = Context::with_policy(
    CliPolicy::allow_all()
        .deny("account.*")
        .deny("trace.*")
        .with_label("automation"),
)?;

// 白名单模式：默认全拒，按需放行命名空间或规则
let plugin = Context::with_policy(
    CliPolicy::allow_only(["media.*", "@queries", "state", "catalog", "help"])
        .deny("media.bind")
        .deny_state("account.*")
        .with_label("example.plugin"),
)?;
```

| 规则模式 | 匹配范围 |
| --- | --- |
| `*` | 所有命令、查询、媒体命令、会话命令与元目标 |
| `<namespace>.*` | 该命名空间下的全部规范 ID（如 `media.*`、`ui.*`、`window.*`、`trace.*`） |
| `@queries` | 所有只读引擎查询 |
| `@engine` | 所有引擎命令与查询 |
| `@media` | 所有媒体命令 |
| `@session` | 所有 UI、窗口与 trace 会话命令 |
| `@meta` | `state`、`sessions`、`catalog` 与 `help` |
| 精确 ID | 指定目标本身（如 `media.seek`、`ui.activate`、`state`） |

- 黑名单优先级始终高于白名单。
- 默认情况下 `catalog` 与 `help` 会按当前上下文策略自动过滤（`filter_catalog(true)`），`state` 快照同时遵守 `allow_state`、`allow_state_only` 与 `deny_state`。
- 需要按具体参数或运行期实时状态拦截时，可通过 `CliPolicy::with_filter(...)` 挂接动态 Rust 拦截器，在执行前进行二次校验。

## 运行模型与超时

- **原生目标（Boa）**：每个上下文运行在带事件循环的轻量独立工作线程上，并限制循环次数与递归深度。支持普通表达式、JSON、连续 `await bern.cli(...)` 链以及求值期间调度的异步回调；依赖未安装外部宿主的 Promise 返回错误。
- **Web 端（`platform = "browser"`）**：每个上下文使用独立 Web Worker，异步结果（含 `bern.cli` 调用）通过宿主桥回传。脚本求值超时 30 秒，`bern.cli` 超时遵循 `CliPolicy::with_timeout_ms`（默认 10 秒），释放上下文或停止宿主会取消所有待处理请求。浏览器宿主提供单调时钟，因此 WebAssembly 不依赖 `std::time::Instant`。

## 编译期 `window.webview` 分支

UI 声明可使用 `if window.webview { ... } else { ... }`。这是编译期条件，未选分支不进入目标能力检查与代码生成。`Svg(static_source: ...)` 的自动静态回退见[动画 SVG](Animated-SVG.md)。

## 验证范围

验证命令：`cargo test -p bern-script`、`cargo test -p bern-script --features boa`、`cargo test -p bern-adapter-cli`、`cargo test -p bern-cli --test svg_targets`、`node --test crates/bern-script/tests/browser.test.mjs`。

2026-11-21 验证：CI 中的工作区测试、`bern-script --features boa` 测试、`bern-adapter-cli::script` 测试与 `browser.test.mjs` Worker 测试全部通过，覆盖进程内 `bern.cli` 全部调用形态、`CliPolicy` 黑白名单／状态／目录过滤、动态拦截器、超时、Boa 持久上下文事件循环与浏览器 Worker 桥接。
