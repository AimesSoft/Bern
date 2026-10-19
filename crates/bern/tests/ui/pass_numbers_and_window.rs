#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;

    #[state]
    pub struct State {
        pub size: f64,
        pub wide: bool,
    }

    #[commands]
    pub trait Commands {
        fn grow(&mut self) -> Result<(), CommandError>;
    }
}

use bern::prelude::*;

#[component]
fn wall(size: f64, gap: f64, titles: bool) -> impl View {
    ui! {
        Column(width: size, padding: gap) {
            if titles {
                Text(content: tr! { "en" => "Titles" })
            }
        }
    }
}

#[component]
fn page(app: demo::Client) -> impl View {
    ui! {
        // `if` branches mix whole and fractional numbers, and chain.
        Column(padding: if window.short { 0 } else if app.wide { 12.5 } else { 24 }) {
            // A comparison is a component argument as a whole.
            wall(size: app.size, gap: if app.size < 110.0 { 10 } else { 20.5 }, titles: app.size >= 110.0 && !window.short)
            Button(id: grow, label: tr! { "en" => "Grow" }) => app.grow()
        }
    }
}

fn main() {}
