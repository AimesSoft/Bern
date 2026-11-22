[English](../../Guides/Protocol-and-Engine.md) · **简体中文**

# 协议与纯 Rust 引擎

在 Bern 中，业务逻辑不引入任何 UI 控件、窗口句柄或平台 SDK。应用通过一份类型化的**协议（Protocol）** crate 定义数据契约，再由纯普通 Rust 编写的**引擎（Engine）** crate 实现业务。

## 1. 声明协议（`#[bern::protocol]`）

协议模块用于集中声明四类契约：可复用值类型（`#[value]`）、只读状态快照（`#[state]`）、修改状态的命令（`#[commands]`），以及可选的只读异步查询（`#[queries]`）与事件（`#[events]`）。

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
        /// 新增一条待办并返回编号。
        #[errors("empty_title")]
        fn add(&mut self, title: String) -> Result<i64, CommandError>;

        /// 切换指定待办的完成状态。
        #[idempotent]
        fn toggle(&mut self, id: i64) -> Result<(), CommandError>;
    }

    #[queries]
    pub trait Queries {
        /// 返回包含关键词 `text` 的待办列表。
        #[reads(items)]
        fn search(&self, text: String, answer: Answer<Vec<Task>>);
    }

    #[events]
    pub trait Events {
        fn added(&self, id: i64, title: String);
    }
}
```

常用协议标注说明：

- `#[content]` — 将 `String` 字段标为用户内容或动态数据，使 `ui!` 可直接用于 `Text` 或控件标签，而不会触发硬编码文案报错（`B103`）。
- `#[label(tr! { ... })]` — 为枚举变体绑定多语言展示文案，使 `Picker`、`Text`、`Label` 等控件可直接展示枚举值。
- `#[errors("code_a", "code_b")]` — 在命令目录与自动生成的 `SKILL.md` 中声明该命令可能返回的 `CommandError` 错误码。
- `#[idempotent]` — 标明相同参数可安全重试的幂等命令。
- `#[reads(field_a, field_b)]` — 声明查询依赖的 `State` 字段；当这些字段发生变化时，所有活跃的 `Query` 视图会自动刷新。

## 2. 用普通 Rust 实现引擎

引擎 crate 只依赖 `bern` 与协议 crate，实现 `Commands`、`Engine` 以及（如有声明的）`Queries` 即可：

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

在 `Bern.toml` 中登记协议与引擎类型：

```toml
[protocol]
package = "tasks-protocol"
source = "tasks-protocol/src/lib.rs"

[engine]
package = "tasks-core"
type = "tasks_core::TasksEngine"
```

## 3. 异步任务与后台工作

`Commands` 方法在引擎宿主上同步执行，因此耗时的网络或磁盘 IO 应放到后台执行：

- **异步查询**：在工作线程或异步运行时（如 `tokio`）中执行查询，完成后调用 `answer.ok(value)` 或 `answer.err(CommandError::new(...))`。如果界面在旧查询返回前发起了新参数查询，Bern 会丢弃过时结果，始终只显示最新一次响应。
- **后台状态更新**：引擎可以持有自己的后台任务（下载、定时同步、播放等），完成工作后通知宿主刷新状态或提交后续命令。
- **完整的 Rust 生态**：在 `*-core` 中可以自由使用 `reqwest`、`rusqlite`、`serde`、`tokio` 或原生 C/C++ 库，完全不受 `ui!` 声明语法限制。

## 4. 不开窗口直接单元测试

因为引擎零 UI 依赖，你可以直接用标准 `cargo test` 覆盖全部业务逻辑：

```rust
#[test]
fn adds_and_toggles_tasks() {
    let mut engine = TasksEngine::default();
    let id = tasks::Commands::add(&mut engine, "写文档".into()).unwrap();
    assert_eq!(engine.state().remaining, 1);

    tasks::Commands::toggle(&mut engine, id).unwrap();
    assert_eq!(engine.state().remaining, 0);
}
```

如果需要测试完整的命令目录、错误码、查询取消与序列化行为，也可以在 Rust 测试中直接实例化 `bern::Host`，参考 [examples/showcase/showcase-core](../../../examples/showcase/showcase-core/src/lib.rs)。
