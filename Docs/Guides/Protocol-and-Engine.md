**English** · [简体中文](../zh-CN/Guides/Protocol-and-Engine.md)

# Protocol & Plain Rust Engine

In Bern, business logic never imports UI widgets, window handles, or platform SDKs. Instead, a typed **protocol** crate defines the data contract, and a plain Rust **engine** crate implements it.

## 1. Declaring the protocol (`#[bern::protocol]`)

A protocol module declares four things: reusable value types (`#[value]`), the read-only state snapshot (`#[state]`), state-mutating commands (`#[commands]`), and optional read-only async queries (`#[queries]`) or events (`#[events]`).

```rust
#[bern::protocol(namespace = "tasks", version = 1)]
pub mod tasks {
    use bern::prelude::*;

    #[value]
    pub enum Priority {
        #[label(tr! { "zh-Hans" => "普通", "en" => "Normal" })]
        Normal,
        #[label(tr! { "zh-Hans" => "高", "en" => "High" })]
        High,
    }

    #[value]
    pub struct Task {
        pub id: i64,
        #[content]
        pub title: String,
        pub done: bool,
        pub priority: Priority,
    }

    #[state]
    pub struct State {
        pub items: Vec<Task>,
        pub remaining: i64,
    }

    #[commands]
    pub trait Commands {
        /// Add a task and return its id.
        #[errors("empty_title")]
        fn add(&mut self, title: String) -> Result<i64, CommandError>;

        /// Toggle completion of a task.
        #[idempotent]
        fn toggle(&mut self, id: i64) -> Result<(), CommandError>;
    }

    #[queries]
    pub trait Queries {
        /// Return tasks matching `text`.
        #[reads(items)]
        fn search(&self, text: String, answer: Answer<Vec<Task>>);
    }

    #[events]
    pub trait Events {
        fn added(&self, id: i64, title: String);
    }
}
```

Key protocol attributes:

- `#[content]` — marks a `String` field as user/data content so `ui!` can render it directly in `Text` or labels without triggering hardcoded-literal translation errors (`B103`).
- `#[label(tr! { ... })]` — attaches localized display names to enum variants so `Picker`, `Text`, and `Label` can show them directly.
- `#[errors("code_a", "code_b")]` — documents and registers expected `CommandError` codes in the command catalog and generated `SKILL.md`.
- `#[idempotent]` — marks commands that can be safely retried with identical parameters.
- `#[reads(field_a, field_b)]` — tells the runtime which `State` fields a query depends on so active `Query` views refresh automatically whenever those fields change.

## 2. Implementing the engine in plain Rust

Your engine crate depends only on `bern` and your protocol crate. Implement `Commands`, `Engine`, and (if declared) `Queries`:

```rust
use bern::CommandError;
use bern::query::Answer;
use tasks_protocol::tasks::{self, Priority, State, Task};

pub struct TasksEngine {
    state: State,
    next_id: i64,
}

impl Default for TasksEngine {
    fn default() -> Self {
        Self { state: State { items: Vec::new(), remaining: 0 }, next_id: 0 }
    }
}

impl tasks::Commands for TasksEngine {
    fn add(&mut self, title: String) -> Result<i64, CommandError> {
        if title.trim().is_empty() {
            return Err(CommandError::new("empty_title", "title cannot be empty"));
        }
        self.next_id += 1;
        self.state.items.push(Task {
            id: self.next_id,
            title: title.clone(),
            done: false,
            priority: Priority::Normal,
        });
        self.state.remaining += 1;
        tasks::emit::added(self.next_id, title);
        Ok(self.next_id)
    }

    fn toggle(&mut self, id: i64) -> Result<(), CommandError> {
        if let Some(task) = self.state.items.iter_mut().find(|t| t.id == id) {
            task.done = !task.done;
            self.state.remaining += if task.done { -1 } else { 1 };
        }
        Ok(())
    }
}

impl tasks::Queries for TasksEngine {
    fn search(&self, text: String, answer: Answer<Vec<Task>>) {
        let needle = text.to_lowercase();
        let found: Vec<Task> = self.state.items.iter()
            .filter(|t| needle.is_empty() || t.title.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        answer.ok(found);
    }
}

impl tasks::Engine for TasksEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
```

Register the engine type in `Bern.toml`:

```toml
[protocol]
package = "tasks-protocol"
source = "tasks-protocol/src/lib.rs"

[engine]
package = "tasks-core"
type = "tasks_core::TasksEngine"
```

## 3. Asynchronous work and background tasks

Because `Commands` methods execute synchronously on the engine host, keep heavy network or disk I/O off the command path:

- **Async queries**: spawn work on a background thread or async runtime (`tokio`, etc.) and resolve `answer.ok(value)` or `answer.err(CommandError::new(...))` when done. If the UI changes query parameters before the previous request finishes, Bern ignores stale answers and only renders the latest subscription.
- **Background state updates**: engines can hold background workers (for downloads, timers, playback, or sync) and notify the host when internal work finishes, or dispatch internal follow-up commands.
- **Full Rust ecosystem**: use `reqwest`, `rusqlite`, `serde`, `tokio`, or native C/C++ crates directly inside your engine crate — nothing in `*-core` is restricted by `ui!` rules.

## 4. Testing without opening a window

Since the engine has zero UI dependency, you test your entire application logic directly with `cargo test`:

```rust
#[test]
fn adds_and_toggles_tasks() {
    let mut engine = TasksEngine::default();
    let id = tasks::Commands::add(&mut engine, "Write docs".into()).unwrap();
    assert_eq!(engine.state().remaining, 1);

    tasks::Commands::toggle(&mut engine, id).unwrap();
    assert_eq!(engine.state().remaining, 0);
}
```

For full command-catalog and host-level integration tests (including query cancellation, error codes, and serialization), construct a `bern::Host` directly in Rust tests — see [examples/showcase/showcase-core](../../examples/showcase/showcase-core/src/lib.rs).
