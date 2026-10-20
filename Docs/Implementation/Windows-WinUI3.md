# Windows WinUI 3 实现记录

当前组件接口见[组件参考](../Reference/UI.md)，逐项支持与近期验证范围见[平台能力表](../Reference/Platforms.md)。下文保留各阶段实现和验收记录。

- 日期：2026-10-11（v0.29、v0.30 的能力于 2026-10-14 补齐，见“样式、动效、自绘与媒体类应用的能力”；页面级布局、场景与控件形态的能力于 2026-10-19 补齐，见同名一节）
- 依据：[Bern 架构设计 v0.30](../Design.md)（最初按 v0.19 实现）、[Windows 与 Linux 适配](../Adaptation/WindowsLinux.md)
- 工具链：Rust 1.99.0；Visual Studio 2022 生成工具 17.14（MSVC 14.44，C++20）；Windows SDK 10.0.26100（含 `cppwinrt.exe`）；Windows App SDK 2.5.1 的组件包（只取头文件与元数据：Foundation 2.3.12、WinUI 2.3.9、InteractiveExperiences 2.1.9，以及 WebView2 1.0.3719.77 的元数据）
- 验证环境：Windows 11 专业版 25H2（版本 26200，x64 实体机），显示缩放 150%

本文记录阶段五第二批 `winui3 + windows` 目标的完成情况。第一版宿主是 C#/.NET，应用需要目标机器装有 .NET 10，并随附约 141 MB 的 Windows App SDK。现已改为 C++/WinRT：同一个可执行文件里有两套前端，启动时自动选择，只使用系统已有的组件。展示应用（`examples/showcase`）的声明、协议、业务核心与 IR 都没有改动；解析器、公共 IR 与核心检查规则也没有为此修改。

## 宿主方案

**界面用 C++/WinRT 编写**，不需要 .NET，C 运行库静态链接，引擎以静态库链入同一个可执行文件：

- 生成的胶水 crate 把引擎、会话运行时与 `bern-ffi` 的 C 接口编译为 `bern_app.lib`（`staticlib`，并以 `+crt-static` 静态链接 C 运行库）。C++ 运行时直接调用与 SwiftUI 目标相同的进程内接口；回调复制字节后经消息窗口转到 UI 线程，按到达顺序处理。
- 界面全部用代码构建，不生成 XAML 页面，也不经过 XAML 编译器。

**两套前端，默认都编译，启动时自动选择：**

`[targets.windows]` 的 `frontends` 默认是 `["winui3", "xaml-islands"]`，也可以只写其中一个：

```toml
[targets.windows]
platform = "windows"
frontends = ["winui3"]   # 或 ["xaml-islands"]；不写则两个都编译
```

| 条件 | 两个都编译 | 只有 WinUI 3 | 只有系统 XAML |
| --- | --- | --- | --- |
| 系统有 Windows App Runtime 2（任何 2.x：其他程序装的，或 Windows 11 自带的） | WinUI 3（`Microsoft.UI.Xaml`），Mica 背景与 `TitleBar`，Windows 11 外观 | 同左 | 系统 XAML |
| 没有（Windows 10，或没有 2.x 的 Windows 11） | 系统自带的 XAML（`Windows.UI.Xaml`），经 XAML Islands 承载在 Win32 窗口中，Windows 10 外观 | 提示框，可在浏览器中打开安装程序 | 同左 |

- **运行时按顺序找。** 先用引导程序 `MddBootstrapInitialize2(0x00020000, L"", 2.0.0.0, None)` 找其他程序装的任何 2.x：2.0 起框架包只按主版本区分（`Microsoft.WindowsAppRuntime.2`），2.x 之间兼容。应用按最新的 2.5.1 编译，只用每个 2.x 都有的接口；展示应用在 2.2 上逐页实测通过。找不到时，再经系统的动态依赖接口（`TryCreatePackageDependency`、`AddPackageDependency`，Windows 11 才有）加载 Windows 自带的 `Microsoft.WindowsAppRuntime.CBS.2`。两者都不弹窗，也不引导安装。
- **Windows 自带的运行时。** Windows 11 25H2 带有 `CBS.2`（2.2）、`CBS.1.8`、`CBS.1.6` 与 `CBS`（1.5），都在 `C:\Windows\SystemApps`，给系统自己的组件用。实测普通程序能挂上 `CBS.2` 与 `CBS.1.6`，挂不上 `CBS.1.8` 与 `CBS`。`CBS.2` 比商店版少了各语言的 `.mui`，而且 WinUI 不会自己找到它的 `resources.pri`：直接使用时报 `Cannot locate resource from 'ms-appx:///Microsoft.UI.Xaml/Themes/themeresources.xaml'` 并退出。所以用它时，应用在 `Application.ResourceManagerRequested` 中用这份运行时自己的 `resources.pri` 创建资源管理器交给 WinUI。
- **只编译 WinUI 3。** 找不到运行时时，按界面语言弹出提示（标题为应用名），选“是”用默认浏览器打开最新运行时的安装程序 `https://aka.ms/windowsappsdk/2.5/latest/windowsappruntimeinstall-<架构>.exe`（微软的固定地址，跳转到下载服务器），然后退出；装好后再次打开即可。
- **只编译系统 XAML。** 不找运行时，不链接也不随附引导程序；预编译头只含一个前端，首次构建约 39 秒。
- 编译了 WinUI 3 时，随程序附带 400 KB 的 `Microsoft.WindowsAppRuntime.Bootstrap.dll`。
- 两套前端共用一份源码。XAML 部分写成 `.inl`，分别在 `bern::winui`（`xaml` 为 `Microsoft::UI::Xaml`）与 `bern::islands`（`xaml` 为 `Windows::UI::Xaml`）两个命名空间里各包含一次。类型差异用 `#if BERN_WINUI` 处理，资源键等取值差异用 `kWinUI ? … : …`。生成的界面代码 `app_ui.inl` 也同样在 `bern_winui.cpp` 与 `bern_islands.cpp` 中各包含一次。编译哪套前端由 `BERN_WITH_WINUI` 与 `BERN_WITH_ISLANDS` 两个宏决定。
- 窗口的事实、命令和关闭拦截在两种模式下都走 Win32：`GetWindowRect`、`IsIconic`、`IsZoomed`、`ShowWindow`、`WM_CLOSE`、`WM_GETMINMAXINFO`。WinUI 窗口用子类化的窗口过程接管；Islands 窗口本来就是 Bern 自己的 Win32 窗口。只有在 WinUI 下设置尺寸与位置时使用 `AppWindow`，它了解延伸到标题栏的外框。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target windows` 先构建 `bern_app.lib`、CLI 与 MCP 服务器，再运行生成的 `build.bat`：生成 C++/WinRT 头文件（按版本缓存），用 MSVC 编译运行时与生成代码，链接为一个可执行文件。生成测试在装有 VS C++ 工具的 Windows 上编译两套前端 |
| 新目标不改解析器、公共 IR 与业务核心 | 完成 | `bern-backend-winui3`：C++ 生成器（`cpp/`）、构建胶水（`glue.rs`）与 `templates/cpp/` 下的运行时 |
| 具体框架 SDK 不是全局前置条件 | 完成 | C++ 运行时以模板随目标模块分发，只在生成的应用中编译；macOS 上 `cargo test --workspace` 与 clippy 不需要 Windows 工具链 |
| 运行时只使用系统依赖 | 完成 | 不需要 .NET；不随附 Windows App SDK；C 运行库静态链接，不需要 VC++ 运行库。有任何 Windows App Runtime 2（其他程序装的，或 Windows 11 自带的）时用它，没有时用系统 XAML |
| 省略 UI 技术时的默认解析 | 完成 | `platform = "windows"` 解析为 `winui3-windows`（测试） |
| 状态订阅、命令与错误返回 | 完成 | 引擎状态按字段镜像为响应式单元；失败显示在窗口底部，WinUI 下是 `InfoBar`，Islands 下是同样位置的错误条 |
| 列表身份、订阅释放 | 完成 | `ForEach` 按键保留行；查询订阅随视图与窗口结束 |
| 深浅色 | 完成 | 颜色全部来自系统主题资源，经 `ThemeResource` 绑定，运行中切换主题也会更新；Islands 窗口的标题栏经 DWM 跟随深浅色 |
| 原生多窗口、关闭确认、窗口事实 | 完成 | 两种模式均已实测 |
| Video 原生呈现 | 完成 | Media Foundation 媒体引擎执行端，两种模式以 `SwapChainPanel` 显示引擎的交换链；HDR 视频在 SDR 显示器上经系统色调映射，HDR 显示器未实测 |
| 阶段四：`Adaptive`、`Menu`、`LanguagePicker`、运行时切换语言 | 完成 | 两种模式均已实测 |
| 样式覆盖（`style!`）：`Button`、`IconButton`、`SidebarItem` | 完成 | 颜色与边框写成控件自己的轻量样式资源，状态由控件模板的视觉状态切换；圆角、内边距、尺寸档直接设置；阴影由 Direct2D 画成位图垫在按钮下；缩放、位移与透明度用运行时的弹簧。选中侧栏项的 `indicator` 不列出。见“样式覆盖” |
| 动效：`appear`、`disappear`、`reflow`、`hero`、`animate`、`stagger`、`nudge`、`scroll_scale`、`scroll_opacity` | 完成 | 弹簧每帧在界面线程上推进，写到元素的 `Translation`、`Scale` 与 `Opacity`，不影响布局；滚动联动是 Composition 表达式动画。见“动效” |
| 自绘：`Canvas`、`Shape`、`Series`、`Pattern`，`Text(mark:)`，`fill_from` 与 `fade` | 完成 | Direct2D 画进 WIC 位图后交给 `Image`，两种前端同一套代码；颜色按元素所在部分的主题解析 |
| 窗口：`chrome`、`immersive`、`idle_after` 与 `window.idle`，`role: dialog`，`window.compact` 等尺寸档 | 完成 | 对话框窗口是主窗口上的 `ContentDialog`；标题栏显示窗口标题（`chrome: unified` 不改变它），Islands 下保留系统标题栏。见“窗口外框、静置与对话框” |
| 可增减的侧栏项与页签 | 完成 | 侧栏项是 `NavigationView.MenuItems` 中的项，随 `if`、`match`、`ForEach` 增减；选中项离开时选中移到 `fallback` 或剩下的第一项 |
| v0.29 的 `features` 与媒体类组件 | 完成 | `SidebarFooter`（`PaneFooter`）、`Inspector`（右侧的 `SplitView` 窗格）、`Toast`、`SplitButton`（`ToggleSplitButton`）、`PopoverButton`（`Flyout`）、`Drop`、`Pinch`、`Swipe`、`Rail`；工具栏搜索与结果、会话动作、开关按钮、绑定长度、`appearance`、快捷键（`KeyboardAccelerator`）、运行时强调色与明暗、运行时图片、取色填充、`pick`（`FileOpenPicker`/`FolderPicker`）、滚动停靠与位置；会话命令 `ui.focus`、`ui.set_file`。描述中列出 `features` 的全部 18 项 |

## 工程结构

| 组成 | 职责 |
| --- | --- |
| `bern-backend-winui3` | 目标描述、检查，从 IR 生成 C++，生成胶水 crate、构建脚本与构建计划；样式支持表（`style.rs`）与图标字形表（`icons.rs`） |
| `templates/cpp/bern_base.h/.cpp` | 与 XAML 无关的部分：值与编解码、响应式单元与作用域、文案与语言、引擎宿主（消息窗口、定时器、单实例）、媒体执行端、开发快照的 PNG 编码 |
| `templates/cpp/bern_ui_*.inl` | XAML 部分，每种前端各包含一次，共七部分：上下文、父容器与区域、控件登记、`FlexPanel`、样式与图标（`core`）；弹簧与变换层、主题颜色与运行时强调色、样式覆盖、共享元素、滚动联动与轨道（`motion`）；绘图（`drawing`）；标准控件（`widgets`）；条件、列表、导航、页签、自适应、查询与视频（`structure`）；滚动停靠、浮层按钮、分体按钮、轻提示、详情面板、拖放、手势、打开面板与快捷键（`features`）；窗口、关闭协议、会话命令、快照与应用生命周期（`windows`） |
| `templates/cpp/bern_pch.h/.cpp` | 预编译头：两个前端的 C++/WinRT 头文件，以及运行时在 `bern::winui` 与 `bern::islands` 两个命名空间中各一份 |
| `bern_winui.cpp`、`bern_islands.cpp`、`bern_main.cpp` | 两个前端的编译单元，各自只把生成的 `app_ui.inl` 放进自己的命名空间；入口（引导、选择前端、单实例） |
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
| `Picker` | `segmented` 为 `SelectorBar`；`chips` 为一组 `ToggleButton`（选中项为强调色），在 `Flow` 中且不显示标签时每个按钮都是 Flow 的子项，否则放进自己的换行面板；其他为 `ComboBox` | `segmented` 与 `chips` 都是 `ToggleButton` 组，其他为 `ComboBox` |
| `Form` / `Section` | 滚动页面，卡片行（`CardBackgroundFillColorDefaultBrush` 与描边、圆角） | 卡片为 Windows 10 的列表底色，无圆角 |
| `Text` / `Label` | WinUI 字体样式（`TitleLargeTextBlockStyle` 等）；次要文字为 0.65 不透明度 | 系统字体样式（`HeaderTextBlockStyle` 等） |
| `Button` / `IconButton` | `prominent` 为 `AccentButtonStyle`；`plain` 与图标按钮以轻量样式资源设为透明；`destructive` 为危险色 | 同左，取 Windows 10 的对应资源 |
| `TextInput` / `Search` / `Switch` / `Slider` / `Menu` / `LanguagePicker` / `Query` / `Adaptive` | 与 C# 版相同：`TextBox`/`PasswordBox`、`AutoSuggestBox`、`ToggleSwitch`/`CheckBox`、带图标的 `Slider`、`DropDownButton` 与 `MenuFlyout`、`ComboBox`、`ProgressRing` 与状态行、按可用宽度选择布局 | 同左 |
| `Video` | `SwapChainPanel` 显示媒体引擎的交换链（见“媒体”），下方为标准控制条 | 同左 |

组件库第一批在两种前端上用相同的代码：

| Bern | 两种前端 |
| --- | --- |
| `Image` | `Image` 显示 `BitmapImage`：从可执行文件旁的 `resources` 读出资源字节，写入内存流后回到界面线程 `SetSourceAsync`（未打包的应用没有 `ms-appx`）。两边都写时外面是声明尺寸的 `Grid`，`contain` 为 `Uniform` 并居中，`cover` 为 `UniformToFill`，`fill` 为 `Fill`；只写一边时为 `Uniform`，另一边按图片比例。`label` 为自动化名称，没有时 `AccessibilityView` 为 `Raw` |
| `Progress` | 有 `value` 时为标签下的 `ProgressBar`（`Minimum` 0，`Maximum` 为 `total`）；没有时为 20 px 的 `ProgressRing` 加次要文字 |
| `Grid` | 自定义面板 `GridPanel`：列数为放得下的最大值，列宽平分，每行与该行最高的单元格一样高 |
| `Stack` | 单格 `Grid`，子元素叠放，按 `align` 设对齐 |

HDR 图片（PQ 或 HLG 的 PNG、带增益图的 JPEG）不经过 XAML 的 `Image`：WinUI 3 的 `Image` 没有 HDR（[microsoft-ui-xaml#67](https://github.com/microsoft/microsoft-ui-xaml/issues/67)、[WindowsAppSDK#6291](https://github.com/microsoft/WindowsAppSDK/issues/6291)），系统 XAML 的文档也只说它能解码。Bern 用 `bern_picture_kind` 认出 HDR 图片后，在后台线程以 `bern_picture_decode`（`bern-image`，经 `bern-ffi` 的 `picture` 功能）解码成线性 scRGB，在界面线程上传为 Direct2D 的 32 位浮点位图，画进 `SwapChainPanel` 里 16 位浮点、scRGB 色彩空间的交换链；系统合成器直接合成它，两种前端用同一套代码（两套 SDK 头文件中的 `ISwapChainPanelNative` 同名不同 ID，预编译头里另行声明）。光的处理按窗口所在的显示器：DXGI 给出是否在 HDR 模式与峰值亮度，显示配置给出用户设的 SDR 亮度；Direct2D 的白电平调整把图片的 1.0 放到 SDR 亮度，HDR 色调映射把高光压到显示器峰值，SDR 模式下压到 SDR 白。带增益图的 JPEG 按显示器余量解码，SDR 显示器上就是作者给的 SDR 图。只要有 HDR 图片在屏幕上，每秒检查一次显示器，HDR 开关、SDR 亮度变化或窗口移到另一块显示器时重画，增益图按新的余量重新解码；设备丢失时重建设备。没有 Direct3D 或解码失败时退回 XAML 的 `Image`。实测（PC 为 SDR 显示器，两种前端相同）：增益图显示为作者给的 SDR 图（编码值 166 与 241，ImageIO 读基础图为 166 与 237）；PQ 图左右分别为 194 与 244，高光没有截断。

`Image` 与 `Stack` 外面各有一层 `Grid`，所以在会拉伸子元素的列中保持自身尺寸并靠前缘。在展示应用的“图片与布局”页按两种前端验证：效果与 macOS 相同，`contain` 的图片在框内居中；网格三列；CLI 修改音量与亮度后两个进度条立即更新。

图标沿用 Bern 的 SF Symbols 命名，映射到 Segoe Fluent Icons（Windows 11）与 Segoe MDL2 Assets（Windows 10）中码位相同的字形。字形表只有一份（`icons.rs`，码位从 PC 上的字体文件渲染比对确认），生成的目录把它交给运行时；表中没有的名称检查时报错，不在运行时显示空白；这项检查与 Web、Compose、GTK 共用 `bern_target_api::icon_set::check`，测试核对字形表包含 Bern 图标集（`icon_set::NAMES`）的全部名字。

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

执行端把观察结果报告给媒体中心，从不自行派发命令。每个 `player_id` 对应一个播放器，每加载一个源新建一个 Media Foundation 媒体引擎（`IMFMediaEngine`），换源后旧引擎迟到的事件因此能认出并丢弃；音量与静音记在播放器上，换源后沿用。本地文件经 `PathCreateFromUrlW` 转成路径交给引擎，网络地址原样交给引擎。引擎的事件从它自己的线程转到界面线程后报告，阶段按 HTML 媒体元素的语义（就绪状态、`paused`、`ended`、是否在等数据）得出。

画面不用 `MediaPlayerElement`：WinUI 3 的 `MediaPlayerElement` 不显示 HDR（[microsoft-ui-xaml#9238](https://github.com/microsoft/microsoft-ui-xaml/issues/9238)，微软工程师的解释是 UWP 靠 CoreWindow 判断显示器，Windows App SDK 应用没有 CoreWindow）。引擎用一个带视频支持的 Direct3D 11 设备解码，拿到元数据后进入无窗口交换链模式（`EnableWindowlessSwapchainMode`），把画面画进它自己的交换链；`Video` 用 `SwapChainPanel` 的 `ISwapChainPanelNative2::SetSwapChainHandle` 显示这个交换链，两种前端同一套代码（与 HDR 图片一样，两套头文件里的接口同名不同 ID，预编译头里另行声明）。这是微软 Media Foundation 示例与 Chromium 播放受保护视频的做法，HDR 视频怎样输出由引擎与系统决定。引擎按视频在屏幕上占的像素绘制（`UpdateVideoStream`）；`SwapChainPanel` 把交换链的一个像素当作一个 DIP，所以面板的 DIP 尺寸设为像素数、再按显示缩放缩回，交换链的像素与屏幕像素一一对应。`fit` 由面板的位置、缩放与外框的裁剪实现：`contain` 居中留边，`cover` 裁掉超出外框的部分，`fill` 拉伸。

一个引擎只有一个交换链，所以一个播放器同一时间只在一个视图中显示画面（`multi_view: false`）：最新附着的视图显示，它解除附着后交还给上一个仍在的视图，与 Web 的单视图一致。

实测（2026-10-12，PC 为 2560×1440、150% 缩放的 SDR 显示器，由 CLI 读回状态；以下在 WinUI 3 前端全部测过，Islands 前端测了 HDR10 测试片与预览播放器在页面间转移，结果相同）：精选片段加载后 `Ready 0 / 8000`，播放、暂停、播完为 `Ended` 后再播放从头开始、跳转到 5000、静音与音量 0.4 都生效；换为色条后静音与音量保留，换为损坏文件后 `Failed`（“The file is damaged or its format is not supported (0xC00D36C4)”）。4:3 的测试片放进 16:9 的外框，三种 `fit` 都正确，最大化与改变窗口大小后重新排布；视频尺寸正确说明 150% 下的缩放校正生效。应用级预览播放器在项目详情页播放，回到视频页后画面出现在视频页的视图中。本机 HTTP 提供的 HLS 直播打开后为 `Ready`，播放后 `Playing`，`duration_ms` 为空，`media seek` 返回 `not_seekable`，画面正常。用 ffmpeg 编成 10 位 HEVC（BT.2020、PQ、HDR10 元数据）的测试片，左半为 SDR 白（203 尼特）、右半为它的 4 倍，在 SDR 模式下由系统做色调映射：左右分别为 237 与 252，渐变平滑，没有发灰。HDR 模式下的输出未验证：测试用的 PC 没有 HDR 显示器。开发快照的窗口截取（`PrintWindow`）里有视频画面。

## 样式、动效、自绘与媒体类应用的能力（v0.29、v0.30）

2026-10-14 补齐：Windows 的能力描述现在列出样式（`Button`、`IconButton`、`SidebarItem`）、动效（`appear`、`disappear`、`reflow`、`hero`、`animate`、`scroll_scale`、`scroll_opacity`、`nudge`、`stagger`）、`window_chrome`、`text_marks`、`dynamic_items`、全部组件（不再有未实现的组件），以及 `features` 中的全部 18 项。`examples/planner` 与 `examples/gallery` 在 Windows 上检查没有错误也没有警告（测试）。两种前端用同一份代码；只在控件或资源不同时分开写，差别在下面注明。

### 样式覆盖

- 每个用到的样式生成一个 `B::StyleDef`：基础外观与各状态（悬停、按下、焦点、禁用、选中）的 `Look`。
- 按钮的底色、前景与边框写成按钮自己 `Resources` 中的轻量样式资源（`ButtonBackgroundPointerOver`、`ButtonForegroundPressed` 等，浅色与深色主题各一份），由控件模板自己的视觉状态切换；样式没写的状态保留系统的外观与反馈。`none` 让该状态与普通状态相同，`system` 保留系统的。`corner` 为 `CornerRadius`，`padding` 为内边距；尺寸档 `small`、`large`、`extra_large` 的最小高度为 24、42、52，并调整字号与左右内边距。
- 缩放、位移与透明度放在“状态”层（见“动效”），以最上层会动的状态所带的弹簧进入，回到普通状态用 `settle`；状态来自指针进入、离开、按下、抬起、捕获丢失与焦点事件，禁用来自 `IsEnabled`。
- `shadow`：Direct2D 的阴影效果按按钮的圆角与尺寸画成位图（只保留按钮形状以外的部分），垫在按钮下方，与按钮在同一个外层里一起缩放、移动；不同状态的阴影以透明度切换。
- 带内容的按钮（`Button { … }`）去掉按钮自己的外框与底色，内容原样显示；`corner` 用 Composition 的圆角几何裁剪内容。内容铺满时按钮随之拉伸。
- 侧栏项：底色与前景写成 `NavigationViewItem` 的轻量样式资源（`NavigationViewItemBackgroundPointerOver`、`…Selected` 等）；`selected: fill(…)` 时选中项整行填色，导航视图自己的选中指示条设为透明。缩放等变换作用在项上：`NavigationViewItem` 拒绝 `UIElement` 的 `Translation`、`Scale`，变换写在它自己的 Composition 视觉上。`indicator` 没有列出：`NavigationView` 只在前缘画自己的指示条，不能换成样式描述的形状。
- 侧栏项的变换与指示条：选中改变时，`NavigationView` 用 `TransformToVisual` 量出新旧两项指示条的位置，横坐标完全相等才播放滑动动画（共 600 毫秒：前 200 毫秒指示条从旧项伸长到新项，三次贝塞尔 (0.9, 0.1, 1.0, 0.2)；后 400 毫秒收拢到新项，(0.1, 0.9, 0.2, 1.0)；旧条在后 400 毫秒淡出，两条重合，看上去是同一条），否则两项的指示条同时各自缩放 600 毫秒（线性），看上去是两段断开的竖条。量出的位置包含写在项视觉上的缩放（WinUI 内核的 `GetLocalTransform` 计入应用在视觉上设的变换），所以点击时被点项正处在悬停与按下的缩放里，总是走后一种。为此：（1）运行时每次改变选中时让所有侧栏项暂时没有变换，量完再写回，同一次调用里完成，屏幕上看不到：点击与按键的顺序是 `ItemInvoked`、量位置与动画、`SelectionChanged`，在前一个事件里放下、后一个事件里恢复（点的是已选中的项时不放下；选中最终没有改变时没有 `SelectionChanged`，由投递的任务恢复）；运行时自己写 `SelectedItem` 时在前后包住。（2）侧栏项缩放时横向以指示条所在的线为原点（找模板中的 `SelectionIndicator`，取包着它的格子在项中的横坐标），行从前缘起伸缩，指示条留在它滑动的轨道上，新旧两条在滑动中重合。系统 XAML（Islands）的导航视图按 2018 年的 WinUI 源码（系统版本由此而来）没有横坐标的判断，`SelectionChanged` 在动画之前；（1）在那里不起作用也无害，（2）照样让两条重合。时长与曲线取自 microsoft-ui-xaml 的 `NavigationView.cpp`（`AnimateSelectionChanged`、`PlayIndicatorAnimations`、`PlayIndicatorNonSameLevelAnimations`）。
- 语义角色映射到各前端的主题资源：WinUI 3 用 `SolidBackgroundFillColorBaseBrush`、`CardBackgroundFillColorDefaultBrush`、`TextFillColorPrimaryBrush`、`AccentFillColorDefaultBrush` 等，系统 XAML 用 `SystemControl…` 一族；系统 XAML 没有的角色按浅色、深色各给一个取值。画布等需要具体颜色的地方，从窗口中隐藏的探针元素读出主题画刷当前的颜色，所以跟随所在部分的明暗与运行时强调色。

### 动效

- 弹簧：质量 1，刚度 (2π / T)²，阻尼比 ζ，与其他目标相同；所有在动的弹簧在 `CompositionTarget.Rendering` 中一起推进，每帧一次，换目标时从当前值与速度继续。系统关闭动画（“设置 → 辅助功能 → 视觉效果 → 动画效果”，`UISettings.AnimationsEnabled`）时，状态变换、错峰、晃动与滚动联动不执行，出现与消失只做短淡入淡出。
- 变换层：元素的移动、缩放与透明度由几层（样式状态、出现、消失、共享元素、重排、晃动、显示、轨道）各自的弹簧合成，位移相加、缩放与透明度相乘，写到 `UIElement.Translation`、`Scale`（以元素中心为原点）与 `Opacity`，不改变布局。会动的元素外面包一层 `Grid`，变换写在这层上。
- 出现与消失：`if`、`match`、`ForEach` 在首次显示之后挂载的元素播放 `appear`；元素内部的元素随它一起出现。卸载的带 `disappear` 的元素经它自己 `Resources` 中的离场钩子留在面板里播完再移除，期间不接收点击；它已经不在自己的区域中，新元素按它不存在时的位置插入。
- `stagger`：容器构建直接子元素时带一个计数，带 `appear` 的子元素依次晚 `stagger` 秒，等待按渲染帧计，在起始姿态（隐藏）中等到自己的轮次；容器在一次更新中出现时子元素也播放出现；`ForEach` 的各行每次更新从 0 计起。
- 共享元素 `hero`：元素卸载时记下它在窗口内容中的位置与尺寸（不含 Bern 自己的变换）。离开的元素本身移到窗口内容上方的覆盖层（一个 `Canvas`）里原来的位置；同一窗口中同键的新元素布置后，从旧位置与尺寸以 `hero` 的弹簧展开，旧元素随同一个弹簧移动并淡出。没有使用 `ConnectedAnimationService`：它要求在导航时成对准备与开始，不适合由条件与列表随时卸载与挂载的元素。
- `reflow`：`Column`、`Row`、`Grid`、`ScrollView` 的面板，以及带 `reflow` 的 `ForEach` 所在的这些面板，在布置后比较每个子元素的新旧位置，被移动的子元素从旧位置以弹簧滑到新位置。表单、`Layers` 等其他容器中的 `reflow` 不起作用（没有警告）。
- `animate`：绑定状态的长度（`padding`、`gap`、`width`、`height`、`Grid` 的 `min_width`、`Canvas` 与 `Image` 的尺寸、`Video` 的 `corner`、`Rail` 的比例）随这个弹簧变化。`Video` 的圆角是 Composition 的圆角几何裁剪。
- `nudge`：条件变真时放大到 1.06 再以 `jelly` 弹回；`ScrollView(arrival: nudge)` 让设置 `position` 滚到的那一项停稳后晃一下。
- `scroll_scale`、`scroll_opacity`：Composition 表达式动画，读 `ScrollViewer` 的滚动属性集（`GetScrollViewerManipulationPropertySet`），按元素中心离视口中心的距离算缩放与透明度，滚动时不经过界面线程。元素位置与视口尺寸在布局更新时写进表达式的参数。

### 自绘与图片

- `Canvas` 在界面线程上用 Direct2D 画进按显示缩放分配的 WIC 位图，再写进 `WriteableBitmap` 由 `Image` 显示；状态、尺寸、元素的明暗或运行时强调色变化时重画。两种前端同一套代码。`Shape` 的 `fade` 为向一侧渐隐为透明的线性渐变，描边向内收半个线宽；`Series` 为面积与折线；`Pattern` 按条带逐条绘制，`twill` 叠 45° 的细线（`text.primary`，不透明度 0.05）。
- `fill_from` 的图片在后台读取（`file:` 从磁盘，http(s) 从网络），用 WIC 缩到 32×32 取平均色，按 URL 缓存；读到之前用 `fill`。
- `Text(mark:)`：按 `TextBlock` 的 `TextPointer` 取每行文字的矩形，在文字下方画胶囊形的标注，高度为行高的 74% 到 98%，左右各延出 0.4 个字宽。
- `Image(source: app.x)` 在后台读取后显示，`None` 时不显示；只写一边时另一边按图片比例。

### 窗口外框、静置与对话框

- `idle_after`：窗口根元素上（含已处理的事件）的指针移动、按下、滚轮与按键重新计时，到时 `window.idle` 为真。指针移动只在屏幕上的光标位置确实改变时才算：沉浸式窗口改变外框时，系统会在原位置再报告一次指针移动，否则会立即被自己唤醒。
- `immersive`：为真时隐藏标题栏与详情页头。WinUI 3 下 `TitleBar` 收起，`OverlappedPresenter.SetBorderAndTitleBar(true, false)` 去掉系统标题栏与标题按钮；Islands 下去掉窗口样式中的 `WS_CAPTION`（保留可调整大小的边框）。同时静置时指针隐藏（`ShowCursor`）。
- `chrome: unified`：WinUI 3 下内容本来就延伸到标题栏，标题栏在图标旁显示窗口标题（设计稿的 Fluent 窗口如此），`unified` 不改变它；Islands 下保留系统标题栏。
- `role: dialog`：没有自己的顶层窗口，是主窗口（或打开时第一个普通窗口）上的 `ContentDialog`，宽度按声明的尺寸在对话框允许的范围内设置；对话框的关闭（包括 Esc）都经关闭协议。对话框窗口报告所属窗口的位置，`minimize`、`maximize`、`move`、`resize` 返回 `target_unavailable`。
- 窗口尺寸档：窗口根元素的宽度按 `[layouts]` 的三个断点得到 `compact`、`medium`、`expanded`、`large`，表达式中的 `window.compact` 等读取它；`Adaptive` 四档各自选择，没有自己布局的档位显示下一个更大的已声明布局，没有更大的取更小的。
- 运行时强调色与明暗：`WindowSet(accent:, appearance:)` 的表达式在每个窗口中求值。控件模板经主题字典（含别名与合并的字典）中的强调色画刷取色，运行时找到这些画刷（浅色与深色两套，各自对应强调色的哪一档）直接改它们的颜色，并更新 `SystemAccentColor` 与 `…Light1–3`、`…Dark1–3`，所以已有的控件立即换色；各档按与白色、黑色混合近似生成。明暗设在窗口根元素的 `RequestedTheme` 上；WinUI 3 的标题按钮经 `AppWindowTitleBar.PreferredTheme` 跟随，Islands 的系统标题栏经 DWM 跟随。
- `appearance: dark` / `light` 的部分：部分的根元素设 `RequestedTheme`，XAML 的主题资源本来就按子树解析，部分中的控件、画布与从中弹出的浮层（浮层的呈现元素另设同样的主题）因此按那种外观绘制。

### 媒体类应用的组件与输入

- 文件：`pick` 用 `FileOpenPicker`（按种类给出扩展名过滤）或 `FolderPicker`（`folder`），经 `IInitializeWithWindow` 交给所在窗口，选中后以 `file:` 地址存进字段再执行动作。`ui.set_file` 校验地址（`file`、`http`、`https`，否则 `invalid_url`）与种类（否则 `unsupported_file`）后交给带 `pick` 的按钮、菜单项或 `Drop`。
- `Drop`：内容区接受拖入的文件（`StandardDataFormats.StorageItems`），种类相符时 `over` 为真，放下后取第一个相符的文件。`opens: true` 时，命令行上的文件与再次启动时交来的文件（第二个进程经 `WM_COPYDATA` 转给第一个）也交给它；构建不登记文件关联。
- `Pinch`：触摸的捏合经 `ManipulationDelta` 的缩放；触控板捏合在 Windows 上以带 Ctrl 的滚轮到达，按每格 e^0.1 缩放。`Swipe`：按下后沿轴拖动，`distance`（默认 300）走完整个范围，向上与向右增大，捕获指针，抬起时执行动作。
- 快捷键：每个快捷键是窗口根元素上的 `KeyboardAccelerator`，随控件挂载与卸载；`cmd` 与 `ctrl` 都是 Control。`Search` 的快捷键聚焦搜索框，开关与分体按钮切换，按钮、菜单项与浮层按钮激活；控件禁用时不起作用，没有 Control 或 Alt 的快捷键在文本框有焦点时不触发。菜单项的快捷键显示在菜单中。
- 工具栏搜索与结果：搜索框是 `AutoSuggestBox`；结果是任意组件，所以不用它自己的建议列表，而是在搜索框下方的 `Flyout` 中显示，搜索框有焦点且有结果时打开，点选结果后收起。
- `PopoverButton`：按钮的 `Flyout`，内容在浮层打开时挂载、关闭后卸载；`presented` 双向跟随。`SplitButton`：`ToggleSplitButton`，主体切换开关，旁边的部分打开同样的浮层；该部分的无障碍名称为 `more_label`，没有时为“更多选项”。
- `Toast`：在所在位置显示系统的 `InfoBar`（信息级别、不可关闭，`icon` 为它的图标），从下方滑入，`duration` 秒后淡出；内容变化时重新显示，朗读为礼貌的实时区域。系统 XAML 没有 `InfoBar`，Islands 下是同形的 4 圆角条。
- `Inspector`：所在容器换成右侧窗格的 `SplitView`（`Inline` 模式），`presented` 为真时窗格滑出，其余内容让出位置。
- `ScrollView(snap:, position:)`：滚动停下（`ViewChanged` 不再是中间状态）时取最近的一项，以动画对齐到锚点；`center` 时首尾按最大一项留出空白，位置按项在内容中的坐标加内容的边距计算。停稳的一项写回 `position`；字段被设成另一项时以动画滚过去；首次布局后直接停在字段的一项。
- 重复项中的控件目标带上每一层的键，经过组件也一样（如 `card.open[3]`）。

## 页面级布局、场景与控件形态的能力（2026-10-19）

能力描述补上此前只有 SwiftUI、Web 与 GTK 声明的 29 项 `features`（`text_weights`、`text_lines`、`image_corners`、`sidebar_width`、`controlled_values`、`search_palette`、`image_template`、`adaptive_images`、`scaled_scene`、`accordion_layout`、`capsule_controls`、`modal_flow_timer`、`page_badges`、`idle_control`、`fill_buttons`、`grid_cells`、`page_search`、`form_layout`、`modal_cover`、`window_short`、`edge_grow`、`delayed_appear`、`toolbar_leading`、`axis_padding`、`icon_menus`、`icon_labels`、`row_baseline`、`sidebar_header`、`untitled_detail`），动效补上 `appear_delay`、`nudge_key`、`nudge_scale`、`reduce_motion`，样式补上 `Stack` 的 `background`、`foreground`、`corner`。`window_landscape` 仍未声明：桌面窗口没有设备方向，检查给出警告。两种前端用同一份代码（新代码在 `bern_ui_scene.inl` 与原有各部分中），只有侧栏计数在系统 XAML 中改为跟在名称后的数字（它的导航项没有 `InfoBadge`）。图标表补上 33 个名称（如 `house` → Home、`calendar` → Calendar、`text.bubble` → Message、`server.rack` → MediaStorageTower、`gobackward.10` → SkipBack10、`goforward.10` → FastForward、`sparkles` → SearchSparkle），码位取自 Segoe Fluent Icons 的官方列表。

### 外观与文字

- `Stack` 的样式：层叠外包一个 `ContentControl`，它的 `Foreground` 向下继承给文字与 `FontIcon`；底色画在层叠的 `Grid` 上（角色经 `ThemeResource` 样式随主题，固定色按元素主题），`corner` 是 `Grid.CornerRadius` 加一个 Composition 圆角裁剪，各层按圆角裁住。`Stack(enabled: false)` 用同一个外包的 `IsEnabled`：其中的控件全部禁用、不接收输入。
- `text { … }`：目录带上各文字样式的字重与字号，`Text` 与 `Label` 在系统的文字样式之上设置 `FontWeight` 与 `FontSize`。`Text(lines:)` 是 `MaxLines` 加末尾省略号；`font_size`（可绑定）与 `line_height`（`LineHeight` 与 `BlockLineHeight`）。`Label(font_size:)` 同时是图标的字号；`display: icon` 只显示图标，标题作提示与无障碍名称。
- `Image(corner:)` 是可绑定、随 `animate` 弹簧的 Composition 圆角裁剪；`rendering: template` 用 `BitmapIcon`（`ShowAsMonochrome`，颜色随所在处的前景色）；`aspect` 取代图片自身比例，只给 `aspect` 时由一个面板按容器宽度与比例定高。

### 控件形态与受控值

- Fluent 2 没有胶囊形按钮（设计稿的 Fluent 一栏中主要与次要按钮都是 4 圆角的 `Button`），所以 `shape: capsule` 保留控件样式自己的圆角，只取 `height`：设置控件高度，去掉最小高度与上下内边距（左右内边距在控件进入可视树、样式生效后保留）；带内容的胶囊按钮把内容按按钮的圆角裁剪。`IconButton` 是 Fluent 的应用栏按钮样子：没有底色、指针移入才有，系统圆角，图标四周各 10（`height` 或样式的 `size` 给出正方形边长）；`prominence: prominent` 为强调色按钮，有边长时是圆形（设计稿的播放键）；`font_size` 是图标字号。图标菜单同样无底色。`Button(width: fill)` 横向撑满。`ControlGroup` 像命令栏那样把按钮并排放（默认高 40），按钮没有底色，组本身也不画底板（Fluent 用位置表达分组）。
- `Menu(style: icon)` 是只显示图标的按钮加 `MenuFlyout`，有 `height` 时是圆形标准按钮，否则无底色。胶囊形的下拉 `Picker` 是 `DropDownButton` 与 `ToggleMenuFlyoutItem`。
- 受控值：`Switch(checked:)`、`Picker(selected:)`、`Slider(current:)` 显示表达式的值，改动时执行动作的命令，参数中与该表达式相同的一项换成新值（整数参数四舍五入）；受控滑块拖动时只移动自己，松开时执行命令，键盘改动立即执行。`PickerOption` 给出自己的值与标签。`label_hidden` 不显示标签，只作无障碍名称。

### 布局与页面结构

- `padding_x`、`padding_y` 在 `padding` 之上分别覆盖左右与上下。`Row(align: baseline)`：各子元素先顶对齐排布，再按各自第一个文字块的 `BaselineOffset`（经 `TransformToVisual` 换算到子元素内）下移到同一条基线，多占的高度在下一轮测量中计入。
- `Grid(row_gap:)` 单独设置行距；`Grid(cell_ratio:, cell_extra:)` 每行高为列宽 ÷ `cell_ratio` + `cell_extra`，各格从格子顶部排，内容更高时伸进下面的行距。`Flow` 是换行面板（`FlowPanel`）：子项在行内竖直居中，`Spacer`（角色 `bern-spacer`）换行时不占宽度，排好后分得所在行余下的宽度。
- `ScaleBox`：子元素在声明的逻辑尺寸中层叠，整体以一个 `CompositeTransform`（缩放加居中平移）适配所得空间，超出部分裁掉；图片、文字与点击区域保持对齐。`Positioned` 在所在层叠中占满，内容按（可弹簧的）`x`、`y`、`width`、`height` 放置并居中。`Marquee`：内容按自然宽度每帧（`CompositionTarget.Rendering`）从右缘外跑到左缘外，`paused` 时停住，减少动态效果时静止在宽度 28% 处；其中的文字由 Composition 的 `DropShadow`（以文字自己的 `GetAlphaMask` 为蒙版）带 1 点黑色阴影。
- `Accordion`：每列一个 0 到 1 的权重弹簧，列宽由权重分配，总宽不变，每列按 `corner` 圆角裁剪（裁剪在每次排布时按列的大小设置）。
- 页头：工具栏中 `Spacer` 之前的项放在返回按钮之后的前缘，之后的放在后缘；`Text` 可以放在工具栏里。`SidebarHeader` 是 `NavigationView.PaneHeader`，`role: title` 时不显示（两种前端的标题栏都已在应用图标旁显示窗口标题）；`Sidebar(width:)` 是 `OpenPaneLength`；`Detail(titled: false)` 时页头不显示所选目的地的名称；`SidebarItem(badge:)` 在 WinUI 3 中是 `InfoBadge`（为 0 时隐藏）。
- `Form(gap:, row_height:, padding_x:, padding_y:)` 与 `Section(style: plain, padding_x:, padding_y:)`：分节间距、卡片行的最小高度与内边距（`Section` 自己的优先），后来加入的行也照此设置；plain 分节的行直接放在页面上（没有卡片），只用自己的内边距、不取表单的；表单以它开头时从页面顶部开始：plain 分节的面板名为 `bern-plain-section`，表单在建好内容和内容尺寸变化时看第一项（穿过包装层），所以插槽、组件或条件给出的标题也算。页面中的 `Search` 用 `width`、`height`、`font_size` 设置搜索框自身。

### 模态、搜索面板与计时

- `Modal` 是系统的 `ContentDialog`（设计稿 Fluent 一栏的模态面板）：系统的遮罩、圆角与出入场动画，内容为声明尺寸（离窗口边缘至少 16），没有标题与按钮区；`cover: compact` 时在紧凑窗口中铺满。它在元素挂载期间一直显示，Esc 不关闭。窗口的 XAML 根就绪后才显示，跟随窗口的明暗。元素的 `appear`、`disappear` 由对话框自己的出入场代替。同一窗口同一时间只能有一个 `ContentDialog`。
- `Search(style: palette)`：所在位置是系统的 `AutoSuggestBox`（`prompt` 为占位文字，工具栏中 240 宽）；点按、点放大镜、在其中打字或快捷键打开面板。框里的文本框会把点按标为已处理，所以点按用 `AddHandler(TappedEvent, …, true)` 订阅（原先普通订阅收不到，点了不打开，2026-10-20 修正）；放大镜按钮发出 `QuerySubmitted`，也打开面板。面板从搜索框长出来（设计稿 10.8 的“搜索框就地展开”、T17）：窗口内容上盖一层，页面变暗（浅色 20%、深色 32% 的黑），面板从搜索框的矩形与圆角（4）按 `hero` 弹簧移到搜索框所在页头那一栏的正中、搜索框下方 26 处，`width` 宽（离窗口边缘至少 16）、圆角 8，高度是输入行与结果的高度、最高 `height`（默认 640、480），结果变化时高度按 `bouncy` 弹簧伸缩；内容在面板长到约三分之一后淡入，关闭时内容先隐去、面板按 `snappy` 弹簧缩回搜索框，搜索框在面板打开期间隐藏。面板是浮层的材质（`AcrylicInAppFillColorDefaultBrush`、`SurfaceStrokeColorFlyoutBrush` 描边、`ThemeShadow` 投在遮罩上；Islands 用 `SystemControlTransient…` 画刷、直角）。上方 56 高的一行是放大镜、系统的 `TextBox`（已打的字带过去）与 `esc` 按钮，一条分隔线下是结果（内边距 6），结果只在面板打开时构建、关闭即释放。原先面板是贴在搜索框下的系统 `Flyout`：没有就地展开的动画，宽度被 `FlyoutPresenter` 的边框挤掉 2 点而出现横向滚动条（2026-10-20 改）。上下方向键在 `navigation` 不为假的 `SearchItem` 之间移动（按屏幕上的先后），回车选中当前项，没有当前项时执行提交动作；Esc、`esc` 按钮或点在面板外关闭；指针移入某项时它成为当前项。`SearchItem` 选中后先关闭面板再执行动作（包括 `pick`）；不在面板中时是普通的结果行。
- `Timer`：启用且挂载时每 `interval` 秒（不少于 0.05）执行动作，关掉或卸载时取消下一次。`Window(idle_enabled:)` 为假时 `window.idle` 保持为假且不计时，变真时重新计时。`window.short` 是窗口高度低于紧凑断点。

### 动效补充

- `appear_delay` 在错峰延迟之上再等待，有延迟时首次显示也播放 `appear`；`nudge_key` 的值每次变化晃动一次，`nudge_scale` 是晃动的倍数（默认 1.06）；`reduce_motion` 为真时整个应用按减少动态效果运行，与系统关闭动画相同。
- `grow(edge:)` 过渡：沿一个轴从 0 伸展、该边保持不动、不淡入淡出。动效层的缩放可以带一个不动点，按缩放补上平移。

### 文本格式（`text_formats`）

- 目录中的字符串表：带参数的翻译文本保留 Bern 的格式原文；不带参数的把 `{{`、`}}` 还原为花括号；月名与星期名（`bern.date.*`）随表写入，没有该语言时用英文。
- `bern::Text::Fmt(key, {{"名", 参数}, …})` 在运行时解析当前语言的文本再写出。`bern::Text::Arg` 是文本、数字（值与 Bern 的写法）或日期；生成器对 `CalendarDate` 传 `DateArg`，对枚举传它的标签，其余传 `ArgOf`（整数与小数保留数值）。复数分支先找 `=N`、再找当前语言的类别（中日韩越泰印尼马来只有 `other`，其余语言 1 为 `one`），最后 `other`，`#` 写出数字；日期按 `y`、`M`、`d`、`E` 的连写取字段或月名、星期名；时长中写出的最大字段吸收更大的单位。文字绑定读取语言单元，切换语言时重新写出。
- 枚举标签显示变体字段：目录的 `VariantDef` 带上标签显示的字段（占位符名与字段序号），`Text::Display` 按字段类型把它们作为参数填入（枚举字段用自己的标签，可以嵌套；`CalendarDate` 是日期）。只有变体、没有值的 `Text::Label`（选项列表）写出时这些字段为空。

### 顺带的修正

- 行与列分配空余空间时，给了高（宽）的元素不再在该方向撑满，即使其中有撑满的内容：此前给了高度、里面铺着 `Canvas` 的 `Stack` 与同列中另一个撑满的元素平分剩余高度。
- `height` 去掉上下内边距时，此前在控件进入可视树之前读取内边距（样式尚未生效，读到 0），把左右内边距也一并清零，带文字的按钮被压成高度那么宽；现在在 `Loaded` 后读取并保留左右。
- `ScrollView` 的指定初始化器顺序与 `ScrollProps` 的声明顺序不一致（`open_at` 与 `indicators: hidden` 同时出现时 MSVC 报 C7560）。
- 对照设计稿（Fluent 2 桌面版）修正的控件与布局：
  - 设计稿的强调色填充在浅色与深色下都是品牌色本身，配白字（亮的强调色配近黑字）；此前浅色用加深一档、深色用混白两档（深色区域里显得发粉）。`AccentFillColor*` 现在都取品牌色，`TextOnAccentFillColor*` 按亮度取白或近黑。
  - 深色区域（如播放器）弹出的菜单与浮层：只设 `RequestedTheme` 时字变白、底仍是窗口主题的浅色，看不清；现在样式里同时放入该主题字典中系统自己的 `MenuFlyoutPresenterBackground`、`FlyoutPresenterBackground` 与边框画刷。
  - `ToggleSwitch` 旁不再显示“开/关”，与设计稿一致。导航窗格展开时不显示折叠按钮（窄的模式保留）。
  - 分段控件（`SelectorBar`）靠前缘按自身宽度，不再在给定宽度中居中。分体按钮显示图标与标题（工具栏中只有图标）。
  - 条件分支里后来出现的、要撑满的内容（如引导每一步的 `ScrollView`），会让外层已放好的列、层叠与容器跟着撑满；此前外层的拉伸只在放入时判断一次，引导页的底栏因此悬在中间。
  - 横向的 `Divider` 横向撑满（此前在层叠中被设为居中而宽度为 0）；`Slider` 横向撑满给它的宽度（此前在 `Positioned` 中缩成 80）；`Rail` 只取所在层叠的尺寸，不再让层叠横向撑满（此前缩略图层叠与 `Spacer` 平分剩余宽度，旁边的标题错位）。
  - 只给 `position`、没给 `snap` 的 `ScrollView` 以起点停靠，与其他目标相同；此前按居中在内容前后留出半个视口，海报墙上方多出约 140 的空白。
  - 不给尺寸的 `Canvas` 在层叠中是其他层的底板：取它们的尺寸，不让层叠撑满（与 `Rail` 相同；层叠里只有底板时仍然撑满）。画布自身不报告期望尺寸（位图在 XAML `Canvas` 里按外框大小显示），否则上次画出的大小会撑住层叠。此前可选功能预览里选中项的底色被拉成一大块。
  - 行太窄时文字让位：文字与含文字的行列按比例收窄（换行或截断），控件与给了宽度的元素保持原尺寸，与其他目标一致；此前设置卡片里较长的说明把开关挤出卡片。
  - 每个窗口与模态的根元素设置界面语言（`Language`，随切换更新），系统按语言选择中文的界面字体；此前半粗的小字回落成宋体的点阵。
  - `Label` 的图标随文字字号缩放（字号的 1.15 倍），与文字的间距为字号的 0.6 倍（4 到 12）；此前图标固定 16。
  - 语义色 `control.border` 是 Fluent 的控件描边 `ControlStrokeColorDefault`（设计稿的 `rgba(0,0,0,.0578)`），此前是未勾选复选框那种深描边。
  - 工具栏前缘（返回一侧）的按钮没有底色，与窗口自己的返回一致；`play.fill`、`pause.fill` 用实心的 PlaySolid、PauseBold。
  - `BERN_DUMP_TREE` 输出的元素树带上每个元素在窗口中的位置、显式高度、文字及其字体、字号与字重，用来对照设计稿定位布局问题；另有元素自己的裁剪，以及布局给的位置小于它要的尺寸时的两者。
  - 带内容的按钮里，不撑满高度的内容在按钮里垂直居中，与文字按钮相同；此前内容贴在顶部（32 高的「继续上次」里图标、文字与键帽比居中高约 5.5）。样式圆角的裁剪按内容在按钮里的实际位置算出按钮的形状。
  - 语义色 `fill.strong` 是 Fluent 的 `ControlAltFillColorQuarternary`（浅色 9.4% 黑、深色 7.1% 白），在两种外观下都比 `fill.subtle`（`SubtleFillColorSecondary`）深一档，与其他目标的约 12% 同一档；此前是滑块轨道那种 `ControlStrongFillColorDefault`（45% 黑），头像底色、键帽与设置图标底都发深。系统 XAML 用 `SystemControlBackgroundListMediumBrush`。
  - 没有外框的带内容按钮（不是胶囊）去掉系统按钮的 1 宽边框，尺寸只由内容与样式的内边距决定，与其他目标相同；样式画边框时，边框在内边距之内。此前按钮比其他目标多出 2，放在定高的位置（纯粹模式的最近播放行，61.5 高）里被压扁，按钮的内容区把缩略图底部约 3 的进度条裁掉（单独渲染这一层叠时画面完整，逐层往外渲染找到按钮这一层）。样式圆角的裁剪按包含内边距的按钮形状，与 SwiftUI 在内边距之后裁剪一致；此前裁在内容框上。
  - `under_bars` 期间内容铺到标题栏下，页头仍留在标题栏下方（上边距为标题栏的高度），后缘让出标题按钮的宽度再加 12，与设计稿 Fluent 版的工具栏相同：150% 缩放下齿轮右缘离窗口右边 150；标题按钮保持标准高度。此前页头跟着内容进了标题栏那一行，齿轮压在关闭按钮上、落在拖动区里。
  - `TitleBar` 按像素数设置模板里给系统菜单与标题按钮让位的首尾两列：150% 缩放下给标题按钮留 207（`RightInset` 的像素数）而不是 138，放在标题栏里的内容离标题按钮多出约 70、标题文字可用的宽度也少了这么多。现在每次布局后按缩放改回单位（只在不同时设置）。
- 动画 SVG 的运行时（见 [Animated-SVG](Animated-SVG.md)）此前没有在 Windows 上编译过：补上 `B` 命名空间、放行文档自己的导航、两种前端都改用 WebView2 的 Win32 呈现器以得到透明背景，缓存放到临时目录；新增生成测试 `svg_animation_generates_and_compiles` 在 Windows 上编译示例。

## 构建与运行

```powershell
cargo run -p bern-cli -- build --manifest examples/showcase/Bern.toml --target windows
target\bern\windows\Showcase\showcase.exe
```

构建需要 Rust、Visual Studio 的“使用 C++ 的桌面开发”（MSVC 与 Windows SDK），不需要 .NET：

- **构建脚本**：用 `vswhere` 找到 Visual Studio，加载 `vcvarsall`。
- **Windows App SDK 组件包**：先在 NuGet 缓存中找，找不到时用 Windows 自带的 `curl` 与 `tar` 从 nuget.org 取到 `target/bern-winrt/packages/`。
- **C++/WinRT 头文件**：用 Windows SDK 的 `cppwinrt.exe` 生成，按 SDK 与 WinUI 版本缓存在 `target/bern-winrt/`（约 110 MB，只生成一次）。
- **预编译头**：C++/WinRT 头文件与运行时占了编译的绝大部分（WinUI 3 前端单独编译约 46 秒，其中解析头文件约 23 秒），而它们与应用无关。`bern_pch.h` 把两者都放进一个预编译头，每个配置一份，放在 `app/obj/debug/` 或 `app/obj/release/`（约 2.4 GB）。它只在运行时模板、编译选项、MSVC 版本、SDK 或路径变化时重新生成：脚本把 Bern 算出的运行时与选项指纹、`VCToolsVersion`、SDK 版本和各路径写进 `pch.key`，下次构建对比一致就直接复用。其余五个编译单元每次都用 `/MP` 并行编译，引擎库由 cargo 增量构建。
- **构建时间**（i5-13490F，16 线程）：

  | 情形 | 之前 | 现在 |
  | --- | --- | --- |
  | 首次调试构建（生成预编译头） | 约 60 秒 | 约 82 秒 |
  | 新项目：未改动、改界面、改引擎 | 约 60 秒 | 5–6 秒 |
  | 展示应用：再次构建 | 约 60 秒 | 约 9 秒 |
  | 首次 release 构建 / 再次 | 约 90 秒 | 约 95 秒 / 5 秒 |

产物位于 `target/bern/windows/`：`Showcase/`（`showcase.exe`、`Microsoft.WindowsAppRuntime.Bootstrap.dll` 与 `resources/`）、`bin/showcase.exe` 与 `bin/showcase-mcp.exe`，以及构建清单 `build.json`（记录前端、组件包版本与最低 Windows 版本）。

应用图标 `app.ico`（没有声明 `[app].icon` 时为 Bern 标志的圆形版，16 到 256 像素各一张 PNG）编入可执行文件的图标资源 1：资源管理器显示它。运行时按窗口所在显示器的 DPI 把它设为每个窗口的大小图标（任务栏、Alt+Tab、Islands 的系统标题栏），WinUI 3 的 `TitleBar` 另以 `ImageIconSource` 显示其中最接近标题栏尺寸的一张。`[app.icon].windows` 可以给出 `.ico` 文件，原样编入；其中的位图项（不是 PNG 的）由标题栏包成单张的 `.ico` 再解码。

**开发辅助**（默认全部关闭）：

- `BERN_SNAPSHOT_DIR`：每次变化后输出两种快照。`<窗口>.png` 是 XAML 内容的渲染，打开的对话框和菜单另存为 `-popup<n>.png`；`<窗口>-window.png` 用 `PrintWindow` 截取应用自己的窗口，含标题栏、Mica 与视频画面。
- `BERN_APPEARANCE=light|dark`：覆盖系统外观。
- `BERN_FORCE_ISLANDS`：在装有运行时的机器上试用回退前端。
- `BERN_APP_RUNTIME=installed|inbox|none`：只用其他程序装的运行时、只用 Windows 自带的，或当作都没有（只编译 WinUI 3 时会出现提示框）。
- WinUI 的未处理错误与控件样式加载失败写到标准错误，然后照常结束进程。
- `BERN_DUMP_TREE`：把元素树和尺寸写到标准错误。
- `BERN_SNAPSHOT_BURST`（与 `BERN_SNAPSHOT_DIR` 一起）：每次变化后，另把屏幕上的窗口每 40 毫秒截一张、共 800 毫秒，存为 `<窗口>-burst<次>-<帧>.png`，用来逐帧查看动效。
- 调试构建在未处理的异常时，按 PDB 把调用栈（函数与行号）写到标准错误。PDB 留在 `app/obj/<配置>/`，不进入应用目录。

### 打包

`bern package --target windows` 按 release 构建，生成 `<产品>-<版本>-windows-<架构>-setup.exe` 与同名 `.zip`：

- 应用目录与 `bin\` 中的 CLI、MCP 服务器先复制到暂存目录；有 `BERN_WINDOWS_CERTIFICATE` 时用 `signtool` 签名这些程序，再签安装程序（证书库指纹或 `.pfx`，RFC 3161 时间戳）。证书与密码只从环境变量读取，不写进生成的脚本。
- 安装程序由 Bern 生成（`templates/installer/setup.cpp`），文件作为资源编进可执行文件，用同一套 MSVC 编译。它按当前用户安装到 `%LOCALAPPDATA%\Programs\<产品>`，不需要管理员权限；创建开始菜单快捷方式，在“应用和功能”中登记卸载项；提示按系统界面语言显示（英文、简体中文、繁体中文、日文），`/S` 静默安装。清单声明 `asInvoker`，否则 Windows 会把名字含 setup 的程序当作需要提权的安装程序。
- 安装目录中的 `uninstall.exe` 是安装程序的副本，先把自己复制到临时目录再执行，以便删除整个目录；`/S` 静默卸载。应用或其工具在运行时（按进程映像路径判断），安装与卸载都拒绝进行，不删任何文件；`uninstall.exe` 最后删除，中途失败也能再次卸载。
- 实测（2026-10-12，新项目）：安装程序 5.08 MB，`.zip` 1.94 MB；静默安装后文件、快捷方式与卸载项正确，从安装位置启动并经 `bin\my-app.exe` 驱动；运行中卸载与覆盖安装都被拒绝，关闭后卸载清除目录、快捷方式与卸载项；中文确认框显示正常；Defender 没有报告。未签名时 SmartScreen 会提示。

## 实测数据

2026-10-11，同一台机器，release 构建。启动时间为从启动到窗口可见，首次启动另计；内存为窗口显示 5 秒后的私有工作集。

| | WinUI 3（测量时为 Runtime 1.8） | 系统 XAML（Islands） | 第一版 C#/.NET |
| --- | --- | --- | --- |
| 启动 | 0.32 秒（首次 0.51 秒） | 0.54 秒 | 0.62 秒 |
| 内存 | 35 MB | 19 MB | 42 MB |
| 应用目录 | 4.2 MB（两种前端共用） | 同左 | 141 MB，另需 .NET 10 |

4.2 MB 是 `showcase.exe`（3.8 MB，内含引擎）加上引导 DLL（0.4 MB），不含展示用的媒体资源。

## 验证记录

2026-10-11，Windows 11 25H2 实体机，经 SSH 构建，以计划任务把应用启动到用户的桌面会话，由生成的 CLI 驱动；截图来自开发快照中的窗口截取。两种前端分别执行：

- **构建与测试**：`cargo test --workspace` 在 macOS 与 Windows 上各 195 项通过；Windows 上包含经预编译头用 MSVC 编译生成的两套前端。`cargo clippy` 与 `cargo fmt --check` 通过。
- **前端选择**（按 Windows App SDK 2.5.1 编译）：默认启动加载 `C:\Program Files\WindowsApps\Microsoft.WindowsAppRuntime.2_2.5.1.0_x64__8wekyb3d8bbwe\Microsoft.ui.xaml.dll`；`BERN_APP_RUNTIME=inbox` 时加载 `C:\Windows\SystemApps\Microsoft.WindowsAppRuntime.CBS.2_8wekyb3d8bbwe` 中的 2.2，新项目与展示应用都正常（展示应用逐页切换并打开设置窗口，样式完整）；`BERN_APP_RUNTIME=none` 或设置 `BERN_FORCE_ISLANDS` 后用 `C:\Windows\System32\Windows.UI.Xaml.dll`，进程中没有 Windows App Runtime。
- **只编译一套前端**：`frontends = ["xaml-islands"]` 时应用目录只有可执行文件，进程不加载引导程序；`frontends = ["winui3"]` 在两种运行时上都正常，`BERN_APP_RUNTIME=none` 时弹出中文提示框（标题为应用名，正文与“是/否”），选“否”后以退出码 1 结束。
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

2026-10-14，v0.29 与 v0.30 的能力，同一台 PC（Windows 11 25H2，150% 缩放），经 SSH 构建、以计划任务把应用启动到桌面会话，由生成的 CLI 驱动；静态画面来自开发快照，动效来自 `BERN_SNAPSHOT_BURST` 的逐帧截取，悬停、按下与真实快捷键由桌面会话中的输入脚本产生（指针与按键事件），截取范围只限应用窗口。“实测”指在这个环境里看到或量到。

- **构建与测试**：planner、gallery 与 showcase 以 `bern build --target windows` 构建成功，检查没有错误与警告；`bern-backend-winui3` 的 16 项测试在 macOS 与 Windows 上都通过（Windows 上生成测试用 MSVC 编译两套前端；新增：planner 与 gallery 检查没有警告并生成预期的代码、图标表有序且都在私用区），`cargo clippy -p bern-backend-winui3 --all-targets -- -D warnings` 与 `cargo fmt --check` 通过。
- **planner（WinUI 3 实测）**：画布的格纹、面积与折线、成功色胶囊；“最近的进度”的标注；`show_archive` 打开后侧栏多出“归档”，在归档页关闭它后选中回到 `fallback` 的“概览”；模块页“笔记”经可选模块的条件出现；切换大标题时新标题从小标题的位置展开、旧标题淡出（逐帧截取）；新标签以 `pop` 从自身中心弹出并带回弹；专注模式打开后静置 3 秒，标题栏、标题按钮与页头都隐藏，移动指针后恢复；`unified` 的标题栏没有标题文字；侧栏项悬停出现浅底并放大、按下缩小、选中填满强调色且文字为强调色上的前景色（真实指针）。
- **gallery（WinUI 3 实测）**：运行时强调色由绯红换成群青、明暗换成深色，控件与画布随之变化；工具栏的图标按钮、菜单、分组竖线、开关按钮与搜索框；Ctrl+K 聚焦搜索框，输入后结果浮层列出匹配的图案；Ctrl+Alt+I 与 `ui.set_bool` 打开详情面板，海报墙从 3 列让到 2 列再到 1 列，取色铺底按海报平均色渐隐、运行时海报图片（`file:` 地址）显示；收藏后出现轻提示；卡片静止、悬停（放大到 1.03、投影、星标显现）、按下（0.97）三种状态（真实指针，屏幕截取）；切到“图案”页时卡片按 `stagger` 依次出现（逐帧截取）；Ctrl+O 打开系统的打开面板、Esc 取消；关于窗口以 `ContentDialog` 打开，按钮与 CLI 都能关闭它；改变窗口宽度后海报墙页边距随尺寸档变化。
- **画框页（WinUI 3 实测）**：`appearance: dark` 的部分在浅色窗口中为深色，其中分体按钮的浮层也是深色并显示菜单刚设的字号 18；静置后视频铺满、圆角 20 收为直角，指针移动后恢复；`PopoverButton` 的滑块在浮层打开时挂载、关闭后卸载；关闭说明后文字原地淡出再离开；`ui.set_file` 给拖放区：非地址返回 `invalid_url`，种类不符返回 `unsupported_file`，视频的 `file:` 地址存入字段并播放。
- **胶片页（WinUI 3 实测）**：初始停在字段指定的一格正中，正中一格放大、两侧变小变淡；“最后一个”“第一个”滚到首尾并停在正中；滚轮滚动后吸附到最近一格，字段随之更新；时间轴的刻度、名称与“现在”竖线；`Adaptive` 在 `compact`、`medium`、`expanded` 之间切换；上拖 80 个单位后 `Swipe` 的数值由 0.5 变为 0.7667（= 0.5 + 80/300）；三格带 Ctrl 的滚轮让 `Pinch` 的卡片宽度由 180 变为 242.97（= 180·e^0.3）。
- **堆叠中的轨道与错峰顺序（WinUI 3 实测）**：“上下拖动”面板是 `Stack` 中的 `Canvas(height: 120)` 与一条轨道，轨道里的水位画布没有自己的高度。窗口高 900 时，量快照的像素：面板高 180 像素（150% 下为 120 点），水位底色同样从面板顶到底铺满 120 点，多出的高度留在面板下方，`Stack` 没有变高；水位宽度在 `level` 为 0.5 时是面板宽的 423/846，上拖到 0.7667 后是 649/846（0.767）。轨道的铺满角色只沿轴向外传，不让外面的 `Stack` 跟着撑开，在 `Stack` 中按其他层给出的尺寸铺满。错峰的轮次在构建各行时按条目顺序分配，与元素何时 `Loaded` 无关；逐帧截取中海报墙依次为 Gingham、Tartan、Madras、Glen、Houndstooth、Windowpane。
- **系统 XAML（Islands）实测**：gallery 的 Windows 10 外观、运行时强调色（绯红、群青）、深色（标题栏经 DWM 变深）、详情面板、取色铺底与海报、轻提示、胶片页（滚到最后一格、轨道、自适应、拖动区）与 Windows 10 样式的关于对话框；planner 的归档项、大标题、标签、主要按钮、画布与标注；专注模式静置后标题栏（`WS_CAPTION`）与页头都隐藏。
- 修正的问题：共享元素的新元素插到了错误位置（离场钩子同步完成时区域计数尚未提交，改为先提交再离场）；弹出动效的缩放中心取了整行的中点（改为按内容实际大小）；卡片中铺满的画布没有宽度（铺满角色改为按方向递归，内容铺满时按钮拉伸）；搜索结果浮层判断焦点用错了属性（`AutoSuggestBox` 的焦点在内部文本框，改为跟踪获得与失去焦点事件）；居中停靠漏算了内容的边距；对话框窗口没有自己的 `Window`，设置标题按钮主题时访问了空对象；Islands 下窗口外框变化时系统在原位置补发的指针移动把静置重置了（改为比较光标位置）。

未验证（本轮）：

- `reflow`：仓库中的示例没有用到，未实测。
- 真实的文件拖放、触摸屏与触控板的原生捏合（只用带 Ctrl 的滚轮模拟了触控板捏合）、触摸拖动。
- 经命令行或再次启动交来的文件（`opens`），http(s) 的运行时图片与取色。
- 系统关闭动画时的退化、`focus: none` 与禁用状态的样式、键盘焦点的外观。
- Islands：专注模式的恢复（测试时 PC 正被使用，停止了模拟输入）；真实指针下的悬停与按下、快捷键、搜索结果浮层、错峰与共享元素的逐帧画面。这些与 WinUI 3 是同一份代码，但没有在 Islands 下逐项实测。
- showcase 只验证到构建通过，本轮没有重新运行。
- 最后加入的三处只验证到编译通过：分体按钮旁边部分的无障碍名称、分体按钮的快捷键切换开关、禁用控件与文本框有焦点时快捷键不触发。

未完成的验证：

- 没有 Windows App Runtime 的干净 Windows 10 与 Windows 11。本机的 Islands 结果来自强制回退。
- 只装有系统组件版运行时（`Microsoft.WindowsAppRuntime.CBS`）时，非打包应用能否经引导程序使用它。
- 键盘与手动操作（Ctrl+W、Esc、Alt+←、输入法）、讲述人与完整的键盘遍历、高对比主题、多显示器、ARM64、远程网络源（HLS 只经本机 HTTP 测过）。

2026-10-19，页面级布局、场景与控件形态，同一台 PC（Windows 11 25H2，150% 缩放），经 SSH 构建、以计划任务把应用启动到桌面会话，由生成的 CLI 驱动，画面来自应用自己的窗口快照：

- **测试**：`bern-backend-winui3` 的测试在 macOS 上通过，Windows 上新增的 `layout_features_generate_for_winui3`（给 showcase 加上场景、手风琴、换行、格子网格、模态、表单版式、纯图标标签与菜单、胶囊控件与搜索项）用 MSVC 编译两套前端通过；`cargo clippy -p bern-backend-winui3 --all-targets -- -D warnings`（另外忽略 `bern-target-api` 中已有的 `for_kv_map`）与 `cargo fmt --check` 通过。
- **巡览应用**：一个只在本机临时目录中的巡览应用覆盖除 `window_landscape` 外的全部 29 项与 `Stack` 样式，`bern check --target windows` 没有错误与警告，`bern build --target windows` 构建成功。
- **真实应用**：加入 `text_formats` 与上面的修正后，用于验收的媒体类应用 `bern check --target windows` 只剩 `landscape` 的警告，`bern build --target windows` 在 PC 上构建成功并启动（WinUI 3 与强制回退的 Islands）。对照设计稿实测引导页：960 × 600 的模态卡片中插画区 480 × 528、底栏 72 高贴底；“跳过”是标准按钮、“开始”是强调色按钮，都是 Fluent 的 4 圆角（设计稿 Fluent 一栏的按钮不是胶囊）；200 尺寸的动画 Logo 透明地居中在格纹插画上。其余页面未逐页对照。
- **对照设计稿**：用无头浏览器把设计稿原型切到 Fluent 2 桌面、逐屏截图，与应用自己的窗口快照对照，并用元素树核对位置。改动后实测：引导四步（模态为系统 `ContentDialog`，底栏贴底且有分隔线，“明暗”分段靠前缘，Logo 透明居中）；首页（顶栏为系统搜索框与“打开”，“继续播放”为品牌色白字，“也在看”三行对齐）；搜索面板（系统 `Flyout` 盖在搜索框上向下展开，列出番剧与继续播放）；播放页（无底色的图标按钮、铺满的品牌色进度条、带标题的“弹幕”分体按钮、品牌色圆形播放键、五个控制按钮）；播放中的字幕菜单（弹出层快照为深色底白字）；通知为系统 `InfoBar`；媒体库海报墙紧接筛选行。随后对照了新番（结构一致；展开列的描边改为 Fluent 的细描边）、番剧详情（前缘返回为无底色按钮）、迷你播放器、下载、设置窗口（通用、可选功能及其预览、插件）。剩下的差别属于应用的声明（写死的苹果圆角、侧栏宽度、个别控件的选择），已交给应用一侧。屏幕上的菜单底色以快照为准，没有屏幕截取。
- **文本格式**：一个本机临时测试应用（复数、日期、时长、显示字段的枚举标签）在 WinUI 3 与 Islands 下经 CLI 驱动，0／1／3 次添加后依次为“no files”“1 file”“3 files”，“Today is Monday, October 12, 2026”，3 次后“3:04 used (0:03:04)”与“3 large files”，保存后“Saved Tides #1.mp4 on Mon, Oct 12”；切到简体中文后为“3 个文件”“今天是 2026年10月12日 星期一”“已用 3:04（0:03:04）”“3 个大文件”“已保存 Tides #1.mp4，10月12日 周一”，与 SwiftUI、Web、GTK 相同。生成测试 `text_formats_generate_for_winui3` 在 Windows 上编译两套前端。
- **WinUI 3 实测**：侧栏顶栏的模板图标与标题、“场景”项的 `InfoBadge` 计数 3、无标题的详情页头（前缘为返回按钮，后缘为工具栏文字与搜索面板入口）、`text { title: heavy 30 }` 的大标题；缩放场景（640×360 的卡片中 1280×720 的场景缩为一半，`Positioned` 的图片带 32 圆角）、跑马灯随时间移动；开关“铺满”后图片按 `smooth` 移到铺满、圆角收为直角，“大字”后跑马灯的绑定字号变大，`Timer` 每秒计数加一；`window resize` 到 1100×500 后出现“窗口很矮”（`window.short`）；关掉引擎开关（`idle_enabled: app.on`）后静置 4 秒 `window.idle` 仍为假，打开后静置 5 秒变为真。布局页：标题与说明按基线对齐；手风琴第二列展开、其余各 60 宽加 8 间距，点“四”后展开列移到第四列（收起列仍为 60 + 8，截图量得）；胶囊按钮换行；格子网格；只给 `aspect` 的图片铺满宽度并带 20 圆角。控件页：控件组胶囊中按钮无底色、44 的圆形图标按钮、36 的圆形图标菜单、纯图标标签、`nudge_key` 卡片、横向撑满的胶囊主按钮、受控的开关、滑块与分段选择、显式选项的胶囊下拉（按内容宽度）；当时 `shape: capsule` 还画成胶囊，之后按设计稿改为 Fluent 自己的圆角，见下一条。搜索面板：`ui activate` 打开，背景变暗，560×420 的卡片，输入“布”后列出带图标、说明与提示的结果；`ui activate` 结果“表单”后面板关闭、页面切到表单。表单页：plain 分节从页面顶部开始，带 `padding_x: 20, padding_y: 4` 的分节行高 48。模态：菜单项打开后背景变暗、卡片 520×320 居中，“关闭”关掉。
- **系统 XAML（Islands）实测**：同一应用强制回退后，上述侧栏（计数显示为“场景 3”）、场景、手风琴（展开列与收起列）、控件页与 `window.short` 相同。
- 修正的问题（实测中发现）：手风琴的列一开始不可见（圆角裁剪的几何在第一次排布时尺寸为 0，改为每次排布时按列的大小设置，通用的圆角裁剪也先用元素已有的大小）；胶囊下拉被拉满整行（改为靠前缘按内容宽度）。
- 未验证：方向键与回车在搜索面板中的选择、指针悬停高亮（没有向桌面发送输入）；跑马灯文字阴影、`grow` 过渡与 `appear_delay`、`nudge_key` 晃动、`reduce_motion` 的逐帧画面；真实应用在 Windows 上的构建与运行。

2026-10-19（续），用于验收的媒体类应用没有页面时的空画框（纯粹模式），同一台 PC（150% 缩放，WinUI 3），窗口快照与元素树：

- 1280 × 800 的场景按窗口的宽与高中较小的一方缩放（1265 × 792 的内容区中为 0.988），空画框、按钮行与最近播放行都在窗口内；`window resize` 到 1100 × 900 后场景随之缩小并居中。
- 最近播放的缩略图带 8 圆角，底部是灰色底轨与品牌色进度条；“继续上次”是胶囊里带“空格”键帽的按钮。
- 齿轮在标准高度的标题按钮下方，中心约在 (1097, 51)，右缘离窗口右边 150（设计稿为 (1112, 76)，右缘 150）；`apply-preset --preset Anime` 回到有侧栏的首页后页头回到原位，首页舞台与“也在看”的缩略图进度条正常。侧栏底部的头像为浅灰圆底（实测 226，背景 243）、中灰人形（122），肩部被圆裁住；「继续上次」的键帽为浅灰底（228，按钮底 251），图标、文字与键帽在按钮里垂直居中。
- 测试：`cargo test -p bern-backend-winui3 -- --test-threads=1` 在 Windows 上用 MSVC 编译两套前端通过（生成测试 11 项）。
- 与设计稿的差别：页头在所有页面都比设计稿高约 17 到 25（设计稿的标题区 48 高、工具栏 56 高，WinUI 的标题栏 32 高，页头紧接其下），纯粹模式的齿轮因此在设计稿位置上方约 25；`fill.strong` 在浅色下是 9.4%，设计稿的键帽与头像底是 6%。未验证：齿轮的点击（没有向桌面发送输入）；Islands 下没有 `TitleBar`，页头位置不变。

2026-10-20，标签选择器与换行工具条（Windows PC，WinUI 3，窗口快照与元素树转储）：

- 应用的设置 › 插件（设置窗口 1000 宽）：分段控件是 `SelectorBar`「市场 / 已安装 3」，靠左；五个 chip 是 `ToggleButton`，靠右，选中的「全部」为强调色填充，4 px 圆角，与原型 Fluent 版一致；两者竖直居中。
- 换行：应用的窗口到不了手机宽度，改用展示应用的「工具条」一节（窗口 720 宽）。`BERN_DUMP_TREE` 的元素树中 `FlowPanel` 宽 336：第一行是 `SelectorBar`（156 × 47）、`Spacer` 与靠右的「全部」「常用」（末端 681，正是行尾），「最近」换到第二行行首；chip 32 高，在 47 高的行里竖直居中。转储同时发现 `Spacer` 的竖直位置按零高度算了偏移，已改正（SwiftUI 的 `BernFlow` 同样改正）。

2026-10-20，侧栏的选中指示条（Windows PC，150% 缩放，WinUI 3 与强制回退的 Islands；`BERN_SNAPSHOT_BURST` 每 40 毫秒一张的窗口截取）：

- 测试应用：四个侧栏项，样式与用于验收的媒体类应用相同（悬停 1.03、按下 0.94、`settle: jelly`），另加 `selected: scale(0.94)`，让换选中时新旧两项的缩放不同，不必向桌面发指针输入。用 CLI 的 `ui select`（运行时写 `SelectedItem`）与 UI 自动化的 `SelectionItemPattern.Select`（导航项的自动化对象由此调用 `Invoke`，与点击走同一条路，经过 `ItemInvoked`）各换一次。
- 改动前（WinUI 3）：两种方式都是两段竖条同时在动：旧项的条向下缩、新项的条从上往下长，20 到约 540 毫秒两条一直同时可见，横向错开约 3 个物理像素，与报告的截图相同。
- 改动后：WinUI 3 下两种方式都播放滑动动画：到 180 毫秒旧条在旧项内向下伸长，220 毫秒旧条淡出、新条从新项顶边进入并收拢，约 300 毫秒落定；行的 9 圆角把条裁在各自的行内，所以看上去是旧条先走、新条后到，两条始终在同一条竖线上。Islands 下行不裁剪，一条竖条从旧项伸到新项再收拢，也在同一条竖线上（改动前的 Islands 没有测）。
- 测试：`cargo test -p bern-backend-winui3 -- --test-threads=1` 在 Windows 上用 MSVC 编译两套前端通过（库测试 12 项，含检查选中改变前后放下与恢复的 `sidebar_items_rest_while_the_selection_changes`；生成测试 11 项）。
- 未验证：真实指针的悬停与按下（没有向桌面发送输入；它们的缩放与上面的选中缩放在同一个状态层里，由同一处放下）；用于验收的媒体类应用本身（PC 上它正开着，没有重新构建）。

## 未完成与已知限制

- **使用 Windows 自带的运行时不是微软公开说明的用法。** 微软的部署指南只说明非打包应用要自己部署运行时；`CBS.2` 是 Windows 给自己组件用的副本，第三方程序能用是实测结果，以后的 Windows 更新可能改变它的名称、内容或可用性。所以其他程序装的 2.x 总是优先。哪些 Windows 11 版本自带 `CBS.2` 也只在 25H2（26200.9457）上确认过。
- 只编译 WinUI 3 时，提示框的“是”只验证到会打开安装程序的地址，测试中没有实际下载安装。
- 应用按 2.5.1 编译、接受 2.0 起的运行时：只在 2.2 与 2.5.1 上实测过，没有 2.0、2.1 的机器。
- 未提供 MSIX 打包。未签名的新程序可能被安全软件的启发式规则误判，正式分发需要签名。
- 回退前端没有 `SelectorBar`、`InfoBar` 与 `TitleBar`，分别以按钮组、自绘错误条与系统标题栏代替。
- HDR 图片只在 SDR 显示器上实测过：测试用的 PC 没有 HDR 显示器，HDR 模式下的亮度未验证。
- HDR 视频只在 SDR 显示器上实测过：HDR 模式下由媒体引擎与系统输出 HDR，测试用的 PC 没有 HDR 显示器，未验证。
- 同一个播放器同一时间只在最新附着的视图中显示画面，较早的视图只显示外框。
- 页面工具放在页头，没有使用 `CommandBar`；`Menu` 的菜单项不呈现 `destructive` 角色。
- 品牌色的浅色与深色档位按与白色、黑色混合近似生成。
- `chrome: unified` 在 WinUI 3 下不改变标题栏（标题照常显示）；Islands 下仍是系统标题栏。
- 选中侧栏项的 `indicator` 不支持：`NavigationView` 只画自己前缘的指示条；侧栏项的样式也没有 `shadow`（与其他目标相同，检查时报错）。
- `reflow` 只在 `Column`、`Row`、`Grid` 与 `ScrollView` 的面板中生效，其他容器中写了不起作用，构建时没有警告。
- 系统 XAML 的导航项没有 `InfoBadge`，侧栏计数是名称后面的数字。
- `Row(align: baseline)` 按第一个文字块对齐；没有文字的子元素保持顶对齐。基线变化后要多一轮布局才计入多占的高度。
- `window_landscape` 未声明：桌面窗口没有设备方向，`Window(landscape:)` 检查时给出警告。
- `shape: capsule` 在 Windows 上不画成胶囊，保留 Fluent 控件自己的圆角（设计稿如此）；只有强调色的定尺寸 `IconButton` 是圆形。
- 动画 SVG 是窗口之上的子 HWND：不随祖先的透明度与滚动以外的裁剪变化，浮层菜单等 XAML 图层会被它挡住；弹出动画中只跟随位置与大小。
- 只有变体、没有值的枚举标签（如选项列表）写出时，标签显示的字段为空。
- 共享元素只在同一窗口内配对，用 Bern 自己的覆盖层实现，没有使用 `ConnectedAnimationService`。
- 搜索结果显示在搜索框下方的 `Flyout` 中，不是 `AutoSuggestBox` 自己的建议列表（结果是任意组件）。
- 运行时强调色通过修改主题字典中的强调色画刷实现，各档颜色按与白色、黑色混合近似生成，不是系统按强调色算出的调色板。
- 对话框窗口是 `ContentDialog`，同一时间一个窗口上只能显示一个；不能移动、调整大小或最小化。
- `Drop(opens: true)` 不登记文件关联：只有命令行与再次启动时交来的文件会到达。
- 触控板捏合依赖系统把它转换为带 Ctrl 的滚轮（精确式触控板的默认行为）。
- 其余与 macOS 相同的限制见 [macOS SwiftUI 实现记录](macOS-SwiftUI.md)“未完成与已知限制”中与平台无关的条目。
