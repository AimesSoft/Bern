//! Elements: their shape, their props by kind and their children.

use super::{Checker, Ctx, FieldExpect, Root, Scope, is_labeled_enum};
use bern_ir::spec::{self, ChildShape, ComponentSpec, ElementShape, PropKind};
use bern_ir::{self as ir, text_key};
use bern_protocol::Type;
use bern_syntax as ast;
use std::collections::BTreeMap;

/// An element as lowered so far: what its props give before its
/// children are lowered.
struct Parts {
    /// The element's kind, such as `Button`.
    kind: String,
    props: BTreeMap<String, ir::Prop>,
    id: Option<String>,
    template: Option<ir::Template>,
    /// The context the children see.
    child_ctx: Ctx,
    /// Template variables pushed onto the scope for the children.
    pushed: usize,
}

impl Parts {
    /// Bind the element's template variable `var` to `ty` for its
    /// children.
    fn bind(&mut self, var: String, ty: Type, scope: &mut Scope) {
        scope.push(var.clone(), Root::Local(ty.clone(), false));
        self.pushed += 1;
        self.template = Some(ir::Template { var, ty });
    }
}

impl Checker<'_> {
    /// Check one element and lower it: its shape, the props that set what
    /// its children see, its other props, its children and its action.
    pub(super) fn element(
        &mut self,
        element: &ast::Element,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Option<ir::Element> {
        let name = element.name.to_string();
        let component = self.shape(element, &name, ctx)?;
        let mut parts = Parts {
            child_ctx: Ctx {
                // A repetition places its template where it stands itself.
                parent: if name == "ForEach" {
                    ctx.parent.clone()
                } else {
                    Some(name.clone())
                },
                is_root: false,
                selection: ctx.selection.clone(),
                route: ctx.route.clone(),
            },
            kind: name,
            props: BTreeMap::new(),
            id: None,
            template: None,
            pushed: 0,
        };
        self.context_props(element, component, ctx, scope, &mut parts);
        for prop in &element.props {
            let prop_name = prop.name.to_string();
            let Some(prop_spec) = component.prop(&prop_name) else {
                continue;
            };
            let Some(lowered) = self.prop(prop, prop_spec.kind, path, ctx, &parts, scope) else {
                continue;
            };
            if let ir::Prop::Id { value } = &lowered {
                parts.id = Some(value.clone());
            }
            parts.props.insert(prop_name, lowered);
        }

        if parts.kind == "Picker"
            && let Some(ir::Prop::Expr { expr }) = parts.props.get("selected")
            && !is_labeled_enum(&expr.ty, self.types)
        {
            self.error(
                element.span,
                "`selected` must be an enum whose variants all have labels",
            );
        }
        if parts.kind == "Picker"
            && matches!(parts.props.get("style"), Some(ir::Prop::Keyword { value }) if value == "chips")
        {
            // Chips are one control per option; `shape` and `height` size
            // the single button of a picker shown as a menu.
            for prop in ["shape", "height"] {
                if let Some(given) = element.prop(prop) {
                    self.error(
                        given.name.span(),
                        format!(
                            "`style: chips` shows a chip per option, so `{prop}:` has no button to size; leave it out"
                        ),
                    );
                }
            }
        }
        let children = self.children(element, path, &parts.child_ctx, scope);
        if parts.kind == "Tabs" {
            self.tab_accessory(element, &children);
        }
        if let (Some(ir::Prop::Number { value: min }), Some(ir::Prop::Number { value: max })) =
            (parts.props.get("min"), parts.props.get("max"))
            && min >= max
        {
            self.error(
                element.span,
                format!("`{}` needs `min` below `max`", parts.kind),
            );
        }
        for _ in 0..parts.pushed {
            scope.pop();
        }
        let action = element
            .action
            .as_ref()
            .and_then(|action| self.action(action, scope));
        Some(ir::Element {
            kind: parts.kind,
            id: parts.id,
            path: path.to_string(),
            props: parts.props,
            template: parts.template,
            children,
            action,
            source: self.loc(element.span),
        })
    }

    /// Check an element against its component's spec: its props, template,
    /// children, action and where it stands. `None` once reported.
    fn shape(
        &mut self,
        element: &ast::Element,
        name: &str,
        ctx: &Ctx,
    ) -> Option<&'static ComponentSpec> {
        let prop_names: Vec<String> = element
            .props
            .iter()
            .map(|prop| prop.name.to_string())
            .collect();
        // Items of navigation containers may be conditional or repeated:
        // what counts is the elements in each branch and template.
        let child_names: Option<Vec<Option<String>>> = element
            .children
            .as_ref()
            .map(|children| ast::child_names(children, spec::DYNAMIC_ITEMS.contains(&name)));
        let shape = ElementShape {
            name,
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
        let component = spec::component(name)?;
        if failed {
            return None;
        }
        Some(component)
    }

    /// The props that set what the children see, lowered before the
    /// others: selections, navigation paths, and the queries, items and
    /// routes a template variable stands for.
    fn context_props(
        &mut self,
        element: &ast::Element,
        component: &ComponentSpec,
        ctx: &Ctx,
        scope: &mut Scope,
        parts: &mut Parts,
    ) {
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
                        parts.child_ctx.selection = Some(field.ty.clone());
                        parts.props.insert(prop_name, ir::Prop::Field { field });
                    }
                }
                PropKind::PathField => {
                    if let Some(field) = self.session_field(&prop.value, scope, FieldExpect::Path) {
                        if let Type::List { inner } = &field.ty {
                            parts.child_ctx.route = Some((**inner).clone());
                        }
                        parts.props.insert(prop_name, ir::Prop::Field { field });
                    }
                }
                PropKind::Query => self.query_prop(element, prop, scope, parts),
                PropKind::Items => self.items_prop(element, prop, scope, parts),
                _ => {}
            }
        }
        if parts.kind == "Destination"
            && let Some(var) = &element.template
        {
            match &ctx.route {
                Some(route) => parts.bind(var.to_string(), route.clone(), scope),
                None => self.error(element.span, "`Destination` must be inside `Navigation`"),
            }
        }
    }

    /// A query's rows, or its one result, are what the template stands
    /// for.
    fn query_prop(
        &mut self,
        element: &ast::Element,
        prop: &ast::Prop,
        scope: &mut Scope,
        parts: &mut Parts,
    ) {
        let Some(query) = self.query(&prop.value, scope) else {
            return;
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
            parts.bind(var.to_string(), shown, scope);
        }
        parts
            .props
            .insert(prop.name.to_string(), ir::Prop::Query { query });
    }

    /// `items` is a list of records; the template stands for one.
    fn items_prop(
        &mut self,
        element: &ast::Element,
        prop: &ast::Prop,
        scope: &mut Scope,
        parts: &mut Parts,
    ) {
        let Some(expr) = self.expr(&prop.value, scope) else {
            return;
        };
        let item_ty = match &expr.ty {
            Type::List { inner } if matches!(**inner, Type::Record { .. }) => (**inner).clone(),
            other => {
                self.error(
                    prop.value.span(),
                    format!("`items` needs a list of records, found {}", other.display()),
                );
                return;
            }
        };
        if let Some(var) = &element.template {
            parts.bind(var.to_string(), item_ty, scope);
        }
        parts
            .props
            .insert(prop.name.to_string(), ir::Prop::Expr { expr });
    }

    /// Lower one prop by its kind; the props `context_props` lowers are
    /// skipped here.
    fn prop(
        &mut self,
        prop: &ast::Prop,
        kind: PropKind,
        path: &str,
        ctx: &Ctx,
        parts: &Parts,
        scope: &Scope,
    ) -> Option<ir::Prop> {
        let name = parts.kind.as_str();
        let prop_name = prop.name.to_string();
        let value = &prop.value;
        match kind {
            PropKind::Id => self.id(value).map(|value| ir::Prop::Id { value }),
            PropKind::Keyword(allowed) => self.keyword(value, allowed),
            PropKind::Flag => self.flag(value),
            PropKind::DisplayText => self
                .display(
                    value,
                    scope,
                    text_key(&self.component, path, &prop_name),
                    false,
                )
                .map(|text| ir::Prop::Text { text }),
            PropKind::LocalizedText => self
                .display(
                    value,
                    scope,
                    text_key(&self.component, path, &prop_name),
                    true,
                )
                .map(|text| ir::Prop::Text { text }),
            PropKind::Icon => self.icon(value),
            PropKind::Shortcut => self.shortcut(value),
            // A picture is a declared resource, or a `MediaSource`
            // the engine gives: a file or an http(s) URL.
            PropKind::SvgResource => self.svg_resource(value),
            PropKind::ImageResource => match value {
                ast::Expr::Path(_) => self.media_source(value, scope),
                _ => self.image_resource(value),
            },
            // A literal length is checked like a number; any other
            // numeric expression is a length that follows it.
            PropKind::Length
                if !matches!(value, ast::Expr::Literal { .. } | ast::Expr::Unary { .. }) =>
            {
                self.length(&prop_name, value, scope)
            }
            PropKind::Number | PropKind::Length => self.literal_number(name, &prop_name, value),
            PropKind::NumberValue => self.number_value(&prop_name, value, scope),
            PropKind::Bool => self
                .expect(value, &Type::Bool, scope, "this property")
                .map(|expr| ir::Prop::Expr { expr }),
            PropKind::TextField => self.field_prop(value, scope, FieldExpect::Text),
            PropKind::BoolField => self.field_prop(value, scope, FieldExpect::Bool),
            PropKind::NumberField => self.field_prop(value, scope, FieldExpect::Number),
            PropKind::FileField => self.field_prop(value, scope, FieldExpect::File),
            PropKind::PositionField => self.field_prop(value, scope, FieldExpect::Position),
            PropKind::FileKinds => self.file_kinds(value),
            PropKind::PickerField => self.field_prop(value, scope, FieldExpect::Picker),
            // A container's `fallback` is one of its own values.
            PropKind::SelectionValue => {
                let selection = if prop_name == "fallback" {
                    parts.child_ctx.selection.clone()
                } else {
                    ctx.selection.clone()
                };
                self.selection_value(value, selection, scope)
            }
            PropKind::Route => self.route(value, ctx.route.clone(), scope),
            PropKind::KeyField => self.key(value, parts.template.as_ref()),
            PropKind::WindowId => self.window_id(value, name, &prop_name),
            // Slots are expanded before elements are lowered.
            PropKind::SlotName => None,
            PropKind::Transition | PropKind::MotionToken | PropKind::HeroName => {
                self.style_name(kind, &prop_name, value)
            }
            PropKind::ColorRole => self.color_role(value),
            PropKind::PatternName => self.pattern_name(value),
            PropKind::NumberList => self.number_list(&prop_name, value, scope),
            PropKind::KeyValue => self.key_value(&prop_name, value, scope),
            PropKind::StyleVariant => self.style_variant(name, value),
            PropKind::TypeName => self.type_name(value),
            PropKind::MediaSource => self.media_source(value, scope),
            PropKind::Player => self.player(value, scope),
            PropKind::Feature => self.feature(value),
            PropKind::SelectionField | PropKind::PathField | PropKind::Items | PropKind::Query => {
                None
            }
        }
    }

    /// Lower an element's children with the context its props set.
    fn children(
        &mut self,
        element: &ast::Element,
        path: &str,
        child_ctx: &Ctx,
        scope: &mut Scope,
    ) -> Vec<ir::Node> {
        match (&element.children, element.name.to_string().as_str()) {
            // The layouts of one `Adaptive` are alternatives: the same
            // control id in two of them names the same control.
            (Some(children), "Adaptive") => {
                let before = self.ids.clone();
                let mut all = before.clone();
                let mut lowered = Vec::new();
                for (index, child) in children.iter().enumerate() {
                    self.ids = before.clone();
                    lowered.extend(self.node(child, &format!("{path}.{index}"), child_ctx, scope));
                    all.append(&mut self.ids);
                }
                self.ids = all;
                self.adaptive(element, &lowered);
                lowered
            }
            (Some(children), _) => self.nodes(children, path, child_ctx, scope),
            (None, _) => Vec::new(),
        }
    }

    /// A `Tabs` keeps at most one `TabAccessory`, written directly in it:
    /// the bar is there for every page, and `shown:` hides it.
    fn tab_accessory(&mut self, element: &ast::Element, children: &[ir::Node]) {
        let direct = children
            .iter()
            .filter(|node| matches!(node, ir::Node::Element(child) if child.kind == "TabAccessory"))
            .count();
        // The tabs' own items: through conditions and `ForEach`, not into
        // the pages, which may hold tabs of their own.
        fn count(nodes: &[ir::Node]) -> usize {
            nodes
                .iter()
                .map(|node| match node {
                    ir::Node::Element(element) if element.kind == "TabAccessory" => 1,
                    ir::Node::Element(element) if element.kind == "ForEach" => {
                        count(&element.children)
                    }
                    ir::Node::Element(_) | ir::Node::Component(_) => 0,
                    ir::Node::If(node) => {
                        node.branches
                            .iter()
                            .map(|(_, body)| count(body))
                            .sum::<usize>()
                            + count(&node.otherwise)
                    }
                    ir::Node::IfLet(node) => count(&node.then) + count(&node.otherwise),
                    ir::Node::Match(node) => node.arms.iter().map(|arm| count(&arm.body)).sum(),
                })
                .sum()
        }
        let all = count(children);
        if direct > 1 {
            self.error(element.span, "`Tabs` keeps one `TabAccessory`");
        } else if all > direct {
            self.error(
                element.span,
                "write a `TabAccessory` directly in its `Tabs` and hide it with `shown:`, not inside `if` or `ForEach`",
            );
        }
    }

    /// An `Adaptive` has one `Layout` per size, and needs `[layouts]`; the
    /// `tv` layout is optional.
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
        // A television's layout is chosen by the device, not the width:
        // optional, one at most, and without a breakpoint.
        let tv = sizes.iter().filter(|name| **name == "tv").count();
        if tv > 1 {
            self.error(
                element.span,
                format!("error[B401]: `Adaptive` has at most one `Layout(size: tv)`, found {tv}"),
            );
        }
    }
}
