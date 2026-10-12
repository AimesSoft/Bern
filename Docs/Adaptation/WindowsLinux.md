# Windows 与 Linux 自适应依据及 Bern 适配设计

- 核查日期：2026-10-09（Asia/Shanghai）。
- 文档性质：官方资料核验与设计建议；不是已实现能力清单。
- Windows 范围：WinUI 3、Windows App SDK 与 Microsoft Windows 应用设计指南。
- Linux 范围：以 GTK 4／libadwaita 的 GNOME 路线为默认研究对象，同时对照 KDE HIG。
- 本文的“官方依据”概括已读取正文；“Bern 映射”是本项目提出的设计决策。
- API 名称用于界定适配职责，具体 Rust 封装、版本下限和生成代码仍须原型验证。

## 资料范围与读取记录

Microsoft 的窗口概览明确面向 WinUI 与 Windows App SDK；不能把同名 UWP `Windows.UI.WindowManagement.AppWindow` 直接当作 WinUI 3 API。本文窗口依据使用 `Microsoft.UI.Windowing`。[Windowing overview](https://learn.microsoft.com/en-us/windows/apps/develop/ui/windowing-overview)

GNOME HIG 明确以近期 GNOME 平台、尤其 GTK 4 与 libadwaita 为范围；KDE HIG 则围绕 KDE Plasma 的使用习惯，并考虑其他环境。由此可见，“Linux 原生”不能指向一套通用于所有桌面的 HIG。[GNOME HIG 平台定义](https://developer.gnome.org/hig/)、[KDE HIG 范围](https://develop.kde.org/hig/)

| 来源组 | 正文读取状态 | 版本与限制 |
| --- | --- | --- |
| Microsoft 布局、导航、输入、无障碍 | 已读取本文所引小节 | 动态更新的 Windows 应用文档，不据此推导 Bern 最低 OS 版本 |
| Windows 窗口 API | 已读取 Closing、Cancel 与窗口概览 | API 链接固定 `windows-app-sdk-1.8` 视图；不声称该版本为最新版本 |
| Windows 媒体与合成 | 已读取播放、控件绑定、资源归属与优化小节 | 仅确定适配方向，不证明全部编码、DRM 或 GPU 环境可用 |
| GNOME HIG | 已读取缩放、指针、键盘、窗口、无障碍正文 | HIG 未给本文涉及全部规则统一标注发行版本 |
| libadwaita Adaptive Layouts | 已读取断点、Clamp、Dialog、View Switcher 小节 | `/doc/1.8/` 页面显示 Library Version 1.8.8；不是 Bern 依赖承诺 |
| libadwaita Styles & Appearance | 已读取 `main` 的深浅色与高对比小节 | 页面显示 1.11.alpha；只作方向依据，使用前须核对项目锁定版本 |
| GTK／GDK API | 已读取缩放、关闭、最小化、视频正文 | 浮动 GTK 4 文档；仅 `get_scale` 的 4.12 起始版本已由正文核实 |
| KDE | 已读取 HIG 范围、布局与导航正文 | KDE 路线对照，不意味着 Bern 已有 Qt／Kirigami 后端 |

读取异常也保留记录：Microsoft Learn 部分页面顶部出现授权提示，但正文实际可读；本次没有登录或绕过权限。libadwaita `/doc/1.8/styles-and-appearance.html` 返回读取错误；另一次 `/doc/1.5/` 返回页面头显示 1.10.0，因此不按 URL 路径推定内容版本。主题引用最终采用明确标注开发版本的 `main`，不把该部分描述为 1.8 已验证 API。

## Windows：官方依据

### W1 窗口尺寸与逻辑单位

Microsoft 建议根据应用可用窗口空间设置断点，而不是按物理显示器尺寸划分。XAML 布局采用 effective pixels；缩放后的设备像素不应直接作为布局宽度。[Screen sizes and breakpoints](https://learn.microsoft.com/en-us/windows/apps/design/layout/screen-sizes-and-breakpoints-for-responsive-design)

官方通用页面的开头与表格对 640 边界措辞略有差别；本文采用 NavigationView 明确列出的默认行为：宽度不超过 640 时为最小导航，641–1007 为紧凑导航，1008 及以上展开。它们是控件默认策略，阈值可修改。[NavigationView：Adaptive behavior](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/navigationview#adaptive-behavior)

### W2 导航不是自动业务路由

NavigationView 支持顶部和侧边导航，默认可在最小、紧凑、展开侧栏之间适应宽度；控件本身不维护应用的前进、返回业务流程。`ItemInvoked` 表示调用，`SelectionChanged` 也可能来自程序写回；两者不能无区别地触发同一导航动作。[NavigationView：Display modes 与 Navigation](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/navigationview)

### W3 输入与命中区域

WinUI 控件具有键盘行为基础；官方要求可预测的 Tab 顺序、合理初始焦点及分组内方向键导航。静态标签不应为了可读而全部变成 Tab 停靠点。[Keyboard interactions](https://learn.microsoft.com/en-us/windows/apps/develop/input/keyboard-interactions)

触屏目标建议通常约为 7.5mm 见方，文档举例为指定密度与缩放下的 40×40 像素；大小还需考虑误触后果和使用频率。这不是要求所有可见图标绘制成固定 40px 方块。[Guidelines for touch targets](https://learn.microsoft.com/en-us/windows/apps/develop/input/guidelines-for-targeting)

### W4 文本缩放、主题与无障碍

Windows 文本大小与整体显示缩放是不同设置。WinUI 文本控件通常支持系统文本缩放，官方不建议大范围关闭 `IsTextScaleFactorEnabled`；静态文本与可编辑文本也应保持正确的 UI Automation 角色。[Accessible text requirements](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessible-text-requirements)

对比主题与深浅色不是同一个维度。主题资源应在运行时跟随系统，HighContrast 资源使用系统语义色及正确前景背景配对，不能用品牌色硬覆盖用户自定义对比主题。[Contrast themes](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/high-contrast-themes)

### W5 窗口、关闭与系统事实

WinUI 的 XAML Window 承载内容与生命周期，AppWindow 对顶层 HWND 提供窗口管理抽象；系统窗口与 Bern 的逻辑窗口身份不是同一对象。[Windowing overview](https://learn.microsoft.com/en-us/windows/apps/develop/ui/windowing-overview)

`AppWindow.Closing` 针对系统提供的关闭入口，直接调用 `Destroy` 不触发该事件。`Cancel=true` 会在 Closing 处理器退出时取消本次系统关闭；不能只依赖该事件统一拦住所有销毁。[Closing](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.appwindow.closing?view=windows-app-sdk-1.8)、[ClosingEventArgs.Cancel](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.appwindowclosingeventargs.cancel?view=windows-app-sdk-1.8)

### W6 视频呈现与合成

MediaPlayerElement 可以绑定已有 MediaPlayer，也可自行创建播放器；两种方式的释放责任不同。官方支持关闭内置控制条，并提醒不要交替写入控件和底层播放器的等价属性，否则状态可能不一致。[Play audio and video with MediaPlayer](https://learn.microsoft.com/en-us/windows/apps/develop/media-playback/play-audio-and-video-with-mediaplayer)

WinUI 对全窗口视频提供 `IsFullWindow` 优化路径；叠加 XAML 内容会影响媒体合成效率。字幕和临时控制仍有合理用途，官方建议不需要时将其折叠；这不是禁止叠加控件的功能限制。[Optimize animations, media, and images](https://learn.microsoft.com/en-us/windows/apps/develop/performance/optimize-animations-and-media#optimize-media-resources)

## Windows：Bern 映射

以下为待实施设计，不是 Microsoft 强制规定。

- **尺寸**：目标适配器上报窗口内容区逻辑尺寸、显示缩放、文本缩放；公共 IR 不接收 HWND 外框像素作为页面可用宽度。
- **断点**：Windows 导航预设可参考 640／1008；公共 Adaptive 允许按内容重定义。实际比较规则须覆盖分数宽度且没有未归类区间。
- **状态保持**：紧凑侧栏与展开侧栏引用同一路由、命令和 Feature；切换宽度保留当前页面、选择、草稿及滚动身份。
- **导航**：原生调用事件转换为 `ui.navigate`／`ui.back` 会话命令；程序写回的选择通知只同步状态，不重复执行命令。
- **输入**：鼠标、触屏、键盘、UI Automation 的控件激活最终引用同一动作；文字输入继续采用原生输入法编辑会话。
- **命中区域**：透明热区可以大于图标；IconButton 默认使用 WinUI 标准的图标按钮样式，可访问名称、焦点指示和足够命中空间不得删除。
- **主题**：按“原生优先”使用 WinUI 主题资源与平台外观，品牌色映射为强调色资源；高对比资源映射到系统色，主题变化不重建业务引擎或窗口身份。
- **文字**：文本缩放影响测量和重排；不能只缩放 glyph 后仍使用原来的固定高度。图标资源与视频缓冲区另按渲染尺寸处理。
- **关闭**：可取消关闭入口先取消原生默认动作，再执行统一关闭流程；确认完成后由受控路径销毁，防止重复确认或重复释放。
- **系统事实**：已经发生的移动、尺寸、激活或销毁通过类型化观察事件归并；不事后补造命令声称事前控制了 OS。
- **视频**：标准 Video 的控制绑定经过 Bern 命令；播放器归属与呈现控件归属分开，切换布局不应意外 Dispose 引擎级播放器。
- **合成**：字幕、弹幕、菜单与虚拟窗口叠加仍是允许能力；目标须单独测量合成成本，不能承诺所有组合都获得最低功耗路径。

## GNOME／GTK：官方依据

### G1 按空间与内容自适应

GNOME 建议从最受限的空间及输入条件出发，使用断点重组 UI；大窗口需要内容最大宽度，避免文本过长或相关控件距离过远。设备外形不能可靠代表是否有键盘或触摸能力。[Scaling & Adaptiveness](https://developer.gnome.org/hig/guidelines/adaptive.html)

该 HIG 当前推荐支持的桌面显示尺寸下限为 1024×600；适合手机形态的应用应能缩至 360×294。前者是显示环境建议，不能解释为每个应用都必须强制最小窗口 1024×600。[Scaling & Adaptiveness：Small Size Handling](https://developer.gnome.org/hig/guidelines/adaptive.html#small-size-handling)

### G2 libadwaita 的可用结构

已读取的 libadwaita 1.8 文档提供 Clamp 限制内容最大宽度、Breakpoint 改变布局、顶部／底部 View Switcher 切换，以及随父窗口尺寸改变呈现方式的 Dialog。使用断点的宿主需要声明所支持的最小尺寸，不能期待内容自然给出全部约束。[Adaptive Layouts](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1.8/adaptive-layouts.html)

### G3 指针、触屏与键盘

GNOME 不允许关键操作只在 hover 时出现；指针可完成的操作也应能通过键盘完成。三指、四指及上下屏幕边缘手势保留给系统，应用不应抢占。[Pointer & Touch](https://developer.gnome.org/hig/guidelines/pointer-touch.html)

键盘指南要求整个界面可键盘导航，沿用标准快捷键；Super 保留给系统，Alt 访问键需与本地化文案一起处理冲突。[Keyboard](https://developer.gnome.org/hig/guidelines/keyboard.html)

### G4 缩放与资源像素

GTK 的 `Widget.get_scale_factor()` 返回整数；面对分数缩放，文档建议在需要实际比例时使用 `Gdk.Surface.get_scale()`。后者从 4.12 起提供，可在 Surface 生命周期内变化。[Gtk.Widget.get_scale_factor](https://docs.gtk.org/gtk4/method.Widget.get_scale_factor.html)、[Gdk.Surface.get_scale](https://docs.gtk.org/gdk4/method.Surface.get_scale.html)

### G5 深浅色、高对比与辅助技术

libadwaita 的外观文档将系统偏好、强制浅色和强制深色分别建模；高对比独立于深浅色，标准控件已有支持，自定义绘制仍需处理。该引用读取的是开发分支，不据此要求 Bern 立即采用新 CSS 功能。[Styles & Appearance](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/styles-and-appearance.html)

GNOME HIG 要求描述性可访问名称，并实际测试高对比、大文本、纯键盘、屏幕阅读器及屏幕键盘；采用 GTK 控件不替代应用层验收。[Accessibility](https://developer.gnome.org/hig/guidelines/accessibility.html)

### G6 窗口关系与关闭

主窗口应可调整大小；多个主窗口相互独立。辅助窗口从属于主窗口，主窗口关闭时一并关闭；标准关闭快捷键为 Ctrl+W，模态窗口还应支持 Esc。[Windows](https://developer.gnome.org/hig/patterns/containers/windows.html)

这里 GNOME 的“主窗口”指独立顶层窗口，可以有多个；它不等同于 Bern WindowSet 中唯一的 primary 角色。Bern 默认会话所有权与 GNOME 辅助窗口约定有差异：应用若需要随父窗口关闭的辅助窗，应显式声明所有者与关闭关系，后端不偷偷改变公共生命周期。

GTK `close-request` 在用户点击关闭按钮时发出，处理器可返回 true 阻止继续传递。另一方面，GDK 明确指出窗口系统可能忽略最小化请求；发送请求并不保证实际窗口状态已经改变。[Gtk.Window::close-request](https://docs.gtk.org/gtk4/signal.Window.close-request.html)、[Gdk.Toplevel.minimize](https://docs.gtk.org/gdk4/method.Toplevel.minimize.html)

### G7 视频不是仅映射 GtkVideo

GtkVideo 面向预览、嵌入动画和简单媒体控制，官方明确列出其缺少视频 overlays、多声道音频及设备选择等功能；完整播放器可使用 GdkPaintable 与 GStreamer 等媒体框架。无控制呈现可走 Paintable／GtkPicture。[Gtk.Video](https://docs.gtk.org/gtk4/class.Video.html)

## GNOME／GTK：Bern 映射

以下为待实施设计，不把 GNOME 约定推广为所有 Linux 桌面的强制规范。

- **布局**：GTK 目标可映射到 libadwaita 的已支持容器；按目标锁定版本声明能力，公共 IR 不暴露 Adw 对象。
- **阈值**：根据导航项、翻译后标签与内容最小宽度选择断点，不复制 Windows 的 640／1008 常量。
- **大窗口**：表单和阅读内容使用最大宽度约束；视频、图表等可扩张区域按自身内容规则分配空间。
- **窄窗口**：导航可以移到页栈、底部切换器或按需面板，Feature 和命令身份保持一致；不允许把装不下的功能直接删除。
- **输入**：触屏不依赖 hover 或右键；快捷键、菜单与可访问动作绑定同一命令，不占用 GNOME 保留手势。
- **缩放**：布局、窗口尺寸与图像／视频缓冲像素分开；跨显示器移动后重新选择资源分辨率，不能把整数 scale 当作真实分数比例。
- **主题**：按“原生优先”使用 GTK 与已支持桌面主题的深浅色样式；所选环境无法提供完整深浅色时，按能力描述改用 Bern 语义角色映射；高对比优先。暂不把开发分支 CSS media query 列为最低可用实现。
- **无障碍**：保留 GTK 的可访问树、编辑角色和焦点行为；生成的自定义控件必须补齐同样的协议。
- **窗口**：专门窗口声明显式区分主窗口、辅助窗口、父级与模态作用域；多个主窗口关闭互不连带销毁。
- **关闭确认**：关闭按钮、Ctrl+W、CLI 和 MCP 使用同一 `window.request_close`；未保存检查可返回待确认结果，不能只弹窗然后让外部调用永久等待。
- **窗口系统**：请求和观察结果分别记录；最小化、激活、尺寸和位置能力按 compositor／后端验证，不能把 Linux 当作统一窗口管理器。
- **视频**：简单预览和完整播放器是不同能力档；后者单列媒体内核、Paintable／纹理路径、字幕叠加、色彩及硬件解码验收。

## KDE 对照：Linux 没有统一 HIG

**官方依据。** KDE 要求窗口在平铺、最大化和窄窗口状态下平滑适应；其布局表建议窄形态工具栏／导航栏向底部调整，侧栏按需显示或进入页栈。Kirigami 的布局单位和 GlobalDrawer／PageStack 是其具体实现体系。[KDE Layout and navigation](https://develop.kde.org/hig/layout_and_nav/)

**Bern 映射。** 首个 Linux 默认目标仍可选择 GTK，但目标描述应明确“GNOME／GTK 约定”；在 Plasma 下运行 GTK 应用不等于已经采用 KDE HIG。未来 Qt／Kirigami 目标通过独立生成器接入，复用窗口、命令、状态和主题语义；它不需要复制 GTK 的控件外形，也不能绕过 Bern 的完整功能要求。

**能力边界。** 分别报告桌面环境、窗口系统、GTK／libadwaita 版本与已验证组合。KDE 资料作为对照，不构成本轮 Qt 实现、发布兼容性或全 Linux 发行版认证。

## 跨平台窗口与命令边界

以下是 Bern 协议要求，不能从单个原生 API 的存在推导为已实现。

| 场景 | Bern 需要的规则 |
| --- | --- |
| 有原生多窗口能力 | 窗口声明映射为原生窗口；每个实例有独立逻辑身份及生命周期 |
| 只有单宿主表面 | 自动使用应用内虚拟窗口；语义命令不改变，实际呈现模式可查询 |
| 虚拟窗口限制 | 不宣称拥有系统任务栏入口、跨显示器独立窗口或系统全局置顶 |
| 外部关闭请求 | UI、CLI、JSON、MCP 共用关闭检查与确认协议；确认关联实例及状态版本 |
| OS 已发生的事实 | 受信任适配器上报观察事件；不能伪造先执行命令，也不能允许外部注入状态 |
| 强制结束宿主 | 子窗口随宿主消失；依靠恢复策略，不能保证每次都有机会展示关闭确认 |
| 引擎共享 | 多窗口绑定同一 engine_id；窗口关闭只释放所拥有资源，是否停止播放由所有权决定 |
| 陈旧目标 | window_id 结合实例代次校验；销毁后旧命令不能命中新建的同名窗口 |

自动回退解决呈现能力差异，不改变目标的功能契约；需要真实系统能力的功能应报告不支持或使用已声明替代方案。

## 可操作验收清单

以下场景须在真实目标上执行。GNOME／GTK 部分的首轮执行情况见 [Linux GTK 实现记录](../Implementation/Linux-GTK.md)，Windows／WinUI 3 部分见 [Windows WinUI 3 实现记录](../Implementation/Windows-WinUI3.md)。

### Windows 验收

- [ ] 在 640 与 1008 附近连续调整内容区宽度，包括边界值；导航保持同一路由且没有功能缺失。
- [ ] 以目标支持的 100%、125%、150%、200% 显示缩放重复测试；移动到不同缩放显示器后命中区域仍对应画面。
- [ ] 单独增大系统文本大小；按钮、菜单、标题及表单可完整读取，不用裁剪代替重排。
- [ ] 鼠标、触屏、键盘分别完成打开媒体、搜索、选择、播放、返回；每次动作只产生一次命令。
- [ ] 仅用键盘遍历页面，初始焦点与模态关闭后的恢复合理；静态标签没有多余 Tab 停靠点。
- [ ] 运行时切换深浅色和系统对比主题，并测试用户修改后的对比颜色；无硬编码失读状态。
- [ ] 使用 Narrator 检查名称、角色、值、禁用态及导航顺序，原生写回不重复派发命令。
- [ ] 分别测试标题栏关闭、程序关闭、外部 CLI 关闭及销毁后迟到消息；关闭确认与资源释放各执行一次。
- [ ] 视频切换嵌入／全窗口、显示／隐藏字幕及控制条，核对实际播放状态、合成性能和释放情况。

### GNOME／GTK 验收

- [ ] 在 1024×600 桌面环境及窄窗口验证所有必要功能；仅当应用声明手机支持时加入 360×294 场景。
- [ ] 长翻译、大文本、侧栏折叠和窗口展开组合下，内容不越界；导航切换不丢失草稿与选择。
- [ ] 鼠标、触摸、触控板与纯键盘分别操作；重要功能不依赖 hover、右键或保留手势。
- [ ] Ctrl+W、Esc、标准快捷键与本地化访问键无冲突；模态作用域及焦点恢复符合声明。
- [ ] 在所声明的整数／分数缩放环境及跨显示器移动后检查文字、图标、视频和指针命中一致性。
- [ ] 深浅色、高对比、大文本、屏幕阅读器及屏幕键盘逐项验证，记录可访问树结果。
- [ ] 两个独立顶层窗口共享引擎，关闭一个保留另一个；显式声明从属关系的辅助窗口按所有者关闭协议处理，不误销毁应用级播放器。
- [ ] 窗口系统拒绝请求、外部关闭、宿主失联时返回实际状态；不得把请求发出记录为已经完成。
- [ ] 完整播放器分别验证字幕／控制叠加、媒体后端失败、换源、释放及实际支持的编码组合。

### 交叉能力验收

- [ ] 同一 fixture 分别用 UI、CLI、JSON、MCP 驱动同一引擎实例，业务状态与版本连续一致。
- [ ] 原生／虚拟窗口运行同一声明；窗口身份、关闭确认、导航与功能覆盖结论一致，系统能力差异明确可查。
- [ ] GNOME 与 Plasma 环境分别记录结果；一个环境通过不能替另一个环境签署支持。
- [ ] 锁定工具链后记录 OS、桌面环境、窗口系统、SDK／GTK／libadwaita 与媒体后端版本，再确定发布支持范围。

验收报告应区分“声明编译通过”“真实输入可操作”“原生 API 请求成功”“实际状态已确认”和“性能测量通过”，不能用其中一项替代全部结论。
