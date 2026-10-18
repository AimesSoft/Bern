#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub count: i64,
        pub title: String,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
        fn rename(&mut self, title: String) -> Result<(), CommandError>;
    }
}

use bern::prelude::*;

#[component]
fn page(app: demo::Client) -> impl View {
    ui! { Button(id: add, label: tr! { "en" => "Add" }, icon: "Plus Circle") => app.add(1) }
}

fn main() {}
