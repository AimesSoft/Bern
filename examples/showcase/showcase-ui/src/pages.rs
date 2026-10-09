//! Detail pages of the main window.

use crate::{AppWindows, MainDraft, Route};
use bern::prelude::*;
use showcase_protocol::showcase::{self, Action, ItemKind};

/// Text styles, labels and text bound to engine state.
#[component]
pub fn text_page(app: showcase::Client) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "字体层级", "en" => "Text Styles" }) {
                Text(content: tr! { "zh-Hans" => "大标题", "en" => "Large Title" }, style: large_title)
                Text(content: tr! { "zh-Hans" => "标题", "en" => "Title" }, style: title)
                Text(content: tr! { "zh-Hans" => "二级标题", "en" => "Title 2" }, style: title2)
                Text(content: tr! { "zh-Hans" => "三级标题", "en" => "Title 3" }, style: title3)
                Text(content: tr! { "zh-Hans" => "重点文字", "en" => "Headline" }, style: headline)
                Text(content: tr! { "zh-Hans" => "正文用于主要内容，长度不限，会自动换行。", "en" => "Body text carries the main content and wraps as needed." })
                Text(content: tr! { "zh-Hans" => "脚注说明次要信息", "en" => "Footnotes explain secondary details" }, style: footnote, emphasis: secondary)
                Text(content: tr! { "zh-Hans" => "注释", "en" => "Caption" }, style: caption, emphasis: secondary)
            }
            Section(title: tr! { "zh-Hans" => "标签", "en" => "Labels" }) {
                Label(title: tr! { "zh-Hans" => "文稿", "en" => "Documents" }, icon: "doc")
                Label(title: tr! { "zh-Hans" => "照片", "en" => "Photos" }, icon: "photo")
                Label(title: tr! { "zh-Hans" => "下载", "en" => "Downloads" }, icon: "arrow.down.circle")
            }
            Section(
                title: tr! { "zh-Hans" => "来自引擎的文字", "en" => "Text from the Engine" },
                footer: tr! { "zh-Hans" => "这些文字绑定 Rust 引擎的状态，状态变化后自动更新。", "en" => "These lines are bound to the Rust engine state and update automatically." }
            ) {
                Text(content: tr! { "zh-Hans" => "按钮共按下 {count} 次", "en" => "Buttons pressed {count} times"; count: app.activations })
                if let Some(action) = app.last_action {
                    Text(content: tr! { "zh-Hans" => "最近操作：{action}", "en" => "Last action: {action}"; action: action })
                } else {
                    Text(content: tr! { "zh-Hans" => "还没有操作", "en" => "No actions yet" }, emphasis: secondary)
                }
            }
        }
    }
}

/// Every button style, each bound to a registered command.
#[component]
pub fn buttons_page(app: showcase::Client, windows: AppWindows) -> impl View {
    ui! {
        Form {
            Section(
                title: tr! { "zh-Hans" => "按钮样式", "en" => "Button Styles" },
                footer: tr! { "zh-Hans" => "每个按钮都绑定已登记的引擎命令。", "en" => "Every button is bound to a registered engine command." }
            ) {
                Row(gap: 12) {
                    Button(id: primary, label: tr! { "zh-Hans" => "主要", "en" => "Primary" }, prominence: prominent) => app.record(Action::Primary)
                    Button(id: standard, label: tr! { "zh-Hans" => "普通", "en" => "Standard" }) => app.record(Action::Standard)
                    Button(id: plain, label: tr! { "zh-Hans" => "无边框", "en" => "Borderless" }, prominence: plain) => app.record(Action::Plain)
                    Button(id: delete, label: tr! { "zh-Hans" => "删除", "en" => "Delete" }, role: destructive) => app.record(Action::Delete)
                }
            }
            Section(title: tr! { "zh-Hans" => "图标按钮", "en" => "Icon Buttons" }) {
                Row(gap: 16) {
                    IconButton(id: favorite, icon: "star", label: tr! { "zh-Hans" => "收藏", "en" => "Favorite" }) => app.record(Action::Icon)
                    IconButton(id: share, icon: "square.and.arrow.up", label: tr! { "zh-Hans" => "分享", "en" => "Share" }) => app.record(Action::Icon)
                    IconButton(id: trash, icon: "trash", label: tr! { "zh-Hans" => "移到废纸篓", "en" => "Move to Trash" }) => app.record(Action::Icon)
                }
            }
            Section(title: tr! { "zh-Hans" => "状态与窗口", "en" => "States and Windows" }) {
                Button(id: unavailable, label: tr! { "zh-Hans" => "不可用的按钮", "en" => "Unavailable Button" }, enabled: false) => app.record(Action::Standard)
                Button(id: open_settings, label: tr! { "zh-Hans" => "打开设置…", "en" => "Open Settings…" }, icon: "gearshape") => windows.settings.open()
            }
            Section(title: tr! { "zh-Hans" => "最近操作", "en" => "Last Action" }) {
                if let Some(action) = app.last_action {
                    Text(content: action)
                } else {
                    Text(content: tr! { "zh-Hans" => "还没有操作", "en" => "No actions yet" }, emphasis: secondary)
                }
            }
            Toolbar {
                Button(id: toolbar_add, label: tr! { "zh-Hans" => "工具栏按钮", "en" => "Toolbar Button" }, icon: "plus") => app.record(Action::Toolbar)
            }
        }
    }
}

/// Switches, sliders and pickers bound to session drafts.
#[component]
pub fn controls_page(app: showcase::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "开关", "en" => "Switches" }) {
                Switch(id: notifications, label: tr! { "zh-Hans" => "通知", "en" => "Notifications" }, value: draft.notifications)
                Switch(id: sync, label: tr! { "zh-Hans" => "同步", "en" => "Sync" }, value: draft.sync)
                Switch(id: autosave, label: tr! { "zh-Hans" => "自动存储", "en" => "Save automatically" }, value: draft.autosave, style: checkbox)
            }
            Section(
                title: tr! { "zh-Hans" => "滑块", "en" => "Sliders" },
                footer: tr! { "zh-Hans" => "拖动时只更新本窗口的预览，松开后提交到引擎。已提交的音量：{volume}", "en" => "Dragging only updates this window's preview; releasing commits to the engine. Committed volume: {volume}"; volume: app.volume }
            ) {
                Slider(id: volume, label: tr! { "zh-Hans" => "音量", "en" => "Volume" }, value: draft.volume, min: 0, max: 100, step: 1, min_icon: "speaker.fill", max_icon: "speaker.wave.3.fill") => app.set_volume(draft.volume)
                Slider(id: brightness, label: tr! { "zh-Hans" => "亮度", "en" => "Brightness" }, value: draft.brightness, min: 0, max: 1, min_icon: "sun.min", max_icon: "sun.max.fill")
            }
            Section(title: tr! { "zh-Hans" => "选择", "en" => "Choices" }) {
                Picker(id: filter, label: tr! { "zh-Hans" => "显示", "en" => "Show" }, value: draft.filter, style: segmented)
                Picker(id: density, label: tr! { "zh-Hans" => "密度", "en" => "Density" }, value: draft.density, style: menu)
            }
            Toolbar {
                Picker(id: toolbar_filter, label: tr! { "zh-Hans" => "显示", "en" => "Show" }, value: draft.filter, style: segmented)
            }
        }
    }
}

/// Text fields with prompts, secure entry and multi-line notes.
#[component]
pub fn inputs_page(app: showcase::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "文本", "en" => "Text" }) {
                TextInput(id: name, label: tr! { "zh-Hans" => "名字", "en" => "Name" }, value: draft.name, prompt: tr! { "zh-Hans" => "你的名字", "en" => "Your name" }) => app.record(Action::Submit)
                TextInput(id: password, label: tr! { "zh-Hans" => "密码", "en" => "Password" }, value: draft.password, kind: secure, prompt: tr! { "zh-Hans" => "必填", "en" => "Required" })
                TextInput(id: notes, label: tr! { "zh-Hans" => "备注", "en" => "Notes" }, value: draft.notes, kind: multiline, prompt: tr! { "zh-Hans" => "写点什么", "en" => "Write something" })
            }
            Section(title: tr! { "zh-Hans" => "预览", "en" => "Preview" }) {
                if draft.name == "" {
                    Text(content: tr! { "zh-Hans" => "还没有输入名字", "en" => "No name entered yet" }, emphasis: secondary)
                } else {
                    Text(content: tr! { "zh-Hans" => "你好，{name}", "en" => "Hello, {name}"; name: draft.name })
                }
            }
        }
    }
}

/// Keyed engine list with push navigation to a detail page.
#[component]
pub fn library_page(app: showcase::Client, #[session] draft: MainDraft) -> impl View {
    ui! {
        Navigation(path: draft.path) {
            List(items: app.items, key: id) |item| {
                NavLink(id: item_link, route: Route::Item { id: item.id }) {
                    match item.kind {
                        ItemKind::Documents => { Label(title: item.kind, icon: "doc") }
                        ItemKind::Photos => { Label(title: item.kind, icon: "photo") }
                        ItemKind::Music => { Label(title: item.kind, icon: "music.note") }
                        ItemKind::Movies => { Label(title: item.kind, icon: "film") }
                        ItemKind::Downloads => { Label(title: item.kind, icon: "arrow.down.circle") }
                    }
                }
            }
            Toolbar {
                Button(id: add_item, label: tr! { "zh-Hans" => "添加项目", "en" => "Add Item" }, icon: "plus") => app.add_item(ItemKind::Downloads)
            }
            Destination |route| {
                match route {
                    Route::Item { id } => { item_detail(app: app, id: id) }
                }
            }
        }
    }
}

/// Detail page for one library item.
#[component]
pub fn item_detail(app: showcase::Client, id: i64, #[session] draft: MainDraft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "zh-Hans" => "项目", "en" => "Item" }) {
                Text(content: tr! { "zh-Hans" => "项目编号 {id}", "en" => "Item number {id}"; id: id }, style: title2)
                if draft.query != "" {
                    Text(content: tr! { "zh-Hans" => "搜索：{query}", "en" => "Search: {query}"; query: draft.query }, emphasis: secondary)
                }
            }
            Section {
                Button(id: remove_item, label: tr! { "zh-Hans" => "删除此项目", "en" => "Delete This Item" }, role: destructive) => app.remove_item(id)
            }
        }
    }
}
