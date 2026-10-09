# Bern

Rust 声明式 UI 框架。当前实现是第一个可运行切片：**WebUI 与独立 Rust 引擎**，支持 Text、Button、TextInput、Checkbox、Row 和 Column。

设计见 [架构文档](Docs/Design.md)，当前实现范围见 [WebUI 第一阶段](Docs/Implementation/WebUI-First-Step.md)。原有桌面实验代码保留在 Git 历史中。

## 启动示例

项目使用 Rust **1.99.0**（2026-10-09 核查的最新稳定版）、2024 Edition。`rust-toolchain.toml` 固定工具链并声明 WASM、rustfmt、Clippy 组件，不更改其他项目的默认 Rust。

```sh
rustup toolchain install 1.99.0 --profile minimal --component rustfmt,clippy --target wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
cargo run --locked -p bern-webui-demo -- build
cargo run --locked -p bern-webui-demo -- serve
```

打开 <http://127.0.0.1:3000>。使用 `serve --port 3010` 可以更改端口；默认连接地址相对于页面来源解析。

页面中的计数器来自独立 Rust 引擎：打开两个标签页，在其中一个增加计数，另一个会同步变化。名字与复选框属于各页面自己的会话草稿，不会串到另一个标签页。断线时引擎按钮禁用，本地草稿保留；重连重新取得完整业务快照，不自动重放未确认命令。

`target/bern/web_ui/` 是可部署的静态资源，包含前端 WASM，不包含示例业务引擎。`serve` 为本地开发同时提供静态文件和引擎 JSON/WebSocket 端点；这两者仍是独立模块。当前宿主只允许 loopback，本阶段没有远程生产部署、账户认证或通用跨域访问。

## Rust 中定义界面

```rust
use bern_core::{Command, Node};
use bern_macros::component;

const INCREMENT: Command = Command::new("counter.increment");

#[component]
fn screen() -> Node {
    ui! {
        Column(id: "root", gap: 12, padding: 24) {
            Text(id: "title", content: "计数器")
            Button(id: "add", label: "增加", command: INCREMENT)
        }
    }
}
```

完整的字段、命令目录和引擎实现见 [基础示例](examples/basic/src/lib.rs)。Button 必须指定已登记的命令；TextInput 绑定 `Field<String>` 会话字段，Checkbox 绑定 `Field<bool>` 会话字段。Text 可显示字面量或类型化字段。

宏拒绝未知控件/属性、任意事件回调、函数调用和缺少行为的按钮；Rust 检查字段类型，导出前检查字段及命令注册、重复身份和编辑作用域。首版引擎命令仅接受空参数 `{}`。

## 从 JSON 控制同一个引擎

```sh
curl http://127.0.0.1:3000/api/state
curl http://127.0.0.1:3000/api/commands
```

取得 `/api/state` 的 `engine_id` 后，向 `/api/command` POST：

```json
{
  "protocol_version": 1,
  "request_id": "manual-1",
  "engine_id": "替换为当前引擎 ID",
  "command": "counter.increment",
  "params": {}
}
```

WebUI 通过 `/api/ws` 调用同一调度入口并订阅快照。`request_id` 只关联响应，不是幂等键；不确定请求结果时不能盲目重发。

## 开发检查

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked -p bern-webui-demo -- build
```

WASM 依赖和 `wasm-bindgen` CLI 必须使用相同版本。首次构建需要下载 Rust 依赖；后续可使用 Cargo 缓存。

启动示例服务后，可用 Node.js 22.4 或以上版本检查真实 HTTP/WebSocket 通道。脚本会修改示例计数器，请针对独立的开发实例运行：

```sh
node scripts/smoke-webui.mjs http://127.0.0.1:3000
```

本阶段没有实现 Web 应用 `app` 构建、原生后端、视频、多窗口、动态列表、完整本地化、应用 CLI/MCP 或外部会话命令路由。设计文档中的这些能力仍是后续工作。
