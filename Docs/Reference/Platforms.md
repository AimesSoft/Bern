# 平台能力与验证范围

更新：2026-10-17。下表按 [bern-target-api](../../crates/bern-target-api/src/lib.rs) 与各后端 `TargetDescriptor.features` 整理。macOS 在共用 SwiftUI 清单之外单独开放搜索面板；iOS 不开放。

## 编译期功能支持

“支持”表示后端声明接受对应特性；“—”表示未声明，用到该特性的声明会被相应能力检查拒绝。它不表示所有属性行为、视觉效果或设备场景都已经验收。颜色覆盖、动效、窗口和媒体还有独立的能力描述，不能只看此表推断完整支持。

| 功能标记 | 对应能力 | macOS | iOS | Web | Android | Linux | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `button_content` | 按钮内自定义内容 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `reveal` | 悬停显现（无指针目标常显） | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toolbar_search` | 工具栏搜索 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `search_results` | 搜索建议结果 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `session_actions` | 会话字段动作 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `toggle_button` | 开关按钮样式 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `dialog_windows` | 对话框窗口 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `bound_lengths` | 状态绑定长度 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `appearance` | 局部明暗外观 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `shortcuts` | 键盘快捷键 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `runtime_theme` | 运行时主题选择 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `runtime_images` | 引擎提供图片 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `picture_fills` | 图片取色与渐隐填充 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `file_pick` | 文件选择 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `scroll_snap` | 滚动停靠 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `rails` | 比例轨道布局 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `popover_button` | 浮层按钮 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |
| `text_weights` | 文本字重样式 | 支持 | 支持 | 支持 | — | — | — |
| `text_lines` | 文本行数上限 | 支持 | 支持 | 支持 | — | — | — |
| `image_corners` | 图片圆角 | 支持 | 支持 | 支持 | — | — | — |
| `sidebar_width` | 固定侧栏宽度 | 支持 | 支持 | 支持 | — | — | — |
| `controlled_values` | Switch / Picker / Slider 引擎受控值 | 支持 | 支持 | 支持 | — | — | — |
| `search_palette` | 搜索命令面板 | 支持 | — | 支持 | — | — | — |
| `image_template` | 模板图片 | 支持 | 支持 | 支持 | — | — | — |
| `adaptive_images` | 图片比例与网格独立行距 | 支持 | 支持 | 支持 | — | — | — |
| `scaled_scene` | ScaleBox / Positioned / Marquee、动态字号 | 支持 | 支持 | 支持 | — | — | — |
| `accordion_layout` | 手风琴列布局 | 支持 | 支持 | 支持 | — | — | — |
| `capsule_controls` | 胶囊控件和 ControlGroup | 支持 | 支持 | 支持 | — | — | — |
| `modal_flow_timer` | Modal / Flow / Timer / PickerOption | 支持 | 支持 | 支持 | — | — | — |
| `page_badges` | 导航和模块页面计数 | 支持 | 支持 | 支持 | — | — | — |
| `edge_grow` | 按边展开过渡 | 支持 | 支持 | 支持 | — | — | — |
| `delayed_appear` | 延迟入场 | 支持 | 支持 | 支持 | — | — | — |
| `toolbar_leading` | 工具栏前缘项和文字 | 支持 | 支持 | 支持 | — | — | — |
| `axis_padding` | 分轴内边距 | 支持 | 支持 | 支持 | — | — | — |
| `icon_menus` | 纯图标菜单 | 支持 | 支持 | 支持 | — | — | — |
| `icon_labels` | 纯图标 Label | 支持 | 支持 | 支持 | — | — | — |
| `row_baseline` | 行内文字基线对齐 | 支持 | 支持 | 支持 | — | — | — |
| `sidebar_header` | 固定侧栏顶栏 | 支持 | 支持 | 支持 | — | — | — |
| `untitled_detail` | 无标题详情 | 支持 | 支持 | 支持 | — | — | — |
| `size_classes` | 四档尺寸适配 | 支持 | 支持 | 支持 | 支持 | 支持 | 支持 |

## 不由上述功能标记完整表达的能力

| 能力 | 当前范围 |
| --- | --- |
| `Window(idle_enabled:)` | SwiftUI / Web 消费启停状态；Compose / GTK / WinUI 3 尚未接入，也没有独立能力检查，不能依赖声明被接受来推断生效 |
| 图片跨分栏 Hero | macOS 使用窗口级覆盖层，Web 使用视口级副本；iOS 与 macOS 其他视图沿用匹配几何。Video 连续画面传递另行验证 |
| 本地持久化 | 原生 FFI 宿主提供文件存储，Web app 使用同源 localStorage；未安装存储宿主时不持久化 |
| 可选 Svg | 六目标有生成路径；专属依赖及验证见[动画 SVG](../Implementation/Animated-SVG.md)，不改变普通界面的默认实现 |

## 最近新增能力的验证

这一节只描述 2026-10-15 至 17 日的新增能力；已有平台的历史验收见各实现记录。构建成功不代替实际设备交互。

| 范围 | 代码 / 构建证据 | 实际交互范围 |
| --- | --- | --- |
| macOS 的 Erika 布局、控件与图片 Hero | 应用构建及相关生成器检查；图片 Hero 有真实 AppKit 容器回归 | 播放器覆盖层、分组、返回入口、舞台图片展开、缩到侧栏及恢复 |
| Web 的 Erika 适配 | WASM 构建、生成器与浏览器运行时回归 | Chromium 桌面及指定窄屏场景；桌面 Safari 27.0.1 的媒体库、主题、导航和布局复查，范围见 Web 记录 |
| iOS 的近期播放器扩展 | 共享 SwiftUI 生成路径与能力声明 | 本轮未做播放器设备交互验收 |
| Android / Linux / Windows 的近期布局扩展 | 仍由上表中的能力限制拒绝未实现声明 | 没有据 macOS / Web 的结果推定其他目标通过 |
| 动画 SVG | 六目标生成；示例 macOS、iOS 模拟器、Android、Web 构建；Windows / Linux 本轮未原生构建 | macOS / Web 实测 Logo 与交互；iOS / Android 本轮未运行设备交互，Windows / Linux 未验收 |

Firefox 与 Safari 手机布局尚未实测。Erika 当前播放器仍有模拟状态；播放器界面和图片过渡的验收不等于已接入完整媒体播放功能。

## 来源与维护

- 功能定义及拒绝规则：[bern-target-api](../../crates/bern-target-api/src/lib.rs)。
- [SwiftUI 清单](../../crates/bern-backend-swiftui/src/lib.rs)及 [iOS 描述](../../crates/bern-backend-swiftui/src/ios.rs)。
- [Web](../../crates/bern-backend-web/src/lib.rs)、[Compose](../../crates/bern-backend-compose/src/lib.rs)、[GTK](../../crates/bern-backend-gtk/src/lib.rs)、[WinUI 3](../../crates/bern-backend-winui3/src/lib.rs) 清单。
- 验收：[macOS](../Implementation/macOS-SwiftUI.md)、[iOS](../Implementation/iOS-SwiftUI.md)、[Android](../Implementation/Android-Compose.md)、[Web](../Implementation/Web.md)、[Linux](../Implementation/Linux-GTK.md)、[Windows](../Implementation/Windows-WinUI3.md)。

新增后端特性时同时更新此表和对应实现记录，并保留对尚未验收场景的说明。
