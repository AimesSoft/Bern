#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[value]
    pub enum Speed {
        #[label(tr! { "en" => "{times}× faster"; times: 2 })]
        Fast,
        #[label(tr! { "en" => "Slow" })]
        Slow,
    }

    #[state]
    pub struct State {
        pub count: i64,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}

fn main() {}
