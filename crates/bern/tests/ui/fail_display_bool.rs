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
fn page(app: demo::Client) -> impl View {
    ui! { Text(content: app.ready) }
}

fn main() {}
