//! Protocol of the notes module.

#[bern::protocol(namespace = "notes", version = 1)]
pub mod notes {
    use bern::prelude::*;

    /// One note.
    #[value]
    pub struct Note {
        /// Stable identity, used as the list key.
        pub id: i64,
        #[content]
        pub text: String,
    }

    #[state]
    pub struct State {
        pub items: Vec<Note>,
        /// How many notes there are.
        pub count: i64,
    }

    #[events]
    pub trait Events {
        /// A note was added.
        fn added(&self, id: i64, text: String);
    }

    #[commands]
    pub trait Commands {
        /// Add a note and return its id.
        #[errors("empty_note")]
        fn add(&mut self, text: String) -> Result<i64, CommandError>;

        /// Remove every note.
        #[idempotent]
        fn clear(&mut self) -> Result<(), CommandError>;
    }
}
