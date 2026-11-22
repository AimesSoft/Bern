[English](../../Platforms/watchOS.md) · **简体中文**

# watchOS

Bern 将 watchOS 目标（`platform = "watchos"`）编译为独立的 Watch-Only **SwiftUI** 应用，最低支持 **watchOS 10**。watchOS 目标必须在 Mac 上构建。

## 环境准备

- **macOS 宿主机**，安装 **Rust 1.99+** 与 **Xcode 26 或更新版本**（需包含 watchOS 平台 SDK 与 Apple Watch 模拟器）。
- 安装 watchOS 模拟器与真机的 Rust 编译目标：

```bash
# Apple Silicon Mac（Intel Mac 请将模拟器目标替换为 x86_64-apple-watchos-sim）
rustup target add aarch64-apple-watchos-sim aarch64-apple-watchos
bern doctor
```

## `Bern.toml` 配置

```toml
[app]
id = "tasks"
bundle_id = "com.example.tasks"
version = "0.1.0"
name = "Tasks"

[targets.watchos]
platform = "watchos"
ui = "swiftui"
```

### 强制提供 `watch` 布局（`B401`）

由于手表屏幕极小且以数码表冠纵向滚动为主，Bern 要求 watchOS 目标中每个可达的 `Adaptive` 块都必须显式提供 `Layout(size: watch)` 分支。若缺失，执行 `bern check --target watchos` 会直接报 **`B401`** 错误。

编译期会自动裁剪所有非 `watch` 分支（`compact`、`medium`、`expanded`、`tv`），大屏专属视图与代码不会被生成进手表二进制文件。

```rust
Adaptive {
    Layout(size: watch) {
        ScrollView {
            Column(gap: 8) {
                Text(tasks.title, style: title)
                ForEach(tasks.items) |item| {
                    Button(item.title) => toggle(id: item.id)
                }
            }
        }
    }
    Layout(size: compact) { /* watchOS 编译时会被裁剪 */ }
}
```

## 构建、运行与调试

```bash
# 检查声明（含 B401 手表布局覆盖检查）
bern check --target watchos

# 为 watchOS 模拟器构建
bern build --target watchos

# 启动 Apple Watch 模拟器运行应用并打通 IPC
bern run --target watchos
```

通过配套应用 CLI 配合 `--engine watchos` 操控模拟器中的应用：

```bash
target/bern/watchos/bin/tasks --engine watchos window screenshot --output watch.png
```

常用调试环境变量：

| 环境变量 | 作用 |
| --- | --- |
| `BERN_APPEARANCE=light\|dark` | 覆盖外观模式以方便检查。 |
| `BERN_MOTION=reduce\|slow` | 强制减弱动态效果或将弹簧动画放慢 10 倍。 |
| `BERN_TRACE=<rules>` | 开启性能追踪。 |
| `BERN_SNAPSHOT_DIR=<dir>` | 自动保存窗口截图。 |

## 平台行为与适配要点

- **手表原生控件适配**：`Menu` 与 `Picker` 映射为支持表冠滚动的原生 Sheet（`BernWatchMenu` / `BernWatchPicker`）；`Table` 降级为纵向 `List`；`Form` 自动避免嵌套卡片底板以保证小屏可读性。
- **导航与弹层**：副窗口与 `Modal` 以全屏手表 Sheet 呈现；`TabView` 采用纵向翻页标签。
- **动态 SVG 与 JavaScript**：纯 Rust 渲染的动态 `Svg`（`BSVG` 字节码）与进程内 `javascript = true` 均可在 watchOS 上直接运行，无任何 WebView 依赖；静态 SVG 在编译期栅格化为 PNG。
- **不可用能力**：`WebPage`、`file_pick`、`paste:`、`MenuBar`、`StatusItem`、`ShortcutInput` 与悬停交互在 watchOS 上不可用。只要将这些桌面/手机控件放在 `Adaptive` 的非 `watch` 分支内，编译期裁剪后即可顺利通过检查。
- **原生视图**：可通过 `NativeView(watchos: "...")` 嵌入 WatchKit/SwiftUI 视图。详见 [原生视图](../Topics/Native-Views.md)。

## 打包与签名

```bash
bern package --target watchos
```

`bern package --target watchos` 会在 `target/bern/dist/` 下构建 Release 版 Watch-Only `.ipa`。安装到配对的 Apple Watch 真机或提交 App Store Connect 前，请使用 Apple 发布证书与 Watch 描述文件对该 `.ipa` 重新签名。
