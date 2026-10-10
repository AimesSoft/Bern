//! The settings window.

use crate::Density;
use bern::prelude::*;
use showcase_protocol::showcase;

/// Tabs of the settings window.
#[bern::value]
pub enum SettingsTab {
    #[label(tr! { "zh-Hans" => "通用", "en" => "General" })]
    General,
    #[label(tr! { "zh-Hans" => "外观", "en" => "Appearance" })]
    Appearance,
    #[label(tr! { "zh-Hans" => "高级", "en" => "Advanced" })]
    Advanced,
}

/// Drafts of the settings window.
#[bern::session]
pub struct SettingsDraft {
    pub tab: SettingsTab,
    #[initial(true)]
    pub launch_at_login: bool,
    #[initial(true)]
    pub check_updates: bool,
    pub display_name: String,
    #[initial(Density::Regular)]
    pub density: Density,
    #[initial(13.0)]
    pub font_size: f64,
    #[initial(true)]
    pub show_previews: bool,
}

/// Settings organized in tabs, as macOS settings windows are.
#[component]
pub fn settings_window(app: showcase::Client, #[session] settings: SettingsDraft) -> impl View {
    ui! {
        Tabs(id: tabs, selection: settings.tab) {
            Tab(value: SettingsTab::General, label: tr! { "zh-Hans" => "通用", "en" => "General" }, icon: "gearshape") {
                Form {
                    Section {
                        LanguagePicker(id: language, label: tr! { "zh-Hans" => "语言", "en" => "Language" })
                    }
                    Section {
                        Switch(id: launch_at_login, label: tr! { "zh-Hans" => "登录时打开", "en" => "Open at login" }, value: settings.launch_at_login)
                        Switch(id: check_updates, label: tr! { "zh-Hans" => "自动检查更新", "en" => "Check for updates automatically" }, value: settings.check_updates)
                    }
                    Section(footer: tr! { "zh-Hans" => "关闭窗口时，若有未存储的名称，会询问是否存储。", "en" => "When you close the window with an unsaved name, you’re asked whether to save it." }) {
                        TextInput(id: display_name, label: tr! { "zh-Hans" => "显示名称", "en" => "Display name" }, value: settings.display_name, prompt: tr! { "zh-Hans" => "新名称", "en" => "New name" }) => app.set_display_name(settings.display_name)
                        if app.display_name != "" {
                            Text(content: tr! { "zh-Hans" => "已存储的名称：{name}", "en" => "Saved name: {name}"; name: app.display_name }, emphasis: secondary)
                        }
                    }
                }
            }
            Tab(value: SettingsTab::Appearance, label: tr! { "zh-Hans" => "外观", "en" => "Appearance" }, icon: "paintbrush") {
                Form {
                    Section {
                        Picker(id: density, label: tr! { "zh-Hans" => "密度", "en" => "Density" }, value: settings.density)
                        Slider(id: font_size, label: tr! { "zh-Hans" => "文字大小", "en" => "Text size" }, value: settings.font_size, min: 10, max: 24, step: 1, min_icon: "textformat.size.smaller", max_icon: "textformat.size.larger")
                        Switch(id: show_previews, label: tr! { "zh-Hans" => "显示预览", "en" => "Show previews" }, value: settings.show_previews)
                    }
                }
            }
            Tab(value: SettingsTab::Advanced, label: tr! { "zh-Hans" => "高级", "en" => "Advanced" }, icon: "gearshape.2") {
                Form {
                    Section(footer: tr! { "zh-Hans" => "设置窗口与主窗口共享同一个引擎实例。", "en" => "The settings window shares one engine instance with the main window." }) {
                        Text(content: tr! { "zh-Hans" => "已记录 {count} 次操作", "en" => "{count} actions recorded"; count: app.activations })
                        Button(id: reset_volume, label: tr! { "zh-Hans" => "将音量恢复为 50", "en" => "Reset Volume to 50" }) => app.set_volume(50.0)
                    }
                }
            }
        }
    }
}
