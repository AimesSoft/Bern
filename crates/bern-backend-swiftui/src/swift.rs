//! Swift source generation from the IR.
//!
//! Every component becomes a SwiftUI `View` built from standard containers
//! and controls with system styles. Nothing here sets a color, a material or
//! a version-gated API: appearance comes from the SDK and the running macOS.

use crate::names::{case, double, member, pascal, string};
use bern_ir::{
    Action, Application, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind, IfLetNode,
    IfNode, MatchNode, MediaOp, Node, ParamKind, Prop, Text, WindowDef, WindowOp, WindowRole,
};
use bern_protocol::{EnumDef, RecordDef, Type, TypeTable, Value};

/// Indented line writer.
#[derive(Default)]
pub struct Out {
    lines: Vec<String>,
    indent: usize,
}

impl Out {
    pub fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.lines.push(String::new());
        } else {
            self.lines
                .push(format!("{}{}", "    ".repeat(self.indent), text));
        }
    }

    pub fn open(&mut self, text: impl AsRef<str>) {
        self.line(text);
        self.indent += 1;
    }

    pub fn close(&mut self, text: impl AsRef<str>) {
        self.indent = self.indent.saturating_sub(1);
        self.line(text);
    }

    pub fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

pub fn swift_type(ty: &Type) -> String {
    match ty {
        Type::Unit => "Void".into(),
        Type::Bool => "Bool".into(),
        Type::Int => "Int64".into(),
        Type::Float => "Double".into(),
        Type::Text => "String".into(),
        Type::Enum { name } | Type::Record { name } => name.clone(),
        Type::Optional { inner } => format!("Optional<{}>", swift_type(inner)),
        Type::List { inner } => format!("[{}]", swift_type(inner)),
    }
}

/// Swift expression for a schema value.
fn value_literal(value: &Value, ty: &Type, types: &TypeTable) -> String {
    match (value, ty) {
        (Value::Bool(value), _) => value.to_string(),
        (Value::Int(value), _) => format!("Int64({value})"),
        (Value::Float(value), _) => format!("Double({})", double(*value)),
        (Value::Text(value), _) => string(value),
        (Value::Optional(None), _) => "nil".into(),
        (Value::Optional(Some(value)), Type::Optional { inner }) => {
            value_literal(value, inner, types)
        }
        (Value::List(items), Type::List { inner }) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| value_literal(item, inner, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            let def = types.enum_def(name).expect("known enum");
            let variant_def = &def.variants[*variant as usize];
            if variant_def.fields.is_empty() {
                format!("{name}.{}", case(&variant_def.name))
            } else {
                let args: Vec<_> = fields
                    .iter()
                    .zip(&variant_def.fields)
                    .map(|(value, field)| {
                        format!(
                            "{}: {}",
                            member(&field.name),
                            value_literal(value, &field.ty, types)
                        )
                    })
                    .collect();
                format!("{name}.{}({})", case(&variant_def.name), args.join(", "))
            }
        }
        (Value::Record(values), Type::Record { name }) => {
            let def = types.record_def(name).expect("known record");
            let args: Vec<_> = values
                .iter()
                .zip(&def.fields)
                .map(|(value, field)| {
                    format!(
                        "{}: {}",
                        member(&field.name),
                        value_literal(value, &field.ty, types)
                    )
                })
                .collect();
            format!("{name}({})", args.join(", "))
        }
        _ => "()".into(),
    }
}

fn default_literal(ty: &Type, types: &TypeTable) -> String {
    let value = Value::default_for(ty, types).unwrap_or(Value::Unit);
    value_literal(&value, ty, types)
}

fn generate_enum(out: &mut Out, def: &EnumDef) {
    let name = &def.name;
    let unit_only = def.is_unit_only();
    let mut conformances = vec!["Hashable", "Sendable", "BernCodable"];
    if unit_only {
        conformances.push("CaseIterable");
    }
    out.open(format!("enum {name}: {} {{", conformances.join(", ")));
    for variant in &def.variants {
        if variant.fields.is_empty() {
            out.line(format!("case {}", case(&variant.name)));
        } else {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("{}: {}", member(&field.name), swift_type(&field.ty)))
                .collect();
            out.line(format!(
                "case {}({})",
                case(&variant.name),
                fields.join(", ")
            ));
        }
    }
    out.line("");
    out.open("init(bern reader: inout BernReader) throws {");
    out.open("switch try reader.u32() {");
    for (index, variant) in def.variants.iter().enumerate() {
        if variant.fields.is_empty() {
            out.line(format!("case {index}: self = .{}", case(&variant.name)));
        } else {
            let args: Vec<_> = variant
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: try {}(bern: &reader)",
                        member(&field.name),
                        swift_type(&field.ty)
                    )
                })
                .collect();
            out.line(format!(
                "case {index}: self = .{}({})",
                case(&variant.name),
                args.join(", ")
            ));
        }
    }
    out.line(format!(
        "default: throw BernDecodeError(message: {})",
        string(&format!("invalid {name}"))
    ));
    out.close("}");
    out.close("}");
    out.line("");
    out.open("func encode(bern writer: inout BernWriter) {");
    out.open("switch self {");
    for (index, variant) in def.variants.iter().enumerate() {
        if variant.fields.is_empty() {
            out.line(format!(
                "case .{}: writer.u32({index})",
                case(&variant.name)
            ));
        } else {
            let names: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("let {}", member(&field.name)))
                .collect();
            out.open(format!(
                "case .{}({}):",
                case(&variant.name),
                names.join(", ")
            ));
            out.line(format!("writer.u32({index})"));
            for field in &variant.fields {
                out.line(format!("{}.encode(bern: &writer)", member(&field.name)));
            }
            out.indent -= 1;
        }
    }
    out.close("}");
    out.close("}");
    out.line("");
    out.line("/// The declared variant name, used by external entries.");
    out.open("var bernName: String {");
    out.open("switch self {");
    for variant in &def.variants {
        out.line(format!(
            "case .{}: {}",
            case(&variant.name),
            string(&variant.name)
        ));
    }
    out.close("}");
    out.close("}");
    if unit_only {
        out.line("");
        out.open("init?(bernName: String) {");
        out.open("switch bernName {");
        for variant in &def.variants {
            out.line(format!(
                "case {}: self = .{}",
                string(&variant.name),
                case(&variant.name)
            ));
        }
        out.line("default: return nil");
        out.close("}");
        out.close("}");
    }
    out.close("}");
    if def.variants.iter().all(|variant| variant.label.is_some()) {
        out.line("");
        out.open(format!("extension {name}: BernLabeled {{"));
        out.open("var bernLabel: String {");
        out.open("switch self {");
        for variant in &def.variants {
            let key = &variant.label.as_ref().unwrap().key;
            out.line(format!(
                "case .{}: BernText.string({})",
                case(&variant.name),
                string(key)
            ));
        }
        out.close("}");
        out.close("}");
        out.close("}");
    }
    out.line("");
}

fn generate_record(out: &mut Out, def: &RecordDef) {
    let name = &def.name;
    out.open(format!("struct {name}: Hashable, Sendable {{"));
    for field in &def.fields {
        out.line(format!(
            "var {}: {}",
            member(&field.name),
            swift_type(&field.ty)
        ));
    }
    out.close("}");
    out.line("");
    out.open(format!("extension {name}: BernCodable {{"));
    out.open("init(bern reader: inout BernReader) throws {");
    let args: Vec<_> = def
        .fields
        .iter()
        .map(|field| {
            format!(
                "{}: try {}(bern: &reader)",
                member(&field.name),
                swift_type(&field.ty)
            )
        })
        .collect();
    out.line(format!("self.init({})", args.join(", ")));
    out.close("}");
    out.line("");
    out.open("func encode(bern writer: inout BernWriter) {");
    for field in &def.fields {
        out.line(format!("{}.encode(bern: &writer)", member(&field.name)));
    }
    out.close("}");
    out.close("}");
    out.line("");
}

fn generate_engine(out: &mut Out, app: &Application) {
    let protocol = &app.protocol;
    out.line(format!(
        "/// Read-only state of the `{}` engine, mirrored from Rust.",
        protocol.namespace
    ));
    out.line("@MainActor");
    out.line("@Observable");
    out.open("final class BernEngine {");
    out.line("static let shared = BernEngine()");
    out.line("");
    for field in &protocol.state {
        if !field.doc.is_empty() {
            out.line(format!("/// {}", field.doc.replace('\n', " ")));
        }
        out.line(format!(
            "private(set) var {}: {} = {}",
            member(&field.name),
            swift_type(&field.ty),
            default_literal(&field.ty, &app.types)
        ));
    }
    out.line("");
    out.open("func bernApply(_ field: UInt32, _ reader: inout BernReader) throws {");
    out.open("switch field {");
    for (index, field) in protocol.state.iter().enumerate() {
        out.line(format!(
            "case {index}: {} = try {}(bern: &reader)",
            member(&field.name),
            swift_type(&field.ty)
        ));
    }
    out.line("default: throw BernDecodeError(message: \"unknown state field \\(field)\")");
    out.close("}");
    out.close("}");
    for (index, command) in protocol.commands.iter().enumerate() {
        out.line("");
        if !command.doc.is_empty() {
            out.line(format!("/// {}", command.doc.replace('\n', " ")));
        }
        let mut params: Vec<String> = command
            .params
            .iter()
            .map(|param| format!("_ {}: {}", member(&param.name), swift_type(&param.ty)))
            .collect();
        params.push("from window: BernWindowContext?".into());
        params.push("completion: (@MainActor (BernCommandFailure?) -> Void)? = nil".into());
        out.open(format!(
            "func {}({}) {{",
            member(&command.name),
            params.join(", ")
        ));
        if command.params.is_empty() {
            out.line(format!(
                "BernHost.shared.dispatch({index}, window: window?.id, encode: {{ _ in }}, completion: completion)"
            ));
        } else {
            out.open(format!(
                "BernHost.shared.dispatch({index}, window: window?.id, encode: {{ writer in"
            ));
            for param in &command.params {
                out.line(format!("{}.encode(bern: &writer)", member(&param.name)));
            }
            out.close("}, completion: completion)");
        }
        out.close("}");
    }
    out.close("}");
    out.line("");
}

fn session_class(name: &str) -> String {
    format!("{name}Session")
}

fn session_var(name: &str) -> String {
    format!("bern{name}")
}

fn generate_sessions(out: &mut Out, app: &Application) {
    for (index, session) in app.sessions.iter().enumerate() {
        let class = session_class(&session.name);
        out.line(format!(
            "/// Drafts of `{}`, one instance per hosting window.",
            session.name
        ));
        out.line("@MainActor");
        out.line("@Observable");
        out.open(format!("final class {class}: BernSessionState {{"));
        out.line(format!("static let bernIndex: UInt32 = {index}"));
        out.line("@ObservationIgnored weak var window: BernWindowContext?");
        for (field_index, field) in session.fields.iter().enumerate() {
            let name = member(&field.name);
            let storage = format!("bern_{}", field.name);
            let ty = swift_type(&field.ty);
            out.line("");
            out.line(format!(
                "private var {storage}: {ty} = {}",
                value_literal(&field.initial, &field.ty, &app.types)
            ));
            out.open(format!("var {name}: {ty} {{"));
            out.line(format!("get {{ {storage} }}"));
            out.open("set {");
            out.line(format!(
                "guard window?.set(session: {index}, field: {field_index}, {{ newValue.encode(bern: &$0) }}) == true else {{ return }}"
            ));
            out.line(format!("{storage} = newValue"));
            out.close("}");
            out.close("}");
        }
        out.line("");
        out.open("func load(_ reader: inout BernReader) throws {");
        for field in &session.fields {
            out.line(format!(
                "bern_{} = try {}(bern: &reader)",
                field.name,
                swift_type(&field.ty)
            ));
        }
        out.close("}");
        out.close("}");
        out.line("");
    }
}

/// What a view body can refer to while generating.
#[derive(Clone, Default)]
struct Ctx {
    component: String,
    /// Swift string expression appended to control targets inside rows.
    key: Option<String>,
    /// Swift expression of the enclosing navigation path session field.
    path: Option<String>,
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
            Binding::Engine { field, path } => {
                let state = &self.app.protocol.state[*field];
                format!(
                    "bernEngine.{}{}",
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

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => value_literal(value, &expr.ty, self.types()),
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let Type::Enum { name } = &expr.ty else {
                    return "nil".into();
                };
                let def = self.types().enum_def(name).expect("enum");
                let variant_def = &def.variants[*variant as usize];
                if variant_def.fields.is_empty() {
                    format!("{name}.{}", case(&variant_def.name))
                } else {
                    let args: Vec<_> = fields
                        .iter()
                        .zip(&variant_def.fields)
                        .map(|(value, field)| {
                            format!("{}: {}", member(&field.name), self.expr(value))
                        })
                        .collect();
                    format!("{name}.{}({})", case(&variant_def.name), args.join(", "))
                }
            }
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

    /// Swift `String` expression for a text.
    fn text(&self, text: &Text) -> String {
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
                        "BernText.format({}, [{}])",
                        string(&text.key),
                        values.join(", ")
                    )
                }
            }
            Text::Dynamic { expr } => format!("BernText.text({})", self.expr(expr)),
        }
    }

    fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text(text))
    }

    fn target(&self, element: &Element, ctx: &Ctx) -> String {
        let base = format!(
            "{}.{}",
            ctx.component,
            element.id.as_deref().unwrap_or(&element.path)
        );
        match &ctx.key {
            Some(key) => format!("\"{}[\\({key})]\"", base.replace('"', "\\\"")),
            None => string(&base),
        }
    }

    fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        let session = self.app.session(&field.session)?;
        Some(format!(
            "{}.{}",
            session_var(&field.session),
            member(&session.fields[field.field].name)
        ))
    }

    fn binding_of(&self, field: &str) -> String {
        format!("SwiftUI.Binding(get: {{ {field} }}, set: {{ {field} = $0 }})")
    }

    fn action(&self, action: &Action) -> String {
        match action {
            Action::Engine { command, args } => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("from: bernWindow".into());
                format!("bernEngine.{}({})", member(&spec.name), values.join(", "))
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => {
                    format!("BernHost.shared.open({})", string(window))
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

    /// A `Video`: the shared runtime view, bound to a declaration's own
    /// player or to an application player.
    fn video(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self
            .text_prop(element, "label")
            .unwrap_or_else(|| "nil".into());
        let binding = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => {
                let value = self.expr(source);
                if matches!(source.ty, Type::Optional { .. }) {
                    format!(".source({value}?.url)")
                } else {
                    format!(".source({value}.url)")
                }
            }
            (None, Some(Prop::Player { id })) => format!(".player({})", string(id)),
            _ => ".source(nil)".into(),
        };
        let controls = element.keyword("controls") != Some("none");
        let fit = match element.keyword("fit") {
            Some("cover") => ".cover",
            Some("fill") => ".fill",
            _ => ".contain",
        };
        out.line(format!(
            "BernVideo(target: {}, label: {label}, binding: {binding}, controls: {controls}, fit: {fit})",
            self.target(element, ctx)
        ));
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node], ctx: &Ctx) {
        let mut emitted = false;
        for node in nodes {
            if matches!(node, Node::Element(element) if element.kind == "Toolbar") {
                continue;
            }
            self.node(out, node, ctx);
            emitted = true;
        }
        if !emitted {
            out.line("SwiftUI.EmptyView()");
        }
    }

    fn toolbar(&self, out: &mut Out, children: &[Node], ctx: &Ctx) {
        let items: Vec<&Element> = children
            .iter()
            .filter_map(|node| match node {
                Node::Element(element) if element.kind == "Toolbar" => Some(element),
                _ => None,
            })
            .flat_map(|toolbar| {
                toolbar.children.iter().filter_map(|node| match node {
                    Node::Element(element) => Some(element),
                    _ => None,
                })
            })
            .collect();
        if items.is_empty() {
            return;
        }
        out.open(".toolbar {");
        for item in items {
            let placement = if item.kind == "Picker" {
                ".principal"
            } else {
                ".primaryAction"
            };
            out.open(format!("SwiftUI.ToolbarItem(placement: {placement}) {{"));
            self.element(out, item, ctx);
            out.close("}");
        }
        out.close("}");
    }

    fn node(&self, out: &mut Out, node: &Node, ctx: &Ctx) {
        match node {
            Node::Element(element) => self.element(out, element, ctx),
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
                                    Some(format!("{}: {}", member(&param.name), self.expr(expr)))
                                }
                                _ => None,
                            }),
                        _ => None,
                    })
                    .collect();
                out.line(format!(
                    "{}View({})",
                    pascal(&component.name),
                    args.join(", ")
                ));
            }
            Node::If(node) => self.if_node(out, node, ctx),
            Node::IfLet(node) => self.if_let(out, node, ctx),
            Node::Match(node) => self.match_node(out, node, ctx),
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode, ctx: &Ctx) {
        for (index, (condition, body)) in node.branches.iter().enumerate() {
            let keyword = if index == 0 { "if" } else { "} else if" };
            if index == 0 {
                out.open(format!("{keyword} {} {{", self.expr(condition)));
            } else {
                out.indent -= 1;
                out.open(format!("{keyword} {} {{", self.expr(condition)));
            }
            self.nodes(out, body, ctx);
        }
        if !node.otherwise.is_empty() {
            out.indent -= 1;
            out.open("} else {");
            self.nodes(out, &node.otherwise, ctx);
        }
        out.close("}");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode, ctx: &Ctx) {
        out.open(format!(
            "if let {} = {} {{",
            member(&node.var),
            self.expr(&node.value)
        ));
        self.nodes(out, &node.then, ctx);
        if !node.otherwise.is_empty() {
            out.indent -= 1;
            out.open("} else {");
            self.nodes(out, &node.otherwise, ctx);
        }
        out.close("}");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode, ctx: &Ctx) {
        let def = self.types().enum_def(&node.enum_name).expect("enum");
        out.open(format!("switch {} {{", self.expr(&node.value)));
        for arm in &node.arms {
            match arm.variant {
                None => out.open("default:"),
                Some(index) => {
                    let variant = &def.variants[index as usize];
                    if variant.fields.is_empty() {
                        out.open(format!("case .{}:", case(&variant.name)));
                    } else {
                        let patterns: Vec<_> = variant
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(field, _)| {
                                arm.bindings
                                    .iter()
                                    .find(|binding| binding.field == field)
                                    .map(|binding| format!("let {}", member(&binding.name)))
                                    .unwrap_or_else(|| "_".into())
                            })
                            .collect();
                        out.open(format!(
                            "case .{}({}):",
                            case(&variant.name),
                            patterns.join(", ")
                        ));
                    }
                }
            }
            self.nodes(out, &arm.body, ctx);
            out.indent -= 1;
        }
        out.close("}");
    }

    fn font(&self, out: &mut Out, element: &Element) {
        if let Some(style) = element.keyword("style") {
            let font = match style {
                "large_title" => "largeTitle",
                "title" => "title",
                "title2" => "title2",
                "title3" => "title3",
                "headline" => "headline",
                "subheadline" => "subheadline",
                "callout" => "callout",
                "footnote" => "footnote",
                "caption" => "caption",
                "caption2" => "caption2",
                _ => "body",
            };
            out.line(format!(".font(.{font})"));
        }
    }

    fn element(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        match element.kind.as_str() {
            "Text" => {
                let content = self.text_prop(element, "content").unwrap_or_default();
                out.line(format!("SwiftUI.Text(verbatim: {content})"));
                self.font(out, element);
                match element.keyword("emphasis") {
                    Some("secondary") => out.line(".foregroundStyle(.secondary)"),
                    Some("tertiary") => out.line(".foregroundStyle(.tertiary)"),
                    _ => {}
                }
            }
            "Label" => {
                let title = self.text_prop(element, "title").unwrap_or_default();
                let icon = element.icon("icon").unwrap_or("circle");
                out.line(format!(
                    "SwiftUI.Label({title}, systemImage: {})",
                    string(icon)
                ));
                self.font(out, element);
            }
            "Button" | "IconButton" => self.button(out, element, ctx),
            "TextInput" => self.text_input(out, element, ctx),
            "Switch" => {
                let field = self.field(element, "value").expect("bool field");
                let label = self.text_prop(element, "label").unwrap_or_default();
                out.open(format!(
                    "SwiftUI.Toggle(isOn: {}) {{",
                    self.binding_of(&field)
                ));
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
                match element.keyword("style") {
                    Some("switch") => out.line(".toggleStyle(.switch)"),
                    Some("checkbox") => out.line(".toggleStyle(.checkbox)"),
                    _ => {}
                }
                out.line(format!(
                    ".bernControl({}, role: \"switch\", label: {label}, actions: BernControlActions(setBool: {{ {field} = $0 }}, value: {{ {field} ? \"true\" : \"false\" }}))",
                    self.target(element, ctx)
                ));
            }
            "Slider" => self.slider(out, element, ctx),
            "Video" => self.video(out, element, ctx),
            "Picker" => {
                let field_ref = element.field("value").expect("picker field");
                let field = self.field(element, "value").expect("picker field");
                let label = self.text_prop(element, "label").unwrap_or_default();
                let enum_type = swift_type(&field_ref.ty);
                out.open(format!(
                    "SwiftUI.Picker(selection: {}) {{",
                    self.binding_of(&field)
                ));
                out.open(format!(
                    "SwiftUI.ForEach({enum_type}.allCases, id: \\.self) {{ option in"
                ));
                out.line("SwiftUI.Text(verbatim: option.bernLabel).tag(option)");
                out.close("}");
                out.close("} label: {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
                match element.keyword("style") {
                    Some("menu") => out.line(".pickerStyle(.menu)"),
                    Some("segmented") => out.line(".pickerStyle(.segmented)"),
                    Some("radio") => out.line(".pickerStyle(.radioGroup)"),
                    Some("inline") => out.line(".pickerStyle(.inline)"),
                    _ => {}
                }
                out.line(format!(
                    ".bernControl({}, role: \"picker\", label: {label}, actions: BernControlActions(select: {{ name in guard let value = {enum_type}(bernName: name) else {{ return false }}; {field} = value; return true }}, value: {{ {field}.bernName }}))",
                    self.target(element, ctx)
                ));
            }
            "Form" => {
                out.open("SwiftUI.Form {");
                self.nodes(out, &element.children, ctx);
                out.close("}");
                out.line(".formStyle(.grouped)");
                if let Some(title) = self.text_prop(element, "title") {
                    out.line(format!(".navigationTitle({title})"));
                }
                self.toolbar(out, &element.children, ctx);
            }
            "Section" => {
                let title = self.text_prop(element, "title");
                let footer = self.text_prop(element, "footer");
                out.open("SwiftUI.Section {");
                self.nodes(out, &element.children, ctx);
                match (&title, &footer) {
                    (None, None) => out.close("}"),
                    _ => {
                        if let Some(title) = &title {
                            out.close("} header: {");
                            out.indent += 1;
                            out.line(format!("SwiftUI.Text(verbatim: {title})"));
                        }
                        if let Some(footer) = &footer {
                            out.close("} footer: {");
                            out.indent += 1;
                            out.line(format!("SwiftUI.Text(verbatim: {footer})"));
                        }
                        out.close("}");
                    }
                }
            }
            "Column" | "Row" => {
                let (stack, alignment) = if element.kind == "Column" {
                    (
                        "VStack",
                        match element.keyword("align") {
                            Some("center") => ".center",
                            Some("trailing") => ".trailing",
                            _ => ".leading",
                        },
                    )
                } else {
                    (
                        "HStack",
                        match element.keyword("align") {
                            Some("top") => ".top",
                            Some("bottom") => ".bottom",
                            _ => ".center",
                        },
                    )
                };
                let spacing = element
                    .number("gap")
                    .map(|gap| format!(", spacing: {}", double(gap)))
                    .unwrap_or_default();
                out.open(format!(
                    "SwiftUI.{stack}(alignment: {alignment}{spacing}) {{"
                ));
                self.nodes(out, &element.children, ctx);
                out.close("}");
                if let Some(padding) = element.number("padding") {
                    out.line(format!(".padding({})", double(padding)));
                }
                self.toolbar(out, &element.children, ctx);
            }
            "Spacer" => out.line("SwiftUI.Spacer()"),
            "Divider" => out.line("SwiftUI.Divider()"),
            "ScrollView" => {
                let axis = match element.keyword("axis") {
                    Some("horizontal") => ".horizontal",
                    _ => ".vertical",
                };
                out.open(format!("SwiftUI.ScrollView({axis}) {{"));
                self.nodes(out, &element.children, ctx);
                out.close("}");
                self.toolbar(out, &element.children, ctx);
            }
            "SplitView" => self.split_view(out, element, ctx),
            "Navigation" => self.navigation(out, element, ctx),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| self.expr(route))
                    .unwrap_or_default();
                out.open(format!("SwiftUI.NavigationLink(value: {route}) {{"));
                self.nodes(out, &element.children, ctx);
                out.close("}");
                if let Some(path) = &ctx.path {
                    out.line(format!(
                        ".bernControl({}, role: \"link\", label: \"\", actions: BernControlActions(activate: {{ {path} = {path} + [{route}] }}))",
                        self.target(element, ctx)
                    ));
                }
            }
            "List" | "ForEach" => {
                let view = if element.kind == "List" {
                    "List"
                } else {
                    "ForEach"
                };
                match (
                    element.expr("items"),
                    &element.template,
                    element.prop("key"),
                ) {
                    (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                        let var = member(&template.var);
                        out.open(format!(
                            "SwiftUI.{view}({}, id: \\.{}) {{ {var} in",
                            self.expr(items),
                            member(name)
                        ));
                        let mut inner = ctx.clone();
                        inner.key = Some(format!("{var}.{}", member(name)));
                        self.nodes(out, &element.children, &inner);
                        out.close("}");
                    }
                    _ => {
                        out.open(format!("SwiftUI.{view} {{"));
                        self.nodes(out, &element.children, ctx);
                        out.close("}");
                    }
                }
                self.toolbar(out, &element.children, ctx);
            }
            "Tabs" => self.tabs(out, element, ctx),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "Search" | "Toolbar"
            | "WindowSet" | "Window" => {
                // Realized by their parents.
            }
            other => out.line(format!("SwiftUI.EmptyView() // unsupported {other}")),
        }
    }

    fn button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_default();
        let role = match element.keyword("role") {
            Some("destructive") => "role: .destructive, ",
            Some("cancel") => "role: .cancel, ",
            _ => "",
        };
        out.open(format!("SwiftUI.Button({role}action: {{"));
        out.line(&action);
        out.close("}) {");
        out.indent += 1;
        let icon = element.icon("icon");
        match (element.kind.as_str(), icon) {
            ("IconButton", Some(icon)) => {
                out.line(format!(
                    "SwiftUI.Label({label}, systemImage: {})",
                    string(icon)
                ));
                out.line(".labelStyle(.iconOnly)");
            }
            (_, Some(icon)) => out.line(format!(
                "SwiftUI.Label({label}, systemImage: {})",
                string(icon)
            )),
            _ => out.line(format!("SwiftUI.Text(verbatim: {label})")),
        }
        out.close("}");
        if element.kind == "IconButton" {
            out.line(".buttonStyle(.borderless)");
            out.line(format!(".help({label})"));
        } else {
            match element.keyword("prominence") {
                Some("prominent") => out.line(".buttonStyle(.borderedProminent)"),
                Some("plain") => out.line(".buttonStyle(.borderless)"),
                _ => {}
            }
        }
        let enabled = element.expr("enabled").map(|expr| self.expr(expr));
        if let Some(enabled) = &enabled {
            out.line(format!(".disabled(!({enabled}))"));
        }
        out.line(format!(
            ".bernControl({}, role: \"button\", label: {label}, enabled: {}, actions: BernControlActions(activate: {{ {action} }}))",
            self.target(element, ctx),
            enabled.unwrap_or_else(|| "true".into())
        ));
    }

    fn text_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("text field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .map(|prompt| format!("SwiftUI.Text(verbatim: {prompt})"))
            .unwrap_or_else(|| "nil".into());
        let binding = self.binding_of(&field);
        let kind = element.keyword("kind").unwrap_or("plain");
        match kind {
            "secure" => out.open(format!(
                "SwiftUI.SecureField(text: {binding}, prompt: {prompt}) {{"
            )),
            "multiline" => out.open(format!(
                "SwiftUI.TextField(text: {binding}, prompt: {prompt}, axis: .vertical) {{"
            )),
            _ => out.open(format!(
                "SwiftUI.TextField(text: {binding}, prompt: {prompt}) {{"
            )),
        }
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        out.close("}");
        if kind == "multiline" {
            out.line(".lineLimit(3...8)");
        }
        let action = element.action.as_ref().map(|a| self.action(a));
        if let Some(action) = &action {
            out.line(format!(".onSubmit {{ {action} }}"));
        }
        let submit = action
            .map(|action| format!(", submit: {{ {action} }}"))
            .unwrap_or_default();
        // Secure text is never exposed to external entries.
        let value = if kind == "secure" {
            String::new()
        } else {
            format!(", value: {{ {field} }}")
        };
        out.line(format!(
            ".bernControl({}, role: \"text_input\", label: {label}, actions: BernControlActions(edit: {{ {field} = $0 }}{submit}{value}))",
            self.target(element, ctx)
        ));
    }

    fn slider(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("number field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        let step = element
            .number("step")
            .map(|step| format!(", step: {}", double(step)))
            .unwrap_or_default();
        let action = element.action.as_ref().map(|a| self.action(a));
        out.open(format!(
            "SwiftUI.Slider(value: {}, in: {min}...{max}{step}) {{",
            self.binding_of(&field)
        ));
        out.line(format!("SwiftUI.Text(verbatim: {label})"));
        let (min_icon, max_icon) = (element.icon("min_icon"), element.icon("max_icon"));
        if min_icon.is_some() || max_icon.is_some() {
            out.close("} minimumValueLabel: {");
            out.indent += 1;
            match min_icon {
                Some(icon) => out.line(format!("SwiftUI.Image(systemName: {})", string(icon))),
                None => out.line("SwiftUI.EmptyView()"),
            }
            out.close("} maximumValueLabel: {");
            out.indent += 1;
            match max_icon {
                Some(icon) => out.line(format!("SwiftUI.Image(systemName: {})", string(icon))),
                None => out.line("SwiftUI.EmptyView()"),
            }
        }
        out.close("} onEditingChanged: { editing in");
        out.indent += 1;
        match &action {
            Some(action) => out.line(format!("if !editing {{ {action} }}")),
            None => out.line("_ = editing"),
        }
        out.close("}");
        let commit = action
            .map(|action| format!("; {action}"))
            .unwrap_or_default();
        out.line(format!(
            ".bernControl({}, role: \"slider\", label: {label}, actions: BernControlActions(setNumber: {{ value in {field} = value{commit} }}, value: {{ BernText.text({field}) }}))",
            self.target(element, ctx)
        ));
    }

    fn split_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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
        out.open("SwiftUI.NavigationSplitView {");
        out.open(format!(
            "SwiftUI.List(selection: SwiftUI.Binding(get: {{ Optional({selection}) }}, set: {{ if let value = $0 {{ {selection} = value }} }})) {{"
        ));
        for node in &sidebar.children {
            if let Node::Element(child) = node {
                self.sidebar_child(out, child);
            }
        }
        out.close("}");
        out.line(".navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 320)");
        let items = sidebar_items(sidebar);
        if let Some(search) = sidebar.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "Search" => Some(child),
            _ => None,
        }) {
            let field = self.field(search, "value").expect("search field");
            let prompt = self.text_prop(search, "prompt");
            let prompt_arg = prompt
                .as_ref()
                .map(|prompt| format!(", prompt: SwiftUI.Text(verbatim: {prompt})"))
                .unwrap_or_default();
            out.line(format!(
                ".searchable(text: {}, placement: .sidebar{prompt_arg})",
                self.binding_of(&field)
            ));
            let action = search.action.as_ref().map(|a| self.action(a));
            if let Some(action) = &action {
                out.line(format!(".onSubmit(of: .search) {{ {action} }}"));
            }
            let submit = action
                .map(|action| format!(", submit: {{ {action} }}"))
                .unwrap_or_default();
            out.line(format!(
                ".bernControl({}, role: \"search\", label: {}, actions: BernControlActions(edit: {{ {field} = $0 }}{submit}, value: {{ {field} }}))",
                self.target(search, ctx),
                prompt.unwrap_or_else(|| "\"\"".into())
            ));
        }
        if sidebar.id.is_some() {
            let options: Vec<String> = items
                .iter()
                .filter_map(|(value, _)| value.map(|value| self.expr(value)))
                .collect();
            out.line(format!(
                ".bernControl({}, role: \"sidebar\", label: \"\", actions: BernControlActions(select: {{ name in guard let value = [{}].first(where: {{ $0.bernName == name }}) else {{ return false }}; {selection} = value; return true }}, value: {{ {selection}.bernName }}))",
                self.target(sidebar, ctx),
                options.join(", ")
            ));
        }
        out.close("} detail: {");
        out.indent += 1;
        out.open("SwiftUI.Group {");
        self.nodes(out, &detail.children, ctx);
        out.close("}");
        // The detail column is titled by the selected sidebar destination,
        // as macOS sidebar apps are.
        let titled: Vec<_> = items
            .iter()
            .filter_map(|(value, label)| Some((self.expr((*value)?), self.text((*label)?))))
            .collect();
        if !titled.is_empty() {
            out.open(".navigationTitle({ () -> String in");
            for (value, label) in &titled {
                out.line(format!("if {selection} == {value} {{ return {label} }}"));
            }
            out.line("return \"\"");
            out.close("}())");
        }
        self.toolbar(out, &detail.children, ctx);
        out.close("}");
    }

    fn sidebar_child(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Section" => {
                let title = self.text_prop(element, "title");
                out.open("SwiftUI.Section {");
                for node in &element.children {
                    if let Node::Element(child) = node {
                        self.sidebar_child(out, child);
                    }
                }
                match title {
                    Some(title) => {
                        out.close("} header: {");
                        out.indent += 1;
                        out.line(format!("SwiftUI.Text(verbatim: {title})"));
                        out.close("}");
                    }
                    None => out.close("}"),
                }
            }
            "SidebarItem" => {
                let label = self.text_prop(element, "label").unwrap_or_default();
                let value = element
                    .expr("value")
                    .map(|v| self.expr(v))
                    .unwrap_or_default();
                match element.icon("icon") {
                    Some(icon) => out.line(format!(
                        "SwiftUI.Label({label}, systemImage: {}).tag({value})",
                        string(icon)
                    )),
                    None => out.line(format!("SwiftUI.Text(verbatim: {label}).tag({value})")),
                }
            }
            _ => {}
        }
    }

    fn navigation(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let path = self.field(element, "path").expect("path field");
        let route_type = match element.field("path").map(|field| &field.ty) {
            Some(Type::List { inner }) => swift_type(inner),
            _ => "Never".into(),
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
        let mut inner = ctx.clone();
        inner.path = Some(path.clone());
        out.open(format!(
            "SwiftUI.NavigationStack(path: {}) {{",
            self.binding_of(&path)
        ));
        out.open("SwiftUI.Group {");
        self.nodes(out, &content, &inner);
        out.close("}");
        if let Some(destination) = destination {
            let var = destination
                .template
                .as_ref()
                .map(|template| member(&template.var))
                .unwrap_or_else(|| "route".into());
            out.open(format!(
                ".navigationDestination(for: {route_type}.self) {{ {var} in"
            ));
            self.nodes(out, &destination.children, &inner);
            out.close("}");
        }
        self.toolbar(out, &content, &inner);
        out.close("}");
        if element.id.is_some() {
            out.line(format!(
                ".bernControl({}, role: \"navigation\", label: \"\", actions: BernControlActions(back: {{ guard !{path}.isEmpty else {{ return false }}; {path}.removeLast(); return true }}, value: {{ String({path}.count) }}))",
                self.target(element, ctx)
            ));
        }
    }

    fn tabs(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let selection = self.field(element, "selection").expect("selection field");
        out.open(format!(
            "SwiftUI.TabView(selection: {}) {{",
            self.binding_of(&selection)
        ));
        let mut options = Vec::new();
        for node in &element.children {
            let Node::Element(tab) = node else { continue };
            let label = self.text_prop(tab, "label").unwrap_or_default();
            let value = tab.expr("value").map(|v| self.expr(v)).unwrap_or_default();
            options.push(value.clone());
            out.open("SwiftUI.Group {");
            self.nodes(out, &tab.children, ctx);
            out.close("}");
            out.open(".tabItem {");
            match tab.icon("icon") {
                Some(icon) => out.line(format!(
                    "SwiftUI.Label({label}, systemImage: {})",
                    string(icon)
                )),
                None => out.line(format!("SwiftUI.Text(verbatim: {label})")),
            }
            out.close("}");
            out.line(format!(".tag({value})"));
            self.toolbar(out, &tab.children, ctx);
        }
        out.close("}");
        if element.id.is_some() {
            out.line(format!(
                ".bernControl({}, role: \"tabs\", label: \"\", actions: BernControlActions(select: {{ name in guard let value = [{}].first(where: {{ $0.bernName == name }}) else {{ return false }}; {selection} = value; return true }}, value: {{ {selection}.bernName }}))",
                self.target(element, ctx),
                options.join(", ")
            ));
        }
    }

    fn uses_engine(&self, nodes: &[&Node]) -> bool {
        let mut uses = false;
        for node in nodes {
            node.walk(&mut |node| {
                if let Node::Element(element) = node
                    && matches!(element.action, Some(Action::Engine { .. }))
                {
                    uses = true;
                }
            });
        }
        uses
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        let name = format!("{}View", pascal(&component.name));
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!("struct {name}: View {{"));
        let mut sessions = Vec::new();
        let mut client = false;
        for param in &component.params {
            match &param.kind {
                ParamKind::Value { ty } => {
                    out.line(format!("let {}: {}", member(&param.name), swift_type(ty)))
                }
                ParamKind::Client => client = true,
                ParamKind::Session { session } => {
                    if !sessions.contains(session) {
                        sessions.push(session.clone());
                    }
                }
                ParamKind::Windows | ParamKind::Players => {}
            }
        }
        if client || self.uses_engine(&[&component.body]) {
            out.line("@Environment(BernEngine.self) private var bernEngine");
        }
        for session in &sessions {
            out.line(format!(
                "@Environment({}.self) private var {}",
                session_class(session),
                session_var(session)
            ));
        }
        out.line("@Environment(BernWindowContext.self) private var bernWindow");
        out.line("");
        out.open("var body: some View {");
        let ctx = Ctx {
            component: component.name.clone(),
            ..Ctx::default()
        };
        self.node(out, &component.body, &ctx);
        out.close("}");
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, index: usize, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!("struct BernWindow{index}: View {{"));
        let sessions: Vec<String> = window
            .sessions
            .iter()
            .map(|session| format!("{}()", session_class(session)))
            .collect();
        out.line(format!(
            "@State private var context = BernWindowContext(kind: {index}, kindID: {}, isSettings: {}, sessions: [{}])",
            string(&window.id),
            window.role == WindowRole::Settings,
            sessions.join(", ")
        ));
        out.line("@Environment(BernEngine.self) private var bernEngine");
        out.line("");
        out.open("var body: some View {");
        out.open("BernWindowRoot(context: context) {");
        let ctx = Ctx {
            component: window.id.clone(),
            ..Ctx::default()
        };
        self.node(out, &window.content, &ctx);
        out.close("}");
        for session in &window.sessions {
            out.line(format!(
                ".environment(context.session({}.self))",
                session_class(session)
            ));
        }
        let frame: Vec<String> = [
            window.min_width.map(|v| format!("minWidth: {}", double(v))),
            window
                .min_height
                .map(|v| format!("minHeight: {}", double(v))),
        ]
        .into_iter()
        .flatten()
        .collect();
        if window.role == WindowRole::Settings {
            if let Some(width) = window.width {
                out.line(format!(".frame(width: {})", double(width)));
            }
            // macOS settings windows fit their content and resize per tab.
            out.line(".scrollDisabled(true)");
            out.line(".fixedSize(horizontal: false, vertical: true)");
        } else if !frame.is_empty() {
            out.line(format!(".frame({})", frame.join(", ")));
        }
        if window.close.is_some() {
            out.line(".onAppear { context.closeCheck = bernCloseCheck() }");
        }
        out.close("}");
        if let Some(close) = &window.close {
            self.close_check(out, close);
        }
        out.close("}");
        out.line("");
    }

    /// Lines that bind the sessions and engine an expression reads.
    fn locals(&self, out: &mut Out, exprs: &[&Expr]) {
        let mut sessions = Vec::new();
        for expr in exprs {
            for session in expr.sessions() {
                if !sessions.contains(&session) {
                    sessions.push(session);
                }
            }
        }
        for session in sessions {
            out.line(format!(
                "let {} = context.session({}.self)",
                session_var(&session),
                session_class(&session)
            ));
        }
    }

    /// The window's close check: evaluated against its own drafts when
    /// closing is requested, with the declared save command.
    fn close_check(&self, out: &mut Out, close: &bern_ir::CloseCheck) {
        let engine = close.exprs().iter().any(|expr| expr.reads_engine())
            || matches!(close.save, Some(Action::Engine { .. }));
        out.line("");
        out.open("private func bernCloseCheck() -> BernCloseCheck {");
        out.line("let context = self.context");
        if engine {
            out.line("let bernEngine = self.bernEngine");
        }
        out.open("return BernCloseCheck(");
        out.open("needsConfirmation: {");
        self.locals(out, &[&close.when]);
        out.line(format!("return {}", self.expr(&close.when)));
        out.close("},");
        out.open("message: {");
        self.locals(out, &close.message.exprs());
        out.line(format!("return {}", self.text(&close.message)));
        out.close("},");
        match &close.save {
            Some(Action::Engine { command, args }) => {
                let spec = &self.app.protocol.commands[*command];
                out.open("save: { done in");
                self.locals(out, &args.iter().collect::<Vec<_>>());
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("from: context".into());
                values.push("completion: done".into());
                out.line(format!(
                    "bernEngine.{}({})",
                    member(&spec.name),
                    values.join(", ")
                ));
                out.close("}");
            }
            _ => out.line("save: nil"),
        }
        out.close(")");
        out.close("}");
    }

    pub fn generate(&self, fingerprint: u64) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("");
        out.line("import AppKit");
        out.line("import SwiftUI");
        out.line("");
        out.line("// MARK: - Values");
        out.line("");
        for def in &app.types.enums {
            generate_enum(&mut out, def);
        }
        for def in &app.types.records {
            generate_record(&mut out, def);
        }
        out.line("// MARK: - Engine state");
        out.line("");
        generate_engine(&mut out, app);
        out.line("// MARK: - Session state");
        out.line("");
        generate_sessions(&mut out, app);
        out.line("// MARK: - Components");
        out.line("");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// MARK: - Windows");
        out.line("");
        let mut order: Vec<usize> = (0..app.windows.windows.len()).collect();
        order.sort_by_key(|index| {
            let window = &app.windows.windows[*index];
            (
                window.id != app.windows.primary,
                window.role == WindowRole::Settings,
            )
        });
        for (index, window) in app.windows.windows.iter().enumerate() {
            self.window(&mut out, index, window);
        }
        let kinds: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                format!(
                    "BernWindowKind(id: {}, isSettings: {})",
                    string(&window.id),
                    window.role == WindowRole::Settings
                )
            })
            .collect();
        let initial: Vec<String> = app
            .windows
            .windows
            .iter()
            .filter(|window| {
                window.initial == bern_ir::WindowInitial::Open
                    && window.id != app.windows.primary
                    && window.role != WindowRole::Settings
            })
            .map(|window| string(&window.id))
            .collect();
        out.line("@main");
        out.open("struct BernApplication: App {");
        out.line("@NSApplicationDelegateAdaptor(BernAppDelegate.self) private var delegate");
        out.line("");
        out.open("init() {");
        out.open(format!(
            "BernHost.shared.start(fingerprint: 0x{fingerprint:016x}, kinds: [{}]) {{ field, reader in",
            kinds.join(", ")
        ));
        out.line("try BernEngine.shared.bernApply(field, &reader)");
        out.close("}");
        out.close("}");
        out.line("");
        out.open("var body: some Scene {");
        for index in order {
            let window = &app.windows.windows[index];
            let title = format!("BernText.string({})", string(&window.title.key));
            if window.role == WindowRole::Settings {
                out.open("SwiftUI.Settings {");
                out.line(format!("BernWindow{index}()"));
                out.line("    .environment(BernEngine.shared)");
                out.close("}");
            } else {
                out.open(format!(
                    "SwiftUI.Window(SwiftUI.Text(verbatim: {title}), id: {}) {{",
                    string(&window.id)
                ));
                out.line(format!("BernWindow{index}()"));
                out.line("    .environment(BernEngine.shared)");
                if window.id == app.windows.primary && !initial.is_empty() {
                    out.line(format!(
                        "    .task {{ BernHost.shared.openInitial([{}]) }}",
                        initial.join(", ")
                    ));
                }
                out.close("}");
                if let (Some(width), Some(height)) = (window.width, window.height) {
                    out.line(format!(
                        ".defaultSize(width: {}, height: {})",
                        double(width),
                        double(height)
                    ));
                }
                if window.min_width.is_some() || window.min_height.is_some() {
                    out.line(".windowResizability(.contentMinSize)");
                }
            }
        }
        out.close("}");
        out.close("}");
        out.finish()
    }
}

/// `(value, label)` of every sidebar item, in declaration order.
fn sidebar_items(sidebar: &Element) -> Vec<(Option<&Expr>, Option<&Text>)> {
    let mut items = Vec::new();
    fn visit<'e>(element: &'e Element, items: &mut Vec<(Option<&'e Expr>, Option<&'e Text>)>) {
        for node in &element.children {
            if let Node::Element(child) = node {
                if child.kind == "SidebarItem" {
                    items.push((child.expr("value"), child.text("label")));
                } else {
                    visit(child, items);
                }
            }
        }
    }
    visit(sidebar, &mut items);
    items
}
