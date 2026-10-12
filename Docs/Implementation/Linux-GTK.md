# Linux GTK 实现记录

- 日期：2026-10-11
- 依据：[Bern 架构设计 v0.15](../Design.md)、[Windows 与 Linux 适配](../Adaptation/WindowsLinux.md)
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
| `Picker` | `segmented` 为 `linked` 的切换按钮组；其他为 `AdwComboRow` 或 `GtkDropDown` |
| `Menu` / `MenuItem` | `GtkMenuButton` 与 `GMenu`；菜单项是动作，可用性来自声明 |
| `LanguagePicker` | `AdwComboRow`，选项为语言的自称 |
| `Query` | 加载：转圈与文案；空结果与无数据失败：紧凑的 `AdwStatusPage`（失败带“重试”）；刷新与有数据的失败：数据上方的状态行 |
| `Adaptive` / `Layout` | 一个零高度的绘图区测量可用宽度，按 `[layouts]` 断点选择布局 |
| `Video` | `GtkPicture` 显示播放器的 `GtkMediaFile`，下方为标准控制条（见“媒体”） |
| `Image` | `GtkPicture`（`content-fit` 对应 `fit`，可缩小）放在固定尺寸的 `BernPictureFrame` 中：`GtkPicture` 本身要求原图大小，这个小部件把它限制为声明的宽高，得到更多空间时仍靠前缘；只写一边时另一边按图片比例。JPEG 经 gdk-pixbuf 解码并按 EXIF 方向转正（GTK 4.14 自己的加载器忽略方向），其他格式用 GTK 的加载器。`label` 为替代文本，没有时对辅助技术隐藏 |
| `Progress` | 有 `value` 时为标签下的 `GtkProgressBar`，由标签命名；没有时为 `GtkSpinner` 加 `dim-label` 文字（libadwaita 1.5 还没有 `AdwSpinner`） |
| `Grid` | 同宽单元格的 `GtkFlowBox`：每格至少 `min_width` 宽，不可选择，单元格本身不获得焦点 |
| `Stack` | `GtkGrid` 的同一个格子，子元素的顺序即叠放顺序，按 `align` 设 `halign` 与 `valign`；外面一层横向 `GtkBox` 让它保持自身尺寸 |

图标使用 Bern 的 SF Symbols 命名，映射到 freedesktop 符号图标名（例如 `magnifyingglass` → `system-search-symbolic`），由用户的图标主题绘制；GNOME 上即 Adwaita 图标。

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
- 构建生成 freedesktop 桌面文件（各语言的名称），可安装到 `~/.local/share/applications`。
- 颜色全部来自 libadwaita：深浅色跟随系统，生成代码不设置颜色；声明了品牌色时映射为 libadwaita 的强调色。

## 构建与运行

```sh
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml --target linux
target/bern/linux/Showcase/showcase
```

需要 GTK 4.14 与 libadwaita 1.5 的开发包（Ubuntu：`libgtk-4-dev`、`libadwaita-1-dev`）。产物位于 `target/bern/linux/`：`Showcase/showcase`（界面程序）、`Showcase/resources/`（随包资源，程序按自身位置查找）、`Showcase/dev.bern.showcase.desktop`、`bin/showcase` 与 `bin/showcase-mcp`、应用 crate `app/`、构建清单 `build.json`（记录 GTK 与 libadwaita 最低版本、媒体后端与呈现、资源名称）。Rust 编译目录为 `target/bern-glue-linux/`。

应用图标在 `Showcase/icons/dev.bern.showcase.png`（没有声明 `[app].icon` 时为 512 像素的 Bern 标志）。启动时这个目录加入图标主题的搜索路径，窗口的默认图标名为 `bundle_id`；找不到时在标准错误报告。生成的 `.desktop` 文件以绝对路径指向它。GNOME 在 Wayland 下从已安装的 `.desktop` 文件取程序坞图标，把生成的文件复制到 `~/.local/share/applications/` 后程序坞显示标志；构建不会写入用户目录。

开发时 `BERN_SNAPSHOT_DIR` 让程序在每次变化后把各窗口渲染为 `<目录>/<窗口>.png`（窗口最小化时不绘制），`BERN_APPEARANCE=light|dark` 覆盖系统外观；两者默认关闭。

### 打包

`bern package --target linux` 按 release 构建，生成 `<程序>_<版本>_<架构>.deb`（系统有 `dpkg-deb` 时）与 `<产品>-<版本>-linux-<架构>.tar.gz`：

- `.deb` 把应用、图标与 `bin/` 中的 CLI、MCP 服务器装到 `/opt/<bundle_id>`，另有 `/usr/share/applications/<bundle_id>.desktop`、`hicolor` 图标主题中的图标与 `/usr/bin/<程序>` 启动器；依赖 `libgtk-4-1 (>= 4.14)` 与 `libadwaita-1-0 (>= 1.5)`，使用媒体时另加 `libgtk-4-media-gstreamer` 与 `gstreamer1.0-plugins-good`，并推荐 `gstreamer1.0-libav`。维护者取自 `[app].publisher`。
- 暂存时保留可执行权限，打包前把文件统一为 644、程序与目录为 755。
- 实测（2026-10-12，Ubuntu 24.04 ARM64）：`.deb` 1.06 MB，`.tar.gz` 1.9 MB；`desktop-file-validate` 通过，`apt-get -s install` 能解析全部依赖；解开 `.deb` 后从 `/opt` 位置启动并经包内 CLI 驱动正常。虚拟机的 sudo 需要密码，没有实际安装。没有桌面会话时，GTK 启动会等约 25 秒的 D-Bus 门户超时，引擎与控制通道随后才就绪。

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

未完成的验证：真实声卡下的播放；完整的键盘遍历、屏幕阅读器；高对比与大字体；X11 会话与 KDE Plasma；分数缩放与多显示器。

## 未完成与已知限制

- WinUI 3 与 `flutter + macos` 属于阶段五后续批次。
- 窗口位置与移动不可用（GTK 4 的限制）；Wayland 下最小化结果无法确认；虚拟窗口呈现未实现（GTK 目标总有原生窗口）。
- `Menu` 的菜单项图标与 `destructive` 角色不呈现（GNOME 菜单通常只有文字）。
- 多实例窗口、焦点迁移、Flatpak 等打包与图标安装尚未实现；桌面文件的 `Exec` 是构建机上的绝对路径。
- 设置窗口切换页签只构建选中的页签；窄窗口下没有底部视图切换栏。
- `Image` 在 Ubuntu 24.04 的 GTK 4.14 上（虚拟机经 Broadway，只有 SDR 输出）实测：带增益图的 JPEG 显示其 SDR 基础图，PQ 的 PNG 整体发灰、对比度低。按 GTK 的 NEWS 与源码：4.14 没有色彩空间；4.16 加入 `GdkColorState`，加载 PNG 时读取 cICP 并在合成时转换，PQ 与 HLG 图片因此显示正确；HDR 输出依赖 Wayland 的 `wp_color_manager_v1`（4.17.5 起），但 4.18.1 起改为需要 `GDK_DEBUG=color-mgmt` 才启用，主线（4.24）仍是如此；GTK 的 JPEG 加载器固定按 sRGB 解码，不读增益图与 ICC，带增益图的 JPEG 只能显示基础图，除非 Bern 自己解码增益图。较新 GTK 上的效果尚未实测。
- 其余与 macOS 相同的限制见 [macOS SwiftUI 实现记录](macOS-SwiftUI.md)“未完成与已知限制”中与平台无关的条目。
