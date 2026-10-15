//! Semantic checking of component declarations and lowering to IR.

use bern_ir::spec::{self, ChildShape, ElementShape, PropKind};
use bern_ir::{self as ir, SourceLocation, text_key};
use bern_protocol::{LocalizedText, ProtocolDescriptor, Type, TypeTable, Value};
use bern_syntax::{self as ast, BinaryOp, Literal, UnaryOp};
use bern_target_api::Diagnostic;
use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};

/// Parameters of a component as seen by its callers.
#[derive(Clone, Debug)]
pub struct Signature {
    pub name: String,
    pub module: Vec<String>,
    pub params: Vec<(String, ir::ParamKind)>,
}

#[derive(Clone, Debug)]
enum Root {
    /// A typed engine client, by the namespace of its protocol.
    Client(String),
    Windows,
    Players,
    Session(String),
    Local(Type),
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

pub struct PageSlot {
    pub enum_name: String,
    pub pages: Vec<ContributedPage>,
}

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
const CODES: [&str; 7] = ["B101", "B102", "B103", "B104", "B301", "B401", "B402"];

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
                ir::ParamKind::Value { ty } => Root::Local(ty.clone()),
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

    fn nodes(
        &mut self,
        nodes: &[ast::Node],
        prefix: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Vec<ir::Node> {
        let mut lowered = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let path = format!("{prefix}.{index}");
            match node {
                ast::Node::Element(element)
                    if matches!(
                        element.name.to_string().as_str(),
                        "Slot" | "Pages" | "PageContent"
                    ) =>
                {
                    lowered.extend(self.expand(element, &path, ctx, scope));
                }
                node => lowered.extend(self.node(node, &path, ctx, scope)),
            }
        }
        lowered
    }

    /// Replace a `Slot`, `Pages` or `PageContent` with what the modules
    /// contribute, written as ordinary declarations and checked like them.
    fn expand(
        &mut self,
        element: &ast::Element,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Vec<ir::Node> {
        let name = element.name.to_string();
        let prop_names: Vec<String> = element
            .props
            .iter()
            .map(|prop| prop.name.to_string())
            .collect();
        let shape = ElementShape {
            name: &name,
            props: prop_names.iter().map(String::as_str).collect(),
            has_template: element.template.is_some(),
            children: element.children.as_ref().map(|_| Vec::new()),
            has_action: element.action.is_some(),
            parent: ctx.parent.as_deref(),
            is_root: ctx.is_root,
        };
        let errors = spec::check_shape(&shape);
        if !errors.is_empty() {
            for error in errors {
                self.error(element.span, error.message);
            }
            return Vec::new();
        }
        let ident = |prop: &str| match element.prop(prop).map(|prop| &prop.value) {
            Some(ast::Expr::Path(path)) => path.single().map(ToString::to_string),
            _ => None,
        };
        let key = if name == "Slot" { "id" } else { "slot" };
        let Some(slot) = ident(key) else {
            self.error(
                element.span,
                format!("`{name}` names its slot with `{key}: name`"),
            );
            return Vec::new();
        };
        let text = match name.as_str() {
            "Slot" => {
                let contributed = self
                    .contributions
                    .slots
                    .get(&slot)
                    .cloned()
                    .unwrap_or_default();
                let mut calls = Vec::new();
                for (module, component) in &contributed {
                    match self.slot_call(component, scope) {
                        Ok(call) => calls.push(self.contributions.guarded(module, call)),
                        Err(message) => self.error(element.span, message),
                    }
                }
                calls.join("\n")
            }
            "Pages" => {
                let Some(pages) = self.contributions.pages.get(&slot) else {
                    return Vec::new();
                };
                pages
                    .pages
                    .iter()
                    .map(|page| {
                        let label: Vec<String> = page
                            .label
                            .iter()
                            .map(|(language, text)| format!("{language:?} => {text:?}"))
                            .collect();
                        let icon = page
                            .icon
                            .as_ref()
                            .map(|icon| format!(", icon: {icon:?}"))
                            .unwrap_or_default();
                        let badge = page.badge.as_ref().map(|field| format!(", badge: {}.{field}", slot_client(&page.module))).unwrap_or_default();
                        self.contributions.guarded(
                            &page.module,
                            format!(
                                "SidebarItem(value: {}::{}, label: tr! {{ {} }}{icon}{badge})",
                                pages.enum_name,
                                page.variant,
                                label.join(", ")
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => {
                let selected = match element.prop("selected").map(|prop| &prop.value) {
                    Some(ast::Expr::Path(path)) => path.dotted(),
                    _ => {
                        self.error(
                            element.span,
                            "`selected:` binds the session field holding the page",
                        );
                        return Vec::new();
                    }
                };
                let Some(pages) = self.contributions.pages.get(&slot) else {
                    return Vec::new();
                };
                let mut arms = Vec::new();
                let mut failures = Vec::new();
                for page in &pages.pages {
                    match self.slot_call(&page.component, scope) {
                        Ok(call) => arms.push(format!(
                            "{}::{} => {{ {} }}",
                            pages.enum_name,
                            page.variant,
                            self.contributions.guarded(&page.module, call)
                        )),
                        Err(message) => failures.push(message),
                    }
                }
                for message in failures {
                    self.error(element.span, message);
                }
                if arms.is_empty() {
                    return Vec::new();
                }
                format!("match {selected} {{ {} _ => {{}} }}", arms.join(" "))
            }
        };
        if text.is_empty() {
            return Vec::new();
        }
        let parsed = match text
            .parse::<proc_macro2::TokenStream>()
            .map_err(|error| error.to_string())
            .and_then(|tokens| ast::parse_ui(tokens).map_err(|error| error.to_string()))
        {
            Ok(body) => body,
            Err(error) => {
                self.error(element.span, format!("cannot expand `{name}`: {error}"));
                return Vec::new();
            }
        };
        let pushed = self.contributions.namespaces.clone();
        for namespace in &pushed {
            scope.push(slot_client(namespace), Root::Client(namespace.clone()));
        }
        let lowered = self.nodes(&parsed.nodes, &format!("{path}.{name}"), ctx, scope);
        for _ in &pushed {
            scope.pop();
        }
        lowered
    }

    /// The call of a contributed component: its clients from the slot's
    /// scope, the window handle and players by the names the shell gave
    /// them; session state comes from the hosting window as usual.
    fn slot_call(&self, component: &str, scope: &Scope) -> Result<String, String> {
        let signature = self
            .signatures
            .get(component)
            .ok_or_else(|| format!("contributed component `{component}` is not declared"))?;
        let mut args = Vec::new();
        for (param, kind) in &signature.params {
            match kind {
                ir::ParamKind::Client { namespace } => {
                    args.push(format!("{param}: {}", slot_client(namespace)));
                }
                ir::ParamKind::Windows => {
                    let name = scope
                        .name_of(|root| matches!(root, Root::Windows))
                        .ok_or_else(|| format!("contributed component `{component}` takes the window handle; give the component holding the slot a `windows` parameter"))?;
                    args.push(format!("{param}: {name}"));
                }
                ir::ParamKind::Players => {
                    let name = scope
                        .name_of(|root| matches!(root, Root::Players))
                        .ok_or_else(|| format!("contributed component `{component}` takes the players; give the component holding the slot a `players` parameter"))?;
                    args.push(format!("{param}: {name}"));
                }
                ir::ParamKind::Session { .. } => {}
                ir::ParamKind::Value { .. } => {
                    return Err(format!(
                        "contributed component `{component}` takes the value `{param}`; components placed in slots take clients, the window handle, players and session state only"
                    ));
                }
            }
        }
        Ok(format!("{component}({})", args.join(", ")))
    }

    fn node(
        &mut self,
        node: &ast::Node,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Option<ir::Node> {
        let inner = Ctx {
            is_root: false,
            ..ctx.clone()
        };
        match node {
            ast::Node::Element(element) => self
                .element(element, path, ctx, scope)
                .map(ir::Node::Element),
            ast::Node::Component(call) => self.component_call(call, scope).map(ir::Node::Component),
            ast::Node::If(node) => {
                let mut branches = Vec::new();
                for (index, (condition, body)) in node.branches.iter().enumerate() {
                    let condition = self.expect(condition, &Type::Bool, scope, "a condition");
                    let body = self.nodes(body, &format!("{path}.if{index}"), &inner, scope);
                    if let Some(condition) = condition {
                        branches.push((condition, body));
                    }
                }
                let otherwise = node
                    .otherwise
                    .as_ref()
                    .map(|body| self.nodes(body, &format!("{path}.else"), &inner, scope))
                    .unwrap_or_default();
                Some(ir::Node::If(ir::IfNode {
                    branches,
                    otherwise,
                }))
            }
            ast::Node::IfLet(node) => {
                let value = self.expr(&node.value, scope)?;
                let Type::Optional { inner: inner_ty } = value.ty.clone() else {
                    self.error(
                        node.value.span(),
                        format!(
                            "`if let Some(…)` needs an optional value, found {}",
                            value.ty.display()
                        ),
                    );
                    return None;
                };
                scope.push(node.binding.to_string(), Root::Local((*inner_ty).clone()));
                let then = self.nodes(&node.then, &format!("{path}.then"), &inner, scope);
                scope.pop();
                let otherwise = node
                    .otherwise
                    .as_ref()
                    .map(|body| self.nodes(body, &format!("{path}.else"), &inner, scope))
                    .unwrap_or_default();
                Some(ir::Node::IfLet(ir::IfLetNode {
                    var: node.binding.to_string(),
                    value,
                    inner: *inner_ty,
                    then,
                    otherwise,
                }))
            }
            ast::Node::Match(node) => self
                .match_node(node, path, &inner, scope)
                .map(ir::Node::Match),
        }
    }

    fn match_node(
        &mut self,
        node: &ast::MatchNode,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Option<ir::MatchNode> {
        let value = self.expr(&node.value, scope)?;
        let Type::Enum { name } = value.ty.clone() else {
            self.error(
                node.value.span(),
                format!("`match` needs an enum value, found {}", value.ty.display()),
            );
            return None;
        };
        let def = self.types.enum_def(&name)?.clone();
        let mut covered = BTreeSet::new();
        let mut wildcard = false;
        let mut arms = Vec::new();
        for (index, arm) in node.arms.iter().enumerate() {
            let arm_path = format!("{path}.arm{index}");
            match &arm.pattern {
                ast::Pattern::Wildcard(span) => {
                    if wildcard {
                        self.error(*span, "`_` appears twice");
                    }
                    wildcard = true;
                    let body = self.nodes(&arm.body, &arm_path, ctx, scope);
                    arms.push(ir::MatchArm {
                        variant: None,
                        bindings: vec![],
                        body,
                    });
                }
                ast::Pattern::Variant {
                    ty,
                    variant,
                    bindings,
                } => {
                    if *ty != name {
                        self.error(
                            ty.span(),
                            format!("pattern type `{ty}` does not match the matched `{name}`"),
                        );
                        continue;
                    }
                    let Some(index) = def.variant_index(&variant.to_string()) else {
                        self.error(
                            variant.span(),
                            format!("`{name}` has no variant `{variant}`"),
                        );
                        continue;
                    };
                    if !covered.insert(index) {
                        self.error(
                            variant.span(),
                            format!("`{name}::{variant}` is matched twice"),
                        );
                    }
                    let fields = &def.variants[index].fields;
                    let mut lowered = Vec::new();
                    for binding in bindings {
                        match fields.iter().position(|field| *binding == field.name) {
                            Some(field) => lowered.push(ir::MatchBinding {
                                name: binding.to_string(),
                                field,
                                ty: fields[field].ty.clone(),
                            }),
                            None => self.error(
                                binding.span(),
                                format!("`{name}::{variant}` has no field `{binding}`"),
                            ),
                        }
                    }
                    for binding in &lowered {
                        scope.push(binding.name.clone(), Root::Local(binding.ty.clone()));
                    }
                    let body = self.nodes(&arm.body, &arm_path, ctx, scope);
                    for _ in &lowered {
                        scope.pop();
                    }
                    arms.push(ir::MatchArm {
                        variant: Some(index as u32),
                        bindings: lowered,
                        body,
                    });
                }
            }
        }
        if !wildcard && covered.len() < def.variants.len() {
            let missing: Vec<_> = def
                .variants
                .iter()
                .enumerate()
                .filter(|(index, _)| !covered.contains(index))
                .map(|(_, variant)| format!("{name}::{}", variant.name))
                .collect();
            self.error(
                node.span,
                format!("non-exhaustive match; missing {}", missing.join(", ")),
            );
        }
        Some(ir::MatchNode {
            value,
            enum_name: name,
            arms,
        })
    }

    fn element(
        &mut self,
        element: &ast::Element,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Option<ir::Element> {
        let name = element.name.to_string();
        let prop_names: Vec<String> = element
            .props
            .iter()
            .map(|prop| prop.name.to_string())
            .collect();
        // Items of navigation containers may be conditional or repeated:
        // what counts is the elements in each branch and template.
        let child_names: Option<Vec<Option<String>>> = element.children.as_ref().map(|children| {
            ast::child_names(children, spec::DYNAMIC_ITEMS.contains(&name.as_str()))
        });
        let shape = ElementShape {
            name: &name,
            props: prop_names.iter().map(String::as_str).collect(),
            has_template: element.template.is_some(),
            children: child_names.as_ref().map(|children| {
                children
                    .iter()
                    .map(|child| match child {
                        Some(name) => ChildShape::Element(name),
                        None => ChildShape::Other,
                    })
                    .collect()
            }),
            has_action: element.action.is_some(),
            parent: ctx.parent.as_deref(),
            is_root: ctx.is_root,
        };
        let shape_errors = spec::check_shape(&shape);
        let failed = !shape_errors.is_empty();
        for error in shape_errors {
            let span = error
                .prop
                .as_ref()
                .and_then(|prop| element.prop(prop))
                .map(|prop| prop.name.span())
                .unwrap_or(element.span);
            self.error(span, error.message);
        }
        let component = spec::component(&name)?;
        if failed {
            return None;
        }

        let mut props = BTreeMap::new();
        let mut id = None;
        let mut template = None;
        let mut child_ctx = Ctx {
            // A repetition places its template where it stands itself.
            parent: if name == "ForEach" {
                ctx.parent.clone()
            } else {
                Some(name.clone())
            },
            is_root: false,
            selection: ctx.selection.clone(),
            route: ctx.route.clone(),
        };
        let mut pushed = 0;

        for prop in &element.props {
            let prop_name = prop.name.to_string();
            let Some(prop_spec) = component.prop(&prop_name) else {
                continue;
            };
            match prop_spec.kind {
                PropKind::SelectionField => {
                    if let Some(field) =
                        self.session_field(&prop.value, scope, FieldExpect::Selection)
                    {
                        child_ctx.selection = Some(field.ty.clone());
                        props.insert(prop_name, ir::Prop::Field { field });
                    }
                }
                PropKind::PathField => {
                    if let Some(field) = self.session_field(&prop.value, scope, FieldExpect::Path) {
                        if let Type::List { inner } = &field.ty {
                            child_ctx.route = Some((**inner).clone());
                        }
                        props.insert(prop_name, ir::Prop::Field { field });
                    }
                }
                PropKind::Query => {
                    let Some(query) = self.query(&prop.value, scope) else {
                        continue;
                    };
                    let shown = match &query.result {
                        Type::Optional { inner } => (**inner).clone(),
                        other => other.clone(),
                    };
                    if let Some(empty) = element.prop("empty")
                        && !matches!(query.result, Type::Optional { .. } | Type::List { .. })
                    {
                        self.error(
                            empty.name.span(),
                            format!(
                                "`empty:` needs a query whose result can be empty (a list or an \
                                 optional), found {}",
                                query.result.display()
                            ),
                        );
                    }
                    if let Some(var) = &element.template {
                        scope.push(var.to_string(), Root::Local(shown.clone()));
                        pushed += 1;
                        template = Some(ir::Template {
                            var: var.to_string(),
                            ty: shown,
                        });
                    }
                    props.insert(prop_name, ir::Prop::Query { query });
                }
                PropKind::Items => {
                    let Some(expr) = self.expr(&prop.value, scope) else {
                        continue;
                    };
                    let item_ty = match &expr.ty {
                        Type::List { inner } if matches!(**inner, Type::Record { .. }) => {
                            (**inner).clone()
                        }
                        other => {
                            self.error(
                                prop.value.span(),
                                format!(
                                    "`items` needs a list of records, found {}",
                                    other.display()
                                ),
                            );
                            continue;
                        }
                    };
                    if let Some(var) = &element.template {
                        scope.push(var.to_string(), Root::Local(item_ty.clone()));
                        pushed += 1;
                        template = Some(ir::Template {
                            var: var.to_string(),
                            ty: item_ty,
                        });
                    }
                    props.insert(prop_name, ir::Prop::Expr { expr });
                }
                _ => {}
            }
        }
        if name == "Destination"
            && let Some(var) = &element.template
        {
            match &ctx.route {
                Some(route) => {
                    scope.push(var.to_string(), Root::Local(route.clone()));
                    pushed += 1;
                    template = Some(ir::Template {
                        var: var.to_string(),
                        ty: route.clone(),
                    });
                }
                None => self.error(element.span, "`Destination` must be inside `Navigation`"),
            }
        }

        for prop in &element.props {
            let prop_name = prop.name.to_string();
            let Some(prop_spec) = component.prop(&prop_name) else {
                continue;
            };
            let key = text_key(&self.component, path, &prop_name);
            let lowered = match prop_spec.kind {
                PropKind::Id => self.id(&prop.value).map(|value| {
                    id = Some(value.clone());
                    ir::Prop::Id { value }
                }),
                PropKind::Keyword(allowed) => self.keyword(&prop.value, allowed),
                PropKind::Flag => match &prop.value {
                    ast::Expr::Literal {
                        value: Literal::Bool(value),
                        ..
                    } => Some(ir::Prop::Keyword {
                        value: value.to_string(),
                    }),
                    other => {
                        self.error(other.span(), "expected `true` or `false`");
                        None
                    }
                },
                PropKind::DisplayText => self
                    .display(&prop.value, scope, key, false)
                    .map(|text| ir::Prop::Text { text }),
                PropKind::LocalizedText => self
                    .display(&prop.value, scope, key, true)
                    .map(|text| ir::Prop::Text { text }),
                PropKind::Icon => self.icon(&prop.value),
                PropKind::Shortcut => match &prop.value {
                    ast::Expr::Literal {
                        value: Literal::Text(text),
                        span,
                    } => match spec::parse_shortcut(text) {
                        Ok(value) => Some(ir::Prop::Keyword { value }),
                        Err(message) => {
                            self.error(*span, format!("shortcut \"{text}\": {message}"));
                            None
                        }
                    },
                    other => {
                        self.error(other.span(), "a shortcut is a literal such as \"cmd+k\"");
                        None
                    }
                },
                // A picture is a declared resource, or a `MediaSource`
                // the engine gives: a file or an http(s) URL.
                PropKind::ImageResource => match &prop.value {
                    ast::Expr::Path(_) => self.media_source(&prop.value, scope),
                    _ => self.image_resource(&prop.value),
                },
                // A literal length is checked like a number; any other
                // numeric expression is a length that follows it.
                PropKind::Length
                    if !matches!(
                        &prop.value,
                        ast::Expr::Literal { .. } | ast::Expr::Unary { .. }
                    ) =>
                {
                    match self.expr(&prop.value, scope) {
                        Some(lowered) if matches!(lowered.ty, Type::Int | Type::Float) => {
                            Some(ir::Prop::Expr { expr: lowered })
                        }
                        Some(lowered) => {
                            self.error(
                                prop.value.span(),
                                format!(
                                    "`{prop_name}` is a length, found {}",
                                    lowered.ty.display()
                                ),
                            );
                            None
                        }
                        None => None,
                    }
                }
                PropKind::Number | PropKind::Length => self.number(&prop.value).and_then(|value| {
                    // Sizes of pictures and grid cells, and the total of a
                    // progress, are lengths and amounts above zero.
                    let positive = matches!(
                        (name.as_str(), prop_name.as_str()),
                        ("Image", "width" | "height")
                            | ("Grid", "min_width")
                            | ("Progress", "total")
                    );
                    let positive = positive
                        || matches!(
                            (name.as_str(), prop_name.as_str()),
                            (_, "scroll_scale") | ("Swipe", "distance")
                        );
                    if positive && value <= 0.0 {
                        self.error(
                            prop.value.span(),
                            format!("`{prop_name}` must be above zero"),
                        );
                        return None;
                    }
                    if prop_name == "nudge_scale" && (!value.is_finite() || value <= 0.0) {
                        self.error(
                            prop.value.span(),
                            "`nudge_scale` must be finite and greater than zero",
                        );
                        return None;
                    }
                    if prop_name == "appear_delay" && (!value.is_finite() || value < 0.0) {
                        self.error(
                            prop.value.span(),
                            "`appear_delay` must be a finite, nonnegative number of seconds",
                        );
                        return None;
                    }
                    if prop_name == "stagger" && !(0.0..=1.0).contains(&value) {
                        self.error(
                            prop.value.span(),
                            "`stagger` is a delay from 0 to 1 second between children",
                        );
                        return None;
                    }
                    if prop_name == "scroll_opacity" && !(0.0..=1.0).contains(&value) {
                        self.error(
                            prop.value.span(),
                            "`scroll_opacity` is an opacity from 0 to 1",
                        );
                        return None;
                    }
                    Some(ir::Prop::Number { value })
                }),
                PropKind::NumberValue => match self.expr(&prop.value, scope) {
                    Some(lowered) if matches!(lowered.ty, Type::Int | Type::Float) => {
                        Some(ir::Prop::Expr { expr: lowered })
                    }
                    Some(lowered) => {
                        self.error(
                            prop.value.span(),
                            format!(
                                "`{prop_name}` must be a number, found {}",
                                lowered.ty.display()
                            ),
                        );
                        None
                    }
                    None => None,
                },
                PropKind::Bool => self
                    .expect(&prop.value, &Type::Bool, scope, "this property")
                    .map(|expr| ir::Prop::Expr { expr }),
                PropKind::TextField => self.field_prop(&prop.value, scope, FieldExpect::Text),
                PropKind::BoolField => self.field_prop(&prop.value, scope, FieldExpect::Bool),
                PropKind::NumberField => self.field_prop(&prop.value, scope, FieldExpect::Number),
                PropKind::FileField => self.field_prop(&prop.value, scope, FieldExpect::File),
                PropKind::PositionField => {
                    self.field_prop(&prop.value, scope, FieldExpect::Position)
                }
                PropKind::FileKinds => match &prop.value {
                    ast::Expr::Literal {
                        value: Literal::Text(text),
                        span,
                    } => match spec::parse_file_kinds(text) {
                        Ok(value) => Some(ir::Prop::Keyword { value }),
                        Err(message) => {
                            self.error(*span, format!("files \"{text}\": {message}"));
                            None
                        }
                    },
                    other => {
                        self.error(
                            other.span(),
                            "the kinds of file are a literal such as \"video .srt\"",
                        );
                        None
                    }
                },
                PropKind::PickerField => self.field_prop(&prop.value, scope, FieldExpect::Picker),
                // A container's `fallback` is one of its own values.
                PropKind::SelectionValue => match if prop_name == "fallback" {
                    child_ctx.selection.clone()
                } else {
                    ctx.selection.clone()
                } {
                    Some(selection) => self
                        .expect(&prop.value, &selection, scope, "this value")
                        .map(|expr| ir::Prop::Expr { expr }),
                    None => {
                        self.error(
                            prop.value.span(),
                            "this value needs an enclosing `Sidebar` or `Tabs` selection",
                        );
                        None
                    }
                },
                PropKind::Route => match ctx.route.clone() {
                    Some(route) => self
                        .expect(&prop.value, &route, scope, "the route")
                        .map(|expr| ir::Prop::Expr { expr }),
                    None => {
                        self.error(prop.value.span(), "`NavLink` must be inside a `Navigation`");
                        None
                    }
                },
                PropKind::KeyField => self.key(&prop.value, template.as_ref()),
                PropKind::WindowId => self.window_id(&prop.value, &name, &prop_name),
                // Slots are expanded before elements are lowered.
                PropKind::SlotName => None,
                PropKind::Transition | PropKind::MotionToken | PropKind::HeroName => {
                    match &prop.value {
                        ast::Expr::Path(path) if path.single().is_some() => {
                            let value = path.single().unwrap().to_string();
                            let known = match prop_spec.kind {
                                PropKind::Transition => self.transitions.contains(&value),
                                PropKind::MotionToken => self.tokens.contains(&value),
                                _ => true,
                            };
                            if known {
                                Some(ir::Prop::Keyword { value })
                            } else {
                                let what = if prop_spec.kind == PropKind::Transition {
                                    "transition; declare it under `transition { … }` in `style!`"
                                } else {
                                    "motion token; declare it under `motion { … }` in `style!`"
                                };
                                self.error(prop.value.span(), format!("`{value}` is not a {what}"));
                                None
                            }
                        }
                        other => {
                            self.error(other.span(), format!("`{prop_name}` takes a name"));
                            None
                        }
                    }
                }
                PropKind::ColorRole => match &prop.value {
                    ast::Expr::Path(path) => {
                        let role = path.dotted();
                        if bern_ir::style::COLOR_ROLES.contains(&role.as_str()) {
                            Some(ir::Prop::Keyword { value: role })
                        } else {
                            self.error(
                                prop.value.span(),
                                format!(
                                    "`{role}` is not a color role; use one of {}",
                                    bern_ir::style::COLOR_ROLES.join(", ")
                                ),
                            );
                            None
                        }
                    }
                    other => {
                        self.error(
                            other.span(),
                            "a color is a role such as `accent` or `fill.subtle`",
                        );
                        None
                    }
                },
                PropKind::PatternName => match &prop.value {
                    ast::Expr::Path(path) if path.single().is_some() => {
                        let value = path.single().unwrap().to_string();
                        if self.patterns.contains(&value) {
                            Some(ir::Prop::Keyword { value })
                        } else {
                            self.error(
                                prop.value.span(),
                                format!("`{value}` is not a pattern; declare it under `pattern {{ … }}` in `style!`"),
                            );
                            None
                        }
                    }
                    other => {
                        self.error(other.span(), "`name` takes a pattern's name");
                        None
                    }
                },
                PropKind::NumberList => match self.expr(&prop.value, scope) {
                    Some(lowered) if matches!(&lowered.ty, Type::List { inner } if matches!(**inner, Type::Int | Type::Float)) => {
                        Some(ir::Prop::Expr { expr: lowered })
                    }
                    Some(lowered) => {
                        self.error(
                            prop.value.span(),
                            format!(
                                "`{prop_name}` is a list of numbers, found {}",
                                lowered.ty.display()
                            ),
                        );
                        None
                    }
                    None => None,
                },
                PropKind::KeyValue => match self.expr(&prop.value, scope) {
                    Some(lowered)
                        if matches!(lowered.ty, Type::Int | Type::Text | Type::Enum { .. }) =>
                    {
                        Some(ir::Prop::Expr { expr: lowered })
                    }
                    Some(lowered) => {
                        self.error(
                            prop.value.span(),
                            format!(
                                "`{prop_name}` identifies with text, a number or an enum, found {}",
                                lowered.ty.display()
                            ),
                        );
                        None
                    }
                    None => None,
                },
                PropKind::StyleVariant => {
                    match &prop.value {
                        ast::Expr::Path(path) if path.single().is_some() => {
                            let variant = path.single().unwrap().to_string();
                            if self.variants.iter().any(|(component, declared)| {
                                *component == name && *declared == variant
                            }) {
                                Some(ir::Prop::Keyword { value: variant })
                            } else {
                                self.error(
                                    prop.value.span(),
                                    format!("`style!` declares no variant `{name}.{variant}`"),
                                );
                                None
                            }
                        }
                        other => {
                            self.error(other.span(), "a variant is a name, e.g. `variant: hero`");
                            None
                        }
                    }
                }
                PropKind::TypeName => match &prop.value {
                    ast::Expr::Path(path) if path.single().is_some() => Some(ir::Prop::TypeName {
                        name: path.single().unwrap().to_string(),
                    }),
                    other => {
                        self.error(other.span(), "expected a type name");
                        None
                    }
                },
                PropKind::MediaSource => self.media_source(&prop.value, scope),
                PropKind::Player => self.player(&prop.value, scope),
                PropKind::Feature => self.feature(&prop.value),
                PropKind::SelectionField
                | PropKind::PathField
                | PropKind::Items
                | PropKind::Query => continue,
            };
            if let Some(lowered) = lowered {
                props.insert(prop_name, lowered);
            }
        }

        if name == "Picker" {
            if let Some(ir::Prop::Expr { expr }) = props.get("selected") {
                if !is_labeled_enum(&expr.ty, self.types) {
                    self.error(
                        element.span,
                        "`selected` must be an enum whose variants all have labels",
                    );
                }
            }
        }
        let children = match (&element.children, name.as_str()) {
            // The layouts of one `Adaptive` are alternatives: the same
            // control id in two of them names the same control.
            (Some(children), "Adaptive") => {
                let before = self.ids.clone();
                let mut all = before.clone();
                let mut lowered = Vec::new();
                for (index, child) in children.iter().enumerate() {
                    self.ids = before.clone();
                    lowered.extend(self.node(child, &format!("{path}.{index}"), &child_ctx, scope));
                    all.append(&mut self.ids);
                }
                self.ids = all;
                self.adaptive(element, &lowered);
                lowered
            }
            (Some(children), _) => self.nodes(children, path, &child_ctx, scope),
            (None, _) => Vec::new(),
        };
        if let (Some(ir::Prop::Number { value: min }), Some(ir::Prop::Number { value: max })) =
            (props.get("min"), props.get("max"))
            && min >= max
        {
            self.error(element.span, format!("`{name}` needs `min` below `max`"));
        }
        for _ in 0..pushed {
            scope.pop();
        }
        let action = element
            .action
            .as_ref()
            .and_then(|action| self.action(action, scope));
        Some(ir::Element {
            kind: name,
            id,
            path: path.to_string(),
            props,
            template,
            children,
            action,
            source: self.loc(element.span),
        })
    }

    fn id(&mut self, expr: &ast::Expr) -> Option<String> {
        let (value, span) = match expr {
            ast::Expr::Path(path) if path.single().is_some() => {
                (path.single().unwrap().to_string(), path.span())
            }
            ast::Expr::Literal {
                value: Literal::Text(text),
                span,
            } => (text.clone(), *span),
            other => {
                self.error(other.span(), "expected an identifier");
                return None;
            }
        };
        if value.is_empty()
            || !value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            self.error(span, "ids use letters, digits, `_` and `-`");
        }
        if !self.ids.insert(value.clone()) {
            self.error(
                span,
                format!("control id `{value}` is used twice in this component"),
            );
        }
        Some(value)
    }

    /// `feature: Contract::Feature`, naming a declared feature.
    fn feature(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        let ast::Expr::Variant(variant) = expr else {
            self.error(
                expr.span(),
                "expected a feature of the component's contract, e.g. `MediaFeature::Clip`",
            );
            return None;
        };
        let contract = variant.ty.to_string();
        let feature = variant.variant.to_string();
        let Some(features) = self.contracts.get(&contract) else {
            self.error(
                variant.ty.span(),
                format!("`{contract}` is not a `#[bern::features]` contract"),
            );
            return None;
        };
        if !features.contains(&feature) {
            self.error(
                variant.variant.span(),
                format!(
                    "contract `{contract}` has no feature `{feature}`; features: {}",
                    features.join(", ")
                ),
            );
            return None;
        }
        Some(ir::Prop::Feature { contract, feature })
    }

    /// An `Adaptive` has one `Layout` per size, and needs `[layouts]`.
    fn adaptive(&mut self, element: &ast::Element, layouts: &[ir::Node]) {
        if self.layouts.is_none() {
            self.error(
                element.span,
                "error[B402]: `Adaptive` needs the breakpoints of `[layouts]` in Bern.toml, e.g. `compact_below = 600`",
            );
        }
        let sizes: Vec<&str> = layouts
            .iter()
            .filter_map(|node| match node {
                ir::Node::Element(layout) => layout.keyword("size"),
                _ => None,
            })
            .collect();
        for size in ir::LayoutSize::ALL {
            let count = sizes.iter().filter(|name| **name == size.name()).count();
            let required = ir::LayoutSize::REQUIRED.contains(&size);
            if (required && count != 1) || count > 1 {
                self.error(
                    element.span,
                    format!(
                        "error[B401]: `Adaptive` needs exactly one `Layout(size: {})`, found {count}",
                        size.name()
                    ),
                );
            }
            // An optional size needs its breakpoint to have any width.
            if count == 1
                && !required
                && let Some(config) = self.layouts
                && !config.has(size)
            {
                let breakpoint = match size {
                    ir::LayoutSize::Medium => "medium_below",
                    _ => "expanded_below",
                };
                self.error(
                    element.span,
                    format!(
                        "error[B401]: `Layout(size: {})` needs `{breakpoint}` in `[layouts]` of Bern.toml",
                        size.name()
                    ),
                );
            }
        }
    }

    fn keyword(&mut self, expr: &ast::Expr, allowed: &[&str]) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Path(path)
                if path
                    .single()
                    .is_some_and(|word| allowed.contains(&word.to_string().as_str())) =>
            {
                Some(ir::Prop::Keyword {
                    value: path.single().unwrap().to_string(),
                })
            }
            other => {
                self.error(
                    other.span(),
                    format!("expected one of: {}", allowed.join(", ")),
                );
                None
            }
        }
    }

    fn icon(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Text(name),
                span,
            } => {
                if name.is_empty()
                    || !name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
                {
                    self.error(
                        *span,
                        "icon names use the Bern icon set (SF Symbols naming), e.g. \"gearshape\"",
                    );
                    return None;
                }
                Some(ir::Prop::Icon { name: name.clone() })
            }
            other => {
                self.error(other.span(), "expected an icon name literal");
                None
            }
        }
    }

    /// `"images/cover.png"`: the name of a PNG or JPEG resource. Whether
    /// `[resources]` declares it is checked for the whole application,
    /// library components included, once the components are lowered.
    fn image_resource(&mut self, expr: &ast::Expr) -> Option<ir::Prop> {
        let ast::Expr::Literal {
            value: Literal::Text(name),
            span,
        } = expr
        else {
            self.error(
                expr.span(),
                "expected the name of a declared resource, e.g. \"images/cover.png\"",
            );
            return None;
        };
        if let Err(message) = bern_protocol::check_resource_name(name) {
            self.error(*span, message);
            return None;
        }
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase());
        if !extension.is_some_and(|ext| spec::IMAGE_EXTENSIONS.contains(&ext.as_str())) {
            self.error(
                *span,
                format!("`{name}` is not a PNG or JPEG image (.png, .jpg or .jpeg)"),
            );
            return None;
        }
        Some(ir::Prop::Resource { name: name.clone() })
    }

    fn number(&mut self, expr: &ast::Expr) -> Option<f64> {
        match expr {
            ast::Expr::Literal {
                value: Literal::Int(value),
                ..
            } => Some(*value as f64),
            ast::Expr::Literal {
                value: Literal::Float(value),
                ..
            } => Some(*value),
            ast::Expr::Unary {
                op: UnaryOp::Neg,
                expr,
                ..
            } => self.number(expr).map(|value| -value),
            other => {
                self.error(other.span(), "expected a numeric literal");
                None
            }
        }
    }

    fn key(&mut self, expr: &ast::Expr, template: Option<&ir::Template>) -> Option<ir::Prop> {
        let (Some(field), Some(template)) = (
            match expr {
                ast::Expr::Path(path) => path.single().cloned(),
                _ => None,
            },
            template,
        ) else {
            self.error(
                expr.span(),
                "expected the name of a row field, e.g. `key: id`",
            );
            return None;
        };
        let Type::Record { name } = &template.ty else {
            return None;
        };
        let def = self.types.record_def(name)?;
        let Some(index) = def.field_index(&field.to_string()) else {
            self.error(field.span(), format!("`{name}` has no field `{field}`"));
            return None;
        };
        if !matches!(def.fields[index].ty, Type::Int | Type::Text) {
            self.error(
                field.span(),
                "list keys must be `i64` or `String` fields; positions are not identities",
            );
            return None;
        }
        Some(ir::Prop::Key {
            field: index,
            name: field.to_string(),
        })
    }

    fn window_id(&mut self, expr: &ast::Expr, element: &str, prop: &str) -> Option<ir::Prop> {
        let ast::Expr::Path(path) = expr else {
            self.error(expr.span(), "expected a window identifier");
            return None;
        };
        let Some(id) = path.single() else {
            self.error(expr.span(), "expected a window identifier");
            return None;
        };
        let id = id.to_string();
        if !(element == "Window" && prop == "id") && !self.window_ids.contains(&id) {
            self.error(
                expr.span(),
                format!("window `{id}` is not declared in the WindowSet"),
            );
            return None;
        }
        Some(ir::Prop::Window { id })
    }

    fn media_source(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::Prop> {
        let ast::Expr::Path(path) = expr else {
            self.error(
                expr.span(),
                "expected a `MediaSource` binding such as `app.clip`",
            );
            return None;
        };
        let lowered = self.binding(path, scope)?;
        let source = Type::Record {
            name: bern_media::MEDIA_SOURCE.into(),
        };
        if lowered.ty != source && lowered.ty != Type::optional(source) {
            self.error(
                path.span(),
                format!(
                    "`source` needs a MediaSource or Option<MediaSource>, found {}",
                    lowered.ty.display()
                ),
            );
            return None;
        }
        Some(ir::Prop::Expr { expr: lowered })
    }

    /// `players.preview` names an application player.
    fn player_ref(&mut self, segments: &[syn::Ident], scope: &Scope) -> Option<String> {
        let root = segments.first()?;
        if !matches!(scope.get(&root.to_string()), Some(Root::Players)) || segments.len() != 2 {
            self.error(
                root.span(),
                "expected an application player such as `players.preview`",
            );
            return None;
        }
        let id = segments[1].to_string();
        if !self.player_ids.contains(&id) {
            self.error(
                segments[1].span(),
                format!("player `{id}` is not declared in the #[bern::players] catalog"),
            );
            return None;
        }
        Some(id)
    }

    fn player(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::Prop> {
        let ast::Expr::Path(path) = expr else {
            self.error(
                expr.span(),
                "expected an application player such as `players.preview`",
            );
            return None;
        };
        self.player_ref(&path.segments, scope)
            .map(|id| ir::Prop::Player { id })
    }

    fn field_prop(
        &mut self,
        expr: &ast::Expr,
        scope: &Scope,
        expect: FieldExpect,
    ) -> Option<ir::Prop> {
        self.session_field(expr, scope, expect)
            .map(|field| ir::Prop::Field { field })
    }

    fn session_field(
        &mut self,
        expr: &ast::Expr,
        scope: &Scope,
        expect: FieldExpect,
    ) -> Option<ir::FieldRef> {
        let ast::Expr::Path(path) = expr else {
            self.error(expr.span(), "expected a session field such as `draft.name`");
            return None;
        };
        let root = path.segments[0].to_string();
        match scope.get(&root) {
            Some(Root::Session(session)) if path.segments.len() == 2 => {
                let session = session.clone();
                let def = self.sessions.iter().find(|def| def.name == session)?;
                let field_name = path.segments[1].to_string();
                let Some(field) = def.field_index(&field_name) else {
                    self.error(
                        path.segments[1].span(),
                        format!("`{session}` has no field `{field_name}`"),
                    );
                    return None;
                };
                let ty = def.fields[field].ty.clone();
                let ok = match expect {
                    FieldExpect::Text => ty == Type::Text,
                    FieldExpect::Bool => ty == Type::Bool,
                    FieldExpect::Number => ty == Type::Float,
                    FieldExpect::File => {
                        let source = Type::Record {
                            name: bern_media::MEDIA_SOURCE.into(),
                        };
                        ty == source || ty == Type::optional(source)
                    }
                    FieldExpect::Position => matches!(ty, Type::Int | Type::Text),
                    FieldExpect::Picker => is_labeled_enum(&ty, self.types),
                    FieldExpect::Selection => matches!(ty, Type::Enum { .. }),
                    FieldExpect::Path => {
                        matches!(&ty, Type::List { inner } if matches!(**inner, Type::Enum { .. }))
                    }
                };
                if !ok {
                    let wanted = match expect {
                        FieldExpect::Text => "a `String` field",
                        FieldExpect::Bool => "a `bool` field",
                        FieldExpect::Number => "an `f64` field",
                        FieldExpect::File => "a `MediaSource` or `Option<MediaSource>` field",
                        FieldExpect::Position => {
                            "an `i64` or `String` field, like the keys of the items"
                        }
                        FieldExpect::Picker => "an enum field whose variants all have labels",
                        FieldExpect::Selection => "an enum field",
                        FieldExpect::Path => "a `Vec` of a route enum",
                    };
                    self.error(
                        path.span(),
                        format!("expected {wanted}, found {}", ty.display()),
                    );
                    return None;
                }
                Some(ir::FieldRef { session, field, ty })
            }
            Some(Root::Client(_)) => {
                self.error(
                    path.span(),
                    "controls only write to session state; engine state changes through commands",
                );
                None
            }
            None => {
                self.error(path.span(), format!("unknown binding `{root}`"));
                None
            }
            _ => {
                self.error(path.span(), "expected a session field such as `draft.name`");
                None
            }
        }
    }

    fn record_path(&mut self, mut ty: Type, segments: &[syn::Ident]) -> Option<(Vec<usize>, Type)> {
        let mut path = Vec::new();
        for segment in segments {
            let Type::Record { name } = &ty else {
                self.error(
                    segment.span(),
                    format!("{} has no field `{segment}`", ty.display()),
                );
                return None;
            };
            let def = self.types.record_def(name)?;
            let Some(index) = def.field_index(&segment.to_string()) else {
                self.error(segment.span(), format!("`{name}` has no field `{segment}`"));
                return None;
            };
            path.push(index);
            ty = def.fields[index].ty.clone();
        }
        Some((path, ty))
    }

    fn binding(&mut self, path: &ast::PathExpr, scope: &Scope) -> Option<ir::Expr> {
        let root_name = path.segments[0].to_string();
        // `window` names the hosting window's UI state unless a parameter
        // or variable takes the name.
        if root_name == "window" && scope.get(&root_name).is_none() {
            let field = path.segments.get(1).map(ToString::to_string);
            let sizes = ir::LayoutSize::ALL.map(ir::LayoutSize::name);
            let known = field.as_deref() == Some("idle")
                || field.as_deref().is_some_and(|name| sizes.contains(&name));
            if !known || path.segments.len() != 2 {
                self.error(
                    path.span(),
                    "the hosting window offers `window.idle` (no input for `idle_after` seconds) and its size: `window.compact`, `window.medium`, `window.expanded`, `window.large`",
                );
                return None;
            }
            let field = field.unwrap_or_default();
            if field != "idle" {
                match self.layouts {
                    None => {
                        self.error(
                            path.span(),
                            format!("error[B402]: `window.{field}` needs the breakpoints of `[layouts]` in Bern.toml, e.g. `compact_below = 600`"),
                        );
                    }
                    Some(config) => {
                        let size = ir::LayoutSize::parse(&field).expect("size");
                        if !config.has(size) {
                            let breakpoint = if size == ir::LayoutSize::Medium {
                                "medium_below"
                            } else {
                                "expanded_below"
                            };
                            self.error(
                                path.span(),
                                format!("`window.{field}` needs `{breakpoint}` in `[layouts]` of Bern.toml; without it the window is never {field}"),
                            );
                        }
                    }
                }
            }
            return Some(ir::Expr {
                kind: ir::ExprKind::Binding {
                    binding: ir::Binding::Window { field },
                },
                ty: Type::Bool,
            });
        }
        let Some(root) = scope.get(&root_name).cloned() else {
            self.error(
                path.span(),
                format!(
                    "unknown binding `{root_name}`; bindings start at a component parameter or template variable"
                ),
            );
            return None;
        };
        let rest = &path.segments[1..];
        let (binding, ty) = match root {
            Root::Client(namespace) => {
                let Some(first) = rest.first() else {
                    self.error(
                        path.span(),
                        "the client is not a value; read a state field such as `app.field`",
                    );
                    return None;
                };
                if namespace == self.protocol.namespace {
                    let Some(field) = self.protocol.state_index(&first.to_string()) else {
                        self.error(
                            first.span(),
                            format!(
                                "protocol `{}` has no state field `{first}`",
                                self.protocol.namespace
                            ),
                        );
                        return None;
                    };
                    let (sub, ty) =
                        self.record_path(self.protocol.state[field].ty.clone(), &rest[1..])?;
                    (ir::Binding::Engine { field, path: sub }, ty)
                } else {
                    // A module's state is the application state field named
                    // after the module.
                    let field = self.protocol.state_index(&namespace)?;
                    let (sub, ty) =
                        self.record_path(self.protocol.state[field].ty.clone(), rest)?;
                    (ir::Binding::Engine { field, path: sub }, ty)
                }
            }
            Root::Windows => {
                self.error(path.span(), "window handles are only used in actions");
                return None;
            }
            Root::Players => {
                self.error(
                    path.span(),
                    "players are used by `Video(player: …)` and in actions such as `players.preview.play()`",
                );
                return None;
            }
            Root::Session(session) => {
                let Some(first) = rest.first() else {
                    self.error(
                        path.span(),
                        "session state is not a value; read a field such as `draft.name`",
                    );
                    return None;
                };
                let def = self.sessions.iter().find(|def| def.name == session)?;
                let Some(field) = def.field_index(&first.to_string()) else {
                    self.error(first.span(), format!("`{session}` has no field `{first}`"));
                    return None;
                };
                let field_ty = def.fields[field].ty.clone();
                let (sub, ty) = self.record_path(field_ty, &rest[1..])?;
                (
                    ir::Binding::Session {
                        session,
                        field,
                        path: sub,
                    },
                    ty,
                )
            }
            Root::Local(root) => {
                let (sub, ty) = self.record_path(root.clone(), rest)?;
                (
                    ir::Binding::Local {
                        name: root_name,
                        path: sub,
                        root,
                    },
                    ty,
                )
            }
        };
        Some(ir::Expr {
            kind: ir::ExprKind::Binding { binding },
            ty,
        })
    }

    fn expect(
        &mut self,
        expr: &ast::Expr,
        ty: &Type,
        scope: &Scope,
        what: &str,
    ) -> Option<ir::Expr> {
        // A whole number written where a decimal is expected is that
        // decimal: `draft.size.set(12)` for an `f64` field.
        if let (
            ast::Expr::Literal {
                value: Literal::Int(whole),
                ..
            },
            Type::Float,
        ) = (expr, ty)
        {
            return Some(ir::Expr {
                kind: ir::ExprKind::Literal {
                    value: Value::Float(*whole as f64),
                },
                ty: Type::Float,
            });
        }
        let lowered = self.expr(expr, scope)?;
        if lowered.ty != *ty {
            self.error(
                expr.span(),
                format!(
                    "{what} must be {}, found {}",
                    ty.display(),
                    lowered.ty.display()
                ),
            );
            return None;
        }
        Some(lowered)
    }

    pub fn literal(value: &Literal) -> (Value, Type) {
        match value {
            Literal::Text(text) => (Value::Text(text.clone()), Type::Text),
            Literal::Int(value) => (Value::Int(*value), Type::Int),
            Literal::Float(value) => (Value::Float(*value), Type::Float),
            Literal::Bool(value) => (Value::Bool(*value), Type::Bool),
        }
    }

    fn expr(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::Expr> {
        match expr {
            ast::Expr::Literal { value, .. } => {
                let (value, ty) = Self::literal(value);
                Some(ir::Expr {
                    kind: ir::ExprKind::Literal { value },
                    ty,
                })
            }
            ast::Expr::Tr(tr) => {
                self.error(
                    tr.span,
                    "translations are display text and cannot be used as values",
                );
                None
            }
            ast::Expr::Path(path) => self.binding(path, scope),
            ast::Expr::Variant(variant) => {
                let ty_name = variant.ty.to_string();
                let Some(def) = self.types.enum_def(&ty_name).cloned() else {
                    self.error(variant.ty.span(), format!("unknown enum `{ty_name}`"));
                    return None;
                };
                let Some(index) = def.variant_index(&variant.variant.to_string()) else {
                    self.error(
                        variant.variant.span(),
                        format!("`{ty_name}` has no variant `{}`", variant.variant),
                    );
                    return None;
                };
                let fields_def = &def.variants[index].fields;
                if fields_def.is_empty() && !variant.fields.is_empty() {
                    self.error(variant.variant.span(), "this variant has no fields");
                    return None;
                }
                let mut fields = Vec::new();
                for field_def in fields_def {
                    let Some((_, value)) = variant
                        .fields
                        .iter()
                        .find(|(name, _)| *name == field_def.name)
                    else {
                        self.error(
                            variant.variant.span(),
                            format!("missing field `{}`", field_def.name),
                        );
                        return None;
                    };
                    fields.push(self.expect(value, &field_def.ty, scope, "the field")?);
                }
                for (name, _) in &variant.fields {
                    if !fields_def.iter().any(|field| *name == field.name) {
                        self.error(
                            name.span(),
                            format!("`{ty_name}::{}` has no field `{name}`", variant.variant),
                        );
                        return None;
                    }
                }
                Some(ir::Expr {
                    kind: ir::ExprKind::Variant {
                        variant: index as u32,
                        fields,
                    },
                    ty: Type::Enum { name: ty_name },
                })
            }
            ast::Expr::Unary {
                op,
                expr: inner,
                span,
            } => {
                let lowered = self.expr(inner, scope)?;
                match op {
                    UnaryOp::Not => {
                        if lowered.ty != Type::Bool {
                            self.error(
                                *span,
                                format!("`!` needs bool, found {}", lowered.ty.display()),
                            );
                            return None;
                        }
                        Some(ir::Expr {
                            kind: ir::ExprKind::Not {
                                expr: Box::new(lowered),
                            },
                            ty: Type::Bool,
                        })
                    }
                    UnaryOp::Neg => match (&lowered.kind, &lowered.ty) {
                        (
                            ir::ExprKind::Literal {
                                value: Value::Int(v),
                            },
                            _,
                        ) => Some(ir::Expr {
                            kind: ir::ExprKind::Literal {
                                value: Value::Int(-v),
                            },
                            ty: Type::Int,
                        }),
                        (
                            ir::ExprKind::Literal {
                                value: Value::Float(v),
                            },
                            _,
                        ) => Some(ir::Expr {
                            kind: ir::ExprKind::Literal {
                                value: Value::Float(-v),
                            },
                            ty: Type::Float,
                        }),
                        (_, Type::Int | Type::Float) => Some(ir::Expr {
                            ty: lowered.ty.clone(),
                            kind: ir::ExprKind::Neg {
                                expr: Box::new(lowered),
                            },
                        }),
                        (_, ty) => {
                            self.error(
                                *span,
                                format!("`-` needs a number, found {}", ty.display()),
                            );
                            None
                        }
                    },
                }
            }
            ast::Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.expr(lhs, scope)?;
                let rhs = self.expr(rhs, scope)?;
                let ir_op = match op {
                    BinaryOp::Eq => ir::BinaryOp::Eq,
                    BinaryOp::Ne => ir::BinaryOp::Ne,
                    BinaryOp::Lt => ir::BinaryOp::Lt,
                    BinaryOp::Le => ir::BinaryOp::Le,
                    BinaryOp::Gt => ir::BinaryOp::Gt,
                    BinaryOp::Ge => ir::BinaryOp::Ge,
                    BinaryOp::And => ir::BinaryOp::And,
                    BinaryOp::Or => ir::BinaryOp::Or,
                };
                let ok = match op {
                    BinaryOp::Eq | BinaryOp::Ne => lhs.ty == rhs.ty,
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        lhs.ty == rhs.ty && matches!(lhs.ty, Type::Int | Type::Float)
                    }
                    BinaryOp::And | BinaryOp::Or => lhs.ty == Type::Bool && rhs.ty == Type::Bool,
                };
                if !ok {
                    self.error(
                        *span,
                        format!(
                            "`{}` cannot combine {} and {}",
                            op.symbol(),
                            lhs.ty.display(),
                            rhs.ty.display()
                        ),
                    );
                    return None;
                }
                Some(ir::Expr {
                    kind: ir::ExprKind::Binary {
                        op: ir_op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: Type::Bool,
                })
            }
            ast::Expr::Query(call) => {
                self.error(
                    call.span,
                    "a query call is only valid as the `query:` property of `Query`",
                );
                None
            }
            ast::Expr::If {
                cond,
                then,
                otherwise,
                span,
            } => {
                let cond = self.expr(cond, scope)?;
                if cond.ty != Type::Bool {
                    self.error(
                        *span,
                        format!("an `if` condition is a `bool`, found {}", cond.ty.display()),
                    );
                    return None;
                }
                let then = self.expr(then, scope)?;
                let otherwise = self.expr(otherwise, scope)?;
                // A whole number beside a float is a float.
                let ty = match (&then.ty, &otherwise.ty) {
                    (a, b) if a == b => a.clone(),
                    (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
                    (a, b) => {
                        self.error(
                            *span,
                            format!(
                                "both branches of an `if` have the same type, found {} and {}",
                                a.display(),
                                b.display()
                            ),
                        );
                        return None;
                    }
                };
                Some(ir::Expr {
                    kind: ir::ExprKind::If {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        otherwise: Box::new(otherwise),
                    },
                    ty,
                })
            }
        }
    }

    fn display(
        &mut self,
        expr: &ast::Expr,
        scope: &Scope,
        key: String,
        localized_only: bool,
    ) -> Option<ir::Text> {
        match expr {
            ast::Expr::Tr(tr) => {
                let config = Some((self.languages, self.strict));
                let problems = bern_syntax::lower::check_translations(tr, config);
                let failed = !problems.is_empty();
                for (span, message) in problems {
                    self.error(span, message);
                }
                let mut args = Vec::new();
                for arg in &tr.args {
                    let lowered = self.expr(&arg.value, scope)?;
                    if !is_displayable(&lowered.ty, self.types) {
                        self.error(
                            arg.value.span(),
                            format!(
                                "{} has no text presentation; interpolate text, numbers or labelled enums",
                                lowered.ty.display()
                            ),
                        );
                        return None;
                    }
                    args.push((arg.name.to_string(), lowered));
                }
                if failed {
                    return None;
                }
                let text = LocalizedText {
                    key,
                    translations: tr
                        .entries
                        .iter()
                        .map(|entry| (entry.language.clone(), entry.text.clone()))
                        .collect(),
                    params: tr.args.iter().map(|arg| arg.name.to_string()).collect(),
                };
                self.texts.push(text.clone());
                Some(ir::Text::Localized { text, args })
            }
            ast::Expr::Literal {
                value: Literal::Text(literal),
                span,
            } => {
                if self.strict {
                    self.error(
                        *span,
                        "error[B103]: user-visible text must be declared with its translations; use `tr! { … }`",
                    );
                    return None;
                }
                let text = LocalizedText {
                    key,
                    translations: BTreeMap::from([(
                        self.default_language.to_string(),
                        literal.clone(),
                    )]),
                    params: vec![],
                };
                self.texts.push(text.clone());
                Some(ir::Text::Localized { text, args: vec![] })
            }
            ast::Expr::Path(path) if !localized_only => {
                let lowered = self.binding(path, scope)?;
                if !is_displayable(&lowered.ty, self.types) {
                    self.error(
                        path.span(),
                        format!(
                            "{} has no text presentation; use `if`, `match` or a labelled enum",
                            lowered.ty.display()
                        ),
                    );
                    return None;
                }
                Some(ir::Text::Dynamic { expr: lowered })
            }
            other => {
                self.error(
                    other.span(),
                    if localized_only {
                        "expected `tr! { … }` text"
                    } else {
                        "expected `tr! { … }` text or a displayable binding"
                    },
                );
                None
            }
        }
    }

    /// Lower `client.query(args)` against the protocol's query catalog.
    fn query(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::QueryBinding> {
        let ast::Expr::Query(call) = expr else {
            self.error(
                expr.span(),
                "expected a query call such as `app.search(draft.text)`",
            );
            return None;
        };
        let root = call.receiver[0].to_string();
        let namespace = match scope.get(&root) {
            Some(Root::Client(namespace)) if call.receiver.len() == 1 => namespace.clone(),
            _ => {
                self.error(
                    call.receiver[0].span(),
                    format!("`{root}` is not the typed engine client; queries are called on it"),
                );
                return None;
            }
        };
        let method = call.method.to_string();
        let id = format!("{namespace}.{method}");
        let Some(query) = self.protocol.commands.iter().position(|c| c.id == id) else {
            self.error(
                call.method.span(),
                format!(
                    "protocol `{namespace}` has no query `{method}`; declare it in the `#[queries]` trait"
                ),
            );
            return None;
        };
        let spec = self.protocol.commands[query].clone();
        if !spec.is_query() {
            self.error(
                call.method.span(),
                format!(
                    "`{method}` is a command; `query:` binds a read-only query from the \
                     `#[queries]` trait"
                ),
            );
            return None;
        }
        if spec.params.len() != call.args.len() {
            self.error(
                call.method.span(),
                format!(
                    "`{}` takes {} argument(s), found {}",
                    spec.id,
                    spec.params.len(),
                    call.args.len()
                ),
            );
            return None;
        }
        let mut args = Vec::new();
        for (arg, param) in call.args.iter().zip(&spec.params) {
            args.push(self.expect(arg, &param.ty, scope, &format!("argument `{}`", param.name))?);
        }
        Some(ir::QueryBinding {
            query,
            args,
            result: spec.result,
        })
    }

    fn action(&mut self, action: &ast::Action, scope: &Scope) -> Option<ir::Action> {
        let root = action.receiver[0].to_string();
        match scope.get(&root) {
            Some(Root::Client(namespace)) => {
                if action.receiver.len() != 1 {
                    self.error(
                        action.span,
                        "commands are called directly on the client, e.g. `app.save()`",
                    );
                    return None;
                }
                let method = action.method.to_string();
                let id = format!("{namespace}.{method}");
                let Some(command) = self.protocol.commands.iter().position(|c| c.id == id) else {
                    self.error(
                        action.method.span(),
                        format!(
                            "protocol `{namespace}` has no command `{method}`; register it in the command trait"
                        ),
                    );
                    return None;
                };
                let spec = self.protocol.commands[command].clone();
                if spec.is_query() {
                    self.error(
                        action.method.span(),
                        format!(
                            "`{method}` is a query: it only reads, so it is shown with \
                             `Query(query: {root}.{method}(…))`; actions run commands"
                        ),
                    );
                    return None;
                }
                if spec.params.len() != action.args.len() {
                    self.error(
                        action.method.span(),
                        format!(
                            "`{}` takes {} argument(s), found {}",
                            spec.id,
                            spec.params.len(),
                            action.args.len()
                        ),
                    );
                    return None;
                }
                let mut args = Vec::new();
                for (arg, param) in action.args.iter().zip(&spec.params) {
                    args.push(self.expect(
                        arg,
                        &param.ty,
                        scope,
                        &format!("argument `{}`", param.name),
                    )?);
                }
                Some(ir::Action::Engine { command, args })
            }
            Some(Root::Windows) => {
                if action.receiver.len() != 2 || !action.args.is_empty() {
                    self.error(
                        action.span,
                        "window actions look like `windows.settings.open()`",
                    );
                    return None;
                }
                let window = action.receiver[1].to_string();
                if !self.window_ids.contains(&window) {
                    self.error(
                        action.receiver[1].span(),
                        format!("window `{window}` is not declared"),
                    );
                    return None;
                }
                let op = match action.method.to_string().as_str() {
                    "open" => ir::WindowOp::Open,
                    "close" => ir::WindowOp::Close,
                    "activate" => ir::WindowOp::Activate,
                    other => {
                        self.error(
                            action.method.span(),
                            format!("windows support open, close and activate, not `{other}`"),
                        );
                        return None;
                    }
                };
                Some(ir::Action::Window { op, window })
            }
            Some(Root::Session(session)) => {
                if action.receiver.len() != 2 {
                    self.error(
                        action.span,
                        "session actions look like `draft.panel.set(true)` or `draft.panel.toggle()`",
                    );
                    return None;
                }
                let session = session.clone();
                let def = self.sessions.iter().find(|def| def.name == session)?;
                let name = action.receiver[1].to_string();
                let Some(field) = def.field_index(&name) else {
                    self.error(
                        action.receiver[1].span(),
                        format!("`{session}` has no field `{name}`"),
                    );
                    return None;
                };
                let ty = def.fields[field].ty.clone();
                let field = ir::FieldRef { session, field, ty };
                let op = match (action.method.to_string().as_str(), action.args.as_slice()) {
                    ("set", [value]) => ir::SessionOp::Set {
                        value: self.expect(value, &field.ty, scope, "the new value")?,
                    },
                    ("toggle", []) if field.ty == Type::Bool => ir::SessionOp::Toggle,
                    ("toggle", []) => {
                        self.error(
                            action.method.span(),
                            format!(
                                "`toggle()` flips a `bool`; `{name}` is {}",
                                field.ty.display()
                            ),
                        );
                        return None;
                    }
                    _ => {
                        self.error(
                            action.method.span(),
                            "session fields support `set(value)` and, for a `bool`, `toggle()`",
                        );
                        return None;
                    }
                };
                Some(ir::Action::Session { field, op })
            }
            Some(Root::Players) => {
                let player = self.player_ref(&action.receiver, scope)?;
                let method = action.method.to_string();
                let Some(op) = ir::MediaOp::ALL
                    .into_iter()
                    .find(|op| op.method() == method)
                else {
                    self.error(
                        action.method.span(),
                        format!(
                            "players support {}, not `{method}`",
                            ir::MediaOp::ALL.map(ir::MediaOp::method).join(", ")
                        ),
                    );
                    return None;
                };
                let params: Vec<(&str, Type)> = match op {
                    ir::MediaOp::Open => vec![(
                        "source",
                        Type::Record {
                            name: bern_media::MEDIA_SOURCE.into(),
                        },
                    )],
                    ir::MediaOp::Seek => vec![("position_ms", Type::Int)],
                    ir::MediaOp::SetVolume => vec![("volume", Type::Float)],
                    ir::MediaOp::SetMuted => vec![("muted", Type::Bool)],
                    ir::MediaOp::Play | ir::MediaOp::Pause => vec![],
                };
                if params.len() != action.args.len() {
                    self.error(
                        action.method.span(),
                        format!(
                            "`{}` takes {} argument(s), found {}",
                            op.command(),
                            params.len(),
                            action.args.len()
                        ),
                    );
                    return None;
                }
                let mut args = Vec::new();
                for (arg, (name, ty)) in action.args.iter().zip(&params) {
                    args.push(self.expect(arg, ty, scope, &format!("argument `{name}`"))?);
                }
                Some(ir::Action::Media { player, op, args })
            }
            _ => {
                self.error(
                    action.receiver[0].span(),
                    format!("unknown client `{root}`; actions call a typed client parameter"),
                );
                None
            }
        }
    }

    fn component_call(
        &mut self,
        call: &ast::ComponentCall,
        scope: &Scope,
    ) -> Option<ir::ComponentRef> {
        let name = call.name().to_string();
        let Some(signature) = self.signatures.get(&name).cloned() else {
            self.error(
                call.span,
                format!(
                    "unknown component `{name}`; declare it with #[component] in a listed source"
                ),
            );
            return None;
        };
        if call.path.len() > 1 {
            let written: Vec<String> = call.path[..call.path.len() - 1]
                .iter()
                .map(|ident| ident.to_string())
                .filter(|segment| segment != "crate" && segment != "self")
                .collect();
            if !signature.module.ends_with(&written) {
                self.error(
                    call.span,
                    format!(
                        "`{}` resolves to module `{}`; aliases and re-exports are not supported",
                        call.path_string(),
                        signature.module.join("::")
                    ),
                );
            }
        }
        let mut args = Vec::new();
        for arg in &call.args {
            let arg_name = arg.name.to_string();
            let Some((_, kind)) = signature.params.iter().find(|(param, kind)| {
                *param == arg_name && !matches!(kind, ir::ParamKind::Session { .. })
            }) else {
                self.error(
                    arg.name.span(),
                    format!("`{name}` has no parameter `{arg_name}`"),
                );
                continue;
            };
            let lowered = match kind {
                ir::ParamKind::Client { namespace } => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Client(given)) if given == namespace
                            ) =>
                    {
                        Some(ir::ComponentArg::Client)
                    }
                    other => {
                        self.error(
                            other.span(),
                            format!("pass the typed `{namespace}` client, e.g. `{namespace}: {namespace}`"),
                        );
                        None
                    }
                },
                ir::ParamKind::Windows => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Windows)
                            ) =>
                    {
                        Some(ir::ComponentArg::Windows)
                    }
                    other => {
                        self.error(
                            other.span(),
                            "pass the window handle, e.g. `windows: windows`",
                        );
                        None
                    }
                },
                ir::ParamKind::Players => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Players)
                            ) =>
                    {
                        Some(ir::ComponentArg::Players)
                    }
                    other => {
                        self.error(
                            other.span(),
                            "pass the player catalog, e.g. `players: players`",
                        );
                        None
                    }
                },
                ir::ParamKind::Value { ty } => self
                    .expect(&arg.value, ty, scope, &format!("argument `{arg_name}`"))
                    .map(|expr| ir::ComponentArg::Value { expr }),
                ir::ParamKind::Session { .. } => None,
            };
            if let Some(lowered) = lowered {
                args.push((arg_name, lowered));
            }
        }
        for (param, kind) in &signature.params {
            if !matches!(kind, ir::ParamKind::Session { .. })
                && !call.args.iter().any(|arg| arg.name == param)
            {
                self.error(
                    call.span,
                    format!("missing argument `{param}` for `{name}`"),
                );
            }
        }
        Some(ir::ComponentRef {
            component: name,
            args,
            source: self.loc(call.span),
        })
    }
}
