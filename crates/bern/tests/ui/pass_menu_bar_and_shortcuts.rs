#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub struct Keys {
        pub open: String,
        pub back: String,
    }

    #[state]
    pub struct State {
        pub keys: Keys,
        pub playing: bool,
    }

    #[commands]
    pub trait Commands {
        fn open(&mut self, source: MediaSource) -> Result<(), CommandError>;
        fn back(&mut self) -> Result<(), CommandError>;
        fn set_open_key(&mut self, keys: String) -> Result<(), CommandError>;
    }
}

#[bern::session]
pub struct Draft {
    pub file: MediaSource,
}

use bern::prelude::*;

#[component]
fn app(app: demo::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Main" }, initial: open) {
                page(app: app)
            }
            MenuBar {
                CommandMenu(role: file) {
                    CommandItem(id: open_file, label: tr! { "en" => "Open…" }, shortcut: app.keys.open, pick: draft.file, files: "video") => app.open(draft.file)
                }
                CommandMenu(label: tr! { "en" => "Playback" }) {
                    CommandItem(id: back, label: tr! { "en" => "Back" }, enabled: app.playing, checked: app.playing, shortcut: "cmd+[") => app.back()
                    Divider
                    CommandMenu(label: tr! { "en" => "More" }) {
                        CommandItem(id: more_back, label: tr! { "en" => "Back" }) => app.back()
                    }
                }
            }
        }
    }
}

#[component]
fn page(app: demo::Client) -> impl View {
    ui! {
        Column {
            Shortcut(key: app.keys.back) => app.back()
            ShortcutInput(id: open_key, label: tr! { "en" => "Open" }, current: app.keys.open) => app.set_open_key(app.keys.open)
            if window.menu_bar {
                Text(content: tr! { "en" => "See the menu bar" })
            }
        }
    }
}

fn main() {}
