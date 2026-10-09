//! Platform-independent descriptions for Bern's first WebUI slice.
//!
//! UI descriptions contain data and command references, never application
//! callbacks. The application engine can run without any UI dependency.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Engine,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Text,
    Integer,
    Boolean,
}

impl ValueKind {
    pub fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Text => value.as_str().is_some_and(|text| text.len() <= 4096),
            Self::Integer => value.as_i64().is_some_and(|n| i32::try_from(n).is_ok()),
            Self::Boolean => value.is_boolean(),
        }
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for String {}
    impl Sealed for i32 {}
    impl Sealed for bool {}
}

pub trait FieldType: sealed::Sealed + Serialize {
    const KIND: ValueKind;
}
impl FieldType for String {
    const KIND: ValueKind = ValueKind::Text;
}
impl FieldType for i32 {
    const KIND: ValueKind = ValueKind::Integer;
}
impl FieldType for bool {
    const KIND: ValueKind = ValueKind::Boolean;
}

/// A typed, read-only reference. Mutation belongs to an engine or session command.
#[derive(Debug)]
pub struct Field<T: FieldType> {
    id: &'static str,
    scope: Scope,
    marker: PhantomData<fn() -> T>,
}
impl<T: FieldType> Copy for Field<T> {}
impl<T: FieldType> Clone for Field<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: FieldType> Field<T> {
    pub const fn engine(id: &'static str) -> Self {
        Self {
            id,
            scope: Scope::Engine,
            marker: PhantomData,
        }
    }
    pub const fn session(id: &'static str) -> Self {
        Self {
            id,
            scope: Scope::Session,
            marker: PhantomData,
        }
    }
    pub fn reference(self) -> FieldRef {
        FieldRef {
            id: self.id.into(),
            scope: self.scope,
            kind: T::KIND,
        }
    }
    pub fn describe(self, initial: T) -> FieldSpec {
        FieldSpec {
            id: self.id.into(),
            scope: self.scope,
            kind: T::KIND,
            initial: serde_json::to_value(initial).expect("sealed field types serialize"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRef {
    pub id: String,
    pub scope: Scope,
    pub kind: ValueKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSpec {
    pub id: String,
    pub scope: Scope,
    pub kind: ValueKind,
    pub initial: Value,
}

/// The first slice supports parameterless engine commands only.
#[derive(Clone, Copy, Debug)]
pub struct Command {
    id: &'static str,
}
impl Command {
    pub const fn new(id: &'static str) -> Self {
        Self { id }
    }
    pub const fn id(self) -> &'static str {
        self.id
    }
    pub fn describe(self, label: impl Into<String>) -> CommandSpec {
        CommandSpec {
            id: self.id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextValue {
    Literal { value: String },
    Binding { field: FieldRef },
}
impl From<&str> for TextValue {
    fn from(value: &str) -> Self {
        Self::Literal {
            value: value.into(),
        }
    }
}
impl From<String> for TextValue {
    fn from(value: String) -> Self {
        Self::Literal { value }
    }
}
impl<T: FieldType> From<Field<T>> for TextValue {
    fn from(field: Field<T>) -> Self {
        Self::Binding {
            field: field.reference(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Column {
        id: String,
        gap: u16,
        padding: u16,
        children: Vec<Node>,
    },
    Row {
        id: String,
        gap: u16,
        padding: u16,
        children: Vec<Node>,
    },
    Text {
        id: String,
        content: TextValue,
    },
    Button {
        id: String,
        label: String,
        command: String,
    },
    TextInput {
        id: String,
        label: String,
        field: FieldRef,
    },
    Checkbox {
        id: String,
        label: String,
        field: FieldRef,
    },
}

impl Node {
    pub fn column(id: impl Into<String>, gap: u16, padding: u16, children: Vec<Node>) -> Self {
        Self::Column {
            id: id.into(),
            gap,
            padding,
            children,
        }
    }
    pub fn row(id: impl Into<String>, gap: u16, padding: u16, children: Vec<Node>) -> Self {
        Self::Row {
            id: id.into(),
            gap,
            padding,
            children,
        }
    }
    pub fn text(id: impl Into<String>, content: impl Into<TextValue>) -> Self {
        Self::Text {
            id: id.into(),
            content: content.into(),
        }
    }
    pub fn button(id: impl Into<String>, label: impl Into<String>, command: Command) -> Self {
        Self::Button {
            id: id.into(),
            label: label.into(),
            command: command.id.into(),
        }
    }
    pub fn text_input(
        id: impl Into<String>,
        label: impl Into<String>,
        field: Field<String>,
    ) -> Self {
        Self::TextInput {
            id: id.into(),
            label: label.into(),
            field: field.reference(),
        }
    }
    pub fn checkbox(id: impl Into<String>, label: impl Into<String>, field: Field<bool>) -> Self {
        Self::Checkbox {
            id: id.into(),
            label: label.into(),
            field: field.reference(),
        }
    }
    pub fn id(&self) -> &str {
        match self {
            Self::Column { id, .. }
            | Self::Row { id, .. }
            | Self::Text { id, .. }
            | Self::Button { id, .. }
            | Self::TextInput { id, .. }
            | Self::Checkbox { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub schema_version: u32,
    pub title: String,
    pub root: Node,
    pub fields: Vec<FieldSpec>,
    pub commands: Vec<CommandSpec>,
}

impl App {
    pub fn new(
        title: impl Into<String>,
        root: Node,
        fields: Vec<FieldSpec>,
        commands: Vec<CommandSpec>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            title: title.into(),
            root,
            fields,
            commands,
        }
    }

    /// Export-time semantic checks supplement the macro's syntax/type checks.
    pub fn validate(&self) -> Result<(), CommandError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(invalid("unsupported UI schema"));
        }
        if self.title.trim().is_empty() {
            return Err(invalid("application title is required"));
        }
        let mut fields = BTreeMap::new();
        for field in &self.fields {
            check_id(&field.id)?;
            // IDs are unique even across scopes, keeping diagnostics unambiguous.
            if fields.insert(field.id.as_str(), field).is_some() {
                return Err(invalid(format!("duplicate field: {}", field.id)));
            }
            if !field.kind.accepts(&field.initial) {
                return Err(invalid(format!("invalid initial value: {}", field.id)));
            }
        }
        let mut commands = BTreeSet::new();
        for command in &self.commands {
            check_id(&command.id)?;
            if !commands.insert(command.id.as_str()) {
                return Err(invalid(format!("duplicate command: {}", command.id)));
            }
            if command.label.trim().is_empty() {
                return Err(invalid("command label is required"));
            }
        }
        validate_node(&self.root, &fields, &commands, &mut BTreeSet::new(), 0)
    }
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("invalid_ui", message)
}

fn check_id(id: &str) -> Result<(), CommandError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(invalid(format!("invalid identifier: {id}")));
    }
    Ok(())
}

fn validate_node<'a>(
    node: &'a Node,
    fields: &BTreeMap<&str, &FieldSpec>,
    commands: &BTreeSet<&str>,
    ids: &mut BTreeSet<&'a str>,
    depth: usize,
) -> Result<(), CommandError> {
    if depth > 64 {
        return Err(invalid("component nesting exceeds 64"));
    }
    check_id(node.id())?;
    if !ids.insert(node.id()) {
        return Err(invalid(format!("duplicate component id: {}", node.id())));
    }
    let check_field =
        |reference: &FieldRef, expected: Option<ValueKind>| -> Result<(), CommandError> {
            let spec = fields
                .get(reference.id.as_str())
                .ok_or_else(|| invalid(format!("unknown field: {}", reference.id)))?;
            if spec.scope != reference.scope || spec.kind != reference.kind {
                return Err(invalid(format!(
                    "field type/scope mismatch: {}",
                    reference.id
                )));
            }
            if let Some(kind) = expected
                && (reference.kind != kind || reference.scope != Scope::Session)
            {
                return Err(invalid(format!(
                    "editable field must be a {kind:?} session field: {}",
                    reference.id
                )));
            }
            Ok(())
        };
    match node {
        Node::Column {
            gap,
            padding,
            children,
            ..
        }
        | Node::Row {
            gap,
            padding,
            children,
            ..
        } => {
            if *gap > 128 || *padding > 128 {
                return Err(invalid("spacing must be in 0..=128"));
            }
            for child in children {
                validate_node(child, fields, commands, ids, depth + 1)?;
            }
        }
        Node::Text {
            content: TextValue::Binding { field },
            ..
        } => check_field(field, None)?,
        Node::Text { .. } => {}
        Node::Button { label, command, .. } => {
            if label.trim().is_empty() {
                return Err(invalid("button label is required"));
            }
            if !commands.contains(command.as_str()) {
                return Err(invalid(format!("unregistered command: {command}")));
            }
        }
        Node::TextInput { label, field, .. } => {
            if label.trim().is_empty() {
                return Err(invalid("input label is required"));
            }
            check_field(field, Some(ValueKind::Text))?;
        }
        Node::Checkbox { label, field, .. } => {
            if label.trim().is_empty() {
                return Err(invalid("checkbox label is required"));
            }
            check_field(field, Some(ValueKind::Boolean))?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}
impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for CommandError {}

/// Trusted business implementation. Hosts validate the request before dispatch.
/// An error must leave business state unchanged in this first synchronous slice.
pub trait Engine: Send + 'static {
    fn commands(&self) -> Vec<CommandSpec>;
    fn snapshot(&self) -> BTreeMap<String, Value>;
    fn dispatch(&mut self, command: &str) -> Result<(), CommandError>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub engine_id: String,
    pub command: String,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub engine_id: String,
    /// Decimal string so JavaScript does not truncate a 64-bit revision.
    pub revision: String,
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    Welcome {
        protocol_version: u32,
        engine_id: String,
        commands: Vec<CommandSpec>,
        snapshot: Snapshot,
    },
    Snapshot {
        snapshot: Snapshot,
    },
    Result {
        request_id: String,
        engine_id: String,
        revision: String,
        ok: bool,
        error: Option<CommandError>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    const COUNT: Field<i32> = Field::engine("count");
    const NAME: Field<String> = Field::session("name");
    const INC: Command = Command::new("counter.increment");
    fn app() -> App {
        App::new(
            "基础控件",
            Node::column(
                "root",
                12,
                24,
                vec![
                    Node::text("count", COUNT),
                    Node::text_input("name", "名字", NAME),
                    Node::button("increment", "增加", INC),
                ],
            ),
            vec![COUNT.describe(0), NAME.describe(String::new())],
            vec![INC.describe("增加")],
        )
    }
    #[test]
    fn portable_description_round_trips() {
        let app = app();
        app.validate().unwrap();
        let restored: App = serde_json::from_str(&serde_json::to_string(&app).unwrap()).unwrap();
        restored.validate().unwrap();
    }
    #[test]
    fn unregistered_commands_are_rejected() {
        let mut app = app();
        app.commands.clear();
        assert!(
            app.validate()
                .unwrap_err()
                .message
                .contains("unregistered command")
        );
    }
    #[test]
    fn editable_engine_state_is_not_a_session_draft() {
        let field = Field::<String>::engine("engine_name");
        let app = App::new(
            "Bad",
            Node::text_input("input", "名称", field),
            vec![field.describe(String::new())],
            vec![],
        );
        assert!(
            app.validate()
                .unwrap_err()
                .message
                .contains("session field")
        );
    }
    #[test]
    fn duplicate_node_ids_are_rejected() {
        let app = App::new(
            "Bad",
            Node::row("same", 0, 0, vec![Node::text("same", "重复")]),
            vec![],
            vec![],
        );
        assert!(
            app.validate()
                .unwrap_err()
                .message
                .contains("duplicate component")
        );
    }
    #[test]
    fn field_type_cannot_drift_in_serialized_description() {
        let mut app = app();
        app.fields[0].kind = ValueKind::Boolean;
        app.fields[0].initial = Value::Bool(false);
        assert!(app.validate().unwrap_err().message.contains("mismatch"));
    }
    #[test]
    fn wire_requests_reject_hidden_fields() {
        let request = r#"{"protocol_version":1,"request_id":"1","engine_id":"a","command":"x","params":{},"admin":true}"#;
        assert!(serde_json::from_str::<CommandRequest>(request).is_err());
    }
}
