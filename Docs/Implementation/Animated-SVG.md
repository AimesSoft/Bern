# 动画 SVG

Bern 内置 `bern-svg` 文档运行时和 `Svg` 组件。它保留 SVG 原文件的矢量图形、CSS 关键帧、SMIL、渐变、滤镜、蒙版、文字和文档内 JavaScript。支持 WebView 的目标由平台的 SVG/DOM 引擎渲染；无 WebView 的目标在构建时转成静态 PNG。

```rust
Svg(
    source: "assets/logo.svg",
    static_source: "assets/logo-final.svg",
    width: 104,
    height: 104,
    label: tr! { "zh-Hans" => "应用图标", "en" => "App icon" },
    playing: app.logo_playing,
    speed: app.logo_speed,
    restart: app.logo_revision
)
```

资源需列入 `Bern.toml` 的 `[resources].paths`。动态路径检查 XML 和 SVG 命名空间；静态路径由 resvg 解析；资源缺失、错误格式和外部实体会报错。`Image` 继续用于位图，`Svg` 明确表示带独立文档执行环境的矢量资源。

## 接口

| 属性 | 含义 |
| --- | --- |
| `source` | 已声明的 `.svg` 文件，构建期确定 |
| `static_source` | 可选，已声明的最终状态 SVG；无 WebView 时用于静态编译，省略则栅格化 source 的基础图形 |
| `width`、`height` | 必填，逻辑尺寸，可绑定数值状态；矢量随视口缩放 |
| `label` | 辅助技术名称；省略时为装饰内容 |
| `fit` | `contain`（默认）、`cover`、`fill`，对应 SVG 的 meet、slice、none |
| `playing` | 布尔状态，默认 true；暂停／恢复 CSS、Web Animations 和 SMIL 时间线 |
| `speed` | 正数状态，默认 1；CSS/Web Animations playbackRate 与 SMIL 统一倍率；无效值回退或保持合法倍率 |
| `restart` | 数值状态；值变化时把现存 CSS/Web Animations 与 SMIL 时间线归零；不重新加载脚本或重置脚本自己的业务状态 |
| `interactive` | 构建期布尔值，默认 true；控制 SVG 内鼠标／触摸交互 |

双击、点击、指针事件等由原 SVG 的脚本处理。动画控制不接管 JavaScript 的 `Date.now()`、`setTimeout()`、自定义 RAF 时钟。CSS 与 SMIL 时间线由统一接口控制；WAAPI 动画在控制状态更新、CSS 动画开始或 SVG 子树／class 变化时被同步，纯脚本新建 WAAPI 动画而没有这些事件时，不保证立即接管。CSS 的 `prefers-reduced-motion` 保留浏览器／操作系统语义，动画作者应提供减少动态效果样式。组件退出时释放文档；平台生命周期和文档可见性通知用于暂停标准动画时间线，Web 还检测 iframe 是否进入视口。

## 呈现层

| 目标 | SVG 文档引擎 | 依赖 |
| --- | --- | --- |
| macOS / iOS | 透明 `WKWebView`，封装成 SwiftUI 视图 | 系统 WebKit，非持久数据存储 |
| Android | Compose `AndroidView` 中的透明 `WebView` | 系统 Android System WebView |
| Windows（WinUI 3 与 XAML Islands） | WebView2 Win32 呈现器（子窗口）跟随 XAML 占位区域 | Microsoft Edge WebView2 Runtime；原生子窗口有 HWND airspace 限制 |
| Linux / GTK 4 | WebKitGTK 6 `WebView` | 使用 `Svg` 才启用 `bern-gtk/svg`；开发包 `libwebkitgtk-6.0-dev`，Debian 运行依赖 `libwebkitgtk-6.0-4` |
| Web | 透明、独立源的 sandbox iframe | 宿主浏览器 |
| tvOS 或 `webview = false` | 构建期 resvg → PNG，运行时 Image | 应用不引入 SVG WebView 呈现器 |

这是浏览器 SVG 标准能力的封装，渲染细节和支持的标准版本仍取决于平台引擎版本。没有宣称重新实现一套“全规范”Rust SVG/JavaScript 引擎。没有使用 `Svg` 的应用不创建这些文档视图；GTK 不引入 WebKit 依赖，SwiftUI 不生成 WebKit 呈现代码。

## 资源边界

SVG 是打包的、自包含的应用资源：图片／字体请嵌入为 data URI，样式和脚本放在文件内。文档不暴露 Bern 引擎、原生命令或宿主 DOM；CSP 限制外部子资源和网络请求，运行时阻止链接点击，原生宿主限制导航和弹窗。Web 的 iframe 使用独立源的 `sandbox="allow-scripts"`，但这不能禁止脚本导航其自身文档，因此只接受应用自己提供的 SVG，不作为不可信脚本的通用沙箱。

Android、Windows 和 Linux 需要表中对应的 WebView 运行环境。发行目标不提供 WebView 时，在对应 `[targets.*]` 配置 `webview = false`，构建会转换 SVG；Linux 的 WebKitGTK 是启用 `svg` 特性后的动态链接依赖，需要随系统安装。WebKitGTK 的网页进程沙箱（bwrap）需要建立用户命名空间：Ubuntu 24.04 起默认只允许带 AppArmor 配置的程序这样做，没有配置时 WebKitGTK 中止整个应用。用到 `Svg` 时，Bern 的 `.deb` 附带这份配置（`/etc/apparmor.d/<bundle_id>`，与 Ubuntu 为 epiphany 等 WebKitGTK 应用提供的相同：不另加限制，只允许 `userns`），并在 AppArmor 启用时安装后立即载入；`.tar.gz` 与开发构建需要手动添加，见 [Linux GTK 实现记录](Linux-GTK.md)“打包”。Windows 两种前端都用 WebView2 的 Win32 呈现器：WinUI 3 的 `WebView2` 控件在透明页面下面垫一层主题底色（浅色主题下是白色，微软文档也写明它不支持透明背景），所以不用它。呈现器是窗口之上的子 HWND，按占位区域在布局后逐帧跟随位置与缩放（直到半秒不动），按外层滚动视图裁剪；它不随祖先的其他裁剪、透明度变化，也总在其他 XAML 图层（如浮层菜单）之上。WebView2 的缓存在用户临时目录（`BernSvgWebView2`），不在可执行文件旁边。

## 验证

`examples/svg-animation` 是所有目标共用的示例：CSS 旋转、SMIL 半径动画、渐变、脚本双击变色，以及原生的暂停、倍速、重播按钮。

```sh
cargo test -p bern-svg -p bern-compiler
node --test crates/bern-svg/tests/runtime.test.mjs
cargo test -p bern-cli --test svg_targets
cd examples/svg-animation
../../target/debug/bern build --target macos
```

`svg_targets` 检查七个目标的代码生成、资源验证、静态回退、脚本后端和运行时打包，并把生成物放到 `target/svg-generation-test/` 供对应平台工具链检查。代码生成通过不能替代目标设备上的构建和视觉检查。

2026-10-17 验证：

- Bern 编译器、呈现后端及 SVG 文档测试通过；共享动画运行时的 6 项 Node 测试通过。六目标生成测试另检查 Android 构建清理会保留 `BernSvg.kt`，资源从 `bern/resources/` 加载。
- 一个真实应用的 macOS 和 Web 整包构建通过，示例的 macOS、iOS 模拟器包、Android APK 和 Web 构建通过。iOS 与 Android 本轮未运行设备上的交互验收；Windows 与 Linux 未做原生编译或设备验收。
- 该应用在 macOS 的引导与关于页透明 Logo 显示正确，并实测双击动作。Web 实测 130／200 尺寸引导、104 尺寸关于页、入场以及眨眼／摸头／坏笑动作；退出引导后 iframe 数量归零。
- Web 示例实测暂停后旋转矩阵跨两次采样保持不变，暂停中重播回到单位矩阵；切换倍速并恢复后继续运动，没有控制台错误。倍率和 SMIL 时钟计算由 Node 测试覆盖。

2026-10-18 补充（Linux）：同一真实应用在 Ubuntu 24.04 虚拟机上原生构建通过（GTK 运行时启用 `svg`；虚拟机只有 WebKitGTK 6 运行库，构建时用手写的 pkg-config 文件链接）。修正了 GTK 呈现器中 `settings()` 在 `WidgetExt` 与 `WebViewExt` 间的歧义调用（此前从未在 Linux 编译）。经 Broadway 实测引导页的 200 尺寸 Logo 居中显示在格纹插画上；虚拟机限制非特权用户命名空间，需以 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` 运行，否则应用在启动时中止。同日为 `.deb` 加入 AppArmor 配置：`examples/svg-animation` 的包含配置与维护脚本，配置在虚拟机的 AppArmor 4.0 上解析通过；在系统自带、形式相同的配置下，同一程序的 WebKitGTK 沙箱建立成功并正常显示 SVG。载入这份配置本身需要 root，未实测安装。双击等交互与暂停、倍速未实测。

2026-10-19 补充（Windows）：同一真实应用在 Windows 11 的 PC 上原生构建通过。修正了 Windows 呈现器此前未编译过的问题：运行时没有放进 `B` 命名空间（`Len`、`BindLen` 等找不到）；WinUI 3 一侧只放行地址为 `about:blank` 的导航，`NavigateToString` 的导航被拦下，文档没有载入（显示为白块）；改用 Win32 呈现器后背景才透明。引导页 200 尺寸的 Logo 在 WinUI 3 与 XAML Islands 下都透明地居中显示在格纹插画上（应用自己的窗口快照）；弹出动画中的位置偏差由逐帧跟随修正。双击等交互、暂停与倍速、关于页（滚动视图中的裁剪）未实测。

2026-10-21 补充（Android）：`WebView` 离开屏幕时不立即销毁，换成空白页（文档随之释放）后按文档与活动留一个；30 秒内同一文档再显示时取回并重新载入文档，脚本与动画仍从头开始，与新建相同；超过 30 秒、活动停止或渲染进程退出时释放。留下的视图处理 `onRenderProcessGone`：它不可见时渲染进程可能被系统回收，不处理会连带结束应用。在 MIX 2S（Android 15）上主线程新建一个 `WebView` 约 50 ms，同一真实应用再次打开“关于”页的切页帧从 97–99 ms 降到 38–39 ms；代价是渲染进程在离开后多存在最多 30 秒（私有脏页约 21 MB）。图标出现那一帧在 RenderThread 上的绘制（9–22 ms）不变。测量见 [Android Compose 实现记录](Android-Compose.md)“切页后由 RenderThread 决定的帧”。其他平台没有改，也没有量它们新建文档视图的耗时。

## 构建期静态回退

`window.webview` 是目标能力常量，Bern 在检查和生成前裁剪条件分支、无用组件，并折叠属性中的条件表达式。tvOS 固定关闭；macOS、iOS、Android、Windows、Linux 默认开启，也可通过 `webview = false` 生成无 WebView 发行版。Web 使用浏览器能力。

无 WebView 时，`Svg` 自动降低成 `Image`，保留尺寸、fit 和辅助名称，移除播放、倍速、重播与互动属性。PNG 在构建目录的 `__bern_static_svg/` 生成并随资源打包；resvg 只链接到构建工具。默认按固有尺寸两倍输出，每条边上限 4096 像素。

`static_source` 用来指定动画完成后的画面，尤其适合入场动画和由脚本生成图形的 SVG；构建器不执行动画脚本来寻找最终帧。该文件也应列入 `[resources].paths`。Erika 的 Logo 使用最终矢量稿，因此无 WebView 的版本直接显示完整 Logo。
