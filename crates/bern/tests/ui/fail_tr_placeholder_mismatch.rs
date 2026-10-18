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
    ui! { Text(content: tr! { "en" => "{count} items", "ja" => "{total} 件"; count: app.count, total: app.count }) }
}

fn main() {}
