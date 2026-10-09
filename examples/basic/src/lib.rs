use bern_core::{App, Command, CommandError, CommandSpec, Engine, Field, Node};
use bern_macros::component;
use serde_json::Value;
use std::collections::BTreeMap;

pub mod fields {
    use super::Field;
    pub const COUNT: Field<i32> = Field::engine("counter.value");
    pub const NAME: Field<String> = Field::session("draft.name");
    pub const ENABLED: Field<bool> = Field::session("draft.enabled");
}

pub mod commands {
    use super::Command;
    pub const INCREMENT: Command = Command::new("counter.increment");
    pub const RESET: Command = Command::new("counter.reset");
}

/// All application UI lives in this restricted Rust declaration.
#[component]
pub fn basic_screen() -> Node {
    ui! {
        Column(id: "main", gap: 24, padding: 0) {
            Text(id: "intro", content: "从几个简单控件开始。")
            Column(id: "counter", gap: 16, padding: 24) {
                Text(id: "counter-label", content: "共享计数器")
                Text(id: "counter-value", content: fields::COUNT)
                Row(id: "counter-actions", gap: 12) {
                    Button(id: "increment", label: "增加一次", command: commands::INCREMENT)
                    Button(id: "reset", label: "重置", command: commands::RESET)
                }
            }
            Column(id: "draft", gap: 16, padding: 24) {
                Text(id: "draft-label", content: "这一页的草稿")
                TextInput(id: "name", label: "你的名字", value: fields::NAME)
                Checkbox(id: "enabled", label: "启用提醒", value: fields::ENABLED)
                Text(id: "preview-label", content: "名字预览")
                Text(id: "name-preview", content: fields::NAME)
                Text(id: "reminder-label", content: "提醒状态")
                Text(id: "reminder-preview", content: fields::ENABLED)
            }
        }
    }
}

pub fn catalog() -> Vec<CommandSpec> {
    vec![
        commands::INCREMENT.describe("增加计数"),
        commands::RESET.describe("重置计数"),
    ]
}

pub fn application() -> App {
    App::new(
        "Bern 基础控件",
        basic_screen(),
        vec![
            fields::COUNT.describe(0),
            fields::NAME.describe(String::new()),
            fields::ENABLED.describe(false),
        ],
        catalog(),
    )
}

/// Business logic depends only on the shared command contract, never the browser.
#[derive(Default)]
pub struct CounterEngine {
    count: i32,
}

impl Engine for CounterEngine {
    fn commands(&self) -> Vec<CommandSpec> {
        catalog()
    }
    fn snapshot(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([("counter.value".into(), Value::from(self.count))])
    }
    fn dispatch(&mut self, command: &str) -> Result<(), CommandError> {
        match command {
            "counter.increment" => {
                self.count = self
                    .count
                    .checked_add(1)
                    .ok_or_else(|| CommandError::new("counter_limit", "计数已达到上限"))?;
            }
            "counter.reset" => self.count = 0,
            _ => return Err(CommandError::new("unknown_command", "未知命令")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ui_and_engine_share_the_same_command_catalog() {
        let app = application();
        app.validate().unwrap();
        let mut engine = CounterEngine::default();
        assert_eq!(app.commands, engine.commands());
        engine.dispatch(commands::INCREMENT.id()).unwrap();
        assert_eq!(engine.snapshot()["counter.value"], 1);
        assert!(engine.dispatch("not.registered").is_err());
        assert_eq!(engine.snapshot()["counter.value"], 1);
        engine.dispatch(commands::RESET.id()).unwrap();
        assert_eq!(engine.snapshot()["counter.value"], 0);
    }
}
