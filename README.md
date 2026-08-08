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

bern 内嵌了 Flutter 的 Material Icons 字体与名称映射（Apache-2.0），
`icon_button` 和 `icon` 控件的图标名直接用 Flutter 里的名字：

```ron
Widget(id: "back", kind: "icon_button", area: "actions", props: { "icon": "arrow_back_rounded" })
Widget(id: "heart", kind: "icon", area: "actions", props: { "name": "favorite_rounded", "size": "16" })
```

- 8825 个图标，名字与 `Icons.xxx` 完全一致（`add`、`dark_mode`、
  `favorite_rounded`……）；
- 未知名字回退为普通文本字形（`"→"` 这类字符仍然可用）；
- 应用启动时调用一次 `bern::icons::load()` 加载字体：

```rust
fn boot() -> (App, iced::Task<AppMessage>) {
    (App::load(), bern::icons::load().map(AppMessage::FontLoaded))
}
```

## 矢量形变底层（图标果冻切换）

框架的 [`core::morph`] 模块是引擎级的「图标形变」基础：用 `ttf-parser`
直接从内嵌字体提取字形轮廓（二次/三次贝塞尔全部采样成折线），按弧长均匀
重采样、归一化到 24×24 图标网格，再在两个字形之间**逐点插值**。配合带过冲
回弹的果冻缓动（[`morph::jelly`]），图标从 1 切换到 2 时是真正的矢量
扭曲形变，而不是交叉淡入。

- `icon_button` 的 `icon` prop 变化时自动触发形变（例如主题开关的
  `light_mode_rounded` ↔ `dark_mode_rounded`），`morph_duration_ms` 调时长
  （默认 420ms）；
- `morph_icon` 是独立的形变图标控件，布局里可直接使用：

```ron
Widget(id: "toggle", kind: "morph_icon", area: "actions",
       props: { "icon": "dark_mode_rounded", "size": "20" })
```

- 任意两个图标都能形变：轮廓数量不一致时（月牙 1 轮廓 → 太阳 9 轮廓），
  缺失的轮廓从对方字形质心“生长/收缩”，视觉上就是果冻鼓包/回缩；
- 字形提取有缓存，同一字符只解析一次；动画运行在控件自身状态里，引擎
  自动跟随主题揭示（`RevealWrapper`），控件无需额外订阅。

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

## 全局界面缩放（引擎级）

所有控件都能整体放大：`registry.scale()` 持有一个共享缩放因子
（默认 1.0，范围 1.0..=2.0）。`Registry::build` 会把整棵界面包进一个
缩放包装器：

- 布局用「虚拟窗口」（窗口尺寸 ÷ scale）排布内容，再整体放大 scale 倍，
  内容始终填满窗口、不会跑出屏幕；
- 绘制对整棵子树施加缩放变换，鼠标坐标按 1/scale 反向换算，点击照常命中；
- 控件代码零感知，应用设置一次、下一次构建整棵界面就按新比例渲染：

```rust
app.registry.scale().set(1.5);
```

helloworld 的视频页有一个「缩放滑块」，value 0..1 映射到 1.0..2.0。

## 横向 Tab（移植自 nipaplay 左上角导航）

`h_tab` 控件就是 nipaplay 主界面左上角那排 Tab 的移植：加粗标签、
悬浮时 1.1 倍平滑放大（200ms ease-out），选中的标签用主题强调色，底部
有一条 3px 高的胶囊指示器，切换时在标签之间滑动（300ms）。

```ron
Widget(id: "nav", kind: "h_tab", area: "topbar",
       props: { "items": "首页:tab_home,视频:tab_video,媒体库:tab_library" })
```

- `items` 是逗号分隔的 `label:key` 列表，`key` 就是该项的交互 id——
  和按钮一样写在应用的 `ids.rs` 里，构建布局时框架会校验每一项都已注册；
- 按下某个 Tab 发布 `(key, Pressed)` 事件，应用按 id 切换内容；高亮和
  胶囊移动由控件自己维护，应用无需驱动；
- **选中项文字加粗**：`selected` prop 指向当前项（应用点击后写回，和
  滑块 value 同一套路），选中标签用粗体字族渲染；
- **多页面切换**：应用收到 Tab 事件后，把页面容器（`kind: "layout"`）
  的 `src` 换成另一个布局文件即可换页——helloworld 里就是
  `hello_card` / `page_video` / `page_library` 三个独立布局在运行时切换，
  同一个二进制不重新编译；
- 配色全部来自 iced 主题（深浅色写死在控件代码里）：未选中标签
  深色 60% / 浅色 54% 透明度，选中标签和胶囊用主题强调色；
- 可调属性：`font_size`、`hover_scale`、`duration_ms`（悬浮）、
  `indicator_ms`（胶囊滑动）、`indicator_height`、`indicator_radius`、
  `item_padding`、`selected`。

> iced 0.14 的字体库默认没有粗体字面，`Weight::Bold` 会被忽略。框架的
> `fonts` 模块启动时加载系统粗体中文字体（macOS 用 Hiragino Sans GB），
> 应用在 boot 里调用一次 `bern::fonts::load_bold()` 即可让加粗生效；
> 找不到粗体字体会优雅回退为常规字重。

## 滑块与进度条（胶囊轨道 + 果冻滑块）

`slider`（可拖动）和 `progress`（纯展示）是**视频播放进度条**的移植，
照抄 nipaplay 的 `VideoProgressBar`：

- 4px 胶囊轨道（文字色低透明度），已播放部分用文字色胶囊段——深色下
  是白色（与 nipaplay 完全一致），浅色下自动变深，任何主题都可见；
- **28 × 16 胶囊滑块**（圆角 = 高的一半，跟随主题文字色）+ 两道柔和阴影；
- 悬浮时滑块放大 8%（160ms easeOutCubic，逐帧动画）；
- 按下时滑块被**弹簧压扁**（变窄变长，刚度 620 / 阻尼 22），松开后换
  低阻尼弹簧（刚度 360 / 阻尼 7.2）**过冲回弹并振荡落定**——这就是
  nipaplay 的果冻动画，参数原样照抄。

```ron
// 可拖动滑块：发布 (id, Changed(value))，value ∈ 0..=1
Widget(id: "seek", kind: "slider", area: "root", props: { "value": "0.35" })

// 纯展示进度条：不响应交互，按 value 显示
Widget(id: "seek_progress", kind: "progress", area: "root", props: { "value": "0.35" })
```

- `value` 属性是唯一数据源：拖动/点击后应用把新值写回布局，下一次构建
  即按新值渲染（helloworld 里 `set_seek` 更新滑块）；
- 滑块是交互控件，id 照旧写在 `ids.rs`；内嵌在页面布局里时事件 id 会带
  嵌入前缀（如 `page.seek`），应用用 `{容器id}.{控件id}` 匹配；
- 可调属性：`value`、`size`（宽度策略）。

## 下拉菜单（移植自 nipaplay BlurDropdown）

`dropdown` 控件与 nipaplay 的模糊下拉菜单外观一致：

```ron
Widget(id: "lib_sort", kind: "dropdown", area: "root",
       props: { "items": "名称:sort_name,大小:sort_size,时间:sort_time",
                "selected": "sort_name" })
```

- 40px 圆角触发器：边框在打开时变强调色，右侧箭头旋转 180°；
- 点击弹出下方菜单：200ms 淡入 + 0.95→1 缩放，圆角 6 面板 + 柔和阴影，
  菜单项带分隔线、选中底色、悬浮高亮；
- 点击菜单项发布 `(item_key, Pressed)` 并关闭；点击菜单外（遮罩）关闭；
- `items` 的键是交互 id（进 ids.rs），`selected` 由应用点击后写回——
  和 Tab / 滑块同一套路；
- 菜单浮层通过 iced overlay 实现，引擎的缩放包装器（ScaleWrapper）与
  主题揭示包装器都会自动转发浮层，缩放状态下菜单位置/尺寸同样按比例。

## 圆角矩形按钮（移植自 nipaplay 大屏可聚焦动作）

`round_button` 是 nipaplay 大屏操作按钮的移植：**固定**的圆角 8 表面，
悬浮时表面不变、只有内容放大并出现强调色描边：

```ron
Widget(id: "lib_sync", kind: "round_button", area: "root",
       props: { "icon": "sync_rounded", "label": "同步" })
```

- 浅色 82% / 深色 10% 的白色填充，圆角 8，默认带 1px 文字色描边
  （一眼可辨「有容器」）；
- 悬浮时内容放大 1.035（140ms easeOutCubic）+ 描边换 2px 强调色；
- 图标（Material，带果冻形变）+ 粗体标签，内容色浅色 black87 / 深色白；
- 可调属性：`label`、`icon`、`icon_size`、`font_size`、`scale`、
  `duration_ms`；
  按下发布 `(id, Pressed)` 并记录主题揭示原点。
- 悬浮时图标和文字变成主题强调色（移开恢复），配合内容放大有明确的
  交互反馈。

## 无容器图标+文本按钮（action_button）

`action_button` 和 `icon_button` 同一个悬浮缩放核心，但没有容器——
只有图标和文字，悬浮时整体放大：

```ron
Widget(id: "lib_sort_btn", kind: "action_button", area: "root",
       props: { "icon": "sort_by_alpha_rounded", "label": "排序" })
```

- 21px 图标 + 8px 间距 + 15px 粗体标签（nipaplay 动作按钮布局）；
- 无背景、无边框，悬浮放大默认 1.08（140ms easeOutCubic），
  `scale` / `duration_ms` 可调；
- 图标 Material（带果冻形变），颜色跟随文字色；按下发布 `(id, Pressed)`。
- 悬浮时图标和文字变成主题强调色（和 `icon_button` 一致）。

## 输入框（移植自 nipaplay 媒体库搜索框）

`text_input` 直接照抄 nipaplay 媒体库的搜索框外观：

```ron
Widget(id: "lib_search", kind: "text_input", area: "root",
       props: { "placeholder": "搜索媒体库", "value": "" })
```

- 提示文字（`placeholder`）由布局文件传入，默认显示；
- 左侧 Material `search_rounded` 图标，正文用粗体；
- 浅色 82% / 深色 9% 的白色填充，圆角 8 边框（文字色 10%），聚焦时
  强调色 2px 边框；
- 输入发布 `(id, TextChanged(text))`，应用把新词写回布局 `value`——
  和滑块/下拉同一套路（布局是唯一数据源）。

## 多窗口分区背景（split_pane）

`split_pane` 把页面像桌面多窗口工作区一样切成多个区块，区块之间使用
主题自适应的 1px 细线分隔。`horizontal` 表示从左到右排列，`vertical`
表示从上到下排列：

```ron
Widget(id: "workspace", kind: "split_pane", area: "root", size: Fill,
       props: {
           "direction": "horizontal",
           "panes": "navigation,editor,inspector",
           "weights": "1,4,2",
           "divider_width": "1",
           "divider_inset": "16",
       })
```

- `panes` 是逗号分隔的布局名，由 `LayoutStore` 解析并分别嵌入每个区块；
- `weights` 控制区块比例，缺省时等分；嵌入控件的事件 id 会自动带上
  `workspace.pane0`、`workspace.pane1` 等前缀；
- 不传 `panes` 时，用 `sections: "3"` 创建纯分区背景，内容可由页面自己的
  区域覆盖上去；
- 某个 pane 对应的布局里可以再次放置 `split_pane`，从而组合左右和上下分区；
- `divider_inset` 控制分隔线两端留白（默认 12px）：竖线留出上下边距，
  横线留出左右边距；
- 背景复用 `rect` 的深浅色圆形揭示动画。当前边界为静态分隔线，不支持拖拽。

## 目录结构

```text
Bern/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── core/
│   │   ├── widget.rs     # WidgetDef、LayoutMessage、事件桥
│   │   ├── registry.rs   # 注册表 + 布局运行时（areas -> iced 树）
│   │   ├── layout.rs     # RON 布局解析（areas + widgets）
│   │   ├── morph.rs      # 矢量形变底层（字形提取/重采样/果冻插值）
│   │   └── store.rs      # 布局目录加载（common + 设备）
│   ├── icons/            # Material Icons（内嵌字体 + 名称映射）
│   └── widgets/          # 每个控件一个文件
│       ├── h_tab.rs      # 横向 Tab（悬浮放大 + 底部胶囊指示器）
│       ├── rect.rs       # 背景/色块
│       ├── title.rs      # 标题
│       ├── icon.rs       # 图标
│       ├── morph_icon.rs # 形变图标（矢量果冻切换）
│       ├── icon_button.rs # 图标按钮（悬浮放大 + 图标形变）
│       ├── split_pane.rs # 左右/上下多窗口分区 + 细线边界
│       ├── slider.rs     # 滑块 + 进度条（胶囊轨道 + 果冻滑块）
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
let store = bern::LayoutStore::load("layouts/desktop", "layouts/common")?;
let layout = store.resolve("login_page").expect("layout exists");

let registry = bern::builtin_registry();
let element = registry.build(layout, &iced::Theme::Dark, &store)?;
```

布局里的控件事件以 `(widget_id, event)` 的通用形式产生
（`bern::LayoutMessage::Event`），应用层再映射到自己的类型化消息。
