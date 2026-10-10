//! Bern's intermediate representation.
//!
//! The IR states cross-platform semantics only: no DOM, SwiftUI, GTK or
//! WinUI types appear here. Every target module consumes the same
//! [`Application`] and decides privately how to realize it.

pub mod spec;

use bern_protocol::{LocalizedText, ProtocolDescriptor, Type, TypeTable, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version of the IR layout. Targets declare the range they consume.
pub const IR_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Application {
    pub ir_version: u32,
    pub app: AppInfo,
    pub languages: Languages,
    pub theme: Theme,
    /// The engine protocol the UI binds to.
    pub protocol: ProtocolDescriptor,
    /// Protocol types merged with UI-only value types.
    pub types: TypeTable,
    pub sessions: Vec<SessionDef>,
    pub windows: WindowCatalog,
    /// Media players owned by the application, shared by every window.
    pub players: Vec<PlayerDef>,
    pub components: Vec<ComponentDef>,
    /// Every localized text with its stable resource key.
    pub texts: Vec<LocalizedText>,
}

impl Application {
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        self.components
            .iter()
            .find(|component| component.name == name)
    }

    pub fn session(&self, name: &str) -> Option<&SessionDef> {
        self.sessions.iter().find(|session| session.name == name)
    }

    pub fn window(&self, id: &str) -> Option<&WindowDef> {
        self.windows.windows.iter().find(|window| window.id == id)
    }

    /// Whether any declaration plays media.
    pub fn uses_media(&self) -> bool {
        let mut uses = !self.players.is_empty();
        let mut visit = |node: &Node| {
            if let Node::Element(element) = node
                && element.kind == "Video"
            {
                uses = true;
            }
        };
        for component in &self.components {
            component.body.walk(&mut visit);
        }
        for window in &self.windows.windows {
            window.content.walk(&mut visit);
        }
        uses
    }
}

/// An application player from the `#[bern::players]` catalog. Its id is
/// the stable `player_id` every entry uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub bundle_id: String,
    pub name: LocalizedText,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Languages {
    pub supported: Vec<String>,
    pub default: String,
    pub strict: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Srgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Srgb {
    pub fn parse(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#')?;
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
        Some(Self {
            red: byte(0)?,
            green: byte(2)?,
            blue: byte(4)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub mode: ThemeMode,
    /// Optional brand color. Native targets map it to their accent color
    /// interface; algorithm targets seed the Bern palette with it.
    pub brand_seed: Option<Srgb>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionFieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub initial: Value,
}

/// Typed drafts owned by a UI session, one instance per hosting window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionDef {
    pub name: String,
    pub fields: Vec<SessionFieldDef>,
}

impl SessionDef {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowInitial {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    Auto,
    Virtual,
    NativeRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowRole {
    Standard,
    /// Application preferences window.
    Settings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowDef {
    pub id: String,
    pub title: LocalizedText,
    pub initial: WindowInitial,
    pub presentation: Presentation,
    pub role: WindowRole,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub content: Node,
    /// Session state types instantiated for each instance of this window.
    pub sessions: Vec<String>,
    /// Declared close check, run whenever closing is requested.
    pub close: Option<CloseCheck>,
}

/// The close check of a window. When `when` holds, closing waits for a
/// decision: Save (running `save`), Discard or Cancel. The expressions are
/// evaluated against the closing window's own session drafts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloseCheck {
    pub when: Expr,
    pub message: Text,
    /// Engine command a Save decision runs; closing waits for its result.
    pub save: Option<Action>,
}

/// Static window kinds declared by the root `WindowSet`. The first version
/// supports single-instance windows only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowCatalog {
    pub handle: String,
    pub primary: String,
    pub windows: Vec<WindowDef>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamKind {
    /// The typed engine client.
    Client,
    /// The generated window handle.
    Windows,
    /// The application's player catalog.
    Players,
    /// A value of a schema type, read-only inside the component.
    Value {
        #[serde(rename = "type")]
        ty: Type,
    },
    /// Session state owned by the hosting window.
    Session { session: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    pub kind: ParamKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentDef {
    pub name: String,
    pub module: Vec<String>,
    pub params: Vec<ParamDef>,
    pub body: Node,
    pub source: SourceLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Where a binding reads from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum Binding {
    /// Engine state field, then record field indices.
    Engine { field: usize, path: Vec<usize> },
    /// Session state of the hosting window.
    Session {
        session: String,
        field: usize,
        path: Vec<usize>,
    },
    /// Template variable, pattern binding or component parameter, with the
    /// type of the variable itself so record paths can be resolved.
    Local {
        name: String,
        path: Vec<usize>,
        root: Type,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExprKind {
    Literal {
        value: Value,
    },
    Binding {
        binding: Binding,
    },
    Variant {
        variant: u32,
        fields: Vec<Expr>,
    },
    Not {
        expr: Box<Expr>,
    },
    Neg {
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// A type-checked restricted expression.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Text {
    /// Declared translations, with typed interpolation arguments.
    Localized {
        text: LocalizedText,
        args: Vec<(String, Expr)>,
    },
    /// Runtime data such as user input or media titles.
    Dynamic { expr: Expr },
}

/// A writable session field bound to a control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldRef {
    pub session: String,
    pub field: usize,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Prop {
    Keyword {
        value: String,
    },
    Number {
        value: f64,
    },
    Icon {
        name: String,
    },
    Text {
        text: Text,
    },
    Expr {
        expr: Expr,
    },
    Field {
        field: FieldRef,
    },
    Window {
        id: String,
    },
    TypeName {
        name: String,
    },
    Key {
        field: usize,
        name: String,
    },
    Id {
        value: String,
    },
    /// An application player.
    Player {
        id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowOp {
    Open,
    Close,
    Activate,
}

/// A standard media command on an application player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaOp {
    Open,
    Play,
    Pause,
    Seek,
    SetVolume,
    SetMuted,
}

impl MediaOp {
    pub const ALL: [MediaOp; 6] = [
        MediaOp::Open,
        MediaOp::Play,
        MediaOp::Pause,
        MediaOp::Seek,
        MediaOp::SetVolume,
        MediaOp::SetMuted,
    ];

    /// Method name in declarations, for example `open`.
    pub fn method(self) -> &'static str {
        match self {
            MediaOp::Open => "open",
            MediaOp::Play => "play",
            MediaOp::Pause => "pause",
            MediaOp::Seek => "seek",
            MediaOp::SetVolume => "set_volume",
            MediaOp::SetMuted => "set_muted",
        }
    }

    /// Command id in the media catalog, for example `media.open`.
    pub fn command(self) -> &'static str {
        match self {
            MediaOp::Open => "media.open",
            MediaOp::Play => "media.play",
            MediaOp::Pause => "media.pause",
            MediaOp::Seek => "media.seek",
            MediaOp::SetVolume => "media.set_volume",
            MediaOp::SetMuted => "media.set_muted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Engine command by catalog index with positional arguments.
    Engine { command: usize, args: Vec<Expr> },
    /// Generated window session command.
    Window { op: WindowOp, window: String },
    /// Standard media command on an application player; the player id is
    /// the first parameter, `args` the rest.
    Media {
        player: String,
        op: MediaOp,
        args: Vec<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub var: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// Built-in component name from [`spec::COMPONENTS`].
    pub kind: String,
    /// Control identity for interactive components.
    pub id: Option<String>,
    /// Structural path inside the declaring component, stable for a build.
    pub path: String,
    pub props: BTreeMap<String, Prop>,
    pub template: Option<Template>,
    pub children: Vec<Node>,
    pub action: Option<Action>,
    pub source: SourceLocation,
}

impl Element {
    pub fn prop(&self, name: &str) -> Option<&Prop> {
        self.props.get(name)
    }

    pub fn keyword(&self, name: &str) -> Option<&str> {
        match self.props.get(name) {
            Some(Prop::Keyword { value }) => Some(value),
            _ => None,
        }
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        match self.props.get(name) {
            Some(Prop::Number { value }) => Some(*value),
            _ => None,
        }
    }

    pub fn text(&self, name: &str) -> Option<&Text> {
        match self.props.get(name) {
            Some(Prop::Text { text }) => Some(text),
            _ => None,
        }
    }

    pub fn icon(&self, name: &str) -> Option<&str> {
        match self.props.get(name) {
            Some(Prop::Icon { name }) => Some(name),
            _ => None,
        }
    }

    pub fn field(&self, name: &str) -> Option<&FieldRef> {
        match self.props.get(name) {
            Some(Prop::Field { field }) => Some(field),
            _ => None,
        }
    }

    pub fn expr(&self, name: &str) -> Option<&Expr> {
        match self.props.get(name) {
            Some(Prop::Expr { expr }) => Some(expr),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComponentArg {
    Client,
    Windows,
    Players,
    Value { expr: Expr },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentRef {
    pub component: String,
    pub args: Vec<(String, ComponentArg)>,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IfNode {
    pub branches: Vec<(Expr, Vec<Node>)>,
    pub otherwise: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IfLetNode {
    pub var: String,
    pub value: Expr,
    #[serde(rename = "inner_type")]
    pub inner: Type,
    pub then: Vec<Node>,
    pub otherwise: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchBinding {
    pub name: String,
    pub field: usize,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    /// `None` for the `_` arm.
    pub variant: Option<u32>,
    pub bindings: Vec<MatchBinding>,
    pub body: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchNode {
    pub value: Expr,
    pub enum_name: String,
    pub arms: Vec<MatchArm>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Node {
    Element(Element),
    Component(ComponentRef),
    If(IfNode),
    IfLet(IfLetNode),
    Match(MatchNode),
}

impl Node {
    /// Visit this node and all descendants, depth first.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Node)) {
        visit(self);
        match self {
            Node::Element(element) => {
                for child in &element.children {
                    child.walk(visit);
                }
            }
            Node::Component(_) => {}
            Node::If(node) => {
                for (_, body) in &node.branches {
                    for child in body {
                        child.walk(visit);
                    }
                }
                for child in &node.otherwise {
                    child.walk(visit);
                }
            }
            Node::IfLet(node) => {
                for child in node.then.iter().chain(&node.otherwise) {
                    child.walk(visit);
                }
            }
            Node::Match(node) => {
                for arm in &node.arms {
                    for child in &arm.body {
                        child.walk(visit);
                    }
                }
            }
        }
    }
}

impl Expr {
    /// Visit this expression and every sub-expression.
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Expr)) {
        visit(self);
        match &self.kind {
            ExprKind::Literal { .. } | ExprKind::Binding { .. } => {}
            ExprKind::Variant { fields, .. } => {
                for field in fields {
                    field.walk(visit);
                }
            }
            ExprKind::Not { expr } | ExprKind::Neg { expr } => expr.walk(visit),
            ExprKind::Binary { lhs, rhs, .. } => {
                lhs.walk(visit);
                rhs.walk(visit);
            }
        }
    }

    /// Session state types this expression reads.
    pub fn sessions(&self) -> Vec<String> {
        let mut sessions = Vec::new();
        self.walk(&mut |expr| {
            if let ExprKind::Binding {
                binding: Binding::Session { session, .. },
            } = &expr.kind
                && !sessions.contains(session)
            {
                sessions.push(session.clone());
            }
        });
        sessions
    }

    /// Whether this expression reads engine state.
    pub fn reads_engine(&self) -> bool {
        let mut reads = false;
        self.walk(&mut |expr| {
            if let ExprKind::Binding {
                binding: Binding::Engine { .. },
            } = &expr.kind
            {
                reads = true;
            }
        });
        reads
    }
}

impl Text {
    /// Expressions interpolated into this text.
    pub fn exprs(&self) -> Vec<&Expr> {
        match self {
            Text::Localized { args, .. } => args.iter().map(|(_, expr)| expr).collect(),
            Text::Dynamic { expr } => vec![expr],
        }
    }
}

impl CloseCheck {
    /// Every expression the check evaluates.
    pub fn exprs(&self) -> Vec<&Expr> {
        let mut exprs = vec![&self.when];
        exprs.extend(self.message.exprs());
        if let Some(Action::Engine { args, .. }) = &self.save {
            exprs.extend(args);
        }
        exprs
    }
}

pub use bern_protocol::label_key;

/// Version of the [`ComponentLibrary`] layout.
pub const LIBRARY_DESCRIPTOR_VERSION: u32 = 1;

/// Versioned description of a component library crate, generated from its
/// sources. Applications check it against their own configuration before
/// using its components: descriptor and IR versions, the crate version it
/// was generated from, and translations for every required language.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentLibrary {
    pub descriptor_version: u32,
    pub ir_version: u32,
    /// Cargo package name of the library.
    pub package: String,
    /// Cargo package version the descriptor was generated from.
    pub version: String,
    /// Fingerprint of the library sources and configuration.
    pub source_fingerprint: String,
    pub languages: Vec<String>,
    pub types: TypeTable,
    pub components: Vec<ComponentDef>,
    pub texts: Vec<LocalizedText>,
}

/// Resource key of a text declared on an element property.
pub fn text_key(component: &str, node_path: &str, prop: &str) -> String {
    format!("{component}.{node_path}.{prop}")
}
