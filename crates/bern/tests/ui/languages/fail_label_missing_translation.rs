// Checked against `[languages] supported = ["en", "ja"]`.

#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Mode {
        #[label(tr! { "en" => "Fast", "ja" => "速い" })]
        Fast,
        #[label(tr! { "en" => "Slow" })]
        Slow,
    }

    #[state]
    pub struct State {
        pub mode: Mode,
    }

    #[commands]
    pub trait Commands {
        fn switch(&mut self, mode: Mode) -> Result<(), CommandError>;
    }
}

fn main() {}
