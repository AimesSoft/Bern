# Bern

Bern 是以命令为中心的 Rust 应用框架。界面用受约束、可静态分析的 Rust 声明编写，每个可操作入口都绑定类型化命令；UI、应用 CLI、JSON 控制通道和 MCP 服务器驱动同一个引擎实例。Bern 把声明编译成各平台的原生 UI，不自研渲染引擎。

- 架构与目标：[设计文档](Docs/Design.md)
- 当前进度、语法参考与验证记录：[macOS SwiftUI 实现记录](Docs/Implementation/macOS-SwiftUI.md)
- 平台适配依据：[UI 自适应适配规范](Docs/Adaptation/README.md)

首个后端是 macOS SwiftUI：生成的应用只使用标准 SwiftUI 组件和系统样式，颜色来自系统，业务引擎在应用进程内运行。

## 环境

- Rust 1.99.0（仓库内 `rust-toolchain.toml` 固定版本）
- Xcode 26 或更新的正式版（提供 macOS 26 及以上 SDK），最低部署目标 macOS 14

## 构建示例应用

```sh
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml
open target/bern/macos/Showcase.app
```

示例展示侧边栏与详情、标题与文字层级、按钮、开关、滑块、输入框、搜索、列表到详情的推入导航、工具栏分段选择器，以及带页签的设置窗口，支持简体中文和英文。

应用运行时，可以用同一套命令从外部驱动它：

```sh
target/bern/macos/bin/showcase --help
target/bern/macos/bin/showcase showcase record --action Primary
target/bern/macos/bin/showcase ui inspect
target/bern/macos/bin/showcase ui select --target main_window.pages --option Library
```

`target/bern/macos/bin/showcase-mcp` 是同一目录派生的 MCP 服务器（stdio）。控制通道只在 `Bern.toml` 的目标声明 `channels = ["ipc"]` 时启动；不声明则应用不监听任何端口。

## 编写应用

一个 Bern 应用由三部分组成：

1. **协议**（`#[bern::protocol]`）：值类型、只读状态和命令 trait。
2. **引擎**：实现命令 trait 的普通 Rust 代码，不依赖任何 UI。
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

`rustc` 会检查每个绑定和命令参数的类型；以下写法都无法编译：按钮不绑定命令、任意回调、让控件写引擎状态、缺少翻译。`Bern.toml` 声明语言、主题、源码清单和目标，详见示例 [examples/showcase/Bern.toml](examples/showcase/Bern.toml)。

## 开发检查

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

测试中包含声明的编译失败用例，以及对生成的 Swift 代码做 Swift 6 类型检查（需要本机安装 Xcode）。
