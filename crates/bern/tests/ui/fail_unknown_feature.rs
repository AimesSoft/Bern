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

#[bern::features]
pub enum PageFeature {
    #[commands(add)]
    Add,
}

#[component(features = PageFeature)]
fn page(app: demo::Client) -> impl View {
    ui! {
        Button(id: add, label: tr! { "en" => "Add" }, feature: PageFeature::Remove) => app.add(1)
    }
}

fn main() {}
