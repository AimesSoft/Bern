//! Rust generation from the IR.
//!
//! The generated module builds every component and window from the
//! runtime's standard components (`bern-gtk`). Expressions become Rust
//! expressions over schema values evaluated in a context `c`; locals
//! (component parameters, row variables, pattern bindings) live in the
//! context, so closures capture nothing and each binding re-runs only when
//! a cell it read changes.

use crate::style;
use bern_ir::style::{ColorRef, TransitionKind};
use bern_ir::{
    Action, Application, BinaryOp, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind,
    IfLetNode, IfNode, MatchNode, MediaOp, Node, ParamKind, Prop, SessionOp, Text, WindowChrome,
    WindowDef, WindowInitial, WindowOp, WindowRole,
};
use bern_protocol::{EnumDef, Type, TypeTable, Value};
use std::cell::Cell;

/// Indented line writer.
#[derive(Default)]
struct Out {
    lines: Vec<String>,
    indent: usize,
}

impl Out {
    fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.lines.push(String::new());
        } else {
            self.lines
                .push(format!("{}{}", "    ".repeat(self.indent), text));
        }
    }

    fn open(&mut self, text: impl AsRef<str>) {
        self.line(text);
        self.indent += 1;
    }

    fn close(&mut self, text: impl AsRef<str>) {
        self.indent = self.indent.saturating_sub(1);
        self.line(text);
    }

    fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

/// A Rust string literal.
pub fn string(value: &str) -> String {
    format!("{value:?}")
}

pub(crate) fn float(value: f64) -> String {
    let text = format!("{value:?}");
    if text.contains('.') || text.contains('e') || text.contains("inf") || text.contains("NaN") {
        text
    } else {
        format!("{text}.0")
    }
}

fn some(value: Option<String>) -> String {
    value
        .map(|value| format!("Some({value})"))
        .unwrap_or_else(|| "None".into())
}

/// A Rust constant name for a style name: `pop_in` → `POP_IN`.
pub(crate) fn constant(prefix: &str, name: &str) -> String {
    let mut out = format!("{prefix}_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

/// A color of `style!` for the runtime.
pub(crate) fn color(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!("b::Color::Role({})", string(role)),
        ColorRef::Fixed { light, dark } => format!(
            "b::Color::Fixed(({}, {}, {}), ({}, {}, {}))",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

/// A schema value as a Rust expression.
fn literal(value: &Value) -> String {
    let list = |values: &[Value]| values.iter().map(literal).collect::<Vec<_>>().join(", ");
    match value {
        Value::Unit => "V::Unit".into(),
        Value::Bool(value) => format!("V::Bool({value})"),
        Value::Int(value) => format!("V::Int({value})"),
        Value::Float(value) => format!("V::Float({})", float(*value)),
        Value::Text(value) => format!("V::Text({}.into())", string(value)),
        Value::Enum { variant, fields } => {
            format!(
                "V::Enum {{ variant: {variant}, fields: vec![{}] }}",
                list(fields)
            )
        }
        Value::Optional(None) => "V::Optional(None)".into(),
        Value::Optional(Some(value)) => format!("V::some({})", literal(value)),
        Value::Record(values) => format!("V::Record(vec![{}])", list(values)),
        Value::List(values) => format!("V::List(vec![{}])", list(values)),
    }
}

/// Elements whose motion props wrap what they build: everything shown in
/// a layout except what builds content later or nothing at all.
fn decorates(kind: &str) -> bool {
    !matches!(
        kind,
        "ForEach" | "List" | "Toolbar" | "Toast" | "Inspector" | "Query"
    )
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether the nodes being generated are a sidebar's items.
    in_sidebar: Cell<bool>,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        Self {
            app,
            in_sidebar: Cell::new(false),
        }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    fn session_index(&self, name: &str) -> usize {
        self.app
            .sessions
            .iter()
            .position(|session| session.name == name)
            .expect("session")
    }

    fn path(expr: String, path: &[usize]) -> String {
        if path.is_empty() {
            expr
        } else {
            let indices: Vec<String> = path.iter().map(|index| index.to_string()).collect();
            format!("rt::at({expr}, &[{}])", indices.join(", "))
        }
    }

    fn binding(&self, binding: &Binding) -> String {
        match binding {
            Binding::Engine { field, path } => Self::path(format!("c.engine({field})"), path),
            Binding::Session {
                session,
                field,
                path,
            } => Self::path(
                format!("c.session({}, {field})", self.session_index(session)),
                path,
            ),
            Binding::Local { name, path, .. } => {
                Self::path(format!("c.local({})", string(name)), path)
            }
            // `window.idle` and the window's layout size, kept per window.
            Binding::Window { field } => format!("c.window({})", string(field)),
        }
    }

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value),
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let values: Vec<_> = fields.iter().map(|field| self.expr(field)).collect();
                format!("rt::variant({variant}, vec![{}])", values.join(", "))
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => format!(
                "(if {} {{ {} }} else {{ {} }})",
                self.condition(cond),
                self.expr(then),
                self.expr(otherwise)
            ),
            ExprKind::Not { expr } => format!("rt::not({})", self.expr(expr)),
            ExprKind::Neg { expr } => format!("rt::neg({})", self.expr(expr)),
            ExprKind::Binary { op, lhs, rhs } => {
                let (left, right) = (self.expr(lhs), self.expr(rhs));
                match op {
                    BinaryOp::And => format!("rt::and({left}, || {right})"),
                    BinaryOp::Or => format!("rt::or({left}, || {right})"),
                    other => {
                        let name = match other {
                            BinaryOp::Eq => "eq",
                            BinaryOp::Ne => "ne",
                            BinaryOp::Lt => "lt",
                            BinaryOp::Le => "le",
                            BinaryOp::Gt => "gt",
                            _ => "ge",
                        };
                        format!("rt::{name}({left}, {right})")
                    }
                }
            }
        }
    }

    fn condition(&self, expr: &Expr) -> String {
        format!("rt::truthy(&{})", self.expr(expr))
    }

    /// Display text of a value, by type.
    fn display(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Enum { name } => format!("b::display(&{value}, Some({}))", string(name)),
            _ => format!("rt::plain(&{value})"),
        }
    }

    /// A `tr!` argument: numbers keep their value for plural branches,
    /// dates stay dates for their patterns.
    fn argument(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Record { name } if name == bern_protocol::format::CALENDAR_DATE => {
                format!("b::text::date(&{value})")
            }
            Type::Enum { name } => format!("b::text::arg(&{value}, Some({}))", string(name)),
            _ => format!("b::text::arg(&{value}, None)"),
        }
    }

    fn text(&self, text: &Text) -> String {
        match text {
            Text::Localized { text, args } => {
                if text.params.is_empty() {
                    format!("b::text::tr({})", string(&text.key))
                } else {
                    let values: Vec<_> = text
                        .params
                        .iter()
                        .map(|param| {
                            let value = args
                                .iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| self.argument(expr))
                                .unwrap_or_else(|| "b::text::arg(&V::Unit, None)".into());
                            format!("({}, {value})", string(param))
                        })
                        .collect();
                    format!(
                        "b::text::fmt({}, &[{}])",
                        string(&text.key),
                        values.join(", ")
                    )
                }
            }
            Text::Dynamic { expr } => self.display(expr),
        }
    }

    fn text_getter(&self, text: &Text) -> String {
        format!("b::str(|c| {})", self.text(text))
    }

    fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text_getter(text))
    }

    fn getter(&self, expr: &Expr) -> String {
        format!("b::get(|c| {})", self.expr(expr))
    }

    fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    fn field_ref(&self, field: &bern_ir::FieldRef) -> String {
        format!(
            "b::Field {{ session: {}, field: {} }}",
            self.session_index(&field.session),
            field.field
        )
    }

    fn field(&self, element: &Element, name: &str) -> Option<String> {
        element.field(name).map(|field| self.field_ref(field))
    }

    /// A motion token's spring.
    fn motion(&self, name: &str) -> String {
        constant("M", name)
    }

    /// `animate`'s spring, for the bound lengths of an element.
    fn animate(&self, element: &Element) -> String {
        some(element.keyword("animate").map(|token| self.motion(token)))
    }

    /// A length: a number, or one bound to an expression.
    fn num(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(format!("b::Num::Fixed({})", float(*value))),
            Prop::Expr { expr } => Some(format!("b::Num::Bound({})", self.getter(expr))),
            _ => None,
        }
    }

    /// The style an element of `kind` uses, as a reference to `STYLES`.
    fn style(&self, kind: &str, element: &Element) -> String {
        some(
            style::resolve(self.app, style::PLATFORM, kind, element.keyword("variant"))
                .and_then(|resolved| {
                    style::used(self.app)
                        .iter()
                        .position(|used| used.name == resolved.name)
                })
                .map(|index| format!("&STYLES[{index}]")),
        )
    }

    fn pick(&self, element: &Element) -> String {
        some(self.field(element, "pick").map(|field| {
            format!(
                "b::Pick {{ field: {field}, kinds: {} }}",
                string(element.keyword("files").unwrap_or("any"))
            )
        }))
    }

    fn action_body(&self, action: &Action) -> String {
        match action {
            Action::Session { field, op } => {
                let target = self.field_ref(field);
                match op {
                    SessionOp::Set { value } => {
                        format!("b::session_set(c, {target}, {})", self.expr(value))
                    }
                    SessionOp::Toggle => format!("b::session_toggle(c, {target})"),
                }
            }
            Action::Engine { command, args } => {
                let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                format!(
                    "b::dispatch(c, {command}, vec![{}], None)",
                    values.join(", ")
                )
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => {
                    format!("b::open_window(c, {})", string(window))
                }
                WindowOp::Close => format!("b::close_window(c, {})", string(window)),
            },
            Action::Media { player, op, args } => {
                let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                let player = string(player);
                match op {
                    MediaOp::Open => format!(
                        "b::media_open(c, {player}, {})",
                        values.first().cloned().unwrap_or_else(|| "V::Unit".into())
                    ),
                    other => {
                        let name = match other {
                            MediaOp::Play => "Play",
                            MediaOp::Pause => "Pause",
                            MediaOp::Seek => "Seek",
                            MediaOp::SetVolume => "SetVolume",
                            _ => "SetMuted",
                        };
                        format!(
                            "b::media(c, b::MediaOp::{name}, {player}, vec![{}])",
                            values.join(", ")
                        )
                    }
                }
            }
        }
    }

    fn action(&self, element: &Element) -> String {
        some(
            element
                .action
                .as_ref()
                .map(|action| format!("b::act(|c| {})", self.action_body(action))),
        )
    }

    fn keyword(element: &Element, name: &str) -> String {
        some(element.keyword(name).map(string))
    }

    fn icon(element: &Element, name: &str) -> String {
        some(element.icon(name).map(string))
    }

    fn number(element: &Element, name: &str) -> String {
        some(element.number(name).map(float))
    }

    fn enabled(&self, element: &Element) -> String {
        some(element.expr("enabled").map(|expr| self.getter(expr)))
    }

    /// A bound value, as a getter.
    fn value(&self, element: &Element, name: &str) -> String {
        some(element.expr(name).map(|expr| self.getter(expr)))
    }

    /// `padding`, `padding_x` and `padding_y` of a container.
    fn padding(&self, element: &Element) -> String {
        format!(
            "b::Padding {{ all: {}, x: {}, y: {} }}",
            some(self.num(element, "padding")),
            some(self.num(element, "padding_x")),
            some(self.num(element, "padding_y")),
        )
    }

    /// What a control shows and changes: its draft field, after which its
    /// action runs; or the value `controlled` reads (`checked`, `selected`,
    /// `current`), which a change sets by running the action's command
    /// with the new value in place of that value.
    fn model(&self, element: &Element, controlled: &str, then: bool) -> Option<String> {
        if let Some(expr) = element.expr(controlled) {
            let commit = match element.action.as_ref()? {
                Action::Engine { command, args } => {
                    let values: Vec<_> = args
                        .iter()
                        .map(|arg| {
                            if arg == expr {
                                "v.clone()".to_string()
                            } else {
                                self.expr(arg)
                            }
                        })
                        .collect();
                    format!(
                        "b::dispatch(c, {command}, vec![{}], None)",
                        values.join(", ")
                    )
                }
                other => self.action_body(other),
            };
            return Some(format!(
                "b::Model::Controlled(b::get(|c| {}), Rc::new(|c: &b::Ctx, v: V| {{ {commit}; }}))",
                self.expr(expr)
            ));
        }
        let field = self.field(element, "value")?;
        Some(format!(
            "b::Model::Field({field}, {})",
            if then {
                self.action(element)
            } else {
                "None".into()
            }
        ))
    }

    /// The name of the enum variant a value names, for `ui.select`.
    fn variant_name(&self, expr: &Expr) -> String {
        let (ExprKind::Variant { variant, .. }, Type::Enum { name }) = (&expr.kind, &expr.ty)
        else {
            return "\"\"".into();
        };
        self.types()
            .enum_def(name)
            .and_then(|def| def.variants.get(*variant as usize))
            .map(|variant| string(&variant.name))
            .unwrap_or_else(|| "\"\"".into())
    }

    /// `shape: capsule` and `height` of a system control.
    fn shape(&self, element: &Element) -> String {
        format!(
            "b::Shape {{ capsule: {}, height: {} }}",
            element.keyword("shape") == Some("capsule"),
            some(self.num(element, "height")),
        )
    }

    /// Nodes in order; an `Inspector` among them takes the others as the
    /// content it pushes aside.
    fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        let inspector = nodes.iter().find_map(|node| match node {
            Node::Element(element) if element.kind == "Inspector" => Some(element),
            _ => None,
        });
        let Some(inspector) = inspector else {
            for node in nodes {
                self.node(out, node);
            }
            return;
        };
        let rest: Vec<Node> = nodes
            .iter()
            .filter(|node| !matches!(node, Node::Element(element) if element.kind == "Inspector"))
            .cloned()
            .collect();
        let props = format!(
            "b::InspectorProps {{ presented: {}, width: {} }}",
            self.field(inspector, "presented")
                .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
            Self::number(inspector, "width")
        );
        out.open(format!("b::Inspector(p, c, {props},"));
        self.children(out, "", &rest, ",");
        self.children(out, "", &inspector.children, ",");
        out.close(");");
    }

    /// A children builder: `b::build(|p, c| { … })`.
    fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        out.open(format!("{prefix}b::build(|p, c| {{"));
        self.nodes(out, nodes);
        out.close(format!("}}){suffix}"));
    }

    /// A container's children builder; with `stagger`, its children take
    /// turns appearing.
    fn container_children(&self, out: &mut Out, prefix: &str, element: &Element, suffix: &str) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!("{prefix}b::build(|p, c| {{"));
                out.open(format!("b::staggered({}, || {{", float(step)));
                self.nodes(out, &element.children);
                out.close("});");
                out.close(format!("}}){suffix}"));
            }
            None => self.children(out, prefix, &element.children, suffix),
        }
    }

    /// The spring a column or row's children slide with: its own `reflow`,
    /// or that of a `ForEach` whose rows are its children.
    fn reflow(element: &Element) -> Option<&str> {
        fn rows(nodes: &[Node]) -> Option<&str> {
            nodes.iter().find_map(|node| match node {
                Node::Element(child) if child.kind == "ForEach" => child.keyword("reflow"),
                Node::If(node) => node
                    .branches
                    .iter()
                    .find_map(|(_, body)| rows(body))
                    .or_else(|| rows(&node.otherwise)),
                Node::IfLet(node) => rows(&node.then).or_else(|| rows(&node.otherwise)),
                Node::Match(node) => node.arms.iter().find_map(|arm| rows(&arm.body)),
                _ => None,
            })
        }
        element
            .keyword("reflow")
            .or_else(|| rows(&element.children))
    }

    /// The decoration of an element's motion props, if it has any.
    fn decoration(&self, element: &Element) -> Option<String> {
        if !decorates(&element.kind) {
            return None;
        }
        let mut entries = Vec::new();
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("&{}", constant("X", transition)))
        };
        if let Some(appear) = transition("appear") {
            entries.push(format!("appear: Some({appear})"));
        }
        if let Some(disappear) = transition("disappear") {
            entries.push(format!("disappear: Some({disappear})"));
        }
        if let Some(hero) = element.keyword("hero") {
            let key = match element.expr("hero_key") {
                Some(key) => format!(
                    "b::get(|c| V::Text(format!(\"{{}}-{{}}\", {}, rt::key_text(&{}))))",
                    string(hero),
                    self.expr(key)
                ),
                None => format!("b::get(|_| V::Text({}.into()))", string(hero)),
            };
            entries.push(format!("hero: Some(({key}, {}))", self.motion("hero")));
        }
        if element.keyword("reveal") == Some("hover") {
            entries.push("reveal: true".into());
        }
        if let Some(appearance) = element.keyword("appearance") {
            entries.push(format!("appearance: Some({})", string(appearance)));
        }
        if let Some(delay) = element.number("appear_delay") {
            entries.push(format!("appear_delay: Some({})", float(delay)));
        }
        if let Some(when) = element.expr("nudge") {
            entries.push(format!(
                "nudge: Some(({}, {}))",
                self.getter(when),
                self.motion("jelly")
            ));
        }
        if let Some(key) = element.expr("nudge_key") {
            entries.push(format!(
                "nudge_key: Some(({}, {}))",
                self.getter(key),
                self.motion("jelly")
            ));
        }
        if let Some(scale) = element.number("nudge_scale") {
            entries.push(format!("nudge_scale: Some({})", float(scale)));
        }
        if let Some(reduce) = element.expr("reduce_motion") {
            entries.push(format!("reduce_motion: Some({})", self.getter(reduce)));
        }
        if let Some(scale) = element.number("scroll_scale") {
            entries.push(format!("scroll_scale: Some({})", float(scale)));
        }
        if let Some(opacity) = element.number("scroll_opacity") {
            entries.push(format!("scroll_opacity: Some({})", float(opacity)));
        }
        let origin = (
            element.number("scroll_origin_x"),
            element.number("scroll_origin_y"),
        );
        if origin.0.is_some() || origin.1.is_some() {
            entries.push(format!(
                "scroll_origin: Some(({}, {}))",
                float(origin.0.unwrap_or(0.5)),
                float(origin.1.unwrap_or(0.5))
            ));
        }
        (!entries.is_empty()).then(|| {
            format!(
                "b::Decor {{ {}, ..Default::default() }}",
                entries.join(", ")
            )
        })
    }

    /// Where an element sits on its `Rail`, if it says.
    fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| match element.prop(name)? {
            Prop::Expr { expr } => Some(format!("b::Num::Bound({})", self.getter(expr))),
            Prop::Number { value } => Some(format!("b::Num::Fixed({})", float(*value))),
            _ => None,
        };
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        (at.is_some() || from.is_some() || to.is_some()).then(|| {
            format!(
                "b::PlaceProps {{ at: {}, from: {}, to: {}, motion: {} }}",
                some(at),
                some(from),
                some(to),
                self.animate(element)
            )
        })
    }

    fn node(&self, out: &mut Out, node: &Node) {
        match node {
            Node::Element(element) => {
                let placement = self.placement(element);
                if let Some(spec) = &placement {
                    out.open(format!("b::Place(p, c, {spec}, b::build(|p, c| {{"));
                }
                // A modal's motion is its card's, above the window.
                let decoration = if element.kind == "Modal" {
                    None
                } else {
                    self.decoration(element)
                };
                match decoration {
                    Some(decor) => {
                        out.open(format!("b::Decorate(p, c, {decor}, b::build(|p, c| {{"));
                        self.element(out, element);
                        out.close("}));");
                    }
                    None => self.element(out, element),
                }
                if placement.is_some() {
                    out.close("}));");
                }
            }
            Node::Component(reference) => {
                let component = self.app.component(&reference.component).expect("component");
                let args: Vec<_> = component
                    .params
                    .iter()
                    .filter_map(|param| match &param.kind {
                        ParamKind::Value { .. } => reference
                            .args
                            .iter()
                            .find(|(name, _)| *name == param.name)
                            .and_then(|(_, arg)| match arg {
                                ComponentArg::Value { expr } => Some(format!(
                                    "({}, {})",
                                    string(&param.name),
                                    self.getter(expr)
                                )),
                                _ => None,
                            }),
                        _ => None,
                    })
                    .collect();
                out.line(format!(
                    "c_{}(p, &c.child({}, vec![{}]));",
                    component.name,
                    string(&component.name),
                    args.join(", ")
                ));
            }
            Node::If(node) => self.if_node(out, node),
            Node::IfLet(node) => self.if_let(out, node),
            Node::Match(node) => self.match_node(out, node),
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode) {
        let mut branch = String::new();
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            branch.push_str(&format!(
                "if {} {{ Some({index}) }} else ",
                self.condition(condition)
            ));
        }
        if node.otherwise.is_empty() {
            branch.push_str("{ None }");
        } else {
            branch.push_str(&format!("{{ Some({}) }}", node.branches.len()));
        }
        out.open(format!(
            "b::When(p, c, Rc::new(|c: &b::Ctx| {branch}), vec!["
        ));
        for (_, body) in &node.branches {
            self.children(out, "", body, ",");
        }
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, ",");
        }
        out.close("]);");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode) {
        out.open(format!(
            "b::IfLet(p, c, {}, {},",
            string(&node.var),
            self.getter(&node.value)
        ));
        self.children(out, "", &node.then, ",");
        if node.otherwise.is_empty() {
            out.line("None,");
        } else {
            self.children(out, "Some(", &node.otherwise, "),");
        }
        out.close(");");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode) {
        out.open(format!(
            "b::Match(p, c, {}, vec![",
            self.getter(&node.value)
        ));
        for arm in &node.arms {
            let bindings: Vec<_> = arm
                .bindings
                .iter()
                .map(|binding| format!("({}, {})", string(&binding.name), binding.field))
                .collect();
            out.open(format!(
                "b::MatchArm {{ variant: {}, bindings: vec![{}], build:",
                some(arm.variant.map(|variant| variant.to_string())),
                bindings.join(", ")
            ));
            self.children(out, "", &arm.body, ",");
            out.close("},");
        }
        out.close("]);");
    }

    /// The text a row shows first, used as the name of links.
    fn row_label(&self, nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            Node::Element(element) => match element.kind.as_str() {
                "Text" => element.text("content").map(|text| self.text(text)),
                "Label" => element.text("title").map(|text| self.text(text)),
                _ => self.row_label(&element.children),
            },
            Node::Match(node) => {
                let arms = node
                    .arms
                    .iter()
                    .map(|arm| {
                        let label = self.row_label(&arm.body)?;
                        Some(match arm.variant {
                            Some(variant) => format!("Some({variant}) => {label},"),
                            None => format!("_ => {label},"),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "match rt::tag(&{}) {{ {} _ => String::new() }}",
                    self.expr(&node.value),
                    arms.join(" ")
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// The enum a selection value belongs to.
    fn enum_name(expr: &Expr) -> String {
        match &expr.ty {
            Type::Enum { name } => string(name),
            _ => "\"\"".into(),
        }
    }

    /// The label getter of a variant: its declared label, or its name.
    fn variant_label(&self, def: &EnumDef, index: usize) -> String {
        let variant = &def.variants[index];
        if variant.label.is_some() {
            format!("b::str(|_| b::label({}, {index}))", string(&def.name))
        } else {
            format!("b::str(|_| {}.to_string())", string(&variant.name))
        }
    }

    /// `b::shortcut(…)` for an element with `shortcut`, running `run`.
    fn shortcut(&self, out: &mut Out, element: &Element, run: &str) {
        if let Some(keys) = element.keyword("shortcut") {
            out.line(format!(
                "b::shortcut(c, {}, b::act(|c| {run}));",
                string(keys)
            ));
        }
    }

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => out.line(format!(
                "b::Text(p, c, b::TextProps {{ content: {}, style: {}, emphasis: {}, mark: {}, lines: {}, font_size: {}, line_height: {} }});",
                some(self.text_prop(element, "content")),
                Self::keyword(element, "style"),
                Self::keyword(element, "emphasis"),
                some(
                    element
                        .keyword("mark")
                        .map(|role| format!("b::Color::Role({})", string(role)))
                ),
                Self::number(element, "lines"),
                some(self.num(element, "font_size")),
                some(self.num(element, "line_height")),
            )),
            "Label" => out.line(format!(
                "b::Label(p, c, b::LabelProps {{ title: {}, icon: {}, style: {}, font_size: {}, icon_only: {} }});",
                some(self.text_prop(element, "title")),
                Self::icon(element, "icon"),
                Self::keyword(element, "style"),
                Self::number(element, "font_size"),
                element.keyword("display") == Some("icon"),
            )),
            "Button" if !element.children.is_empty() => {
                self.children(
                    out,
                    &format!(
                        "b::ContentButton(p, c, b::ContentButtonProps {{ id: {}, label: {}, enabled: {}, style: {}, pick: {}, shape: {} }}, {}, ",
                        self.id(element),
                        some(self.text_prop(element, "label")),
                        self.enabled(element),
                        self.style("Button", element),
                        self.pick(element),
                        self.shape(element),
                        self.action(element),
                    ),
                    &element.children,
                    ");",
                );
                self.shortcut(
                    out,
                    element,
                    &format!("b::activate_target(c, {})", self.id(element)),
                );
            }
            "Button" => {
                out.line(format!(
                    "b::Button(p, c, b::ButtonProps {{ id: {}, label: {}, icon: {}, role: {}, prominence: {}, enabled: {}, style: {}, pick: {}, shape: {}, fill: {} }}, {});",
                    self.id(element),
                    some(self.text_prop(element, "label")),
                    Self::icon(element, "icon"),
                    Self::keyword(element, "role"),
                    Self::keyword(element, "prominence"),
                    self.enabled(element),
                    self.style("Button", element),
                    self.pick(element),
                    self.shape(element),
                    element.keyword("width") == Some("fill"),
                    self.action(element),
                ));
                self.shortcut(
                    out,
                    element,
                    &format!("b::activate_target(c, {})", self.id(element)),
                );
            }
            "IconButton" => {
                out.line(format!(
                    "b::IconButton(p, c, b::IconButtonProps {{ id: {}, icon: {}, label: {}, enabled: {}, style: {}, pick: {}, font_size: {}, diameter: {}, prominence: {} }}, {});",
                    self.id(element),
                    string(element.icon("icon").unwrap_or("circle")),
                    some(self.text_prop(element, "label")),
                    self.enabled(element),
                    self.style("IconButton", element),
                    self.pick(element),
                    Self::number(element, "font_size"),
                    some(self.num(element, "height")),
                    Self::keyword(element, "prominence"),
                    self.action(element),
                ));
                self.shortcut(
                    out,
                    element,
                    &format!("b::activate_target(c, {})", self.id(element)),
                );
            }
            "TextInput" => out.line(format!(
                "b::TextInput(p, c, b::TextInputProps {{ id: {}, label: {}, field: {}, prompt: {}, kind: {} }}, {});",
                self.id(element),
                some(self.text_prop(element, "label")),
                some(self.field(element, "value")),
                some(self.text_prop(element, "prompt")),
                Self::keyword(element, "kind"),
                self.action(element),
            )),
            "Search" => {
                let (function, run) = if element.keyword("style") == Some("palette") {
                    ("SearchPalette", "activate_target")
                } else {
                    ("Search", "focus_target")
                };
                if element.children.is_empty() {
                    out.line(format!(
                        "b::{function}(p, c, {}, {}, None);",
                        self.search_props(element),
                        self.action(element),
                    ));
                } else {
                    self.children(
                        out,
                        &format!(
                            "b::{function}(p, c, {}, {}, Some(",
                            self.search_props(element),
                            self.action(element),
                        ),
                        &element.children,
                        "));",
                    );
                }
                self.shortcut(
                    out,
                    element,
                    &format!("b::{run}(c, {})", self.id(element)),
                );
            }
            "SearchItem" => {
                let props = format!(
                    "b::SearchItemProps {{ id: {}, label: {}, icon: {}, detail: {}, hint: {}, navigation: {}, pick: {} }}",
                    self.id(element),
                    some(self.text_prop(element, "label")),
                    Self::icon(element, "icon"),
                    some(self.text_prop(element, "detail")),
                    some(self.text_prop(element, "hint")),
                    self.value(element, "navigation"),
                    self.pick(element),
                );
                if element.children.is_empty() {
                    out.line(format!(
                        "b::SearchItem(p, c, {props}, {}, None);",
                        self.action(element)
                    ));
                } else {
                    self.children(
                        out,
                        &format!("b::SearchItem(p, c, {props}, {}, Some(", self.action(element)),
                        &element.children,
                        "));",
                    );
                }
            }
            "Switch" => {
                let model = self.model(element, "checked", true);
                out.line(format!(
                    "b::Switch(p, c, b::SwitchProps {{ id: {}, label: {}, model: {}, style: {}, icon: {}, label_hidden: {}, shape: {} }});",
                    self.id(element),
                    some(self.text_prop(element, "label")),
                    some(model.clone()),
                    Self::keyword(element, "style"),
                    Self::icon(element, "icon"),
                    element.keyword("label_hidden") == Some("true"),
                    self.shape(element),
                ));
                if let Some(model) = model {
                    self.shortcut(out, element, &format!("b::toggle(c, &{model})"));
                }
            }
            "Slider" => out.line(format!(
                "b::Slider(p, c, b::SliderProps {{ id: {}, label: {}, model: {}, min: {}, max: {}, step: {}, min_icon: {}, max_icon: {}, label_hidden: {} }}, {});",
                self.id(element),
                some(self.text_prop(element, "label")),
                some(self.model(element, "current", false)),
                Self::number(element, "min"),
                Self::number(element, "max"),
                Self::number(element, "step"),
                Self::icon(element, "min_icon"),
                Self::icon(element, "max_icon"),
                element.keyword("label_hidden") == Some("true"),
                // A controlled slider's action is its change.
                if element.expr("current").is_some() {
                    "None".into()
                } else {
                    self.action(element)
                },
            )),
            "Picker" => self.picker(out, element),
            "LanguagePicker" => out.line(format!(
                "b::LanguagePicker(p, c, b::LanguagePickerProps {{ id: {}, label: {} }});",
                self.id(element),
                some(self.text_prop(element, "label")),
            )),
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => self.children(
                out,
                &format!(
                    "b::Form(p, c, b::FormProps {{ title: {}, gap: {}, row_height: {}, padding_x: {}, padding_y: {} }}, ",
                    some(self.text_prop(element, "title")),
                    Self::number(element, "gap"),
                    Self::number(element, "row_height"),
                    Self::number(element, "padding_x"),
                    Self::number(element, "padding_y"),
                ),
                &element.children,
                ");",
            ),
            "Section" if self.in_sidebar.get() => self.children(
                out,
                &format!(
                    "b::SidebarSection(p, c, {}, ",
                    some(self.text_prop(element, "title"))
                ),
                &element.children,
                ");",
            ),
            "Section" => self.children(
                out,
                &format!(
                    "b::Section(p, c, b::SectionProps {{ title: {}, footer: {}, plain: {}, padding_x: {}, padding_y: {} }}, ",
                    some(self.text_prop(element, "title")),
                    some(self.text_prop(element, "footer")),
                    element.keyword("style") == Some("plain"),
                    Self::number(element, "padding_x"),
                    Self::number(element, "padding_y"),
                ),
                &element.children,
                ");",
            ),
            "SidebarItem" => {
                let Some(value) = element.expr("value") else {
                    return;
                };
                out.line(format!(
                    "b::SidebarItem(p, c, b::SidebarItemProps {{ value: {}, enum_name: {}, label: {}, icon: {}, style: {}, badge: {} }});",
                    self.getter(value),
                    Self::enum_name(value),
                    some(self.text_prop(element, "label")),
                    Self::icon(element, "icon"),
                    self.style("SidebarItem", element),
                    some(self.num(element, "badge")),
                ));
            }
            "Column" | "Row" => self.container_children(
                out,
                &format!(
                    "b::Stack(p, c, b::StackProps {{ axis: {}, gap: {}, padding: {}, width: {}, height: {}, align: {}, animate: {}, reflow: {} }}, ",
                    string(if element.kind == "Column" { "column" } else { "row" }),
                    some(self.num(element, "gap")),
                    self.padding(element),
                    some(self.num(element, "width")),
                    some(self.num(element, "height")),
                    Self::keyword(element, "align"),
                    self.animate(element),
                    some(Self::reflow(element).map(|token| self.motion(token))),
                ),
                element,
                ");",
            ),
            "Grid" => self.container_children(
                out,
                &format!(
                    "b::Grid(p, c, b::GridProps {{ min_width: {}, gap: {}, row_gap: {}, padding: {}, animate: {}, cells: {}, reflow: {} }}, ",
                    some(self.num(element, "min_width")),
                    some(self.num(element, "gap")),
                    some(self.num(element, "row_gap")),
                    self.padding(element),
                    self.animate(element),
                    some(element.number("cell_ratio").map(|ratio| format!(
                        "({}, {})",
                        float(ratio),
                        self.num(element, "cell_extra").unwrap_or_else(|| "b::Num::Fixed(0.0)".into())
                    ))),
                    some(Self::reflow(element).map(|token| self.motion(token))),
                ),
                element,
                ");",
            ),
            "Stack" => self.container_children(
                out,
                &format!(
                    "b::Layers(p, c, b::LayersProps {{ align: {}, width: {}, height: {}, animate: {}, style: {}, enabled: {}, reflow: {} }}, ",
                    Self::keyword(element, "align"),
                    some(self.num(element, "width")),
                    some(self.num(element, "height")),
                    self.animate(element),
                    self.style("Stack", element),
                    self.enabled(element),
                    some(Self::reflow(element).map(|token| self.motion(token))),
                ),
                element,
                ");",
            ),
            "ScaleBox" => self.container_children(
                out,
                &format!(
                    "b::ScaleBox(p, c, b::ScaleBoxProps {{ width: {}, height: {} }}, ",
                    some(self.num(element, "width")),
                    some(self.num(element, "height")),
                ),
                element,
                ");",
            ),
            "Positioned" => self.container_children(
                out,
                &format!(
                    "b::Positioned(p, c, b::PositionedProps {{ x: {}, y: {}, width: {}, height: {}, animate: {} }}, ",
                    some(self.num(element, "x")),
                    some(self.num(element, "y")),
                    some(self.num(element, "width")),
                    some(self.num(element, "height")),
                    self.animate(element),
                ),
                element,
                ");",
            ),
            "Marquee" => self.children(
                out,
                &format!(
                    "b::Marquee(p, c, b::MarqueeProps {{ duration: {}, phase: {}, paused: {}, opacity: {} }}, ",
                    some(self.num(element, "duration")),
                    float(element.number("phase").unwrap_or(0.0)),
                    self.value(element, "paused"),
                    some(self.num(element, "opacity")),
                ),
                &element.children,
                ");",
            ),
            "Accordion" => self.children(
                out,
                &format!(
                    "b::Accordion(p, c, b::AccordionProps {{ count: {}, selected: {}, collapsed_width: {}, gap: {}, height: {}, corner: {}, animate: {} }}, ",
                    some(self.num(element, "count")),
                    some(self.num(element, "selected")),
                    some(self.num(element, "collapsed_width")),
                    some(self.num(element, "gap")),
                    some(self.num(element, "height")),
                    some(self.num(element, "corner")),
                    self.animate(element),
                ),
                &element.children,
                ");",
            ),
            "Flow" => self.container_children(
                out,
                &format!(
                    "b::Flow(p, c, b::FlowProps {{ gap: {} }}, ",
                    some(self.num(element, "gap")),
                ),
                element,
                ");",
            ),
            "Modal" => self.children(
                out,
                &format!(
                    "b::Modal(p, c, b::ModalProps {{ width: {}, height: {}, cover: {}, decor: {} }}, ",
                    some(self.num(element, "width")),
                    some(self.num(element, "height")),
                    element.keyword("cover") == Some("compact"),
                    some(self.decoration(element)),
                ),
                &element.children,
                ");",
            ),
            "Timer" => out.line(format!(
                "b::Timer(p, c, b::TimerProps {{ interval: {}, enabled: {} }}, {});",
                float(element.number("interval").unwrap_or(1.0)),
                self.enabled(element),
                self.action(element),
            )),
            "ControlGroup" => self.children(
                out,
                &format!(
                    "b::ControlGroup(p, c, b::ControlGroupProps {{ label: {}, height: {} }}, ",
                    some(self.text_prop(element, "label")),
                    some(self.num(element, "height")),
                ),
                &element.children,
                ");",
            ),
            "Svg" => out.line(format!(
                "b::Svg(p, c, b::SvgProps {{ source: {}, label: {}, width: {}, height: {}, fit: {}, playing: {}, speed: {}, restart: {}, interactive: {} }});",
                string(element.resource("source").unwrap_or_default()),
                some(self.text_prop(element, "label")),
                some(self.num(element, "width")), some(self.num(element, "height")),
                Self::keyword(element, "fit"),
                some(element.expr("playing").map(|e| self.getter(e))),
                some(element.expr("speed").map(|e| self.getter(e))),
                some(element.expr("restart").map(|e| self.getter(e))),
                element.keyword("interactive").unwrap_or("true"),
            )),
            "Image" => {
                let shared = format!(
                    "label: {}, width: {}, height: {}, fit: {}, animate: {}, corner: {}, template: {}, aspect: {}",
                    some(self.text_prop(element, "label")),
                    some(self.num(element, "width")),
                    some(self.num(element, "height")),
                    Self::keyword(element, "fit"),
                    self.animate(element),
                    some(self.num(element, "corner")),
                    element.keyword("rendering") == Some("template"),
                    Self::number(element, "aspect"),
                );
                match element.expr("source") {
                    Some(source) => out.line(format!(
                        "b::RuntimeImage(p, c, b::RuntimeImageProps {{ url: {}, {shared} }});",
                        self.getter(source),
                    )),
                    None => out.line(format!(
                        "b::Image(p, c, b::ImageProps {{ source: {}, {shared} }});",
                        string(element.resource("source").unwrap_or_default()),
                    )),
                }
            }
            "Progress" => out.line(format!(
                "b::Progress(p, c, b::ProgressProps {{ value: {}, total: {}, label: {} }});",
                some(element.expr("value").map(|value| self.getter(value))),
                Self::number(element, "total"),
                some(self.text_prop(element, "label")),
            )),
            "Spacer" => out.line("b::Spacer(p);"),
            "Divider" => out.line("b::Divider(p);"),
            "ScrollView" => self.children(
                out,
                &format!(
                    "b::ScrollView(p, c, b::ScrollProps {{ axis: {}, snap: {}, position: {}, arrival_nudge: {}, hide_indicators: {}, nudge_motion: {}, smooth: {}, open_at: {} }}, ",
                    Self::keyword(element, "axis"),
                    Self::keyword(element, "snap"),
                    some(self.field(element, "position")),
                    element.keyword("arrival") == Some("nudge"),
                    element.keyword("indicators") == Some("hidden"),
                    self.motion("jelly"),
                    self.motion("smooth"),
                    some(element.number("open_at").map(float)),
                ),
                &element.children,
                ");",
            ),
            "Canvas" => self.canvas(out, element),
            "Rail" => self.children(
                out,
                &format!(
                    "b::Rail(p, c, b::RailProps {{ vertical: {}, anchor: {}, overhang: {} }}, ",
                    element.keyword("axis") == Some("vertical"),
                    Self::keyword(element, "anchor"),
                    some(self.num(element, "overhang")),
                ),
                &element.children,
                ");",
            ),
            "PopoverButton" => {
                self.children(
                    out,
                    &format!(
                        "b::PopoverButton(p, c, b::PopoverButtonProps {{ id: {}, label: {}, icon: {}, presented: {} }}, ",
                        self.id(element),
                        some(self.text_prop(element, "label")),
                        Self::icon(element, "icon"),
                        some(self.field(element, "presented")),
                    ),
                    &element.children,
                    ");",
                );
                self.shortcut(
                    out,
                    element,
                    &format!("b::activate_target(c, {})", self.id(element)),
                );
            }
            "SplitButton" => {
                let field = self
                    .field(element, "value")
                    .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into());
                self.children(
                    out,
                    &format!(
                        "b::SplitButton(p, c, b::SplitButtonProps {{ id: {}, label: {}, icon: {}, field: {field}, more: {}, shape: {} }}, ",
                        self.id(element),
                        some(self.text_prop(element, "label")),
                        Self::icon(element, "icon"),
                        some(self.text_prop(element, "more_label")),
                        self.shape(element),
                    ),
                    &element.children,
                    ");",
                );
                self.shortcut(out, element, &format!("b::session_toggle(c, {field})"));
            }
            "Toast" => {
                let content = self
                    .text_prop(element, "content")
                    .unwrap_or_else(|| "b::str(|_| String::new())".into());
                let key = match (element.expr("key"), element.text("content")) {
                    (Some(key), _) => self.getter(key),
                    (None, Some(text)) => format!("b::get(|c| V::Text({}))", self.text(text)),
                    (None, None) => "b::get(|_| V::Unit)".into(),
                };
                out.line(format!(
                    "b::Toast(p, c, b::ToastProps {{ content: {content}, icon: {}, key: {key}, duration: {} }});",
                    Self::icon(element, "icon"),
                    Self::number(element, "duration"),
                ));
            }
            "Drop" => self.children(
                out,
                &format!(
                    "b::Drop(p, c, b::DropProps {{ id: {}, label: {}, field: {}, kinds: {}, over: {}, opens: {} }}, {}, ",
                    self.id(element),
                    some(self.text_prop(element, "label")),
                    self.field(element, "value")
                        .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
                    string(element.keyword("files").unwrap_or("any")),
                    some(self.field(element, "over")),
                    element.keyword("opens") == Some("true"),
                    self.action(element),
                ),
                &element.children,
                ");",
            ),
            "Pinch" | "Swipe" => self.children(
                out,
                &format!(
                    "b::{}(p, c, b::GestureProps {{ id: {}, label: {}, field: {}, min: {}, max: {}, vertical: {}, distance: {} }}, {}, ",
                    element.kind,
                    self.id(element),
                    some(self.text_prop(element, "label")),
                    self.field(element, "value")
                        .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
                    float(element.number("min").unwrap_or(0.0)),
                    float(element.number("max").unwrap_or(1.0)),
                    element.keyword("axis") != Some("horizontal"),
                    Self::number(element, "distance"),
                    self.action(element),
                ),
                &element.children,
                ");",
            ),
            // Items before a `Spacer` stand at the leading end of the header.
            "Toolbar" => match element
                .children
                .iter()
                .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"))
            {
                Some(index) => {
                    self.children(
                        out,
                        "b::LeadingToolbar(p, c, ",
                        &element.children[..index],
                        ");",
                    );
                    self.children(
                        out,
                        "b::Toolbar(p, c, ",
                        &element.children[index + 1..],
                        ");",
                    );
                }
                None => self.children(out, "b::Toolbar(p, c, ", &element.children, ");"),
            },
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| self.getter(route))
                    .unwrap_or_else(|| "b::get(|_| V::Unit)".into());
                let label = self
                    .row_label(&element.children)
                    .unwrap_or_else(|| "String::new()".into());
                self.children(
                    out,
                    &format!(
                        "b::NavLink(p, c, {}, {route}, b::str(|c| {label}), ",
                        self.id(element)
                    ),
                    &element.children,
                    ");",
                );
            }
            "List" => self.children(out, "b::List(p, c, ", &element.children, ");"),
            "ForEach" => self.for_each(out, element),
            "Tabs" => self.tabs(out, element),
            "Tab" => self.tab(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "Detail" | "Destination" | "Sidebar" | "SidebarHeader" | "SidebarFooter"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" | "Inspector" | "Shape" | "Series"
            | "Pattern" | "TabAccessory" | "PickerOption" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }

    fn search_props(&self, element: &Element) -> String {
        format!(
            "b::SearchProps {{ id: {}, field: {}, prompt: {}, panel_prompt: {}, width: {}, height: {}, font_size: {} }}",
            self.id(element),
            some(self.field(element, "value")),
            some(self.text_prop(element, "prompt")),
            some(self.text_prop(element, "panel_prompt")),
            Self::number(element, "width"),
            Self::number(element, "height"),
            Self::number(element, "font_size"),
        )
    }

    fn picker(&self, out: &mut Out, element: &Element) {
        let ty = element
            .expr("selected")
            .map(|expr| &expr.ty)
            .or_else(|| element.field("value").map(|field| &field.ty));
        let Some(Type::Enum { name }) = ty else {
            return;
        };
        let def = self.types().enum_def(name).expect("enum");
        // `PickerOption`s list their own values and labels; without them
        // every variant is an option.
        let explicit: Vec<_> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(option) if option.kind == "PickerOption" => Some(option),
                _ => None,
            })
            .filter_map(|option| {
                let value = option.expr("value")?;
                Some(format!(
                    "b::option({}, {}, {})",
                    self.expr(value),
                    self.variant_name(value),
                    self.text_prop(option, "label")?
                ))
            })
            .collect();
        let options: Vec<_> = if explicit.is_empty() {
            def.variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    format!(
                        "b::option(V::unit_variant({index}), {}, {})",
                        string(&variant.name),
                        self.variant_label(def, index)
                    )
                })
                .collect()
        } else {
            explicit
        };
        out.line(format!(
            "b::Picker(p, c, b::PickerProps {{ id: {}, label: {}, model: {}, options: vec![{}], style: {}, label_hidden: {}, shape: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "selected", true)),
            options.join(", "),
            Self::keyword(element, "style"),
            element.keyword("label_hidden") == Some("true"),
            self.shape(element),
        ));
    }

    fn for_each(&self, out: &mut Out, element: &Element) {
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { field, .. })) => self.children(
                out,
                &format!(
                    "b::Each(p, c, {}, {field}, {}, {}, ",
                    self.getter(items),
                    string(&template.var),
                    Self::number(element, "stagger"),
                ),
                &element.children,
                ");",
            ),
            _ => self.nodes(out, &element.children),
        }
    }

    fn video(&self, out: &mut Out, element: &Element) {
        let (source, player) = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => (Some(self.getter(source)), None),
            (None, Some(Prop::Player { id })) => (None, Some(string(id))),
            _ => (None, None),
        };
        out.line(format!(
            "b::Video(p, c, b::VideoProps {{ id: {}, source: {}, player: {}, label: {}, controls: {}, fit: {}, corner: {}, animate: {} }});",
            self.id(element),
            some(source),
            some(player),
            some(self.text_prop(element, "label")),
            element.keyword("controls") != Some("none"),
            Self::keyword(element, "fit"),
            some(self.num(element, "corner")),
            self.animate(element),
        ));
    }

    fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let params: Vec<_> = binding.args.iter().map(|arg| self.getter(arg)).collect();
        self.children(
            out,
            &format!(
                "b::Query(p, c, b::QueryProps {{ id: {}, query: {}, params: vec![{}], loading: {}, failed: {}, empty: {}, icon: {}, var: {} }}, ",
                some(element.id.as_ref().map(|_| self.id(element))),
                binding.query,
                params.join(", "),
                some(self.text_prop(element, "loading")),
                some(self.text_prop(element, "failed")),
                some(self.text_prop(element, "empty")),
                Self::icon(element, "icon"),
                string(&template.var),
            ),
            &element.children,
            ");",
        );
    }

    /// A `Canvas`: its drawing elements in order, read in one getter.
    fn canvas(&self, out: &mut Out, element: &Element) {
        let role = |child: &Element, name: &str| {
            some(
                child
                    .keyword(name)
                    .map(|role| format!("b::Color::Role({})", string(role))),
            )
        };
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else { continue };
            match child.kind.as_str() {
                "Shape" => {
                    let fill_from = match (child.expr("fill_from"), child.resource("fill_from")) {
                        (Some(expr), _) => {
                            Some(format!("b::Source::Url(b::url(&{}))", self.expr(expr)))
                        }
                        (None, Some(name)) => {
                            Some(format!("b::Source::Resource({})", string(name)))
                        }
                        (None, None) => None,
                    };
                    ops.push(format!(
                        "b::Op::Shape(b::ShapeOp {{ kind: {}, x: {}, y: {}, width: {}, height: {}, corner: {}, fill: {}, fill_from: {}, fade: {}, stroke: {}, stroke_width: {}, opacity: {} }})",
                        string(child.keyword("kind").unwrap_or("rect")),
                        Self::number(child, "x"),
                        Self::number(child, "y"),
                        Self::number(child, "width"),
                        Self::number(child, "height"),
                        Self::number(child, "corner"),
                        role(child, "fill"),
                        some(fill_from),
                        Self::keyword(child, "fade"),
                        role(child, "stroke"),
                        Self::number(child, "stroke_width"),
                        Self::number(child, "opacity"),
                    ));
                }
                "Series" => {
                    let values = child
                        .expr("values")
                        .map(|list| format!("b::numbers({})", self.expr(list)))
                        .unwrap_or_else(|| "Vec::new()".into());
                    let max = child
                        .expr("max")
                        .map(|max| format!("Some(rt::as_f64(&{}))", self.expr(max)))
                        .unwrap_or_else(|| Self::number(child, "max"));
                    ops.push(format!(
                        "b::Op::Series(b::SeriesOp {{ values: {values}, max: {max}, area: {}, fill: {}, stroke: {}, stroke_width: {}, opacity: {} }})",
                        child.keyword("kind") == Some("area"),
                        role(child, "fill"),
                        role(child, "stroke"),
                        Self::number(child, "stroke_width"),
                        Self::number(child, "opacity"),
                    ));
                }
                "Pattern" => {
                    if let Some(name) = child.keyword("name") {
                        ops.push(format!(
                            "b::Op::Pattern(&{}, {})",
                            constant("P", name),
                            Self::number(child, "opacity")
                        ));
                    }
                }
                _ => {}
            }
        }
        out.line(format!(
            "b::Canvas(p, c, b::CanvasProps {{ width: {}, height: {}, label: {}, animate: {} }}, Rc::new(|c: &b::Ctx| vec![{}]));",
            some(self.num(element, "width")),
            some(self.num(element, "height")),
            some(self.text_prop(element, "label")),
            self.animate(element),
            ops.join(", ")
        ));
    }

    fn split_view(&self, out: &mut Out, element: &Element) {
        let child = |kind: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(child) if child.kind == kind => Some(child),
                _ => None,
            })
        };
        let (Some(sidebar), Some(detail)) = (child("Sidebar"), child("Detail")) else {
            return;
        };
        let Some(selection) = self.field(sidebar, "selection") else {
            return;
        };
        let mut search = None;
        let mut header = None;
        let mut footer = None;
        let mut content = Vec::new();
        for node in &sidebar.children {
            match node {
                Node::Element(child) if child.kind == "Search" => search = Some(child),
                Node::Element(child) if child.kind == "SidebarHeader" => header = Some(child),
                Node::Element(child) if child.kind == "SidebarFooter" => footer = Some(child),
                other => content.push(other.clone()),
            }
        }
        out.open("b::SplitView(p, c, b::SplitSpec {");
        out.line(format!(
            "id: {},",
            some(sidebar.id.as_ref().map(|_| self.id(sidebar)))
        ));
        out.line(format!("selection: {selection},"));
        out.line(format!(
            "fallback: {},",
            some(sidebar.expr("fallback").map(|value| self.getter(value)))
        ));
        match search {
            Some(search) => out.line(format!(
                "search: Some(({}, {})),",
                self.search_props(search),
                self.action(search)
            )),
            None => out.line("search: None,"),
        }
        match header {
            Some(header) => self.children(out, "header: Some(", &header.children, "),"),
            None => out.line("header: None,"),
        }
        match footer {
            Some(footer) => self.children(out, "footer: Some(", &footer.children, "),"),
            None => out.line("footer: None,"),
        }
        out.line(format!("width: {},", Self::number(sidebar, "width")));
        out.line(format!("titled: {},", self.value(detail, "titled")));
        out.close("},");
        out.indent += 1;
        self.in_sidebar.set(true);
        self.children(out, "", &content, ",");
        self.in_sidebar.set(false);
        self.children(out, "", &detail.children, "");
        out.indent -= 1;
        out.line(");");
    }

    fn navigation(&self, out: &mut Out, element: &Element) {
        let Some(path) = self.field(element, "path") else {
            return;
        };
        let destination = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "Destination" => Some(child),
            _ => None,
        });
        let content: Vec<Node> = element
            .children
            .iter()
            .filter(|node| !matches!(node, Node::Element(child) if child.kind == "Destination"))
            .cloned()
            .collect();
        let var = destination
            .and_then(|destination| destination.template.as_ref())
            .map(|template| template.var.clone())
            .unwrap_or_else(|| "route".into());
        out.open(format!(
            "b::Navigation(p, c, {}, {path},",
            some(element.id.as_ref().map(|_| self.id(element)))
        ));
        self.children(out, "", &content, ",");
        out.line(format!("{},", string(&var)));
        let destination_nodes = destination
            .map(|destination| destination.children.clone())
            .unwrap_or_default();
        self.children(out, "", &destination_nodes, ",");
        out.close(");");
    }

    /// One builder per layout size; the runtime picks one by the width
    /// available to it and the breakpoints from `[layouts]`.
    fn adaptive(&self, out: &mut Out, element: &Element) {
        let layout = |size: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size) => Some(layout),
                _ => None,
            })
        };
        out.open(format!(
            "b::Adaptive(p, c, {}, [",
            some(element.id.as_ref().map(|_| self.id(element)))
        ));
        for size in ["compact", "medium", "expanded", "large"] {
            match layout(size) {
                Some(layout) => self.children(out, "Some(", &layout.children, "),"),
                None => out.line("None,"),
            }
        }
        out.close("]);");
    }

    /// A menu button and its items; dividers are `None`.
    fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "b::Menu(p, c, b::MenuProps {{ id: {}, label: {}, icon: {}, icon_only: {}, shape: {} }}, vec![",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            element.keyword("style") == Some("icon"),
            self.shape(element),
        ));
        let mut shortcuts = Vec::new();
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("None,");
                continue;
            }
            if item.keyword("shortcut").is_some() {
                shortcuts.push(item);
            }
            out.line(format!(
                "Some(b::MenuItem {{ id: {}, label: {}, icon: {}, role: {}, enabled: {}, action: {}, pick: {} }}),",
                self.id(item),
                some(self.text_prop(item, "label")),
                Self::icon(item, "icon"),
                Self::keyword(item, "role"),
                self.enabled(item),
                self.action(item),
                self.pick(item),
            ));
        }
        out.close("]);");
        for item in shortcuts {
            self.shortcut(
                out,
                item,
                &format!("b::activate_target(c, {})", self.id(item)),
            );
        }
    }

    fn tabs(&self, out: &mut Out, element: &Element) {
        let Some(selection) = self.field(element, "selection") else {
            return;
        };
        let accessory = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "TabAccessory" => Some(child),
            _ => None,
        });
        self.children(
            out,
            &format!(
                "b::Tabs(p, c, {}, {selection}, ",
                some(element.id.as_ref().map(|_| self.id(element)))
            ),
            &element.children,
            ",",
        );
        match accessory {
            Some(accessory) => self.children(
                out,
                &format!(
                    "Some(b::TabAccessory {{ shown: {}, build: ",
                    some(accessory.expr("shown").map(|shown| self.getter(shown)))
                ),
                &accessory.children,
                " }));",
            ),
            None => out.line("None);"),
        }
    }

    fn tab(&self, out: &mut Out, tab: &Element) {
        let Some(value) = tab.expr("value") else {
            return;
        };
        self.children(
            out,
            &format!(
                "b::Tab(p, c, b::TabProps {{ value: {}, enum_name: {}, label: {}, icon: {} }}, ",
                self.getter(value),
                Self::enum_name(value),
                self.text_prop(tab, "label")
                    .unwrap_or_else(|| "b::str(|_| String::new())".into()),
                Self::icon(tab, "icon"),
            ),
            &tab.children,
            ");",
        );
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!(
            "fn c_{}(p: &b::Parent, c: &b::Ctx) {{",
            component.name
        ));
        self.nodes(out, std::slice::from_ref(&component.body));
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!("fn w_{}(p: &b::Parent, c: &b::Ctx) {{", window.id));
        self.nodes(out, std::slice::from_ref(&window.content));
        out.close("}");
        out.line("");
        if let Some(close) = &window.close {
            let save = match &close.save {
                Some(Action::Engine { command, args }) => {
                    let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                    format!(
                        "Some(Rc::new(|c: &b::Ctx, done: b::Done| b::dispatch(c, {command}, vec![{}], Some(done))))",
                        values.join(", ")
                    )
                }
                _ => "None".into(),
            };
            out.line(format!(
                "/// The close check of the `{}` window, read against its own drafts.",
                window.id
            ));
            out.open(format!(
                "fn close_{}(c: &b::Ctx) -> b::CloseCheck {{",
                window.id
            ));
            out.open("b::CloseCheck {");
            out.line(format!(
                "needs_confirmation: {},",
                self.condition(&close.when)
            ));
            out.line(format!("message: {},", self.text(&close.message)));
            out.line(format!("save: {save},"));
            out.close("}");
            out.close("}");
            out.line("");
        }
    }

    /// The motion tokens, transitions, patterns and style overrides of
    /// `style!`.
    fn style_tables(&self, out: &mut Out) {
        let sheet = &self.app.style;
        out.line("// Motion tokens: period and damping ratio of each spring.");
        for token in &sheet.motion {
            out.line(format!(
                "const {}: b::Curve = ({}, {});",
                constant("M", &token.name),
                float(token.period),
                float(token.damping)
            ));
        }
        // Tokens the runtime uses even when the application changes none.
        for (name, period, damping) in bern_ir::style::BUILT_IN_MOTION {
            if sheet.token(name).is_none() {
                out.line(format!(
                    "const {}: b::Curve = ({}, {});",
                    constant("M", name),
                    float(period),
                    float(damping)
                ));
            }
        }
        out.line("");
        out.line("// Transitions of `appear` and `disappear`.");
        for transition in &sheet.transitions {
            let kind = match &transition.kind {
                TransitionKind::Fade => "b::TransitionKind::Fade".to_string(),
                TransitionKind::Grow { edge } => {
                    format!("b::TransitionKind::Grow(b::Edge::{edge:?})")
                }
                TransitionKind::Pop { from } => format!("b::TransitionKind::Pop({})", float(*from)),
                TransitionKind::Slide { edge, distance } => format!(
                    "b::TransitionKind::Slide(b::Edge::{edge:?}, {})",
                    float(*distance)
                ),
            };
            out.line(format!(
                "static {}: b::Transition = b::Transition {{ kind: {kind}, motion: {} }};",
                constant("X", &transition.name),
                self.motion(&transition.motion)
            ));
        }
        out.line("");
        out.line("// Patterns of `style!`.");
        for pattern in &sheet.patterns {
            let bands: Vec<_> = pattern
                .bands
                .iter()
                .map(|band| {
                    format!(
                        "b::Band {{ color: {}, width: {}, opacity: {} }}",
                        some(band.color.as_ref().map(color)),
                        float(band.width),
                        float(band.opacity)
                    )
                })
                .collect();
            out.line(format!(
                "static {}: b::PatternDef = b::PatternDef {{ bands: &[{}], twill: {} }};",
                constant("P", &pattern.name),
                bands.join(", "),
                pattern.twill
            ));
        }
        out.line("");
        out.line("/// Style overrides of `style!`, as this target shows them.");
        out.open("static STYLES: &[b::StyleDef] = &[");
        for resolved in style::used(self.app) {
            out.line(format!("{},", style::declaration(self.app, &resolved)));
        }
        out.close("];");
        out.line("");
    }

    /// The accent and appearance of `WindowSet`, chosen at run time.
    fn theme(&self, out: &mut Out) {
        let windows = &self.app.windows;
        if windows.accent.is_none() && windows.appearance.is_none() {
            out.line("theme: None,");
            return;
        }
        let variants = |expr: &Expr| match &expr.ty {
            Type::Enum { name } => self
                .types()
                .enum_def(name)
                .map(|def| {
                    def.variants
                        .iter()
                        .map(|variant| variant.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let accents: Vec<String> = windows
            .accent
            .as_ref()
            .map(|expr| {
                variants(expr)
                    .iter()
                    .enumerate()
                    .filter_map(|(index, name)| {
                        let accent = self.app.style.accent(name)?;
                        let ColorRef::Fixed { light, dark } = &accent.color else {
                            return None;
                        };
                        Some(format!(
                            "({index}, ({}, {}, {}), ({}, {}, {}))",
                            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let appearances: Vec<String> = windows
            .appearance
            .as_ref()
            .map(|expr| {
                variants(expr)
                    .iter()
                    .enumerate()
                    .map(|(index, name)| format!("({index}, {})", string(&name.to_lowercase())))
                    .collect()
            })
            .unwrap_or_default();
        out.open("theme: Some(b::ThemeDef {");
        out.line(format!(
            "accent: {},",
            some(
                windows
                    .accent
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.expr(expr)))
            )
        ));
        out.line(format!("accents: &[{}],", accents.join(", ")));
        out.line(format!(
            "appearance: {},",
            some(
                windows
                    .appearance
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.expr(expr)))
            )
        ));
        out.line(format!("appearances: &[{}],", appearances.join(", ")));
        out.close("}),");
    }

    /// The application's `ui.rs`.
    pub fn generate(&self, fingerprint: u64, descriptor: &str) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line(
            "#![allow(unused_imports, unused_variables, unused_parens, dead_code, clippy::all)]",
        );
        out.line("");
        out.line("use bern_gtk as b;");
        out.line("use bern_gtk::protocol::Value as V;");
        out.line("use bern_gtk::value as rt;");
        out.line("use std::rc::Rc;");
        out.line("");
        self.style_tables(&mut out);
        out.line("// Components.");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }

        out.line("/// Strings of every supported language.");
        out.open("static STRINGS: b::text::StringTable = &[");
        for (language, table) in crate::strings::tables(app) {
            out.open(format!("({}, &[", string(&language)));
            for (key, text) in table {
                out.line(format!("({}, {}),", string(&key), string(&text)));
            }
            out.close("]),");
        }
        out.close("];");
        out.line("");

        out.line("/// The application, as the runtime sees it.");
        out.open("pub fn app() -> b::AppDef {");
        out.open("b::AppDef {");
        out.line(format!("id: {},", string(&app.app.id)));
        out.line(format!("bundle_id: {},", string(&app.app.bundle_id)));
        out.line("name: \"app.name\",");
        out.line(format!("version: {},", string(&app.app.version)));
        out.line(format!("fingerprint: 0x{fingerprint:016x},"));
        let languages: Vec<_> = app
            .languages
            .supported
            .iter()
            .map(|language| string(language))
            .collect();
        out.line(format!("languages: &[{}],", languages.join(", ")));
        out.line(format!(
            "default_language: {},",
            string(&app.languages.default)
        ));
        out.line("strings: STRINGS,");
        out.line(format!("descriptor: ({descriptor}).clone(),"));
        out.line(format!(
            "types: {},",
            bern_target_api::glue::types_expr(&app.types)
        ));
        out.open("sessions: vec![");
        for session in &app.sessions {
            let fields: Vec<_> = session
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "({}, {}, {})",
                        string(&field.name),
                        bern_target_api::glue::type_expr(&field.ty),
                        literal(&field.initial)
                    )
                })
                .collect();
            out.line(format!(
                "b::SessionDef {{ name: {}, fields: vec![{}] }},",
                string(&session.name),
                fields.join(", ")
            ));
        }
        out.close("],");
        out.open("windows: vec![");
        for window in &app.windows.windows {
            let optional = |value: Option<f64>| some(value.map(float));
            let sessions: Vec<_> = window
                .sessions
                .iter()
                .map(|name| self.session_index(name).to_string())
                .collect();
            let immersive = some(
                window
                    .immersive
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.condition(expr))),
            );
            let idle_enabled = some(
                window
                    .idle_enabled
                    .as_ref()
                    .map(|expr| format!("|c: &b::Ctx| {}", self.condition(expr))),
            );
            out.line(format!(
                "b::WindowDef {{ id: {}, title: {}, settings: {}, width: {}, height: {}, min_width: {}, min_height: {}, initial_open: {}, sessions: vec![{}], render: w_{}, close: {}, dialog: {}, unified: {}, idle_after: {}, immersive: {immersive}, idle_enabled: {idle_enabled} }},",
                string(&window.id),
                string(&window.title.key),
                window.role == WindowRole::Settings,
                optional(window.width),
                optional(window.height),
                optional(window.min_width),
                optional(window.min_height),
                window.initial == WindowInitial::Open,
                sessions.join(", "),
                window.id,
                if window.close.is_some() {
                    format!("Some(close_{})", window.id)
                } else {
                    "None".into()
                },
                window.role == WindowRole::Dialog,
                window.chrome == WindowChrome::Unified,
                optional(window.idle_after),
            ));
        }
        out.close("],");
        out.line(format!("primary: {},", string(&app.windows.primary)));
        let players: Vec<_> = app
            .players
            .iter()
            .map(|player| string(&player.id))
            .collect();
        out.line(format!("players: vec![{}],", players.join(", ")));
        out.line(format!(
            "compact_below: {},",
            some(app.layouts.map(|layouts| float(layouts.compact_below)))
        ));
        out.line(format!(
            "medium_below: {},",
            some(
                app.layouts
                    .and_then(|layouts| layouts.medium_below)
                    .map(float)
            )
        ));
        out.line(format!(
            "expanded_below: {},",
            some(
                app.layouts
                    .and_then(|layouts| layouts.expanded_below)
                    .map(float)
            )
        ));
        out.line(format!(
            "brand_seed: {},",
            some(
                app.theme
                    .brand_seed
                    .map(|seed| format!("({}, {}, {})", seed.red, seed.green, seed.blue))
            )
        ));
        out.line("styles: STYLES,");
        // The weights and sizes `text { … }` gives text styles.
        let text_styles: Vec<String> = app
            .style
            .text_weights
            .iter()
            .map(|style| {
                format!(
                    "({}, {}, {})",
                    string(&style.style),
                    string(&style.weight),
                    some(style.size.map(float))
                )
            })
            .collect();
        out.line(format!("text_styles: &[{}],", text_styles.join(", ")));
        self.theme(&mut out);
        let kinds: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
            .iter()
            .map(|(kind, extensions)| {
                let extensions: Vec<String> = extensions.iter().map(|ext| string(ext)).collect();
                format!("({}, &[{}])", string(kind), extensions.join(", "))
            })
            .collect();
        out.line(format!("file_kinds: &[{}],", kinds.join(", ")));
        out.close("}");
        out.close("}");
        out.finish()
    }
}
