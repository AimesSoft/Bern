//! The Bern compiler front end.
//!
//! It reads the explicitly listed source files with the same parser the
//! macros use, checks commands, bindings, localization, windows and
//! component references, and produces the versioned [`bern_ir::Application`]
//! that independent target modules consume. It never runs application code.

mod check;
pub mod config;

pub use check::{Signature, location};
pub use config::{Config, SourceFile};

use bern_ir::{self as ir, IR_VERSION};
use bern_protocol::{
    EnumDef, FieldDef, LocalizedText, ProtocolDescriptor, RecordDef, Type, TypeTable, Value,
    VariantDef,
};
use bern_syntax::{self as ast, ValueKind};
use bern_target_api::Diagnostic;
use check::{Checker, shared_diagnostic};
use std::collections::{BTreeMap, BTreeSet};

/// Result of a successful compilation.
pub struct Compiled {
    pub app: ir::Application,
    /// Rust path of the protocol module, e.g. `showcase_protocol::showcase`.
    pub protocol_module: String,
    pub warnings: Vec<Diagnostic>,
}

struct Located<T> {
    file: String,
    module: Vec<String>,
    item: T,
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

fn merge_ui_types(
    protocol: &ProtocolDescriptor,
    values: &[Located<ast::ValueItem>],
    config: &Config,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypeTable {
    let mut types = protocol.types.clone();
    let protocol_names: BTreeSet<String> = types
        .enums
        .iter()
        .map(|def| def.name.clone())
        .chain(types.records.iter().map(|def| def.name.clone()))
        .collect();
    let mut seen = BTreeSet::new();
    for value in values {
        let name = value.item.name.to_string();
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
    let languages = Some((config.languages.as_slice(), config.strict));
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
    protocol_module: &str,
    windows_handle: Option<&str>,
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
                    && (path.len() == 1 || path[path.len() - 2] == protocol_module) =>
            {
                Ok(ir::ParamKind::Client)
            }
            ast::TypeRef::Named(_) if Some(name.as_str()) == windows_handle => {
                Ok(ir::ParamKind::Windows)
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
            "native_required" => ir::Presentation::NativeRequired,
            _ => ir::Presentation::Auto,
        };
        let role = match keyword("role", "standard").as_str() {
            "settings" => ir::WindowRole::Settings,
            _ => ir::WindowRole::Standard,
        };
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
    })
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
    let protocol = bern_syntax::lower::protocol_descriptor(protocol_item)
        .map_err(|error| syn_error(&protocol_file.display, error))?;
    let protocol_module_name = protocol_item.module.ident.to_string();
    let mut protocol_module = vec![config.protocol_package.crate_name()];
    protocol_module.extend(protocol_file.module.iter().cloned());
    protocol_module.extend(protocol_path.iter().cloned());
    protocol_module.push(protocol_module_name.clone());
    let languages = Some((config.languages.as_slice(), config.strict));
    for value in &protocol_item.values {
        if let ValueKind::Enum(variants) = &value.kind {
            for tr in variants.iter().filter_map(|variant| variant.label.as_ref()) {
                for (span, message) in bern_syntax::lower::check_translations(tr, languages) {
                    diagnostics.push(shared_diagnostic(&protocol_file.display, span, &message));
                }
            }
        }
    }

    // UI sources.
    let mut values = Vec::new();
    let mut sessions_ast = Vec::new();
    let mut components = Vec::new();
    for file in &config.ui_sources {
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
                item,
            });
        }
        for (path, item) in items.sessions {
            sessions_ast.push(Located {
                file: file.display.clone(),
                module: module(&path),
                item,
            });
        }
        for (path, item) in items.components {
            components.push(Located {
                file: file.display.clone(),
                module: module(&path),
                item,
            });
        }
    }

    let types = merge_ui_types(&protocol, &values, config, &mut diagnostics);

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

    // Signatures.
    let mut signatures = BTreeMap::new();
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
                &protocol_module_name,
                Some(&handle),
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

    // Components.
    let mut checker = Checker::new(
        &types,
        &protocol,
        &sessions,
        &signatures,
        &window_ids,
        &config.languages,
        &config.default_language,
        config.strict,
    );
    let mut lowered = Vec::new();
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
            components: lowered,
            texts,
        },
        protocol_module: protocol_module.join("::"),
        warnings: diagnostics,
    })
}

#[cfg(test)]
mod tests;
