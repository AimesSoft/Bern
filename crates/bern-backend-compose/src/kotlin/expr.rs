//! Expressions, texts, bindings and actions, as Kotlin source.

use super::values::{session_var, value_literal};
use super::{Ctx, Generator};
use crate::names::{float, member, string};
use bern_ir::{Action, Binding, Element, Expr, ExprKind, MediaOp, Prop, Text, WindowOp};
use bern_protocol::Type;

impl Generator<'_> {
    fn binding_path(&self, mut ty: Type, path: &[usize]) -> String {
        let mut out = String::new();
        for index in path {
            let Type::Record { name } = &ty else { break };
            let def = self.types().record_def(name).expect("record");
            let field = &def.fields[*index];
            out.push('.');
            out.push_str(&member(&field.name));
            ty = field.ty.clone();
        }
        out
    }

    fn binding(&self, binding: &Binding) -> String {
        match binding {
            Binding::Window { field } => format!("bernWindow.{}", member(field)),
            Binding::Engine { field, path } => {
                let state = &self.app.protocol.state[*field];
                format!(
                    "BernEngine.{}{}",
                    member(&state.name),
                    self.binding_path(state.ty.clone(), path)
                )
            }
            Binding::Session {
                session,
                field,
                path,
            } => {
                let def = self.app.session(session).expect("session");
                let field = &def.fields[*field];
                format!(
                    "{}.{}{}",
                    session_var(session),
                    member(&field.name),
                    self.binding_path(field.ty.clone(), path)
                )
            }
            Binding::Local { name, path, root } => {
                format!("{}{}", member(name), self.binding_path(root.clone(), path))
            }
        }
    }

    pub(super) fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => value_literal(value, &expr.ty, self.types()),
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let Type::Enum { name } = &expr.ty else {
                    return "null".into();
                };
                let def = self.types().enum_def(name).expect("enum");
                let variant_def = &def.variants[*variant as usize];
                if variant_def.fields.is_empty() {
                    format!("{name}.{}", variant_def.name)
                } else {
                    let args: Vec<_> = fields
                        .iter()
                        .zip(&variant_def.fields)
                        .map(|(value, field)| {
                            format!("{} = {}", member(&field.name), self.expr(value))
                        })
                        .collect();
                    format!("{name}.{}({})", variant_def.name, args.join(", "))
                }
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => format!(
                "(if ({}) ({}) else ({}))",
                self.expr(cond),
                self.branch(then, &expr.ty),
                self.branch(otherwise, &expr.ty)
            ),
            ExprKind::Not { expr } => format!("!({})", self.expr(expr)),
            ExprKind::Neg { expr } => format!("-({})", self.expr(expr)),
            ExprKind::Binary { op, lhs, rhs } => {
                let op = match op {
                    bern_ir::BinaryOp::Eq => "==",
                    bern_ir::BinaryOp::Ne => "!=",
                    bern_ir::BinaryOp::Lt => "<",
                    bern_ir::BinaryOp::Le => "<=",
                    bern_ir::BinaryOp::Gt => ">",
                    bern_ir::BinaryOp::Ge => ">=",
                    bern_ir::BinaryOp::And => "&&",
                    bern_ir::BinaryOp::Or => "||",
                };
                format!("({}) {op} ({})", self.expr(lhs), self.expr(rhs))
            }
        }
    }

    /// A branch of a value `if`: a whole number where the `if` is a float
    /// becomes one.
    fn branch(&self, expr: &Expr, ty: &Type) -> String {
        if *ty == Type::Float && expr.ty == Type::Int {
            format!("({}).toDouble()", self.expr(expr))
        } else {
            self.expr(expr)
        }
    }

    /// Kotlin `String` expression for a text.
    pub(super) fn text(&self, text: &Text) -> String {
        match text {
            Text::Localized { text, args } => {
                if text.params.is_empty() {
                    format!("BernText.string({})", string(&text.key))
                } else {
                    let values: Vec<_> = text
                        .params
                        .iter()
                        .map(|param| {
                            args.iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| format!("BernText.text({})", self.expr(expr)))
                                .unwrap_or_else(|| "\"\"".into())
                        })
                        .collect();
                    format!(
                        "BernText.format({}, listOf({}))",
                        string(&text.key),
                        values.join(", ")
                    )
                }
            }
            Text::Dynamic { expr } => format!("BernText.text({})", self.expr(expr)),
        }
    }

    pub(super) fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text(text))
    }

    /// The control identity as a Kotlin string expression.
    pub(super) fn target(&self, element: &Element, ctx: &Ctx) -> String {
        let base = string(&format!(
            "{}.{}",
            ctx.component,
            element.id.as_deref().unwrap_or(&element.path)
        ));
        // The keys of the rows around it are added where it registers.
        base
    }

    /// The session field a control writes, readable and assignable.
    pub(super) fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        let session = self.app.session(&field.session)?;
        Some(format!(
            "{}.{}",
            session_var(&field.session),
            member(&session.fields[field.field].name)
        ))
    }

    pub(super) fn action(&self, action: &Action) -> String {
        match action {
            Action::Session { field, op } => {
                let session = self.app.session(&field.session).expect("session");
                let target = format!(
                    "{}.{}",
                    session_var(&field.session),
                    member(&session.fields[field.field].name)
                );
                match op {
                    bern_ir::SessionOp::Set { value } => format!("{target} = {}", self.expr(value)),
                    bern_ir::SessionOp::Toggle => format!("{target} = !{target}"),
                }
            }
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("bernWindow".into());
                format!(
                    "BernEngine.{}({})",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                )
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => format!("BernHost.open({})", string(window)),
                WindowOp::Close => format!("BernHost.close({})", string(window)),
            },
            Action::Media { player, op, args } => {
                let player = string(player);
                let arg = || args.first().map(|arg| self.expr(arg)).unwrap_or_default();
                match op {
                    MediaOp::Open => {
                        format!("BernMedia.open({player}, ({}).url, bernWindow.id)", arg())
                    }
                    MediaOp::Play => {
                        format!("BernMedia.command(BernMediaCommand.Play, {player}, bernWindow.id)")
                    }
                    MediaOp::Pause => format!(
                        "BernMedia.command(BernMediaCommand.Pause, {player}, bernWindow.id)"
                    ),
                    MediaOp::Seek => {
                        format!("BernMedia.seek({player}, {}, bernWindow.id)", arg())
                    }
                    MediaOp::SetVolume => {
                        format!("BernMedia.setVolume({player}, {}, bernWindow.id)", arg())
                    }
                    MediaOp::SetMuted => {
                        format!("BernMedia.setMuted({player}, {}, bernWindow.id)", arg())
                    }
                }
            }
        }
    }

    pub(super) fn icon(name: Option<&str>) -> String {
        match name {
            Some(name) => format!("BernIcons.get({})", string(name)),
            None => "null".into(),
        }
    }

    pub(super) fn enabled(&self, element: &Element) -> String {
        element
            .expr("enabled")
            .map(|expr| self.expr(expr))
            .unwrap_or_else(|| "true".into())
    }

    /// A length in dp: a literal, or an expression it follows, moving with
    /// the element's `animate` spring. Compose refuses negative padding and
    /// sizes, so a value below zero, or a spring overshooting zero, stops at
    /// zero.
    pub(super) fn length(&self, element: &Element, name: &str) -> Option<String> {
        let value = match element.prop(name)? {
            Prop::Number { value } => format!("{}.dp", float(*value)),
            Prop::Expr { expr } => format!("({}).toFloat().coerceAtLeast(0f).dp", self.expr(expr)),
            _ => return None,
        };
        Some(match element.keyword("animate") {
            Some(token) => format!(
                "bernLength({value}, {})",
                crate::style::motion(self.app, token)
            ),
            None => value,
        })
    }

    /// The file a control takes (`pick:`, or a `Drop`'s `value:`): the
    /// lambda that stores it and runs `action`, and the control actions
    /// that read it and take one from an external entry.
    pub(super) fn file_target(
        &self,
        element: &Element,
        name: &str,
        action: &str,
    ) -> Option<(String, String)> {
        let field = self.field(element, name)?;
        let optional = matches!(element.field(name)?.ty, Type::Optional { .. });
        let kinds = string(element.keyword("files").unwrap_or("any"));
        let give = format!("{{ url: String -> {field} = MediaSource(url); {action} }}");
        let actions = format!(
            "value = {{ {field}{}.url }}, setFile = {give}, fileKinds = {kinds}",
            if optional { "?" } else { "" }
        );
        Some((give, actions))
    }

    /// A control's action, and with `pick:` first the document picker:
    /// what pressing runs, and the extra control actions.
    pub(super) fn pressed(&self, element: &Element) -> (String, String) {
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_else(|| "Unit".into());
        match self.file_target(element, "pick", &action) {
            Some((give, actions)) => (
                format!(
                    "BernFiles.pick({}, {give})",
                    string(element.keyword("files").unwrap_or("any"))
                ),
                format!(", {actions}"),
            ),
            None => (action, String::new()),
        }
    }
}
