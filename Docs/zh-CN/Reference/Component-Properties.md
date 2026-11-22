[English](../../Reference/Component-Properties.md) · **简体中文**

# 内置组件属性表

更新：2026-11-20。本表从 [bern-ir::spec](../../../crates/bern-ir/src/spec.rs) 的当前公共定义导出，共 88 个内置组件。**粗体属性必填**，其余可选；这些是声明规则，目标实现范围见[平台能力表](Platforms.md)，具体使用见[组件用法](UI.md)。

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
| `TextValue` | 只读文本表达式 |
| `Bool` | 布尔表达式 |
| `Flag` | 构建期 true / false |
| `Shortcut` | 快捷键字符串字面量，或快捷键跟随的文本绑定 |
| `FileKinds` | 文件种类字符串 |
| `FileField` | 可写 `MediaSource`、`Option<MediaSource>` 或 `Vec<MediaSource>`（多选文件）会话字段 |
| `SaveFileField` | 由保存文件对话框写入的 `MediaSource`、`Option<MediaSource>` 或 `String` 会话字段 |
| `DateField` | 可写 `CalendarDate` 会话字段 |
| `DateValue` | 只读 `CalendarDate` 表达式 |
| `PositionField` | 可写滚动位置会话字段 |
| `TextField` | 可写文本会话字段 |
| `KeysField` | 可写 `Vec<String>` 或 `Vec<i64>` 会话字段，存行的键 |
| `BoolField` | 可写布尔会话字段 |
| `IntField` | 可写整数（`i64`）会话字段 |
| `NumberField` | 可写浮点会话字段 |
| `OptionalNumberField` | 可写的 `Option<f64>` 会话字段，由元素写入，所报告的情况结束时清为 `None` |
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
| `Watched` | 只读的布尔、数值、文本或枚举，或它们的可选值与列表：`Watch` 跟随的值 |
| `ColorRole` | 语义颜色角色 |
| `NumberList` | 只读数值列表 |
| `PatternName` | style! 图案名称 |
| `MediaSource` | 媒体来源绑定，可选或非可选 |
| `Player` | 应用播放器目录中的播放器 |
| `Query` | 类型化查询调用 |
| `Feature` | 所属功能契约中的入口 |
| `NativeFile` | 相对声明所在 `.rs` 文件的紧邻原生界面实现路径字面量 |
| `NativeName` | 暴露给 `NativeView` 的只读值、文本或草稿字段的小写 `snake_case` 标识符 |
| `NativeScalar` | 传入 `NativeView` 的只读标量表达式（`bool`、`i64`、`f64`、`String` 或 `i64`/`f64`/`String` 的可选值） |
| `NativeDraftField` | 暴露给 `NativeView` 读写的可写会话草稿字段（`bool`、`i64`、`f64`、`String` 或 `Option<f64>`） |

固定枚举直接列出允许的关键字。表内没有逐一重复所有跨属性约束：Switch / Picker / Slider / ColorPicker 必须二选一指定草稿来源或受控来源，受控来源还必须提供动作；`Window` 的 `overlay` 必须与 `role: utility` 同用（主窗口不能声明 `role: utility`）；`appear_delay` 必须配合 `appear`；具体错误由编译器检查。资源、语义颜色、本地化、功能覆盖与类型仍会进一步校验。

## 组件

| 组件 | 属性（粗体必填） | 子内容、动作与位置 |
| --- | --- | --- |
| `WindowSet` | **`handle: TypeName`**<br>**`primary: WindowId`**<br>`accent: KeyValue`<br>`palette: KeyValue`<br>`appearance: KeyValue`<br>`icon: KeyValue` | 仅含 ["Window", "MenuBar", "StatusItem"]；无动作；仅根节点 |
| `MenuBar` | 无 | 仅含 ["CommandMenu"]；无动作；直接位于 ["WindowSet"]，最多一个；只在 macOS 的菜单栏显示 |
| `StatusItem` | **`label: LocalizedText`**<br>`icon: Icon`<br>`image: SvgResource`<br>`login_item: Flag`<br>`shown: Bool` | 仅含 ["CommandItem", "CommandMenu", "Divider"]，可在 `if`、`match`、`ForEach` 中；无动作；直接位于 ["WindowSet"]，最多一个；菜单不读写草稿与窗口字段，菜单项不带 `pick`、`shortcut`；`icon` 与 `image` 二选一：`icon` 只在 macOS 显示，Windows 与 Linux 显示应用图标；`image` 是单色 SVG，三个桌面平台都按所在栏的明暗显示成白色或黑色；iOS、tvOS、Android 与网页不显示 |
| `CommandMenu` | `label: LocalizedText`<br>`role: app / file / edit / view / window / help` | 仅含 ["CommandItem", "CommandMenu", "Divider"]，可在 `if`、`match`、`ForEach` 中；无动作；直接位于 ["MenuBar", "CommandMenu", "StatusItem"]；`label` 与 `role` 二选一，`role` 只用于菜单栏上的菜单 |
| `CommandItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`shortcut: Shortcut`<br>`enabled: Bool`<br>`checked: Bool`<br>`pick: FileField`<br>`save: SaveFileField`<br>`suggested_name: DisplayText`<br>`files: FileKinds`<br>`paste: TextField` | 不含子节点；必须绑定动作；直接位于 ["CommandMenu", "StatusItem"]；菜单栏的项的草稿属于主窗口；`pick`、`save` 与 `paste` 最多选一个 |
| `Window` | **`id: WindowId`**<br>**`title: LocalizedText`**<br>`initial: open / closed`<br>`presentation: auto / virtual / floating / native_required`<br>`role: standard / settings / dialog / utility`<br>`overlay: Id`<br>`width: Number`<br>`height: Number`<br>`min_width: Number`<br>`min_height: Number`<br>`confirm_close: Bool`<br>`close_message: LocalizedText`<br>`chrome: standard / unified`<br>`immersive: Bool`<br>`landscape: Bool`<br>`idle_after: Number`<br>`idle_enabled: Bool` | 恰好一个子节点；动作可选；直接位于 ["WindowSet"]；`role: utility` 声明紧凑的浮动工具窗口，`overlay` 在首次打开时将其置于同名 `bern::overlay` 旁，生成后保持位置不再随浮层变动而推移（当该浮层隐藏、锁定或切入任务栏时自动关闭） |
| `Column` | `variant: StyleVariant`<br>`gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`width: Length`<br>`height: Length`<br>`align: leading / center / trailing` | 可含子内容；无动作 |
| `FocusRegion` | 无 | 一个内容视图；文本编辑时首次区域外指针操作只失焦并消费整次点击，保留草稿；区域内控件和下一次点击正常 |
| `Row` | `variant: StyleVariant`<br>`gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`width: Length`<br>`height: Length`<br>`align: top / center / bottom / baseline` | 可含子内容；无动作 |
| `Accordion` | **`count: NumberValue`**<br>**`selected: NumberValue`**<br>**`collapsed_width: Length`**<br>`height: Length`<br>`gap: Length`<br>`corner: Length` | 可含子内容；无动作；省略 height 时使用剩余可用高度 |
| `Grid` | **`min_width: Length`**<br>`gap: Length`<br>`row_gap: Length`<br>`padding: Length`<br>`padding_x: Length`<br>`padding_y: Length`<br>`cell_ratio: Number`<br>`cell_extra: Length`<br>`masonry: Flag` | 可含子内容；无动作 |
| `Stack` | `variant: StyleVariant`<br>`enabled: Bool`<br>`align: top_leading / top / top_trailing / leading / center / trailing / bottom_leading / bottom / bottom_trailing`<br>`width: Length`<br>`height: Length`<br>`under_bars: Flag` | 可含子内容；无动作 |
| `Marquee` | **`duration: Length`**<br>`phase: Number`<br>`paused: Bool`<br>`opacity: NumberValue` | 可含子内容；无动作 |
| `ScaleBox` | **`width: Length`**<br>**`height: Length`** | 可含子内容；无动作 |
| `Positioned` | **`x: Length`**<br>**`y: Length`**<br>**`width: Length`**<br>**`height: Length`**<br>`bleed: Bool` | 可含子内容；无动作；`bleed` 让 `ScaleBox` 场景里的图层在到达的画布边上延伸到盒子边缘 |
| `Modal` | `width: Length`<br>`height: Length`<br>`cover: compact` | 可含子内容；无动作；不写 `height` 时与内容一样高（其中的 `ScrollView` 与它滚动的内容一样高），窗口放不下时到窗口边缘为止 |
| `Flow` | `gap: Length` | 可含子内容；无动作；每行的子项竖直居中，`Spacer` 占满所在行余下的宽度 |
| `Timer` | **`interval: Number`**<br>`enabled: Bool` | 不含子节点；必须绑定动作 |
| `Watch` | **`value: Watched`** | 不含子节点；必须绑定动作；挂载时与值变化时运行 |
| `Shortcut` | **`key: Shortcut`**<br>`enabled: Bool` | 不含子节点；必须绑定动作 |
| `PickerOption` | **`value: KeyValue`**<br>**`label: DisplayText`** | 不含子节点；无动作；直接位于 ["Picker"] |
| `Rail` | `axis: horizontal / vertical`<br>`anchor: start / center / end`<br>`overhang: Length` | 可含子内容；无动作 |
| `Spacer` | 无专有属性 | 不含子节点；无动作 |
| `WindowButtons` | 无专有属性 | 不含子节点；无动作；macOS 上占出红绿灯的位置，其他平台不占位置 |
| `Divider` | 无专有属性 | 不含子节点；无动作 |
| `ScrollView` | `axis: vertical / horizontal`<br>`snap: start / center`<br>`position: PositionField`<br>`arrival: nudge`<br>`indicators: automatic / hidden / mouse`<br>`open_at: Number`<br>`open_center: NumberValue`<br>`anchor: Number`<br>`browsing: BoolField` | 可含子内容；无动作 |
| `SplitView` | 无专有属性 | 仅含 ["Sidebar", "Detail"]；无动作 |
| `Sidebar` | **`selection: SelectionField`**<br>`collapsed: BoolField`<br>`width: Length`<br>`id: Id`<br>`fallback: SelectionValue` | 仅含 ["Search", "Section", "SidebarItem", "Pages", "SidebarHeader", "SidebarFooter"]；无动作；直接位于 ["SplitView"]；`collapsed`（`false` 为展开）在宽窗口下双向同步，窄窗口自动折叠不覆盖用户偏好 |
| `SidebarHeader` | `role: content / title` | 可含子内容；无动作；直接位于 ["Sidebar"] |
| `SidebarFooter` | 无专有属性 | 可含子内容；无动作；直接位于 ["Sidebar"] |
| `Detail` | `titled: Bool` | 可含子内容；无动作；直接位于 ["SplitView"] |
| `DetailFooter` | 无专有属性 | 可含子内容；无动作；直接位于 ["Detail"]，最多一个，不放在 `if`、`match` 或 `ForEach` 里。固定在详情列底部的内容，在详情列内推入子页面时仍保持可见 |
| `SidebarItem` | **`value: SelectionValue`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`badge: NumberValue`<br>`variant: StyleVariant` | 不含子节点；无动作；直接位于 ["Sidebar", "Section"] |
| `Section` | `title: DisplayText`<br>`footer: DisplayText`<br>`style: grouped / plain`<br>`padding_x: Length`<br>`padding_y: Length` | 可含子内容；无动作 |
| `Slot` | **`id: SlotName`** | 不含子节点；无动作 |
| `Pages` | **`slot: SlotName`**<br>`all: Flag` | 不含子节点；无动作；直接位于 ["Sidebar", "Section", "Tabs"]。在 `Tabs` 里每个页面展开成一个 `Tab`，内容是该页面的组件；`all: true` 包含全部贡献页面 |
| `PageContent` | **`slot: SlotName`**<br>**`selected: SelectionField`** | 不含子节点；无动作 |
| `Form` | `title: DisplayText`<br>`variant: StyleVariant`<br>`gap: Length`<br>`row_height: Length`<br>`padding_x: Length`<br>`padding_y: Length` | 可含子内容；无动作 |
| `Navigation` | **`path: PathField`**<br>`id: Id`<br>`reset: KeyValue` | 可含子内容；无动作；`reset` 变化时回到根页 |
| `Destination` | 无专有属性 | 可含子内容；无动作；直接位于 ["Navigation"] |
| `NavLink` | **`id: Id`**<br>**`route: Route`** | 可含子内容；无动作 |
| `ControlGroup` | `label: DisplayText`<br>`height: Length` | 可含子内容；无动作 |
| `Tabs` | **`selection: SelectionField`**<br>`id: Id`<br>`fallback: SelectionValue`<br>`bar: standard / floating` | 仅含 ["Tab", "Pages", "TabAccessory"]；无动作。选中的标签取应用强调色；`bar: floating` 让手机上的标签栏成为离开屏幕边缘的胶囊：iOS 26 起本来如此，Compose 画成浮起的胶囊（选中的一项另有一块胶囊底），`TabAccessory` 也是胶囊；GTK、WinUI 3 与 Web 保持各自的标签栏 在 Android 上，窗口高度低于紧凑断点（`window.short`，如手机横屏）时，浮动标签栏改为页面旁的一列图标，配件是页面底部的胶囊。 |
| `TabAccessory` | `shown: Bool` | 可含子内容；无动作；直接位于 ["Tabs"]，最多一个，不放在 `if` 或 `ForEach` 里。各页之上常驻的一条，例如正在播放的小播放器；`shown` 为假时收起 |
| `Tab` | **`value: SelectionValue`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`display: title_and_icon / icon` | 可含子内容；无动作；直接位于 ["Tabs"]。`display: icon` 只显示图标（必须写 `icon`），`label` 仍是可访问名称 |
| `SplitButton` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: BoolField`<br>`checked: Bool`<br>`icon: Icon`<br>`shape: capsule`<br>`height: Length`<br>`more_label: DisplayText`<br>`shortcut: Shortcut` | 可含子内容；`value` 与 `checked` 二选一，`checked` 须有动作，切换时执行 |
| `PopoverButton` | `panel_appearance: inherit / window / surface`<br>**`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`font_size: Number`<br>`shape: capsule`<br>`height: Length`<br>`prominence: standard / plain`<br>`presented: BoolField`<br>`open: Bool`<br>`shortcut: Shortcut` | 可含子内容；`open` 必须绑定动作，与 `presented` 互斥；未使用 `open` 时无动作 |
| `Inspector` | **`presented: BoolField`**<br>`width: Number` | 可含子内容；无动作 |
| `Toast` | **`content: DisplayText`**<br>`icon: Icon`<br>`key: KeyValue`<br>`duration: Number` | 不含子节点；无动作 |
| `Pinch` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: NumberField`**<br>**`min: Number`**<br>**`max: Number`**<br>`feature: Feature` | 恰好一个子节点；动作可选 |
| `Swipe` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: NumberField`**<br>**`min: Number`**<br>**`max: Number`**<br>`axis: vertical / horizontal`<br>`distance: Number`<br>`continuous: Flag`<br>`active: BoolField`<br>`feature: Feature` | 恰好一个可视子节点外加可选 `OnCommit`；动作可选 |
| `OnCommit` | 无 | 不含子节点；必须绑定动作；直接位于 ["Swipe"]，最多一个；在 `Swipe` 完成松手后运行一次，取消时不运行 |
| `ScreenBrightness` | **`value: NumberField`**<br>**`available: BoolField`** | 不含子节点；无动作；挂载期间临时覆盖宿主亮度（0..1）；`available` 表示宿主是否支持该覆盖 |
| `Drop` | **`id: Id`**<br>**`label: DisplayText`**<br>**`value: FileField`**<br>`files: FileKinds`<br>`over: BoolField`<br>`opens: Flag`<br>`feature: Feature` | 恰好一个子节点；必须绑定动作 |
| `Toolbar` | 无专有属性 | 仅含 ["Button", "IconButton", "Picker", "Menu", "Search", "Switch", "SplitButton", "PopoverButton", "Divider", "Spacer", "Text"]；无动作 |
| `Adaptive` | `id: Id` | 仅含 ["Layout"]；无动作 |
| `Layout` | **`size: compact / medium / expanded / large / tv`** | 可含子内容；无动作；直接位于 ["Adaptive"] |
| `Menu` | **`id: Id`**<br>**`label: DisplayText`**<br>`hide_disabled: Flag`<br>`icon: Icon`<br>`style: button / icon`<br>`shape: capsule`<br>`height: Length` | 仅含 ["MenuItem", "Divider"]；无动作 |
| `MenuItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`role: normal / destructive`<br>`checked: Bool`<br>`enabled: Bool`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`save: SaveFileField`<br>`suggested_name: DisplayText`<br>`files: FileKinds`<br>`paste: TextField` | 不含子节点；必须绑定动作；直接位于 ["Menu", "TableMenu", "ContextMenu"]；`pick`、`save` 与 `paste` 最多选一个；`checked` 显示平台原生的勾选状态 |
| `LanguagePicker` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag` | 不含子节点；无动作 |
| `List` | `items: Items`<br>`key: KeyField` | 可含子内容；无动作 |
| `ForEach` | **`items: Items`**<br>**`key: KeyField`** | 可含子内容；无动作 |
| `Table` | **`id: Id`**<br>**`label: DisplayText`**<br>**`items: Items`**<br>**`key: KeyField`**<br>`selection: KeysField` | 仅含 ["TableColumn", "TableMenu"]，至少一个 `TableColumn`，最多一个 `TableMenu`；动作可选，是选中行的主操作（双击或 Enter，触屏上点一行）；`selection` 是 `Vec<String>` 或 `Vec<i64>` 草稿，按行的顺序存选中行的键。使用 `\|item\|` 行模板 |
| `TableColumn` | **`title: DisplayText`**<br>`width: Number`<br>`sorted: Bool`<br>`descending: Bool` | 恰好一个子节点，是每行这一格的内容；动作可选，点表头时运行；直接位于 ["Table"]；`sorted`、`descending` 只显示排序标记，行的顺序由引擎决定 |
| `TableMenu` | 无 | 仅含 ["MenuItem", "Divider"]；无动作；直接位于 ["Table"]；右键或长按一行时打开，该行不在选中行里时先只选中它 |
| `ContextMenu` | 无 | 仅含 ["MenuItem", "Divider"]；无动作；直接位于指针事件的宿主元素内（见下文），每个元素最多一个；不算元素显示的内容；右键或长按元素时打开 |
| `DoubleTap` | 无 | 不含子节点；必须绑定动作；直接位于指针事件的宿主元素内，每个元素最多一个；双击或双击触摸时运行；在 `Button` 上与按钮自己的动作互斥，在其他元素上单击仍交给内容 |
| `WheelUp` | 无 | 不含子节点；必须绑定动作；直接位于指针事件的宿主元素内，每个元素最多一个；滚轮或触控板在元素上向上滚动（让滚动视图回到顶部的方向）每一格运行一次，这段滚动由元素接住 |
| `WheelDown` | 无 | 不含子节点；必须绑定动作；直接位于指针事件的宿主元素内，每个元素最多一个；滚轮或触控板在元素上向下滚动每一格运行一次，这段滚动由元素接住 |
| `PreviewList` | **`items: Items`**<br>**`key: KeyField`**<br>**`item_height: Number`**<br>`gap: Number`<br>`more_height: Number` | 按可用高度创建能完整放下的前几项；有剩余时为 `+N` 留位，默认高度 22；不滚动，也不创建隐藏项。使用 `\|item\|` 行模板 |
| `Text` | **`content: DisplayText`**<br>`style: large_title / title / title2 / title3 / headline / subheadline / body / callout / footnote / caption / caption2 / control / code`<br>`emphasis: primary / secondary / tertiary`<br>`color: accent / status.success / status.warning / status.error`<br>`mark: ColorRole`<br>`lines: Number`<br>`font_size: Length`<br>`line_height: Length`<br>`ruby: TextValue`<br>`markdown: Flag`<br>`selectable: Flag`<br>`bold: Bool`<br>`lit: Length`<br>`lit_path: TextValue`<br>`lit_clock: NumberValue`<br>`lit_since: NumberValue` | 不含子节点；无动作；`color` 与 `emphasis` 二选一；`selectable: true` 允许原生文本选择与复制 |
| `Canvas` | `width: Length`<br>`height: Length`<br>`label: DisplayText`<br>`bleed: Flag` | 仅含 ["Shape", "Series", "Pattern"]；无动作 |
| `Shape` | **`kind: rect / rounded / capsule / ellipse`**<br>`x: Number`<br>`y: Number`<br>`width: Number`<br>`height: Number`<br>`corner: Number`<br>`fill: ColorRole`<br>`fill_from: ImageResource`<br>`fade: up / down / left / right`<br>`stroke: ColorRole`<br>`stroke_width: Number`<br>`dash: Number`<br>`dash_gap: Number`<br>`opacity: Number` | 不含子节点；无动作；直接位于 ["Canvas"] |
| `Series` | **`values: NumberList`**<br>`kind: area / line`<br>`max: NumberValue`<br>`fill: ColorRole`<br>`stroke: ColorRole`<br>`stroke_width: Number`<br>`opacity: Number` | 不含子节点；无动作；直接位于 ["Canvas"] |
| `Pattern` | **`name: PatternName`**<br>`opacity: Number` | 不含子节点；无动作；直接位于 ["Canvas"] |
| `Label` | **`title: DisplayText`**<br>**`icon: Icon`**<br>`style: large_title / title / title2 / title3 / headline / subheadline / body / callout / footnote / caption / caption2 / control / code`<br>`color: ColorRole`<br>`font_size: Number`<br>`display: title_and_icon / icon` | 不含子节点；无动作 |
| `Svg` | **`source: SvgResource`**<br>`static_source: SvgResource`<br>**`width: Length`**<br>**`height: Length`**<br>`label: DisplayText`<br>`fit: contain / cover / fill`<br>`playing: Bool`<br>`speed: NumberValue`<br>`restart: NumberValue`<br>`interactive: Flag`<br>`data: DisplayText`<br>`external_links: Flag` | 不含子节点；无动作 |
| `WebPage` | **`url: DisplayText`**<br>`cookies_file: DisplayText`<br>`cookie_domain: DisplayText`<br>`user_agent: DisplayText`<br>`content_mode: auto / mobile / desktop`<br>`close_on: DisplayText`<br>`label: DisplayText` | 不含子节点；动作可选（点击匹配 CSS 选择器 `close_on` 的元素时触发）；tvOS、watchOS 与 Wear OS 不支持 |
| `Image` | **`source: ImageResource`**<br>`label: DisplayText`<br>`width: Length`<br>`height: Length`<br>`fit: contain / cover / fill`<br>`corner: Length`<br>`rendering: original / template`<br>`saturation: NumberValue`<br>`aspect: Number` | 不含子节点；无动作；`saturation` 取值 0（灰度）到 1（原色），可跟随状态并随 `animate` 动画过渡 |
| `Progress` | `value: NumberValue`<br>`total: Number`<br>`label: DisplayText`<br>`style: bar / ring`<br>`width: Length`<br>`stroke_width: Number` | 不含子节点；无动作；`style: ring` 需要 `value`；`width` 与 `stroke_width` 只用于 `style: ring` |
| `Button` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`shape: capsule / chip`<br>`height: Length`<br>`width: fill`<br>`role: normal / destructive / cancel`<br>`prominence: standard / prominent / plain`<br>`enabled: Bool`<br>`cursor: default / pointer`<br>`variant: StyleVariant`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`save: SaveFileField`<br>`suggested_name: DisplayText`<br>`files: FileKinds`<br>`default_focus: Flag`<br>`clears: TextField`<br>`paste: TextField` | 可含子内容；必须绑定动作；`pick` 打开单选或多选（`Vec<MediaSource>`）文件选择器，`save` 打开保存文件对话框（可配合 `suggested_name` 预填默认文件名），`paste` 先把剪贴板文字放进草稿再运行动作；三者最多选一个 |
| `IconButton` | **`id: Id`**<br>**`icon: Icon`**<br>`font_size: Number`<br>`height: Length`<br>**`label: DisplayText`**<br>`prominence: standard / prominent / plain`<br>`color: ColorRole`<br>`enabled: Bool`<br>`variant: StyleVariant`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`pick: FileField`<br>`save: SaveFileField`<br>`suggested_name: DisplayText`<br>`files: FileKinds`<br>`paste: TextField`<br>`default_focus: Flag`<br>`icon_progress: NumberValue`<br>`badge_icon: Icon`<br>`badge_visible: Bool` | 不含子节点；必须绑定动作；`pick`、`save` 与 `paste` 最多选一个 |
| `TextInput` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: TextField`<br>`current: TextValue`<br>`prompt: DisplayText`<br>`kind: plain / secure / multiline`<br>`submit: clear`<br>`clear_key: NumberValue`<br>`feature: Feature` | 不含子节点；动作可选；`value` 与 `current` 二选一；`value` 时动作在提交时运行，`submit: clear` 在动作后清空草稿、交出焦点；`current` 时每次编辑都运行动作，动作参数里同一表达式就是输入的文字，编辑中引擎的文字不覆盖用户的输入；SwiftUI 的 `clear_key` 在数值变化时清空，供异步成功回执使用 |
| `Search` | **`id: Id`**<br>**`value: TextField`**<br>`prompt: DisplayText`<br>`feature: Feature`<br>`shortcut: Shortcut`<br>`style: automatic / palette`<br>`display: automatic / icon`<br>`panel_prompt: DisplayText`<br>`width: Number`<br>`height: Number`<br>`font_size: Number`<br>`suggest: Flag` | 可含子内容；动作可选；`display: icon` 仅用于 `style: palette` |
| `SearchItem` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`detail: DisplayText`<br>`hint: DisplayText`<br>`navigation: Bool`<br>`pick: FileField`<br>`files: FileKinds`<br>`feature: Feature` | 可含子内容；必须绑定动作 |
| `Switch` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: BoolField`<br>`checked: Bool`<br>`label_hidden: Flag`<br>`shape: capsule / chip`<br>`height: Length`<br>`style: switch / checkbox / button`<br>`icon: Icon`<br>`shortcut: Shortcut`<br>`login_item: Flag` | 不含子节点；动作可选；`login_item: true` 显示并改变系统的登录项，不写 `value`、`checked` 与动作 |
| `ShortcutInput` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>**`current: Shortcut`**（状态，不可为字面量）<br>`prompt: DisplayText`<br>`empty: DisplayText`<br>`enabled: Bool`<br>`feature: Feature` | 不含子节点；必须绑定动作，动作中与 `current` 相同的表达式换成选中的快捷键 |
| `Slider` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: NumberField`<br>`current: NumberValue`<br>`continuous: Flag`（仅 `current`）<br>`hover: OptionalNumberField`<br>**`min: Number`**<br>**`max: Number`**<br>`step: Number`<br>`min_icon: Icon`<br>`max_icon: Icon`<br>`feature: Feature` | 除指针事件外不含子节点；动作可选；指针停在滑块上时 `hover` 是指针在轨道上的位置，`min` 处为 0，`max` 处为 1，其余时候为 `None`；它不改变滑块的值 |
| `ColorPicker` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: NumberField`<br>`current: NumberValue`<br>`feature: Feature` | 不含子节点；`value` 时动作可选，`current` 时必须绑定动作；以 24 位 sRGB 整数（`0xRRGGBB`）表示颜色；tvOS、watchOS 与 Wear OS 不支持 |
| `Video` | **`id: Id`**<br>`source: MediaSource`<br>`player: Player`<br>`label: DisplayText`<br>`controls: standard / none`<br>`fit: contain / cover / fill`<br>`sizing: standard / fill`<br>`corner: Length` | 不含子节点；无动作。`controls: none` 的视频只显示画面，可以作为 `Button` 的内容。`sizing: fill` 让播放表面占满有明确大小的父容器，`fit` 只控制视频内容如何放进表面；内核弹幕可以使用黑边。默认 `standard` 保持原有布局。应用登记了媒体内核时由内核画进视图给它的表面，见[设计文档](../Design.md#应用自带的媒体内核) |
| `Query` | **`id: Id`**<br>**`query: Query`**<br>`presented: Bool`<br>`loading: DisplayText`<br>`loading_style: automatic / skeleton / spinner`<br>`empty: DisplayText`<br>`failed: DisplayText`<br>`icon: Icon` | 结果模板；开头可放一个 `QueryStatusHeader`；无动作；`automatic` 在未显式指定 `loading` 时默认投影编译期骨架图，显式指定 `loading` 时保持原生菊花与提示文案 |
| `QueryStatusHeader` | — | 恰好一个子节点；必须是 `Query` 的第一个直接子节点，至多一次；无动作 |
| `Picker` | **`id: Id`**<br>**`label: DisplayText`**<br>`value: PickerField`<br>`selected: KeyValue`<br>`items: Items`<br>`key: KeyField`<br>`item_label: KeyField`<br>`prompt: DisplayText`<br>`label_hidden: Flag`<br>`shape: capsule`<br>`height: Length`<br>`style: automatic / menu / segmented / radio / inline / chips / combobox` | 仅含 ["PickerOption"]（声明 `items` 时不可写 `PickerOption`）；动作可选；`items` 必须同时指定 `key` 与 `item_label`，且 `value` / `selected` 类型须与 `key` 字段一致；`style: combobox` 提供可搜索下拉选择器；`chips` 不接受 `shape` / `height` |
| `Tooltip` | **`content: DisplayText`** | 恰好一个子节点；无动作；在全部后端上为子控件提供悬停或聚焦提示 |
| `Banner` | `id: Id`<br>`severity: info / success / warning / error`<br>`title: DisplayText`<br>**`content: DisplayText`**<br>`icon: Icon`<br>`dismiss_label: DisplayText` | 可含尾部操作子节点（如 `Button`）；动作可选（绑定后显示关闭按钮） |
| `Disclosure` | **`id: Id`**<br>**`label: DisplayText`**<br>`icon: Icon`<br>`expanded: BoolField`<br>`open: Bool` | 可含子内容；动作可选（`open` 必须绑定动作且与 `expanded` 互斥；二者都省略时使用控件本地展开状态） |
| `NumberInput` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: NumberField`<br>`current: NumberValue`<br>`min: Number`<br>`max: Number`<br>`step: Number`<br>`precision: Number`<br>`enabled: Bool` | 不含子节点；`value` 时动作可选，`current` 时必须绑定动作；带步进器与范围约束的原生数字输入框 |
| `Alert` | **`title: DisplayText`**<br>`message: DisplayText` | 仅含 ["AlertAction"]；动作可选（取消/关闭时触发）；挂载即弹出原生确认对话框（通常放在 `if` 条件内） |
| `AlertAction` | **`id: Id`**<br>**`label: DisplayText`**<br>`role: default / cancel / destructive`<br>`enabled: Bool` | 不含子节点；必须绑定动作；直接位于 ["Alert"] |
| `DatePicker` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: DateField`<br>`current: DateValue`<br>`min: DateValue`<br>`max: DateValue`<br>`enabled: Bool` | 不含子节点；`value` 时动作可选，`current` 时必须绑定动作；绑定 `CalendarDate` 的原生日期选择器 |
| `TimePicker` | **`id: Id`**<br>**`label: DisplayText`**<br>`label_hidden: Flag`<br>`value: NumberField`<br>`current: NumberValue`<br>`min: Number`<br>`max: Number`<br>`step: Number`<br>`enabled: Bool` | 不含子节点；`value` 时动作可选，`current` 时必须绑定动作；以自午夜起的分钟数（`0..=1439`）表示的原生时间选择器 |
| `EmptyState` | **`title: DisplayText`**<br>`message: DisplayText`<br>`icon: Icon` | 可含操作按钮等子节点；无动作；用于空视图与空搜索结果的居中占位 |
| `Badge` | **`content: DisplayText`**<br>`tone: neutral / accent / success / warning / error`<br>`icon: Icon` | 不含子节点；无动作；紧凑的行内语义状态胶囊 |
| `Avatar` | `source: ImageResource`<br>**`label: DisplayText`**<br>`size: Length`<br>`shape: circle / rounded` | 不含子节点；无动作；支持图片与首字母回退的圆形或圆角头像 |
| `TreeView` | **`id: Id`**<br>**`label: DisplayText`**<br>**`items: Items`**<br>**`key: KeyField`**<br>**`parent: KeyField`**<br>`selection: PositionField`<br>`expanded: KeysField`<br>`feature: Feature` | 使用 `\|item\|` 行模板；动作可选（双击或回车触发）；`parent` 指定指向父节点 `key` 的 `Option<Key>` 字段；`selection` 保存当前选中键，`expanded` 保存已展开节点键列表 |
| `SplitPane` | `id: Id`<br>`axis: horizontal / vertical`<br>`ratio: NumberField`<br>`min_first: Length`<br>`min_second: Length` | 恰好两个子节点；无动作；带可拖动分隔条的双栏容器，`ratio` 绑定前导栏占比（`0..1`） |
| `Reorderable` | **`id: Id`**<br>**`label: DisplayText`**<br>**`items: Items`**<br>**`key: KeyField`**<br>**`move_from: IntField`**<br>**`move_to: IntField`**<br>`feature: Feature` | 使用 `\|item\|` 行模板；必须绑定动作；拖拽或上下移动时先把零基源/目标索引写入 `move_from`、`move_to`，再触发动作 |
| `BottomSheet` | `detents: medium / large / both`<br>`drag_indicator: Flag` | 可含子节点；动作可选（关闭时触发）；挂载即弹出原生底部抽屉（通常放在 `if` 条件内） |
| `Refreshable` | **`id: Id`**<br>`refreshing: Bool`<br>`feature: Feature` | 恰好一个子节点；必须绑定动作；支持下拉刷新（及 CLI `activate`），并在 `refreshing` 为真时显示原生刷新指示器 |
| `NativeView` | **`id: Id`**<br>**`label: DisplayText`**<br>`swiftui: NativeFile`<br>`winui3: NativeFile`<br>`compose: NativeFile`<br>`gtk: NativeFile`<br>`gpui: NativeFile`<br>`web: NativeFile` | 仅含 ["NativeValue", "NativeText", "NativeDraft", "NativeAction", "NativeNumber", "NativeBool", "NativeInput", "Fallback", "DoubleTap", "ContextMenu", "WheelUp", "WheelDown"]；至少声明一个后端文件且至少声明一个 CLI 功能入口（`NativeAction` / `NativeNumber` / `NativeBool` / `NativeInput`），最多一个 `Fallback`；缺少当前后端文件时渲染 `Fallback`（若无则编译报错） |
| `NativeValue` | **`name: NativeName`**<br>**`value: NativeScalar`** | 不含子节点；无动作；直接位于 ["NativeView"]；在生成的 `Props` 中暴露强类型只读字段 |
| `NativeText` | **`name: NativeName`**<br>**`content: DisplayText`** | 不含子节点；无动作；直接位于 ["NativeView"]；把经过 `tr!` 校验的多语言文本传入 `Props` |
| `NativeDraft` | **`name: NativeName`**<br>**`value: NativeDraftField`** | 不含子节点；无动作；直接位于 ["NativeView"]；在 `Props` 中暴露当前草稿值并在 `Actions` 中生成 `set_<name>` 写回回调 |
| `NativeAction` | **`id: Id`**<br>**`label: DisplayText`**<br>`enabled: Bool`<br>`feature: Feature` | 不含子节点；必须绑定动作；直接位于 ["NativeView"]；以 `<component>.<view>.<id>` 自动注册到 CLI（`ui activate`） |
| `NativeNumber` | **`id: Id`**<br>**`label: DisplayText`**<br>**`current: NumberValue`**<br>**`min: Number`**<br>**`max: Number`**<br>`step: Number`<br>`enabled: Bool`<br>`feature: Feature` | 不含子节点；必须绑定动作；直接位于 ["NativeView"]；以 `<component>.<view>.<id>` 自动注册到 CLI（`ui set-number`） |
| `NativeBool` | **`id: Id`**<br>**`label: DisplayText`**<br>**`checked: Bool`**<br>`enabled: Bool`<br>`feature: Feature` | 不含子节点；必须绑定动作；直接位于 ["NativeView"]；以 `<component>.<view>.<id>` 自动注册到 CLI（`ui set-bool`） |
| `NativeInput` | **`id: Id`**<br>**`label: DisplayText`**<br>**`current: TextValue`**<br>`enabled: Bool`<br>`feature: Feature` | 不含子节点；必须绑定动作；直接位于 ["NativeView"]；以 `<component>.<view>.<id>` 自动注册到 CLI（`ui edit` / `ui submit`） |
| `Fallback` | 无 | 可含子内容；无动作；直接位于 ["NativeView"]，最多一个；在未提供原生文件的目标平台上渲染 |
| `Skeleton` | `width: Length`<br>`height: Length`<br>`fraction: Number`<br>`max_width: Length`<br>`aspect: Number`<br>`corner: Length`<br>`shape: rect / capsule / circle`<br>`tone: primary / secondary / media`<br>`opacity: Number`<br>`pulse: Flag` | 不含子节点；无动作；以 `text.primary` 低透明度绘制的非交互占位形状（`primary` 0.09、`media` 0.07、`secondary` 0.06 或显式 `opacity`），附带随系统减少动态效果冻结的轻柔呼吸脉冲；`fraction` 不可与 `width` 同用，按 `min(可用宽, max_width) * fraction` 计算宽度 |

`Grid` 给出 `cell_ratio` 时每行一样高：列宽 ÷ `cell_ratio` + `cell_extra`（如海报 2 : 3 加两行字的高度：`cell_ratio: 0.6667, cell_extra: 54`），各项从格子顶部排，内容更高时伸进下方的行间距，与按最高一项定行高的默认方式不同。

`Grid` 加 `masonry` 时成为瀑布流：各项按自己的高度排，每一项放进目前最短的那一列（一样短时取最左边的），适合高度不一的卡片。SwiftUI 与 Compose 用自定义布局，Web 用一像素高的网格行让各项跨过自己的高度；GTK 与 Windows 暂按普通网格逐行排。

`Text(style: code)` 用平台自己的等宽字体，比正文略小，用于代码、命令和文件内容：SwiftUI 的 `.monospaced` 设计，Compose 的 `FontFamily.Monospace`，Web 的 `ui-monospace` 一族，GTK 的 `monospace` 样式类，Windows 的 Cascadia Mono（没有时 Consolas）。`style!` 的 `text` 块可以像其他样式一样给它定字号与字重。

**指针事件。** `DoubleTap`、`ContextMenu`、`WheelUp` 与 `WheelDown` 写成所作用元素的直接子节点，由它作宿主：`Button`、`Video`、`Slider`、`Image`、`Svg`、`Text`、`Label`、`Progress`、`Column`、`Row`、`Grid`、`Stack`、`Flow`、`ScaleBox` 或 `Positioned`。它们不是视图，不算元素显示的内容，所以 `Text`、`Slider` 这样的叶子元素也能用花括号带上它们；它们不能放在 `if` 或 `ForEach` 里。每个元素每种最多一个。它们是捷径：所做的事还要能经按钮、菜单或快捷键完成，键盘、遥控器与辅助技术不会双击、右键或滚动滚轮。

`Svg` 在构建期将带动画与交互的 `.svg` 资源预编译为紧凑的 `BSVG` 场景，在全部目标平台由纯 Rust 原生播放，无需 WebView；指针交互通过根 `<svg>` 上的 `data-bern-click` / `data-bern-dblclick` 与可选 `data-bern-ready` 声明，`static_source`、`data` 与 `external_links` 仍为兼容既有源码而保留。不含动画或交互的静态 SVG 自动降级为 `Image`。见[动画 SVG](../Implementation/Animated-SVG.md)。

`Text` 加 `lit` 时，前面这么多保持原色，其余淡显，用于逐字演唱：2.5 是两个字加第三个字的左半部分，能在字内绘制的目标会画出来；小于零时全部正常显示。按 Unicode 标量计数。SwiftUI、Compose、WinUI 和 GTK 会绘制这一效果，其他目标显示完整文字。

有 `lit_path` 时，只要 `lit` 不小于零，前端就自己每帧移动这个位置：`lit_path` 依次列出秒数和字符数，用空格隔开（`"10 0 11 2 12 2"`），位置在相邻两点之间匀速移动。`lit_clock` 是 `lit_since` 那一刻的秒数，`lit_since` 是 Unix 时间（秒），为零表示时钟停着。只有路线或时钟变了状态才变，不用逐步发送。Windows 由合成器移动它。

`Text(ruby:)` 在原文的指定范围上方显示读音。动态字符串是 JSON 数组，例如 `[[0,2,"とうきょう"]]`：起止位置是原文 Unicode 标量的半开区间，不是 UTF-8 字节或 UTF-16 单元。空串或 `[]` 保持普通文字排版。注音为对应的换行预留上方空间，不增加 `lit` 或 `lit_path` 的字符数；浮动文字的 `overlay::Line::ruby` 使用相同格式。非法、重叠和越界范围会被忽略。注音字符串是元数据，不是待翻译的正文；支持字符串字面量和条件表达式。

`Text` 加 `markdown` 时，内容是行内 Markdown：`**粗体**`、`*斜体*` 或 `_斜体_`、`` `代码` ``、`~~删除线~~` 和 `[链接](地址)`，反斜杠让下一个标记照字面显示。SwiftUI 交给系统的 `AttributedString(markdown:)`，Compose 写成带样式与 `LinkAnnotation` 的 `AnnotatedString`，Web 生成元素（不当作 HTML，链接只接受网页和邮件地址），GTK 转成 Pango 标记；Windows 暂时去掉标记只显示文字。整篇文档（标题、列表、表格等）由应用自己分块，每块用这样的文字显示。

`Search` 放在 `Toolbar` 或 `Sidebar` 里由它们接管；放在页面里是平台自己的搜索框（iOS 的 `UISearchTextField`、macOS 的 `NSSearchField`、网页的 `input[type=search]`），子内容是在框获得焦点或有文字时列在框下的结果，`width` / `height` / `font_size` 是框本身的尺寸与字号（工具栏里的搜索面板则是面板尺寸）。

`Rail(overhang:)` 让刻度在轨道两端各伸出这么多点：比例 f 的子项站在 f × (长度 + 2 × overhang) − overhang，如 CSS 里负外边距的时间轴。`ScrollView(open_at:)` 是打开时的位置：内容中比例为 f 的点对齐到视图中同一比例处（0 起点，0.5 正中对正中，1 末尾）。`ScrollView(open_center:)` 也是打开时的位置：内容中比例为 f 的点停在视图正中；靠近两端放不到正中时滚到尽头为止，这一点仍在视图里。它可以绑定表达式，只在打开时读一次，之后值变了也不跟着滚，例如时间轴打开时让“现在”尽量居中。

`Form` 的 `gap` 是分组之间的距离，`row_height` 是行的最小高度，`padding_x` / `padding_y` 是所有分组行的内边距（`Section` 自己的同名属性优先）；表单以 `Section(style: plain)` 开头时从页面顶部开始，这一组的行没有底色、边距和分隔线，作为随表单滚动的标题。表单给分组的行内边距不作用于 plain 分组，它只用自己的 `padding_x` / `padding_y`。打头的 plain 分组也可以来自插槽、组件或条件：表单在布局时看第一项是不是 plain 分组，条件变了跟着变，所以 `if window.compact { Section(style: plain) { … } }` 只在窄窗口里让表单贴顶。`style!` 里 `Form { background: 角色 }` 是行的底色，表单本身透出页面背景。

`Row` 的各项放得下时取各自的宽度，剩余空间由 `Spacer` 和撑满的项平分；放不下时其余项按各自宽度的比例收缩，不小于各自的最小宽度。`Spacer` 没有最小长度，放不下时先收到零，各目标相同。`Row` 里领头的文字后面紧跟 `Spacer` 时（没写宽度的 `Text`，或不写 `align`、`align: leading` 的 `Column`），由文字占去 `Spacer` 的位置：后面的控件保持自己的宽度，文字在剩下的宽度里换行，如系统列表行的标签；SwiftUI 与 Compose 这样排，其他目标按上面的一般规则。给定 `width` / `height` 而内容较小时，`Column` 从顶部开始、横向按 `align` 摆放，`Row` 从前缘开始、纵向按 `align` 摆放。`Column` 不写 `align` 时与 `leading` 相同：按钮等控件、内层的 `Column` 与 `Stack` 保持自己的宽度，输入框、分隔线、`Grid`、`List` 等横向填满的内容横跨整列，要让按钮横跨整列写 `Button(width: fill)`。直接写在窗口里、没写宽度的 `Column` 与窗口内容区同宽，从顶部开始、横向按 `align` 摆放；没写尺寸的 `Stack` 铺满内容区；比内容区小的其他正文从左上角排起。

## 切换分页时保留页面的查询

把 `Query` **放在宿主页面之内、切换分页的条件分支之外**，用 `presented` 决定何时显示结果和状态控件。如果分页选择器固定不滚动，把它和查询并排声明，首次答复到达前也能切换。

```rust
Column {
    Switch(id: show_results, label: tr! { "en" => "Show results" }, value: draft.show_results)
    Query(id: results, query: app.search(draft.text, 20), presented: draft.show_results,
          empty: tr! { "en" => "No matches" }) |books| {
        ForEach(items: books, key: id) |book| { Text(content: book.title) }
    }
    if !draft.show_results { Text(content: tr! { "en" => "Settings" }) }
}
```

`presented` 默认为 `true`。它是外层作用域中的布尔表达式，**不是**查询参数；单独切换它不会发起请求或推进请求版本。为 `false` 时不创建结果、状态或重试控件，不占布局，不进入焦点顺序、无障碍树或外部控件列表。订阅仍处理参数变化、`#[reads]` 声明的依赖失效以及查询回复。切回来显示的是**当前**状态：已有结果或空态立即呈现；真正尚未完成的首次请求仍显示加载，失败仍可重试。框架不为每个分页或每组参数缓存一份结果。

移除整个声明（例如把 `Query` 写进 `if draft.show_results`）或关闭所属窗口都会释放订阅。另一个窗口有自己的订阅。

### 让标题随结果滚动

如果分页选择器应随结果滚动，把它放进成功模板的 `ScrollView`。要在还没有答复时也能切换，在 `Query` 的第一个直接子节点 `QueryStatusHeader` 里调用同一个组件：

```rust
Query(id: results, query: app.search(draft.text, 20), presented: draft.show_results) |books| {
    QueryStatusHeader { browse_header(app: app) }
    ScrollView {
        browse_header(app: app)
        ForEach(items: books, key: id) |book| { Text(content: book.title) }
    }
}
```

`QueryStatusHeader` 只收一个子节点，使用外层作用域：可以读 `app` 和 `draft`，不能读本次查询的 `books`。如果只想在某个条件下显示，把 `if` 写在标题**里面**。等待首次答复时，标题位于原生进度控件上方；失败且没有旧答复时，它位于原生错误提示和重试按钮上方。它不替换这些控件，也不另开订阅。

成功返回空列表或 `None` 也算已有答复。只要当前有答复，包括刷新中或失败时带回了旧答复，就不显示状态标题。查询仍按原有方式呈现数据、空结果和错误。`presented` 为假时，两处内容都不构造。省略 `QueryStatusHeader` 不改变查询的默认界面。

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
| `tint` | `ColorRole`：这部分界面的强调色改用该角色，里面的控件、选中态与链接随之着色 |
| `scroll_scale` | `Number` |
| `scroll_opacity` | `Number` |
| `scroll_origin_x` | `Number` |
| `scroll_origin_y` | `Number` |
| `nudge` | `Bool` |
| `nudge_key` | `KeyValue` |
| `nudge_scale` | `Number` |
| `nudge_motion` | `MotionToken`：提示后元素回到原样的动效，省略时为 `jelly`；需要 `nudge` 或 `nudge_key` |
| `reduce_motion` | `Bool` |
| `spin` | `Bool`：成立时绕中心匀速转动，不成立时停在当时的角度 |
| `spin_period` | `Number`：转一圈的秒数，默认 1，负数逆时针；需要 `spin` |
| `glow` | `Number`：指针（鼠标悬停，或手指按住时）在元素范围内时跟着它的白光，半径为元素短边的这个倍数；上面叠着别的元素也亮 |
| `glow_opacity` | `Number`：光中心的不透明度，大于 0、不超过 1，默认 0.4；需要 `glow` |
| `blur` | `Length`：把元素的画面按这个半径（点）模糊，不越出元素的范围与圆角；可以跟随状态，配 `animate` 时按它的动效变化（`bound_blur`：六个目标都已实现） |
| `opacity` | `NumberValue`：元素不透明度（0 到 1）；可以跟随状态，配 `animate` 时按它的动效变化（自带 `opacity` 属性的 `Marquee`、`Skeleton`、`Shape`、`Series`、`Pattern` 仍由组件自身处理） |
| `stagger` | `Number` |
| `idle_area` | `Number`：鼠标在此停住多少秒后窗口静置；有静置区时，鼠标停在别处不静置 |
| `at` | `NumberValue` |
| `from` | `NumberValue` |
| `to` | `NumberValue` |
| `skeleton` | `Bool`：为真时在 120 ms 防闪烁延迟后切换为由当前元素子树编译期投影出的骨架图，为假时在 150 ms 内淡入真实内容；不可用于 `Query` 或 `Skeleton` |
| `skeleton_count` | `Number`：1–30 之间的整数，控制 `ForEach`、`List`、`Table`、`PreviewList` 或 `Query` 投影出的占位条数 |

不接受上述通用属性的当前组件：`WindowSet`、`Window`、`Detail`、`SidebarItem`、`Section`、`Slot`、`Pages`、`PageContent`、`Destination`、`Tab`、`Layout`、`MenuItem`。

`scroll_origin_x`、`scroll_origin_y` 是 `scroll_scale` 放大时不动的那一点，按元素宽高的比例取 0 到 1，默认 0.5（正中），如 CSS 的 `transform-origin`。`at`、`from`、`to` 用于 Rail 中的定位；`stagger` 仅用于支持错峰的 Column、Row、Grid、Stack、ForEach、List。通用属性存在并不表示它在任意容器中都有意义。

## 浮层状态与消息输入


`PopoverButton(panel_appearance: window)` 的气泡跟随窗口（应用明暗选择或系统），触发按钮仍保留所在区域的外观。省略时继承所在区域；气泡打开期间也响应明暗变化。`panel_appearance: surface` 根据当前平台样式选择：指定背景色时跟随窗口，原生材质气泡继承所在区域，以便视频上方的玻璃气泡保持深色和浅色文字。macOS 未指定背景色时保留系统气泡材质（支持的系统上为 Liquid Glass）。Windows 未指定背景色时是系统的应用内亚克力，同样继承所在区域的明暗，并限制在窗口内。所有目标都支持 `style! { PopoverButton { background: color(light: "#F2F2F2", dark: "#1E1E1E"), foreground: text.primary } }`，颜色作用于完整气泡表面及内容，不作用于触发按钮。

`PopoverButton(open: app.panel) { … } => app.set_panel(app.panel)` 将动作中相同的状态表达式替换为新的开关值；打开、点击外部收起和 Escape 关闭都走这个动作。省略 `open` 时可继续使用 `presented: draft.panel`，或让组件自行保存状态。

macOS 上，`TextInput` 点击外部时释放焦点、保留草稿，也不阻止所点击控件的正常动作。点击内部仍可定位光标和选中文字。`submit: clear` 在提交后立即清空并释放键盘；等待服务器确认的输入框应使用 `clear_key`，在成功后递增数值，失败时保留。`clear_key` 初次挂载不清空，目前仅 SwiftUI 后端实现其运行行为。
