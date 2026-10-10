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
fn page(players: Players, #[session] draft: Draft) -> impl View {
    ui! { Button(id: seek, label: tr! { "en" => "Seek" }) => players.preview.seek(draft.name) }
}

fn main() {}
