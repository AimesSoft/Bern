//! Framework-provided session and window commands.
//!
//! Standard controls are driven through these commands. They are executed
//! by the UI session that owns the target control, so external entries must
//! name a live `session_id`; without one the host answers
//! `requires_ui_session`.

use crate::{
    CommandPolicy, CommandSpec, ExternalNames, FieldDef, OperationKind, ParamSpec, RecordDef,
    Retry, Scope, Type, TypeTable,
};

/// Identifies one framework session command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessionCommand {
    Inspect,
    Activate,
    Edit,
    Submit,
    SetBool,
    SetNumber,
    Select,
    Back,
    WindowOpen,
    WindowClose,
    WindowActivate,
}

impl SessionCommand {
    pub const ALL: [SessionCommand; 11] = [
        SessionCommand::Inspect,
        SessionCommand::Activate,
        SessionCommand::Edit,
        SessionCommand::Submit,
        SessionCommand::SetBool,
        SessionCommand::SetNumber,
        SessionCommand::Select,
        SessionCommand::Back,
        SessionCommand::WindowOpen,
        SessionCommand::WindowClose,
        SessionCommand::WindowActivate,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SessionCommand::Inspect => "ui.inspect",
            SessionCommand::Activate => "ui.activate",
            SessionCommand::Edit => "ui.edit",
            SessionCommand::Submit => "ui.submit",
            SessionCommand::SetBool => "ui.set_bool",
            SessionCommand::SetNumber => "ui.set_number",
            SessionCommand::Select => "ui.select",
            SessionCommand::Back => "ui.back",
            SessionCommand::WindowOpen => "window.open",
            SessionCommand::WindowClose => "window.close",
            SessionCommand::WindowActivate => "window.activate",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }
}

fn param(name: &str, ty: Type, doc: &str) -> ParamSpec {
    ParamSpec {
        name: name.into(),
        ty,
        doc: doc.into(),
    }
}

fn spec(
    command: SessionCommand,
    kind: OperationKind,
    doc: &str,
    params: Vec<ParamSpec>,
    result: Type,
    retry: Retry,
) -> CommandSpec {
    let (namespace, name) = command.id().split_once('.').expect("namespaced id");
    CommandSpec {
        id: command.id().into(),
        name: name.into(),
        doc: doc.into(),
        kind,
        scope: Scope::Session,
        params,
        result,
        errors: vec![
            "requires_ui_session".into(),
            "target_not_found".into(),
            "target_unavailable".into(),
        ],
        policy: CommandPolicy {
            retry,
            ..CommandPolicy::default()
        },
        external: ExternalNames::derive(namespace, name),
    }
}

/// Type table and command specs for the framework session catalog.
pub fn session_catalog() -> (TypeTable, Vec<CommandSpec>) {
    let control = Type::Record {
        name: "ControlInfo".into(),
    };
    let field = |name: &str, ty: Type, doc: &str| FieldDef {
        name: name.into(),
        ty,
        doc: doc.into(),
    };
    let types = TypeTable {
        enums: vec![],
        records: vec![RecordDef {
            name: "ControlInfo".into(),
            fields: vec![
                field("target", Type::Text, "Stable control identity"),
                field("window", Type::Text, "Owning window instance"),
                field("role", Type::Text, "Semantic role"),
                field("label", Type::Text, "Resolved accessible name"),
                field("enabled", Type::Bool, "Whether the control can be operated"),
                field(
                    "value",
                    Type::optional(Type::Text),
                    "Current value, when the control has one",
                ),
            ],
        }],
    };
    let target = || param("target", Type::Text, "Control identity from ui.inspect");
    let commands = vec![
        spec(
            SessionCommand::Inspect,
            OperationKind::Query,
            "List the mounted controls of the session",
            vec![],
            Type::list(control),
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::Activate,
            OperationKind::Command,
            "Activate a button or link through its real control binding",
            vec![target()],
            Type::Unit,
            Retry::Never,
        ),
        spec(
            SessionCommand::Edit,
            OperationKind::Command,
            "Replace the draft text of a text input",
            vec![target(), param("value", Type::Text, "New text")],
            Type::Unit,
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::Submit,
            OperationKind::Command,
            "Submit a text input",
            vec![target()],
            Type::Unit,
            Retry::Never,
        ),
        spec(
            SessionCommand::SetBool,
            OperationKind::Command,
            "Set a switch or checkbox",
            vec![target(), param("value", Type::Bool, "New state")],
            Type::Unit,
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::SetNumber,
            OperationKind::Command,
            "Set and commit a slider",
            vec![target(), param("value", Type::Float, "New value")],
            Type::Unit,
            Retry::Never,
        ),
        spec(
            SessionCommand::Select,
            OperationKind::Command,
            "Select an option of a picker, sidebar or tab view by variant name",
            vec![target(), param("option", Type::Text, "Option name")],
            Type::Unit,
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::Back,
            OperationKind::Command,
            "Go back one step in a navigation container",
            vec![target()],
            Type::Unit,
            Retry::Never,
        ),
        spec(
            SessionCommand::WindowOpen,
            OperationKind::Command,
            "Open a window kind or activate its existing instance",
            vec![param("kind", Type::Text, "Window kind id")],
            Type::Text,
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::WindowClose,
            OperationKind::Command,
            "Request closing a window instance",
            vec![param("window", Type::Text, "Window instance id")],
            Type::Unit,
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::WindowActivate,
            OperationKind::Command,
            "Bring a window instance to the front",
            vec![param("window", Type::Text, "Window instance id")],
            Type::Unit,
            Retry::Idempotent,
        ),
    ];
    (types, commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::check_external_names;

    #[test]
    fn session_catalog_is_consistent() {
        let (types, commands) = session_catalog();
        types.validate().unwrap();
        check_external_names(commands.iter()).unwrap();
        assert_eq!(commands.len(), SessionCommand::ALL.len());
        for command in SessionCommand::ALL {
            assert_eq!(SessionCommand::from_id(command.id()), Some(command));
            assert!(commands.iter().any(|spec| spec.id == command.id()));
        }
    }
}
