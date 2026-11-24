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
    /// Record type of `items:` (present for template lists and dynamic `Picker`).
    item_ty: Option<Type>,
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
            item_ty: None,
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

        if parts.kind == "Picker" {
            if let Some(Type::Record { name: rec_name }) = &parts.item_ty {
                if let Some(ir::Prop::Key { field: key_idx, .. }) = parts.props.get("key")
                    && let Some(def) = self.types.record_def(rec_name)
                    && let Some(key_field) = def.fields.get(*key_idx)
                {
                    let key_ty = &key_field.ty;
                    if let Some(ir::Prop::Field { field }) = parts.props.get("value")
                        && &field.ty != key_ty
                    {
                        let span = element
                            .prop("value")
                            .map_or(element.span, |p| p.value.span());
                        self.error(
                            span,
                            format!(
                                "`value` must match the `key` field's type ({}), found {}",
                                key_ty.display(),
                                field.ty.display()
                            ),
                        );
                    }
                    if let Some(ir::Prop::Expr { expr }) = parts.props.get("selected")
                        && &expr.ty != key_ty
                    {
                        let span = element
                            .prop("selected")
                            .map_or(element.span, |p| p.value.span());
                        self.error(
                            span,
                            format!(
                                "`selected` must match the `key` field's type ({}), found {}",
                                key_ty.display(),
                                expr.ty.display()
                            ),
                        );
                    }
                }
            } else if let Some(ir::Prop::Expr { expr }) = parts.props.get("selected")
                && !is_labeled_enum(&expr.ty, self.types)
            {
                self.error(
                    element.span,
                    "`selected` must be an enum whose variants all have labels",
                );
            }
        }
        if parts.kind == "Picker"
            && matches!(parts.props.get("style"), Some(ir::Prop::Keyword { value }) if value == "chips")
        {
            // Chips are one control per option; `height` sizes the single
            // button of a picker shown as a menu. `shape: capsule` draws
            // each chip as a capsule.
            if let Some(given) = element.prop("height") {
                self.error(
                    given.name.span(),
                    "`style: chips` shows a chip per option, so `height:` has no button to size; leave it out",
                );
            }
        }
        // The shortcut a user chooses is state: the action keeps it.
        if parts.kind == "ShortcutInput"
            && let Some(ir::Prop::Keyword { .. }) = parts.props.get("current")
        {
            self.error(
                element
                    .prop("current")
                    .map_or(element.span, |prop| prop.value.span()),
                "`current` is the shortcut the state holds, such as `app.keys.open`; the action keeps the one chosen",
            );
        }
        if parts.kind == "Shortcut"
            && let Some(ir::Prop::Number { value }) = parts.props.get("hold_after")
        {
            let span = element
                .prop("hold_after")
                .map_or(element.span, |prop| prop.value.span());
            if !parts.props.contains_key("hold") {
                self.error(span, "`hold_after` on `Shortcut` requires `hold`");
            } else if !value.is_finite() || *value <= 0.0 {
                self.error(span, "`hold_after` on `Shortcut` must be greater than 0");
            }
        }
        if parts.kind == "TreeView"
            && let Some(Type::Record { name: rec_name }) = parts.item_ty.as_ref()
            && let Some(def) = self.types.record_def(rec_name)
            && let Some(ir::Prop::Key { field: key_idx, .. }) = parts.props.get("key")
            && let Some(key_field) = def.fields.get(*key_idx)
        {
            let key_ty = &key_field.ty;
            if let Some(ir::Prop::Key {
                field: parent_idx, ..
            }) = parts.props.get("parent")
                && let Some(parent_field) = def.fields.get(*parent_idx)
                && let Type::Optional { inner } = &parent_field.ty
                && inner.as_ref() != key_ty
            {
                let span = element
                    .prop("parent")
                    .map_or(element.span, |p| p.value.span());
                self.error(
                    span,
                    format!(
                        "`parent` must be `Option<{}>` to match `key`, found {}",
                        key_ty.display(),
                        parent_field.ty.display()
                    ),
                );
            }
            if let Some(ir::Prop::Field { field }) = parts.props.get("selection")
                && &field.ty != key_ty
            {
                let span = element
                    .prop("selection")
                    .map_or(element.span, |p| p.value.span());
                self.error(
                    span,
                    format!(
                        "`selection` must match the `key` field's type ({}), found {}",
                        key_ty.display(),
                        field.ty.display()
                    ),
                );
            }
            if let Some(ir::Prop::Field { field }) = parts.props.get("expanded")
                && let Type::List { inner } = &field.ty
                && inner.as_ref() != key_ty
            {
                let span = element
                    .prop("expanded")
                    .map_or(element.span, |p| p.value.span());
                self.error(
                    span,
                    format!(
                        "`expanded` must be `Vec<{}>` to match `key`, found {}",
                        key_ty.display(),
                        field.ty.display()
                    ),
                );
            }
        }
        if parts.kind == "Tab"
            && matches!(parts.props.get("display"), Some(ir::Prop::Keyword { value }) if value == "icon")
            && !parts.props.contains_key("icon")
        {
            self.error(element.span, "a `Tab` with `display: icon` needs an `icon`");
        }
        if parts.kind == "Search"
            && matches!(parts.props.get("display"), Some(ir::Prop::Keyword { value }) if value == "icon")
            && !matches!(parts.props.get("style"), Some(ir::Prop::Keyword { value }) if value == "palette")
        {
            self.error(
                element.span,
                "`display: icon` is for `Search(style: palette)`, whose trigger is a button",
            );
        }
        let children = if parts.kind == "Query" {
            let (header, body) =
                ast::query_children(element.children.as_deref().unwrap_or_default());
            let mut lowered = Vec::new();
            // The two layouts never coexist. A control may use the same id in
            // both, but never twice within one, or anywhere else on the page.
            let header_ids = header.map(|header| {
                let before = self.ids.clone();
                lowered.extend(self.node(
                    header,
                    &format!("{path}.status"),
                    &parts.child_ctx,
                    scope,
                ));
                std::mem::replace(&mut self.ids, before)
            });
            if let (Some(ir::Prop::Query { query }), Some(var)) =
                (parts.props.get("query"), &element.template)
            {
                let shown = match &query.result {
                    Type::Optional { inner } => (**inner).clone(),
                    other => other.clone(),
                };
                parts.bind(var.to_string(), shown, scope);
            }
            lowered.extend(self.nodes(body, path, &parts.child_ctx, scope));
            if let Some(mut ids) = header_ids {
                self.ids.append(&mut ids);
            }
            lowered
        } else {
            self.children(element, path, &parts.child_ctx, scope)
        };
        if parts.kind == "Tabs" {
            self.tab_accessory(element, &children);
        }
        if parts.kind == "Detail" {
            self.detail_footer(element, &children);
        }
        if parts.kind == "Table"
            && let Some(template) = &parts.template
        {
            self.table_parts(element, &template.var, &children);
        }
        if parts.kind == "NativeView" {
            self.native_view_parts(element, &children);
        }
        if parts.kind == "Alert" {
            let cancels = children
                .iter()
                .filter(|child| {
                    matches!(
                        child,
                        ir::Node::Element(action)
                            if action.kind == "AlertAction"
                                && action.keyword("role") == Some("cancel")
                    )
                })
                .count();
            if cancels > 1 {
                self.error(
                    element.span,
                    "an `Alert` has at most one `AlertAction` with `role: cancel`",
                );
            }
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
            skeleton: None,
        })
    }

    /// The headings of a table's columns and its row menu belong to no
    /// row: they must not read the row variable `var`.
    fn table_parts(&mut self, element: &ast::Element, var: &str, children: &[ir::Node]) {
        let reads_row = |expr: &ir::Expr| {
            let mut found = false;
            expr.walk(&mut |e| {
                if let ir::ExprKind::Binding {
                    binding: ir::Binding::Local { name, .. },
                } = &e.kind
                    && name == var
                {
                    found = true;
                }
            });
            found
        };
        let prop_reads = |prop: &ir::Prop| match prop {
            ir::Prop::Expr { expr } => reads_row(expr),
            ir::Prop::Text { text } => text.exprs().into_iter().any(reads_row),
            _ => false,
        };
        let action_reads = |action: &Option<ir::Action>| match action {
            Some(ir::Action::Engine { args, .. }) => args.iter().any(reads_row),
            Some(ir::Action::Session {
                op: ir::SessionOp::Set { value } | ir::SessionOp::PushStr { value },
                ..
            }) => reads_row(value),
            _ => false,
        };
        let mut misplaced = None;
        for child in children {
            let ir::Node::Element(part) = child else {
                continue;
            };
            if part.kind == "TableColumn"
                && (part.props.values().any(prop_reads) || action_reads(&part.action))
            {
                misplaced = Some("a column's heading and sort action");
            }
            if part.kind == "TableMenu" {
                child.walk(&mut |node| {
                    if let ir::Node::Element(item) = node
                        && (item.props.values().any(prop_reads) || action_reads(&item.action))
                    {
                        misplaced = Some("the row menu");
                    }
                });
            }
        }
        if let Some(what) = misplaced {
            self.error(
                element.span,
                format!(
                    "{what} belong to no row, so they cannot read `{var}`; act on the table's `selection`"
                ),
            );
        }
    }

    /// Check name and entry-id uniqueness inside one `NativeView` so the
    /// generated per-backend `Props` / `Actions` structs never collide.
    fn native_view_parts(&mut self, element: &ast::Element, children: &[ir::Node]) {
        let mut prop_names = std::collections::BTreeSet::new();
        let mut entry_ids = std::collections::BTreeSet::new();
        for child in children {
            let ir::Node::Element(part) = child else {
                continue;
            };
            match part.kind.as_str() {
                "NativeValue" | "NativeText" | "NativeDraft" => {
                    if let Some(ir::Prop::Id { value: name }) = part.prop("name")
                        && !prop_names.insert(name.clone())
                    {
                        self.error(
                            element.span,
                            format!("`NativeView` prop name `{name}` is used twice"),
                        );
                    }
                }
                "NativeAction" | "NativeNumber" | "NativeBool" | "NativeInput" => {
                    if let Some(id) = &part.id {
                        let snake = ir::native::snake_ident(id);
                        if crate::native::is_reserved_native_ident(&snake) {
                            self.error(
                                element.span,
                                format!(
                                    "entry id `{id}` maps to reserved identifier `{snake}`; pick another id"
                                ),
                            );
                        }
                        if !entry_ids.insert(snake.clone()) {
                            self.error(
                                element.span,
                                format!("`NativeView` entry id `{id}` is used twice"),
                            );
                        }
                        if matches!(
                            part.kind.as_str(),
                            "NativeNumber" | "NativeBool" | "NativeInput"
                        ) && !prop_names.insert(snake)
                        {
                            self.error(
                                element.span,
                                format!(
                                    "`NativeView` entry `{id}` collides with a `NativeValue`/`NativeText`/`NativeDraft` of the same name"
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
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
        let keywords: Vec<(String, String)> = element
            .props
            .iter()
            .filter_map(|prop| Some((prop.name.to_string(), prop.keyword()?)))
            .collect();
        let shape = ElementShape {
            name,
            props: prop_names.iter().map(String::as_str).collect(),
            keywords: keywords
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect(),
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
        scope: &Scope,
        parts: &mut Parts,
    ) {
        let Some(query) = self.query(&prop.value, scope) else {
            return;
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
        parts.item_ty = Some(item_ty.clone());
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
        // A corner may name a radius of `style!`, alone or in its
        // expression; the platform's radius takes its place at build time.
        if prop_name == "corner" && matches!(kind, PropKind::Number | PropKind::Length) {
            if let Some(corner) = self.corner_name(value) {
                return Some(ir::Prop::Expr { expr: corner });
            }
            if let ast::Expr::Path(path) = value
                && let [name] = path.segments.as_slice()
                && scope.get(&name.to_string()).is_none()
            {
                let message = crate::style::unknown_corner(
                    &name.to_string(),
                    self.corners.iter().map(|(name, _)| name.as_str()),
                );
                self.error(path.span(), message);
                return None;
            }
            if matches!(kind, PropKind::Length)
                && !matches!(value, ast::Expr::Literal { .. } | ast::Expr::Unary { .. })
            {
                self.corner_names = true;
                let lowered = self.length(&prop_name, value, scope);
                self.corner_names = false;
                return lowered;
            }
        }
        match kind {
            PropKind::Id => {
                let scoped_to_native = ctx.parent.as_deref() == Some("NativeView")
                    && ir::native::NATIVE_ENTRIES.contains(&name);
                self.id(value, scoped_to_native)
                    .map(|value| ir::Prop::Id { value })
            }
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
            PropKind::Shortcut => self.shortcut(value, scope),
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
            PropKind::TextValue => self
                .expect(value, &Type::Text, scope, "this property")
                .map(|expr| ir::Prop::Expr { expr }),
            PropKind::TextField => self.field_prop(value, scope, FieldExpect::Text),
            PropKind::BoolField => self.field_prop(value, scope, FieldExpect::Bool),
            PropKind::NumberField => self.field_prop(value, scope, FieldExpect::Number),
            PropKind::IntField => self.field_prop(value, scope, FieldExpect::Int),
            PropKind::OptionalNumberField => {
                self.field_prop(value, scope, FieldExpect::OptionalNumber)
            }
            PropKind::FileField => self.field_prop(value, scope, FieldExpect::File),
            PropKind::SaveFileField => self.field_prop(value, scope, FieldExpect::SaveFile),
            PropKind::DateField => self.field_prop(value, scope, FieldExpect::Date),
            PropKind::DateValue => self
                .expect(
                    value,
                    &Type::Record {
                        name: bern_protocol::format::CALENDAR_DATE.into(),
                    },
                    scope,
                    "a date",
                )
                .map(|expr| ir::Prop::Expr { expr }),
            PropKind::PositionField => self.field_prop(value, scope, FieldExpect::Position),
            PropKind::KeysField => self.field_prop(value, scope, FieldExpect::Keys),
            PropKind::FileKinds => self.file_kinds(value),
            PropKind::PickerField => {
                let expect = if name == "Picker" && parts.item_ty.is_some() {
                    FieldExpect::Position
                } else {
                    FieldExpect::Picker
                };
                self.field_prop(value, scope, expect)
            }
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
            PropKind::KeyField => {
                let item_ty = parts
                    .item_ty
                    .as_ref()
                    .or_else(|| parts.template.as_ref().map(|t| &t.ty));
                if prop_name == "item_label" {
                    self.label_field(value, item_ty)
                } else if prop_name == "parent" {
                    self.parent_key(value, item_ty)
                } else {
                    self.key(value, item_ty)
                }
            }
            PropKind::WindowId => self.window_id(value, name, &prop_name),
            // Slots are expanded before elements are lowered.
            PropKind::SlotName => None,
            PropKind::Transition | PropKind::MotionToken | PropKind::HeroName => {
                self.style_name(kind, &prop_name, value)
            }
            PropKind::ColorRole if name == "Text" && prop_name == "color" => self.text_color(value),
            PropKind::ColorRole => self.color_role(value),
            PropKind::PatternName => self.pattern_name(value),
            PropKind::NumberList => self.number_list(&prop_name, value, scope),
            PropKind::KeyValue => self.key_value(&prop_name, value, scope),
            PropKind::Watched => self.watched(&prop_name, value, scope),
            PropKind::StyleVariant => self.style_variant(name, value),
            PropKind::TypeName => self.type_name(value),
            PropKind::MediaSource => self.media_source(value, scope),
            PropKind::Player => self.player(value, scope),
            PropKind::Feature => self.feature(value),
            PropKind::NativeFile => self.native_file(&prop_name, value),
            PropKind::NativeName => self.native_name(value),
            PropKind::NativeScalar => self.native_scalar(&prop_name, value, scope),
            PropKind::NativeDraftField => self.field_prop(value, scope, FieldExpect::NativeDraft),
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

    /// A `Detail` keeps at most one `DetailFooter`, written directly in it:
    /// conditional content belongs inside the footer rather than around it.
    fn detail_footer(&mut self, element: &ast::Element, children: &[ir::Node]) {
        let direct = children
            .iter()
            .filter(|node| matches!(node, ir::Node::Element(child) if child.kind == "DetailFooter"))
            .count();
        fn count(nodes: &[ir::Node]) -> usize {
            nodes
                .iter()
                .map(|node| match node {
                    ir::Node::Element(element) if element.kind == "DetailFooter" => 1,
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
        if all > direct {
            self.error(
                element.span,
                "write a `DetailFooter` directly in its `Detail`, not inside `if`, `match` or `ForEach`",
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
        // A television's or watch's layout is chosen by the device, not the
        // width: optional, one at most, and without a breakpoint.
        let tv = sizes.iter().filter(|name| **name == "tv").count();
        if tv > 1 {
            self.error(
                element.span,
                format!("error[B401]: `Adaptive` has at most one `Layout(size: tv)`, found {tv}"),
            );
        }
        let watch = sizes.iter().filter(|name| **name == "watch").count();
        if watch > 1 {
            self.error(
                element.span,
                format!(
                    "error[B401]: `Adaptive` has at most one `Layout(size: watch)`, found {watch}"
                ),
            );
        }
    }
}
