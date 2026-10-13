//! Protocol of the gallery: the patterns, which one is open and which are
//! favorites.

#[bern::protocol(namespace = "gallery", version = 1)]
pub mod gallery {
    use bern::prelude::*;

    /// The accent the windows show.
    #[value]
    pub enum Accent {
        Crimson,
        Ultramarine,
        Neon,
    }

    /// Light, dark, or as the system is.
    #[value]
    pub enum Appearance {
        System,
        Light,
        Dark,
    }

    #[value]
    pub struct Swatch {
        pub id: i64,
        pub name: String,
        pub favorite: bool,
        /// Where the pattern sits along all patterns, from 0 to 1.
        pub at: f64,
    }

    #[state]
    pub struct State {
        pub swatches: Vec<Swatch>,
        /// The favorites, in the order of `swatches`.
        pub favorites: Vec<Swatch>,
        /// The pattern last opened, or 0.
        pub opened: i64,
        /// Name of the pattern last opened, or empty.
        pub opened_name: String,
        /// The latest notice and its number, which changes with each one.
        pub notice: String,
        pub notice_id: i64,
        /// The clip the player page shows.
        pub clip: MediaSource,
        /// The picture the details panel shows, read at run time.
        pub poster: MediaSource,
        pub accent: Accent,
        pub appearance: Appearance,
    }

    #[commands]
    pub trait Commands {
        /// Open a pattern.
        #[errors("unknown_swatch")]
        fn open(&mut self, id: i64) -> Result<(), CommandError>;

        /// Mark a pattern as a favorite, or no longer.
        #[errors("unknown_swatch")]
        fn toggle_favorite(&mut self, id: i64) -> Result<(), CommandError>;

        /// No pattern is a favorite any more.
        fn clear_favorites(&mut self) -> Result<(), CommandError>;

        /// Show `accent` in every window.
        #[idempotent]
        fn set_accent(&mut self, accent: Accent) -> Result<(), CommandError>;

        /// Show every window light, dark, or as the system is.
        #[idempotent]
        fn set_appearance(&mut self, appearance: Appearance) -> Result<(), CommandError>;

        /// Play `source` in the frame.
        fn play(&mut self, source: MediaSource) -> Result<(), CommandError>;

        /// Show `source` as the poster.
        #[idempotent]
        fn set_poster(&mut self, source: MediaSource) -> Result<(), CommandError>;
    }

    #[queries]
    pub trait Queries {
        /// Patterns whose name contains `text`, ignoring case; an empty
        /// text finds none.
        #[reads(swatches)]
        fn find(&self, text: String, answer: Answer<Vec<Swatch>>);
    }
}
