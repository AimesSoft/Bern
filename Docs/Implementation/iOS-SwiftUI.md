# iOS SwiftUI 实现记录

当前组件接口见[组件参考](../Reference/UI.md)，逐项支持与近期验证范围见[平台能力表](../Reference/Platforms.md)。下文保留各阶段实现和验收记录。

- 日期：2026-10-12
- 依据：[Bern 架构设计 v0.24](../Design.md)
- 工具链：Rust 1.99.0（`aarch64-apple-ios-sim` 与 `aarch64-apple-ios`）；Xcode 26.5，iOS 26.5 SDK，Swift 6.3.2；iPad 窗口化模式的后半部分用 Xcode 27.0，iOS 27.0 模拟器 SDK，Swift 6.4
- 验证环境：iPhone 17 Pro 与 iPad Pro 13 英寸（M5）模拟器，iOS 26.5（M4 Mac）

本文记录 `swiftui + ios` 目标的实际完成情况。同一个应用包在 iPhone 与 iPad 上运行，窗口的呈现方式在运行时按设备决定（见“iPad”）。展示应用（`examples/showcase`）的声明、协议、业务核心和 IR 不变，只增加了 `[targets.ios]`；生成器与 macOS 共用，按平台区分窗口、场景与少数控件样式。这一版构建模拟器应用；真机目标能编译链接，但还没有签名、安装与打包。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target ios` 生成 Swift 与胶水 crate，构建模拟器应用包、Mac 上的 CLI 与 MCP 服务器；生成测试按 iOS 模拟器 SDK 对生成的 Swift 做类型检查 |
| 窗口、导航、菜单与生命周期按平台区分 | 完成 | 运行时分为共用的 `BernRuntime.swift`、`BernMedia.swift` 与平台文件 `BernAppKit.swift`（macOS）、`BernUIKit.swift`（iOS），构建只编译自己平台的那一个 |
| 不支持原生多窗口时用虚拟窗口 | 完成 | iPhone：一个场景，主窗口铺满，其他窗口是叠在上面的 sheet；窗口事实报告为 `Virtual` |
| 支持多窗口的环境按已验证的原生能力选择 | 完成 | iPad：除设置外每种窗口是独立的原生窗口（`WindowGroup` 场景），报告为 `Native`；设置是主窗口上的 sheet |
| 前后台生命周期 | 完成 | 进入后台时窗口报告为隐藏、失去焦点，回到前台恢复；媒体按 AVFoundation 的实际状态报告 |
| 屏幕旋转、键盘、安全区域 | 部分 | 键盘输入、安全区域与 iPad 窗口缩放（停在最小尺寸）实测；旋转由 SwiftUI 处理，未实测 |
| 状态可订阅；带参数命令与错误返回 | 完成 | 与 macOS 相同的运行时：引擎状态、会话草稿、查询、关闭协议 |
| Video 原生呈现 | 完成 | AVFoundation 执行端与 `AVPlayerLayer` 呈现、命令驱动的控制条 |
| 真机构建与打包 | 未完成 | 引擎与界面能按 `aarch64-apple-ios` 编译链接；签名、描述文件、安装与 `bern package` 未做 |

## 工程结构

| 位置 | 内容 |
| --- | --- |
| `crates/bern-backend-swiftui/src/ios.rs` | iOS 目标：目标描述、检查、生成与构建步骤 |
| `crates/bern-backend-swiftui/src/swift/` | 两个平台共用的生成器，`Platform` 决定差异 |
| `templates/BernUIKit.swift` | iOS 平台部分：虚拟窗口宿主、sheet、窗口事实、开发快照、视频视图 |
| `templates/BernAppKit.swift` | macOS 平台部分，从原来的 `BernRuntime.swift` 分出，行为不变 |

生成的应用包是 iOS 的扁平结构：可执行文件、`Info.plist`、各语言的 `.lproj` 与随包资源都在包的顶层。图标由 `actool` 把 1024 像素的 PNG 编译进 `Assets.car`（未声明图标时是 Bern 标志的方形版本，声明的较小图标先用 `sips` 放大；`[app.icon].ios` 为 `.appiconset` 文件夹时原样编译，其中的深色、着色版随之生效）。`Info.plist` 声明启动屏（否则应用以兼容模式显示，不铺满屏幕）、多场景（iPad 的多窗口；iPhone 始终只有一个场景）、iPhone 与 iPad、iPad 上的全部四个方向、最低 iOS 17.0。

最低 iOS 17：与 macOS 14 同一代，运行时用到的 Observation、`ContentUnavailableView` 等从这一版起提供。

## 与 macOS 的差异

生成器只在以下几处按平台区分：

- **场景**：macOS 每个窗口种类一个 `Window` 或 `Settings` 场景；iOS 没有这两种场景，主窗口与设置以外的每种窗口各是一个 `WindowGroup`，主窗口的场景内容是 `BernVirtualHost`（它在 iPhone 上承载所有虚拟窗口，在 iPad 上承载设置）。
- **窗口尺寸**：声明的宽高与最小尺寸只用于人能调整大小的窗口，即 iPad；iPhone 上由屏幕与 sheet 决定。
- **侧边栏**：iOS 上分栏视图在窄屏时折叠为导航栈，侧边栏是第一屏，标题为窗口标题（`.navigationTitle(bernWindow.title)`）。侧边栏控件登记在整个分栏视图上，所以只显示详情时外部入口也能切换页面（macOS 上效果相同）。
- **文本输入**：iOS 的表单只显示输入框的提示文字，不显示标签。单行与密码输入改为 `LabeledContent`（标签在左、输入在右，与“设置”应用相同），多行输入的标签放在上方。
- **控件样式**：`Switch` 的 `checkbox` 样式在 iOS 上是开关（iOS 没有复选框）；`Picker` 的 `radio` 样式在 iOS 上是带勾选标记的内嵌列表；`chips` 与 macOS 相同，是 30 点的系统玻璃胶囊按钮（见 macOS 文档“标签选择器与换行工具条”）。

## 窗口

iPhone 只能有一个场景，按设计文档用虚拟窗口，形式遵循 iOS 的习惯（iPad 上设置窗口同样如此）：

- **主窗口**铺满场景。被命令关闭后，场景显示应用名与“打开“…””按钮，点按后重新打开（新的实例代次）。
- **其他窗口**是 sheet，按打开顺序一层叠一层：导航栏显示窗口标题，右上角“完成”按钮经关闭协议请求关闭。设置窗口的页签是 sheet 底部的标签栏。
- **下滑关闭**：没有待确认的修改时可以下滑关闭，作为系统事实报告（`changed_by: System`）；关闭检查需要确认时禁止下滑，只能用“完成”按钮，此时弹出存储、不存储、取消。
- **窗口动作**：只有最上面的窗口可以激活；它和主窗口可以关闭。被 sheet 盖住的窗口不能被提到前面（iOS 只能先关掉上面的），`window.activate` 与 `window.close` 返回 `target_unavailable`；`minimize`、`restore`、`maximize`、`move`、`resize` 同样返回 `target_unavailable`，并列出可用动作。
- **窗口事实**：可见性跟随场景（进入后台为隐藏），帧是窗口根视图在场景中的位置（点），只有活动场景中最上面的窗口有焦点。

## iPad

同一个应用包在 iPad 上按苹果的说明处理窗口：

- **能否开多个窗口**：SwiftUI 应用在 iPadOS 上声明了 `UIApplicationSupportsMultipleScenes` 时可以打开多个窗口（`supportsMultipleWindows` 的说明），iPhone 上同样的声明不起作用。运行时在启动引擎前按“iPad 且声明了多场景”决定，经 `bern_app_native_windows` 告诉引擎，窗口事实里的呈现方式因此与实际一致：iPad 上是 `Native`，iPhone 上是 `Virtual`。
- **窗口**：iPadOS 没有 macOS 的 `Window`（单个窗口）场景，只有 `WindowGroup`。每种窗口是一个 `WindowGroup(id:for:)`，打开时以窗口种类作为值（`openWindow(id:value:)`）；SwiftUI 对同一个值不新开窗口而把已有的调到前面，与 Bern 每种窗口默认一个实例的规则一致。关闭用 `dismissWindow(id:value:)`，激活即再次打开。窗口标题是声明的标题，系统的窗口列表与菜单栏用它区分窗口。
- **设置**：iPadOS 没有 `Settings` 场景；设置窗口是主窗口上的表单 sheet，与 iPhone 相同。人机界面指南要求接了键盘时 ⌘, 打开设置，生成的 `.commands` 把 iPadOS 菜单栏里的“设置”项换成打开设置窗口，快捷键 ⌘,。
- **次级窗口的外观**：内容没有自带导航时放在带行内标题的导航栈里。WWDC25 的《Elevate the design of your iPad app》要求工具栏包住窗口控件，否则系统使用只为兼容而存在的安全区；有了工具栏，窗口化时窗口控件排在标题栏前缘。
- **尺寸**：TN3192 要求支持可调整大小的场景（启动屏、iPad 上全部四个方向、不用 `UIRequiresFullScreen`），SwiftUI 以 `.windowResizability(.contentMinSize)` 与内容的最小 `frame` 表达最小尺寸。生成代码对声明了最小尺寸的窗口加上这两者（`frame` 只在 iPad 上生效，iPhone 不受影响），声明了宽高的窗口加 `.defaultSize`。
- **窗口事实**：每个窗口的可见性来自它自己场景的阶段（全屏模式下切到另一个窗口时，原来的窗口为隐藏），帧是窗口根视图在屏幕上的位置。窗口化模式下几个窗口的场景同时是活动的，系统窗口也各自是自己场景的 key，只有最前面的窗口“显示为活动”，所以焦点还要看 SwiftUI 的 `appearsActive`（iOS 18 起）；iOS 17 没有这个值，焦点只看场景阶段与 key。被设置 sheet 盖住的主窗口不能激活，与 iPhone 相同。没有公开接口最小化、移动或调整 iPad 窗口，这些命令返回 `target_unavailable`。
- **导航**：常规宽度下分栏视图并排显示侧边栏与详情，侧边栏顶部是窗口标题；`Adaptive` 按内容宽度选择布局，视频页在 iPad 上是展开布局。

## 本地控制通道

模拟器里的应用是 Mac 上的进程，可以和 Mac 共用文件系统，所以 CLI、JSON 客户端与 MCP 服务器（为 Mac 构建）能经 IPC 通道连到它。应用自己的临时目录在很深的容器路径里，套接字路径超过 Unix 套接字的长度上限；`bern run` 因此把 Mac 的临时目录经 `BERN_IPC_DIR` 交给应用（`simctl` 的 `SIMCTL_CHILD_` 变量），通道在其中建立套接字。iOS 上通道只在设置了这个变量时监听：从主屏幕点按启动或装在真机上时，没有别的进程能连到应用，通道不启动。

`BERN_IPC_DIR` 在其他平台上同样有效：它代替临时目录作为各应用套接字目录的上级目录。

模拟器里的应用与 macOS 版包标识相同时，两者的引擎在同一个目录里，CLI 会列出两个引擎并要求用 `--engine` 指定。

iOS 会挂起完全在后台的应用，挂起时引擎不应答。客户端的握手因此有 5 秒的超时，超时报告“引擎不应答”，不再无限等待；握手之后的命令不受这个超时限制。

## 媒体

执行端与 macOS 共用（`BernMedia.swift`）。视频视图是只呈现的 `UIView`，以 `AVPlayerLayer` 显示播放器，不处理触摸，不会自行改变播放。第一个播放器创建时把音频会话设为 `.playback`（影片模式），静音开关打开时视频仍有声音，与视频应用相同。

播完后若播放器没有收到跳转命令就离开了结尾，它不再是“已结束”：iOS 在应用进入后台时把已播完的播放器移回开头（AVFoundation 报告的当前时间变为 0），这时报告为暂停、位置 0，再播放从头开始。这个处理在两个平台共用的执行端中，macOS 不受影响。

## 构建与运行

```bash
rustup target add aarch64-apple-ios-sim
bern build --target ios
open -a Simulator
bern run --target ios
```

`bern run --target ios` 把应用装进正在运行的 iOS 模拟器并启动，应用的输出显示在终端，直到应用退出；`BERN_*` 环境变量（例如 `BERN_APPEARANCE=dark`）一并交给应用。没有正在运行的模拟器时报错并提示启动方法。不带 `--target` 的 `bern run` 优先选择与本机系统相同的目标，Mac 上仍然启动 macOS 版。`bern doctor` 检查 iOS 模拟器 SDK 与 Rust 的模拟器目标；有没有可运行的模拟器只影响 `bern run`，不影响构建（Xcode 更新后模拟器设备要过一阵才加载完，此时构建不应失败）。

构建步骤：为模拟器构建引擎静态库（`IPHONEOS_DEPLOYMENT_TARGET=17.0`）与为 Mac 构建 CLI、MCP 服务器；把随包资源镜像到包顶层；`actool` 编译图标；`xcrun --sdk iphonesimulator swiftc` 编译界面；dSYM 移到包旁；ad hoc 签名。

## 验证记录

2026-10-12，iPhone 17 Pro 模拟器（iOS 26.5），由生成的 CLI 与模拟器截图驱动：

- **构建与测试**：`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check` 通过；新增 iOS 生成测试（按 iOS 模拟器 SDK 类型检查、Info.plist、构建步骤、资源命名规则）。真机：引擎按 `aarch64-apple-ios` 构建，界面按 `arm64-apple-ios17.0` 编译链接，得到平台为 iOS、最低 17.0 的 arm64 可执行文件；未签名、未安装。
- **外观**：iOS 26 的标准控件与 Liquid Glass 导航栏、搜索栏与标签栏；侧边栏在底部有搜索栏；按钮页的工具栏按钮、开关与滑块页的分段与菜单选择器、滑块两端图标均为系统控件。`BERN_APPEARANCE=dark` 时全部为深色。
- **导航**：启动时直接显示详情（标题与文字），返回后是侧边栏；点按侧边栏项进入对应页面；CLI 的 `ui select --target main_window.pages` 在只显示详情时也能切换页面；列表页点按“影片”推入项目详情。
- **输入**：点按名字输入框后用键盘输入“Ada”，预览即时显示“你好，Ada”。
- **窗口**：“打开设置…”以 sheet 打开设置窗口，主窗口失去焦点且只剩关闭动作；输入名称后点“完成”弹出存储确认，选“存储”后名称写入引擎并关闭；未修改时下滑关闭（`changed_by: System`），有未存储修改时下滑无效；CLI 关闭返回 `ConfirmationRequired`，期间控件为 `blocked_by_modal`，`Discard` 后关闭；`minimize`、`resize` 与激活被盖住的主窗口返回 `target_unavailable`；关闭主窗口后显示“打开“Bern 控件展示””，点按后主窗口以 `main#2` 打开；`ui set-language --language en` 后界面全部变为英文。
- **生命周期**：按 Home 键后主窗口为 `Hidden`、无焦点，CLI 仍能读取状态；回到前台后恢复 `Visible` 与焦点。
- **媒体**：预览播放器播放色条片段、跳转与结束（`Ended 6000 / 6000`）；离开并回到视频页后状态不变；播放中进入后台时模拟器里继续播放直到结束；播完后进入后台报告为 `Paused 0`，再播放从头开始。视频页窄屏时片段选择是“换片”菜单。
- **图片**：随包图片与 Grid 正常；PQ 的 PNG 做了色调映射。带增益图的 JPEG 在模拟器里按 HDR 解码得到全黑的像素（把解码结果画到 8 位上下文读回为 0），所以模拟器构建只解码 SDR 版本，显示作者给的 SDR 图。
- **MCP**：`showcase-mcp` 完成 `initialize`，`window_list` 返回模拟器里应用的 `Virtual` 窗口。
- **macOS 未受影响**：运行时拆分后重新构建运行 macOS 版，设置窗口、关闭协议、最小化与恢复由 CLI 实测正常。

iPad，同日，iPad Pro 13 英寸（M5）模拟器（iOS 26.5，全屏模式）。展示应用只有主窗口与设置窗口，为测试多窗口临时加了一个普通的“检查器”窗口（未提交）：

- 主窗口为 `Native`、可见、有焦点（帧 1032 × 1324），分栏视图并排显示侧边栏与详情，右下角是 iPadOS 26 的窗口缩放手柄。
- `window open --kind inspector` 打开独立的窗口（`Native`，带“检查器”标题栏），全屏模式下切到它，主窗口为 `Hidden`；`window activate --window main#1` 切回主窗口；再次打开检查器得到同一个实例 `inspector#1`；`window close` 关闭检查器，系统回到主屏幕，主窗口仍在后台。
- 设置以表单 sheet 显示在主窗口上（`Virtual`，宽 580），此时主窗口无焦点，检查器在后台。
- 视频页在 iPad 宽度下是展开布局，预览播放器播放正常。
- 应用整个进入后台后被挂起，原来的 CLI 在握手时一直等待；加上握手超时后改为报告引擎不应答（新增测试）。
- 回到 iPhone 回归：三个窗口都是 `Virtual`，检查器是带“完成”按钮的 sheet，被设置盖住时 `window activate` 返回 `target_unavailable`。

iPad 窗口化模式，同日，同一模拟器在“设置 > 多任务与手势”中选了“窗口化 App”；后半部分换到 Xcode 27.0 重新构建：

- 窗口带 iPadOS 26 的窗口控件，主窗口的控件排在侧边栏工具栏前缘，检查器的控件排在它的标题栏前缘；拖动右下角缩放时停在声明的最小尺寸 720 × 480；主窗口与检查器可以同时显示、互相部分遮挡。
- 焦点：最初按“场景活动且系统窗口为 key”报告，两个窗口同时报告有焦点——窗口化时两个场景都是活动的，系统窗口也都是 key。之后改为记住最近被命令或场景激活带到前面的窗口，命令切换正确，但点按后面的窗口时两个场景都保持活动、没有任何阶段变化，记住的窗口就过时了；打开新窗口时，后面窗口的场景还会先变为不活动再变回活动，被当成了“到了前面”。最后改用 `appearsActive`：`window open`、`window activate` 来回切换与点按两个窗口，每次都只有最前面的窗口报告有焦点。
- 按 Home 键时 iPadOS 把窗口推到屏幕边缘露出主屏幕，应用没有被挂起（CLI 立即得到回答），场景仍是活动的，窗口仍报告可见、主窗口有焦点；点程序坞图标回来后不变。
- 重新启动应用时 iPadOS 只恢复了上次最前面的检查器，主窗口留在“隐藏窗口”里，引擎只看到 `inspector#1`；`window open --kind main` 把主窗口找回来。
- `window close` 关闭检查器，主窗口得到焦点；设置在 iPad 上以 sheet 打开（`Virtual`），关闭后主窗口恢复焦点。
- 有一次重装后应用窗口全黑、没有控制通道（进程里没有 IPC 线程，环境里没有 `BERN_IPC_DIR`），同时留下一份启动时在 `UIApplicationMain` 准备阶段读取包信息崩溃的报告，像是安装与启动重叠；重新运行后正常，没有再出现。
- iPhone 17 Pro 回归：检查器与设置依次叠为 sheet，被盖住的窗口没有可用动作，点“完成”关闭检查器后主窗口恢复焦点。

展示应用随之调整：图片页的两行图片从 `Row` 改为 `Grid`，原来四张 120 宽的图片在 iPhone 上放不下、被两侧裁掉，现在窄屏时自动换行，宽窗口中仍是一行。

### 标签选择器与换行工具条（2026-10-20）

iPhone 17 模拟器（iOS 26.1），经 CLI 打开设置 › 插件：第一行是分段控件「市场 / 已安装 3」与靠右的「全部」「弹幕」「画面」，「工具」「外观」换到第二行靠左，与原型苹果版手机布局一致；chip 是 30 点的系统玻璃胶囊，选中的「全部」为主题色。首次引导的按钮在 iOS 上同时挂载了两份（`ui activate` 报 `target_ambiguous`），与本次改动无关，当时未处理（已修复，见下一节）。

### 控件登记不留残项（2026-10-20）

iPhone 17 模拟器（iOS 26.1），一个真实应用首次启动即显示全屏的引导流程（`Modal(cover: compact)`），用生成的 CLI 检查：

- **现象**：`ui inspect` 把引导脚部的 `skip`、`start` 各列两次，`ui activate --target onboarding_footer.skip` 报 `target_ambiguous`；被引导盖住的首页也有许多控件重复 2～5 次，窗口里共 125 项。走完引导后 `skip`、`start` 仍在列表里，纯播放页上还列着只有宽屏才显示的桌面版按钮；启动后再打开引导，桌面版的那些按钮又多一份。Mac 与 Android 上每个目标只列一次。
- **原因**：首个布局时还不知道宽度。窗口宽度为 0 时不算 compact，`Adaptive` 的宽度为 `nil` 时按无限宽取布局，所以先挂上宽屏布局（模态先排成浮动面板），量到 402 后再换成手机布局。控件原来在 `.task(id:)` 里登记、在 `onDisappear` 里注销；iOS 26.1 上任务在这次更新之后才开始，这些一闪而过的控件先被注销、后被登记，从此留在列表里。macOS 27 上任务随视图出现立即执行，所以 Mac 上看不到：在屏幕外窗口里打印出现、任务与消失的顺序，任务总在出现之前，按 macOS 15 的部署目标编译也一样。
- **修复**：`BernControl` 在 `onAppear` 登记，窗口、目标、标签或可用状态变化时重新登记，在 `onDisappear` 注销，全部同步。Compose（`SideEffect` 与 `DisposableEffect`）、Web（`onCleanup`）、GTK 与 WinUI 3（作用域清理）本来就同步配对，没有这个问题。首个布局仍会先构建一次宽屏布局再换掉，只是不再留下登记（已修复，见下一节）。
- **测试**：`tests/control_registry.rs` 在屏幕外窗口里运行真实的 SwiftUI 修饰符：模态式的先浮动后铺满、`Adaptive` 式的先宽后窄，每个目标只列一次、`find` 能找到，移除后清空，再插入后恢复；另一项检查登记不在任务里，因为 macOS 27 无法在运行时重现晚开始的任务。
- **验证**：修复后首次启动，`ui inspect` 共 32 项，`onboarding_footer.skip` 与 `.start` 各一次，桌面版的 `stage.*`、`timeline.schedule` 不再出现；`ui activate --target onboarding_footer.skip` 跳到最后一步，`ui activate --target onboarding_footer.finish` 关闭引导，之后只剩手机纯播放页的 9 个控件和 `main_shell.shell`，引导的目标全部消失；启动后再打开引导，每个目标一次，桌面版控件不再出现。

### 首个布局就是手机布局（2026-10-20）

- **问题**：上一节的修复之后，手机上每挂一棵新的子树——应用启动时，或条件切换时新建的 `Adaptive`——仍先构建一次宽屏布局，`Modal(cover: compact)` 仍先排成浮动面板，量到宽度后才换。白做的构建会带出入场动画、共享元素计数、播放器绑定等副作用。
- **修复**：窗口量到宽度之前，`window.compact` 等档位与 `window.short` 按平台给出的尺寸计算（`BernWindowContext.layoutWidth`、`layoutHeight`）：只有一个场景时取场景的尺寸；能开独立窗口时（iPad）取屏幕的尺寸，因为新窗口可能开成任意大小。两者都不小于窗口实际得到的尺寸，宽窗口不会先构建窄布局。`Adaptive` 在量到自己的可用宽度之前取所在窗口的宽度，可用宽度不会比窗口更宽。macOS 在首次布局前不知道窗口尺寸，仍从最宽的档开始。
- **测试**：`tests/adaptive_first_layout.rs` 在屏幕外窗口里运行真实的 `BernAdaptive`，记录每个布局的构建次数：390 宽、平台尺寸为手机的窗口，启动时与之后插入的 `Adaptive` 都只构建 `compact`，盖满式模态只构建铺满的一种；1000 宽的窗口，平台尺寸为平板屏幕或未知（macOS）时都只构建 `expanded`。同一测试换成改前的运行时会失败：启动时 `expanded` 与浮动面板各构建一次。
- **验证**：iPhone 17 模拟器（iOS 26.1），一个首屏有 `Adaptive(id: start)` 与 `Modal(cover: compact)`、打开开关后插入 `Adaptive(id: inserted)` 的最小应用，在布局与模态的构建处临时打印选中的尺寸（验证后删除）：
  - 改前：启动时 `start` 先构建 `expanded`，模态先构建浮动面板，量到宽度后再分别换成 `compact` 与铺满；`ui set-bool --target home.more --value true` 插入的 `inserted` 也先构建 `expanded`。
  - 改后：启动时窗口宽度还是 0，按场景宽 402 估计，`start` 第一次构建就是 `compact`，模态第一次构建就铺满；插入的 `inserted` 第一次构建就是 `compact`（自身宽度未知，窗口宽 402）。整个过程没有构建过 `expanded`，`ui inspect` 里两个 adaptive 一开始就是 `compact`。
  - 示例 gallery 在手机上切到胶片页（`ui select --target main_window.pages --option Film`），`film.film_controls` 同样只构建 `compact`。

### 直接写在窗口正文里的样式变体（2026-10-20）

生成器测试 `a_style_variant_in_a_window_is_defined` 为 iOS 生成窗口正文里的 `variant: stage`，`BernStyles` 有 `buttonStage`，按模拟器 SDK 做类型检查通过（原因见 WinUI 3 实现记录的同日条目）。模拟器当时没有启动 iOS 设备，没有看画面。

直接写在窗口正文里的 `Drop(opens: true)` 此前不进 Info.plist 的文档类型与导入类型，原因与修正见 macOS 实现记录的同日条目；生成器测试 `a_drop_in_a_window_lists_its_document_types` 也为 iOS 断言导入类型。

### 窗口正文的摆放（2026-10-21）

窗口正文比窗口小时从内容区的左上角排起（规则与修正见 macOS 实现记录的同日条目，iOS 共用同一段生成）。iPhone 17 Pro 模拟器（iOS 26.5），探针应用经 CLI 切换正文，`simctl io screenshot` 截屏：改前默认列整体在屏幕正中（水平、竖直都居中）；改后默认列从安全区左上角排起、按钮保持自身宽度，写在组件里的 `Column(align: center)` 水平居中、靠顶，只有一个小层的 `Stack` 在内容区正中；图标按钮、菜单、浮层按钮在默认列里保持自身宽度。生成器测试 `content_in_a_window_starts_at_its_top_leading_corner` 按模拟器 SDK 类型检查 iOS 的生成。

### 与框架类型同名的值类型（2026-10-21）

值类型在生成的 Swift 中写成 `App_` 加声明的名字，原因与写法见 macOS 实现记录的同日条目（iOS 共用同一段生成）。`tests/framework_names.rs` 为 iOS 生成示例 `examples/framework-names`，按 iOS 27.0 模拟器 SDK 做 swiftc 类型检查，通过；UIKit 运行时 `BernUIKit.swift` 也在检查之内。没有在模拟器上运行。

### 与框架类型同名的组件与会话（2026-10-21）

组件写成 `AppView_` 加声明的名字、会话类写成 `App_` 加声明的名字、视图读会话的变量写成 `session_` 加声明的名字，原因与实测见 macOS 实现记录的同日条目（iOS 共用同一段生成）。加入这些组件与会话的示例 `examples/framework-names` 按 iOS 27.0 模拟器 SDK 做 swiftc 类型检查，通过；没有在模拟器上运行。

## 错峰、比例轨道、浮层按钮与四档尺寸

2026-10-14，v0.30，iPhone 17 Pro 模拟器（iOS 26.5），与 macOS 共用同一套 SwiftUI 实现，启动时用 `SIMCTL_CHILD_BERN_MOTION=slow` 放慢动效。

- **错峰**：录下模拟器画面（`simctl io recordVideo`，只含模拟器）逐帧查看，从胶片页回到图案页时卡片按 Gingham、Tartan、Madras 的顺序依次淡入升起。
- **比例轨道**：胶片页的时间轴、刻度与名字在窄屏上按比例排开；`film.level` 设为 0.8 后竖线与底色移到 80%，底色与面板等高。名字用 `anchor: start`，窄屏时最后两个名字会相互挤压，属于界面声明。
- **浮层按钮**：画框页静置后控件隐藏；后台循环等待 `frame.volume` 挂载，再用模拟器点按画面唤醒，挂载后立即 `ui activate`：按钮值为 `open`，浮层中的音量滑块挂载、值为 0.7。截图中 iPhone 上显示的是指向按钮的气泡（`presentationCompactAdaptation(.popover)`），外观随画框页为深色。
- **四档尺寸**：iPhone 竖屏时胶片页的 `Adaptive` 为 `compact`，显示两个图标按钮。iPad 与横屏未测。

## 未完成与已知限制

- 只构建模拟器应用。真机需要开发者签名与描述文件，安装与 `bern package` 未实现。
- HDR：模拟器显示不了 HDR，增益图在模拟器里只解码 SDR 版本；真机上图片按 HDR 解码，视频由 `AVPlayerLayer` 显示，HDR 均未验证。
- 真机上没有声明后台音频，进入后台后系统会暂停播放；只在模拟器里观察过（模拟器里继续播放）。
- iPadOS 菜单栏中的“设置”与 ⌘, 只经过类型检查，未实测（模拟器上没有接键盘的菜单栏操作）。
- iOS 17 没有 `appearsActive`，几个窗口同时显示时可能都报告有焦点；手上没有 iOS 17 的模拟器，未验证。
- iPadOS 重新启动应用时只恢复它记住的窗口，主窗口不一定在启动时打开；Bern 按系统恢复的窗口报告。
- 全屏模式下关闭当前窗口后，系统回到主屏幕，不会切到应用的其他窗口；其他窗口仍在后台，从程序坞或应用切换器回到它们。
- 屏幕旋转未实测。
- 被 sheet 盖住的窗口不能单独激活或关闭，需要先关掉上面的窗口。
- 窗口内容自带分栏或导航栈时，它作为 sheet 打开时没有“完成”按钮（嵌套的导航栈会出问题），只能下滑或用命令关闭。

## 手机外壳与像素对照（2026-10-18）

iPhone 17e 模拟器（iOS 26.5，390 × 844 点，截图 1170 × 2532）上，以一个真实应用的手机首页对照它的网页原型。原型用无界面 Chrome 按 3 倍渲染成同尺寸图片，两边用同一工具逐行、逐列找出内容边界比较；以下改动后，首页各元素的位置和尺寸与原型相差都在 1 点以内（中文字形见下文）。

- **搜索面板**：`Search(style: palette)` 与 `SearchItem` 在 iOS 上也生成同一个面板（原先只有 macOS 接受）。输入框换成系统 `TextField`，点遮罩关闭，硬件键盘的方向键、回车、Esc 仍可用；macOS 保留 AppKit 输入框和窗口点击监视。生成器回归测试为 iOS 生成并类型检查同一夹具。
- **`Tabs` 里的 `Pages`**：编译期把模块页面展开成 `Tab`，内容是页面组件，和侧栏里的 `Pages` 同源。选中的标签取应用强调色（`.bernAccentTint()` 加在 `TabView` 上）；普通玻璃按钮和圆形图标按钮在 iOS 上改用主文字色（`bernPrimaryLabel`），不被标签栏的强调色染色。
- **`Adaptive` 量可用宽度**：原先测的是所选布局自己的宽度，`frame(maxWidth:)` 没有最小值时不小于内容，宽屏布局先被选中就一直量出宽屏。现在 `minWidth: 0`，量的是父视图给的宽度。
- **固定尺寸的 `Stack` 按 `align` 摆放**：`Stack(align:, width:, height:)` 的 `.frame` 带上同一对齐，内容不再总在正中。
- **无圆角的表面铺满安全区**：没有 `corner` 的 `Stack` 样式背景用 SwiftUI 的形状样式背景，延伸到状态栏和标签栏下面。
- **`Button(width: fill)`**：系统按钮随标签撑满容器宽度（能力 `fill_buttons`，SwiftUI 与 Web）。
- **`IconButton(height:)`**：指定直径的系统玻璃圆钮，和指定高度的胶囊控件同一做法；iOS 原生圆钮只有约 31 与 47 点两档。胶囊控件按设计尺寸档取字号：42 点及以上为 15 点半粗，其余 13 点中等。
- **`Row` 按项目自己的宽度排**：`HStack` 把空间按弹性从小到大平分给各项，一行放得下时也会先给某个可截断的短文字不足它宽度的一份，把它截成省略号（实测一行三项、标签组只分到 81 点而需要 88 点）。现在 `Row` 生成 `BernRow`（`Layout`），规则与其他目标的行相同：放得下时每项取自己的宽度，剩余空间由会伸展的项（`Spacer`、撑满的框）尽量平分；放不下时其余项按各自宽度的比例收缩，不小于各自的最小宽度。行里的 `Spacer` 带上 `BernRowSpacer` 标记，只沿行伸展、不参与行高；行里的 `Divider` 包一层 `HStack`，保持竖线方向。行与列里的 `Spacer` 都生成 `Spacer(minLength: 0)`，`BernRow` 也不取它的最小宽度：系统 `Spacer()` 的最小长度随平台不同（macOS 实测 8 点，tvOS 60 点），其他目标的 `Spacer` 没有最小长度。
- **定宽高的 `Column` / `Row` 从起始边排内容**：原先 `.frame(width:)` 把较窄的内容放在正中，`Column(width: 58)` 里的文字因而偏右。现在列从顶部开始、横向按 `align`（默认前缘），行从前缘开始、纵向按 `align`，与其他目标一致。
- **手机下拉胶囊**：iOS 的 `Menu(shape: capsule)` 按设计 6.3 排列：符号在 16 点框内，箭头在 12 点框内、60 % 不透明度，间距 6，箭头一侧离边 11 点。`Picker(style: menu, shape: capsule)` 的按钮同样带这个箭头（iOS 的菜单按钮本身不画箭头）。
- **胶囊分段按钮**：`SplitButton(shape: capsule)` 的开关部分不用系统 `Label`，符号在 16 点框内、与文字相隔 6，离胶囊端 14、离分隔线 10；下拉部分宽 30。macOS 与 iOS 同一做法。
- **沉浸窗口铺满屏幕**：`immersive` 为真时，iOS 窗口内容伸到安全区下（`ignoresSafeArea`），横屏手机上 844 × 390 的画面按 1 倍铺满；为假时回到安全区内。
- **标签栏附件**：`TabAccessory` 在 iOS 26.1 起生成 `tabViewBottomAccessory(isEnabled:)`，内容只构建一次（`AnyView`），`shown` 为假时系统收起附件；更早的 iOS 在每页底部的安全区内嵌一条材质胶囊。附件里的图标按钮是条上的符号（环境 `bernOnBar`）：不再各带玻璃，`IconButton(height:)` 只给出点按区域的大小。iPhone 17e 模拟器（iOS 26.5）上实测附件随播放出现在标签栏上方、关闭播放器后收起。
- **胶囊按钮的内部排列**：`Button(shape: capsule)` 带图标时不再用系统 `Label`（两端平台间距不同），改为符号在 16 点框内（30 点高的标签胶囊为 14 点）、与文字相隔 6；左右内边距按尺寸档取：42 点及以上 22，30 点及以下 13，其余 14。macOS 与 iOS 同一做法。

中文字形：同为 30 点系统字体，Chrome 渲染的苹方汉字前进宽度为 1 em，iOS 原生渲染约 0.947 em，所以中文标题看起来小约 5%。布局按点对齐，字形交给系统渲染，没有放大字号补偿。

## 跟随状态的快捷键与改键（2026-10-24）

与 macOS 共用生成：`shortcut:` 与 `Shortcut(key:)` 绑定文本时生成 `BernShortcuts.keyboard(…)`。`ShortcutInput` 在 iOS 上等待硬件键盘的按键（`focusable` 加 `onKeyPress`），没有硬件键盘时只显示快捷键。`MenuBar` 在 iOS 上不生成（iPad 的菜单栏未接入，能力 `menu_bar` 不适用）。验证：`tests/menu_bar.rs` 按 iOS 模拟器 SDK 类型检查；未在模拟器或设备上运行。
