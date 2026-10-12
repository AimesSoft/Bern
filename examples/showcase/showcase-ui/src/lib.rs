//! UI declarations of the control showcase.
//!
//! Everything here is a restricted, analyzable declaration: `bern build`
//! reads these files to generate native UI, and `rustc` checks every
//! binding and command through the macro expansion.

use bern::prelude::*;
use showcase_protocol::showcase;

mod media;
mod pages;
mod settings;

pub use media::*;
pub use pages::*;
pub use settings::*;

/// Top-level destinations of the main window.
#[bern::value]
pub enum Page {
    #[label(tr! { "zh-Hans" => "标题与文字", "en" => "Text" })]
    Text,
    #[label(tr! { "zh-Hans" => "按钮", "en" => "Buttons" })]
    Buttons,
    #[label(tr! { "zh-Hans" => "开关与滑块", "en" => "Switches & Sliders" })]
    Controls,
    #[label(tr! { "zh-Hans" => "输入框", "en" => "Text Fields" })]
    Inputs,
    #[label(tr! { "zh-Hans" => "图片与布局", "en" => "Images & Layout" })]
    Layout,
    #[label(tr! { "zh-Hans" => "列表与详情", "en" => "List & Detail" })]
    Library,
    #[label(tr! { "zh-Hans" => "视频", "en" => "Video" })]
    Media,
}

/// Segmented filter shown in toolbars and forms.
#[bern::value]
pub enum Filter {
    #[label(tr! { "zh-Hans" => "全部", "en" => "All" })]
    All,
    #[label(tr! { "zh-Hans" => "常用", "en" => "Favorites" })]
    Favorites,
    #[label(tr! { "zh-Hans" => "最近", "en" => "Recent" })]
    Recent,
}

/// Layout density choice.
#[bern::value]
pub enum Density {
    #[label(tr! { "zh-Hans" => "紧凑", "en" => "Compact" })]
    Compact,
    #[label(tr! { "zh-Hans" => "标准", "en" => "Regular" })]
    Regular,
    #[label(tr! { "zh-Hans" => "宽松", "en" => "Spacious" })]
    Spacious,
}

/// Routes pushed inside the library page.
#[bern::value]
pub enum Route {
    Item { id: i64, clip: MediaSource },
}

/// Media players owned by the application.
#[bern::players]
pub struct Players {
    /// Plays library items; keeps playing across pages and windows.
    pub preview: Player,
}

/// Drafts of one main window. Each window instance owns its own copy.
#[bern::session]
pub struct MainDraft {
    pub page: Page,
    pub query: String,
    pub filter: Filter,
    pub path: Vec<Route>,
    pub name: String,
    pub password: String,
    pub notes: String,
    #[initial(true)]
    pub notifications: bool,
    pub sync: bool,
    #[initial(true)]
    pub autosave: bool,
    #[initial(50.0)]
    pub volume: f64,
    #[initial(0.6)]
    pub brightness: f64,
    #[initial(Density::Regular)]
    pub density: Density,
}

/// The application: a main window and a settings window. Each window's
/// close check reads that window's own drafts.
#[component]
pub fn app(
    app: showcase::Client,
    windows: AppWindows,
    players: Players,
    #[session] draft: MainDraft,
    #[session] profile: SettingsDraft,
) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(
                id: main,
                title: tr! { "zh-Hans" => "Bern 控件展示", "en" => "Bern Showcase" },
                initial: open,
                width: 920,
                height: 640,
                min_width: 720,
                min_height: 480,
                confirm_close: draft.notes != "",
                close_message: tr! { "zh-Hans" => "要放弃备注吗？", "en" => "Discard your notes?" }
            ) {
                main_window(app: app, windows: windows, players: players)
            }
            Window(
                id: settings,
                title: tr! { "zh-Hans" => "设置", "en" => "Settings" },
                role: settings,
                width: 520,
                confirm_close: profile.display_name != "" && profile.display_name != app.display_name,
                close_message: tr! { "zh-Hans" => "要存储对显示名称的更改吗？", "en" => "Do you want to save the display name?" }
            ) {
                settings_window(app: app)
            } => app.set_display_name(profile.display_name)
        }
    }
}

/// Sidebar and detail layout of the main window.
#[component]
pub fn main_window(
    app: showcase::Client,
    windows: AppWindows,
    players: Players,
    #[session] draft: MainDraft,
) -> impl View {
    ui! {
        SplitView {
            Sidebar(id: pages, selection: draft.page) {
                Search(
                    id: sidebar_search,
                    value: draft.query,
                    prompt: tr! { "zh-Hans" => "搜索", "en" => "Search" }
                )
                Section(title: tr! { "zh-Hans" => "控件", "en" => "Controls" }) {
                    SidebarItem(value: Page::Text, label: tr! { "zh-Hans" => "标题与文字", "en" => "Text" }, icon: "textformat")
                    SidebarItem(value: Page::Buttons, label: tr! { "zh-Hans" => "按钮", "en" => "Buttons" }, icon: "hand.tap")
                    SidebarItem(value: Page::Controls, label: tr! { "zh-Hans" => "开关与滑块", "en" => "Switches & Sliders" }, icon: "switch.2")
                    SidebarItem(value: Page::Inputs, label: tr! { "zh-Hans" => "输入框", "en" => "Text Fields" }, icon: "character.cursor.ibeam")
                    SidebarItem(value: Page::Layout, label: tr! { "zh-Hans" => "图片与布局", "en" => "Images & Layout" }, icon: "photo")
                }
                Section(title: tr! { "zh-Hans" => "数据", "en" => "Data" }) {
                    SidebarItem(value: Page::Library, label: tr! { "zh-Hans" => "列表与详情", "en" => "List & Detail" }, icon: "list.bullet.rectangle")
                    SidebarItem(value: Page::Media, label: tr! { "zh-Hans" => "视频", "en" => "Video" }, icon: "play.rectangle")
                }
            }
            Detail {
                match draft.page {
                    Page::Text => { text_page(app: app) }
                    Page::Buttons => { buttons_page(app: app, windows: windows) }
                    Page::Controls => { controls_page(app: app) }
                    Page::Inputs => { inputs_page(app: app) }
                    Page::Layout => { layout_page(app: app) }
                    Page::Library => { library_page(app: app, players: players) }
                    Page::Media => { media_page(app: app, players: players) }
                }
            }
        }
    }
}
