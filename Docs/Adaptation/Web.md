# Bern Web 视觉规范

- 版本：v0.6
- Bern 接口说明更新：2026-10-17（显式强调控件与共享胶囊）。
- 官方资料核查日期：2026-10-10，Asia/Shanghai
- 阶段：Web 后端的视觉与交互规范（[架构设计](../Design.md)“Web 应用与 WebUI”要求先单独成文）
- 关联：[Bern 架构设计](../Design.md)、[自适应适配规范](README.md)

Web 没有统一的平台样式规范。本文是 Bern 为 Web 目标定义的视觉语言：**Bern 自有的中性网页风格**。它提炼各家网页设计系统与成熟网页应用共有的做法，风格接近 Linear、Vercel、GitHub 这类网页应用，整体不复制 Material、Fluent 或任何一家公司的视觉语言，也不模仿桌面操作系统的窗口。个别控件按项目决定借用成熟的形态：开关采用类似 Fluent 2 的形态，滑块的圆钮是页面底色的圆盘加 2 px 强调色圆环；中性色取冷灰色阶（zinc：深色页面 #09090B，填充 #18181B、#27272A）；借用的只是形状与尺寸，颜色全部来自 Bern 主题内核（`bern-theme`）。除“官方依据”一节引用的 W3C 规范外，文中规则都是 Bern 的设计决策。

## 1. 原则

- **页面就是应用。** 主窗口不画窗口外框、标题栏或窗口按钮；网页没有“窗口”这个概念，次级界面用对话框或侧边面板呈现。可拖动的浮动窗口只在应用显式声明 `presentation: floating` 时出现。
- **安静的表面，清楚的层级。** 页面为纯色画布，内容分组用细边框卡片；颜色只用于主要操作、选中、焦点与状态，不用大面积品牌色或装饰性渐变。
- **交互反馈用高饱和的强调色。** 侧边栏与竖排导航的选中项填满强调色，选中的页签、开关的开启与滑块的已选范围都用强调色；导航项不带左侧竖线。图标按钮是例外：它是没有底板的 Ghost 按钮，悬停与按下铺一层中性浅底。
- **网页的密度与尺度。** 正文 14 px，按钮、输入框、下拉菜单与图标按钮高 36 px；粗略指针下控件与点按区域整体放大。网页没有胶囊形按钮：声明 `shape: capsule` 的按钮、分体按钮、菜单与选择器就是标准控件，用标准高度与圆角；只有按钮样式的开关是胶囊形的 chip。
- **一套配色算法。** 生成的 CSS 只引用 `--bern-*` 变量，组件样式中没有字面颜色；浅色、深色、提高对比由同一算法生成，应用不写任何深浅色分支。
- **浏览器托管部分不伪装。** 滚动条、文本选区由浏览器绘制，Bern 通过 `color-scheme` 让它们与当前模式一致。下拉菜单由 Bern 自绘，使它与页面的样式、深浅模式一致，并在各浏览器中表现相同。
- **语义不只靠颜色。** 选中、错误、禁用同时保留文字、图标、形状或可访问状态。

## 2. 官方依据

| 依据 | 用于 |
| --- | --- |
| [WCAG 2.2](https://www.w3.org/TR/WCAG22/)：1.4.3 对比度（最低）AA，正文 4.5:1、大字号 3:1 | 文字与背景的受控配对 |
| WCAG 2.2：1.4.11 非文本对比 AA，与相邻颜色 3:1 | 识别控件所必需的边界、滑轨、开关、焦点指示 |
| WCAG 2.2：2.4.7 焦点可见 AA；2.4.11 焦点不被遮挡（最低）AA；2.4.13 焦点外观 AAA（至少 2 CSS px 周长、焦点前后 3:1） | 焦点环；Bern 以 AAA 的焦点外观为目标 |
| WCAG 2.2：2.5.8 目标尺寸（最低）AA，指针目标至少 24 × 24 CSS px | 图标按钮与紧凑控件的热区 |
| [Media Queries Level 5](https://www.w3.org/TR/mediaqueries-5/)（工作草案，2026-02-19）：`prefers-color-scheme`、`prefers-contrast`、`forced-colors`、`prefers-reduced-motion` | 外观、对比、强制颜色与动效偏好 |
| [WAI-ARIA APG 模式](https://www.w3.org/WAI/ARIA/apg/patterns/)：Button、Checkbox、Switch、Slider、Tabs、Listbox、Combobox（Select-Only）、Radio Group、Toolbar、Dialog (Modal)、Window Splitter | 各组件的角色、状态与键盘交互 |

## 3. 度量

### 3.1 网格与圆角

- 间距以 4 px 为单位：控件内 8–12，行内 16，卡片之间 24，页面边距 24（窄屏 16），分区之间 32。
- 圆角：控件与图标按钮 8 px；卡片、菜单、下拉 8 px；对话框与浮动窗口 12 px；开关、chip 与胶囊形标记全圆角。
- 边框 1 px：卡片与分隔线用 `separator`；需要靠边界识别的控件（输入框、复选框）用 `control.border`。

### 3.2 字体与字号

字体栈：`system-ui, -apple-system, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans", "Noto Sans CJK SC", sans-serif`；时间与计数用 `tabular-nums`。

| `Text.style` | 字号 / 行高（px） | 字重 |
| --- | --- | --- |
| `large_title` | 30 / 36 | 600 |
| `title` | 24 / 32 | 600 |
| `title2` | 20 / 28 | 600 |
| `title3` | 16 / 24 | 600 |
| `headline` | 14 / 20 | 600 |
| `body`（默认） | 14 / 20 | 400 |
| `callout` | 13 / 20 | 400 |
| `subheadline` | 13 / 18 | 400 |
| `footnote` | 12 / 16 | 400 |
| `caption` | 12 / 16 | 400 |
| `caption2` | 11 / 16 | 500 |

`pointer: coarse` 时正文及以下各级增加 2 px。标题各级的字距收紧 −0.01em。`emphasis: secondary` 用 `text.secondary`，`tertiary` 用 `text.placeholder`，两者都要通过 4.5:1。

### 3.3 控件尺寸

| 项目 | 精确指针 | 粗略指针（`pointer: coarse`） |
| --- | --- | --- |
| 按钮、输入框、下拉菜单高度 | 36 px（大号 42 px） | 44 px |
| 按钮横向 | 左右内边距 14 px（大号 20 px），13.5 px 字重 500（大号 14.5 px 字重 600），不设最小宽度 | 同左 |
| 图标按钮热区 | 36 × 36 px（图标 18 px），圆角 8 | 44 × 44 px（图标 20 px） |
| 卡片内的行最小高度 | 52 px | 60 px |
| 侧边栏导航项高度 | 34 px | 44 px |
| 开关 | 轨道 36 × 20，圆钮 16（距轨道边 2） | 轨道 48 × 26，圆钮 18 |
| 滑块 | 轨道高 6，圆钮 16（页面底色圆盘加 2 px 强调色圆环，悬停与拖动放大到 1.15 倍） | 轨道高 6，圆钮 28 |
| 视频进度条 | 轨道高 6，滑块 20 × 16、圆角 7 | 轨道高 8，滑块 28 × 22 |
| 下拉菜单项 | 高 32，最多显示约 10 项后滚动 | 高 44 |

## 4. 颜色

### 4.1 主题快照到 CSS

- 构建时由 `bern-theme` 生成 `light`、`dark` 各自的标准对比与提高对比共四份快照，写成 `:root` 上的 `--bern-<角色>` 变量，并按 `prefers-color-scheme` 与 `prefers-contrast: more` 选择。首帧即为正确模式。
- `[theme] mode = "light"` 或 `"dark"` 时只输出对应模式；`system` 跟随浏览器偏好，偏好变化时在同一次样式计算中整体切换，不重建 DOM，不丢失焦点、输入法组合与编辑内容。
- `:root` 声明 `color-scheme`，使浏览器托管部分同步深浅。
- `forced-colors: active` 时改用系统颜色关键字（`Canvas`、`CanvasText`、`ButtonFace`、`ButtonText`、`Highlight`、`HighlightText`、`GrayText`），选中与焦点改用轮廓。

### 4.2 角色

每个角色声明配对关系，求解时只保证声明的配对。

| 角色 | 用途 | 声明的配对 |
| --- | --- | --- |
| `surface.canvas` | 页面背景、卡片 | — |
| `surface.panel` | 侧边栏、表格表头、次要区域 | — |
| `surface.raised` | 对话框、菜单、下拉、浮动窗口、输入框底色 | — |
| `surface.host` | 对话框遮罩下方与浮动窗口之间的页面底色 | — |
| `text.primary`、`text.secondary`、`text.placeholder` | 正文、次要说明、输入提示 | 对各表面与悬停、按下底色各 ≥ 4.5:1 |
| `icon.primary`、`icon.secondary` | 操作图标、辅助图标 | 同上 ≥ 3:1 |
| `accent`、`on_accent` | 主要按钮、侧边栏与竖排导航的选中项、开关开启、滑块已选范围、图标按钮悬停；其上文字图标 | `on_accent`/`accent` ≥ 4.5:1；`accent` 对画布、面板与浮层 ≥ 3:1 |
| `accent.text` | 链接、平直按钮、选中页签的文字，图标按钮按下与选中；从 `accent` 的明度出发，只移动到可读所需的程度，保持与强调色一样鲜明 | 对各表面 ≥ 4.5:1 |
| `status.success`、`status.warning`、`status.error` 及 `on_*` | 状态前景与状态底色上的前景 | 前景对各表面 ≥ 4.5:1；`on_error`/`status.error.surface` ≥ 4.5:1 |
| `control.border` | 输入框、复选框等靠边界识别的控件 | 对所在表面 ≥ 3:1 |
| `separator` | 卡片边框、分隔线、带文字按钮的轮廓（装饰性） | 不检查对比，分类为装饰 |
| `control.track`、`control.thumb` | 滑块的未选轨道；圆钮与圆钮外环 | track 对表面 ≥ 3:1；thumb 对 track 与 accent ≥ 3:1 |
| `media.played`、`media.track` | 视频进度条的已播放部分与未播放轨道 | played 对 track 与面板 ≥ 3:1；track 为装饰 |
| `focus.indicator` | 焦点环 | 对各表面 ≥ 3:1 |
| `hover.overlay`、`pressed.overlay` | 行、导航项与平直控件的悬停、按下底色（不透明） | 其上的文字仍满足各自配对 |
| `disabled.foreground`、`disabled.surface` | 禁用文字与禁用底色 | 不可用控件单独分类：foreground/surface ≥ 3:1 |
| `scrim` | 对话框遮罩 | 遮罩上的对话框为不透明 raised 表面 |

### 4.3 控件配方

| 配方 | 状态 | 底色 / 前景 / 边界 |
| --- | --- | --- |
| `button.standard` | rest、hover、pressed、focus、disabled | 透明 / text.primary / separator；悬停 hover.overlay，按下 pressed.overlay |
| `button.prominent` | 同上 | accent / on_accent；悬停、按下为 accent 的明度变体，仍满足 on_accent 配对 |
| `button.plain` | 同上 | 透明 / accent.text；悬停 hover.overlay |
| `button.destructive` | 同上 | status.error.surface / on_error；standard 与 plain 的危险角色改用 status.error 文字 |
| `icon_button` | rest、hover、pressed、selected、focus、disabled | Ghost 按钮：静止时没有底色或边框，前景 icon.primary；悬停 hover.overlay，按下 pressed.overlay；选中前景 accent.text |
| `field` | rest、hover、focus、error、disabled | raised / text.primary / control.border；聚焦时边界 accent 并显示焦点环；错误时边界与说明 status.error |
| `switch` | off、on、focus、disabled | 关：透明轨道加 1 px control.border 边框，圆钮 control.border，悬停时边框与圆钮为 text.secondary；开：轨道 accent，圆钮 on_accent，悬停轨道 button.prominent.hover |
| `slider` | rest、hover、active、focus、disabled | 未选轨道 separator，已选范围 accent；圆钮为 surface.canvas 圆盘加 2 px accent 圆环，悬停与拖动时放大到 1.15 倍 |
| `media_scrubber` | rest、hover、active、focus、disabled | 已播放 media.played，未播放 media.track；滑块 control.thumb 加 0.5 px control.border 轮廓与投影 |
| `segmented` | rest、selected、hover、focus、disabled | 外槽 panel；选中段 canvas + text.primary + separator；未选中 text.secondary |
| `chip` | rest、selected、hover、focus、disabled | 透明 / text.secondary / 1 px control.border 内描边；悬停 hover.overlay + text.primary；选中反色：text.primary 底、surface.canvas 字、无描边 |
| `nav_item` | rest、hover、selected、focus | 透明 / text.secondary；悬停 hover.overlay + text.primary；选中 accent + on_accent（文字与图标），选中且悬停 button.prominent.hover；没有指示条 |
| `row` | rest、hover、pressed、focus | 透明 / text.primary；悬停 hover.overlay |
| `tab` | rest、selected、hover、focus | 透明 / text.secondary；选中 accent.text + 2 px accent 下划线；竖排时选中同 `nav_item` |
| `menu` | rest、hover、selected、focus | 弹出层 raised / separator / 投影；项透明 / text.primary，悬停与键盘活动项 hover.overlay，选中项尾端 accent.text 对勾、字重 500；触发器用 `field` |
| `dialog` | — | raised / text.primary / separator；遮罩 scrim |
| `split_button` | rest、on、hover、focus | 一个整体：1 px separator 轮廓、8 px 圆角，两段高 32，箭头段宽 30、前面一条 separator 分隔线；开启时主段 pressed.overlay 底、text.primary 字、图标 accent |
| `toast` | — | 反色胶囊：text.primary 底、surface.canvas 字，高 40，距页面底边 30 |

## 5. 组件

组件语义以 [`bern-ir` 组件规范](../../crates/bern-ir/src/spec.rs) 为准；下文规定 Web 呈现、角色与键盘行为。

### 5.1 窗口：WindowSet 与 Window

- **应用栏。** 页面顶部是 48 px 的应用栏（`surface.canvas`，下边框 `separator`）：前端为应用名（`headline`）；尾端为设置按钮（`gearshape` 图标按钮），在应用声明了 `role: settings` 的窗口时出现，Ctrl / Cmd + , 有同样效果。只有存在 `presentation: floating` 的窗口时，应用栏中间才出现窗口切换条，当前窗口的项填满强调色。
- **主窗口就是页面。** 主窗口的内容直接铺满应用栏下方的区域，没有标题栏、窗口按钮和投影。主窗口被命令关闭后，该区域显示居中的应用名与“打开 {窗口标题}”按钮，页面本身不关闭。
- **对话框（次级窗口的默认呈现）。** 设置窗口与其他次级窗口呈现为居中的对话框：
  - 宽度取窗口声明的宽度（未声明时 560 px），不超过视口减 32 px；高度贴合内容，最高为视口的 85%，内容区滚动。
  - 头部 56 px：标题（`title3`）在前端，关闭按钮（`xmark` 图标按钮）在尾端；没有最小化与最大化。
  - 遮罩为 `scrim`。点按遮罩、按 Esc 或点关闭按钮都会请求关闭，并经过关闭协议（有未存储内容时先显示确认）。打开时焦点进入对话框，关闭后回到打开它的元素。
  - 对话框内的根 `Tabs` 在宽度 ≥ 640 px 时呈现为左侧竖排导航（宽 180 px），否则为顶部下划线页签。
- **浮动窗口（`presentation: floating`）。** 只在应用显式声明时出现：可拖动、可调整大小的面板（`surface.raised`、12 px 圆角、投影），标题栏 40 px，尾端有最小化、最大化 / 恢复、关闭；最小化的窗口在应用栏切换条中恢复；拖动与调整尺寸松开时提交一次 `window.move` 或 `window.resize`，键盘：聚焦标题栏后方向键移动、Shift + 方向键调整尺寸。
- **窄屏（视口宽度 < 768 px）。** 对话框与浮动窗口占满应用栏下方的区域，头部保留标题与关闭按钮。
- **窗口动作。** 页面与对话框只有激活与关闭；最小化、最大化、移动、调整尺寸不可用，`window.list` 如实列出，执行时返回 `target_unavailable`。呈现方式报告为 `Virtual`。
- **关闭确认。** 以所属窗口内的模态确认框呈现（Dialog (Modal) 模式），焦点限制在确认框内，Esc 等同“取消”。

### 5.2 SplitView、Sidebar、Detail

- 侧边栏宽 240 px（可拖动 200–320 px，Window Splitter 模式），`surface.panel`，右边框 `separator`；详情栏 `surface.canvas`。
- 侧边栏顶部为 Search（高 32 px）；Section 标题 `caption2`、`text.secondary`，上方留 16 px。
- SidebarItem 用 `nav_item` 配方：高 32 px、左右内边距 10 px、6 px 圆角；图标 16 px；选中项填满强调色，文字与图标为 `on_accent`，字重 500，不带左侧竖线。Listbox 模式：上下方向键移动并选中，Home / End 到首尾。
- 详情栏顶部是页面头：高 64 px，标题为所选项的文字（`title2`），工具栏项在尾端；窄屏为 56 px、`title3`。
- 视口宽度 < 768 px 时，侧边栏成为第一页，选中项后显示详情，页面头前端出现返回按钮。

### 5.3 Form 与 Section

- Form 内容最大宽 720 px，水平居中，页面边距 24 px；有 `title` 时它作为页面标题。
- Section：标题在卡片上方（`title3`，与卡片间距 8 px）；内容在一张卡片里（`surface.canvas`、1 px `separator` 边框、8 px 圆角）；行之间以 `separator` 分隔；脚注在卡片下方（`footnote`、`text.secondary`）。卡片之间 32 px。
- 卡片中的行：开关、复选框、选择器、按钮是“标签在前、控件在尾”的横排，行内边距 16 px；文本输入、多行输入与滑块是“标签在上、控件占满宽度”的竖排。

### 5.4 Navigation、Destination、NavLink

- Navigation 是推入栈；推入的页面替换内容区，页面头前端出现返回按钮（`chevron.left`），Esc 或 Alt + ← 返回；根页面保持挂载。
- NavLink 是整行可点的链接行：内容在前，尾端 `chevron.right`；悬停 `hover.overlay`；Enter / Space 推入。

### 5.5 Tabs 与 Tab

- 默认为顶部下划线页签（Tabs 模式，`role="tablist"`）：标签 14 px，有图标时图标 16 px 在标签前；选中项文字与图标为 `accent.text`，加 2 px `accent` 下划线，其他 `text.secondary`；页签栏下边框 `separator`。左右方向键切换并激活。
- 对话框根部的 Tabs 在宽屏为左侧竖排导航，形状与选中样式同 `nav_item`（选中项填满强调色、不带竖线）；上下方向键切换。

### 5.6 Toolbar

- 位于页面头或对话框头的尾端，项间距 8 px，高 36 px。Button 用 standard 外观（`prominence: prominent` 保持主要按钮），Picker 用 segmented 外观，IconButton 无容器。Toolbar 模式：方向键在项之间移动焦点。
- `ControlGroup` 在网页上没有共享的底板：组里的图标按钮依次排开，相隔 6 px；组与同一行里相邻的东西（另一个组、标题、间隔）也只隔 6 px，不论所在行声明多大的间距，读起来是一排连续的 Ghost 图标按钮。

### 5.7 List 与 ForEach

- List 在 Section 内为卡片中的行；独立使用时为带边框卡片。行身份按 `key` 保持，插入、删除、重排不重建未变化的行。

### 5.8 Text、Label、Divider、Spacer、ScrollView、Column、Row

- Text、Label 按 3.2 的字号；Label 的图标 16 px，图标与文字间距 8 px，图标用 `icon.secondary`。
- Divider：1 px `separator`。ScrollView：原生滚动，`overscroll-behavior: contain`。Column、Row 为 flex 布局，`gap`、`padding` 以 px 计。

### 5.9 Button 与 IconButton

- Button：`<button>`，高 36 px，左右内边距 14 px，不设最小宽度，圆角 8 px，13.5 px 字重 500；有图标时图标在文字前，间距 6 px。外观由 `prominence` 与 `role` 选择 4.3 的配方；`cancel` 角色用 standard。`shape: capsule` 不改变它的形状与高度（网页没有胶囊形按钮），菜单与选择器的触发按钮同样如此，尾端箭头 14 px、左 12 右 10 的内边距。
- IconButton：`<button aria-label>`，默认热区 36 × 36 px、圆角 8 px，Ghost 按钮：静止时只有图标（`icon.primary`），悬停与按下铺中性浅底，焦点环保留。显式 prominent 提供强调容器（44 × 44、圆角 12）；ControlGroup 不加底板（见 5.6）。`font_size` 调整图标字号；完整尺寸与分组用法见[当前组件参考](../Reference/UI.md)。
- 禁用：`disabled` 属性与配方的禁用颜色；不用整体降低不透明度表示禁用。

### 5.10 TextInput 与 Search

- TextInput：`<input>`（`secure` 为 `type="password"`，`multiline` 为至少 3 行的 `<textarea>`），`field` 配方，高 32 px，标签在上方（14 px 字重 500，与输入框间距 6 px）；提示文字用 `text.placeholder`。
- 输入法组合期间只更新本地显示，组合结束后一次性写入会话。
- Search：前置 `magnifyingglass` 图标，尾端清除按钮（有内容时出现）；Enter 运行声明的提交动作，Esc 清空。

### 5.11 Switch

- `style: switch`：`<button role="switch" aria-checked>`，`switch` 配方。形态类似 Fluent 2：关闭时是带边框的空心轨道与同色圆钮，开启时轨道填满强调色、圆钮变为 `on_accent`；圆钮移动 150 ms。Space / Enter 切换。
- `style: checkbox`：16 px 方框、4 px 圆角，选中时 `accent` 底与 `on_accent` 对勾。

### 5.12 Slider

- `<input type="range">`，以 `slider` 配方自绘轨道与圆钮：6 px 轨道，已选范围为强调色；圆钮 16 px，页面底色的圆盘加 2 px 强调色圆环，悬停与拖动时放大到 1.15 倍。`min_icon` / `max_icon` 在两端（16 px，`icon.secondary`）。
- 拖动时只更新会话预览；松开或键盘调整结束时运行声明的提交动作。

### 5.13 Picker

- `automatic`、`menu`：Bern 自绘的下拉菜单，遵循 WAI-ARIA APG 的“只选组合框”（Select-Only Combobox）模式。
  - 触发器：`<button role="combobox" aria-haspopup="listbox" aria-expanded>`，`field` 配方，高 32 px，最小宽 160 px，显示当前选项与尾端 `chevron.down`。
  - 弹出层：`role="listbox"`，置于浏览器顶层（Popover API），不被卡片、对话框或滚动容器裁切；宽度不小于触发器，下方空间不足时向上弹出；`menu` 配方，8 px 圆角，内边距 4 px。
  - 选项：`role="option"`，高 32 px，左右内边距 8 px，6 px 圆角；选中项尾端对勾并有 `aria-selected="true"`。
  - 键盘：Enter、Space、↓、Alt + ↓ 打开并把活动项放在当前选项；↑ ↓ 移动活动项，Home / End 到首尾，输入字符跳到以其开头的选项；Enter 或 Space 选择并关闭；Esc 关闭且不改变值；Tab 选择活动项后关闭并移走焦点。点按弹出层外部关闭。
  - 选择只经过与触屏、外部入口相同的选择闭包；弹出层的打开与关闭不写入会话。
- `segmented`：Radio Group 模式的分段控件，`segmented` 配方，选中段带细微投影。
- `chips`：Radio Group 模式的一排药丸按钮（高 30 px、左右 12 px、13 px 字重 500），`chip` 配方，组内换行、间距 8 px。直接放在 `Flow` 里且 `label_hidden` 时，组容器为 `display: contents`，每枚标签都是 Flow 的子项。
- `radio`、`inline`：纵向单选列表，每项 16 px 圆形指示。

### 5.14 Video

- `<video>` 元素，`object-fit` 由 `fit` 决定，画面保持原色。**关闭浏览器原生 `controls`**，使用 Bern 标准控制条，置于画面下方的不透明条上（高 44 px，`surface.panel`），不叠在动态画面上。
- 控制条：播放 / 暂停、当前时间 / 时长（`tabular-nums`）、进度条、静音、音量。时长未知显示 `--:--`，直播显示“直播”且进度禁用。
- 进度与音量使用专用的**媒体进度条**（`media_scrubber` 配方），不复用 Slider：已播放部分为中性的 `media.played`，不使用强调色；滑块是类似 macOS 26 的白色圆角胶囊（20 × 16 px、圆角 7、柔和投影），轨道 6 px。它仍是 `role="slider"`（`aria-valuetext` 为时间文字），方向键每次 5 秒（音量每次 5%）、PageUp / PageDown 每次 10%、Home / End 到首尾；拖动只更新预览，松开或键盘调整结束时提交一次跳转。
- 加载、失败状态在画面区域居中显示进度指示或错误图标与文字。

### 5.15 Query

- 首次加载：居中的进度指示与 `loading` 文案；刷新：保留数据，在其上方显示一行进度与文案；失败：无数据时居中的错误图标、文案与“重试”，有数据时为数据上方的一行错误与“重试”；空结果：居中的 32 px 图标（默认 `tray`）与 `empty` 文案。

### 5.16 Adaptive 与 Layout

- `Adaptive` 是填满可用宽度的块容器，用 `ResizeObserver` 量自己的宽度：小于 `[layouts].compact_below` 时挂载 `compact` 布局，否则挂载 `expanded` 布局。容器未渲染（宽度不可知）时保留上一个尺寸。
- 切换只重建布局内容；会话草稿与引擎状态不在视图树中，不受影响。切换前焦点所在控件的 `data-bern-target` 在新布局中找到同名控件则把焦点移过去；同名控件在未打开的菜单里时，焦点移到该菜单的按钮。
- 在卡片（Section）中，布局的每个子元素各占一行，与卡片的其他行一样有分隔线。
- 带 `id` 时登记为 `adaptive` 角色，值为当前尺寸。

### 5.17 Menu 与 MenuItem

- 遵循 WAI-ARIA APG 的 Menu Button 模式：触发器是 standard 按钮（图标、标签、尾端 `chevron.down`），`aria-haspopup="menu"`；菜单是浏览器顶层中的弹出层（Popover API，`role="menu"`），外观与下拉菜单相同（`menu` 配方），不被卡片、对话框与滚动容器裁切。
- 菜单与弹出面板从触发按钮处打开：以按钮一侧为原点，从 0.9 倍大小沿 bouncy 弹簧（周期 0.5 s、阻尼 0.62）放大到 1，同时在 120 ms 内淡入；触发按钮在页面右半边时，弹出层的尾端对齐按钮的尾端，向左展开。减少动态效果时只做 150 ms 淡入。
- 菜单项是 `role="menuitem"` 的按钮，高 32 px，图标 16 px 在前；`role: destructive` 用 `status.error` 文字；`Divider` 为 1 px `separator`。悬停与键盘焦点为 `hover.overlay` 底色。
- 键盘：↓、Enter、Space 打开并聚焦第一项，↑ 打开并聚焦最后一项；菜单内 ↑ ↓ 循环移动，Home / End 到首尾，输入字母跳到以其开头的项；Enter / Space 执行并关闭，焦点回到按钮；Esc 关闭（不关闭外层对话框）并回到按钮；Tab 关闭。
- 每个菜单项按自己的 `id` 登记为按钮控件，外部入口的 `ui.activate` 在菜单未打开时也走同一个命令闭包。

### 5.18 LanguagePicker

- 外观与行为同下拉菜单（5.13），选项是支持的语言，每项用该语言自己的名称（`Intl.DisplayNames`，如“简体中文”“English”）。
- 选择后整页的本地化文本在同一次更新中切换，`<html lang>` 与页面标题随之改变；窗口、对话框、草稿、导航与焦点保持。

## 6. 状态与焦点

- **焦点环。** 仅 `:focus-visible`：2 px `focus.indicator` 外框，外扩 2 px，圆角随控件；卡片内的控件焦点环内收，避免被裁切。焦点元素滚动到可见区域，不被应用栏或页面头遮挡。
- **悬停。** 只在 `(hover: hover)` 下应用。
- **按下。** `:active` 应用配方的 pressed 状态。
- **组合状态。** 选中且悬停保持选中颜色；错误且聚焦时边界保持错误色，焦点环照常显示；禁用阻止操作但保留选中与错误语义。

## 7. 动效

- 颜色、底色与边框变化 150 ms `ease-out`；对话框打开 180 ms 透明度与 0.98→1 缩放，遮罩淡入；菜单与弹出面板从触发按钮弹出（见 5.17）；推入导航不做位移动画。
- `prefers-reduced-motion: reduce` 时只保留不超过 150 ms 的透明度变化。

## 8. 图标

- Bern 图标集采用 SF Symbols 命名。Web 后端用 [Lucide](https://lucide.dev)（`lucide-static` v1.54.0）绘制：每个名字对应一个 Lucide 图标，对照表在 `bern-backend-web` 中；所用的 Lucide SVG 原样保存在仓库里，构建时不联网获取。
- 外观即 Lucide 的设计规范：24 × 24 画布、2 px 居中描边、圆头与圆角连接，颜色为 `currentColor`，跟随所在控件的配方。
- 实心名称（以 `.fill` 结尾）：Lucide 官方不提供实心版，Bern 在线条版上加填充并保留 2 px 描边，使实心版与线条版轮廓一致。每个实心名称在对照表中标明填充方式：
  - **整体填充**：只有闭合形状的图标，如 `play.fill`、`pause.fill`。
  - **部分填充**：只填指定的形状，其余保留线条，如 `speaker.wave.3.fill` 只填喇叭，声波仍是线条。
  - **镂空**：填满外形，再把内部的线条从中挖空，如 `xmark.circle.fill` 是实心圆中挖出的叉号。
- 应用用到对照表中没有的名字时构建失败并指向该声明。彩色图像与品牌图形属于内容，不参与主题着色。
- 授权：Lucide 为 ISC 授权，其中源自 Feather 的图标为 MIT 授权，两者都要求随副本保留版权与授权声明。生成的页面目录包含 `THIRD_PARTY_NOTICES.txt`，收录两份授权全文；加了填充的图标是基于 Lucide 的修改版，不作为官方 Lucide 图标发布。

## 9. 可访问性与本地化

- 根元素 `lang` 为解析后的语言。
- 角色映射：Menu → `button`（`aria-haspopup="menu"`）+ `menu` / `menuitem`；LanguagePicker → `combobox` + `listbox`；Button → `button`；IconButton → `button` + `aria-label`；Switch → `switch` / `checkbox`；Slider → 原生 range；媒体进度条 → `slider`；Picker automatic / menu → `combobox` + `listbox` / `option`；Picker segmented / radio / chips → `radiogroup` + `radio`；Tabs → `tablist` / `tab` / `tabpanel`；Sidebar → `listbox` / `option`（`aria-selected`）；NavLink → `link`；对话框 → `dialog`（`aria-modal`、`aria-labelledby`）；浮动窗口 → `dialog`（非模态）；关闭确认 → `alertdialog`；Query 的进度与错误行 → `status` / `alert`。
- 控件身份写入 `data-bern-target`，与 `ui.inspect` 的目标 ID 一致。

## 10. Bern 验收

- 浅色、深色、提高对比、强制颜色四种环境下，每个组件的全部适用状态都有截图检查；受控配对由 `bern-theme` 测试证明。
- 页面中没有桌面窗口的外观：主窗口无标题栏与窗口按钮，设置窗口为带标题与关闭按钮的对话框；浮动窗口只在声明 `presentation: floating` 时出现。
- 至少三种品牌种子下重复配对检查，并目视检查主要按钮、选中与焦点。
- 键盘可以完成侧边栏、页签、表单、下拉菜单、媒体进度条、对话框（打开、切换、Esc 关闭、焦点返回）与关闭确认的全部操作；输入法组合期间不提交写入。
- 下拉菜单在卡片、对话框与滚动容器中弹出时不被裁切；视频进度条的已播放部分不使用强调色。
- `pointer: coarse` 下所有可点目标不小于 44 × 44 px。
- 宽屏与窄屏之间切换时，窗口实例、草稿、导航与焦点保持。
