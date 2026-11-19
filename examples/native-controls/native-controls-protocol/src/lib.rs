//! Protocol for the native controls showcase.

#[bern::protocol(namespace = "native_controls", version = 1)]
pub mod native_controls {
    use bern::prelude::*;

    #[state]
    pub struct State {
        /// Meter level from 0.0 to 10.0.
        pub level: f64,
        /// Whether boost mode is enabled.
        pub boosted: bool,
        /// Selected ISO-8601 date (`YYYY-MM-DD`).
        #[content]
        pub selected_date: String,
    }

    #[commands]
    pub trait Commands {
        /// Set the meter level (clamped to 0..=10).
        fn set_level(&mut self, level: f64) -> Result<(), CommandError>;
        /// Toggle or set boost mode.
        fn set_boosted(&mut self, boosted: bool) -> Result<(), CommandError>;
        /// Reset level and boost mode to defaults.
        fn reset(&mut self) -> Result<(), CommandError>;
        /// Set the selected date as `YYYY-MM-DD`.
        fn set_date(&mut self, date: String) -> Result<(), CommandError>;
        /// Jump back to the default date.
        fn pick_today(&mut self) -> Result<(), CommandError>;
    }
}
