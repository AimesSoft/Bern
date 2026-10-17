# 当前组件用法

更新：2026-10-17。本文说明当前可声明的接口；完整属性、必填项、子节点与动作规则见[组件属性表](Component-Properties.md)，各后端的接受范围见[平台能力表](Platforms.md)。可直接构建的工程在 `examples/hello`、`examples/showcase`、`examples/planner`、`examples/gallery`；下文片段省略所在组件、协议与状态定义。

## 声明与数据来源

在 `#[component]` 函数内用 `ui!` 声明树。静态用户可见文案使用 `tr!`，引擎状态只读，`draft` 会话字段可由标准控件更新，业务修改通过 `=>` 绑定的类型化命令执行。`id` 是控制入口的稳定身份，`label` 同时用于可访问名称。`label_hidden: true` 隐藏可见文字，仍须提供 label；当前支持它的控件包括 TextInput、Switch、Slider、Picker。

长度通常使用逻辑单位，`Length` 接受数字或受限数值表达式。`Number` 只接受静态数字，`Flag` 只接受构建期布尔值。条件、列表和状态引用仍受编译器检查；目标不支持的组件特性由能力检查拒绝，不能只根据组件名称判断平台覆盖。

## 坐标场景与滚动内容

`ScaleBox(width:, height:)` 定义固定逻辑画布，在父容器给出的可用区域内等比缩放并居中，保留整个画布。父容器需要提供可用宽高。`Positioned(x:, y:, width:, height:)` 从场景左上角定位图层，坐标允许为负；位置和尺寸可绑定状态，配合 `animate` 平滑变化。画布之外的内容会被裁切。

```rust
ScaleBox(width: 1280, height: 720) {
    Positioned(x: 0, y: 0, width: 1280, height: 720) {
        Image(source: app.poster, width: 1280, height: 720, fit: cover)
    }
    Positioned(x: 48, y: 40, width: 1184, height: 32) {
        Marquee(duration: 12, phase: 0.25, paused: app.paused, opacity: 0.8) {
            Text(content: app.message, font_size: 24)
        }
    }
}
```

`Marquee` 的 `duration` 是一轮从右向左经过内容区的秒数，当前运行时至少使用 1 秒；`phase` 是起始周期比例，建议 0–1，默认 0。`paused` 默认 false，`opacity` 默认 1 并限制到 0–1。时钟由呈现层管理，暂停后可以继续，减少动态效果时静态呈现。它只表达循环滚动内容，不提供逐帧回调、视频时钟同步或弹幕调度引擎。

这三个组件由 `scaled_scene` 能力提供，当前支持 SwiftUI 和 Web。SwiftUI 使用一次整体缩放；Web 通过 ResizeObserver 更新缩放并在卸载时解除观察。iOS 共用生成路径，本轮未做播放器设备交互验收。

## 控件容器与尺寸

`ControlGroup(height: 40)` 将功能相近的控件放进一个共享胶囊，保留子控件各自的点击、菜单和浮层。高度默认 40；`label` 可选，Web 将其用作组的可访问名称，SwiftUI 当前保留子控件的可访问名称。SwiftUI 在 macOS / iOS 26 起使用交互式 Liquid Glass，旧系统使用系统材质胶囊；Web 使用自己的胶囊样式。

```rust
ControlGroup(height: 40) {
    IconButton(id: subtitles, icon: "captions.bubble",
        label: tr! { "zh-Hans" => "字幕", "en" => "Subtitles" }) => app.toggle_subtitles()
    IconButton(id: fullscreen, icon: "arrow.up.left.and.arrow.down.right",
        label: tr! { "zh-Hans" => "全屏", "en" => "Fullscreen" }) => app.toggle_fullscreen()
}
```

独立控件可用 `Button`、`Menu`、`Picker`、`Switch`、`SplitButton` 的 `shape: capsule` 和 `height` 声明容器及高度。相邻菜单使用一致的 shape / height，相关操作可共享 ControlGroup；形状本身不会改变命令语义。`IconButton(prominence: plain)` 用于需要普通图标的独立按钮，`font_size` 调整图标字号；它不代替容器高度。`prominence: prominent` 表达强调按钮。

带图标与文字的返回入口使用一个 `Button(icon:, label:)`，将两者作为同一控件进行布局与命中。容器大小属于声明，不能通过移除容器来回避内容溢出。普通控件的默认外观仍由目标平台决定。

## 引擎受控控件

| 控件 | 会话草稿来源 | 引擎快照来源 | 命令接收到的值 |
| --- | --- | --- | --- |
| Switch | `value: draft.enabled` | `checked: app.enabled` | 用户刚选择的布尔值 |
| Picker | `value: draft.appearance` | `selected: app.appearance` | 用户刚选择的枚举值 |
| Slider | `value: draft.volume` | `current: app.volume` | 用户提交的数值 |

每个控件只能选择一列作为来源，不能两者都写或都不写。引擎来源必须提供动作；动作参数中与 `checked` / `selected` / `current` 完全相同的表达式被替换为用户的新值，不替换其子表达式或其他参数。Picker 的枚举变体必须具有标签。

```rust
Slider(id: volume,
    label: tr! { "zh-Hans" => "音量", "en" => "Volume" },
    label_hidden: true, current: app.volume, min: 0, max: 1, step: 0.01
) => app.set_volume(app.volume)
```

受控 Slider 拖动中显示本地预览，拖动结束提交；引擎快照不会在编辑中抢回滑块位置。`PickerOption(value:, label:)` 可显式声明选项并把实时计数插入 label。受控值由 SwiftUI 与 Web 的 `controlled_values` 支持，显式选项另由 `modal_flow_timer` 检查。

## 图片、排版与共享元素

| 声明 | 用途与边界 |
| --- | --- |
| `Image(aspect: 0.6666666667, fit: cover)` | 随容器宽度保持 2:3，避免加载前后画框跳动；比例必须为正 |
| `Image(corner:)` | 裁切图片圆角，可绑定长度 |
| `Image(rendering: template)` | 按当前前景色渲染模板图片；原始彩色图片使用 original |
| `Grid(row_gap:)` | 独立设置行距；省略时沿用 gap |
| `Text(font_size:, lines:)` | 字号可绑定长度，lines 指定行数上限；动态字号要求 scaled_scene |
| `Row(align: baseline)` | 按第一行文字基线对齐 |
| `padding_x` / `padding_y` | 支持的布局容器分别设置水平、垂直内边距；见属性表 |

共享元素使用 `hero: poster, hero_key: item.id`：静态 hero 名称区分用途，动态 key 区分具体条目。两端须处于同一窗口且键一致；不要同时保留多个同键可见元素。`hero_key` 不替代 ForEach 的稳定 key。

macOS 的 Image 使用窗口上层像素覆盖层，Web 使用视口坐标的图片副本，处理跨侧栏与 ScaleBox 的坐标变化；临时画面不接受点击、不进入无障碍树，减少动态效果时不飞行。macOS / Web 已验收图片从舞台展开、缩到侧栏及恢复。iOS 和 macOS 的其他视图仍沿用匹配几何；Video 连续画面的跨分栏传递不在此次图片验收范围。

## 同窗口引导、侧栏与搜索

| 接口 | 当前语义 |
| --- | --- |
| `Modal(width:, height:)` | 窗口内居中面板，尺寸受可用窗口约束，背景遮罩；正文可滚动、页脚可单独放置。背景子树使用 `Stack(enabled: false)` 禁用 |
| `Flow(gap:)` | 按内容固有尺寸从左到右换行 |
| `Timer(interval:, enabled:) => command()` | 挂载且启用时定期提交命令；卸载、禁用时取消，间隔单位为秒。不能用来驱动逐帧绘制 |
| `Accordion(count:, selected:, collapsed_width:, height:, gap:, corner:)` | 一列展开，其余列固定折叠宽度；selected 从 0 开始，animate 驱动列宽及位置 |
| `Sidebar(width:)` | 固定列宽。`SidebarHeader` 固定在列表上方，`SidebarFooter` 固定在下方 |
| `SidebarItem(badge:)` | 显示正数计数；模块页面清单的 badge 可绑定模块状态字段 |
| `Search(style: palette)` | 从工具栏入口打开命令面板；`panel_prompt`、`width`、`height` 设置提示与面板尺寸 |
| `SearchItem(id:, label:) => command()` | 面板结果，可带 icon、detail、hint 或自定义子内容；`navigation: false` 不参与方向键选择，仍可点击与 Tab 聚焦 |

搜索面板支持方向键、Enter、Esc、外部点击及关闭后的焦点恢复，当前只在 macOS 和 Web 开放。其他表内扩展由 SwiftUI 和 Web 提供；应用的布局与平台选择仍须通过目标能力检查。

## 动效与空闲状态

- `appear` / `disappear` 控制结构变化；`animate` 控制支持的长度变化，`reflow` 让相邻元素让位。动效名称来自 `style!`。
- `grow(edge: bottom, bouncy)` 固定某条边展开；`appear_delay` 必须配合 appear，延迟非负，可与 stagger 相加。普通状态更新不会重播入场，卸载取消等待。
- `nudge` 响应布尔触发；`nudge_key` 让不同事件值触发提示，`nudge_scale` 指定幅度。容器的 `reduce_motion` 与系统减少动态效果偏好合并。
- `Window(idle_after:, idle_enabled:, immersive:)` 控制空闲与沉浸。idle_after 单位为秒，`window.idle` 为只读会话状态。idle_enabled 默认 true；关闭后清除闲置并停用检测，开启后重新计时。SwiftUI 和 Web 已接入此开关（能力 `idle_control`）；其他目标当前未消费该属性，`bern check` 会对它们给出警告，空闲检测在那里始终开启。

macOS 编辑文字、菜单或子窗口交互时保留控制层；Web 仅在启用状态变化时重启计时器，播放进度快照更新不会阻止空闲隐藏。UIKit 共享启停逻辑，播放器设备交互尚未验收。

## 持久化与本地预览

`bern::storage::Store::host()` 使用宿主安装的、按应用 bundle ID 隔离的小数据存储。macOS / iOS 使用 Application Support，Windows 使用 APPDATA，Linux 使用 XDG_CONFIG_HOME / ~/.config；Web app 使用同源 localStorage，WebUI 使用原生引擎宿主。key 限简单文件名；不存在的值与空值不同，读写失败返回错误。`Store::directory` 适用于指定目录的原生测试，没有宿主时的纯引擎测试不持久化。

```sh
bern run --target web_app --port 4173
```

目标名由项目 Bern.toml 定义。`--port` 只用于浏览器 app，范围 1–65535；省略时使用空闲端口。固定端口保持相同 origin，避免开发时端口改变而读到另一份 localStorage。浏览器权限、容量与清理站点数据会影响持久化。

## 其他接口

动画 SVG 是按需使用的独立组件，接口、资源声明、平台依赖与生命周期见[专项文档](../Implementation/Animated-SVG.md)。视频播放器、查询、模块、主题及命令协议的完整语义继续见[设计文档](../Design.md)与对应实现记录。
