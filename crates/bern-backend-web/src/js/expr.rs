//! Expressions, bindings, actions and the props elements take.

use super::codecs::{is_scalar, literal};
use super::{Generator, Out, local, number, string};
use bern_ir::{Action, Binding, Element, Expr, ExprKind, MediaOp, Prop, Text, WindowOp};
use bern_protocol::{EnumDef, Type, Value};

impl Generator<'_> {
    fn path(&self, mut ty: Type, path: &[usize]) -> String {
        let mut out = String::new();
        for index in path {
            let Type::Record { name } = &ty else { break };
            let def = self.types().record_def(name).expect("record");
            let field = &def.fields[*index];
            out.push('.');
            out.push_str(&field.name);
            ty = field.ty.clone();
        }
        out
    }

    fn binding(&self, binding: &Binding) -> String {
        match binding {
            Binding::Engine { field, path } => {
                let state = &self.app.protocol.state[*field];
                format!(
                    "E.{}.get(){}",
                    state.name,
                    self.path(state.ty.clone(), path)
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
                    "ctx.s.{session}.{}.get(){}",
                    field.name,
                    self.path(field.ty.clone(), path)
                )
            }
            // `window.idle`, kept by the runtime per window.
            Binding::Window { field } => format!("ctx.win.{field}.get()"),
            Binding::Local { name, path, root } => {
                format!("{}(){}", local(name), self.path(root.clone(), path))
            }
        }
    }

    pub(super) fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value, &expr.ty, self.types()),
            // A named corner a build for a platform has not resolved:
            // its radius on every platform.
            ExprKind::Corner { radius, .. } => {
                literal(&Value::Float(*radius), &expr.ty, self.types())
            }
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let Type::Enum { name } = &expr.ty else {
                    return "null".into();
                };
                let def = self.types().enum_def(name).expect("enum");
                if def.is_unit_only() {
                    variant.to_string()
                } else {
                    let values: Vec<_> = fields.iter().map(|field| self.expr(field)).collect();
                    // Parenthesized, so an arrow function returns the object.
                    format!("({{ tag: {variant}, fields: [{}] }})", values.join(", "))
                }
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => format!(
                "(({}) ? ({}) : ({}))",
                self.expr(cond),
                self.expr(then),
                self.expr(otherwise)
            ),
            ExprKind::Not { expr } => format!("!({})", self.expr(expr)),
            ExprKind::Neg { expr } => format!("-({})", self.expr(expr)),
            ExprKind::Binary { op, lhs, rhs } => {
                let (left, right) = (self.expr(lhs), self.expr(rhs));
                let scalar = is_scalar(&lhs.ty, self.types());
                match op {
                    bern_ir::BinaryOp::Eq if !scalar => format!("B.eq({left}, {right})"),
                    bern_ir::BinaryOp::Ne if !scalar => format!("!B.eq({left}, {right})"),
                    _ => {
                        let op = match op {
                            bern_ir::BinaryOp::Eq => "===",
                            bern_ir::BinaryOp::Ne => "!==",
                            bern_ir::BinaryOp::Lt => "<",
                            bern_ir::BinaryOp::Le => "<=",
                            bern_ir::BinaryOp::Gt => ">",
                            bern_ir::BinaryOp::Ge => ">=",
                            bern_ir::BinaryOp::And => "&&",
                            bern_ir::BinaryOp::Or => "||",
                        };
                        format!("({left}) {op} ({right})")
                    }
                }
            }
        }
    }

    /// Display text of a value, by type.
    fn display(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Text => value,
            Type::Int => format!("B.int({value})"),
            Type::Float => format!("B.num({value})"),
            Type::Enum { name } => format!("L.{name}({value})"),
            _ => format!("String({value})"),
        }
    }

    pub(super) fn text(&self, text: &Text) -> String {
        match text {
            Text::Localized { text, args } => {
                if text.params.is_empty() {
                    format!("B.tr({})", string(&text.key))
                } else {
                    let values: Vec<_> = text
                        .params
                        .iter()
                        .map(|param| {
                            // A date stays a record for its pattern.
                            let value = args
                                .iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| match &expr.ty {
                                    Type::Record { name }
                                        if name == bern_protocol::format::CALENDAR_DATE =>
                                    {
                                        self.expr(expr)
                                    }
                                    _ => self.display(expr),
                                })
                                .unwrap_or_else(|| "\"\"".into());
                            format!("{}: {value}", string(param))
                        })
                        .collect();
                    format!("B.fmt({}, {{ {} }})", string(&text.key), values.join(", "))
                }
            }
            Text::Dynamic { expr } => self.display(expr),
        }
    }

    fn getter(&self, text: &Text) -> String {
        format!("() => {}", self.text(text))
    }

    pub(super) fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.getter(text))
    }

    pub(super) fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    pub(super) fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        let session = self.app.session(&field.session)?;
        Some(format!(
            "ctx.s.{}.{}",
            field.session, session.fields[field.field].name
        ))
    }

    /// Controls with an engine-owned value commit the proposed value through
    /// their command. Compare IR expressions, never generated source text: a
    /// string argument may contain the same spelling as the controlled field.
    pub(super) fn control_field(&self, element: &Element, controlled: &str) -> Option<String> {
        self.control_binding(element, controlled, false)
    }

    /// Popovers need command completion to distinguish stale engine frames
    /// from an external change while a native open/close is in flight.
    pub(super) fn acknowledged_control_field(
        &self,
        element: &Element,
        controlled: &str,
    ) -> Option<String> {
        self.control_binding(element, controlled, true)
    }

    fn control_binding(
        &self,
        element: &Element,
        controlled: &str,
        acknowledged: bool,
    ) -> Option<String> {
        if let Some(expr) = element.expr(controlled) {
            let value = self.expr(expr);
            let action = element.action.as_ref()?;
            let acknowledged = acknowledged && matches!(action, Action::Engine { .. });
            let commit = match action {
                Action::Engine { command, args } => {
                    let spec = &self.app.protocol.commands[*command];
                    let mut values = vec!["ctx".to_string()];
                    values.extend(args.iter().map(|arg| {
                        if arg != expr {
                            self.expr(arg)
                        } else if arg.ty == Type::Int {
                            // A whole number takes the slider's value
                            // rounded.
                            "BigInt(Math.round(Number(newValue)))".into()
                        } else {
                            "newValue".into()
                        }
                    }));
                    if acknowledged {
                        values.push("done".into());
                    }
                    format!(
                        "C.{}({})",
                        self.app.protocol.command_symbol(spec),
                        values.join(", ")
                    )
                }
                _ => self.action(action),
            };
            let setter = if acknowledged {
                "commit: (newValue, done)"
            } else {
                "set: (newValue)"
            };
            Some(format!(
                "{{ get: () => {value}, peek: () => {value}, {setter} => {{ {commit}; }} }}"
            ))
        } else {
            let field = self.field(element, "value")?;
            Some(match &element.action {
                Some(action) => format!(
                    "{{ get: () => {field}.get(), peek: () => {field}.peek(), set: (newValue) => {{ {field}.set(newValue); {}; }} }}",
                    self.action(action)
                ),
                None => field,
            })
        }
    }

    pub(super) fn value_prop(&self, element: &Element, name: &str) -> Option<String> {
        element
            .expr(name)
            .map(|expr| format!("() => {}", self.expr(expr)))
    }

    fn action(&self, action: &Action) -> String {
        match action {
            Action::Session { field, op } => {
                let session = self.app.session(&field.session).expect("session");
                let cell = format!(
                    "ctx.s.{}.{}",
                    field.session, session.fields[field.field].name
                );
                match op {
                    bern_ir::SessionOp::Set { value } => {
                        format!("{cell}.set({})", self.expr(value))
                    }
                    bern_ir::SessionOp::Toggle => format!("{cell}.set(!{cell}.peek())"),
                    bern_ir::SessionOp::PushStr { value } => {
                        format!("{cell}.set({cell}.peek() + {})", self.expr(value))
                    }
                    // The last character, not half of a surrogate pair.
                    bern_ir::SessionOp::Pop => {
                        format!("{cell}.set([...{cell}.peek()].slice(0, -1).join(\"\"))")
                    }
                }
            }
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values = vec!["ctx".to_string()];
                values.extend(args.iter().map(|arg| self.expr(arg)));
                format!(
                    "C.{}({})",
                    self.app.protocol.command_symbol(spec),
                    values.join(", ")
                )
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => format!("B.openWindow({})", string(window)),
                WindowOp::ToggleFullscreen => format!("B.toggleFullscreen({})", string(window)),
                WindowOp::Close => format!("B.closeWindow({})", string(window)),
            },
            Action::Media { player, op, args } => {
                let player = string(player);
                let arg = || args.first().map(|arg| self.expr(arg)).unwrap_or_default();
                match op {
                    MediaOp::Open => format!("B.mediaOpen(ctx, {player}, ({}).url)", arg()),
                    MediaOp::Play => format!("B.mediaCommand(ctx, \"play\", {player})"),
                    MediaOp::Pause => format!("B.mediaCommand(ctx, \"pause\", {player})"),
                    MediaOp::Seek => format!("B.mediaCommand(ctx, \"seek\", {player}, {})", arg()),
                    MediaOp::SetVolume => {
                        format!("B.mediaCommand(ctx, \"set_volume\", {player}, {})", arg())
                    }
                    MediaOp::SetMuted => {
                        format!("B.mediaCommand(ctx, \"set_muted\", {player}, {})", arg())
                    }
                }
            }
        }
    }

    /// For an element with `pick:`: the action that asks for a file first,
    /// and the `setFile` external entries use; otherwise the plain action.
    pub(super) fn file_actions(
        &self,
        element: &Element,
    ) -> (String, Option<String>, Option<String>) {
        let inner = element
            .action
            .as_ref()
            .map(|action| self.action(action))
            .unwrap_or_default();
        match self.field(element, "pick") {
            Some(field) => {
                let kinds = string(element.keyword("files").unwrap_or("any"));
                let give = format!("(url) => {{ {field}.set({{ url }}); {inner} }}");
                (
                    format!("() => F.pick({kinds}, {give})"),
                    Some(give),
                    Some(kinds),
                )
            }
            None => (self.action_fn(element), None, None),
        }
    }

    /// `F.shortcut(…)` running `run` while the element is mounted.
    pub(super) fn shortcut(&self, out: &mut Out, element: &Element, run: &str) {
        if let Some(shortcut) = self.shortcut_keys(element, "shortcut") {
            out.line(format!("F.shortcut({shortcut}, {run});"));
        }
    }

    /// The keys of the shortcut prop `name`: a literal, or a getter of the
    /// text the shortcut follows.
    pub(super) fn shortcut_keys(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            bern_ir::Prop::Keyword { value } => Some(string(value)),
            bern_ir::Prop::Expr { expr } => Some(format!("() => {}", self.expr(expr))),
            _ => None,
        }
    }

    pub(super) fn action_fn(&self, element: &Element) -> String {
        element
            .action
            .as_ref()
            .map(|action| match element.clearing() {
                // The draft empties and the keyboard goes back to the page.
                Some(clear) => format!(
                    "() => {{ {}; {}; document.activeElement?.blur?.(); }}",
                    self.action(action),
                    self.action(&clear)
                ),
                None => format!("() => {}", self.action(action)),
            })
            .unwrap_or_else(|| "null".into())
    }

    /// Props object entries, skipping absent ones.
    pub(super) fn props(entries: Vec<(&str, Option<String>)>) -> String {
        let entries: Vec<_> = entries
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| format!("{name}: {value}")))
            .collect();
        format!("{{ {} }}", entries.join(", "))
    }

    pub(super) fn keyword(element: &Element, name: &str) -> Option<String> {
        element.keyword(name).map(string)
    }

    /// Name of a selection value, for `ui.select` by variant name.
    pub(super) fn variant_name(&self, expr: &Expr) -> String {
        match (&expr.kind, &expr.ty) {
            (ExprKind::Variant { variant, .. }, Type::Enum { name }) => self
                .types()
                .enum_def(name)
                .map(|def| string(&def.variants[*variant as usize].name))
                .unwrap_or_else(|| "\"\"".into()),
            (_, Type::Enum { name }) => {
                let unit = self
                    .types()
                    .enum_def(name)
                    .is_some_and(EnumDef::is_unit_only);
                let value = self.expr(expr);
                if unit {
                    format!("N.{name}[{value}]")
                } else {
                    format!("N.{name}[({value}).tag]")
                }
            }
            _ => "\"\"".into(),
        }
    }

    /// A length: a number, or a getter for one bound to an expression.
    pub(super) fn length(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(number(*value)),
            Prop::Expr { expr } => Some(format!("() => Number({})", self.expr(expr))),
            _ => None,
        }
    }
}
