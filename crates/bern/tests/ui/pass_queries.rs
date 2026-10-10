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
        /// Books whose title contains `text`.
        #[reads(books)]
        #[errors("invalid_search")]
        fn search(&self, text: String, limit: i64, answer: Answer<Vec<Book>>);

        /// The book with `id`, if any.
        #[reads(books)]
        fn book(&self, id: i64, answer: Answer<Option<Book>>);

        /// Number of books.
        fn count(&self, answer: Answer<i64>);
    }
}

#[bern::session]
pub struct Draft {
    pub text: String,
}

use bern::prelude::*;
use library::{Book, State};

#[component]
fn page(app: library::Client, #[session] draft: Draft) -> impl View {
    ui! {
        Column {
            Query(
                id: results,
                query: app.search(draft.text, 20),
                loading: tr! { "en" => "Searching…" },
                empty: tr! { "en" => "No books match {text}"; text: draft.text },
                failed: tr! { "en" => "Search failed" },
                icon: "magnifyingglass"
            ) |books| {
                ForEach(items: books, key: id) |book| {
                    Text(content: book.title)
                }
            }
            Query(id: first, query: app.book(1), empty: tr! { "en" => "No first book" }) |book| {
                Text(content: book.title)
            }
            Query(id: total, query: app.count()) |count| {
                Text(content: tr! { "en" => "{count} books"; count: count })
            }
        }
    }
}

struct Engine {
    state: State,
    clock: bern::query::ManualClock,
}

impl library::Commands for Engine {
    fn add(&mut self, title: String) -> Result<i64, CommandError> {
        let id = self.state.books.len() as i64 + 1;
        self.state.books.push(Book { id, title });
        Ok(id)
    }
}

impl library::Queries for Engine {
    fn search(&self, text: String, limit: i64, answer: Answer<Vec<Book>>) {
        if text.starts_with('!') {
            return answer.fail(CommandError::new("invalid_search", "no operators"));
        }
        let found: Vec<Book> = self
            .state
            .books
            .iter()
            .filter(|book| book.title.contains(&text))
            .take(limit as usize)
            .cloned()
            .collect();
        // Answer later, from the clock.
        self.clock.after(100, move || answer.ok(found));
    }

    fn book(&self, id: i64, answer: Answer<Option<Book>>) {
        answer.ok(self.state.books.iter().find(|book| book.id == id).cloned());
    }

    fn count(&self, answer: Answer<i64>) {
        answer.ok(self.state.books.len() as i64);
    }
}

impl library::Engine for Engine {
    fn state(&self) -> &State {
        &self.state
    }
}

fn main() {
    use bern::{CallContext, Entry, Host, Invocation, Value};
    use bern::bind::BernValue;

    let _ = page;
    let clock = bern::query::ManualClock::new();
    let host = Host::start(library::Adapter(Engine {
        state: State { books: vec![] },
        clock: clock.clone(),
    }));
    let context = || CallContext {
        entry: Entry::Test,
        request_id: "1".into(),
    };
    let index = |id: &str| host.resolve(id).unwrap();
    host.dispatch_blocking(
        Invocation {
            command: index("library.add"),
            params: vec![Value::Text("Rust in Action".into())],
        },
        context(),
    );
    assert!(host.is_query(index("library.search")));
    let (sender, receiver) = std::sync::mpsc::channel();
    host.dispatch(
        Invocation {
            command: index("library.search"),
            params: vec![Value::Text("Rust".into()), Value::Int(5)],
        },
        context(),
        move |outcome| sender.send(outcome.result).unwrap(),
    );
    // The answer waits for the clock.
    while clock.pending() == 0 {
        std::thread::yield_now();
    }
    assert!(receiver.try_recv().is_err());
    clock.advance(100);
    let books = Vec::<Book>::from_value(&receiver.recv().unwrap().unwrap()).unwrap();
    assert_eq!(books[0].title, "Rust in Action");
    let outcome = host.dispatch_blocking(
        Invocation {
            command: index("library.search"),
            params: vec![Value::Text("!x".into()), Value::Int(5)],
        },
        context(),
    );
    assert_eq!(outcome.result.unwrap_err().code, "invalid_search");
    let outcome = host.dispatch_blocking(
        Invocation {
            command: index("library.book"),
            params: vec![Value::Int(2)],
        },
        context(),
    );
    assert_eq!(outcome.result, Ok(Value::Optional(None)));
    let spec = library::descriptor().command("library.search").unwrap().1;
    assert_eq!(spec.reads, vec!["books"]);
    assert_eq!(spec.doc, "Books whose title contains `text`.");
}
