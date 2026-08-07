# Rern

一个基于 [iced](https://github.com/iced-rs/iced) 的运行时驱动 UI 框架。

核心构想：**控件、布局、行为三者解耦**。布局是运行期文本文件——
同一个二进制，换设备只需换布局文件；深浅色不靠任何外部配置，直接写死在
每个控件代码里，用 iced 的 `Theme` 作为标准接口。

## 三个文件体系

| 体系 | 位置 | 内容 | 决定什么 |
| --- | --- | --- | --- |
| 控件 | `src/widgets/*.rs` | 每个控件一个文件，实现 `WidgetDef` | 控件行为 + 深浅色配色 |
| 布局 | `layouts/{common,desktop}/*.ron` | 平铺的两张表：areas + widgets | 界面长什么样 |

## 布局：平铺，不嵌套

布局文件不写嵌套树，而是两张平表，用 id 表达层级：

```ron
Layout(
    name: "phone login",
    areas: [
        Area(id: "root", kind: Stack),
        Area(id: "form", kind: Column, parent: "root", padding: 16, spacing: 10),
    ],
    widgets: [
        // 页面背景就是一个普通控件：rect + z: -1
        Widget(id: "bg", kind: "rect", area: "root", z: -1, size: Fill),
        Widget(id: "greeting", kind: "title", area: "form", props: { "text": "Welcome" }),
        Widget(id: "login", kind: "button", area: "form", size: Fill, props: { "label": "Sign in" }),
    ],
)
```

- `Area` 是布局容器（Row / Column / Stack），`parent` 指向父区域——任意深度都能表达，但没有缩进地狱；
- `Widget` 的 `area` 声明它属于哪个区域，`size` 声明尺寸策略（`Auto` / `Fill` / `Fixed(px)` / `Weight(n)`）；
- 区域内排列规则：子区域在前，控件在后（Stack 内按 `z` 排序）。

## 交互 id：集中在单个 rs 文件

所有**交互控件**（`button` / `icon_button` / `text_input`）的 id 集中在应用
的一个 `ids.rs` 文件里管理，这是单一事实来源：

```rust
pub const THEME_TOGGLE: &str = "theme_toggle";
pub const ALL: &[&str] = &[THEME_TOGGLE, /* ... */];
```

- 启动时 `registry.ids().register_all(ids::ALL)` 注册；
- 框架构建布局时校验：交互控件 id 必须已注册，否则报
  `BuildError::UnregisteredId`——布局里拼错/漏改 id 会在启动时直接暴露；
- 应用代码一律用常量匹配事件，不手写字符串字面量；
- `ids.rs` 里带一个测试，反向校验布局里的交互 id 与文件声明完全一致，
  防止两侧漂移。

## 布局是积木：一个布局可以调用另一个布局

布局文件可以像积木一样组合：用 `kind: "layout"` 控件把另一个布局文件
作为控件绘制。`src` 通过 `LayoutStore` 解析——先找 `desktop` 目录，找不到
再回退到 `common` 目录。

```ron
// layouts/common/login_form.ron —— 共享积木
Layout(
    name: "login_form",
    areas: [ Area(id: "form", kind: Column, padding: 16, spacing: 10) ],
    widgets: [
        Widget(id: "greeting", kind: "title", area: "form", props: { "text": "Welcome" }),
        Widget(id: "login", kind: "button", area: "form", size: Fill, props: { "label": "Sign in" }),
    ],
)
```

```ron
// layouts/desktop/login_page.ron —— 桌面页面，把积木画进来
Layout(
    name: "login_page",
    areas: [ Area(id: "root", kind: Stack) ],
    widgets: [
        Widget(id: "bg", kind: "rect", area: "root", z: -1, size: Fill),
        Widget(id: "form", kind: "layout", area: "root", props: { "src": "login_form" }),
    ],
)
```

被嵌入的布局里所有 id 都会加上前缀（`form.greeting`、`form.login`），
所以同一个积木可以在一个页面里用多次，事件也能区分是哪个实例发出的。

## 图标包：Material Icons（默认）

rern 内嵌了 Flutter 的 Material Icons 字体与名称映射（Apache-2.0），
`icon_button` 和 `icon` 控件的图标名直接用 Flutter 里的名字：

```ron
Widget(id: "back", kind: "icon_button", area: "actions", props: { "icon": "arrow_back_rounded" })
Widget(id: "heart", kind: "icon", area: "actions", props: { "name": "favorite_rounded", "size": "16" })
```

- 8825 个图标，名字与 `Icons.xxx` 完全一致（`add`、`dark_mode`、
  `favorite_rounded`……）；
- 未知名字回退为普通文本字形（`"→"` 这类字符仍然可用）；
- 应用启动时调用一次 `rern::icons::load()` 加载字体：

```rust
fn boot() -> (App, iced::Task<AppMessage>) {
    (App::load(), rern::icons::load().map(AppMessage::FontLoaded))
}
```

## 深浅色：写进控件代码，用规范接口

深浅色**不允许**在单独的地方（主题文件、配置项）配置。每个控件在它自己的
`.rs` 文件里内置浅色和深色配色，构建时从 `BuildContext::theme`
（`&iced::Theme`）取色：

- `rect` 背景 = `theme.palette().background`
- `text` / `title` = `theme.palette().text`
- `button` = 主题的 primary 系列色（含 hover/pressed）
- `icon_button` = `theme.palette().text`

切换深浅色就是切换 `iced::Theme`（Light/Dark）这一个标准接口，所有控件
自动跟着变，没有任何外部配置参与。

框架里有一个**主题路由**（[`ThemeRouter`]）：它是运行期唯一持有当前
`iced::Theme` 的地方，应用代码通过它切换深浅色，`registry.build` 接收路由
并把主题分发给每个控件。控件只负责「拿到主题 → 用自己的调色板着色」。

### 背景的圆形切换动画（参考 nipaplay）

`rect` 背景控件在颜色因交互切换时（例如点击主题开关），会以**按下按钮的
坐标**为原点做圆形揭示动画，与 nipaplay 的深浅色切换一致：

- 切到深色：新颜色从按钮位置**向外铺开**，直到盖满控件；
- 切回浅色：**反向**——旧颜色圆从边缘**向内收缩**到按钮位置，露出新颜色；
- 非交互式的颜色变化（没有按钮坐标）直接切换，不做动画；
- 时长默认 420ms、`easeOutCubic` 缓动，可用 `duration_ms` 属性调整：

```ron
Widget(id: "bg", kind: "rect", area: "root", z: -1, size: Fill,
       props: { "duration_ms": "600" })
```

机制：可交互控件（如 `icon_button`）按下时把自身中心坐标写入共享的
[`PressOrigin`]，背景控件构建时读取并消费它。

### 通知式两阶段切换

点击主题开关时，深浅色模式**不会立即切换**，而是走通知式两阶段流程：

1. 背景从按钮位置开始圆形揭示**目标色**（此时模式还是旧的）；
2. **引擎层自动跟随**：注册表构建每个控件时自动用
   [`RevealWrapper`] 包裹（背景控件除外），wrapper 在事件驱动下把控件坐标
   注册进 [`ThemeReveal`] 协调器（不轮询）；
3. 圆形扫过控件坐标时，协调器向该控件投递**一条一次性命令**
   （`take_command`），wrapper 用**目标主题**重新构建该控件——按钮、文本、
   标题、图标全部随扩散依次切换到目标色，控件本身无需任何订阅代码；
4. 整个扩散动画完成后（所有控件必然已被覆盖），背景发布
   `LayoutMessage::ThemeRevealDone`；
5. 应用收到通知后才真正切换 `ThemeRouter`——保证动画全程可见，且切换瞬间
   每个控件下方的背景颜色都已改变。

扩散方向由背景自动判定：变暗时圆向外扩大（扫到即命令），变亮时圆向内
收缩（控件离开旧色圆即命令）。非交互式颜色变化（没有按钮坐标）直接切换。
这一层是框架的底层机制：控件零参与（引擎自动包裹），应用只负责在收到
`ThemeRevealDone` 后切换模式，互不耦合。

## 目录结构

```text
Rern/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── core/
│   │   ├── widget.rs     # WidgetDef、LayoutMessage、事件桥
│   │   ├── registry.rs   # 注册表 + 布局运行时（areas -> iced 树）
│   │   ├── layout.rs     # RON 布局解析（areas + widgets）
│   │   └── store.rs      # 布局目录加载（common + 设备）
│   ├── icons/            # Material Icons（内嵌字体 + 名称映射）
│   └── widgets/          # 每个控件一个文件
│       ├── rect.rs       # 背景/色块
│       ├── title.rs      # 标题
│       ├── icon.rs       # 图标
│       ├── icon_button.rs # 图标按钮（悬浮放大动画）
│       ├── text.rs
│       ├── text_input.rs
│       └── button.rs
└── layouts/
    ├── common/
    │   └── login_form.ron
    └── desktop/
        └── login_page.ron

assets/fonts/                # 内嵌的 MaterialIcons-Regular.otf + 许可证
```

## 用法

```rust
let store = rern::LayoutStore::load("layouts/desktop", "layouts/common")?;
let layout = store.resolve("login_page").expect("layout exists");

let registry = rern::builtin_registry();
let element = registry.build(layout, &iced::Theme::Dark, &store)?;
```

布局里的控件事件以 `(widget_id, event)` 的通用形式产生
（`rern::LayoutMessage::Event`），应用层再映射到自己的类型化消息。
