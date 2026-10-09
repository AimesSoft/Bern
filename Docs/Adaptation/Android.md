# Android、折叠屏与 Android TV 自适应适配

- 核查日期：2026-10-09。
- 文档性质：Bern 架构准备与后端验收依据；尚未实现的接口、布局策略与测试均为设计要求。
- 范围：Android 手机、平板、折叠屏、可调整窗口与外接输入，以及 Android TV / Google TV。
- 关联：[框架设计](../Design.md)。Android 默认前端技术沿用 Jetpack Compose；媒体后端单独选择。
- 证据边界：下文“官方依据”描述来源事实；“Bern 决策”是框架对这些事实的映射，不冒充 Google 规范。

## 1. 自适应的输入是当前窗口

**官方依据。** Android 将当前应用可用窗口划分为尺寸类别，并分别观察宽与高；类别会随旋转、分屏和折叠改变。不能用 `isTablet`、整块物理屏幕或设备型号代替。[窗口尺寸类别](https://developer.android.com/develop/adaptive-apps/guides/use-window-size-classes)

| 维度 | 当前官方类别 | 边界 |
| --- | --- | --- |
| 宽 | Compact | `< 600 dp` |
| 宽 | Medium | `600 ≤ width < 840 dp` |
| 宽 | Expanded | `840 ≤ width < 1200 dp` |
| 宽 | Large | `1200 ≤ width < 1600 dp` |
| 宽 | Extra-large | `width ≥ 1600 dp` |
| 高 | Compact | `< 480 dp` |
| 高 | Medium | `480 ≤ height < 900 dp` |
| 高 | Expanded | `height ≥ 900 dp` |

表中数值来自上述官方页面，不能把旧资料中的“≥ 840 dp 全部属于 Expanded”继续作为当前完整分类。宽度足够但高度很低时，仍需评估多栏是否可用。

**Bern 决策。** Android 适配器上报窗口逻辑尺寸、密度、字体缩放、系统遮挡和姿态；公共布局规则消费这些能力与尺寸。上述断点属于 Android 策略，不成为所有平台的硬编码阈值。

默认策略先选布局关系，再计算列宽和留白。增大屏幕应增加信息、并置关联内容或改善阅读宽度，不能无限拉伸同一个手机表单。

Android 当前文档分别出现 `currentWindowAdaptiveInfo(supportLargeAndXLargeWidth = true)` 与较新的 `currentWindowAdaptiveInfoV2()`。后端实现时需锁定实际依赖和稳定性，不能把网站上的最新示例名直接承诺为 Bern 的最低库版本。[尺寸 API](https://developer.android.com/develop/adaptive-apps/guides/use-window-size-classes)、[姿态 API](https://developer.android.com/develop/adaptive-apps/guides/foldables/make-your-app-fold-aware)

## 2. 布局关系、导航与内容连续性

**官方依据。** 官方典型布局包括列表—详情、内容网格和主内容—辅助面板。列表—详情在足够宽时并置；缩小时保留用户当前查看的详情，返回后再显示列表。[Canonical layouts](https://developer.android.com/develop/adaptive-apps/guides/canonical-layouts)

**Bern 决策。** 为这些关系提供语义化适配，而不要求业务分别写“手机页面”和“平板页面”：

| 内容关系 | 狭窄窗口 | 宽窗口 | 必须保留 |
| --- | --- | --- | --- |
| 媒体列表—详情 | 当前页与返回栈 | 列表和详情并置 | 选中 ID、列表锚点、详情路由 |
| 视频—播放列表 | 视频优先，列表按需打开 | 主播放器加辅助面板 | 同一 `player_id`、当前集、播放位置 |
| 海报内容流 | 可读的少量列 | 根据最小卡片宽度增加列数 | 稳定项目 key、筛选与滚动锚点 |
| 设置与表单 | 有界阅读宽度、自然滚动 | 分类与表单并置 | 编辑草稿、验证结果、输入焦点 |

布局重排不能触发“重新搜索”“重新加载当前影片”等业务命令。系统尺寸变化是环境事实，产生新的布局；用户选择内容才执行对应业务命令。

窄屏导航入口与宽屏导航入口可以改变位置或表现，但必须引用同一命令与 Feature。底部导航、导航轨、侧栏的选择属于目标策略；不能把外观切换误当作应用路由变化。

## 3. 折叠屏、双屏与铰链

**官方依据。** `FoldingFeature` 提供边界、方向、折叠状态、遮挡类型及是否分隔窗口；半开且水平的折痕对应 tabletop，半开且垂直对应 book。该接口不提供精确铰链角度。[Make your app fold aware](https://developer.android.com/develop/adaptive-apps/guides/foldables/make-your-app-fold-aware)

**Bern 决策。** 环境协议记录可分隔区域和遮挡几何，不记录某个品牌或“折叠手机”的布尔值：

- 使用窗口坐标系转换铰链区域；原生像素先转换为目标布局坐标，不直接当作 `dp`。
- `occlusionType = FULL` 的区域不能承载正文、按钮、字幕或拖动把手。
- `isSeparating` 为真时优先把自然分界放在区域之间；不能只看遮挡宽度是否为零。
- 折叠信息缺失时采用当前尺寸的普通布局；不能据此声称设备没有铰链。
- 支持多个 display feature 的输入结构，布局不依赖“永远只有第一条折痕”。

| 姿态 | Bern 的初始适配策略 |
| --- | --- |
| 窄外屏 / 折叠后 | 单栏导航；保持当前详情、草稿和播放器身份 |
| 平展大屏 | 按窗口与分隔区域选择一栏或多栏 |
| Tabletop | 上方显示视频，下方放命令化播放控制与辅助内容 |
| Book / 被铰链分开的双屏 | 将列表—详情等关系分别放入两侧可用区域 |

同样的视觉“平展”不代表同样的遮挡：双屏铰链在平展时仍可能形成分隔。用户控件应远离不便触达的折痕边缘。[折叠与双屏说明](https://developer.android.com/develop/adaptive-apps/guides/foldables/make-your-app-fold-aware)

上表是 Bern 初始策略；应用可以声明主要内容和辅助内容角色。折叠展开不能创建第二个播放器，也不能把当前进度恢复成页面初始值。

## 4. 系统版本与多窗口边界

| 官方行为 | Bern 适配要求 | 官方来源 |
| --- | --- | --- |
| Android 16、target API 36，在最小显示宽度 `sw ≥ 600 dp` 的适用环境忽略方向、比例和不可缩放限制；存在官方列出的例外及临时退出机制 | 默认可调整尺寸；不依赖锁定方向维持 UI 正确性 | [方向、比例和可缩放性](https://developer.android.com/develop/adaptive-apps/guides/app-orientation-aspect-ratio-resizability) |
| Android 17、target API 37 移除上述开发者临时退出机制 | 新目标不能把退出开关当长期适配方案；构建矩阵记录运行系统和 target SDK | [Android 17 行为变更](https://developer.android.com/about/versions/17/behavior-changes-17) |
| 系统提供分屏、画中画和桌面窗口等不同模式，具体支持取决于环境 | 分别声明能力，不用一个 `multi_window = true` 概括全部 | [多窗口模式](https://developer.android.com/develop/adaptive-apps/guides/support-multi-window-mode) |

这里的 `sw` 系统行为条件与第 1 节“当前应用窗口宽度”不是同一个量。小窗口可以运行在大显示器上；布局仍以自己的窗口空间为准。

**Bern 决策。** `Window(presentation: auto)` 只有在宿主确实能创建所需独立窗口、并维持 Bern 窗口契约时才映射原生窗口。否则在应用内提供虚拟窗口；窄屏使用活动窗口加切换器，不模拟难以操作的小型桌面浮窗。

系统分屏支持不等于任意应用都能随时创建第二个自由浮动窗口。跨窗口复用同一引擎和所属 UI 会话；每个窗口保留独立的窗口作用域、返回栈、焦点和草稿。原生窗口已发生的尺寸与生命周期变化按事实回报，不循环执行同名命令。

官方说明相邻 Activity 启动是尽力请求而非创建位置保证；多实例也不同于同一窗口的多面板布局。Bern 必须检查实际创建结果后再报告窗口已打开。[多窗口与多实例](https://developer.android.com/develop/adaptive-apps/guides/support-multi-window-mode)

## 5. 系统边缘、键盘与返回

**官方依据。** Android 15 及以上、应用 target SDK 35 后强制 edge-to-edge；适配器需处理状态栏、导航栏、桌面标题栏、显示缺口和 IME 等实际 insets。`safeDrawing`、`safeGestures`、`safeContent` 保护范围不同。[Window insets](https://developer.android.com/develop/ui/compose/system/insets)

**Bern 决策。** 背景和视频可铺满窗口，正文、可操作入口与字幕遵守相应安全区域。系统手势区与纯绘制遮挡分开保存；不能统一给整个 UI 增加固定顶部或底部空白。

IME 出现时优先让当前编辑位置和提交入口可见；防止父子容器重复消费同一 inset。虚拟窗口内部仍继承宿主的系统约束。文本组合、候选选择和提交分别建模，不能把中文输入过程中每次组合更新当成一次业务搜索。

官方预测返回允许用户预览返回目标，并通过系统返回接口接入应用导航。[Predictive back](https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture)

**Bern 决策。** 返回手势的采样只更新交互预览，完成后执行一次类型化返回命令；取消时恢复预览，不改变持久路由。模态、输入法、窗口与应用返回层次由宿主协调，不同时弹出两层。

## 6. 键盘、鼠标、触控与无障碍

**官方依据。** 大屏指南要求考虑键盘 Tab/方向键导航、常用快捷键、鼠标右键与滚轮。复杂滚动集合需要明确焦点分组，不能只依赖默认空间推断。[Input compatibility](https://developer.android.com/develop/ui/compose/touch-input/input-compatibility-on-large-screens)

**Bern 决策。** 输入方式可以运行时并存与切换；不能检测到触屏就隐藏全部键盘能力：

- 点击、Enter、遥控确认键和无障碍激活共用同一动作命令。
- Tab 顺序、方向焦点和读屏顺序是不同关系；各自检查可达性。
- 列表虚拟化和重排通过项目 ID 恢复焦点，不能恢复到已被回收的原生对象。
- 滚轮、触控滚动与方向跳转共享滚动状态；连续输入可合并，最终语义可观察。
- 保存、撤销、复制等快捷键采用平台约定；TextField 正在编辑时优先保留文本系统语义。
- 右键菜单、长按和无障碍自定义动作都需保留功能入口；不能提供只能悬停后发现的必要功能。

**官方依据。** Android Compose 的无障碍默认建议可交互触控区域至少 `48 × 48 dp`，并要求为不能从文本理解的图片或图标提供语义描述。[Accessibility API defaults](https://developer.android.com/develop/ui/compose/accessibility/api-defaults)

**Bern 决策。** 最小热区与图形尺寸分离。Compose 目标使用 Material 标准的图标按钮样式，按平台规则提供触达反馈；透明热区保证触达，焦点反馈必须清晰。

控件输出名称、角色、值、选中/禁用状态与动作；装饰性图片不重复朗读。适配器消费字体缩放和语言变化，文字允许换行与自然增高；不能把字体缩小回默认大小来挤入固定高度。

## 7. 深浅色与平台主题

**官方依据。** Material 3 使用语义颜色角色；动态取色在 Android 12 及以上可用，不可用时回落到自定义深浅色方案。[Material 3 in Compose](https://developer.android.com/develop/ui/compose/designsystems/material3)

**Bern 决策。** Compose 具备完整的深浅色体系，按“原生优先”使用 Material 3 配色：可接入动态取色，不可用时使用由品牌色或默认种子生成的 Material 深浅色方案。Bern 主题算法只用于 Bern 自行绘制的部分；应用作者不需逐控件编写深色分支。

系统栏图标、弹层、选择状态与输入控件跟随窗口解析后的主题。视频像素、海报与字幕内容不做全局反色；媒体上方的文字与控件通过受控承载表面保证可辨识性。

TV 媒体应用可显式选择适合观看环境的深色策略，但框架仍必须生成可用的浅色方案。默认 system 模式下没有外观信号时，沿用 Bern 核心规定的浅色回退，不能伪造一个系统深色状态。

## 8. Android TV / Google TV

### 8.1 布局与安全区

**官方依据。** TV 面向远距离观看和横向显示；开发指南建议使用 `dp`/`sp`，关键内容置于约 5% 的 overscan 安全区域，背景可以铺满。常用 `960 × 540` 设计基准对应左右 `48 dp`、上下 `27 dp`。[Build TV layouts](https://developer.android.com/training/tv/playback/compose/layouts)

**来源冲突。** 另一官方设计页同时写有 `48/27 dp`、`58/28 dp` 和上下 `27` 四舍五入为 `24` 的示例；不能把这些数字全写成统一强制标准。Bern 首版采用约 5% 安全策略，以真实电视测试校正，而非对所有分辨率硬编码一组数。[TV Layouts 原文](https://developer.android.com/design/ui/tv/guides/styles/layouts)

**Bern 决策。** TV 是独立交互配置，不是把平板 UI 等比放大。主要浏览采用可预测的横向内容行或网格；限制一屏文本密度，所有必需功能都能用遥控器完成。不得以滑动、悬停、双击或精细拖动作为唯一入口。

### 8.2 焦点与返回

**官方依据。** TV 导航以方向键与返回键为核心，应有清楚路径、可辨识焦点；返回不应陷入菜单循环，也不应通过普通退出确认阻挡离开。遥控器已提供返回，通常无需屏幕上的重复返回按钮。[TV navigation](https://developer.android.com/training/tv/get-started/navigation)

**Bern 决策。** 每个页面、列表行与弹层声明焦点作用域。打开详情记住来源卡片，返回时优先恢复；来源已移除时选取明确备用项。弹层获得焦点后，底层不可误激活。长按连续移动必须跟随项目身份，不能因异步海报加载跳焦。

普通离开应用不弹“是否退出”；确有未保存编辑时走关闭契约的具体草稿确认。虚拟窗口关闭之后回到上一个活动窗口，不依靠遥控器操作桌面式标题栏。

Google TV / Android TV Live tab 合作接入有额外 direct-back 要求，且与普通深链接不同；仅在应用启用此集成时实现相应来源标记与返回规则，不能把它强加给所有页面。[Live tab navigation](https://developer.android.com/training/tv/get-started/navigation#live-tab-navigation)

### 8.3 播放操作

**官方依据。** 播放场景中，中心键播放/暂停，左右单击前后跳转，左右长按连续定位；上下键显示进度等信息且不因此暂停。跳转秒数由应用定义，官方表用 `N`，不是统一 10 秒。[Playback controls on TV](https://developer.android.com/training/tv/playback/controls)

**Bern 决策。** `Video(controls: standard)` 的 TV 配方自动产生上述命令绑定，并区分“直接控制播放”与“控制条内导航”的焦点上下文。一个按键不能既移动焦点又额外快进。

进度定位、字幕、音轨与播放速度需要可聚焦的语义入口；直播或不可定位媒体明确禁用相应动作并报告原因。控件自动隐藏后保持确定的恢复位置，字幕不因控制条出现而落到安全区外。

## 9. 播放器、生命周期与系统入口

**官方依据。** 配置变化后应用应维持上下文与状态，尤其不能丢失输入或中断连续的媒体体验。[Configuration and continuity](https://developer.android.com/guide/topics/large-screens/configuration-and-continuity)

**Bern 决策。** Rust 引擎、播放器所有者、Activity、窗口会话和视频呈现挂载是不同生命周期。旋转、折叠、调整尺寸只重挂视图；系统实际释放资源时可恢复状态，但不能宣称进程被杀后仍在连续播放。

可见、获得输入焦点和 resumed 是不同状态；不能在任何 `onPause` 到来时一律停止视频。多窗口中的可见播放器按产品播放策略和系统资源约束处理。[多窗口生命周期](https://developer.android.com/develop/adaptive-apps/guides/support-multi-window-mode)

**官方依据。** Media3 的后台播放方案通过 `MediaSessionService` 管理 Player 与 Session，UI 以 `MediaController` 连接；播放前台服务需要对应 manifest 权限和 `mediaPlayback` 类型。[Background playback](https://developer.android.com/media/media3/session/background-playback)

**Bern 决策。** 系统通知、蓝牙媒体键、遥控播放键与应用 UI 进入同一播放器命令入口。Media3 默认直接处理 Controller 命令，适配器必须在实际播放器调用边界接入 Bern 命令路由，不能仅在播放后补写命令日志。

播放器观察到的缓冲、结束、音轨变化、系统强制暂停属于事实事件；这些状态更新不能再次反向发出播放命令形成回环。系统音频焦点策略可以先于应用生效，框架应如实同步实际状态。

**版本约束。** target API 35 及以上请求音频焦点需要处于顶层应用或运行前台服务。Android 17 对后台音频进一步收紧，target API 37 应按当前 while-in-use 等要求校验；拥有一个常驻 Rust 引擎不代表有后台播放授权。[音频焦点](https://developer.android.com/media/optimize/audio-focus)、[Android 17 后台音频](https://developer.android.com/about/versions/17/changes/bg-audio)

**Bern 决策。** UI、CLI、JSON 和 MCP 收到同样的“需要前台交互/权限”“播放能力不可用”等结构化结果。后台限制、设备丢失或用户拒绝不能降格成“命令成功”。

Android 17 部分后台音频 API 会静默失效，因此后端需先校验可知的生命周期条件，再区分请求已接受和已确认状态；无法确认音频输出时不得伪造成功证据。诊断测试应覆盖官方提供的音频强化测试模式。[静默失败与验证方法](https://developer.android.com/about/versions/17/changes/bg-audio)

## 10. 媒体服务器连接与实施边界

**官方依据。** Android 17、target API 37 的本地网络访问要求适用权限或系统中介路径；这会影响局域网服务器发现与连接。[Android 17 本地网络权限](https://developer.android.com/about/versions/17/behavior-changes-17)

**Bern 决策。** Emby/Jellyfin 等应用的“找不到服务器”必须区分未授权、地址不可达、TLS 失败、认证失败和服务器不支持。系统权限提示由平台宿主触发；远程命令返回待授权状态，不能绕过用户选择。

框架提供通用连接状态、权限能力、焦点和播放器语义；媒体库模型、账号认证、服务器选择、转码与协议兼容由应用/协议模块实现。API 权限变化不应迫使业务重新编写所有布局。

## 11. 验收清单

以下为待执行的后端验收要求，本次仅完成研究与文档整理：

- [ ] 在 599/600、839/840、1199/1200、1599/1600 dp 两侧检查宽度策略；同时验证矮窗口。
- [ ] 在手机、平板、内外折叠屏、分屏与可调整窗口中保留选中项、草稿、滚动锚点和播放器身份。
- [ ] 注入水平/垂直铰链、FULL 遮挡、分隔但零宽、无姿态数据；重要内容与热区均可达。
- [ ] API 35/36/37 的运行系统与 target SDK 组合分别测试；不靠锁定方向掩盖问题。
- [ ] 覆盖手势导航、三键导航、显示缺口、桌面标题栏与 IME 动画；无重复 inset 或提交入口遮挡。
- [ ] 预测返回完成与取消各测试；弹层、窗口与应用路由只处理一次。
- [ ] 字体放大、长翻译、RTL、TalkBack、键盘 Tab/方向键、鼠标滚轮/右键均能完成关键流程。
- [ ] 所有主题与控件状态通过 Bern 既有配方检查；透明图标热区不相互重叠。
- [ ] TV 仅用遥控器完成登录、浏览、搜索、播放、字幕/音轨选择和返回；无焦点死路。
- [ ] TV 测试 overscan、不同分辨率、长按连续导航；真实观看距离下文本和焦点清晰。
- [ ] 每个播放按键只触发一个预期命令；系统媒体入口与四种应用入口共享实际播放器状态。
- [ ] 测试切页、旋转、Activity 重建、进后台、音频焦点丢失、服务结束与状态恢复。
- [ ] 拒绝局域网权限和后台播放限制时显示真实原因，命令结果与 UI 一致。

## 12. 来源读取状态与待核实项

| 来源组 | 本次读取状态 | 采用范围 / 限制 |
| --- | --- | --- |
| Android 窗口尺寸、典型布局、fold-aware 指南 | 已打开原文并读取相关段落 | 尺寸分类、布局关系、姿态与 API 语义 |
| Android 16 方向/比例/缩放指南；Android 17 行为变更与专项页 | 已打开原文 | 必须保留运行版本与 target SDK 条件；专项页将阈值写为 `>600`，总览写 `≥600`，Bern 在边界一并测试 |
| WindowInsets、外接输入、无障碍默认、Material 3 in Compose | 已打开原文并读取相关段落 | 系统遮挡、输入、热区、动态取色；不声称覆盖所有无障碍标准 |
| TV Layouts、Build TV layouts、TV navigation、Playback controls | 已打开原文并读取相关段落 | 安全区数字冲突已在正文显式记录 |
| 配置连续性、MediaSessionService、音频焦点、Android 17 后台音频 | 已打开原文 | 生命周期与系统控制入口；实际依赖组合仍需原型验收 |
| Material 官网窗口尺寸页 | 已尝试打开，但读取工具返回错误 | 未据此引入未读取事实；使用 Android 官方对应指南 |
| Android 多窗口模式指南 | 已打开原文并读取模式、生命周期及多实例段落 | 不把相邻窗口请求当作成功保证；精细 Activity 启动策略仍需设备验证 |

后端实施前还需确定 Compose/Adaptive/WindowManager/Media3 的锁定版本、最低 Android 版本、原生多窗口实现范围及 TV 发布渠道要求。本文不把文档研究等同于已通过设备认证。
