# Windows WinUI 3 实现记录

- 日期：2026-10-11
- 依据：[Bern 架构设计 v0.16](../Design.md)、[Windows 与 Linux 适配](../Adaptation/WindowsLinux.md)
- 工具链：Rust 1.99.0；.NET SDK 10.0.401（C# 14，目标框架 `net10.0-windows10.0.26100.0`）；Windows App SDK 1.8.260921001（WinUI 3）
- 验证环境：Windows 11 专业版 25H2（版本 26200.9457，x64 实体机），显示缩放 150%

本文记录阶段五第二批 `winui3 + windows` 目标的实际完成情况。同一份展示应用（`examples/showcase`）的声明、协议、业务核心和 IR 不变，只增加了 `[targets.windows]`，由新的 WinUI 3 目标模块生成 Windows 应用；解析器、公共 IR、核心检查规则与业务核心没有为 WinUI 3 做任何修改。为了让工作区在 Windows 上构建，本地控制通道、WebSocket 握手的随机数、资源的文件 URL 与 `Bern.toml` 所在目录的规范化补上了 Windows 分支（见“跨平台修正”）。阶段二、阶段四的通用验收在 Windows 上重新执行，结果见“验证记录”。`flutter + macos` 属于阶段五的后续批次，本轮没有开始。

## 宿主方案

设计文档把宿主语言留到原型阶段决定。比较 C++/WinRT、C#/.NET 与 Rust 直接调用 WinRT 后选用 C#：

- WinUI 3 的控件、样式、可访问性与 Windows App SDK 的窗口接口以 C# 为一等语言，文档与示例最完整；C# 14 的静态 lambda 能表达与其他目标相同的“闭包不捕获、在上下文中求值”的生成形式。
- 引擎仍是 Rust：生成的胶水 crate 把引擎、会话运行时与 `bern-ffi` 的 C 接口编译为 `bern_app.dll`，C# 运行时以 P/Invoke 调用，与 SwiftUI 目标调用的是同一套函数。回调用 `[UnmanagedCallersOnly]` 函数指针接收，复制字节后排入 UI 线程的 `DispatcherQueue`，按到达顺序处理。
- 界面全部用代码构建，不生成 XAML 页面，不经过 XAML 编译器的页面生成；应用实现 `IXamlMetadataProvider` 以使用标准控件，自行提供 `Main`（`DISABLE_XAML_GENERATED_MAIN`）。
- 不打包（`WindowsPackageType=None`），Windows App SDK 随应用部署（`WindowsAppSDKSelfContained`），不依赖系统安装的 Windows App Runtime；.NET 为框架依赖，需要 .NET 10 运行时。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target windows` 先构建 `bern_app.dll`、CLI 与 MCP 服务器，再用 `dotnet build` 构建应用；生成测试在装有 .NET 的 Windows 上编译生成的应用 |
| 新目标不改解析器、公共 IR 与业务核心 | 完成 | 新增 `bern-backend-winui3`（生成器与 C# 运行时模板）；其余改动只有构建工具注册目标、示例增加目标配置与跨平台修正 |
| 目标禁用后其他构建独立可用；具体框架 SDK 不是全局前置条件 | 完成 | C# 运行时以模板随目标模块分发，只在生成的应用中编译；macOS 上 `cargo test --workspace` 与 clippy 不需要 .NET |
| 省略 UI 技术时的默认解析 | 完成 | `platform = "windows"` 解析为 `winui3-windows`（测试） |
| 状态可订阅；带参数命令与错误返回 | 完成 | 引擎状态按字段镜像为响应式单元；命令经真实控件闭包与外部入口执行，失败以窗口底部的 `InfoBar` 显示并返回给调用方 |
| 列表身份正确、订阅可释放 | 完成 | `ForEach` 按键保留行；查询订阅随视图与窗口结束 |
| 同一主题快照正确映射深浅色 | 完成 | 原生优先：颜色全部来自 WinUI 的主题资源，生成代码不写颜色；声明的品牌色映射为强调色资源；深色截图见验证记录 |
| 原生多窗口、关闭确认、窗口事实 | 完成 | 每个窗口是 WinUI `Window`，经 `AppWindow` 与 `OverlappedPresenter` 管理；最小化、恢复、最大化、移动、调整尺寸都由命令驱动并以平台事实答复 |
| Video 原生呈现 | 完成 | `Windows.Media.Playback.MediaPlayer` 执行端与 `MediaPlayerElement` 呈现、命令驱动的控制条；播放、暂停、跳转、静音、音量、播完、损坏文件与应用级播放器跨页面已实测 |
| 阶段四：`Adaptive`、`Menu`、`LanguagePicker`、运行时切换语言 | 完成 | 窗口变宽后布局切换为 `expanded`，变窄后挂载菜单；语言切换后两个窗口与标题栏全部更新 |

## 工程结构

| 组成 | 职责 |
| --- | --- |
| `bern-backend-winui3` | `winui3 + windows` 目标：描述、检查、从 IR 生成 C#、生成胶水 crate、.NET 项目与构建计划。工作区成员，不依赖 .NET |
| `templates/*.cs` | C# 运行时：值与编解码、响应式单元、上下文与父容器、宿主互操作、窗口、会话命令、布局面板、控件、结构容器、查询与媒体。随目标模块嵌入，生成时复制到应用的 `app/Bern/` |
| 生成的胶水 crate | `target/bern/windows/glue/`：`bern_app.dll`（`cdylib`，导出 `bern_app_start` 与 `bern-ffi` 的全部 C 接口）、`cli.rs`、`mcp.rs` |
| 生成的 .NET 项目 | `target/bern/windows/app/`：`App.cs`（组件、窗口、文案表、类型表与 `AppDef`，以及 `Main`）、`Showcase.csproj`、`app.manifest`（每显示器 DPI 感知 v2） |

生成的 C# 不另写一套 UI 运行时，而是调用运行时的标准组件（`B.Text`、`B.SplitView`……），形式与 GTK、Web 目标相同：每个组件生成一个静态方法，表达式生成在上下文 `c` 中求值的 C# 表达式，局部变量放在上下文里，所有 lambda 都是 `static`，编译器保证它们不捕获任何东西。值以 `Value` 表示，编码与解码按 Bern 二进制布局实现，类型表合并了应用、媒体目录与会话目录的类型。

## 控件映射

依据 Windows 11 的 Fluent 设计与 WinUI 3 标准控件；表单里的控件是 Windows 设置应用那样的卡片行。

| Bern | WinUI 3 |
| --- | --- |
| `Window` | `Window`，Mica 背景，内容延伸到标题栏并使用 `TitleBar` 控件；窗口底部的 `InfoBar` 显示失败的命令 |
| `SplitView` / `Sidebar` / `Detail` | `NavigationView`：分节标题为 `NavigationViewItemHeader`，侧边栏搜索为窗格中的 `AutoSuggestBox`；详情区的页头显示所选目的地、工具与返回。窗口宽度不足 800 时窗格收为图标栏，更窄时收为汉堡菜单 |
| `Navigation` / `Destination` / `NavLink` | 根内容与顶层目的地叠放，推入时根内容保持挂载；页头左侧出现返回按钮，Esc、Alt+← 返回；链接在表单中是带 `›` 的可点按卡片 |
| `Tabs` / `Tab` | 页头中的 `SelectorBar`（带图标），只构建选中的页签 |
| `Toolbar` | 所在页面页头的末端；推入目的地时隐藏根页面的工具 |
| `Form` / `Section` | 滚动的页面，内容最宽 1000；分节为标题、卡片行与脚注 |
| `Text` / `Label` | `TextBlock` 使用 WinUI 字体样式（`TitleLargeTextBlockStyle`…`CaptionTextBlockStyle`），次要文字为 `TextFillColorSecondaryBrush`；`Label` 前有 Segoe Fluent Icons 图标 |
| `Button` / `IconButton` | `Button`；`prominent` 为 `AccentButtonStyle`，`plain` 与图标按钮以轻量样式资源设为透明（静止与禁用时无填充，悬停与按下为系统的 subtle 填充），`destructive` 文字为 `SystemFillColorCriticalBrush`；图标按钮的名称为提示与可访问名称 |
| `TextInput` | `TextBox`、`PasswordBox`；多行为可换行的 `TextBox`；表单中单行输入在卡片右侧，多行在标题下方 |
| `Search` | `AutoSuggestBox`，带查找图标 |
| `Switch` | 表单中为卡片右侧的 `ToggleSwitch`；`checkbox` 样式为 `CheckBox` |
| `Slider` | 标题与两端带图标的 `Slider`；拖动只改本窗口预览，松开提交一次，键盘立即提交 |
| `Picker` | `segmented` 为 `SelectorBar`；其他为 `ComboBox`，表单中在卡片右侧 |
| `Menu` / `MenuItem` | `DropDownButton` 与 `MenuFlyout`；菜单项各自登记，菜单未打开时也能执行 |
| `LanguagePicker` | `ComboBox`，选项为语言的自称 |
| `Query` | 加载：`ProgressRing` 与文案；空结果与无数据失败：居中的图标与标题（失败带“重试”）；刷新与有数据的失败：数据上方的状态行 |
| `Adaptive` / `Layout` | 容器的 `SizeChanged` 给出可用宽度，按 `[layouts]` 断点选择布局 |
| `Video` | `MediaPlayerElement` 显示播放器，下方为标准控制条（见“媒体”） |

内容沿一个方向排列时使用运行时的 `FlexPanel`：与 `StackPanel` 一样堆叠，但 `Spacer` 在行中占据剩余宽度，滚动表单、导航视图与导航容器在列中占据剩余高度，与其他目标的盒子布局一致。图标使用 Bern 的 SF Symbols 命名，映射到 Windows 11 的 Segoe Fluent Icons 字形。

## 运行结构

- **窗口与草稿**：窗口打开时向会话运行时登记，获得代次与外部 ID（`main#1`），草稿单元由运行时返回的初值创建；写入先经运行时，再更新单元。窗口关闭即释放草稿、控件登记与查询订阅，应用级播放器继续。
- **控件登记与会话命令**：与其他目标相同：目标 ID 为 `组件.控件id`，列表行附加 `[键]`；禁用、被关闭确认阻挡、多处挂载分别返回 `target_unavailable`、`target_blocked`、`target_ambiguous`；密码框不暴露值。
- **应用生命周期**：按 Windows 应用的惯例，同一用户只运行一个实例（`AppInstance.FindOrRegisterForKey` 以 `bundle_id` 为键），再次启动会把激活转交给运行中的实例并显示主窗口；最后一个窗口关闭后应用退出，引擎随之停止。Ctrl+W 关闭当前窗口，设置窗口按 Esc 关闭，设置窗口不可最小化与最大化。
- **定时器**：运行时的延时（窗口命令超时、跳转超时、开发快照）经同一个辅助函数创建 `DispatcherQueueTimer` 并保持引用到触发为止；只放在局部变量里的定时器会被回收而永不触发。
- **外部入口**：与其他目标相同的本地控制通道（`channels = ["ipc"]`），Windows 上为当前用户临时目录中的 AF_UNIX 套接字；CLI 与 MCP 服务器从同一命令目录生成。

## 窗口

- **关闭协议**：标题栏关闭按钮（`AppWindow.Closing` 先取消，再走协议）、Ctrl+W、声明的动作与 `window.close` 都先求值关闭检查。需要确认时显示标准 `ContentDialog`，按钮为“存储 / 不存储 / 取消”或“放弃更改 / 取消”；待决时该窗口内容不可操作，外部入口返回 `target_blocked`。外部入口作出决定时对话框随之关闭；保存失败时窗口保留，并以 `InfoBar` 显示原因。
- **事实**：可见性、尺寸状态（普通、最大化、全屏）、位置与外框尺寸（以逻辑像素报告，按窗口所在显示器的 DPI 换算）、是否聚焦与可用动作，以带来源的事实报告给会话运行时。最小化的窗口被系统停放在屏幕外，此时报告的是它恢复后的位置与尺寸。
- **命令**：`window.minimize`、`restore`、`maximize`、`move`、`resize` 发出后等到 `AppWindow` 报告的实际状态满足请求才答复，5 秒内未满足返回 `ui_timeout`；尺寸小于声明的最小尺寸返回 `out_of_range`。Windows 支持读取与设置位置，`window.move` 可用。
- **启动方式**：窗口第一次显示时遵循启动方传入的显示方式（例如快捷方式设为“最小化”时以最小化启动）。

## 媒体

- 执行端为每个 `player_id` 维护一个 `MediaPlayer`（不自动播放，关闭系统媒体传输控件，使播放只经媒体命令），把打开、失败、播完、播放状态、时长、音量、静音、跳转完成与每 0.5 秒的位置作为观察结果报告给媒体中心，从不派发命令。播放器的事件在后台线程到达，转到 UI 线程处理；播放器释放后迟到的事件被忽略。构建清单记录媒体后端 `windows-mediaplayer` 与呈现 `mediaplayerelement`。
- 本地文件经 `StorageFile` 打开，网络地址直接交给 `MediaSource.CreateFromUri`；新源到达时较早的加载结果作废。
- `Video(source:)` 的播放器属于组件实例，绑定变化时按新源版本加载；`Video(player:)` 只附着呈现。控制条只发送标准媒体命令，状态来自媒体中心；拖动进度与音量只更新预览，松开时提交一次。控件登记为 `目标.play_pause`、`.position`、`.mute`、`.volume`。画面区按可用宽度保持 16:9。
- 失败原因按 `MediaPlayerError` 给出一句说明并附 HRESULT，例如损坏文件为 “The file is damaged or its format is not supported (0xC00D36C4)”，完整信息写入日志。

## 本地化与主题

- 文案表生成到程序中；启动时按 Windows 的用户语言列表（`GlobalizationPreferences.Languages`）与当前 UI 区域选择支持的语言（`zh-CN` 对应 `zh-Hans`，`zh-TW`、`zh-HK` 对应 `zh-Hant`），`ui.set_language` 与 `LanguagePicker` 在运行中切换，所有文本绑定在同一次更新中重新计算。运行时自己的文字内置 zh-Hans、zh-Hant、en、ja。
- 颜色全部来自 WinUI 主题资源：深浅色跟随系统，卡片、文字与分隔线使用 `CardBackgroundFillColorDefaultBrush` 等语义资源；声明了品牌色时设置 `SystemAccentColor` 及其三档浅色、三档深色，未声明时使用用户在系统中选择的强调色。
- 运行时自带的样式（卡片、透明按钮）以 XAML 资源字典在启动时加载，引用的都是 `ThemeResource`，主题变化时随之更新。

## 跨平台修正

为了在 Windows 上构建与运行，以下与平台无关的模块补上了 Windows 分支，macOS 与 Linux 行为不变：

- `bern-transport-ipc`：本地控制通道在 Windows 上使用 AF_UNIX 套接字（`uds_windows`），放在当前用户的临时目录中。
- `bern-transport-ws`：握手随机数在 Windows 上取自系统随机源（`getrandom`）。
- `bern-compiler`：`Bern.toml` 所在目录规范化后去掉 Windows 的 `\\?\` 前缀，生成的 Cargo 清单才能使用这些路径。
- `bern-command`：资源的文件 URL 在 Windows 上为 `file:///C:/…` 形式（盘符冒号不编码、反斜杠转为斜杠，`\\?\` 前缀去掉，共享路径为 `file://server/share/…`）。

## 构建与运行

```powershell
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml --target windows
target\bern\windows\Showcase\showcase.exe
```

需要 .NET SDK 10 与 Rust；Windows App SDK 的 NuGet 包在首次构建时还原。产物位于 `target/bern/windows/`：`Showcase/`（`showcase.exe`、`bern_app.dll`、Windows App SDK 运行库与 `resources/`，程序按自身位置查找资源）、`bin/showcase.exe` 与 `bin/showcase-mcp.exe`、胶水 crate `glue/`、.NET 项目 `app/`、构建清单 `build.json`（记录 Windows App SDK 与 .NET 版本、最低 Windows 版本、运行时标识、媒体后端与呈现、资源名称）。Rust 编译目录为 `target/bern-glue-windows/`。

开发时 `BERN_SNAPSHOT_DIR` 让程序在每次变化后把各窗口渲染为 `<目录>/<窗口>.png`，打开中的对话框与菜单另存为 `<窗口>-popup<n>.png`；快照不含 Mica 背景（按主题底色合成）和视频画面（视频经独立的合成表面呈现）。`BERN_APPEARANCE=light|dark` 覆盖系统外观。两者默认关闭。

## 验证记录

2026-10-11，Windows 11 25H2 实体机，经 SSH 构建，并以计划任务把应用启动到用户的桌面会话中，由生成的 CLI 驱动，截图来自开发快照：

- `cargo test --workspace` 在 macOS 与 Windows 各 171 项通过（Windows 上包含用 `dotnet build` 编译生成的应用）；`cargo clippy` 与 `cargo fmt --check` 通过。新增测试：Windows 默认解析为 `winui3-windows`；不支持的语言报错；生成的胶水 crate、.NET 项目、构建清单与构建计划；生成代码不含颜色与未实现的组件；C# 字符串、数值与值字面量；Windows 文件 URL。
- **控件与引擎**：`ui select` 切换侧边栏页面；`ui activate` 按钮后引擎计数增加，禁用按钮返回 `target_unavailable`；`ui set-bool`、`ui set-number`（音量 80）、`ui select` 选择器与工具栏中的同一字段同步变化；`ui edit` 中文与多行文字、`ui submit` 生效，预览显示“你好，小明”；密码框在 `ui.inspect` 中没有值。
- **查询**：页面打开时 `loading v1`，随后 `ready v1 (5)`；侧边栏搜索“照”得到照片、“music”得到音乐（`ready v2 (1)`、`ready v3 (1)`），“zzz”显示“没有与“zzz”匹配的项目”；清空搜索后 `add-item` 新增的行出现在列表末尾，原有行保持身份。
- **导航**：`ui activate --target "library_page.item_link[2]"` 推入“项目 2”，页头出现返回按钮；`ui back` 返回列表。
- **自适应与菜单**：默认宽度下 `media_page.clip_choice` 为 `compact`，挂载“换片”菜单；`window resize --width 1300` 后为 `expanded`，显示三个按钮；改回 920 后重新为 `compact`。
- **语言**：`ui set-language --language en` 后界面、侧边栏、按钮与窗口标题全部变为英文；`fr` 返回 `unsupported_language`。
- **关闭协议**：设置窗口修改显示名称后 `window close` 返回确认 `settings#1:c1`（决策“存储 / 不存储 / 取消”），对话框为标准 `ContentDialog`；待决时 `ui edit` 返回 `target_blocked`，控件的 `reason` 为 `blocked_by_modal`；名称为空格时 `Save` → `Rejected`（`save_failed: invalid_name: …`），窗口保留并以 `InfoBar` 显示原因；有效名称 `Save` → 关闭且引擎 `display_name` 为 “Alice”。
- **窗口命令**：`window minimize` 返回 `Minimized`，`restore` 回到 920 × 640；`maximize` 返回 `Maximized`；`move --x 200 --y 120` 与 `resize --width 1100 --height 720` 返回实际位置与尺寸（来源 `Command`）；`resize 100 × 100` 返回 `out_of_range`（至少 720 × 480）。
- **单实例与退出**：主窗口最小化时再次启动程序，第二个进程退出，原窗口恢复并获得焦点；`window close --window main#1` 后进程退出，日志无错误。
- **媒体**：详情页“在预览播放器中打开”后 `Ready 0:00 / 0:06`；`play_pause` 播放，`position` 跳到 1000 后回报 1000；`mute` 与 `volume 0.4` 生效；播完为 `Ended`，位置 6000；回到视频页后预览播放器保持 `Ended` 状态并重新附着。精选片段换为损坏文件后为 `Failed`，控制条禁用，状态区显示“无法播放”与原因。
- **外观**：浅色、深色（`BERN_APPEARANCE=dark`）下主窗口与设置窗口均为 Windows 11 标准样式；临时声明品牌色 `#B4235A` 后开关、复选框、滑块、选中指示全部使用该颜色。
- **MCP**：`showcase-mcp.exe` 完成 `initialize` 与 `tools/list`，`ui_inspect` 返回运行中界面的控件。

未完成的验证：视频画面与声音的人工确认（快照不含画面）；键盘与手动操作（Ctrl+W、Esc、Alt+←、输入法）；讲述人与完整的键盘遍历；高对比主题与文字缩放；多显示器与不同缩放比例；ARM64；网络与 HLS 直播源。

## 未完成与已知限制

- `flutter + macos` 属于阶段五后续批次。
- 未提供 MSIX 打包、安装程序、应用图标与代码签名；.NET 为框架依赖，目标机器需要 .NET 10 运行时。
- 页面工具放在页头，没有使用 `CommandBar`；`Menu` 的菜单项不呈现 `destructive` 角色。
- 一个播放器被多个视图同时显示时依赖 `MediaPlayerElement` 共享同一 `MediaPlayer`，尚未专门验证。
- 品牌色的浅色与深色档位按与白色、黑色混合近似生成，没有采用系统的强调色算法。
- 设置窗口切换页签只构建选中的页签。
- 其余与 macOS 相同的限制见 [macOS SwiftUI 实现记录](macOS-SwiftUI.md)“未完成与已知限制”中与平台无关的条目。
