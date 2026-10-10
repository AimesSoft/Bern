# Bern Web 视觉规范

- 版本：v0.2
- 官方资料核查日期：2026-10-10，Asia/Shanghai
- 阶段：Web 后端的视觉与交互规范（[架构设计](../Design.md)“Web 应用与 WebUI”要求先单独成文）
- 关联：[Bern 架构设计 v0.13](../Design.md)、[自适应适配规范](README.md)

Web 没有统一的平台样式规范。本文是 Bern 为 Web 目标定义的视觉语言：**Bern 自有的中性网页风格**。它提炼各家网页设计系统与成熟网页应用共有的做法，风格接近 Linear、Vercel、GitHub 这类网页应用，但不复制 Material、Fluent 或任何一家公司的视觉语言，也不模仿桌面操作系统的窗口和控件。颜色全部来自 Bern 主题内核（`bern-theme`）。除“官方依据”一节引用的 W3C 规范外，文中规则都是 Bern 的设计决策。

## 1. 原则

- **页面就是应用。** 主窗口不画窗口外框、标题栏或窗口按钮；网页没有“窗口”这个概念，次级界面用对话框或侧边面板呈现。可拖动的浮动窗口只在应用显式声明 `presentation: floating` 时出现。
- **安静的表面，清楚的层级。** 页面为纯色画布，内容分组用细边框卡片；颜色只用于主要操作、选中、焦点与状态，不用大面积品牌色或装饰性渐变。
- **网页的密度与尺度。** 正文 14 px，控件 36 px；粗略指针下控件与点按区域整体放大。
- **一套配色算法。** 生成的 CSS 只引用 `--bern-*` 变量，组件样式中没有字面颜色；浅色、深色、提高对比由同一算法生成，应用不写任何深浅色分支。
- **浏览器托管部分不伪装。** `<select>` 的弹出列表、滚动条、文本选区由浏览器绘制，Bern 通过 `color-scheme` 让它们与当前模式一致。
- **语义不只靠颜色。** 选中、错误、禁用同时保留文字、图标、形状或可访问状态。

## 2. 官方依据

| 依据 | 用于 |
| --- | --- |
| [WCAG 2.2](https://www.w3.org/TR/WCAG22/)：1.4.3 对比度（最低）AA，正文 4.5:1、大字号 3:1 | 文字与背景的受控配对 |
| WCAG 2.2：1.4.11 非文本对比 AA，与相邻颜色 3:1 | 识别控件所必需的边界、滑轨、开关、焦点指示 |
| WCAG 2.2：2.4.7 焦点可见 AA；2.4.11 焦点不被遮挡（最低）AA；2.4.13 焦点外观 AAA（至少 2 CSS px 周长、焦点前后 3:1） | 焦点环；Bern 以 AAA 的焦点外观为目标 |
| WCAG 2.2：2.5.8 目标尺寸（最低）AA，指针目标至少 24 × 24 CSS px | 图标按钮与紧凑控件的热区 |
| [Media Queries Level 5](https://www.w3.org/TR/mediaqueries-5/)（工作草案，2026-02-19）：`prefers-color-scheme`、`prefers-contrast`、`forced-colors`、`prefers-reduced-motion` | 外观、对比、强制颜色与动效偏好 |
| [WAI-ARIA APG 模式](https://www.w3.org/WAI/ARIA/apg/patterns/)：Button、Checkbox、Switch、Slider、Tabs、Listbox、Radio Group、Toolbar、Dialog (Modal)、Window Splitter | 各组件的角色、状态与键盘交互 |

## 3. 度量

### 3.1 网格与圆角

- 间距以 4 px 为单位：控件内 8–12，行内 16，卡片之间 24，页面边距 24（窄屏 16），分区之间 32。
- 圆角：控件 6 px；卡片、菜单、下拉 8 px；对话框与浮动窗口 12 px；开关与胶囊形标记全圆角。
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
| 按钮、输入框、选择器高度 | 36 px（工具栏与侧边栏内 32 px） | 44 px |
| 图标按钮热区 | 32 × 32 px（图标 16 px） | 44 × 44 px（图标 20 px） |
| 卡片内的行最小高度 | 52 px | 60 px |
| 侧边栏导航项高度 | 32 px | 44 px |
| 开关 | 轨道 36 × 20，圆钮 16 | 轨道 44 × 26，圆钮 22 |
| 滑块 | 轨道高 4，圆钮 16 | 轨道高 6，圆钮 24 |

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
| `accent`、`on_accent` | 主要按钮、开关开启；其上文字图标 | `on_accent`/`accent` ≥ 4.5:1；`accent` 对画布与面板 ≥ 3:1 |
| `accent.text` | 链接与平直按钮的文字 | 对各表面 ≥ 4.5:1 |
| `status.success`、`status.warning`、`status.error` 及 `on_*` | 状态前景与状态底色上的前景 | 前景对各表面 ≥ 4.5:1；`on_error`/`status.error.surface` ≥ 4.5:1 |
| `control.border` | 输入框、复选框等靠边界识别的控件 | 对所在表面 ≥ 3:1 |
| `separator` | 卡片边框、分隔线、带文字按钮的轮廓（装饰性） | 不检查对比，分类为装饰 |
| `control.track`、`control.thumb` | 开关关闭轨道、滑轨；圆钮 | track 对表面 ≥ 3:1；thumb 对 track 与 accent ≥ 3:1 |
| `focus.indicator` | 焦点环 | 对各表面 ≥ 3:1 |
| `selection.background`、`selection.foreground`、`selection.indicator` | 侧边栏与列表的选中项：浅色强调底、文字、左侧指示条 | foreground/background ≥ 4.5:1；indicator 对 background 与面板 ≥ 3:1 |
| `hover.overlay`、`pressed.overlay` | 行、导航项与平直控件的悬停、按下底色（不透明） | 其上的文字仍满足各自配对 |
| `disabled.foreground`、`disabled.surface` | 禁用文字与禁用底色 | 不可用控件单独分类：foreground/surface ≥ 3:1 |
| `scrim` | 对话框遮罩 | 遮罩上的对话框为不透明 raised 表面 |

### 4.3 控件配方

| 配方 | 状态 | 底色 / 前景 / 边界 |
| --- | --- | --- |
| `button.standard` | rest、hover、pressed、focus、disabled | canvas / text.primary / separator；悬停、按下为求解后的底色 |
| `button.prominent` | 同上 | accent / on_accent；悬停、按下为 accent 的明度变体，仍满足 on_accent 配对 |
| `button.plain` | 同上 | 透明 / accent.text；悬停 hover.overlay |
| `button.destructive` | 同上 | status.error.surface / on_error；standard 与 plain 的危险角色改用 status.error 文字 |
| `icon_button` | rest、hover、pressed、selected、focus、disabled | **所有状态无可见底色或边框**；前景 icon.secondary → icon.primary（悬停）→ accent.text（按下、选中） |
| `field` | rest、hover、focus、error、disabled | raised / text.primary / control.border；聚焦时边界 accent 并显示焦点环；错误时边界与说明 status.error |
| `switch` | off、on、focus、disabled | 轨道 control.track / accent；圆钮 control.thumb |
| `slider` | rest、active、focus、disabled | 已填充段 accent，未填充段 control.track，圆钮 control.thumb 加 1 px control.border |
| `segmented` | rest、selected、hover、focus、disabled | 外槽 panel；选中段 canvas + text.primary + separator；未选中 text.secondary |
| `nav_item` | rest、hover、selected、focus | 透明 / text.secondary；悬停 hover.overlay + text.primary；选中 selection.background + selection.foreground + 左侧 2 px selection.indicator |
| `row` | rest、hover、pressed、focus | 透明 / text.primary；悬停 hover.overlay |
| `tab` | rest、selected、hover、focus | 透明 / text.secondary；选中 text.primary + 2 px accent 下划线 |
| `dialog` | — | raised / text.primary / separator；遮罩 scrim |

## 5. 组件

组件语义以 [`bern-ir` 组件规范](../../crates/bern-ir/src/spec.rs) 为准；下文规定 Web 呈现、角色与键盘行为。

### 5.1 窗口：WindowSet 与 Window

- **应用栏。** 页面顶部是 48 px 的应用栏（`surface.canvas`，下边框 `separator`）：前端为应用名（`headline`）；尾端为设置按钮（`gearshape` 图标按钮），在应用声明了 `role: settings` 的窗口时出现，Ctrl / Cmd + , 有同样效果。只有存在 `presentation: floating` 的窗口时，应用栏中间才出现窗口切换条。
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
- SidebarItem 用 `nav_item` 配方：高 32 px、左右内边距 10 px、6 px 圆角；图标 16 px；选中项浅色强调底、文字加粗到 500，左侧 2 px 指示条。Listbox 模式：上下方向键移动并选中，Home / End 到首尾。
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

- 默认为顶部下划线页签（Tabs 模式，`role="tablist"`）：标签 14 px，有图标时图标 16 px 在标签前；选中项 `text.primary` 加 2 px `accent` 下划线，其他 `text.secondary`；页签栏下边框 `separator`。左右方向键切换并激活。
- 对话框根部的 Tabs 在宽屏为左侧竖排导航，用 `nav_item` 配方，上下方向键切换。

### 5.6 Toolbar

- 位于页面头或对话框头的尾端，项间距 8 px，高 32 px。Button 用 standard 外观（`prominence: prominent` 保持主要按钮），Picker 用 segmented 外观，IconButton 无容器。Toolbar 模式：方向键在项之间移动焦点。

### 5.7 List 与 ForEach

- List 在 Section 内为卡片中的行；独立使用时为带边框卡片。行身份按 `key` 保持，插入、删除、重排不重建未变化的行。

### 5.8 Text、Label、Divider、Spacer、ScrollView、Column、Row

- Text、Label 按 3.2 的字号；Label 的图标 16 px，图标与文字间距 8 px，图标用 `icon.secondary`。
- Divider：1 px `separator`。ScrollView：原生滚动，`overscroll-behavior: contain`。Column、Row 为 flex 布局，`gap`、`padding` 以 px 计。

### 5.9 Button 与 IconButton

- Button：`<button>`，高 36 px，左右内边距 14 px，圆角 6 px，14 px 字重 500；有图标时图标 16 px 在文字前，间距 6 px。外观由 `prominence` 与 `role` 选择 4.3 的配方；`cancel` 角色用 standard。
- IconButton：`<button aria-label>`，热区 32 × 32 px，只有图标，**任何状态都不出现可见底板、边框或波纹**；反馈只作用于图标颜色，焦点环属于焦点反馈。
- 禁用：`disabled` 属性与配方的禁用颜色；不用整体降低不透明度表示禁用。

### 5.10 TextInput 与 Search

- TextInput：`<input>`（`secure` 为 `type="password"`，`multiline` 为至少 3 行的 `<textarea>`），`field` 配方，高 36 px，标签在上方（14 px 字重 500，与输入框间距 6 px）；提示文字用 `text.placeholder`。
- 输入法组合期间只更新本地显示，组合结束后一次性写入会话。
- Search：前置 `magnifyingglass` 图标，尾端清除按钮（有内容时出现）；Enter 运行声明的提交动作，Esc 清空。

### 5.11 Switch

- `style: switch`：`<button role="switch" aria-checked>`，`switch` 配方；圆钮移动 150 ms。Space / Enter 切换。
- `style: checkbox`：16 px 方框、4 px 圆角，选中时 `accent` 底与 `on_accent` 对勾。

### 5.12 Slider

- `<input type="range">`，以 `slider` 配方自绘轨道与圆钮；`min_icon` / `max_icon` 在两端（16 px，`icon.secondary`）。
- 拖动时只更新会话预览；松开或键盘调整结束时运行声明的提交动作。

### 5.13 Picker

- `automatic`、`menu`：原生 `<select>`，外观为 `field` 配方加尾端 `chevron.up.chevron.down`，最小宽 160 px；弹出列表由浏览器托管。
- `segmented`：Radio Group 模式的分段控件，`segmented` 配方，选中段带细微投影。
- `radio`、`inline`：纵向单选列表，每项 16 px 圆形指示。

### 5.14 Video

- `<video>` 元素，`object-fit` 由 `fit` 决定，画面保持原色。**关闭浏览器原生 `controls`**，使用 Bern 标准控制条，置于画面下方的不透明条上（高 44 px，`surface.panel`），不叠在动态画面上。
- 控制条：播放 / 暂停、当前时间 / 时长（`tabular-nums`）、进度滑块、静音、音量滑块。时长未知显示 `--:--`，直播显示“直播”且进度禁用。
- 加载、失败状态在画面区域居中显示进度指示或错误图标与文字。

### 5.15 Query

- 首次加载：居中的进度指示与 `loading` 文案；刷新：保留数据，在其上方显示一行进度与文案；失败：无数据时居中的错误图标、文案与“重试”，有数据时为数据上方的一行错误与“重试”；空结果：居中的 32 px 图标（默认 `tray`）与 `empty` 文案。

## 6. 状态与焦点

- **焦点环。** 仅 `:focus-visible`：2 px `focus.indicator` 外框，外扩 2 px，圆角随控件；卡片内的控件焦点环内收，避免被裁切。焦点元素滚动到可见区域，不被应用栏或页面头遮挡。
- **悬停。** 只在 `(hover: hover)` 下应用。
- **按下。** `:active` 应用配方的 pressed 状态。
- **组合状态。** 选中且悬停保持选中颜色；错误且聚焦时边界保持错误色，焦点环照常显示；禁用阻止操作但保留选中与错误语义。

## 7. 动效

- 颜色、底色与边框变化 150 ms `ease-out`；对话框打开 180 ms 透明度与 0.98→1 缩放，遮罩淡入；推入导航不做位移动画。
- `prefers-reduced-motion: reduce` 时只保留不超过 150 ms 的透明度变化。

## 8. 图标

- Bern 图标集采用 SF Symbols 命名。Web 后端为每个名字提供 Bern 自绘的 24 × 24 viewBox 描边 SVG（1.75 px 圆头，`currentColor`）。
- 应用用到 Web 图标表中没有的名字时构建失败并指向该声明。彩色图像与品牌图形属于内容，不参与主题着色。

## 9. 可访问性与本地化

- 根元素 `lang` 为解析后的语言。
- 角色映射：Button → `button`；IconButton → `button` + `aria-label`；Switch → `switch` / `checkbox`；Slider → 原生 range；Picker segmented / radio → `radiogroup` + `radio`；Tabs → `tablist` / `tab` / `tabpanel`；Sidebar → `listbox` / `option`（`aria-selected`）；NavLink → `link`；对话框 → `dialog`（`aria-modal`、`aria-labelledby`）；浮动窗口 → `dialog`（非模态）；关闭确认 → `alertdialog`；Query 的进度与错误行 → `status` / `alert`。
- 控件身份写入 `data-bern-target`，与 `ui.inspect` 的目标 ID 一致。

## 10. Bern 验收

- 浅色、深色、提高对比、强制颜色四种环境下，每个组件的全部适用状态都有截图检查；受控配对由 `bern-theme` 测试证明。
- 页面中没有桌面窗口的外观：主窗口无标题栏与窗口按钮，设置窗口为带标题与关闭按钮的对话框；浮动窗口只在声明 `presentation: floating` 时出现。
- 至少三种品牌种子下重复配对检查，并目视检查主要按钮、选中与焦点；图标按钮在全部状态下都没有可见容器。
- 键盘可以完成侧边栏、页签、表单、对话框（打开、切换、Esc 关闭、焦点返回）与关闭确认的全部操作；输入法组合期间不提交写入。
- `pointer: coarse` 下所有可点目标不小于 44 × 44 px。
- 宽屏与窄屏之间切换时，窗口实例、草稿、导航与焦点保持。
