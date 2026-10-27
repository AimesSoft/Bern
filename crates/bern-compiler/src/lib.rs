//! The Bern compiler front end.
//!
//! It reads the explicitly listed source files with the same parser the
//! macros use, checks commands, bindings, localization, windows and
//! component references, and produces the versioned [`bern_ir::Application`]
//! that independent target modules consume. It never runs application code.

mod check;
pub mod config;
mod features;
mod placement;
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
    read_selected(file, &bern_syntax::build::Modules::default())
}

fn read_selected(
    file: &SourceFile,
    modules: &bern_syntax::build::Modules,
) -> Result<ast::SourceItems, Vec<Diagnostic>> {
    let text = std::fs::read_to_string(&file.path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot read {}: {error}",
            file.path.display()
        ))]
    })?;
    bern_syntax::items::parse_source_file_with_modules(&text, modules)
        .map_err(|error| syn_error(&file.display, error))
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
    let date = bern_protocol::format::CALENDAR_DATE;
    if types.record_def(date).is_none()
        && mentioned.iter().any(|ty| bern_syntax::mentions(ty, date))
    {
        types
            .records
            .push(bern_protocol::format::calendar_date_def());
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
        if name == bern_media::MEDIA_SOURCE || name == bern_protocol::format::CALENDAR_DATE {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`{name}` is a framework type; use it from `bern::prelude` instead of declaring it",
                ))
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
            Ok(ty) => {
                let content = bern_syntax::lower::content(field, &ty).unwrap_or_else(|error| {
                    diagnostics.extend(syn_error(&value.file, error));
                    false
                });
                Some(FieldDef {
                    name: field.name.to_string(),
                    ty,
                    doc: field.doc.clone(),
                    content,
                })
            }
            Err(message) => {
                diagnostics.push(Diagnostic::error(message).at(location(&value.file, field.span)));
                None
            }
        };
        match &value.item.kind {
            ValueKind::Enum(variants) => {
                let mut defs = Vec::new();
                for variant in variants {
                    let fields: Vec<FieldDef> = variant
                        .fields
                        .iter()
                        .filter_map(|f| field(f, diagnostics))
                        .collect();
                    let label = variant.label.as_ref().and_then(|tr| {
                        for (span, message) in bern_syntax::lower::check_languages(tr, languages) {
                            diagnostics.push(shared_diagnostic(&value.file, span, &message));
                        }
                        bern_syntax::lower::label(&name, &variant.name.to_string(), tr, &fields)
                            .map_err(|error| diagnostics.extend(syn_error(&value.file, error)))
                            .ok()
                    });
                    defs.push(VariantDef {
                        name: variant.name.to_string(),
                        fields,
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

/// `#[content]` is for text parameters.
fn check_content_param(
    param: &ast::Param,
    kind: &ir::ParamKind,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !param.content {
        return;
    }
    match kind {
        ir::ParamKind::Value { ty } if bern_syntax::lower::is_text(ty) => {}
        _ => diagnostics.push(
            Diagnostic::error(format!(
                "`#[content]` marks text that is data, such as a title; `{}` is not text",
                param.name
            ))
            .at(location(file, param.span)),
        ),
    }
}

/// Names of a component's `#[content]` parameters.
fn content_params(component: &ast::ComponentItem) -> BTreeSet<String> {
    component
        .params
        .iter()
        .filter(|param| param.content)
        .map(|param| param.name.to_string())
        .collect()
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

/// Session state types the elements of `node` read and write themselves,
/// not those of the components it places.
fn read_sessions(node: &ir::Node) -> Vec<String> {
    let mut sessions = Vec::new();
    let mut exprs: Vec<&ir::Expr> = Vec::new();
    node.walk(&mut |node| match node {
        ir::Node::Element(element) => {
            for prop in element.props.values() {
                match prop {
                    ir::Prop::Text { text } => exprs.extend(text.exprs()),
                    ir::Prop::Expr { expr } => exprs.push(expr),
                    ir::Prop::Field { field } => sessions.push(field.session.clone()),
                    ir::Prop::Query { query } => exprs.extend(&query.args),
                    _ => {}
                }
            }
            match &element.action {
                Some(ir::Action::Engine { args, .. } | ir::Action::Media { args, .. }) => {
                    exprs.extend(args);
                }
                Some(ir::Action::Session { field, op }) => {
                    sessions.push(field.session.clone());
                    if let ir::SessionOp::Set { value } | ir::SessionOp::PushStr { value } = op {
                        exprs.push(value);
                    }
                }
                Some(ir::Action::Window { .. }) | None => {}
            }
        }
        ir::Node::If(node) => exprs.extend(node.branches.iter().map(|(when, _)| when)),
        ir::Node::IfLet(node) => exprs.push(&node.value),
        ir::Node::Match(node) => exprs.push(&node.value),
        ir::Node::Component(_) => {}
    });
    sessions.extend(exprs.into_iter().flat_map(ir::Expr::sessions));
    let mut seen = BTreeSet::new();
    sessions.retain(|session| seen.insert(session.clone()));
    sessions
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
        // Elements written directly in the window use its sessions too.
        for session in read_sessions(&content) {
            if !sessions.contains(&session) {
                sessions.push(session);
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
        if let Some(check) = &close
            && matches!(
                check.save,
                Some(
                    ir::Action::Window { .. }
                        | ir::Action::Media { .. }
                        | ir::Action::Session { .. }
                )
            )
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "window `{id}`: the save action must be an engine command"
                ))
                .at(window.source.clone()),
            );
        }
        // The close check and the window's own expressions read that
        // window instance's drafts.
        let mut reads: Vec<(String, &ir::Expr)> = close
            .iter()
            .flat_map(ir::CloseCheck::exprs)
            .map(|expr| ("close check".to_string(), expr))
            .collect();
        for name in ["immersive", "idle_enabled", "landscape"] {
            if let Some(expr) = window.expr(name) {
                reads.push((format!("`{name}`"), expr));
            }
        }
        for (what, expr) in reads {
            for session in expr.sessions() {
                if !sessions.contains(&session) {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "window `{id}`'s {what} reads `{session}`, which this window does not own; its components own: {}",
                            if sessions.is_empty() { "none".to_string() } else { sessions.join(", ") }
                        ))
                        .at(window.source.clone()),
                    );
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
            idle_enabled: window.expr("idle_enabled").cloned(),
            landscape: window.expr("landscape").cloned(),
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
    let bars: Vec<&ir::Element> = root
        .children
        .iter()
        .filter_map(|child| match child {
            ir::Node::Element(bar) if bar.kind == "MenuBar" => Some(bar),
            _ => None,
        })
        .collect();
    if let [_, second, ..] = bars.as_slice() {
        diagnostics.push(
            Diagnostic::error("a `WindowSet` has one `MenuBar`; put every menu in it")
                .at(second.source.clone()),
        );
    }
    let menu_bar = bars.first().map(|bar| (*bar).clone());
    // The menu bar's items read and write the primary window's drafts,
    // so that window owns them.
    if let Some(bar) = &menu_bar
        && let Some(window) = windows.iter_mut().find(|window| window.id == primary)
    {
        for session in read_sessions(&ir::Node::Element(bar.clone())) {
            if !window.sessions.contains(&session) {
                window.sessions.push(session);
            }
        }
    }
    let items: Vec<&ir::Element> = root
        .children
        .iter()
        .filter_map(|child| match child {
            ir::Node::Element(item) if item.kind == "StatusItem" => Some(item),
            _ => None,
        })
        .collect();
    if let [_, second, ..] = items.as_slice() {
        diagnostics.push(
            Diagnostic::error("a `WindowSet` has one `StatusItem`; put every item in its menu")
                .at(second.source.clone()),
        );
    }
    let status_item = items.first().map(|item| (*item).clone());
    if let Some(item) = &status_item {
        check_status_item(item, diagnostics);
    }
    Some(ir::WindowCatalog {
        handle,
        primary,
        windows,
        accent: root.expr("accent").cloned(),
        palette: root.expr("palette").cloned(),
        appearance: root.expr("appearance").cloned(),
        icon: root.expr("icon").cloned(),
        menu_bar,
        status_item,
    })
}

/// The `StatusItem`'s menu runs while no window is open: it reads engine
/// state and runs commands, and takes nothing that needs a window: drafts,
/// the window's own fields, files asked for in its open panel, shortcuts.
fn check_status_item(item: &ir::Element, diagnostics: &mut Vec<Diagnostic>) {
    let node = ir::Node::Element(item.clone());
    if let Some(session) = read_sessions(&node).first() {
        diagnostics.push(
            Diagnostic::error(format!(
                "the `StatusItem`'s menu runs without a window, so it has no drafts: read engine state and run commands instead of `{session}`"
            ))
            .at(item.source.clone()),
        );
    }
    node.walk(&mut |node| {
        let exprs: Vec<&ir::Expr> = match node {
            ir::Node::Element(element) => {
                for prop in ["pick", "shortcut"] {
                    if element.prop(prop).is_some() {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "a `CommandItem` of the `StatusItem` takes no `{prop}:`: its menu belongs to no window"
                            ))
                            .at(element.source.clone()),
                        );
                    }
                }
                element
                    .props
                    .values()
                    .flat_map(|prop| match prop {
                        ir::Prop::Text { text } => text.exprs(),
                        ir::Prop::Expr { expr } => vec![expr],
                        _ => Vec::new(),
                    })
                    .chain(match &element.action {
                        Some(ir::Action::Engine { args, .. } | ir::Action::Media { args, .. }) => {
                            args.iter().collect()
                        }
                        _ => Vec::new(),
                    })
                    .collect()
            }
            ir::Node::If(node) => node.branches.iter().map(|(when, _)| when).collect(),
            ir::Node::IfLet(node) => vec![&node.value],
            ir::Node::Match(node) => vec![&node.value],
            ir::Node::Component(_) => Vec::new(),
        };
        for expr in exprs {
            expr.walk(&mut |expr| {
                if let ir::ExprKind::Binding {
                    binding: ir::Binding::Window { field },
                } = &expr.kind
                    // Known when the target builds.
                    && !matches!(field.as_str(), "menu_bar" | "webview" | "tv")
                {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "the `StatusItem`'s menu belongs to no window, so it cannot read `window.{field}`"
                        ))
                        .at(item.source.clone()),
                    );
                }
            });
        }
    });
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
                Ok(kind) => {
                    check_content_param(param, &kind, &component.file, &mut diagnostics);
                    params.push((param.name.to_string(), kind));
                }
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
                content: content_params(&component.item),
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
                        content: signature.content.contains(name),
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

/// One `#[bern::protocol]` module and its descriptor, lowered by the
/// shared code.
struct ProtocolSource {
    /// Path of the protocol module inside its file.
    path: Vec<String>,
    item: ast::ProtocolItem,
    descriptor: ProtocolDescriptor,
}

impl ProtocolSource {
    /// Rust name of the protocol module.
    fn name(&self) -> String {
        self.item.module.ident.to_string()
    }

    /// Rust path of the protocol module, declared in `file` of `package`.
    fn rust_path(&self, package: &config::Package, file: &SourceFile) -> String {
        let mut path = vec![package.crate_name()];
        path.extend(file.module.iter().cloned());
        path.extend(self.path.iter().cloned());
        path.push(self.name());
        path.join("::")
    }
}

/// Read a protocol source, which holds exactly one protocol module.
fn read_protocol(
    file: &SourceFile,
    modules: &bern_syntax::build::Modules,
) -> Result<ProtocolSource, Vec<Diagnostic>> {
    let items = read_selected(file, modules)?;
    let Ok([(path, item)]) = <[_; 1]>::try_from(items.protocols) else {
        return Err(vec![Diagnostic::error(format!(
            "{} must contain exactly one #[bern::protocol] module",
            file.display
        ))]);
    };
    let descriptor = bern_syntax::lower::protocol_descriptor(&item)
        .map_err(|error| syn_error(&file.display, error))?;
    Ok(ProtocolSource {
        path,
        item,
        descriptor,
    })
}

/// Check the translations of the variant labels a protocol declares.
fn check_protocol_labels(
    item: &ast::ProtocolItem,
    file: &str,
    config: &Config,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let languages = Some((config.languages.as_slice(), config.strict));
    for value in &item.values {
        if let ValueKind::Enum(variants) = &value.kind {
            for tr in variants.iter().filter_map(|variant| variant.label.as_ref()) {
                for (span, message) in bern_syntax::lower::check_languages(tr, languages) {
                    diagnostics.push(shared_diagnostic(file, span, &message));
                }
            }
        }
    }
}

/// The application's protocol composed with its modules' into one
/// catalog, and the Rust paths of their protocol modules.
struct Protocols {
    protocol: ProtocolDescriptor,
    clients: Clients,
    /// Rust path of the application's protocol module.
    module: String,
    /// Rust paths of the modules' protocol modules, in application order.
    modules: Vec<String>,
}

/// Read the protocols of the application and its modules and compose
/// them. A protocol source that cannot be read stops compilation with its
/// own errors alone; a catalog that does not compose stops it with
/// everything reported so far.
fn compose_protocols(
    config: &Config,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Protocols, Vec<Diagnostic>> {
    let protocol_file = &config.protocol_source;
    let shell = read_protocol(protocol_file, &config.build_modules)?;
    check_protocol_labels(&shell.item, &protocol_file.display, config, diagnostics);
    let mut clients = Clients {
        protocols: vec![(shell.name(), shell.descriptor.namespace.clone())],
    };
    let mut module_descriptors = Vec::new();
    let mut module_protocols = Vec::new();
    for module in &config.modules {
        let file = &module.protocol_source;
        let source = read_protocol(file, &config.build_modules)?;
        if source.descriptor.namespace != module.id {
            diagnostics.push(Diagnostic::error(format!(
                "{}: the protocol of module `{}` must use `namespace = \"{}\"`, not `{}`",
                file.display, module.id, module.id, source.descriptor.namespace
            )));
        }
        check_protocol_labels(&source.item, &file.display, config, diagnostics);
        let name = source.name();
        if clients.protocols.iter().any(|(other, _)| *other == name) {
            diagnostics.push(Diagnostic::error(format!(
                "{}: protocol module `{name}` has the name of another protocol module; name it after the module",
                file.display
            )));
        }
        clients
            .protocols
            .push((name, source.descriptor.namespace.clone()));
        module_protocols.push(source.rust_path(&module.protocol_package, file));
        module_descriptors.push(source.descriptor);
    }
    check_event_routes(config, &shell.descriptor, &module_descriptors, diagnostics);
    let module = shell.rust_path(&config.protocol_package, protocol_file);
    let protocol = if module_descriptors.is_empty() {
        shell.descriptor
    } else {
        let parts: Vec<bern_protocol::ModulePart<'_>> = module_descriptors
            .iter()
            .zip(&config.modules)
            .map(|(descriptor, module)| bern_protocol::ModulePart {
                descriptor,
                optional: module.optional,
            })
            .collect();
        match compose(&shell.descriptor, &parts) {
            Ok(protocol) => protocol,
            Err(error) => {
                diagnostics.push(Diagnostic::error(error.0));
                return Err(std::mem::take(diagnostics));
            }
        }
    };
    Ok(Protocols {
        protocol,
        clients,
        module,
        modules: module_protocols,
    })
}

/// A module answers only the events of modules it depends on; the
/// application answers any module's events, but not its own.
fn check_event_routes(
    config: &Config,
    shell: &ProtocolDescriptor,
    modules: &[ProtocolDescriptor],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (descriptor, module) in modules.iter().zip(&config.modules) {
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
                config.protocol_source.display, command.id
            )));
        }
    }
}

/// Declarations of the UI sources of the application and its modules.
#[derive(Default)]
struct Sources {
    values: Vec<Located<ast::ValueItem>>,
    sessions: Vec<Located<ast::SessionItem>>,
    players: Vec<Located<ast::PlayersItem>>,
    components: Vec<Located<ast::ComponentItem>>,
    features: Vec<Located<ast::FeaturesItem>>,
    /// The application's `style!` items, with the file declaring each.
    styles: Vec<(String, ast::StyleItem)>,
}

/// Collect the declarations of every UI source: the application's, whose
/// namespace is `namespace`, then each module's. A source that cannot be
/// read is reported and skipped.
fn collect_sources(config: &Config, namespace: &str, diagnostics: &mut Vec<Diagnostic>) -> Sources {
    let mut sources = Sources::default();
    let files = config
        .ui_sources
        .iter()
        .map(|file| (file, namespace.to_string()))
        .chain(config.modules.iter().flat_map(|module| {
            module
                .ui_sources
                .iter()
                .map(move |file| (file, module.id.clone()))
        }));
    for (file, home) in files {
        let items = match read_selected(file, &config.build_modules) {
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
        sources.values.extend(locate(file, &home, items.values));
        sources.sessions.extend(locate(file, &home, items.sessions));
        sources.players.extend(locate(file, &home, items.players));
        sources
            .components
            .extend(locate(file, &home, items.components));
        for (_, item) in items.styles {
            if home == namespace {
                sources.styles.push((file.display.clone(), item));
            } else {
                diagnostics.push(Diagnostic::error(format!(
                    "{}: modules cannot declare `style!`; the application's interface declares one for every module",
                    file.display
                )));
            }
        }
        sources.features.extend(locate(file, &home, items.features));
    }
    sources
}

/// The `items` of `file`, a source of the application or module whose
/// namespace is `home`, each with the Rust module declaring it.
fn locate<T>(
    file: &SourceFile,
    home: &str,
    items: Vec<(Vec<String>, T)>,
) -> impl Iterator<Item = Located<T>> {
    items.into_iter().map(move |(path, item)| {
        let mut module = file.module.clone();
        module.extend(path);
        Located {
            file: file.display.clone(),
            module,
            home: home.to_string(),
            item,
        }
    })
}

/// The handle name and players of the `#[bern::players]` catalog; an
/// application declares at most one.
fn player_catalog(
    declared: &[Located<ast::PlayersItem>],
    diagnostics: &mut Vec<Diagnostic>,
) -> (Option<String>, Vec<ir::PlayerDef>) {
    for extra in declared.iter().skip(1) {
        diagnostics.push(
            Diagnostic::error(
                "an application declares at most one #[bern::players] catalog; add players as fields",
            )
            .at(location(&extra.file, extra.item.span)),
        );
    }
    let Some(catalog) = declared.first() else {
        return (None, Vec::new());
    };
    let players = catalog
        .item
        .players
        .iter()
        .map(|player| ir::PlayerDef {
            id: player.name.to_string(),
            doc: player.doc.clone(),
        })
        .collect();
    (Some(catalog.item.name.to_string()), players)
}

/// Component libraries: versioned descriptors generated from their
/// sources and checked against this application's configuration.
fn load_libraries(config: &Config, diagnostics: &mut Vec<Diagnostic>) -> Vec<ir::ComponentLibrary> {
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
    libraries
}

/// The application's type table: the protocol's types, the libraries'
/// and those the UI sources declare, with the framework types they use.
fn application_types(
    protocol: &ProtocolDescriptor,
    libraries: &[ir::ComponentLibrary],
    sources: &Sources,
    config: &Config,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypeTable {
    let mut base_types = protocol.types.clone();
    for library in libraries {
        if let Err(error) = base_types.merge(&library.types) {
            diagnostics.push(Diagnostic::error(format!(
                "library `{}`: {}",
                library.package, error.message
            )));
        }
    }
    let mentioned: Vec<&ast::TypeRef> = sources
        .values
        .iter()
        .flat_map(|value| value_types(&value.item))
        .chain(
            sources
                .sessions
                .iter()
                .flat_map(|session| session.item.fields.iter().map(|field| &field.ty)),
        )
        .chain(
            sources
                .components
                .iter()
                .flat_map(|component| component.item.params.iter().map(|param| &param.ty)),
        )
        .collect();
    add_framework_types(&mut base_types, &mentioned);
    merge_ui_types(
        &base_types,
        &sources.values,
        (&config.languages, config.strict),
        diagnostics,
    )
}

/// What the modules contribute to the application's slots. The enums
/// marked `pages = "slot"` gain a variant for every page the modules add
/// to that slot.
fn module_contributions(
    config: &Config,
    namespace: &str,
    values: &[Located<ast::ValueItem>],
    types: &mut TypeTable,
    diagnostics: &mut Vec<Diagnostic>,
) -> Contributions {
    let mut page_enums: BTreeMap<String, String> = BTreeMap::new();
    for value in values {
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
        namespaces: std::iter::once(namespace.to_string())
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
    // The application's own contributions come first, and always show.
    for (slot, placed) in config.contributes.iter().rev() {
        let entries = contributions.slots.entry(slot.clone()).or_default();
        for component in placed.iter().rev() {
            entries.insert(0, (namespace.to_string(), component.clone()));
        }
    }
    contributions
}

/// Lower the `#[bern::session]` structs, with each field's initial value.
fn lower_sessions(
    declared: &[Located<ast::SessionItem>],
    types: &TypeTable,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ir::SessionDef> {
    let mut sessions = Vec::new();
    for located in declared {
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
            let ty = match resolve_type(&field.ty, types) {
                Ok(ty) => ty,
                Err(message) => {
                    diagnostics
                        .push(Diagnostic::error(message).at(location(&located.file, field.span)));
                    continue;
                }
            };
            let initial = match &field.initial {
                Some(expr) => match initial_value(expr, &ty, types) {
                    Ok(value) => value,
                    Err(message) => {
                        diagnostics.push(
                            Diagnostic::error(message).at(location(&located.file, field.span)),
                        );
                        continue;
                    }
                },
                None => Value::default_for(&ty, types).unwrap_or(Value::Unit),
            };
            fields.push(ir::SessionFieldDef {
                name: field.name.to_string(),
                ty,
                initial,
            });
        }
        sessions.push(ir::SessionDef { name, fields });
    }
    sessions
}

/// The window handle name and window ids of the root component, whose
/// body is a `WindowSet`. `None`, once reported, when the root is not
/// declared or its body is something else.
fn root_window_set(
    config: &Config,
    components: &[Located<ast::ComponentItem>],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, Vec<String>)> {
    let Some(root) = components
        .iter()
        .find(|component| component.item.name == config.root)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "root component `{}` from [ui].root is not declared in the listed sources",
            config.root
        )));
        return None;
    };
    let Some(windows) = root_windows(&root.item) else {
        diagnostics.push(
            Diagnostic::error(
                "the root component's body must be a `WindowSet(handle: …, primary: …)`",
            )
            .at(location(&root.file, root.item.span)),
        );
        return None;
    };
    Some(windows)
}

/// The signature of every component, the libraries' first. `kind`
/// resolves a parameter, given the namespace of the application or
/// module declaring the component.
fn component_signatures(
    libraries: &[ir::ComponentLibrary],
    components: &[Located<ast::ComponentItem>],
    kind: impl Fn(&ast::Param, &str) -> Result<ir::ParamKind, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, Signature> {
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
                content: component
                    .params
                    .iter()
                    .filter(|param| param.content)
                    .map(|param| param.name.clone())
                    .collect(),
            },
        );
    }
    for component in components {
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
            match kind(param, &component.home) {
                Ok(kind) => {
                    check_content_param(param, &kind, &component.file, diagnostics);
                    params.push((param.name.to_string(), kind));
                }
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
                content: content_params(&component.item),
            },
        );
    }
    signatures
}

/// Every component the application places is declared by its own UI
/// sources, and every component a module places by that module's.
fn check_placed_components(
    config: &Config,
    namespace: &str,
    components: &[Located<ast::ComponentItem>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for component in config.contributes.values().flatten() {
        let home = components
            .iter()
            .find(|located| located.item.name == component.as_str())
            .map(|located| located.home.as_str());
        if home != Some(namespace) {
            diagnostics.push(Diagnostic::error(format!(
                "the application places component `{component}`, which its UI sources do not declare"
            )));
        }
    }
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
}

/// The feature names of each `#[bern::features]` contract, by contract.
/// A contract's name is unique among contracts and components.
fn declare_contracts(
    declared: &[Located<ast::FeaturesItem>],
    signatures: &BTreeMap<String, Signature>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, Vec<String>> {
    let mut contract_features: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for located in declared {
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
    contract_features
}

/// The component each contract scopes, by contract: every contract
/// scopes exactly one component, which names it in `features =`.
fn scope_contracts(
    sources: &Sources,
    contract_features: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, String> {
    let mut contract_scopes: BTreeMap<String, String> = BTreeMap::new();
    for component in &sources.components {
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
    for located in &sources.features {
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
    contract_scopes
}

/// Let `checker` check the names components take from `style!`.
fn use_style(checker: &mut Checker<'_>, style: &ir::style::StyleSheet) {
    checker.variants = style::variants(style);
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
    checker.corners = style
        .corners
        .iter()
        .filter(|corner| corner.targets.is_empty())
        .map(|corner| (corner.name.clone(), corner.radius))
        .collect();
}

/// Check and lower the application's components, after the libraries'.
/// The root's body comes back on its own: it becomes the window catalog,
/// not a component.
fn lower_components(
    checker: &mut Checker<'_>,
    config: &Config,
    components: &[Located<ast::ComponentItem>],
    libraries: &[ir::ComponentLibrary],
) -> (Vec<ir::ComponentDef>, Option<ir::Element>) {
    let signatures = checker.signatures;
    let mut lowered: Vec<ir::ComponentDef> = libraries
        .iter()
        .flat_map(|library| library.components.clone())
        .collect();
    let mut root_element = None;
    for component in components {
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
                    content: signature.content.contains(name),
                })
                .collect(),
            body,
            source: location(&component.file, component.item.span),
        });
    }
    (lowered, root_element)
}

/// A component that shows a page reads the session holding the selection,
/// as if it declared it.
fn add_page_sessions(
    page_sessions: BTreeSet<(String, String)>,
    lowered: &mut [ir::ComponentDef],
    signatures: &mut BTreeMap<String, Signature>,
) {
    for (name, session) in page_sessions {
        let kind = ir::ParamKind::Session { session };
        if let Some(component) = lowered.iter_mut().find(|component| component.name == name)
            && !component.params.iter().any(|param| param.kind == kind)
        {
            component.params.push(ir::ParamDef {
                name: "bern_pages".into(),
                kind: kind.clone(),
                content: false,
            });
        }
        if let Some(signature) = signatures.get_mut(&name)
            && !signature
                .params
                .iter()
                .any(|(_, declared)| *declared == kind)
        {
            signature.params.push(("bern_pages".into(), kind));
        }
    }
}

/// The components each lowered component places. Components cannot place
/// each other recursively, and neither they nor the windows of the `root`
/// window set place the root.
fn component_graph(
    components: &[ir::ComponentDef],
    root: Option<&ir::Element>,
    root_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, Vec<String>> {
    let mut graph = BTreeMap::new();
    for component in components {
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
    let mut windows = Vec::new();
    if let Some(root) = root {
        component_refs(&ir::Node::Element(root.clone()), &mut windows);
    }
    if graph
        .values()
        .chain([&windows])
        .any(|refs| refs.iter().any(|name| name == root_name))
    {
        diagnostics.push(Diagnostic::error(
            "the root component cannot be placed by a component or a window",
        ));
    }
    graph
}

/// The accent and appearance the windows show come from enums whose
/// variants name what they show.
fn check_accent_and_appearance(
    catalog: &ir::WindowCatalog,
    root: &ir::Element,
    types: &TypeTable,
    style: &ir::style::StyleSheet,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if catalog
        .palette
        .as_ref()
        .is_some_and(|value| value.ty != Type::Text)
    {
        diagnostics.push(
            Diagnostic::error("`palette` is a text binding containing semantic color pairs")
                .at(root.source.clone()),
        );
    }
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
                Diagnostic::error("`accent` is an enum whose variants name accents of `style!`")
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

/// The icon the system shows comes from an enum: `Default` is `[app.icon]`,
/// every other variant names an icon of `[app.alternate_icons]`.
fn check_icon_choice(
    catalog: &ir::WindowCatalog,
    root: &ir::Element,
    types: &TypeTable,
    alternates: &BTreeMap<String, BTreeMap<String, String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(icon) = &catalog.icon else {
        return;
    };
    let variants = match &icon.ty {
        Type::Enum { name } => types.enum_def(name).map(|def| {
            def.variants
                .iter()
                .map(|variant| variant.name.clone())
                .collect::<Vec<_>>()
        }),
        _ => None,
    };
    let Some(variants) = variants else {
        diagnostics.push(
            Diagnostic::error(
                "`icon` is an enum: `Default` shows `[app.icon]`, and every other variant names an icon of `[app.alternate_icons]`",
            )
            .at(root.source.clone()),
        );
        return;
    };
    for variant in variants.iter().filter(|variant| *variant != "Default") {
        let name = bern_target_api::icon::alternate_name(variant);
        if !alternates.contains_key(&name) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "icon `{variant}` names `[app.alternate_icons.{name}]` in Bern.toml, which is not declared"
                ))
                .at(root.source.clone()),
            );
        }
    }
}

/// Lower the feature contracts that scope a component and prove what each
/// scope reaches.
fn prove_contracts(
    declared: &[Located<ast::FeaturesItem>],
    contract_scopes: &BTreeMap<String, String>,
    components: &[ir::ComponentDef],
    windows: Option<&ir::WindowCatalog>,
    protocol: &ProtocolDescriptor,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ir::FeatureContract> {
    let mut contracts: Vec<ir::FeatureContract> = declared
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
                protocol,
                diagnostics,
            ))
        })
        .collect();
    features::prove(&mut contracts, components, windows, protocol, diagnostics);
    contracts
}

/// Pictures come from declared resources, in library components and in
/// the `root` window set too.
fn check_resources(
    components: &[ir::ComponentDef],
    root: Option<&ir::Element>,
    resources: &[ir::ResourceDef],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = root.cloned().map(ir::Node::Element);
    let trees = components
        .iter()
        .map(|component| &component.body)
        .chain(root.as_ref());
    for tree in trees {
        tree.walk(&mut |node| {
            let ir::Node::Element(element) = node else {
                return;
            };
            for prop in element.props.values() {
                if let ir::Prop::Resource { name } = prop
                    && !resources.iter().any(|resource| &resource.name == name)
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
}

/// Every text the application shows: the components', the libraries',
/// every variant label and the application's `name`. Each key names one
/// text, the first given.
fn collect_texts(
    mut texts: Vec<LocalizedText>,
    libraries: &[ir::ComponentLibrary],
    types: &TypeTable,
    name: &LocalizedText,
) -> Vec<LocalizedText> {
    for library in libraries {
        texts.extend(library.texts.iter().cloned());
    }
    // Every label is a resource.
    for def in &types.enums {
        for variant in &def.variants {
            if let Some(label) = &variant.label {
                texts.push(label.clone());
            }
        }
    }
    texts.push(name.clone());
    let mut keys = BTreeMap::new();
    texts.retain(|text| keys.insert(text.key.clone(), text.clone()).is_none());
    texts
}

/// Collect, check and lower a project to IR.
pub fn compile(config: &Config) -> Result<Compiled, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // Declarations: protocols, UI sources, libraries, types and state.
    let Protocols {
        protocol,
        clients,
        module: protocol_module,
        modules: module_protocols,
    } = compose_protocols(config, &mut diagnostics)?;
    let sources = collect_sources(config, &protocol.namespace, &mut diagnostics);
    let (players_handle, players) = player_catalog(&sources.players, &mut diagnostics);
    let player_ids: Vec<String> = players.iter().map(|player| player.id.clone()).collect();
    let libraries = load_libraries(config, &mut diagnostics);
    let mut types = application_types(&protocol, &libraries, &sources, config, &mut diagnostics);
    let contributions = module_contributions(
        config,
        &protocol.namespace,
        &sources.values,
        &mut types,
        &mut diagnostics,
    );
    let sessions = lower_sessions(&sources.sessions, &types, &mut diagnostics);
    let Some((handle, window_ids)) = root_window_set(config, &sources.components, &mut diagnostics)
    else {
        return Err(diagnostics);
    };
    let mut signatures = component_signatures(
        &libraries,
        &sources.components,
        |param, home| {
            param_kind(
                param,
                Some((&clients, home)),
                Some(&handle),
                players_handle.as_deref(),
                &sessions,
                &types,
            )
        },
        &mut diagnostics,
    );
    check_placed_components(
        config,
        &protocol.namespace,
        &sources.components,
        &mut diagnostics,
    );
    let contract_features = declare_contracts(&sources.features, &signatures, &mut diagnostics);
    let contract_scopes = scope_contracts(&sources, &contract_features, &mut diagnostics);

    // Components, checked against those declarations.
    let style = style::lower(&sources.styles, &mut diagnostics);
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
    use_style(&mut checker, &style);
    let (mut lowered, root_element) =
        lower_components(&mut checker, config, &sources.components, &libraries);
    diagnostics.append(&mut checker.diagnostics);
    let texts = std::mem::take(&mut checker.texts);
    let page_sessions = std::mem::take(&mut checker.page_sessions);
    add_page_sessions(page_sessions, &mut lowered, &mut signatures);

    // Checks across the lowered components and the windows they fill.
    let graph = component_graph(
        &lowered,
        root_element.as_ref(),
        &config.root,
        &mut diagnostics,
    );
    let windows = root_element
        .as_ref()
        .and_then(|root| window_catalog(root, handle, &graph, &signatures, &mut diagnostics));
    if let (Some(catalog), Some(root)) = (&windows, &root_element) {
        check_accent_and_appearance(catalog, root, &types, &style, &mut diagnostics);
        check_icon_choice(
            catalog,
            root,
            &types,
            &config.alternate_icons,
            &mut diagnostics,
        );
    }
    let contracts = prove_contracts(
        &sources.features,
        &contract_scopes,
        &lowered,
        windows.as_ref(),
        &protocol,
        &mut diagnostics,
    );
    placement::check(&lowered, root_element.as_ref(), &mut diagnostics);
    check_resources(
        &lowered,
        root_element.as_ref(),
        &config.resources,
        &mut diagnostics,
    );

    let name = LocalizedText {
        key: "app.name".into(),
        translations: config.name.clone(),
        params: vec![],
    };
    let texts = collect_texts(texts, &libraries, &types, &name);
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
                platform_icons: config.platform_icons.clone(),
                alternate_icons: config.alternate_icons.clone(),
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
        protocol_module,
        module_protocols,
        libraries,
        warnings: diagnostics,
    })
}

#[cfg(test)]
mod tests;
