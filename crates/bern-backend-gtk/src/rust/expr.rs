//! Expressions, bindings, actions and the props elements take.

use super::{Generator, Out, float, literal, some, string};
use crate::style;
use bern_ir::{
    Action, BinaryOp, Binding, Element, Expr, ExprKind, MediaOp, Prop, SessionOp, Text, WindowOp,
};
use bern_protocol::{EnumDef, Type, Value};

impl Generator<'_> {
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

    pub(super) fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value),
            // A named corner a build for a platform has not resolved:
            // its radius on every platform.
            ExprKind::Corner { radius, .. } => literal(&Value::Float(*radius)),
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

    pub(super) fn condition(&self, expr: &Expr) -> String {
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
    pub(super) fn argument(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Record { name } if name == bern_protocol::format::CALENDAR_DATE => {
                format!("b::text::date(&{value})")
            }
            Type::Enum { name } => format!("b::text::arg(&{value}, Some({}))", string(name)),
            _ => format!("b::text::arg(&{value}, None)"),
        }
    }

    pub(super) fn text(&self, text: &Text) -> String {
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

    pub(super) fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text_getter(text))
    }

    pub(super) fn getter(&self, expr: &Expr) -> String {
        format!("b::get(|c| {})", self.expr(expr))
    }

    pub(super) fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    fn field_ref(&self, field: &bern_ir::FieldRef) -> String {
        format!(
            "b::Field {{ session: {}, field: {} }}",
            self.session_index(&field.session),
            field.field
        )
    }

    pub(super) fn field(&self, element: &Element, name: &str) -> Option<String> {
        element.field(name).map(|field| self.field_ref(field))
    }

    /// A length: a number, or one bound to an expression.
    pub(super) fn num(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(format!("b::Num::Fixed({})", float(*value))),
            Prop::Expr { expr } => Some(format!("b::Num::Bound({})", self.getter(expr))),
            _ => None,
        }
    }

    /// The style an element of `kind` uses, as a reference to `STYLES`.
    pub(super) fn style(&self, kind: &str, element: &Element) -> String {
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

    pub(super) fn pick(&self, element: &Element) -> String {
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
                    SessionOp::PushStr { value } => {
                        format!("b::session_push_str(c, {target}, {})", self.expr(value))
                    }
                    SessionOp::Pop => format!("b::session_pop(c, {target})"),
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
                WindowOp::ToggleFullscreen => {
                    format!("b::toggle_fullscreen(c, {})", string(window))
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

    pub(super) fn action(&self, element: &Element) -> String {
        some(
            element
                .action
                .as_ref()
                .map(|action| match element.clearing() {
                    // The draft empties after the action.
                    Some(clear) => format!(
                        "b::act(|c| {{ {}; {}; }})",
                        self.action_body(action),
                        self.action_body(&clear)
                    ),
                    None => format!("b::act(|c| {})", self.action_body(action)),
                }),
        )
    }

    pub(super) fn keyword(element: &Element, name: &str) -> String {
        some(element.keyword(name).map(string))
    }

    pub(super) fn icon(element: &Element, name: &str) -> String {
        some(element.icon(name).map(string))
    }

    pub(super) fn number(element: &Element, name: &str) -> String {
        some(element.number(name).map(float))
    }

    pub(super) fn enabled(&self, element: &Element) -> String {
        some(element.expr("enabled").map(|expr| self.getter(expr)))
    }

    /// A bound value, as a getter.
    pub(super) fn value(&self, element: &Element, name: &str) -> String {
        some(element.expr(name).map(|expr| self.getter(expr)))
    }

    /// `padding`, `padding_x` and `padding_y` of a container.
    pub(super) fn padding(&self, element: &Element) -> String {
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
    pub(super) fn model(&self, element: &Element, controlled: &str, then: bool) -> Option<String> {
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
    pub(super) fn variant_name(&self, expr: &Expr) -> String {
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
    pub(super) fn shape(&self, element: &Element) -> String {
        format!(
            "b::Shape {{ capsule: {}, height: {} }}",
            element.keyword("shape") == Some("capsule"),
            some(self.num(element, "height")),
        )
    }

    /// The enum a selection value belongs to.
    pub(super) fn enum_name(expr: &Expr) -> String {
        match &expr.ty {
            Type::Enum { name } => string(name),
            _ => "\"\"".into(),
        }
    }

    /// The label getter of a variant: its declared label, or its name.
    pub(super) fn variant_label(&self, def: &EnumDef, index: usize) -> String {
        let variant = &def.variants[index];
        if variant.label.is_some() {
            format!("b::str(|_| b::label({}, {index}))", string(&def.name))
        } else {
            format!("b::str(|_| {}.to_string())", string(&variant.name))
        }
    }

    /// `b::shortcut(…)` for an element with `shortcut`, running `run`; a
    /// shortcut that follows state is `b::bound_shortcut(…)`.
    pub(super) fn shortcut(&self, out: &mut Out, element: &Element, run: &str) {
        match element.prop("shortcut") {
            Some(bern_ir::Prop::Keyword { value }) => out.line(format!(
                "b::shortcut(c, {}, b::act(|c| {run}));",
                string(value)
            )),
            Some(bern_ir::Prop::Expr { expr }) => out.line(format!(
                "b::bound_shortcut(c, b::str(|c| {}), b::act(|c| {run}));",
                self.display(expr)
            )),
            _ => {}
        }
    }

    /// The text a shortcut prop `name` follows, as a `Str`, when it is not
    /// a literal.
    pub(super) fn bound_keys(&self, element: &Element, name: &str) -> Option<String> {
        element
            .expr(name)
            .map(|expr| format!("b::str(|c| {})", self.display(expr)))
    }
}
