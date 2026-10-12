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
#[component]
fn page(app: demo::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Image(source: "pictures/cover.jpg", label: tr! { "en" => "Cover" }, width: 120, height: 90, fit: cover)
            Image(source: "pictures/mark.png", width: 32)
            Progress(value: app.count, total: 10, label: tr! { "en" => "Done" })
            Progress(value: draft.level)
            Progress
            Grid(min_width: 140, gap: 12) {
                Stack(align: top_trailing) {
                    Image(source: "pictures/cover.jpg", height: 80)
                    Text(content: app.title)
                }
            }
        }
    }
}

fn main() {}
