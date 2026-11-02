//! Protocol of the catalog: thousands of rows, a grid of cards and lines
//! that follow a current one, as lyrics do.

#[bern::protocol(namespace = "catalog", version = 1)]
pub mod catalog {
    use bern::prelude::*;

    /// A row of the list, or a card of the grid.
    #[value]
    pub struct Entry {
        pub id: i64,
        #[content]
        pub title: String,
        #[content]
        pub detail: String,
        /// Its place in the list, from 1.
        pub number: i64,
    }

    /// A line of the lyrics-like list.
    #[value]
    pub struct Line {
        pub key: i64,
        #[content]
        pub text: String,
        pub current: bool,
        /// None for the current line, more the farther a line is from it;
        /// none while the lines are browsed.
        pub blur: f64,
    }

    #[state]
    pub struct State {
        pub entries: Vec<Entry>,
        pub count: i64,
        pub cards: Vec<Entry>,
        /// The row last opened, or empty.
        #[content]
        pub opened: String,
        pub lines: Vec<Line>,
        /// The line to scroll to, each time `scroll_tick` changes.
        pub scroll_to: i64,
        pub scroll_tick: i64,
        /// The lines are scrolled by hand.
        pub browsing: bool,
    }

    #[commands]
    pub trait Commands {
        /// A thousand rows more at the end.
        fn grow(&mut self) -> Result<(), CommandError>;
        /// The first ten rows renamed, so the rows in sight change.
        fn rename(&mut self) -> Result<(), CommandError>;
        /// The first ten rows removed.
        fn trim(&mut self) -> Result<(), CommandError>;
        fn open(&mut self, id: i64) -> Result<(), CommandError>;
        fn remove(&mut self, id: i64) -> Result<(), CommandError>;
        /// The next line becomes the current one.
        fn step(&mut self) -> Result<(), CommandError>;
        fn browse(&mut self, active: bool) -> Result<(), CommandError>;
    }
}
