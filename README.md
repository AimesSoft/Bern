# Bern

Bern 是以命令为中心的 Rust 应用框架。界面用受约束、可静态分析的 Rust 声明编写，每个可操作入口都绑定类型化命令；UI、应用 CLI、JSON 控制通道和 MCP 服务器驱动同一个引擎实例。Bern 把声明编译成各平台的原生 UI，不自研渲染引擎。

- 架构与目标：[设计文档](Docs/Design.md)
- 当前进度、语法参考与验证记录：[macOS SwiftUI 实现记录](Docs/Implementation/macOS-SwiftUI.md)、[Web 实现记录](Docs/Implementation/Web.md)
- 平台适配依据：[UI 自适应适配规范](Docs/Adaptation/README.md)、[Web 视觉规范](Docs/Adaptation/Web.md)

首个后端是 macOS SwiftUI：生成的应用只使用标准 SwiftUI 组件和系统样式，颜色来自系统，业务引擎在应用进程内运行。第二个后端是 Web：页面使用 Bern 自定的视觉规范，颜色由 Bern 主题内核生成并自动适配浅色、深色与更高对比；`app` 构建把引擎编译为 WebAssembly 在页面内运行，窗口呈现为页面内的虚拟窗口。

## 环境

- Rust 1.99.0（仓库内 `rust-toolchain.toml` 固定版本）
- Xcode 26 或更新的正式版（提供 macOS 26 及以上 SDK），最低部署目标 macOS 14
- Web 构建需要 Rust 的 `wasm32-unknown-unknown` 目标：`rustup target add wasm32-unknown-unknown`

## 构建示例应用

```sh
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml
open target/bern/macos/Showcase.app
```

示例展示侧边栏与详情、标题与文字层级、按钮、开关、滑块、输入框、经异步引擎查询筛选列表的侧边栏搜索、列表到详情的推入导航、工具栏分段选择器、视频播放（AVFoundation，命令驱动的标准控制条，示例片段作为随包资源发布在应用包内），以及带页签的设置窗口和关闭前确认，支持简体中文和英文。

应用运行时，可以用同一套命令从外部驱动它：

```sh
target/bern/macos/bin/showcase --help
target/bern/macos/bin/showcase showcase record --action Primary
target/bern/macos/bin/showcase showcase search-library --text mo
target/bern/macos/bin/showcase ui inspect
target/bern/macos/bin/showcase ui select --target main_window.pages --option Library
target/bern/macos/bin/showcase media players
target/bern/macos/bin/showcase window list
target/bern/macos/bin/showcase window minimize --window main#1
target/bern/macos/bin/showcase window close --window settings#1
```

`target/bern/macos/bin/showcase-mcp` 是同一目录派生的 MCP 服务器（stdio）。控制通道只在 `Bern.toml` 的目标声明 `channels = ["ipc"]` 时启动；不声明则应用不监听任何端口。

同一份声明的 Web 版本：

```sh
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml --target web_app
python3 -m http.server 8765 --bind 127.0.0.1 --directory target/bern/web_app
```

然后在浏览器打开 `http://127.0.0.1:8765/`。页面需要经 HTTP 提供，直接打开本地文件无法加载 WebAssembly 模块。

## 编写应用

一个 Bern 应用由三部分组成：

1. **协议**（`#[bern::protocol]`）：值类型、只读状态、命令 trait，以及可选的只读查询 trait。
2. **引擎**：实现命令与查询 trait 的普通 Rust 代码，不依赖任何 UI；查询可以稍后在任意线程作答。
3. **界面**：`#[component]` 函数，主体是一个 `ui!` 声明。

```rust
#[component]
pub fn counter(app: counter::Client) -> impl View {
    ui! {
        Form {
            Text(content: tr! { "zh-Hans" => "已计数 {n} 次", "en" => "Counted {n} times"; n: app.count })
            Button(id: add, label: tr! { "zh-Hans" => "增加", "en" => "Add" }) => app.add(1)
        }
    }
}
```

`rustc` 会检查每个绑定和命令参数的类型；以下写法都无法编译：按钮不绑定命令、任意回调、让控件写引擎状态、缺少翻译。`Bern.toml` 声明语言、主题、源码清单、随包资源和目标，详见示例 [examples/showcase/Bern.toml](examples/showcase/Bern.toml)。

## 开发检查

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

测试中包含声明的编译失败用例，以及对生成的 Swift 代码做 Swift 6 类型检查（需要本机安装 Xcode）。
