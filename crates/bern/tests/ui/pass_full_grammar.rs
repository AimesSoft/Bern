#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => "Fast" })]
        Fast,
        #[label(tr! { "en" => "Slow" })]
        Slow,
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub ready: bool,
        pub title: String,
        pub mode: Mode,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
        fn rename(&mut self, title: String) -> Result<(), CommandError>;
    }
}

#[bern::session]
pub struct Draft {
    pub name: String,
    pub enabled: bool,
    pub level: f64,
    pub mode: demo::Mode,
}

use bern::prelude::*;
use demo::Mode;

#[component]
fn page(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Form {
            Section(title: tr! { "en" => "Count {n}"; n: app.count }) {
                Text(content: app.title)
                Text(content: app.mode, style: headline)
                TextInput(id: name, label: tr! { "en" => "Name" }, value: draft.name) => app.rename(draft.name)
                Switch(id: enabled, label: tr! { "en" => "On" }, value: draft.enabled)
                Slider(id: level, label: tr! { "en" => "Level" }, value: draft.level, min: 0, max: 10) => app.add(3)
                Picker(id: mode, label: tr! { "en" => "Mode" }, value: draft.mode)
                if app.ready && draft.name != "" {
                    Button(id: go, label: tr! { "en" => "Go" }, enabled: app.count < 10) => app.add(1)
                }
                match draft.mode {
                    Mode::Fast => { child(app: app, amount: 2) }
                    Mode::Slow => { child(app: app, amount: app.count) }
                }
            }
        }
    }
}

#[component]
fn child(app: demo::Client, amount: i64) -> impl View {
    ui! {
        Button(id: add, label: tr! { "en" => "Add {amount}"; amount: amount }) => app.add(amount)
    }
}

fn main() {}
