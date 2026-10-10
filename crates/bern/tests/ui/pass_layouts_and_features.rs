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

/// What the page lets users do.
#[bern::features]
pub enum PageFeature {
    /// Add one.
    #[commands(add)]
    #[layouts(compact, expanded)]
    #[exempt(browser, "not offered in the browser")]
    Add,
}

#[component(features = PageFeature)]
fn page(app: demo::Client) -> impl View {
    ui! {
        Column {
            Adaptive(id: choice) {
                Layout(size: expanded) {
                    Button(id: add, label: tr! { "en" => "Add" }, feature: PageFeature::Add) => app.add(1)
                }
                Layout(size: compact) {
                    Menu(id: more, label: tr! { "en" => "More" }, icon: "ellipsis") {
                        MenuItem(id: add, label: tr! { "en" => "Add" }, feature: PageFeature::Add) => app.add(1)
                        Divider
                    }
                }
            }
            LanguagePicker(id: language, label: tr! { "en" => "Language" })
        }
    }
}

fn main() {
    // A features enum is an ordinary enum.
    let feature = PageFeature::Add;
    assert_eq!(feature, PageFeature::Add);
}
