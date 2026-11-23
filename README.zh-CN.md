<div align="center">

<img src="Docs/Assets/logo/bern-logo.svg" width="128" alt="Bern 的标志">

<h1>Bern</h1>

<p><b>用 Rust 编写跨平台原生应用的 UI 框架。</b></p>

<p>业务逻辑和声明式 <code>ui!</code> 界面全用 Rust 编写——不捆绑浏览器内核，也无需按平台手写胶水代码。<br>
构建时直接编译成各平台自己的原生界面（SwiftUI、WinUI 3、GTK 4、Jetpack Compose，以及可选 GPUI 与 Web），<br>
覆盖桌面、手机、平板、手表、电视与网页。<br>
基于类型化协议与命令架构：逻辑留在纯 Rust，写漏在编译期报错，自动生成应用 CLI 与 <code>SKILL.md</code>，且随时可用 <code>bern eject</code> 导出配好 Rust FFI 的独立原生工程（无框架锁定）。</p>

<p>
<img alt="macOS · SwiftUI" src="https://img.shields.io/badge/macOS-SwiftUI-0A84FF?style=for-the-badge&logo=apple&logoColor=white">
<img alt="iOS · SwiftUI" src="https://img.shields.io/badge/iOS-SwiftUI-0A84FF?style=for-the-badge&logo=apple&logoColor=white">
<img alt="watchOS · SwiftUI" src="https://img.shields.io/badge/watchOS-SwiftUI-0A84FF?style=for-the-badge&logo=apple&logoColor=white">
<img alt="tvOS · SwiftUI" src="https://img.shields.io/badge/tvOS-SwiftUI-0A84FF?style=for-the-badge&logo=apple&logoColor=white">
<img alt="Windows · WinUI 3" src="https://img.shields.io/badge/Windows-WinUI%203-0078D4?style=for-the-badge&logo=windows11&logoColor=white">
<img alt="Linux · GTK 4" src="https://img.shields.io/badge/Linux-GTK%204-4A86CF?style=for-the-badge&logo=gtk&logoColor=white">
<img alt="Android · Jetpack Compose" src="https://img.shields.io/badge/Android-Jetpack%20Compose-3DDC84?style=for-the-badge&logo=android&logoColor=white">
<img alt="Wear OS · Compose" src="https://img.shields.io/badge/Wear%20OS-Compose-3DDC84?style=for-the-badge&logo=wearos&logoColor=white">
<img alt="Desktop · GPUI" src="https://img.shields.io/badge/Desktop-GPUI-084CCF?style=for-the-badge&logo=zedindustries&logoColor=white">
<img alt="Web · WebAssembly" src="https://img.shields.io/badge/Web-WebAssembly-654FF0?style=for-the-badge&logo=webassembly&logoColor=white">
</p>

<p>
<a href="https://bern.aimes-soft.com"><img alt="官网" src="https://img.shields.io/badge/%E5%AE%98%E7%BD%91-bern.aimes--soft.com-111827?style=flat-square"></a>
<img alt="原生界面：无 WebView" src="https://img.shields.io/badge/%E7%95%8C%E9%9D%A2-%E5%8E%9F%E7%94%9F%20%C2%B7%20%E6%97%A0%20WebView-111827?style=flat-square">
<a href="#导出独立原生工程无锁定与-rust-ffi-模板"><img alt="无锁定：bern eject" src="https://img.shields.io/badge/%E9%94%81%E5%AE%9A-%E6%97%A0%20%C2%B7%20bern%20eject-10B981?style=flat-square"></a>
<img alt="内存约 40 MB" src="https://img.shields.io/badge/%E5%86%85%E5%AD%98-%E7%BA%A6%2040%20MB-0EA5E9?style=flat-square">
<img alt="Skills + CLI" src="https://img.shields.io/badge/AI-Skills%20%2B%20CLI-8B5CF6?style=flat-square">
<img alt="Rust 1.99" src="https://img.shields.io/badge/Rust-1.99-B7410E?style=flat-square&logo=rust&logoColor=white">
<a href="LICENSE"><img alt="许可证：MIT" src="https://img.shields.io/badge/%E8%AE%B8%E5%8F%AF%E8%AF%81-MIT-22C55E?style=flat-square"></a>
<img alt="状态：早期预览" src="https://img.shields.io/badge/%E7%8A%B6%E6%80%81-%E6%97%A9%E6%9C%9F%E9%A2%84%E8%A7%88-F59E0B?style=flat-square">
</p>

<p>
<a href="README.md">English</a> ·
<b>简体中文</b> ·
<a href="README.zh-Hant.md">繁體中文</a> ·
<a href="README.ja.md">日本語</a>
</p>

<p>
<a href="https://bern.aimes-soft.com">官网</a> ·
<a href="https://bern.aimes-soft.com/docs/zh-CN">在线文档</a> ·
<a href="#核心特色">核心特色</a> ·
<a href="#用-rust-怎么编写软件">用 Rust 怎么编写软件</a> ·
<a href="#为-ai-而建">为 AI 而建</a> ·
<a href="#导出独立原生工程无锁定与-rust-ffi-模板">原生导出 / FFI 模板</a> ·
<a href="#实测数据">实测数据</a> ·
<a href="#与其他跨平台框架对比">对比</a> ·
<a href="#快速开始">快速开始</a>
</p>

</div>

<br>

<table>
<tr>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/macos-zh.png" alt="展示应用在 macOS（SwiftUI）上"><br><sub><b>macOS</b> · SwiftUI</sub></td>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/windows-zh.png" alt="展示应用在 Windows（WinUI 3）上"><br><sub><b>Windows</b> · WinUI 3</sub></td>
</tr>
<tr>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/linux-en.png" alt="展示应用在 Linux（GTK 4 与 libadwaita）上"><br><sub><b>Linux</b> · GTK 4 + libadwaita</sub></td>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/web-zh.png" alt="展示应用在浏览器中"><br><sub><b>Web</b> · DOM + WebAssembly</sub></td>
</tr>
</table>

<table>
<tr>
<td width="25%" align="center"><img src="Docs/Assets/screenshots/ios-zh.png" width="175" alt="展示应用在 iPhone（iOS SwiftUI）上"><br><sub><b>iOS</b> · SwiftUI</sub></td>
<td width="25%" align="center"><img src="Docs/Assets/screenshots/android-zh.png" width="175" alt="展示应用在 Android（Jetpack Compose）上"><br><sub><b>Android</b> · Jetpack Compose</sub></td>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/gpui-zh.png" width="360" alt="原生控件示例在 macOS GPUI 上"><br><sub><b>桌面端 · GPUI</b>（<code>ui = "gpui"</code>，搭配紧邻 <code>NativeView</code>）</sub></td>
</tr>
</table>

<details>
<summary><b>更多设备形态：</b>iPad、Android 平板、Apple TV、Android 电视、iPhone Duo、watchOS 与 Wear OS</summary>
<br>

<table>
<tr>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/ipad-zh.png" alt="展示应用在 iPad（iPadOS SwiftUI）上"><br><sub><b>iPad</b> · iPadOS SwiftUI</sub></td>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/android-tablet-zh.png" alt="展示应用在 Android 平板（Jetpack Compose）上"><br><sub><b>Android 平板</b> · Jetpack Compose</sub></td>
</tr>
<tr>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/tvos-zh.png" alt="展示应用在 Apple TV（tvOS SwiftUI）上"><br><sub><b>Apple TV</b> · tvOS SwiftUI</sub></td>
<td width="50%" align="center"><img src="Docs/Assets/screenshots/android-tv-zh.png" alt="展示应用在 Android 电视（Jetpack Compose）上"><br><sub><b>Android 电视</b> · Jetpack Compose</sub></td>
</tr>
</table>

<table>
<tr>
<td width="40%" align="center"><img src="Docs/Assets/screenshots/iphone-duo-zh.png" width="235" alt="展示应用在 iPhone Duo（iOS SwiftUI）上"><br><sub><b>iPhone Duo</b> · SwiftUI</sub></td>
<td width="30%" align="center"><img src="Docs/Assets/screenshots/watchos-en.png" width="145" alt="手表应用在 watchOS（SwiftUI）上"><br><sub><b>watchOS</b> · SwiftUI</sub></td>
<td width="30%" align="center"><img src="Docs/Assets/screenshots/wearos-zh.png" width="145" alt="手表应用在 Wear OS（Compose）上"><br><sub><b>Wear OS</b> · Compose Material 3</sub></td>
</tr>
</table>

</details>

<p align="center"><sub>同一份 Rust 代码覆盖桌面、手机、平板、电视、手表与网页。在系统原生目标上 Bern 不自己画任何一个像素；<br>需要统一 GPU 渲染时，切换到 <code>ui = "gpui"</code> 即可直接把 Bern 当作 GPUI 的声明式 UI 框架。</sub></p>

## 核心特色

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/why-dark.svg">
<img alt="核心特色：编译成真正的原生界面，用 Rust 生成 SwiftUI、WinUI 3、GTK 4、Compose、GPUI 与 Web 代码，默认无 WebView，且随时可用 bern eject 导出配好 Rust FFI 的独立原生工程；全 Rust 开发、逻辑与界面分离，协议定义状态与命令、引擎纯 Rust 可独立单测、漏翻译与漏入口编译期报错并自动生成 CLI 与 SKILL.md；轻到不像跨平台，macOS 内存 39 MB、0.4 秒出窗口、应用包 2.7 MB，Windows 35 MB、0.3 秒、4.2 MB，Web 189 KB" src="Docs/Assets/tables/why-light.svg" width="100%">
</picture>

## 用 Rust 怎么编写软件

在 Bern 里，从数据模型到各平台安装包全程只写 Rust，不需要手写 Swift、C++ 或 Kotlin 胶水代码（只有在确实要调用某个平台特有控件时，才可选用 `NativeView` 局部嵌入）。为了避开 Rust 写 GUI 最常见的痛点——回调与借用检查冲突、异步 IO 和界面线程纠缠、多端工程配置繁琐——每个 Bern 应用都固定拆成三层：

```mermaid
flowchart LR
    subgraph app["你的应用（纯 Rust）"]
        P["协议<br/>类型 · 状态 · 命令"]
        U["界面声明<br/>#[component] + ui!"]
        E["引擎<br/>普通 Rust"]
    end
    P --> C{{"bern build<br/>编译器 + IR"}}
    U --> C
    C --> S["SwiftUI<br/>macOS · iOS · watchOS · tvOS"]
    C --> W["WinUI 3 / Islands<br/>Windows"]
    C --> G["GTK 4<br/>Linux"]
    C --> A["Jetpack Compose<br/>Android · Wear OS"]
    C --> P2["GPUI<br/>macOS · Windows · Linux"]
    C --> B["DOM + WASM<br/>Web"]
    E <--> K[["命令目录"]]
    K <--> N["原生界面"]
    K <--> L["应用 CLI"]
    K <--> JS["JS · bern.cli"]
    K <--> J["JSON 通道"]
    K --> SK["SKILL.md"]
```

1. **协议（Protocol）**：用 `#[bern::protocol]` 声明数据结构、只读状态快照，以及命令和查询 trait。
2. **引擎（Engine）**：用最普通的 Rust 实现这些 trait。引擎看不到任何窗口、控件或传输层，你可以照常使用 `tokio`、`reqwest`、`rusqlite` 等任意 Rust crate，并用 `cargo test` 直接做单元测试。
3. **界面（Interface）**：在 `.rs` 文件中编写带 `#[component]` 与 `ui!` 的组件函数。界面只读取状态和派发类型化命令；编译器在构建期把声明读入共享 IR，生成各平台自己的原生 UI 代码，并直接链接到你的 Rust 引擎。

### 项目结构

运行 `bern new tasks` 会生成一个标准 Cargo 工作区，根目录的 `Bern.toml` 负责声明语言、主题、随包资源和目标平台：

```text
tasks/
├── Bern.toml           # 应用信息、多语言、资源与目标平台配置
├── tasks-protocol/     # 1. 共享协议：值类型、只读状态与命令签名
├── tasks-core/         # 2. 业务引擎：纯普通 Rust，零 UI 依赖
└── tasks-ui/           # 3. 声明式界面：#[component] + ui! + tr!
```

### 迷你软件示例：待办清单

**1 · 协议（`tasks-protocol/src/lib.rs`）**：值类型、只读状态与类型化命令

```rust
#[bern::protocol(namespace = "tasks", version = 1)]
pub mod tasks {
    use bern::prelude::*;

    #[value]
    pub struct Task {
        pub id: i64,
        #[content]
        pub title: String,
        pub done: bool,
    }

    #[state]
    pub struct State {
        pub items: Vec<Task>,
        pub remaining: i64,
    }

    #[commands]
    pub trait Commands {
        /// 新增一条待办并返回编号。
        #[errors("empty_title")]
        fn add(&mut self, title: String) -> Result<i64, CommandError>;

        /// 切换指定待办的完成状态。
        fn toggle(&mut self, id: i64) -> Result<(), CommandError>;
    }
}
```

**2 · 引擎（`tasks-core/src/lib.rs`）**：普通 Rust 业务逻辑，完全不碰 UI

```rust
use bern::CommandError;
use tasks_protocol::tasks::{self, State, Task};

pub struct TasksEngine {
    state: State,
    next_id: i64,
}

impl Default for TasksEngine {
    fn default() -> Self {
        Self { state: State { items: Vec::new(), remaining: 0 }, next_id: 0 }
    }
}

impl tasks::Commands for TasksEngine {
    fn add(&mut self, title: String) -> Result<i64, CommandError> {
        if title.trim().is_empty() {
            return Err(CommandError::new("empty_title", "title cannot be empty"));
        }
        self.next_id += 1;
        self.state.items.push(Task { id: self.next_id, title, done: false });
        self.state.remaining += 1;
        Ok(self.next_id)
    }

    fn toggle(&mut self, id: i64) -> Result<(), CommandError> {
        if let Some(task) = self.state.items.iter_mut().find(|t| t.id == id) {
            task.done = !task.done;
            self.state.remaining += if task.done { -1 } else { 1 };
        }
        Ok(())
    }
}

impl tasks::Engine for TasksEngine {
    fn state(&self) -> &State { &self.state }
}
```

**3 · 界面（`tasks-ui/src/lib.rs`）**：在 Rust 里用 `ui!` 声明界面、局部草稿与多语言文案

```rust
#[bern::session]
pub struct Draft {
    pub title: String,
    pub only_open: bool,
}

#[component]
pub fn tasks_view(app: tasks::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Form {
            Section(title: tr! {
                "zh-Hans" => "待办（剩余 {n} 项）",
                "en" => "Tasks ({n} left)";
                n: app.remaining
            }) {
                Row(gap: 8) {
                    TextInput(
                        id: new_task,
                        label: tr! { "zh-Hans" => "新任务", "en" => "New task" },
                        value: draft.title,
                        prompt: tr! { "zh-Hans" => "写点要做的事…", "en" => "Add a task…" },
                        submit: clear
                    ) => app.add(draft.title)
                    Button(
                        id: add,
                        label: tr! { "zh-Hans" => "添加", "en" => "Add" },
                        prominence: prominent,
                        clears: draft.title
                    ) => app.add(draft.title)
                }
                Switch(
                    id: only_open,
                    label: tr! { "zh-Hans" => "只看未完成", "en" => "Only open" },
                    value: draft.only_open
                )
                ForEach(items: app.items, key: id) |task| {
                    if !draft.only_open || !task.done {
                        Switch(
                            id: done,
                            label: task.title,
                            checked: task.done,
                            style: checkbox
                        ) => app.toggle(task.id)
                    }
                }
            }
        }
    }
}
```

### 检查、运行与打包闭环

同一个 `bern` 工具贯通编译期检查、多端运行、发布打包，并自动产出应用自己的 CLI：

```sh
bern check                          # 编译期校验类型、翻译完整性与布局入口覆盖
bern run                            # 直接运行本机原生界面（SwiftUI / WinUI 3 / GTK 4）
bern run --target web_app           # 或同一份代码切到 Android、iOS、GPUI、Web
bern package                        # 打出各端 release 包（.dmg / 安装器 / .deb / .apk / .ipa / Web）
bern eject --target macos           # 随时导出已配好 Rust FFI 的独立原生 IDE 工程

# 因为每个界面操作都绑定了协议命令，编译出来的应用天然附带 CLI：
tasks ui inspect
tasks tasks add --title "发布 v1.0"
```

更多完整的多页应用见仓库示例：[控件展示应用](examples/showcase)（全部标准控件、引擎异步搜索、列表到详情导航、视频以及带关闭确认的设置窗口）、[计划应用](examples/planner)（模块化应用、样式覆盖、弹簧动效与 `Canvas`）、[图案集](examples/gallery)（海报墙、比例时间轴、四档自适应布局与文件拖入）、[原生控件示例](examples/native-controls)（六个后端紧邻嵌入 `NativeView`）、[手表端示例](examples/wearos)（手表目标）与[动画 SVG](examples/svg-animation)（纯 Rust 矢量动画）。

## 为 AI 而建

正因为界面在编译期受严格约束、所有交互又全部经由命令目录派发，Bern 天生让 AI 既写不错界面，也能直接操控运行中的应用。

### AI 写代码：漏掉的，编译器替它记着

AI 写界面最常见的问题不是写错，而是写漏：加了一个功能却忘了在窄屏布局放入口，新增一段文案只写了中文，随手挂一个回调绕过了命令。这些在别的框架里要等到用户发现，在 Bern 里都过不了编译。约束越严，AI 越不容易交出半成品：

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/ai-dark.svg">
<img alt="AI 漏掉的五类问题与 Bern 的处理：少写一种语言报 B102，某个布局缺功能入口报 B201，入口没有登记到功能报 B202，入口只在条件分支里报 B203，给按钮挂任意代码则根本写不出来" src="Docs/Assets/tables/ai-light.svg" width="100%">
</picture>

报错是给人看的，也是给 Agent 看的：编号、原因、位置都很明确，Agent 能自己改完再编译。下面是展示应用的两次真实输出，第一次故意删掉一个英文翻译，补上之后，第二次删掉窄屏布局里的“换片”菜单：

```text
$ bern check
error[B102]: Missing translation
Missing language: en
Required languages: zh-Hans, en
  --> showcase-ui/src/pages.rs:13:31

$ bern check
error[B201]: Feature missing in adaptive layout
Contract: MediaFeature
Feature: Clip
compact: missing
  --> showcase-ui/src/media.rs:14:5
```

每次构建还会为每个目标输出完整的功能覆盖报告（`features.json`），哪些已覆盖、哪些豁免、哪些只有人工保证，都分开列出。

### AI 用应用：读语义，不读像素

开启控制通道后，Bern 从应用的命令目录生成 `SKILL.md`。以下面的开关为例：Agent 在 `skills/showcase/SKILL.md` 中查到 `ui set-bool` 的用法，再运行 `showcase ui inspect` 找到当前窗口里开关的名称。把这个名称传给应用 CLI，就能像用户点击一样切换开关，不需要截图或坐标。

```sh
$ showcase ui inspect                 # Agent 读到的是语义，不是像素
[{ "target": "controls_page.sync", "role": "switch", "label": "同步", "value": "false", "enabled": true, … }, …]

$ showcase ui set-bool --target controls_page.sync --value true
$ showcase window screenshot --output shot.png

$ showcase window close --window settings#1
{ "outcome": "ConfirmationRequired", "message": "要存储对显示名称的更改吗？",
  "decisions": ["Save", "Discard", "Cancel"], "confirmation": "settings#1:c1", … }

$ showcase window confirm-close --window settings#1 --confirmation settings#1:c1 --decision Save
{ "outcome": "Rejected", "reason": "save_failed: invalid_name: a display name has 1 to 40 characters …" }
```

你也可以运行同样的命令，包括 `showcase showcase set-display-name --name Demo` 这样的业务命令，或用内置的 `window screenshot` 直接截取窗口画面。请使用与应用同一次构建生成的 CLI；远端操作时，用 SSH 登录控制通道所在的机器，以拥有该端点的账户运行。控件被禁用、被对话框挡住或不存在时，CLI 会返回明确的错误码。

## 实测数据

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/measured-dark.svg">
<img alt="实测数据：macOS（SwiftUI）启动 0.41 秒、内存 39 MB、应用包 2.7 MB；Windows（WinUI 3）0.32 秒、35 MB、应用目录 4.2 MB；Web 全部资源 gzip 后 189 KB" src="Docs/Assets/tables/measured-light.svg" width="100%">
</picture>

<details>
<summary>测量方法</summary>
<br>

- 对象：release 构建的展示应用（6 个页面、主窗口与设置窗口、视频播放器），2026-10-11 测量，每项至少 3 次取中位数。
- macOS：Apple M4、16 GB 内存、macOS 27.0.1。内存为 `footprint` 的物理占用，即活动监视器“内存”一栏的口径。
- Windows：Intel Core i5-13490F、32 GB 内存、Windows 11 25H2。内存为私有工作集，即任务管理器“内存”一栏的口径。
- 启动时间：从启动进程到命令行读到可见的原生窗口为止，包含引擎启动与本地控制通道就绪。
- Windows 表中为 WinUI 3 前端（测量时系统中有 Windows App Runtime 1.8，现在按 2.x 构建）。没有运行时的系统上，同一个程序回退到系统 XAML：启动 0.54 秒，内存 19 MB。两种情况都不需要 .NET 与 VC++ 运行库，应用目录只有可执行文件和 400 KB 的引导 DLL。

</details>

## 谁使用了 Bern

<table>
<tr>
<td width="33%" valign="top">
<a href="https://github.com/AimesSoft/RustSend"><img src="Docs/Assets/projects/rustsend.png" width="56" height="56" alt="RustSend"></a>
<h3><a href="https://github.com/AimesSoft/RustSend">RustSend</a></h3>
<p>兼容 LocalSend 的局域网文件传输工具，支持文件、文件夹与文字互传。</p>
</td>
<td width="33%" valign="top">
<a href="https://github.com/zhaohuaxs/lambda"><img src="Docs/Assets/projects/lambda.png" width="56" height="56" alt="Lambda"></a>
<h3><a href="https://github.com/zhaohuaxs/lambda">Lambda</a></h3>
<p>内置 7-Zip 的 Windows / Android 压缩包管理器，支持加密压缩、解压与内容预览。</p>
</td>
<td width="33%" valign="top">
<a href="https://github.com/MCDFsteve/Enji"><img src="Docs/Assets/projects/enji.png" width="56" height="56" alt="Enji Music"></a>
<h3><a href="https://github.com/MCDFsteve/Enji">Enji Music</a></h3>
<p>网易云音乐第三方客户端，支持在线与本地音乐、同步歌词和歌单管理。</p>
</td>
</tr>
</table>

## 更多能力与扩展

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/more-dark.svg">
<img alt="更多能力：每个操作都是命令、编译期就做检查、业务逻辑留在 Rust、从第一天起就本地化、深浅色零配置、声明式多窗口" src="Docs/Assets/tables/more-light.svg" width="100%">
</picture>

<details>
<summary><b>还有：</b>完整标准控件、默认骨架图、样式与动效、媒体与文件、系统集成、模块化应用、<code>NativeView</code> 与可选 JS…</summary>
<br>

- **自适应布局与导航**：`compact`、`medium`、`expanded`、`large` 四档尺寸与 `watch`、`tv` 布局；类型化推入导航、`SplitView` 侧边栏、`Tabs` 与 `TabAccessory`、工具栏、菜单、`Grid`（含瀑布流）、`Stack`、`SplitPane`、`Accordion` 与编译期功能契约。
- **完整原生控件体系**：`Table`、`TreeView`、`Reorderable`、`BottomSheet`、`Refreshable`、`DatePicker`、`TimePicker`、`NumberInput`、`ColorPicker`、`ShortcutInput`、`Tooltip`、`Banner`、`Disclosure`、`Alert`、`EmptyState`、`Badge`、`Avatar`、`Text(selectable:)` 与行内 Markdown、动态可搜索 `Picker(style: combobox)`、多文件 `pick:` 与原生 `save:` 对话框。
- **默认骨架图、样式与动效**：`Query` 与 `skeleton: <bool>` 容器在编译期直接从真实界面树投影骨架图（120 ms 防闪烁、错峰轻浮入场、自动遵循减少动态效果）；`style!` 全局覆盖、弹簧动效与共享元素 Hero、纯 Rust 动画 `Svg`（零 WebView）、HDR `Image`、`Canvas` 自绘与比例轨道 `Rail`。
- **媒体、异步查询与文件**：`Video` 接入各平台原生媒体栈或应用自带 `MediaKernel`，支持直读 `players.<id>.<field>`；`Query` 订阅管理加载、空态、失败与刷新；`Bern.toml` 随包资源；跨本地路径 / Android `content://` / Web `blob:` 统一的 `MediaSource`、系统打开方式 `[[app.opens]]` 与内存供图 `bern::files::serve`。
- **桌面与系统集成**：系统托盘 `StatusItem`（单色 SVG 图标与登录时启动）、macOS `MenuBar`、浮层锚定工具窗、内置 `window.screenshot`、指针事件（`DoubleTap`、`ContextMenu`、`WheelUp`/`WheelDown`）、`Slider(hover:)` 与内嵌 `WebPage`。
- **扩展能力**：模块化应用（`bern new --module`）、紧邻嵌入各平台原生代码的 `NativeView`（含强类型 props、`tr!` 检查、强制 CLI 入口与 `Fallback`，见[紧邻原生视图](Docs/zh-CN/Topics/Native-Views.md)），以及按需启用的进程内 JavaScript 与 `bern.cli` 黑白名单（见[JavaScript 与目标能力](Docs/zh-CN/Topics/JavaScript.md)）。

</details>

### 导出独立原生工程：无锁定与 Rust FFI 模板

因为 Bern 在构建期生成的本来就是各平台自己的原生 UI 代码，你永远不会被框架锁定。随时运行 `bern eject`，即可在不改动原 Bern 项目的前提下，导出按组件与窗口拆好文件、清除 `Do not edit` 标记的独立标准 IDE 工程，直接交由原生流程继续演进。

它同时也是一款**开箱即用的“原生界面调用 Rust”工程模板生成器**：即便你从第一天起就打算手写 SwiftUI、Kotlin Compose、WinUI 3 C++、GTK 或 Web 界面，用 `bern new` + `bern eject` 也能立刻拿到已搭好 Rust 核心、C ABI / FFI 桥接、状态快照绑定与原生 IDE 构建脚本的全平台起步模板——省去手配 Xcode Run Script、Android NDK/JNI、Visual Studio MSBuild 或 WASM 桥接的繁琐工作。

```sh
bern eject --target macos --out ./native-macos                   # 离线 Rust 后端源码 + Xcode 工程
bern eject --target ios --engine prebuilt --release --out ./ios  # 预编译 XCFramework，无需安装 Rust

# 导出的 macOS 工程示例（Android Gradle / Visual Studio .sln / Cargo / Web ES 模块结构类似）：
# native-macos/
# ├── MyApp.xcodeproj/           # 已联动 Build Rust Engine 阶段（或直接嵌入 XCFramework）
# ├── Sources/{App,Models,Styles,Views,Windows,Native,Runtime}/
# └── rust/                      # 离线自包含的 Rust 工作区 + C FFI 桥接层
```

| 平台 | 独立原生工程入口 | 已配好的 Rust FFI 交付（`--engine source` / `--engine prebuilt`） |
| --- | --- | --- |
| **macOS / iOS / tvOS / watchOS** | `<Product>.xcodeproj` + `.xcscheme` | `rust/` + Xcode `build-rust.sh` / `BernEngine.xcframework` |
| **Android / Wear OS** | Gradle 工程（`app/build.gradle.kts`） | `rust/` + Gradle NDK 任务 / `jniLibs/<abi>/libbern_app.so` |
| **Windows（WinUI 3 / Islands）** | Visual Studio `.sln` + `.vcxproj` | `rust/` + MSBuild `build-rust.cmd` / `lib\<arch>\bern_app.lib` |
| **Linux（GTK 4）** | 独立 Cargo 工作区 | 自包含 `Cargo.toml`（`--engine source`） |
| **桌面（GPUI）** | 独立 Cargo 工作区 | 自包含 `Cargo.toml`（`--engine source`） |
| **Web（DOM + WASM）** | 零打包器 `index.html` + ES 模块 | `rust/` + `build-wasm.{sh,cmd}` / 预编译 `app.wasm` |

默认 `--engine source`（支持跨宿主系统导出全平台工程）便于继续同步开发 Rust 引擎与原生界面；`--engine prebuilt` 则直接预编译并嵌入引擎二进制，让接手的原生开发者**无需安装 Rust 或 Bern** 即可直接打开构建。详见[导出为独立纯原生项目](Docs/zh-CN/Topics/Eject.md)。

### 用 JavaScript 扩展：进程内 `bern.cli`

未启用脚本的应用不链接任何 JS 运行时。当目标配置 `javascript = true` 后，每个 `bern::script::Context`（原生目标统一进程内 Boa，Web 目标为浏览器 Worker）都会默认注入 `globalThis.bern.cli(...)`，直通应用命令目录，并由 Rust 侧黑白名单控制权限（默认 `CliPolicy::allow_all()`）：

```rust
let js = bern::script::Context::with_policy(
    bern::script::CliPolicy::allow_only(["media.*", "@queries", "state", "catalog"])
        .deny("media.bind")
        .with_label("example.plugin"),
)?;
js.eval(r#"await bern.cli("media seek --player-id main --position-ms 90000")"#).await?;
```

完整的调用形式、规则模式与动态拦截器见[JavaScript 与目标能力](Docs/zh-CN/Topics/JavaScript.md)。

## 支持的平台

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/platforms-dark.svg">
<img alt="支持的平台：macOS、Windows、Linux、Web 应用、WebUI、Android、Wear OS、watchOS、tvOS、iOS 与桌面端 GPUI" src="Docs/Assets/tables/platforms-light.svg" width="100%">
</picture>

平台操作手册：[macOS](Docs/zh-CN/Platforms/macOS.md) · [iOS](Docs/zh-CN/Platforms/iOS.md) · [watchOS](Docs/zh-CN/Platforms/watchOS.md) · [tvOS](Docs/zh-CN/Platforms/tvOS.md) · [Windows](Docs/zh-CN/Platforms/Windows.md) · [Linux](Docs/zh-CN/Platforms/Linux.md) · [Android 与 Wear OS](Docs/zh-CN/Platforms/Android.md) · [GPUI](Docs/zh-CN/Platforms/GPUI.md) · [Web 与 WebUI](Docs/zh-CN/Platforms/Web.md)。

不指定 UI 技术时使用平台的默认技术；默认技术不可用时直接报错，不会悄悄换成另一套技术。

### 把 Bern 当作 GPUI 的 UI 框架

GPUI 提供了高性能 GPU 渲染，但本身不带标准控件库。在 macOS、Windows 或 Linux 目标里写上 `ui = "gpui"`，就可以直接把 Bern 当作 GPUI 的声明式 UI 框架使用：一份 `ui!` 开箱自带全套桌面控件、语义深浅色主题、弹簧动效、编译期多语言与功能契约检查，以及 Skills + CLI。需要自定义 GPU 绘制的地方，用 `NativeView(gpui: "...")` 紧邻嵌入原生 GPUI 代码即可；未来想切回系统原生控件，或扩展到 iOS、Android、watchOS、Wear OS 与 Web，业务与声明都无需重写。

### Windows 与 iOS 各有两个前端

这两个平台的系统跨度大，一个 UI 框架覆盖不全，所以同一份声明各生成两个前端。默认两个都编译进同一个包，应用启动时按系统自动选择，用户和应用代码都不用管。

| 平台 | 前端 | `frontends` 里的名字 | 运行在 |
| --- | --- | --- | --- |
| Windows | WinUI 3 | `winui3` | 有 Windows App Runtime 2 的系统，Mica 背景与 Windows 11 外观 |
| Windows | 系统 XAML，经 XAML Islands | `xaml-islands` | 其余的 Windows 10 与 11，不需要安装运行时 |
| iOS | SwiftUI | `swiftui` | iOS 17 起，所有 Apple 目标共用这个前端 |
| iOS | SwiftUI 的 iOS 16 版 | `swiftui-16` | iOS 16，由同一份生成代码改写而成 |

只要一个前端时写明：

```toml
[targets.windows]
frontends = ["winui3"]   # 没有运行时的系统上会提示安装

[targets.ios]
frontends = ["swiftui"]  # 最低 iOS 17
```

iOS 两个前端都留着时，写 `packages = "separate"` 可以打成两个 `.ipa`。选择规则与 iOS 16 上的降级项详见 [Windows](Docs/zh-CN/Platforms/Windows.md) 与 [iOS](Docs/zh-CN/Platforms/iOS.md) 平台手册。

## 与其他跨平台框架对比

<p align="center">
<a href="https://github.com/zed-industries/zed/tree/main/crates/gpui"><img alt="GPUI" src="https://img.shields.io/badge/GPUI-084CCF?style=for-the-badge&logo=zedindustries&logoColor=white"></a>
<a href="https://dioxuslabs.com"><img alt="Dioxus" src="https://img.shields.io/badge/Dioxus-4A5568?style=for-the-badge"></a>
<a href="https://flutter.dev"><img alt="Flutter" src="https://img.shields.io/badge/Flutter-02569B?style=for-the-badge&logo=flutter&logoColor=white"></a>
<a href="https://reactnative.dev"><img alt="React Native" src="https://img.shields.io/badge/React%20Native-20232A?style=for-the-badge&logo=react&logoColor=61DAFB"></a>
<a href="https://uniapp.dcloud.net.cn"><img alt="uni-app" src="https://img.shields.io/badge/uni--app-4A5568?style=for-the-badge"></a>
<br>
<a href="https://www.electronjs.org"><img alt="Electron" src="https://img.shields.io/badge/Electron-47848F?style=for-the-badge&logo=electron&logoColor=white"></a>
<a href="https://tauri.app"><img alt="Tauri" src="https://img.shields.io/badge/Tauri-24C8D8?style=for-the-badge&logo=tauri&logoColor=white"></a>
<a href="https://avaloniaui.net"><img alt="Avalonia" src="https://img.shields.io/badge/Avalonia-8B44AC?style=for-the-badge&logo=avaloniaui&logoColor=white"></a>
<a href="https://www.qt.io"><img alt="Qt" src="https://img.shields.io/badge/Qt-41CD52?style=for-the-badge&logo=qt&logoColor=white"></a>
</p>

每个框架都在回答同一个问题：一份代码，怎样变成各平台上的界面。回答不同，取舍也不同。下面依据各项目的官方文档整理；体积和内存随版本与配置变化很大，这里只比较结构上的差别，Bern 自己的数字见[实测数据](#实测数据)。

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/compare/routes-dark.svg">
<img alt="四条技术路线：构建时生成原生界面代码（Bern）、运行时协调原生视图（React Native、uni-app x）、自带渲染引擎（Flutter、GPUI、Avalonia、Qt）、网页技术（Electron、Tauri、Dioxus、uni-app）" src="Docs/Assets/compare/routes-light.svg" width="100%">
</picture>

前两条路线得到的是系统控件：外观、无障碍、输入法、深浅色都由系统负责，代价是各平台控件不完全相同。后两条路线在每个平台上画出一样的界面，代价是要自己追随各平台的设计与行为，或者携带一个浏览器。

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/compare/landscape-dark.svg">
<img alt="界面来源与随应用携带的运行时：Bern 在系统原生控件、携带最少的一角" src="Docs/Assets/compare/landscape-light.svg" width="100%">
</picture>

上图按架构示意，不代表优劣：横轴是界面元素来自系统控件还是框架自绘、网页；纵轴是应用要随身携带多少运行时（浏览器内核、JS 引擎、渲染引擎、语言运行时）。

### 技术路线

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/technology-dark.svg">
<img alt="十个框架的技术路线：写界面与业务逻辑的语言、界面怎么画、随应用携带什么、许可证" src="Docs/Assets/tables/technology-light.svg" width="100%">
</picture>

### 平台与能力

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/capabilities-dark.svg">
<img alt="平台与能力：各框架对 macOS、Windows、Linux、移动端、Web 的支持，以及原生外观、Skills + CLI 与编译期检查；Bern 全平台支持且三项能力都具备" src="Docs/Assets/tables/capabilities-light.svg" width="100%">
</picture>

- **移动端**：iOS / iPadOS 与 Android（并覆盖 watchOS、Wear OS、tvOS 与 Android 电视）。React Native 的桌面版本由微软维护，uni-app 另覆盖小程序与鸿蒙；Bern 覆盖的桌面、手机、平板、手表与电视目标均已在真机实际测试通过，并支持打包 `.ipa` 与 `.apk`。
- **原生外观**：控件就是平台自己的控件，外观、无障碍与输入法由系统提供。Qt Widgets 与 uni-app x 接近原生，记为部分。
- **Skills + CLI**：Bern 从同一份命令目录生成应用 CLI 和 `SKILL.md`，人和 Agent 使用相同的命令。其他框架可以借助无障碍接口或 Playwright、Flutter Driver 等测试工具从外部自动化。
- **编译期检查**：翻译缺失、功能在某个布局没有入口、入口没有登记功能，都由编译器直接报错（见[为 AI 而建](#为-ai-而建)）。其他框架多借助 lint、翻译工具或测试发现这类问题。
- **成熟度**：Electron 与 Qt 非常成熟；Flutter、React Native、uni-app、Tauri 与 Avalonia 成熟；Dioxus 发展中；GPUI 支撑着 Zed 编辑器，通用组件仍在早期；Bern 是早期预览。

### 性能与资源

界面层和逻辑层用什么语言、怎样执行，决定了运行时的开销上限：原生编译且没有垃圾回收的语言最省，JIT 与字节码解释的语言需要携带引擎、占用更多内存。

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/compare/languages-dark.svg">
<img alt="前后端语言与执行方式：Bern 的界面层是各平台原生 UI 代码（Swift、C++、Rust），逻辑层是 Rust，同进程经 C ABI 调用，没有垃圾回收" src="Docs/Assets/compare/languages-light.svg" width="100%">
</picture>

同样一个只显示一行字的窗口，各框架的包体、启动时间与内存差别很大。下图在同一台 CI 虚拟机（GitHub 托管的 Apple M1）上实测：各框架用自己的工具打 release 包，测量时轮流启动，每个 10 次取中位数。脚本在 [benchmarks/](benchmarks/) 中，可以在 Actions 里重新运行 Benchmarks 工作流。

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/compare/resources-dark.svg">
<img alt="Hello World 的资源占用（实测）：Bern 包体 2.1 MB、启动 0.26 秒、内存 18 MB；GPUI 4.2 MB、0.28 秒、27 MB；Electron 301 MB、1.07 秒、77 MB" src="Docs/Assets/compare/resources-light.svg" width="100%">
</picture>

### 怎么选

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/choose-dark.svg">
<img alt="怎么选：Electron、Tauri、Flutter、React Native、Avalonia、Qt、GPUI、Dioxus、uni-app 与 Bern 各自适合的场景和代价" src="Docs/Assets/tables/choose-light.svg" width="100%">
</picture>

## 快速开始

需要 Rust 1.99（`rustup` 按 `rust-toolchain.toml` 自动安装），以及所构建目标的工具链。先安装构建工具 `bern`：

```sh
cargo binstall bern-cli
```

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) 从 [Releases](https://github.com/AimesSoft/Bern/releases) 下载编译好的 `bern`，也可以自己去那里下载；`cargo install bern-cli` 则从源码编译。这样装的 `bern` 创建的应用从 crates.io 取同一版本的 Bern。要修改 Bern 本身，在克隆下来的仓库里用 `cargo install --path crates/bern-cli` 安装 `bern`，它创建的应用从这个克隆取 Bern。

### 创建应用

在任意目录中：

```sh
bern new my_app
cd my_app
bern run
```

新应用是一个带计数的窗口，协议、引擎与界面各一个 crate，六个目标都已在 `Bern.toml` 中声明。`bern run` 构建本机的目标并启动它：macOS 上是 SwiftUI，Windows 上是 WinUI 3，Linux 上是 GTK；`bern run --target web_app` 在浏览器中打开，`bern run --target ios` 装进正在运行的 iOS 模拟器，`bern run --target android` 装到用 USB 调试连接的 Android 手机（或模拟器）上并启动。目标声明 `channels = ["ipc"]` 时，可在 `target/bern/<目标>/bin/` 找到应用 CLI，在同级的 `skills/` 目录找到 `SKILL.md`。

`bern doctor` 列出本机构建每个目标还缺什么、怎么装：

```text
$ bern doctor
bern 0.2.2 on macOS (aarch64)

[✓] Rust: rustc 1.99.0, cargo 1.99.0
[✓] macOS · SwiftUI: macOS SDK 26.5, Swift 6.3.2
[✓] iOS · SwiftUI: iOS simulator SDK 26.5, the aarch64-apple-ios-sim target
[-] Windows · WinUI 3: builds on Windows
[-] Linux · GTK 4: builds on Linux
[✓] Android · Compose: Android SDK, NDK 29.0.14206865, gradle-9.5.1, the aarch64-linux-android target
[✓] Web · WebAssembly: the wasm32-unknown-unknown target
```

<details>
<summary><b>各平台需要的工具</b></summary>
<br>

- **macOS**：Xcode 26 或更新版本。从 App Store 安装后运行 `sudo xcode-select --switch /Applications/Xcode.app`。
- **iOS / iPadOS / tvOS / watchOS**：Xcode 对应的 Apple 平台 SDK 与 Rust 目标（`aarch64-apple-ios` / `aarch64-apple-ios-sim`、`aarch64-apple-tvos` / `aarch64-apple-tvos-sim`、`aarch64-apple-watchos` / `aarch64-apple-watchos-sim`）。`bern run` 可在模拟器调试，`bern package` 生成可供真机安装运行的 `.ipa`。
- **Windows**：Visual Studio 2022 或更新版本的“使用 C++ 的桌面开发”（只装生成工具也可以），含 Windows 11 SDK；Rust 使用 MSVC 工具链。首次构建时用 Windows SDK 的 `cppwinrt` 生成 C++/WinRT 头文件，并把它们与运行时预编译（每个配置约 2.4 GB，放在 `target` 中）；之后的构建只编译应用自己的代码，新项目约 5 秒，展示应用约 9 秒。应用目录只有可执行文件、400 KB 的引导 DLL 和资源，不需要 .NET，也不随附 Windows App SDK。
- **Linux**：GTK 4.14 与 libadwaita 1.5 的开发包，例如 `sudo apt install libgtk-4-dev libadwaita-1-dev libsoup-3.0-dev gstreamer1.0-libav`。
- **Android**：Android Studio（带 SDK 与 NDK，在 SDK Manager 的 SDK Tools 里勾选 NDK）、Gradle 9.4 或更新版本，以及 `rustup target add aarch64-linux-android`。第一次构建时 Gradle 下载 Android 插件与 Compose 库。
- **Web**：`rustup target add wasm32-unknown-unknown`。`web_app` 构建把引擎编译为 WebAssembly 在页面内运行；`web_ui` 构建让页面作为独立引擎宿主的界面，`bern run` 启动宿主并打开它的地址。

</details>

### 发布应用

```sh
bern package
```

按 release 构建本机能构建的目标，在 `target/bern/dist/` 中生成安装包：macOS 的 `.dmg`，Windows 的安装程序（按当前用户安装，不需要管理员权限）与免安装的 `.zip`，Linux 的 `.deb` 与 `.tar.gz`，Android 的 `.apk` 与上架用的 `.aab`，Web 的静态站点压缩包。桌面和 WebUI 安装包带有应用 CLI，开启 IPC 时还带有 Skill。静态 Web 包只含站点文件；Android CLI 在开发电脑上运行，不打进 APK。签名用环境变量配置，证书不进仓库：`BERN_MACOS_SIGNING_IDENTITY` 与 `BERN_MACOS_NOTARY_PROFILE`（签名并公证），`BERN_WINDOWS_CERTIFICATE`（证书指纹或 `.pfx`），`BERN_ANDROID_KEYSTORE`、`BERN_ANDROID_KEYSTORE_PASSWORD` 与 `BERN_ANDROID_KEY_ALIAS`（发布密钥）。没有配置时照样生成，并提示未签名的后果。如果需要导出已配好 Rust FFI 的独立原生 IDE 工程，运行 `bern eject`（见[导出独立原生工程](#导出独立原生工程无锁定与-rust-ffi-模板)）。

### 运行展示应用

Bern 仓库中的展示应用用到全部组件、两个窗口和视频播放器。在仓库目录中：

```sh
cd examples/showcase
bern run
```

### 从外部驱动运行中的应用

```sh
showcase ui inspect                                          # 列出已挂载的控件
showcase ui select --target main_window.pages --option Controls
showcase ui set-bool --target controls_page.sync --value true
showcase showcase search-library --text mo                   # 引擎查询
showcase window open --kind settings
showcase window list                                         # 窗口的实际状态
showcase window screenshot --output shot.png                 # 截取窗口 PNG
ssh HOST /path/to/showcase state
```

目标的 `channels = ["ipc"]` 用于开启外部控制。Android 的 CLI 在开发电脑上运行，连接 `bern run` 通过 `adb forward` 转发的端点，不在手机上执行。独立 Web `app` 开启 IPC 后，`bern run` 会启动本地桥，连接已打开的浏览器标签页；发布的静态站点包不含本机 CLI 和 `SKILL.md`。

## 路线图

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/roadmap-dark.svg">
<img alt="路线图：阶段 1 到 4、阶段 6 标准控件体系与工具链、阶段 7 样式动效与模块化应用均已完成；阶段 5 继续推进其他可选前端" src="Docs/Assets/tables/roadmap-light.svg" width="100%">
</picture>

## 仓库结构

<details>
<summary>crate 与目录</summary>
<br>

<picture>
<source media="(prefers-color-scheme: dark)" srcset="Docs/Assets/tables/repository-dark.svg">
<img alt="仓库结构：核心、运行时、目标模块、入口与工具、示例与文档各自包含的 crate 与目录" src="Docs/Assets/tables/repository-light.svg" width="100%">
</picture>

</details>

## 开发

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

测试中包含必须编译失败的声明，并在装有相应工具包的环境中对生成的应用做类型检查：用 Xcode 检查 Swift 6，用开发包检查 GTK，在 Windows 上用 MSVC 编译生成的两套前端。Android 的 Kotlin 由 `bern build --target android` 经 Gradle 编译。

## 文档

在线浏览可检索文档：**[bern.aimes-soft.com/docs/zh-CN](https://bern.aimes-soft.com/docs/zh-CN)**，或在仓库中直接阅读：

- **开发指南**：[快速上手](Docs/zh-CN/Guides/Quick-Start.md) · [协议与引擎](Docs/zh-CN/Guides/Protocol-and-Engine.md) · [布局与导航](Docs/zh-CN/Guides/Layouts-and-Navigation.md) · [本地化与样式](Docs/zh-CN/Guides/Localization-and-Styling.md) · [窗口、资源与文件](Docs/zh-CN/Guides/Windows-Resources-and-Files.md) · [应用 CLI 与 Skill](Docs/zh-CN/Guides/App-CLI-and-Skills.md)
- **平台手册**：[macOS](Docs/zh-CN/Platforms/macOS.md) · [iOS](Docs/zh-CN/Platforms/iOS.md) · [watchOS](Docs/zh-CN/Platforms/watchOS.md) · [tvOS](Docs/zh-CN/Platforms/tvOS.md) · [Windows](Docs/zh-CN/Platforms/Windows.md) · [Linux](Docs/zh-CN/Platforms/Linux.md) · [Android 与 Wear OS](Docs/zh-CN/Platforms/Android.md) · [GPUI](Docs/zh-CN/Platforms/GPUI.md) · [Web 与 WebUI](Docs/zh-CN/Platforms/Web.md)
- **专题扩展**：[导出独立纯原生项目](Docs/zh-CN/Topics/Eject.md) · [紧邻原生视图](Docs/zh-CN/Topics/Native-Views.md) · [JavaScript 与 `bern.cli`](Docs/zh-CN/Topics/JavaScript.md) · [动画 SVG](Docs/zh-CN/Topics/Animated-SVG.md) · [按模块编译](Docs/zh-CN/Topics/Module-Builds.md) · [应用浏览器扩展](Docs/zh-CN/Topics/Web-Extensions.md)
- **接口参考与架构**：[文档目录](Docs/zh-CN/README.md) · [架构总览](Docs/zh-CN/Architecture.md) · [当前组件用法](Docs/zh-CN/Reference/UI.md) · [完整属性表](Docs/zh-CN/Reference/Component-Properties.md) · [平台能力与验证范围](Docs/zh-CN/Reference/Platforms.md) · [性能追踪](Docs/zh-CN/Reference/Tracing.md)

文档另有[英文版](Docs/README.md)（[在线版](https://bern.aimes-soft.com/docs)）。

## 许可证

[MIT](LICENSE) © 2026 MCDFSteve

<div align="center">
<br>
<sub>Bern 仍处于早期开发阶段，API 与声明语法可能还会变化。</sub>
</div>
