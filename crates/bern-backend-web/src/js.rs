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
    IfNode, MatchNode, MediaOp, Node, ParamKind, Prop, Text, WindowDef, WindowInitial, WindowOp,
    WindowRole,
};
use bern_protocol::{EnumDef, RecordDef, Type, TypeTable, Value};
use std::collections::BTreeSet;

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
pub fn string(value: &str) -> String {
    serde_json::to_string(value).expect("string")
}

fn number(value: f64) -> String {
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
        let index = if def.is_unit_only() { "v" } else { "v.tag" };
        out.line(format!(
            "L.{name} = (v) => B.tr([{}][{index}]);",
            keys.join(", ")
        ));
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
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        Self { app }
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
                            let value = args
                                .iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| self.display(expr))
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

    fn action(&self, action: &Action) -> String {
        match action {
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values = vec!["ctx".to_string()];
                values.extend(args.iter().map(|arg| self.expr(arg)));
                format!("C.{}({})", spec.name, values.join(", "))
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
        out.open(format!("{prefix}(p, ctx) => {{"));
        self.nodes(out, nodes);
        out.close(format!("}}{suffix}"));
    }

    fn node(&self, out: &mut Out, node: &Node) {
        match node {
            Node::Element(element) => self.element(out, element),
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
                "B.Text(p, ctx, {});",
                Self::props(vec![
                    ("content", self.text_prop(element, "content")),
                    ("style", Self::keyword(element, "style")),
                    ("emphasis", Self::keyword(element, "emphasis")),
                ])
            )),
            "Label" => out.line(format!(
                "B.Label(p, ctx, {});",
                Self::props(vec![
                    ("title", self.text_prop(element, "title")),
                    ("icon", element.icon("icon").map(string)),
                    ("style", Self::keyword(element, "style")),
                ])
            )),
            "Button" | "IconButton" => {
                let enabled = element
                    .expr("enabled")
                    .map(|expr| format!("() => {}", self.expr(expr)));
                out.line(format!(
                    "B.{}(p, ctx, {}, {});",
                    element.kind,
                    Self::props(vec![
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("icon", element.icon("icon").map(string)),
                        ("role", Self::keyword(element, "role")),
                        ("prominence", Self::keyword(element, "prominence")),
                        ("enabled", enabled),
                    ]),
                    self.action_fn(element)
                ));
            }
            "TextInput" => out.line(format!(
                "B.TextInput(p, ctx, {}, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("prompt", self.text_prop(element, "prompt")),
                    ("kind", Self::keyword(element, "kind")),
                ]),
                self.action_fn(element)
            )),
            "Search" => out.line(format!(
                "B.Search(p, ctx, {}, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("field", self.field(element, "value")),
                    ("prompt", self.text_prop(element, "prompt")),
                ]),
                self.action_fn(element)
            )),
            "Switch" => out.line(format!(
                "B.Switch(p, ctx, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("style", Self::keyword(element, "style")),
                ])
            )),
            "Slider" => out.line(format!(
                "B.Slider(p, ctx, {}, {});",
                Self::props(vec![
                    ("id", Some(self.id(element))),
                    ("label", self.text_prop(element, "label")),
                    ("field", self.field(element, "value")),
                    ("min", element.number("min").map(number)),
                    ("max", element.number("max").map(number)),
                    ("step", element.number("step").map(number)),
                    ("minIcon", element.icon("min_icon").map(string)),
                    ("maxIcon", element.icon("max_icon").map(string)),
                ]),
                self.action_fn(element)
            )),
            "Picker" => {
                let field_ref = element.field("value").expect("picker field");
                let Type::Enum { name } = &field_ref.ty else {
                    return;
                };
                let def = self.types().enum_def(name).expect("enum");
                let options: Vec<_> = def
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        format!(
                            "{{ value: {index}, name: {}, label: () => L.{name}({index}) }}",
                            string(&variant.name)
                        )
                    })
                    .collect();
                out.line(format!(
                    "B.Picker(p, ctx, {});",
                    Self::props(vec![
                        ("id", Some(self.id(element))),
                        ("label", self.text_prop(element, "label")),
                        ("field", self.field(element, "value")),
                        ("options", Some(format!("[{}]", options.join(", ")))),
                        ("style", Self::keyword(element, "style")),
                    ])
                ));
            }
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => {
                let props = Self::props(vec![("title", self.text_prop(element, "title"))]);
                self.children(
                    out,
                    &format!("B.Form(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "Section" => {
                let props = Self::props(vec![
                    ("title", self.text_prop(element, "title")),
                    ("footer", self.text_prop(element, "footer")),
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
                    ("gap", element.number("gap").map(number)),
                    ("padding", element.number("padding").map(number)),
                    ("align", Self::keyword(element, "align")),
                ]);
                self.children(
                    out,
                    &format!("B.Stack(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "Spacer" => out.line("B.Spacer(p);"),
            "Divider" => out.line("B.Divider(p);"),
            "ScrollView" => {
                let props = Self::props(vec![("axis", Self::keyword(element, "axis"))]);
                self.children(
                    out,
                    &format!("B.ScrollView(p, ctx, {props}, "),
                    &element.children,
                    ");",
                );
            }
            "Toolbar" => self.children(out, "B.tools(ctx, ", &element.children, ");"),
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
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "WindowSet"
            | "Window" => {
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
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                out.open(format!(
                    "B.Each(p, ctx, {{ items: () => {}, key: (v) => v.{name} }}, (p, ctx, {}) => {{",
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
        // Sidebar groups: titled sections, and untitled runs of items.
        let mut sections: Vec<(Option<String>, Vec<&Element>)> = Vec::new();
        let mut search = None;
        for node in &sidebar.children {
            let Node::Element(child) = node else { continue };
            match child.kind.as_str() {
                "Search" => search = Some(child),
                "Section" => {
                    let items = child
                        .children
                        .iter()
                        .filter_map(|node| match node {
                            Node::Element(item) if item.kind == "SidebarItem" => Some(item),
                            _ => None,
                        })
                        .collect();
                    sections.push((self.text_prop(child, "title"), items));
                }
                "SidebarItem" => match sections.last_mut() {
                    Some((None, items)) => items.push(child),
                    _ => sections.push((None, vec![child])),
                },
                _ => {}
            }
        }
        let mut options = Vec::new();
        out.open("B.SplitView(p, ctx, {");
        if sidebar.id.is_some() {
            out.line(format!("id: {},", self.id(sidebar)));
        }
        out.line(format!("selection: {selection},"));
        if let Some(search) = search {
            out.line(format!(
                "search: (p, ctx) => B.Search(p, ctx, {}, {}),",
                Self::props(vec![
                    ("id", Some(self.id(search))),
                    ("field", self.field(search, "value")),
                    ("prompt", self.text_prop(search, "prompt")),
                ]),
                self.action_fn(search)
            ));
        }
        out.open("sections: [");
        for (title, items) in &sections {
            out.open(format!(
                "{{ title: {}, items: [",
                title.clone().unwrap_or_else(|| "null".into())
            ));
            for item in items {
                let Some(value) = item.expr("value") else {
                    continue;
                };
                let label = item.text("label").map(|text| self.text(text));
                if let Some(label) = &label {
                    options.push((self.expr(value), label.clone()));
                }
                out.line(format!(
                    "{},",
                    Self::props(vec![
                        ("value", Some(self.expr(value))),
                        ("name", Some(self.variant_name(value))),
                        ("label", label.map(|label| format!("() => {label}"))),
                        ("icon", item.icon("icon").map(string)),
                    ])
                ));
            }
            out.close("] },");
        }
        out.close("],");
        out.line(format!(
            "selectedLabel: {},",
            self.selected_label(&selection, &options)
        ));
        out.close("}, (p, ctx) => {");
        out.indent += 1;
        self.nodes(out, &detail.children);
        out.close("});");
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
                format!("(done) => C.{}({})", spec.name, values.join(", "))
            }
            _ => "null".into(),
        };
        format!(
            "(win) => {{ const ctx = {{ win, s: win.sessions }}; return {{ needsConfirmation: () => {}, message: () => {}, save: {save} }}; }}",
            self.expr(&close.when),
            self.text(&close.message)
        )
    }

    pub fn generate(&self, fingerprint: u64, strings: &serde_json::Value, wasm: &str) -> String {
        let app = self.app;
        let types = self.types();
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("");
        out.line("import * as B from \"./bern.js\";");
        out.line("");
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
                command.name,
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
                let optional = |value: Option<f64>| value.map(number).unwrap_or("null".into());
                format!(
                    "{{ id: {}, title: {}, settings: {settings}, resizable: {}, width: {}, height: {}, minWidth: {}, minHeight: {}, initial: {}, render: w_{}, closeCheck: {} }}",
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
                        .unwrap_or_else(|| "null".into())
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
        out.line("// Startup.");
        out.open("B.start({");
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
        out.line(format!("strings: {strings},"));
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
