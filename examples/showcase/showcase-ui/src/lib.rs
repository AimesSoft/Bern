//! UI declarations of the control showcase.
//!
//! Everything here is a restricted, analyzable declaration: `bern build`
//! reads these files to generate native UI, and `rustc` checks every
//! binding and command through the macro expansion.

use bern::prelude::*;
use showcase_protocol::showcase;

mod pages;
mod settings;

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
    #[label(tr! { "zh-Hans" => "列表与详情", "en" => "List & Detail" })]
    Library,
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
    Item { id: i64 },
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

/// The application: a main window and a settings window.
#[component]
pub fn app(app: showcase::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(
                id: main,
                title: tr! { "zh-Hans" => "Bern 控件展示", "en" => "Bern Showcase" },
                initial: open,
                width: 920,
                height: 640,
                min_width: 720,
                min_height: 480
            ) {
                main_window(app: app, windows: windows)
            }
            Window(
                id: settings,
                title: tr! { "zh-Hans" => "设置", "en" => "Settings" },
                role: settings,
                width: 520
            ) {
                settings_window(app: app)
            }
        }
    }
}

/// Sidebar and detail layout of the main window.
#[component]
pub fn main_window(
    app: showcase::Client,
    windows: AppWindows,
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
                }
                Section(title: tr! { "zh-Hans" => "数据", "en" => "Data" }) {
                    SidebarItem(value: Page::Library, label: tr! { "zh-Hans" => "列表与详情", "en" => "List & Detail" }, icon: "list.bullet.rectangle")
                }
            }
            Detail {
                match draft.page {
                    Page::Text => { text_page(app: app) }
                    Page::Buttons => { buttons_page(app: app, windows: windows) }
                    Page::Controls => { controls_page(app: app) }
                    Page::Inputs => { inputs_page(app: app) }
                    Page::Library => { library_page(app: app) }
                }
            }
        }
    }
}
