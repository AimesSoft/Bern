# Linux GTK 实现记录

当前组件接口见[组件参考](../Reference/UI.md)，逐项支持与近期验证范围见[平台能力表](../Reference/Platforms.md)。下文保留各阶段实现和验收记录。

- 日期：2026-10-11（v0.29、v0.30 的能力于 2026-10-14 补齐，见“样式、动效、自绘与媒体类应用的能力”；页面级布局、场景与控件形态的能力于 2026-10-18 补齐，见同名一节）
- 依据：[Bern 架构设计 v0.30](../Design.md)（最初按 v0.15 实现）、[Windows 与 Linux 适配](../Adaptation/WindowsLinux.md)
- 工具链：Rust 1.99.0；gtk4-rs 0.11（`gtk4`，启用 `v4_14`）、libadwaita-rs 0.9（启用 `v1_5`）、glib/gio 0.22
- 验证环境：Ubuntu 24.04 LTS（aarch64，VMware Fusion 虚拟机），GNOME Shell 46.0，Wayland 会话；GTK 4.14.5、libadwaita 1.5.0、GStreamer 1.24.2

本文记录阶段五中 `gtk + linux` 目标的实际完成情况。同一份展示应用（`examples/showcase`）的声明、协议、业务核心和 IR 不变，只增加了 `[targets.linux]`，由新的 GTK 目标模块生成 Linux 应用；解析器、公共 IR、核心检查规则与业务核心没有为 GTK 做任何修改。阶段二、阶段四的通用验收在 Linux 上重新执行，结果见“验证记录”。WinUI 3 与 `flutter + macos` 属于阶段五的后续批次，本轮没有开始。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target linux` 生成应用 crate 并构建出界面程序、CLI 与 MCP 服务器；生成测试在装有 GTK 4.14 与 libadwaita 1.5 的环境中对生成的应用做类型检查 |
| 新目标不改解析器、公共 IR 与业务核心 | 完成 | 新增 `bern-backend-gtk`（生成器）与 `bern-gtk`（运行时）；其余改动只有构建工具注册目标、`bern-ffi` 公开查询与媒体模块、示例增加目标配置 |
| 目标禁用后其他构建独立可用；具体框架 SDK 不是全局前置条件 | 完成 | `bern-gtk` 不是工作区成员，只在生成的 Linux 应用里编译；macOS 上 `cargo test --workspace` 与 clippy 不需要 GTK |
| 省略 UI 技术时的默认解析，默认目标不可用时明确失败 | 完成 | `platform = "linux"` 解析为 `gtk-linux`；显式 `ui = "flutter"` 报 “the `flutter + linux` backend is not available”（测试） |
| 状态可订阅；带参数命令与错误返回 | 完成 | 引擎状态按字段镜像为响应式单元；`set_volume`、`record`、`set_display_name` 等经真实控件闭包与外部入口执行，错误以 Toast 显示并返回给调用方 |
| 列表身份正确、订阅可释放 | 完成 | `ForEach` 按键保留行；查询订阅随视图与窗口结束；见“验证记录” |
| 同一主题快照正确映射深浅色 | 完成 | 原生优先：颜色全部来自 libadwaita 的样式管理器，生成代码与运行时不写颜色；深色截图见验证记录 |
| 原生多窗口、关闭确认、窗口事实 | 完成 | 每个窗口是 `AdwApplicationWindow`；关闭协议、最大化、恢复、调整尺寸由命令驱动并以平台事实答复；Wayland 的限制见“窗口” |
| Video 原生呈现 | 完成 | `GtkMediaFile`（GStreamer）执行端与 `GtkPicture` 呈现、命令驱动的控制条；播放、暂停、跳转、静音、音量、播完与重播、换源、损坏文件、应用级播放器跨页面与跨窗口均已实测（见“验证记录”）；HLS 直播（时长未知、不可跳转）已实测 |
| 阶段四：`Adaptive`、`Menu`、`LanguagePicker`、运行时切换语言 | 完成 | 窗口缩窄后布局切换为紧凑并挂载菜单，菜单未打开时也能执行菜单项；语言切换后界面全部更新，草稿保留 |
| 样式覆盖（`style!`）：`Button`、`IconButton`、`SidebarItem` | 完成 | 颜色、边框、圆角、内边距、尺寸档与阴影生成应用优先级的样式表，状态取 GTK 的状态标志；缩放、位移与透明度用 `AdwSpringAnimation`，回到普通状态用 `settle`。见“样式覆盖” |
| 动效：`appear`、`disappear`、`reflow`、`hero`、`animate`、`stagger`、`nudge`、`scroll_scale`、`scroll_opacity` | 完成 | 变换在分配子控件时施加，不影响布局；`reflow` 只在 `Column`、`Row` 里（含其中的 `ForEach`），其他容器构建时警告。见“动效” |
| 自绘：`Canvas`、`Shape`、`Series`、`Pattern`，`Text(mark:)`，`fill_from` 与 `fade` | 完成 | cairo 绘制，颜色取 libadwaita 的命名颜色；标注按 Pango 的行位置画在文字下 |
| 窗口：`chrome`、`immersive`、`idle_after` 与 `window.idle`，`role: dialog`，`window.compact` 等尺寸档 | 完成 | 见“窗口外框、静置与对话框” |
| 可增减的侧栏项、页签与工具栏项 | 完成 | 侧栏项与页签在挂载期间登记；选中项离开时选中移到 `fallback` 或剩下的第一项 |
| v0.29 的 `features` 与媒体类组件 | 完成 | `SidebarFooter`、`Inspector`（`AdwOverlaySplitView`）、`Toast`（`AdwToast`）、`SplitButton`、`PopoverButton`、`Drop`（`GtkDropTarget`）、`Pinch`（`GtkGestureZoom`）、`Swipe`（`GtkGestureDrag`）、`Rail`；工具栏搜索与结果、会话动作、开关按钮、绑定长度、`appearance`、快捷键、运行时强调色与明暗、运行时图片、取色填充、`pick`（`GtkFileDialog`）、滚动停靠与位置；会话命令 `ui.focus`、`ui.set_file` |

## 工程结构

| crate | 职责 |
| --- | --- |
| `bern-backend-gtk` | `gtk + linux` 目标：描述、检查、从 IR 生成 Rust 源码、生成应用 crate 与构建计划。工作区成员，不依赖 GTK |
| `bern-gtk` | GTK 4 运行时：响应式单元、控件、窗口、会话命令、查询与媒体执行端。不是工作区成员（`exclude`），只在生成的应用中编译 |
| 生成的应用 crate | `target/bern/linux/app/`：`ui.rs`（组件、窗口、文案表与 `AppDef`）、`main.rs`（启动引擎）、`cli.rs`、`mcp.rs` |

生成的代码不另写一套 UI 运行时，而是调用 `bern-gtk` 的标准组件，形式与 Web 目标相同：每个组件生成一个函数，表达式生成在上下文 `c` 中求值的 Rust 表达式，局部变量（组件参数、行变量、模式绑定）放在上下文里，闭包不捕获任何东西，绑定只在读过的单元变化时重新求值。值以协议的 `Value` 表示，编码与解码直接使用 `bern-protocol`。

引擎、会话运行时与界面在同一进程，经过与 SwiftUI、Web 目标相同的进程内接口（`bern-ffi`）连接：GTK 运行时直接调用这些函数，值按 Bern 二进制布局传递，回调从引擎和通道线程到达后复制字节、在 GTK 主线程上按到达顺序处理。回调边界捕获 panic，一个回调出错只写日志，不会终止应用。

## 控件映射

依据 GNOME HIG 与 libadwaita 1.5 的标准控件；表单里的控件都是 libadwaita 的行，放在 `boxed-list` 样式的列表中。

| Bern | GTK / libadwaita |
| --- | --- |
| `Window` | `AdwApplicationWindow`，内容为 `AdwToolbarView` 与 `AdwHeaderBar`；`ToastOverlay` 显示失败的命令 |
| `SplitView` / `Sidebar` / `Detail` | `AdwNavigationSplitView`；侧边栏为 `navigation-sidebar` 样式的列表、分节标题与搜索框；详情页标题栏显示所选目的地、工具与返回；窗口窄于 400sp 时折叠为单栏 |
| `Navigation` / `Destination` / `NavLink` | 根内容与顶层目的地放在 `GtkStack` 中左右滑动；根内容在推入时保持挂载；返回按钮、Esc、Alt+← 返回；链接在表单中是可激活的行，末尾为 `go-next` 图标 |
| `Tabs` / `Tab` | `AdwViewStack` 与标题栏中的 `AdwViewSwitcher`，只构建选中的页签 |
| `Toolbar` | 所在页面标题栏的末端；推入目的地时隐藏根页面的工具 |
| `Form` / `Section` | 滚动、`AdwClamp`（680）的页面；分节为标题、`boxed-list` 列表与脚注 |
| `Text` / `Label` | `GtkLabel` 用 libadwaita 字体类（`title-1`…`title-4`、`heading`、`caption`、`dim-label`）；表单中的 `Label` 为带前缀图标的 `AdwActionRow` |
| `Button` / `IconButton` | `GtkButton`；`prominent` 为 `suggested-action`，`plain` 为 `flat`，`destructive` 为 `destructive-action`；图标按钮无边框，名称为提示与可访问名称 |
| `TextInput` | 表单中为 `AdwEntryRow`、`AdwPasswordEntryRow`，多行为带边框的 `GtkTextView`；其他位置为 `GtkEntry` |
| `Search` | `GtkSearchEntry`；Esc 清空 |
| `Switch` | 表单中为 `AdwSwitchRow`，`checkbox` 样式为带 `GtkCheckButton` 的行 |
| `Slider` | 行内为标题与带两端图标的 `GtkScale`；拖动只改本窗口预览，松开提交一次，键盘与滚轮立即提交 |
| `Picker` | `segmented` 为 `linked` 的切换按钮组；`chips` 为同组（`set_group`）、不相连的 `GtkToggleButton`，在 `Flow` 中且不显示标签时每个按钮都是 Flow 的子项，否则放进自己的 `BernFlow`（间距 8）；其他为 `AdwComboRow` 或 `GtkDropDown` |
| `Menu` / `MenuItem` | `GtkMenuButton` 与 `GMenu`；菜单项是动作，可用性来自声明 |
| `LanguagePicker` | `AdwComboRow`，选项为语言的自称 |
| `Query` | 加载：转圈与文案；空结果与无数据失败：紧凑的 `AdwStatusPage`（失败带“重试”）；刷新与有数据的失败：数据上方的状态行 |
| `Adaptive` / `Layout` | 一个零高度的绘图区测量可用宽度，按 `[layouts]` 断点选择布局 |
| `Video` | `GtkPicture` 显示播放器的 `GtkMediaFile`，下方为标准控制条（见“媒体”） |
| `Image` | `GtkPicture`（`content-fit` 对应 `fit`，可缩小）放在固定尺寸的 `BernPictureFrame` 中：`GtkPicture` 本身要求原图大小，这个小部件把它限制为声明的宽高，得到更多空间时仍靠前缘；只写一边时另一边按图片比例。JPEG 经 gdk-pixbuf 解码并按 EXIF 方向转正（GTK 4.14 自己的加载器忽略方向），其他格式用 GTK 的加载器。HDR 图片（PQ、HLG 的 PNG，带增益图的 JPEG）由 `bern-image` 解码：GTK 4.16 起在 Wayland 上且打开了色彩管理（4.16 至 4.18.0 默认打开，4.18.1 起要用户设置 `GDK_DEBUG=color-mgmt`）时，以全部增益、线性 BT.2020 交给 GTK（`GdkMemoryTextureBuilder` 与 rec2100-linear，运行时查找，同一个程序也能在 4.14 上运行）；其他情况下先转成 SDR：增益图为作者给的 SDR 图，PQ 与 HLG 做色调映射。实测 GTK 4.14：PQ 图不再发灰，增益图显示 SDR 图。`label` 为替代文本，没有时对辅助技术隐藏 |
| `Progress` | 有 `value` 时为标签下的 `GtkProgressBar`，由标签命名；没有时为 `GtkSpinner` 加 `dim-label` 文字（libadwaita 1.5 还没有 `AdwSpinner`） |
| `Grid` | 同宽单元格的 `GtkFlowBox`：每格至少 `min_width` 宽，不可选择，单元格本身不获得焦点；绑定的 `min_width` 随 `animate` 的弹簧变化，列数随之改变 |
| `Stack` | `BernLayers`：子元素叠在同一处，后写的在上面、先接收输入，按 `align` 对齐；整体与最大的一层一样大，得到更多空间时保持在前缘，除非某层要填满（没有尺寸的画布、轨道、视频等，GTK 的 expand 向上传递） |
| `Canvas` | `GtkDrawingArea`，cairo 绘制；没写 `width`、`height` 的方向填满可用空间；有 `label` 时是带名称的图像，没有时对辅助技术隐藏 |
| `Text(mark:)` | `BernMarkedText`：标签之下按 Pango 每一行的逻辑范围画胶囊形底纹（行高 74% 至 98%，右端多出 0.4 个字宽），文字变化或主题变化时重画 |
| `Rail` | `BernRail`：沿轴取得到的长度，交叉方向为最厚的子元素；`at` 的子元素按 `anchor` 对准比例点，`from`/`to` 的子元素铺在两点之间、没有交叉尺寸时铺满厚度；比例变化随 `animate` 的弹簧移动。轨道只沿轴向外要空间：没有尺寸的子元素不让外面的容器在交叉方向变大。在 `Stack` 中轨道两个方向都铺满其他层给出的尺寸，自己只按有尺寸的子元素计入 |
| `SidebarFooter` | 侧栏列表下方、分隔线之下的一栏，列表在它上方滚动 |
| `Inspector` | `AdwOverlaySplitView` 的尾侧边栏：`presented` 双向绑定，宽窗口推开内容，窄于 640sp 时浮在内容上 |
| `Toast` | 窗口的 `AdwToastOverlay` 中的 `AdwToast`，`key` 或内容变化时替换，`duration` 秒后消失；有 `icon` 时为图标加文字的标题 |
| `SplitButton` | `linked` 的开关按钮加箭头菜单按钮，浮层中放子元素（始终挂载） |
| `PopoverButton` | `GtkMenuButton` 与 `GtkPopover`：子元素在浮层显示时挂载、关闭时卸载；`presented` 双向绑定 |
| `Drop` | `GtkDropTarget`（文件列表，预读值以便按种类拒绝）：可接受的文件经过时 `over` 为真，松手后存入字段并执行动作；`opens: true` 时应用接收系统交来的文件（`GApplication` 的 open），桌面文件登记 `MimeType` 与 `%U` |
| `Pinch` / `Swipe` | `GtkGestureZoom`（另接受 Ctrl 加滚轮）/ `GtkGestureDrag`（10 点后开始，`distance` 默认 300） |
| `Switch(style: button)`、工具栏中的 `Switch` | `GtkToggleButton`，工具栏中有图标时只显示图标 |
| 带内容的 `Button` | 无外框的 `GtkButton`，内容按原样显示，`overflow: hidden` 让样式的圆角裁住内容；没有样式时按下变暗 |
| 工具栏中的 `Search` | 页眉末端的 `GtkSearchEntry`；子元素是不抢焦点的 `GtkPopover`，有焦点且有文字时显示，点选结果后收起 |

图标使用 Bern 的 SF Symbols 命名，映射到 freedesktop 符号图标名（例如 `magnifyingglass` → `system-search-symbolic`），由用户的图标主题绘制；GNOME 上即 Adwaita 图标。对照表只有一份（`crates/bern-gtk/src/icon_table.rs`，按名字排序）：运行时读它，`bern-backend-gtk` 也以 `include!` 读它，应用用到表中没有的名字时检查报错并指向声明，不再在运行时显示缺失图标。表中的图标都在 Adwaita 46 里（Yaru 等主题经 `Inherits` 最终回落到 Adwaita），只有 `arrow.up.right.square` 用 libadwaita 自带资源中的 `adw-external-link-symbolic`。表至少包含 Bern 图标集（`bern_target_api::icon_set::NAMES`）的全部名字。

## 运行结构

- **窗口与草稿**：窗口出现时向会话运行时登记，获得代次与外部 ID（`main#1`），草稿单元由运行时返回的初值创建；写入先经运行时，再更新单元。窗口关闭即释放草稿、控件登记与查询订阅，应用级播放器继续。
- **控件登记与会话命令**：可操作控件挂载时登记目标 ID（`组件.控件id`，列表行附加 `[键]`）、角色、名称、可用性与动作闭包，外部的 `ui.*` 命令经这些闭包执行；禁用、被关闭确认阻挡、多处挂载分别返回 `target_unavailable`、`target_blocked`、`target_ambiguous`；密码框不暴露值。
- **应用生命周期**：`AdwApplication` 以 `bundle_id` 为应用 ID，同一用户只运行一个实例；最后一个窗口关闭后应用保持运行（引擎与外部入口仍可用），再次启动应用会打开主窗口。Ctrl+Q 退出（与 macOS 的 Cmd-Q 一样不经过关闭检查），Ctrl+W 关闭当前窗口，设置窗口按 Esc 关闭。
- **外部入口**：与 macOS 相同的本地控制通道（`channels = ["ipc"]`），CLI 与 MCP 服务器从同一命令目录生成。

## 窗口

- **关闭协议**：标题栏关闭按钮、Ctrl+W、声明的动作与 `window.close` 都先求值关闭检查。需要确认时以 `AdwAlertDialog` 显示，按钮为“存储 / 不存储 / 取消”或“放弃更改 / 取消”；待决时该窗口内容不可操作，外部入口返回 `target_blocked`。决定关闭后直接销毁窗口：对话框仍在关闭动画中时，再发关闭请求只会关掉对话框。
- **事实**：可见性、尺寸状态（普通、最大化、全屏）、窗口尺寸、是否聚焦与可用动作，以带来源的事实报告给会话运行时；值不变时版本号不变。
- **GTK 4 与 Wayland 的限制**：
  - GTK 4 没有读取或设置窗口位置的接口，`frame` 的 `x`、`y` 恒为 0，`window.move` 不可用（`target_unavailable`）。
  - GNOME 的 Wayland 合成器不报告窗口已最小化（最小化后只失去焦点，没有状态标志）。`window.minimize` 照常发出请求，然后立即以 `ui_timeout` 答复并说明结果未知，不把请求当作已完成；X11 下报告最小化状态，命令等到事实到达再答复。`window.activate` 可以把窗口重新显示出来。
  - GNOME 有防抢焦点机制：后台进程发起的激活不一定获得焦点，事实如实反映。
  - `window.maximize`、`window.restore`（退出最大化或全屏）与 `window.resize` 等到窗口实际变化后才答复，结果是运行时记录的实际状态；尺寸小于声明的最小尺寸返回 `out_of_range`。

## 媒体

- 执行端为每个 `player_id` 维护一个 `GtkMediaFile`（GTK 的媒体后端，Linux 上为 GStreamer），每个新源换一个新的媒体流（同一个流换文件后会保留第一个文件的时长），音量与静音沿用，正在显示它的画面随之切换；把准备、播放、结束、错误、时长、音量、静音、跳转与每 0.5 秒的位置作为观察结果报告给媒体中心，从不派发命令。构建清单记录媒体后端 `gtk-mediafile` 与呈现 `gtkpicture`。
- `Video(source:)` 的播放器属于组件实例，绑定变化时按新源版本加载，同值不重复加载；`Video(player:)` 只附着呈现。控制条（播放/暂停、进度、时长、静音、音量）只发送标准媒体命令，状态来自媒体中心；拖动进度与音量只更新预览，松开时提交一次。控件登记为 `目标.play_pause`、`.position`、`.mute`、`.volume`。
- 位置每 0.5 秒回报一次，位置跳变（例如跳转完成）时立即回报；播完时位置为时长。画面区按可用宽度保持 16:9。
- 错误只显示 GStreamer 消息的第一句（不含元素路径），完整内容写入日志。
- **系统依赖**：GStreamer 的 H.264 与 AAC 解码器不在 Ubuntu 的默认安装中，需要 `gstreamer1.0-libav`；缺少时片段报告 `Failed`（“您的 GStreamer 安装缺少插件。”），`media.play` 返回 `media_failed`。MPEG-TS 的 HLS 直播还需要 `gstreamer1.0-plugins-bad`（`tsdemux`）。媒体的网络请求遵循 `http_proxy` 等环境变量。

## 本地化与主题

- 文案表（应用声明的文字、应用名与运行时自己的文字）生成到程序中；启动时按 `LANGUAGE`、`LC_ALL`、`LC_MESSAGES`、`LANG` 选择支持的语言（`zh_CN` 对应 `zh-Hans`），`ui.set_language` 与 `LanguagePicker` 在运行中切换，所有文本绑定在同一次更新中重新计算。运行时自己的文字内置 zh-Hans、zh-Hant、en、ja。
- 带参数的文本与显示变体字段的枚举标签按[消息格式](../Reference/UI.md#文本内容与格式化)写出：运行时直接复用 `bern_protocol::format`（与编译器同一份解析与格式化），参数按类型传入（文本、带数值的数字、`CalendarDate`、枚举标签），复数类别取当前语言。月名与星期名随文案表生成，内置语言之外用英文。生成的类型表带上标签的键与参数，标签的文字仍来自文案表。2026-10-19 在 bern-ubuntu（Broadway）上以临时工程实测中英文的 `=0` 分支、复数分支内嵌枚举标签、日期、时长与带字段的标签，并确认一个实际应用的英文界面只剩标题等数据为原文。
- 构建生成 freedesktop 桌面文件（各语言的名称），可安装到 `~/.local/share/applications`。
- 颜色全部来自 libadwaita：深浅色跟随系统；没有样式覆盖时生成代码不设置颜色。声明了品牌色时映射为 libadwaita 的强调色。样式覆盖与画布的语义角色映射到 libadwaita 的命名颜色（见“样式覆盖”）。

## 样式、动效、自绘与媒体类应用的能力（v0.29、v0.30）

2026-10-14 补齐：GTK 的能力描述现在列出样式（`Button`、`IconButton`、`SidebarItem`）、动效（`appear`、`disappear`、`reflow`、`hero`、`animate`、`scroll_scale`、`scroll_opacity`、`nudge`、`stagger`）、`window_chrome`、`text_marks`、`dynamic_items`、全部绘图组件与媒体类组件，以及 `features` 中的全部 18 项。`examples/planner` 与 `examples/gallery` 在 GTK 上检查没有错误也没有警告（测试）。

### 样式覆盖

- 每个用到的样式生成一个 `StyleDef` 表项；运行时把它们写成一张应用优先级的样式表：类 `bern-style-<名称>` 是基础外观，`bern-selected`、`bern-hover`、`bern-pressed`、`bern-focus`、`bern-disabled` 依次是各状态，同优先级下后写的胜出，正是“选中、悬停、按下、焦点、禁用”的合并顺序。样式没写的属性留给主题，所以只写变换的 `Button { pressed: scale(…) }` 保留 libadwaita 按钮的外观与反馈；写了底色的状态盖住主题的悬停底色。`none` 去掉该状态的反馈，`system` 保留主题的。颜色变化用 150 毫秒的 CSS 过渡。
- 状态来自 GTK 自己的状态标志（`PRELIGHT`、`ACTIVE`、`FOCUS_VISIBLE`、`INSENSITIVE`、`SELECTED`/`CHECKED`），不另装事件控制器，键盘按下与指针按下都是 `ACTIVE`。
- 缩放、位移与透明度作用在控件外层的 `BernMotion` 上（见“动效”），用状态所带的弹簧进入、`settle` 回到普通状态，可在动画中途换目标。
- 侧栏项有样式时，行本身不再画底色，样式画在行内容上，内容在 `BernMotion` 里随状态缩放；没有写的状态沿用 libadwaita 侧栏的样子（悬停 7%、选中 10%、按下 16% 的前景色）。选中由行的 `SELECTED` 标志驱动；行仍是 `GtkListBox` 的行，键盘导航不变（未实测）。
- 尺寸档：`small` 最小高 24、`large` 44、`extra_large` 52，并调整内边距与字号；`corner` 为 `border-radius`，带内容的按钮另有 `overflow: hidden` 裁住内容，`shadow` 为沿圆角的 `box-shadow`。
- 语义角色映射到 libadwaita 的命名颜色：`surface.canvas` → `window_bg_color`，`surface.panel` → `card_bg_color`，`surface.raised` → `popover_bg_color`，`text.primary` → `window_fg_color`，`text.secondary` → 它的 55%，`accent` → `accent_bg_color`，`on_accent` → `accent_fg_color`，`status.*` → `success`/`warning`/`error_bg_color`，`fill.subtle` → 前景色 7%，`fill.strong`、`control.track`、`control.border` → 15%。固定的 `color(light:, dark:)` 按当前明暗取值，明暗变化时样式表重写。

### 动效

- 弹簧是 libadwaita 的 `AdwSpringAnimation`：质量 1，刚度 (2π / T)²，阻尼比 ζ；换目标时从当前值与速度继续。动画由控件的帧时钟驱动，控件未上屏时直接到终点，所以出现过渡等元素上屏后才开始。系统关闭动画（`gtk-enable-animations`）或 `BERN_MOTION=reduce` 时，状态变换、错峰、晃动与滚动联动不执行，出现与消失只做 0.15 秒的淡入淡出；系统关闭动画时 libadwaita 会把这次淡入淡出也直接跳到终点。
- `BernMotion` 只有一个子控件，测量与子控件相同，分配时给子控件一个以其中心为原点的变换，所以动效不改变布局；变换由几层（样式状态、出现、消失、共享元素、晃动、滚动联动、显示）各自的弹簧合成：位移相加、缩放与透明度相乘。
- 出现与消失：`if`、`match`、`ForEach` 在首次显示之后挂载的元素播放 `appear`；元素内部的元素随它一起出现，不再各自播放。卸载的带 `disappear` 的元素留在容器里播完再离开，此时它已不在自己的区域中，新元素按它不存在时的位置插入。列表行由列表布置，行本身不动，行的内容在行内移动。
- `stagger`：容器构建直接子元素时带一个计数，带 `appear` 的子元素依次晚 `stagger` 秒；容器本身在一次更新中出现时，子元素也播放出现；`ForEach` 的各行即其子元素，每次更新从 0 计起。
- 共享元素 `hero`：元素卸载时记下它在窗口中的位置并留一张画面（视频与画布不留，它们会自己变化）；同一窗口中同键的新元素布置后，从旧位置与尺寸以 `hero` 弹簧展开，旧画面在窗口上方的 `GtkFixed` 层里随之移动并淡出。
- `reflow`：写了它的 `Column`、`Row`，或其中有带 `reflow` 的 `ForEach`，用 `BernReflowBox`：GTK 的盒子布局排好后，比较每个子元素的新旧位置，被移动的子元素从旧位置以弹簧滑到新位置（动画中再次移动时接着当前偏移）。`Stack` 与 `cell_ratio` 网格见下文“页面级布局”；`GtkFlowBox` 网格、表单行等其他容器中的 `reflow` 构建时警告，子元素直接到新位置。
- `animate`：绑定状态的长度（`padding`、`gap`、`width`、`height`、`Grid` 的 `min_width`、`Canvas` 与 `Image` 的尺寸、`Video` 的 `corner`、`Rail` 的比例）随这个弹簧变化。`Video` 的圆角由 `BernClip` 在绘制时裁剪，变化只重绘。
- `nudge`：条件变真时放大到 1.06 再以 `jelly` 弹回；`ScrollView(arrival: nudge)` 让设置 `position` 滚到的那一项停稳后晃一下。
- `scroll_scale`、`scroll_opacity`：滚动区的调整量每次变化与元素每次分配时，按元素中心离视口中心的距离设置缩放与透明度。

### 自绘与图片

- `Canvas` 是 `GtkDrawingArea`，绘图元素在一个绑定中读取，状态、尺寸或颜色（系统明暗、对比度、应用的强调色与明暗）变化时重绘；颜色在绘制时按控件所在部分的外观解析。图案按条带逐条绘制，`twill` 叠 45° 细线，不缓存位图。
- `fill_from` 的图片在后台读取（`GFile`，本地文件或 http(s)），按 EXIF 方向转正后缩到 32×32 取平均色，按 URL 缓存；读到之前用 `fill`。`fade` 是向一侧渐隐为透明的线性渐变。
- `Image(source: app.x)` 在后台读取后显示，`None` 时不显示；只写一边时另一边按图片比例。

### 窗口外框、静置与对话框

- `idle_after`：窗口捕获阶段的指针移动、按键、滚轮与按下重新计时，到时 `window.idle` 为真。内容在静止的指针下移动时 GTK 会在原位置报告指针移动，位置没变的不算输入，否则画框铺满时会立即被自己的布局变化唤醒。
- `immersive`：为真时窗口里所有标题栏（窗口自己的、侧栏与详情的）`reveal-top-bars` 关闭；同时静置时指针隐藏。
- `chrome: unified`：标题栏用 `AdwToolbarView` 的平铺样式（`flat`），不再有分隔阴影；内容不延伸到标题栏下。
- `role: dialog`：设为主窗口的临时窗口并模态，Esc 关闭；在 GNOME 上由 mutter 附着在主窗口上（未验证，测试环境 Broadway 没有窗口管理器，对话框出现在左上角）。
- 窗口尺寸档：窗口内容顶部一个高 0 的探针测量宽度，按 `[layouts]` 的三个断点得到 `compact`、`medium`、`expanded`、`large`，表达式中的 `window.compact` 等读取它；窗口内容之上铺满的另一个探针测量高度，低于 `compact_below` 时 `window.short` 为真。`Adaptive` 四档各自选择，没有自己布局的档位显示下一个更大的已声明布局，没有更大的取更小的。`Adaptive` 按容器“提供”的宽度选择：它的容器 `BernOffered` 在横向不要求最小宽度，所以当前显示的宽布局不会把窗口或探针撑宽（否则会一直停在宽布局）；比提供的宽度更宽的布局被裁剪，直到宽度选出另一档。
- 运行时强调色与明暗：`WindowSet(accent:, appearance:)` 的表达式在每个窗口中求值；强调色以 `@define-color accent_bg_color`、`accent_color`（浅色窗口略暗、深色略亮，保证在窗口底色上可读）与 `accent_fg_color`（按亮度取黑或白）写入一份更高优先级的样式表，明暗切换时重写；明暗由 `AdwStyleManager` 的配色方案设置。libadwaita 1.5 没有设置强调色的接口，这是它支持的方式。
- `appearance: dark` / `light` 的部分：第一次用到时，从 libadwaita 读取它自己的 `base.css` 与另一种外观的命名颜色，把每条规则限定在 `.bern-appearance-dark`（或 `light`）之下、命名颜色换成那种外观的取值（强调色保持跟随窗口），作为一份样式表加载；部分里的控件、从中弹出的浮层与菜单因此按那种外观绘制，画布的语义角色也按它解析。只在部分的子孙上生效，部分本身的底色由内容决定。

### 媒体类应用的组件与输入

- 文件：`pick` 用 `GtkFileDialog`（按种类加 MIME 与扩展名过滤，`folder` 选目录），选中后以 `file://` 地址存进字段再执行动作；种类与扩展名表随生成代码一起提供。`ui.set_file` 校验地址（`file`、`http`、`https`，否则 `invalid_url`）与种类（否则 `unsupported_file`）后交给带 `pick` 的按钮、菜单项或 `Drop`。
- 快捷键：每个窗口一个捕获阶段的按键控制器，按控件挂载的先后取最新的；`cmd` 与 `ctrl` 都是 Control。没有 Control 或 Alt 的快捷键在编辑文字时不触发。`Search` 的快捷键聚焦搜索框，`Switch`、`SplitButton` 的开关，按钮、菜单项、`PopoverButton` 的激活。
- `ScrollView(snap:, position:)`：滚动停下（160 毫秒不变）时取最近的一项对齐到锚点；离上次停下的位置走过 24 点以上而最近的一项在身后时，取运动方向上的下一项，所以一格滚轮也能前进。`center` 时首尾按最大一项留出空白；位置按项在内容中的坐标加内容的边距计算，与当前滚动无关。字段被设成另一项时以 `smooth` 滚过去；首次显示等边距生效后直接停在字段的一项。
- 重复项中的控件目标带上每一层的键，经过组件也一样（`card.open_swatch[2]`）。

## 页面级布局、场景与控件形态的能力（2026-10-18）

GTK 的能力描述补上了此前只有 SwiftUI 与 Web 声明的 29 项 `features`（`text_weights`、`text_lines`、`image_corners`、`sidebar_width`、`controlled_values`、`search_palette`、`image_template`、`adaptive_images`、`scaled_scene`、`accordion_layout`、`capsule_controls`、`modal_flow_timer`、`page_badges`、`idle_control`、`fill_buttons`、`grid_cells`、`page_search`、`form_layout`、`modal_cover`、`window_short`、`edge_grow`、`delayed_appear`、`toolbar_leading`、`axis_padding`、`icon_menus`、`icon_labels`、`row_baseline`、`sidebar_header`、`untitled_detail`），动效补上 `appear_delay`、`nudge_key`、`nudge_scale`、`reduce_motion`，样式补上 `Stack` 的 `background`、`foreground`、`corner`。`window_landscape` 仍未声明：桌面 GTK 没有设备方向，检查给出警告。一个真实的媒体类应用在 GTK 上的检查从 67 个错误变为 0 个错误，构建并运行（见“验证记录”）。

### 外观与文字

- `Stack` 的样式画在层叠本身上：`bern-style-<名称>` 的底色、文字与符号图标的颜色（CSS `color` 向下继承）、圆角；有圆角时 `overflow: hidden` 按圆角裁住各层。
- `text { … }`：每个文字样式另带类 `bern-text-<样式>`，样式表按 `style!` 写入字重与字号，应用优先级高于主题的 `title-1` 等类。
- `Text(lines:)` 用 `GtkLabel` 的 `lines` 与末尾省略号；`font_size`（可绑定）与 `line_height` 是 Pango 的绝对字号与绝对行高属性。`Label(font_size:)` 同时设置图标的像素尺寸，`display: icon` 只显示图标，标题作为提示与无障碍名称。
- 文字先取一行的宽度，放不下才换行：GTK 的横向盒子按行高去量换行标签的宽度（width-for-height），旁边有更高的元素时文字会被压成多行窄条；`Text` 外包一层 `BernWords`，量宽度时不看给定的高度，加了标注的文字同理。
- `Image(corner:)` 由 `BernClip` 按（可绑定、可弹簧的）半径裁剪；`rendering: template` 用 `BernTemplatePicture`：图片的 alpha 作蒙版填前景色（GSK 的 `push_mask`）；`aspect` 取代图片自身比例，只给 `aspect` 时铺满容器宽度、高度按比例（高度随宽度变化，最小高度为 0，在滚动视图中按自然高度排布）。

### 控件形态与受控值

- `shape: capsule` 是类 `bern-capsule`（9999 圆角，分组控件的首尾按钮同理），`height` 用尺寸请求并去掉主题的最小高度与上下内边距（类 `bern-sized-control`）。`IconButton(height:)` 是该直径的圆形按钮，`font_size` 是图标像素尺寸，`prominence: prominent` 为强调色底。`Button(width: fill)` 横向撑满。`ControlGroup` 是 libadwaita 的 `linked` 分组，整体为胶囊、默认高 40。
- `Menu(style: icon)`：只显示图标的圆形扁平菜单按钮，标签为提示与无障碍名称。
- 受控值：`Switch(checked:)`、`Picker(selected:)`、`Slider(current:)` 显示表达式的值，改动时执行动作的命令，参数中与该表达式相同的一项换成新值；绑定草稿并带动作的 `Switch`、`Picker` 先写草稿再执行动作。受控的滑块拖动时只移动滑块，松开时提交；键盘与滚轮立即提交。`PickerOption` 给出自己的值与标签。`label_hidden` 不显示标签，只作无障碍名称。

### 布局

- `padding_x`、`padding_y`：在 `padding` 之上分别覆盖左右与上下的外边距。`Row(align: baseline)` 用 GTK 的 `BaselineCenter` 对齐。
- 声明的宽高就是请求的尺寸：`Column`、`Row` 给出 `width` 或 `height` 时外包 `BernSized`，`Stack` 自己测量，它们都不再因内容（如未截断的长标题）要求更宽。
- `Grid(row_gap:)` 单独设置行距。`Grid(cell_ratio:, cell_extra:)` 用 `BernCellGrid`：按最小宽度算出列数，每行高为列宽 ÷ `cell_ratio` + `cell_extra`，各项从格子顶部排，内容更高时伸进下面的行距。
- `Flow` 是 `BernFlow`：子项各取自然宽度，放不下时换到下一行，行高取该行最高者，子项在行内竖直居中；`Spacer`（带 `bern-spacer` 样式类）换行时不占宽度，排好后分得所在行余下的宽度。
- `ScaleBox` 是 `BernScaleBox`：子项在声明的逻辑尺寸中层叠（左上对齐），整体以一个变换缩放到能放下并居中、超出部分裁掉，所以图片、文字与点击区域保持对齐。`Positioned` 在所在层叠中占满，按（可弹簧的）`x`、`y`、`width`、`height` 放置内容。
- `Positioned(bleed:)`：`BernScaleBox` 分配时算出画布四周的空白（场景点）；有 `bleed` 的图层分配内容时向上找到最近的 `BernScaleBox`（中间隔着另一个图层就不延伸），把到达的每条画布边延伸到空白的边缘。四条边各是一个随图层 `animate` 的弹簧，碰到哪条边按图层要去的位置判断。窗口改变比例时 GTK 只改场景的变换，同样大小的子部件不会重新分配（`gtk_widget_allocate` 走 `ensure_allocate_on_children`），所以空白一变，盒子先让场景里有 `bleed` 的图层 `queue_allocate`，延伸量随即跳到新值。
- `Marquee`：内容按自然宽度从右缘外跑到左缘外，每 `duration` 秒一次，`phase` 为起始进度；`paused` 时停在原处，减少动态效果时静止在宽度 28% 处；文字带 1 点黑色阴影；`opacity` 可绑定。
- `Accordion`：列宽由每列一个 0 到 1 的权重弹簧决定，收起列为固定宽度，总宽保持不变；每列按 `corner` 圆角裁剪。
- `reflow` 现在也作用于 `Stack` 的各层和 `cell_ratio` 网格的格子（共用 `Slides`：布局后比较新旧位置，以弹簧从旧位置滑到新位置）；`GtkFlowBox` 网格与表单行仍然只警告。
- 滚动视图：内容沿滚动方向按自然尺寸排布（视口在该方向用 `Natural` 策略），横向滚动视图的高度取内容的自然高度。

### 页面结构

- `SidebarHeader` 固定在侧栏列表上方；`Sidebar(width:)` 以像素固定 `AdwNavigationSplitView` 的侧栏宽度；`Detail(titled: false)` 时详情标题栏不再显示选中项的名称；`SidebarItem(badge:)` 在行尾显示计数（为 0 时隐藏）。
- 工具栏中 `Spacer` 之前的项放在页面标题栏的前缘（返回按钮之后），之后的放在后缘；`Text` 可以放在工具栏里。
- 标签页：宽窗口的切换器在页面标题栏，紧凑窗口（`window.compact`）改为页面下方的 `AdwViewSwitcherBar`，与 GNOME 自适应应用相同；`TabAccessory` 在页面与底部标签栏之间。
- `Form(gap:, row_height:, padding_x:, padding_y:)` 与 `Section(style: plain, padding_x:, padding_y:)`：分组间距为盒子间距；行高与行内边距写成一条按值生成的规则（plain 行与 libadwaita 行的 `box.header`），`Section` 自己的边距优先，plain 分组不取表单的行高与内边距、只用自己的；plain 分组去掉列表的底色、边框、内边距与分隔，表单以它开头时从页面顶部开始：表单顶部的 24 由样式规则 `.bern-form-rows > :first-child:not(.bern-plain-section)` 给第一项，条件分支换掉第一项时跟着变。非 libadwaita 行的内边距改由行的 CSS 给出，才能被这些规则覆盖。
- 页面中的 `Search` 是 `GtkSearchEntry`，`width`、`height`、`font_size` 为框自身的尺寸与字号，结果列在框下的浮层中（已有实现）。

### 模态、搜索面板与计时

- `Modal` 在窗口之内：窗口的覆盖层里放一个 `BernModal`，它自己是铺满窗口、吸收输入的半透明背景（0.24 黑，200 毫秒淡入），卡片居中，为声明尺寸且离窗口边缘至少 16 点，24 圆角、阴影与细边；`cover: compact` 时在紧凑窗口中铺满整个窗口、没有圆角与边。元素的 `appear`、`disappear` 作用在卡片上。模态存在时下面的内容不取得焦点；卡片离开（播完消失过渡）后背景层随之移除。
- `Search(style: palette)`：一个扁平的触发按钮（图标与提示）；打开时在窗口之内显示同样的模态卡片（默认 640×480），上方是搜索框与关闭按钮，下面是结果。结果只在面板打开时构建，关闭即释放。上下方向键在 `navigation` 为真的 `SearchItem` 之间移动（按屏幕上的先后），回车选中当前项，没有当前项时执行提交动作；Esc、关闭按钮或点在卡片外关闭；指针停在某项上时选中它。快捷键激活目标即打开面板。`SearchItem` 选中后先关闭面板再执行动作（包括 `pick` 选文件）。
- `Timer`：启用且在显示时每 `interval` 秒执行动作（不少于 0.05 秒）；关掉或离开时取消下一次。
- `Window(idle_enabled:)`：为假时 `window.idle` 保持为假且不计时，变真时重新开始计时。

### 动效补充

- `appear_delay`：在错峰延迟之上再等待；有延迟时首次显示也播放 `appear`。
- `nudge_key`：值每次变化时晃动一次；`nudge_scale` 为晃动的放大倍数（默认 1.06）。
- `reduce_motion`：为真时整个应用按减少动态效果运行（与 `BERN_MOTION=reduce`、系统关闭动画同等）。
- `grow(edge:)` 过渡：沿一个轴从 0 伸展、该边保持不动、不淡入淡出。动效层的缩放现在可以指定不动的点（`pivot`），`scroll_origin_x`、`scroll_origin_y` 也改用它。

## 构建与运行

```sh
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml --target linux
target/bern/linux/Showcase/showcase
```

需要 GTK 4.14 与 libadwaita 1.5 的开发包（Ubuntu：`libgtk-4-dev`、`libadwaita-1-dev`）。产物位于 `target/bern/linux/`：`Showcase/showcase`（界面程序）、`Showcase/resources/`（随包资源，程序按自身位置查找）、`Showcase/dev.bern.showcase.desktop`、`bin/showcase` 与 `bin/showcase-mcp`、应用 crate `app/`、构建清单 `build.json`（记录 GTK 与 libadwaita 最低版本、媒体后端与呈现、资源名称）。Rust 编译目录为 `target/bern-glue-linux/`。

应用图标在 `Showcase/icons/dev.bern.showcase.png`（没有声明 `[app].icon` 时为 512 像素的 Bern 标志）。启动时这个目录加入图标主题的搜索路径，窗口的默认图标名为 `bundle_id`；找不到时在标准错误报告。生成的 `.desktop` 文件以绝对路径指向它。`[app.icon].linux` 为图标主题文件夹时，其中最大的 PNG 作为这个文件，全部图标改名为 `bundle_id` 后放在 `Showcase/icons/hicolor/` 下，`.deb` 把它们装进 `/usr/share/icons/hicolor`。GNOME 在 Wayland 下从已安装的 `.desktop` 文件取程序坞图标，把生成的文件复制到 `~/.local/share/applications/` 后程序坞显示标志；构建不会写入用户目录。

开发时 `BERN_SNAPSHOT_DIR` 让程序在每次变化后（包括 `window.idle` 变化）把各窗口渲染为 `<目录>/<窗口>.png`（窗口最小化时不绘制，浮层是独立的表面，不在其中），`BERN_APPEARANCE=light|dark` 覆盖系统外观，`BERN_MOTION=reduce` 按减少动态效果运行，`BERN_MOTION_LOG` 把每次变换、`reflow` 偏移与错峰延迟带毫秒时间写到标准错误；都默认关闭。

### 打包

`bern package --target linux` 按 release 构建，生成 `<程序>_<版本>_<架构>.deb`（系统有 `dpkg-deb` 时）与 `<产品>-<版本>-linux-<架构>.tar.gz`：

- `.deb` 把应用、图标与 `bin/` 中的 CLI、MCP 服务器装到 `/opt/<bundle_id>`，另有 `/usr/share/applications/<bundle_id>.desktop`、`hicolor` 图标主题中的图标与 `/usr/bin/<程序>` 启动器；依赖 `libgtk-4-1 (>= 4.14)` 与 `libadwaita-1-0 (>= 1.5)`，使用媒体时另加 `libgtk-4-media-gstreamer` 与 `gstreamer1.0-plugins-good`，并推荐 `gstreamer1.0-libav`。维护者取自 `[app].publisher`。
- 用到 `Svg` 时，`.deb` 另带 AppArmor 配置 `/etc/apparmor.d/<bundle_id>`。WebKitGTK 用 bubblewrap 为网页进程建沙箱，需要建立用户命名空间；Ubuntu 24.04 起（`kernel.apparmor_restrict_unprivileged_userns = 1`）只有 AppArmor 配置里有 `userns` 的程序能建立，沙箱失败时 WebKitGTK 中止整个应用。配置与 Ubuntu 为自带的 WebKitGTK 应用（epiphany、devhelp 等）提供的相同：`flags=(unconfined)`，只给程序一个名字和 `userns` 权限，不另加限制，并包含可选的 `local/<bundle_id>` 供管理员补充；附着路径是 `/opt/<bundle_id>/<程序>`（`/usr/bin/<程序>` 是指向它的链接，AppArmor 按解析后的路径匹配）。配置列为 conffile，`dpkg` 保留管理员的修改，卸载（remove）后保留、彻底卸载（purge）时删除。`postinst` 只在 AppArmor 启用时（`aa-enabled`）以 `apparmor_parser -r -T -W` 立即载入，载入失败不影响安装；`postrm` 在 purge 时删去编译缓存。配置使用 AppArmor 4 的语法（`abi <abi/4.0>`、`userns`），更早的 AppArmor 不能解析它（载入失败，安装照常完成）。没有 `Svg` 的应用不带配置，也没有维护脚本。
- `.tar.gz` 与开发构建不带配置。在限制用户命名空间的系统上运行用到 `Svg` 的应用，需要为解开后的界面程序添加同样的配置（需要 root；路径含空格时加引号），`bern package` 会提示这一点：

  ```sh
  sudo tee /etc/apparmor.d/<bundle_id> >/dev/null <<EOF
  abi <abi/4.0>,
  include <tunables/global>
  profile <bundle_id> /解开的目录/<产品>/<程序> flags=(unconfined) {
    userns,
  }
  EOF
  sudo apparmor_parser -r /etc/apparmor.d/<bundle_id>
  ```

  只为测试时也可以用 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` 运行，这会关闭 WebKitGTK 的网页进程沙箱。
- 暂存时保留可执行权限，打包前把文件统一为 644、程序与目录为 755，维护脚本另设为 755。
- 实测（2026-10-12，Ubuntu 24.04 ARM64）：`.deb` 1.06 MB，`.tar.gz` 1.9 MB；`desktop-file-validate` 通过，`apt-get -s install` 能解析全部依赖；解开 `.deb` 后从 `/opt` 位置启动并经包内 CLI 驱动正常。虚拟机的 sudo 需要密码，没有实际安装。没有桌面会话时，GTK 启动会等约 25 秒的 D-Bus 门户超时，引擎与控制通道随后才就绪。
- 实测（2026-10-18，同一虚拟机，AppArmor 4.0.0~beta3、WebKitGTK 2.52.3、bubblewrap 0.9.0，`examples/svg-animation`）：`.deb` 1.2 MB，含 `etc/apparmor.d/dev.bern.svgdemo`、`DEBIAN/conffiles` 与权限为 755 的 `postinst`、`postrm`；两个脚本 `sh -n` 通过；`apparmor_parser -Q -K`（不载入内核、不用缓存）解析配置通过，结果为 `Mode: unconfined` 与 `userns`；`apt-get -s install` 只新装这一个包。解开的程序不带配置运行时，约 26 秒后报 `bwrap: setting up uid map: Permission denied` 与 `Failed to fully launch dbus-proxy` 并中止。载入新配置需要 root，而虚拟机的 sudo 需要密码，所以没有安装 `.deb`，也没有用这份配置本身运行；改用系统自带、形式相同的 `busybox` 配置（`profile busybox /usr/bin/busybox flags=(unconfined) { userns, … }`），经 `busybox env` 启动同一程序：程序、WebKitNetworkProcess、两层 bwrap、xdg-dbus-proxy 与 WebKitWebProcess 都带 `busybox (unconfined)` 标签，沙箱建立成功，应用不中止，Broadway 中 SVG 正常显示。没有桌面会话时，带沙箱的网页视图在窗口出现约 44 秒后才创建（关闭沙箱时不到 1 秒），原因未查。

## 验证记录

2026-10-11，Ubuntu 24.04（aarch64，GNOME 46 Wayland），经 SSH 驱动虚拟机桌面会话中的应用，截图来自开发快照：

- `cargo test --workspace` 在 macOS 与 Linux 各 163 项通过；`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check` 通过；`bern-gtk` 的 clippy（`-D warnings`）与 fmt 在 Linux 上通过。新增测试：Linux 默认解析为 `gtk-linux`、显式选择未实现的技术失败；不支持的语言报错；生成的应用 crate、资源清单、构建清单、桌面文件与构建计划；生成代码不含颜色与未实现的组件；在装有 GTK 的环境中对生成的应用做类型检查。
- **控件与引擎**：`ui select` 切换侧边栏页面；`ui activate` 按钮后引擎计数增加，禁用按钮返回 `target_unavailable`；`ui set-bool`、`ui set-number`（引擎音量变为 72，页脚“已提交的音量：72”）、`ui select` 选择器与工具栏分段控件、`ui edit` 与 `ui submit`（引擎记录 `Submit`）均生效；密码框在 `ui.inspect` 中没有值。
- **查询**：相隔约 20 ms 输入“m”与“mo”，先 `refreshing v3 (5)`，随后 `ready v3 (1)`（影片），“m”的较慢答复没有出现；搜索“影”得到影片；搜索“下载”时 `showcase add-item --kind Downloads` 后 `refreshing v6 (1)` → `ready v6 (2)`；“!x” 时 `failed v7 (2)`，保留旧数据。
- **导航**：`ui activate --target "library_page.item_link[3]"` 推入“项目 3”，标题栏出现返回按钮，根页面的“添加项目”隐藏；`ui back` 返回后列表、搜索与工具恢复。
- **自适应与菜单**：`window resize --width 720 --height 600` 等到窗口实际变为 720 × 600 后返回；`media_page.clip_choice` 由 `expanded` 变为 `compact`，挂载“换片”菜单（`closed`），菜单项仍是 `media_page.pattern`、`bars`、`damaged`；菜单未打开时 `ui activate --target media_page.bars` 让引擎的 `featured` 变为 `Bars`。
- **语言**：用户在设置窗口的 `LanguagePicker` 中切换为简体中文后两个窗口全部更新；`ui set-language --language en` 后 `ui language` 为 `en`，`fr` 返回 `unsupported_language`。
- **关闭协议**：备注有内容时 `window close` 返回确认 `main#1:c1`（决策“放弃更改 / 取消”），对话框为标准 `AdwAlertDialog`，窗口内容变灰；待决时 `ui edit` 返回 `target_blocked`；`Cancel` → `Rejected`（`cancelled`）；之后修改草稿再用旧确认 → `stale_confirmation`；新确认 `Discard` → 窗口关闭、引擎继续运行；`window open --kind main` 得到 `main#2`，草稿为新。设置窗口：显示名称为空格时 `Save` → `Rejected`（`save_failed: invalid_name: …`）窗口保留；有效名称 `Save` → 关闭且引擎 `display_name` 为 “Ada”。
- **窗口命令**：`window maximize` 返回 `Maximized`、宽 1470（来源 `Command`）；`window restore` 回到 920 × 640；`window minimize` 立即返回 `ui_timeout` 并说明 Wayland 不报告最小化，窗口确已最小化（快照因不绘制而跳过）；`window activate` 后 `focused` 为 false（防抢焦点）。
- **媒体**（安装 `gstreamer1.0-libav` 之前）：精选片段为 `Failed`，错误为 “您的 GStreamer 安装缺少插件。”，`media play` 返回 `media_failed`；应用级预览播放器为 `Idle`，控制条禁用。
- **媒体**（安装之后）：精选片段 `Ready 0 / 8000`、可跳转；经控制条 `play_pause` 播放，`position` 跳到 5000 后立即回报 5000；`mute` 与 `volume` 生效；播完为 `Ended 8000 / 8000`，再播放从头开始；重复 `showcase feature --clip Pattern` 不重新加载；换为色条后源版本 2、`0 / 6000`，静音保留；外部 `media open` 返回 `source_bound`；换为损坏文件后 `Failed`（“无法确定流类型。”），`media play` 返回 `media_failed`。应用级播放器：详情页“在预览播放器中打开”后播放，返回列表后仍为 `Playing`、附着视图数由 1 变 0，回到视频页重新附着，关闭主窗口后继续播放。截图中画面按宽度铺满 16:9，控制条为系统控件。
- **虚拟机音频**：本机 VMware 虚拟机的声音输出时钟只有正常速度的约 40%（GStreamer 直接播放 8 秒片段：丢弃音频 8.2 秒，系统声音输出 20.2 秒），带声音播放时进度被拖慢并会停住。播完与重播的验证在关闭声音输出（`GST_PLUGIN_FEATURE_RANK` 把音频输出插件设为 NONE）下进行；这是验证环境的问题，不是 Bern 的。
- **直播**（安装 `gstreamer1.0-plugins-bad` 后，本机 HTTP 提供 `media/live/live.m3u8`）：`media open` 预览播放器后为 `Loading`，播放后 `Playing`，`duration_ms` 为空、不可跳转；`media seek` 返回 `not_seekable`；进度条禁用，时长处显示“直播”，画面正常。媒体的网络请求遵循代理环境变量，开发机的代理变量会让本机地址也走代理，测试时启动应用前去掉了这些变量。
- **外观**：浅色与深色（`BERN_APPEARANCE=dark`）下主窗口与设置窗口全部为 libadwaita 标准样式。

- **手动验证**（在虚拟机桌面中由用户操作）：备注有内容时点按标题栏关闭按钮与按 Ctrl+W 都先弹出“要放弃备注吗？”；设置窗口按 Esc 关闭；推入的详情页按 Esc 与 Alt+← 返回列表；中文输入法在“名字”中输入，预览正确显示。

2026-10-12，组件库第一批，同一虚拟机，应用经 Broadway（`gtk4-broadwayd`，不需要桌面会话）显示在浏览器中：

- `bern-backend-gtk` 的测试（包括对生成的应用做类型检查）通过，`bern-gtk` 的 clippy 没有警告，应用日志没有 GTK 警告。
- “图片与布局”页：三种 `fit` 在 120×90 的框中分别留白、裁掉两边、拉伸，只写宽度的标志保持比例；层叠中标志在右上角；网格在 680 宽的表单中为三列；不确定进度为转圈加灰色文字。CLI 执行 `showcase set-volume --value 90` 与 `ui set-number --target layout_page.layout_brightness --value 0.2` 后两个进度条立即变为 90% 与 20%。

2026-10-14，v0.29 与 v0.30 的能力，同一虚拟机，应用经 Broadway 显示在内置浏览器中；“实测”指在这个环境里看到或量到，动效的逐帧数据来自开发用的 `BERN_MOTION_LOG`（每次变换带毫秒时间打印），静态画面来自 `BERN_SNAPSHOT_DIR`：

- 测试：`bern-backend-gtk` 的测试在 macOS 与虚拟机上通过（虚拟机上对生成的 showcase 应用做类型检查）；新增测试确认 planner 与 gallery 在 GTK 上检查没有错误与警告、生成代码含样式表、绘图、装饰、轨道、滚动停靠等；侧栏项的阴影仍在检查时报错，豁免后通过；桌面文件为 `Drop(opens: true)` 登记 `MimeType` 与 `%U`。`bern-gtk` 与 `bern-backend-gtk` 的 clippy（`-D warnings`）通过。showcase 回归：各页面、设置窗口与页签切换正常。
- **测试环境**：Broadway 下 GTK 读不到 `gtk-xft-dpi`（为 -1），libadwaita 以 sp 为单位的尺寸（侧栏最小宽 180sp、折叠断点）换算得很小，侧栏只有约 80 像素；在虚拟机用户的 `~/.config/gtk-4.0/settings.ini` 写入 `gtk-xft-dpi=98304`（96 DPI，GNOME 的默认）后正常。内置浏览器窗格隐藏时浏览器节流，Broadway 的帧约 1 秒一帧，动画按时间推进但逐帧采样只能拿到首尾几帧。
- planner：画布的格纹（不透明度 0.8）、面积与折线、成功色胶囊；“最近的进度”的标注底纹；侧栏选中项填强调色、文字为强调色上的前景色；悬停时侧栏项从 1.0 弹到 1.03（约 15 毫秒一帧，0.15 秒内到位），离开后以 `settle` 回到 1（回弹的过冲因帧间隔未采到）；`show_archive` 打开后侧栏多出“归档”，在归档页关闭它后选中回到 `fallback` 的“概览”；模块页“笔记”经可选模块的条件出现；切换大标题时共享元素从小标题的位置与高度展开（首帧缩放 0.744、位移 −5，0.25 秒内到 1），旧画面淡出；新标签以 `pop(from: 0.4)` 出现（上屏后开始，首帧 0.463）；专注模式打开后静置 3 秒，侧栏与详情的标题栏都隐藏；主要按钮为强调色底、10 圆角、大尺寸。
- `reflow`（只在虚拟机的临时副本中给 planner 的标签列表加上 `reflow: smooth` 实测，仓库中的示例未改）：加入标签后，下面的两个按钮与摘要文字被布局下移 34 点，先以 −34 的偏移留在原处再滑向新位置（首几帧 −34.0 → −33.9，之后的帧因浏览器节流未采到）。
- gallery：侧栏底栏；工具栏（打开视频、外观菜单、分组竖线、详情开关、搜索框）；卡片为无外框的按钮、12 圆角裁住画布，悬停放大到 1.03 并投下阴影、星标出现（快照中看到）；按 `ForEach(stagger: 0.04)` 切到“图案”页时 6 张卡片的出现延迟为 0、0.04…0.2 秒（日志）；Ctrl+K 聚焦搜索框，输入后结果浮层列出匹配的图案，点选“Tartan”后打开它并收起结果；Ctrl+Alt+I 与 `ui.set_bool` 打开详情面板，内容让出位置，取色铺底按海报的平均色向下渐隐、海报叠在左下；收藏后底部出现 `AdwToast`；“外观”菜单把强调色换成群青、明暗换成深色，画布随之重绘；关于窗口以模态对话框打开（深色、强调色按钮），“好”关闭它。
- 画框页：`appearance: dark` 的部分在浅色窗口中为深色，其中的分体按钮、字号菜单、音量按钮与说明文字按深色绘制，分体按钮与音量的浮层也是深色；视频铺满列宽、圆角 20；静置 3 秒后边距收为 0、圆角收为直角、控制条卸载（中途快照可见边距在变化）；`ui.activate` 打开音量浮层后其中的滑块出现在 `ui.inspect`，分体按钮的开关与浮层生效，关闭说明后文字淡出；`ui.set_file` 给拖放区：`/etc/hostname` 返回 `unsupported_file`，非地址返回 `invalid_url`，视频地址存入字段并播放。
- 胶片页：初始停在字段指定的“Madras”正中，正中一格放大到 1.12、两侧变小变淡；“最后一个”滚到末尾那格并停在正中，右侧留出空白；时间轴的底线、6 个按比例的刻度、`anchor: start` 的名称、绑定 `level` 的竖线随 `animate: smooth` 移动；`ui.set_number` 设置 `Swipe` 与 `Pinch`（卡片宽 300）；中号窗口下 `Adaptive` 选中 `medium` 布局。
- `BERN_MOTION=reduce` 下切换页面：卡片不再错峰、没有位移与缩放，只做短淡入。
- 堆叠中的轨道：轨道原先会把其中没有高度的画布的 `vexpand` 传给外面的 `Stack`，窗口有多余高度时，“上下拖动”面板的水位会比 120 点的面板高。现已改为轨道只沿轴扩展、在 `Stack` 中铺满其他层给出的尺寸。只在虚拟机的临时副本里把窗口高度改成 900（Broadway 限为 768）实测：面板仍高 120（窗口坐标 470 至 590），水位底色正好铺满这 120 点，宽度为面板的 0.30（`level` 为 0.3），多余的高度留给 `Spacer`。
- 错峰顺序：错峰的次序在构建各行时按条目顺序分配，与 GTK 映射的先后无关。切到“图案”页时，日志中 6 张卡片都在约 0 毫秒写入起始姿态，各自的上升从 Gingham 3、Tartan 44、Madras 84、Glen 123、Houndstooth 164、Windowpane 204 毫秒开始（这次没有连接 Broadway 客户端，帧不前进，只量到每张卡开始的时刻）。

2026-10-18，页面级布局、场景与控件形态（上一节），同一虚拟机，GTK 4.14.5、libadwaita 1.5.0，应用经 Broadway 显示在内置浏览器中；目标是一个真实的媒体类应用，它的桌面与手机布局用到了上述大部分能力：

- 测试：`bern-backend-gtk` 的测试在 macOS 与虚拟机上通过；新增测试给 showcase 加上场景、手风琴、换行行、格子网格、模态、表单版式、纯图标标签与菜单、胶囊控件与搜索项，检查生成代码，并在虚拟机上对生成的应用做类型检查。`bern-gtk` 带与不带 `svg` 特性都能编译。
- 构建：该应用 `bern check --target linux` 由 67 个错误变为 0；`bern build --target linux` 构建出界面程序、CLI 与 MCP 服务器。
- **测试环境**：虚拟机装有 WebKitGTK 6 的运行库但没有开发包（sudo 需要密码）；构建时用 `PKG_CONFIG_PATH` 指向手写的 `webkitgtk-6.0`、`javascriptcoregtk-6.0`、`libsoup-3.0` pkg-config 文件，链接已装的库。Ubuntu 24.04 默认限制非特权用户命名空间（`kernel.apparmor_restrict_unprivileged_userns = 1`），WebKitGTK 的沙箱（bwrap）因此失败并中止整个应用（`Failed to fully launch dbus-proxy`）；测试时以 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` 运行；安装包为此附带 AppArmor 配置，见“打包”。Broadway 下，被点击的控件随即在指针下被替换后，GTK 报 “Broken accounting of active state”，此后的点击都丢失；一个纯 PyGObject 的 GTK 4 程序同样如此，所以这是 GTK/Broadway 与合成点击的问题，在窗口外的页面空白处点一下即可恢复。部分操作改用应用 CLI 的 `ui activate`（与点击执行同一个控件闭包）。
- 修正（实测中发现并已改）：层叠在子层第一次插入时按当时是否扩展定下对齐，之后内容变为撑满（如换成滚动视图）时不再更新，现在每次排布时决定；有声明尺寸的层叠以声明尺寸为排布区域；带声明尺寸的图片框与 WebView 不再把扩展传给外面；轨道不再让所在层叠横向扩展；Toast 自定义标题省略过长文字，不再把窗口撑宽。
- 引导模态（实测）：960×600 的卡片居中、圆角与阴影，背景变暗；格纹插画与居中的动画标志；“跳过”到最后一步时内容居中、底栏在底部。
- 桌面布局（1024×768，Broadway 的屏幕上限，实测）：`SidebarHeader` 的模板图标（前景色）与标题、侧栏计数、工具栏前缘的返回与后缘的搜索面板入口；时间轴的 `Rail(overhang: 8)`：轨道宽 704，第 0 小时刻度（宽 30、中心锚点）在 x = −23，即 0 × 720 − 8 − 15，最后一格 697；海报列 `Column(width: 88)` 宽 88（修正前被标题撑到 118）。播放器：1280×800 的场景缩放进窗口、`Positioned` 的画面按“后退一步”带圆角、带章节与标记的进度条、胶囊按钮与下拉；播放时 `immersive` 隐藏标题栏。
- 手机布局（只在虚拟机的临时副本中把主窗口改为 390×844、最小宽 360；声明的最小宽 720 让桌面窗口到不了紧凑档，实测）：`Adaptive` 选中 `compact`，底部 `AdwViewSwitcherBar` 标签栏；首页 16:9 圆角舞台、整宽胶囊按钮、横向卡片；页面没有超出 390 的宽度。
  - `TabAccessory`：播放后收起，小播放器（64×36 缩略图带进度、标题、集数与时间、暂停与关闭）出现在页面与底部标签栏之间；关闭播放后它收起（快照与画面）。
  - `ScrollView(open_at: 0.4852…)`：时间轴横向滚动视图打开时的调整量为 263 / 上限 916 / 页宽 374，即 0.4852 × (916 − 374)。
  - `scroll_scale: 1.12` 与 `scroll_origin_y: 0.6`（详情页的影片条）：居中一格的图片高 79 点、旁边一格 71 点（≈1.12 倍），居中一格的顶边比旁边上移约 4.8 点、底边下移约 3.2 点，即 60% 向上、40% 向下。`scroll_origin_x` 走同一条代码，未单独实测。
  - `pages.new.show()`：首页“新番表 ›”经 `ui activate`（与点击同一个闭包）执行后，标签页切到“新番”。
- 未验证：`Marquee` 的弹幕滚动（场景中未出现弹幕行）、`Accordion` 的宽度弹簧（只看到窄窗口的纵向布局）、搜索面板的键盘导航、`Flow`、`Timer` 的计时以外的部分、`grow` 过渡与 `appear_delay` 的逐帧数据、`reduce_motion` 的切换、`Form` 的行高与内边距规则；真实触屏与 Linux 手机外壳（Phosh）。

未验证：真实的拖放（Broadway 不支持从系统拖入文件）、`GtkFileDialog` 的选择（无桌面会话时门户超时）、系统交来文件（`opens`）、触摸板捏合与触摸拖动、对话框在 GNOME 中的附着位置、高对比、真实 Wayland/X11 会话中的帧率与手感。

未完成的验证：真实声卡下的播放；完整的键盘遍历、屏幕阅读器；高对比与大字体；X11 会话与 KDE Plasma；分数缩放与多显示器。

2026-10-20，Ubuntu 24.04 虚拟机经 Broadway，用于验收的媒体类应用的设置窗口“可选功能”（窗口快照与生成的 CLI）：

- `Stack` 里两个方向都不给尺寸的绘图（`DrawingArea`）是其他层的底板，和轨道一样不让 `Stack` 撑开：此前预览里选中项的 `Stack` 因底板而纵向撑开。
- `Form` 横向也占满给它的空间，与其他目标相同：此前它的横向撑开是借每行图标底的 `Stack` 撑开得来的，底板的规则改正后表单被挤成一列窄条。
- 写了 `reflow` 的行与列（`ReflowBox`）像 GTK 的盒子一样汇总子元素的撑开：此前预览播放条那一行只有自然宽度，其中的轨道分到 0 宽，进度条不显示。
- 带底板的 `Stack` 横向占满分给它的宽度、底板随之铺满：此前层叠按其他层的自然宽度分配，设置页插件横幅的底图只有文字那么宽。
- 实测：选中项一行高；表单与预览并排、文字不再逐字换行；播放条显示进度；插件横幅与正文同宽，卡片两列对齐。`crates/bern-gtk` 的 `cargo clippy --all-features --all-targets -- -D warnings` 与 `cargo test --all-features`、`cargo test -p bern-backend-gtk` 通过。图标主题中没有的名字仍显示缺图占位（见下）。

2026-10-20，图标补齐：

- 此前对照表只有 55 个名字，表中没有的显示缺失图标（`image-missing-symbolic`），检查不知道。用于验收的媒体类应用的主侧栏（首页、新番、媒体库）、设置侧栏的若干项与插件卡片都是缺失图标。
- 对照表移到 `bern-gtk/src/icon_table.rs`，与检查共用（见“控件映射”后的图标一段）；检查报“the GTK icon set has no `…`”。
- 新增 38 个名字，都取 Adwaita 46 中的图标（只有一个取自 libadwaita）：`house` → `go-home`、`calendar` → `x-office-calendar`、`film.stack` → `applications-multimedia`、`server.rack` → `network-server`、`wifi` → `network-wireless`、`keyboard` → `input-keyboard`、`puzzlepiece.extension` → `application-x-addon`、`arrow.triangle.2.circlepath` → `emblem-synchronizing`、`arrow.up.arrow.down` → `view-sort-descending`、`arrow.up.left.and.arrow.down.right` → `view-fullscreen`、`backward.end.fill` / `forward.end.fill` → `media-skip-backward` / `media-skip-forward`、`gobackward.10` / `goforward.10` → `media-seek-backward` / `media-seek-forward`、`play` / `pause` → `media-playback-start` / `media-playback-pause`、`speaker.wave.2` → `audio-volume-medium`、`camera` → `camera-photo`、`link` → `insert-link`、`paperplane` → `mail-send`、`moon` → `weather-clear-night`、`eye.slash` → `view-conceal`、`scissors` → `edit-cut`、`shield` → `security-high`、`tv` → `tv`、`checkmark.circle` → `selection-mode`、`circle` / `circle.fill` → `radio` / `media-record`、`square` → `checkbox`、`chevron.up` → `pan-up`、`arrow.left` → `go-previous`、`arrow.down` → `document-save`，以及上一段列出的代替图标（名字都省略了 `-symbolic` 后缀）。选择时把虚拟机上 Adwaita 46 的全部符号图标渲染成图表对照。
- 测试：`bern-backend-gtk` 的单元测试核对表已排序并覆盖 Bern 图标集；生成测试把画廊的第一个图标换成新名字（检查通过）和不存在的名字（检查报错）；`bern-gtk` 的测试在装有 Adwaita 的机器上核对表中每个图标都在 Adwaita 里（`adw-` 开头的除外）。
- 实测（Ubuntu 24.04 虚拟机，GTK 4.14.5、libadwaita 1.5.0、Adwaita 46，经 Broadway）：该应用 `bern check --target linux` 没有错误；主侧栏的首页、新番、媒体库，设置侧栏全部 15 项（经 CLI 打开 5 个可选模块后），“可选功能”页的各行与右侧预览（含播放条），插件市场的 6 张卡片都显示主题图标，没有缺失图标；`adw-external-link-symbolic` 在 libadwaita 1.5 下解析成功。虚拟机上 `crates/bern-gtk` 的 `cargo test --all-features icons` 与 `cargo clippy --all-features --all-targets -- -D warnings` 通过。

2026-10-20，标签选择器与换行工具条（Ubuntu 24.04 虚拟机，经 Broadway，无头 Chromium 截图）：

- 应用的设置 › 插件（设置窗口 1000 宽）：分段控件为 `linked` 的切换按钮「市场 / 已安装 3」，靠左；五个 chip 为同组的 `GtkToggleButton`，靠右，选中的「全部」底色更深，与原型 Adwaita 版（6 px 圆角、粗体、选中更深的中性底）一致；两者竖直居中。
- 未验证：窄宽度下的换行。该应用的主窗口最小 720、设置窗口最小 1000，到不了手机布局；Broadway 下 `window resize` 不生效。`BernFlow` 的断行与分配抽成 `flow_lines`，单元测试用原型手机版的实测尺寸（分段 123、chip 54、宽 358、间距 10）算出第一行 chip 在 176、240、304，第二行在 0、64，与原型一致。
- `cargo clippy --all-features --all-targets -- -D warnings` 与 `cargo test --all-features`（92 项）通过；`cargo test -p bern-backend-gtk` 对生成的展示应用做类型检查通过。

2026-10-20，量到宽度之前的布局选择（Ubuntu 24.04 虚拟机，经 Broadway；提交 67fc1be 的副本，只在虚拟机副本的 `bern-gtk` 里临时向 stderr 打印窗口的打开尺寸、量到的宽度与 `Adaptive` 每次填入的布局，看完改回）：

- `examples/gallery`（主窗口声明 900 × 600，断点 600 / 1024 / 1440）：主窗口在挂内容之前按打开尺寸 900 定为 `medium`，量到 900 后不变。经 CLI 的 `ui select` 切到胶片页，新挂的 `Adaptive(id: film_controls)` 只填一次 `medium`（按窗口宽 900），之后量到自己的可用宽度 675，仍是 `medium`，没有重填；画面是 `medium` 布局（两个文字按钮，没有 `expanded` 那行说明）。「关于」对话框按声明的 360 × 220 打开即为 `compact`，量到 360 不变。
- Broadway 的显示器在浏览器视口改变大小之后才跟着变，此前（包括刚连上或重新加载页面时）一直报 1024 × 768，所以上面的打开尺寸没有受显示器限制。
- 比显示器宽的窗口：视口改为 520 宽后，经 CLI 关掉再打开主窗口，打开尺寸按显示器限为 520，定为 `compact`；实际窗口 751 宽，量到后换成 `medium`。推断是 GTK 把默认尺寸限在显示器内，但窗口不会窄于内容的最小宽度，所以这时的估计比实际窄，会先按窄一档构建一次。
- 所在位置比窗口窄一档：在这个 751 宽的窗口里切到胶片页，`Adaptive` 先按窗口宽填 `medium`，量到自己的 552 后换成 `compact`。用窗口宽代替的估计不小于它的可用宽度，侧栏占去宽度时就会这样多构建一次，与规则相符。
- 未验证：真实 Wayland/X11 会话与手机外壳（Phosh）中窗口是否被限在屏幕内；多显示器。

## 未完成与已知限制

- WinUI 3 与 `flutter + macos` 属于阶段五后续批次。
- `reflow` 在 `Column`、`Row`、`Stack` 与 `cell_ratio` 网格中生效；没有 `cell_ratio` 的 `Grid` 是 `GtkFlowBox`，表单行由 `GtkListBox` 布置，它们的子元素不能在布局后另加偏移，写了会构建警告。
- 用到 `Svg` 的应用依赖 WebKitGTK 的沙箱，`.deb` 为此附带 AppArmor 配置（见“打包”）。实际安装未实测（需要 root）：`postinst` 载入配置、安装后从菜单或 `/usr/bin` 启动、remove 与 purge 后的情形都没有验证，只验证了包的内容、配置的解析和同形配置下的运行。`.tar.gz` 与开发构建仍需手动添加配置，或只为测试以 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` 运行。
- `Adaptive` 在第一次排布之前按 `expanded` 构建，窄窗口启动时先构建宽布局，排布后才换成窄布局。
- Adwaita 46 没有气泡、筛选、滑杆、闪光、画中画这几类图标，对应名字用最接近的图标代替：`text.bubble` → `chat-message-new-symbolic`（带加号的气泡）、`line.3.horizontal.decrease` → `format-justify-center-symbolic`、`slider.horizontal.3` → `preferences-system-symbolic`、`sparkles` → `star-new-symbolic`、`pip.enter` → `focus-windows-symbolic`。`note.text`、`paintbrush`、`paintpalette` 用的是 Adwaita 46 `legacy` 目录中的图标，更新的 Adwaita 若移除它们会显示缺失图标（未验证）。
- `hero` 只在同一窗口内配对；离开的元素若是视频或画布不留画面，只有新元素从旧位置展开。
- 侧栏项的样式没有 `shadow`（与其他目标相同，检查时报错）。
- `appearance` 的部分依赖 libadwaita 自带的 `base.css` 与 `defaults-*.css`（1.5 的资源路径）；Ubuntu 的 Yaru 强调色变体未单独处理，部分里使用 libadwaita 默认的取值。
- 搜索结果浮层在结果变少时不一定缩小（GTK 浮层保持较大的尺寸），实测中出现过空白下方。
- 强调色在深色部分里沿用窗口模式下的 `accent_color`。
- 窗口位置与移动不可用（GTK 4 的限制）；Wayland 下最小化结果无法确认；虚拟窗口呈现未实现（GTK 目标总有原生窗口）。
- `Menu` 的菜单项图标与 `destructive` 角色不呈现（GNOME 菜单通常只有文字）。
- 多实例窗口、焦点迁移、Flatpak 等打包与图标安装尚未实现；桌面文件的 `Exec` 是构建机上的绝对路径。
- 设置窗口切换页签只构建选中的页签。
- HDR 图片的 HDR 输出（GTK 4.16 起、Wayland、打开色彩管理）没有实测：测试用的虚拟机是 GTK 4.14，也不能输出 HDR。GTK 的 JPEG 加载器不读增益图，所以增益图由 Bern 应用；GTK 的色彩空间转换只截断不做色调映射，所以 GTK 不输出 HDR 时由 Bern 先转成 SDR。
- `Video` 的 HDR 视频依赖 GTK：GTK 4.16 起 GStreamer 的帧带有色彩空间，HDR 输出同样需要打开色彩管理；未实测。
- 其余与 macOS 相同的限制见 [macOS SwiftUI 实现记录](macOS-SwiftUI.md)“未完成与已知限制”中与平台无关的条目。
