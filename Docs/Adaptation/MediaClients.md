# Emby 和 Jellyfin 客户端适配

- 文档版本：v0.1
- 官方资料核查日期：2026-10-09
- 适用范围：用 Bern 开发的媒体客户端；作为框架验证场景，不将服务端业务写入 Bern 核心。
- 上位约束：[架构设计](../Design.md)与[自适应总则](README.md)。

Emby 和 Jellyfin 提供客户端文档、接口、品牌资源及部分界面规则。本次检索未定位到一份覆盖所有客户端平台、相当于 Apple HIG 的统一设计规范。因此，原生交互采用对应操作系统指南，服务端连接与媒体行为采用各自官方协议，下面的页面组织是 Bern 客户端设计建议。

## 官方资料的适用范围

| 官方资料 | 可以据此确定的内容 | 不能推导的内容 |
| --- | --- | --- |
| Emby 客户端用户文档 | 首页配置、功能行为、指定客户端的主题与交互选项 | 所有平台必须使用相同控件与布局 |
| Emby Plugin UI | 服务端插件设置页面的声明式生成方式 | 第三方完整播放器的跨平台 HIG |
| Emby REST 与 WebSocket 文档 | 会话控制、播放状态报告与能力字段 | 请求已发送即设备已执行 |
| Jellyfin Branding 与 jellyfin-ux | 项目命名、标识使用及共享视觉资源 | 第三方客户端必须复刻 Jellyfin Web |
| Jellyfin 客户端及媒体文档 | 登录方式、客户端范围、播放兼容性的判断维度 | 每个原生客户端都执行服务端 CSS |
| Jellyfin Style Guides | 源代码风格 | 视觉与交互设计系统 |

Emby 的插件界面由模型/属性描述生成，其适用对象是插件配置。Jellyfin 的 Style Guides 明确讨论代码风格；jellyfin-ux 的 README 主要列出品牌图像、标识等资源。它们可以提供参考，但不能替代平台适配规范。[Emby Plugin UI](https://dev.emby.media/doc/plugins/ui/index.html)、[Jellyfin Style Guides](https://jellyfin.org/docs/general/style-guides/)、[Jellyfin UX](https://github.com/jellyfin/jellyfin-ux)。

## Emby 官方行为与 Bern 映射

### 首页与浏览

Emby 当前首页文档针对 Server 4.10.x 及以后版本：区块可配置筛选、展示图片与滚动方向；Auto 滚动方向由客户端根据尺寸、方向和设备形态选择。旧版首页配置另有文档，不能将新版选项无条件发送给旧服务器。[User Preferences Home Screen](https://emby.media/support/articles/apps/common/User-Prefs-HomeScreen.html)。

Bern 客户端保留服务器支持的首页区块、顺序与用户偏好，用自己的组件呈现。窄屏可减少同时出现的列数，电视可以使用遥控器可达的横向内容行，桌面可使用更高信息密度的网格。重排不能改变筛选含义、已选媒体或播放队列；不支持的配置需明确呈现原因。

### 远程会话与播放状态

Emby 的 Remote Control 接口先枚举可控制会话，再按会话身份派发播放或通用控制；会话提供 SupportedCommands 等能力信息。WebSocket 文档将通知与远程控制消息分别描述。[Remote Control](https://dev.emby.media/doc/restapi/Remote-Control.html)、[Web Socket](https://dev.emby.media/doc/restapi/Web-Socket.html)。

Bern 映射规则：

- 用户先明确选择播放目标，界面持续显示本机或远程设备名称；同名设备仍按稳定身份区分。
- 服务端 Session ID、Bern session_id 和 player_id 是不同命名空间，由服务适配器维护映射，不能直接视为同一 ID。
- 远程播放请求先进入 Bern 已登记命令及权限检查，再由 Emby 适配器发送；远端状态回报更新同一播放目标的观察状态。
- `SupportedCommands` 影响动作是否可用，不动态改写 Bern 已编译命令 schema。
- 连接中、已发送、已确认、失败与目标离线分开显示。协议不能确认执行时，不宣称“已经播放”。

Emby Playback Check-ins 分别报告开始、进度与停止，并携带媒体源、播放会话和轨道等信息。其当前指南建议约每 10 秒报告进度，用户交互后及时报告；这个频率是 Emby 接口行为，不能成为 Bern 的绘制时钟或全部后端的固定规则。[Playback Check-ins](https://dev.emby.media/doc/restapi/Playback-Check-ins.html)。

Bern 只依据实际播放结果报告 started/stopped。换源、切换远端或重连后，旧播放会话的延迟回报不得覆盖新会话。服务端进度估计可以用于平滑展示，但要保留更新时间与连接状态。

### 主题与可访问性

Emby Windows 客户端的无障碍说明包含键盘焦点和高对比度；其中自动跟随系统高对比度有启用 Windows 主题且不覆盖深浅色的前提。这是该客户端的配置条件，不是 Bern 可以要求用户手动配置主题的理由。[Feature Accessibility](https://emby.media/support/articles/apps/windows/features/Win-Feature-Accessibility.html)。

Bern 控件仍执行自动深浅色和高对比度契约。服务端返回的封面与视频保持内容原色；颜色、标题长度及海报比例不能成为判断设备种类的条件。

## Jellyfin 官方行为与 Bern 映射

### 品牌与客户端独立性

Jellyfin 的品牌指南要求第三方使用独立名称与标识，可在说明中表达兼容关系。官方客户端目录的收录条件和界面设计要求是不同问题；本章不把“支持 Jellyfin”表述为“Jellyfin 官方客户端”。[Branding](https://jellyfin.org/docs/general/contributing/branding/)、[Clients](https://jellyfin.org/docs/general/clients/)。

Jellyfin Custom CSS 只作用于使用 Jellyfin Web 的客户端，并不覆盖所有客户端。因此原生 Bern 前端不需要解释或执行任意服务端 CSS；需要支持的用户展示偏好应映射成经过定义的类型化设置。[CSS Customization](https://jellyfin.org/docs/general/clients/css-customization/)。

### 电视登录

Quick Connect 使用临时代码，由已经登录的设备授权新客户端，适合电视等输入受限设备；服务器可关闭该功能，而且“允许登录”和“授权其他设备”是不同客户端能力。[Quick Connect](https://jellyfin.org/docs/general/server/quick-connect/)。

Bern 的电视连接页在服务端支持时提供这一入口，并保留明确的其他登录路径。等待授权、已授权、过期、拒绝和重新生成代码是不同状态。切换服务器或用户后，旧授权结果不得进入新会话。代码应便于远距离阅读，不把账号输入设计成只能依赖手机才能完成。

Emby 的登录由 Emby 适配器独立实现；不能因为两个产品存在历史关系就直接发送 Jellyfin Quick Connect 请求。

### 媒体兼容性

Jellyfin 官方媒体说明按容器、视频、音频与字幕的组合判断 Direct Play、Direct Stream 或转码。字幕可能改变播放方案，因此不能仅检查文件扩展名或一个视频编码名称就断言可直接播放。[Codec Support](https://jellyfin.org/docs/general/clients/codec-support/)。

Bern 的 MediaCapabilities 来自真实执行端，服务适配器据此协商播放源。UI 提供清晰的质量、音轨、字幕与播放目标选择；调试详情可解释转码原因，但普通播放流程不暴露内部协议字段。切换字幕或质量需要源版本及请求身份，避免多次异步协商互相覆盖。

## Bern 媒体客户端页面契约

下表是 Bern 的产品设计建议，功能由应用的 Feature Contract 声明；不是 Emby 或 Jellyfin 官方规定的页面顺序。

| 页面或动作 | 紧凑触屏 | 宽窗口和键鼠 | 电视与遥控器 |
| --- | --- | --- | --- |
| 服务器与用户选择 | 单列流程，清楚区分连接和登录错误 | 可并列展示已保存服务器与详情 | 大字号、少输入、支持时提供配对登录 |
| 媒体库与首页 | 当前区块和可滚动网格 | 侧边导航与内容网格 | 焦点明确的内容行或网格 |
| 搜索与筛选 | 搜索页或覆盖面板，筛选可返回 | 搜索框、快捷键与辅助筛选区 | 遥控器可达搜索入口，不依赖悬停 |
| 详情与剧集 | 逐级导航，保留来源列表位置 | 列表详情或辅助面板 | 明确的播放/继续播放入口，焦点恢复到原条目 |
| 播放控制 | 触控控件与可发现的音轨字幕菜单 | 键盘与鼠标并行，菜单列出快捷键 | 方向键、确认、返回、播放暂停键完整可用 |
| 播放目标 | 明确显示当前本机或远端目标 | 目标列表和状态可查询 | 大屏上仍可查看并切换目标，不只显示模糊投屏图标 |
| 设置 | 分类导航或虚拟窗口 | 原生设置窗口或应用内面板 | 按任务分组，全屏虚拟窗口和明确返回路径 |

继续播放与从头播放是不同命令；切换视图不能重复派发播放。恢复进度、收藏、标记已看、字幕选择、远程控制等操作均进入统一引擎目录。不能通过调用外部播放器私有回调绕过四入口契约。

认证、媒体目录、播放历史、服务端偏好和服务协议位于应用业务模块。Bern 核心只提供通用命令、状态、导航、Window、Video、主题和可访问性机制。

## 状态和错误设计

| 情况 | 客户端必须表达的差异 |
| --- | --- |
| 未发现服务器 | 发现失败与手动地址入口；不能直接判断账号错误 |
| 局域网权限被拒绝 | 权限状态与服务端离线分别报告，按平台规则引导恢复 |
| 会话失效 | 保留允许恢复的导航，重新认证后重新验证访问权限 |
| 媒体不兼容 | 区分本地后端不支持、服务器可转码、无可用播放方案 |
| 远端掉线 | 目标不可用与最后观察状态并存，不继续确认播放成功 |
| 有进度但未知总时长 | 不把未知长度显示为零，不提供无法完成的任意跳转 |
| 用户或服务器切换 | 清理旧身份所属订阅与敏感显示，不串用进度或授权 |

局域网与后台播放要求随目标平台及版本变化，需按目标平台另行核验具体权限与运行条件；布局和生命周期参考 [Apple](Apple.md)、[Android](Android.md) 和 [HarmonyOS](HarmonyOS.md) 分册，不用重试掩盖权限拒绝。

## 验收清单

- [ ] 同一媒体从首页、搜索、详情和外部命令进入时定位相同媒体与播放目标。
- [ ] 服务器能力不足时动作提供不可用原因，不因窄屏隐藏必需功能。
- [ ] 遥控器能完成登录、搜索、选集、播放、字幕选择和退出，焦点没有死路。
- [ ] 从详情返回长列表后恢复正确条目，列表刷新或重排不恢复到同位置的另一项。
- [ ] 关闭设置窗口不会停止应用级播放器，切换设备布局不重复播放。
- [ ] 同一 player_id 的 UI、CLI、JSON 和 MCP 操作经过相同命令处理与结果同步。
- [ ] 远程请求不冒充本地播放，旧会话回报和过期登录码不会污染新状态。
- [ ] 分别测试可直接播放、需改变封装、需音视频转码、字幕导致方案变化和彻底不支持。
- [ ] 播放 started/progress/stopped 与真实执行端一致；协议测试不以 UI 截图代替。
- [ ] 深浅色、高对比度、长标题、放大字体、屏幕阅读器和电视远距离可读性通过对应平台检查。
- [ ] Emby 与 Jellyfin 分别对明确记录的服务器版本进行协议测试，不假设接口永久相同。

## 来源核验记录

以下官方页面在 2026-10-09 已读取相关正文；其中 REST 指南包含历史示例，API 接入前仍应匹配待支持服务器的正式 schema。

| 来源 | 范围或版本边界 |
| --- | --- |
| [Emby Home Screen](https://emby.media/support/articles/apps/common/User-Prefs-HomeScreen.html) | 页面明确覆盖 Server 4.10.x 及以后 |
| [Emby Plugin UI](https://dev.emby.media/doc/plugins/ui/index.html) | 服务端插件配置界面 |
| [Emby Accessibility](https://emby.media/support/articles/apps/windows/features/Win-Feature-Accessibility.html) | 指定 Windows 客户端及主题条件 |
| [Emby Remote Control](https://dev.emby.media/doc/restapi/Remote-Control.html) | 会话枚举、能力与远控请求 |
| [Emby Web Socket](https://dev.emby.media/doc/restapi/Web-Socket.html) | 通知与远控消息；不是 Bern 公共协议 |
| [Emby Playback Check-ins](https://dev.emby.media/doc/restapi/Playback-Check-ins.html) | 播放状态回报 |
| [Jellyfin Branding](https://jellyfin.org/docs/general/contributing/branding/) | 品牌规则 |
| [Jellyfin UX](https://github.com/jellyfin/jellyfin-ux) | 官方资源仓库 README，未据此审核全部资源 |
| [Jellyfin Style Guides](https://jellyfin.org/docs/general/style-guides/) | 代码风格说明 |
| [Jellyfin Clients](https://jellyfin.org/docs/general/clients/) | 客户端范围与目录收录条件 |
| [Jellyfin CSS Customization](https://jellyfin.org/docs/general/clients/css-customization/) | Jellyfin Web 的自定义 CSS 范围 |
| [Jellyfin Quick Connect](https://jellyfin.org/docs/general/server/quick-connect/) | 配对登录行为及客户端能力差异 |
| [Jellyfin Codec Support](https://jellyfin.org/docs/general/clients/codec-support/) | 兼容性判断原则；表中结果不可替代设备运行检测 |

尚未完成服务器联调、真实播放或客户端视觉测试。上述勾选项用于后续实现验收。
