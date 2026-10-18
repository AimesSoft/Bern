// Checked against `[languages] supported = ["en", "ja"]`.

#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub count: i64,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}

use bern::prelude::*;

#[component]
fn page(app: demo::Client) -> impl View {
    ui! { Text(content: tr! { "en" => "Hello" }) }
}

fn main() {}
