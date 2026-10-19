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
fn app(app: demo::Client, windows: AppWindows) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: editor, title: tr! { "en" => "Editor" }, initial: open) {
                Spacer
            }
        }
    }
}

fn main() {}
