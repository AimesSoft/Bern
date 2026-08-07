# Bern

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
Widget(id: "back", kind: "icon_button", area: "actions", props: { "icon": "arrow_back" })
Widget(id: "heart", kind: "icon", area: "actions", props: { "name": "favorite", "size": "16" })
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
