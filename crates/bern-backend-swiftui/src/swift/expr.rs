//! Expressions, texts, bindings and actions, as Swift source.

use super::values::{argument, session_var, value_literal};
use super::{Ctx, Generator};
use crate::names::{case, double, label, member, state_member, string, value_type};
use bern_ir::{Action, Binding, Element, Expr, ExprKind, MediaOp, Prop, SessionOp, Text, WindowOp};
use bern_protocol::{Type, Value};

impl Generator<'_> {
    /// A length as a Swift `Double`: the literal, or the expression it is
    /// bound to.
    pub(super) fn length(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(double(*value)),
            Prop::Expr { expr } => Some(format!("Double({})", self.expr(expr))),
            _ => None,
        }
    }

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
            Binding::Player { player, field } => format!(
                "BernPlayerReading(BernMedia.shared.players[{}]).{}",
                string(player),
                member(field)
            ),
            Binding::Engine { field, path } => {
                let state = &self.app.protocol.state[*field];
                format!(
                    "bernEngine.{}{}",
                    state_member(&state.name),
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
            // A named corner a build for a platform has not resolved:
            // its radius on every platform.
            ExprKind::Corner { radius, .. } => {
                value_literal(&Value::Float(*radius), &expr.ty, self.types())
            }
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let Type::Enum { name } = &expr.ty else {
                    return "nil".into();
                };
                let def = self.types().enum_def(name).expect("enum");
                let variant_def = &def.variants[*variant as usize];
                let name = value_type(name);
                if variant_def.fields.is_empty() {
                    format!("{name}.{}", case(&variant_def.name))
                } else {
                    let args: Vec<_> = fields
                        .iter()
                        .zip(&variant_def.fields)
                        .map(|(value, field)| {
                            format!("{}: {}", label(&field.name), self.expr(value))
                        })
                        .collect();
                    format!("{name}.{}({})", case(&variant_def.name), args.join(", "))
                }
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => format!(
                "(({}) ? ({}) : ({}))",
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
            format!("Double({})", self.expr(expr))
        } else {
            self.expr(expr)
        }
    }

    /// Swift `String` expression for a text.
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
                            let value = args
                                .iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| argument(&self.expr(expr), &expr.ty))
                                .unwrap_or_else(|| "BernText.arg(\"\")".into());
                            format!("{}: {value}", string(param))
                        })
                        .collect();
                    format!(
                        "BernText.format({}, [{}])",
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

    pub(super) fn target(&self, element: &Element, ctx: &Ctx) -> String {
        let base = format!(
            "{}.{}",
            ctx.component,
            element.id.as_deref().unwrap_or(&element.path)
        );
        // Rows add their keys at run time, through components too.
        string(&base)
    }

    pub(super) fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        let session = self.app.session(&field.session)?;
        Some(format!(
            "{}.{}",
            session_var(&field.session),
            member(&session.fields[field.field].name)
        ))
    }

    /// A controlled value commits through its action. References to the
    /// selected/checked expression in that action denote the new value.
    pub(super) fn control_value(
        &self,
        element: &Element,
        controlled: &str,
    ) -> (String, String, String) {
        if let Some(expr) = element.expr(controlled) {
            let value = self.expr(expr);
            let action = element.action.as_ref().expect("controlled action");
            let float_to_int = matches!(
                element.kind.as_str(),
                "Slider" | "ColorPicker" | "NumberInput" | "TimePicker"
            );
            // Only replace complete argument expressions, never text within
            // string literals or a neighbouring field's name.
            let commit = match action {
                Action::Engine { command, args } => {
                    let spec = &self.app.protocol.commands[*command];
                    let mut values: Vec<_> = args
                        .iter()
                        .map(|arg| {
                            if arg != expr {
                                self.expr(arg)
                            } else if arg.ty == Type::Int && float_to_int {
                                // A whole number takes the slider's or number input's value
                                // rounded.
                                "Int64(newValue.rounded())".into()
                            } else {
                                "newValue".into()
                            }
                        })
                        .collect();
                    values.push("from: bernWindow".into());
                    format!(
                        "bernEngine.{}({})",
                        member(&self.app.protocol.command_symbol(spec)),
                        values.join(", ")
                    )
                }
                Action::Session {
                    field,
                    op: bern_ir::SessionOp::Set { value },
                } if value == expr => {
                    let session = self.app.session(&field.session).expect("session");
                    let target = format!(
                        "{}.{}",
                        session_var(&field.session),
                        member(&session.fields[field.field].name)
                    );
                    let rhs = if field.ty == Type::Int && float_to_int {
                        "Int64(newValue.rounded())"
                    } else {
                        "newValue"
                    };
                    format!("{target} = {rhs}")
                }
                _ => self.action(action),
            };
            let setter = format!("{{ newValue in {commit} }}");
            (
                value.clone(),
                format!("SwiftUI.Binding(get: {{ {value} }}, set: {setter})"),
                setter,
            )
        } else {
            let field = self.field(element, "value").expect("session field");
            let action = element
                .action
                .as_ref()
                .map(|a| self.action(a))
                .unwrap_or_default();
            let setter = format!("{{ newValue in {field} = newValue; {action} }}");
            (
                field.clone(),
                format!("SwiftUI.Binding(get: {{ {field} }}, set: {setter})"),
                setter,
            )
        }
    }

    pub(super) fn binding_of(&self, field: &str) -> String {
        format!("SwiftUI.Binding(get: {{ {field} }}, set: {{ {field} = $0 }})")
    }

    /// The file(s) a control takes (`pick:`, or a `Drop`'s `value:`): the
    /// closure that stores a single URL (for drop/CLI), optional multi-URL
    /// closure (when bound to `Vec<MediaSource>`), and the control actions.
    pub(super) fn file_target(
        &self,
        element: &Element,
        name: &str,
        action: &str,
    ) -> Option<(String, String)> {
        let field = self.field(element, name)?;
        let ty = &element.field(name)?.ty;
        let optional = matches!(ty, Type::Optional { .. });
        let is_list = matches!(ty, Type::List { .. });
        let kinds = string(element.keyword("files").unwrap_or("any"));
        let source = value_type(bern_media::MEDIA_SOURCE);
        if is_list {
            let give =
                format!("{{ url in {field} = [{source}(url: url.absoluteString)]; {action} }}");
            let actions = format!(
                "value: {{ ({field}).map(\\.url).joined(separator: \"\\n\") }}, setFile: {give}, fileKinds: {kinds}"
            );
            Some((give, actions))
        } else {
            let give =
                format!("{{ url in {field} = {source}(url: url.absoluteString); {action} }}");
            let actions = format!(
                "value: {{ {field}{}.url }}, setFile: {give}, fileKinds: {kinds}",
                if optional { "?" } else { "" }
            );
            Some((give, actions))
        }
    }

    /// A control's action, and with `pick:`/`save:` first the file panel: what
    /// pressing runs, and the extra control actions.
    pub(super) fn pressed(&self, element: &Element) -> (String, String) {
        let action = element
            .action
            .as_ref()
            .map(|a| match element.clearing() {
                // The draft empties and the keyboard goes back to the window.
                Some(clear) => format!(
                    "{}; {}; bernEndEditing()",
                    self.action(a),
                    self.action(&clear)
                ),
                None => self.action(a),
            })
            .unwrap_or_default();
        // `paste:`: the clipboard's text goes into the draft first.
        if let Some(field) = self.field(element, "paste") {
            return (
                format!("BernClipboard.paste {{ text in {field} = text; {action} }}"),
                String::new(),
            );
        }
        if let Some(save_ref) = element.field("save") {
            let field = self.field(element, "save").expect("save field");
            let kinds = string(element.keyword("files").unwrap_or("any"));
            let suggested = self
                .text_prop(element, "suggested_name")
                .unwrap_or_else(|| "nil".into());
            let source = value_type(bern_media::MEDIA_SOURCE);
            let (give, value_expr) = match &save_ref.ty {
                Type::Text => (
                    format!(
                        "{{ url in {field} = url.isFileURL ? url.path : url.absoluteString; {action} }}"
                    ),
                    field,
                ),
                Type::Optional { .. } => (
                    format!("{{ url in {field} = {source}(url: url.absoluteString); {action} }}"),
                    format!("{field}?.url"),
                ),
                _ => (
                    format!("{{ url in {field} = {source}(url: url.absoluteString); {action} }}"),
                    format!("{field}.url"),
                ),
            };
            let actions =
                format!(", value: {{ {value_expr} }}, setFile: {give}, fileKinds: {kinds}");
            return (
                format!("BernFiles.save({kinds}, suggestedName: {suggested}, then: {give})"),
                actions,
            );
        }
        match self.file_target(element, "pick", &action) {
            Some((give, actions)) => {
                let kinds = string(element.keyword("files").unwrap_or("any"));
                if matches!(
                    element.field("pick").map(|f| &f.ty),
                    Some(Type::List { .. })
                ) {
                    let field = self.field(element, "pick").expect("pick field");
                    let source = value_type(bern_media::MEDIA_SOURCE);
                    let give_many = format!(
                        "{{ urls in {field} = urls.map {{ {source}(url: $0.absoluteString) }}; {action} }}"
                    );
                    (
                        format!("BernFiles.pickMany({kinds}, then: {give_many})"),
                        format!(", {actions}"),
                    )
                } else {
                    (
                        format!("BernFiles.pick({kinds}, then: {give})"),
                        format!(", {actions}"),
                    )
                }
            }
            None => (action, String::new()),
        }
    }

    pub(super) fn action(&self, action: &Action) -> String {
        match action {
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("from: bernWindow".into());
                format!(
                    "bernEngine.{}({})",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                )
            }
            Action::Session { field, op } => {
                let session = self.app.session(&field.session).expect("session");
                let target = format!(
                    "{}.{}",
                    session_var(&field.session),
                    member(&session.fields[field.field].name)
                );
                match op {
                    SessionOp::Set { value } => format!("{target} = {}", self.expr(value)),
                    SessionOp::Toggle => format!("{target}.toggle()"),
                    SessionOp::PushStr { value } => format!("{target} += {}", self.expr(value)),
                    SessionOp::Pop => format!("_ = {target}.popLast()"),
                }
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => {
                    format!("BernHost.shared.open({})", string(window))
                }
                WindowOp::ToggleFullscreen => {
                    format!("BernHost.shared.toggleFullscreen({})", string(window))
                }
                WindowOp::Close => format!("BernHost.shared.close({})", string(window)),
            },
            Action::Media { player, op, args } => {
                let player = string(player);
                let arg = || args.first().map(|arg| self.expr(arg)).unwrap_or_default();
                let media = "BernMedia.shared";
                match op {
                    MediaOp::Open => format!(
                        "{media}.open({player}, url: ({}).url, window: bernWindow.id)",
                        arg()
                    ),
                    MediaOp::Play => {
                        format!("{media}.command(.play, player: {player}, window: bernWindow.id)")
                    }
                    MediaOp::Pause => {
                        format!("{media}.command(.pause, player: {player}, window: bernWindow.id)")
                    }
                    MediaOp::Seek => format!(
                        "{media}.seek({player}, positionMs: {}, window: bernWindow.id)",
                        arg()
                    ),
                    MediaOp::SetVolume => format!(
                        "{media}.setVolume({player}, volume: {}, window: bernWindow.id)",
                        arg()
                    ),
                    MediaOp::SetMuted => format!(
                        "{media}.setMuted({player}, muted: {}, window: bernWindow.id)",
                        arg()
                    ),
                }
            }
        }
    }
}
