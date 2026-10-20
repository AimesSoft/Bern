# Android Compose 实现记录

当前组件接口见[组件参考](../Reference/UI.md)，逐项支持与近期验证范围见[平台能力表](../Reference/Platforms.md)。下文保留各阶段实现和验收记录。

- 日期：2026-10-14（页面级布局、场景与控件形态的能力于 2026-10-19 补齐，见同名一节）
- 依据：[Bern 架构设计 v0.30](../Design.md)，[Android 适配规范](../Adaptation/Android.md)
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
| `src/kotlin/` | Kotlin 生成器：值类型、引擎状态、会话草稿、每个组件与窗口一个 composable、Activity |
| `src/style.rs` | `style!` 在 Compose 上的能力与 `BernStyle` 声明 |
| `src/project.rs` | Gradle 脚本、清单、主题、应用名与字符串表 |
| `src/glue.rs` | 引擎库（cdylib）与这台电脑上的 CLI、MCP 服务器 |
| `src/toolchain.rs` | 查找 SDK、NDK、Gradle 与 JDK |
| `src/icons.rs` | 图标表：每个图标名对应的 Material 图标，生成 `BernIcons.get` 的分支，检查据此拒绝表中没有的名字 |
| `templates/BernRuntime.kt` | 编解码、宿主、窗口、控件登记、外部会话命令、窗口根 |
| `templates/BernViews.kt` | 标准组件的 Material 3 实现、图标映射、查询 |
| `templates/BernStyle.kt` | 颜色角色、样式覆盖、弹簧、过渡、共享元素、`Canvas` 与标注 |
| `templates/BernMedia.kt` | 媒体执行端与视频视图 |
| `templates/BernFeatures.kt` | v0.29 与 v0.30 的能力：明暗与强调色、快捷键、绑定的长度、阴影、带内容的按钮、开关按钮、工具栏搜索、侧面板、轻提示、分体按钮、运行时图片与取色、文件选取与拖放、滚动联动与停靠、晃一下、手势区、按比例摆放的轨道、浮层按钮 |
| `crates/bern-ffi/src/android.rs` | JNI 方法与回调 |

生成的 Gradle 工程在 `target/bern/android/project/`。引擎库复制到 `app/src/main/jniLibs/arm64-v8a/`，随包资源镜像到 `assets/bern/resources/`，运行时首次启动（以及每次更新后）把它们解压到应用的文件目录，引擎按文件路径读取。生成的 Kotlin 在以包标识命名的包中，运行时在 `dev.bern.runtime` 中。

## 引擎与 JNI

Kotlin 对象 `BernNative` 声明 C ABI 的每个函数，`bern-ffi` 的 `android` 模块以 JNI 方法实现它们，只做字节数组的复制；`BernNative.start` 由生成的胶水实现，因为它要构造应用自己的引擎。引擎的回调（状态、命令答复、外部会话命令、媒体请求与状态、查询）在引擎或通道线程上调用 Kotlin 的 `BernBridge`，线程以守护线程方式附着到 Java 虚拟机，`BernBridge` 再把工作转到主线程。回调抛出的异常被清除，不会让引擎线程崩溃。JNI 只用 `jni-sys` 的函数表，不引入更大的绑定库。

## 界面

生成器与 SwiftUI 生成器一一对应，组件改用 Material 3：

- **窗口根**：每个窗口一个 `Scaffold`，顶栏显示窗口标题，或里面的页面声明的标题（`Form` 的标题、分栏的选中项）、返回与工具栏操作。被叠在上面的窗口不响应系统返回。叠在主窗口上的窗口左上角是关闭按钮，经关闭协议请求关闭。
- **分栏**：宽度不小于 600 dp 时侧边栏（280 dp）与详情并排；更窄时先显示侧边栏列表，选中后进入详情，系统返回与顶栏的返回箭头回到列表。外部入口选择侧栏项时同样进入详情。
- **页签**：底部 `NavigationBar`；工具栏中的选择器显示为顶栏里的下拉菜单，因为顶栏只有一行高。
- **控件**：`Button` 默认是色调按钮，`prominent` 为实心按钮，`plain` 为文字按钮；`Switch` 是整行可点按的开关或复选框；`Picker` 按样式是下拉框、分段按钮、单选列表或一排 `FilterChip`（`chips`）；分段按钮不带对勾（设计的 `.c-seg` 只以底色标出选中段），标签按整行宽度量，计数等后缀不会被截掉；`TextInput` 是带标签的描边输入框；`Slider` 可带两端图标。
- **启动器图标**：没有声明或声明 PNG 时是 `mipmap-xxxhdpi/ic_launcher.png`；`[app.icon].android` 为 `res` 文件夹时原样并入应用的 `res`，自适应图标与主题图标（单色层）随之生效，有 `ic_launcher_round` 时清单加 `android:roundIcon`。
- **图标**：Bern 的图标名（SF Symbols 命名）映射到 Material 图标：Material 核心图标集里有的直接用，核心集没有的（暂停、音量、亮度、网格、归档、文件夹、调色板、媒体库、同步、插件、键盘、无线网络等）由运行时按 Material 图标（baseline 样式）的路径数据绘制。对照表只有一份（`src/icons.rs`），生成时写进 `BernIcons.get`；应用用到表中没有的名字时检查报错并指向声明，不再在运行时显示中性圆点（圆点只留给没有图标的位置）。表至少包含 Bern 图标集（`bern_target_api::icon_set::NAMES`）的全部名字，测试逐个核对。
- **导航项按条件出现**：`if`、`match` 与 `ForEach` 中的侧栏项与页签照常生成，选中项消失时回到 `fallback` 或第一个可见项。

## 样式与动效

- **样式覆盖**：`Button`、`IconButton` 与 `SidebarItem` 支持与 SwiftUI 相同的属性与状态。只有变换的样式套在 Material 控件上，由控件自己的交互状态驱动；改变外观的样式由 Bern 绘制。颜色角色映射到 Material 配色。
- **出现与消失**：条件分支用 `AnimatedContent` 切换，新旧分支在同一位置交替，进入与离开各用分支第一个元素声明的过渡；列表中在窗口第一次显示之后加入的元素播放出现过渡。列表项移除时没有消失过渡。单个元素的出现过渡（`BernAppear`）不用 `AnimatedVisibility`：一个从 0 到 1 的进度按声明的弹簧（及错峰、`appear_delay` 的等待）推进，所在层的透明度、位移与缩放由它算出，与 `AnimatedVisibility` 用同一弹簧分别推进各属性的效果相同；布局与 `AnimatedVisibility` 一样（同样的约束，取最大的子元素，放在左上角）。进度走完协程就结束，元素留在页面上时不再有等待的协程。
- **样式的状态变换**：缩放与位移合成一个四维值、透明度一个值，只在外观变化时启动弹簧，走完即结束；按下与悬停由按钮自己的交互源在发出时记下（`BernInteractions`），不再各开一个收集协程。阴影同样只在半径变化时运行。
- **重排**：声明了 `reflow` 的容器以弹簧动画改变尺寸。
- **共享元素**：窗口根是共享过渡布局，带相同 `hero` 键的元素在分支切换时从原位置移到新位置；元素按自身尺寸布局、再缩放到移动的边界，所以文字在过渡中不重新换行，两者交叉淡化。
- **系统关闭动画时**（开发者选项或无障碍中的“移除动画”），变换保持静止，过渡改为 150 毫秒的淡入淡出。这个设置在应用启动时与每次回到前台时读一次，存为一个状态；`reduce_motion` 的持有者也合成一个状态，各元素组合时只读这两个值。
- **`Canvas` 与标注**：Compose 的 `Canvas` 绘制形状、数值序列与图案；里面什么也不画、只用来占位置的 `Canvas` 生成为同样大小的 `Spacer`（没有绘制节点，也不带清空的语义节点；有 `label` 时保留它的语义）。`Text(mark:)` 从文字的行布局取得每行位置，在文字后面画底纹。

## 窗口外观

- `chrome` 在手机上不起作用：手机窗口没有标题栏，顶栏是导航的一部分（与 iPhone 相同）。
- `immersive` 为真时隐藏顶栏与系统状态栏、导航栏，从屏幕边缘滑动可暂时显示系统栏；为假时恢复。只有最上面的窗口控制系统栏。
- `idle_after`：触摸、指针与按键都会重新计时，计时到期后 `window.idle` 为真。

## 本地控制通道

应用是自己的系统用户，开发者的工具不能与它共用目录，所以 Android 上通道监听抽象套接字 `bern.<包标识>`。对端检查允许应用自己的用户和 `adb` 所在的 shell 用户（开发版系统上为 root），其他应用连不上。`bern run` 在应用启动后执行 `adb forward`，把这台电脑上应用目录里的 `android.sock` 接到这个抽象套接字，CLI、JSON 客户端与 MCP 服务器（为这台电脑构建）因此和桌面一样找到它。再次运行时先移除旧的转发再重建，否则旧转发不会重新创建套接字文件。

## 媒体

ExoPlayer 是执行端：每个播放器一个 ExoPlayer，按媒体契约执行载入、播放、暂停、跳转、音量与静音，并按源版本报告阶段、位置、时长与错误；静音由执行端记住，播放器音量设为 0。视频视图是不带控件的 `PlayerView`，所有播放变化都经 Bern 的媒体命令。能力描述声明：一个播放器同一时刻只在一个视图中显示；后台继续播放需要媒体服务（`MediaSessionService`），应用目前不声明，所以声明为不支持。

## 图片的读取

`Image(source:)` 的声明资源与引擎给出的图片（文件、`content://`、http(s)）都由 `BernPictures` 读取，不在主线程解码：

- 在 `Dispatchers.IO` 上先只读尺寸（`inJustDecodeBounds`），再按显示大小取 `inSampleSize`：图片自身像素除以仍能盖住显示尺寸的最大 2 的幂。显示尺寸由声明的宽高与屏幕密度算出，跟随图片比例的一边不计；没有声明宽高、铺满容器宽度的图片按布局量出的宽度读取，量出之前不读（一帧）。显示变大、已读的像素不够时重新读取，期间仍显示已读的那张。解码后调用 `prepareToDraw()`，在第一次绘制前交给 GPU。
- 最近读过的图片按位图字节保留 64 MB（`LruCache`，以 URL 为键），重新出现的图片立即显示；同一图片正在读取时共用那次读取。各图片的比例在进程内一直记住。
- 读到之前，框保持声明的大小：两边都给出或给了 `aspect` 时与读到后相同；比例要跟随图片时，声明资源在组合时只读文件头（每个文件一次），框不跳动；引擎给出的图片在比例未知时按海报的 2:3 占位。读不出文件头的声明资源不占位置，与此前一样。

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
- **基线配置文件**：生成的工程带 `app/src/main/baseline-prof.txt`，列出 Bern 运行时（`dev/bern/runtime`）与生成的界面代码（应用的包）的全部类与方法；Gradle 把它与 Compose 等库自带的规则一起编进 APK 与 AAB。商店安装时系统按它预先编译；发布版构建另把 Gradle 为 Android 12 及以上生成的 `.dm` 复制为 APK 旁的 `<应用>.dm`，`bern run` 在 Android 12 及以上的设备上用 `adb install-multiple` 把两者一起装上，装完即为 `speed-profile`（设备不接受时只装 APK 并提示）；调试版不编译，构建时删掉旧的 `.dm`。直接装 APK 时由 Compose 带的 ProfileInstaller 在首次启动后写入配置文件，系统在下次后台编译时才用上，在那之前这些代码由解释器与 JIT 运行。
- `bern package` 在 `target/bern/dist/` 生成 `<应用>-<版本>-android.apk` 与 `.aab`。Gradle 从 `BERN_ANDROID_KEYSTORE`、`BERN_ANDROID_KEYSTORE_PASSWORD`、`BERN_ANDROID_KEY_ALIAS`（可选 `BERN_ANDROID_KEY_PASSWORD`）读取发布密钥，密码不写进任何文件；没有设置时用调试密钥签名，并提示不能上架。

## 媒体类应用的能力（v0.29）

能力描述列出全部 `features`（`session_actions`、`reveal`、`bound_lengths`、`appearance`、`shortcuts`、`runtime_theme`、`button_content`、`toolbar_search`、`search_results`、`toggle_button`、`dialog_windows`、`runtime_images`、`picture_fills`、`file_pick`、`scroll_snap`），动效属性加上 `animate`、`scroll_scale`、`scroll_opacity` 与 `nudge`。

- **带内容的按钮与阴影**：`clickable` 加 `hoverable`，阴影随状态的弹簧变化；重复项中的控件经 `LocalBernItemKeys` 带上每一层的 key。触屏没有悬停，`reveal` 的内容一直显示。
- **明暗与强调色**：`appearance` 在子树里换一套 Material 配色；`WindowSet(accent:, appearance:)` 改全局配色，状态栏图标随明暗变色。
- **快捷键**：活动的 `dispatchKeyEvent` 先查当前屏幕上的控件登记的快捷键，`cmd` 是 Ctrl；按键也让 `window.idle` 重新计时。
- **绑定的长度**：`animateDpAsState` 跟随，值与回弹都不低于零。
- **视频圆角**：有 `corner` 时 `PlayerView` 改用 `TextureView` 才能被裁住。
- **对话框窗口**：居中的 `Dialog`，宽高按声明。分体按钮的浮层是 `DropdownMenu`；侧面板在手机上是底部弹层。
- **运行时图片与取色**：图片在后台读取，平均色由缩到 32×32 的整张图算出（取色只按 32 像素见方的大小解码，不进缓存）。图片的读法见下文“图片的读取”。
- **文件**：`pick` 用系统的文档选择器，按种类过滤并保留读取授权；`Drop` 用 `dragAndDropTarget`，并向系统请求拖入内容的读取权限；`opens: true` 在清单里登记 `VIEW` 与 `SEND` 意图，活动为 `singleTask`，运行中交来的文件经 `onNewIntent` 送到拖放区。用到网络地址时清单加上 `INTERNET`。
- **滚动联动与停靠**：带 `snap` 或 `position` 的 `ScrollView` 记录每一格在滚动内容中的位置（不随滚动变化），`graphicsLayer` 按每格离视口中心的距离缩放、变淡；停靠用自己的 `FlingBehavior`，按惯性算出落点后取最近的一格；首次显示时先定位到字段所指的一格，再开始跟随用户滚动。
- **晃一下与手势区**：`nudge` 用 `graphicsLayer` 放大再弹回；`Pinch` 从多指指针事件累计缩放（`calculateZoom`），`Swipe` 用竖向拖动手势，松手执行动作。

## v0.30 的四项扩展

能力描述的 `features` 加上 `rails`、`popover_button` 与 `size_classes`，动效属性加上 `stagger`。

- **错峰出现（`stagger`）**：声明了 `stagger` 的 `Column`、`Row`、`Grid`、`Stack`、`ForEach` 与 `List` 用 `BernStaggered` 给直接子元素提供一个时钟（`LocalBernStagger`）。带 `appear` 的直接子元素第一次组合时向时钟取一个轮次，延迟为轮次乘 `stagger`；同一次更新中组合的子元素（主线程空下来之前）按组合顺序依次取轮次，也就是子元素与各行的书写顺序（`Grid` 与非惰性的 `ForEach` 按顺序组合，惰性列表的首屏也按序号组合），之后计数归零，所以容器出现时、一次更新加入几个子元素时都从 0 开始依次排开。Compose 的弹簧没有延迟参数，出现过渡的动画规格由 `BernDelayed` 包一层：等待期间数值停在起点，元素透明地占着自己的位置，不让兄弟元素跳动。`ForEach` 不加布局，它的各行是所在容器的直接子元素；组件的根也可能是直接子元素（海报墙的卡片就是组件），所以应用中有 `stagger` 时，直接子元素中不出现的元素与组件的根在下面用 `BernUnstaggered` 清掉时钟，只有直接子元素取轮次。窗口首次显示不播放；系统关闭动画时不错峰，仍是 150 毫秒的淡入。
- **按比例摆放（`Rail`）**：`BernRail` 是一个自定义 `Layout`。写了 `at`、`from` 或 `to` 的元素外面包一层 `BernPlace`，以 `layoutId` 把比例交给轨道；比例夹到 0 到 1，有 `animate` 时由 `animateFloatAsState` 按弹簧跟随。轨道沿轴取它得到的最大长度（没有上限时取 `at` 子元素中最长的）；交叉方向的厚度是 `at` 子元素的尺寸、`from`/`to` 子元素自身的固有厚度与父容器给的最小值中最大的。`at` 子元素按自身大小、按 `anchor` 以起点、中心或终点对准那一点；`from`/`to` 子元素沿轴量得两点之间的长度，交叉方向最多到轨道厚度，没写交叉尺寸的 `Canvas` 因此铺满厚度。子元素在交叉方向居中，可以超出轨道，不裁切；从右到左的语言中横向轨道从右边量起（`placeRelative`）。轨道没有指针输入，触摸落到下面的层。`Stack` 中直接作为一层、只放着没写交叉尺寸的 `from`/`to` 子元素的轨道加 `matchParentSize()`，随整个 `Stack` 的大小铺满（与 Web 中“含无尺寸绘图的层铺满 `Stack`”一致）。
- **浮层按钮（`PopoverButton`）**：有图标时是 `IconButton`，只有文字时是色调按钮，在顶栏中是文字按钮；浮层是与分体按钮相同的 `DropdownMenu`，内容放在内边距 16、最小宽 240 的竖排中，只在打开时组合，所以其中的控件只在打开时出现在 `ui.inspect` 里。`presented` 绑定会话字段时，浮层跟随字段，点按按钮、点到外面关闭都写回字段；没有绑定时由按钮自己记住。控件角色是 `popover_button`，`ui.activate` 打开，值为 `open` 或 `closed`；`shortcut` 也打开它。
- **四档尺寸**：`[layouts]` 的断点在活动启动时写入 `BernHost.layouts`。每个窗口在 `Scaffold` 上量自己的宽度（`onSizeChanged`），量到之前用屏幕宽度（对话框窗口用声明的宽度）估计，所以首帧就是正确的档位；`window.compact`、`window.medium`、`window.expanded`、`window.large` 是窗口上下文的属性，读它们的表达式随窗口宽度换档。`Adaptive` 仍按所在位置的可用宽度（`BoxWithConstraints`）选择，某档没有自己的 `Layout` 时用更大一档的，没有更大的就用更小一档的；`ui.inspect` 中的值是宽度所在的档。
- **图标**：`chevron.left` 映射到 Material 的左箭头（胶片页紧凑布局的“第一个”之前显示为圆点）。
- **构建**：输出目录 `target/bern/android` 被这里构建的每个应用共用，Gradle 前用 `Mirror` 只留下本应用的 Kotlin，另一个应用留下的生成文件不再被编进来（之前运行时接口变化后，留下的旧文件会让编译失败）。

## 页面级布局、场景与控件形态的能力（2026-10-19）

能力描述补上此前只有 SwiftUI、Web 与 GTK 声明的 29 项 `features`（`text_weights`、`text_lines`、`image_corners`、`sidebar_width`、`controlled_values`、`search_palette`、`image_template`、`adaptive_images`、`scaled_scene`、`accordion_layout`、`capsule_controls`、`modal_flow_timer`、`page_badges`、`idle_control`、`fill_buttons`、`grid_cells`、`page_search`、`form_layout`、`modal_cover`、`window_short`、`edge_grow`、`delayed_appear`、`toolbar_leading`、`axis_padding`、`icon_menus`、`icon_labels`、`row_baseline`、`sidebar_header`、`untitled_detail`），另加 `window_landscape`（手机转为横屏），动效补上 `appear_delay`、`nudge_key`、`nudge_scale`、`reduce_motion`，样式补上 `Stack` 的 `background`、`foreground`、`corner`。新的运行时在 `BernScene.kt`。

### 外观与文字

- `Stack` 的样式：`BernStack` 先按圆角裁剪（`clip`），再画底色，`foreground` 作为 `LocalContentColor` 向下传给文字与图标。`Stack(enabled: false)` 在各层之上盖一层吃掉全部指针事件的透明层，各层原样显示。
- `text { … }`：活动启动时把各文字样式的字重与字号写入 `BernTypography`；`bernTextStyle` 取 Material 的对应样式，换上字重，给了字号时用该字号（sp）与字体自身的行高。`Text(lines:)` 是 `maxLines` 加末尾省略号；`font_size`（可绑定）与 `line_height` 是 sp，行高按 CSS 把多出的空间均分在上下（`LineHeightStyle.Alignment.Center`、`Trim.None`）。`Label(font_size:)` 同时设置文字与图标的大小，`display: icon` 只显示图标，标题作无障碍描述。
- `Image(corner:)` 是（可绑定、可弹簧的）圆角裁剪；`rendering: template` 用 `ColorFilter.tint(LocalContentColor, SrcIn)`；`aspect` 取代图片自身比例；两边都不给时铺满宽度、高度按 `aspect` 或图片自身的比例（`aspectRatio`，与 SwiftUI 的 `resizable().aspectRatio(contentMode: .fit)` 一致；此前没有 `aspect` 时高度取位图的像素高度）。

### 控件形态与受控值

- `shape: capsule` 是 `CircleShape`（Material 3 的按钮本来就是胶囊），`height` 设置控件高度并去掉上下内边距。`IconButton(height:)` 是该直径的 `FilledTonalIconButton`，`prominence: prominent` 为 `FilledIconButton`，`plain` 与没有 `height` 时仍是无底色的 `IconButton`；`font_size` 是图标大小。`Button(width: fill)` 横向撑满。`ControlGroup` 是 `surfaceContainerHigh` 的胶囊 `Surface`（默认高 40），控件排在其中。
- `Menu(style: icon)`：只显示图标，有 `height` 时是该直径的色调圆形按钮；`Menu(shape:, height:)` 是色调胶囊按钮。胶囊形的下拉 `Picker` 是色调胶囊按钮加 `DropdownMenu`，当前项前有对勾。
- 受控值：`Switch(checked:)`、`Picker(selected:)`、`Slider(current:)` 显示表达式的值，改动时执行动作的命令，参数中与该表达式相同的一项换成新值（整数参数四舍五入）；受控滑块拖动时只移动自己，松开时执行命令。绑定草稿并带动作的 `Switch`、`Picker` 先写草稿再执行动作。`PickerOption` 给出自己的值与标签。`label_hidden` 不显示标签，只作无障碍描述。

### 布局与页面结构

- `padding_x`、`padding_y` 在 `padding` 之上分别覆盖左右与上下。`Row(align: baseline)` 的每个直接子元素（`Spacer` 除外）外包一层 `Box(Modifier.alignByBaseline())`，组件也一样。
- `Grid(row_gap:)` 单独设置行距；`Grid(cell_ratio:, cell_extra:)` 每行高为列宽 ÷ `cell_ratio` + `cell_extra`，各格从顶部排，内容更高时伸进行距。`Flow` 是 `FlowRow`：子项竖直居中（`itemVerticalAlignment`），`Spacer` 是 `weight(1f)`，占满所在行余下的宽度；其中的分段 `Picker` 取自身宽度，不显示标签的 `chips` 把每个 `FilterChip` 直接放进这个 `FlowRow`。
- `ScaleBox`：子元素在声明的逻辑尺寸中层叠（左上对齐），整体在 `placeWithLayer` 中缩放到能放下并居中，超出部分裁掉；Compose 的命中测试经过同一个变换，点击区域与画面对齐。`Positioned` 在所在层叠中占满，内容按（可弹簧的）`x`、`y`、`width`、`height` 放置并居中。`Marquee`：内容按自然宽度从右缘外跑到左缘外，位置在图层中按帧时钟写入，不重新组合；`paused` 时停住，减少动态效果时静止在宽度 28% 处；其中的文字带 1 dp 的黑色阴影（`LocalBernTextShadow`）。
- `Accordion`：每列一个 0 到 1 的权重（`Animatable`），列宽由权重分配，总宽不变，每列在图层中按 `corner` 圆角裁剪。
- 顶栏：工具栏中 `Spacer` 之前的项放在返回按钮之后（`TopAppBar` 的导航位置），之后的放在末端；`Text` 可以放在工具栏里。`SidebarHeader` 固定在侧栏列表上方，`Sidebar(width:)` 是宽窗口中侧栏的宽度，`Detail(titled: false)` 时顶栏不显示所选目的地的名称，`SidebarItem(badge:)` 是 `NavigationDrawerItem` 的 `badge`（为 0 时隐藏）。
- `Form(gap:, row_height:, padding_x:, padding_y:)` 与 `Section(style: plain, padding_x:, padding_y:)`：分节间距；有行高或内边距时，分节的每个直接子元素是一行，按内边距缩进、至少为该行高、内容竖向居中（`Section` 自己的内边距优先；plain 分节不取表单的内边距与行高，只用自己的内边距）；表单以 plain 分节开头时从页面顶部开始：plain 分节在顶边给出一条对齐线（`BernHeadingTop`），表单的列把子项的对齐线往上传，表单量出这条线在 0 处时不留顶部的 16，所以插槽、组件或条件给出的标题也算，条件变了跟着变。页面中的 `Search` 给了 `width`、`height` 或 `font_size` 时是 Material 搜索框样式的胶囊（`surfaceContainerHigh`、搜索图标、清除按钮），按这些尺寸。

### 模态、搜索面板与计时

- `Modal` 是 Material 的 `Dialog`：背景按 0.24 变暗并吸收输入，卡片是对话框的形状与底色，为声明尺寸且离窗口边缘至少 16；`cover: compact` 时在紧凑窗口中铺满整个屏幕，避开系统栏。元素的 `appear`、`disappear` 作用在卡片上：在带过渡的条件分支中，卡片随分支的进出（`animateEnterExit`）播放，播完消失过渡后对话框才关闭。系统返回不关闭模态。
- `Search(style: palette)`：一个搜索图标按钮；打开时是一个对话框，紧凑窗口铺满屏幕，较宽时是 `width` × `height`（默认 640×480）的卡片：上方是搜索框与关闭按钮，下面是结果，结果只在打开时组合。方向键在 `navigation` 不为假的 `SearchItem` 之间移动（按屏幕上的先后），回车选中当前项，没有当前项时执行提交动作；系统返回、Esc 或关闭按钮关闭。`SearchItem` 是 Material 的 `ListItem`，选中后先关闭面板再执行动作；不在面板中时是普通的结果行。
- `Timer`：启用且组合中时每 `interval` 秒（不少于 0.05）执行动作，关掉或离开时取消。`Window(idle_enabled:)` 为假时 `window.idle` 保持为假且不计时，变真时重新计时。`window.short` 是窗口高度低于紧凑断点。`Window(landscape:)` 为真时活动请求横屏（`SCREEN_ORIENTATION_SENSOR_LANDSCAPE`），变假后交还系统决定方向。

### 动效补充

- `appear_delay` 在错峰延迟之上再等待，有延迟时窗口首次显示也播放 `appear`；`nudge_key` 的值每次变化晃动一次，`nudge_scale` 是晃动的倍数（默认 1.06）；`reduce_motion` 为真时整个应用按减少动态效果运行，与系统关闭动画相同。
- `grow(edge:)` 过渡：单独出现的元素在图层中沿一个轴从 0 伸展、该边不动、不淡入淡出；条件分支切换时用 `expandHorizontally`/`expandVertically` 从该边展开。

### 文本格式（`text_formats`）

- 带参数的翻译文本在字符串表（`assets/bern/texts/<语言>.json`）中保留 Bern 的格式原文，不再转成 `%1$s`；不带参数的文本把 `{{`、`}}` 还原为花括号后原样显示。月名与星期名（`bern.date.*`）随表写入，没有该语言时用英文。
- `BernText.format(key, mapOf("名" to 参数))` 在运行时按 `BernTextParser` 解析当前语言的文本再写出：`BernArg` 是文本、数字（值与 Bern 的写法）或日期；复数分支先找 `=N`、再找当前语言的类别（中日韩越泰印尼马来只有 `other`，其余语言 1 为 `one`），最后 `other`，`#` 写出数字；日期按 `y`、`M`、`d`、`E` 的连写取字段或月名、星期名；时长中写出的最大字段吸收更大的单位。语言切换时文字随 `BernLanguage.current` 重新组合。
- 生成器给每个参数传带类型的值：`CalendarDate` 是 `(值).bernArg`（生成的记录类带这个属性），整数与小数是数字，枚举是它的标签，其余是文本。枚举标签显示变体字段时，`bernLabel` 对该变体调用 `BernText.format`，字段按同样的规则传入（枚举字段用自己的标签，可以嵌套）。

### 对照设计稿（Material 3 手机版）的修正

- 滚动视图在所在的列或行中占剩余空间，与 SwiftUI 的 `ScrollView` 一致：直接在列中的用 `Modifier.weight(1f)`；在组件里的，组件在列或行中被调用时由 `LocalBernFlex` 带入那一列或行的作用域，`Modifier.bernFlexible()` 据此占剩余空间（组件函数体不在 `ColumnScope` 中，不能直接写 `weight`）。带 `snap`/`position` 的 `BernScroll` 同样占剩余空间，并把自己的列或行作用域提供给内容。此前引导页（铺满手机屏幕）的底栏悬在内容下面，没有贴底。
- 动画 SVG 的 `WebView` 设为 `MATCH_PARENT` 布局参数：`AndroidView` 默认的 `WRAP_CONTENT` 让页面的布局视口按内容定高，`height: 100%` 解析为 0，SVG 高度为 0 而不显示（经 WebView 的 DevTools 查得）。
- 不给尺寸的 `Canvas` 在层叠中是其他层的底板（`Modifier.matchParentSize()`），层叠只有底板时仍按原样铺满。
- 语言的自称：`zh-Hans`、`zh-Hant` 写作“简体中文”“繁體中文”，与其他目标相同（此前为系统给出的“中文 (简体中文)”）。
- `chrome: unified`：手机没有窗口标题栏，只有窗口标题可显示时不出现顶栏；内容给出标题、返回、前缘或后缘工具时照常显示（此前没有页面的空画框上方多出一条写着应用名的 `TopAppBar`）。
- 行里撑满的按钮（`width: fill`，或内容里有直接的 `Spacer`、`width: fill` 而撑开的带内容按钮）在行中用 `Modifier.weight(1f)` 分剩余宽度；此前用 `fillMaxWidth()`，第一个按钮占满整行，“打开文件夹”旁的“打开链接”被挤出，迷你播放器的暂停与关闭也被挤出屏幕。
- `Stack` 里有自身厚度的 `Rail`，在给了尺寸的层旁按 `Stack` 的对齐摆放（外面套一层 `matchParentSize()` 的 `Box`），缩略图底部的进度条贴底；此前轨道撑满层叠，缩略图被拉高。只在层叠里有给了尺寸的层时这样做，画廊中只有轨道与不定尺寸的层时保持原样。
- 模态（`Dialog`）里的内容不参与共享元素（`LocalBernShared` 设为空）：对话框是另一个窗口，与页面中的同名元素不在同一个布局层级，引导结束时 `SharedTransitionLayout` 抛出“layouts are not part of the same hierarchy”而崩溃。

### 顺带的修正

- `BernRail` 的参数顺序：`overhang` 加入后，在 `Stack` 中铺满的轨道把 `Modifier.matchParentSize()` 传给了 `overhang`（Kotlin 编译失败）；`modifier` 现在在 `overhang` 之前。
- 侧栏与页签收集选项时会进入搜索框的结果，结果中的查询模板变量在那里不存在（Kotlin 编译失败）；现在只进入 `Section`。

## 图标补齐（2026-10-20）

- 此前 Material 对照表写在 `BernViews.kt` 的 `when` 里，表中没有的名字显示中性圆点，检查不知道。用于验收的媒体类应用在手机上的“媒体库”标签、设置“可选功能”里的插件、快捷键、画质增强、账号同步、网络媒体源、局域网与远程等行以及插件卡片上都是圆点。
- 对照表移到 `src/icons.rs`（按名字排序），生成 `BernViews.kt` 时替换 `/*bern:icon-branches*/` 写成 `when` 的分支；检查对表中没有的图标名报错（“the Material icon set has no `…`”），与 Web、Windows 相同。
- 新增的名字与 Material 图标：`film.stack` → Video Library、`server.rack` → Dns、`sparkles` → Auto Awesome、`puzzlepiece.extension` → Extension、`keyboard` → Keyboard、`wifi` → Wifi、`arrow.triangle.2.circlepath` → Sync、`arrow.up.arrow.down` → Swap Vert、`arrow.up.left.and.arrow.down.right` → Open In Full、`arrow.up.right.square` → Open In New、`backward.end.fill` / `forward.end.fill` → Skip Previous / Skip Next、`gobackward.10` / `goforward.10` → Replay 10 / Forward 10、`pip.enter` → Picture In Picture Alt、`camera` → Photo Camera、`eye.slash` → Visibility Off、`line.3.horizontal.decrease` → Filter List、`link` → Link、`scissors` → Content Cut、`shield` → Shield、`tv` → Tv、`arrow.down` → Arrow Downward、`arrow.left` → 自动镜像的 Arrow Back（核心集）、`circle` / `circle.fill` → Radio Button Unchecked / Circle、`square` → Crop Square、`square.on.square` → Filter None、`switch.2` → Toggle On、`hand.tap` → Touch App、`character.cursor.ibeam` → Text Fields。路径取自 Android Studio 自带的 Material Icons 矢量资源（与 `material-icons-extended` 同源）；有几条路径的图标（Photo Camera、Replay 10、Forward 10）逐条加入，不合并成一条路径。
- 顺带改正：`minus` 原为叉号（Clear），改为 Remove；`moon` 原为调色板，改为 Dark Mode；`text.bubble` 原与字幕共用 Subtitles，改为带文字的对话气泡 Chat。
- 测试：`icons.rs` 的单元测试核对表已排序、覆盖 Bern 图标集、每个表达式在 `BernViews.kt` 中有定义或导入；生成测试核对生成的 `BernViews.kt` 对图标集中每个名字都有分支、新名字落到预期的图标，并把画廊的第一个图标换成新名字（检查通过）和不存在的名字（检查报错）。


## 与框架类型同名的值类型（2026-10-21）

生成的 Kotlin 在应用的包里声明值类型，原先直接用声明的名字，三处会撞：`BernGenerated.kt` 的显式导入优先于同包的声明，名为 `Color`、`Modifier`、`Alignment`、`Arrangement`、`Text`、`Role`、`Bundle`、`Canvas`、`Row`、`Column`、`Box`、`Spacer`、`Composable` 的应用类型在这个文件里被导入的框架类型顶替；同包的声明又遮住默认导入（`List`、`Map`、`Pair`）；带字段的变体是 sealed class 内部的类，名为 `String`、`List`、`Boolean`、`Long`、`Double` 或与某个记录同名（`Text { text: Text }`）的变体在类内遮住这些类型。现在值类型写成 `App_` 加声明的名字（`enum class App_Layout`、`data class App_Text`、`sealed class App_Content`），变体类字段里的 Kotlin 标准类型写全名（`data class String(val value: kotlin.String) : App_Content()`），sealed class 内的 `bernName`、`bernLabel` 为 `kotlin.String`；规则见设计文档“跨语言绑定”。变体名、字符串表的键与解码错误（`invalid Layout`）不变。`pick:` 与 `Drop` 存文件时原先直接写 `MediaSource(url)`，现在同样经 `value_type`。

`tests/framework_names.rs` 生成示例 `examples/framework-names` 并核对这些写法；设 `BERN_TEST_KOTLIN=1` 时用 Gradle 的 `:app:compileDebugKotlin` 编译生成的工程。实测（2026-10-21，macOS 27.0.1，Gradle 9.5.1）：编译通过（约 30 秒）；把生成文件换回原名后 kotlinc 报 73 个错误（期望 `androidx.compose.ui.graphics.Color`、实际 `dev.bern.names.Color` 一类的类型不符，`Content.String` 与 `String` 互不相容等）；只加前缀、不写全名时还有 16 个，都来自变体类的遮挡。没有在手机上运行。

## 与框架类型同名的组件与会话（2026-10-21）

组件原先生成大驼峰名加 `View` 的函数、会话类为声明的名字加 `Session`、组件函数里读会话的局部变量为 `bern` 加声明的名字。名为 `Window` 的会话因此与组件函数开头的 `val bernWindow = LocalBernWindow.current` 同名，在同一作用域重复声明；组件函数与生成的代码同在应用的包里，同包的声明优先于通配导入的 `dev.bern.runtime.*`，名为 `bern_query` 的组件生成的 `BernQueryView` 与运行时的同名函数争同一个调用名。现在与 Swift 一致：组件写成 `fun AppView_empty()`，会话类写成 `class App_URL : BernSessionState(1)`，局部变量写成 `val session_Window = bernWindow.session(App_Window::class.java)`（规则见设计文档“跨语言绑定”）。

`tests/framework_names.rs` 核对这些写法（示例新增的组件与会话见 macOS 实现记录的同日条目）。这次没有设 `BERN_TEST_KOTLIN=1` 编译生成的工程，上面两处冲突也没有在 kotlinc 上实测，未验证；新的断言没有运行。

## 验证记录

2026-10-12 至 13，小米 MIX 2S（Android 15），由生成的 CLI、`adb` 截图与录屏驱动：

- **构建与测试**：`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`（另对 `bern-ffi` 与 `bern-transport-ipc` 按 `aarch64-linux-android` 运行）、`cargo fmt --check` 通过；hello、planner、showcase 的 Kotlin 由 Gradle 编译通过。
- **hello**：标题栏与“你好，世界！”；系统语言为简体中文时界面为简体中文；深色跟随系统；`hello hello wave` 从电脑改变了手机上引擎的状态。
- **planner**：模块的页面在侧栏中；带样式的侧栏项（选中时填充强调色）与“改名”按钮；`Canvas` 的格纹、面积与折线随“记一笔”更新；文字标注；“在侧栏显示归档”打开后侧栏出现归档项，在归档页关闭后选中回到概览。录屏逐帧确认：新加的标签从 0.4 倍弹出、略微过冲再回落，后面的按钮平滑让位；打开“大标题”时标题在两种字号间放大，文字不被裁切，下方内容平滑下移。专注模式下静置后顶栏与系统栏隐藏，点按后恢复。
- **showcase**：冷启动显示侧栏列表（搜索框、分组、图标）；文字、按钮（含工具栏按钮）、开关与滑块（工具栏的“全部”下拉）、输入框、图片与布局（适配方式与 `Grid`）、列表与详情（`Query` 结果 5 项、推入链接）各页正常；视频页从 CLI 播放精选片段，状态为 `Playing 0:02 / 0:08`，控制条同步；`window open --kind settings` 打开设置窗口（语言、开关、输入框、底部页签），`window list` 报告两个 `Virtual` 窗口；设置窗口叠在详情页上时，第一次系统返回只关闭设置窗口，第二次回到侧栏列表；`ui set-language --language en` 后界面全部变为英文。
- **控制通道**：`ui inspect`、`ui select`、`ui set-bool`、`ui activate`、`window open`、`window list` 经 `adb forward` 正常。

2026-10-13，Android 模拟器（Android 15，arm64），`examples/gallery`，由生成的 CLI、`adb` 截图与模拟器的 gRPC 触摸接口驱动（`input` 命令只能模拟单指）：

- **图案页**：卡片点按打开图案、星标点按收藏并弹出轻提示；双指张开把卡片撑到上限 320、捏合缩小，`ui set-number` 也能设置。
- **工具栏**：分组之间的空隙；外观菜单切换强调色与明暗；详情开关打开底部弹层，里面的海报与取色铺底；外接键盘的 Ctrl+O 打开系统选择器、Ctrl+Alt+I 开关详情。
- **文件**：系统选择器只列出视频或图片，选中后播放或显示；分屏时从“文件”长按拖入视频，经过时 `over` 变真、松手后播放；`am start` 的 `VIEW` 意图交来的视频交给拖放区播放。
- **画框页**：在浅色窗口中保持深色，其中的菜单也是深色；静置三秒后铺满、圆角收为直角、按钮隐藏，触摸后退回；分体按钮的浮层是深色。
- **胶片页**：初始位置停在设定的一格正中；手指快滑前进一格、短拖弹回原格；按钮滚到首尾并停在正中；上下拖动改变值。
- **关于对话框**：从侧栏底部打开，按“好”关闭。

2026-10-14，Android 模拟器（Pixel 9 Pro XL 镜像，Android 15，arm64），v0.30 的四项扩展，`examples/gallery` 与一个只在本机临时目录中的探针应用（覆盖画廊没有用到的 `presented`、工具栏中的浮层按钮、只有文字的浮层按钮、竖向轨道与 `Column` 的错峰）。这台电脑同时有其他构建，模拟器用软件渲染时每帧约 0.7 秒，所以把显示改为 672 × 1496，用 `wm density` 改变逻辑宽度；需要看时序时把系统的动画时长倍率临时调为 100（测完还原为 1），在模拟器内连续截图并记下时间，逐张量每个元素与背景的差别：

- **错峰**：从收藏页用 `ui select` 切到图案页，海报墙（`ForEach(stagger: 0.04)`，卡片组件的根 `appear: rise`）的 6 张卡片按阅读顺序（Gingham、Tartan、Madras、Glen、Houndstooth、Windowpane）依次开始升起，在倍率 100 下相邻约 4 秒开始（达到 30% 的时刻平均相隔 3.8 秒），即正常速度下约 40 毫秒；等待中的卡片占着自己的格子，其他卡片不移动。探针中打开开关让 `Column(stagger: 0.1)` 出现，其中 4 行文字依次在约 0、13、21、31 秒（倍率 100）开始，即约 0.1 秒一个。
- **轨道**：胶片页 448 dp 宽时，6 个刻度落在 x = 36 + f × 600（像素，f = 0 到 1），“现在”竖线在 0.5 处，名字以起点对准刻度；`ui set-number` 把 level 从 0.5 改为 0.8 后，竖线按 `smooth` 弹簧从 336 平滑移到 516（倍率 100 下逐张截图看到 336、342、358、377、464、…、497），没有 `animate` 的底色立即涨到 0.8（538 / 672）；拖动区的底色铺满 120 dp 高、宽随 level，`Stack` 仍是 120 dp 高（轨道用 `matchParentSize()`：不参与 `Stack` 的尺寸，再按 `Stack` 的大小量）。窗口变为 896 dp 宽时轨道随宽度重排，竖线仍在 0.8 处。探针中竖向轨道 `anchor: end`：底色从顶端铺到 0.3 × 200 dp，文字的底端对准同一点，两者在轨道厚度内居中；`Stack` 中叠在滑块上的轨道，在它 0.25 处的竖条上点按，滑块照常收到（0.5 变为 0.247）。
- **浮层按钮**：画框页的音量按钮只显示图标；`ui.inspect` 中为 `closed`、其中的滑块不在列表里，`ui activate` 后为 `open`，“音量”标题与 0.7 的滑块出现在按钮下方的深色浮层中（画框页固定深色），滑块也出现在 `ui.inspect` 中；点到浮层外关闭，滑块随之卸载；点按按钮打开。探针中工具栏的浮层按钮显示为文字按钮，`presented` 绑定会话字段：把绑定同一字段的开关设为真时浮层打开、设为假时关闭，`ui activate` 打开后开关变为真，点到外面关闭后变回假；`shortcut: "cmd+j"` 的 Ctrl+J 打开它；只有文字的浮层按钮是色调按钮。
- **四档尺寸**：用 `wm density` 把窗口设为 448、896、1344、1493 dp 宽（断点 600 / 1024 / 1440），海报墙的页边距量得 16、24、32、约 48 dp（`window.compact`、`medium`、`expanded`、`large`）；896 dp 起侧栏与详情并排。胶片页的 `Adaptive(id: film_controls)` 在 `ui.inspect` 中依次为 `compact`（图标按钮）、`medium`（文字按钮）、`expanded`（文字按钮加提示文字），1493 dp 时详情约 1213 dp 宽，仍为 `expanded`。
- **构建与测试**：`cargo test -p bern-backend-compose`（新增画廊的生成测试）、`cargo clippy -p bern-backend-compose --all-targets -- -D warnings` 通过；画廊与探针的 Kotlin 由 Gradle 编译通过。

2026-10-14，引擎读取交来的文件，Android 模拟器（Android 15），`examples/gallery`：

- 把 `tides.mp4` 以“海浪 Tides #2.mp4”放进模拟器的 `Movies` 并让媒体库建索引，用 `am start -a android.intent.action.VIEW --grant-read-uri-permission` 把 `content://media/external/video/media/29` 交给应用；切到画框页后拖放区收下它并播放，引擎状态中的提示为“▶ 海浪 Tides #2.mp4 · MP4 · 49 KB”：名字来自系统的显示名，MP4 由文件开头的 `ftyp` 认出，大小来自描述符。
- 经 CLI 播放不存在的 `content://media/external/video/media/9999`：取不到名字、打不开，提示只显示地址。

2026-10-19，页面级布局、场景、控件形态与文本格式：

- **测试与构建**：`cargo test -p bern-backend-compose`（新增 `layout_features_generate_for_compose` 与 `text_formats_generate_for_compose`）、`cargo clippy -p bern-backend-compose --all-targets -- -D warnings`（另外忽略 `bern-target-api` 中已有的 `for_kv_map`）与 `cargo fmt --check` 通过；gallery、showcase、planner、一个覆盖 29 项与 `Stack` 样式的临时巡览应用、文本格式的临时测试应用与用于验收的媒体类应用 `bern build --target android` 由 Gradle 编译成 APK。
- **文本格式**：把运行时中的解析与写出代码（`BernArg`、`BernPiece`、`BernTextParser` 与 `BernText.format`）原样取出，用 Gradle 缓存中的 Kotlin 编译器在 JVM 上运行，读取为测试应用生成的字符串表：英文依次为“no files”“1 file”“3 files”“Today is Monday, October 12, 2026”“3:04 used (0:03:04)”“3 large files”“Saved Tides #1.mp4 on Mon, Oct 12”，简体中文为“0 个文件”“3 个文件”“今天是 2026年10月12日 星期一”“已用 3:04（0:03:04）”“3 个大文件”“已保存 Tides #1.mp4，10月12日 周一”，与 SwiftUI、Web、GTK 相同；`{{`、`}}` 写出花括号，`#` 只在复数分支中写出数字。这是 JVM 上的结果，不是设备上的。
- **设备**：手机解锁后实测（MIX 2S，`adb` 截图与生成的 CLI，没有发送按键）：文本格式测试应用英文与简体中文的全部输出与上面相同；用于验收的媒体类应用的引导页与设计稿 Material 手机版对照，修正后动画 Logo 显示在格纹插画上、底栏贴底、“跳过”为 Tonal、“开始”为品牌色 Filled。其余 29 项、`Stack` 样式与动效补充在设备上的显示与交互仍**未验证**（应用的其他页面没有逐屏对照）。

2026-10-19（续），没有页面时的空画框（纯粹模式），MIX 2S（Android 15），`adb` 截图与生成的 CLI，没有发送按键，滚动用一次 `input swipe`：

- 引导选“纯粹播放”并完成，不再崩溃；顶栏不出现，空画框（虚线描边）、“打开文件”、并排的“打开文件夹”与“打开链接”、“继续上次”、说明文字与“最近播放”在设计稿 Material 手机版的位置；最近播放的缩略图底部是进度条。
- 从“继续上次”播放后收起：迷你播放器显示缩略图、标题、暂停与关闭；列表滚到底时最后一行完整地停在迷你播放器上方（应用在播放时给列表底部留出空间）。
- 测试：`cargo test -p bern-backend-compose`、`cargo clippy --workspace --all-targets`、`cargo fmt --check` 通过。

2026-10-20，可选功能的预览与设置页，MIX 2S（Android 15），`adb` 截图与生成的 CLI，没有发送按键，滚动用一次 `input swipe`：

- 不给尺寸的 `Canvas` 作为 `Stack` 的一层时用 `Modifier.matchParentSize()` 当底板：此前元素生成时只把层叠标记传给 `Rail`（拆分生成器时留下），画布拿到的是铺满，引导第二步预览里选中的“首页”底色撑满整列、“新番”“媒体库”被挤走。
- 行里的水平 `Rail`（列里的竖直 `Rail`）用 `Modifier.weight(1f)` 分剩余空间：此前它先占走全部剩余宽度，预览播放条上“弹幕”和后面的图标被挤出。
- 没写 `font_size` 的 `Label` 的图标随文字样式的字号缩放（`bernIconSize`），图标与文字的间距为字号的 0.6 倍（4 到 12）：此前固定 24 dp 与 8 dp，小字号的“首页”被挤成竖排。
- 给了高度的 `Row` 里，文字（`Text`、`Label`）按自己的行高排、按行的对齐超出这一行（`wrapContentHeight(…, unbounded = true)`），与其他目标相同：此前被压进行高并裁掉，设置子页的大标题“可选功能”上沿与“能”字右侧缺了一块。
- 实测：引导第二步的预览里选中项只有一行高，三项都在，播放条显示进度、“弹幕”与图标；设置子页只有一份返回与大标题（应用一侧删去重复的 `Slot(id: form_header)`）；侧栏与设置页的头像肩部被圆裁住。未覆盖的图标名仍显示圆点（见下）。
- 列里没写宽度、带底板的 `Stack` 横向填满（`fillMaxWidth()`），与其他目标的“列里的块”一致：设置页插件横幅与正文同宽。
- 列里的列（行里的行）直接含 `Spacer` 且没写这个方向的尺寸时用 `Modifier.weight(1f)` 占满剩余空间，`Spacer` 才有空间可分，与其他目标相同；滚动内容、列表与 `Form` 的行不分（它们的高度不定）。此前引导最后一步的列只有内容高，`Spacer` 为零，底部的“上一步 / 开始使用”被挤出屏幕。实测引导最后一步底栏在屏幕底部，纯净模式首页、设置列表与设置子页没有变化。

2026-10-20，图标补齐，MIX 2S（Android 15），只用 `adb` 截屏，不发送按键、不操作界面（当时另有会话在用这台手机，手机上是它用当前 Bern 工作树构建的包，含本次改动）：底部标签的首页、新番、媒体库（Video Library）与搜索，设置列表的播放、可选功能、插件（Extension）、快捷键（Keyboard）、关于，插件市场卡片的防剧透（Visibility Off）、正则弹幕过滤（Filter List）、敏感词屏蔽（Shield）、片段导出（Content Cut）、外部播放器（Open In New）都显示 Material 图标，没有圆点。之后手机空出来，用生成的 CLI（`ui activate`，不发按键）打开“可选功能”页：弹幕（Chat）、画质增强（Auto Awesome）、媒体库、追番、账号同步（Sync）、网络媒体源（Dns）、局域网与远程（Wifi）、下载八行的图标都正常，看完回到原来的插件页。另外把 `BernViews.kt` 的 58 条路径用 rsvg 渲染核对过图形（`character.cursor.ibeam` 因此改用已有的 `text`，两者路径相同）。

2026-10-20，标签选择器与换行工具条，MIX 2S（Android 15，393 dp 宽），装包后经 CLI 打开设置 › 插件，用 `adb` 截屏与 `uiautomator` 读位置，不发送按键：

- 插件页的 `Flow(gap: 10)` 里是分段 `Picker`、`Spacer` 与 `Picker(style: chips)`。第一行是分段控件与靠右的「全部」「弹幕」，其余三个 chip 换到第二行靠左；`FilterChip` 32 dp 高，选中的「全部」为色调底色，不带对勾。经 CLI 选「工具」后它变为选中，列表只剩工具类插件。
- 原型（Material，手机）第一行放得下三个 chip：它的分段控件 123 宽、chip 54 宽（13 px 字）。Compose 的 `SegmentedButton` 各段等宽，且总为对勾留出 18 + 8 dp（不显示对勾时也留），`FilterChip` 为 14 sp 字、60 dp 宽，这一行在 361 dp 里放不下第三个。这是系统控件自己的尺寸，没有改画。
- 发现并修正：分段按钮按各段的最小固有宽度排，文字的最小固有宽度是最长的一个词，带计数的标签「已安装 3」原来被截成「已安装」；现在标签按整行宽度量。

2026-10-20，直接写在窗口正文里的样式变体：此前 `style::used` 只看组件的正文，窗口正文里写的 `variant:` 不生成 `BernStyles` 的条目，生成的 Kotlin 引用不存在的 `BernStyles.buttonStage`（原因见 WinUI 3 实现记录的同日条目）。生成器测试 `a_style_variant_in_a_window_is_defined` 断言定义与引用；测试应用 `bern build --target android` 完成 Kotlin 编译并打出 APK。MIX 2S（Android 15）上装包后截屏（不发按键，看完切回原来的应用并卸载测试包）：窗口正文里的按钮是强调色底、圆角 12，组件里的是 `fill.subtle` 底、圆角 4。

2026-10-20，直接写在窗口正文里的 `Drop(opens: true)` 与网络图片：清单的 `VIEW`、`SEND` 意图与 `INTERNET` 权限此前只看组件的正文（与上一条同因），窗口正文里直接写的拖放区不登记它的种类、活动不是 `singleTask`，那里取自网络的图片不给清单加 `INTERNET`。改为连同窗口正文一起看；生成器测试 `a_drop_and_a_network_picture_in_a_window_reach_the_manifest` 改前失败、改后通过。没有装到手机上。

2026-10-21，图片不在主线程解码（见“图片的读取”）。此前声明资源的 `Image` 在组合时用 `BitmapFactory.decodeFile` 整张解码，页面每次重新出现都再解一次；引擎给出的图片已在后台读取，但按原尺寸解码、一直保留，而且共用一个状态表，每读到一张就让屏幕上所有图片重组。在小米 MIX 2S（Android 15）上用独立包名的 release 测试包对照，两版只差这两个 Kotlin 文件；经控制通道切页（不发输入事件），每次切换前清零、切换后取 `dumpsys gfxinfo framestats`，记下这次切换里最长的 UI 线程时间（`HandleInputStart` 到 `SyncQueued`），下面是中位数：

- 应用自带的图片（字标 473×216、海报 240×344）：两版没有可测出的差别，打开“关于”页 81 / 80 ms，其余页面相同。
- 把字标放大到 3784×1728、海报放大到 960×1376：打开“关于”页 190–194 ms → 73–81 ms（三轮，每轮 12 次），省下的就是字标在主线程的解码；媒体库、首页与新番切日两版相同（这些图片改前已在后台读取）。走一遍海报页、强制回收后，Native Heap 114–126 MB → 53 MB，Graphics 121 MB → 64 MB（各两轮）。第一版对只给 `aspect`、铺满网格格子的海报以窗口宽度为上限，结果仍按原尺寸解码，同一张海报在不同页面又各读一次，内存反而更高；改为按布局量出的宽度读取后才是上面的数字。
- “关于”页的字标与媒体库海报网格的画面改前改后一致。
- 两版切页本身都有 60–130 ms 的 UI 线程帧（打开“关于”页约 80 ms、首页约 125 ms、媒体库约 105 ms、切日约 58 ms），与图片无关，这次没有处理。

生成的调用没有变化，生成器测试不用改；`cargo test -p bern-backend-compose` 通过。

2026-10-21，窗口正文的摆放（设计文档“一套窗口声明”）：

- 改前：窗口内容区是 `Box(contentAlignment = Alignment.TopCenter)`，比窗口小的正文整体水平居中、靠顶；直接写在窗口里的 `Stack` 也只是水平居中、靠顶。
- 修正：内容区改为 `Alignment.TopStart`。生成器把直接写在窗口里（经条件或组件也算）、没写宽度的 `Column` 放进 `Box(Modifier.fillMaxWidth(), contentAlignment = …)`，对齐按它的 `align` 取 `TopStart`、`TopCenter` 或 `TopEnd`；没写尺寸的 `Stack` 放进 `Box(Modifier.fillMaxSize(), contentAlignment = …)`，对齐取它的 `align`（`under_bars` 本来就铺满）。列里的 Material 按钮本来就保持自身宽度。
- 测试：`content_in_a_window_starts_at_its_top_leading_corner` 断言内容区与这几层 `Box`，组件自己的正文不加；撤掉修正时失败。探针应用的 Kotlin 编译通过。
- 实机（MIX 2S，Android 15，探针应用经 CLI 切换正文，`screencap` 截屏，用完卸载）：改前默认列与写在组件里的 `Column(align: center)` 都整体水平居中、靠顶，`Stack` 的小层水平居中、靠顶；改后默认列从左上排起、按钮保持自身宽度，居中列水平居中、靠顶，`Stack` 的小层在内容区正中。图标按钮、菜单、浮层按钮那一种正文没有在手机上看。

2026-10-21，切页时的长帧（上面图片一条最后说的 60–130 ms）。测法同那一条：独立包名的 release 测试包，只经控制通道切页，每次切换前清零、切换后取 `framestats`，记这次切换里最长的 UI 线程时间，取中位数；另用 `atrace`（带应用自己的段）看每帧的阶段，在加了 `profileable` 的同一个包上用 `simpleperf`（DWARF 调用栈）看长帧里的调用。

长帧从哪里来：

- **代码没有预先编译**：`adb install` 装上的 release 包是 `status=verify`，APK 里只有 Compose 等库自带的基线规则，Bern 运行时与生成的界面代码都在解释器与 JIT 里跑。同一个包用 `cmd package compile -m speed` 全编译后，打开“关于”80 → 47 ms、返回 74 → 30、首页 121 → 84、媒体库 112 → 63、新番切日 55 → 43。
- **全编译后剩下的**：切到首页那一帧约 86 ms，组合几乎都发生在测量阶段（`Adaptive` 的 `BoxWithConstraints` 在测量时子组合；窗口根是共享元素的 `SharedTransitionLayout`，所以先做前瞻测量），`measureAndLayout` 约 50 ms；应用节点变更约 17 ms、释放旧页约 8 ms、绘制约 5 ms。长帧里的采样按调用归类：组合 22%、协程的启动与取消 18%、测量与布局 16%、快照读写 14%、节点增删 9%、文字布局 6%、绘制 4%。
- **协程那一份是每个元素各自带的**：`BernAppear` 的 `AnimatedVisibility`（过渡对象，加一个一直等着的 `snapshotFlow` 收集）；带样式按钮的 5 个 `animateFloatAsState` 与阴影的 `animateDpAsState`，每个都有一个一直等着的协程；按下与悬停各一个收集协程；Material 按钮自己的水波纹。页面离开时这些挂起的协程逐个取消，每次取消在协程里抛一次异常，ART 为它回溯一遍栈。
- **其余属于运行时的**：`bernReduceMotion` 在每个新元素里读一次 `Settings.Global`，每次组合还遍历一个状态表；`BernScheme` 每次组合重算壁纸取色（`dynamicDarkColorScheme`，`appearance:` 固定明暗的每个元素各算一次）；`BernText.format` 每次逐字重新解析模板；只用来占位置的空 `Canvas` 每个都是带绘制与清空语义的节点（测试应用生成的代码里有 153 处）。
- 打开“关于”页的那一帧里，动画 SVG 的 `WebView` 创建（`WebViewChromium.init`）占 10–15 ms。
- 切换后约 1 秒内超过 16.7 ms 的帧大多由 RenderThread 决定（每帧 12–20 ms），UI 线程每帧只有 3–8 ms；其中很多帧只超出一点（17–18 ms）。按超过 33 ms 计，每次切换 4–24 帧，同一个包两轮之间就差到一倍，改前改后看不出差别，这次没有处理。

修正（“样式与动效”“`Canvas` 与标注”“构建、运行与打包”各条已改）：基线配置文件列出运行时与生成代码的全部方法，`bern run` 连同 `.dm` 一起安装；`BernAppear` 用一个进度驱动所在层，不再用 `AnimatedVisibility`；样式变换与阴影只在外观变化时启动弹簧，按下与悬停由 `BernInteractions` 在发出时记下；动画开关的系统设置在启动与回到前台时读一次，`reduce_motion` 的持有者合成一个状态；配色按当前配置、明暗与强调色缓存，同一配置只算一次；文字模板解析一次后缓存；空 `Canvas` 生成为 `Spacer`。

结果（MIX 2S，Android 15，电池温度 35.0–36.0 ℃；每次切换里最长的 UI 线程时间的中位数，ms。“改前，装后即测”是与改后交替测的一轮；“带配置文件安装”是 `bern run --release` 的装法，装完即为 `status=speed-profile`；“全编译”是 `cmd package compile -m speed`。测试脚本没有关掉首次启动的引导页，它盖在主窗口上，切页仍在下面的主窗口里组合、布局与绘制，两版条件相同；关掉引导页后另录了屏，见下）：

| 切换 | 改前，装后即测 | 改后，装后即测 | 改后，带配置文件安装 | 改前，全编译 | 改后，全编译 |
| --- | --- | --- | --- | --- | --- |
| 打开“关于” | 78.5 | 73.1 | 54.0 | 47.1 | 46.9 |
| 从“关于”返回 | 75.8 | 69.4 | 34.1 | 30.0 | 28.7 |
| 新番切日 | 58.5 | 52.0 | 36.9 | 43.2 | 35.6 |
| 切到首页 | 123.4 | 106.2 | 75.9 | 83.6 | 69.0 |
| 切到媒体库 | 106.2 | 91.8 | 61.6 | 62.5 | 53.5 |

- 改前用户拿到的是第一列；改后按 `bern run` 的方式安装是第三列，长帧短了 31–55%。直接装 APK（第二列）只有代码改动的 7–14%（另一轮 4–23%），配置文件要等系统下次后台编译才用上。
- 全编译的两列只差代码：首页、媒体库与切日短了 14–18%；“关于”与返回没有变，那一帧主要是 `WebView` 的创建与页面本身的组合。
- 带配置文件安装仍比全编译慢一些（首页 75.9 / 69.0）：库里不在它们自带配置文件中的代码仍由 JIT 跑，系统后台编译时会把运行中热起来的方法补上。没有把整个 Compose 与协程库加进规则（安装时间与编译产物会大很多）。
- 关掉引导页、套用全部功能后录屏（`screenrecord`，按 30 帧抽图）：切到媒体库时先是“正在载入…”，随后整页淡入上移，海报按错峰逐个淡入上移，约半秒排齐；新番切日时周一收成胶囊、周四展开，最后的画面与改前一致。带样式按钮的按下反馈没有在真机上点按确认（测试不发输入事件）。
- 生成器测试 `a_canvas_with_nothing_to_draw_is_a_spacer`、`the_runtime_and_the_interface_are_in_the_baseline_profile`；`cargo test -p bern-backend-compose` 通过。

2026-10-21，切页后由 RenderThread 决定的帧（上一条最后说的每帧 12–20 ms）。测法同上一条：独立包名的 release 测试包，按 `bern run` 的方式带配置文件安装，只经控制通道切页；这次先关掉首次启动的引导页再套用全部功能（上一条测量时引导页一直盖在主窗口上）。除 `framestats` 外，用手机上的 `perfetto` 同时记下应用的 atrace 段（`gfx`、`view`、`webview`）、Adreno 驱动的 kgsl 事件（每批 GPU 命令的提交与完成，完成事件带 GPU 时钟上的开始与结束）与 SurfaceFlinger 的 FrameTimeline（系统为每一帧判定的掉帧类型）。这台手机没有 `gpu.counters` 数据源，GPU 时间取自 kgsl。

这些帧从哪里来：

- **大多不是掉帧，是排队**：切页那一两帧在 UI 线程误了截止时间，之后的帧在缓冲队列里多排一到两块，RenderThread 在 `dequeueBuffer` 里等空闲的缓冲区，或者 GPU 等拿到的缓冲区被显示放开。`framestats` 的 `SyncStart` 到 `FrameCompleted` 包含这段等待，所以每帧显示 12–20 ms；按 `IntendedVsync` 到 `FrameCompleted` 计，多排一块时每帧约 17 ms、两块时约 33 ms，取决于切页长帧落在哪里。同一个包两轮之间“超过 33 ms 的帧数”因此差很多（首页每次切换 8.5 与 38.4 帧）。FrameTimeline 把这些帧判为缓冲区塞满（Buffer Stuffing）；`DisplayPresentTime` 仍是每 16.7 ms 一帧（多排一块时比预定晚约 49 ms），画面连续，只是整体晚一到两帧。
- **RenderThread 自己的工作每帧约 7 ms，GPU 约 2 ms**：媒体库、首页与新番切日共 927 帧，`Drawing`（录制并提交绘制命令）中位 5.3–5.8 ms、最长 8.2–9.6 ms，同步 0.6 ms，其中 `eglSwapBuffers` 1.2 ms；kgsl 量得每帧 GPU 执行中位 2.1–2.2 ms、最长 4.1–4.4 ms。出现过渡的透明层已由 HWUI 提升为离屏层，每帧只重画变了的部分（`drawLayer [graphicsLayer] 1080 x 1863` 约 1 ms），没有纹理上传或着色器编译造成的长帧。FrameTimeline 中这三种共 17 次切换，除切页的一两帧外只有 1 帧判为应用误时，那一帧 RenderThread 只画了 5.5 ms，是在等缓冲区。
- **由 RenderThread 误时的只有动画 SVG**：“关于”页的动画图标在 `WebView` 里播放，页面显示期间每帧都重绘。`WebViewFunctor::drawGl` 平时每帧约 5 ms；每次打开后图标出现的那一帧 9–22 ms，有时误一帧。同一次打开里，主线程新建 `WebView`（`AwContents.constructor`）三轮中位数 47–52 ms，占切页帧（96–105 ms）的一半。
- 剩下看得见的卡顿是切页本身的一两帧，在 UI 线程：媒体库先是约 25 ms，数据到达后整页在测量阶段组合的一帧 72–76 ms；首页 77–82 ms；切日 26–52 ms（见上一条）。

修正（[动画 SVG](Animated-SVG.md) 的 Android 补充）：离开屏幕的 `WebView` 不立即销毁，换成空白页后按文档与活动留一个，30 秒内同一文档再显示时取回并重新载入文档，脚本与动画仍从头开始；超过 30 秒、活动停止或渲染进程退出时释放。留下的视图处理 `onRenderProcessGone`：它不可见时渲染进程可能被系统回收，不处理会连带结束应用。

结果（MIX 2S，Android 15，电池 35–36 ℃；改前改后各三轮，穿插进行，改后第三轮是带 30 秒释放的最终版本；每轮每种切换 10 次，trace 每轮打开 4 次）：

| 打开“关于” | 改前 | 改后 |
| --- | --- | --- |
| 最长 UI 线程时间（`framestats` 中位数，ms） | 99.4 / 98.7 / 97.2 | 39.2 / 38.7 / 37.9 |
| 切页帧（trace 中位数，ms） | 96.4 / 105.2 / 103.7 | 39.8 / 39.4 / 40.7 |
| 其中新建 `WebView`（ms） | 46.8 / 51.7 / 51.4 | 0 |
| 图标出现那一帧的 `drawGl`（ms） | 17.5 / 10.5 / 11.9 | 12.4 / 10.2 / 13.9 |
| 切页帧之后 1.2 s 内判为应用误时的帧（每次打开） | 0.75 / 0.5 / 0.75 | 0.25 / 0.25 / 0 |

- 返回、媒体库、首页与切日两版相同（最长 UI 线程时间分别约 30–37、60–63、71–75、31–39 ms）。
- 图标出现那一帧与平时每帧的 `drawGl` 没有变：重新载入的文档仍要整页栅格化，这是 `WebView` 自己的代价。
- 留下的视图让 `WebView` 的渲染进程继续存在：离开“关于”后 PSS 98.5 MB，其中私有脏页 21 MB、私有干净页 55 MB；应用进程两版都是 199 MB。改前离开后渲染进程随即退出。所以只留 30 秒。
- 生命周期（控制通道驱动）：第一次打开新建；30 秒内再打开复用，这次切页帧 27 ms；离开 35 秒后渲染进程已退出，再打开重新新建（114 ms）；测试包退到后台时立即释放，回来再打开新建；全程没有崩溃。第一次打开“关于”仍要新建，复用只在同一次使用里再次打开时生效。
- 以后看切页后是否流畅，用 FrameTimeline 的掉帧类型或 `DisplayPresentTime` 的间隔，不用 `framestats` 的每帧时长。

## 未完成与已知限制

- 只构建 arm64-v8a。32 位设备与 x86_64 模拟器的引擎库未构建。
- 宽度不小于 600 dp 的布局只在模拟器上用 `wm density` 改变逻辑宽度验证过；真的平板、折叠屏展开与横屏未实测。
- 屏幕旋转未实测。
- 窗口事实不报告窗口的位置与尺寸（为 0）。
- 后台继续播放未实现（需要媒体服务）；一个播放器同一时刻只在一个视图中显示。
- HDR 图片与 HDR 视频未验证；这台手机的屏幕按 SDR 显示。
- 发布版没有开启 R8 缩减，hello 的发布版 APK 约 12 MB。
- 列表项移除时没有消失过渡。
- 带样式的侧栏项不支持方向键切换。
- 交给应用的文件是 `content://` 地址。引擎经 `MediaSource::file_name()` 与 `open()` 读取：Rust 一侧经 JNI 调 `BernBridge.fileName` 与 `openFile`，前者查 `OpenableColumns.DISPLAY_NAME`，后者用内容解析器打开文档并交出文件描述符（`detachFd`），引擎当普通文件读取。经管道提供内容的文档提供方给出的描述符不能跳转、长度为 0。
- 只有当前屏幕上的控件登记快捷键：手机上停在侧栏时，页面工具栏的快捷键不生效。
- 胶片的放大与停靠在宽屏布局、`arrival: nudge` 的晃动在 Android 上未逐帧确认。
- 错峰：一次更新加入几个子元素（而不是容器本身出现）时的错峰、系统关闭动画时不错峰，未验证；画廊与探针都没有一次加入几行的操作。时序是在动画放慢 100 倍时量的，正常速度下的间隔是按倍率换算的。
- 出现过渡的规则比设计宽（按实现推断，未逐帧确认）：窗口首帧之后组合的带 `appear` 的元素都会播放，包括随所在的 `ForEach` 一起出现的各行（设计中只有错峰的容器才这样）。
- 轨道：从右到左的语言中横向轨道从右边量起（`placeRelative`），未验证。`Stack` 中只在轨道直接放着没写交叉尺寸的 `from`/`to` 元素时才随 `Stack` 铺满；其中有组件、或轨道本身带出现过渡等包装时，厚度只来自子元素。
- 浮层按钮：浮层是 Material 的 `DropdownMenu`，位置由它决定（锚在按钮下方，放不下时左移），没有箭头。画框页静置三秒后按钮行隐藏，打开的浮层随之卸载，这是声明的行为。
- 动画 SVG 第一次显示时新建 `WebView`，主线程约 50 ms；离开后 30 秒内再显示同一文档才复用。图标出现的那一帧在 RenderThread 上 9–22 ms，可能误一帧；显示期间每帧约 5 ms。
- 切页的长帧之后，其后的帧在缓冲队列里多排一到两块，整体晚一到两帧上屏，直到动画结束（不是掉帧）；这是切页长帧的后果，没有单独处理。
