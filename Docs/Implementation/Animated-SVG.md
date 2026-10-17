# 动画 SVG

Bern 内置 `bern-svg` 文档运行时和 `Svg` 组件。它保留 SVG 原文件的矢量图形、CSS 关键帧、SMIL、渐变、滤镜、蒙版、文字和文档内 JavaScript。渲染由平台的 SVG/DOM 引擎完成，不转成 PNG 或预录动画。

```rust
Svg(
    source: "assets/logo.svg",
    width: 104,
    height: 104,
    label: tr! { "zh-Hans" => "应用图标", "en" => "App icon" },
    playing: app.logo_playing,
    speed: app.logo_speed,
    restart: app.logo_revision
)
```

资源需列入 `Bern.toml` 的 `[resources].paths`。构建检查 XML 和 SVG 命名空间；资源缺失、错误格式和外部实体会报错。`Image` 继续用于位图，`Svg` 明确表示带独立文档执行环境的矢量资源。

## 接口

| 属性 | 含义 |
| --- | --- |
| `source` | 已声明的 `.svg` 文件，构建期确定 |
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
| Windows / WinUI 3 | `WebView2` 控件 | Microsoft Edge WebView2 Runtime |
| Windows / XAML Islands | WebView2 Win32 呈现器跟随 XAML 占位区域 | 同上；原生子窗口有 HWND airspace 限制 |
| Linux / GTK 4 | WebKitGTK 6 `WebView` | 使用 `Svg` 才启用 `bern-gtk/svg`；开发包 `libwebkitgtk-6.0-dev`，Debian 运行依赖 `libwebkitgtk-6.0-4` |
| Web | 透明、独立源的 sandbox iframe | 宿主浏览器 |

这是浏览器 SVG 标准能力的封装，渲染细节和支持的标准版本仍取决于平台引擎版本。没有宣称重新实现一套“全规范”Rust SVG/JavaScript 引擎。没有使用 `Svg` 的应用不创建这些文档视图；GTK 不引入 WebKit 依赖，SwiftUI 不生成 WebKit 呈现代码。

## 资源边界

SVG 是打包的、自包含的应用资源：图片／字体请嵌入为 data URI，样式和脚本放在文件内。文档不暴露 Bern 引擎、原生命令或宿主 DOM；CSP 限制外部子资源和网络请求，运行时阻止链接点击，原生宿主限制导航和弹窗。Web 的 iframe 使用独立源的 `sandbox="allow-scripts"`，但这不能禁止脚本导航其自身文档，因此只接受应用自己提供的 SVG，不作为不可信脚本的通用沙箱。

Android、Windows 和 Linux 需要表中对应的 WebView 运行环境。当前不提供运行环境缺失时的静态图片回退；Linux 的 WebKitGTK 是启用 `svg` 特性后的动态链接依赖，需要随系统安装。XAML Islands 的子 HWND 不支持任意祖先裁切或与其他 XAML 图层交错。

## 验证

`examples/svg-animation` 是所有目标共用的示例：CSS 旋转、SMIL 半径动画、渐变、脚本双击变色，以及原生的暂停、倍速、重播按钮。

```sh
cargo test -p bern-svg -p bern-compiler
node --test crates/bern-svg/tests/runtime.test.mjs
cargo test -p bern-cli --test svg_targets
cd examples/svg-animation
../../target/debug/bern build --target macos
```

`svg_targets` 检查六个目标的代码生成、资源验证和运行时打包，并把生成物放到 `target/svg-generation-test/` 供对应平台工具链检查。代码生成通过不能替代目标设备上的构建和视觉检查。

2026-10-17 验证：

- Bern 编译器、呈现后端及 SVG 文档测试通过；共享动画运行时的 6 项 Node 测试通过。六目标生成测试另检查 Android 构建清理会保留 `BernSvg.kt`，资源从 `bern/resources/` 加载。
- 一个真实应用的 macOS 和 Web 整包构建通过，示例的 macOS、iOS 模拟器包、Android APK 和 Web 构建通过。iOS 与 Android 本轮未运行设备上的交互验收；Windows 与 Linux 未做原生编译或设备验收。
- 该应用在 macOS 的引导与关于页透明 Logo 显示正确，并实测双击动作。Web 实测 130／200 尺寸引导、104 尺寸关于页、入场以及眨眼／摸头／坏笑动作；退出引导后 iframe 数量归零。
- Web 示例实测暂停后旋转矩阵跨两次采样保持不变，暂停中重播回到单位矩阵；切换倍速并恢复后继续运动，没有控制台错误。倍率和 SMIL 时钟计算由 Node 测试覆盖。
