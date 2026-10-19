//! C++ generation from the IR.
//!
//! Two sources come out. `app_catalog.cpp` holds what does not depend on a
//! XAML namespace: strings, types, commands, sessions and the rest of the
//! catalog, compiled once. `app_ui.inl` builds every component and window
//! from the runtime's standard components (`B::`); the runtime includes it
//! once per front end, inside `bern::winui` (WinUI 3) and `bern::islands`
//! (the system XAML), so the same declarations drive both.
//!
//! Expressions become C++ expressions over schema values evaluated in a
//! context `c`; locals (component parameters, row variables, pattern
//! bindings) live in the context, so every lambda captures nothing, and
//! each binding re-runs only when a cell it read changes.

use bern_ir::style::{ColorRef, ControlSize, StyleEffect, TransitionKind};
use bern_ir::{
    Action, Application, BinaryOp, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind,
    FieldRef, IfLetNode, IfNode, MatchNode, MediaOp, Node, ParamKind, Prop, SessionOp, Text,
    WindowChrome, WindowDef, WindowInitial, WindowOp, WindowRole,
};
use bern_protocol::{
    EnumDef, OperationKind, SessionCommand, Type, TypeTable, Value, session_catalog,
};

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

fn escape(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Three octal digits: an escape never runs into the next
            // character.
            c if (c as u32) < 0x20 || c == '\u{7f}' => out.push_str(&format!("\\{:03o}", c as u32)),
            c => out.push(c),
        }
    }
}

/// A UTF-8 C++ string literal; sources compile with `/utf-8`.
pub fn string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    escape(value, &mut out);
    out.push('"');
    out
}

/// A UTF-16 C++ string literal, for XAML text.
pub fn wide(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 3);
    out.push_str("L\"");
    escape(value, &mut out);
    out.push('"');
    out
}

/// A `std::string` with the text, NUL characters included.
fn std_string(value: &str) -> String {
    if value.contains('\0') {
        format!("std::string({}, {})", string(value), value.len())
    } else {
        string(value)
    }
}

fn float(value: f64) -> String {
    if value.is_nan() {
        return "std::numeric_limits<double>::quiet_NaN()".into();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "std::numeric_limits<double>::infinity()".into()
        } else {
            "-std::numeric_limits<double>::infinity()".into()
        };
    }
    let text = format!("{value:?}");
    if text.contains('.') || text.contains('e') {
        text
    } else {
        format!("{text}.0")
    }
}

fn int(value: i64) -> String {
    if value == i64::MIN {
        "(-9223372036854775807LL - 1)".into()
    } else {
        format!("{value}LL")
    }
}

fn null(value: Option<String>) -> String {
    value.unwrap_or_else(|| "nullptr".into())
}

/// Designated initializers in declaration order, leaving out what is not
/// given.
fn designated(fields: &[(&str, Option<String>)]) -> String {
    let given: Vec<String> = fields
        .iter()
        .filter_map(|(name, value)| value.as_ref().map(|value| format!(".{name} = {value}")))
        .collect();
    format!("{{{}}}", given.join(", "))
}

/// A schema value as a C++ expression.
fn literal(value: &Value) -> String {
    let list = |values: &[Value]| values.iter().map(literal).collect::<Vec<_>>().join(", ");
    match value {
        Value::Unit => "Value::Unit()".into(),
        Value::Bool(value) => format!("Value::Bool({value})"),
        Value::Int(value) => format!("Value::Int({})", int(*value)),
        Value::Float(value) => format!("Value::Float({})", float(*value)),
        Value::Text(value) => format!("Value::Text({})", std_string(value)),
        Value::Enum { variant, fields } if fields.is_empty() => format!("Value::Enum({variant}u)"),
        Value::Enum { variant, fields } => format!("Value::Enum({variant}u, {{{}}})", list(fields)),
        Value::Optional(None) => "Value::None()".into(),
        Value::Optional(Some(value)) => format!("Value::Some({})", literal(value)),
        Value::Record(values) => format!("Value::Record({{{}}})", list(values)),
        Value::List(values) => format!("Value::List({{{}}})", list(values)),
    }
}

/// A schema type as a C++ `BType` expression.
pub fn type_expr(ty: &Type) -> String {
    match ty {
        Type::Unit => "BType::Unit()".into(),
        Type::Bool => "BType::Bool()".into(),
        Type::Int => "BType::Int()".into(),
        Type::Float => "BType::Float()".into(),
        Type::Text => "BType::Text()".into(),
        Type::Enum { name } => format!("BType::Enum({})", string(name)),
        Type::Record { name } => format!("BType::Record({})", string(name)),
        Type::Optional { inner } => format!("BType::Optional({})", type_expr(inner)),
        Type::List { inner } => format!("BType::List({})", type_expr(inner)),
    }
}

fn types_list(types: &[Type]) -> String {
    format!(
        "{{{}}}",
        types.iter().map(type_expr).collect::<Vec<_>>().join(", ")
    )
}

/// Every type the runtime decodes: the application's (protocol and UI
/// types), the media catalog's and the session catalog's, first
/// definition winning.
fn merged_types(app: &Application) -> TypeTable {
    let mut merged = TypeTable::default();
    for table in [
        app.types.clone(),
        app.protocol.types.clone(),
        bern_media::media_types(),
        session_catalog().0,
    ] {
        for def in table.enums {
            if !merged.enums.iter().any(|known| known.name == def.name) {
                merged.enums.push(def);
            }
        }
        for def in table.records {
            if !merged.records.iter().any(|known| known.name == def.name) {
                merged.records.push(def);
            }
        }
    }
    merged
}

/// A children builder's opening.
const BUILDER: &str = "[](Parent const& p, Ctx const& c) {";

/// The platform `style!` rules and exemptions name for this target.
pub const PLATFORM: &str = "windows";

/// A C++ identifier for a name from `style!`: letters, digits and `_`.
fn ident(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// A color of `style!` or a role keyword as a `B::Paint`.
fn paint(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!("B::Paint{{{}}}", string(role)),
        ColorRef::Fixed { light, dark } => format!(
            "B::Paint{{nullptr, 0x{:02X}{:02X}{:02X}u, 0x{:02X}{:02X}{:02X}u}}",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

fn role(element: &Element, name: &str) -> Option<String> {
    element
        .keyword(name)
        .map(|role| format!("B::Paint{{{}}}", string(role)))
}

fn motion_ref(name: &str) -> String {
    format!("&M_{}", ident(name))
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether the nodes being written are the items of a sidebar.
    in_sidebar: std::cell::Cell<bool>,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        Self {
            app,
            in_sidebar: std::cell::Cell::new(false),
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
            format!("R::At({expr}, {{{}}})", indices.join(", "))
        }
    }

    fn binding(&self, binding: &Binding) -> String {
        match binding {
            Binding::Engine { field, path } => Self::path(format!("c.Engine({field})"), path),
            Binding::Session {
                session,
                field,
                path,
            } => Self::path(
                format!("c.Session({}, {field})", self.session_index(session)),
                path,
            ),
            Binding::Local { name, path, .. } => {
                Self::path(format!("c.Local({})", string(name)), path)
            }
            // `window.idle` and the window's size, `window.compact` …
            Binding::Window { field } => format!("c.WindowFlag({})", string(field)),
        }
    }

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value),
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let values = std::iter::once(format!("{variant}u"))
                    .chain(fields.iter().map(|field| self.expr(field)))
                    .collect::<Vec<_>>();
                format!("R::Variant({})", values.join(", "))
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => format!(
                "({} ? ({}) : ({}))",
                self.condition(cond),
                self.expr(then),
                self.expr(otherwise)
            ),
            ExprKind::Not { expr } => format!("R::Not({})", self.expr(expr)),
            ExprKind::Neg { expr } => format!("R::Neg({})", self.expr(expr)),
            ExprKind::Binary { op, lhs, rhs } => {
                let (left, right) = (self.expr(lhs), self.expr(rhs));
                match op {
                    BinaryOp::And => format!("R::And({left}, [&] {{ return {right}; }})"),
                    BinaryOp::Or => format!("R::Or({left}, [&] {{ return {right}; }})"),
                    other => {
                        let name = match other {
                            BinaryOp::Eq => "Eq",
                            BinaryOp::Ne => "Ne",
                            BinaryOp::Lt => "Lt",
                            BinaryOp::Le => "Le",
                            BinaryOp::Gt => "Gt",
                            _ => "Ge",
                        };
                        format!("R::{name}({left}, {right})")
                    }
                }
            }
        }
    }

    fn condition(&self, expr: &Expr) -> String {
        format!("R::Truthy({})", self.expr(expr))
    }

    /// Display text of a value, by type.
    fn display(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Enum { name } => format!("bern::Text::Display({value}, {})", string(name)),
            _ => format!("R::Plain({value})"),
        }
    }

    /// A `bern::Text::Arg` for a `tr!` argument: dates stay dates for their
    /// patterns, numbers keep their value for plural branches, enums show
    /// their labels.
    fn argument(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Enum { name } => format!(
                "bern::Text::Arg(bern::Text::Display({value}, {}))",
                string(name)
            ),
            Type::Record { name } if name == bern_protocol::format::CALENDAR_DATE => {
                format!("bern::Text::DateArg({value})")
            }
            _ => format!("bern::Text::ArgOf({value})"),
        }
    }

    fn text(&self, text: &Text) -> String {
        match text {
            Text::Localized { text, args } => {
                if text.params.is_empty() {
                    format!("bern::Text::Tr({})", string(&text.key))
                } else {
                    let values: Vec<_> = text
                        .params
                        .iter()
                        .map(|param| {
                            let value = args
                                .iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| self.argument(expr))
                                .unwrap_or_else(|| "bern::Text::Arg()".into());
                            format!("{{{}, {value}}}", string(param))
                        })
                        .collect();
                    format!(
                        "bern::Text::Fmt({}, {{{}}})",
                        string(&text.key),
                        values.join(", ")
                    )
                }
            }
            Text::Dynamic { expr } => self.display(expr),
        }
    }

    fn text_getter(&self, text: &Text) -> String {
        format!("[](Ctx const& c) {{ return {}; }}", self.text(text))
    }

    fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text_getter(text))
    }

    fn getter(&self, expr: &Expr) -> String {
        format!("[](Ctx const& c) {{ return {}; }}", self.expr(expr))
    }

    fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    fn optional_id(&self, element: &Element) -> String {
        null(element.id.as_ref().map(|_| self.id(element)))
    }

    fn field_ref(&self, field: &FieldRef) -> String {
        format!(
            "Field{{{}, {}}}",
            self.session_index(&field.session),
            field.field
        )
    }

    fn field(&self, element: &Element, name: &str) -> Option<String> {
        element.field(name).map(|field| self.field_ref(field))
    }

    fn values(&self, args: &[Expr]) -> String {
        let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
        format!("Values{{{}}}", values.join(", "))
    }

    fn action_body(&self, action: &Action) -> String {
        match action {
            Action::Session { field, op } => {
                let target = self.field_ref(field);
                match op {
                    SessionOp::Set { value } => {
                        format!("c.Write({target}, {})", self.expr(value))
                    }
                    SessionOp::Toggle => {
                        format!("c.Write({target}, R::Not(c.Peek({target})))")
                    }
                }
            }
            Action::Engine { command, args } => {
                format!("Dispatch(c, {command}, {})", self.values(args))
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => {
                    format!("WindowOps::Open(c, {})", string(window))
                }
                WindowOp::Close => format!("WindowOps::Close(c, {})", string(window)),
            },
            Action::Media { player, op, args } => {
                let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                let player = string(player);
                match op {
                    MediaOp::Open => format!(
                        "MediaOps::Open(c, {player}, {})",
                        values
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "Value::Unit()".into())
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
                            "MediaOps::Send(c, MediaOp::{name}, {player}, Values{{{}}})",
                            values.join(", ")
                        )
                    }
                }
            }
        }
    }

    fn action(&self, element: &Element) -> Option<String> {
        element
            .action
            .as_ref()
            .map(|action| format!("[](Ctx const& c) {{ {}; }}", self.action_body(action)))
    }

    fn keyword(element: &Element, name: &str) -> Option<String> {
        element.keyword(name).map(string)
    }

    fn icon(element: &Element, name: &str) -> Option<String> {
        element.icon(name).map(string)
    }

    fn number(element: &Element, name: &str) -> Option<String> {
        element.number(name).map(float)
    }

    fn enabled(&self, element: &Element) -> Option<String> {
        element.expr("enabled").map(|expr| self.getter(expr))
    }

    /// A length: a number, or an expression the length follows.
    fn len(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(format!("B::Len({})", float(*value))),
            Prop::Expr { expr } => Some(format!("B::Len(Get({}))", self.getter(expr))),
            _ => None,
        }
    }

    /// `shape: capsule` and `height` of a system control.
    fn shape(&self, element: &Element) -> Option<String> {
        let capsule = element.keyword("shape") == Some("capsule");
        let height = self.len(element, "height");
        (capsule || height.is_some()).then(|| {
            designated(&[
                ("capsule", capsule.then(|| "true".to_string())),
                ("height", height),
            ])
            .replacen('{', "B::ControlShape{", 1)
        })
    }

    /// What a control shows and how it changes, as designated fields: the
    /// value `controlled` reads (`checked`, `selected`, `current`), which a
    /// change sets by running the action's command with the new value `v`
    /// in place of that value; or its draft field, after which its action
    /// runs.
    fn model(&self, element: &Element, controlled: &str) -> Vec<(&'static str, Option<String>)> {
        if let Some(expr) = element.expr(controlled) {
            let change = match element.action.as_ref() {
                Some(Action::Engine { command, args }) => {
                    let values: Vec<_> = args
                        .iter()
                        .map(|arg| {
                            if arg == expr {
                                match arg.ty {
                                    Type::Int => {
                                        "Value::Int(std::llround(R::AsDouble(v)))".to_string()
                                    }
                                    Type::Float => "Value::Float(R::AsDouble(v))".to_string(),
                                    _ => "v".to_string(),
                                }
                            } else {
                                self.expr(arg)
                            }
                        })
                        .collect();
                    format!(
                        "[](Ctx const& c, Value const& v) {{ Dispatch(c, {command}, Values{{{}}}); }}",
                        values.join(", ")
                    )
                }
                Some(other) => format!(
                    "[](Ctx const& c, Value const&) {{ {}; }}",
                    self.action_body(other)
                ),
                None => "nullptr".into(),
            };
            return vec![
                ("current", Some(self.getter(expr))),
                ("change", Some(change)),
            ];
        }
        vec![
            ("field", self.field(element, "value")),
            ("then", self.action(element)),
        ]
    }

    /// `animate`'s spring, for the bound lengths of an element.
    fn animate(element: &Element) -> Option<String> {
        element.keyword("animate").map(motion_ref)
    }

    fn reflow(element: &Element) -> Option<String> {
        element.keyword("reflow").map(motion_ref)
    }

    /// The style an element of `kind` (and `variant`) uses, `&S_name`.
    fn style(&self, kind: &str, variant: Option<&str>) -> Option<String> {
        bern_target_api::style::resolve(self.app, PLATFORM, kind, variant)
            .map(|style| format!("&S_{}", style.name))
    }

    /// The kinds of file `pick:` asks for, or a drop area takes.
    fn files(element: &Element) -> Option<String> {
        element
            .keyword("files")
            .map(string)
            .or_else(|| element.field("pick").map(|_| string("any")))
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        // An inspector stands beside the other content of its container.
        let inspector = nodes.iter().find_map(|node| match node {
            Node::Element(element) if element.kind == "Inspector" => Some(element),
            _ => None,
        });
        if let Some(inspector) = inspector {
            let others: Vec<Node> = nodes
                .iter()
                .filter(
                    |node| !matches!(node, Node::Element(element) if element.kind == "Inspector"),
                )
                .cloned()
                .collect();
            out.open(format!(
                "B::Inspector(p, c, {},",
                designated(&[
                    ("presented", self.field(inspector, "presented")),
                    ("width", Self::number(inspector, "width")),
                ])
            ));
            self.children(out, "", &others, ",");
            self.children(out, "", &inspector.children, ");");
            out.indent -= 1;
            return;
        }
        for node in nodes {
            self.node(out, node);
        }
    }

    /// A children builder, `[](Parent const& p, Ctx const& c) { … }`,
    /// between `prefix` and `suffix`.
    fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        out.open(format!("{prefix}{BUILDER}"));
        self.nodes(out, nodes);
        out.close(format!("}}{suffix}"));
    }

    /// A call whose last argument is a children builder.
    fn call(&self, out: &mut Out, head: &str, nodes: &[Node]) {
        self.children(out, head, nodes, ");");
    }

    /// A container's call: with `stagger`, its children take turns
    /// appearing.
    fn container(&self, out: &mut Out, element: &Element, head: &str) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!("{head}[](Parent const& p, Ctx const& outer) {{"));
                out.line(format!(
                    "auto const c = B::Staggered(outer, {});",
                    float(step)
                ));
                self.nodes(out, &element.children);
                out.close("});");
            }
            None => self.call(out, head, &element.children),
        }
    }

    /// Where an element sits on its `Rail`, for `B::Place`.
    fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| self.len(element, name);
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        (at.is_some() || from.is_some()).then(|| {
            designated(&[
                ("at", at),
                ("from", from),
                ("to", to),
                ("animate", Self::animate(element)),
            ])
        })
    }

    /// The motion and appearance props of an element, for `B::Decorate`.
    fn decoration(&self, element: &Element) -> Option<String> {
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("&X_{}", ident(transition)))
        };
        let hero = element.keyword("hero").map(string);
        let hero_key = hero
            .as_ref()
            .and(element.expr("hero_key"))
            .map(|key| self.getter(key));
        let fields = [
            ("appear", transition("appear")),
            ("disappear", transition("disappear")),
            ("hero", hero.clone()),
            ("heroKey", hero_key),
            ("heroMotion", hero.as_ref().map(|_| motion_ref("hero"))),
            (
                "reveal",
                (element.keyword("reveal") == Some("hover")).then(|| "true".to_string()),
            ),
            ("appearance", Self::keyword(element, "appearance")),
            ("nudge", element.expr("nudge").map(|when| self.getter(when))),
            (
                "nudgeMotion",
                (element.expr("nudge").is_some() || element.expr("nudge_key").is_some())
                    .then(|| motion_ref("jelly")),
            ),
            (
                "nudgeKey",
                element.expr("nudge_key").map(|key| self.getter(key)),
            ),
            ("nudgeScale", Self::number(element, "nudge_scale")),
            ("appearDelay", Self::number(element, "appear_delay")),
            (
                "reduceMotion",
                element
                    .expr("reduce_motion")
                    .map(|reduce| self.getter(reduce)),
            ),
            ("scrollScale", Self::number(element, "scroll_scale")),
            ("scrollOpacity", Self::number(element, "scroll_opacity")),
            ("scrollOriginX", Self::number(element, "scroll_origin_x")),
            ("scrollOriginY", Self::number(element, "scroll_origin_y")),
        ];
        fields
            .iter()
            .any(|(_, value)| value.is_some())
            .then(|| designated(&fields))
    }

    fn node(&self, out: &mut Out, node: &Node) {
        match node {
            Node::Element(element) => {
                let placement = self.placement(element);
                if let Some(spec) = &placement {
                    out.open(format!("B::Place(p, c, {spec}, {BUILDER}"));
                }
                // A modal's motion is its card's, above the window.
                let decoration = if element.kind == "Modal" {
                    None
                } else {
                    self.decoration(element)
                };
                match decoration {
                    Some(spec) => {
                        out.open(format!("B::Decorate(p, c, {spec}, {BUILDER}"));
                        self.element(out, element);
                        out.close("});");
                    }
                    None => self.element(out, element),
                }
                if placement.is_some() {
                    out.close("});");
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
                                    "{{{}, {}}}",
                                    string(&param.name),
                                    self.getter(expr)
                                )),
                                _ => None,
                            }),
                        _ => None,
                    })
                    .collect();
                let child = if args.is_empty() {
                    format!("c.Child({})", string(&component.name))
                } else {
                    format!(
                        "c.Child({}, {{{}}})",
                        string(&component.name),
                        args.join(", ")
                    )
                };
                out.line(format!("c_{}(p, {child});", component.name));
            }
            Node::If(node) => self.if_node(out, node),
            Node::IfLet(node) => self.if_let(out, node),
            Node::Match(node) => self.match_node(out, node),
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode) {
        let mut branch = String::from("[](Ctx const& c) -> std::optional<int> { ");
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            branch.push_str(&format!(
                "if ({}) return {index}; ",
                self.condition(condition)
            ));
        }
        if node.otherwise.is_empty() {
            branch.push_str("return std::nullopt; }");
        } else {
            branch.push_str(&format!("return {}; }}", node.branches.len()));
        }
        out.open(format!("B::When(p, c, {branch}, std::vector<Build>{{"));
        for (_, body) in &node.branches {
            self.children(out, "", body, ",");
        }
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, ",");
        }
        out.close("});");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode) {
        out.open(format!(
            "B::IfLet(p, c, {}, {},",
            string(&node.var),
            self.getter(&node.value)
        ));
        if node.otherwise.is_empty() {
            self.children(out, "", &node.then, ");");
        } else {
            self.children(out, "", &node.then, ",");
            self.children(out, "", &node.otherwise, ");");
        }
        out.indent -= 1;
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode) {
        out.open(format!(
            "B::Match(p, c, {}, std::vector<B::MatchArm>{{",
            self.getter(&node.value)
        ));
        for arm in &node.arms {
            let bindings: Vec<String> = arm
                .bindings
                .iter()
                .map(|binding| format!("{{{}, {}}}", string(&binding.name), binding.field))
                .collect();
            let suffix = if bindings.is_empty() {
                "),".to_string()
            } else {
                format!(", {{{}}}),", bindings.join(", "))
            };
            self.children(
                out,
                &format!(
                    "B::Arm({}, ",
                    arm.variant
                        .map(|variant| format!("{variant}u"))
                        .unwrap_or_else(|| "std::nullopt".into())
                ),
                &arm.body,
                &suffix,
            );
        }
        out.close("});");
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
                            Some(variant) => format!("if (tag == {variant}u) return {label}; "),
                            None => format!("return {label}; "),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "[&]() -> winrt::hstring {{ auto tag = R::Tag({}); {}return winrt::hstring(); }}()",
                    self.expr(&node.value),
                    arms.concat()
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// Name of an enum variant value, for selections: the literal's, or
    /// looked up when the value is computed.
    fn variant_name(&self, expr: &Expr) -> String {
        match (&expr.kind, &expr.ty) {
            (ExprKind::Variant { variant, .. }, Type::Enum { name }) => self
                .types()
                .enum_def(name)
                .map(|def| string(&def.variants[*variant as usize].name))
                .unwrap_or_else(|| "\"\"".into()),
            (_, Type::Enum { name }) => format!(
                "[](Ctx const& c) {{ return R::VariantName({}, {}); }}",
                self.expr(expr),
                string(name)
            ),
            _ => "\"\"".into(),
        }
    }

    /// The label getter of a variant: its declared label, or its name.
    fn variant_label(&self, def: &EnumDef, index: usize) -> String {
        let variant = &def.variants[index];
        if variant.label.is_some() {
            format!(
                "[](Ctx const&) {{ return bern::Text::Label({}, {index}); }}",
                string(&def.name)
            )
        } else {
            format!(
                "[](Ctx const&) {{ return winrt::hstring({}); }}",
                wide(&variant.name)
            )
        }
    }

    /// `B::Shortcut(…)` for an element with `shortcut:`, running the
    /// control's `activate` (or `focus` for search fields) while mounted.
    fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(keys) = element.keyword("shortcut") {
            out.line(format!(
                "B::Shortcut(c, {}, {}, {});",
                string(keys),
                self.id(element),
                string(action)
            ));
        }
    }

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => out.line(format!(
                "B::Text(p, c, {});",
                designated(&[
                    ("content", self.text_prop(element, "content")),
                    ("style", Self::keyword(element, "style")),
                    ("emphasis", Self::keyword(element, "emphasis")),
                    ("mark", Self::keyword(element, "mark")),
                    ("lines", Self::number(element, "lines")),
                    ("fontSize", self.len(element, "font_size")),
                    ("lineHeight", self.len(element, "line_height")),
                ])
            )),
            "Label" => out.line(format!(
                "B::Label(p, c, {});",
                designated(&[
                    ("title", self.text_prop(element, "title")),
                    ("icon", Self::icon(element, "icon")),
                    ("style", Self::keyword(element, "style")),
                    ("fontSize", Self::number(element, "font_size")),
                    (
                        "iconOnly",
                        (element.keyword("display") == Some("icon")).then(|| "true".to_string()),
                    ),
                ])
            )),
            "Button" | "IconButton" => {
                let kind = element.kind.as_str();
                let props = designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    (
                        "icon",
                        if kind == "IconButton" {
                            Some(string(element.icon("icon").unwrap_or("circle")))
                        } else {
                            Self::icon(element, "icon")
                        },
                    ),
                    ("role", Self::keyword(element, "role")),
                    ("prominence", Self::keyword(element, "prominence")),
                    ("enabled", self.enabled(element)),
                    ("action", self.action(element)),
                    ("style", self.style(kind, element.keyword("variant"))),
                    ("pick", self.field(element, "pick")),
                    ("files", Self::files(element)),
                    ("shape", self.shape(element)),
                    (
                        "fill",
                        (element.keyword("width") == Some("fill")).then(|| "true".to_string()),
                    ),
                    ("iconSize", Self::number(element, "font_size")),
                ]);
                if kind == "Button" && !element.children.is_empty() {
                    self.call(
                        out,
                        &format!("B::ContentButton(p, c, {props}, "),
                        &element.children,
                    );
                } else {
                    out.line(format!("B::{kind}(p, c, {props});"));
                }
                self.shortcut(out, element, "activate");
            }
            "TextInput" => out.line(format!(
                "B::TextInput(p, c, {});",
                designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("prompt", self.text_prop(element, "prompt")),
                    ("kind", Self::keyword(element, "kind")),
                    ("submit", self.action(element)),
                ])
            )),
            "Search" => {
                let props = self.search_props(element);
                // A palette opens over the window; its shortcut opens it.
                let (function, run) = if element.keyword("style") == Some("palette") {
                    ("SearchPalette", "activate")
                } else {
                    ("Search", "focus")
                };
                if element.children.is_empty() {
                    out.line(format!("B::{function}(p, c, {props}, nullptr);"));
                } else {
                    self.call(
                        out,
                        &format!("B::{function}(p, c, {props}, "),
                        &element.children,
                    );
                }
                self.shortcut(out, element, run);
            }
            "SearchItem" => {
                let props = designated(&[
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("icon", Self::icon(element, "icon")),
                    ("detail", self.text_prop(element, "detail")),
                    ("hint", self.text_prop(element, "hint")),
                    (
                        "navigation",
                        element.expr("navigation").map(|e| self.getter(e)),
                    ),
                    ("action", self.action(element)),
                    ("pick", self.field(element, "pick")),
                    ("files", Self::files(element)),
                ]);
                if element.children.is_empty() {
                    out.line(format!("B::SearchItem(p, c, {props}, nullptr);"));
                } else {
                    self.call(
                        out,
                        &format!("B::SearchItem(p, c, {props}, "),
                        &element.children,
                    );
                }
            }
            "Switch" => {
                let mut fields = vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                ];
                fields.extend(self.model(element, "checked"));
                fields.extend([
                    ("style", Self::keyword(element, "style")),
                    ("icon", Self::icon(element, "icon")),
                    (
                        "labelHidden",
                        (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
                    ),
                    ("shape", self.shape(element)),
                ]);
                out.line(format!("B::Switch(p, c, {});", designated(&fields)));
                self.shortcut(out, element, "activate");
            }
            "Slider" => {
                let controlled = element.expr("current").is_some();
                let mut fields = vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                ];
                if controlled {
                    fields.extend(self.model(element, "current"));
                } else {
                    fields.push(("field", self.field(element, "value")));
                }
                fields.extend([
                    ("min", Self::number(element, "min")),
                    ("max", Self::number(element, "max")),
                    ("step", Self::number(element, "step")),
                    ("minIcon", Self::icon(element, "min_icon")),
                    ("maxIcon", Self::icon(element, "max_icon")),
                    // A controlled slider's action is its change.
                    ("commit", if controlled { None } else { self.action(element) }),
                    (
                        "labelHidden",
                        (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
                    ),
                ]);
                out.line(format!("B::Slider(p, c, {});", designated(&fields)));
            }
            "Picker" => self.picker(out, element),
            "LanguagePicker" => out.line(format!(
                "B::LanguagePicker(p, c, {}, {});",
                self.id(element),
                null(self.text_prop(element, "label")),
            )),
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => self.call(
                out,
                &format!(
                    "B::Form(p, c, {}, {}, ",
                    null(self.text_prop(element, "title")),
                    designated(&[
                        ("gap", Self::number(element, "gap")),
                        ("rowHeight", Self::number(element, "row_height")),
                        ("paddingX", Self::number(element, "padding_x")),
                        ("paddingY", Self::number(element, "padding_y")),
                        (
                            "plainFirst",
                            matches!(
                                element.children.first(),
                                Some(Node::Element(first)) if first.kind == "Section" && first.keyword("style") == Some("plain")
                            )
                            .then(|| "true".to_string())
                        ),
                    ])
                ),
                &element.children,
            ),
            "Section" if self.in_sidebar.get() => self.call(
                out,
                &format!(
                    "B::SidebarSection(p, c, {}, ",
                    null(self.text_prop(element, "title"))
                ),
                &element.children,
            ),
            "Section" => self.call(
                out,
                &format!(
                    "B::Section(p, c, {}, {}, {}, ",
                    null(self.text_prop(element, "title")),
                    null(self.text_prop(element, "footer")),
                    designated(&[
                        (
                            "plain",
                            (element.keyword("style") == Some("plain")).then(|| "true".to_string())
                        ),
                        ("paddingX", Self::number(element, "padding_x")),
                        ("paddingY", Self::number(element, "padding_y")),
                    ])
                ),
                &element.children,
            ),
            "SidebarItem" => {
                let Some(value) = element.expr("value") else {
                    return;
                };
                out.line(format!(
                    "B::SidebarItem(p, c, {});",
                    designated(&[
                        ("value", Some(self.getter(value))),
                        ("name", Some(self.variant_name(value))),
                        ("label", self.text_prop(element, "label")),
                        ("icon", Self::icon(element, "icon")),
                        (
                            "style",
                            self.style("SidebarItem", element.keyword("variant"))
                        ),
                        ("badge", element.expr("badge").map(|badge| self.getter(badge))),
                    ])
                ));
            }
            "Column" | "Row" => self.container(
                out,
                element,
                &format!(
                    "B::Stack(p, c, {}, ",
                    designated(&[
                        (
                            "axis",
                            Some(string(if element.kind == "Column" {
                                "column"
                            } else {
                                "row"
                            }))
                        ),
                        ("gap", self.len(element, "gap")),
                        ("padding", self.len(element, "padding")),
                        ("width", self.len(element, "width")),
                        ("height", self.len(element, "height")),
                        ("align", Self::keyword(element, "align")),
                        ("animate", Self::animate(element)),
                        ("reflow", Self::reflow(element)),
                        ("paddingX", self.len(element, "padding_x")),
                        ("paddingY", self.len(element, "padding_y")),
                    ])
                ),
            ),
            "Grid" => self.container(
                out,
                element,
                &format!(
                    "B::Grid(p, c, {}, ",
                    designated(&[
                        ("minWidth", self.len(element, "min_width")),
                        ("gap", self.len(element, "gap")),
                        ("padding", self.len(element, "padding")),
                        ("animate", Self::animate(element)),
                        ("reflow", Self::reflow(element)),
                        ("rowGap", self.len(element, "row_gap")),
                        ("paddingX", self.len(element, "padding_x")),
                        ("paddingY", self.len(element, "padding_y")),
                        ("cellRatio", Self::number(element, "cell_ratio")),
                        ("cellExtra", self.len(element, "cell_extra")),
                    ])
                ),
            ),
            "Stack" => self.container(
                out,
                element,
                &format!(
                    "B::Layers(p, c, {}, ",
                    designated(&[
                        ("align", Self::keyword(element, "align")),
                        ("width", self.len(element, "width")),
                        ("height", self.len(element, "height")),
                        ("animate", Self::animate(element)),
                        ("reflow", Self::reflow(element)),
                        ("style", self.style("Stack", element.keyword("variant"))),
                        ("enabled", self.enabled(element)),
                    ])
                ),
            ),
            "ScaleBox" => self.call(
                out,
                &format!(
                    "B::ScaleBox(p, c, {}, ",
                    designated(&[
                        ("width", self.len(element, "width")),
                        ("height", self.len(element, "height")),
                    ])
                ),
                &element.children,
            ),
            "Positioned" => self.call(
                out,
                &format!(
                    "B::Positioned(p, c, {}, ",
                    designated(&[
                        ("x", self.len(element, "x")),
                        ("y", self.len(element, "y")),
                        ("width", self.len(element, "width")),
                        ("height", self.len(element, "height")),
                        ("animate", Self::animate(element)),
                    ])
                ),
                &element.children,
            ),
            "Marquee" => self.call(
                out,
                &format!(
                    "B::Marquee(p, c, {}, ",
                    designated(&[
                        ("duration", self.len(element, "duration")),
                        ("phase", Self::number(element, "phase")),
                        ("paused", element.expr("paused").map(|e| self.getter(e))),
                        ("opacity", self.len(element, "opacity")),
                    ])
                ),
                &element.children,
            ),
            "Accordion" => self.call(
                out,
                &format!(
                    "B::Accordion(p, c, {}, ",
                    designated(&[
                        ("count", self.len(element, "count")),
                        ("selected", self.len(element, "selected")),
                        ("collapsedWidth", self.len(element, "collapsed_width")),
                        ("gap", self.len(element, "gap")),
                        ("height", self.len(element, "height")),
                        ("corner", self.len(element, "corner")),
                        ("animate", Self::animate(element)),
                    ])
                ),
                &element.children,
            ),
            "Flow" => self.container(
                out,
                element,
                &format!(
                    "B::Flow(p, c, {}, ",
                    designated(&[("gap", self.len(element, "gap"))])
                ),
            ),
            "Modal" => {
                let transition = |name: &str| {
                    element
                        .keyword(name)
                        .map(|transition| format!("&X_{}", ident(transition)))
                };
                self.call(
                    out,
                    &format!(
                        "B::Modal(p, c, {}, ",
                        designated(&[
                            ("width", self.len(element, "width")),
                            ("height", self.len(element, "height")),
                            (
                                "cover",
                                (element.keyword("cover") == Some("compact"))
                                    .then(|| "true".to_string())
                            ),
                            ("appear", transition("appear")),
                            ("disappear", transition("disappear")),
                            ("appearDelay", Self::number(element, "appear_delay")),
                        ])
                    ),
                    &element.children,
                )
            }
            "Timer" => out.line(format!(
                "B::Timer(p, c, {});",
                designated(&[
                    ("interval", Self::number(element, "interval")),
                    ("enabled", self.enabled(element)),
                    ("action", self.action(element)),
                ])
            )),
            "ControlGroup" => self.call(
                out,
                &format!(
                    "B::ControlGroup(p, c, {}, ",
                    designated(&[
                        ("label", self.text_prop(element, "label")),
                        ("height", self.len(element, "height")),
                    ])
                ),
                &element.children,
            ),
            "Rail" => self.call(
                out,
                &format!(
                    "B::Rail(p, c, {}, ",
                    designated(&[
                        ("axis", Self::keyword(element, "axis")),
                        ("anchor", Self::keyword(element, "anchor")),
                        ("overhang", self.len(element, "overhang")),
                    ])
                ),
                &element.children,
            ),
            "Svg" => out.line(format!(
                "B::Svg(p, c, {});",
                designated(&[
                    ("source", element.resource("source").map(string)),
                    ("label", self.text_prop(element, "label")),
                    ("width", self.len(element, "width")),
                    ("height", self.len(element, "height")),
                    ("fit", Self::keyword(element, "fit")),
                    ("playing", element.expr("playing").map(|e| self.getter(e))),
                    ("speed", element.expr("speed").map(|e| self.getter(e))),
                    ("restart", element.expr("restart").map(|e| self.getter(e))),
                    (
                        "interactive",
                        element.keyword("interactive").map(str::to_owned)
                    ),
                ])
            )),
            "Image" => {
                let url = element.expr("source").map(|expr| self.getter(expr));
                out.line(format!(
                    "B::Image(p, c, {});",
                    designated(&[
                        ("source", element.resource("source").map(string)),
                        ("url", url),
                        ("label", self.text_prop(element, "label")),
                        ("width", self.len(element, "width")),
                        ("height", self.len(element, "height")),
                        ("fit", Self::keyword(element, "fit")),
                        ("animate", Self::animate(element)),
                        ("corner", self.len(element, "corner")),
                        (
                            "templated",
                            (element.keyword("rendering") == Some("template"))
                                .then(|| "true".to_string())
                        ),
                        ("aspect", Self::number(element, "aspect")),
                    ])
                ))
            }
            "Progress" => out.line(format!(
                "B::Progress(p, c, {});",
                designated(&[
                    (
                        "value",
                        element.expr("value").map(|value| self.getter(value))
                    ),
                    ("total", Self::number(element, "total")),
                    ("label", self.text_prop(element, "label")),
                ])
            )),
            "Canvas" => self.canvas(out, element),
            "Spacer" => out.line("B::Spacer(p);"),
            "Divider" => out.line("B::Divider(p, c);"),
            "ScrollView" => self.call(
                out,
                &format!(
                    "B::ScrollView(p, c, {}, ",
                    designated(&[
                        ("axis", Self::keyword(element, "axis")),
                        ("snap", Self::keyword(element, "snap")),
                        ("position", self.field(element, "position")),
                        ("arrival", Self::keyword(element, "arrival")),
                        ("nudgeMotion", Some(motion_ref("jelly"))),
                        ("reflow", Self::reflow(element)),
                        (
                            "hideIndicators",
                            (element.keyword("indicators") == Some("hidden")).then(|| "true".into()),
                        ),
                        ("openAt", Self::number(element, "open_at")),
                    ])
                ),
                &element.children,
            ),
            // Items before a `Spacer` stand at the leading end of the
            // header, after its way back; the rest at the trailing end.
            "Toolbar" => match element
                .children
                .iter()
                .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"))
            {
                Some(index) => {
                    self.call(out, "B::LeadingToolbar(p, c, ", &element.children[..index]);
                    self.call(out, "B::Toolbar(p, c, ", &element.children[index + 1..]);
                }
                None => self.call(out, "B::Toolbar(p, c, ", &element.children),
            },
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| self.getter(route))
                    .unwrap_or_else(|| "[](Ctx const&) { return Value::Unit(); }".into());
                let label = self
                    .row_label(&element.children)
                    .unwrap_or_else(|| "winrt::hstring()".into());
                self.call(
                    out,
                    &format!(
                        "B::NavLink(p, c, {}, {route}, [](Ctx const& c) {{ return {label}; }}, ",
                        self.id(element)
                    ),
                    &element.children,
                );
            }
            "List" => {
                if element.expr("items").is_some() {
                    out.open(format!("B::List(p, c, {BUILDER}"));
                    self.for_each(out, element);
                    out.close("});");
                } else {
                    self.call(out, "B::List(p, c, ", &element.children);
                }
            }
            "ForEach" => self.for_each(out, element),
            "Tabs" => self.tabs(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "PopoverButton" => {
                self.call(
                    out,
                    &format!(
                        "B::PopoverButton(p, c, {}, ",
                        designated(&[
                            ("id", Some(self.id(element))),
                            ("label", self.text_prop(element, "label")),
                            ("icon", Self::icon(element, "icon")),
                            ("presented", self.field(element, "presented")),
                        ])
                    ),
                    &element.children,
                );
                self.shortcut(out, element, "activate");
            }
            "SplitButton" => {
                self.call(
                    out,
                    &format!(
                        "B::SplitButton(p, c, {}, ",
                        designated(&[
                            ("id", Some(self.id(element))),
                            ("label", self.text_prop(element, "label")),
                            ("icon", Self::icon(element, "icon")),
                            ("field", self.field(element, "value")),
                            ("more", self.text_prop(element, "more_label")),
                            ("shape", self.shape(element)),
                        ])
                    ),
                    &element.children,
                );
                self.shortcut(out, element, "activate");
            }
            "Toast" => {
                let key = element.expr("key").map(|key| self.getter(key)).or_else(|| {
                    element.text("content").map(|text| {
                        format!(
                            "[](Ctx const& c) {{ return Value::Text(Utf8({})); }}",
                            self.text(text)
                        )
                    })
                });
                out.line(format!(
                    "B::Toast(p, c, {});",
                    designated(&[
                        ("content", self.text_prop(element, "content")),
                        ("icon", Self::icon(element, "icon")),
                        ("key", key),
                        ("duration", Self::number(element, "duration")),
                        ("motion", Some(motion_ref("bouncy"))),
                    ])
                ));
            }
            "Drop" => self.call(
                out,
                &format!(
                    "B::Drop(p, c, {}, ",
                    designated(&[
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("field", self.field(element, "value")),
                        ("files", Self::keyword(element, "files")),
                        ("over", self.field(element, "over")),
                        (
                            "opens",
                            (element.keyword("opens") == Some("true")).then(|| "true".to_string())
                        ),
                        ("action", self.action(element)),
                    ])
                ),
                &element.children,
            ),
            "Pinch" | "Swipe" => self.call(
                out,
                &format!(
                    "B::{}(p, c, {}, ",
                    element.kind,
                    designated(&[
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("field", self.field(element, "value")),
                        ("min", Self::number(element, "min")),
                        ("max", Self::number(element, "max")),
                        ("axis", Self::keyword(element, "axis")),
                        ("distance", Self::number(element, "distance")),
                        ("action", self.action(element)),
                    ])
                ),
                &element.children,
            ),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarHeader" | "SidebarFooter"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" | "Inspector" | "TabAccessory"
            | "PickerOption" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }

    fn search_props(&self, element: &Element) -> String {
        designated(&[
            ("id", Some(self.id(element))),
            ("field", self.field(element, "value")),
            ("prompt", self.text_prop(element, "prompt")),
            ("submit", self.action(element)),
            ("panelPrompt", self.text_prop(element, "panel_prompt")),
            ("width", Self::number(element, "width")),
            ("height", Self::number(element, "height")),
            ("fontSize", Self::number(element, "font_size")),
        ])
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
                let ExprKind::Variant { variant, .. } = &value.kind else {
                    return None;
                };
                Some(format!(
                    "B::PickOption{{{variant}, {}, {}}}",
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
                        "B::PickOption{{{index}, {}, {}}}",
                        string(&variant.name),
                        self.variant_label(def, index)
                    )
                })
                .collect()
        } else {
            explicit
        };
        let mut fields = vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
        ];
        fields.extend(self.model(element, "selected"));
        fields.extend([
            ("options", Some(format!("{{{}}}", options.join(", ")))),
            ("style", Self::keyword(element, "style")),
            (
                "labelHidden",
                (element.keyword("label_hidden") == Some("true")).then(|| "true".to_string()),
            ),
            ("shape", self.shape(element)),
        ]);
        out.line(format!("B::Picker(p, c, {});", designated(&fields)));
    }

    fn for_each(&self, out: &mut Out, element: &Element) {
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { field, .. })) => self.call(
                out,
                &format!(
                    "B::Each(p, c, {}, ",
                    designated(&[
                        ("items", Some(self.getter(items))),
                        ("key", Some(field.to_string())),
                        ("variable", Some(string(&template.var))),
                        ("stagger", Self::number(element, "stagger")),
                        ("reflow", Self::reflow(element)),
                    ])
                ),
                &element.children,
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
        let controls = element.keyword("controls") != Some("none");
        out.line(format!(
            "B::Video(p, c, {});",
            designated(&[
                ("id", Some(self.id(element))),
                ("source", source),
                ("player", player),
                ("label", self.text_prop(element, "label")),
                ("controls", (!controls).then(|| "false".to_string())),
                ("fit", Self::keyword(element, "fit")),
                ("corner", self.len(element, "corner")),
                ("animate", Self::animate(element)),
            ])
        ));
    }

    fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let params: Vec<_> = binding.args.iter().map(|arg| self.getter(arg)).collect();
        let props = designated(&[
            ("id", element.id.as_ref().map(|_| self.id(element))),
            ("query", Some(binding.query.to_string())),
            (
                "params",
                (!params.is_empty()).then(|| format!("{{{}}}", params.join(", "))),
            ),
            ("loading", self.text_prop(element, "loading")),
            ("failed", self.text_prop(element, "failed")),
            ("empty", self.text_prop(element, "empty")),
            ("icon", Self::icon(element, "icon")),
            ("variable", Some(string(&template.var))),
        ]);
        self.call(out, &format!("B::Query(p, c, {props}, "), &element.children);
    }

    /// The navigation view: its sidebar's items may come and go with `if`,
    /// `match` and `ForEach`, each registering while it is mounted.
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
        let search = search
            .filter(|search| search.field("value").is_some())
            .map(|search| format!("B::SearchProps{}", self.search_props(search)));
        out.open(format!(
            "B::SplitView(p, c, {},",
            designated(&[
                ("id", sidebar.id.as_ref().map(|_| self.id(sidebar))),
                ("selection", Some(selection)),
                (
                    "fallback",
                    sidebar.expr("fallback").map(|value| self.getter(value))
                ),
                ("search", search),
                ("width", Self::number(sidebar, "width")),
                (
                    "titled",
                    detail.expr("titled").map(|titled| self.getter(titled))
                ),
            ])
        ));
        // A header that only names the window is left out: the title bar
        // shows the application's icon and name.
        match header.filter(|header| header.keyword("role") != Some("title")) {
            Some(header) => self.children(out, "", &header.children, ","),
            None => out.line("nullptr,"),
        }
        match footer {
            Some(footer) => self.children(out, "", &footer.children, ","),
            None => out.line("nullptr,"),
        }
        self.in_sidebar.set(true);
        self.children(out, "", &content, ",");
        self.in_sidebar.set(false);
        self.children(out, "", &detail.children, ");");
        out.indent -= 1;
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
            "B::Navigation(p, c, {}, {path},",
            self.optional_id(element)
        ));
        self.children(out, "", &content, ",");
        out.line(format!("{},", string(&var)));
        let destination_nodes = destination
            .map(|destination| destination.children.clone())
            .unwrap_or_default();
        self.children(out, "", &destination_nodes, ");");
        out.indent -= 1;
    }

    /// One builder per layout size; the runtime picks the size by the width
    /// available and the breakpoints of `[layouts]`, and shows the layout of
    /// a size without one of its own as the next larger, else smaller.
    fn adaptive(&self, out: &mut Out, element: &Element) {
        let layout = |size: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size) => Some(layout),
                _ => None,
            })
        };
        out.open(format!(
            "B::Adaptive(p, c, {}, std::array<Build, 4>{{",
            self.optional_id(element)
        ));
        for size in ["compact", "medium", "expanded", "large"] {
            match layout(size) {
                Some(layout) => self.children(out, "", &layout.children, ","),
                None => out.line("nullptr,"),
            }
        }
        out.close("});");
    }

    /// A menu button and its items; dividers are empty items.
    fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "B::Menu(p, c, {}, {}, {}, {}, std::vector<std::optional<B::MenuItemDef>>{{",
            self.id(element),
            null(self.text_prop(element, "label")),
            null(Self::icon(element, "icon")),
            designated(&[
                (
                    "iconOnly",
                    (element.keyword("style") == Some("icon")).then(|| "true".to_string())
                ),
                ("shape", self.shape(element)),
            ])
            .replacen('{', "B::MenuLook{", 1),
        ));
        let mut shortcuts = Vec::new();
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("std::nullopt,");
                continue;
            }
            if item.keyword("shortcut").is_some() {
                shortcuts.push(item);
            }
            out.line(format!(
                "B::MenuItemDef{},",
                designated(&[
                    ("id", Some(self.id(item))),
                    ("label", self.text_prop(item, "label")),
                    ("icon", Self::icon(item, "icon")),
                    ("role", Self::keyword(item, "role")),
                    ("enabled", self.enabled(item)),
                    ("action", self.action(item)),
                    ("pick", self.field(item, "pick")),
                    ("files", Self::files(item)),
                    ("shortcut", Self::keyword(item, "shortcut")),
                ])
            ));
        }
        out.close("});");
        for item in shortcuts {
            self.shortcut(out, item, "activate");
        }
    }

    /// Tabs whose items may come and go: each `Tab` registers while it is
    /// mounted.
    fn tabs(&self, out: &mut Out, element: &Element) {
        let Some(selection) = self.field(element, "selection") else {
            return;
        };
        out.open(format!(
            "B::Tabs(p, c, {}, {BUILDER}",
            designated(&[
                ("id", element.id.as_ref().map(|_| self.id(element))),
                ("selection", Some(selection)),
                (
                    "fallback",
                    element.expr("fallback").map(|value| self.getter(value))
                ),
            ])
        ));
        self.tab_items(out, &element.children);
        let accessory = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "TabAccessory" => Some(child),
            _ => None,
        });
        match accessory {
            Some(accessory) => {
                out.close("},");
                out.indent += 1;
                self.children(
                    out,
                    &format!(
                        "B::TabAccessory{{.shown = {}, .build = ",
                        accessory
                            .expr("shown")
                            .map(|shown| self.getter(shown))
                            .unwrap_or_else(|| "nullptr".into())
                    ),
                    &accessory.children,
                    "});",
                );
                out.indent -= 1;
            }
            None => out.close("});"),
        }
    }

    fn tab_items(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element(tab) if tab.kind == "Tab" => {
                    let Some(value) = tab.expr("value") else {
                        continue;
                    };
                    let head = designated(&[
                        ("value", Some(self.getter(value))),
                        ("name", Some(self.variant_name(value))),
                        ("label", self.text_prop(tab, "label")),
                        ("icon", Self::icon(tab, "icon")),
                    ]);
                    // The builder is the last field: leave the braces open.
                    let head = &head[..head.len() - 1];
                    let separator = if head.ends_with('{') { "" } else { ", " };
                    self.children(
                        out,
                        &format!("B::Tab(p, c, B::TabSpec{head}{separator}.build = "),
                        &tab.children,
                        "});",
                    );
                }
                Node::Element(other) if other.kind == "ForEach" => {
                    if let (Some(items), Some(template), Some(Prop::Key { field, .. })) =
                        (other.expr("items"), &other.template, other.prop("key"))
                    {
                        out.open(format!(
                            "B::Each(p, c, {}, {BUILDER}",
                            designated(&[
                                ("items", Some(self.getter(items))),
                                ("key", Some(field.to_string())),
                                ("variable", Some(string(&template.var))),
                            ])
                        ));
                        self.tab_items(out, &other.children);
                        out.close("});");
                    }
                }
                Node::If(branch) => {
                    let mut cases = String::from("[](Ctx const& c) -> std::optional<int> { ");
                    for (index, (condition, _)) in branch.branches.iter().enumerate() {
                        cases.push_str(&format!(
                            "if ({}) return {index}; ",
                            self.condition(condition)
                        ));
                    }
                    if branch.otherwise.is_empty() {
                        cases.push_str("return std::nullopt; }");
                    } else {
                        cases.push_str(&format!("return {}; }}", branch.branches.len()));
                    }
                    out.open(format!("B::When(p, c, {cases}, std::vector<Build>{{"));
                    for body in branch
                        .branches
                        .iter()
                        .map(|(_, body)| body)
                        .chain((!branch.otherwise.is_empty()).then_some(&branch.otherwise))
                    {
                        out.open(BUILDER);
                        self.tab_items(out, body);
                        out.close("},");
                    }
                    out.close("});");
                }
                Node::IfLet(branch) => {
                    out.open(format!(
                        "B::IfLet(p, c, {}, {}, {BUILDER}",
                        string(&branch.var),
                        self.getter(&branch.value)
                    ));
                    self.tab_items(out, &branch.then);
                    if branch.otherwise.is_empty() {
                        out.close("});");
                    } else {
                        out.close(format!("}}, {BUILDER}"));
                        out.indent += 1;
                        self.tab_items(out, &branch.otherwise);
                        out.close("});");
                    }
                }
                Node::Match(branch) => {
                    out.open(format!(
                        "B::Match(p, c, {}, std::vector<B::MatchArm>{{",
                        self.getter(&branch.value)
                    ));
                    for arm in &branch.arms {
                        let bindings: Vec<String> = arm
                            .bindings
                            .iter()
                            .map(|binding| {
                                format!("{{{}, {}}}", string(&binding.name), binding.field)
                            })
                            .collect();
                        out.open(format!(
                            "B::Arm({}, {BUILDER}",
                            arm.variant
                                .map(|variant| format!("{variant}u"))
                                .unwrap_or_else(|| "std::nullopt".into())
                        ));
                        self.tab_items(out, &arm.body);
                        if bindings.is_empty() {
                            out.close("}),");
                        } else {
                            out.close(format!("}}, {{{}}}),", bindings.join(", ")));
                        }
                    }
                    out.close("});");
                }
                _ => {}
            }
        }
    }

    /// A `Canvas`: its drawing elements in order, read in one getter so the
    /// canvas redraws when the state they show changes.
    fn canvas(&self, out: &mut Out, element: &Element) {
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else { continue };
            let number = |name: &str| Self::number(child, name);
            match child.kind.as_str() {
                "Shape" => {
                    let fill_from = match (child.expr("fill_from"), child.resource("fill_from")) {
                        (Some(expr), _) => Some(format!("R::Url({})", self.expr(expr))),
                        (None, Some(name)) => Some(format!("B::ResourceUrl({})", string(name))),
                        (None, None) => None,
                    };
                    ops.push(format!(
                        "B::ShapeOp{}",
                        designated(&[
                            (
                                "kind",
                                Some(string(child.keyword("kind").unwrap_or("rect")))
                            ),
                            ("x", number("x")),
                            ("y", number("y")),
                            ("width", number("width")),
                            ("height", number("height")),
                            ("corner", number("corner")),
                            ("fill", role(child, "fill")),
                            ("fillFrom", fill_from),
                            ("fade", Self::keyword(child, "fade")),
                            ("stroke", role(child, "stroke")),
                            ("strokeWidth", number("stroke_width")),
                            ("opacity", number("opacity")),
                        ])
                    ));
                }
                "Series" => {
                    let values = child
                        .expr("values")
                        .map(|list| format!("R::Numbers({})", self.expr(list)));
                    let max = match child.prop("max") {
                        Some(Prop::Expr { expr }) => {
                            Some(format!("R::AsDouble({})", self.expr(expr)))
                        }
                        Some(Prop::Number { value }) => Some(float(*value)),
                        _ => None,
                    };
                    ops.push(format!(
                        "B::SeriesOp{}",
                        designated(&[
                            ("values", values),
                            ("max", max),
                            (
                                "area",
                                (child.keyword("kind") == Some("area")).then(|| "true".to_string())
                            ),
                            ("fill", role(child, "fill")),
                            ("stroke", role(child, "stroke")),
                            ("strokeWidth", number("stroke_width")),
                            ("opacity", number("opacity")),
                        ])
                    ));
                }
                "Pattern" => ops.push(format!(
                    "B::PatternOp{}",
                    designated(&[
                        (
                            "pattern",
                            child
                                .keyword("name")
                                .map(|name| format!("&P_{}", ident(name)))
                        ),
                        ("opacity", number("opacity")),
                    ])
                )),
                _ => {}
            }
        }
        out.open(format!(
            "B::Canvas(p, c, {}, [](Ctx const& c) {{",
            designated(&[
                ("width", self.len(element, "width")),
                ("height", self.len(element, "height")),
                ("label", self.text_prop(element, "label")),
                ("animate", Self::animate(element)),
            ])
        ));
        out.line("std::vector<B::DrawOp> ops;");
        for op in ops {
            out.line(format!("ops.push_back({op});"));
        }
        out.line("return ops;");
        out.close("});");
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!(
            "static void c_{}(Parent const& p, Ctx const& c) {{",
            component.name
        ));
        self.node(out, &component.body);
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!(
            "static void w_{}(Parent const& p, Ctx const& c) {{",
            window.id
        ));
        self.node(out, &window.content);
        out.close("}");
        out.line("");
        if let Some(close) = &window.close {
            let save = match &close.save {
                Some(Action::Engine { command, args }) => format!(
                    "[](Ctx const& c, Done done) {{ Dispatch(c, {command}, {}, std::move(done)); }}",
                    self.values(args)
                ),
                _ => "nullptr".into(),
            };
            out.line(format!(
                "/// The close check of the `{}` window, read against its own drafts.",
                window.id
            ));
            out.open(format!(
                "static CloseCheck close_{}(Ctx const& c) {{",
                window.id
            ));
            out.line(format!(
                "return CloseCheck{{{}, {}, {save}}};",
                self.condition(&close.when),
                self.text(&close.message)
            ));
            out.close("}");
            out.line("");
        }
    }

    /// One look of a style: a state's colors and transforms, assigned
    /// through `target` (`s.base.` or `s.hover->`).
    fn look(out: &mut Out, target: &str, effects: &[StyleEffect]) {
        for effect in effects {
            match effect {
                StyleEffect::Background { color } => {
                    out.line(format!("{target}background = {};", paint(color)))
                }
                StyleEffect::Foreground { color } => {
                    out.line(format!("{target}foreground = {};", paint(color)))
                }
                StyleEffect::Border { color, width } => {
                    out.line(format!("{target}border = {};", paint(color)));
                    out.line(format!("{target}borderWidth = {};", float(*width)));
                }
                StyleEffect::Fill { color } => {
                    out.line(format!("{target}fill = {};", paint(color)))
                }
                StyleEffect::Indicator { color, edge, width } => {
                    out.line(format!("{target}indicator = {};", paint(color)));
                    out.line(format!(
                        "{target}indicatorEdge = {};",
                        string(&format!("{edge:?}").to_lowercase())
                    ));
                    out.line(format!("{target}indicatorWidth = {};", float(*width)));
                }
                StyleEffect::Scale { x, y, motion } => {
                    out.line(format!("{target}scaleX = {};", float(*x)));
                    out.line(format!("{target}scaleY = {};", float(*y)));
                    out.line(format!("{target}motion = {};", motion_ref(motion)));
                }
                StyleEffect::Offset { x, y, motion } => {
                    out.line(format!("{target}offsetX = {};", float(*x)));
                    out.line(format!("{target}offsetY = {};", float(*y)));
                    out.line(format!("{target}motion = {};", motion_ref(motion)));
                }
                StyleEffect::Opacity { value, motion } => {
                    out.line(format!("{target}opacity = {};", float(*value)));
                    out.line(format!("{target}motion = {};", motion_ref(motion)));
                }
                StyleEffect::Shadow { radius, y, opacity } => out.line(format!(
                    "{target}shadow = std::array<double, 3>{{{}, {}, {}}};",
                    float(*radius),
                    float(*y),
                    float(*opacity)
                )),
                StyleEffect::System => out.line(format!("{target}system = true;")),
                StyleEffect::Nothing => out.line(format!("{target}quiet = true;")),
                StyleEffect::Corner { .. }
                | StyleEffect::Padding { .. }
                | StyleEffect::Size { .. } => {}
            }
        }
    }

    /// The motion tokens (`M_`), transitions (`X_`), patterns (`P_`) and
    /// style overrides (`S_`) of `style!`.
    fn style_tables(&self, out: &mut Out) {
        let style = &self.app.style;
        out.line("// Motion tokens: period and damping ratio of each spring.");
        for token in &style.motion {
            out.line(format!(
                "[[maybe_unused]] static B::Motion const M_{}{{{}, {}}};",
                ident(&token.name),
                float(token.period),
                float(token.damping)
            ));
        }
        out.line("// Transitions of `appear` and `disappear`.");
        for transition in &style.transitions {
            let motion = motion_ref(&transition.motion);
            let body = match &transition.kind {
                TransitionKind::Fade => format!(".kind = B::Transition::Fade, .motion = {motion}"),
                TransitionKind::Grow { edge } => format!(
                    ".kind = B::Transition::Grow, .edge = {}, .motion = {motion}",
                    string(&format!("{edge:?}").to_lowercase())
                ),
                TransitionKind::Pop { from } => format!(
                    ".kind = B::Transition::Pop, .from = {}, .motion = {motion}",
                    float(*from)
                ),
                TransitionKind::Slide { edge, distance } => format!(
                    ".kind = B::Transition::Slide, .edge = {}, .distance = {}, .motion = {motion}",
                    string(&format!("{edge:?}").to_lowercase()),
                    float(*distance)
                ),
            };
            out.line(format!(
                "[[maybe_unused]] static B::Transition const X_{}{{{body}}};",
                ident(&transition.name)
            ));
        }
        out.line("// Patterns of `style!`.");
        for pattern in &style.patterns {
            let bands: Vec<String> = pattern
                .bands
                .iter()
                .map(|band| {
                    format!(
                        "B::Band{{{}, {}, {}}}",
                        band.color
                            .as_ref()
                            .map(paint)
                            .unwrap_or_else(|| "B::Paint{}".into()),
                        float(band.width),
                        float(band.opacity)
                    )
                })
                .collect();
            out.line(format!(
                "[[maybe_unused]] static B::PatternDef const P_{}{{{{{}}}, {}}};",
                ident(&pattern.name),
                bands.join(", "),
                pattern.twill
            ));
        }
        out.line("// Style overrides of `style!`, as this target shows them.");
        for style in bern_target_api::style::used(self.app, PLATFORM) {
            out.open(format!(
                "[[maybe_unused]] static B::StyleDef const S_{} = [] {{",
                style.name
            ));
            out.line("B::StyleDef s;");
            Self::look(out, "s.base.", &style.base);
            for effect in &style.base {
                match effect {
                    StyleEffect::Corner { radius } => {
                        out.line(format!("s.corner = {};", float(*radius)))
                    }
                    StyleEffect::Padding { length } => {
                        out.line(format!("s.padding = {};", float(*length)))
                    }
                    _ => {}
                }
            }
            if let Some(size) = style.size() {
                let size = match size {
                    ControlSize::Small => "small",
                    ControlSize::Regular => "regular",
                    ControlSize::Large => "large",
                    ControlSize::ExtraLarge => "extra_large",
                };
                out.line(format!("s.size = {};", string(size)));
            }
            if style.draws_look() {
                out.line("s.draws = true;");
            }
            for (state, effects) in &style.states {
                let name = state.name();
                out.line(format!("s.{name} = B::Look{{}};"));
                Self::look(out, &format!("s.{name}->"), effects);
            }
            if let Some(settle) = &style.settle {
                out.line(format!("s.settle = {};", motion_ref(settle)));
            }
            out.line("return s;");
            out.close("}();");
        }
        out.line("");
    }

    /// The accent and appearance the `WindowSet` chooses at run time.
    fn theme_functions(&self, out: &mut Out) {
        let windows = &self.app.windows;
        if let Some(accent) = &windows.accent {
            out.line(
                "/// The accent of `WindowSet(accent:)`: its index in the accents of `style!`.",
            );
            out.open("static int ThemeAccent(Ctx const& c) {");
            out.line(format!("auto tag = R::Tag({});", self.expr(accent)));
            if let Type::Enum { name } = &accent.ty
                && let Some(def) = self.types().enum_def(name)
            {
                for (index, variant) in def.variants.iter().enumerate() {
                    if let Some(position) = self
                        .app
                        .style
                        .accents
                        .iter()
                        .position(|accent| Some(accent) == self.app.style.accent(&variant.name))
                    {
                        out.line(format!("if (tag == {index}u) return {position};"));
                    }
                }
            }
            out.line("return -1;");
            out.close("}");
        }
        if let Some(appearance) = &windows.appearance {
            out.line(
                "/// The appearance of `WindowSet(appearance:)`: light, dark or the system's.",
            );
            out.open("static char const* ThemeAppearance(Ctx const& c) {");
            out.line(format!("auto tag = R::Tag({});", self.expr(appearance)));
            if let Type::Enum { name } = &appearance.ty
                && let Some(def) = self.types().enum_def(name)
            {
                for (index, variant) in def.variants.iter().enumerate() {
                    let mode = variant.name.to_lowercase();
                    if mode == "light" || mode == "dark" {
                        out.line(format!("if (tag == {index}u) return {};", string(&mode)));
                    }
                }
            }
            out.line("return \"\";");
            out.close("}");
        }
        out.line("");
    }

    /// The application's interface, `app_ui.inl`: its components, windows
    /// and their close checks, included once per front end.
    pub fn interface(&self) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("// Included by the runtime once per front end.");
        out.line("");
        self.style_tables(&mut out);
        self.theme_functions(&mut out);
        out.line("// Components.");
        for component in &app.components {
            out.line(format!(
                "static void c_{}(Parent const& p, Ctx const& c);",
                component.name
            ));
        }
        out.line("");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }
        out.line("/// The window kinds, as the runtime sees them.");
        out.open("static std::vector<WindowDef> Windows() {");
        out.line("std::vector<WindowDef> windows;");
        for window in &app.windows.windows {
            let optional = |value: Option<f64>| match value {
                Some(value) => float(value),
                None => "std::nullopt".into(),
            };
            let sessions: Vec<_> = window
                .sessions
                .iter()
                .map(|name| self.session_index(name).to_string())
                .collect();
            out.open("{");
            out.line("WindowDef w;");
            out.line(format!("w.id = {};", string(&window.id)));
            out.line(format!("w.title = {};", string(&window.title.key)));
            out.line(format!(
                "w.settings = {};",
                window.role == WindowRole::Settings
            ));
            out.line(format!("w.dialog = {};", window.role == WindowRole::Dialog));
            out.line(format!("w.width = {};", optional(window.width)));
            out.line(format!("w.height = {};", optional(window.height)));
            out.line(format!("w.minWidth = {};", optional(window.min_width)));
            out.line(format!("w.minHeight = {};", optional(window.min_height)));
            out.line(format!(
                "w.initialOpen = {};",
                window.initial == WindowInitial::Open
            ));
            out.line(format!("w.sessions = {{{}}};", sessions.join(", ")));
            out.line(format!("w.render = w_{};", window.id));
            if window.close.is_some() {
                out.line(format!("w.close = close_{};", window.id));
            }
            out.line(format!(
                "w.unified = {};",
                window.chrome == WindowChrome::Unified
            ));
            out.line(format!("w.idleAfter = {};", optional(window.idle_after)));
            if let Some(enabled) = &window.idle_enabled {
                out.line(format!(
                    "w.idleEnabled = [](Ctx const& c) {{ return {}; }};",
                    self.condition(enabled)
                ));
            }
            if let Some(immersive) = &window.immersive {
                out.line(format!(
                    "w.immersive = [](Ctx const& c) {{ return {}; }};",
                    self.condition(immersive)
                ));
            }
            if app.windows.accent.is_some() {
                out.line("w.accent = ThemeAccent;");
            }
            if app.windows.appearance.is_some() {
                out.line("w.appearance = ThemeAppearance;");
            }
            out.line("windows.push_back(std::move(w));");
            out.close("}");
        }
        out.line("return windows;");
        out.close("}");
        out.line("");
        out.line("/// The accents of `style!`, for `WindowSet(accent:)`: light and dark sRGB.");
        out.open("static std::vector<std::pair<uint32_t, uint32_t>> AccentColors() {");
        let accents: Vec<String> = app
            .style
            .accents
            .iter()
            .map(|accent| match &accent.color {
                ColorRef::Fixed { light, dark } => format!(
                    "{{0x{:02X}{:02X}{:02X}u, 0x{:02X}{:02X}{:02X}u}}",
                    light.red, light.green, light.blue, dark.red, dark.green, dark.blue
                ),
                ColorRef::Role { .. } => "{0u, 0u}".to_string(),
            })
            .collect();
        out.line(format!("return {{{}}};", accents.join(", ")));
        out.close("}");
        out.finish()
    }

    fn type_table(out: &mut Out, types: &TypeTable) {
        for def in &types.enums {
            let variants: Vec<_> = def
                .variants
                .iter()
                .map(|variant| {
                    let fields: Vec<_> = variant
                        .fields
                        .iter()
                        .map(|field| field.ty.clone())
                        .collect();
                    // The fields its label shows, by placeholder name.
                    let shown: Vec<_> = variant
                        .label
                        .iter()
                        .flat_map(|label| &label.params)
                        .filter_map(|param| {
                            let index = variant
                                .fields
                                .iter()
                                .position(|field| field.name == *param)?;
                            Some(format!("{{{}, {index}}}", string(param)))
                        })
                        .collect();
                    if shown.is_empty() {
                        format!(
                            "VariantDef{{{}, {}}}",
                            string(&variant.name),
                            types_list(&fields)
                        )
                    } else {
                        format!(
                            "VariantDef{{{}, {}, {{{}}}}}",
                            string(&variant.name),
                            types_list(&fields),
                            shown.join(", ")
                        )
                    }
                })
                .collect();
            out.line(format!(
                "c.types.Enum({}, {{{}}});",
                string(&def.name),
                variants.join(", ")
            ));
        }
        for def in &types.records {
            let fields: Vec<_> = def.fields.iter().map(|field| field.ty.clone()).collect();
            out.line(format!(
                "c.types.Record({}, {});",
                string(&def.name),
                types_list(&fields)
            ));
        }
    }

    /// The application catalog, `app_catalog.cpp`: everything the runtime
    /// knows about the application that does not depend on a XAML
    /// namespace.
    pub fn catalog(&self, fingerprint: u64) -> String {
        let app = self.app;
        let protocol = &app.protocol;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("");
        // Through the precompiled header, which starts with bern_base.h.
        out.line("#include \"bern_pch.h\"");
        out.line("");
        out.line("#include <limits>");
        out.line("");
        out.open("namespace bern {");
        out.line("");
        let tables = crate::strings::tables(app);
        out.line("/// Strings of every supported language.");
        for (index, (language, table)) in tables.iter().enumerate() {
            out.line(format!("// {language}"));
            out.open(format!(
                "static std::pair<char const*, wchar_t const*> const kStrings{index}[] = {{"
            ));
            for (key, text) in table {
                out.line(format!("{{{}, {}}},", string(key), wide(text)));
            }
            out.close("};");
        }
        out.line("");
        out.line("/// The application, as the runtime sees it.");
        out.open("Catalog BernCatalog() {");
        out.line("Catalog c;");
        out.line(format!("c.id = {};", string(&app.app.id)));
        out.line(format!("c.bundleId = {};", string(&app.app.bundle_id)));
        out.line("c.name = \"app.name\";");
        out.line(format!("c.version = {};", string(&app.app.version)));
        out.line(format!("c.fingerprint = 0x{fingerprint:016x}ULL;"));
        let languages: Vec<_> = app
            .languages
            .supported
            .iter()
            .map(|language| string(language))
            .collect();
        out.line(format!("c.languages = {{{}}};", languages.join(", ")));
        out.line(format!(
            "c.defaultLanguage = {};",
            string(&app.languages.default)
        ));
        for (index, (language, table)) in tables.iter().enumerate() {
            out.open("{");
            out.line(format!("auto& table = c.strings[{}];", string(language)));
            out.line(format!("table.reserve({});", table.len()));
            out.line(format!(
                "for (auto const& [key, text] : kStrings{index}) table.emplace(key, text);"
            ));
            out.close("}");
        }
        out.line("// Protocol, UI, media and session catalog types.");
        Self::type_table(&mut out, &merged_types(app));
        let state: Vec<Type> = protocol
            .state
            .iter()
            .map(|field| field.ty.clone())
            .collect();
        out.line(format!("c.state = {};", types_list(&state)));
        out.open("c.commands = {");
        for command in &protocol.commands {
            let params: Vec<Type> = command
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect();
            out.line(format!(
                "CommandDef{{{}, {}, {}, {}}},",
                string(&command.id),
                types_list(&params),
                type_expr(&command.result),
                command.kind == OperationKind::Query,
            ));
        }
        out.close("};");
        out.open("c.sessions = {");
        for session in &app.sessions {
            let fields: Vec<_> = session
                .fields
                .iter()
                .map(|field| format!("{{{}, {}}}", string(&field.name), type_expr(&field.ty)))
                .collect();
            out.line(format!(
                "SessionDef{{{}, {{{}}}}},",
                string(&session.name),
                fields.join(", ")
            ));
        }
        out.close("};");
        let (_, session_specs) = session_catalog();
        let session_results: Vec<Type> = SessionCommand::ALL
            .iter()
            .map(|command| {
                session_specs
                    .iter()
                    .find(|spec| spec.id == command.id())
                    .map(|spec| spec.result.clone())
                    .unwrap_or(Type::Unit)
            })
            .collect();
        out.line(format!(
            "c.sessionResults = {};",
            types_list(&session_results)
        ));
        out.line(format!("c.primary = {};", string(&app.windows.primary)));
        let players: Vec<_> = app
            .players
            .iter()
            .map(|player| string(&player.id))
            .collect();
        out.line(format!("c.players = {{{}}};", players.join(", ")));
        if let Some(layouts) = app.layouts {
            out.line(format!(
                "c.compactBelow = {};",
                float(layouts.compact_below)
            ));
            if let Some(below) = layouts.medium_below {
                out.line(format!("c.mediumBelow = {};", float(below)));
            }
            if let Some(below) = layouts.expanded_below {
                out.line(format!("c.expandedBelow = {};", float(below)));
            }
        }
        // The weights and sizes `text { … }` gives text styles.
        for entry in &app.style.text_weights {
            out.line(format!(
                "c.textStyles.push_back({{{}, {}, {}}});",
                string(&entry.style),
                string(&entry.weight),
                entry
                    .size
                    .map(|size| format!("std::optional<double>({})", float(size)))
                    .unwrap_or_else(|| "std::nullopt".into())
            ));
        }
        out.line("// File kinds of `files:`, by extension.");
        for (kind, extensions) in bern_ir::spec::FILE_KIND_EXTENSIONS {
            let list: Vec<String> = extensions.iter().map(|ext| string(ext)).collect();
            out.line(format!(
                "c.fileKinds.push_back({{{}, {{{}}}}});",
                string(kind),
                list.join(", ")
            ));
        }
        if let Some(seed) = app.theme.brand_seed {
            out.line(format!(
                "c.brandSeed = std::array<uint8_t, 3>{{{}, {}, {}}};",
                seed.red, seed.green, seed.blue
            ));
        }
        out.line("return c;");
        out.close("}");
        out.line("");
        out.line("/// The glyph of a Bern icon name in the system icon fonts.");
        out.open("wchar_t const* IconGlyph(std::string_view name) {");
        out.open("static std::unordered_map<std::string_view, wchar_t const*> const glyphs = {");
        for (name, code) in crate::icons::GLYPHS {
            out.line(format!("{{{}, L\"\\x{code:04X}\"}},", string(name)));
        }
        out.close("};");
        out.line("auto found = glyphs.find(name);");
        out.line("return found == glyphs.end() ? L\"\" : found->second;");
        out.close("}");
        out.line("");
        out.close("}");
        out.finish()
    }

    /// `app_id.h`: what the entry point needs before any front end runs.
    pub fn identity(&self) -> String {
        format!(
            "// Generated by Bern. Do not edit.\n#pragma once\n#define BERN_BUNDLE_ID {}\n\n// What a build with WinUI 3 alone offers when the system has no\n// Windows App Runtime: the installer of the newest release.\n#define BERN_RUNTIME_INSTALLER {}\n",
            string(&self.app.app.bundle_id),
            wide(&crate::glue::runtime_installer())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_escape_for_cpp() {
        assert_eq!(string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        assert_eq!(string("中文"), "\"中文\"");
        assert_eq!(string("\u{1}2"), "\"\\0012\"");
        assert_eq!(wide("中文"), "L\"中文\"");
    }

    #[test]
    fn floats_are_double_literals() {
        assert_eq!(float(1.0), "1.0");
        assert_eq!(float(0.25), "0.25");
        assert_eq!(
            float(f64::INFINITY),
            "std::numeric_limits<double>::infinity()"
        );
    }

    #[test]
    fn literals_build_values() {
        let value = Value::Enum {
            variant: 2,
            fields: vec![Value::Text("x".into()), Value::Optional(None)],
        };
        assert_eq!(
            literal(&value),
            "Value::Enum(2u, {Value::Text(\"x\"), Value::None()})"
        );
        assert_eq!(literal(&Value::List(vec![])), "Value::List({})");
        assert_eq!(
            literal(&Value::Int(i64::MIN)),
            "Value::Int((-9223372036854775807LL - 1))"
        );
    }

    #[test]
    fn designated_initializers_skip_missing_fields() {
        assert_eq!(
            designated(&[
                ("id", Some("\"a\"".into())),
                ("label", None),
                ("icon", Some("\"x\"".into()))
            ]),
            "{.id = \"a\", .icon = \"x\"}"
        );
        assert_eq!(designated(&[("id", None)]), "{}");
    }
}
