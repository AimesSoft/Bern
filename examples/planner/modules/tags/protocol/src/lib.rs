//! Protocol of the tags module.

#[bern::protocol(namespace = "tags", version = 1)]
pub mod tags {
    use bern::prelude::*;

    /// One label.
    #[value]
    pub struct Tag {
        /// The label's text, unique among the tags.
        #[content]
        pub name: String,
    }

    #[state]
    pub struct State {
        pub labels: Vec<Tag>,
    }

    #[commands]
    pub trait Commands {
        /// Add a label unless it exists.
        #[idempotent]
        fn tag(&mut self, label: String) -> Result<(), CommandError>;

        /// Tag a new note by the `#words` in it.
        #[on(notes::added)]
        fn label_note(&mut self, id: i64, text: String) -> Result<(), CommandError>;

        /// Add a note saying how many notes there were, through the notes
        /// module.
        fn note_count(&mut self) -> Result<(), CommandError>;
    }
}
