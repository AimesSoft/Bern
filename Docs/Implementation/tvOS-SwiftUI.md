# tvOS SwiftUI 实现记录

当前组件接口见[组件参考](../Reference/UI.md)，逐项支持与近期验证范围见[平台能力表](../Reference/Platforms.md)，Apple 平台的官方依据见 [Apple 适配规范](../Adaptation/Apple.md)。

- 日期：2026-10-20
- 依据：[Bern 架构设计 v0.33](../Design.md)
- 工具链：Rust 1.99.0（`aarch64-apple-tvos-sim`）；Xcode 27.0，tvOS 27.0 模拟器 SDK
- 验证环境：Apple TV 4K（第 3 代）（1080p）模拟器，tvOS 26.5（M4 Mac）

本文记录 `swiftui + tvos` 目标的实际完成情况。它与 iOS 共用生成器与运行时（`Platform::Tvos`），在电视上是一个铺满屏幕的场景，由焦点引擎与 Siri Remote 操作，没有触摸与指针。这一版构建模拟器应用；真机目标未做。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 平台解析 | 完成 | `platform = "tvos"` 默认 `ui = "swiftui"`，`bern doctor`、`bern build`、`bern run` 认得这个目标 |
| 生成代码可构建 | 完成 | `bern build --target tvos` 生成 Swift 与胶水 crate，构建 Apple TV 模拟器应用包、Mac 上的 CLI 与 MCP 服务器；生成测试按 tvOS 模拟器 SDK 类型检查 |
| 只构建电视显示的部分 | 完成 | `Adaptive` 有 `Layout(size: tv)` 时只保留它，`window.tv` 为真，窗口尺寸档由屏幕决定；永远不显示的分支与组件不生成、不参与能力检查（见“电视布局”） |
| 焦点与遥控导航 | 完成 | 系统焦点引擎与系统焦点效果；`default_focus` 指定起始焦点；滑块是可聚焦的轨道 |
| 遥控按键 | 部分 | `Shortcut` 与控件的 `shortcut:` 对应返回、播放/暂停与触控板边缘；方向键在模拟器实测，返回与播放/暂停未验证（见“验证记录”） |
| 应用图标 | 完成 | `[app.icon].tvos` 接受 `.brandassets` 文件夹或方形 PNG；未声明时由默认图标生成分层图标与顶部货架图 |
| `bern run --target tvos` | 完成 | 装进正在运行的 tvOS 模拟器并启动；生成的 CLI 经 IPC 控制 |
| 真机构建与打包 | 未完成 | 签名、描述文件、安装与 `bern package` 未做；`aarch64-apple-tvos` 的引擎构建未试 |

## 工具链

Rust 1.99.0 的 rustup 为 `aarch64-apple-tvos` 与 `aarch64-apple-tvos-sim` 提供预编译的标准库（tier 2，没有宿主工具），`rustup target add aarch64-apple-tvos-sim` 后用工作区固定的稳定版即可构建。因此没有用 nightly 的 `-Zbuild-std`：那要求换 nightly 工具链、每次构建重新编译标准库，与其他目标用的 1.99.0 不一致，而这里没有它解决得了、预编译标准库解决不了的问题。Intel Mac 上对应的模拟器目标是 `x86_64-apple-tvos`。

最低 tvOS 18.0：运行时用到的 Observation、开关样式的 `Toggle` 与新的标签页 API 从这一版起都有；tvOS 18 支持的设备与 tvOS 17 相同（Apple TV HD 与全部 Apple TV 4K）。

## 工程结构

| 位置 | 内容 |
| --- | --- |
| `crates/bern-backend-swiftui/src/tvos.rs` | tvOS 目标：目标描述、检查、生成与构建步骤 |
| `crates/bern-ir/src/television.rs` | `Application::on_television`：电视显示的应用 |
| `crates/bern-target-api/src/brand_assets.rs` | 分层图标与顶部货架图：检查、生成 |
| `templates/BernUIKit.swift` | 与 iOS 共用的平台部分；`#if os(tvOS)` 里是遥控按键、焦点滑块与窗口的遥控命令 |
| `templates/BernStyle.swift` | 焦点效果、缩放场景里的控件外观、默认焦点 |

应用包与 iOS 相同是扁平结构。`Info.plist` 声明 `UIDeviceFamily` 3、`AppleTVSimulator`、单场景、图标名 `App Icon` 与顶部货架图 `TVTopShelfImage`。

## 与 iOS 的差异

- **场景与窗口**：一个 `WindowGroup`，内容是 `BernVirtualHost`，主窗口铺满屏幕；其他窗口（含设置）以 `fullScreenCover` 铺满屏幕打开，没有标题栏与“完成”按钮（tvOS 自己的全屏页也没有），菜单键（返回）关闭；需要关闭按钮时由窗口内容自己画。tvOS 的 sheet 是屏幕中间的小卡片，放不下表单与分栏。窗口事实报告为 `Virtual`。没有 iOS 的方向、状态栏、iPad 多窗口与 ⌘, 菜单。
- **正文字号**：tvOS 的正文是 29 点，生成的默认字号随之取 29（iOS 17）。应用在 `text` 块里给了电视字号（`font(size: 13, tv: 16)`）时，tvOS 取电视字号。
- **控件**：都是 tvOS 的系统控件，焦点下抬起、变白、带阴影。tvOS 没有的控件按系统做法替换：
  - `Slider` 是 `BernFocusSlider`：一条可聚焦的轨道，焦点在上时触控板左右各移动一步（没有 `step` 时为范围的二十分之一），连续按的一串算一次编辑（最后一次后 0.5 秒结束），辅助功能为可调节元素。`Video` 的进度条同样如此。
  - `PopoverButton` 的内容以 sheet 显示；`Switch(style: button)` 是普通开关（tvOS 没有按钮样式的开关）；`TextInput` 与搜索框用系统输入框，输入时是系统键盘与听写。
  - `Pinch`、`Swipe` 只保留辅助功能的调节，没有手势；`Drop` 没有拖放；`reveal: hover` 的内容一直显示（检查给出警告），焦点像其他控件一样到达它。
  - `Sidebar(width:)` 无效（tvOS 的侧边栏是系统宽度的浮动面板），检查给出警告；侧边栏的搜索框在系统默认位置。
- **焦点效果**：系统按钮用系统的焦点效果。样式自己画的按钮按设计稿的 Apple TV 焦点画（`bernTelevisionFocus`）：样式只在悬停时有背景的是列表行，焦点下变白、内容转深色、放大 1.03；有自己背景的（如屏幕键盘的键）焦点下变白、放大 1.1；其余是图片（海报、画框、缩略图），放大 1.08 带阴影。没有焦点时什么也不画。最初用的 `hoverEffect(.highlight)` 在 tvOS 26 上没有焦点时也画一圈底板与描边，与设计稿不符，已换掉。样式写 `focus: none` 时不显示焦点效果，例如铺满画面、只为接收遥控而存在的按钮。
- **缩放场景**：`ScaleBox` 把按设计尺寸画的场景放大到屏幕（例如 1280 × 720 放大 1.5 倍到 1920 × 1080）。tvOS 的控件按 1080p 设计，放大后会再大一半，所以场景里的控件按设计尺寸画：
  - 胶囊与圆形按钮：设计高度（未写高度时 46，大号 52），焦点下白底黑字、放大 1.1、带阴影；`prominence: plain` 的按钮只是文字（16 点、次要色），焦点下套上白色胶囊。
  - 没有指定字体的系统文字取应用正文的电视字号（`bernSceneBodySize`）；表单的分组标题取设计稿的 12.5 点半粗次要色，不是 tvOS 的大标题。
  - 输入框与搜索框用 tvOS 自己的输入框（它画底板与焦点），只设设计高度与字号。
  - 菜单式 `Picker`：tvOS 的菜单选择器只显示当前值，在 tvOS 上改为一行：左边是标签，行尾是带下拉箭头的胶囊菜单（46 点）。
  - 分段 `Picker`：tvOS 的分段控件无法改尺寸，场景里画成设计稿的分段（40 点、15 点字，选中的一段抬起，焦点的一段变白放大）；不在场景里时仍是 tvOS 自己的。
  - 开关：tvOS 没有开关，只有写着“开”“关”的按钮；改为画出来的开关（64 × 28 胶囊，开时填强调色，白色圆钮），遥控的选择键切换，焦点下外面一圈描边、放大 1.1。不在场景里时按 1.5 倍画。
  - 滑块：细轨道填强调色到白色胶囊滑块（40 × 24），焦点下外面一圈描边；长的滑块不放大，免得盖住旁边的时间。
  - `Modal` 被提到窗口根部显示，按它所在场景的放大倍数放大。
- **无键盘快捷键**：tvOS 没有 `keyboardShortcut`；`shortcut:` 与 `Shortcut` 改为遥控按键（见下）。

`Svg` 在 tvOS 上不可用：动画 SVG 由 WebKit 播放，tvOS SDK 没有 WebKit（已知，编译即报错）。检查器报告用到它的位置；应用在电视布局里换成静态图片。

## 电视布局

电视的窗口永远是整个屏幕，所以窗口相关的条件在构建时就已知。`Application::on_television` 在 tvOS 的检查与生成之前改写应用：

- `Adaptive` 有 `Layout(size: tv)` 时只保留这一个布局；没有时照常按尺寸档选择（电视按 1920 点宽、`[layouts]` 的断点得到尺寸档，常见为 `large`）。
- `window.tv` 为真；`window.compact` 等尺寸档与 `window.short` 由屏幕的 1920 × 1080 点与断点得到；`!`、`&&`、`||` 按已知部分化简；`window.idle` 等运行时才知道的条件保留。
- 条件已知为假的分支去掉，已知为真的分支取代整个条件；只被去掉的部分放置的组件不生成。

因此手机或桌面布局里用到电视没有的能力（例如 `pick:` 选文件）时，tvOS 的检查不会报错，只检查电视实际显示的部分。其他目标上 `window.tv` 在 Android TV 与网页的电视浏览器上为真（见[平台能力表](../Reference/Platforms.md)，未在电视设备上验证），桌面与 iOS 为假，`tv` 布局不被选中。Android TV 的遥控按键与 tvOS 相同：返回键对应 `escape`、播放/暂停键对应 `space`，方向键先移动焦点。

`Stack(under_bars: true)` 在 tvOS 上忽略全部容器安全区，内容铺到屏幕边缘；电视的过扫描边距（HIG：上下 60、左右 80 点）由设计在场景内留出，避免在已有安全区的容器外重复留白。

## 遥控器

`Shortcut(key:)` 与控件的 `shortcut:` 在 tvOS 上对应遥控器的按键：

| 键 | Siri Remote | SwiftUI |
| --- | --- | --- |
| `escape` | 返回（菜单） | `onExitCommand` |
| `space` | 播放/暂停 | `onPlayPauseCommand` |
| `left`、`right`、`up`、`down` | 触控板的方向，焦点无处可移时 | `onMoveCommand` |

带修饰键的组合与其他键在电视上没有对应按键，不生成。每个窗口的根视图把这些命令交给本窗口当前显示且启用的快捷键，同一个键有多个时取最后显示的那个，停用的不参与；没有返回键的快捷键时，返回交给系统（关闭全屏页、回到主屏幕）。方向键只在焦点移不动时到达快捷键，所以“画面上没有可聚焦的控件时左右快退快进”这类设计用一个铺满画面、`focus: none` 的按钮接住焦点。没有快捷键接住的方向按压算作输入，让 `window.idle` 回到假。

`default_focus: true` 写在 `Button` 或 `IconButton` 上，界面出现时焦点从它开始。tvOS 上它是所在焦点区域的首选焦点（`prefersDefaultFocus`），每个标签页是一个焦点区域，所以标签栏下的每页各有自己的起始焦点；其他平台上是键盘焦点（见能力表）。

## 应用图标

`[app.icon]` 的 `tvos` 键：

- `.brandassets` 文件夹原样编译：检查 `Contents.json` 列出的资源都在、每个图像栈有 2 至 5 层、每层的图片存在，且有 400 × 240 的主应用图标。
- 方形 PNG，或未声明时用默认图标：生成分层图标。背景层是图标不透明像素中最常见的颜色（按 5 位量化），前景层是透明底上的图标；图标本身是满幅方形时铺满高度，有透明边时按安全区留出边距。生成 400 × 240（@1x、@2x）的主屏幕图标、1280 × 768 的 App Store 图标、1920 × 720 与 2320 × 720（@1x、@2x）的顶部货架图。

`actool` 以 `--platform appletvsimulator --target-device tv --app-icon "App Icon & Top Shelf Image"` 编译，`Info.plist` 的图标与顶部货架图键由它的局部 plist 给出。

## 本地控制通道

与 iOS 相同：`bern run` 把 Mac 的临时目录经 `SIMCTL_CHILD_BERN_IPC_DIR` 交给应用，Mac 上的 CLI、JSON 客户端与 MCP 服务器经 IPC 连到模拟器里的应用；从主屏幕启动时通道不监听。同一包标识的 macOS、iOS 或 Android 引擎同时在运行时，用 `--engine` 指定。

## 构建与运行

```bash
rustup target add aarch64-apple-tvos-sim
```

```bash
bern build --target tvos
```

```bash
bern run --target tvos
```

`bern run --target tvos` 只找正在运行的 tvOS 模拟器（运行时标识含 `.tvOS-`），没有时报错并提示启动方法。构建步骤：为模拟器构建引擎静态库（`TVOS_DEPLOYMENT_TARGET=18.0`）与为 Mac 构建 CLI、MCP 服务器；把随包资源镜像到包顶层；`actool` 编译图标；`xcrun --sdk appletvsimulator swiftc` 以 `arm64-apple-tvos18.0-simulator` 编译界面；dSYM 移到包旁；ad hoc 签名。

## 验证记录

2026-10-20，Apple TV 4K（第 3 代）（1080p）模拟器，tvOS 26.5，Xcode 27.0。只截取模拟器屏幕（`simctl io screenshot`），没有录屏、没有桌面范围的模拟输入。

模拟器的输入：Simulator 应用的遥控器窗口要在桌面上点按，所以改用一个只对这台模拟器发送 HID 事件的小工具，以模拟游戏手柄的方向键和按钮输入。方向键能移动焦点并产生 `onMoveCommand`；手柄的 A 键在系统按钮、自定义样式按钮与纯样式按钮上都没有触发按钮动作（在一个只含这三种按钮的探针应用里确认），B 键让应用回到主屏幕，没有到达探针里的 `onExitCommand`，X、Y 没有产生播放/暂停。所以按钮动作经 CLI 的 `ui activate` 触发，返回与播放/暂停的映射**未验证**；真实遥控器上 `onExitCommand` 与 `onPlayPauseCommand` 的行为是 Apple 文档给出的（已知），Bern 的对应未在真实遥控器上实测。

- **构建与测试**：`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check` 通过；新增 tvOS 生成测试（Info.plist、生成的分层图标通过与声明图标相同的检查、构建步骤与 `actool` 参数、按 tvOS 模拟器 SDK 类型检查）、电视布局的裁剪与遥控快捷键的生成测试（同一应用在 macOS 上生成键盘快捷键并类型检查）、分层图标的单元测试。
- **其他目标的新代码**：`examples/gallery` 用到电视布局、`Shortcut` 与 `default_focus`；GTK 在 Ubuntu 24.04 虚拟机上过 Clippy 与生成应用的类型检查，WinUI 3 在 Windows PC 上用 MSVC 编译，Android 版 gallery 用 Gradle 构建成功（Kotlin 编译），Web 运行时测试覆盖快捷键让键。都没有运行交互。
- **展示应用**：构建、安装、启动；主屏幕显示生成的分层图标，选中时顶部货架显示生成的货架图。
- **一个实际应用的电视布局**（按设计稿 1280 × 720 场景放大 1.5 倍）：
  - 顶部是系统标签栏（搜索在最前），右上角是“打开”、设置与账户；方向键在标签栏与内容之间、内容的控件之间移动焦点，焦点效果为系统的抬起与变白。
  - 引导页铺满屏幕，起始焦点在“开始”，之后每步在“继续”；CLI 读到 `Adaptive` 选中的布局为 `tv`。
  - “打开链接”的模态按场景放大，焦点只在模态里。
  - 播放器：控制条显示时起始焦点在暂停按钮；控制条隐藏时，铺满画面的按钮接住焦点，方向左右快退、快进 10 秒且不唤醒控制条（位置 558.65 → 549.15，558.65 → 569.4），方向上唤醒控制条；小窗播放在右下角。
  - 新番、媒体库、搜索（含结果）、详情与设置（`fullScreenCover`，tvOS 的表单与侧边栏）页面显示正常。
- **未验证**：真实遥控器；返回与播放/暂停的映射；手柄 A 键之外的“选择”（模拟器里按钮动作经 CLI 触发）；真机；HDR 与杜比视界的显示；Top Shelf 的动态内容（未实现）。

同日第二轮：对照设计稿电视模式（`design/app.html` 选“电视”）逐屏量取后修正。第一轮的实际应用复用了桌面的组件与尺寸，没有设计稿给电视的那套覆盖（字号、页边距、按钮高度），设置页行标签丢失、搜索结果太小、开关显示成“开/关”文字等问题都来自这里。修正：

- Bern：文本样式的电视字号（`font(tv:)`，六个目标）；文字会话动作 `push_str`、`pop`（六个目标，屏幕键盘用）；`Search(suggest: true)`（六个目标）；上面“缩放场景”一节的控件画法；自绘按钮的电视焦点；电视上其他窗口不加标题栏与“完成”。
- 实际应用：字号、56 点页边距、首页舞台 560 × 315、新番 500 高、媒体库六列海报与 40 点筛选、详情、全屏引导、屏幕键盘搜索页、全屏设置（300 点的页面面板、页标题、关闭按钮）、右上角顺序。
- 模拟器截图对照：首页、新番、媒体库、详情、搜索（含输入首字母与删除）、设置的通用 / 外观 / 播放 / 弹幕 / 可选功能 / 关于、引导四步、播放器、打开链接、小窗，位置与设计稿相差在几点以内。
- 观察到的问题（已在应用里绕开，Bern 里未查明）：tvOS 按钮里的 `Row`，其中一项是带 `Spacer` 的 `Row` 时，文字被压到最小宽度（新番收起的星期列标题）；改成不用 `Row` 后正常。

2026-10-21，窗口正文的摆放（规则与修正见 macOS 实现记录的同日条目，tvOS 共用同一段生成）：Apple TV 4K（第 3 代）（1080p）模拟器，tvOS 26.5，探针应用经 CLI 切换正文，只截模拟器屏幕。改后默认列从左上排起、按钮保持自身宽度，写在组件里的 `Column(align: center)` 水平居中、靠顶，只有一个小层的 `Stack` 在屏幕正中。改前没有在电视上截图。

## 未完成与已知限制

- 只构建模拟器应用；真机需要签名与描述文件，安装与 `bern package` 未实现。
- `Svg` 不可用（tvOS 没有 WebKit）；动画 Logo 在电视上用静态图片。
- 选文件（`pick:`）、拖放、悬停显示在电视上没有对应的系统部件；这些能力只能出现在电视不显示的布局里。
- 设置窗口侧边栏的搜索框获得焦点时，系统键盘行会盖住详情的上部（tvOS 的系统行为）。
- 模拟器里的输入受工具限制，见上文。
