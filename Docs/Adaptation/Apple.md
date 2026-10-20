# Apple 平台自适应适配

- 核查日期：2026-10-09（Asia/Shanghai）；第 9 节的来源于 2026-10-10 核查。
- Bern 接口说明更新：2026-10-17（容器、分组与系统版本回退）。
- 范围：macOS、iOS / iPhone、iPadOS / iPad、tvOS / Apple TV，以及已公布的 iPhone Duo。
- 关联：[Bern 框架设计](../Design.md)。本文是后端设计与验收依据，不表示相关后端已经实现。
- **官方依据**说明 Apple 已发表的规则或能力；**Bern 决策**说明本项目如何映射，二者不能互换。
- 来源状态“正文”表示已读取官方正文或视频文字稿；部分 HIG 正文通过官方页面数据取得。“仅搜索可见”不得单独支撑硬性要求。
- 文中系统版本限定具体能力，不构成 Bern 最低系统版本承诺；最低版本、SDK 与可用性分支需由目标模块另行锁定。

## 1. 以当前场景空间和能力适配

**官方依据。** Apple 要求布局响应实际空间、尺寸类别、安全区域、文字大小与本地化变化；横竖屏或设备名称不能代替这些输入。布局调整应保持功能可达。[Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout)

**Bern 决策。** Apple 适配器至少提供以下相互独立的环境信息；字段名是设计概念，尚不是已实现 API。

| 输入 | 用途 | 禁止的简化 |
| --- | --- | --- |
| 当前 scene / window 的逻辑边界 | 选择列数、阅读宽度、主辅关系 | 使用整块显示器尺寸布局 |
| 水平、垂直尺寸类别 | 接入原生容器的布局语义 | 用一个 `isTablet` 代替 |
| 系统安全区域 | 保护正文、交互与字幕 | 所有设备固定顶部和底部留白 |
| 预留、分隔与遮挡区域 | 表达折叠及不规则可用空间 | 只记录四边 inset |
| 文字缩放与辅助功能偏好 | 重排、对比度与交互反馈 | 只等比放大整个画布 |
| 当前输入能力 | 触摸、指针、键盘、遥控器 | 按产品名称固定一种输入 |
| 外观与系统材料上下文 | 生成主题并处理原生区域 | 强制覆盖所有系统颜色 |
| 场景创建及媒体能力 | 选择原生 / 虚拟呈现 | 把能力写成永久平台常量 |

尺寸变化是受信任的环境事实，不是业务命令。重排不能重新搜索、重置草稿或创建第二个播放器。

布局可改变导航的表现，但必须保留选中实体、路由、滚动锚点、编辑草稿、逻辑窗口 ID 与 `player_id`。

## 2. macOS：桌面窗口与键盘工作流

**官方依据。** SwiftUI 的窗口场景支持按用途设置尺寸、位置及恢复行为；窗口标题即使视觉隐藏，仍影响辅助功能和窗口菜单。视频窗口的放置还应考虑屏幕可见区域。[Tailor macOS windows with SwiftUI，WWDC24](https://developer.apple.com/videos/play/wwdc2024/10148/)

该演讲涉及 macOS Sequoia 时期的新 API。Bern 若支持更早版本，应采用相应替代实现并单独验证，不能直接视为全版本能力。

**Bern 决策。** macOS 默认 SwiftUI，必要的菜单、窗口和宿主集成由 AppKit 补充。

| 维度 | Bern 适配决策 |
| --- | --- |
| 窗口 | `Window(presentation: auto)` 优先原生 scene；按逻辑实例绑定系统窗口，不用标题充当 ID |
| 导航 | 宽窗口可并置侧栏与详情；窄窗口保留当前详情和可用的返回入口 |
| 菜单 | 应用菜单、窗口菜单和快捷键引用同一命令目录；启用状态来自当前会话 |
| 尺寸 | 提供合理默认值和最小可用布局；系统最终位置、尺寸及屏幕变化通过事件同步 |
| 输入 | 支持键盘遍历、快捷键、指针与上下文操作；拖动不能成为唯一完成方式 |
| 主题 | 使用系统语义颜色、材质与标准控件样式，品牌色映射为强调色；Bern 只为自绘部分使用共享主题算法 |
| 视频 | 播放器与窗口独立；关闭呈现不应错误销毁应用级播放器 |

**官方依据。** macOS 与 iPad 的键盘导航受系统设置影响；焦点语义随输入类型和场景变化。SwiftUI 焦点 API 也有系统版本限制。[The SwiftUI cookbook for focus，WWDC23](https://developer.apple.com/videos/play/wwdc2023/10162/)

**Bern 验收。**

- 连续缩小、放大窗口后，内容和操作仍可达；没有依赖屏幕底边的关键固定操作。
- 主窗口与设置窗口同时存在时，快捷键与菜单作用于正确会话。
- 窗口恢复、重新打开和重复打开均保持实例策略，且不误恢复临时说明窗口。
- 通过系统关闭入口请求关闭时，验证脏草稿、取消与异步确认失效处理。
- 开启和关闭 Keyboard Navigation 后分别验证焦点、可访问名称与激活。
- 更换显示器、菜单栏 / Dock 占用改变时，窗口仍能回到可见范围。

## 3. iOS / iPhone：触摸与导航连续性

**官方依据。** 顶层目的地、层级导航和模态任务承担不同信息组织职责。[Explore navigation design for iOS，WWDC22](https://developer.apple.com/videos/play/wwdc2022/10001/)

该来源可支持导航语义，但其 2022 年视觉示例不能规定所有未来 iPhone 的导航固定在底部；iPhone Duo 的侧向控件适配见第 6 节。

**官方依据。** Dynamic Type 使用可缩放文字样式；固定高度容易造成截断，应允许换行、增长及重排。[Get started with Dynamic Type，WWDC24](https://developer.apple.com/videos/play/wwdc2024/10074/)

**Bern 决策。** iOS 默认 SwiftUI；手机布局首先保证当前任务可操作，而不缩小桌面界面。

| 维度 | Bern 适配决策 |
| --- | --- |
| 窗口 | 查询当前 scene 创建能力；不可用时采用全区域活动虚拟窗口加切换器 |
| 导航 | 以目的地、层级和任务角色声明；原生目标决定当前环境的导航位置 |
| 返回 | 返回手势、可见返回入口和外部入口使用同一会话导航语义 |
| 尺寸 | 读取当前 scene、键盘及安全区域；横屏低高度不能只按宽度选择多栏 |
| 输入 | 触摸热区与视觉图标尺寸分离；允许外接键盘和指针带来的额外输入 |
| 主题 | 系统外观、对比度和材料变化能更新 Bern 控件配方 |
| 视频 | 全屏、画中画与前后台播放分别协商能力，不由 `Video` 的存在推断支持 |

**Bern 验收。**

- 最小支持窗口与横屏下，表单、字幕、错误信息和主要操作不被裁掉。
- 键盘出现、消失或切换输入法时，当前编辑项可见，草稿不丢失。
- 最大支持文字级别下检查换行、滚动、标签关联与操作顺序。
- 返回交互被取消后，路由、焦点与命令结果一致，没有执行两次导航。
- 窗口切换后继续同一详情和播放会话；后台限制明确反映到状态。
- VoiceOver 可辨认所有仅图标操作，朗读顺序与当前布局一致。

## 4. iPadOS / iPad：可调整窗口与混合输入

**官方依据。** iPadOS 26 的设计包含可调整窗口、窗口控件、菜单栏及指针交互；侧栏与标签栏需随空间调整，窗口变化应保持用户当前上下文。适用文档工作流可为新文档打开独立窗口并提供明确标题。[Elevate the design of your iPad app，WWDC25](https://developer.apple.com/videos/play/wwdc2025/208/)

这些具体窗口与外观能力有 iPadOS 26 版本边界，不应投射到所有历史版本。

**Bern 决策。** iPad 复用业务协议与声明，不复用“放大的手机固定布局”。

| 维度 | Bern 适配决策 |
| --- | --- |
| 窗口 | 原生能力可用时映射多个 scene；每个窗口独立导航、焦点和草稿 |
| 导航 | 主辅栏可并置、折叠及恢复；被折叠区域的功能仍通过入口可达 |
| 菜单 | 菜单栏、快捷键、上下文入口绑定相同命令，不额外维护业务执行路径 |
| 尺寸 | 以实际窗口边界重排；预留系统窗口控件，避免自绘工具栏覆盖它们 |
| 输入 | 触摸操作完整，同时支持指针悬停和键盘；悬停不能暴露唯一入口 |
| 主题 | 原生材料可按边界保留 |
| 视频 | 宽空间展示视频与辅助内容；缩小时保留播放器身份和当前控制焦点 |

同一 iPad 上小窗口、全屏和不同外接显示场景可能需要不同布局；“iPad”不是某个固定断点。

系统允许多窗口不等于 Bern 能强制指定任意桌面式位置。原生创建请求可失败，失败不能留下无系统句柄的“已打开”幽灵实例。

**Bern 验收。**

- 从宽窗口连续缩到窄窗口再恢复，选中内容、返回关系与草稿保持。
- 工具栏避开窗口控件；弹出菜单与辅助窗口在缩小后仍可关闭。
- 触摸、键盘和指针分别完成同一主要 Feature，不依赖悬停提示。
- 两个文档窗口具有可区分标题，命令参数包含正确窗口 / 会话目标。
- 系统切换场景、关闭窗口或恢复窗口后，订阅和资源所有权正确。
- iPadOS 26 与最低支持版本分别验收；仅存在于新版本的行为有可见替代路径。

## 5. tvOS / Apple TV：遥控焦点优先

**官方依据。** tvOS 通过焦点支持遥控方向导航；焦点移动和键盘 Tab 遍历并不是同一机制。[The SwiftUI cookbook for focus](https://developer.apple.com/videos/play/wwdc2023/10162/)

**官方依据。** Layout HIG 给出电视主要内容距顶部 / 底部 60 pt、左右 80 pt 的安全区域指导，并要求考虑焦点放大所需空间。[Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout)

**Bern 决策。** 上述数值是电视内容设计基线，需结合系统容器提供的实际安全区域核对；不能在已应用安全区域的容器外重复叠加相同留白。

| 维度 | Bern 适配决策 |
| --- | --- |
| 窗口 | 默认以单一活动内容与可导航切换器呈现逻辑窗口；不模拟多个桌面小浮窗 |
| 导航 | 方向焦点图、返回层级与初始焦点明确；从弹层返回恢复到有效控件 |
| 尺寸 | 为远距离观看重做文字层级、内容密度及间距，不把手机界面整体放大 |
| 输入 | 遥控器完成播放、定位、选择和返回；可选输入设备使用同一语义动作 |
| 主题 | 同时检查深浅外观、视频背景和焦点状态；焦点不能仅靠细微色差 |
| 辅助功能 | 暴露名称、角色、状态及动作；隐去的控制条不得留下可操作幽灵节点 |
| 视频 | 优先研究 AVKit 系统体验，随后验证 Bern 的命令拦截要求 |

**官方依据。** Apple 明确建议 tvOS 使用系统媒体播放器，以适配遥控器等输入并保持熟悉的播放体验。[Create a great video playback experience，WWDC22](https://developer.apple.com/videos/play/wwdc2022/10147/)

**Bern 决策。** 官方推荐 AVKit 不等于它已满足“所有 Bern UI 操作在副作用发生前派发命令”。必须原型验证其动作拦截点；事后观察 `rate` 或播放状态不能充当已执行命令的证据。

若系统播放器某些自带控件不能满足契约，后端需明确呈现边界及可用方案。不能同时宣称“完全原生控制”与“全部可预先拦截”，却没有逐项验证。

**Bern 验收。**

- 仅使用遥控器，从启动到主要 Feature 完成全程不需要指针或触屏。
- 每个焦点状态清晰；容器边缘的缩放或动画不裁切当前焦点内容。
- 长列表、空状态、加载失败、弹层和关闭后的焦点都有确定落点。
- 控制条消失后仍能恢复；返回不会同时关闭弹层并退出详情。
- 对播放、暂停、定位、速度和字幕分别核查命令执行顺序与能力错误。
- 实际电视输出上检查可读性和安全区域，不只看桌面模拟器截图。

**Bern 实现（2026-10-20）。** `swiftui + tvos` 目标已实现，细节与验证记录见 [tvOS 实现记录](../Implementation/tvOS-SwiftUI.md)。按上表各维度：

| 维度 | 当前实现 | 验证 |
| --- | --- | --- |
| 窗口 | 一个铺满屏幕的场景；主窗口铺满，其他窗口（含设置）以全屏页打开，返回键关闭 | 模拟器实测设置页 |
| 导航 | 系统焦点引擎；标签页的每页是一个焦点区域，`default_focus` 给出首选焦点；`Shortcut(key: "escape")` 是返回键，没有时返回交给系统 | 焦点移动与起始焦点在模拟器实测；返回键未验证 |
| 尺寸 | `Layout(size: tv)` 与 `window.tv` 让应用给电视单独的布局；按设计尺寸画的场景用 `ScaleBox` 放大到屏幕，场景里的控件按设计尺寸画 | 一个实际应用的电视布局在模拟器实测 |
| 输入 | `Shortcut` 与 `shortcut` 对应返回、播放/暂停与触控板方向；滑块是按方向步进的可聚焦轨道 | 方向在模拟器实测；返回、播放/暂停与真实遥控器未验证 |
| 主题 | 系统控件的焦点效果（抬起、变白、阴影）；缩放场景里的胶囊与圆形按钮焦点下白底黑字 | 深色外观在模拟器实测；浅色外观未实看 |
| 辅助功能 | 系统控件的名称与角色；焦点滑块是可调节元素；`Pinch`、`Swipe` 只保留辅助功能调节 | 未用旁白实测 |
| 视频 | 与 iOS 相同：AVFoundation 执行端、`AVPlayerLayer` 呈现，控制层由 Bern 的命令驱动，动作先经命令再改变播放；未采用 AVKit 的系统播放器，它的拦截点仍待原型 | 模拟器里播放、快退快进实测；HDR 未验证 |

验收清单中“实际电视输出上检查可读性和安全区域”尚未进行，目前只有模拟器截图。

## 6. iPhone Duo：已公布设备的动态适配

### 官方状态与版本边界

截至核查日期，iPhone Duo **已由 Apple 正式公布，尚未上市**。官方产品页列出 2026-10-16 开始预订、2026-10-23 发售；不能在此日期将其写成传闻，也不能称为已经完成实机验证。[产品页](https://www.apple.com/iphone-duo/)、[官方发布稿](https://www.apple.com/newsroom/2026/09/apple-unveils-iphone-duo/)

官方准备指南要求使用 iOS 27.1 SDK / Xcode 27.1 获得完整边缘布局体验，并介绍 Device Hub 的开合、旋转和折叠测试；兼容运行不等于已经充分适配。[Prepare your app for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111461/)

### 尺寸、折叠与导航

**官方依据。** 外屏和内屏的基本尺寸类别不同，布局应读取实际 size class 与 scene geometry。功能及状态在显示屏、姿态之间连续；展开后的 iPhone 应用不能直接等同于 iPad 应用。[Designing for iPhone Duo HIG](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo)、[Prepare your app](https://developer.apple.com/videos/play/tech-talks/111461/)

系统导航与工具栏可移动到侧边，Split View 中还涉及外侧放置；固定画中画及部分折叠会改变剩余内容空间。[Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/)

折叠适配应识别预留区域，使用 arrangement / region 相关布局机制避让折痕及相机区域，而不是猜测物理角度与像素分界。[Strike a pose with adaptive layouts on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111463/)

**Bern 决策。**

- 公共布局表达“主内容与辅助内容的关系”，由 Apple 目标映射其区域机制。
- 安全区域和内部分隔分别建模；不能仅在窗口四周加 padding。
- 不把底部标签栏写入公共导航语义；提供侧边呈现所需的可访问顺序。
- 不根据内屏尺寸创建新应用会话；播放器、导航与编辑状态继续使用原身份。
- 姿态信息缺失时使用实际可用空间适配，避免凭设备型号制造假的折痕区域。
- 不预设屏幕尺寸、折叠角度阈值或相机遮挡坐标；这些不是 Bern 的跨平台常量。

### 多场景与多显示边界

**官方依据。** iPhone Duo 支持同一应用的多个 UI 场景，但新场景只能在内屏创建，外屏不可；可用性会变化，必须处理请求失败。铰链数据适合交互和特效，布局使用区域机制。Scene accessories 也受动态可用性限制，并非任意双屏窗口权限。[Leverage multiple displays and scenes on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111464/)

**Bern 决策。** `native_window_creation` 是当前环境能力，不能按 `platform == iOS` 永久置为 false，也不能因 Duo 内屏支持就对外屏永久置为 true。

`Window(presentation: auto)` 发起创建前读取当前能力，并处理检查后能力变化的竞争条件。已接受的创建请求失败时，仅在能力不足且已清理部分创建的原生对象后，按声明契约执行虚拟回退；其他失败保留实际错误类别，不能重复创建逻辑实例。

`native_required` 在能力不足时返回结构化能力错误，资源、配置等其他创建失败保留实际错误类别。切换到外屏不等于用户请求关闭已有逻辑窗口，场景销毁与内容保留策略必须单独处理。

Scene accessory 属于附属呈现能力；不能自动映射成所有 `Window` 都可在外屏独立运行。相机相关 accessory 的特定条件也不能泛化为普通视频多显示能力。

**Bern 验收。**

- 外屏到内屏再回外屏，当前详情、草稿、焦点恢复位置及 `player_id` 保持。
- 内屏 Split View 两侧分别测试，导航和工具栏不落在不可操作的内部边缘。
- 固定画中画出现 / 消失后，低高度布局仍可完成当前任务。
- 部分折叠与旋转组合下，主要控件、正文和字幕避开有效预留区域。
- 外屏请求新原生场景得到明确能力结果；内屏创建失败也不会丢失实例状态。
- 在创建请求期间切换内外屏，验证竞态、回退和重复事件处理。
- accessory 可用性改变时，入口状态和呈现同步；没有无内容的可激活操作。
- SDK / 模拟环境检查与最终实机验收分开记录，上市前不填“实机通过”。

## 7. 主题、无障碍与图标按钮

**官方依据。** 系统语义颜色与外观、对比度和材料有关；品牌色也会影响用户对可交互性的判断。[Color HIG](https://developer.apple.com/design/human-interface-guidelines/color)

**官方依据。** 可访问界面需支持较大文字、足够对比、非纯颜色提示及 VoiceOver；自绘或重构控件需要保留标签、角色和动作语义。[Accessibility HIG](https://developer.apple.com/design/human-interface-guidelines/accessibility)、[Catch up on accessibility in SwiftUI，WWDC24](https://developer.apple.com/videos/play/wwdc2024/10073/)

**Bern 决策。** Apple 平台具备完整的深浅色与语义颜色体系，SwiftUI 目标使用系统语义颜色、材料与标准控件样式，默认不在窗口、侧边栏、表单等系统表面叠加自定义背景色，应用声明的样式覆盖除外；应用声明的品牌色映射为 tint 或 AccentColor，未声明时使用用户选择的系统强调色。共享 Rust 主题算法只用于 Bern 自行绘制的控件与表面。

IconButton 默认使用平台标准的图标按钮样式，系统托管的呈现位置遵循平台规则，例如工具栏为工具栏项统一提供的背景。`Button`、`Menu`、`Picker`、`Switch`、`SplitButton` 的 `shape: capsule` / `height` 及 `ControlGroup` 表达容器、尺寸和功能分组，macOS / iOS 26 起可使用系统 `glassEffect`，旧系统回退到系统材质胶囊。不得为了保持默认样式而移除应用明确声明的容器；普通图标操作可显式使用 plain。当前接口见[组件用法](../Reference/UI.md)。

使用标准组件不等于辅助功能自动合规，仍要单独验证 macOS 键盘焦点和 tvOS 遥控焦点的清晰度。

**Bern 共同验收。** 用实际可访问树与操作检查名称、角色、状态和动作；同时覆盖较大文字、深浅色、增强对比及减少动态效果。仅做截图比色不足以证明可用。

## 8. 命令、系统事实与媒体边界

**Bern 决策。** Bern 自己提供的导航、菜单、图标、焦点移动和播放控件必须绑定已登记命令；UI、CLI、JSON 与 MCP 驱动相同引擎及明确目标会话。

系统已发生的窗口尺寸、系统焦点转移、媒体解码及生命周期变化作为受信任事件接收。不得事后补造“用户命令”，也不得响应事件再次执行相同操作形成循环。

原生交互是否可在副作用前拦截，需要按控件核查。对手势导航、系统窗口关闭以及 AVKit 控件，测试应区分请求、允许 / 拒绝、系统完成和事件回报。

媒体后端独立于 SwiftUI 目标。`Video(source: …)` 与 `Video(player: …)` 都使用同一媒体协议；布局变化仅重新附着呈现，不暗中改变资源所有权。

系统外部媒体入口、应用自有控制条和系统自带播放器控件分别记录来源与支持边界；不能将三者笼统标成“原生播放”。

**Bern 决策（标准控制条的进度与音量）。** `controls: standard` 的进度与音量使用专用的媒体进度条，不复用 `Slider` 组件的外观：已播放部分为中性的 `.primary`，不使用应用强调色。macOS 26 及以上直接使用系统 `Slider`，以 `.tint(.primary)` 取得中性的已播放部分，滑块保持系统外观；低于 macOS 26 时使用 Bern 绘制的回退，形状照 macOS 26 的系统滑块：6 pt 圆角轨道（未播放部分为 `quaternaryLabelColor`），20 × 16 pt 的白色圆角滑块（圆角 7 pt）带柔和阴影与细轮廓。回退向辅助功能报告为系统 `Slider`，接受方向键调整；拖动只更新预览，松开时提交一次跳转，与系统 `Slider` 的编辑结束相同。上述尺寸来自本机 macOS 27.0.1 上系统 `Slider` 的实际渲染（2026-10-10），不是 Apple 公布的规格。

## 9. 随包资源

**官方依据。** macOS 应用包把不是代码的内容放在 `Contents/Resources/`；可执行代码、框架、插件等各有固定位置，位置错误可能到公证时才暴露为签名问题。同一文档说明签名工具会把名称含点的目录当作包处理（针对嵌套代码的子目录给出）。[Placing content in a bundle](https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle)

**官方依据。** `Bundle.resourceURL` 返回包内存放资源的子目录的文件 URL，macOS 10.6 起可用；非标准包格式可能没有可用路径。[Bundle.resourceURL](https://developer.apple.com/documentation/foundation/bundle/resourceurl)

**Bern 决策。** 资源声明与引擎解析规则见设计文档“随包资源”；macOS SwiftUI 目标的映射如下。

| 维度 | Bern 适配决策 |
| --- | --- |
| 位置 | 资源 `a/b.ext` 复制到 `Contents/Resources/a/b.ext`；`<语言>.lproj` 留给生成的本地化表 |
| 名称 | 目录段不能含 `.`（文件名可以），同时避开 `.lproj` 与签名工具的包识别；违反时构建检查报错并指向 `Bern.toml` 条目 |
| 复制 | 只复制文件内容，不带扩展属性；在签名之前完成；删除不再声明的旧资源，保留生成的本地化文件 |
| 解析 | Swift 宿主启动引擎时传入 `Bundle.main.resourceURL`，生成的 Rust 胶水带有声明的名称目录，二者在构造引擎前装配；引擎得到包内文件 URL，不依赖构建机路径 |
| 范围 | 只发布数据文件；可执行代码、框架和插件需要各自的位置与签名，首版不作为资源支持 |

其他 Apple 平台的包结构在相应目标实现时按官方文档另行映射，应用的资源声明不变。

**Bern 验收。**

- 把应用复制到构建目录之外并移走源码中的资源文件，声明的资源仍能从包内加载。
- 未声明的名称即使在包中有同名文件也返回 `undeclared_resource`。
- 从清单删除资源后重新构建，包中不再残留旧文件，签名校验通过。

## 10. 官方来源与读取状态

以下条目均在核查日期访问；“正文”只说明资料证据已读，不说明 Bern 已实现或通过测试。

| 官方来源 | 状态 | 适用边界 |
| --- | --- | --- |
| [Layout HIG](https://developer.apple.com/design/human-interface-guidelines/layout) · [官方正文数据](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/layout.json) | 正文 | 跨 Apple 平台；页面更新记录 2026-09-09 |
| [Accessibility HIG](https://developer.apple.com/design/human-interface-guidelines/accessibility) · [官方正文数据](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/accessibility.json) | 正文 | 跨 Apple 平台；页面更新记录 2025-06-09 |
| [Color HIG](https://developer.apple.com/design/human-interface-guidelines/color) · [官方正文数据](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/color.json) | 正文 | 跨 Apple 平台；材料表现依 OS 与上下文 |
| [Tailor macOS windows](https://developer.apple.com/videos/play/wwdc2024/10148/) | 正文 | WWDC24；新窗口 API 按 macOS Sequoia 及可用性核查 |
| [SwiftUI cookbook for focus](https://developer.apple.com/videos/play/wwdc2023/10162/) | 正文 | macOS、iPadOS、tvOS；部分 API 从 iOS 17 / macOS Sonoma 起 |
| [Explore navigation design for iOS](https://developer.apple.com/videos/play/wwdc2022/10001/) | 正文 | WWDC22；导航语义可参考，旧视觉示例不是当前所有设备规则 |
| [Get started with Dynamic Type](https://developer.apple.com/videos/play/wwdc2024/10074/) | 正文 | WWDC24；文字缩放与布局验证 |
| [Catch up on accessibility in SwiftUI](https://developer.apple.com/videos/play/wwdc2024/10073/) | 正文 | WWDC24；SwiftUI 辅助功能语义 |
| [Elevate the design of your iPad app](https://developer.apple.com/videos/play/wwdc2025/208/) | 正文 | iPadOS 26 的窗口、菜单与外观 |
| [Create a great video playback experience](https://developer.apple.com/videos/play/wwdc2022/10147/) | 正文 | iOS、iPadOS、tvOS；系统播放器与输入，不证明 Bern 拦截能力 |
| [iPhone Duo 产品](https://www.apple.com/iphone-duo/) · [发布稿](https://www.apple.com/newsroom/2026/09/apple-unveils-iphone-duo/) | 正文 | 2026 年 9 月公布；核查时尚未上市 |
| [Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo) · [官方正文数据](https://developer.apple.com/tutorials/data/design/human-interface-guidelines/designing-for-iphone-duo.json) | 正文 | 2026-09-09 新增指南；iPhone Duo |
| [Prepare your app for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111461/) | 正文 | iOS 27.1 SDK / Xcode 27.1 的准备与模拟测试 |
| [Design for iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111466/) | 正文 | Duo 姿态、侧向控件与内容空间 |
| [Strike a pose with adaptive layouts](https://developer.apple.com/videos/play/tech-talks/111463/) | 正文 | Duo 区域与排列机制；具体符号版本实现时复核 |
| [Leverage multiple displays and scenes](https://developer.apple.com/videos/play/tech-talks/111464/) | 正文 | Duo 铰链、动态多场景与附属呈现 |
| [Placing content in a bundle](https://developer.apple.com/documentation/bundleresources/placing-content-in-a-bundle) · [官方正文数据](https://developer.apple.com/tutorials/data/documentation/bundleresources/placing-content-in-a-bundle.json) | 正文 | 2026-10-10 核查；macOS 包内各类内容的位置与签名影响 |
| [Bundle.resourceURL](https://developer.apple.com/documentation/foundation/bundle/resourceurl) · [官方正文数据](https://developer.apple.com/tutorials/data/documentation/foundation/bundle/resourceurl.json) | 正文 | 2026-10-10 核查；macOS 10.6+ 等平台 |
| [Designing for macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos/) | 仅搜索可见 | HTML 要求 JavaScript；本文 macOS 硬性结论由已读资料支撑 |
| [Designing for iOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-ios) | 仅搜索可见 | HTML 要求 JavaScript；未把索引当完整正文 |
| [Designing for iPadOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-ipados) | 仅搜索可见 | HTML 要求 JavaScript；具体新能力采用 WWDC25 正文 |
| [Designing for tvOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-tvos) | 仅搜索可见 | HTML 要求 JavaScript；焦点、安全区域和视频采用已读正文 |

## 11. 实现前必须明确的项目决策

- SwiftUI 目标按 macOS、iOS、iPadOS、tvOS 分别列版本与能力，不用一个总开关代表 Apple 支持。
- 环境协议保留不规则区域及动态 scene 能力；Apple 类型映射留在目标模块，公共 IR 不绑定 UIKit 符号。
- 原生多场景、虚拟窗口与附属呈现分别验证身份、关闭、恢复及资源所有权。
- 先原型验证导航手势、窗口关闭与播放器命令顺序，再承诺对应标准控件支持。
- 将本文件每个平台验收项转为后端场景清单；未完成实机验证的设备明确标注证据等级。
