# Android Compose 实现记录

- 日期：2026-10-13
- 依据：[Bern 架构设计 v0.28](../Design.md)，[Android 适配规范](../Adaptation/Android.md)
- 工具链：Rust 1.99.0（`aarch64-linux-android`）；Android NDK 29.0.14206865；Android Gradle 插件 9.3.3、Gradle 9.5.1、JDK 21（Android Studio 自带的运行时）；Kotlin 2.4.21 的 Compose 编译器插件；Compose BOM 2026.09.00（Compose 1.12.1、Material 3）；Media3 1.11.1；compileSdk 37、targetSdk 36、minSdk 26
- 验证环境：小米 MIX 2S，LineageOS，Android 15（API 35），arm64-v8a，1080 × 2160、440 dpi（约 393 dp 宽），经 USB 调试连接 M4 Mac

本文记录 `compose + android` 目标的实际完成情况。目标模块是新的 `bern-backend-compose`：生成 Kotlin 与 Jetpack Compose 代码和一个 Gradle 工程，引擎经 NDK 编译后在应用进程内运行。展示应用（`examples/showcase`）、`examples/planner` 与 `examples/hello` 只增加了 `[targets.android]`，声明、协议与业务核心不变。

## 完成情况

| 设计要求 | 状态 | 说明 |
| --- | --- | --- |
| 生成代码可构建 | 完成 | `bern build --target android` 生成 Kotlin、Gradle 工程与胶水 crate，用 NDK 编译引擎库、Gradle 打包 APK，并为这台电脑构建 CLI 与 MCP 服务器；生成测试检查三个示例的 Kotlin、清单、构建脚本与字符串表 |
| 引擎在应用进程内 | 完成 | 引擎、会话运行时与 JNI 桥编译为 `libbern_app.so`；值按与 C ABI 相同的二进制布局传递 |
| 不支持原生多窗口时用虚拟窗口 | 完成 | 一个 Activity：主窗口铺满屏幕，其他窗口叠在上面，系统返回关闭最上面的窗口；窗口事实报告为 `Virtual` |
| 屏幕旋转、键盘、安全区域 | 部分 | Activity 自己处理配置变化，窗口与草稿不因旋转重建；内容避开系统栏与输入法（edge-to-edge 下按 insets 留出位置）；旋转未实测 |
| 状态可订阅；带参数命令与错误返回 | 完成 | Kotlin 运行时与 Swift 运行时一一对应：引擎状态、会话草稿、查询、关闭协议、外部会话命令 |
| 深浅色与平台主题 | 完成 | Material 3 配色；Android 12 起用壁纸的动态取色；深浅色跟随系统 |
| 运行时切换语言 | 完成 | 字符串表是 assets 中的 JSON，切换语言不重建界面 |
| Video 原生呈现 | 完成 | Media3 的 ExoPlayer 执行端与不带控件的 `PlayerView`，命令驱动的控制条 |
| 样式覆盖、动效、`Canvas` 与标注 | 完成 | 与 SwiftUI 相同的组件与状态；出现、消失、重排与共享元素动效 |
| 本地控制通道 | 完成 | 抽象套接字，`bern run` 用 `adb forward` 接到这台电脑 |
| 打包与签名 | 完成 | `bern package` 生成发布版 APK 与 AAB，发布密钥从环境变量读取 |

## 工程结构

| 位置 | 内容 |
| --- | --- |
| `crates/bern-backend-compose/src/lib.rs` | 目标：描述、检查、生成、构建与打包步骤 |
| `src/kotlin.rs` | Kotlin 生成器：值类型、引擎状态、会话草稿、每个组件与窗口一个 composable、Activity |
| `src/style.rs` | `style!` 在 Compose 上的能力与 `BernStyle` 声明 |
| `src/project.rs` | Gradle 脚本、清单、主题、应用名与字符串表 |
| `src/glue.rs` | 引擎库（cdylib）与这台电脑上的 CLI、MCP 服务器 |
| `src/toolchain.rs` | 查找 SDK、NDK、Gradle 与 JDK |
| `templates/BernRuntime.kt` | 编解码、宿主、窗口、控件登记、外部会话命令、窗口根 |
| `templates/BernViews.kt` | 标准组件的 Material 3 实现、图标映射、查询 |
| `templates/BernStyle.kt` | 颜色角色、样式覆盖、弹簧、过渡、共享元素、`Canvas` 与标注 |
| `templates/BernMedia.kt` | 媒体执行端与视频视图 |
| `crates/bern-ffi/src/android.rs` | JNI 方法与回调 |

生成的 Gradle 工程在 `target/bern/android/project/`。引擎库复制到 `app/src/main/jniLibs/arm64-v8a/`，随包资源镜像到 `assets/bern/resources/`，运行时首次启动（以及每次更新后）把它们解压到应用的文件目录，引擎按文件路径读取。生成的 Kotlin 在以包标识命名的包中，运行时在 `dev.bern.runtime` 中。

## 引擎与 JNI

Kotlin 对象 `BernNative` 声明 C ABI 的每个函数，`bern-ffi` 的 `android` 模块以 JNI 方法实现它们，只做字节数组的复制；`BernNative.start` 由生成的胶水实现，因为它要构造应用自己的引擎。引擎的回调（状态、命令答复、外部会话命令、媒体请求与状态、查询）在引擎或通道线程上调用 Kotlin 的 `BernBridge`，线程以守护线程方式附着到 Java 虚拟机，`BernBridge` 再把工作转到主线程。回调抛出的异常被清除，不会让引擎线程崩溃。JNI 只用 `jni-sys` 的函数表，不引入更大的绑定库。

## 界面

生成器与 SwiftUI 生成器一一对应，组件改用 Material 3：

- **窗口根**：每个窗口一个 `Scaffold`，顶栏显示窗口标题，或里面的页面声明的标题（`Form` 的标题、分栏的选中项）、返回与工具栏操作。被叠在上面的窗口不响应系统返回。叠在主窗口上的窗口左上角是关闭按钮，经关闭协议请求关闭。
- **分栏**：宽度不小于 600 dp 时侧边栏（280 dp）与详情并排；更窄时先显示侧边栏列表，选中后进入详情，系统返回与顶栏的返回箭头回到列表。外部入口选择侧栏项时同样进入详情。
- **页签**：底部 `NavigationBar`；工具栏中的选择器显示为顶栏里的下拉菜单，因为顶栏只有一行高。
- **控件**：`Button` 默认是色调按钮，`prominent` 为实心按钮，`plain` 为文字按钮；`Switch` 是整行可点按的开关或复选框；`Picker` 按样式是下拉框、分段按钮或单选列表；`TextInput` 是带标签的描边输入框；`Slider` 可带两端图标。
- **图标**：Bern 的图标名（SF Symbols 命名）映射到 Material 图标：Material 核心图标集里有的直接用，常用而核心集没有的（暂停、音量、亮度、网格、归档、文件夹、调色板等）由运行时按 Material 图标的路径数据绘制，没有对应的显示一个中性圆点。
- **导航项按条件出现**：`if`、`match` 与 `ForEach` 中的侧栏项与页签照常生成，选中项消失时回到 `fallback` 或第一个可见项。

## 样式与动效

- **样式覆盖**：`Button`、`IconButton` 与 `SidebarItem` 支持与 SwiftUI 相同的属性与状态。只有变换的样式套在 Material 控件上，由控件自己的交互状态驱动；改变外观的样式由 Bern 绘制。颜色角色映射到 Material 配色。
- **出现与消失**：条件分支用 `AnimatedContent` 切换，新旧分支在同一位置交替，进入与离开各用分支第一个元素声明的过渡；列表中在窗口第一次显示之后加入的元素播放出现过渡。列表项移除时没有消失过渡。
- **重排**：声明了 `reflow` 的容器以弹簧动画改变尺寸。
- **共享元素**：窗口根是共享过渡布局，带相同 `hero` 键的元素在分支切换时从原位置移到新位置；元素按自身尺寸布局、再缩放到移动的边界，所以文字在过渡中不重新换行，两者交叉淡化。
- **系统关闭动画时**（开发者选项或无障碍中的“移除动画”），变换保持静止，过渡改为 150 毫秒的淡入淡出。
- **`Canvas` 与标注**：Compose 的 `Canvas` 绘制形状、数值序列与图案；`Text(mark:)` 从文字的行布局取得每行位置，在文字后面画底纹。

## 窗口外观

- `chrome` 在手机上不起作用：手机窗口没有标题栏，顶栏是导航的一部分（与 iPhone 相同）。
- `immersive` 为真时隐藏顶栏与系统状态栏、导航栏，从屏幕边缘滑动可暂时显示系统栏；为假时恢复。只有最上面的窗口控制系统栏。
- `idle_after`：触摸、指针与按键都会重新计时，计时到期后 `window.idle` 为真。

## 本地控制通道

应用是自己的系统用户，开发者的工具不能与它共用目录，所以 Android 上通道监听抽象套接字 `bern.<包标识>`。对端检查允许应用自己的用户和 `adb` 所在的 shell 用户（开发版系统上为 root），其他应用连不上。`bern run` 在应用启动后执行 `adb forward`，把这台电脑上应用目录里的 `android.sock` 接到这个抽象套接字，CLI、JSON 客户端与 MCP 服务器（为这台电脑构建）因此和桌面一样找到它。再次运行时先移除旧的转发再重建，否则旧转发不会重新创建套接字文件。

## 媒体

ExoPlayer 是执行端：每个播放器一个 ExoPlayer，按媒体契约执行载入、播放、暂停、跳转、音量与静音，并按源版本报告阶段、位置、时长与错误；静音由执行端记住，播放器音量设为 0。视频视图是不带控件的 `PlayerView`，所有播放变化都经 Bern 的媒体命令。能力描述声明：一个播放器同一时刻只在一个视图中显示；后台继续播放需要媒体服务（`MediaSessionService`），应用目前不声明，所以声明为不支持。

## 构建、运行与打包

```bash
rustup target add aarch64-linux-android
bern build --target android
bern run --target android
bern package --target android
```

- `bern doctor` 检查 Android SDK、NDK、Gradle 与 Rust 的 Android 目标。工具的位置：`ANDROID_HOME`（或 Android Studio 的默认位置）、`ANDROID_NDK_HOME`（或 SDK 中最新的 NDK）、`BERN_GRADLE`（或 `PATH` 中的 `gradle`、Gradle 包装器下载的 9.4 以上版本）、`BERN_JAVA_HOME`（或 Android Studio 自带的运行时、`JAVA_HOME`）。缺少时 `bern build` 跳过 Android 并说明要装什么。
- 构建步骤：用 NDK 的 clang 为 `aarch64-linux-android` 构建引擎库；复制到 `jniLibs`；镜像随包资源；Gradle 构建 APK；为这台电脑构建 CLI 与 MCP 服务器。第一次构建时 Gradle 从 Google 与 Maven Central 下载插件与库。
- `bern run --target android` 要求恰好连着一台设备（真机或模拟器），安装 APK、启动 Activity、转发控制通道并显示应用的日志，应用进程结束时返回。
- `bern package` 在 `target/bern/dist/` 生成 `<应用>-<版本>-android.apk` 与 `.aab`。Gradle 从 `BERN_ANDROID_KEYSTORE`、`BERN_ANDROID_KEYSTORE_PASSWORD`、`BERN_ANDROID_KEY_ALIAS`（可选 `BERN_ANDROID_KEY_PASSWORD`）读取发布密钥，密码不写进任何文件；没有设置时用调试密钥签名，并提示不能上架。

## 验证记录

2026-10-12 至 13，小米 MIX 2S（Android 15），由生成的 CLI、`adb` 截图与录屏驱动：

- **构建与测试**：`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`（另对 `bern-ffi` 与 `bern-transport-ipc` 按 `aarch64-linux-android` 运行）、`cargo fmt --check` 通过；hello、planner、showcase 的 Kotlin 由 Gradle 编译通过。
- **hello**：标题栏与“你好，世界！”；系统语言为简体中文时界面为简体中文；深色跟随系统；`hello hello wave` 从电脑改变了手机上引擎的状态。
- **planner**：模块的页面在侧栏中；带样式的侧栏项（选中时填充强调色）与“改名”按钮；`Canvas` 的格纹、面积与折线随“记一笔”更新；文字标注；“在侧栏显示归档”打开后侧栏出现归档项，在归档页关闭后选中回到概览。录屏逐帧确认：新加的标签从 0.4 倍弹出、略微过冲再回落，后面的按钮平滑让位；打开“大标题”时标题在两种字号间放大，文字不被裁切，下方内容平滑下移。专注模式下静置后顶栏与系统栏隐藏，点按后恢复。
- **showcase**：冷启动显示侧栏列表（搜索框、分组、图标）；文字、按钮（含工具栏按钮）、开关与滑块（工具栏的“全部”下拉）、输入框、图片与布局（适配方式与 `Grid`）、列表与详情（`Query` 结果 5 项、推入链接）各页正常；视频页从 CLI 播放精选片段，状态为 `Playing 0:02 / 0:08`，控制条同步；`window open --kind settings` 打开设置窗口（语言、开关、输入框、底部页签），`window list` 报告两个 `Virtual` 窗口；设置窗口叠在详情页上时，第一次系统返回只关闭设置窗口，第二次回到侧栏列表；`ui set-language --language en` 后界面全部变为英文。
- **控制通道**：`ui inspect`、`ui select`、`ui set-bool`、`ui activate`、`window open`、`window list` 经 `adb forward` 正常。

## 未完成与已知限制

- 只构建 arm64-v8a。32 位设备与 x86_64 模拟器的引擎库未构建。
- 宽度不小于 600 dp 的布局（平板、折叠屏展开、横屏）未实测；只在约 393 dp 宽的手机上验证。
- 屏幕旋转未实测。
- 窗口事实不报告窗口的位置与尺寸（为 0）。
- 后台继续播放未实现（需要媒体服务）；一个播放器同一时刻只在一个视图中显示。
- HDR 图片与 HDR 视频未验证；这台手机的屏幕按 SDR 显示。
- 发布版没有开启 R8 缩减，hello 的发布版 APK 约 12 MB。
- 列表项移除时没有消失过渡。
- 带样式的侧栏项不支持方向键切换。
- 图标只覆盖常用的名字，其余显示圆点。
