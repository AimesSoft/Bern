// Checked against `[languages] supported = ["en", "ja"]`.

#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => "Fast", "ja" => "速い" })]
        Fast,
        #[label("en" => "Slow", "ja" => "遅い")]
        Slow,
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub mode: Mode,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}

/// Where a list is sorted.
#[bern::value]
pub enum Order {
    #[label(tr! { "en" => "Newest first", "ja" => "新しい順" })]
    Newest,
    #[label(tr! { "ja" => "古い順", "en" => "Oldest first" })]
    Oldest,
}

use bern::prelude::*;

#[component]
fn page(app: demo::Client) -> impl View {
    ui! {
        Column {
            Text(content: tr! { "en" => "{count} items", "ja" => "{count} 件"; count: app.count })
            Text(content: app.mode)
            Button(id: add, label: tr! { "ja" => "追加", "en" => "Add" }) => app.add(1)
        }
    }
}

fn main() {}
