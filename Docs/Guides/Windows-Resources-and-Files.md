**English** · [简体中文](../zh-CN/Guides/Windows-Resources-and-Files.md)

# Multi-Window, Resources & Files

This guide covers declarative windows, system tray and menu bar integration, bundled assets, and cross-platform file access.

## 1. Declarative multi-window (`WindowSet` & `Window`)

Declare all application windows in one `WindowSet` at your root component:

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

- **Native vs. virtual automatically**: desktop targets and iPadOS open native OS windows; iPhone, Android, Web, watchOS, and tvOS automatically present secondary windows as native sheets, full-screen covers, or dialogs while keeping the exact same `windows.<id>.open()` / `close()` / `activate()` commands.
- **Close confirmation protocol**: setting `confirm_close:` and binding a save command (`=> app.save_settings(...)`) wires up native Save / Discard / Cancel prompts — and works identically over the app CLI (`window close` → `ConfirmationRequired` → `window confirm-close`).

## 2. Desktop system integration (`StatusItem` & `MenuBar`)

Inside `WindowSet`, you can also declare desktop system surfaces:

- **`StatusItem`** — system tray / menu bar status icon (monochrome SVG icon + `CommandItem` menu + optional `Switch(login_item: true)` for launch at login on desktop targets).
- **`MenuBar`** — macOS top menu bar integration (merging `CommandMenu(role: file | edit | view | window | help | app | about)` into standard system menus alongside custom menus).
- **Floating utility windows** — anchor overlay windows or hide window chrome (`chrome: unified`, `idle_after:`, `immersive:`).

## 3. Bundled resources (`[resources]`)

Declare static assets (images, sample media, icons, fonts, animated SVGs) relative to `Bern.toml`:

```toml
[resources]
paths = ["images", "media/intro.mp4"]
```

- In **`ui!`**, reference bundled assets by declared relative path: `Image(source: "images/cover.png")`.
- In **your Rust engine**, resolve them via `bern::resources::Resources::host()` without assuming macOS `.app`, Android `assets/`, or Windows directory layouts:

```rust
let source = bern::media::MediaSource::resource(&resources, "media/intro.mp4")?;
```

## 4. App icons (`[app.icon]`)

Provide a single square PNG for all targets, or specify per-platform native icon assets:

```toml
[app.icon]
default = "assets/icon.png"
macos = "assets/AppIcon.icon"        # PNG, .icns, or Icon Composer .icon
ios = "assets/AppIcon.icon"          # PNG, .appiconset, or Icon Composer .icon
tvos = "assets/AppIcon.brandassets"  # PNG (auto-layered) or .brandassets
windows = "assets/app.ico"           # PNG or .ico
linux = "assets/hicolor"             # PNG or hicolor icon theme dir
android = "assets/android-res"       # PNG or Android res dir
web = "assets/icon.svg"              # PNG, SVG, or favicon folder
```

Runtime alternate icons can be declared under `[app.alternate_icons]` and switched via `WindowSet(icon:)`.

## 5. Picking, saving, dropping & opening files

Bern unifies user-selected files across desktop paths, Android SAF `content://` URIs, and Web `blob:` URLs through `MediaSource` and `bern::files`:

- **File / folder picker**: `Button(id: open, label: ..., pick: draft.file, files: "video") => app.open(draft.file)` (use `files: "folder"` for directories, or bind a `Vec<MediaSource>` draft for multi-file selection).
- **Native save dialog**: `Button(id: export, label: ..., save: draft.out, suggested_name: tr! { ... }) => app.export(draft.out)`.
- **Drag and drop**: wrap a surface in `Drop(value: draft.dropped, files: "video") { ... } => app.open(draft.dropped)`.
- **System file associations**: declare `[[app.opens]]` in `Bern.toml` so OS "Open With" routes files to your app:

```toml
[[app.opens]]
files = ["mp4", "mkv"]
menu = { "zh-Hans" = "打开视频", "en" = "Open Video" }
```

- **Reading files in the engine**: use `source.file_name()`, `source.open()`, `bern::files::list(&dir)`, and `bern::files::walk(&dir)` — they transparently handle local paths, Android `content://` trees (via JNI), and Web `blob:` directories.
- **Serving in-memory images**: expose dynamically generated or decrypted images from your Rust engine to UI `Image` views via `bern::files::serve`.
