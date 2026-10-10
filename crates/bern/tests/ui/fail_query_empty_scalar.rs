#[bern::protocol(namespace = "library", version = 1)]
pub mod library {
    use bern::prelude::*;

    #[value]
    pub struct Book {
        pub id: i64,
        pub title: String,
    }

    #[state]
    pub struct State {
        pub books: Vec<Book>,
    }

    #[commands]
    pub trait Commands {
        fn add(&mut self, title: String) -> Result<i64, CommandError>;
    }

    #[queries]
    pub trait Queries {
        fn search(&self, text: String, answer: Answer<Vec<Book>>);
        fn count(&self, answer: Answer<i64>);
    }
}

#[bern::session]
pub struct Draft {
    pub text: String,
    pub on: bool,
}

use bern::prelude::*;

#[component]
fn page(app: library::Client) -> impl View {
    ui! { Query(id: total, query: app.count(), empty: tr! { "en" => "None" }) |count| { Text(content: count) } }
}

fn main() {}
