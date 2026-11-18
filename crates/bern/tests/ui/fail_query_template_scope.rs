use bern::prelude::*;

#[bern::protocol(namespace = "demo", version = 1)]
pub mod demo {
    use bern::prelude::*;
    #[state]
    pub struct State {
        pub loaded: bool,
    }
    #[commands]
    pub trait Commands {
        fn refresh(&mut self) -> Result<(), CommandError>;
    }
    #[queries]
    pub trait Queries {
        fn ready(&self, answer: Answer<bool>);
    }
}

#[component]
fn page(app: demo::Client) -> impl View {
    ui! {
        Query(id: result, query: app.ready(),
            presented: answer) |answer| { Spacer }
    }
}

#[component]
fn header(app: demo::Client) -> impl View {
    ui! {
        Query(id: result, query: app.ready()) |answer| {
            QueryStatusHeader { if answer { Spacer } }
            Spacer
        }
    }
}

fn main() {}
