# Bern Web 视觉规范

- 版本：v0.1
- 官方资料核查日期：2026-10-10，Asia/Shanghai
- 阶段：Web 后端实现前的视觉与交互规范（[架构设计](../Design.md)“Web 应用与 WebUI”要求先单独成文）
- 关联：[Bern 架构设计 v0.12](../Design.md)、[自适应适配规范](README.md)

Web 没有统一的平台样式规范。本文是 Bern 为 Web 目标自行定义的视觉语言，颜色全部来自 Bern 主题内核（`bern-theme`）。除“官方依据”一节引用的 W3C 规范外，文中规则都是 Bern 的设计决策，不是浏览器或 W3C 的规定。

## 1. 原则

- **内容优先、表面安静。** 页面、分组和浮层用明度层级区分，不用装饰性渐变、阴影堆叠或大面积品牌色。强调色只用于主要操作、选中与焦点。
- **一套配色算法。** 所有颜色来自主题快照的语义角色；生成的 CSS 只引用 `--bern-*` 变量，组件样式中没有字面颜色。浅色、深色和提高对比由同一算法生成，应用不写任何深浅色分支。
- **桌面密度，触摸可用。** 默认按精确指针的桌面密度排版；`pointer: coarse` 时控件与点按区域整体放大，满足最小目标尺寸。
- **浏览器托管部分不伪装。** `<select>` 的弹出列表、滚动条、文本选区等由浏览器绘制；Bern 通过 `color-scheme` 让它们与当前模式一致，不用脚本重画一份。
- **语义不只靠颜色。** 选中、错误、禁用同时保留文字、图标、形状或可访问状态。

## 2. 官方依据

| 依据 | 用于 |
| --- | --- |
| [WCAG 2.2](https://www.w3.org/TR/WCAG22/)：1.4.3 对比度（最低）AA，正文 4.5:1、大字号 3:1 | 文字与背景的受控配对 |
| WCAG 2.2：1.4.11 非文本对比 AA，与相邻颜色 3:1 | 控件边界、滑轨、开关、焦点指示 |
| WCAG 2.2：2.4.7 焦点可见 AA；2.4.11 焦点不被遮挡（最低）AA；2.4.13 焦点外观 AAA（至少 2 CSS px 周长、焦点前后 3:1） | 焦点环设计；Bern 以 AAA 的焦点外观为目标 |
| WCAG 2.2：2.5.8 目标尺寸（最低）AA，指针目标至少 24 × 24 CSS px（有间距、等效、行内、浏览器控件等例外） | 图标按钮与紧凑控件的热区 |
| [Media Queries Level 5](https://www.w3.org/TR/mediaqueries-5/)（工作草案，2026-02-19）：`prefers-color-scheme`（light、dark）、`prefers-contrast`（no-preference、less、more、custom）、`forced-colors`（none、active）、`prefers-reduced-motion`（no-preference、reduce） | 外观、对比、强制颜色与动效偏好 |
| [WAI-ARIA APG 模式](https://www.w3.org/WAI/ARIA/apg/patterns/)：Button、Checkbox、Switch、Slider、Tabs、Listbox、Radio Group、Toolbar、Dialog (Modal)、Window Splitter | 各组件的角色、状态与键盘交互 |

## 3. 度量

### 3.1 网格与圆角

- 间距以 4 px 为单位；组件内边距与间隔取 4、8、12、16、20、24。
- 圆角：控件 6 px；分组、列表行容器与菜单 10 px；虚拟窗口 12 px；开关轨道与胶囊形标签用全圆角。
- 分隔线 1 px，使用 `control.border` 的低强度变体 `separator`（见 4.2）。

### 3.2 字体与字号

字体栈：`system-ui, -apple-system, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans", "Noto Sans CJK SC", sans-serif`；等宽数字用 `font-variant-numeric: tabular-nums`（时间、计数）。

| `Text.style` | 字号 / 行高（px） | 字重 |
| --- | --- | --- |
| `large_title` | 26 / 32 | 700 |
| `title` | 22 / 28 | 700 |
| `title2` | 17 / 22 | 600 |
| `title3` | 15 / 20 | 600 |
| `headline` | 13 / 18 | 600 |
| `body`（默认） | 13 / 18 | 400 |
| `callout` | 12 / 16 | 400 |
| `subheadline` | 11 / 14 | 400 |
| `footnote` | 11 / 14 | 400 |
| `caption` | 10 / 13 | 400 |
| `caption2` | 10 / 13 | 500 |

`pointer: coarse` 时 `body` 及以下各级增加 2 px，标题各级增加 2 px。`emphasis: secondary` 用 `text.secondary`；`tertiary` 用 `text.placeholder`。两者都要通过 4.5:1，不因“弱化”免检。

### 3.3 控件尺寸

| 项目 | 精确指针 | 粗略指针（`pointer: coarse`） |
| --- | --- | --- |
| 按钮、输入框、选择器高度 | 28 px | 40 px |
| 图标按钮热区 | 28 × 28 px（图标 16 px） | 44 × 44 px（图标 20 px） |
| 表单行最小高度 | 36 px | 48 px |
| 侧边栏行高 | 28 px | 44 px |
| 开关 | 轨道 34 × 20，圆钮 16 | 轨道 44 × 26，圆钮 22 |
| 滑块 | 轨道高 4，圆钮 16 | 轨道高 6，圆钮 24 |

精确指针下的最小热区 28 px 已超过 WCAG 2.5.8 的 24 px。

## 4. 颜色

### 4.1 主题快照到 CSS

- 构建时由 `bern-theme` 生成 `light`、`dark` 各自的标准对比与提高对比共四份快照，写成 `:root` 上的 `--bern-<角色>` 变量，并按 `prefers-color-scheme` 与 `prefers-contrast: more` 选择。首帧即为正确模式，不先显示错误配色。
- `[theme] mode = "light"` 或 `"dark"` 时只输出对应模式；`system` 跟随浏览器偏好，偏好变化时由 CSS 在同一样式计算中整体切换，不重建 DOM，不丢失焦点、输入法组合与编辑内容。
- `:root` 声明 `color-scheme`，与当前模式一致，使浏览器托管部分（滚动条、`<select>` 弹出列表、自动填充）同步深浅。
- `forced-colors: active` 时，Bern 不再强制自己的颜色：背景、文字、边框改用系统颜色关键字（`Canvas`、`CanvasText`、`ButtonFace`、`ButtonText`、`Highlight`、`HighlightText`、`GrayText`），焦点与选中改用轮廓而非底色，确保在强制颜色下仍可辨识。

### 4.2 角色

使用设计文档的角色组，并为 Web 补充少量派生角色。每个角色都声明配对关系，求解时只保证声明的配对。

| 角色 | 用途 | 声明的配对 |
| --- | --- | --- |
| `surface.canvas` | 页面与窗口内容背景 | — |
| `surface.panel` | 侧边栏、表单分组、列表容器 | — |
| `surface.raised` | 虚拟窗口、菜单、浮层、标准按钮底色、输入框底色 | — |
| `text.primary`、`text.secondary`、`text.placeholder` | 正文、次要说明、输入提示 | 对 canvas、panel、raised 各 ≥ 4.5:1 |
| `icon.primary`、`icon.secondary` | 操作图标、辅助图标 | 对 canvas、panel、raised ≥ 3:1 |
| `accent`、`on_accent` | 主要按钮、开关开启、选中指示；其上文字图标 | `on_accent`/`accent` ≥ 4.5:1；`accent` 对 canvas、panel ≥ 3:1 |
| `accent.text` | 平直按钮、链接式文字 | 对 canvas、panel、raised ≥ 4.5:1 |
| `status.success`、`status.warning`、`status.error` 及 `on_*` | 状态前景（文字、图标）与状态底色上的前景 | 前景对 canvas、panel ≥ 4.5:1；`on_error`/`status.error.surface` ≥ 4.5:1 |
| `control.border` | 输入框、标准按钮边界 | 对其所在表面 ≥ 3:1 |
| `separator` | 分组内分隔线（装饰性，不承担辨识） | 不检查对比，分类为装饰 |
| `control.track`、`control.thumb` | 开关关闭轨道、滑轨；圆钮 | track 对所在表面 ≥ 3:1；thumb 对 track 与 accent ≥ 3:1 |
| `focus.indicator` | 焦点环 | 对 canvas、panel、raised ≥ 3:1 |
| `selection.background`、`selection.foreground` | 侧边栏与列表选中行 | foreground/background ≥ 4.5:1 |
| `hover.overlay`、`pressed.overlay` | 行与平直控件的悬停、按下底色（不透明，按表面求解） | 叠加后文字仍满足其配对 |
| `disabled.foreground`、`disabled.surface` | 禁用文字与禁用底色 | 不可用控件单独分类：foreground/surface ≥ 3:1，保证仍可辨认，不要求操作对比 |
| `scrim` | 模态遮罩 | 遮罩上的对话框为不透明 raised 表面 |

### 4.3 控件配方

每个控件的颜色由配方声明“状态 → 角色”。配方由 `bern-theme` 求解并随快照输出；Web CSS 只按状态选择变量。

| 配方 | 状态 | 底色 / 前景 / 边界 |
| --- | --- | --- |
| `button.standard` | rest、hover、pressed、focus、disabled | raised / text.primary / control.border；悬停与按下用求解后的更深 raised |
| `button.prominent` | 同上 | accent / on_accent；悬停、按下为 accent 的明度变体，仍满足 on_accent 配对 |
| `button.plain` | 同上 | 透明 / accent.text；悬停用 hover.overlay |
| `button.destructive` | 同上 | standard 与 plain 的前景改为 status.error；prominent 改为 status.error.surface / on_error |
| `icon_button` | rest、hover、pressed、selected、focus、disabled | **所有状态无可见底色或边框**；前景 icon.secondary → icon.primary（悬停）→ accent.text（按下、选中）；禁用 disabled.foreground |
| `field` | rest、hover、focus、error、disabled | raised / text.primary / control.border；聚焦时边界为 accent；错误时边界与说明为 status.error |
| `switch` | off、on、focus、disabled | 轨道 control.track / accent；圆钮 control.thumb |
| `slider` | rest、active、focus、disabled | 已填充段 accent，未填充段 control.track，圆钮 control.thumb 加 1 px control.border |
| `segmented` | rest、selected、hover、focus、disabled | 外槽 panel；选中段 raised + text.primary；未选中 text.secondary |
| `row` | rest、hover、selected、focus | 透明 / text.primary；悬停 hover.overlay；选中 selection.background / selection.foreground |
| `tab` | rest、selected、hover、focus | 透明 / text.secondary；选中 accent.text 与 hover.overlay 胶囊 |
| `window.chrome` | active、inactive | 标题栏 raised；标题 text.primary（非活动 text.secondary） |

## 5. 组件

组件语义以 [`bern-ir` 组件规范](../../crates/bern-ir/src/spec.rs) 为准；下表规定 Web 呈现、角色与键盘行为。

### 5.1 窗口：WindowSet 与 Window

- **页面宿主。** `WindowSet` 占满视口，是虚拟窗口的呈现区域，背景为 `surface.host`。浏览器窗口本身不是 Bern 窗口。
- **应用栏。** 宿主顶部是 40 px 的应用栏（`surface.raised`）：前端为应用名；中间是窗口切换条，列出每个非设置窗口种类，已关闭的种类以 `text.placeholder` 显示，点按即重新打开，因此主窗口关闭后页面仍保留打开入口；打开中的设置窗口也出现在切换条里。尾端是设置按钮（`gearshape` 图标按钮），声明了 `role: settings` 的窗口时出现，Ctrl / Cmd + , 有同样效果。页面没有应用菜单，这一栏承担 macOS 菜单栏中“设置…”与“窗口”菜单的作用。切换条遵循 Toolbar 模式，当前窗口以 `hover.overlay` 胶囊标出，最小化的窗口以斜体显示并在此恢复。
- **宽屏（呈现区域宽度 ≥ 960 px）。** 窗口是可拖动、可调整大小的浮动面板：`surface.canvas`、12 px 圆角、柔和投影（深色模式改用 1 px `control.border` 描边代替投影）。
  - 主窗口默认以“最大化”打开，即占满呈现区域（无圆角与投影）；可恢复为声明尺寸的浮动窗口。
  - 其他窗口以声明尺寸在呈现区域中居中打开；`role: settings` 的窗口宽度固定、高度贴合内容，不可调整大小、最小化或最大化，只能移动。
  - 标题栏高 36 px：标题居中（`headline`）；尾端为最小化（`minus`）、最大化 / 恢复（`square` / `square.on.square`）、关闭（`xmark`）图标按钮，均遵循无容器规则；设置窗口只有关闭。双击标题栏切换最大化。
  - 调整尺寸的热区为四边 6 px、四角 12 px；拖动只改变预览，松开时提交一次 `window.move` 或 `window.resize`。键盘：聚焦标题栏后方向键移动 8 px，Shift + 方向键调整尺寸。
  - 在窗口任意位置按下指针即激活并置顶该窗口。
- **窄屏（呈现区域宽度 < 960 px）。** 窗口占满呈现区域，不显示浮动边框与调整尺寸的热区；通过应用栏的切换条在窗口间切换。
- **模态。** 关闭确认以窗口内模态对话框呈现（Dialog (Modal) 模式）：只遮住所属窗口（`scrim` 覆盖该窗口），焦点限制在对话框内，Esc 等同“取消”。命令失败时，同样以窗口内对话框显示原因。
- **呈现事实。** 虚拟窗口的呈现方式为 `Virtual`；可见性、尺寸状态、相对呈现区域的外框、焦点（当前窗口且页面拥有焦点）与可用动作，按会话运行时的窗口事实报告；页面隐藏时为隐藏。

### 5.2 SplitView、Sidebar、Detail

- 侧边栏宽 240 px（可拖动 200–320 px，Window Splitter 模式），`surface.panel`；详情栏 `surface.canvas`。
- 侧边栏顶部为 Search；Section 标题为 `subheadline`、`text.secondary`、全大写不适用于中文，直接显示原文。
- SidebarItem：行高 28 px、左右内边距 8 px、6 px 圆角；图标 16 px（`icon.secondary`，选中时随 `selection.foreground`）。选中行 `selection.background`。Listbox 模式：上下方向键移动并选中，Home / End 到首尾。
- 详情栏顶部是页面标题栏：高 52 px，标题为所选项的文字（`title3`），工具栏项位于尾端。
- 所在窗口内容宽度 < 720 px 时（容器查询），侧边栏成为第一页，选中项后显示详情，详情标题栏前端出现返回按钮（`chevron.left`）。

### 5.3 Form 与 Section

- Form 内容最大宽 680 px，水平居中，上下留 20 px；有 `title` 时它作为页面标题。
- Section：标题在分组上方（`subheadline`、`text.secondary`，左缩进 12 px）；分组为 `surface.panel`、10 px 圆角；行之间 1 px `separator`，左侧缩进 12 px；脚注在分组下方（`footnote`、`text.secondary`）。
- 行布局：标签在前端，控件在尾端；多行文本输入、滑块独占一行，标签在上。

### 5.4 Navigation、Destination、NavLink

- Navigation 是推入栈；推入的页面替换当前内容区，页面标题栏前端显示返回按钮，Esc 或 Alt + ← 返回。
- NavLink 是整行可点的链接行：内容在前，尾端 `chevron.right`（`icon.secondary`）；悬停 `hover.overlay`；Enter / Space 推入。

### 5.5 Tabs 与 Tab

- 标签栏位于所属内容顶部、水平居中（Tabs 模式，`role="tablist"`）。有图标时图标 20 px 在上、标签 `caption2` 在下，单项最小宽 64 px；无图标时为胶囊形文字标签。
- 选中项 `accent.text` 加 `hover.overlay` 胶囊底；左右方向键切换并激活。

### 5.6 Toolbar

- 位于页面或窗口标题栏尾端，项间距 8 px。Button 在工具栏中使用 plain 外观；Picker 使用 segmented 外观；IconButton 无容器。Toolbar 模式：方向键在项之间移动焦点。

### 5.7 List 与 ForEach

- List 在 Form / Section 内为分组行；独立使用时为 `surface.canvas` 上的平铺行，行间 `separator`。行身份按 `key` 保持，插入、删除、重排不重建未变化的行。

### 5.8 Text、Label、Divider、Spacer、ScrollView、Column、Row

- Text、Label 按 3.2 的字号；Label 的图标 16 px，图标与文字间距 6 px，图标用 `icon.primary`。
- Divider：1 px `separator`。ScrollView：原生滚动，`overscroll-behavior: contain`。Column、Row 为 flex 布局，`gap`、`padding` 以 px 计。

### 5.9 Button 与 IconButton

- Button：`<button>`，高 28 px，左右内边距 12 px，圆角 6 px，`body` 字号字重 500；有图标时图标 14 px 在文字前，间距 6 px。外观由 `prominence` 与 `role` 选择 4.3 的配方；`cancel` 角色使用 standard。
- IconButton：`<button aria-label>`，热区 28 × 28 px，只有图标，**任何状态都不出现可见底板、边框或波纹**；反馈只作用于图标颜色。焦点环见第 6 节，属于焦点反馈而非容器。
- 禁用：`disabled` 属性，配方的禁用颜色；不用整体降低不透明度表示禁用。

### 5.10 TextInput 与 Search

- TextInput：`<input>`（`secure` 为 `type="password"`，`multiline` 为至少 3 行的 `<textarea>`），`field` 配方；提示文字用 `text.placeholder`。
- 输入法组合期间（`compositionstart` 至 `compositionend`）只更新本地显示，不提交会话写入；组合结束后一次性写入。
- Search：带前置 `magnifyingglass` 图标与尾端清除按钮（`xmark.circle.fill`，有内容时出现），圆角 6 px。Enter 运行声明的提交动作。

### 5.11 Switch

- `style: switch`：`<button role="switch" aria-checked>`，`switch` 配方；圆钮移动 120 ms。Space / Enter 切换。
- `style: checkbox`：16 px 方框、4 px 圆角，选中时 `accent` 底与 `on_accent` 对勾。Checkbox 模式。

### 5.12 Slider

- `<input type="range">`，以 `appearance: none` 和 `slider` 配方自绘轨道与圆钮；`min_icon` / `max_icon` 在两端（16 px，`icon.secondary`）。
- 拖动时（`input` 事件）只更新会话预览；松开或键盘调整结束（`change` 事件）时运行声明的提交动作，与原生平台一致。

### 5.13 Picker

- `automatic`、`menu`：原生 `<select>`，外观为 `field` 配方加尾端 `chevron.up.chevron.down`；弹出列表由浏览器托管。
- `segmented`：Radio Group 模式的分段控件，`segmented` 配方，最小段宽 56 px。
- `radio`、`inline`：纵向单选列表，每项 16 px 圆形指示（选中 `accent` 实心圆点）。

### 5.14 Video

- `<video>` 元素，`object-fit` 由 `fit` 决定，画面保持原色，不加滤镜。**关闭浏览器原生 `controls`**，使用 Bern 标准控制条，控制条置于画面下方的不透明 `surface.raised` 条上（高 36 px），不叠在动态画面上，使对比可证明。
- 控制条：播放 / 暂停 IconButton、当前时间 / 时长（`tabular-nums`）、进度滑块、静音 IconButton、音量滑块。时长未知显示 `--:--`，直播显示“直播”且进度滑块禁用。
- 加载、失败状态在画面区域居中显示进度指示或错误图标与文字（`surface.panel` 底）。

### 5.15 Query

- 首次加载：居中的进度指示（16 px 旋转圆弧，`icon.secondary`）与 `loading` 文案。
- 刷新：保留数据，在其上方显示一行进度指示与文案。
- 失败：无数据时为居中的错误图标、文案与“重试”按钮；有数据时为数据上方的一行错误文字与“重试”。
- 空结果：居中的 32 px 图标（默认 `tray`，`icon.secondary`）与 `empty` 文案。

## 6. 状态与焦点

- **焦点环。** 仅 `:focus-visible` 显示：2 px `focus.indicator` 实线轮廓，外扩 2 px，圆角随控件；分组行内的控件焦点环内收，避免被裁切。满足 WCAG 2.4.13 的周长与 3:1 要求。焦点元素滚动到可见区域，不被固定标题栏或切换条遮挡（2.4.11）。
- **悬停。** 只在 `(hover: hover)` 下应用，避免触摸设备残留悬停态。
- **按下。** `:active` 应用配方的 pressed 状态。
- **组合状态。** 选中且悬停取选中颜色并叠加悬停变体；错误且聚焦时边界保持错误色，焦点环照常显示；禁用阻止操作但保留选中与错误语义。

## 7. 动效

- 颜色、底色与边框变化 120 ms `ease-out`；开关圆钮 120 ms；窗口打开与关闭 160 ms 透明度与 0.98→1 缩放；推入导航 200 ms 水平位移。
- `prefers-reduced-motion: reduce` 时取消位移与缩放，只保留不超过 120 ms 的透明度变化。

## 8. 图标

- Bern 图标集采用 SF Symbols 命名。Web 后端为每个支持的名字提供一个 24 × 24 viewBox 的 SVG：1.75 px 圆头描边、`currentColor`，以内联 SVG 输出，颜色随配方变化。
- 应用使用了 Web 图标表中没有的名字时，Web 目标构建失败并指向该声明，不显示占位或空白。
- 彩色图像与品牌图形属于内容，不参与主题着色。

## 9. 可访问性与本地化

- 根元素 `lang` 为解析后的语言；文字方向由语言决定。
- 角色映射：Button → `button`；IconButton → `button` + `aria-label`；Switch → `switch` / `checkbox`；Slider → 原生 range；Picker segmented / radio → `radiogroup` + `radio`；Tabs → `tablist` / `tab` / `tabpanel`；Sidebar → `listbox` / `option`（`aria-selected`）；NavLink → `link`；虚拟窗口 → `dialog`（非模态，`aria-labelledby` 指向标题）；关闭确认 → `alertdialog`（模态）；Query 的进度与错误行 → `status` / `alert`。
- 控件身份写入 `data-bern-target`，与 `ui.inspect` 的目标 ID 一致，供自动化测试定位；不替代可访问名称。

## 10. Bern 验收

- 浅色、深色、提高对比、强制颜色四种环境下，每个组件的全部适用状态都有截图检查；受控配对由 `bern-theme` 测试证明达标。
- 至少三种品牌种子（内置、暖色、低饱和）下重复 4.2 的配对检查，并目视检查主要按钮、选中与焦点。
- 图标按钮在明暗背景与全部状态下都没有可见容器。
- 键盘可以完成侧边栏、标签、表单、窗口切换、窗口移动与调整尺寸、关闭确认的全部操作；输入法组合期间不提交写入。
- `pointer: coarse` 下所有可点目标不小于 44 × 44 px。
- 宽屏与窄屏之间切换时，窗口实例、草稿、导航与焦点保持。
