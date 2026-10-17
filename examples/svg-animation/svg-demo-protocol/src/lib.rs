#[bern::protocol(namespace = "svg_demo", version = 1)]
pub mod svg_demo {
    use bern::prelude::*;
    #[state]
    pub struct State {
        pub playing: bool,
        pub speed: f64,
        pub revision: i64,
    }
    #[commands]
    pub trait Commands {
        fn toggle(&mut self) -> Result<(), CommandError>;
        fn faster(&mut self) -> Result<(), CommandError>;
        fn restart(&mut self) -> Result<(), CommandError>;
    }
}
