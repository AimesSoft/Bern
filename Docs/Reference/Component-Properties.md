# 内置组件属性表

更新：2026-10-20。本表从 [bern-ir::spec](../../crates/bern-ir/src/spec.rs) 的当前公共定义导出，共 70 个内置组件。**粗体属性必填**，其余可选；这些是声明规则，目标实现范围见[平台能力表](Platforms.md)，具体使用见[组件用法](UI.md)。

## 属性类型

| 类型 | 声明含义 |
| --- | --- |
| `Id` | 稳定标识符或字符串 |
| `DisplayText` | tr! 文案或可显示的状态表达式 |
| `LocalizedText` | tr! 静态本地化文案 |
| `Icon` | 图标名称字符串（SF Symbols 命名）。Apple 目标可用任何 SF Symbol；其他目标把名字映射到平台自己的图标，至少都画 Bern 图标集（`bern_target_api::icon_set::NAMES`）中的名字，用到该目标没有的名字时检查报错 |
| `ImageResource` | 图片资源；Image 也支持引擎图片状态 |
| `SvgResource` | 已声明的随包 SVG 资源 |
| `Number` | 构建期数字 |
| `Length` | 逻辑长度数字或受限数值表达式 |
| `NumberValue` | 只读数值表达式 |
| `Bool` | 布尔表达式 |
| `Flag` | 构建期 true / false |
| `Shortcut` | 快捷键字符串 |
| `FileKinds` | 文件种类字符串 |
| `FileField` | 可写 MediaSource 会话字段 |
| `PositionField` | 可写滚动位置会话字段 |
| `TextField` | 可写文本会话字段 |
| `BoolField` | 可写布尔会话字段 |
| `NumberField` | 可写浮点会话字段 |
| `PickerField` | 可写、变体带标签的枚举会话字段 |
| `SelectionField` | 可写导航选择枚举字段 |
| `SelectionValue` | 与导航选择字段匹配的枚举值 |
| `Items` | 记录列表绑定 |
| `KeyField` | 列表记录的稳定键字段名 |
| `PathField` | 可写路由列表会话字段 |
| `Route` | 类型化路由值 |
| `WindowId` | 窗口种类标识符 |
| `TypeName` | 生成的窗口句柄类型名 |
| `SlotName` | 模块插槽名 |
| `StyleVariant` | style! 样式变体名 |
| `Transition` | 过渡声明或名称 |
| `MotionToken` | 动效 token 名 |
| `HeroName` | 共享元素静态名称 |
| `KeyValue` | 只读文本、数值或枚举身份值 |
| `ColorRole` | 语义颜色角色 |
| `NumberList` | 只读数值列表 |
| `PatternName` | style! 图案名称 |
| `MediaSource` | 媒体来源绑定，可选或非可选 |
| `Player` | 应用播放器目录中的播放器 |
| `Query` | 类型化查询调用 |
| `Feature` | 所属功能契约中的入口 |

固定枚举直接列出允许的关键字。表内没有逐一重复所有跨属性约束：Switch / Picker / Slider 必须二选一指定草稿来源或受控来源，受控来源还必须提供动作；`appear_delay` 必须配合 `appear`；具体错误由编译器检查。资源、语义颜色、本地化、功能覆盖与类型仍会进一步校验。

## 组件

| 组件 | 属性（粗体必填） | 子内容、动作与位置 |
| --- | --- | --- |
| `WindowSet` | **`handle: TypeName`**<br>**`primary: WindowId`**<br>`accent: KeyValue`<br>`appearance: KeyValue` | 仅含 ["Window"]；无动作；仅根节点 |
| `Window` | **`id: WindowId`**<br>**`title: LocalizedText`**<br>`initial: open / closed`<br>`presentation: auto / virtual / floating / native_required`<br>`role: standard / settings / dialog`<br>`width: Number`<br>`height: Number`<br>`min_width: Number`<br>`min_height: Number`<br>`confirm_close: Bool`<br>`close_message: LocalizedText`<br>`chrome: standard / unified`<br>`immersive: Bool`<br>`landscape: Bool`<br>`idle_after: Number`<br>`idle_enabled: Bool` | 恰好一个子节点；动作可选；直接位于 ["WindowSet"] |
| `Column` | `variant: StyleVariant`<br>`gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`width: Length`<br>`height: Length`<br>`align: leading / center / trailing` | 可含子内容；无动作 |
| `FocusRegion` | 无 | 一个内容视图；文本编辑时首次区域外指针操作只失焦并消费整次点击，保留草稿；区域内控件和下一次点击正常 |
| `Row` | `variant: StyleVariant`<br>`gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`width: Length`<br>`height: Length`<br>`align: top / center / bottom / baseline` | 可含子内容；无动作 |
| `Accordion` | **`count: NumberValue`**<br>**`selected: NumberValue`**<br>**`collapsed_width: Length`**<br>`height: Length`<br>`gap: Length`<br>`corner: Length` | 可含子内容；无动作；省略 height 时使用剩余可用高度 |
| `Grid` | **`min_width: Length`**<br>`gap: Length`<br>`row_gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`cell_ratio: Number`<br>`cell_extra: Length` | 可含子内容；无动作 |
| `Stack` | `variant: StyleVariant`<br>`enabled: Bool`<br>`align: top_leading / top / top_trailing / leading / center / trailing / bottom_leading / bottom / bottom_trailing`<br>`width: Length`<br>`height: Length`<br>`under_bars: Flag` | 可含子内容；无动作 |
| `Marquee` | **`duration: Length`**<br>`phase: Number`<br>`paused: Bool`<br>`opacity: NumberValue` | 可含子内容；无动作 |
| `ScaleBox` | **`width: Length`**<br>**`height: Length`** | 可含子内容；无动作 |
| `Positioned` | **`x: Length`**<br>**`y: Length`**<br>**`width: Length`**<br>**`height: Length`**<br>`bleed: Bool` | 可含子内容；无动作；`bleed` 让 `ScaleBox` 场景里的图层在到达的画布边上延伸到盒子边缘 |
| `Modal` | `width: Length`<br>`height: Length`<br>`cover: compact` | 可含子内容；无动作 |
| `Flow` | `gap: Length` | 可含子内容；无动作；每行的子项竖直居中，`Spacer` 占满所在行余下的宽度 |
| `Timer` | **`interval: Number`**<br>`enabled: Bool` | 不含子节点；必须绑定动作 |
| `Shortcut` | **`key: Shortcut`**<br>`enabled: Bool` | 不含子节点；必须绑定动作 |
| `PickerOption` | **`value: KeyValue`**<br>**`label: DisplayText`** | 不含子节点；无动作；直接位于 ["Picker"] |
| `Rail` | `axis: horizontal / vertical`<br>`anchor: start / center / end`<br>`overhang: Length` | 可含子内容；无动作 |
| `Spacer` | 无专有属性 | 不含子节点；无动作 |
| `WindowButtons` | 无专有属性 | 不含子节点；无动作；macOS 上占出红绿灯的位置，其他平台不占位置 |
| `Divider` | 无专有属性 | 不含子节点；无动作 |
| `ScrollView` | `axis: vertical / horizontal`<br>`snap: start / center`<br>`position: PositionField`<br>`arrival: nudge`<br>`indicators: automatic / hidden`<br>`open_at: Number`<br>`open_center: Number` | 可含子内容；无动作 |
| `SplitView` | 无专有属性 | 仅含 ["Sidebar", "Detail"]；无动作 |
| `Sidebar` | **`selection: SelectionField`**<br>`width: Length`<br>`id: Id`<br>`fallback: SelectionValue` | 仅含 ["Search", "Section", "SidebarItem", "Pages", "SidebarHeader", "SidebarFooter"]；无动作；直接位于 ["SplitView"] |
| `SidebarHeader` | `role: content / title` | 可含子内容；无动作；直接位于 ["Sidebar"] |
| `SidebarFooter` | 无专有属性 | 可含子内容；无动作；直接位于 ["Sidebar"] |
| `Detail` | `titled: Bool` | 可含子内容；无动作；直接位于 ["SplitView"] |
| `SidebarItem` | **`value: SelectionValue`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`badge: NumberValue`<br>`variant: StyleVariant` | 不含子节点；无动作；直接位于 ["Sidebar", "Section"] |
| `Section` | `title: DisplayText`<br>`footer: DisplayText`<br>`style: grouped / plain`<br>`padding_x: Length`<br>`padding_y: Length` | 可含子内容；无动作 |
| `Slot` | **`id: SlotName`** | 不含子节点；无动作 |
| `Pages` | **`slot: SlotName`** | 不含子节点；无动作；直接位于 ["Sidebar", "Section", "Tabs"]。在 `Tabs` 里每个页面展开成一个 `Tab`，内容是该页面的组件 |
| `PageContent` | **`slot: SlotName`**<br>**`selected: SelectionField`** | 不含子节点；无动作 |
| `Form` | `title: DisplayText`<br>`variant: StyleVariant`<br>`gap: Length`<br>`row_height: Length`<br>`padding_x: Length`<br>`padding_y: Length` | 可含子内容；无动作 |
| `Navigation` | **`path: PathField`**<br>`id: Id` | 可含子内容；无动作 |
| `Destination` | 无专有属性 | 可含子内容；无动作；直接位于 ["Navigation"] |
| `NavLink` | **`id: Id`**<br>**`route: Route`** | 可含子内容；无动作 |
| `ControlGroup` | `label: DisplayText`<br>`height: Length` | 可含子内容；无动作 |
| `Tabs` | **`selection: SelectionField`**<br>`id: Id`<br>`fallback: SelectionValue` | 仅含 ["Tab", "Pages", "TabAccessory"]；无动作。选中的标签取应用强调色 |
| `TabAccessory` | `shown: Bool` | 可含子内容；无动作；直接位于 ["Tabs"]，最多一个，不放在 `if` 或 `ForEach` 里。各页之上常驻的一条，例如正在播放的小播放器；`shown` 为假时收起 |
| `Tab` | **`value: SelectionValue`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`display: title_and_icon / icon` | 可含子内容；无动作；直接位于 ["Tabs"]。`display: icon` 只显示图标（必须写 `icon`），`label` 仍是可访问名称 |
| `SplitButton` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: BoolField`<br>`checked: Bool`<br>`icon: Icon`<br>`shape: capsule`<br>`height: Length`<br>`more_label: DisplayText`<br>`shortcut: Shortcut` | 可含子内容；`value` 与 `checked` 二选一，`checked` 须有动作，切换时执行 |
| `PopoverButton` | `panel_appearance: inherit / window / surface`<br>**`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`presented: BoolField`<br>`open: Bool`<br>`shortcut: Shortcut` | 可含子内容；`open` 必须绑定动作，与 `presented` 互斥；未使用 `open` 时无动作 |
| `Inspector` | **`presented: BoolField`**<br>`width: Number` | 可含子内容；无动作 |
| `Toast` | **`content: DisplayText`**<br>`icon: Icon`<br>`key: KeyValue`<br>`duration: Number` | 不含子节点；无动作 |
| `Pinch` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: NumberField`**<br>**`min: Number`**<br>**`max: Number`**<br>`feature: Feature` | 恰好一个子节点；动作可选 |
| `Swipe` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: NumberField`**<br>**`min: Number`**<br>**`max: Number`**<br>`axis: vertical / horizontal`<br>`distance: Number`<br>`feature: Feature` | 恰好一个子节点；动作可选 |
| `Drop` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: FileField`**<br>`files: FileKinds`<br>`over: BoolField`<br>`opens: Flag`<br>`feature: Feature` | 恰好一个子节点；必须绑定动作 |
| `Toolbar` | 无专有属性 | 仅含 ["Button", "IconButton", "Picker", "Menu", "Search", "Switch", "SplitButton", "PopoverButton", "Divider", "Spacer", "Text"]；无动作 |
| `Adaptive` | `id: Id` | 仅含 ["Layout"]；无动作 |
| `Layout` | **`size: compact / medium / expanded / large / tv`** | 可含子内容；无动作；直接位于 ["Adaptive"] |
| `Menu` | **`id: Id`**<br>**`label: DisplayText`**<br>`hide_disabled: Flag`<br>`icon: Icon`<br>`style: button / icon`<br>`shape: capsule`<br>`height: Length` | 仅含 ["MenuItem", "Divider"]；无动作 |
| `MenuItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`role: normal / destructive`<br>`enabled: Bool`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`files: FileKinds` | 不含子节点；必须绑定动作；直接位于 ["Menu"] |
| `LanguagePicker` | **`id: Id`**<br>**`label: DisplayText`** | 不含子节点；无动作 |
| `List` | `items: Items`<br>`key: KeyField` | 可含子内容；无动作 |
| `ForEach` | **`items: Items`**<br>**`key: KeyField`** | 可含子内容；无动作 |
| `PreviewList` | **`items: Items`**<br>**`key: KeyField`**<br>**`item_height: Number`**<br>`gap: Number`<br>`more_height: Number` | 按可用高度创建能完整放下的前几项；有剩余时为 `+N` 留位，默认高度 22；不滚动，也不创建隐藏项。使用 `\|item\|` 行模板 |
| `Text` | **`content: DisplayText`**<br>`style: large_title / title / title2 / title3 / headline / subheadline / body / callout / footnote / caption / caption2 / control`<br>`emphasis: primary / secondary / tertiary`<br>`mark: ColorRole`<br>`lines: Number`<br>`font_size: Length` | 不含子节点；无动作 |
| `Canvas` | `width: Length`<br>`height: Length`<br>`label: DisplayText` | 仅含 ["Shape", "Series", "Pattern"]；无动作 |
| `Shape` | **`kind: rect / rounded / capsule / ellipse`**<br>`x: Number`<br>`y: Number`<br>`width: Number`<br>`height: Number`<br>`corner: Number`<br>`fill: ColorRole`<br>`fill_from: ImageResource`<br>`fade: up / down / left / right`<br>`stroke: ColorRole`<br>`stroke_width: Number`<br>`dash: Number`<br>`dash_gap: Number`<br>`opacity: Number` | 不含子节点；无动作；直接位于 ["Canvas"] |
| `Series` | **`values: NumberList`**<br>`kind: area / line`<br>`max: NumberValue`<br>`fill: ColorRole`<br>`stroke: ColorRole`<br>`stroke_width: Number`<br>`opacity: Number` | 不含子节点；无动作；直接位于 ["Canvas"] |
| `Pattern` | **`name: PatternName`**<br>`opacity: Number` | 不含子节点；无动作；直接位于 ["Canvas"] |
| `Label` | **`title: DisplayText`**<br>**`icon: Icon`**<br>`style: large_title / title / title2 / title3 / headline / subheadline / body / callout / footnote / caption / caption2 / control`<br>`font_size: Number`<br>`display: title_and_icon / icon` | 不含子节点；无动作 |
| `Svg` | **`source: SvgResource`**<br>**`width: Length`**<br>**`height: Length`**<br>`label: DisplayText`<br>`fit: contain / cover / fill`<br>`playing: Bool`<br>`speed: NumberValue`<br>`restart: NumberValue`<br>`interactive: Flag` | 不含子节点；无动作 |
| `Image` | **`source: ImageResource`**<br>`label: DisplayText`<br>`width: Length`<br>`height: Length`<br>`fit: contain / cover / fill`<br>`corner: Length`<br>`rendering: original / template`<br>`aspect: Number` | 不含子节点；无动作 |
| `Progress` | `value: NumberValue`<br>`total: Number`<br>`label: DisplayText` | 不含子节点；无动作 |
| `Button` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`shape: capsule / chip`<br>`height: Length`<br>`width: fill`<br>`role: normal / destructive / cancel`<br>`prominence: standard / prominent / plain`<br>`enabled: Bool`<br>`variant: StyleVariant`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`files: FileKinds`<br>`default_focus: Flag`<br>`clears: TextField` | 可含子内容；必须绑定动作 |
| `IconButton` | **`id: Id`**<br>**`icon: Icon`**<br>`font_size: Number`<br>`height: Length`<br>**`label: DisplayText`**<br>`prominence: standard / prominent / plain`<br>`enabled: Bool`<br>`variant: StyleVariant`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`files: FileKinds`<br>`default_focus: Flag` | 不含子节点；必须绑定动作 |
| `TextInput` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>**`value: TextField`**<br>`prompt: DisplayText`<br>`kind: plain / secure / multiline`<br>`submit: clear`<br>`clear_key: NumberValue`<br>`feature: Feature` | 不含子节点；动作可选；`submit: clear` 在动作后清空草稿、交出焦点；SwiftUI 的 `clear_key` 在数值变化时清空，供异步成功回执使用 |
| `Search` | **`id: Id`**<br>**`value: TextField`**<br>`prompt: DisplayText`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`style: automatic / palette`<br>`panel_prompt: DisplayText`<br>`width: Number`<br>`height: Number`<br>`font_size: Number`<br>`suggest: Flag` | 可含子内容；动作可选 |
| `SearchItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`detail: DisplayText`<br>`hint: DisplayText`<br>`navigation: Bool`<br>`pick: FileField`<br>`files: FileKinds`<br>`feature: Feature` | 可含子内容；必须绑定动作 |
| `Switch` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: BoolField`<br>`checked: Bool`<br>`label_hidden: Flag`<br>`shape: capsule / chip`<br>`height: Length`<br>`style: switch / checkbox / button`<br>`icon: Icon`<br>`shortcut: Shortcut` | 不含子节点；动作可选 |
| `Slider` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: NumberField`<br>`current: NumberValue`<br>`continuous: Flag`（仅 `current`）<br>**`min: Number`**<br>**`max: Number`**<br>`step: Number`<br>`min_icon: Icon`<br>`max_icon: Icon`<br>`feature: Feature` | 不含子节点；动作可选 |
| `Video` | **`id: Id`**<br>`source: MediaSource`<br>`player: Player`<br>`label: DisplayText`<br>`controls: standard / none`<br>`fit: contain / cover / fill`<br>`sizing: standard / fill`<br>`corner: Length` | 不含子节点；无动作。`controls: none` 的视频只显示画面，可以作为 `Button` 的内容。`sizing: fill` 让播放表面占满有明确大小的父容器，`fit` 只控制视频内容如何放进表面；内核弹幕可以使用黑边。默认 `standard` 保持原有布局。应用登记了媒体内核时由内核画进视图给它的表面，见[设计文档](../Design.md#应用自带的媒体内核) |
| `Query` | **`id: Id`**<br>**`query: Query`**<br>`loading: DisplayText`<br>`empty: DisplayText`<br>`failed: DisplayText`<br>`icon: Icon` | 可含子内容；无动作 |
| `Picker` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: PickerField`<br>`selected: KeyValue`<br>`label_hidden: Flag`<br>`shape: capsule`<br>`height: Length`<br>`style: automatic / menu / segmented / radio / inline / chips` | 仅含 ["PickerOption"]；动作可选；`chips` 不接受 `shape` / `height` |

`Grid` 给出 `cell_ratio` 时每行一样高：列宽 ÷ `cell_ratio` + `cell_extra`（如海报 2 : 3 加两行字的高度：`cell_ratio: 0.6667, cell_extra: 54`），各项从格子顶部排，内容更高时伸进下方的行间距，与按最高一项定行高的默认方式不同。

`Search` 放在 `Toolbar` 或 `Sidebar` 里由它们接管；放在页面里是平台自己的搜索框（iOS 的 `UISearchTextField`、macOS 的 `NSSearchField`、网页的 `input[type=search]`），子内容是在框获得焦点或有文字时列在框下的结果，`width` / `height` / `font_size` 是框本身的尺寸与字号（工具栏里的搜索面板则是面板尺寸）。

`Rail(overhang:)` 让刻度在轨道两端各伸出这么多点：比例 f 的子项站在 f × (长度 + 2 × overhang) − overhang，如 CSS 里负外边距的时间轴。`ScrollView(open_at:)` 是打开时的位置：内容中比例为 f 的点对齐到视图中同一比例处（0 起点，0.5 正中对正中，1 末尾）。`ScrollView(open_center:)` 也是打开时的位置：内容中比例为 f 的点停在视图正中；靠近两端放不到正中时滚到尽头为止，这一点仍在视图里。它可以绑定表达式，只在打开时读一次，之后值变了也不跟着滚，例如时间轴打开时让“现在”尽量居中。

`Form` 的 `gap` 是分组之间的距离，`row_height` 是行的最小高度，`padding_x` / `padding_y` 是所有分组行的内边距（`Section` 自己的同名属性优先）；表单以 `Section(style: plain)` 开头时从页面顶部开始，这一组的行没有底色、边距和分隔线，作为随表单滚动的标题。表单给分组的行内边距不作用于 plain 分组，它只用自己的 `padding_x` / `padding_y`。打头的 plain 分组也可以来自插槽、组件或条件：表单在布局时看第一项是不是 plain 分组，条件变了跟着变，所以 `if window.compact { Section(style: plain) { … } }` 只在窄窗口里让表单贴顶。`style!` 里 `Form { background: 角色 }` 是行的底色，表单本身透出页面背景。

`Row` 的各项放得下时取各自的宽度，剩余空间由 `Spacer` 和撑满的项平分；放不下时其余项按各自宽度的比例收缩，不小于各自的最小宽度。`Spacer` 没有最小长度，放不下时先收到零，各目标相同。给定 `width` / `height` 而内容较小时，`Column` 从顶部开始、横向按 `align` 摆放，`Row` 从前缘开始、纵向按 `align` 摆放。`Column` 不写 `align` 时与 `leading` 相同：按钮等控件、内层的 `Column` 与 `Stack` 保持自己的宽度，输入框、分隔线、`Grid`、`List` 等横向填满的内容横跨整列，要让按钮横跨整列写 `Button(width: fill)`。直接写在窗口里、没写宽度的 `Column` 与窗口内容区同宽，从顶部开始、横向按 `align` 摆放；没写尺寸的 `Stack` 铺满内容区；比内容区小的其他正文从左上角排起。

## 通用动效与轨道属性

以下属性只对 `ComponentSpec::moves()` 为真的布局元素开放，不应加到 Window、导航结构、插槽等不接受动效的节点上。各后端可能另有 capability 限制。

| 属性 | 类型 |
| --- | --- |
| `appear` | `Transition` |
| `appear_delay` | `Number` |
| `disappear` | `Transition` |
| `reflow` | `MotionToken` |
| `hero` | `HeroName` |
| `hero_key` | `KeyValue` |
| `reveal` | `hover` |
| `animate` | `MotionToken` |
| `appearance` | `light / dark` |
| `scroll_scale` | `Number` |
| `scroll_opacity` | `Number` |
| `scroll_origin_x` | `Number` |
| `scroll_origin_y` | `Number` |
| `nudge` | `Bool` |
| `nudge_key` | `KeyValue` |
| `nudge_scale` | `Number` |
| `reduce_motion` | `Bool` |
| `stagger` | `Number` |
| `idle_area` | `Number`：鼠标在此停住多少秒后窗口静置；有静置区时，鼠标停在别处不静置 |
| `at` | `NumberValue` |
| `from` | `NumberValue` |
| `to` | `NumberValue` |

不接受上述通用属性的当前组件：`WindowSet`、`Window`、`Detail`、`SidebarItem`、`Section`、`Slot`、`Pages`、`PageContent`、`Destination`、`Tab`、`Layout`、`MenuItem`。

`scroll_origin_x`、`scroll_origin_y` 是 `scroll_scale` 放大时不动的那一点，按元素宽高的比例取 0 到 1，默认 0.5（正中），如 CSS 的 `transform-origin`。`at`、`from`、`to` 用于 Rail 中的定位；`stagger` 仅用于支持错峰的 Column、Row、Grid、Stack、ForEach、List。通用属性存在并不表示它在任意容器中都有意义。

## 浮层状态与消息输入


`PopoverButton(panel_appearance: window)` 的气泡跟随窗口（应用明暗选择或系统），触发按钮仍保留所在区域的外观。省略时继承所在区域；气泡打开期间也响应明暗变化。`panel_appearance: surface` 根据当前平台样式选择：指定背景色时跟随窗口，原生材质气泡继承所在区域，以便视频上方的玻璃气泡保持深色和浅色文字。macOS 未指定背景色时保留系统气泡材质（支持的系统上为 Liquid Glass）。所有目标都支持 `style! { PopoverButton { background: color(light: "#F2F2F2", dark: "#1E1E1E"), foreground: text.primary } }`，颜色作用于完整气泡表面及内容，不作用于触发按钮。

`PopoverButton(open: app.panel) { … } => app.set_panel(app.panel)` 将动作中相同的状态表达式替换为新的开关值；打开、点击外部收起和 Escape 关闭都走这个动作。省略 `open` 时可继续使用 `presented: draft.panel`，或让组件自行保存状态。

macOS 上，`TextInput` 点击外部时释放焦点、保留草稿，也不阻止所点击控件的正常动作。点击内部仍可定位光标和选中文字。`submit: clear` 在提交后立即清空并释放键盘；等待服务器确认的输入框应使用 `clear_key`，在成功后递增数值，失败时保留。`clear_key` 初次挂载不清空，目前仅 SwiftUI 后端实现其运行行为。
