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
    pub hover: Option<f64>,
}

/// Media players owned by the application.
#[bern::players]
pub struct Players {
    pub preview: Player,
}

use bern::prelude::*;

#[component]
fn app(app: demo::Client, windows: AppWindows, players: Players, #[session] draft: Draft) -> impl View {
    ui! {
        WindowSet(handle: AppWindows, primary: main) {
            Window(
                id: main,
                title: tr! { "en" => "Main" },
                initial: open,
                confirm_close: draft.name != app.saved,
                close_message: tr! { "en" => "Save {name}?"; name: draft.name }
            ) {
                page(app: app, players: players)
            } => app.save(draft.name)
        }
    }
}

#[component]
fn page(app: demo::Client, players: Players, #[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Video(id: clip, source: app.clip, fit: contain)
            Video(id: preview, player: players.preview, controls: none, label: tr! { "en" => "Preview" }) {
                DoubleTap => players.preview.pause()
                ContextMenu {
                    MenuItem(id: replay, label: tr! { "en" => "Replay" }) => players.preview.seek(0)
                }
                WheelUp => players.preview.set_volume(1.0)
                WheelDown => players.preview.set_volume(0.0)
            }
            Slider(id: seek, label: tr! { "en" => "Position" }, current: players.preview.progress, hover: draft.hover, min: 0, max: 1) => players.preview.play()
            if players.preview.playing {
                Text(content: tr! { "en" => "{at} of {length}"; at: players.preview.position_ms, length: players.preview.duration })
            }
            if let Some(clip) = app.clip {
                Button(id: open, label: tr! { "en" => "Open" }) => players.preview.open(clip)
            }
            Button(id: play, label: tr! { "en" => "Play" }) => players.preview.play()
            Button(id: start, label: tr! { "en" => "Start" }) => players.preview.seek(0)
            Button(id: quiet, label: tr! { "en" => "Quiet" }) => players.preview.set_volume(0.2)
            TextInput(id: name, label: tr! { "en" => "Name" }, value: draft.name)
        }
    }
}

fn main() {}
