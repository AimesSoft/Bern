//! Protocol of the planner shell: what belongs to no module.

#[bern::protocol(namespace = "planner", version = 1)]
pub mod planner {
    use bern::prelude::*;

    #[state]
    pub struct State {
        /// Title of the plan, shown above the notes.
        #[content]
        pub title: String,
        /// Work done on each recent day, oldest first.
        pub activity: Vec<i64>,
    }

    #[commands]
    pub trait Commands {
        /// Rename the plan.
        #[idempotent]
        #[errors("empty_title")]
        fn rename(&mut self, title: String) -> Result<(), CommandError>;

        /// Record a day's work; the last twelve days are kept.
        fn record(&mut self, amount: i64) -> Result<(), CommandError>;
    }
}
