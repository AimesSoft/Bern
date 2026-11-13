//! One engine drives watch and phone controls.
#[bern::protocol(namespace = "watch_demo", version = 1)]
pub mod watch_demo {
    use bern::prelude::*;

    #[value]
    pub enum Goal {
        #[label(tr! { "en" => "Daily", "zh-Hans" => "每天" })]
        Daily,
        #[label(tr! { "en" => "Weekly", "zh-Hans" => "每周" })]
        Weekly,
    }

    #[value]
    pub struct Entry {
        pub id: i64,
        pub count: i64,
    }

    #[state]
    pub struct State {
        pub count: i64,
        pub reminders: bool,
        pub level: f64,
        pub goal: Goal,
        pub entries: Vec<Entry>,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self) -> Result<(), CommandError>;
        fn reset(&mut self) -> Result<(), CommandError>;
        fn set_reminders(&mut self, enabled: bool) -> Result<(), CommandError>;
        fn set_level(&mut self, value: f64) -> Result<(), CommandError>;
        fn set_goal(&mut self, goal: Goal) -> Result<(), CommandError>;
    }
}
