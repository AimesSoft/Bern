//! Bern's intermediate representation.
//!
//! The IR states cross-platform semantics only: no DOM, SwiftUI, GTK or
//! WinUI types appear here. Every target module consumes the same
//! [`Application`] and decides privately how to realize it.

pub mod spec;
pub mod style;

use bern_protocol::{LocalizedText, ProtocolDescriptor, Type, TypeTable, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version of the IR layout. Targets declare the range they consume.
pub const IR_VERSION: u32 = 8;

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
    /// Files shipped with the application, sorted by name.
    pub resources: Vec<ResourceDef>,
    /// Layout breakpoints from `[layouts]`, when the application declares
    /// them.
    pub layouts: Option<LayoutConfig>,
    /// Feature contracts with the coverage the compiler proved.
    pub contracts: Vec<FeatureContract>,
    /// Style overrides and motion tokens from `style!`.
    #[serde(default)]
    pub style: style::StyleSheet,
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

/// Breakpoints choosing between layout sizes, from `[layouts]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Available width, in logical units, below which `compact` applies.
    pub compact_below: f64,
}

impl LayoutConfig {
    /// The size for an available width: every width maps to exactly one.
    pub fn size(&self, width: f64) -> LayoutSize {
        if width < self.compact_below {
            LayoutSize::Compact
        } else {
            LayoutSize::Expanded
        }
    }
}

/// A layout size of `Adaptive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutSize {
    Compact,
    Expanded,
}

impl LayoutSize {
    pub const ALL: [LayoutSize; 2] = [LayoutSize::Compact, LayoutSize::Expanded];

    pub fn name(self) -> &'static str {
        match self {
            LayoutSize::Compact => "compact",
            LayoutSize::Expanded => "expanded",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.name() == name)
    }
}

/// A `#[bern::features]` contract: what users can do within the scope of
/// one component, and how each feature is reached in each layout size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureContract {
    /// The features enum, for example `MediaFeature`.
    pub name: String,
    /// The component whose subtree is the contract's scope.
    pub component: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
    pub features: Vec<FeatureDef>,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureDef {
    /// The variant, for example `Clip`.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
    /// Command ids implementing the feature: protocol commands such as
    /// `showcase.feature`, or standard media commands such as `media.play`.
    pub commands: Vec<String>,
    /// Sizes the feature applies to.
    pub layouts: Vec<LayoutSize>,
    /// A manual guarantee for coverage that cannot be proven statically.
    pub guaranteed: Option<String>,
    pub exemptions: Vec<Exemption>,
    /// What the compiler proved, per applicable size.
    pub coverage: Vec<SizeCoverage>,
    pub source: SourceLocation,
}

/// A platform on which a feature need not be reachable, with the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exemption {
    pub platform: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeCoverage {
    pub size: LayoutSize,
    pub coverage: Coverage,
}

/// Entries found for a feature in one size, by control target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Coverage {
    /// At least one entry is present whatever the state.
    Covered {
        entries: Vec<String>,
    },
    /// Entries exist only under conditions the framework cannot prove.
    Conditional {
        entries: Vec<String>,
    },
    Missing,
}

/// The outcome for one feature in one size on one platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum FeatureResult {
    Covered {
        entries: Vec<String>,
    },
    /// Only conditional entries, accepted by a manual guarantee; not proven.
    Guaranteed {
        entries: Vec<String>,
        reason: String,
    },
    /// Not reachable, and exempt on this platform.
    Exempt {
        reason: String,
    },
    Missing,
    /// Only conditional entries and no guarantee.
    Unproven {
        entries: Vec<String>,
    },
}

impl FeatureResult {
    /// Whether the result fails a strict build.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            FeatureResult::Missing | FeatureResult::Unproven { .. }
        )
    }
}

impl FeatureDef {
    /// The results on `platform`, per applicable size.
    pub fn results(&self, platform: &str) -> Vec<(LayoutSize, FeatureResult)> {
        let exemption = self
            .exemptions
            .iter()
            .find(|exemption| exemption.platform == platform);
        self.coverage
            .iter()
            .map(|SizeCoverage { size, coverage }| {
                let result = match (coverage, exemption, &self.guaranteed) {
                    (Coverage::Covered { entries }, _, _) => FeatureResult::Covered {
                        entries: entries.clone(),
                    },
                    (_, Some(exemption), _) => FeatureResult::Exempt {
                        reason: exemption.reason.clone(),
                    },
                    (Coverage::Conditional { entries }, None, Some(reason)) => {
                        FeatureResult::Guaranteed {
                            entries: entries.clone(),
                            reason: reason.clone(),
                        }
                    }
                    (Coverage::Conditional { entries }, None, None) => FeatureResult::Unproven {
                        entries: entries.clone(),
                    },
                    (Coverage::Missing, None, _) => FeatureResult::Missing,
                };
                (*size, result)
            })
            .collect()
    }
}

/// A file the application ships with its build, declared in the
/// `[resources]` table of `Bern.toml`. Engines resolve it by name at run
/// time; each target decides where it lives in the product. Neither the
/// contents nor a build machine path are part of the IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDef {
    /// The file's `/`-separated path relative to `Bern.toml`, for example
    /// `media/aurora.mp4`.
    pub name: String,
    /// The `[resources]` entry that declared the file.
    pub declared: SourceLocation,
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
    /// The application icon from `[app].icon`: a square PNG's
    /// `/`-separated path relative to `Bern.toml`. Without one, targets use
    /// the Bern logo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Who publishes the application, from `[app].publisher`: shown by
    /// installers and package managers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
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
    /// Native where the target has system windows, otherwise the host's
    /// virtual presentation (on the web: the page, or a dialog).
    Auto,
    /// The host's in-app presentation, even where native windows exist.
    Virtual,
    /// A draggable, resizable window: a native window, or a floating panel
    /// inside the page.
    Floating,
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
    /// Whether content extends under a transparent title bar.
    #[serde(default)]
    pub chrome: WindowChrome,
    /// While true, the window hides its title bar and toolbar (and on
    /// touch platforms the status bar), and the pointer once it rests.
    #[serde(default)]
    pub immersive: Option<Expr>,
    /// Seconds without input after which `window.idle` turns true.
    #[serde(default)]
    pub idle_after: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowChrome {
    /// The platform's title bar.
    #[default]
    Standard,
    /// Content under a transparent title bar, the system window controls
    /// over it.
    Unified,
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
    /// The typed engine client of the application protocol (its namespace)
    /// or of one of its modules.
    Client { namespace: String },
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
    /// UI state of the hosting window, such as `window.idle`.
    Window { field: String },
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
    /// A declared resource, by its name in `[resources]`.
    Resource {
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
    /// A typed query call whose result feeds the element's template.
    Query {
        query: QueryBinding,
    },
    /// The feature of the enclosing contract an entry reaches.
    Feature {
        contract: String,
        feature: String,
    },
}

/// A query of the engine catalog bound with its arguments. The arguments
/// are restricted expressions; when their values change, the target asks
/// again under a new parameter version and shows only the current answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryBinding {
    /// Catalog index of the query in the protocol.
    pub query: usize,
    pub args: Vec<Expr>,
    /// The query's declared result.
    pub result: Type,
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

    pub fn resource(&self, name: &str) -> Option<&str> {
        match self.props.get(name) {
            Some(Prop::Resource { name }) => Some(name),
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

    pub fn query(&self, name: &str) -> Option<&QueryBinding> {
        match self.props.get(name) {
            Some(Prop::Query { query }) => Some(query),
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
