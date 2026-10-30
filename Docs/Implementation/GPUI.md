# GPUI 实现记录

- 依据：[设计文档的 GPUI 一节](../Design.md#gpui-可选桌面目标)
- 状态：第一版（2026-10-30）。macOS 上 RustSend 与一个大型媒体类应用都能在这个目标上通过检查、构建并运行；只有下文列出的组件。Windows 与 Linux 没有构建过。

## 选择与配置

GPUI 目标只在显式选择时使用，不是任何平台的默认值：

```toml
[targets.gpui_macos]
platform = "macos"       # 或 windows、linux
ui = "gpui"
channels = ["ipc"]
```

注册的目标为 `gpui-macos`、`gpui-windows`、`gpui-linux`，各自只在同名系统上构建。颜色来源为 `BernTheme`。

## 依赖

| 依赖 | 版本 | 说明 |
| --- | --- | --- |
| `gpui-pre`（以 `gpui` 引用） | `=0.3.10` | Zed 主线由 longbridge 每周发布的快照；crates.io 上的官方 `gpui` 停在 0.2.2（2025-10-22） |
| `gpui-pre-macos`、`gpui-pre-linux`、`gpui-pre-windows` | `=0.3.10`，可选 | 平台包，见下 |
| `gpui-base` | `=0.8.1` | GPUI Kit 的无样式层：输入框的编辑、选择与输入法 |
| `gpui-pre-reqwest-client`、`gpui-pre-http-client` | `=0.3.10` | 状态里的 http(s) 图片；图片平均色的下载 |
| `image` | 0.25（png、jpeg、webp、gif） | 模板图片着色、图片平均色 |
| `objc2`、`objc2-app-kit`、`objc2-quartz-core`、`objc2-core-graphics`、`objc2-foundation`、`raw-window-handle` | 只在 macOS | 应用图标、隐藏指针、减少动态效果、关于面板、窗口命令、视频的 Metal 层 |
| `bern-ffi`、`bern-theme` | 工作区 | 进程内接口；主题快照与控件配方 |
| `sys-locale`、`url`、`smallvec`、`futures` | — | 首选语言、文件地址、帧、事件通道 |

- **平台包按特性启用**：`bern-gpui` 有 `macos`、`linux`、`windows` 三个特性，各自只启用对应的 GPUI 平台包，生成的应用只打开它所构建的那个。不用 GPUI 的 `gpui-pre-platform`：它在依赖解析时会带上 Web 平台的 wgpu 30，要求 `js-sys >= 0.3.104`，而应用自己的依赖可能钉着更早的版本（一个大型媒体类应用的媒体内核钉在 `=0.3.103`，构建在解析依赖时就失败）。Cargo 解析依赖时包括所有平台的依赖，但不包括没有启用的可选依赖。
- **macOS 平台包的特性**：`runtime_shaders` 让 Metal 着色器在启动时编译，构建不需要 Xcode 的 Metal 工具链（这台 Mac 的 Xcode 没有装它）；代价是应用在这台机器上第一次启动时要编译着色器。`font-kit`：不开时 GPUI 用的是空的文字系统，所有文字都不画。
- 版本是钉死的一组；GPUI Kit 的 `gpui-component` 0.8.1 也依赖 `gpui-pre =0.3.10`。升级是一次单独的改动，并重跑本记录的检查。

## 结构

- `crates/bern-backend-gpui`（工作区成员，只生成代码，不依赖 GPUI）：目标描述、检查、生成、构建计划。`bern package` 尚不支持这个目标。
- `crates/bern-gpui`（不在工作区里，与 `bern-gtk` 相同，只由生成的应用编译）：
  - `host`：经 `bern_ffi::start_app` 启动引擎。回调在引擎与通道线程上，字节复制进一个通道，由 GPUI 主线程上的任务按到达顺序处理。
  - `windows`：每个 `Window` 一个原生窗口，打开与关闭都告诉会话运行时；最后一个窗口关闭时退出，退出前停掉引擎。
  - 元素：`layout`（行列、层叠、网格、滚动、标签页、预览列表、手风琴）、`content`（文字、标签、图片）、`controls`（按钮、开关、选择器、输入框）、`overlay`（弹层、菜单、对话框、分体按钮）、`search`（搜索、搜索面板、工具栏）、`scroll`（停靠与定位）、`query`（异步查询）、`media`（视频）、`spin`（转动的图片）、`navigation`、`form`、`scene`、`canvas`、`behaviors`、`menus`；`look` 是 `style!` 规则，`theme`、`text`、`session`（外部入口）、`native`（平台接口）。
- 生成的应用与其他目标的布局相同：Windows 与 Linux 上是 `<产品>/<程序>`（旁边是 `resources/`），macOS 上是 `<产品>.app`（见下）；`bin/` 里的应用 CLI 与 MCP 服务器、`skills/` 里的 Agent skill。
- **macOS 应用包**：与 SwiftUI 目标相同，`Info.plist` 写名字、版本、标识与 `CFBundleIconFile`，各语言的名字在 `<语言>.lproj/InfoPlist.strings`；图标是声明的 PNG（或 Bern 标志）做成的 `.icns`，由系统裁成它的形状、套上底板，声明 `.icns` 时原样使用，Icon Composer 文档不支持（报错）；程序在 `Contents/MacOS`，资源在 `Contents/Resources/resources`，CLI 与 MCP 服务器在 `Contents/Helpers`、skill 在 `Contents/Resources/skills`，先签它们再临时签名整个包，然后 `touch` 并 `lsregister -f` 让 Finder 与 Dock 读新图标；`bin/` 里是指向包内程序的链接。运行时在包里不再用 `setApplicationIconImage` 设应用自己的图标（那样 Dock 显示的是没有底板的原图），只在 `WindowSet(icon:)` 换成别的图标时设置。

窗口内容在 GPUI 每次渲染时重新构建：生成的代码给每个窗口与组件各一个 `fn(&Ctx) -> El`，直接读引擎状态。引擎状态变化后，各窗口的根视图收到 `notify`，下一帧重画。所以这个目标不需要 GTK 运行时那样逐字段的响应式绑定。

## 已实现

更新：2026-10-31。生成器只生成下列组件与属性，其余都在构建时报错并指向声明处。

| 类别 | 组件 |
| --- | --- |
| 窗口 | `WindowSet`（运行时选择的 `accent`、`appearance` 与 `icon`）、`Window`（尺寸、最小尺寸、`chrome: unified`、草稿、`idle_after`、`idle_enabled`、`immersive`） |
| 布局 | `Column`、`Row`（含 `align: baseline`）、`Flow`、`Grid`、`Stack`（`style!` 底色、前景、圆角、阴影，`enabled`、`under_bars`）、`ScrollView`（纵向、横向、`snap`、`position`、`open_at`、`open_center`）、`Spacer`、`Divider`、`Adaptive`/`Layout`、`Form`、`Section`、`Accordion`、`PreviewList`、`FocusRegion` |
| 场景 | `ScaleBox`、`Positioned`、`Rail`、`Canvas`（`Shape` 的矩形、圆角、胶囊、椭圆、描边、虚线、渐隐与 `fill_from` 图片平均色，`Series`，`Pattern`） |
| 导航 | `Navigation`/`NavLink`/`Destination`、`SplitView`/`Sidebar`/`SidebarItem`/`SidebarHeader`/`SidebarFooter`/`Detail`、`Tabs`/`Tab`（`display: icon`）/`TabAccessory`、`Toolbar`（`Spacer` 前的在前端） |
| 内容 | `Text`、`Label`、`Image`（随包资源、状态里的文件与 http(s) 图片、深色图、圆角、`fit`、`aspect`、`rendering: template`）、`Progress`、`Toast`、`Video`（见下） |
| 控件 | `Button`（形状、高度、`style!` 规则、`pick`、快捷键、带内容）、`IconButton`、`Switch`、`SplitButton`、`Picker`、`LanguagePicker`、`TextInput`、`Search`（页内、侧栏、工具栏里的搜索框与下方结果，`style: palette` 的搜索面板）、`SearchItem`、`Slider`、`Menu`/`MenuItem`、`PopoverButton`、`ControlGroup`、`ShortcutInput`、`Drop`（含 `over`）、`Pinch` |
| 行为 | `ForEach`、`if`/`if let`/`match`、带参数的组件、`Query`、`Modal`、`Shortcut`、`Timer`、`Watch`、`reveal: hover`、媒体命令动作 |

`style!` 规则：`Stack`、`Form`、`Sidebar`、`Detail` 与 `PopoverButton` 的面板取底色、前景（`Stack` 还有圆角与阴影）；`Button`、`IconButton` 与 `SidebarItem` 取静止外观与悬停、按下、禁用（侧栏项还有选中）状态，包括 `scale`。

其他能力：

- **颜色**：`bern-theme` 按明暗与种子生成快照，种子是运行时选的强调色或 `[theme].brand_seed`；`appearance: dark` 让一段内容用深色。`fill.subtle`、`fill.strong`、`selection.background`、`selection.foreground` 与 Web 目标一样由别的令牌导出；主题里没有的角色画成洋红，便于发现。文字与图标没有指定颜色时取所在处的前景：按钮样式或 `Stack` 样式的前景，没有时是主要文字色。
- **尺寸**：与 Web 视觉规范相同的字号表与控件尺寸；`style!` 的 `text { … }` 改字重与字号。`ScaleBox` 里所有长度（含画布）按场景缩放。行、列与文字不超过所在容器的宽度，多行文字因此在容器里换行：GPUI 按单行宽度量文字的最小宽度，只靠 flex 收缩不会换行。
- **缩放状态**：`style!` 的 `scale` 在悬停或按下时把按钮按比例重画在一个不可见的原样副本之上，副本占住布局，周围不移动；没有动画。
- **动效**：只有 `spin`；出现、消失、过渡等属性被接受，变化立即发生，共用检查对此给出警告。
- **转动**：只转资源里的 `Image`，其他元素与状态里的图片不转，检查时给出警告。GPUI 的彩色图片在 GPU 上不能带变换，单色图形可以：图里不透明处（透明度 96 以上）都是同一种颜色、或 `rendering: template` 时，把图片包成一个内嵌位图的 SVG，按单色图形由 `paint_svg` 带旋转矩阵绘制，GPUI 按路径与尺寸缓存它的遮罩，转动只改矩阵；多色图片在后台线程按设备像素逐帧转成对角线大小的方图（双线性取样），主线程画最新的一帧，用完的帧从图集里删掉。转动时每帧请求下一帧，所以整个窗口每帧重建。减少动态效果取系统的辅助功能设置与 `BERN_MOTION=reduce`。
- **菜单栏**（macOS）：系统的标准菜单，名字与快捷键按 macOS：应用菜单（关于、服务、隐藏、隐藏其他、全部显示、退出）、文件（关闭 ⌘W）、编辑（撤销到全选）、窗口（最小化、缩放、前置全部窗口）。编辑菜单用输入框（`gpui-base`）自己的动作，有输入框取得焦点时可用，否则变灰；关闭、最小化、缩放调用窗口自己的 `performClose:` 等，关闭因此与点关闭按钮一样经过窗口的关闭回调。关于面板是系统的标准面板，带应用名与版本。切换界面语言后菜单跟着换。应用声明的 `MenuBar` 还不显示。Windows 与 Linux 上只有退出。
- **带内容的按钮**：与 Web 目标相同，除胶囊形以外没有自己的边框与底色，外观只来自 `style!` 规则，规则的圆角裁切内容；按下时稍暗。
- **搜索面板**：工具栏里的入口显示提示与快捷键；打开后遮住窗口，面板里的输入框取得键盘，上下方向键在可导航的 `SearchItem` 之间移动，回车执行，Esc 或点面板外关闭。
- **视频**：应用的媒体内核播放所有播放器（`bern_media_kernel_attach`）。macOS 上每个 `Video` 是窗口内容视图里、GPUI 视图下面的一个原生视图，宿主一个 `CAMetalLayer` 交给内核绘制，位置与大小跟随布局；窗口背景改为透明，窗口与各层的底色在视频处留空，所以叠在视频上的控件仍画在上面。帧时钟是一个按 60 Hz 调用 `bern_media_surface_frame` 的线程，时间取 `CACurrentMediaTime`。没有媒体内核时没有播放器能播放：这个目标还没有自己的媒体执行器；Windows 与 Linux 不向内核提供表面。`Video(controls: standard)` 的标准控制条没有实现，构建时报错。
- **语言**：从系统首选语言里选；运行中经 `LanguagePicker` 或 `ui.set_language` 切换。
- **外部入口**：`ui.inspect`、`ui.activate`、`ui.edit`、`ui.submit`、`ui.set_bool`、`ui.select`、`ui.language`、`ui.set_language`；其他会话命令立即返回 `ui_unavailable`。控件清单在请求到来时把窗口内容再构建一次得到，窗口被遮挡时也是当前状态；列清单时不打开查询订阅。

### `BERN_WINDOW_OFFSCREEN`

开发检查用。窗口开在所有显示器之外（-10000, -10000），应用设为 accessory（不进 Dock），窗口不取焦点。用途是在有人使用的电脑上检查：窗口不挡任何东西，也不抢键盘；又因为不会被别的窗口挡住，会一直出帧，可以用 `screencapture -l <窗口号>` 截图。

macOS 上被其他窗口完全挡住的窗口，GPUI 会停掉它的 display link：既不重画，也不处理点击，重新露出来时才补画。第一次在屏幕上检查时，经 CLI 改了状态，窗口看起来没变，原因就在这里。

## 实测

### 一个大型媒体类应用（2026-10-30）

同一份声明不改，测试副本另设 `bundle_id`，运行时把 `HOME` 指向空目录，数据目录因此与真实应用分开。

- 检查报错从 1749 处降到 0（只剩动效、`MenuBar` 等警告）。
- 构建：应用、CLI、MCP 服务器与 skill 都产出；引擎与它自带的媒体内核一起编译。
- 在 `BERN_WINDOW_OFFSCREEN` 下运行并截图，经应用 CLI 操作：
  - 首次启动的引导对话框四步：欢迎页（格纹图案、矢量吉祥物、字标、换行的说明）、选择起点（选项卡片、选中描边与对勾、模块开关胶囊、跟着变的小窗口预览）、外观（四个主题色圆点、明暗分段）、准备好了；
  - 主窗口的侧栏与工具栏（搜索面板入口、“打开”按钮），首页的今日更新（http 海报、时间线与“现在”线、横向滚动按 `open_center` 打开），新番页的一周手风琴与每天的预览列表，媒体库页（分段、筛选胶囊、滑块、排序菜单、空状态），下载页；
  - 搜索面板：打开、输入后按分组列出结果，第一项高亮。
- 检查中修正的问题：层叠容器只看第一层是否占满（窗口内容只占上半部）；横向滚动视图在高度上也占满；滚动内容被压缩；对话框按声明处而不是按窗口铺开（被压成一条竖线）；行与文字不换行；`fill.subtle` 等角色画成洋红；带内容的按钮多一圈按钮外观；按钮样式的前景没有传给图标；被禁用的层里侧栏项与搜索仍报告为可用。

### RustSend：转动的标志、浅色、菜单与图标（2026-10-31）

测试副本把接收页的 `Svg`（只为转动而用文档引擎）换成位图加 `spin: app.ready && !app.pending, spin_period: 15`，标志是单色的，走 GPU。以 `.app` 包构建，`BERN_WINDOW_OFFSCREEN` 下运行：

- 相隔两秒的两张窗口截图，标志区域有 4366 个像素不同、文字区域没有不同；转动时整个进程约占一个核心的 12–15%（debug 构建），主要是每帧重建整个窗口。
- `ui select` 把设置里的外观改成“浅色”，窗口变为浅色，设置文件写入 `light`；改回“跟随系统”后随系统（这台 Mac 是深色）。
- 经“系统事件”读菜单栏：苹果菜单、RustSend、文件、编辑、窗口；各项名字、可用状态与快捷键（⌘Q、⌥⌘H、⌘W、⌘C、⌘Z、⌘M）都对，编辑菜单在没有输入框取得焦点时变灰。
- 用 `NSWorkspace` 画出包的图标（复制到新路径并重新登记）：标志在系统的圆角底板上。

### RustSend（2026-10-30）

RustSend 1.0 的界面声明不改，加上 `gpui_macos` 目标后检查通过，debug 构建 2 分 41 秒（新的 target 目录）。上面那个应用的修正之后用同一运行时重建，接收页与设置页截图正常。测试副本另设 `bundle_id`，并用 `RUSTSEND_DATA_DIR` 指向空数据目录（它的引擎默认读写 `~/Library/Application Support/localsend-bern`，与 `bundle_id` 无关）。在 `BERN_WINDOW_OFFSCREEN` 下运行并截图：

- 接收、发送、传输记录、设置四页按紧凑布局显示，Logo（构建时把动画 SVG 画成图片）、图标、分段选择器、弹出按钮、开关、进度条、表单分节都在；
- `ui select` 换页与设置分类，`ui activate` 打开网络信息弹层，`ui set-bool` 打开快速保存，`ui edit` 改名后“保存”变为可用、`ui activate` 保存后引擎里的名字随之改变；
- `ui set-language` 切到英文、经引擎状态切到深色外观，单色强调色在两种外观下都对；
- 临时把紧凑断点调到 400 时，桌面布局（标题行、页面大标题）同样显示。

### 最小应用（2026-10-30）

Apple M4、16 GB、macOS 27.2，Rust 1.99。

**最小应用**：`bern new` 生成的计数器，加上 `gpui_macos` 目标：

- `bern check --target gpui_macos` 通过；
- debug 构建（新的 target 目录，包括编译 GPUI）50 秒；
- 在 `BERN_WINDOW_OFFSCREEN` 下运行、窗口截图：
  - 标题、计数与两个按钮按简体中文显示；
  - 经应用 CLI 执行三次 `counter add-one` 后显示 3，`counter reset` 后显示 0；
  - `ui inspect` 列出 `main.add_one` 与 `main.reset`，标签为“加一”“重置”；
  - 两次 `ui activate --target main.add_one` 后，引擎状态与窗口都是 2；
  - 目标不存在时返回 `target_not_found`；
  - 检查期间前台应用一直不变，测试窗口没有取得焦点。

同一应用的 release 构建（LTO、`codegen-units = 1`、去符号）：

| 项目 | 结果 |
| --- | --- |
| 编译时间 | 3 分 27 秒，冷编译，包括 GPUI |
| 应用程序 | 6.6 MB |
| CLI、MCP 服务器 | 各 0.9 MB |
| 内存（`footprint`） | 第一次启动 73 MB（在编译着色器），之后 29 MB |

**探针**：只用 GPUI 的计数器窗口与加上 `gpui-component` 的窗口，都是 LTO release，编译时没开 `font-kit`，所以不画文字，体积和内存都不含文字系统。

| 探针 | 编译时间 | 体积 | 首帧 | 内存 |
| --- | --- | --- | --- | --- |
| 只用 GPUI | 1 分 47 秒（冷编译） | 5.3 MB | 首次启动 1.8 秒，之后约 0.2 秒 | 首次 70 MB，之后 26 MB |
| 加 `gpui-component` | 在上一行基础上再 3 分 59 秒 | 14.0 MB | 约 0.14 秒 | 27 MB |

首次启动慢、内存高，是在编译 Metal 着色器（`runtime_shaders`）。

## 未验证与未实现

- **视频播放**：没有实际打开过媒体，内核绘制、留空的底色、帧时钟都未验证。
- **真实指针与键盘**：没有实测点击；按钮的点击与 `ui.activate` 共用同一个闭包，但没有经过真实的鼠标事件。检查时用 `CGEventPostToPid` 发给进程的点击，应用不在前台时没有被处理。缩放状态、悬停高亮、搜索面板的方向键、拖放的 `over`、捏合、`FocusRegion` 都只编译、没有实测。
- **其他平台**：Windows 与 Linux 上没有编译过，生成的应用、`gpui-pre` 的这两个平台都没有构建；嵌入式设备没有试过。
- **外观**：系统强调色没有接入；对比度偏好没有看过；运行中改变系统明暗时“跟随系统”的切换没有看过（检查不改系统设置）；应用图标的切换没有看过，换上的图标经 `setApplicationIconImage` 显示，没有系统的底板。
- **转动**：多色图片的后台线程逐帧转动只编译、没有实测。
- **窗口**：前端没有报告 `bern_window_facts`；窗口的 `role`、`presentation`、关闭确认没有实现；`immersive` 只隐藏工具栏与静止时的指针，标题栏按钮仍在；应用声明的 `MenuBar` 不显示（只有系统的标准菜单）。
- **外部入口**：`ui.select` 按选项的显示文字匹配，应用 CLI 的说明写的是枚举变体名；同一组件的多个实例（例如四张选项卡片）控件标识相同。
- **macOS 产物**：`bern package` 不支持这个目标，没有发布签名与公证；Dock 里的图标没有截图看过（离屏运行的应用不进 Dock）。
- **其余**：无障碍（GPUI 依赖 AccessKit，但控件还没给出角色与名称）、`Video` 的标准控制条、平台的媒体执行器、`spin` 以外的动效都没有实现。
