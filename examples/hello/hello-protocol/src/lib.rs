//! Protocol of the hello world: no state to speak of, one command.

#[bern::protocol(namespace = "hello", version = 1)]
pub mod hello {
    use bern::prelude::*;

    #[state]
    pub struct State {
        /// How many times the greeting was waved.
        pub waves: i64,
    }

    #[commands]
    pub trait Commands {
        /// Wave back.
        fn wave(&mut self) -> Result<(), CommandError>;
    }
}
