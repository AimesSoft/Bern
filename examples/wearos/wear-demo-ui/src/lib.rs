use bern::prelude::*;
use wear_demo_protocol::watch_demo;

#[bern::value]
pub enum Route {
    History,
}

#[bern::session]
pub struct Draft {
    pub path: Vec<Route>,
    pub note: String,
}

#[component]
pub fn app(app: watch_demo::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Counter", "zh-Hans" => "计数" }, initial: open, width: 400, height: 500) {
                Adaptive(id: layout) {
                    Layout(size: watch) { counter(app: app, windows: windows) }
                    Layout(size: compact) { counter(app: app, windows: windows) }
                    Layout(size: expanded) { counter(app: app, windows: windows) }
                }
            }
            Window(id: settings, role: settings, title: tr! { "en" => "Settings", "zh-Hans" => "设置" }, confirm_close: draft.note != "", close_message: tr! { "en" => "Discard your note?", "zh-Hans" => "放弃备注？" }) {
                Form {
                    Switch(id: reminders, label: tr! { "en" => "Reminders", "zh-Hans" => "提醒" }, checked: app.reminders) => app.set_reminders(app.reminders)
                    Slider(id: level, label: tr! { "en" => "Level", "zh-Hans" => "强度" }, current: app.level, min: 0, max: 100, step: 10) => app.set_level(app.level)
                    Picker(id: goal, label: tr! { "en" => "Goal", "zh-Hans" => "目标" }, selected: app.goal) => app.set_goal(app.goal)
                    TextInput(id: note, label: tr! { "en" => "Note", "zh-Hans" => "备注" }, value: draft.note)
                    LanguagePicker(id: language, label: tr! { "en" => "Language", "zh-Hans" => "语言" })
                }
            }
        }
    }
}

#[component]
pub fn counter(app: watch_demo::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        Navigation(id: navigation, path: draft.path) {
            Form {
                Text(content: tr! { "en" => "{count}", "zh-Hans" => "{count}"; count: app.count }, style: large_title)
                Button(id: add, label: tr! { "en" => "Add one", "zh-Hans" => "加一" }, icon: "plus", prominence: prominent, width: fill) => app.add()
                NavLink(id: history, route: Route::History) {
                    Label(title: tr! { "en" => "History", "zh-Hans" => "历史" }, icon: "clock.arrow.circlepath")
                }
                Button(id: settings, label: tr! { "en" => "Settings", "zh-Hans" => "设置" }, icon: "gear") => windows.settings.open()
                Menu(id: actions, label: tr! { "en" => "Actions", "zh-Hans" => "操作" }) {
                    MenuItem(id: reset, label: tr! { "en" => "Reset", "zh-Hans" => "重置" }, role: destructive) => app.reset()
                }
            }
            Destination |route| {
                match route {
                    Route::History => {
                        List(items: app.entries, key: id) |entry| {
                            Text(content: tr! { "en" => "Day {day}: {count}", "zh-Hans" => "第 {day} 天：{count}"; day: entry.id, count: entry.count })
                        }
                    }
                }
            }
        }
    }
}
