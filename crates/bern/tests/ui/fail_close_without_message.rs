#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub title: String,
        pub saved: String,
        pub clip: Option<MediaSource>,
    }

    #[commands]
    pub trait Commands {
        fn save(&mut self, name: String) -> Result<(), CommandError>;
    }
}

#[bern::session]
pub struct Draft {
    pub name: String,
}

/// Media players owned by the application.
#[bern::players]
pub struct Players {
    pub preview: Player,
}

use bern::prelude::*;

#[component]
fn app(app: demo::Client, windows: AppWindows, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(id: main, title: tr! { "en" => "Main" }, initial: open, confirm_close: draft.name != "") {
                Spacer
            } => app.save(draft.name)
        }
    }
}

fn main() {}
