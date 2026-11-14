[English](../../Implementation/Animated-SVG.md) · **简体中文**

# 动画 SVG

Bern 在 `Svg` 组件背后内置纯 Rust 动画 SVG 编译器与播放器（`bern-svg`）。`Svg` 与 `bern-svg` 在任何目标平台都不使用 WebView 或 JavaScript 引擎：静止的 SVG 在构建期降级为 PNG `Image`，带动画或交互的 SVG 在构建期编译为紧凑二进制 `BSVG` 场景（`__bern_anim_svg/<source>.bsvg`），运行时通过进程内 `tiny-skia` SIMD 栅格化在 SwiftUI（macOS、iOS、tvOS、watchOS）、Compose（Android、Wear OS）、WinUI 3 与 XAML Islands、GTK 4、GPUI 以及 WebAssembly（`<canvas>`）上原生播放。

## 快速上手

1. 在 `Bern.toml` 的 `[resources].paths` 中声明 `.svg` 资源：

```toml
[resources]
paths = ["assets/logo-animated.svg"]
```

2. 在 `ui!` 中放置 `Svg` 组件：

```rust
Svg(
    source: "assets/logo-animated.svg",
    width: 120,
    height: 120,
    label: tr! { "zh-Hans" => "应用徽标", "en" => "Application logo" },
    playing: app.logo_playing,
    speed: app.logo_speed,
    restart: app.logo_revision
)
```

3. 使用标准 CSS `@keyframes`、CSS `transition` 或 SMIL `<animate>` / `<animateTransform>` 编写动画，并在根 `<svg>` 上通过 `data-bern-*` 属性声明点击或双击状态切换（无需编写任何 JavaScript）：

```xml
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="0 0 200 200"
     data-bern-ready="1.8s"
     data-bern-dblclick="random(do-wink: 1.5s, do-nod: 2.2s, do-wave: 2.0s)">
  <style>
    .mark { transform-box: fill-box; transform-origin: 50% 50%; animation: pop .6s cubic-bezier(.3, .7, .4, 1) both; }
    .do-wink .eye-left { transform-box: fill-box; transform-origin: 50% 75%; animation: wink 1.5s ease-in-out both; }
    .do-nod  .head     { transform-box: fill-box; transform-origin: 50% 90%; animation: nod 2.2s ease-in-out both; }
    .do-wave .hand     { transform-box: fill-box; transform-origin: 20% 90%; animation: wave 2.0s ease-in-out both; }
    @keyframes pop  { 0% { transform: scale(0); } 65% { transform: scale(1.06); } 100% { transform: none; } }
    @keyframes wink { 0%, 100% { transform: scaleY(1); } 15%, 80% { transform: scaleY(0.05); } }
    @keyframes nod  { 0%, 100% { transform: rotate(0); } 35% { transform: rotate(6deg); } 70% { transform: rotate(-3deg); } }
    @keyframes wave { 0%, 100% { transform: rotate(0); } 30% { transform: rotate(-18deg); } 60% { transform: rotate(14deg); } }
    @media (prefers-reduced-motion: reduce) { .mark { animation: none; } }
  </style>
  <g class="mark">
    <g class="head">
      <circle cx="100" cy="100" r="72" fill="#3E5BD8"/>
      <g class="eye-left"><ellipse cx="76" cy="94" rx="8" ry="14" fill="#DCE6FF"/></g>
      <ellipse cx="124" cy="94" rx="8" ry="14" fill="#DCE6FF"/>
    </g>
    <g class="hand"><circle cx="162" cy="138" r="14" fill="#F48BA6"/></g>
  </g>
</svg>
```

## 组件接口

| 属性 | 含义 |
| --- | --- |
| `source` | 必填，已在 `Bern.toml` 声明的 `.svg` 文件，构建期校验并编译 |
| `static_source` | 可选，为兼容既有源码保留的 `.svg` 资源；动画 SVG 在全部目标平台均原生运行 |
| `width`、`height` | 必填，逻辑点尺寸（可绑定数值状态），运行时按屏幕物理像素倍率清晰栅格化 |
| `label` | 无障碍名称；省略时标记为装饰图形 |
| `fit` | `contain`（默认）、`cover`、`fill`，对应 SVG 的 `meet`、`slice`、`none` |
| `playing` | 布尔状态，默认 `true`；暂停或恢复 CSS、过渡、SMIL 与定时交互时钟 |
| `speed` | 正数状态，默认 `1`；统一缩放时间线速率，非正或非有限值保持原有效倍率 |
| `restart` | 数值状态；值变化时将时间线归零重播，同时保留已切换的常驻状态类 |
| `interactive` | 构建期布尔值，默认 `true`；启用指针交互（`click`、`dblclick`、`:hover`、`:active`） |
| `data`、`external_links` | 为兼容保留的可选属性；原生 `BSVG` 场景不执行脚本也不发生导航 |

## 声明式交互

无需为了点击切换类名而启动浏览器 DOM 与 JavaScript 虚拟机，`bern-svg` 直接将根 `<svg>` 上的声明式属性编译进 `BSVG` 状态机：

| 属性 / 动作 | 语法与行为 |
| --- | --- |
| `data-bern-ready` | 可选入场保护时长，如 `"2.3s"` 或 `"500ms"`；入场时间线未到达该时刻前忽略点击与双击；系统开启「减少动态效果」时在 `0s` 立即就绪 |
| `data-bern-click` | 单击／轻点（或电视遥控器确认键）触发的动作 |
| `data-bern-dblclick` | 双击／双点触发的动作 |
| `toggle(class)` | 在根 `<svg>` 上切换 `class` 的有无；可配合 CSS `transition` 平滑过渡（例如 `data-bern-dblclick="toggle(alternate)"`） |
| `trigger(class: 1.5s)` | 为 `<svg>` 加上 `class` 并从 `0%` 播放对应 `@keyframes`，经过 `1.5s` 动画时间后自动移除（时长可省略） |
| `cycle(a: 1.2s, b: 1.2s, none)` | 每次触发按顺序轮换状态类；`none` 表示清空当前状态类 |
| `random(a: 1.5s, b: 2.6s, c: 2.8s)` | 从候选动作中随机挑选一个且**连续两次绝不重复**（两个及以上候选项时），从 `0%` 启动动画并在对应时长结束后自动移除类名 |

当 `interactive` 为 `true` 时，`<svg>` 或内部图形上的 CSS `:hover` 与 `:active` 伪类也会自动生效。

**与独立网页预览共用同一 `.svg` 文件：** 若同一个 `.svg` 文件同时通过 `<object>` 嵌入独立 HTML 设计稿预览，可在保留浏览器 `<script>` 的同时在 `<svg>` 上声明 `data-bern-*`。只要存在 `data-bern-*`，`bern-svg` 会采用声明式交互并在编译 `BSVG` 时安全剥离 `<script>`；若包含 `<script>` 或 `on*` 事件属性但未声明 `data-bern-*`，`bern check` 与 `bern build` 会在编译期报错并提示改用 `data-bern-click` / `data-bern-dblclick`。

## 支持的 SVG、CSS 与 SMIL 范围

`bern-svg` 专为动画图标、品牌徽标、状态指示与交互矢量插画量身定制：

- **几何与结构**：`<svg>`、`<g>`、`<defs>`、`<use>`、`<path>`（完整支持 SVG 1.1/2 指令 `M/m`、`L/l`、`H/h`、`V/v`、`C/c`、`S/s`、`Q/q`、`T/t`、`A/a`、`Z/z`）、`<rect>`（含圆角 `rx`/`ry`）、`<circle>`、`<ellipse>`、`<line>`、`<polyline>`、`<polygon>`。
- **填充、描边与裁剪**：纯色（`#hex`、`rgb()`/`rgba()`、命名色）、`currentColor`、`light-dark(浅, 深)`、`<linearGradient>` 与 `<radialGradient>`（支持 `objectBoundingBox`、`userSpaceOnUse` 与 `gradientTransform`）、`fill-opacity`、`fill-rule`（`nonzero`、`evenodd`）、`stroke`、`stroke-width`、`stroke-opacity`、`stroke-linecap`、`stroke-linejoin`、`stroke-dasharray`、`stroke-dashoffset`、`<clipPath id="...">`（`clip-path="url(#...)"`）以及 `<g opacity="...">` 组级离屏透明合成。
- **CSS 层叠与媒体查询**：标签、`#id`、`.class`、复合类（`.eye.l`）、直接子代（`>`）与后代选择器（`.do-wink .rx`）、`:hover`、`:active`、`!important`、`@media (prefers-color-scheme: dark|light)` 与 `@media (prefers-reduced-motion: reduce)`。
- **CSS `@keyframes` 与 `transition`**：支持 `animation` 简写及各展开属性（`animation-name`、`animation-duration`、`animation-delay`、`animation-timing-function`、`animation-iteration-count`、`animation-direction`、`animation-fill-mode`）、逐属性独立关键帧点、`linear` / `ease` / `ease-in` / `ease-out` / `ease-in-out` / `cubic-bezier(...)` / `steps(...)` 分段缓动、`transform-box: fill-box | view-box`（构建期精确计算贝塞尔极值包围盒 `getBBox()`）、百分比与像素 `transform-origin`、前缀对齐变换插值与 W3C 2D 极分解矩阵插值回退，以及 `fill`、`stroke`、`opacity`、`transform`、`stroke-width`、`stroke-dashoffset` 的 CSS `transition` 过渡。
- **SMIL 时间线**：`<animate>`、`<set>` 与 `<animateTransform>`（支持 `values`、`from`/`to`、`keyTimes`、`keySplines`、`calcMode`、`begin`、`dur`、`repeatCount`、`fill="freeze"`）。

不在 `bern-svg` 覆盖范围内的特性（请改用由 `resvg` 离线渲染的静态 SVG，或使用 Bern 原生 `Text` / `WebPage` 组件）：动画 SVG 内的 `<text>` 文字排版、`<filter>` 滤镜（如 `feGaussianBlur`）、`<foreignObject>`、外部网络资源引用以及任意 JavaScript 代码执行。

## 双阶段编译架构与静默零唤醒休眠

1. **构建期分流与字节码预编译（`bern-target-api::svg`）**：
   - **静态 SVG（`bern_svg::picture`）**：不含动画、过渡、`:hover`/`:active`、`<script>` 与 `data-bern-*` 的文档在构建期由 `resvg` 一次性栅格化为 `__bern_static_svg/<source>.png`（明暗双色文档额外生成 `.dark.png`，纯 `currentColor` 图标设为 `rendering: template`），并降级为普通 `Image`。
   - **动画 SVG（`bern_svg::compile`）**：构建期编译为 `__bern_anim_svg/<source>.bsvg`。XML 解析、CSS 选择器解析、圆弧转三次贝塞尔及精确 `fill-box` 包围盒全部在构建期完成；最终发行的原生与 WASM 产物以 `default-features = false` 链接 `bern-svg`，二进制中不携带任何 XML/CSS 解析器。
2. **进程内场景缓存与静默休眠（`bern-ffi::svg`）**：
   - 解码后的 `Scene`（含预构建的 `tiny_skia::Path`）在进程内按资源路径缓存为共享的 `Arc<Scene>`，重复进入页面时零解析直接复用。
   - 每次 `bern_svg_render` 返回 `changed`（画面未变时跳过纹理解码上传）与 `active`（当前是否仍有未结束的 CSS 动画、CSS 过渡、SMIL 轨道或定时交互）两个状态位。
   - 一旦有限时长的入场动画或双击动作播放结束（`active == false`），所有前端立即暂停帧时钟（Apple 端 `TimelineView`、Android/Wear OS 端 `withFrameNanos`、Windows 端 `CompositionTarget::Rendering`、GTK 4 端 `add_tick_callback`、GPUI 端 `request_animation_frame`、Web 端 `requestAnimationFrame`），静止期间 CPU 占用降至 **0%**，直至下一次交互或状态变化将其唤醒。

## 各目标平台呈现层

| 目标 | 原生呈现方式 | 运行时依赖 |
| --- | --- | --- |
| macOS / iOS / tvOS / watchOS | SwiftUI `TimelineView` + `CGImage`（`RgbaPremul`） | 进程内 `bern-ffi`（`bern-svg`），零 `WebKit` 依赖 |
| Android / Wear OS | Compose `Canvas` + JNI `DirectByteBuffer` → `Bitmap` | 进程内 `libbern_app.so`，零 `android.webkit.WebView` 依赖 |
| Windows（WinUI 3 与 XAML Islands） | XAML `Image` + `WriteableBitmap`（`BgraPremul`） | 进程内 `bern-ffi`，零 `WebView2` 依赖，无子 HWND 遮挡问题 |
| Linux / GTK 4 | `GtkPicture` + `gdk::MemoryTexture`（`R8g8b8a8Premultiplied`） | 进程内 `bern-gtk`，零 `libwebkitgtk-6.0-4` 依赖，无需 AppArmor `userns` 配置 |
| GPUI（macOS / Windows / Linux） | GPUI `canvas` + `RenderImage`（`BgraStraight`） | 进程内 `bern-gpui` |
| Web（`web_app` 与 `web_ui`） | `<canvas class="bern-svg">` + `ImageData`（`RgbaStraight`） | `app.wasm` 导出函数，零 `<iframe>` |

## 作为独立 Rust 库使用

除在 Bern `ui!` 中声明 `Svg` 外，开发者也可在任意 Rust 项目中直接调用 `bern-svg`：

```rust
use bern_svg::{Config, Fit, PixelFormat, Player, PointerEvent, Scene};
use std::sync::Arc;

let bsvg_bytes = bern_svg::compile(svg_source)?;
let scene = Arc::new(Scene::from_bytes(&bsvg_bytes)?);
let mut player = Player::new(scene);

player.configure(Config {
    playing: true,
    speed: 1.0,
    fit: Fit::Contain,
    dark: false,
    ..Config::default()
});

let (step, rgba_premul) = player.render(0.0, 256, 256, PixelFormat::RgbaPremul);
if step.active {
    // 继续请求下一帧；当 `step.active` 变为 false 时暂停帧循环即可零开销休眠。
}
let wake = player.pointer(PointerEvent::DoubleClick(0.5, 0.5));
```

## 验证

`examples/svg-animation` 覆盖 CSS 旋转、SMIL 半径脉动、径向渐变、声明式 `data-bern-dblclick="toggle(alternate)"` 配合 CSS 颜色过渡，以及原生播放／暂停、倍速与重播状态绑定：

```sh
cargo test -p bern-svg -p bern-ffi -p bern-compiler
cargo test -p bern-cli --test svg_targets
cargo test -p bern --test docs
```
