//! Framework-provided session and window commands.
//!
//! Standard controls are driven through these commands. They are executed
//! by the UI session that owns the target control, so external entries must
//! name a live `session_id`; without one the host answers
//! `requires_ui_session`.

use crate::{
    CommandPolicy, CommandSpec, EnumDef, ExternalNames, FieldDef, OperationKind, ParamSpec,
    RecordDef, Retry, Scope, Type, TypeTable, VariantDef,
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
    WindowConfirmClose,
    WindowList,
    WindowMinimize,
    WindowRestore,
    WindowMaximize,
    WindowMove,
    WindowResize,
    Language,
    SetLanguage,
    Focus,
    SetFile,
    TraceStart,
    TraceStop,
    TraceReport,
}

impl SessionCommand {
    /// Frontends address commands by position: new commands go last.
    pub const ALL: [SessionCommand; 25] = [
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
        SessionCommand::WindowConfirmClose,
        SessionCommand::WindowList,
        SessionCommand::WindowMinimize,
        SessionCommand::WindowRestore,
        SessionCommand::WindowMaximize,
        SessionCommand::WindowMove,
        SessionCommand::WindowResize,
        SessionCommand::Language,
        SessionCommand::SetLanguage,
        SessionCommand::Focus,
        SessionCommand::SetFile,
        SessionCommand::TraceStart,
        SessionCommand::TraceStop,
        SessionCommand::TraceReport,
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
            SessionCommand::WindowConfirmClose => "window.confirm_close",
            SessionCommand::WindowList => "window.list",
            SessionCommand::WindowMinimize => "window.minimize",
            SessionCommand::WindowRestore => "window.restore",
            SessionCommand::WindowMaximize => "window.maximize",
            SessionCommand::WindowMove => "window.move",
            SessionCommand::WindowResize => "window.resize",
            SessionCommand::Language => "ui.language",
            SessionCommand::SetLanguage => "ui.set_language",
            SessionCommand::Focus => "ui.focus",
            SessionCommand::SetFile => "ui.set_file",
            SessionCommand::TraceStart => "trace.start",
            SessionCommand::TraceStop => "trace.stop",
            SessionCommand::TraceReport => "trace.report",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }

    /// Commands of the performance trace, which the session runtime
    /// answers itself; the frontend only records.
    pub fn is_trace(self) -> bool {
        matches!(
            self,
            SessionCommand::TraceStart | SessionCommand::TraceStop | SessionCommand::TraceReport
        )
    }

    /// Commands that act on a window's presentation. The target performs
    /// them and the session runtime answers with the window's facts.
    pub fn is_presentation(self) -> bool {
        matches!(
            self,
            SessionCommand::WindowMinimize
                | SessionCommand::WindowRestore
                | SessionCommand::WindowMaximize
                | SessionCommand::WindowMove
                | SessionCommand::WindowResize
        )
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
            "target_blocked".into(),
            "stale_window".into(),
            "stale_confirmation".into(),
        ],
        policy: CommandPolicy {
            retry,
            ..CommandPolicy::default()
        },
        external: ExternalNames::derive(namespace, name),
        reads: vec![],
        on: None,
    }
}

fn with_errors(mut spec: CommandSpec, errors: &[&str]) -> CommandSpec {
    spec.errors
        .extend(errors.iter().map(|error| (*error).to_string()));
    spec
}

fn named(name: &str) -> Type {
    Type::Record { name: name.into() }
}

fn distribution() -> Type {
    named("TraceDistribution")
}

fn segment() -> Type {
    named("TraceSegment")
}

fn trace_record(name: &str, fields: &[(&str, Type, &str)]) -> RecordDef {
    RecordDef {
        name: name.into(),
        fields: fields
            .iter()
            .map(|(name, ty, doc)| FieldDef {
                name: (*name).into(),
                ty: ty.clone(),
                doc: (*doc).into(),
                content: false,
            })
            .collect(),
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
        content: false,
    };
    let unit_enum = |name: &str, variants: &[&str]| EnumDef {
        name: name.into(),
        variants: variants
            .iter()
            .map(|variant| VariantDef {
                name: (*variant).into(),
                fields: vec![],
                label: None,
            })
            .collect(),
    };
    let decision = || Type::Enum {
        name: "CloseDecision".into(),
    };
    let close_result = Type::Record {
        name: "CloseResult".into(),
    };
    let named_enum = |name: &str| Type::Enum { name: name.into() };
    let window_info = Type::Record {
        name: "WindowInfo".into(),
    };
    let types = TypeTable {
        enums: vec![
            unit_enum(
                "CloseOutcome",
                &["Closed", "Rejected", "ConfirmationRequired"],
            ),
            unit_enum("CloseDecision", &["Save", "Discard", "Cancel"]),
            unit_enum("WindowPresentation", &["Native", "Virtual"]),
            unit_enum("WindowLifecycle", &["Opening", "Open", "Closing"]),
            unit_enum("WindowVisibility", &["Visible", "Minimized", "Hidden"]),
            unit_enum("WindowSize", &["Normal", "Maximized", "FullScreen"]),
            unit_enum(
                "WindowAction",
                &[
                    "Activate", "Close", "Minimize", "Restore", "Maximize", "Move", "Resize",
                ],
            ),
            unit_enum("WindowChangeSource", &["Command", "System"]),
        ],
        records: vec![
            RecordDef {
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
                    field(
                        "reason",
                        Type::optional(Type::Text),
                        "Why the control cannot be operated: `disabled` or `blocked_by_modal`",
                    ),
                ],
            },
            RecordDef {
                name: "CloseResult".into(),
                fields: vec![
                    field(
                        "outcome",
                        Type::Enum {
                            name: "CloseOutcome".into(),
                        },
                        "Closed, rejected, or waiting for a confirmation decision",
                    ),
                    field("window", Type::Text, "Window instance"),
                    field(
                        "confirmation",
                        Type::optional(Type::Text),
                        "Confirmation to decide with window.confirm_close",
                    ),
                    field(
                        "draft_revision",
                        Type::Int,
                        "Draft revision the confirmation is bound to",
                    ),
                    field(
                        "decisions",
                        Type::list(decision()),
                        "Decisions the confirmation accepts",
                    ),
                    field(
                        "message",
                        Type::optional(Type::Text),
                        "The declared confirmation message, as shown in the dialog",
                    ),
                    field(
                        "reason",
                        Type::optional(Type::Text),
                        "Why closing was rejected",
                    ),
                ],
            },
            RecordDef {
                name: "WindowFrame".into(),
                fields: vec![
                    field(
                        "x",
                        Type::Float,
                        "Left edge in points from the main display's left edge",
                    ),
                    field(
                        "y",
                        Type::Float,
                        "Top edge in points below the main display's top edge",
                    ),
                    field("width", Type::Float, "Outer width in points"),
                    field("height", Type::Float, "Outer height in points"),
                ],
            },
            trace_record(
                "TraceStarted",
                &[
                    ("rules", Type::Text, "The rules in their normal spelling"),
                    (
                        "file",
                        Type::optional(Type::Text),
                        "Where the timeline goes, when it is written to a file",
                    ),
                ],
            ),
            trace_record(
                "TraceStopped",
                &[
                    (
                        "files",
                        Type::list(Type::Text),
                        "The timeline files written, for ui.perfetto.dev",
                    ),
                    ("events", Type::Int, "Records read"),
                    (
                        "dropped",
                        Type::Int,
                        "Records lost because a ring was full; raise `buffer:`",
                    ),
                    (
                        "timeline",
                        Type::optional(Type::Text),
                        "The timeline itself, when asked for",
                    ),
                ],
            ),
            trace_record(
                "TraceDistribution",
                &[
                    ("count", Type::Int, "Samples in all"),
                    ("p50", Type::Int, "Median of the recent samples"),
                    ("p95", Type::Int, "95th percentile of the recent samples"),
                    ("max", Type::Int, "Largest sample"),
                ],
            ),
            trace_record(
                "TraceCommand",
                &[
                    ("id", Type::Text, "Command id"),
                    ("entry", Type::Text, "ui, cli, json, mcp or test"),
                    ("total_us", distribution(), "From dispatch to on screen, or to the engine's answer"),
                    ("queue_us", distribution(), "Waiting for the engine"),
                    ("exec_us", distribution(), "Running the command"),
                    ("publish_us", distribution(), "Snapshot, comparison and encoding of the state"),
                    ("apply_us", distribution(), "Decoding and writing the state on the UI thread"),
                    ("bytes", distribution(), "State bytes sent to the UI"),
                ],
            ),
            trace_record(
                "TraceQuery",
                &[
                    ("id", Type::Text, "Query id"),
                    ("cancelled", Type::Int, "Answers no one waited for any more"),
                    ("queue_us", distribution(), "Waiting for the engine"),
                    ("provider_us", distribution(), "From the provider's start to its answer"),
                    ("round_trip_us", distribution(), "From the request to the UI showing the answer"),
                ],
            ),
            trace_record(
                "TraceField",
                &[
                    ("name", Type::Text, "State field"),
                    ("sent", Type::Int, "Times it changed, and was sent to a UI if one runs"),
                    ("bytes_max", Type::Int, "Largest encoding sent"),
                    (
                        "items_max",
                        Type::optional(Type::Int),
                        "Most items, for a list",
                    ),
                    ("snapshot_us", distribution(), "Taking it from the engine"),
                    ("compare_us", distribution(), "Comparing it with the last state"),
                    ("encode_us", distribution(), "Encoding it for the UI"),
                    ("apply_us", distribution(), "Decoding and writing it on the UI thread"),
                ],
            ),
            trace_record(
                "TraceSegment",
                &[
                    ("name", Type::Text, "Part of the work"),
                    ("us", Type::Int, "Microseconds"),
                ],
            ),
            trace_record(
                "TraceInteraction",
                &[
                    (
                        "control",
                        Type::optional(Type::Text),
                        "The control operated; absent for commands from other entries",
                    ),
                    ("entry", Type::Text, "Where it came from"),
                    ("commands", Type::list(Type::Text), "Commands it ran"),
                    ("at_ms", Type::Int, "Milliseconds after tracing started"),
                    ("total_us", Type::Int, "From the action to the frame showing its result"),
                    (
                        "revision",
                        Type::optional(Type::Int),
                        "The state revision it produced",
                    ),
                    ("segments", Type::list(segment()), "Where the time went"),
                ],
            ),
            trace_record(
                "TraceFrames",
                &[
                    ("spacing_us", distribution(), "Time between frame ticks"),
                    ("long", Type::Int, "Frames over the long-frame threshold"),
                ],
            ),
            trace_record(
                "TraceFrame",
                &[
                    ("at_ms", Type::Int, "Milliseconds after tracing started"),
                    ("us", Type::Int, "Time since the frame before"),
                    ("refresh_us", Type::Int, "The display's refresh interval, 0 when unknown"),
                    ("bern_us", Type::Int, "Bern's own work on the UI thread"),
                    (
                        "outside_us",
                        Type::Int,
                        "The rest: generated views and the platform's layout and drawing",
                    ),
                    ("segments", Type::list(segment()), "Bern's longest work on the UI thread"),
                    ("engine", Type::list(segment()), "What the engine did meanwhile"),
                ],
            ),
            trace_record(
                "TraceSpan",
                &[
                    ("name", Type::Text, "Span name"),
                    ("us", distribution(), "Durations"),
                ],
            ),
            trace_record(
                "TraceReport",
                &[
                    ("running", Type::Bool, "Whether tracing is still on"),
                    ("window_ms", Type::Int, "Milliseconds the statistics cover"),
                    ("events", Type::Int, "Records read"),
                    ("dropped", Type::Int, "Records lost because a ring was full"),
                    ("commands", Type::list(named("TraceCommand")), "Per command, slowest first"),
                    ("queries", Type::list(named("TraceQuery")), "Per query"),
                    ("fields", Type::list(named("TraceField")), "State fields sent, largest first"),
                    (
                        "interactions",
                        Type::list(named("TraceInteraction")),
                        "The slowest interactions and commands",
                    ),
                    ("frames", named("TraceFrames"), "Frame ticks"),
                    ("long_frames", Type::list(named("TraceFrame")), "The latest long frames"),
                    ("spans", Type::list(named("TraceSpan")), "Spans by name"),
                ],
            ),
            RecordDef {
                name: "WindowInfo".into(),
                fields: vec![
                    field("window", Type::Text, "Window instance"),
                    field("kind", Type::Text, "Window kind id"),
                    field(
                        "presentation",
                        named_enum("WindowPresentation"),
                        "Native system window or virtual window",
                    ),
                    field(
                        "lifecycle",
                        named_enum("WindowLifecycle"),
                        "Opening until the platform reports it; Closing while a close confirmation waits",
                    ),
                    field(
                        "visibility",
                        named_enum("WindowVisibility"),
                        "Visible, minimized or hidden, as the platform reports it",
                    ),
                    field(
                        "size",
                        named_enum("WindowSize"),
                        "Normal, maximized or full screen",
                    ),
                    field(
                        "frame",
                        Type::optional(Type::Record {
                            name: "WindowFrame".into(),
                        }),
                        "Outer frame; absent while opening",
                    ),
                    field(
                        "focused",
                        Type::Bool,
                        "Whether the window has keyboard focus",
                    ),
                    field(
                        "actions",
                        Type::list(named_enum("WindowAction")),
                        "Window actions available now",
                    ),
                    field(
                        "version",
                        Type::Int,
                        "Bumped by every change of this instance's presentation",
                    ),
                    field(
                        "changed_by",
                        Type::optional(named_enum("WindowChangeSource")),
                        "Whether the latest change came from a command or from the user and system",
                    ),
                ],
            },
        ],
    };
    let target = || param("target", Type::Text, "Control identity from ui.inspect");
    let window = || param("window", Type::Text, "Window instance id");
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
            SessionCommand::Focus,
            OperationKind::Command,
            "Give a control the keyboard focus, such as a search field, whose results then show",
            vec![target()],
            Type::Unit,
            Retry::Idempotent,
        ),
        with_errors(
            spec(
                SessionCommand::SetFile,
                OperationKind::Command,
                "Give a file to a control that asks for one or takes dropped files, as if the user chose or dropped it; its action then runs",
                vec![
                    target(),
                    param(
                        "url",
                        Type::Text,
                        "A `file://` or http(s) URL; it must be of a kind the control accepts",
                    ),
                ],
                Type::Unit,
                Retry::Never,
            ),
            &["invalid_url", "unsupported_file"],
        ),
        spec(
            SessionCommand::Language,
            OperationKind::Query,
            "The session's interface language tag",
            vec![],
            Type::Text,
            Retry::Idempotent,
        ),
        with_errors(
            spec(
                SessionCommand::SetLanguage,
                OperationKind::Command,
                "Switch the interface language; every localized text updates while drafts, navigation and windows stay",
                vec![param(
                    "language",
                    Type::Text,
                    "A supported language tag, such as `en`",
                )],
                Type::Unit,
                Retry::Idempotent,
            ),
            &["unsupported_language"],
        ),
        with_errors(
            spec(
                SessionCommand::TraceStart,
                OperationKind::Command,
                "Start the performance trace with rules such as `engine,ui,slow:16ms` or `command:library.*`; a running trace ends first",
                vec![param(
                    "rules",
                    Type::Text,
                    "Categories (engine, ui, frame, span, motion, style, all), filters (command:, query:, control:, field:, feature:), slow:, long:, file:, quiet, buffer:",
                )],
                named("TraceStarted"),
                Retry::Idempotent,
            ),
            &["invalid_rules"],
        ),
        spec(
            SessionCommand::TraceStop,
            OperationKind::Command,
            "Stop the performance trace and write its timeline",
            vec![param(
                "timeline",
                Type::optional(Type::Bool),
                "Also return the timeline itself, for an app whose files are on another device",
            )],
            named("TraceStopped"),
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::TraceReport,
            OperationKind::Query,
            "What the performance trace measured: p50, p95 and maximum per command, the slowest interactions and the long frames",
            vec![
                param(
                    "top",
                    Type::optional(Type::Int),
                    "How many of the slowest interactions to list (10)",
                ),
                param(
                    "reset",
                    Type::optional(Type::Bool),
                    "Start the statistics over after this report",
                ),
            ],
            named("TraceReport"),
            Retry::Idempotent,
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
            "Request closing a window instance; its declared close check may ask for a confirmation",
            vec![window()],
            close_result.clone(),
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::WindowActivate,
            OperationKind::Command,
            "Bring a window instance to the front",
            vec![window()],
            Type::Unit,
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::WindowConfirmClose,
            OperationKind::Command,
            "Decide a pending close confirmation, as its dialog would",
            vec![
                window(),
                param(
                    "confirmation",
                    Type::Text,
                    "Confirmation id from window.close",
                ),
                param("decision", decision(), "Save, Discard or Cancel"),
            ],
            close_result,
            Retry::Never,
        ),
        spec(
            SessionCommand::WindowList,
            OperationKind::Query,
            "List the open window instances with their actual presentation and available actions",
            vec![],
            Type::list(window_info.clone()),
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::WindowMinimize,
            OperationKind::Command,
            "Minimize a window instance; its drafts and navigation stay",
            vec![window()],
            window_info.clone(),
            Retry::Idempotent,
        ),
        spec(
            SessionCommand::WindowRestore,
            OperationKind::Command,
            "Undo one step: bring a minimized window back, or leave full screen or the maximized size",
            vec![window()],
            window_info.clone(),
            Retry::Never,
        ),
        spec(
            SessionCommand::WindowMaximize,
            OperationKind::Command,
            "Grow a window instance to fill its display, as the platform zooms windows",
            vec![window()],
            window_info.clone(),
            Retry::Idempotent,
        ),
        with_errors(
            spec(
                SessionCommand::WindowMove,
                OperationKind::Command,
                "Move a window instance so its top-left corner is at x, y; its title bar must stay on a display",
                vec![
                    window(),
                    param("x", Type::Float, "Left edge in points"),
                    param("y", Type::Float, "Top edge in points"),
                ],
                window_info.clone(),
                Retry::Idempotent,
            ),
            &["out_of_range"],
        ),
        with_errors(
            spec(
                SessionCommand::WindowResize,
                OperationKind::Command,
                "Resize a window instance, keeping its top-left corner; the size must respect the window's minimum",
                vec![
                    window(),
                    param("width", Type::Float, "Outer width in points"),
                    param("height", Type::Float, "Outer height in points"),
                ],
                window_info,
                Retry::Idempotent,
            ),
            &["out_of_range"],
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
