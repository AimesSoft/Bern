//! JavaScript generation from the IR.
//!
//! The generated module decodes and encodes the application's values,
//! mirrors engine state into reactive cells, and builds every component
//! from the runtime's standard components. Expressions become JavaScript
//! expressions evaluated inside reactive effects; locals (component
//! parameters, row variables, pattern bindings) are getter functions, so a
//! change re-runs only the bindings that read it.
//!
//! Value layout: bool, `BigInt` for Int, number for Float, string; unit-only
//! enums are the variant index; other enums are `{ tag, fields }`; records
//! are objects with their declared field names; `null` is `None`.

use crate::icons;
use bern_ir::{
    Action, Application, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind, IfLetNode,
    IfNode, MatchNode, MediaOp, Node, ParamKind, Presentation, Prop, Text, WindowDef,
    WindowInitial, WindowOp, WindowRole,
};
use bern_protocol::{EnumDef, RecordDef, Type, TypeTable, Value};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "js_tests.rs"]
mod tests;

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
                .push(format!("{}{}", "  ".repeat(self.indent), text));
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

/// A JavaScript string literal.
/// A color for the runtime: a role, or a fixed pair for light and dark.
pub fn color_ref(color: &bern_ir::style::ColorRef) -> String {
    match color {
        bern_ir::style::ColorRef::Role { role } => format!("{{ role: {} }}", string(role)),
        bern_ir::style::ColorRef::Fixed { light, dark } => format!(
            "{{ light: \"#{:02x}{:02x}{:02x}\", dark: \"#{:02x}{:02x}{:02x}\" }}",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

/// A role color for the runtime, from a keyword prop.
fn role(element: &Element, name: &str) -> Option<String> {
    element
        .keyword(name)
        .map(|role| format!("{{ role: {} }}", string(role)))
}

/// A JavaScript identifier for a style name.
pub(crate) fn member(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        out.push(if ch.is_ascii_alphanumeric() { ch } else { '_' });
    }
    out
}

pub fn string(value: &str) -> String {
    serde_json::to_string(value).expect("string")
}

pub(crate) fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value:?}")
    }
}

fn local(name: &str) -> String {
    format!("l_{name}")
}

/// Whether `===` compares values of this type correctly.
fn is_scalar(ty: &Type, types: &TypeTable) -> bool {
    match ty {
        Type::Unit | Type::Bool | Type::Int | Type::Float | Type::Text => true,
        Type::Enum { name } => types.enum_def(name).is_some_and(EnumDef::is_unit_only),
        _ => false,
    }
}

/// Decoding expression for a value of `ty` read from `r`.
fn decode(ty: &Type) -> String {
    match ty {
        Type::Unit => "null".into(),
        Type::Bool => "r.bool()".into(),
        Type::Int => "r.i64()".into(),
        Type::Float => "r.f64()".into(),
        Type::Text => "r.text()".into(),
        Type::Enum { name } | Type::Record { name } => format!("T.{name}.d(r)"),
        Type::Optional { inner } => format!("B.readOptional(r, (r) => {})", decode(inner)),
        Type::List { inner } => format!("B.readList(r, (r) => {})", decode(inner)),
    }
}

/// Statement encoding the value `v` of `ty` into `w`.
fn encode(ty: &Type, value: &str) -> String {
    match ty {
        Type::Unit => String::new(),
        Type::Bool => format!("w.u8({value} ? 1 : 0)"),
        Type::Int => format!("w.i64({value})"),
        Type::Float => format!("w.f64({value})"),
        Type::Text => format!("w.text({value})"),
        Type::Enum { name } | Type::Record { name } => format!("T.{name}.e(w, {value})"),
        Type::Optional { inner } => format!(
            "B.writeOptional(w, {value}, (w, v) => {{ {} }})",
            encode(inner, "v")
        ),
        Type::List { inner } => format!(
            "B.writeList(w, {value}, (w, v) => {{ {} }})",
            encode(inner, "v")
        ),
    }
}

fn codec(ty: &Type) -> String {
    format!(
        "[(r) => {}, (w, v) => {{ {} }}]",
        decode(ty),
        encode(ty, "v")
    )
}

/// JavaScript literal of a schema value.
fn literal(value: &Value, ty: &Type, types: &TypeTable) -> String {
    match (value, ty) {
        (Value::Unit, _) => "null".into(),
        (Value::Bool(value), _) => value.to_string(),
        (Value::Int(value), _) => format!("{value}n"),
        (Value::Float(value), _) => number(*value),
        (Value::Text(value), _) => string(value),
        (Value::Optional(None), _) => "null".into(),
        (Value::Optional(Some(value)), Type::Optional { inner }) => literal(value, inner, types),
        (Value::List(items), Type::List { inner }) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| literal(item, inner, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            let def = types.enum_def(name).expect("known enum");
            if def.is_unit_only() {
                variant.to_string()
            } else {
                let values: Vec<_> = fields
                    .iter()
                    .zip(&def.variants[*variant as usize].fields)
                    .map(|(value, field)| literal(value, &field.ty, types))
                    .collect();
                format!("({{ tag: {variant}, fields: [{}] }})", values.join(", "))
            }
        }
        (Value::Record(values), Type::Record { name }) => {
            let def = types.record_def(name).expect("known record");
            let fields: Vec<_> = values
                .iter()
                .zip(&def.fields)
                .map(|(value, field)| {
                    format!("{}: {}", field.name, literal(value, &field.ty, types))
                })
                .collect();
            format!("({{ {} }})", fields.join(", "))
        }
        _ => "null".into(),
    }
}

fn generate_enum(out: &mut Out, def: &EnumDef) {
    let name = &def.name;
    if def.is_unit_only() {
        out.open(format!("T.{name} = {{"));
        out.open("d(r) {");
        out.line("const v = r.u32();");
        out.line(format!(
            "if (v >= {}) throw new B.DecodeError({});",
            def.variants.len(),
            string(&format!("invalid {name}"))
        ));
        out.line("return v;");
        out.close("},");
        out.line("e(w, v) { w.u32(v); },");
        out.close("};");
    } else {
        out.open(format!("T.{name} = {{"));
        out.open("d(r) {");
        out.open("switch (r.u32()) {");
        for (index, variant) in def.variants.iter().enumerate() {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|field| decode(&field.ty))
                .collect();
            out.line(format!(
                "case {index}: return {{ tag: {index}, fields: [{}] }};",
                fields.join(", ")
            ));
        }
        out.line(format!(
            "default: throw new B.DecodeError({});",
            string(&format!("invalid {name}"))
        ));
        out.close("}");
        out.close("},");
        out.open("e(w, v) {");
        out.line("w.u32(v.tag);");
        out.open("switch (v.tag) {");
        for (index, variant) in def.variants.iter().enumerate() {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .enumerate()
                .map(|(field_index, field)| {
                    format!(
                        "{};",
                        encode(&field.ty, &format!("v.fields[{field_index}]"))
                    )
                })
                .collect();
            out.line(format!("case {index}: {} break;", fields.join(" ")));
        }
        out.close("}");
        out.close("},");
        out.close("};");
    }
    let names: Vec<_> = def
        .variants
        .iter()
        .map(|variant| string(&variant.name))
        .collect();
    out.line(format!("N.{name} = [{}];", names.join(", ")));
    if def.variants.iter().all(|variant| variant.label.is_some()) {
        let keys: Vec<_> = def
            .variants
            .iter()
            .map(|variant| string(&variant.label.as_ref().expect("label").key))
            .collect();
        if def
            .variants
            .iter()
            .all(|variant| variant.label.as_ref().is_some_and(|label| label.params.is_empty()))
        {
            let index = if def.is_unit_only() { "v" } else { "v.tag" };
            out.line(format!(
                "L.{name} = (v) => B.tr([{}][{index}]);",
                keys.join(", ")
            ));
        } else {
            // Labels that show the variant's fields fill them in.
            out.open(format!("L.{name} = (v) => {{"));
            out.open("switch (v.tag) {");
            for (index, variant) in def.variants.iter().enumerate() {
                let label = variant.label.as_ref().expect("label");
                if label.params.is_empty() {
                    out.line(format!("case {index}: return B.tr({});", string(&label.key)));
                    continue;
                }
                let args: Vec<_> = label
                    .params
                    .iter()
                    .filter_map(|param| {
                        let at = variant.fields.iter().position(|field| field.name == *param)?;
                        let value = format!("v.fields[{at}]");
                        let shown = match &variant.fields[at].ty {
                            Type::Int => format!("B.int({value})"),
                            Type::Float => format!("B.num({value})"),
                            Type::Enum { name } => format!("L.{name}({value})"),
                            _ => value,
                        };
                        Some(format!("{}: {shown}", string(param)))
                    })
                    .collect();
                out.line(format!(
                    "case {index}: return B.fmt({}, {{ {} }});",
                    string(&label.key),
                    args.join(", ")
                ));
            }
            out.close("}");
            out.line("return \"\";");
            out.close("};");
        }
    }
    out.line("");
}

fn generate_record(out: &mut Out, def: &RecordDef) {
    let name = &def.name;
    out.open(format!("T.{name} = {{"));
    let fields: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("{}: {}", field.name, decode(&field.ty)))
        .collect();
    out.line(format!("d(r) {{ return {{ {} }}; }},", fields.join(", ")));
    let encoders: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("{};", encode(&field.ty, &format!("v.{}", field.name))))
        .collect();
    out.line(format!("e(w, v) {{ {} }},", encoders.join(" ")));
    out.close("};");
    out.line("");
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether the nodes being written are a sidebar's content, where
    /// `Section` groups sidebar items.
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

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value, &expr.ty, self.types()),
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

    fn text(&self, text: &Text) -> String {
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

    fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.getter(text))
    }

    fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    fn field(&self, element: &Element, name: &str) -> Option<String> {
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
    fn control_field(&self, element: &Element, controlled: &str) -> Option<String> {
        if let Some(expr) = element.expr(controlled) {
            let value = self.expr(expr);
            let action = element.action.as_ref()?;
            let commit = match action {
                Action::Engine { command, args } => {
                    let spec = &self.app.protocol.commands[*command];
                    let mut values = vec!["ctx".to_string()];
                    values.extend(args.iter().map(|arg| {
                        if arg == expr {
                            "newValue".into()
                        } else {
                            self.expr(arg)
                        }
                    }));
                    format!(
                        "C.{}({})",
                        self.app.protocol.command_symbol(spec),
                        values.join(", ")
                    )
                }
                _ => self.action(action),
            };
            Some(format!(
                "{{ get: () => {value}, peek: () => {value}, set: (newValue) => {{ {commit}; }} }}"
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

    fn value_prop(&self, element: &Element, name: &str) -> Option<String> {
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
    fn file_actions(&self, element: &Element) -> (String, Option<String>, Option<String>) {
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
    fn shortcut(&self, out: &mut Out, element: &Element, run: &str) {
        if let Some(shortcut) = element.keyword("shortcut") {
            out.line(format!("F.shortcut({}, {run});", string(shortcut)));
        }
    }

    fn action_fn(&self, element: &Element) -> String {
        element
            .action
            .as_ref()
            .map(|action| format!("() => {}", self.action(action)))
            .unwrap_or_else(|| "null".into())
    }

    /// Props object entries, skipping absent ones.
    fn props(entries: Vec<(&str, Option<String>)>) -> String {
        let entries: Vec<_> = entries
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| format!("{name}: {value}")))
            .collect();
        format!("{{ {} }}", entries.join(", "))
    }

    fn keyword(element: &Element, name: &str) -> Option<String> {
        element.keyword(name).map(string)
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            self.node(out, node);
        }
    }

    /// A children callback: `(p, ctx) => { … }`.
    fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        self.children_with(out, prefix, &[], nodes, suffix);
    }

    /// A children callback whose first lines act on the container `p`.
    fn children_with(
        &self,
        out: &mut Out,
        prefix: &str,
        first: &[String],
        nodes: &[Node],
        suffix: &str,
    ) {
        out.open(format!("{prefix}(p, ctx) => {{"));
        for line in first {
            out.line(line);
        }
        self.nodes(out, nodes);
        out.close(format!("}}{suffix}"));
    }

    /// A container's children callback; with `stagger`, its children take
    /// turns appearing.
    fn container_children(
        &self,
        out: &mut Out,
        element: &Element,
        prefix: &str,
        first: &[String],
        suffix: &str,
    ) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!("{prefix}(p, ctx) => {{"));
                for line in first {
                    out.line(line);
                }
                out.open(format!("F.staggered({}, () => {{", number(step)));
                self.nodes(out, &element.children);
                out.close("});");
                out.close(format!("}}{suffix}"));
            }
            None => self.children_with(out, prefix, first, &element.children, suffix),
        }
    }

    /// Where an element sits on its `Rail`, for `F.Place`.
    fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| match element.prop(name)? {
            Prop::Expr { expr } => Some(format!("() => Number({})", self.expr(expr))),
            Prop::Number { value } => Some(number(*value)),
            _ => None,
        };
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        (at.is_some() || from.is_some()).then(|| {
            Self::props(vec![
                ("at", at),
                ("from", from),
                ("to", to),
                ("motion", self.animate(element)),
            ])
        })
    }

    /// The motion and appearance props of an element, for `F.Decorate`.
    fn decoration(&self, element: &Element) -> Option<String> {
        let mut entries: Vec<(&str, Option<String>)> = Vec::new();
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("X.{}", member(transition)))
        };
        entries.push(("appear", transition("appear")));
        entries.push(("appearDelay", element.number("appear_delay").map(number)));
        entries.push(("disappear", transition("disappear")));
        if let Some(hero) = element.keyword("hero") {
            let key = match element.expr("hero_key") {
                Some(key) => format!(
                    "() => {} + \"-\" + String({})",
                    string(hero),
                    self.expr(key)
                ),
                None => format!("() => {}", string(hero)),
            };
            entries.push(("hero", Some(key)));
            entries.push(("heroMotion", Some(self.motion("hero"))));
        }
        if element.keyword("reveal") == Some("hover") {
            entries.push(("reveal", Some("true".into())));
        }
        entries.push(("appearance", Self::keyword(element, "appearance")));
        if let Some(when) = element.expr("nudge") {
            entries.push(("nudge", Some(format!("() => ({})", self.expr(when)))));
            entries.push(("nudgeMotion", Some(self.motion("jelly"))));
        }
        if let Some(key) = self.value_prop(element, "nudge_key") {
            entries.push(("nudgeKey", Some(key)));
            entries.push(("nudgeMotion", Some(self.motion("jelly"))));
        }
        entries.push(("nudgeScale", element.number("nudge_scale").map(number)));
        entries.push(("reduceMotion", self.value_prop(element, "reduce_motion")));
        entries.push(("scrollScale", element.number("scroll_scale").map(number)));
        entries.push((
            "scrollOpacity",
            element.number("scroll_opacity").map(number),
        ));
        entries.push((
            "scrollOriginX",
            element.number("scroll_origin_x").map(number),
        ));
        entries.push((
            "scrollOriginY",
            element.number("scroll_origin_y").map(number),
        ));
        entries
            .iter()
            .any(|(_, value)| value.is_some())
            .then(|| Self::props(entries))
    }

    /// Lines that act on a container `p`: its `reflow`, and the lengths
    /// bound to expressions, or given in sizes the runtime does not take
    /// as props, moving with `animate`.
    fn layout_lines(&self, element: &Element, lengths: &[(&str, &str)]) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(token) = element.keyword("reflow") {
            lines.push(format!("F.reflow(p, {});", self.motion(token)));
        }
        let animate = self.animate(element).unwrap_or_else(|| "null".into());
        for (name, property) in lengths {
            if let Some(value) = self.length(element, name) {
                lines.push(format!(
                    "F.bindLength(p, {}, {value}, {animate});",
                    string(property)
                ));
            }
        }
        lines
    }

    fn node(&self, out: &mut Out, node: &Node) {
        match node {
            Node::Element(element) => {
                let placement = self.placement(element);
                if let Some(spec) = &placement {
                    out.open(format!("F.Place(p, ctx, {spec}, (p, ctx) => {{"));
                }
                match self.decoration(element) {
                    Some(spec) => {
                        out.open(format!("F.Decorate(p, ctx, {spec}, (p, ctx) => {{"));
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
                                ComponentArg::Value { expr } => {
                                    Some(format!("{}: () => {}", param.name, self.expr(expr)))
                                }
                                _ => None,
                            }),
                        _ => None,
                    })
                    .collect();
                out.line(format!(
                    "c_{}(p, B.child(ctx, {}), {{ {} }});",
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
            branch.push_str(&format!("({}) ? {index} : ", self.expr(condition)));
        }
        if node.otherwise.is_empty() {
            branch.push_str("-1");
        } else {
            branch.push_str(&node.branches.len().to_string());
        }
        out.open(format!("B.When(p, ctx, () => {branch}, ["));
        for (_, body) in &node.branches {
            self.children(out, "", body, ",");
        }
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, ",");
        }
        out.close("]);");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode) {
        let var = local(&node.var);
        out.open("{");
        out.line(format!("const {var} = () => {};", self.expr(&node.value)));
        out.open(format!(
            "B.When(p, ctx, () => ({var}() !== null ? 0 : {}), [",
            if node.otherwise.is_empty() { "-1" } else { "1" }
        ));
        self.children(out, "", &node.then, ",");
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, ",");
        }
        out.close("]);");
        out.close("}");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode) {
        let def = self.types().enum_def(&node.enum_name).expect("enum");
        let unit = def.is_unit_only();
        out.open("{");
        out.line(format!("const m = () => {};", self.expr(&node.value)));
        let tag = if unit { "m()" } else { "m().tag" };
        let explicit: Vec<String> = node
            .arms
            .iter()
            .filter_map(|arm| arm.variant.map(|variant| variant.to_string()))
            .collect();
        let has_default = node.arms.iter().any(|arm| arm.variant.is_none());
        let branch = if has_default {
            format!(
                "() => {{ const t = {tag}; return [{}].includes(t) ? t : \"_\"; }}",
                explicit.join(", ")
            )
        } else {
            format!("() => {tag}")
        };
        out.open(format!("B.When(p, ctx, {branch}, {{"));
        for arm in &node.arms {
            let key = arm
                .variant
                .map(|variant| variant.to_string())
                .unwrap_or_else(|| "\"_\"".into());
            out.open(format!("[{key}]: (p, ctx) => {{"));
            for binding in &arm.bindings {
                out.line(format!(
                    "const {} = () => m().fields[{}];",
                    local(&binding.name),
                    binding.field
                ));
            }
            self.nodes(out, &arm.body);
            out.close("},");
        }
        out.close("});");
        out.close("}");
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
                let def = self.types().enum_def(&node.enum_name)?;
                let tag = if def.is_unit_only() { "m" } else { "m.tag" };
                let arms = node
                    .arms
                    .iter()
                    .map(|arm| {
                        let label = self.row_label(&arm.body)?;
                        Some(match arm.variant {
                            Some(variant) => format!("case {variant}: return {label};"),
                            None => format!("default: return {label};"),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "(() => {{ const m = {}; switch ({tag}) {{ {} }} return \"\"; }})()",
                    self.expr(&node.value),
                    arms.join(" ")
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// Name of a selection value, for `ui.select` by variant name.
    fn variant_name(&self, expr: &Expr) -> String {
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

    fn selected_label(&self, selection: &str, options: &[(String, String)]) -> String {
        let cases: Vec<_> = options
            .iter()
            .map(|(value, label)| format!("if (B.eq(v, {value})) return {label}; "))
            .collect();
        format!(
            "() => {{ const v = {selection}.get(); {}return \"\"; }}",
            cases.concat()
        )
    }

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => out.line(format!(
                "{}(p, ctx, {});",
                if element.prop("mark").is_some() {
                    "F.MarkedText"
                } else {
                    "B.Text"
                },
                Self::props(vec![
                    ("content", self.text_prop(element, "content")),
                    ("style", Self::keyword(element, "style")),
                    ("emphasis", Self::keyword(element, "emphasis")),
                    ("mark", Self::keyword(element, "mark")),
                    ("lines", element.number("lines").map(number)),
                    ("fontSize", self.length(element, "font_size")),
                    ("lineHeight", self.length(element, "line_height")),
                ])
            )),
            "Canvas" => self.canvas(out, element),
            "Label" => out.line(format!(
                "B.Label(p, ctx, {});",
                Self::props(vec![
                    ("title", self.text_prop(element, "title")),
                    ("icon", element.icon("icon").map(string)),
                    ("style", Self::keyword(element, "style")),
                    ("fontSize", element.number("font_size").map(number)),
                    ("display", Self::keyword(element, "display")),
                ])
            )),
            "Button" | "IconButton" => {
                let enabled = element
                    .expr("enabled")
                    .map(|expr| format!("() => {}", self.expr(expr)));
                let (action, set_file, kinds) = self.file_actions(element);
                let style =
                    crate::style::reference(self.app, &element.kind, element.keyword("variant"));
                if element.kind == "Button" && !element.children.is_empty() {
                    let props = Self::props(vec![
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("enabled", enabled),
                        ("style", style),
                        ("shape", Self::keyword(element, "shape")),
                        ("height", self.length(element, "height")),
                        ("setFile", set_file),
                        ("fileKinds", kinds),
                    ]);
                    self.children(
                        out,
                        &format!("F.ContentButton(p, ctx, {props}, {action}, "),
                        &element.children,
                        ");",
                    );
                } else {
                    out.line(format!(
                        "B.{}(p, ctx, {}, {action});",
                        element.kind,
                        Self::props(vec![
                            ("id", Some(self.id(element))),
                            ("label", self.text_prop(element, "label")),
                            ("icon", element.icon("icon").map(string)),
                            ("role", Self::keyword(element, "role")),
                            ("prominence", Self::keyword(element, "prominence")),
                            ("fontSize", element.number("font_size").map(number)),
                            ("diameter", (element.kind == "IconButton").then(|| self.length(element, "height")).flatten()),
                            ("enabled", enabled),
                            ("style", style),
                            ("shape", Self::keyword(element, "shape")),
                            ("height", self.length(element, "height")),
                            ("fill", (element.keyword("width") == Some("fill")).then(|| "true".into())),
                            ("setFile", set_file),
                            ("fileKinds", kinds),
                        ]),
                    ));
                }
                self.shortcut(out, element, &action);
            }
            "TextInput" => out.line(format!(
                "B.TextInput(p, ctx, {}, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    (
                        "labelHidden",
                        Some((element.keyword("label_hidden") == Some("true")).to_string())
                    ),
                    ("field", self.field(element, "value")),
                    ("prompt", self.text_prop(element, "prompt")),
                    ("kind", Self::keyword(element, "kind")),
                ]),
                self.action_fn(element)
            )),
            "Search" => {
                let palette = element.keyword("style") == Some("palette");
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("field", self.field(element, "value")),
                    ("prompt", self.text_prop(element, "prompt")),
                    ("panelPrompt", self.text_prop(element, "panel_prompt")),
                    ("width", element.number("width").map(number)),
                    ("height", element.number("height").map(number)),
                    ("fontSize", element.number("font_size").map(number)),
                ]);
                let submit = self.action_fn(element);
                let function = if palette {
                    "B.SearchPalette"
                } else {
                    "B.Search"
                };
                if element.children.is_empty() {
                    out.line(format!("{function}(p, ctx, {props}, {submit});"));
                } else {
                    self.children(
                        out,
                        &format!("{function}(p, ctx, {props}, {submit}, "),
                        &element.children,
                        ");",
                    );
                }
                self.shortcut(
                    out,
                    element,
                    &format!(
                        "() => F.{}(ctx, {})",
                        if palette {
                            "activateTarget"
                        } else {
                            "focusTarget"
                        },
                        self.id(element)
                    ),
                );
            }
            "SearchItem" => {
                let (action, set_file, kinds) = self.file_actions(element);
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("icon", element.icon("icon").map(string)),
                    ("detail", self.text_prop(element, "detail")),
                    ("hint", self.text_prop(element, "hint")),
                    ("navigation", self.value_prop(element, "navigation")),
                    ("setFile", set_file),
                    ("fileKinds", kinds),
                ]);
                if element.children.is_empty() {
                    out.line(format!("B.SearchItem(p, ctx, {props}, {action});"));
                } else {
                    self.children(
                        out,
                        &format!("B.SearchItem(p, ctx, {props}, {action}, "),
                        &element.children,
                        ");",
                    );
                }
            }
            "Switch" => {
                let field = self.control_field(element, "checked");
                out.line(format!(
                    "F.Switch(p, ctx, {});",
                    Self::props(vec![
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("field", field.clone()),
                        ("style", Self::keyword(element, "style")),
                        ("shape", Self::keyword(element, "shape")),
                        ("height", self.length(element, "height")),
                        ("icon", element.icon("icon").map(string)),
                        (
                            "labelHidden",
                            Some((element.keyword("label_hidden") == Some("true")).to_string())
                        ),
                    ])
                ));
                if let Some(field) = field {
                    self.shortcut(
                        out,
                        element,
                        &format!("() => {{ const field = {field}; field.set(!field.peek()); }}"),
                    );
                }
            }
            "Slider" => out.line(format!(
                "B.Slider(p, ctx, {}, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    (
                        "field",
                        if element.expr("current").is_some() {
                            self.control_field(element, "current")
                        } else {
                            self.field(element, "value")
                        }
                    ),
                    (
                        "controlled",
                        Some(element.expr("current").is_some().to_string())
                    ),
                    ("min", element.number("min").map(number)),
                    ("max", element.number("max").map(number)),
                    ("step", element.number("step").map(number)),
                    ("minIcon", element.icon("min_icon").map(string)),
                    ("maxIcon", element.icon("max_icon").map(string)),
                    (
                        "labelHidden",
                        Some((element.keyword("label_hidden") == Some("true")).to_string())
                    ),
                ]),
                if element.expr("current").is_some() {
                    "null".into()
                } else {
                    self.action_fn(element)
                }
            )),
            "Picker" => {
                let ty = element
                    .expr("selected")
                    .map(|expr| &expr.ty)
                    .or_else(|| element.field("value").map(|field| &field.ty));
                let Some(Type::Enum { name }) = ty else {
                    return;
                };
                let def = self.types().enum_def(name).expect("enum");
                let options: Vec<_> = if element.children.is_empty() {
                    def.variants
                        .iter()
                        .enumerate()
                        .map(|(index, variant)| {
                            format!(
                                "{{ value: {index}, name: {}, label: () => L.{name}({index}) }}",
                                string(&variant.name)
                            )
                        })
                        .collect()
                } else {
                    element
                        .children
                        .iter()
                        .filter_map(|node| {
                            let Node::Element(option) = node else {
                                return None;
                            };
                            let value = option.expr("value")?;
                            Some(format!(
                                "{{ value: {}, name: {}, label: {} }}",
                                self.expr(value),
                                self.variant_name(value),
                                self.text_prop(option, "label")?
                            ))
                        })
                        .collect()
                };
                out.line(format!(
                    "B.Picker(p, ctx, {});",
                    Self::props(vec![
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("field", self.control_field(element, "selected")),
                        ("options", Some(format!("[{}]", options.join(", ")))),
                        ("style", Self::keyword(element, "style")),
                        ("shape", Self::keyword(element, "shape")),
                        ("height", self.length(element, "height")),
                        (
                            "labelHidden",
                            Some((element.keyword("label_hidden") == Some("true")).to_string())
                        ),
                    ])
                ));
            }
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => {
                let props = Self::props(vec![
                    ("title", self.text_prop(element, "title")),
                    ("gap", element.number("gap").map(number)),
                    ("rowHeight", element.number("row_height").map(number)),
                    ("paddingX", element.number("padding_x").map(number)),
                    ("paddingY", element.number("padding_y").map(number)),
                ]);
                // A form style's background is its rows'.
                let first: Vec<String> =
                    crate::style::reference(self.app, "Form", element.keyword("variant"))
                        .map(|style| format!("F.formRows(p, {style});"))
                        .into_iter()
                        .collect();
                self.children_with(
                    out,
                    &format!("B.Form(p, ctx, {props}, "),
                    &first,
                    &element.children,
                    ");",
                );
            }
            "Section" if self.in_sidebar.get() => {
                let props = Self::props(vec![("title", self.text_prop(element, "title"))]);
                self.children(
                    out,
                    &format!("F.SidebarSection(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "SidebarItem" => {
                let Some(value) = element.expr("value") else {
                    return;
                };
                let label = element.text("label").map(|text| self.text(text));
                out.line(format!(
                    "F.SidebarItem(p, ctx, {});",
                    Self::props(vec![
                        ("value", Some(self.expr(value))),
                        ("name", Some(self.variant_name(value))),
                        ("label", label.map(|label| format!("() => {label}"))),
                        ("icon", element.icon("icon").map(string)),
                        ("badge", self.length(element, "badge")),
                        (
                            "style",
                            crate::style::reference(
                                self.app,
                                "SidebarItem",
                                element.keyword("variant")
                            )
                        ),
                    ])
                ));
            }
            "Section" => {
                let props = Self::props(vec![
                    ("title", self.text_prop(element, "title")),
                    ("footer", self.text_prop(element, "footer")),
                    (
                        "plain",
                        (element.keyword("style") == Some("plain")).then(|| "true".into()),
                    ),
                    ("paddingX", element.number("padding_x").map(number)),
                    ("paddingY", element.number("padding_y").map(number)),
                ]);
                self.children(
                    out,
                    &format!("B.Section(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "Column" | "Row" => {
                let props = Self::props(vec![
                    (
                        "axis",
                        Some(string(if element.kind == "Column" {
                            "column"
                        } else {
                            "row"
                        })),
                    ),
                    ("align", Self::keyword(element, "align")),
                ]);
                let first = self.layout_lines(
                    element,
                    &[
                        ("gap", "gap"),
                        ("padding", "padding"),
                        ("padding_x", "paddingInline"),
                        ("padding_y", "paddingBlock"),
                        ("width", "width"),
                        ("height", "height"),
                    ],
                );
                self.container_children(
                    out,
                    element,
                    &format!("B.Stack(p, ctx, {props}, "),
                    &first,
                    ");",
                );
            }
            "Grid" => {
                let props = Self::props(vec![(
                    "minWidth",
                    Some(
                        element
                            .number("min_width")
                            .map(number)
                            .unwrap_or("0".into()),
                    ),
                )]);
                let mut first = self.layout_lines(
                    element,
                    &[
                        ("gap", "gap"),
                        ("row_gap", "rowGap"),
                        ("padding", "padding"),
                        ("padding_x", "paddingInline"),
                        ("padding_y", "paddingBlock"),
                    ],
                );
                if let Some(Prop::Expr { .. }) = element.prop("min_width") {
                    first.push(format!(
                        "F.bindGridMinWidth(p, {}, {});",
                        self.length(element, "min_width").expect("min width"),
                        self.animate(element).unwrap_or_else(|| "null".into())
                    ));
                }
                if let Some(ratio) = element.number("cell_ratio") {
                    first.push(format!(
                        "F.bindGridCells(p, {}, {}, {});",
                        number(ratio),
                        self.length(element, "cell_extra").unwrap_or_else(|| "0".into()),
                        self.animate(element).unwrap_or_else(|| "null".into())
                    ));
                }
                self.container_children(
                    out,
                    element,
                    &format!("B.Grid(p, ctx, {props}, "),
                    &first,
                    ");",
                );
            }
            "Stack" => {
                let props = Self::props(vec![
                    ("align", Self::keyword(element, "align")),
                    ("enabled", self.value_prop(element, "enabled")),
                    (
                        "style",
                        crate::style::reference(self.app, "Stack", element.keyword("variant")),
                    ),
                ]);
                let first = self.layout_lines(element, &[("width", "width"), ("height", "height")]);
                self.container_children(
                    out,
                    element,
                    &format!("B.Layers(p, ctx, {props}, "),
                    &first,
                    ");",
                );
            }
            "Marquee" => {
                let props = Self::props(vec![
                    ("duration", self.length(element, "duration")),
                    ("phase", element.number("phase").map(number)),
                    ("paused", self.value_prop(element, "paused")),
                    ("opacity", self.value_prop(element, "opacity")),
                ]);
                self.children(
                    out,
                    &format!("B.Marquee(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "ScaleBox" | "Positioned" => {
                let props = Self::props(vec![
                    ("width", self.length(element, "width")),
                    ("height", self.length(element, "height")),
                ]);
                let first = if element.kind == "Positioned" {
                    {
                        let mut lines =
                            self.layout_lines(element, &[("width", "width"), ("height", "height")]);
                        for (name, property) in [("x", "left"), ("y", "top")] {
                            lines.push(format!(
                                "F.bindPosition(p, \"{property}\", {}, {});",
                                self.length(element, name).unwrap(),
                                self.animate(element).unwrap_or_else(|| "null".into())
                            ));
                        }
                        lines
                    }
                } else {
                    vec![]
                };
                self.container_children(
                    out,
                    element,
                    &format!("B.{}(p, ctx, {props}, ", element.kind),
                    &first,
                    ");",
                );
            }
            "Modal" | "Flow" | "Accordion" => {
                let props = Self::props(vec![
                    ("width", self.length(element, "width")),
                    ("height", self.length(element, "height")),
                    ("gap", self.length(element, "gap")),
                    ("count", self.length(element, "count")),
                    ("selected", self.length(element, "selected")),
                    ("collapsedWidth", self.length(element, "collapsed_width")),
                    ("corner", self.length(element, "corner")),
                    ("animate", self.animate(element)),
                    ("cover", Self::keyword(element, "cover")),
                ]);
                self.children(
                    out,
                    &format!("B.{}(p, ctx, {props}, ", element.kind),
                    &element.children,
                    ");",
                );
            }
            "Timer" => out.line(format!(
                "B.Timer(p, ctx, {}, {});",
                Self::props(vec![
                    ("interval", element.number("interval").map(number)),
                    ("enabled", self.value_prop(element, "enabled")),
                ]),
                self.action_fn(element)
            )),
            "Svg" => out.line(format!(
                "F.Svg(p, ctx, {});",
                Self::props(vec![
                    ("source", element.resource("source").map(string)),
                    ("label", self.text_prop(element, "label")),
                    ("width", self.length(element, "width")),
                    ("height", self.length(element, "height")),
                    ("fit", Self::keyword(element, "fit")),
                    ("playing", self.value_prop(element, "playing")),
                    ("speed", self.value_prop(element, "speed")),
                    ("restart", self.value_prop(element, "restart")),
                    (
                        "interactive",
                        element.keyword("interactive").map(str::to_owned)
                    ),
                ])
            )),
            "Image" => {
                let url = element.expr("source").map(|expr| {
                    if matches!(expr.ty, Type::Optional { .. }) {
                        format!("() => ({})?.url ?? null", self.expr(expr))
                    } else {
                        format!("() => ({}).url", self.expr(expr))
                    }
                });
                out.line(format!(
                    "F.Image(p, ctx, {});",
                    Self::props(vec![
                        ("source", element.resource("source").map(string)),
                        ("url", url),
                        ("label", self.text_prop(element, "label")),
                        ("width", self.length(element, "width")),
                        ("height", self.length(element, "height")),
                        ("fit", Self::keyword(element, "fit")),
                        ("animate", self.animate(element)),
                        ("corner", self.length(element, "corner")),
                        ("rendering", Self::keyword(element, "rendering")),
                        ("aspect", element.number("aspect").map(number)),
                    ])
                ));
            }
            "Progress" => out.line(format!(
                "B.Progress(p, ctx, {});",
                Self::props(vec![
                    (
                        "value",
                        element
                            .expr("value")
                            .map(|value| format!("() => {}", self.expr(value)))
                    ),
                    ("total", element.number("total").map(number)),
                    ("label", self.text_prop(element, "label")),
                ])
            )),
            "Spacer" => out.line("B.Spacer(p);"),
            "Divider" => out.line("B.Divider(p);"),
            "ScrollView" => {
                let position = self.field(element, "position");
                let snapping = element.keyword("snap").is_some() || position.is_some();
                let props = Self::props(vec![
                    ("axis", Self::keyword(element, "axis")),
                    ("snap", Self::keyword(element, "snap")),
                    ("position", position),
                    ("arrival", Self::keyword(element, "arrival")),
                    ("nudgeMotion", snapping.then(|| self.motion("jelly"))),
                    (
                        "hideIndicators",
                        (element.keyword("indicators") == Some("hidden")).then(|| "true".into()),
                    ),
                    ("openAt", element.number("open_at").map(number)),
                ]);
                let first = self.layout_lines(element, &[]);
                self.children_with(
                    out,
                    &format!(
                        "{}(p, ctx, {props}, ",
                        if snapping {
                            "F.ScrollView"
                        } else {
                            "B.ScrollView"
                        }
                    ),
                    &first,
                    &element.children,
                    ");",
                );
            }
            "Rail" => {
                let props = Self::props(vec![
                    ("axis", Self::keyword(element, "axis")),
                    ("anchor", Self::keyword(element, "anchor")),
                    ("overhang", self.length(element, "overhang")),
                ]);
                self.children(
                    out,
                    &format!("F.Rail(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "PopoverButton" => {
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("icon", element.icon("icon").map(string)),
                    ("presented", self.field(element, "presented")),
                ]);
                self.children(
                    out,
                    &format!("F.PopoverButton(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
                self.shortcut(
                    out,
                    element,
                    &format!("() => F.activateTarget(ctx, {})", self.id(element)),
                );
            }
            "Toast" => {
                let content = self.text_prop(element, "content").unwrap_or_default();
                let key = element
                    .expr("key")
                    .map(|key| format!("() => {}", self.expr(key)))
                    .unwrap_or_else(|| content.clone());
                out.line(format!(
                    "F.Toast(p, ctx, {});",
                    Self::props(vec![
                        ("content", Some(content)),
                        ("icon", element.icon("icon").map(string)),
                        ("key", Some(key)),
                        ("duration", element.number("duration").map(number)),
                        ("motion", Some(self.motion("bouncy"))),
                    ])
                ));
            }
            "Inspector" => {
                let props = Self::props(vec![
                    ("presented", self.field(element, "presented")),
                    ("width", element.number("width").map(number)),
                    ("motion", Some(self.motion("bouncy"))),
                ]);
                self.children(
                    out,
                    &format!("F.Inspector(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "ControlGroup" => {
                let props = Self::props(vec![
                    ("label", self.text_prop(element, "label")),
                    ("height", self.length(element, "height")),
                ]);
                self.children(
                    out,
                    &format!("F.ControlGroup(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "SplitButton" => {
                let field = self.field(element, "value");
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("shape", Self::keyword(element, "shape")),
                    ("height", self.length(element, "height")),
                    ("label", self.text_prop(element, "label")),
                    ("icon", element.icon("icon").map(string)),
                    ("field", field.clone()),
                    ("more", self.text_prop(element, "more_label")),
                ]);
                self.children(
                    out,
                    &format!("F.SplitButton(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
                if let Some(field) = field {
                    self.shortcut(out, element, &format!("() => {field}.set(!{field}.peek())"));
                }
            }
            "Drop" => {
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("kinds", Self::keyword(element, "files")),
                    ("over", self.field(element, "over")),
                    (
                        "opens",
                        (element.keyword("opens") == Some("true")
                            || matches!(element.prop("opens"), Some(Prop::Keyword { .. })))
                        .then(|| "true".into()),
                    ),
                ]);
                self.children(
                    out,
                    &format!("F.Drop(p, ctx, {props}, {}, ", self.action_fn(element)),
                    &element.children,
                    ");",
                );
            }
            "Pinch" | "Swipe" => {
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("min", element.number("min").map(number)),
                    ("max", element.number("max").map(number)),
                    ("axis", Self::keyword(element, "axis")),
                    ("distance", element.number("distance").map(number)),
                ]);
                self.children(
                    out,
                    &format!(
                        "F.{}(p, ctx, {props}, {}, ",
                        element.kind,
                        self.action_fn(element)
                    ),
                    &element.children,
                    ");",
                );
            }
            "Toolbar" => self.toolbar(out, element),
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| format!("() => {}", self.expr(route)));
                let label = self
                    .row_label(&element.children)
                    .map(|label| format!("() => {label}"));
                let props = Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("route", route),
                    ("label", label.or_else(|| Some("() => \"\"".into()))),
                ]);
                self.children(
                    out,
                    &format!("B.NavLink(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "List" | "ForEach" => self.list(out, element),
            "Tabs" => self.tabs(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "LanguagePicker" => out.line(format!(
                "B.LanguagePicker(p, ctx, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                ])
            )),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarHeader" | "SidebarFooter"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" | "PickerOption" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }

    fn list(&self, out: &mut Out, element: &Element) {
        let wrap = element.kind == "List";
        if wrap {
            out.open("B.List(p, ctx, (p, ctx) => {");
        }
        if let Some(token) = element.keyword("reflow") {
            out.line(format!("F.reflow(p, {});", self.motion(token)));
        }
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                let stagger = element
                    .number("stagger")
                    .map(|step| format!(", stagger: {}", number(step)))
                    .unwrap_or_default();
                out.open(format!(
                    "B.Each(p, ctx, {{ items: () => {}, key: (v) => v.{name}{stagger} }}, (p, ctx, {}) => {{",
                    self.expr(items),
                    local(&template.var)
                ));
                self.nodes(out, &element.children);
                out.close("});");
            }
            _ => self.nodes(out, &element.children),
        }
        if wrap {
            out.close("});");
        }
    }

    fn video(&self, out: &mut Out, element: &Element) {
        let binding = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => {
                let value = self.expr(source);
                if matches!(source.ty, Type::Optional { .. }) {
                    format!("source: () => ({value})?.url ?? null")
                } else {
                    format!("source: () => ({value}).url")
                }
            }
            (None, Some(Prop::Player { id })) => format!("player: {}", string(id)),
            _ => "source: () => null".into(),
        };
        out.line(format!(
            "B.Video(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "controls",
                    Some((element.keyword("controls") != Some("none")).to_string())
                ),
                ("fit", Self::keyword(element, "fit")),
                ("corner", self.length(element, "corner")),
                ("animate", self.animate(element)),
            ])
            .replacen("{ ", &format!("{{ {binding}, "), 1)
        ));
    }

    fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let spec = &self.app.protocol.commands[binding.query];
        let encoders: Vec<_> = binding
            .args
            .iter()
            .zip(&spec.params)
            .map(|(arg, param)| format!("{};", encode(&param.ty, &self.expr(arg))))
            .collect();
        let params = format!(
            "() => {{ const w = new B.Writer(); {} return w.finish(); }}",
            encoders.join(" ")
        );
        let framework = |key: &str| format!("() => B.tr({})", string(key));
        let empty = self.text_prop(element, "empty");
        let (shown, count) = match &binding.result {
            Type::List { .. } if empty.is_some() => {
                ("(d) => (d.length ? d : null)", "(d) => d.length")
            }
            Type::List { .. } => ("(d) => d", "(d) => d.length"),
            Type::Optional { .. } => ("(d) => d", "(d) => (d === null ? 0 : 1)"),
            _ => ("(d) => d", "() => null"),
        };
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("query", Some(binding.query.to_string())),
            ("params", Some(params)),
            (
                "decode",
                Some(format!("(r) => {}", decode(&binding.result))),
            ),
            (
                "loading",
                self.text_prop(element, "loading")
                    .or_else(|| Some(framework("bern.query.loading"))),
            ),
            (
                "failed",
                self.text_prop(element, "failed")
                    .or_else(|| Some(framework("bern.query.failed"))),
            ),
            ("empty", empty),
            ("emptyIcon", element.icon("icon").map(string)),
            ("count", Some(count.into())),
            ("shown", Some(shown.into())),
        ]);
        out.open(format!(
            "B.Query(p, ctx, {props}, (p, ctx, {}) => {{",
            local(&template.var)
        ));
        self.nodes(out, &element.children);
        out.close("});");
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
        let selection = self.field(sidebar, "selection").expect("selection field");
        let mut search = None;
        let mut footer = None;
        let mut header = None;
        let mut content = Vec::new();
        for node in &sidebar.children {
            match node {
                Node::Element(child) if child.kind == "Search" => search = Some(child),
                Node::Element(child) if child.kind == "SidebarFooter" => footer = Some(child),
                Node::Element(child) if child.kind == "SidebarHeader" => header = Some(child),
                other => content.push(other.clone()),
            }
        }
        out.open("F.SplitView(p, ctx, {");
        if sidebar.id.is_some() {
            out.line(format!("id: {},", self.id(sidebar)));
        }
        out.line(format!("selection: {selection},"));
        if let Some(width) = self.length(sidebar, "width") {
            out.line(format!("width: {width},"));
        }
        if let Some(titled) = self.value_prop(detail, "titled") {
            out.line(format!("titled: {titled},"));
        }
        if let Some(fallback) = sidebar.expr("fallback") {
            out.line(format!("fallback: {},", self.expr(fallback)));
        }
        if let Some(search) = search {
            out.open("search: (p, ctx) => {");
            self.element(out, search);
            out.close("},");
        }
        if let Some(header) = header {
            self.children(out, "header: ", &header.children, ",");
        }
        if let Some(footer) = footer {
            self.children(out, "footer: ", &footer.children, ",");
        }
        out.close("}, (p, ctx) => {");
        out.indent += 1;
        self.in_sidebar.set(true);
        self.nodes(out, &content);
        self.in_sidebar.set(false);
        out.close("}, (p, ctx) => {");
        out.indent += 1;
        self.nodes(out, &detail.children);
        out.close("});");
    }

    /// Toolbar children before the spacer occupy the leading title-bar
    /// slot; children after it occupy the ordinary trailing slot.
    fn toolbar(&self, out: &mut Out, element: &Element) {
        if let Some(index) = element
            .children
            .iter()
            .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"))
        {
            self.children(
                out,
                "B.tools(ctx, ",
                &element.children[..index],
                ", { placement: \"leading\" });",
            );
            self.children(out, "B.tools(ctx, ", &element.children[index + 1..], ");");
        } else {
            self.children(out, "B.tools(ctx, ", &element.children, ");");
        }
    }

    fn navigation(&self, out: &mut Out, element: &Element) {
        let path = self.field(element, "path").expect("path field");
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
        let id = element
            .id
            .as_ref()
            .map(|_| format!("id: {}, ", self.id(element)))
            .unwrap_or_default();
        out.open(format!(
            "B.Navigation(p, ctx, {{ {id}path: {path} }}, (p, ctx) => {{"
        ));
        self.nodes(out, &content);
        let var = destination
            .and_then(|destination| destination.template.as_ref())
            .map(|template| local(&template.var))
            .unwrap_or_else(|| "l_route".into());
        out.close(format!("}}, (p, ctx, {var}) => {{"));
        out.indent += 1;
        if let Some(destination) = destination {
            self.nodes(out, &destination.children);
        }
        out.close("});");
    }

    /// One builder per layout size; the runtime picks one by the available
    /// width and the breakpoint from `[layouts]`.
    fn adaptive(&self, out: &mut Out, element: &Element) {
        out.open("B.Adaptive(p, ctx, {");
        if element.id.is_some() {
            out.line(format!("id: {},", self.id(element)));
        }
        out.line(format!("breakpoints: {},", self.breakpoints()));
        out.open("layouts: {");
        for node in &element.children {
            let Node::Element(layout) = node else {
                continue;
            };
            let Some(size) = layout.keyword("size") else {
                continue;
            };
            out.open(format!("{size}: (p, ctx) => {{"));
            self.nodes(out, &layout.children);
            out.close("},");
        }
        out.close("},");
        out.close("});");
    }

    /// The breakpoints of `[layouts]` for the runtime, `{ compact, medium,
    /// expanded }`; absent ones are `null`.
    fn breakpoints(&self) -> String {
        match self.app.layouts {
            Some(layouts) => format!(
                "{{ compact: {}, medium: {}, expanded: {} }}",
                number(layouts.compact_below),
                layouts.medium_below.map(number).unwrap_or("null".into()),
                layouts.expanded_below.map(number).unwrap_or("null".into())
            ),
            None => "{ compact: 0, medium: null, expanded: null }".into(),
        }
    }

    /// A menu button and its items; dividers are `null`.
    fn menu(&self, out: &mut Out, element: &Element) {
        let mut shortcuts = Vec::new();
        out.open(format!(
            "B.Menu(p, ctx, {}, [",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("icon", element.icon("icon").map(string)),
                ("style", Self::keyword(element, "style")),
                ("shape", Self::keyword(element, "shape")),
                ("height", self.length(element, "height")),
            ])
        ));
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("null,");
                continue;
            }
            let enabled = item
                .expr("enabled")
                .map(|expr| format!("() => {}", self.expr(expr)));
            let (action, set_file, kinds) = self.file_actions(item);
            if item.keyword("shortcut").is_some() {
                shortcuts.push((item, action.clone()));
            }
            out.line(format!(
                "{},",
                Self::props(vec![
                    ("id", Some(self.id(item))),
                    ("label", self.text_prop(item, "label")),
                    ("icon", item.icon("icon").map(string)),
                    ("role", Self::keyword(item, "role")),
                    ("enabled", enabled),
                    ("action", Some(action)),
                    ("setFile", set_file),
                    ("fileKinds", kinds),
                ])
            ));
        }
        out.close("]);");
        for (item, action) in shortcuts {
            self.shortcut(out, item, &action);
        }
    }

    fn tabs(&self, out: &mut Out, element: &Element) {
        let selection = self.field(element, "selection").expect("selection field");
        let mut options = Vec::new();
        out.open("B.Tabs(p, ctx, {");
        if element.id.is_some() {
            out.line(format!("id: {},", self.id(element)));
        }
        out.line(format!("selection: {selection},"));
        out.open("tabs: [");
        for node in &element.children {
            let Node::Element(tab) = node else { continue };
            let Some(value) = tab.expr("value") else {
                continue;
            };
            let label = tab
                .text("label")
                .map(|text| self.text(text))
                .unwrap_or_default();
            options.push((self.expr(value), label.clone()));
            out.open(format!(
                "{{ value: {}, name: {}, label: () => {label}, icon: {}, build: (p, ctx) => {{",
                self.expr(value),
                self.variant_name(value),
                tab.icon("icon")
                    .map(string)
                    .unwrap_or_else(|| "null".into())
            ));
            self.nodes(out, &tab.children);
            out.close("} },");
        }
        out.close("],");
        let accessory = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "TabAccessory" => Some(child),
            _ => None,
        });
        if let Some(accessory) = accessory {
            out.open(format!(
                "accessory: {{ shown: {}, build: (p, ctx) => {{",
                accessory
                    .expr("shown")
                    .map(|shown| format!("() => {}", self.expr(shown)))
                    .unwrap_or_else(|| "null".into())
            ));
            self.nodes(out, &accessory.children);
            out.close("} },");
        }
        out.line(format!(
            "selectedLabel: {},",
            self.selected_label(&selection, &options)
        ));
        out.close("});");
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        out.line(format!(
            "// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!("function c_{}(p, ctx, a) {{", component.name));
        for param in &component.params {
            if let ParamKind::Value { .. } = param.kind {
                out.line(format!("const {} = a.{};", local(&param.name), param.name));
            }
        }
        self.node(out, &component.body);
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("// Root of the `{}` window.", window.id));
        out.open(format!("function w_{}(p, ctx) {{", window.id));
        self.node(out, &window.content);
        out.close("}");
        out.line("");
    }

    /// The close check of a window, evaluated against its own drafts.
    fn close_check(&self, close: &bern_ir::CloseCheck) -> String {
        // The save command's completion decides whether closing goes on.
        let save = match &close.save {
            Some(Action::Engine { command, args }) => {
                let spec = &self.app.protocol.commands[*command];
                let mut values = vec!["ctx".to_string()];
                values.extend(args.iter().map(|arg| self.expr(arg)));
                values.push("done".into());
                format!(
                    "(done) => C.{}({})",
                    self.app.protocol.command_symbol(spec),
                    values.join(", ")
                )
            }
            _ => "null".into(),
        };
        format!(
            "(win) => {{ const ctx = {{ win, s: win.sessions }}; return {{ needsConfirmation: () => {}, message: () => {}, save: {save} }}; }}",
            self.expr(&close.when),
            self.text(&close.message)
        )
    }

    /// The page's `app.js`. `version` is appended to the URLs it loads, so
    /// a rebuilt page never runs with a cached runtime or module.
    /// The motion tokens (`M`), transitions (`X`) and patterns (`P`) of
    /// `style!`.
    fn style_tables(&self, out: &mut Out) {
        let style = &self.app.style;
        out.line("// Motion tokens: period and damping ratio of each spring.");
        out.open("const M = {");
        for token in &style.motion {
            out.line(format!(
                "{}: [{}, {}],",
                member(&token.name),
                number(token.period),
                number(token.damping)
            ));
        }
        out.close("};");
        out.line("// Transitions of `appear` and `disappear`.");
        out.open("const X = {");
        for transition in &style.transitions {
            let motion = format!("M.{}", member(&transition.motion));
            let kind = match &transition.kind {
                bern_ir::style::TransitionKind::Fade => "{ kind: \"fade\"".to_string(),
                bern_ir::style::TransitionKind::Grow { edge } => format!(
                    "{{ kind: \"grow\", edge: {}",
                    string(&format!("{edge:?}").to_lowercase())
                ),
                bern_ir::style::TransitionKind::Pop { from } => {
                    format!("{{ kind: \"pop\", from: {}", number(*from))
                }
                bern_ir::style::TransitionKind::Slide { edge, distance } => format!(
                    "{{ kind: \"slide\", edge: {}, distance: {}",
                    string(&format!("{edge:?}").to_lowercase()),
                    number(*distance)
                ),
            };
            out.line(format!(
                "{}: {kind}, motion: {motion} }},",
                member(&transition.name)
            ));
        }
        out.close("};");
        out.line("// Patterns of `style!`.");
        out.open("const P = {");
        for pattern in &style.patterns {
            let bands: Vec<_> = pattern
                .bands
                .iter()
                .map(|band| {
                    format!(
                        "{{ color: {}, width: {}, opacity: {} }}",
                        band.color
                            .as_ref()
                            .map(color_ref)
                            .unwrap_or_else(|| "null".into()),
                        number(band.width),
                        number(band.opacity)
                    )
                })
                .collect();
            out.line(format!(
                "{}: {{ bands: [{}], twill: {} }},",
                member(&pattern.name),
                bands.join(", "),
                pattern.twill
            ));
        }
        out.close("};");
        out.line("// Style overrides of `style!`, as this target shows them.");
        out.open("const S = {");
        for style in crate::style::used(self.app, crate::style::PLATFORM) {
            out.line(format!("{},", crate::style::declaration(&style)));
        }
        out.close("};");
        out.line("");
    }

    /// The spring of a motion token, `M.name`.
    fn motion(&self, name: &str) -> String {
        format!("M.{}", member(name))
    }

    /// A length: a number, or a getter for one bound to an expression.
    fn length(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(number(*value)),
            Prop::Expr { expr } => Some(format!("() => Number({})", self.expr(expr))),
            _ => None,
        }
    }

    /// `animate`'s spring, for the bound lengths of an element.
    fn animate(&self, element: &Element) -> Option<String> {
        element.keyword("animate").map(|token| self.motion(token))
    }

    /// A `Canvas`: its drawing elements in order, read in one getter.
    fn canvas(&self, out: &mut Out, element: &Element) {
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else { continue };
            let number_of = |name: &str| child.number(name).map(number);
            match child.kind.as_str() {
                "Shape" => {
                    let fill_from = match (child.expr("fill_from"), child.resource("fill_from")) {
                        (Some(expr), _) if matches!(expr.ty, Type::Optional { .. }) => Some(
                            format!("F.averageColor(({})?.url ?? null)", self.expr(expr)),
                        ),
                        (Some(expr), _) => {
                            Some(format!("F.averageColor(({}).url)", self.expr(expr)))
                        }
                        (None, Some(name)) => {
                            Some(format!("F.averageColor(F.resource({}))", string(name)))
                        }
                        (None, None) => None,
                    };
                    ops.push(Self::props(vec![
                        (
                            "shape",
                            Some(string(child.keyword("kind").unwrap_or("rect"))),
                        ),
                        ("x", number_of("x")),
                        ("y", number_of("y")),
                        ("width", number_of("width")),
                        ("height", number_of("height")),
                        ("corner", number_of("corner")),
                        ("fill", role(child, "fill")),
                        ("fillFrom", fill_from),
                        ("fade", Self::keyword(child, "fade")),
                        ("stroke", role(child, "stroke")),
                        ("strokeWidth", number_of("stroke_width")),
                        ("opacity", number_of("opacity")),
                    ]));
                }
                "Series" => {
                    let values = child
                        .expr("values")
                        .map(|list| format!("({}).map(Number)", self.expr(list)));
                    let max = child
                        .expr("max")
                        .map(|max| format!("Number({})", self.expr(max)))
                        .or_else(|| number_of("max"));
                    ops.push(Self::props(vec![
                        ("series", values),
                        ("max", max),
                        (
                            "area",
                            Some((child.keyword("kind") == Some("area")).to_string()),
                        ),
                        ("fill", role(child, "fill")),
                        ("stroke", role(child, "stroke")),
                        ("strokeWidth", number_of("stroke_width")),
                        ("opacity", number_of("opacity")),
                    ]));
                }
                "Pattern" => ops.push(Self::props(vec![
                    (
                        "pattern",
                        child
                            .keyword("name")
                            .map(|name| format!("P.{}", member(name))),
                    ),
                    ("opacity", number_of("opacity")),
                ])),
                _ => {}
            }
        }
        out.line(format!(
            "F.Canvas(p, ctx, {}, () => [{}]);",
            Self::props(vec![
                ("width", self.length(element, "width")),
                ("height", self.length(element, "height")),
                ("label", self.text_prop(element, "label")),
                ("animate", self.animate(element)),
            ]),
            ops.join(", ")
        ));
    }

    pub fn generate(
        &self,
        fingerprint: u64,
        strings: &serde_json::Value,
        wasm: &str,
        mode: &str,
        version: &str,
    ) -> String {
        let app = self.app;
        let types = self.types();
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("");
        out.line(format!("import * as B from \"./bern.js?v={version}\";"));
        out.line(format!("import * as F from \"./features.js?v={version}\";"));
        out.line("");
        self.style_tables(&mut out);
        out.line("// Values: codecs, variant names and labels.");
        out.line("const T = {};");
        out.line("const N = {};");
        out.line("const L = {};");
        out.line("");
        for def in &types.enums {
            generate_enum(&mut out, def);
        }
        for def in &types.records {
            generate_record(&mut out, def);
        }

        out.line("// Engine state, mirrored from Rust.");
        out.open("const E = {");
        for field in &app.protocol.state {
            let initial = Value::default_for(&field.ty, types).unwrap_or(Value::Unit);
            out.line(format!(
                "{}: B.cell({}),",
                field.name,
                literal(&initial, &field.ty, types)
            ));
        }
        out.close("};");
        out.open("const engine = {");
        out.open("apply(field, r) {");
        out.open("switch (field) {");
        for (index, field) in app.protocol.state.iter().enumerate() {
            out.line(format!(
                "case {index}: E.{}.set({}); break;",
                field.name,
                decode(&field.ty)
            ));
        }
        out.line("default: throw new B.DecodeError(`unknown state field ${field}`);");
        out.close("}");
        out.close("},");
        out.close("};");
        out.line("");

        out.line("// Engine commands.");
        out.open("const C = {");
        for (index, command) in app.protocol.commands.iter().enumerate() {
            if command.is_query() {
                continue;
            }
            let params: Vec<_> = command
                .params
                .iter()
                .map(|param| local(&param.name))
                .collect();
            let encoders: Vec<_> = command
                .params
                .iter()
                .map(|param| format!("{};", encode(&param.ty, &local(&param.name))))
                .collect();
            let mut signature = vec!["ctx".to_string()];
            signature.extend(params);
            signature.push("done".into());
            out.line(format!(
                "{}: ({}) => B.dispatch(ctx, {index}, (w) => {{ {} }}, done),",
                app.protocol.command_symbol(command),
                signature.join(", "),
                encoders.join(" ")
            ));
        }
        out.close("};");
        out.line("");

        out.line("// Components.");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }

        let sessions: Vec<String> = app
            .sessions
            .iter()
            .map(|session| {
                let fields: Vec<_> = session
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{{ name: {}, codec: {} }}",
                            string(&field.name),
                            codec(&field.ty)
                        )
                    })
                    .collect();
                format!(
                    "{{ name: {}, fields: [{}] }}",
                    string(&session.name),
                    fields.join(", ")
                )
            })
            .collect();
        let windows: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                let settings = window.role == WindowRole::Settings;
                // The main window is the page and other windows are
                // dialogs; only an explicit `floating` gets a window frame.
                let presentation = if window.presentation == Presentation::Floating {
                    "floating"
                } else if window.id == app.windows.primary {
                    "page"
                } else {
                    "dialog"
                };
                let optional = |value: Option<f64>| value.map(number).unwrap_or("null".into());
                let immersive = window
                    .immersive
                    .as_ref()
                    .map(|expr| format!("(ctx) => ({})", self.expr(expr)))
                    .unwrap_or_else(|| "null".into());
                let idle_enabled = window.idle_enabled.as_ref().map(|e| format!("(ctx) => ({})", self.expr(e))).unwrap_or_else(|| "null".into());
                format!(
                    "{{ id: {}, title: {}, presentation: \"{presentation}\", settings: {settings}, resizable: {}, width: {}, height: {}, minWidth: {}, minHeight: {}, initial: {}, render: w_{}, closeCheck: {}, idleAfter: {}, idleEnabled: {idle_enabled}, immersive: {immersive} }}",
                    string(&window.id),
                    string(&window.title.key),
                    !settings,
                    optional(window.width),
                    optional(window.height),
                    optional(window.min_width),
                    optional(window.min_height),
                    window.initial == WindowInitial::Open,
                    window.id,
                    window
                        .close
                        .as_ref()
                        .map(|close| self.close_check(close))
                        .unwrap_or_else(|| "null".into()),
                    optional(window.idle_after)
                )
            })
            .collect();
        let used = self.icons();
        let icons: Vec<_> = used
            .iter()
            .filter_map(|name| {
                icons::svg(name).map(|svg| format!("{}: {}", string(name), string(&svg)))
            })
            .collect();
        out.line("// File kinds of `files:`, by extension.");
        let kinds: Vec<String> = bern_ir::spec::FILE_KIND_EXTENSIONS
            .iter()
            .map(|(kind, extensions)| {
                let list: Vec<String> = extensions.iter().map(|ext| string(ext)).collect();
                format!("{kind}: [{}]", list.join(", "))
            })
            .collect();
        out.line(format!("F.fileKinds({{ {} }});", kinds.join(", ")));
        let accent = app
            .windows
            .accent
            .as_ref()
            .map(|expr| format!("() => {}", self.variant_name(expr)));
        let appearance = app
            .windows
            .appearance
            .as_ref()
            .map(|expr| format!("() => {}", self.variant_name(expr)));
        if accent.is_some() || appearance.is_some() {
            out.line("// The accent and appearance the application chooses.");
            out.line(format!(
                "F.theme({}, {});",
                accent.unwrap_or_else(|| "null".into()),
                appearance.unwrap_or_else(|| "null".into())
            ));
        }
        out.line("// Startup.");
        out.open("B.start({");
        out.line(format!("mode: {},", string(mode)));
        out.line(format!("wasm: {},", string(wasm)));
        out.line(format!("fingerprint: 0x{fingerprint:016x}n,"));
        out.line("resources: \"resources/\",");
        out.line(format!(
            "languages: {},",
            serde_json::to_string(&app.languages.supported).expect("languages")
        ));
        out.line(format!(
            "defaultLanguage: {},",
            string(&app.languages.default)
        ));
        out.line(format!("primary: {},", string(&app.windows.primary)));
        if app.layouts.is_some() {
            out.line(format!("layouts: {},", self.breakpoints()));
        }
        out.line(format!("strings: {strings},"));
        out.line(
            "// Icons: Lucide (ISC; icons derived from Feather MIT), see THIRD_PARTY_NOTICES.txt.",
        );
        out.line(format!("icons: {{ {} }},", icons.join(", ")));
        out.line("engine,");
        out.line(format!("sessions: [{}],", sessions.join(", ")));
        out.open("windows: [");
        for window in windows {
            out.line(format!("{window},"));
        }
        out.close("],");
        out.close("});");
        out.finish()
    }

    /// Every icon the page draws: declared ones and the runtime's own.
    pub fn icons(&self) -> BTreeSet<String> {
        let mut used: BTreeSet<String> = icons::RUNTIME_ICONS
            .iter()
            .map(|name| name.to_string())
            .collect();
        let mut visit = |node: &Node| {
            if let Node::Element(element) = node {
                for prop in element.props.values() {
                    if let Prop::Icon { name } = prop {
                        used.insert(name.clone());
                    }
                }
            }
        };
        for component in &self.app.components {
            component.body.walk(&mut visit);
        }
        for window in &self.app.windows.windows {
            window.content.walk(&mut visit);
        }
        used
    }
}
