# 平台能力与验证范围

更新：2026-10-24。下表按 [bern-target-api](../../crates/bern-target-api/src/lib.rs) 与各后端 `TargetDescriptor.features` 整理。macOS 在共用 SwiftUI 清单之外单独开放搜索面板；iOS 不开放。tvOS 与 iOS 共用 SwiftUI 生成器，清单在 [tvOS 描述](../../crates/bern-backend-swiftui/src/tvos.rs)中单列。

## 编译期功能支持

“支持”表示后端声明接受对应特性；“不适用”表示后端同样声明接受，但那里没有对应的行为（例如电视与桌面窗口没有方向可转），用到时不报错也不警告；“—”表示未声明，用到该特性的声明会被相应能力检查拒绝。它不表示所有属性行为、视觉效果或设备场景都已经验收。颜色覆盖、动效、窗口和媒体还有独立的能力描述，不能只看此表推断完整支持。

| 功能标记 | 对应能力 | macOS | iOS | tvOS | Web | Android | Linux | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `button_content` | 按钮内自定义内容 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `reveal` | 悬停显现（无指针目标常显） | 支持 | 支持 | —（常显，警告） | 支持 | 支持 | 支持 | 支持 |
| `toolbar_search` | 工具栏搜索 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `search_results` | 搜索建议结果 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `session_actions` | 会话字段动作 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toggle_button` | 开关按钮样式 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `dialog_windows` | 对话框窗口 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `bound_lengths` | 状态绑定长度 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `appearance` | 局部明暗外观 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `shortcuts` | 键盘快捷键 | 支持 | 支持 | 支持（遥控按键） | 支持 | 支持 | 支持 | 支持 |
| `runtime_theme` | 运行时主题选择 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `runtime_images` | 引擎提供图片 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `picture_fills` | 图片取色与渐隐填充 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `file_pick` | 文件选择 | 支持 | 支持 | — | 支持 | 支持 | 支持 | 支持 |
| `scroll_snap` | 滚动停靠 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `rails` | 比例轨道布局 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `popover_button` | 浮层按钮 | 支持 | 支持 | 支持（sheet） | 支持 | 支持 | 支持 | 支持 |
| `text_weights` | 文本字重样式 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `text_lines` | 文本行数上限 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `image_corners` | 图片圆角 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `sidebar_width` | 固定侧栏宽度 | 支持 | 支持 | —（系统宽度，警告） | 支持 | 支持 | 支持 | 支持 |
| `controlled_values` | Switch / Picker / Slider 引擎受控值 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `search_palette` | 搜索命令面板 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `image_template` | 模板图片 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `adaptive_images` | 图片比例与网格独立行距 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `scaled_scene` | ScaleBox / Positioned / Marquee、动态字号 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `accordion_layout` | 手风琴列布局 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `capsule_controls` | 胶囊控件和 ControlGroup | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `modal_flow_timer` | Modal / Flow / Timer / PickerOption | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `page_badges` | 导航和模块页面计数 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `edge_grow` | 按边展开过渡 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `delayed_appear` | 延迟入场 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toolbar_leading` | 工具栏前缘项和文字 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `axis_padding` | 分轴内边距 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `icon_menus` | 纯图标菜单 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `icon_labels` | 纯图标 Label | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `icon_tabs` | 纯图标页签（`Tab(display: icon)`），`label` 为可访问名称；未声明的目标连同标题一起显示并警告 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `row_baseline` | 行内文字基线对齐 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `sidebar_header` | 固定侧栏顶栏 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `untitled_detail` | 无标题详情 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `idle_control` | 按状态启停空闲检测（`Window(idle_enabled:)`） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `fill_buttons` | 横向撑满容器的系统按钮（`Button(width: fill)`） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `page_search` | 页面里的系统搜索框（不在 `Toolbar` / `Sidebar` 里的 `Search`），结果列在框下 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `window_short` | 窗口高度低于紧凑断点（`window.short`），如手机横屏 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `window_landscape` | 手机上按状态转为横屏或竖屏（`Window(landscape:)`），如播放器打开时横过来 | 不适用 | 支持 | 不适用 | 支持（浏览器允许锁定方向时） | 支持 | 不适用 | 不适用 |
| `modal_cover` | 紧凑窗口里铺满整个窗口的模态（`Modal(cover: compact)`），如手机的全屏流程 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `form_layout` | 表单版式：`Section(style: plain)` 标题组、行内边距、`Form(gap:, row_height:)` | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `tab_accessory` | 随页签常驻的一条（`TabAccessory`），如小播放器；iOS 26.1 起为标签栏上方的系统附件 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `grid_cells` | 按列宽定行高的网格（`Grid(cell_ratio:, cell_extra:)`），格内内容顶对齐、可伸进行间距 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `size_classes` | 四档尺寸适配 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `text_formats` | 文本中的复数分支、日期、时长，以及显示变体字段的枚举标签（见[文本、内容与格式化](UI.md#文本内容与格式化)） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |

## 不由上述功能标记完整表达的能力

| 能力 | 当前范围 |
| --- | --- |
| `Window(idle_enabled:)` | 六个目标都消费启停状态（能力 `idle_control`）：为假时 `window.idle` 保持为假且不计时，变真时重新计时 |
| 静置区（`idle_area`） | 七个目标的动效能力都声明它。macOS、Web、Android、Linux、Windows 上，页面有静置区时鼠标只在静置区上停住才静置，停在别处或移出窗口保持醒着，触摸、遥控器与键盘仍按 `idle_after`；iOS 与 tvOS 上静置区不改变计时（接上鼠标的 iPad 也是）。macOS 用一个实际应用的播放画面实测；Web 有运行时测试；Android 编译打包、Windows 两种前端经 MSVC 编译、Linux 运行时在 macOS 的 GTK 4 上通过 Clippy，这三者都未运行实测 |
| `Window(landscape:)` 在网页上 | 用屏幕方向接口锁成横屏：浏览器只在手机与平板上锁定，多数还要求页面全屏或作为已安装的应用运行，iOS 的 Safari 不提供锁定；被拒绝时保持设备方向，进入全屏时再请求。运行时测试覆盖，未在手机浏览器上实测 |
| 图片跨分栏 Hero | macOS 使用窗口级覆盖层，Web 使用视口级副本；iOS 与 macOS 其他视图沿用匹配几何。Video 连续画面传递另行验证 |
| 本地持久化 | 原生 FFI 宿主提供文件存储，Web app 使用同源 localStorage；未安装存储宿主时不持久化 |
| 可选 Svg | 六目标有生成路径；tvOS 没有 WebKit，不可用，用到时检查报错；专属依赖及验证见[动画 SVG](../Implementation/Animated-SVG.md)，不改变普通界面的默认实现 |
| 平台各自的应用图标（`[app.icon]`） | 七个目标都接受方形 PNG 与各自的原生格式：macOS `.icns`、iOS `.appiconset`、tvOS `.brandassets`（未声明时由默认图标生成分层图标与顶部货架图）、Windows `.ico`、Linux hicolor 图标主题、Android `res`、Web 页面图标文件夹或 SVG；配置阶段检查，生成时原样使用。规则见[应用图标](../Design.md#应用图标) |
| `SidebarHeader(role: title)` | WinUI 3 与系统 XAML 不显示（标题栏已显示应用图标与标题）；其他目标照常显示 |
| 电视布局（`Layout(size: tv)`、`window.tv`） | tvOS 永远是电视：构建时只保留 `tv` 布局并按已知的窗口条件裁剪（实测）。Android 按系统的电视模式或 leanback 特性、Web 按电视浏览器的用户代理判断，未在电视设备上验证。macOS、iOS、Windows、Linux 永远为假，`tv` 布局不生成 |
| `Shortcut` | 没有控件的快捷键，六个目标生成：macOS、iOS、Web、Android、Linux、Windows 为键盘快捷键；tvOS 与 Android TV 为遥控按键（`escape` 返回、`space` 播放/暂停、方向键为焦点无处可移时的方向）。停用的快捷键把键让给之前显示的（Web 运行时测试覆盖）。tvOS 模拟器实测方向键；返回与播放/暂停、Android TV、其他目标的按键未实测 |
| 电视字号（`text` 块的 `font(tv:)`） | tvOS 构建时取电视字号（模拟器实测）；Android 与 Web 运行时判断为电视时取（Web 由 `bern-television` 类切换，未在电视设备上验证）；macOS、iOS、Windows、Linux 不受影响。各后端生成器测试覆盖 |
| 文字会话动作（`push_str`、`pop`） | 六个目标生成；tvOS 模拟器上经屏幕键盘实测输入与删除；SwiftUI 在 tvOS 与 macOS 上类型检查，Compose 随 Android 构建编译，GTK 在 Ubuntu 虚拟机上编译，WinUI 3 在 Windows PC 上编译 |
| `Search(suggest: true)` | 未输入时就列出结果：SwiftUI 与 Web 获得焦点时、电视上一直列出（tvOS 模拟器实测）；Compose 的工具栏搜索页本来打开就列出；GTK 与 WinUI 3 获得焦点时弹出。tvOS 以外未实测交互 |
| 性能追踪（`BERN_TRACE`、`trace.*`、`bern run --trace`） | 共用的 Rust 一侧对所有目标相同：规则与过滤、记录与汇总、时间线、会话命令，以及引擎（排队、执行、快照、比较、模块调用、`span!`）与桥接（解码、按字段编码、回调触发）的分段。各前端界面线程的打点尚未接入，交互链路与长帧暂不出现。见[性能追踪](Tracing.md) |
| `default_focus` | `Button` / `IconButton` 出现时取得焦点，六个目标生成：tvOS 是焦点区域的首选焦点（模拟器实测），macOS / iOS 用 `FocusState`，Web 调用 `focus()`，Compose 用 `FocusRequester`，GTK 与 WinUI 3 在控件显示后请求焦点；tvOS 以外未实测；GTK 在 Ubuntu 24.04 虚拟机上编译并通过 Clippy 与生成应用的类型检查，WinUI 3 在 Windows PC 上用 MSVC 编译生成的 gallery |

## 最近新增能力的验证

这一节只描述 2026-10-15 至 24 日的新增能力；已有平台的历史验收见各实现记录。构建成功不代替实际设备交互。

| 范围 | 代码 / 构建证据 | 实际交互范围 |
| --- | --- | --- |
| macOS 的多页应用布局、控件与图片 Hero | 应用构建及相关生成器检查；图片 Hero 有真实 AppKit 容器回归 | 播放器覆盖层、分组、返回入口、舞台图片展开、缩到侧栏及恢复 |
| Web 的多页应用适配 | WASM 构建、生成器与浏览器运行时回归 | Chromium 桌面及指定窄屏场景；桌面 Safari 27.0.1 的媒体库、主题、导航和布局复查，范围见 Web 记录 |
| iOS 的近期播放器扩展 | 共享 SwiftUI 生成路径与能力声明 | 本轮未做播放器设备交互验收 |
| Linux（GTK）的近期布局扩展（2026-10-18） | 上表 29 项能力与 `Stack` 样式在 GTK 实现；生成器测试含生成应用的类型检查；真实应用 `bern build --target linux` 通过 | Ubuntu 24.04 虚拟机经 Broadway 实测：引导模态、桌面侧栏与时间轴（`Rail(overhang:)` 量到）、播放器场景；手机布局的 `TabAccessory`、`ScrollView(open_at:)`、`scroll_scale` 与 `scroll_origin_y`、`pages.<id>.show()`；手机布局在临时把最小窗口宽改小后测得。未验证项见 [Linux 记录](../Implementation/Linux-GTK.md) |
| Android / Windows 的近期布局扩展（2026-10-19） | 上表 29 项能力与 `Stack` 样式在 Compose 与 WinUI 3 实现，Android 另有 `window_landscape`；两个后端的生成器测试覆盖这些能力，WinUI 3 的测试在 Windows 上用 MSVC 编译两套前端；两个后端也实现了 `text_formats`；真实应用 `bern build --target android` 与 `--target windows`（在 Windows PC 上）构建成功 | Windows：一个临时巡览应用经 WinUI 3 与 Islands 实测侧栏顶栏与计数、无标题详情、工具栏前缘、缩放场景与跑马灯、手风琴、受控值、胶囊控件与控件组、搜索面板、模态、表单版式、`window.short`、`idle_enabled`；文本格式的测试应用在两种前端下与 SwiftUI、Web、GTK 输出相同；真实应用的引导页对照设计稿（Fluent 圆角按钮、透明动画 Logo）。Android：见 [Android 记录](../Implementation/Android-Compose.md)。未验证项见 [Windows 记录](../Implementation/Windows-WinUI3.md) |
| 应用图标与只给窗口命名的侧栏头部（2026-10-19） | 配置测试覆盖各平台原生格式的检查与错误；六个后端的生成器测试覆盖原生格式的落位（`.icns`、`.appiconset`、`.ico`、图标主题改名、`res` 与 `roundIcon`、页面图标的链接） | 一个实际应用：Windows 11 实体机两种前端的标题栏显示应用图标与标题（图标取自 `.ico` 的位图项），`role: title` 的侧栏头部不显示；macOS 27.0.1 访达图标；iOS 26.5 模拟器主屏幕与 `Assets.car` 的着色版；Ubuntu 24.04 虚拟机运行时找到图标、`.deb` 装入全部尺寸；Web 的链接与开发服务器类型；Android 只检查 APK 内容，未装到设备 |
| 虚线描边、`under_bars`、带内容的胶囊按钮（2026-10-19） | `Shape(dash:, dash_gap:)`、`Stack(under_bars: true)` 与 `shape: capsule` 的带内容按钮在六个目标实现；SwiftUI 另补上 `Stack` 里的 `Toolbar`（此前被丢掉）。各后端生成器测试覆盖虚线；SwiftUI 测试用 `swiftc` 类型检查；GTK 运行时在 Ubuntu 24.04 虚拟机上过 Clippy 与测试，生成应用的类型检查通过；Android 真实应用构建成功；WinUI 3 见右栏。Web 运行时测试覆盖撑满宽度的按钮让所在行撑满、固定尺寸的层在行里不收缩 | 一个实际应用里没有页面时的空画框窗口：macOS 27.0.1 窗口截图量得空画框在 1280 × 800 场景的 (103, 64, 1074 × 604)，与播放器退后时的画框逐点重合，工具栏与红绿灯浮在其上；iOS 26.5 模拟器上画框、按钮列与列表在设计稿位置 ±2 点，迷你播放器距屏幕底边 24 点；Chromium 桌面与 390 宽窄屏、Ubuntu 虚拟机 Broadway 的窗口快照。GTK 上顺带修正：`Stack` 里有自身厚度的 `Rail` 按 `Stack` 的对齐摆放（缩略图底部的进度条原先在正中）。之后在 MIX 2S（Android 15）与 Windows 11 实体机（150% 缩放）上实看，修正了 Compose 的 `unified` 顶栏、行里撑满的按钮、层叠里的轨道与模态里的共享元素，以及 WinUI 3 带内容按钮的边框与内容居中、`fill.strong` 的深浅、内容铺到标题栏下时页头的位置与 `TitleBar` 的让位宽度，见 [Android 记录](../Implementation/Android-Compose.md)与 [Windows 记录](../Implementation/Windows-WinUI3.md) |
| `Canvas` 作为 `Stack` 的底板（2026-10-20） | 两个方向都不给尺寸的 `Canvas` 直接作为 `Stack` 的一层时，取其他层给出的尺寸而不撑开 `Stack`：SwiftUI 用 `BernLayers`（`Stack` 写了尺寸时按写的尺寸），Web 在布局意图中排除，GTK 在 `compute_expand` 中排除，Compose 修正了层叠标记没有传给画布的问题，WinUI 3 此前已实现；没写宽度的这种 `Stack` 在列里横向填满（SwiftUI `fillsWidth`、Web `align-self: stretch`、Compose `fillMaxWidth`、GTK 按分到的宽度、WinUI 3 `HorizontalAlignment.Stretch`）。顺带：Compose 行里的轨道分剩余宽度、`Label` 图标随字号缩放、定高行里的文字按行的对齐超出；GTK 的 `Form` 横向占满、`reflow` 行汇总子元素的撑开。SwiftUI、Compose、Web 运行时测试与 GTK（虚拟机上）的测试通过 | 一个实际应用的可选功能预览：macOS 27.0.1 设置窗口、Chromium 引导第二步、MIX 2S 引导第二步与设置子页、Ubuntu 24.04 虚拟机（Broadway）设置窗口，选中项只有一行高，侧栏底板铺满；设置页插件横幅在 macOS、Windows、Chromium、MIX 2S 与虚拟机上与正文同宽。iOS 未实看 |
| 文字样式 `control`（2026-10-20） | `Text`、`Label` 的 `style: control` 取平台按钮标题的字体：SwiftUI 不设字体、WinUI 3 与 GTK 继承按钮、Compose 用 `labelLarge`、Web 继承按钮的 `font`；`style!` 的 `text` 块给它另定字号时构建失败（编译器测试覆盖） | 一个实际应用胶囊按钮里的文字：Windows 11 实体机 14 号常规体、macOS 27.0.1 与 Chromium（13 px、500）、Ubuntu 24.04 虚拟机，都与同一行的系统胶囊按钮标题一致 |
| tvOS 目标与电视布局、`Shortcut`、`default_focus`（2026-10-20） | `swiftui + tvos` 生成与构建；`bern run --target tvos` 装进 tvOS 模拟器并启动；生成测试按 tvOS 模拟器 SDK 类型检查；分层图标的生成与检查有单元测试。`Layout(size: tv)`、`window.tv`、`Shortcut`、`default_focus` 六个目标生成：`examples/gallery` 用到它们，各后端的生成器测试覆盖，GTK 在 Ubuntu 24.04 虚拟机上编译（Clippy、生成应用的类型检查），WinUI 3 在 Windows PC 上用 MSVC 编译，Web 运行时测试覆盖停用的快捷键让键 | Apple TV 4K（第 3 代）模拟器（tvOS 26.5）：展示应用与一个实际应用的电视布局，标签栏、焦点移动、默认焦点、缩放场景与模态、播放器隐藏控制条时左右快退快进、全屏设置页；主屏幕图标与顶部货架图。返回与播放/暂停、真实遥控器与真机未验证，见 [tvOS 记录](../Implementation/tvOS-SwiftUI.md) |
| 只显示图标的页签 `Tab(display: icon)`（2026-10-20） | 编译器测试（缺 `icon` 时报错）；六个后端的生成器测试（SwiftUI 在 tvOS 与 macOS 上类型检查，WinUI 3 在 Windows PC 上用 MSVC 编译生成的 gallery，GTK 在 Ubuntu 虚拟机上过 Clippy 与生成应用的类型检查）；Web 运行时测试覆盖 `aria-label` 与只有图标的按钮 | tvOS 模拟器上实际应用的标签栏只有放大镜（选中与未选中）；macOS 27 上窗口工具栏的页签只显示符号、对话框表单里的经典页签显示标题；Chromium、Ubuntu 虚拟机（Broadway，宽窗口的头部切换器与紧凑窗口的底部切换栏）、Windows PC 的 WinUI 3 与系统 XAML、Android 真机上 gallery 的“关于”页签只显示图标。iOS 未运行；读屏朗读未实测 |
| 动画 SVG | 六目标生成；示例 macOS、iOS 模拟器、Android、Web 构建；Linux 随真实应用原生构建（WebKitGTK 6）；Windows 本轮未原生构建 | macOS / Web 实测 Logo 与交互；Linux 实测引导页的动画 Logo；Ubuntu 24.04 上 WebKit 沙箱需要 AppArmor 配置，`.deb` 已附带（同形配置下沙箱运行实测，安装未实测），`.tar.gz` 需手动添加，见 Linux 记录；iOS / Android 本轮未运行设备交互，Windows 未验收 |
| 画布延伸 `Positioned(bleed:)`、`WindowButtons`、网页控件与中性色（2026-10-20） | `bleed` 在 Web、SwiftUI、Compose、GTK 与 WinUI 3 实现；`WindowButtons` 六个目标生成，SwiftUI 在 macOS 上占 84 点；Web、SwiftUI、Compose 的生成器测试覆盖 `bleed` 与 `WindowButtons`，GTK 与 WinUI 3 的生成器测试覆盖 `bleed`（GTK 生成的应用在 Ubuntu 24.04 虚拟机上类型检查，WinUI 3 的两种前端在 Windows PC 上用 MSVC 编译），GTK 的单元测试覆盖画布四周的空白与各边的延伸量；真实应用的 macOS 构建通过 Swift 类型检查，Linux 与 Windows 构建通过。Web 运行时测试覆盖带内容的胶囊按钮取标准高度 | Chromium 1280 × 800、1512 × 860、1920 × 970：播放器铺满时画面贴到窗口边（超宽窗口按 16:9 左右留黑），显示控件时画框与控制条在居中的设计画布里；顶栏图标按钮没有共享底板、相隔 6 px；菜单从按钮处弹出、尾端对齐按钮；控件在铺满与退后之间淡入淡出并移动 8 px。Ubuntu 24.04 虚拟机（Broadway，窗口 1600 × 713）：铺满时画面 1267 宽（按窗口高的 16:9），越出 1280 × 800 设计画布、两侧只留 16:9 之外的黑边；退后时画框在设计坐标 (103, 64, 1074)，控件在居中的设计画布里；各边的延伸量与画框随同一个弹簧逐帧移动，窗口变宽后下一次布局即用新的空白。Windows 11 实体机（WinUI 3，150% 缩放）：窗口 1600 × 800 时显示控件的画框在居中的设计画布里（x 268.7、宽 1063，即设计坐标 (103, 1074) 乘画布缩放 0.99），铺满时画面 1408 宽（按内容高的 16:9），越出设计画布；拉成 1400 × 800 后铺满的画面横跨整个内容宽度，1280 × 977 时按宽铺满、上下居中；静置隐藏控件后画面同样铺满，Windows 上过渡未逐帧采样。Android、iOS 上的 `bleed` 与 macOS 改窗口比例后的外观未实测 |
| 按平台给出的长度与 `shape: chip`（2026-10-20） | 编译器测试覆盖 `style!` 里 `gap`、`padding_x`、`padding_y`、`width`（含 `fit`）、`height`、`font_size`、`row_height` 的解析与报错；构建每个目标前按它的平台替换元素写下的长度、在网页以外把 chip 换成 30 点胶囊，生成器测试覆盖两者；六个目标都经过这一步，不需要各自实现 | Chromium 1280 × 800 对照网页设计稿：侧边栏、品牌行、导航项、账户行、工具栏与搜索框、媒体库的标签与缩放、设置对话框的标题行、分组与行内下拉逐项量得与设计稿一致（相差不超过 1 px，系统滚动条占位与 Inter 字宽除外）；macOS 上同一应用的外观未在本轮实测 |
| 应用自带的媒体内核（2026-10-21） | `bern-media` 的 `MediaKernel` 与 C ABI；SwiftUI 的视频视图在内核模式下给出 `CAMetalLayer` 与 `CADisplayLink` 帧时钟（macOS、iOS、tvOS 生成），macOS 上用一个实际应用自带的播放内核实测播放、跳转、视图间转移与命令行控制；iOS、tvOS 未实测；Compose、GTK、WinUI 3、Web 不提供内核表面，内核不挂接，平台媒体设施照常 |
| 性能追踪的引擎与桥接一侧（2026-10-21） | Rust 测试：规则解析与报错、环形缓冲（写满丢弃并计数、边写边读）、乱序到达的记录按 token 与修订号对上、长帧归因、同一命令过频时合并成一行、关闭时不读时钟也不分配缓冲；真实引擎走完命令写出可解析的时间线；bern-ffi 测试模拟一次控件操作，报告里从动作到上屏各段对上；`bern run --trace` 的传参有测试；WebAssembly 编译通过 | 未在设备或真实应用上实测；各前端界面线程一侧未接入 |
| 静置区与 `landscape` 的适用范围（2026-10-24） | `idle_area` 在 Compose、GTK 与 WinUI 3 实现：Compose 按静置区的悬停进出，GTK 按拾取、WinUI 3 按命中测试找出鼠标所在的静置区；`window_landscape` 在 Web 实现（屏幕方向锁定），tvOS、Linux、Windows 声明为不适用。四个后端的生成器测试覆盖，Web 的运行时测试覆盖横屏锁定；临时应用 `bern check` 各目标无警告、`bern build --target android` 通过；WinUI 3 的两种前端在 bern-pc 上经 MSVC 编译（需先绕过当时 HEAD 中三处与本项无关的编译错误）；GTK 运行时在 macOS 的 GTK 4 上通过 Clippy | 未运行实测：Ubuntu 虚拟机磁盘已满，Windows 只编译，Android 没有接鼠标的设备，横屏锁定没有在手机浏览器上试 |

Firefox 与 Safari 手机布局尚未实测。用于验收的应用播放器仍有模拟状态；播放器界面和图片过渡的验收不等于已接入完整媒体播放功能。

## 来源与维护

- 功能定义及拒绝规则：[bern-target-api 的功能定义](../../crates/bern-target-api/src/lib.rs)与[功能检查](../../crates/bern-target-api/src/check/features.rs)。
- [SwiftUI 清单](../../crates/bern-backend-swiftui/src/lib.rs)及 [iOS 描述](../../crates/bern-backend-swiftui/src/ios.rs)、[tvOS 描述](../../crates/bern-backend-swiftui/src/tvos.rs)。
- [Web](../../crates/bern-backend-web/src/lib.rs)、[Compose](../../crates/bern-backend-compose/src/lib.rs)、[GTK](../../crates/bern-backend-gtk/src/lib.rs)、[WinUI 3](../../crates/bern-backend-winui3/src/lib.rs) 清单。
- 验收：[macOS](../Implementation/macOS-SwiftUI.md)、[iOS](../Implementation/iOS-SwiftUI.md)、[tvOS](../Implementation/tvOS-SwiftUI.md)、[Android](../Implementation/Android-Compose.md)、[Web](../Implementation/Web.md)、[Linux](../Implementation/Linux-GTK.md)、[Windows](../Implementation/Windows-WinUI3.md)。

新增后端特性时同时更新此表和对应实现记录，并保留对尚未验收场景的说明。
