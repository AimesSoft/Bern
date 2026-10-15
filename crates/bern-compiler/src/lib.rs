//! The Bern compiler front end.
//!
//! It reads the explicitly listed source files with the same parser the
//! macros use, checks commands, bindings, localization, windows and
//! component references, and produces the versioned [`bern_ir::Application`]
//! that independent target modules consume. It never runs application code.

mod check;
pub mod config;
mod features;
pub mod style;

pub use check::{Signature, location};
pub use config::{Config, SourceFile};

use bern_ir::{self as ir, IR_VERSION};
use bern_protocol::{
    EnumDef, FieldDef, LocalizedText, ProtocolDescriptor, RecordDef, Type, TypeTable, Value,
    VariantDef, compose,
};
use bern_syntax::{self as ast, ValueKind};
use bern_target_api::Diagnostic;
use check::{Checker, ContributedPage, Contributions, PageSlot, shared_diagnostic};
use std::collections::{BTreeMap, BTreeSet};

/// Result of a successful compilation.
pub struct Compiled {
    pub app: ir::Application,
    /// Rust path of the protocol module, e.g. `showcase_protocol::showcase`.
    pub protocol_module: String,
    /// Rust paths of the modules' protocol modules, in application order.
    pub module_protocols: Vec<String>,
    /// Versioned descriptors of the component libraries in use.
    pub libraries: Vec<ir::ComponentLibrary>,
    pub warnings: Vec<Diagnostic>,
}

struct Located<T> {
    file: String,
    module: Vec<String>,
    /// Namespace of the application or module the source belongs to.
    home: String,
    item: T,
}

/// Which protocol a `…::Client` parameter names: the protocol modules of
/// the application and of its modules, by their Rust module name.
struct Clients {
    /// `(Rust module name, namespace)`, the application first.
    protocols: Vec<(String, String)>,
}

impl Clients {
    /// The namespace of a `Client` type written as `path`, in a source of
    /// the module (or application) whose namespace is `home`. A bare
    /// `Client` names the home protocol.
    fn namespace(&self, path: &[String], home: &str) -> Option<String> {
        if path.len() == 1 {
            return Some(home.to_string());
        }
        let module = &path[path.len() - 2];
        self.protocols
            .iter()
            .find(|(name, _)| name == module)
            .map(|(_, namespace)| namespace.clone())
    }
}

fn syn_error(file: &str, error: syn::Error) -> Vec<Diagnostic> {
    error
        .into_iter()
        .map(|error| shared_diagnostic(file, error.span(), &error.to_string()))
        .collect()
}

fn read(file: &SourceFile) -> Result<ast::SourceItems, Vec<Diagnostic>> {
    let text = std::fs::read_to_string(&file.path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read {}: {error}",
            file.path.display()
        ))]
    })?;
    ast::parse_source_file(&text).map_err(|error| syn_error(&file.display, error))
}

/// Resolve a UI-side type reference against the merged type table.
fn resolve_type(ty: &ast::TypeRef, types: &TypeTable) -> Result<Type, String> {
    Ok(match ty {
        ast::TypeRef::Unit => Type::Unit,
        ast::TypeRef::Bool => Type::Bool,
        ast::TypeRef::Int => Type::Int,
        ast::TypeRef::Float => Type::Float,
        ast::TypeRef::Text => Type::Text,
        ast::TypeRef::Optional(inner) => Type::optional(resolve_type(inner, types)?),
        ast::TypeRef::List(inner) => Type::list(resolve_type(inner, types)?),
        ast::TypeRef::Named(path) => {
            let name = path.last().unwrap().clone();
            if types.enum_def(&name).is_some() {
                Type::Enum { name }
            } else if types.record_def(&name).is_some() {
                Type::Record { name }
            } else {
                return Err(format!(
                    "unknown type `{name}`; declare it with #[bern::value] in a listed source"
                ));
            }
        }
    })
}

fn initial_value(expr: &ast::Expr, ty: &Type, types: &TypeTable) -> Result<Value, String> {
    let value = match expr {
        ast::Expr::Literal { value, .. } => Checker::literal(value).0,
        ast::Expr::Unary {
            op: ast::UnaryOp::Neg,
            expr,
            ..
        } => match initial_value(expr, ty, types)? {
            Value::Int(value) => Value::Int(-value),
            Value::Float(value) => Value::Float(-value),
            _ => return Err("only numbers can be negated".into()),
        },
        ast::Expr::Variant(variant) if variant.fields.is_empty() => {
            let def = types
                .enum_def(&variant.ty.to_string())
                .ok_or_else(|| format!("unknown enum `{}`", variant.ty))?;
            let index = def
                .variant_index(&variant.variant.to_string())
                .ok_or_else(|| format!("`{}` has no variant `{}`", variant.ty, variant.variant))?;
            Value::Enum {
                variant: index as u32,
                fields: vec![],
            }
        }
        _ => return Err("an initial value is a literal or a unit enum variant".into()),
    };
    value
        .check(ty, types)
        .map_err(|error| format!("initial value does not match the field: {error}"))?;
    Ok(value)
}

/// Types written in the fields of a value declaration.
fn value_types(value: &ast::ValueItem) -> Vec<&ast::TypeRef> {
    match &value.kind {
        ValueKind::Enum(variants) => variants
            .iter()
            .flat_map(|variant| variant.fields.iter().map(|field| &field.ty))
            .collect(),
        ValueKind::Record(fields) => fields.iter().map(|field| &field.ty).collect(),
    }
}

/// Add framework value types such as `MediaSource` when declarations use
/// them and the protocol has not brought them in already.
fn add_framework_types(types: &mut TypeTable, mentioned: &[&ast::TypeRef]) {
    if types.record_def(bern_media::MEDIA_SOURCE).is_none()
        && mentioned
            .iter()
            .any(|ty| bern_syntax::mentions(ty, bern_media::MEDIA_SOURCE))
    {
        types.records.push(bern_media::media_source_def());
    }
}

fn merge_ui_types(
    base: &TypeTable,
    values: &[Located<ast::ValueItem>],
    languages: (&[String], bool),
    diagnostics: &mut Vec<Diagnostic>,
) -> TypeTable {
    let mut types = base.clone();
    let protocol_names: BTreeSet<String> = types
        .enums
        .iter()
        .map(|def| def.name.clone())
        .chain(types.records.iter().map(|def| def.name.clone()))
        .collect();
    let mut seen = BTreeSet::new();
    for value in values {
        let name = value.item.name.to_string();
        if name == bern_media::MEDIA_SOURCE {
            diagnostics.push(
                Diagnostic::error(
                    "`MediaSource` is a framework type; use it from `bern::prelude` instead of declaring it",
                )
                .at(location(&value.file, value.item.span)),
            );
            continue;
        }
        if protocol_names.contains(&name) || !seen.insert(name.clone()) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "type `{name}` is declared more than once; Bern type names are unique per application"
                ))
                .at(location(&value.file, value.item.span)),
            );
            continue;
        }
        match &value.item.kind {
            ValueKind::Enum(_) => types.enums.push(EnumDef {
                name,
                variants: vec![],
            }),
            ValueKind::Record(_) => types.records.push(RecordDef {
                name,
                fields: vec![],
            }),
        }
    }
    let lookup = types.clone();
    let languages = Some(languages);
    for value in values {
        let name = value.item.name.to_string();
        let field = |field: &ast::FieldItem, diagnostics: &mut Vec<Diagnostic>| match resolve_type(
            &field.ty, &lookup,
        ) {
            Ok(ty) => Some(FieldDef {
                name: field.name.to_string(),
                ty,
                doc: field.doc.clone(),
            }),
            Err(message) => {
                diagnostics.push(Diagnostic::error(message).at(location(&value.file, field.span)));
                None
            }
        };
        match &value.item.kind {
            ValueKind::Enum(variants) => {
                let mut defs = Vec::new();
                for variant in variants {
                    let label = variant.label.as_ref().map(|tr| {
                        for (span, message) in bern_syntax::lower::check_translations(tr, languages)
                        {
                            diagnostics.push(shared_diagnostic(&value.file, span, &message));
                        }
                        LocalizedText {
                            key: bern_protocol::label_key(&name, &variant.name.to_string()),
                            translations: tr
                                .entries
                                .iter()
                                .map(|entry| (entry.language.clone(), entry.text.clone()))
                                .collect(),
                            params: vec![],
                        }
                    });
                    defs.push(VariantDef {
                        name: variant.name.to_string(),
                        fields: variant
                            .fields
                            .iter()
                            .filter_map(|f| field(f, diagnostics))
                            .collect(),
                        label,
                    });
                }
                if let Some(def) = types.enums.iter_mut().find(|def| def.name == name) {
                    def.variants = defs;
                }
            }
            ValueKind::Record(fields) => {
                let defs = fields
                    .iter()
                    .filter_map(|f| field(f, diagnostics))
                    .collect();
                if let Some(def) = types.records.iter_mut().find(|def| def.name == name) {
                    def.fields = defs;
                }
            }
        }
    }
    if let Err(error) = types.validate() {
        diagnostics.push(Diagnostic::error(error.message));
    }
    types
}

fn param_kind(
    param: &ast::Param,
    clients: Option<(&Clients, &str)>,
    windows_handle: Option<&str>,
    players_handle: Option<&str>,
    sessions: &[ir::SessionDef],
    types: &TypeTable,
) -> Result<ir::ParamKind, String> {
    let name = param.ty.name().unwrap_or_default().to_string();
    match param.kind {
        ast::ParamKind::Session => {
            if sessions.iter().any(|session| session.name == name) {
                Ok(ir::ParamKind::Session { session: name })
            } else {
                Err(format!(
                    "`#[session]` needs a #[bern::session] struct; `{name}` is not one"
                ))
            }
        }
        ast::ParamKind::Prop => match &param.ty {
            ast::TypeRef::Named(path)
                if name == "Client"
                    && clients
                        .is_some_and(|(clients, home)| clients.namespace(path, home).is_some()) =>
            {
                let (clients, home) = clients.expect("checked");
                Ok(ir::ParamKind::Client {
                    namespace: clients.namespace(path, home).expect("checked"),
                })
            }
            ast::TypeRef::Named(_) if Some(name.as_str()) == windows_handle => {
                Ok(ir::ParamKind::Windows)
            }
            ast::TypeRef::Named(_) if Some(name.as_str()) == players_handle => {
                Ok(ir::ParamKind::Players)
            }
            ast::TypeRef::Named(_) if sessions.iter().any(|session| session.name == name) => Err(
                format!("session state `{name}` must be marked `#[session]`"),
            ),
            other => resolve_type(other, types).map(|ty| ir::ParamKind::Value { ty }),
        },
    }
}

/// Window handle name and window ids declared by the root component.
fn root_windows(root: &ast::ComponentItem) -> Option<(String, Vec<String>)> {
    let ast::Node::Element(set) = root.body.nodes.first()? else {
        return None;
    };
    if set.name != "WindowSet" {
        return None;
    }
    let handle = match &set.prop("handle")?.value {
        ast::Expr::Path(path) => path.single()?.to_string(),
        _ => return None,
    };
    let ids = set
        .children
        .iter()
        .flatten()
        .filter_map(|child| match child {
            ast::Node::Element(window) => match &window.prop("id")?.value {
                ast::Expr::Path(path) => path.single().map(ToString::to_string),
                _ => None,
            },
            _ => None,
        })
        .collect();
    Some((handle, ids))
}

fn component_refs(node: &ir::Node, out: &mut Vec<String>) {
    node.walk(&mut |node| {
        if let ir::Node::Component(reference) = node {
            out.push(reference.component.clone());
        }
    });
}

/// Inspectors that do not stand directly in their container, and
/// containers with more than one.
fn inspectors<'a>(
    nodes: &'a [ir::Node],
    direct: bool,
    out: &mut Vec<(&'a ir::Element, &'static str)>,
) {
    for node in nodes {
        match node {
            ir::Node::Element(element) => {
                if element.kind == "Inspector" && !direct {
                    out.push((
                        element,
                        "an `Inspector` stands directly in the container it attaches to, not inside `if`, `match` or `ForEach`; `presented` shows and hides it",
                    ));
                }
                let panels = element
                    .children
                    .iter()
                    .filter(|child| matches!(child, ir::Node::Element(e) if e.kind == "Inspector"))
                    .collect::<Vec<_>>();
                if let Some(ir::Node::Element(extra)) = panels.get(1) {
                    out.push((extra, "a container has at most one `Inspector`"));
                }
                inspectors(&element.children, element.kind != "ForEach", out);
            }
            ir::Node::If(node) => {
                for (_, body) in &node.branches {
                    inspectors(body, false, out);
                }
                inspectors(&node.otherwise, false, out);
            }
            ir::Node::IfLet(node) => {
                inspectors(&node.then, false, out);
                inspectors(&node.otherwise, false, out);
            }
            ir::Node::Match(node) => {
                for arm in &node.arms {
                    inspectors(&arm.body, false, out);
                }
            }
            ir::Node::Component(_) => {}
        }
    }
}

/// The items of a `Rail`: the elements standing in it, also inside `if`,
/// `match` and the rows of `ForEach`; components placed there directly.
fn rail_items<'a>(
    nodes: &'a [ir::Node],
    items: &mut Vec<&'a ir::Element>,
    components: &mut Vec<&'a ir::ComponentRef>,
) {
    for node in nodes {
        match node {
            ir::Node::Element(element) if element.kind == "ForEach" => {
                rail_items(&element.children, items, components);
            }
            ir::Node::Element(element) => items.push(element),
            ir::Node::If(node) => {
                for (_, body) in &node.branches {
                    rail_items(body, items, components);
                }
                rail_items(&node.otherwise, items, components);
            }
            ir::Node::IfLet(node) => {
                rail_items(&node.then, items, components);
                rail_items(&node.otherwise, items, components);
            }
            ir::Node::Match(node) => {
                for arm in &node.arms {
                    rail_items(&arm.body, items, components);
                }
            }
            ir::Node::Component(component) => components.push(component),
        }
    }
}

/// The first control in `nodes`, looking into the components they place;
/// component cycles are reported elsewhere, so each is visited once.
fn control_inside<'a>(
    nodes: &'a [ir::Node],
    bodies: &BTreeMap<&str, &'a ir::Node>,
) -> Option<&'a ir::Element> {
    let mut pending: Vec<&'a ir::Node> = nodes.iter().collect();
    let mut seen = BTreeSet::new();
    while let Some(node) = pending.pop() {
        let mut found = None;
        node.walk(&mut |node| match node {
            ir::Node::Element(element)
                if found.is_none()
                    && ir::spec::component(&element.kind).is_some_and(|spec| spec.interactive) =>
            {
                found = Some(element);
            }
            ir::Node::Component(reference) if seen.insert(reference.component.clone()) => {
                if let Some(body) = bodies.get(reference.component.as_str()) {
                    pending.push(body);
                }
            }
            _ => {}
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Dividers among the items a toolbar shows through its conditions and
/// repetitions, not inside the items themselves.
fn toolbar_dividers<'a>(nodes: &'a [ir::Node], found: &mut Vec<&'a ir::Element>) {
    for node in nodes {
        match node {
            ir::Node::Element(element) if element.kind == "Divider" => found.push(element),
            ir::Node::Element(element) if element.kind == "ForEach" => {
                toolbar_dividers(&element.children, found);
            }
            ir::Node::Element(_) | ir::Node::Component(_) => {}
            ir::Node::If(node) => {
                for (_, body) in &node.branches {
                    toolbar_dividers(body, found);
                }
                toolbar_dividers(&node.otherwise, found);
            }
            ir::Node::IfLet(node) => {
                toolbar_dividers(&node.then, found);
                toolbar_dividers(&node.otherwise, found);
            }
            ir::Node::Match(node) => {
                for arm in &node.arms {
                    toolbar_dividers(&arm.body, found);
                }
            }
        }
    }
}

fn find_cycle(components: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    fn visit(
        name: &str,
        graph: &BTreeMap<String, Vec<String>>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
    ) -> Option<Vec<String>> {
        if let Some(position) = stack.iter().position(|entry| entry == name) {
            let mut cycle = stack[position..].to_vec();
            cycle.push(name.to_string());
            return Some(cycle);
        }
        if done.contains(name) {
            return None;
        }
        stack.push(name.to_string());
        for callee in graph.get(name).into_iter().flatten() {
            if let Some(cycle) = visit(callee, graph, stack, done) {
                return Some(cycle);
            }
        }
        stack.pop();
        done.insert(name.to_string());
        None
    }
    let mut done = BTreeSet::new();
    for name in components.keys() {
        if let Some(cycle) = visit(name, components, &mut Vec::new(), &mut done) {
            return Some(cycle);
        }
    }
    None
}

fn window_catalog(
    root: &ir::Element,
    handle: String,
    graph: &BTreeMap<String, Vec<String>>,
    signatures: &BTreeMap<String, Signature>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ir::WindowCatalog> {
    let primary = match root.prop("primary") {
        Some(ir::Prop::Window { id }) => id.clone(),
        _ => return None,
    };
    let mut windows = Vec::new();
    for child in &root.children {
        let ir::Node::Element(window) = child else {
            continue;
        };
        let id = match window.prop("id") {
            Some(ir::Prop::Window { id }) => id.clone(),
            _ => continue,
        };
        let title = match window.text("title") {
            Some(ir::Text::Localized { text, .. }) => text.clone(),
            _ => continue,
        };
        let content = window.children.first()?.clone();
        let mut reachable = Vec::new();
        component_refs(&content, &mut reachable);
        let mut queue = reachable.clone();
        while let Some(name) = queue.pop() {
            for callee in graph.get(&name).into_iter().flatten() {
                if !reachable.contains(callee) {
                    reachable.push(callee.clone());
                    queue.push(callee.clone());
                }
            }
        }
        let mut sessions: Vec<String> = Vec::new();
        for name in &reachable {
            for (_, kind) in signatures
                .get(name)
                .map(|s| s.params.as_slice())
                .unwrap_or(&[])
            {
                if let ir::ParamKind::Session { session } = kind
                    && !sessions.contains(session)
                {
                    sessions.push(session.clone());
                }
            }
        }
        let keyword = |name: &str, default: &str| -> String {
            window.keyword(name).unwrap_or(default).to_string()
        };
        let initial = match keyword("initial", "closed").as_str() {
            "open" => ir::WindowInitial::Open,
            _ => ir::WindowInitial::Closed,
        };
        let presentation = match keyword("presentation", "auto").as_str() {
            "virtual" => ir::Presentation::Virtual,
            "floating" => ir::Presentation::Floating,
            "native_required" => ir::Presentation::NativeRequired,
            _ => ir::Presentation::Auto,
        };
        let role = match keyword("role", "standard").as_str() {
            "settings" => ir::WindowRole::Settings,
            "dialog" => ir::WindowRole::Dialog,
            _ => ir::WindowRole::Standard,
        };
        let close = match (window.expr("confirm_close"), window.text("close_message")) {
            (Some(when), Some(message)) => Some(ir::CloseCheck {
                when: when.clone(),
                message: message.clone(),
                save: window.action.clone(),
            }),
            _ => None,
        };
        if let Some(check) = &close {
            if matches!(
                check.save,
                Some(
                    ir::Action::Window { .. }
                        | ir::Action::Media { .. }
                        | ir::Action::Session { .. }
                )
            ) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "window `{id}`: the save action must be an engine command"
                    ))
                    .at(window.source.clone()),
                );
            }
            // The check reads the closing window's own drafts.
            for exprs in check.exprs() {
                for session in exprs.sessions() {
                    if !sessions.contains(&session) {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "window `{id}`'s close check reads `{session}`, which this window does not own; its components own: {}",
                                if sessions.is_empty() { "none".to_string() } else { sessions.join(", ") }
                            ))
                            .at(window.source.clone()),
                        );
                    }
                }
            }
        }
        // Session parameters of the root exist per window instance and are
        // only meaningful in that window's close check.
        let mut content_sessions = Vec::new();
        content.walk(&mut |node| {
            if let ir::Node::Component(reference) = node {
                for (_, arg) in &reference.args {
                    if let ir::ComponentArg::Value { expr } = arg {
                        content_sessions.extend(expr.sessions());
                    }
                }
            }
        });
        if let Some(session) = content_sessions.first() {
            diagnostics.push(
                Diagnostic::error(format!(
                    "window `{id}` passes `{session}` state from the root; components take their own `#[session]` parameters"
                ))
                .at(window.source.clone()),
            );
        }
        windows.push(ir::WindowDef {
            id,
            title,
            initial,
            presentation,
            role,
            width: window.number("width"),
            height: window.number("height"),
            min_width: window.number("min_width"),
            min_height: window.number("min_height"),
            content,
            sessions,
            close,
            chrome: match window.keyword("chrome") {
                Some("unified") => ir::WindowChrome::Unified,
                _ => ir::WindowChrome::Standard,
            },
            immersive: window.expr("immersive").cloned(),
            idle_after: window.number("idle_after"),
        });
    }
    if windows
        .iter()
        .filter(|window| window.role == ir::WindowRole::Settings)
        .count()
        > 1
    {
        diagnostics.push(
            Diagnostic::error("only one window may have `role: settings`").at(root.source.clone()),
        );
    }
    if windows
        .iter()
        .find(|window| window.id == primary)
        .is_some_and(|window| window.role == ir::WindowRole::Settings)
    {
        diagnostics.push(
            Diagnostic::error("the primary window cannot be the settings window")
                .at(root.source.clone()),
        );
    }
    Some(ir::WindowCatalog {
        handle,
        primary,
        windows,
        accent: root.expr("accent").cloned(),
        appearance: root.expr("appearance").cloned(),
    })
}

fn source_fingerprint(files: &[&std::path::Path]) -> String {
    let mut bytes = Vec::new();
    for file in files {
        bytes.extend(file.display().to_string().into_bytes());
        bytes.extend(std::fs::read(file).unwrap_or_default());
    }
    format!("{:016x}", bern_protocol::fingerprint_bytes(&bytes))
}

/// Generate the versioned descriptor of a component library from its
/// sources. Library components are presentational: they take value
/// parameters only and cannot own session state or call commands.
pub fn compile_library(
    library: &config::LibraryConfig,
) -> Result<ir::ComponentLibrary, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut values = Vec::new();
    let mut components = Vec::new();
    let crate_name = library.package.crate_name();
    let home = String::new();
    for file in &library.sources {
        let items = match read(file) {
            Ok(items) => items,
            Err(errors) => {
                diagnostics.extend(errors);
                continue;
            }
        };
        if !items.protocols.is_empty() || !items.sessions.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "{}: component libraries cannot declare protocols or session state",
                file.display
            )));
        }
        let module = |relative: &Vec<String>| {
            let mut module = vec![crate_name.clone()];
            module.extend(file.module.iter().cloned());
            module.extend(relative.iter().cloned());
            module
        };
        for (path, item) in items.values {
            values.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
        for (path, item) in items.components {
            components.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
    }
    let mut base = TypeTable::default();
    let mentioned: Vec<&ast::TypeRef> = values
        .iter()
        .flat_map(|value| value_types(&value.item))
        .chain(
            components
                .iter()
                .flat_map(|component| component.item.params.iter().map(|param| &param.ty)),
        )
        .collect();
    add_framework_types(&mut base, &mentioned);
    let types = merge_ui_types(
        &base,
        &values,
        (&library.languages, library.strict),
        &mut diagnostics,
    );
    let mut signatures = BTreeMap::new();
    for component in &components {
        let mut params = Vec::new();
        for param in &component.item.params {
            match param_kind(param, None, None, None, &[], &types) {
                Ok(kind) => params.push((param.name.to_string(), kind)),
                Err(message) => diagnostics.push(
                    Diagnostic::error(format!(
                        "{message}; library components take value parameters only"
                    ))
                    .at(location(&component.file, param.span)),
                ),
            }
        }
        let name = component.item.name.to_string();
        signatures.insert(
            name.clone(),
            Signature {
                name,
                module: component.module.clone(),
                params,
            },
        );
    }
    let protocol = ProtocolDescriptor {
        modules: Vec::new(),
        events: Vec::new(),
        descriptor_version: bern_protocol::DESCRIPTOR_VERSION,
        namespace: "library".into(),
        version: 0,
        types: TypeTable::default(),
        state: vec![],
        commands: vec![],
    };
    let no_contracts = BTreeMap::new();
    let mut checker = Checker::new(
        &types,
        &protocol,
        &[],
        &signatures,
        &[],
        &[],
        &library.languages,
        &library.default_language,
        library.strict,
        &no_contracts,
        None,
    );
    let mut lowered = Vec::new();
    for component in &components {
        let name = component.item.name.to_string();
        let signature = &signatures[&name];
        if let Some(body) = checker.component(&component.file, &component.item, signature) {
            lowered.push(ir::ComponentDef {
                name,
                module: component.module.clone(),
                params: signature
                    .params
                    .iter()
                    .map(|(name, kind)| ir::ParamDef {
                        name: name.clone(),
                        kind: kind.clone(),
                    })
                    .collect(),
                body,
                source: location(&component.file, component.item.span),
            });
        }
    }
    diagnostics.append(&mut checker.diagnostics);
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }
    let mut texts = std::mem::take(&mut checker.texts);
    for def in &types.enums {
        texts.extend(
            def.variants
                .iter()
                .filter_map(|variant| variant.label.clone()),
        );
    }
    let mut files: Vec<&std::path::Path> = vec![&library.manifest];
    files.extend(library.sources.iter().map(|source| source.path.as_path()));
    Ok(ir::ComponentLibrary {
        descriptor_version: ir::LIBRARY_DESCRIPTOR_VERSION,
        ir_version: IR_VERSION,
        package: library.package.name.clone(),
        version: library.version.clone(),
        source_fingerprint: source_fingerprint(&files),
        languages: library.languages.clone(),
        types,
        components: lowered,
        texts,
    })
}

/// Check that a library descriptor matches this application: versions and
/// translations for every language the application requires.
pub fn check_library(library: &ir::ComponentLibrary, config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if library.descriptor_version != ir::LIBRARY_DESCRIPTOR_VERSION
        || library.ir_version != IR_VERSION
    {
        diagnostics.push(Diagnostic::error(format!(
            "library `{}` was described with descriptor {} / IR {}; this compiler uses {} / {}",
            library.package,
            library.descriptor_version,
            library.ir_version,
            ir::LIBRARY_DESCRIPTOR_VERSION,
            IR_VERSION
        )));
    }
    if config.strict {
        for text in &library.texts {
            let missing: Vec<_> = config
                .languages
                .iter()
                .filter(|language| !text.translations.contains_key(*language))
                .cloned()
                .collect();
            if !missing.is_empty() {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "Missing translation\nLibrary: {} {}\nText: {}\nMissing language: {}\nRequired languages: {}",
                        library.package,
                        library.version,
                        text.key,
                        missing.join(", "),
                        config.languages.join(", ")
                    ))
                    .code("B102"),
                );
            }
        }
    }
    diagnostics
}

/// Collect, check and lower a project to IR.
pub fn compile(config: &Config) -> Result<Compiled, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // Protocol: exactly one module, lowered by the shared code.
    let protocol_file = &config.protocol_source;
    let protocol_items = read(protocol_file)?;
    let [(protocol_path, protocol_item)] = protocol_items.protocols.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{} must contain exactly one #[bern::protocol] module",
            protocol_file.display
        ))]);
    };
    let shell = bern_syntax::lower::protocol_descriptor(protocol_item)
        .map_err(|error| syn_error(&protocol_file.display, error))?;
    let protocol_module_name = protocol_item.module.ident.to_string();
    let mut protocol_module = vec![config.protocol_package.crate_name()];
    protocol_module.extend(protocol_file.module.iter().cloned());
    protocol_module.extend(protocol_path.iter().cloned());
    protocol_module.push(protocol_module_name.clone());
    let languages = Some((config.languages.as_slice(), config.strict));
    let check_labels = |item: &ast::ProtocolItem, file: &str, diagnostics: &mut Vec<Diagnostic>| {
        for value in &item.values {
            if let ValueKind::Enum(variants) = &value.kind {
                for tr in variants.iter().filter_map(|variant| variant.label.as_ref()) {
                    for (span, message) in bern_syntax::lower::check_translations(tr, languages) {
                        diagnostics.push(shared_diagnostic(file, span, &message));
                    }
                }
            }
        }
    };
    check_labels(protocol_item, &protocol_file.display, &mut diagnostics);

    // Module protocols, composed with the application's into one catalog.
    let mut clients = Clients {
        protocols: vec![(protocol_module_name.clone(), shell.namespace.clone())],
    };
    let mut module_descriptors = Vec::new();
    let mut module_protocols = Vec::new();
    for module in &config.modules {
        let file = &module.protocol_source;
        let items = read(file)?;
        let [(path, item)] = items.protocols.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "{} must contain exactly one #[bern::protocol] module",
                file.display
            ))]);
        };
        let descriptor = bern_syntax::lower::protocol_descriptor(item)
            .map_err(|error| syn_error(&file.display, error))?;
        if descriptor.namespace != module.id {
            diagnostics.push(Diagnostic::error(format!(
                "{}: the protocol of module `{}` must use `namespace = \"{}\"`, not `{}`",
                file.display, module.id, module.id, descriptor.namespace
            )));
        }
        check_labels(item, &file.display, &mut diagnostics);
        let name = item.module.ident.to_string();
        if clients.protocols.iter().any(|(other, _)| *other == name) {
            diagnostics.push(Diagnostic::error(format!(
                "{}: protocol module `{name}` has the name of another protocol module; name it after the module",
                file.display
            )));
        }
        clients
            .protocols
            .push((name.clone(), descriptor.namespace.clone()));
        let mut rust_path = vec![module.protocol_package.crate_name()];
        rust_path.extend(file.module.iter().cloned());
        rust_path.extend(path.iter().cloned());
        rust_path.push(name);
        module_protocols.push(rust_path.join("::"));
        module_descriptors.push(descriptor);
    }
    // A module answers only the events of modules it depends on; the
    // application answers any module's events.
    for (descriptor, module) in module_descriptors.iter().zip(&config.modules) {
        for command in &descriptor.commands {
            let Some(on) = &command.on else { continue };
            let emitter = on.split('.').next().unwrap_or_default();
            if !module
                .depends
                .iter()
                .any(|dependency| dependency == emitter)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{}: `{}` answers `{on}`, but module `{}` does not depend on `{emitter}`; add it to `depends`",
                    module.protocol_source.display, command.id, module.id
                )));
            }
        }
    }
    for command in &shell.commands {
        if let Some(on) = &command.on
            && on.split('.').next() == Some(shell.namespace.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "{}: `{}` answers the application's own event `{on}`; events reach the modules and the application above the emitter",
                protocol_file.display, command.id
            )));
        }
    }
    let protocol = if module_descriptors.is_empty() {
        shell
    } else {
        let parts: Vec<bern_protocol::ModulePart<'_>> = module_descriptors
            .iter()
            .zip(&config.modules)
            .map(|(descriptor, module)| bern_protocol::ModulePart {
                descriptor,
                optional: module.optional,
            })
            .collect();
        match compose(&shell, &parts) {
            Ok(protocol) => protocol,
            Err(error) => {
                diagnostics.push(Diagnostic::error(error.0));
                return Err(diagnostics);
            }
        }
    };

    // UI sources.
    let mut values = Vec::new();
    let mut sessions_ast = Vec::new();
    let mut players_ast = Vec::new();
    let mut components = Vec::new();
    let mut features_ast = Vec::new();
    let mut styles = Vec::new();
    let ui_sources = config
        .ui_sources
        .iter()
        .map(|file| (file, protocol.namespace.clone()))
        .chain(config.modules.iter().flat_map(|module| {
            module
                .ui_sources
                .iter()
                .map(move |file| (file, module.id.clone()))
        }));
    for (file, home) in ui_sources {
        let items = match read(file) {
            Ok(items) => items,
            Err(errors) => {
                diagnostics.extend(errors);
                continue;
            }
        };
        if !items.protocols.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "{} declares a protocol; protocols live in the [protocol] source",
                file.display
            )));
        }
        let module = |relative: &Vec<String>| {
            let mut module = file.module.clone();
            module.extend(relative.iter().cloned());
            module
        };
        for (path, item) in items.values {
            values.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
        for (path, item) in items.sessions {
            sessions_ast.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
        for (path, item) in items.players {
            players_ast.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
        for (path, item) in items.components {
            components.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
        for (_, item) in items.styles {
            if home == protocol.namespace {
                styles.push((file.display.clone(), item));
            } else {
                diagnostics.push(Diagnostic::error(format!(
                    "{}: modules cannot declare `style!`; the application's interface declares one for every module",
                    file.display
                )));
            }
        }
        for (path, item) in items.features {
            features_ast.push(Located {
                file: file.display.clone(),
                module: module(&path),
                home: home.clone(),
                item,
            });
        }
    }
    for extra in players_ast.iter().skip(1) {
        diagnostics.push(
            Diagnostic::error(
                "an application declares at most one #[bern::players] catalog; add players as fields",
            )
            .at(location(&extra.file, extra.item.span)),
        );
    }
    let players_handle = players_ast
        .first()
        .map(|located| located.item.name.to_string());
    let players: Vec<ir::PlayerDef> = players_ast
        .first()
        .map(|located| {
            located
                .item
                .players
                .iter()
                .map(|player| ir::PlayerDef {
                    id: player.name.to_string(),
                    doc: player.doc.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let player_ids: Vec<String> = players.iter().map(|player| player.id.clone()).collect();

    // Component libraries: versioned descriptors generated from their
    // sources and checked against this application's configuration.
    let mut libraries = Vec::new();
    for manifest in &config.libraries {
        match config::load_library(manifest).and_then(|library| compile_library(&library)) {
            Ok(library) => {
                diagnostics.extend(check_library(&library, config));
                libraries.push(library);
            }
            Err(errors) => diagnostics.extend(errors),
        }
    }
    let mut base_types = protocol.types.clone();
    for library in &libraries {
        if let Err(error) = base_types.merge(&library.types) {
            diagnostics.push(Diagnostic::error(format!(
                "library `{}`: {}",
                library.package, error.message
            )));
        }
    }
    let mentioned: Vec<&ast::TypeRef> = values
        .iter()
        .flat_map(|value| value_types(&value.item))
        .chain(
            sessions_ast
                .iter()
                .flat_map(|session| session.item.fields.iter().map(|field| &field.ty)),
        )
        .chain(
            components
                .iter()
                .flat_map(|component| component.item.params.iter().map(|param| &param.ty)),
        )
        .collect();
    add_framework_types(&mut base_types, &mentioned);
    let mut types = merge_ui_types(
        &base_types,
        &values,
        (&config.languages, config.strict),
        &mut diagnostics,
    );

    // Slots: the enums marked `pages = "slot"` gain a variant for every
    // page the modules add to that slot.
    let mut page_enums: BTreeMap<String, String> = BTreeMap::new();
    for value in &values {
        if let Some(slot) = &value.item.pages
            && let Some(other) = page_enums.insert(slot.clone(), value.item.name.to_string())
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "page slot `{slot}` is already selected by `{other}`; one enum per page slot"
                ))
                .at(location(&value.file, value.item.span)),
            );
        }
    }
    let mut contributions = Contributions {
        namespaces: std::iter::once(protocol.namespace.clone())
            .chain(config.modules.iter().map(|module| module.id.clone()))
            .collect(),
        optional: config
            .modules
            .iter()
            .filter(|module| module.optional)
            .map(|module| module.id.clone())
            .collect(),
        ..Contributions::default()
    };
    for module in &config.modules {
        for page in &module.pages {
            let Some(enum_name) = page_enums.get(&page.slot) else {
                diagnostics.push(Diagnostic::error(format!(
                    "module `{}` adds page `{}` to slot `{}`, but no enum in the application's UI is marked `#[bern::value(pages = \"{}\")]`",
                    module.id, page.id, page.slot, page.slot
                )));
                continue;
            };
            let variant = ast::pascal_case(&page.id);
            let Some(def) = types.enums.iter_mut().find(|def| &def.name == enum_name) else {
                continue;
            };
            if def.variant_index(&variant).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "module `{}` adds page `{}`, but `{enum_name}` already has a variant `{variant}`",
                    module.id, page.id
                )));
                continue;
            }
            if config.strict {
                for language in &config.languages {
                    if !page.label.contains_key(language) {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "Missing translation\nProperty: label of page `{}` in {}\nMissing language: {language}",
                                page.id,
                                module.manifest.display()
                            ))
                            .code("B102"),
                        );
                    }
                }
            }
            def.variants.push(VariantDef {
                name: variant.clone(),
                fields: Vec::new(),
                label: Some(LocalizedText {
                    key: bern_protocol::label_key(enum_name, &variant),
                    translations: page.label.clone(),
                    params: Vec::new(),
                }),
            });
            contributions
                .pages
                .entry(page.slot.clone())
                .or_insert_with(|| PageSlot {
                    enum_name: enum_name.clone(),
                    pages: Vec::new(),
                })
                .pages
                .push(ContributedPage {
                    module: module.id.clone(),
                    variant,
                    label: page.label.clone(),
                    icon: page.icon.clone(),
                    badge: page.badge.clone(),
                    component: page.component.clone(),
                });
        }
        for (slot, placed) in &module.contributes {
            for component in placed {
                contributions
                    .slots
                    .entry(slot.clone())
                    .or_default()
                    .push((module.id.clone(), component.clone()));
            }
        }
    }

    // Session state.
    let mut sessions = Vec::new();
    for located in &sessions_ast {
        let session = &located.item;
        let name = session.name.to_string();
        if sessions.iter().any(|def: &ir::SessionDef| def.name == name)
            || types.enum_def(&name).is_some()
            || types.record_def(&name).is_some()
        {
            diagnostics.push(
                Diagnostic::error(format!("type `{name}` is declared more than once"))
                    .at(location(&located.file, session.span)),
            );
            continue;
        }
        let mut fields = Vec::new();
        for field in &session.fields {
            let ty = match resolve_type(&field.ty, &types) {
                Ok(ty) => ty,
                Err(message) => {
                    diagnostics
                        .push(Diagnostic::error(message).at(location(&located.file, field.span)));
                    continue;
                }
            };
            let initial = match &field.initial {
                Some(expr) => match initial_value(expr, &ty, &types) {
                    Ok(value) => value,
                    Err(message) => {
                        diagnostics.push(
                            Diagnostic::error(message).at(location(&located.file, field.span)),
                        );
                        continue;
                    }
                },
                None => Value::default_for(&ty, &types).unwrap_or(Value::Unit),
            };
            fields.push(ir::SessionFieldDef {
                name: field.name.to_string(),
                ty,
                initial,
            });
        }
        sessions.push(ir::SessionDef { name, fields });
    }

    // Root component and window handle.
    let Some(root) = components
        .iter()
        .find(|component| component.item.name == config.root)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "root component `{}` from [ui].root is not declared in the listed sources",
            config.root
        )));
        return Err(diagnostics);
    };
    let Some((handle, window_ids)) = root_windows(&root.item) else {
        diagnostics.push(
            Diagnostic::error(
                "the root component's body must be a `WindowSet(handle: …, primary: …)`",
            )
            .at(location(&root.file, root.item.span)),
        );
        return Err(diagnostics);
    };

    // Signatures, starting with library components.
    let mut signatures = BTreeMap::new();
    for component in libraries.iter().flat_map(|library| &library.components) {
        signatures.insert(
            component.name.clone(),
            Signature {
                name: component.name.clone(),
                module: component.module.clone(),
                params: component
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.kind.clone()))
                    .collect(),
            },
        );
    }
    for component in &components {
        let name = component.item.name.to_string();
        if signatures.contains_key(&name) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "component `{name}` is declared twice; component names are unique per application"
                ))
                .at(location(&component.file, component.item.span)),
            );
            continue;
        }
        let mut params = Vec::new();
        for param in &component.item.params {
            match param_kind(
                param,
                Some((&clients, &component.home)),
                Some(&handle),
                players_handle.as_deref(),
                &sessions,
                &types,
            ) {
                Ok(kind) => params.push((param.name.to_string(), kind)),
                Err(message) => diagnostics
                    .push(Diagnostic::error(message).at(location(&component.file, param.span))),
            }
        }
        signatures.insert(
            name.clone(),
            Signature {
                name,
                module: component.module.clone(),
                params,
            },
        );
    }

    // Every contributed component is declared by the module contributing it.
    for module in &config.modules {
        let placed = module
            .pages
            .iter()
            .map(|page| &page.component)
            .chain(module.contributes.values().flatten());
        for component in placed {
            let home = components
                .iter()
                .find(|located| located.item.name == component.as_str())
                .map(|located| located.home.as_str());
            if home != Some(module.id.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "module `{}` places component `{component}`, which its UI sources do not declare",
                    module.id
                )));
            }
        }
    }

    // Feature contracts and the components scoping them.
    let mut contract_features: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for located in &features_ast {
        let name = located.item.name.to_string();
        if contract_features.contains_key(&name) || signatures.contains_key(&name) {
            diagnostics.push(
                Diagnostic::error(format!("contract `{name}` is declared more than once"))
                    .at(location(&located.file, located.item.span)),
            );
            continue;
        }
        contract_features.insert(
            name,
            located
                .item
                .features
                .iter()
                .map(|feature| feature.name.to_string())
                .collect(),
        );
    }
    let mut contract_scopes: BTreeMap<String, String> = BTreeMap::new();
    for component in &components {
        let Some(path) = &component.item.features else {
            continue;
        };
        let contract = path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        let span = path.segments.last().map(|segment| segment.ident.span());
        let at = location(&component.file, span.unwrap_or(component.item.span));
        if !contract_features.contains_key(&contract) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`{contract}` is not a `#[bern::features]` contract in the listed sources"
                ))
                .at(at),
            );
        } else if let Some(other) = contract_scopes.get(&contract) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "contract `{contract}` already scopes component `{other}`; a contract has one scope"
                ))
                .at(at),
            );
        } else {
            contract_scopes.insert(contract, component.item.name.to_string());
        }
    }
    for located in &features_ast {
        let name = located.item.name.to_string();
        if !contract_scopes.contains_key(&name) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "contract `{name}` scopes no component; add `#[component(features = {name})]` to the screen it describes"
                ))
                .at(location(&located.file, located.item.span)),
            );
        }
    }

    // Components.
    let mut checker = Checker::new(
        &types,
        &protocol,
        &sessions,
        &signatures,
        &window_ids,
        &player_ids,
        &config.languages,
        &config.default_language,
        config.strict,
        &contract_features,
        config.layouts,
    );
    checker.contributions = contributions;
    let style = style::lower(&styles, &mut diagnostics);
    checker.variants = style::variants(&style);
    checker.transitions = style
        .transitions
        .iter()
        .map(|transition| transition.name.clone())
        .collect();
    checker.tokens = style
        .motion
        .iter()
        .map(|token| token.name.clone())
        .collect();
    checker.patterns = style
        .patterns
        .iter()
        .map(|pattern| pattern.name.clone())
        .collect();
    let mut lowered: Vec<ir::ComponentDef> = libraries
        .iter()
        .flat_map(|library| library.components.clone())
        .collect();
    let mut root_element = None;
    for component in &components {
        let name = component.item.name.to_string();
        let Some(signature) = signatures.get(&name) else {
            continue;
        };
        let Some(body) = checker.component(&component.file, &component.item, signature) else {
            continue;
        };
        if name == config.root {
            if let ir::Node::Element(element) = body {
                root_element = Some(element);
            }
            continue;
        }
        lowered.push(ir::ComponentDef {
            name,
            module: component.module.clone(),
            params: signature
                .params
                .iter()
                .map(|(name, kind)| ir::ParamDef {
                    name: name.clone(),
                    kind: kind.clone(),
                })
                .collect(),
            body,
            source: location(&component.file, component.item.span),
        });
    }
    diagnostics.append(&mut checker.diagnostics);
    let mut texts = std::mem::take(&mut checker.texts);
    for library in &libraries {
        texts.extend(library.texts.iter().cloned());
    }

    let mut graph = BTreeMap::new();
    for component in &lowered {
        let mut refs = Vec::new();
        component_refs(&component.body, &mut refs);
        graph.insert(component.name.clone(), refs);
    }
    if let Some(cycle) = find_cycle(&graph) {
        diagnostics.push(Diagnostic::error(format!(
            "components reference each other recursively ({}); recursive structures need a dedicated design",
            cycle.join(" → ")
        )));
    }
    if lowered.iter().any(|component| {
        let mut refs = Vec::new();
        component_refs(&component.body, &mut refs);
        refs.contains(&config.root)
    }) {
        diagnostics.push(Diagnostic::error(
            "the root component cannot be referenced by another component",
        ));
    }

    let windows = root_element
        .as_ref()
        .and_then(|root| window_catalog(root, handle, &graph, &signatures, &mut diagnostics));

    // The accent and appearance the windows show come from enums whose
    // variants name what they show.
    if let (Some(catalog), Some(root)) = (&windows, &root_element) {
        let variants = |expr: &ir::Expr| match &expr.ty {
            Type::Enum { name } => types.enum_def(name).map(|def| {
                def.variants
                    .iter()
                    .map(|v| v.name.clone())
                    .collect::<Vec<_>>()
            }),
            _ => None,
        };
        if let Some(accent) = &catalog.accent {
            match variants(accent) {
                Some(names) => {
                    for name in names.iter().filter(|name| style.accent(name).is_none()) {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "accent `{name}` is not declared; add it to the `accents` block of `style!`"
                            ))
                            .at(root.source.clone()),
                        );
                    }
                }
                None => diagnostics.push(
                    Diagnostic::error(
                        "`accent` is an enum whose variants name accents of `style!`",
                    )
                    .at(root.source.clone()),
                ),
            }
        }
        if let Some(appearance) = &catalog.appearance {
            let fits = variants(appearance).is_some_and(|names| {
                names
                    .iter()
                    .all(|name| ["system", "light", "dark"].contains(&name.to_lowercase().as_str()))
            });
            if !fits {
                diagnostics.push(
                    Diagnostic::error("`appearance` is an enum of `System`, `Light` and `Dark`")
                        .at(root.source.clone()),
                );
            }
        }
    }

    // Feature contracts: what each scope proves it reaches.
    let mut contracts: Vec<ir::FeatureContract> = features_ast
        .iter()
        .filter_map(|located| {
            let name = located.item.name.to_string();
            let component = contract_scopes.get(&name)?.clone();
            Some(features::lower(
                &features::Declared {
                    file: &located.file,
                    item: &located.item,
                },
                component,
                &protocol,
                &mut diagnostics,
            ))
        })
        .collect();
    features::prove(
        &mut contracts,
        &lowered,
        windows.as_ref(),
        &protocol,
        &mut diagnostics,
    );

    // A button's content is what it shows: no other control inside it,
    // also through the components it places.
    let bodies: BTreeMap<&str, &ir::Node> = lowered
        .iter()
        .map(|component| (component.name.as_str(), &component.body))
        .collect();
    let root_node = root_element.clone().map(ir::Node::Element);
    for body in bodies.values().copied().chain(root_node.as_ref()) {
        body.walk(&mut |node| {
            let ir::Node::Element(button) = node else {
                return;
            };
            if button.kind != "Button" || button.children.is_empty() {
                return;
            }
            if let Some(control) = control_inside(&button.children, &bodies) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "a `Button` cannot hold `{}`: one press would mean two things; put the `{}` beside the button, for example in a `Stack`",
                        control.kind, control.kind
                    ))
                    .at(control.source.clone()),
                );
            }
        });
    }

    // A toolbar's search field stands in the toolbar itself, not in its
    // conditions or repetitions: a window shows one or none.
    for body in bodies.values().copied().chain(root_node.as_ref()) {
        body.walk(&mut |node| {
            let ir::Node::Element(toolbar) = node else {
                return;
            };
            if toolbar.kind != "Toolbar" {
                return;
            }
            for child in &toolbar.children {
                if matches!(child, ir::Node::Element(_)) {
                    continue;
                }
                child.walk(&mut |node| {
                    if let ir::Node::Element(search) = node
                        && search.kind == "Search"
                    {
                        diagnostics.push(
                            Diagnostic::error(
                                "a `Search` stands directly in its `Toolbar`, not inside `if`, `match` or `ForEach`",
                            )
                            .at(search.source.clone()),
                        );
                    }
                });
            }
        });
    }

    // A divider in a toolbar parts two groups of items: it stands directly
    // in the toolbar, with an item before and after it.
    for body in bodies.values().copied().chain(root_node.as_ref()) {
        body.walk(&mut |node| {
            let ir::Node::Element(toolbar) = node else {
                return;
            };
            if toolbar.kind != "Toolbar" {
                return;
            }
            let mut nested = Vec::new();
            for child in &toolbar.children {
                if !matches!(child, ir::Node::Element(_)) {
                    toolbar_dividers(std::slice::from_ref(child), &mut nested);
                }
            }
            for divider in nested {
                diagnostics.push(
                    Diagnostic::error(
                        "a `Divider` parts the groups of a `Toolbar` and stands directly in it, not inside `if`, `match` or `ForEach`",
                    )
                    .at(divider.source.clone()),
                );
            }
            // Search fields have their own place; they part nothing.
            let items: Vec<&ir::Node> = toolbar
                .children
                .iter()
                .filter(|child| !matches!(child, ir::Node::Element(e) if e.kind == "Search"))
                .collect();
            for (index, child) in items.iter().enumerate() {
                let ir::Node::Element(divider) = child else {
                    continue;
                };
                if divider.kind != "Divider" {
                    continue;
                }
                let is_divider = |node: Option<&&ir::Node>| {
                    matches!(node, Some(ir::Node::Element(e)) if e.kind == "Divider")
                };
                if index == 0 || index + 1 == items.len() || is_divider(items.get(index + 1)) {
                    diagnostics.push(
                        Diagnostic::error(
                            "a `Divider` in a `Toolbar` parts two groups of items; put it between items",
                        )
                        .at(divider.source.clone()),
                    );
                }
            }
        });
    }

    // An element sits on a `Rail` at a fraction or between two; only
    // there. Containers stagger their children.
    for body in bodies.values().copied().chain(root_node.as_ref()) {
        let mut items = Vec::new();
        let mut components = Vec::new();
        body.walk(&mut |node| {
            if let ir::Node::Element(rail) = node
                && rail.kind == "Rail"
            {
                rail_items(&rail.children, &mut items, &mut components);
            }
        });
        for component in components {
            diagnostics.push(
                Diagnostic::error(format!(
                    "a `Rail` places elements by their `at` or `from` and `to`; put `{}` inside one, such as a `Column` with `at:`",
                    component.component
                ))
                .at(component.source.clone()),
            );
        }
        body.walk(&mut |node| {
            let ir::Node::Element(element) = node else {
                return;
            };
            let on_rail = items.iter().any(|item| std::ptr::eq(*item, element));
            let at = element.prop("at").is_some();
            let from = element.prop("from").is_some();
            let to = element.prop("to").is_some();
            let message = if on_rail {
                match (at, from, to) {
                    (true, false, false) | (false, true, true) => None,
                    (false, false, false) => Some(
                        "an element on a `Rail` needs `at:` (a fraction of the rail) or `from:` and `to:`",
                    ),
                    (true, _, _) => Some("write either `at:` or `from:` and `to:`, not both"),
                    _ => Some("`from:` and `to:` go together"),
                }
            } else if at || from || to {
                Some("`at`, `from` and `to` place an element on the `Rail` it stands in")
            } else {
                None
            };
            if let Some(message) = message {
                diagnostics.push(Diagnostic::error(message).at(element.source.clone()));
            }
            if element.prop("stagger").is_some()
                && !bern_ir::spec::STAGGERING.contains(&element.kind.as_str())
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "`stagger` spaces out the children of a container (`{}`); `{}` has none to space",
                        bern_ir::spec::STAGGERING.join("`, `"),
                        element.kind
                    ))
                    .at(element.source.clone()),
                );
            }
        });
    }

    // A side panel attaches to the container it stands in: directly there,
    // one per container, not inside a condition or repetition.
    for body in bodies.values().copied().chain(root_node.as_ref()) {
        let mut misplaced = Vec::new();
        inspectors(std::slice::from_ref(body), true, &mut misplaced);
        for (inspector, message) in misplaced {
            diagnostics.push(Diagnostic::error(message).at(inspector.source.clone()));
        }
    }

    // Pictures come from declared resources, in library components too.
    for component in &lowered {
        component.body.walk(&mut |node| {
            let ir::Node::Element(element) = node else {
                return;
            };
            for prop in element.props.values() {
                if let ir::Prop::Resource { name } = prop
                    && !config
                        .resources
                        .iter()
                        .any(|resource| &resource.name == name)
                {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "resource `{name}` is not declared; list it under [resources] in Bern.toml"
                        ))
                        .at(element.source.clone()),
                    );
                }
            }
        });
    }

    // Every label is a resource; keys must be unique.
    for def in &types.enums {
        for variant in &def.variants {
            if let Some(label) = &variant.label {
                texts.push(label.clone());
            }
        }
    }
    let name = LocalizedText {
        key: "app.name".into(),
        translations: config.name.clone(),
        params: vec![],
    };
    texts.push(name.clone());
    let mut keys = BTreeMap::new();
    texts.retain(|text| keys.insert(text.key.clone(), text.clone()).is_none());

    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }
    let Some(windows) = windows else {
        return Err(vec![Diagnostic::error(
            "the root WindowSet could not be lowered",
        )]);
    };
    Ok(Compiled {
        app: ir::Application {
            ir_version: IR_VERSION,
            app: ir::AppInfo {
                id: config.app_id.clone(),
                bundle_id: config.bundle_id.clone(),
                name,
                version: config.version.clone(),
                icon: config.icon.clone(),
                publisher: config.publisher.clone(),
            },
            languages: ir::Languages {
                supported: config.languages.clone(),
                default: config.default_language.clone(),
                strict: config.strict,
            },
            theme: ir::Theme {
                mode: config.theme_mode,
                brand_seed: config.brand_seed,
            },
            protocol,
            types,
            sessions,
            windows,
            players,
            components: lowered,
            texts,
            resources: config.resources.clone(),
            layouts: config.layouts,
            contracts,
            style,
        },
        protocol_module: protocol_module.join("::"),
        module_protocols,
        libraries,
        warnings: diagnostics,
    })
}

#[cfg(test)]
mod tests;
