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

mod catalog;
mod content;
mod controls;
mod interface;
mod layout;
mod navigation;
mod nodes;
mod styles;

use bern_ir::{
    Action, Application, BinaryOp, Binding, Element, Expr, ExprKind, FieldRef, MediaOp, Prop,
    SessionOp, Text, WindowOp,
};
use bern_protocol::{Type, TypeTable, Value};

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
            // A named corner a build for a platform has not resolved:
            // its radius on every platform.
            ExprKind::Corner { radius, .. } => literal(&Value::Float(*radius)),
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
                    SessionOp::PushStr { value } => format!(
                        "c.Write({target}, R::Append(c.Peek({target}), {}))",
                        self.expr(value)
                    ),
                    SessionOp::Pop => format!("c.Write({target}, R::DropLast(c.Peek({target})))"),
                }
            }
            Action::Engine { command, args } => {
                format!("Dispatch(c, {command}, {})", self.values(args))
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => {
                    format!("WindowOps::Open(c, {})", string(window))
                }
                WindowOp::ToggleFullscreen => {
                    format!("WindowOps::ToggleFullscreen(c, {})", string(window))
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
            .map(|action| match element.clearing() {
                // The draft empties after the action.
                Some(clear) => format!(
                    "[](Ctx const& c) {{ {}; {}; }}",
                    self.action_body(action),
                    self.action_body(&clear)
                ),
                None => format!("[](Ctx const& c) {{ {}; }}", self.action_body(action)),
            })
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
