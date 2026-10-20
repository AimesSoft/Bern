//! Semantic checking of component declarations and lowering to IR.
//!
//! The checker walks a component body node by node (`nodes`), expanding
//! the slots modules fill (`slots`). Each element (`element`) checks its
//! props (`props`), the bindings and expressions they hold (`bindings`,
//! `expr`) and its action (`action`).

use bern_ir::{self as ir, SourceLocation};
use bern_protocol::{LocalizedText, ProtocolDescriptor, Type, TypeTable};
use bern_syntax as ast;
use bern_target_api::Diagnostic;
use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};

mod action;
mod bindings;
mod element;
mod expr;
mod nodes;
mod props;
mod slots;

/// Parameters of a component as seen by its callers.
#[derive(Clone, Debug)]
pub struct Signature {
    pub name: String,
    pub module: Vec<String>,
    pub params: Vec<(String, ir::ParamKind)>,
    /// Parameters marked `#[content]`: text the component shows as data.
    pub content: BTreeSet<String>,
}

#[derive(Clone, Debug)]
enum Root {
    /// A typed engine client, by the namespace of its protocol.
    Client(String),
    Windows,
    Players,
    Session(String),
    /// A template variable, pattern binding or parameter, and whether the
    /// value itself is content: text that is data, shown as it is.
    Local(Type, bool),
}

#[derive(Default)]
struct Scope(Vec<(String, Root)>);

/// What the application's modules contribute to the shell's slots.
#[derive(Default)]
pub struct Contributions {
    /// Slot id to `(module namespace, component)`, in module order.
    pub slots: BTreeMap<String, Vec<(String, String)>>,
    /// Page slot id to the enum selecting its pages and the pages the
    /// modules add, in module order.
    pub pages: BTreeMap<String, PageSlot>,
    /// Namespaces of every protocol, whose clients expansions pass on;
    /// the application's first.
    pub namespaces: Vec<String>,
    /// Modules that can be switched off: what they contribute shows only
    /// while they run.
    pub optional: BTreeSet<String>,
}

#[derive(Clone)]
pub struct PageSlot {
    pub enum_name: String,
    pub pages: Vec<ContributedPage>,
}

#[derive(Clone)]
pub struct ContributedPage {
    pub module: String,
    pub variant: String,
    pub label: BTreeMap<String, String>,
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub component: String,
}

/// Scope name under which an expansion passes the `namespace` client.
fn slot_client(namespace: &str) -> String {
    format!("__bern_client_{namespace}")
}

impl Contributions {
    /// `declaration`, shown only while `module` runs when it is optional.
    fn guarded(&self, module: &str, declaration: String) -> String {
        match self.namespaces.first() {
            Some(shell) if self.optional.contains(module) => format!(
                "if {}.{}.{module} {{ {declaration} }}",
                slot_client(shell),
                bern_protocol::MODULES_NAMESPACE
            ),
            _ => declaration,
        }
    }
}

impl Scope {
    /// The first name bound to a root of the same kind as `root`.
    fn name_of(&self, wanted: fn(&Root) -> bool) -> Option<String> {
        self.0
            .iter()
            .rev()
            .find(|(_, root)| wanted(root))
            .map(|(name, _)| name.clone())
    }

    fn get(&self, name: &str) -> Option<&Root> {
        self.0
            .iter()
            .rev()
            .find(|(entry, _)| entry == name)
            .map(|(_, root)| root)
    }
    fn push(&mut self, name: String, root: Root) {
        self.0.push((name, root));
    }
    fn pop(&mut self) {
        self.0.pop();
    }
}

#[derive(Clone, Default)]
struct Ctx {
    parent: Option<String>,
    is_root: bool,
    selection: Option<Type>,
    route: Option<Type>,
}

#[derive(Clone, Copy)]
enum FieldExpect {
    Text,
    Bool,
    Number,
    File,
    Position,
    Picker,
    Selection,
    Path,
}

/// Locations are reported relative to the project, like rustc does.
pub fn location(file: &str, span: Span) -> SourceLocation {
    let start = span.start();
    SourceLocation {
        file: file.to_string(),
        line: start.line as u32,
        column: start.column as u32 + 1,
    }
}

/// Turn a message from the shared checks into a diagnostic, lifting a
/// leading `error[Bxxx]: ` into the diagnostic code.
pub fn shared_diagnostic(file: &str, span: Span, message: &str) -> Diagnostic {
    for code in CODES {
        let prefix = format!("error[{code}]: ");
        if let Some(rest) = message.strip_prefix(&prefix) {
            return Diagnostic::error(rest).code(code).at(location(file, span));
        }
    }
    Diagnostic::error(message).at(location(file, span))
}

/// Diagnostic codes the shared checks put in front of their messages.
const CODES: [&str; 8] = [
    "B101", "B102", "B103", "B104", "B105", "B301", "B401", "B402",
];

/// B105 for text shown as it is that nothing declares as data.
fn not_content(path: &str) -> String {
    format!(
        "error[B105]: `{path}` is shown as it is, but is not marked `#[content]`; mark the field or parameter `#[content]` if it is data such as a title or a file name, otherwise have the engine give a number, date or enum and write the sentence with `tr!`"
    )
}

pub fn is_labeled_enum(ty: &Type, types: &TypeTable) -> bool {
    match ty {
        Type::Enum { name } => types
            .enum_def(name)
            .is_some_and(|def| def.variants.iter().all(|variant| variant.label.is_some())),
        _ => false,
    }
}

pub fn is_displayable(ty: &Type, types: &TypeTable) -> bool {
    matches!(ty, Type::Text | Type::Int | Type::Float) || is_labeled_enum(ty, types)
}

pub struct Checker<'a> {
    pub types: &'a TypeTable,
    pub protocol: &'a ProtocolDescriptor,
    pub sessions: &'a [ir::SessionDef],
    pub signatures: &'a BTreeMap<String, Signature>,
    pub window_ids: &'a [String],
    pub player_ids: &'a [String],
    pub languages: &'a [String],
    pub default_language: &'a str,
    pub strict: bool,
    pub file: String,
    pub component: String,
    pub diagnostics: Vec<Diagnostic>,
    pub texts: Vec<LocalizedText>,
    /// Feature contracts by enum name, with their feature names.
    pub contracts: &'a BTreeMap<String, Vec<String>>,
    /// Breakpoints from `[layouts]`, if declared.
    pub layouts: Option<ir::LayoutConfig>,
    /// What modules contribute to slots, expanded where the shell places
    /// `Slot`, `Pages` and `PageContent`.
    pub contributions: Contributions,
    /// `(control, variant)` pairs `style!` declares.
    pub variants: Vec<(String, String)>,
    /// Transition, motion token and pattern names `style!` provides.
    pub transitions: Vec<String>,
    pub tokens: Vec<String>,
    pub patterns: Vec<String>,
    /// Corner names `style!` declares, with their radius on every platform.
    pub corners: Vec<(String, f64)>,
    /// Whether the expression being lowered is a `corner`, where those
    /// names stand for their radii.
    corner_names: bool,
    /// `(component, session)`: components that show a page with
    /// `pages.<id>.show()`, and the session holding the selection they set.
    pub page_sessions: BTreeSet<(String, String)>,
    ids: BTreeSet<String>,
}

impl<'a> Checker<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        types: &'a TypeTable,
        protocol: &'a ProtocolDescriptor,
        sessions: &'a [ir::SessionDef],
        signatures: &'a BTreeMap<String, Signature>,
        window_ids: &'a [String],
        player_ids: &'a [String],
        languages: &'a [String],
        default_language: &'a str,
        strict: bool,
        contracts: &'a BTreeMap<String, Vec<String>>,
        layouts: Option<ir::LayoutConfig>,
    ) -> Self {
        Self {
            contracts,
            layouts,
            types,
            protocol,
            sessions,
            signatures,
            window_ids,
            player_ids,
            languages,
            default_language,
            strict,
            file: String::new(),
            component: String::new(),
            diagnostics: Vec::new(),
            texts: Vec::new(),
            contributions: Contributions::default(),
            variants: Vec::new(),
            transitions: Vec::new(),
            tokens: Vec::new(),
            patterns: Vec::new(),
            corners: Vec::new(),
            corner_names: false,
            page_sessions: BTreeSet::new(),
            ids: BTreeSet::new(),
        }
    }

    fn loc(&self, span: Span) -> SourceLocation {
        location(&self.file, span)
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        let message = message.into();
        self.diagnostics
            .push(shared_diagnostic(&self.file, span, &message));
    }

    /// Check one component body and lower it to an IR node.
    pub fn component(
        &mut self,
        file: &str,
        item: &ast::ComponentItem,
        signature: &Signature,
    ) -> Option<ir::Node> {
        self.file = file.to_string();
        self.component = item.name.to_string();
        self.ids.clear();
        let mut scope = Scope::default();
        for (name, kind) in &signature.params {
            let root = match kind {
                ir::ParamKind::Client { namespace } => Root::Client(namespace.clone()),
                ir::ParamKind::Windows => Root::Windows,
                ir::ParamKind::Players => Root::Players,
                ir::ParamKind::Value { ty } => {
                    Root::Local(ty.clone(), signature.content.contains(name))
                }
                ir::ParamKind::Session { session } => Root::Session(session.clone()),
            };
            scope.push(name.clone(), root);
        }
        if item.body.nodes.len() != 1 {
            self.error(
                item.span,
                "a component body has exactly one root node; wrap siblings in `Column { … }`",
            );
            return None;
        }
        let ctx = Ctx {
            is_root: true,
            ..Ctx::default()
        };
        self.node(&item.body.nodes[0], "0", &ctx, &mut scope)
    }
}
