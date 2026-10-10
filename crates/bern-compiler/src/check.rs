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
    Client,
    Windows,
    Players,
    Session(String),
    Local(Type),
}

#[derive(Default)]
struct Scope(Vec<(String, Root)>);

impl Scope {
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
                ir::ParamKind::Client => Root::Client,
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
        nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| self.node(node, &format!("{prefix}.{index}"), ctx, scope))
            .collect()
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
        let child_names: Option<Vec<Option<String>>> = element.children.as_ref().map(|children| {
            children
                .iter()
                .map(|child| match child {
                    ast::Node::Element(child) => Some(child.name.to_string()),
                    _ => None,
                })
                .collect()
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
            parent: Some(name.clone()),
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
                PropKind::DisplayText => self
                    .display(&prop.value, scope, key, false)
                    .map(|text| ir::Prop::Text { text }),
                PropKind::LocalizedText => self
                    .display(&prop.value, scope, key, true)
                    .map(|text| ir::Prop::Text { text }),
                PropKind::Icon => self.icon(&prop.value),
                PropKind::Number => self
                    .number(&prop.value)
                    .map(|value| ir::Prop::Number { value }),
                PropKind::Bool => self
                    .expect(&prop.value, &Type::Bool, scope, "this property")
                    .map(|expr| ir::Prop::Expr { expr }),
                PropKind::TextField => self.field_prop(&prop.value, scope, FieldExpect::Text),
                PropKind::BoolField => self.field_prop(&prop.value, scope, FieldExpect::Bool),
                PropKind::NumberField => self.field_prop(&prop.value, scope, FieldExpect::Number),
                PropKind::PickerField => self.field_prop(&prop.value, scope, FieldExpect::Picker),
                PropKind::SelectionValue => match ctx.selection.clone() {
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
            if count != 1 {
                self.error(
                    element.span,
                    format!(
                        "error[B401]: `Adaptive` needs exactly one `Layout(size: {})`, found {count}",
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
            Some(Root::Client) => {
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
            Root::Client => {
                let Some(first) = rest.first() else {
                    self.error(
                        path.span(),
                        "the client is not a value; read a state field such as `app.field`",
                    );
                    return None;
                };
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
        if !matches!(scope.get(&root), Some(Root::Client)) || call.receiver.len() != 1 {
            self.error(
                call.receiver[0].span(),
                format!("`{root}` is not the typed engine client; queries are called on it"),
            );
            return None;
        }
        let method = call.method.to_string();
        let Some(query) = self.protocol.commands.iter().position(|c| c.name == method) else {
            self.error(
                call.method.span(),
                format!(
                    "protocol `{}` has no query `{method}`; declare it in the `#[queries]` trait",
                    self.protocol.namespace
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
            Some(Root::Client) => {
                if action.receiver.len() != 1 {
                    self.error(
                        action.span,
                        "commands are called directly on the client, e.g. `app.save()`",
                    );
                    return None;
                }
                let method = action.method.to_string();
                let Some(command) = self.protocol.commands.iter().position(|c| c.name == method)
                else {
                    self.error(
                        action.method.span(),
                        format!(
                            "protocol `{}` has no command `{method}`; register it in the command trait",
                            self.protocol.namespace
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
                ir::ParamKind::Client => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Client)
                            ) =>
                    {
                        Some(ir::ComponentArg::Client)
                    }
                    other => {
                        self.error(other.span(), "pass the typed client, e.g. `app: app`");
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
