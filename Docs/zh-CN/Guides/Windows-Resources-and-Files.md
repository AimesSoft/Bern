[English](../../Guides/Windows-Resources-and-Files.md) · **简体中文**

# 多窗口、随包资源与文件访问

本指南介绍声明式多窗口、托盘与菜单栏集成、随包资源管理以及跨平台文件读写接口。

## 1. 声明式多窗口（`WindowSet` 与 `Window`）

在根组件中用一个 `WindowSet` 统一声明应用的全部窗口：

```rust
#[component]
pub fn app(
    app: my_app::Client,
    windows: AppWindows,
    #[session] main_draft: MainDraft,
    #[session] settings_draft: SettingsDraft,
) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(
                id: main,
                title: tr! { "zh-Hans" => "主窗口", "en" => "Main" },
                initial: open,
                width: 960,
                height: 640,
                min_width: 720,
                min_height: 480
            ) {
                main_view(app: app)
            }
            Window(
                id: settings,
                title: tr! { "zh-Hans" => "设置", "en" => "Settings" },
                role: settings,
                width: 520,
                confirm_close: settings_draft.dirty,
                close_message: tr! { "zh-Hans" => "要保存更改吗？", "en" => "Save your changes?" }
            ) {
                settings_view(app: app)
            } => app.save_settings(settings_draft.name)
        }
    }
}
```

- **原生与虚拟窗口自动适配**：桌面平台与 iPadOS 打开独立原生系统窗口；iPhone、Android、Web、watchOS 与 tvOS 自动将次级窗口呈现为原生 sheet、全屏页或对话框，同时保留完全一致的 `windows.<id>.open()` / `close()` / `activate()` 命令。
- **关闭确认协议**：声明 `confirm_close:` 并绑定保存命令（`=> app.save_settings(...)`）后，自动接通系统的「存储 / 不存储 / 取消」确认流程，且同样可通过应用 CLI 驱动（`window close` → `ConfirmationRequired` → `window confirm-close`）。

## 2. 桌面系统集成（`StatusItem` 与 `MenuBar`）

在 `WindowSet` 中还可以直接声明桌面系统入口：

- **`StatusItem`** — 系统托盘 / 菜单栏图标（单色 SVG 图标 + `CommandItem` 菜单，支持桌面端 `Switch(login_item: true)` 开机自启开关）。
- **`MenuBar`** — macOS 顶部菜单栏集成（支持 `CommandMenu(role: file | edit | view | window | help | app | about)` 并入系统标准菜单，也可添加自定义菜单）。
- **统一标题栏与沉浸窗口** — 通过 `chrome: unified`、`idle_after:` 与 `immersive:` 控制窗口顶栏外观与静置隐藏行为。

## 3. 随包资源（`[resources]`）

在 `Bern.toml` 中按相对路径显式声明需要随包发布的静态资源（图片、媒体、图标、动画 SVG 等）：

```toml
[resources]
paths = ["images", "media/intro.mp4"]
```

- 在 **`ui!`** 中直接按声明的相对路径引用：`Image(source: "images/cover.png")`。
- 在 **Rust 引擎** 中通过 `bern::resources::Resources::host()` 解析，完全无需关心 macOS `.app`、Android `assets/` 或 Windows 目录结构差异：

```rust
let source = bern::media::MediaSource::resource(&resources, "media/intro.mp4")?;
```

## 4. 应用图标（`[app.icon]`）

既可以只给一张通用方形 PNG，也可以按平台指定原生图标格式：

```toml
[app.icon]
default = "assets/icon.png"
macos = "assets/AppIcon.icon"        # PNG、.icns 或 Icon Composer .icon
ios = "assets/AppIcon.icon"          # PNG、.appiconset 或 Icon Composer .icon
tvos = "assets/AppIcon.brandassets"  # PNG（自动生成分层图与货架图）或 .brandassets
windows = "assets/app.ico"           # PNG 或 .ico
linux = "assets/hicolor"             # PNG 或 hicolor 图标主题目录
android = "assets/android-res"       # PNG 或 Android res 目录
web = "assets/icon.svg"              # PNG、SVG 或网页图标目录
```

如需运行时动态切换备用图标，可在 `[app.alternate_icons]` 声明并绑定 `WindowSet(icon:)`。

## 5. 文件选择、保存、拖放与系统打开方式

Bern 用 `MediaSource` 与 `bern::files` 统一封装桌面本地路径、Android SAF `content://` URI 与 Web `blob:` 地址：

- **文件 / 文件夹选择**：`Button(id: open, label: ..., pick: draft.file, files: "video") => app.open(draft.file)`（选目录写 `files: "folder"`；绑定 `Vec<MediaSource>` 草稿即支持多选）。
- **原生保存对话框**：`Button(id: export, label: ..., save: draft.out, suggested_name: tr! { ... }) => app.export(draft.out)`。
- **拖放接收**：用 `Drop(value: draft.dropped, files: "video") { ... } => app.open(draft.dropped)` 包裹区域。
- **系统关联打开**：在 `Bern.toml` 声明 `[[app.opens]]`，让系统「打开方式」把文件交给应用：

```toml
[[app.opens]]
files = ["mp4", "mkv"]
menu = { "zh-Hans" = "打开视频", "en" = "Open Video" }
```

- **在引擎中读取**：直接调用 `source.file_name()`、`source.open()`、`bern::files::list(&dir)` 与 `bern::files::walk(&dir)`，框架自动处理本地路径、Android `content://` 文档树（经 JNI）与 Web `blob:` 目录。
- **内存供图**：Rust 引擎动态生成或解密的图片可通过 `bern::files::serve` 直接供 `Image` 组件展示。
