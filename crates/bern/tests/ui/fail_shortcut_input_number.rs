#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub count: i64,
    }

    #[commands]
    pub trait Commands {
        fn set_key(&mut self, keys: String) -> Result<(), CommandError>;
    }
}

use bern::prelude::*;

#[component]
fn page(app: demo::Client) -> impl View {
    ui! { ShortcutInput(id: key, label: tr! { "en" => "Key" }, current: app.count) => app.set_key("x") }
}

fn main() {}
