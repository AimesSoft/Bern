use bern_core::{Command, Field, Node, Scope, TextValue};
use bern_macros::{component, ui};

mod fields {
    use super::Field;
    pub const COUNT: Field<i32> = Field::engine("count");
    pub const NAME: Field<String> = Field::session("name");
    pub const ENABLED: Field<bool> = Field::session("enabled");
}

mod commands {
    use super::Command;
    pub const INCREMENT: Command = Command::new("counter.increment");
}

#[component]
fn controls() -> Node {
    ui! {
        Column(id: "root", gap: 16, padding: 24) {
            Text(id: "title", content: "标题")
            Text(id: "count", content: fields::COUNT)
            Row(id: "actions") {
                Button(id: "inc", label: "增加", command: commands::INCREMENT)
            }
            TextInput(id: "name", label: "名字", value: fields::NAME)
            Checkbox(id: "enabled", label: "启用", value: fields::ENABLED)
        }
    }
}

#[test]
fn compiled_controls_preserve_command_and_field_identity() {
    let Node::Column {
        children,
        gap,
        padding,
        ..
    } = controls()
    else {
        panic!("root")
    };
    assert_eq!((gap, padding), (16, 24));
    let Node::Text {
        content: TextValue::Binding { field },
        ..
    } = &children[1]
    else {
        panic!("count")
    };
    assert_eq!(field.id, "count");
    assert_eq!(field.scope, Scope::Engine);
    let Node::Row {
        children: actions,
        gap,
        padding,
        ..
    } = &children[2]
    else {
        panic!("actions")
    };
    assert_eq!((*gap, *padding), (12, 0));
    let Node::Button { command, .. } = &actions[0] else {
        panic!("button")
    };
    assert_eq!(command, "counter.increment");
    let Node::TextInput { field, .. } = &children[3] else {
        panic!("input")
    };
    assert_eq!(field.id, "name");
    assert_eq!(field.scope, Scope::Session);
    let Node::Checkbox { field, .. } = &children[4] else {
        panic!("checkbox")
    };
    assert_eq!(field.id, "enabled");
}

#[test]
fn standalone_ui_macro_accepts_raw_strings_and_absolute_paths() {
    let node = ui! { Text(id: "text", content: r#"literal text"#) };
    assert_eq!(node.id(), "text");
    let node = ui! { Text(id: "field", content: crate::fields::NAME) };
    assert_eq!(node.id(), "field");
}
