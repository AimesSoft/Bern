# 平台能力与验证范围

更新：2026-10-19。下表按 [bern-target-api](../../crates/bern-target-api/src/lib.rs) 与各后端 `TargetDescriptor.features` 整理。macOS 在共用 SwiftUI 清单之外单独开放搜索面板；iOS 不开放。

## 编译期功能支持

“支持”表示后端声明接受对应特性；“—”表示未声明，用到该特性的声明会被相应能力检查拒绝。它不表示所有属性行为、视觉效果或设备场景都已经验收。颜色覆盖、动效、窗口和媒体还有独立的能力描述，不能只看此表推断完整支持。

| 功能标记 | 对应能力 | macOS | iOS | Web | Android | Linux | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `button_content` | 按钮内自定义内容 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `reveal` | 悬停显现（无指针目标常显） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toolbar_search` | 工具栏搜索 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `search_results` | 搜索建议结果 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `session_actions` | 会话字段动作 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toggle_button` | 开关按钮样式 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `dialog_windows` | 对话框窗口 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `bound_lengths` | 状态绑定长度 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `appearance` | 局部明暗外观 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `shortcuts` | 键盘快捷键 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `runtime_theme` | 运行时主题选择 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `runtime_images` | 引擎提供图片 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `picture_fills` | 图片取色与渐隐填充 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `file_pick` | 文件选择 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `scroll_snap` | 滚动停靠 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `rails` | 比例轨道布局 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `popover_button` | 浮层按钮 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `text_weights` | 文本字重样式 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `text_lines` | 文本行数上限 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `image_corners` | 图片圆角 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `sidebar_width` | 固定侧栏宽度 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `controlled_values` | Switch / Picker / Slider 引擎受控值 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `search_palette` | 搜索命令面板 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `image_template` | 模板图片 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `adaptive_images` | 图片比例与网格独立行距 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `scaled_scene` | ScaleBox / Positioned / Marquee、动态字号 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `accordion_layout` | 手风琴列布局 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `capsule_controls` | 胶囊控件和 ControlGroup | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `modal_flow_timer` | Modal / Flow / Timer / PickerOption | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `page_badges` | 导航和模块页面计数 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `edge_grow` | 按边展开过渡 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `delayed_appear` | 延迟入场 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toolbar_leading` | 工具栏前缘项和文字 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `axis_padding` | 分轴内边距 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `icon_menus` | 纯图标菜单 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `icon_labels` | 纯图标 Label | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `row_baseline` | 行内文字基线对齐 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `sidebar_header` | 固定侧栏顶栏 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `untitled_detail` | 无标题详情 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `idle_control` | 按状态启停空闲检测（`Window(idle_enabled:)`） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `fill_buttons` | 横向撑满容器的系统按钮（`Button(width: fill)`） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `page_search` | 页面里的系统搜索框（不在 `Toolbar` / `Sidebar` 里的 `Search`），结果列在框下 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `window_short` | 窗口高度低于紧凑断点（`window.short`），如手机横屏 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `window_landscape` | 手机上按状态转为横屏或竖屏（`Window(landscape:)`），如播放器打开时横过来 | 不适用 | 支持 | — | 支持 | — | — |
| `modal_cover` | 紧凑窗口里铺满整个窗口的模态（`Modal(cover: compact)`），如手机的全屏流程 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `form_layout` | 表单版式：`Section(style: plain)` 标题组、行内边距、`Form(gap:, row_height:)` | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `tab_accessory` | 随页签常驻的一条（`TabAccessory`），如小播放器；iOS 26.1 起为标签栏上方的系统附件 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `grid_cells` | 按列宽定行高的网格（`Grid(cell_ratio:, cell_extra:)`），格内内容顶对齐、可伸进行间距 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `size_classes` | 四档尺寸适配 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `text_formats` | 文本中的复数分支、日期、时长，以及显示变体字段的枚举标签（见[文本、内容与格式化](UI.md#文本内容与格式化)） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |

## 不由上述功能标记完整表达的能力

| 能力 | 当前范围 |
| --- | --- |
| `Window(idle_enabled:)` | 六个目标都消费启停状态（能力 `idle_control`）：为假时 `window.idle` 保持为假且不计时，变真时重新计时 |
| 图片跨分栏 Hero | macOS 使用窗口级覆盖层，Web 使用视口级副本；iOS 与 macOS 其他视图沿用匹配几何。Video 连续画面传递另行验证 |
| 本地持久化 | 原生 FFI 宿主提供文件存储，Web app 使用同源 localStorage；未安装存储宿主时不持久化 |
| 可选 Svg | 六目标有生成路径；专属依赖及验证见[动画 SVG](../Implementation/Animated-SVG.md)，不改变普通界面的默认实现 |
| 平台各自的应用图标（`[app.icon]`） | 六个目标都接受方形 PNG 与各自的原生格式：macOS `.icns`、iOS `.appiconset`、Windows `.ico`、Linux hicolor 图标主题、Android `res`、Web 页面图标文件夹或 SVG；配置阶段检查，生成时原样使用。规则见[应用图标](../Design.md#应用图标) |
| `SidebarHeader(role: title)` | WinUI 3 与系统 XAML 不显示（标题栏已显示应用图标与标题）；其他目标照常显示 |

## 最近新增能力的验证

这一节只描述 2026-10-15 至 20 日的新增能力；已有平台的历史验收见各实现记录。构建成功不代替实际设备交互。

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
| 动画 SVG | 六目标生成；示例 macOS、iOS 模拟器、Android、Web 构建；Linux 随真实应用原生构建（WebKitGTK 6）；Windows 本轮未原生构建 | macOS / Web 实测 Logo 与交互；Linux 实测引导页的动画 Logo；Ubuntu 24.04 上 WebKit 沙箱需要 AppArmor 配置，`.deb` 已附带（同形配置下沙箱运行实测，安装未实测），`.tar.gz` 需手动添加，见 Linux 记录；iOS / Android 本轮未运行设备交互，Windows 未验收 |

Firefox 与 Safari 手机布局尚未实测。用于验收的应用播放器仍有模拟状态；播放器界面和图片过渡的验收不等于已接入完整媒体播放功能。

## 来源与维护

- 功能定义及拒绝规则：[bern-target-api 的功能定义](../../crates/bern-target-api/src/lib.rs)与[功能检查](../../crates/bern-target-api/src/check/features.rs)。
- [SwiftUI 清单](../../crates/bern-backend-swiftui/src/lib.rs)及 [iOS 描述](../../crates/bern-backend-swiftui/src/ios.rs)。
- [Web](../../crates/bern-backend-web/src/lib.rs)、[Compose](../../crates/bern-backend-compose/src/lib.rs)、[GTK](../../crates/bern-backend-gtk/src/lib.rs)、[WinUI 3](../../crates/bern-backend-winui3/src/lib.rs) 清单。
- 验收：[macOS](../Implementation/macOS-SwiftUI.md)、[iOS](../Implementation/iOS-SwiftUI.md)、[Android](../Implementation/Android-Compose.md)、[Web](../Implementation/Web.md)、[Linux](../Implementation/Linux-GTK.md)、[Windows](../Implementation/Windows-WinUI3.md)。

新增后端特性时同时更新此表和对应实现记录，并保留对尚未验收场景的说明。
