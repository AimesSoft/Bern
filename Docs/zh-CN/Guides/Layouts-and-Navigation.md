[English](../../Guides/Layouts-and-Navigation.md) · **简体中文**

# 布局、导航与功能契约

Bern 的界面声明全部写在 UI crate（`*-ui`）的普通 `.rs` 文件中。本指南介绍组件、窗口草稿、多尺寸自适应布局、导航容器以及编译期功能契约如何协同工作。

## 1. 组件（`#[component]`）与窗口草稿（`#[bern::session]`）

界面状态明确拆为两类：**引擎状态**（跨窗口、跨 CLI 共享，通过 `app: <命名空间>::Client` 只读访问）与**窗口局部草稿**（输入框内容、当前选中页、筛选开关、弹层状态等，每个窗口实例独立持有，用 `#[bern::session]` 声明）：

```rust
#[bern::session]
pub struct MainDraft {
    pub page: Page,
    pub query: String,
    #[initial(true)]
    pub only_open: bool,
}

#[component]
pub fn app(app: tasks::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Switch(
                id: only_open,
                label: tr! { "zh-Hans" => "只看未完成", "en" => "Only open" },
                value: draft.only_open
            )
            ForEach(items: app.items, key: id) |task| {
                if !draft.only_open || !task.done {
                    Switch(id: done, label: task.title, checked: task.done, style: checkbox)
                        => app.toggle(task.id)
                }
            }
        }
    }
}
```

核心声明规则：

- 所有可操作控件必须绑定引擎命令（`=> app.add(...)`）或草稿/窗口动作（`=> draft.only_open.set(false)`、`=> windows.settings.open()`）。
- 绑定 `value: draft.<字段>` 的控件会自动生成 UI 会话命令；直接绑定引擎状态（`checked:`、`current:`、`selected:`）的控件在操作时派发引擎命令。
- 支持 `if`、`if let`、`match` 以及按键复用身份的 `ForEach(items: ..., key: <键字段>)`。
- 全部内置控件与属性见[当前组件用法](../Reference/UI.md)与[完整组件属性表](../Reference/Component-Properties.md)。

## 2. 导航容器

Bern 将高层导航容器直接编译为各平台的标准系统导航控件：

- **`SplitView`（`Sidebar` + `Detail`）** — 桌面和平板并排展示侧边栏与详情区，手机（`compact`）自动折叠为推入式导航栈。
- **`Tabs`（`Tab` + 可选 `TabAccessory`）** — 标准标签栏（移动端/桌面支持 `bar: floating` 悬浮胶囊样式），以及可选的迷你播放器或状态附件条。
- **`Navigation`（`NavLink` + `Destination`）** — 基于 `Vec<Route>` 草稿字段的类型化推入导航，每个窗口独立保存返回栈。
- **`Toolbar`、`Menu`、`BottomSheet`、`Modal`、`Alert`** — 各平台标准工具栏、菜单、底部抽屉、模态与确认框。

## 3. 自适应布局与断点

在 `Bern.toml` 中统一声明宽度断点：

```toml
[layouts]
compact_below = 600
medium_below = 1024
expanded_below = 1440
```

在 `ui!` 中用 `Adaptive` 为各尺寸档（`compact`、`medium`、`expanded`、`large`）以及专用手表（`watch`）和电视（`tv`）形态声明对应布局：

```rust
Adaptive(id: controls) {
    Layout(size: expanded) {
        Row(gap: 8) {
            Button(id: prev, label: tr! { "zh-Hans" => "上一项", "en" => "Previous" }) => app.prev()
            Button(id: next, label: tr! { "zh-Hans" => "下一项", "en" => "Next" }) => app.next()
        }
    }
    Layout(size: compact) {
        Menu(id: actions, label: tr! { "zh-Hans" => "操作", "en" => "Actions" }) {
            MenuItem(id: prev, label: tr! { "zh-Hans" => "上一项", "en" => "Previous" }) => app.prev()
            MenuItem(id: next, label: tr! { "zh-Hans" => "下一项", "en" => "Next" }) => app.next()
        }
    }
}
```

也可以直接依据运行时或编译期窗口标记分支，如 `window.compact`、`window.expanded`、`window.tv`、`window.watch`、`window.short`、`window.keyboard`、`window.touch` 与 `window.webview`。在 `tvos`、`watchos` 和 `wearos` 目标上，不可达分支会在能力检查前由编译器自动裁剪。

## 4. 编译期功能契约（`#[bern::features]`）

页面存在多套自适应布局时，很容易在宽屏加了按钮却漏掉窄屏或手表入口。通过功能契约可以把这类遗漏直接变成编译错误：

```rust
#[bern::features(protocol = media)]
pub enum MediaFeature {
    #[commands(select_clip)]
    #[sizes(compact, expanded)]
    Clip,
}

#[component(features = MediaFeature)]
pub fn media_controls(app: media::Client) -> impl View {
    ui! {
        Adaptive(id: clip_choice) {
            Layout(size: expanded) {
                Button(id: bars, label: tr! { "zh-Hans" => "色条", "en" => "Color Bars" }, feature: Clip)
                    => app.select_clip(Clip::Bars)
            }
            Layout(size: compact) {
                Menu(id: clip_menu, label: tr! { "zh-Hans" => "换片", "en" => "Change Clip" }) {
                    MenuItem(id: bars, label: tr! { "zh-Hans" => "色条", "en" => "Color Bars" }, feature: Clip)
                        => app.select_clip(Clip::Bars)
                }
            }
        }
    }
}
```

`bern check` 会强制执行的契约诊断码：

| 错误码 | 含义 |
| --- | --- |
| `B201` | 契约中的某项功能在必需布局尺寸下缺少可达的界面入口 |
| `B202` | 契约组件内的交互入口未登记归属到任何功能 |
| `B203` | 功能入口只位于条件分支中，编译器无法证明其始终可达 |
| `B204` | 入口调用的命令不属于它所声明的功能 |
| `B401` | 手表目标（`watchos` / `wearos`）可达的 `Adaptive` 缺少 `Layout(size: watch)` |

每次构建都会在 `target/bern/<目标>/` 下生成机器可读的功能覆盖报告 `features.json`。
