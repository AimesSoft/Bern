# iOS SwiftUI 实现记录

- 日期：2026-10-12
- 依据：[Bern 架构设计 v0.23](../Design.md)
- 工具链：Rust 1.99.0（`aarch64-apple-ios-sim` 与 `aarch64-apple-ios`）；Xcode 26.5，iOS 26.5 SDK，Swift 6.3.2
- 验证环境：iPhone 17 Pro 模拟器，iOS 26.5（M4 Mac）

本文记录 `swiftui + ios` 目标的实际完成情况。展示应用（`examples/showcase`）的声明、协议、业务核心和 IR 不变，只增加了 `[targets.ios]`；生成器与 macOS 共用，按平台区分窗口、场景与少数控件样式。这一版构建模拟器应用；真机目标能编译链接，但还没有签名、安装与打包。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target ios` 生成 Swift 与胶水 crate，构建模拟器应用包、Mac 上的 CLI 与 MCP 服务器；生成测试按 iOS 模拟器 SDK 对生成的 Swift 做类型检查 |
| 窗口、导航、菜单与生命周期按平台区分 | 完成 | 运行时分为共用的 `BernRuntime.swift`、`BernMedia.swift` 与平台文件 `BernAppKit.swift`（macOS）、`BernUIKit.swift`（iOS），构建只编译自己平台的那一个 |
| 不支持原生多窗口时用虚拟窗口 | 完成 | 一个场景：主窗口铺满，其他窗口是叠在上面的 sheet；窗口事实报告为 `Virtual` |
| 前后台生命周期 | 完成 | 进入后台时窗口报告为隐藏、失去焦点，回到前台恢复；媒体按 AVFoundation 的实际状态报告 |
| 屏幕旋转、键盘、安全区域 | 部分 | 键盘输入与安全区域实测；旋转由 SwiftUI 处理，未实测 |
| 状态可订阅；带参数命令与错误返回 | 完成 | 与 macOS 相同的运行时：引擎状态、会话草稿、查询、关闭协议 |
| Video 原生呈现 | 完成 | AVFoundation 执行端与 `AVPlayerLayer` 呈现、命令驱动的控制条 |
| 真机构建与打包 | 未完成 | 引擎与界面能按 `aarch64-apple-ios` 编译链接；签名、描述文件、安装与 `bern package` 未做 |

## 工程结构

| 位置 | 内容 |
| --- | --- |
| `crates/bern-backend-swiftui/src/ios.rs` | iOS 目标：目标描述、检查、生成与构建步骤 |
| `crates/bern-backend-swiftui/src/swift.rs` | 两个平台共用的生成器，`Platform` 决定差异 |
| `templates/BernUIKit.swift` | iOS 平台部分：虚拟窗口宿主、sheet、窗口事实、开发快照、视频视图 |
| `templates/BernAppKit.swift` | macOS 平台部分，从原来的 `BernRuntime.swift` 分出，行为不变 |

生成的应用包是 iOS 的扁平结构：可执行文件、`Info.plist`、各语言的 `.lproj` 与随包资源都在包的顶层。图标由 `actool` 把 1024 像素的 PNG 编译进 `Assets.car`（未声明图标时是 Bern 标志的方形版本，声明的较小图标先用 `sips` 放大）。`Info.plist` 声明启动屏（否则应用以兼容模式显示，不铺满屏幕）、一个场景、iPhone 与 iPad、最低 iOS 17.0。

最低 iOS 17：与 macOS 14 同一代，运行时用到的 Observation、`ContentUnavailableView` 等从这一版起提供。

## 与 macOS 的差异

生成器只在以下几处按平台区分：

- **场景**：macOS 每个窗口种类一个 `Window` 或 `Settings` 场景；iOS 只有一个 `WindowGroup`，内容是 `BernVirtualHost`。
- **窗口尺寸**：声明的宽高与最小尺寸只用于桌面窗口，iOS 上由屏幕与 sheet 决定。
- **侧边栏**：iOS 上分栏视图在窄屏时折叠为导航栈，侧边栏是第一屏，标题为窗口标题（`.navigationTitle(bernWindow.title)`）。侧边栏控件登记在整个分栏视图上，所以只显示详情时外部入口也能切换页面（macOS 上效果相同）。
- **文本输入**：iOS 的表单只显示输入框的提示文字，不显示标签。单行与密码输入改为 `LabeledContent`（标签在左、输入在右，与“设置”应用相同），多行输入的标签放在上方。
- **控件样式**：`Switch` 的 `checkbox` 样式在 iOS 上是开关（iOS 没有复选框）；`Picker` 的 `radio` 样式在 iOS 上是带勾选标记的内嵌列表。

## 窗口

iOS 应用只有一个场景，按设计文档用虚拟窗口，形式遵循 iOS 的习惯：

- **主窗口**铺满场景。被命令关闭后，场景显示应用名与“打开“…””按钮，点按后重新打开（新的实例代次）。
- **其他窗口**是 sheet，按打开顺序一层叠一层：导航栏显示窗口标题，右上角“完成”按钮经关闭协议请求关闭。设置窗口的页签是 sheet 底部的标签栏。
- **下滑关闭**：没有待确认的修改时可以下滑关闭，作为系统事实报告（`changed_by: System`）；关闭检查需要确认时禁止下滑，只能用“完成”按钮，此时弹出存储、不存储、取消。
- **窗口动作**：只有最上面的窗口可以激活；它和主窗口可以关闭。被 sheet 盖住的窗口不能被提到前面（iOS 只能先关掉上面的），`window.activate` 与 `window.close` 返回 `target_unavailable`；`minimize`、`restore`、`maximize`、`move`、`resize` 同样返回 `target_unavailable`，并列出可用动作。
- **窗口事实**：可见性跟随场景（进入后台为隐藏），帧是窗口根视图在场景中的位置（点），只有活动场景中最上面的窗口有焦点。

## 本地控制通道

模拟器里的应用是 Mac 上的进程，可以和 Mac 共用文件系统，所以 CLI、JSON 客户端与 MCP 服务器（为 Mac 构建）能经 IPC 通道连到它。应用自己的临时目录在很深的容器路径里，套接字路径超过 Unix 套接字的长度上限；`bern run` 因此把 Mac 的临时目录经 `BERN_IPC_DIR` 交给应用（`simctl` 的 `SIMCTL_CHILD_` 变量），通道在其中建立套接字。iOS 上通道只在设置了这个变量时监听：从主屏幕点按启动或装在真机上时，没有别的进程能连到应用，通道不启动。

`BERN_IPC_DIR` 在其他平台上同样有效：它代替临时目录作为各应用套接字目录的上级目录。

模拟器里的应用与 macOS 版包标识相同时，两者的引擎在同一个目录里，CLI 会列出两个引擎并要求用 `--engine` 指定。

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

`bern run --target ios` 把应用装进正在运行的 iOS 模拟器并启动，应用的输出显示在终端，直到应用退出；`BERN_*` 环境变量（例如 `BERN_APPEARANCE=dark`）一并交给应用。没有正在运行的模拟器时报错并提示启动方法。不带 `--target` 的 `bern run` 优先选择与本机系统相同的目标，Mac 上仍然启动 macOS 版。`bern doctor` 检查 iOS 模拟器 SDK、Rust 的模拟器目标与可用的模拟器。

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

展示应用随之调整：图片页的两行图片从 `Row` 改为 `Grid`，原来四张 120 宽的图片在 iPhone 上放不下、被两侧裁掉，现在窄屏时自动换行，宽窗口中仍是一行。

## 未完成与已知限制

- 只构建模拟器应用。真机需要开发者签名与描述文件，安装与 `bern package` 未实现。
- HDR：模拟器显示不了 HDR，增益图在模拟器里只解码 SDR 版本；真机上图片按 HDR 解码，视频由 `AVPlayerLayer` 显示，HDR 均未验证。
- 真机上没有声明后台音频，进入后台后系统会暂停播放；只在模拟器里观察过（模拟器里继续播放）。
- iPad 未实测。iPad 的多窗口（多场景）没有使用，同样是一个场景中的虚拟窗口。
- 屏幕旋转未实测。
- 被 sheet 盖住的窗口不能单独激活或关闭，需要先关掉上面的窗口。
- 窗口内容自带分栏或导航栈时，它作为 sheet 打开时没有“完成”按钮（嵌套的导航栈会出问题），只能下滑或用命令关闭。
