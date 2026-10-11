# Windows WinUI 3 实现记录

- 日期：2026-10-11
- 依据：[Bern 架构设计 v0.17](../Design.md)、[Windows 与 Linux 适配](../Adaptation/WindowsLinux.md)
- 工具链：Rust 1.99.0；Visual Studio 2022 生成工具 17.14（MSVC 14.44，C++20）；Windows SDK 10.0.26100（含 `cppwinrt.exe`）；Windows App SDK 1.8 的组件包（只取头文件与元数据：Foundation 1.8.260803002、WinUI 1.8.260803003、InteractiveExperiences 1.8.260708001，以及 WebView2 1.0.3179.45 的元数据）
- 验证环境：Windows 11 专业版 25H2（版本 26200，x64 实体机），显示缩放 150%

本文记录阶段五第二批 `winui3 + windows` 目标的完成情况。第一版宿主是 C#/.NET，应用需要目标机器装有 .NET 10，并随附约 141 MB 的 Windows App SDK。现已改为 C++/WinRT：同一个可执行文件里有两套前端，启动时自动选择，只使用系统已有的组件。展示应用（`examples/showcase`）的声明、协议、业务核心与 IR 都没有改动；解析器、公共 IR 与核心检查规则也没有为此修改。

## 宿主方案

**界面用 C++/WinRT 编写**，不需要 .NET，C 运行库静态链接，引擎以静态库链入同一个可执行文件：

- 生成的胶水 crate 把引擎、会话运行时与 `bern-ffi` 的 C 接口编译为 `bern_app.lib`（`staticlib`，并以 `+crt-static` 静态链接 C 运行库）。C++ 运行时直接调用与 SwiftUI 目标相同的进程内接口；回调复制字节后经消息窗口转到 UI 线程，按到达顺序处理。
- 界面全部用代码构建，不生成 XAML 页面，也不经过 XAML 编译器。

**一个程序，两套前端，启动时自动选择：**

| 条件 | 前端 | 外观 |
| --- | --- | --- |
| 引导程序在系统中找到 Windows App Runtime 1.8 | WinUI 3（`Microsoft.UI.Xaml`），Mica 背景与 `TitleBar` | Windows 11 |
| 找不到运行时（Windows 10，或没有装 1.8 的 Windows 11） | 系统自带的 XAML（`Windows.UI.Xaml`），经 XAML Islands 承载在 Win32 窗口中 | Windows 10 |

- 选择由入口的 `MddBootstrapInitialize2(0x00010008, …, MddBootstrapInitializeOptions_None)` 决定：找到运行时就用 WinUI 3，找不到时不弹窗、不引导安装，直接回退。随程序只附带 400 KB 的 `Microsoft.WindowsAppRuntime.Bootstrap.dll`。
- 两套前端共用一份源码。XAML 部分写成 `.inl`，分别在 `bern::winui`（`xaml` 为 `Microsoft::UI::Xaml`）与 `bern::islands`（`xaml` 为 `Windows::UI::Xaml`）两个命名空间里各包含一次。类型差异用 `#if BERN_WINUI` 处理，资源键等取值差异用 `kWinUI ? … : …`。生成的界面代码 `app_ui.inl` 也同样包含两次。
- 窗口的事实、命令和关闭拦截在两种模式下都走 Win32：`GetWindowRect`、`IsIconic`、`IsZoomed`、`ShowWindow`、`WM_CLOSE`、`WM_GETMINMAXINFO`。WinUI 窗口用子类化的窗口过程接管；Islands 窗口本来就是 Bern 自己的 Win32 窗口。只有在 WinUI 下设置尺寸与位置时使用 `AppWindow`，它了解延伸到标题栏的外框。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target windows` 先构建 `bern_app.lib`、CLI 与 MCP 服务器，再运行生成的 `build.bat`：生成 C++/WinRT 头文件（按版本缓存），用 MSVC 编译运行时与生成代码，链接为一个可执行文件。生成测试在装有 VS C++ 工具的 Windows 上编译两套前端 |
| 新目标不改解析器、公共 IR 与业务核心 | 完成 | `bern-backend-winui3`：C++ 生成器（`cpp.rs`）、构建胶水（`glue.rs`）与 `templates/cpp/` 下的运行时 |
| 具体框架 SDK 不是全局前置条件 | 完成 | C++ 运行时以模板随目标模块分发，只在生成的应用中编译；macOS 上 `cargo test --workspace` 与 clippy 不需要 Windows 工具链 |
| 运行时只使用系统依赖 | 完成 | 不需要 .NET；不随附 Windows App SDK；C 运行库静态链接，不需要 VC++ 运行库。有系统的 Windows App Runtime 1.8 时用它，没有时用系统 XAML |
| 省略 UI 技术时的默认解析 | 完成 | `platform = "windows"` 解析为 `winui3-windows`（测试） |
| 状态订阅、命令与错误返回 | 完成 | 引擎状态按字段镜像为响应式单元；失败显示在窗口底部，WinUI 下是 `InfoBar`，Islands 下是同样位置的错误条 |
| 列表身份、订阅释放 | 完成 | `ForEach` 按键保留行；查询订阅随视图与窗口结束 |
| 深浅色 | 完成 | 颜色全部来自系统主题资源，经 `ThemeResource` 绑定，运行中切换主题也会更新；Islands 窗口的标题栏经 DWM 跟随深浅色 |
| 原生多窗口、关闭确认、窗口事实 | 完成 | 两种模式均已实测 |
| Video 原生呈现 | 完成 | `Windows.Media.Playback.MediaPlayer` 执行端，两种模式各自的 `MediaPlayerElement` 呈现 |
| 阶段四：`Adaptive`、`Menu`、`LanguagePicker`、运行时切换语言 | 完成 | 两种模式均已实测 |

## 工程结构

| 组成 | 职责 |
| --- | --- |
| `bern-backend-winui3` | 目标描述、检查，从 IR 生成 C++，生成胶水 crate、构建脚本与构建计划 |
| `templates/cpp/bern_base.h/.cpp` | 与 XAML 无关的部分：值与编解码、响应式单元与作用域、文案与语言、引擎宿主（消息窗口、定时器、单实例）、媒体执行端、开发快照的 PNG 编码 |
| `templates/cpp/bern_ui_*.inl` | XAML 部分，每种前端各包含一次：上下文、父容器与区域、控件登记、`FlexPanel`、样式与图标（`core`）；标准控件（`widgets`）；条件、列表、导航、页签、自适应、查询与视频（`structure`）；窗口、关闭协议、会话命令、快照与应用生命周期（`windows`） |
| `bern_winui.cpp`、`bern_islands.cpp`、`bern_main.cpp` | 两个前端的编译单元与入口（引导、选择前端、单实例） |
| 生成的胶水 crate | `target/bern/windows/glue/`：`bern_app.lib`、`cli.rs`、`mcp.rs`，以及静态链接 C 运行库的 `.cargo/config.toml` |
| 生成的应用源码 | `target/bern/windows/app/`：`app_ui.inl`（组件与窗口，两套前端共用）、`app_catalog.cpp`（文案表、类型表、命令与会话，只编译一次）、`app_id.h`、`app.manifest`（每显示器 DPI 感知 v2，声明测试过的 Windows 版本，Islands 需要）、`app.rc`（版本信息）、`build.bat`、`runtime/` |

生成代码调用运行时的标准组件（`B::Text`、`B::SplitView`……），形式与 GTK、Web 目标相同：每个组件一个函数，表达式是在上下文 `c` 中求值的 C++ 表达式，局部变量放在上下文里，所有 lambda 都不捕获任何东西。属性用 C++20 指定初始化器，未给出的属性直接省略。

## 控件映射

两种前端的控件相同，样式各自取系统的主题资源，差别列在最后一列。

| Bern | WinUI 3 | 系统 XAML（Islands） |
| --- | --- | --- |
| `Window` | `Window`，Mica 背景，内容延伸到标题栏并使用 `TitleBar`；底部 `InfoBar` 显示失败 | Win32 窗口加 `DesktopWindowXamlSource`，系统标题栏（经 DWM 跟随深浅色），页面底色；底部为错误条 |
| `SplitView` / `Sidebar` / `Detail` | `NavigationView`：分节标题、窗格中的 `AutoSuggestBox`；详情页头显示所选目的地、工具与返回。宽度不足 800 时窗格收为图标栏 | 同左，Windows 10 样式 |
| `Navigation` / `NavLink` | 根内容与顶层目的地叠放，推入时根内容保持挂载；页头出现返回按钮，Esc、Alt+← 返回 | 同左 |
| `Tabs` / `Tab` | 页头中带图标的 `SelectorBar` | 页头中的一组 `ToggleButton`（系统 XAML 没有 `SelectorBar`） |
| `Picker` | `segmented` 为 `SelectorBar`，其他为 `ComboBox` | `segmented` 为 `ToggleButton` 组，其他为 `ComboBox` |
| `Form` / `Section` | 滚动页面，卡片行（`CardBackgroundFillColorDefaultBrush` 与描边、圆角） | 卡片为 Windows 10 的列表底色，无圆角 |
| `Text` / `Label` | WinUI 字体样式（`TitleLargeTextBlockStyle` 等）；次要文字为 0.65 不透明度 | 系统字体样式（`HeaderTextBlockStyle` 等） |
| `Button` / `IconButton` | `prominent` 为 `AccentButtonStyle`；`plain` 与图标按钮以轻量样式资源设为透明；`destructive` 为危险色 | 同左，取 Windows 10 的对应资源 |
| `TextInput` / `Search` / `Switch` / `Slider` / `Menu` / `LanguagePicker` / `Query` / `Adaptive` / `Video` | 与 C# 版相同：`TextBox`/`PasswordBox`、`AutoSuggestBox`、`ToggleSwitch`/`CheckBox`、带图标的 `Slider`、`DropDownButton` 与 `MenuFlyout`、`ComboBox`、`ProgressRing` 与状态行、按可用宽度选择布局、`MediaPlayerElement` 与标准控制条 | 同左 |

图标沿用 Bern 的 SF Symbols 命名，映射到 Segoe Fluent Icons（Windows 11）与 Segoe MDL2 Assets（Windows 10）中码位相同的字形。

## 运行结构

- **区域与生命周期**：条件、列表这类会重建的内容各占父面板中的一个区域，插入位置由区域树算出。面板经自己的 `Resources` 持有根区域，因此构建函数返回后，区域树依然随面板存在。事件处理函数通过弱引用访问自己所属的元素，不形成循环引用。
- **响应式**：单元、效果与作用域和其他目标相同；效果在运行中创建的子作用域在下一次运行前结束；批量更新后每个受影响的效果只运行一次。
- **宿主**：UI 线程上有一个只接收消息的窗口，负责投递任务（`WM_APP`）和定时器（`WM_TIMER`），在两种前端下都可用。它还兼作单实例标识：互斥量 `Local\Bern.<bundle_id>`。再次启动时，新进程找到这个窗口、交出前台权限并请求显示主窗口，然后自己退出。
- **应用生命周期**：最后一个窗口关闭后应用退出，引擎随之停止（WinUI 调用 `Application.Exit`，Islands 结束消息循环）。Ctrl+W 关闭当前窗口；设置窗口按 Esc 关闭，不可最小化与最大化。
- **外部入口**：与其他目标相同的本地控制通道（`channels = ["ipc"]`）；CLI 与 MCP 服务器从同一命令目录生成，CRT 同样静态链接。

## 窗口

- **关闭协议**：标题栏关闭、Alt+F4（拦截 `WM_CLOSE`）、Ctrl+W、声明的动作与 `window.close` 都先求值关闭检查。需要确认时显示标准 `ContentDialog`，样式随前端而定。待决期间窗口内容不可操作，外部入口返回 `target_blocked`；外部作出决定时，对话框随之关闭。
- **事实与命令**：可见性、尺寸状态、位置与外框尺寸（按窗口所在显示器的 DPI 换算为逻辑像素）、焦点与可用动作。`minimize`、`restore`、`maximize`、`move`、`resize` 要等实际状态满足请求才答复，5 秒未满足返回 `ui_timeout`。最小化的窗口报告它恢复后的位置与尺寸。

## 媒体

与 C# 版行为相同：每个 `player_id` 对应一个 `MediaPlayer`，把观察结果报告给媒体中心，从不自行派发命令。本地文件经 `PathCreateFromUrlW` 与 `StorageFile` 打开，网络地址交给 `MediaSource.CreateFromUri`；新源到达时，较早的加载结果作废。`MediaPlayerElement` 在两种前端下都是系统控件，同一个 `MediaPlayer` 可以交给任一前端呈现。

## 构建与运行

```powershell
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml --target windows
target\bern\windows\Showcase\showcase.exe
```

构建需要 Rust、Visual Studio 的“使用 C++ 的桌面开发”（MSVC 与 Windows SDK），不需要 .NET：

- **构建脚本**：用 `vswhere` 找到 Visual Studio，加载 `vcvarsall`。
- **Windows App SDK 组件包**：先在 NuGet 缓存中找，找不到时用 Windows 自带的 `curl` 与 `tar` 从 nuget.org 取到 `target/bern-winrt/packages/`。
- **C++/WinRT 头文件**：用 Windows SDK 的 `cppwinrt.exe` 生成，按 SDK 与 WinUI 版本缓存在 `target/bern-winrt/`（约 110 MB，只生成一次）。
- **增量构建**：头文件缓存好以后，一次调试构建约 55 秒，release 约 90 秒（含 Rust 部分）。

产物位于 `target/bern/windows/`：`Showcase/`（`showcase.exe`、`Microsoft.WindowsAppRuntime.Bootstrap.dll` 与 `resources/`）、`bin/showcase.exe` 与 `bin/showcase-mcp.exe`，以及构建清单 `build.json`（记录前端、组件包版本与最低 Windows 版本）。

应用图标 `app.ico`（没有声明 `[app].icon` 时为 Bern 标志的圆形版，16 到 256 像素各一张 PNG）编入可执行文件的图标资源 1：资源管理器显示它。运行时按窗口所在显示器的 DPI 把它设为每个窗口的大小图标（任务栏、Alt+Tab、Islands 的系统标题栏），WinUI 3 的 `TitleBar` 另以 `ImageIconSource` 显示其中最接近标题栏尺寸的一张。

**开发辅助**（默认全部关闭）：

- `BERN_SNAPSHOT_DIR`：每次变化后输出两种快照。`<窗口>.png` 是 XAML 内容的渲染，打开的对话框和菜单另存为 `-popup<n>.png`；`<窗口>-window.png` 用 `PrintWindow` 截取应用自己的窗口，含标题栏、Mica 与视频画面。
- `BERN_APPEARANCE=light|dark`：覆盖系统外观。
- `BERN_FORCE_ISLANDS`：在装有运行时的机器上试用回退前端。
- `BERN_DUMP_TREE`：把元素树和尺寸写到标准错误。
- 调试构建在未处理的异常时，按 PDB 把调用栈（函数与行号）写到标准错误。PDB 留在 `app/obj/`，不进入应用目录。

## 实测数据

2026-10-11，同一台机器，release 构建。启动时间为从启动到窗口可见，首次启动另计；内存为窗口显示 5 秒后的私有工作集。

| | WinUI 3（系统 Runtime 1.8） | 系统 XAML（Islands） | 第一版 C#/.NET |
| --- | --- | --- | --- |
| 启动 | 0.32 秒（首次 0.51 秒） | 0.54 秒 | 0.62 秒 |
| 内存 | 35 MB | 19 MB | 42 MB |
| 应用目录 | 4.2 MB（两种前端共用） | 同左 | 141 MB，另需 .NET 10 |

4.2 MB 是 `showcase.exe`（3.8 MB，内含引擎）加上引导 DLL（0.4 MB），不含展示用的媒体资源。

## 验证记录

2026-10-11，Windows 11 25H2 实体机，经 SSH 构建，以计划任务把应用启动到用户的桌面会话，由生成的 CLI 驱动；截图来自开发快照中的窗口截取。两种前端分别执行：

- **构建与测试**：`cargo test --workspace` 在 macOS 与 Windows 上各 172 项通过；Windows 上包含用 MSVC 编译生成的两套前端。`cargo clippy` 与 `cargo fmt --check` 通过。
- **前端选择**：默认启动加载 `C:\Program Files\WindowsApps\Microsoft.WindowsAppRuntime.1.8_…\Microsoft.UI.Xaml.dll`；设置 `BERN_FORCE_ISLANDS` 后加载 `C:\Windows\System32\Windows.UI.Xaml.dll`，进程中没有 Windows App Runtime。
- **控件与引擎**：`ui select` 在各页之间切换，按钮页、开关页、输入页、列表页、视频页的控件全部登记。
- **控件操作**：`ui activate` 后引擎计数增加，禁用按钮报告 `disabled`；`ui set-bool`、`ui set-number`、`ui select` 生效，工具栏中绑定同一字段的选择器同步变化；`ui edit` 中英文混合文字与 `ui submit` 生效。
- **查询与导航**：查询从 `loading v1` 变为 `ready v1 (5)`，每行生成一个链接；`ui activate --target "library_page.item_link[2]"` 推入详情页，页头出现返回按钮，`ui back` 返回列表。
- **媒体**：“在预览播放器中打开”后播放，窗口截图中可见视频画面，进度与时间在走。精选片段为 `Ready`；`Adaptive` 随内容区宽度在 `compact` 与 `expanded` 之间切换。
- **语言**：`ui set-language --language en` 后两个窗口全部变为英文。
- **关闭协议**：设置窗口修改显示名称后，`window close` 返回确认。WinUI 下是 Windows 11 样式的 `ContentDialog`，Islands 下是 Windows 10 样式，待决时控件为 `blocked_by_modal`。`Cancel` 返回 `Rejected`（`cancelled`）；再次关闭后 `Discard` 关闭窗口；`Save` 关闭窗口，引擎中的 `display_name` 更新。
- **窗口命令**：`minimize`、`restore`、`maximize`、`restore`、`move`、`resize` 返回实际状态（来源 `Command`）。设置窗口没有最小化与最大化动作。
- **单实例与退出**：主窗口最小化时再次启动，第二个进程退出，原窗口恢复并获得焦点；关闭主窗口后进程退出。
- **外观**：深色下两种前端的内容、卡片与标题栏都随主题变化。
- **MCP**：`showcase-mcp.exe` 完成 `initialize`、`tools/list`，`ui_inspect` 返回运行中界面的控件。

改写过程中发现并修正的问题：

- **区域根被提前释放**：根区域原本只由构建时的临时对象持有，切换页面时会访问已释放的内存（Islands 下表现为崩溃）。已改为由面板持有。
- **深色下内容区白底**：画刷原先按应用主题静态查找，元素主题与应用主题不同时就会出错。已改为 `ThemeResource` 样式。
- **Defender 误报**：构建脚本模板里原本内嵌了 PowerShell 下载命令，被编进 `bern.exe` 后，Defender 以 `Trojan:Win32/Bearfoos.B!ml` 误报并隔离了它。已改用 Windows 自带的 `curl` 与 `tar`，之后没有再被拦截。

未完成的验证：

- 没有 Windows App Runtime 的干净 Windows 10 与 Windows 11。本机的 Islands 结果来自强制回退。
- 只装有系统组件版运行时（`Microsoft.WindowsAppRuntime.CBS`）时，非打包应用能否经引导程序使用它。
- 键盘与手动操作（Ctrl+W、Esc、Alt+←、输入法）、讲述人与完整的键盘遍历、高对比主题、多显示器、ARM64、网络与 HLS 源。

## 未完成与已知限制

- **WinUI 3 只认 Windows App Runtime 1.8。** 引导程序按主次版本查找：只装有 2.x（例如系统应用依赖的 2.5）或其他 1.x 的机器会回退到系统 XAML。是否接受 2.x，要等确认 2.x 与按 1.8 元数据编译的代码二进制兼容之后再定。
- 未提供 MSIX 打包、安装程序、应用图标与代码签名。未签名的新程序可能被安全软件的启发式规则误判，正式分发需要签名。
- 回退前端没有 `SelectorBar`、`InfoBar` 与 `TitleBar`，分别以按钮组、自绘错误条与系统标题栏代替。
- 页面工具放在页头，没有使用 `CommandBar`；`Menu` 的菜单项不呈现 `destructive` 角色。
- 品牌色的浅色与深色档位按与白色、黑色混合近似生成。
- 其余与 macOS 相同的限制见 [macOS SwiftUI 实现记录](macOS-SwiftUI.md)“未完成与已知限制”中与平台无关的条目。
