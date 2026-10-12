# Windows WinUI 3 实现记录

- 日期：2026-10-11
- 依据：[Bern 架构设计 v0.19](../Design.md)、[Windows 与 Linux 适配](../Adaptation/WindowsLinux.md)
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
| 新目标不改解析器、公共 IR 与业务核心 | 完成 | `bern-backend-winui3`：C++ 生成器（`cpp.rs`）、构建胶水（`glue.rs`）与 `templates/cpp/` 下的运行时 |
| 具体框架 SDK 不是全局前置条件 | 完成 | C++ 运行时以模板随目标模块分发，只在生成的应用中编译；macOS 上 `cargo test --workspace` 与 clippy 不需要 Windows 工具链 |
| 运行时只使用系统依赖 | 完成 | 不需要 .NET；不随附 Windows App SDK；C 运行库静态链接，不需要 VC++ 运行库。有任何 Windows App Runtime 2（其他程序装的，或 Windows 11 自带的）时用它，没有时用系统 XAML |
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
| `Picker` | `segmented` 为 `SelectorBar`，其他为 `ComboBox` | `segmented` 为 `ToggleButton` 组，其他为 `ComboBox` |
| `Form` / `Section` | 滚动页面，卡片行（`CardBackgroundFillColorDefaultBrush` 与描边、圆角） | 卡片为 Windows 10 的列表底色，无圆角 |
| `Text` / `Label` | WinUI 字体样式（`TitleLargeTextBlockStyle` 等）；次要文字为 0.65 不透明度 | 系统字体样式（`HeaderTextBlockStyle` 等） |
| `Button` / `IconButton` | `prominent` 为 `AccentButtonStyle`；`plain` 与图标按钮以轻量样式资源设为透明；`destructive` 为危险色 | 同左，取 Windows 10 的对应资源 |
| `TextInput` / `Search` / `Switch` / `Slider` / `Menu` / `LanguagePicker` / `Query` / `Adaptive` / `Video` | 与 C# 版相同：`TextBox`/`PasswordBox`、`AutoSuggestBox`、`ToggleSwitch`/`CheckBox`、带图标的 `Slider`、`DropDownButton` 与 `MenuFlyout`、`ComboBox`、`ProgressRing` 与状态行、按可用宽度选择布局、`MediaPlayerElement` 与标准控制条 | 同左 |

组件库第一批在两种前端上用相同的代码：

| Bern | 两种前端 |
| --- | --- |
| `Image` | `Image` 显示 `BitmapImage`：从可执行文件旁的 `resources` 读出资源字节，写入内存流后回到界面线程 `SetSourceAsync`（未打包的应用没有 `ms-appx`）。两边都写时外面是声明尺寸的 `Grid`，`contain` 为 `Uniform` 并居中，`cover` 为 `UniformToFill`，`fill` 为 `Fill`；只写一边时为 `Uniform`，另一边按图片比例。`label` 为自动化名称，没有时 `AccessibilityView` 为 `Raw` |
| `Progress` | 有 `value` 时为标签下的 `ProgressBar`（`Minimum` 0，`Maximum` 为 `total`）；没有时为 20 px 的 `ProgressRing` 加次要文字 |
| `Grid` | 自定义面板 `GridPanel`：列数为放得下的最大值，列宽平分，每行与该行最高的单元格一样高 |
| `Stack` | 单格 `Grid`，子元素叠放，按 `align` 设对齐 |

HDR 图片（PQ 或 HLG 的 PNG、带增益图的 JPEG）不经过 XAML 的 `Image`：WinUI 3 的 `Image` 没有 HDR（[microsoft-ui-xaml#67](https://github.com/microsoft/microsoft-ui-xaml/issues/67)、[WindowsAppSDK#6291](https://github.com/microsoft/WindowsAppSDK/issues/6291)），系统 XAML 的文档也只说它能解码。Bern 用 `bern_picture_kind` 认出 HDR 图片后，在后台线程以 `bern_picture_decode`（`bern-image`，经 `bern-ffi` 的 `picture` 功能）解码成线性 scRGB，在界面线程上传为 Direct2D 的 32 位浮点位图，画进 `SwapChainPanel` 里 16 位浮点、scRGB 色彩空间的交换链；系统合成器直接合成它，两种前端用同一套代码（两套 SDK 头文件中的 `ISwapChainPanelNative` 同名不同 ID，预编译头里另行声明）。光的处理按窗口所在的显示器：DXGI 给出是否在 HDR 模式与峰值亮度，显示配置给出用户设的 SDR 亮度；Direct2D 的白电平调整把图片的 1.0 放到 SDR 亮度，HDR 色调映射把高光压到显示器峰值，SDR 模式下压到 SDR 白。带增益图的 JPEG 按显示器余量解码，SDR 显示器上就是作者给的 SDR 图。只要有 HDR 图片在屏幕上，每秒检查一次显示器，HDR 开关、SDR 亮度变化或窗口移到另一块显示器时重画，增益图按新的余量重新解码；设备丢失时重建设备。没有 Direct3D 或解码失败时退回 XAML 的 `Image`。实测（PC 为 SDR 显示器，两种前端相同）：增益图显示为作者给的 SDR 图（编码值 166 与 241，ImageIO 读基础图为 166 与 237）；PQ 图左右分别为 194 与 244，高光没有截断。

`Image` 与 `Stack` 外面各有一层 `Grid`，所以在会拉伸子元素的列中保持自身尺寸并靠前缘。在展示应用的“图片与布局”页按两种前端验证：效果与 macOS 相同，`contain` 的图片在框内居中；网格三列；CLI 修改音量与亮度后两个进度条立即更新。

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
- **预编译头**：C++/WinRT 头文件与运行时占了编译的绝大部分（WinUI 3 前端单独编译约 46 秒，其中解析头文件约 23 秒），而它们与应用无关。`bern_pch.h` 把两者都放进一个预编译头，每个配置一份，放在 `app/obj/debug/` 或 `app/obj/release/`（约 2.4 GB）。它只在运行时模板、编译选项、MSVC 版本、SDK 或路径变化时重新生成：脚本把 Bern 算出的运行时与选项指纹、`VCToolsVersion`、SDK 版本和各路径写进 `pch.key`，下次构建对比一致就直接复用。其余五个编译单元每次都用 `/MP` 并行编译，引擎库由 cargo 增量构建。
- **构建时间**（i5-13490F，16 线程）：

  | 情形 | 之前 | 现在 |
  | --- | --- | --- |
  | 首次调试构建（生成预编译头） | 约 60 秒 | 约 82 秒 |
  | 新项目：未改动、改界面、改引擎 | 约 60 秒 | 5–6 秒 |
  | 展示应用：再次构建 | 约 60 秒 | 约 9 秒 |
  | 首次 release 构建 / 再次 | 约 90 秒 | 约 95 秒 / 5 秒 |

产物位于 `target/bern/windows/`：`Showcase/`（`showcase.exe`、`Microsoft.WindowsAppRuntime.Bootstrap.dll` 与 `resources/`）、`bin/showcase.exe` 与 `bin/showcase-mcp.exe`，以及构建清单 `build.json`（记录前端、组件包版本与最低 Windows 版本）。

应用图标 `app.ico`（没有声明 `[app].icon` 时为 Bern 标志的圆形版，16 到 256 像素各一张 PNG）编入可执行文件的图标资源 1：资源管理器显示它。运行时按窗口所在显示器的 DPI 把它设为每个窗口的大小图标（任务栏、Alt+Tab、Islands 的系统标题栏），WinUI 3 的 `TitleBar` 另以 `ImageIconSource` 显示其中最接近标题栏尺寸的一张。

**开发辅助**（默认全部关闭）：

- `BERN_SNAPSHOT_DIR`：每次变化后输出两种快照。`<窗口>.png` 是 XAML 内容的渲染，打开的对话框和菜单另存为 `-popup<n>.png`；`<窗口>-window.png` 用 `PrintWindow` 截取应用自己的窗口，含标题栏、Mica 与视频画面。
- `BERN_APPEARANCE=light|dark`：覆盖系统外观。
- `BERN_FORCE_ISLANDS`：在装有运行时的机器上试用回退前端。
- `BERN_APP_RUNTIME=installed|inbox|none`：只用其他程序装的运行时、只用 Windows 自带的，或当作都没有（只编译 WinUI 3 时会出现提示框）。
- WinUI 的未处理错误与控件样式加载失败写到标准错误，然后照常结束进程。
- `BERN_DUMP_TREE`：把元素树和尺寸写到标准错误。
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

未完成的验证：

- 没有 Windows App Runtime 的干净 Windows 10 与 Windows 11。本机的 Islands 结果来自强制回退。
- 只装有系统组件版运行时（`Microsoft.WindowsAppRuntime.CBS`）时，非打包应用能否经引导程序使用它。
- 键盘与手动操作（Ctrl+W、Esc、Alt+←、输入法）、讲述人与完整的键盘遍历、高对比主题、多显示器、ARM64、网络与 HLS 源。

## 未完成与已知限制

- **使用 Windows 自带的运行时不是微软公开说明的用法。** 微软的部署指南只说明非打包应用要自己部署运行时；`CBS.2` 是 Windows 给自己组件用的副本，第三方程序能用是实测结果，以后的 Windows 更新可能改变它的名称、内容或可用性。所以其他程序装的 2.x 总是优先。哪些 Windows 11 版本自带 `CBS.2` 也只在 25H2（26200.9457）上确认过。
- 只编译 WinUI 3 时，提示框的“是”只验证到会打开安装程序的地址，测试中没有实际下载安装。
- 应用按 2.5.1 编译、接受 2.0 起的运行时：只在 2.2 与 2.5.1 上实测过，没有 2.0、2.1 的机器。
- 未提供 MSIX 打包、安装程序、应用图标与代码签名。未签名的新程序可能被安全软件的启发式规则误判，正式分发需要签名。
- 回退前端没有 `SelectorBar`、`InfoBar` 与 `TitleBar`，分别以按钮组、自绘错误条与系统标题栏代替。
- HDR 图片只在 SDR 显示器上实测过：测试用的 PC 没有 HDR 显示器，HDR 模式下的亮度未验证。
- `Video` 不显示 HDR：WinUI 3 的 `MediaPlayerElement` 目前不支持 HDR（[microsoft-ui-xaml#9238](https://github.com/microsoft/microsoft-ui-xaml/issues/9238)，微软工程师的解释是 UWP 靠 CoreWindow 判断显示器，Windows App SDK 应用没有 CoreWindow），要显示 HDR 视频需要让播放引擎直接输出到 `SwapChainPanel`，未实现。
- 页面工具放在页头，没有使用 `CommandBar`；`Menu` 的菜单项不呈现 `destructive` 角色。
- 品牌色的浅色与深色档位按与白色、黑色混合近似生成。
- 其余与 macOS 相同的限制见 [macOS SwiftUI 实现记录](macOS-SwiftUI.md)“未完成与已知限制”中与平台无关的条目。
