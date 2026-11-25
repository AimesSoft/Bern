[English](../../Platforms/Web.md) · **简体中文**

# Web（WASM 应用与 WebUI）

Bern 将浏览器目标（`platform = "browser"`、`ui = "web"`）编译为轻量的 **DOM + CSS**，并与你的 Rust 引擎协同运行。由于浏览器目标既可以在标签页内运行引擎，也可以连接本机原生引擎，每个 Web 目标都必须显式选择 `web_mode`：

- **`web_mode = "app"`（WASM 应用）**：将 Rust 引擎编译为 WebAssembly 直接在浏览器中运行，适合静态托管和零安装网页应用。
- **`web_mode = "ui"`（WebUI）**：生成通过 WebSocket JSON 连接本机原生 `<app>-host` 进程（或通过 `web_ui` 嵌入桌面/移动原生应用）的网页界面，适合需要完整本地 I/O、文件系统访问或配套 CLI/Skill 的场景。

Web 目标可在 macOS、Linux 或 Windows 上构建。

## 环境准备

- **Rust 1.99+**（`rustup update`）。
- 安装 WebAssembly Rust 编译目标：

```bash
rustup target add wasm32-unknown-unknown
bern doctor
```

如果项目或模块引入了 `wasm-bindgen`，请在 `PATH` 中安装对应版本的 `wasm-bindgen-cli`（或通过 `BERN_WASM_BINDGEN` 指定可执行文件路径）。

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[app.icon]
default = "assets/icon.png"
web = "assets/web-icons"         # 可选：PNG、SVG 或页面图标目录

# 模式一：纯前端 WebAssembly 应用
[targets.web_app]
platform = "browser"
ui = "web"
web_mode = "app"
# javascript = true              # 可选：在 Web Worker 中运行 bern::script

# 模式二：由本机原生引擎驱动的 WebUI
[targets.web_ui]
platform = "browser"
ui = "web"
web_mode = "ui"
```

### 将 WebUI 嵌入原生应用

任意原生目标都可以把一个 `web_mode = "ui"` 目标打包进自身资源目录（`__bern_web_ui/`），让用户从局域网内的任意浏览器访问原生应用的实时界面：

```toml
[targets.macos]
platform = "macos"
ui = "swiftui"
web_ui = "web_ui"                # 引用上方定义的 [targets.web_ui]
```

在 Rust 引擎中可按需启停内嵌服务（通常做成由用户手动开启的设置项）：

```rust
use bern::web_ui::{self, Options};

// 在 8787 端口启动 WebUI 服务（可附加自定义 HTTP Routes）
web_ui::set(Some(Options { port: 8787, routes: None }));

// 读取运行状态（Stopped / Running { port } / Failed(String)）
let status = web_ui::status();

// 停止服务
web_ui::set(None);
```

## 构建、运行与调试

```bash
# 构建静态 WASM 应用或 WebUI + 原生宿主
bern build --target web_app
bern build --target web_ui

# 启动本地开发服务并在浏览器中打开
# 指定固定 --port 可在重启后保留浏览器 localStorage 与同源存储
bern run --target web_app --port 8080
bern run --target web_ui --port 8081
```

在 `bern run --target web_app` 开发运行期间，Bern 会启动本地 IPC 桥接，使配套应用 CLI 依然可以直接操控并截图当前浏览器标签页：

```bash
target/bern/web_app/bin/tasks --help
target/bern/web_app/bin/tasks window screenshot --output web.png
```

在 `web_mode = "ui"` 模式下，独立的原生宿主（`<app>-host`）监听 `127.0.0.1`，校验 HTTP `Origin`/`Host` 请求头，并通过每次启动随机生成的 URL fragment token 鉴权 WebSocket 会话，同时完整提供配套应用 CLI 与 Skill。

## 平台行为与适配要点

- **共享自绘视觉体系（`bern-surface`）**：基于 `bern-surface` 与 `bern-theme` 快照输出 `--bern-*` 主题变量与控件样式规则（与 GPUI 共享同一自绘 IR），统一呈现半透明表面、间距、排版与白色竖向胶囊滑块，同时保留语义化 HTML/CSS、原生输入法与 WAI-ARIA，并自动响应系统 `prefers-color-scheme`、`WindowSet(appearance:)` 与 `Window(idle_enabled:)`。
- **窗口与对话框**：副窗口与 `Dialog` 在视口内渲染为具备无障碍语义的虚拟对话框。
- **文件与存储**：`web_mode = "app"` 使用同源 `localStorage` 做本地持久化，`web_mode = "ui"` 直接使用原生文件存储。浏览器文件选择器返回 `blob:` URL，可直接交给 `MediaSource` 使用。
- **横屏与电视适配**：`Window(landscape: true)` 在支持的移动浏览器中调用 Screen Orientation API；电视浏览器会自动启用 `Layout(size: tv)` 与 `font(tv:)`。
- **动态 SVG 与 JavaScript**：动态 `Svg` 编译为 `BSVG` 字节码，由纯 Rust `bern-svg` 渲染。`web_mode = "app"` 开启 `javascript = true` 后会在隔离的 Web Worker 中运行脚本。
- **Web 扩展与原生视图**：可通过 `NativeView(web: "...")` 挂载自定义 DOM / Web Component 视图（详见 [原生视图](../Topics/Native-Views.md)），或使用 `web_extension` 集成第三方 JS/WASM 扩展（详见 [Web 扩展](../Topics/Web-Extensions.md)）。
- **不适用能力**：`MenuBar`、`StatusItem` 与 `Switch(login_item: true)` 会被安全视作不适用而不报错。

## 打包与签名

```bash
bern package --target web_app
bern package --target web_ui
```

打包产物位于 `target/bern/dist/`：

| 模式 | 产物 | 包含内容 |
| --- | --- | --- |
| `web_mode = "app"` | 静态 `.zip` 与 `.tar.gz` | `index.html`、编译后的 `.wasm`、JS 胶水、CSS 与资源文件，可直接部署至任意静态主机或 CDN（不含 CLI 与 Skill）。 |
| `web_mode = "ui"` | 平台 `.zip` / `.tar.gz` | 原生 `<app>-host` 可执行文件、静态 WebUI 页面、配套应用 CLI（`bin/<app>`）与 Skill（`skills/<app>/SKILL.md`）。 |
