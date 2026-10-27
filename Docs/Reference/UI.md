# 当前组件用法

更新：2026-10-28。本文说明当前可声明的接口；完整属性、必填项、子节点与动作规则见[组件属性表](Component-Properties.md)，各后端的接受范围见[平台能力表](Platforms.md)。可直接构建的工程在 `examples/hello`、`examples/showcase`、`examples/planner`、`examples/gallery`；下文片段省略所在组件、协议与状态定义。

## 声明与数据来源

在 `#[component]` 函数内用 `ui!` 声明树。静态用户可见文案使用 `tr!`，引擎状态只读，`draft` 会话字段可由标准控件更新，业务修改通过 `=>` 绑定的类型化命令执行。`id` 是控制入口的稳定身份，`label` 同时用于可访问名称。`label_hidden: true` 隐藏可见文字，仍须提供 label；当前支持它的控件包括 TextInput、Switch、Slider、Picker。

长度通常使用逻辑单位，`Length` 接受数字或受限数值表达式。`Number` 只接受静态数字，`Flag` 只接受构建期布尔值。条件、列表和状态引用仍受编译器检查；目标不支持的组件特性由能力检查拒绝，不能只根据组件名称判断平台覆盖。

## 文本、内容与格式化

界面上的每段文字，要么是 `tr!` 声明的文案，要么是声明为内容的数据，编译期区分二者：

- 协议与 UI 类型的文本字段、组件的文本参数写 `#[content]`，表示它是数据（标题、文件名、用户写的笔记）。只有它能直接显示（`Text(content: app.title)`），或作为 `tr!` 参数嵌进句子（`{title}`）。显示未标记的文本、给 `tr!` 参数写字面量，都报 B105：固定的字写进翻译。会话字段是用户输入的，算作内容。
- 引擎不拼句子。它给出数字、`CalendarDate` 与枚举，由界面用 `tr!` 或带标签的枚举写成读者的语言。

```rust
#[value]
pub enum Notice {
    #[label(tr! { "zh-Hans" => "已收藏 {name}", "en" => "{name} is a favorite" })]
    Favorite {
        #[content]
        name: String,
    },
    #[label(tr! { "zh-Hans" => "{n} 个文件", "en" => "{n, plural, one {# file} other {# files}}" })]
    Files { n: i64 },
}

#[component]
pub fn heading(#[content] title: String) -> impl View {
    ui! { Text(content: title, style: headline) }
}
```

`tr!` 与枚举标签用同一种消息格式（ICU MessageFormat 的子集），各后端按同一语义、随会话当前语言写出：

| 写法 | 写出 | 参数 |
| --- | --- | --- |
| `{name}` | 文本原样，数字按 Bern 的写法，枚举写它的标签 | `#[content]` 文本、整数、小数、带标签的枚举 |
| `{n, plural, =0 {…} one {# 项} other {# 项}}` | 按数字在该语言里的复数类别选分支，`=N` 精确匹配优先；分支里的 `#` 是这个数 | 整数、小数 |
| `{d, date, EEE, MMM d}` | 日期：`y`/`yy` 年，`M`/`MM` 月，`MMM`/`MMMM` 月名，`d`/`dd` 日，`EEE`/`EEEE` 星期；其他字符原样 | `CalendarDate` |
| `{t, duration, m:ss}` | 秒数：`h`、`m`、`s`，双写补零；写出的最大单位容纳更大的部分（`m:ss` 下 125 秒是 `2:05`，3725 秒是 `62:05`） | 整数、小数 |

- `{{`、`}}` 是字面括号；复数分支里 `}` 总是结束分支。
- 每种语言要写出自己用到的复数类别：中文、日文、韩文等只有 `other`，英文等需要 `one` 与 `other`；缺少分支报 B104。参数类型不符（复数给文本、日期给数字、`{d}` 直接写日期）也报 B104。
- 枚举标签的占位符就是该变体的字段，不写 `; 名: 值`。文本字段须 `#[content]`；枚举字段的类型须每个变体都有标签。
- 月名与星期名是框架文本，内置简体中文、繁体中文、英文、日文，其他语言用英文。
- `CalendarDate { year, month, day }` 在 `bern::prelude` 中，只表示日期，不含时刻与时区。`Text(content: date)` 会被拒绝，须用 `tr!` 写出格式。
- 这些格式与显示字段的标签由 SwiftUI、Web、GTK、Compose 与 WinUI 3 实现（目标功能 `text_formats`）：各后端的运行时按同一格式解析翻译文本，复数分支、日期与时长在当前语言下写出。

数据里的说明文字（插件的介绍、更新日志）常带行内 Markdown。`Text(markdown: true)` 把 `**粗体**`、`*斜体*`、`` `代码` ``、`~~删除线~~` 与 `[链接](地址)` 按平台的样式显示，反斜杠让下一个标记照字面显示；它只认行内标记，标题、列表、引用、代码块与表格由应用拆成块，每块用合适的组件显示。代码块用 `style: code`，即平台的等宽字体：

```rust
Column(gap: 10) {
    Text(content: plugin.summary, markdown: true)
    Text(content: plugin.install_command, style: code)
}
```

Windows 暂时去掉标记只显示文字；其他目标的实现见[属性表](Component-Properties.md)。

## 坐标场景与滚动内容

`ScaleBox(width:, height:)` 定义固定逻辑画布，在父容器给出的可用区域内等比缩放并居中，保留整个画布。父容器需要提供可用宽高。`Positioned(x:, y:, width:, height:)` 从场景左上角定位图层，坐标允许为负；位置和尺寸可绑定状态，配合 `animate` 平滑变化。`ScaleBox` 之外的内容会被裁切。

区域与画布的比例不同时，画布两侧（或上下）会留出空白。`Positioned(bleed: true)` 的图层在它到达的每条画布边上继续延伸，铺满到 `ScaleBox` 的边缘：背景、铺满窗口的画面都写成到达画布边的图层并带 `bleed`，就不会在宽窗口里留出黑边；画布按设计尺寸显示时空白为零，与不写 `bleed` 完全相同。`bleed` 可绑定布尔表达式（例如只在画面铺满时延伸），延伸的边随图层的 `animate` 动效移入移出，窗口尺寸变化时立即跟随。它只对 `ScaleBox` 场景里的图层生效；图层里面的 `Positioned` 用它自己的坐标，不再延伸。实现：Web、SwiftUI（macOS、iOS、tvOS）、Compose、GTK 与 WinUI 3。

`WindowButtons` 占出窗口自己的按钮在内容前端所占的位置：内容铺到标题栏下面时，macOS 的红绿灯与其后的间距共 84 点；网页、手机、平板、电视、Windows、Linux 的窗口在那里没有按钮，它不占位置，也不在所在行里产生间距。

```rust
ScaleBox(width: 1280, height: 720) {
    Positioned(x: 0, y: 0, width: 1280, height: 720) {
        Image(source: app.poster, width: 1280, height: 720, fit: cover)
    }
    Positioned(x: 48, y: 40, width: 1184, height: 32) {
        Marquee(duration: 12, phase: 0.25, paused: app.paused, opacity: 0.8) {
            Text(content: app.message, font_size: 24)
        }
    }
}
```

`Marquee` 的 `duration` 是一轮从右向左经过内容区的秒数，当前运行时至少使用 1 秒；`phase` 是起始周期比例，建议 0–1，默认 0。`paused` 默认 false，`opacity` 默认 1 并限制到 0–1。时钟由呈现层管理，暂停后可以继续，减少动态效果时静态呈现。它只表达循环滚动内容，不提供逐帧回调、视频时钟同步或弹幕调度引擎。

这三个组件由 `scaled_scene` 能力提供，当前支持 SwiftUI 和 Web。SwiftUI 使用一次整体缩放；Web 通过 ResizeObserver 更新缩放并在卸载时解除观察。iOS 共用生成路径，本轮未做播放器设备交互验收。

## 控件容器与尺寸

`ControlGroup(height: 40)` 将功能相近的控件放进一个共享胶囊，保留子控件各自的点击、菜单和浮层。高度默认 40；`label` 可选，Web 将其用作组的可访问名称，SwiftUI 当前保留子控件的可访问名称。SwiftUI 在 macOS / iOS 26 起使用交互式 Liquid Glass，旧系统使用系统材质胶囊；Web 使用自己的胶囊样式。

```rust
ControlGroup(height: 40) {
    IconButton(id: subtitles, icon: "captions.bubble",
        label: tr! { "zh-Hans" => "字幕", "en" => "Subtitles" }) => app.toggle_subtitles()
    IconButton(id: fullscreen, icon: "arrow.up.left.and.arrow.down.right",
        label: tr! { "zh-Hans" => "全屏", "en" => "Fullscreen" }) => app.toggle_fullscreen()
}
```

独立控件可用 `Button`、`Menu`、`Picker`、`Switch`、`SplitButton` 的 `shape: capsule` 和 `height` 声明容器及高度。网页没有胶囊形按钮：在网页上它们是标准控件，取标准高度与 8 点圆角。`Button` 与 `Switch(style: button)` 另有 `shape: chip`，表示一枚标签：网页上是描边药丸（30 高，选中或 `prominence: prominent` 时反色），其他平台是 30 点高的胶囊，写了 `height` 时取所写高度。相邻菜单使用一致的 shape / height，相关操作可共享 ControlGroup；形状本身不会改变命令语义。`IconButton(prominence: plain)` 用于需要普通图标的独立按钮，`font_size` 调整图标字号；它不代替容器高度。`prominence: prominent` 表达强调按钮。

带图标与文字的返回入口使用一个 `Button(icon:, label:)`，将两者作为同一控件进行布局与命中。容器大小属于声明，不能通过移除容器来回避内容溢出。普通控件的默认外观仍由目标平台决定。

## 引擎受控控件

| 控件 | 会话草稿来源 | 引擎快照来源 | 命令接收到的值 |
| --- | --- | --- | --- |
| Switch | `value: draft.enabled` | `checked: app.enabled` | 用户刚选择的布尔值 |
| Picker | `value: draft.appearance` | `selected: app.appearance` | 用户刚选择的枚举值 |
| Slider | `value: draft.volume` | `current: app.volume` | 用户提交的数值 |

每个控件只能选择一列作为来源，不能两者都写或都不写。引擎来源必须提供动作；动作参数中与 `checked` / `selected` / `current` 完全相同的表达式被替换为用户的新值，不替换其子表达式或其他参数。Picker 的枚举变体必须具有标签。

```rust
Slider(id: volume,
    label: tr! { "zh-Hans" => "音量", "en" => "Volume" },
    label_hidden: true, current: app.volume, min: 0, max: 1, step: 0.01
) => app.set_volume(app.volume)
```

受控 Slider 默认拖动中显示本地预览，拖动结束提交；`current:` 搭配 `continuous: true` 可在拖动中提交（适用于音量、透明度、字号等即时设置），Apple 平台合并到每秒最多 30 次，松手立即提交最后值；引擎快照不会在编辑中抢回滑块位置。`PickerOption(value:, label:)` 可显式声明选项并把实时计数插入 label。受控值由 SwiftUI 与 Web 的 `controlled_values` 支持，显式选项另由 `modal_flow_timer` 检查。

`Picker(style: chips)` 每个选项一枚标签（chip），选中的一枚突出：Apple 平台是系统玻璃胶囊按钮（设计的 30 点 chip，选中项为主题色，26 之前是材质胶囊），Compose 是 Material `FilterChip`，WinUI 3 是 `ToggleButton`，GTK 是同组的 `GtkToggleButton`，Web 是描边药丸按钮（选中项反色）。没有可见标签（`label_hidden: true`）而直接放在 `Flow` 里时，每枚标签都是这个 Flow 的子项，窄的时候逐枚换行；其他位置它们在自己的一行里换行。下面是一条工具栏：分段控件靠左，标签靠右，放不下的标签换到下一行。

```rust
Flow(gap: 10) {
    Picker(id: tab, label: tr! { "zh-Hans" => "插件", "en" => "Plugins" }, selected: app.tab, style: segmented, label_hidden: true) => app.set_tab(app.tab)
    Spacer
    Picker(id: category, label: tr! { "zh-Hans" => "分类", "en" => "Category" }, selected: app.category, style: chips, label_hidden: true) => app.set_category(app.category)
}
```

## 图片、排版与共享元素

| 声明 | 用途与边界 |
| --- | --- |
| `Image(aspect: 0.6666666667, fit: cover)` | 随容器宽度保持 2:3，避免加载前后画框跳动；比例必须为正 |
| `Image(corner:)` | 裁切图片圆角，可绑定长度；也可以写 `style!` 的具名圆角，例如 `corner: poster` |
| `Image(rendering: template)` | 按当前前景色渲染模板图片；原始彩色图片使用 original |
| `Grid(row_gap:)` | 独立设置行距；省略时沿用 gap |
| `Grid(min_width: 300, gap: 12, masonry: true)` | 瀑布流：高度不一的卡片各按自己的高度，放进目前最短的一列；GTK 与 Windows 按普通网格逐行排 |
| `Row { Text(…) Spacer Switch(…) }` | 领头的文字占去 `Spacer` 的位置，在剩下的宽度里换行，开关保持自己的宽度，如系统设置的一行；SwiftUI 与 Compose 这样排 |
| `Text(font_size:, lines:)` | 字号可绑定长度，lines 指定行数上限；动态字号要求 scaled_scene |
| `Row(align: baseline)` | 按第一行文字基线对齐 |
| `padding_x` / `padding_y` | 支持的布局容器分别设置水平、垂直内边距；见属性表 |

共享元素使用 `hero: poster, hero_key: item.id`：静态 hero 名称区分用途，动态 key 区分具体条目。两端须处于同一窗口且键一致；不要同时保留多个同键可见元素。`hero_key` 不替代 ForEach 的稳定 key。

macOS 的 Image 使用窗口上层像素覆盖层，Web 使用视口坐标的图片副本，处理跨侧栏与 ScaleBox 的坐标变化；临时画面不接受点击、不进入无障碍树，减少动态效果时不飞行。macOS / Web 已验收图片从舞台展开、缩到侧栏及恢复。iOS 和 macOS 的其他视图仍沿用匹配几何；Video 连续画面的跨分栏传递不在此次图片验收范围。

## 同窗口引导、侧栏与搜索

SwiftUI 目标可在 `style!` 中用 `Sidebar { background: color(light: "#FAFAFA", dark: "#171717") }` 和 `Detail { background: color(light: "#F2F2F2", dark: "#1E1E1E") }` 设置分栏底色，规则加 `#[target(macos, ios, tvos)]`。背景直接绘制在原生分栏上并延伸到窗口栏后方，内容仍遵循标题栏的安全区域，不必额外包裹 `Stack`。侧栏列表的系统底色会隐藏，顶栏和底栏也使用同一背景。

| 接口 | 当前语义 |
| --- | --- |
| `Modal(width:, height:)` | 窗口内居中面板，尺寸受可用窗口约束，背景遮罩；正文可滚动、页脚可单独放置。不写 `height` 时与内容一样高：正文的 `ScrollView` 取它滚动的内容的高度，超出窗口时面板停在窗口边缘、正文滚动。背景子树使用 `Stack(enabled: false)` 禁用 |
| `Flow(gap:)` | 按内容固有尺寸从左到右换行；每行的子项竖直居中，`Spacer` 换行时不占宽度，排好后占满所在行余下的宽度 |
| `Timer(interval:, enabled:) => command()` | 挂载且启用时定期提交命令；卸载、禁用时取消，间隔单位为秒。不能用来驱动逐帧绘制 |
| `Accordion(count:, selected:, collapsed_width:, height:, gap:, corner:)` | 一列展开，其余列固定折叠宽度；selected 从 0 开始，animate 驱动列宽及位置；height 可省略，此时填满父布局剩余高度，应放在有高度约束的容器中（如窗口内的 Column） |
| `Sidebar(width:)` | 固定列宽。`SidebarHeader` 固定在列表上方，`SidebarFooter` 固定在下方。`SidebarHeader(role: title)` 表示头部只用应用的图标与名称给窗口命名：标题栏本身就显示应用名称的平台（Windows 的两种前端与 Linux GTK）不显示它，其他平台照常显示 |
| `SidebarItem(badge:)` | 显示正数计数；模块页面清单的 badge 可绑定模块状态字段 |
| `Search(style: palette)` | 从工具栏入口打开命令面板；`panel_prompt`、`width`、`height` 设置提示与面板尺寸 |
| `SearchItem(id:, label:) => command()` | 面板结果，可带 icon、detail、hint 或自定义子内容；`navigation: false` 不参与方向键选择，仍可点击与 Tab 聚焦 |

搜索面板支持方向键、Enter、Esc、外部点击及关闭后的焦点恢复，当前只在 macOS 和 Web 开放。其他表内扩展由 SwiftUI 和 Web 提供；应用的布局与平台选择仍须通过目标能力检查。

## 动效与空闲状态

- `appear` / `disappear` 控制结构变化；`animate` 控制支持的长度变化，`reflow` 让相邻元素让位。动效名称来自 `style!`。
- `style!` 的 `corner { card: 22 }` 给圆角起名，`#[target(windows)] corner { card: 8 }` 给该平台另定取值；样式规则、`Image` / `Video` / `Shape` / `Accordion` 的 `corner` 与其中的 `if` 表达式都可写名字，构建时换成目标平台的数值。
- `grow(edge: bottom, bouncy)` 固定某条边展开；`appear_delay` 必须配合 appear，延迟非负，可与 stagger 相加。普通状态更新不会重播入场，卸载取消等待。
- `nudge` 响应布尔触发；`nudge_key` 让不同事件值触发提示，`nudge_scale` 指定幅度。容器的 `reduce_motion` 与系统减少动态效果偏好合并。
- `Window(idle_after:, idle_enabled:, immersive:)` 控制空闲与沉浸。idle_after 单位为秒，`window.idle` 为只读会话状态。idle_enabled 默认 true；关闭后清除闲置并停用检测，开启后重新计时。SwiftUI 和 Web 已接入此开关（能力 `idle_control`）；其他目标当前未消费该属性，`bern check` 会对它们给出警告，空闲检测在那里始终开启。

macOS 编辑文字、菜单或子窗口交互时保留控制层；Web 仅在启用状态变化时重启计时器，播放进度快照更新不会阻止空闲隐藏。UIKit 共享启停逻辑，播放器设备交互尚未验收。

## 持久化与本地预览

`bern::storage::Store::host()` 使用宿主安装的、按应用 bundle ID 隔离的小数据存储。macOS / iOS 使用 Application Support，Windows 使用 APPDATA，Linux 使用 XDG_CONFIG_HOME / ~/.config；Web app 使用同源 localStorage，WebUI 使用原生引擎宿主。key 限简单文件名；不存在的值与空值不同，读写失败返回错误。`Store::directory` 适用于指定目录的原生测试，没有宿主时的纯引擎测试不持久化。

```sh
bern run --target web_app --port 4173
```

目标名由项目 Bern.toml 定义。`--port` 只用于浏览器 app，范围 1–65535；省略时使用空闲端口。固定端口保持相同 origin，避免开发时端口改变而读到另一份 localStorage。浏览器权限、容量与清理站点数据会影响持久化。

## 电视、快捷键与起始焦点

- `Layout(size: tv)` 是电视上的布局，`Adaptive` 里最多一个，不影响其他尺寸的必需与回退规则；表达式可读 `window.tv`。tvOS 在构建时只保留电视显示的部分，所以手机与桌面布局里电视没有的能力（如 `pick:`）不会让 tvOS 构建失败。
- `Shortcut(key:, enabled:) => 动作` 是没有控件的快捷键，键的写法与控件的 `shortcut` 相同。电视上 `escape`、`space` 与方向键分别是遥控器的返回、播放/暂停与触控板方向（方向只在焦点无处可移时到达）。
- `Button` / `IconButton` 的 `default_focus: true` 让界面出现时焦点从它开始；电视上每个页面都应给出一个。
- `window.keyboard` 是否连着实体键盘：电脑上总是成立，手机与平板跟随键盘的连接与断开。只对键盘有用的内容（如快捷键设置）写在 `if window.keyboard { … }` 里，没接键盘的手机上就不出现。
- macOS 窗口统一隐藏系统焦点框，包括按钮、输入框、滑块、工具栏和弹出面板；键盘焦点、文本编辑和快捷键仍正常工作。
- `style!` 的 `text` 块可以给电视另定字号：`body: font(size: 13, tv: 16)`。
- 元素自己写的长度也可以交给样式按平台给出，元素用 `variant` 选择：`Row`、`Column` 的 `gap`、`padding`、`padding_x`、`padding_y`、`width`、`height`，`Stack` 的 `width`、`height`，`Form` 的 `gap`、`padding_x`、`padding_y`、`row_height`，`IconButton` 的 `font_size`、`height`。`width: fit` 与 `height: fit` 去掉写下的尺寸，元素取内容的大小。构建某个平台时，适用于它的规则（不带 `#[target]` 的在前，带的在后）替换元素写下的值，所以设计在各平台尺寸不同的地方不需要在界面里判断平台：

```rust
style! {
    Row.brand { gap: 5, padding_x: 12 }
    #[target(browser)]
    Row.brand { gap: 10, padding_x: 0 }
    Row.menu_box { width: 92 }
    #[target(browser)]
    Row.menu_box { width: fit }
}
```

  元素没有这项长度时报错，例如 `Button.x { gap: 4 }`；长度不小于 0，`font_size` 大于 0。
- 屏幕键盘一类的按键用文字会话动作：`=> draft.query.push_str("A")`、`=> draft.query.pop()`；搜索页想在输入前就列出结果时写 `Search(suggest: true)`。
- `Tab(display: icon)` 让页签只显示图标，例如电视标签栏里只是放大镜的搜索页；必须写 `icon`，`label` 仍是读屏读出的名称（桌面上也是悬停提示）。关键词与 `Label(display:)` 相同，默认 `title_and_icon`。它不限于电视，六个目标都生成。

```rust
Adaptive(id: shell) {
    Layout(size: compact) { phone_home(app: app) }
    Layout(size: expanded) { desktop_home(app: app) }
    Layout(size: tv) { tv_home(app: app) }
}

Stack {
    Button(id: resume, label: tr! { "zh-Hans" => "继续播放", "en" => "Resume" }, default_focus: true) => app.resume()
    if app.details_open {
        Shortcut(key: "escape") => app.close_details()
    }
}

Tabs(id: tv_tabs, selection: draft.page) {
    Tab(value: Page::Search, label: tr! { "zh-Hans" => "搜索", "en" => "Search" }, icon: "magnifyingglass", display: icon) { search(app: app) }
    Tab(value: Page::Home, label: tr! { "zh-Hans" => "首页", "en" => "Home" }) { tv_home(app: app) }
}
```

各目标的范围见[平台能力表](Platforms.md)，tvOS 的焦点、遥控与缩放场景见 [tvOS 实现记录](../Implementation/tvOS-SwiftUI.md)。

## 菜单栏与可改的快捷键

- `shortcut:` 与 `Shortcut(key:)` 可以绑定文本，快捷键跟随它：`shortcut: app.keys.open`。文本按快捷键的写法读取（`cmd+shift+o`），空文本或读不成快捷键时没有快捷键。引擎用 `bern::shortcut::parse_shortcut` 规范化用户选的键，用 `bern::shortcut::same_keys` 判断两段键是不是同一组按键。
- `ShortcutInput(id:, label:, current: app.keys.x) => app.set_key(app.keys.x)` 是改键的按钮：显示当前快捷键（没有时显示 `empty`），按下后等待新的组合键（显示 `prompt`），Esc 保持原样，单独按 Delete 清空；等待时按键不触发其他快捷键。冲突、保存与恢复默认由应用的引擎处理。
- `WindowSet` 里的 `MenuBar` 是应用的菜单：`CommandMenu(label:)` 是自己的菜单，`CommandMenu(role: file)` 等把项加进系统的同名菜单，嵌套的 `CommandMenu` 是子菜单，`Divider` 分组；`CommandItem` 有动作、`shortcut`、`enabled`（变灰）、`checked`（打勾）与 `pick`。菜单项读写的草稿属于主窗口，根组件用 `#[session]` 参数声明。
- 只有 macOS 显示菜单栏；其他目标接受声明但不显示（能力 `menu_bar` 不适用，不警告）。`window.menu_bar` 在构建时已知，macOS 为真，用来只在 Mac 上显示菜单才有的设置。

```rust
WindowSet(handle: AppWindows, primary: main) {
    Window(id: main, title: tr! { "zh-Hans" => "主窗口", "en" => "Main" }, initial: open) { page(app: app) }
    MenuBar {
        CommandMenu(role: file) {
            CommandItem(id: open, label: tr! { "zh-Hans" => "打开…", "en" => "Open…" }, shortcut: app.keys.open, pick: draft.file, files: "video") => app.open(draft.file)
        }
        CommandMenu(label: tr! { "zh-Hans" => "播放", "en" => "Playback" }) {
            CommandItem(id: play, label: tr! { "zh-Hans" => "播放 / 暂停", "en" => "Play / Pause" }, shortcut: app.keys.play, enabled: app.has_video) => app.toggle_pause()
            Divider
            CommandMenu(label: tr! { "zh-Hans" => "倍速", "en" => "Speed" }) {
                CommandItem(id: double, label: tr! { "zh-Hans" => "2×", "en" => "2×" }, checked: app.speed == Speed::Double) => app.set_speed(Speed::Double)
            }
        }
    }
}
```

各目标的范围见[平台能力表](Platforms.md)，语义与实现进度见[设计文档的快捷键与菜单栏](../Design.md#菜单栏)。

## 状态栏项（托盘）

`WindowSet` 里的 `StatusItem` 是应用在系统状态区域的一项：Windows 任务栏的通知区域、Linux 桌面的状态区与 macOS 菜单栏右侧。声明了它，关闭最后一个窗口时应用不退出，留在那里。

- `label` 是提示文字与可访问名称；`icon` 只给 macOS，菜单栏里的状态项是系统着色的单色符号。Windows 与 Linux 显示应用图标。
- 菜单第一项打开主窗口、最后一项退出，中间是声明的 `CommandItem`、`CommandMenu` 与 `Divider`，可以写在 `if`、`match` 与 `ForEach` 里。Windows 与 Linux 上单击图标打开主窗口，右键是菜单；macOS 上点按就是菜单。
- 菜单不属于任何窗口：只读引擎状态、执行命令与窗口动作，不读写草稿、不读窗口字段，菜单项不带 `pick` 与 `shortcut`。
- 手机、电视与网页没有这个位置，接受声明而不显示（能力 `status_item` 不适用）。

```rust
StatusItem(icon: "sun.max.fill", label: tr! { "zh-Hans" => "显示器调节", "en" => "Monitor Dial" }) {
    CommandItem(id: dim, label: tr! { "zh-Hans" => "调暗", "en" => "Dim" }, enabled: app.brightness > 10) => app.set_brightness(10)
    Divider
    CommandMenu(label: tr! { "zh-Hans" => "预设", "en" => "Presets" }) {
        ForEach(items: app.presets, key: id) |preset| {
            CommandItem(id: preset, label: preset.name, checked: preset.id == app.preset) => app.apply(preset.id)
        }
    }
}
```

语义与各平台的实现见[设计文档的状态栏项](../Design.md#状态栏项)。

## 其他接口

动画 SVG 是按需使用的独立组件，接口、资源声明、平台依赖与生命周期见[专项文档](../Implementation/Animated-SVG.md)。视频播放器、查询、模块、主题及命令协议的完整语义继续见[设计文档](../Design.md)与对应实现记录。

### 隐藏不可用的菜单项

`Menu(hide_disabled: true)` 根据每个 `MenuItem` 的 `enabled` 条件隐藏不可用项，条件恢复时重新显示。省略此选项时仍显示灰色的禁用项。SwiftUI、Web、Compose、GTK 和 WinUI 3 都支持。


### 输入区域

`FocusRegion` 包住一个内容视图（通常是输入框与发送按钮的 `Row`）。当其中的文本框正在编辑时，首次指针操作落在区域外，只取消编辑焦点，并消费这次按下及后续抬起，保留草稿且不提交。区域内按钮照常工作；下一次外部点击照常执行。键盘 Tab 切换不受影响。各平台后端负责事件处理，应用无需按控件 ID 或平台拦截。

```rust
FocusRegion {
    Row {
        TextInput(id: message, label: tr! { "en" => "Message" }, value: draft.text)
        Button(id: send, label: tr! { "en" => "Send" }) => app.send(draft.text)
    }
}
```

### 运行时语义调色板

`WindowSet(palette: app.palette)` 绑定一个文本字段，内容是从语义角色到浅色与深色 `#RRGGBB` 的 JSON 对象，例如 `{"accent":{"light":"#006A60","dark":"#4FDAC8"},"surface.canvas":{"light":"#F4FBF8","dark":"#101815"}}`。调色板可以在运行时更换，没写的角色保持平台默认。两种外观都要给出，前景与背景要成对可读。`accent` 这一对也给原生控件着色。SwiftUI（macOS、iOS、tvOS）、GTK 与 WinUI 3 支持 `runtime_palette`，其他目标用到这个属性时构建报错。不写调色板时，原有的 `accent:` 与 `appearance:` 照常有效。
