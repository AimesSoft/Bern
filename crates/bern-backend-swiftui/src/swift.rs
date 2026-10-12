//! Swift source generation from the IR.
//!
//! Every component becomes a SwiftUI `View` built from standard containers
//! and controls with system styles. Nothing here sets a color, a material or
//! a version-gated API: appearance comes from the SDK and the running macOS.

use crate::names::{case, double, member, pascal, string};
use bern_ir::{
    Action, Application, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind, IfLetNode,
    IfNode, MatchArm, MatchNode, MediaOp, Node, ParamKind, Prop, Text, WindowDef, WindowOp,
    WindowRole,
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
            member(&app.protocol.command_symbol(command)),
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

/// The system the Swift is generated for. SwiftUI is shared; windows,
/// scenes and a few control styles are not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Ios,
}

pub struct Generator<'a> {
    app: &'a Application,
    platform: Platform,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application, platform: Platform) -> Self {
        Self { app, platform }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    /// The platform's name in `style!`.
    fn style_platform(&self) -> &'static str {
        match self.platform {
            Platform::Macos => "macos",
            Platform::Ios => "ios",
        }
    }

    /// The style of a control from `style!`, if any.
    fn style_of(&self, element: &Element) -> Option<crate::style::Resolved> {
        crate::style::resolve(
            self.app,
            self.style_platform(),
            &element.kind,
            element.keyword("variant"),
        )
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

    /// A `Canvas`: its drawing runs on values captured when the view is
    /// built, so state the drawing reads still updates the view.
    fn canvas(&self, out: &mut Out, element: &Element) {
        let mut captures = Vec::new();
        let mut body = Out {
            indent: out.indent + 1,
            ..Out::default()
        };
        self.drawing(&mut body, &element.children, &mut captures);
        let list = if captures.is_empty() {
            String::new()
        } else {
            format!(
                "[{}] ",
                captures
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("v{index} = {value}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        out.line(format!("SwiftUI.Canvas {{ {list}context, size in"));
        out.lines.extend(body.lines);
        out.line("}");
        let length = |name| {
            element
                .number(name)
                .map(double)
                .unwrap_or_else(|| "nil".into())
        };
        let (width, height) = (length("width"), length("height"));
        out.line(format!(
            ".frame(width: {width}, height: {height}){}",
            match (element.number("width"), element.number("height")) {
                (None, None) => ".frame(maxWidth: .infinity, maxHeight: .infinity)",
                (None, Some(_)) => ".frame(maxWidth: .infinity)",
                (Some(_), None) => ".frame(maxHeight: .infinity)",
                _ => "",
            }
        ));
        match self.text_prop(element, "label") {
            Some(label) => {
                out.line(".accessibilityElement(children: .ignore)");
                out.line(".accessibilityAddTraits(.isImage)");
                out.line(format!(
                    ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
                ));
            }
            None => out.line(".accessibilityHidden(true)"),
        }
    }

    fn drawing(&self, out: &mut Out, nodes: &[Node], captures: &mut Vec<String>) {
        fn capture(captures: &mut Vec<String>, value: String) -> String {
            captures.push(value);
            format!("v{}", captures.len() - 1)
        }
        let color = |element: &Element, name: &str| {
            element
                .keyword(name)
                .map(|role| format!(".role({})", string(role)))
                .unwrap_or_else(|| "nil".into())
        };
        let number = |element: &Element, name: &str, default: f64| {
            double(element.number(name).unwrap_or(default))
        };
        for node in nodes {
            // A canvas holds its drawing elements directly.
            let Node::Element(element) = node else {
                continue;
            };
            match element.kind.as_str() {
                "Shape" => {
                    let at = |name: &str, extent: &str, origin: &str| match element.number(name) {
                        Some(value) => double(value),
                        None => format!("size.{extent} - {origin}"),
                    };
                    let x = number(element, "x", 0.0);
                    let y = number(element, "y", 0.0);
                    out.line(format!(
                            "BernDraw.shape(&context, {}, CGRect(x: {x}, y: {y}, width: {}, height: {}), corner: {}, fill: {}, stroke: {}, strokeWidth: {}, opacity: {})",
                            string(element.keyword("kind").unwrap_or("rect")),
                            at("width", "width", &x),
                            at("height", "height", &y),
                            number(element, "corner", 8.0),
                            color(element, "fill"),
                            color(element, "stroke"),
                            number(element, "stroke_width", 1.0),
                            number(element, "opacity", 1.0),
                        ));
                }
                "Series" => {
                    let values = element.expr("values").expect("series values");
                    let list = match &values.ty {
                        Type::List { inner } if matches!(**inner, Type::Int) => {
                            format!("({}).map {{ Double($0) }}", self.expr(values))
                        }
                        _ => self.expr(values),
                    };
                    let values = capture(captures, list);
                    let max = match element.expr("max") {
                        Some(max) => capture(captures, format!("Double({})", self.expr(max))),
                        None => "nil".into(),
                    };
                    out.line(format!(
                            "BernDraw.series(&context, size, values: {values}, max: {max}, area: {}, fill: {}, stroke: {}, strokeWidth: {}, opacity: {})",
                            element.keyword("kind") == Some("area"),
                            color(element, "fill"),
                            color(element, "stroke"),
                            number(element, "stroke_width", 2.0),
                            number(element, "opacity", 1.0),
                        ));
                }
                "Pattern" => out.line(format!(
                    "BernDraw.pattern(&context, size, BernPatterns.`{}`, opacity: {})",
                    element.keyword("name").expect("pattern name"),
                    number(element, "opacity", 1.0),
                )),
                _ => {}
            }
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
                format!(
                    "bernEngine.{}({})",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                )
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

    /// A `Query`: the runtime view owns the subscription and shows the
    /// loading, refreshing, failed and empty states; the template shows
    /// the data. Its parameters are encoded on every render; the runtime
    /// requests them only when they change.
    fn query(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let data = swift_type(&binding.result);
        let shown = swift_type(&template.ty);
        let params = if binding.args.is_empty() {
            "[]".to_string()
        } else {
            let encode: Vec<_> = binding
                .args
                .iter()
                .map(|arg| format!("({}).encode(bern: &writer)", self.expr(arg)))
                .collect();
            format!(
                "{{ var writer = BernWriter(); {}; return writer.bytes }}()",
                encode.join("; ")
            )
        };
        let framework = |key: &str| format!("BernText.string({})", string(key));
        let loading = self
            .text_prop(element, "loading")
            .unwrap_or_else(|| framework("bern.query.loading"));
        let failed = self
            .text_prop(element, "failed")
            .unwrap_or_else(|| framework("bern.query.failed"));
        let empty = self.text_prop(element, "empty");
        let icon = element.icon("icon").unwrap_or("tray");
        // The declared empty rule: an empty list or a missing optional.
        let (shown_body, count) = match &binding.result {
            Type::List { .. } if empty.is_some() => (
                "data.isEmpty ? nil : data",
                format!("{{ (data: {data}) in data.count }}"),
            ),
            Type::List { .. } => ("data", format!("{{ (data: {data}) in data.count }}")),
            Type::Optional { .. } => (
                "data",
                format!("{{ (data: {data}) in data == nil ? 0 : 1 }}"),
            ),
            _ => ("data", "nil".to_string()),
        };
        out.open("BernQueryView(");
        out.line(format!(
            "query: {}, target: {},",
            binding.query,
            self.target(element, ctx)
        ));
        out.line(format!("params: {params},"));
        out.line(format!(
            "loading: {loading}, empty: {}, emptyIcon: {}, failed: {failed},",
            empty.unwrap_or_else(|| "nil".into()),
            string(icon)
        ));
        out.line(format!(
            "count: {count}, shown: {{ (data: {data}) -> {shown}? in {shown_body} }}"
        ));
        out.close(format!(") {{ {} in", member(&template.var)));
        out.indent += 1;
        self.nodes(out, &element.children, ctx);
        out.close("}");
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
        let toolbars: Vec<&Element> = children
            .iter()
            .filter_map(|node| match node {
                Node::Element(element) if element.kind == "Toolbar" => Some(element),
                _ => None,
            })
            .filter(|toolbar| !toolbar.children.is_empty())
            .collect();
        if toolbars.is_empty() {
            return;
        }
        out.open(".toolbar {");
        for toolbar in toolbars {
            self.nav_walk(
                out,
                &toolbar.children,
                &|out, item| {
                    let placement = if item.kind == "Picker" {
                        ".principal"
                    } else {
                        ".primaryAction"
                    };
                    out.open(format!("SwiftUI.ToolbarItem(placement: {placement}) {{"));
                    self.element(out, item, ctx);
                    out.close("}");
                },
                Some(ctx),
            );
        }
        out.close("}");
    }

    /// Whether an element moves when it appears, disappears or replaces a
    /// shared element: its container then animates the change.
    fn moves(element: &Element) -> bool {
        ["appear", "disappear", "hero"]
            .iter()
            .any(|prop| element.keyword(prop).is_some())
    }

    /// The spring a container animates its moving children with.
    fn children_motion(&self, nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            Node::Element(element) if Self::moves(element) => {
                let transition = element
                    .keyword("appear")
                    .or(element.keyword("disappear"))
                    .and_then(|name| crate::style::transition(self.app, name));
                Some(match transition {
                    Some((_, motion)) => motion,
                    None => crate::style::motion(self.app, "hero"),
                })
            }
            _ => None,
        })
    }

    /// What changes when children appear, disappear or reorder: their
    /// conditions and list identities, for `.animation(_:value:)`.
    fn change_key(&self, nodes: &[Node]) -> Option<String> {
        let mut parts = Vec::new();
        for node in nodes {
            match node {
                Node::If(node) => {
                    for (condition, _) in &node.branches {
                        parts.push(format!("AnyHashable({})", self.expr(condition)));
                    }
                }
                Node::Element(element) if element.kind == "ForEach" || element.kind == "List" => {
                    if let (Some(items), Some(Prop::Key { name, .. })) =
                        (element.expr("items"), element.prop("key"))
                    {
                        parts.push(format!(
                            "AnyHashable(({}).map(\\.{}))",
                            self.expr(items),
                            member(name)
                        ));
                    }
                }
                _ => {}
            }
        }
        (!parts.is_empty()).then(|| format!("[{}]", parts.join(", ")))
    }

    /// An element's transitions, shared element and reflow.
    fn motion_modifiers(&self, out: &mut Out, element: &Element) {
        let appear = element
            .keyword("appear")
            .and_then(|name| crate::style::transition(self.app, name));
        let disappear = element
            .keyword("disappear")
            .and_then(|name| crate::style::transition(self.app, name));
        if appear.is_some() || disappear.is_some() {
            let part = |transition: &Option<(String, String)>| match transition {
                Some((swift, motion)) => (format!("AnyTransition{swift}"), motion.clone()),
                None => ("nil".into(), "nil".into()),
            };
            let (insertion, insertion_motion) = part(&appear);
            let (removal, removal_motion) = part(&disappear);
            out.line(format!(
                ".bernTransition(insertion: {insertion}, insertionMotion: {insertion_motion}, removal: {removal}, removalMotion: {removal_motion})"
            ));
        }
        if let Some(hero) = element.keyword("hero") {
            match element.expr("hero_key") {
                Some(key) => out.line(format!(".bernHero(\"{hero}-\\({})\")", self.expr(key))),
                None => out.line(format!(".bernHero({})", string(hero))),
            }
        }
        if let Some(token) = element.keyword("reflow")
            && element.kind != "ForEach"
            && element.kind != "List"
            && let Some(key) = self.change_key(&element.children)
        {
            out.line(format!(
                ".animation({}.animation, value: {key})",
                crate::style::motion(self.app, token)
            ));
        }
    }

    fn node(&self, out: &mut Out, node: &Node, ctx: &Ctx) {
        match node {
            Node::Element(element) => {
                self.element(out, element, ctx);
                self.motion_modifiers(out, element);
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
        // Branches whose elements move animate the change of condition.
        let motion = node
            .branches
            .iter()
            .map(|(_, body)| body)
            .chain(std::iter::once(&node.otherwise))
            .find_map(|body| self.children_motion(body));
        if motion.is_some() {
            out.open("SwiftUI.Group {");
        }
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
        if let Some(motion) = motion {
            out.close("}");
            let conditions: Vec<String> = node
                .branches
                .iter()
                .map(|(condition, _)| self.expr(condition))
                .collect();
            out.line(format!(
                ".animation({motion}.animation, value: [{}])",
                conditions.join(", ")
            ));
        }
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
            out.open(format!("{}:", case_pattern(def, arm)));
            self.nodes(out, &arm.body, ctx);
            out.indent -= 1;
        }
        out.close("}");
    }

    /// The text a row shows first, as a Swift `String` expression: the
    /// first `Text` or `Label` in reading order, through containers and
    /// `match` arms. Links use it as their name in `ui.inspect`.
    fn row_label(&self, nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            Node::Element(element) => match element.kind.as_str() {
                "Text" => self.text_prop(element, "content"),
                "Label" => self.text_prop(element, "title"),
                _ => self.row_label(&element.children),
            },
            Node::Match(node) => {
                let def = self.types().enum_def(&node.enum_name)?;
                let arms = node
                    .arms
                    .iter()
                    .map(|arm| {
                        let label = self.row_label(&arm.body)?;
                        Some(format!("{}: return {label}", case_pattern(def, arm)))
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "{{ () -> String in switch {} {{ {} }} }}()",
                    self.expr(&node.value),
                    arms.join("; ")
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// The label of the selected option, as a Swift `String` expression.
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
                if let Some(role) = element.keyword("mark") {
                    out.line(format!(".bernMark(.role({}))", string(role)));
                }
            }
            "Canvas" => self.canvas(out, element),
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
                // iOS has no checkboxes; its toggles are switches.
                match (element.keyword("style"), self.platform) {
                    (Some("switch"), _) => out.line(".toggleStyle(.switch)"),
                    (Some("checkbox"), Platform::Macos) => out.line(".toggleStyle(.checkbox)"),
                    _ => {}
                }
                out.line(format!(
                    ".bernControl({}, role: \"switch\", label: {label}, actions: BernControlActions(setBool: {{ {field} = $0 }}, value: {{ {field} ? \"true\" : \"false\" }}))",
                    self.target(element, ctx)
                ));
            }
            "Slider" => self.slider(out, element, ctx),
            "Video" => self.video(out, element, ctx),
            "Query" => self.query(out, element, ctx),
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
                    // iOS lists the options with a checkmark instead of
                    // radio buttons.
                    Some("radio") if self.platform == Platform::Ios => {
                        out.line(".pickerStyle(.inline)")
                    }
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
            "Grid" => {
                let gap = double(element.number("gap").unwrap_or(8.0));
                out.open(format!(
                    "SwiftUI.LazyVGrid(columns: [SwiftUI.GridItem(.adaptive(minimum: {}), spacing: {gap})], alignment: .leading, spacing: {gap}) {{",
                    double(element.number("min_width").unwrap_or(120.0))
                ));
                self.nodes(out, &element.children, ctx);
                out.close("}");
                if let Some(padding) = element.number("padding") {
                    out.line(format!(".padding({})", double(padding)));
                }
            }
            "Stack" => {
                let alignment = match element.keyword("align") {
                    Some("top_leading") => ".topLeading",
                    Some("top") => ".top",
                    Some("top_trailing") => ".topTrailing",
                    Some("leading") => ".leading",
                    Some("trailing") => ".trailing",
                    Some("bottom_leading") => ".bottomLeading",
                    Some("bottom") => ".bottom",
                    Some("bottom_trailing") => ".bottomTrailing",
                    _ => ".center",
                };
                out.open(format!("SwiftUI.ZStack(alignment: {alignment}) {{"));
                self.nodes(out, &element.children, ctx);
                out.close("}");
            }
            "Image" => {
                let fit = match element.keyword("fit") {
                    Some("cover") => ".cover",
                    Some("fill") => ".fill",
                    _ => ".contain",
                };
                let length = |name| {
                    element
                        .number(name)
                        .map(double)
                        .unwrap_or_else(|| "nil".into())
                };
                out.line(format!(
                    "BernImage(name: {}, label: {}, width: {}, height: {}, fit: {fit})",
                    string(element.resource("source").unwrap_or_default()),
                    self.text_prop(element, "label")
                        .unwrap_or_else(|| "nil".into()),
                    length("width"),
                    length("height"),
                ));
            }
            "Progress" => {
                let label = self.text_prop(element, "label");
                let head = match element.expr("value") {
                    Some(value) => {
                        let total = double(element.number("total").unwrap_or(1.0));
                        format!(
                            "SwiftUI.ProgressView(value: min(max(Double({}), 0), {total}), total: {total})",
                            self.expr(value)
                        )
                    }
                    None => "SwiftUI.ProgressView()".into(),
                };
                match label {
                    Some(label) => {
                        out.open(format!("{} {{", head.trim_end_matches("()")));
                        out.line(format!("SwiftUI.Text(verbatim: {label})"));
                        out.close("}");
                    }
                    None => out.line(head),
                }
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
                        ".bernControl({}, role: \"link\", label: {}, actions: BernControlActions(activate: {{ {path} = {path} + [{route}] }}))",
                        self.target(element, ctx),
                        self.row_label(&element.children)
                            .unwrap_or_else(|| "\"\"".into())
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
                        // Items that move, or a list that reflows, animate
                        // when the list changes.
                        let motion = self.children_motion(&element.children).or_else(|| {
                            element
                                .keyword("reflow")
                                .map(|token| crate::style::motion(self.app, token))
                        });
                        if let Some(motion) = motion {
                            out.line(format!(
                                ".animation({motion}.animation, value: ({}).map(\\.{}))",
                                self.expr(items),
                                member(name)
                            ));
                        }
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
            "Adaptive" => self.adaptive(out, element, ctx),
            "Menu" => self.menu(out, element, ctx),
            "LanguagePicker" => {
                let label = self.text_prop(element, "label").unwrap_or_default();
                out.open(
                    "SwiftUI.Picker(selection: SwiftUI.Binding(get: { BernLanguage.shared.current }, set: { BernLanguage.shared.select($0) })) {",
                );
                out.open(
                    "SwiftUI.ForEach(BernLanguage.shared.supported, id: \\.self) { language in",
                );
                out.line("SwiftUI.Text(verbatim: BernLanguage.autonym(language)).tag(language)");
                out.close("}");
                out.close("} label: {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
                out.line(format!(
                    ".bernControl({}, role: \"picker\", label: {label}, actions: BernControlActions(select: {{ BernLanguage.shared.select($0) }}, value: {{ BernLanguage.shared.current }}))",
                    self.target(element, ctx)
                ));
            }
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "Search" | "Toolbar"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" => {
                // Realized by their parents.
            }
            other => out.line(format!("SwiftUI.EmptyView() // unsupported {other}")),
        }
    }

    /// One closure per layout size; `BernAdaptive` picks one by the
    /// available width and the breakpoint from `[layouts]`.
    fn adaptive(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let breakpoint = self
            .app
            .layouts
            .map(|layouts| layouts.compact_below)
            .unwrap_or(0.0);
        let target = if element.id.is_some() {
            self.target(element, ctx)
        } else {
            "nil".into()
        };
        let layout = |size: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size) => Some(layout),
                _ => None,
            })
        };
        out.open(format!(
            "BernAdaptive(breakpoint: {breakpoint:?}, target: {target}, compact: {{"
        ));
        if let Some(compact) = layout("compact") {
            self.nodes(out, &compact.children, ctx);
        }
        out.close("}, expanded: {");
        out.indent += 1;
        if let Some(expanded) = layout("expanded") {
            self.nodes(out, &expanded.children, ctx);
        }
        out.close("})");
    }

    /// A menu of command items. The items are registered on the menu button,
    /// so external entries reach them whether the menu is open or not.
    fn menu(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let items: Vec<&Element> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(item) => Some(item),
                _ => None,
            })
            .collect();
        out.open("SwiftUI.Menu {");
        for item in &items {
            if item.kind == "Divider" {
                out.line("SwiftUI.Divider()");
                continue;
            }
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let action = item
                .action
                .as_ref()
                .map(|a| self.action(a))
                .unwrap_or_default();
            let role = if item.keyword("role") == Some("destructive") {
                "role: .destructive, "
            } else {
                ""
            };
            out.open(format!("SwiftUI.Button({role}action: {{"));
            out.line(&action);
            out.close("}) {");
            out.indent += 1;
            match item.icon("icon") {
                Some(icon) => out.line(format!(
                    "SwiftUI.Label({item_label}, systemImage: {})",
                    string(icon)
                )),
                None => out.line(format!("SwiftUI.Text(verbatim: {item_label})")),
            }
            out.close("}");
            if let Some(enabled) = item.expr("enabled") {
                out.line(format!(".disabled(!({}))", self.expr(enabled)));
            }
        }
        out.close("} label: {");
        out.indent += 1;
        match element.icon("icon") {
            Some(icon) => out.line(format!(
                "SwiftUI.Label({label}, systemImage: {})",
                string(icon)
            )),
            None => out.line(format!("SwiftUI.Text(verbatim: {label})")),
        }
        out.close("}");
        out.line(format!(
            ".bernControl({}, role: \"menu\", label: {label}, actions: BernControlActions())",
            self.target(element, ctx)
        ));
        for item in items.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let action = item
                .action
                .as_ref()
                .map(|a| self.action(a))
                .unwrap_or_default();
            let enabled = item
                .expr("enabled")
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "true".into());
            out.line(format!(
                ".bernControl({}, role: \"button\", label: {item_label}, enabled: {enabled}, actions: BernControlActions(activate: {{ {action} }}))",
                self.target(item, ctx)
            ));
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
        let style = self.style_of(element);
        match &style {
            // The style draws the button; its transforms follow the
            // button's own pressed state.
            Some(style) if style.draws_look() => {
                out.line(format!(
                    ".buttonStyle(BernButtonStyle(style: BernStyles.{}))",
                    style.name
                ));
                if element.kind == "IconButton" {
                    out.line(format!(".help({label})"));
                }
            }
            _ => {
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
                if let Some(style) = &style {
                    if let Some(size) = style.size() {
                        out.line(format!(
                            ".controlSize({})",
                            crate::style::control_size(size)
                        ));
                    }
                    out.line(format!(".bernFeedback(BernStyles.{})", style.name));
                }
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
        // An iOS form shows a text field's prompt, not its label: the label
        // goes beside a single line, as in Settings, and above several.
        let ios = self.platform == Platform::Ios;
        if ios {
            if kind == "multiline" {
                out.open("SwiftUI.VStack(alignment: .leading, spacing: 6) {");
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
            } else {
                out.open("SwiftUI.LabeledContent {");
            }
        }
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
        if ios {
            if kind == "multiline" {
                out.close("}");
            } else {
                out.line(".multilineTextAlignment(.trailing)");
                out.close("} label: {");
                out.indent += 1;
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
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
        // Styled items draw their own selection: the list keeps none, and
        // each row selects itself.
        let mut styled = false;
        for child in &sidebar.children {
            child.walk(&mut |node| {
                if let Node::Element(item) = node
                    && item.kind == "SidebarItem"
                    && self.style_of(item).is_some()
                {
                    styled = true;
                }
            });
        }
        let rows = styled.then(|| {
            format!("SwiftUI.Binding(get: {{ {selection} }}, set: {{ {selection} = $0 }})")
        });
        if rows.is_some() {
            out.open("SwiftUI.List {");
        } else {
            out.open(format!(
                "SwiftUI.List(selection: SwiftUI.Binding(get: {{ Optional({selection}) }}, set: {{ if let value = $0 {{ {selection} = value }} }})) {{"
            ));
        }
        self.sidebar_nodes(out, &sidebar.children, rows.as_deref());
        out.close("}");
        out.line(".navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 320)");
        let mut labeled = false;
        for child in &sidebar.children {
            child.walk(&mut |node| {
                labeled |= matches!(node, Node::Element(item) if item.kind == "SidebarItem");
            });
        }
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
        // The detail column and the sidebar control are both named after
        // the selected destination.
        let title = labeled.then(|| self.nav_label(&selection, &sidebar.children, "SidebarItem"));
        // A compact iOS split view shows the sidebar as the first screen of
        // a stack, titled like the window.
        if self.platform == Platform::Ios {
            out.line(".navigationTitle(bernWindow.title)");
        }
        out.close("} detail: {");
        out.indent += 1;
        out.open("SwiftUI.Group {");
        self.nodes(out, &detail.children, ctx);
        out.close("}");
        // The detail column is titled by the selected sidebar destination,
        // as macOS sidebar apps are.
        if let Some(title) = &title {
            out.line(format!(".navigationTitle({title})"));
        }
        self.toolbar(out, &detail.children, ctx);
        out.close("}");
        // The sidebar is a control of the whole split view, which stays
        // mounted while a compact layout shows only the detail.
        if sidebar.id.is_some() {
            let ty = swift_type(&sidebar.field("selection").expect("selection field").ty);
            out.line(format!(
                ".bernControl({}, role: \"sidebar\", label: {}, actions: BernControlActions(select: {{ name in guard let value = {}.first(where: {{ $0.bernName == name }}) else {{ return false }}; {selection} = value; return true }}, value: {{ {selection}.bernName }}))",
                self.target(sidebar, ctx),
                title.clone().unwrap_or_else(|| "\"\"".into()),
                self.nav_values(&sidebar.children, "SidebarItem", &ty)
            ));
        }
        self.nav_fallback(out, sidebar, &selection, "SidebarItem");
    }

    /// Sidebar children: sections and items, also inside conditions and
    /// lists, which is how items of modules that can be switched off arrive.
    fn sidebar_nodes(&self, out: &mut Out, nodes: &[Node], rows: Option<&str>) {
        self.nav_walk(
            out,
            nodes,
            &|out, element| self.sidebar_child(out, element, rows),
            None,
        );
    }

    /// Items of a navigation container through `if`, `if let`, `match` and
    /// `ForEach`, as the container's own builder takes them; `item` writes
    /// each one. In a toolbar (`toolbar` set), a repetition is one group.
    fn nav_walk(
        &self,
        out: &mut Out,
        nodes: &[Node],
        item: &dyn Fn(&mut Out, &Element),
        toolbar: Option<&Ctx>,
    ) {
        for node in nodes {
            match node {
                Node::Element(element) if element.kind == "ForEach" => {
                    let (Some(items), Some(template), Some(Prop::Key { name, .. })) = (
                        element.expr("items"),
                        &element.template,
                        element.prop("key"),
                    ) else {
                        continue;
                    };
                    let var = member(&template.var);
                    let each = format!(
                        "SwiftUI.ForEach({}, id: \\.{}) {{ {var} in",
                        self.expr(items),
                        member(name)
                    );
                    match toolbar {
                        Some(ctx) => {
                            out.open("SwiftUI.ToolbarItemGroup(placement: .primaryAction) {");
                            out.open(each);
                            let mut inner = ctx.clone();
                            inner.key = Some(format!("{var}.{}", member(name)));
                            self.nodes(out, &element.children, &inner);
                            out.close("}");
                            out.close("}");
                        }
                        None => {
                            out.open(each);
                            self.nav_walk(out, &element.children, item, toolbar);
                            out.close("}");
                        }
                    }
                }
                Node::Element(element) => item(out, element),
                Node::If(node) => {
                    for (index, (condition, body)) in node.branches.iter().enumerate() {
                        if index > 0 {
                            out.indent -= 1;
                        }
                        let keyword = if index == 0 { "if" } else { "} else if" };
                        out.open(format!("{keyword} {} {{", self.expr(condition)));
                        self.nav_walk(out, body, item, toolbar);
                    }
                    if !node.otherwise.is_empty() {
                        out.indent -= 1;
                        out.open("} else {");
                        self.nav_walk(out, &node.otherwise, item, toolbar);
                    }
                    out.close("}");
                }
                Node::IfLet(node) => {
                    out.open(format!(
                        "if let {} = {} {{",
                        member(&node.var),
                        self.expr(&node.value)
                    ));
                    self.nav_walk(out, &node.then, item, toolbar);
                    if !node.otherwise.is_empty() {
                        out.indent -= 1;
                        out.open("} else {");
                        self.nav_walk(out, &node.otherwise, item, toolbar);
                    }
                    out.close("}");
                }
                Node::Match(node) => {
                    let def = self.types().enum_def(&node.enum_name).expect("enum");
                    out.open(format!("switch {} {{", self.expr(&node.value)));
                    for arm in &node.arms {
                        out.open(format!("{}:", case_pattern(def, arm)));
                        if arm.body.is_empty() {
                            out.line("SwiftUI.EmptyView()");
                        }
                        self.nav_walk(out, &arm.body, item, toolbar);
                        out.indent -= 1;
                    }
                    out.close("}");
                }
                Node::Component(_) => {}
            }
        }
    }

    /// Swift statements that run `item` for each item a navigation
    /// container shows now, through the same conditions and lists as its
    /// view; elements that are not items are looked into.
    fn nav_statements(&self, nodes: &[Node], item: &dyn Fn(&Element) -> Option<String>) -> String {
        let mut statements = Vec::new();
        for node in nodes {
            match node {
                Node::Element(element) if element.kind == "ForEach" => {
                    let (Some(items), Some(template)) = (element.expr("items"), &element.template)
                    else {
                        continue;
                    };
                    statements.push(format!(
                        "for {} in {} {{ {} }}",
                        member(&template.var),
                        self.expr(items),
                        self.nav_statements(&element.children, item)
                    ));
                }
                Node::Element(element) => match item(element) {
                    Some(statement) => statements.push(statement),
                    None => statements.push(self.nav_statements(&element.children, item)),
                },
                Node::If(node) => {
                    let mut text = String::new();
                    for (index, (condition, body)) in node.branches.iter().enumerate() {
                        if index > 0 {
                            text.push_str(" else ");
                        }
                        text.push_str(&format!(
                            "if {} {{ {} }}",
                            self.expr(condition),
                            self.nav_statements(body, item)
                        ));
                    }
                    if !node.otherwise.is_empty() {
                        text.push_str(&format!(
                            " else {{ {} }}",
                            self.nav_statements(&node.otherwise, item)
                        ));
                    }
                    statements.push(text);
                }
                Node::IfLet(node) => statements.push(format!(
                    "if let {} = {} {{ {} }} else {{ {} }}",
                    member(&node.var),
                    self.expr(&node.value),
                    self.nav_statements(&node.then, item),
                    self.nav_statements(&node.otherwise, item)
                )),
                Node::Match(node) => {
                    let def = self.types().enum_def(&node.enum_name).expect("enum");
                    let arms: Vec<_> = node
                        .arms
                        .iter()
                        .map(|arm| {
                            let body = self.nav_statements(&arm.body, item);
                            format!(
                                "{}: {}",
                                case_pattern(def, arm),
                                if body.is_empty() {
                                    "break".into()
                                } else {
                                    body
                                }
                            )
                        })
                        .collect();
                    statements.push(format!(
                        "switch {} {{ {} }}",
                        self.expr(&node.value),
                        arms.join("; ")
                    ));
                }
                Node::Component(_) => {}
            }
        }
        statements.retain(|statement| !statement.is_empty());
        statements.join("; ")
    }

    /// The values of the items of `kind` shown now, as a Swift `[ty]`.
    fn nav_values(&self, nodes: &[Node], kind: &str, ty: &str) -> String {
        let statements = self.nav_statements(nodes, &|element| {
            (element.kind == kind).then(|| {
                let value = element
                    .expr("value")
                    .map(|v| self.expr(v))
                    .unwrap_or_default();
                format!("items.append({value})")
            })
        });
        format!("{{ () -> [{ty}] in var items: [{ty}] = []; {statements}; return items }}()")
    }

    /// The label of the selected item of `kind`, as a Swift `String`.
    fn nav_label(&self, selection: &str, nodes: &[Node], kind: &str) -> String {
        let statements = self.nav_statements(nodes, &|element| {
            (element.kind == kind).then(|| {
                let value = element
                    .expr("value")
                    .map(|v| self.expr(v))
                    .unwrap_or_default();
                let label = self.text_prop(element, "label").unwrap_or_default();
                format!("if {selection} == {value} {{ return {label} }}")
            })
        });
        format!("{{ () -> String in {statements}; return \"\" }}()")
    }

    /// When the selected item goes away, select the container's
    /// `fallback`, or else the first item shown.
    fn nav_fallback(&self, out: &mut Out, element: &Element, selection: &str, kind: &str) {
        if !dynamic_items(&element.children) {
            return;
        }
        let ty = swift_type(&element.field("selection").expect("selection field").ty);
        let target = element
            .expr("fallback")
            .map(|value| format!("Optional({})", self.expr(value)))
            .unwrap_or_else(|| "visible.first".into());
        out.line(format!(
            ".onChange(of: {}, initial: true) {{ _, visible in if !visible.contains({selection}), let value = {target} {{ {selection} = value }} }}",
            self.nav_values(&element.children, kind, &ty)
        ));
    }

    fn sidebar_child(&self, out: &mut Out, element: &Element, rows: Option<&str>) {
        match element.kind.as_str() {
            "Section" => {
                let title = self.text_prop(element, "title");
                out.open("SwiftUI.Section {");
                self.sidebar_nodes(out, &element.children, rows);
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
            "SidebarItem" if rows.is_some() => {
                let label = self.text_prop(element, "label").unwrap_or_default();
                let value = element
                    .expr("value")
                    .map(|v| self.expr(v))
                    .unwrap_or_default();
                let icon = element
                    .icon("icon")
                    .map(string)
                    .unwrap_or_else(|| "nil".into());
                // Items without a style of their own still show which is
                // selected.
                let style = self
                    .style_of(element)
                    .map(|style| format!("BernStyles.{}", style.name))
                    .unwrap_or_else(|| {
                        "BernStyle(selected: BernLook(fill: .role(\"selection.background\")))"
                            .into()
                    });
                out.line(format!(
                    "BernSidebarRow(label: {label}, systemImage: {icon}, value: {value}, selection: {}, style: {style})",
                    rows.unwrap_or_default()
                ));
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
        self.nav_walk(
            out,
            &element.children,
            &|out, tab| {
                let label = self.text_prop(tab, "label").unwrap_or_default();
                let value = tab.expr("value").map(|v| self.expr(v)).unwrap_or_default();
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
            },
            None,
        );
        out.close("}");
        if element.id.is_some() {
            let ty = swift_type(&element.field("selection").expect("selection field").ty);
            out.line(format!(
                ".bernControl({}, role: \"tabs\", label: {}, actions: BernControlActions(select: {{ name in guard let value = {}.first(where: {{ $0.bernName == name }}) else {{ return false }}; {selection} = value; return true }}, value: {{ {selection}.bernName }}))",
                self.target(element, ctx),
                self.nav_label(&selection, &element.children, "Tab"),
                self.nav_values(&element.children, "Tab", &ty)
            ));
        }
        self.nav_fallback(out, element, &selection, "Tab");
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
                ParamKind::Client { .. } => client = true,
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
        // Content written inline in the window sees the same names as a
        // component body: the window and its sessions.
        out.line("private var bernWindow: BernWindowContext { context }");
        for session in &window.sessions {
            out.line(format!(
                "private var {}: {} {{ context.session({}.self) }}",
                session_var(session),
                session_class(session),
                session_class(session)
            ));
        }
        out.line("");
        out.open("var body: some View {");
        out.open("BernWindowRoot(context: context) {");
        let ctx = Ctx {
            component: window.id.clone(),
            ..Ctx::default()
        };
        self.node(out, &window.content, &ctx);
        out.close("}");
        if window.chrome == bern_ir::WindowChrome::Unified
            || window.immersive.is_some()
            || window.idle_after.is_some()
        {
            out.line(format!(
                ".modifier(BernWindowBehavior(context: context, unified: {}, idleAfter: {}, immersive: {}))",
                window.chrome == bern_ir::WindowChrome::Unified,
                window
                    .idle_after
                    .map(double)
                    .unwrap_or_else(|| "nil".into()),
                window
                    .immersive
                    .as_ref()
                    .map(|expr| self.expr(expr))
                    .unwrap_or_else(|| "false".into())
            ));
        }
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
        match self.platform {
            Platform::Macos if window.role == WindowRole::Settings => {
                if let Some(width) = window.width {
                    out.line(format!(".frame(width: {})", double(width)));
                }
                // macOS settings windows fit their content and resize per tab.
                out.line(".scrollDisabled(true)");
                out.line(".fixedSize(horizontal: false, vertical: true)");
            }
            Platform::Macos if !frame.is_empty() => {
                out.line(format!(".frame({})", frame.join(", ")));
            }
            // The minimum size applies where people resize windows (iPad);
            // iPhone screens and sheets size them.
            Platform::Ios
                if window.role != WindowRole::Settings
                    && (window.min_width.is_some() || window.min_height.is_some()) =>
            {
                let side = |value: Option<f64>| value.map(double).unwrap_or_else(|| "nil".into());
                out.line(format!(
                    ".bernMinimumSize(width: {}, height: {})",
                    side(window.min_width),
                    side(window.min_height)
                ));
            }
            _ => {}
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
                    member(&self.app.protocol.command_symbol(spec)),
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
        out.line(match self.platform {
            Platform::Macos => "import AppKit",
            Platform::Ios => "import UIKit",
        });
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
        let styles = crate::style::used(app, self.style_platform());
        if !styles.is_empty() {
            out.line("// MARK: - Styles");
            out.line("");
            out.line("/// The style overrides of `style!` on this platform.");
            out.open("enum BernStyles {");
            for style in &styles {
                out.line(crate::style::declaration(app, style));
            }
            out.close("}");
            out.line("");
        }
        if !app.style.patterns.is_empty() {
            out.line("/// The patterns of `style!`.");
            out.open("enum BernPatterns {");
            for pattern in &app.style.patterns {
                let bands = pattern
                    .bands
                    .iter()
                    .map(|band| {
                        format!(
                            "BernBand(color: {}, width: {}, opacity: {})",
                            band.color
                                .as_ref()
                                .map(crate::style::color)
                                .unwrap_or_else(|| "nil".into()),
                            double(band.width),
                            double(band.opacity)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.line(format!(
                    "static let `{}` = BernPatternDef(bands: [{bands}], twill: {})",
                    pattern.name, pattern.twill
                ));
            }
            out.close("}");
            out.line("");
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
                let size = match (window.width, window.height) {
                    (Some(width), Some(height)) => format!(
                        "CGSize(width: {}, height: {})",
                        double(width),
                        double(height)
                    ),
                    _ => "nil".into(),
                };
                format!(
                    "BernWindowKind(id: {}, title: {}, isSettings: {}, defaultSize: {size})",
                    string(&window.id),
                    string(&window.title.key),
                    window.role == WindowRole::Settings
                )
            })
            .collect();
        // Windows that open with the session besides the primary; a macOS
        // settings window opens only on request.
        let initial: Vec<String> = app
            .windows
            .windows
            .iter()
            .filter(|window| {
                window.initial == bern_ir::WindowInitial::Open
                    && window.id != app.windows.primary
                    && (self.platform == Platform::Ios || window.role != WindowRole::Settings)
            })
            .map(|window| string(&window.id))
            .collect();
        out.line("@main");
        out.open("struct BernApplication: App {");
        if self.platform == Platform::Macos {
            out.line("@NSApplicationDelegateAdaptor(BernAppDelegate.self) private var delegate");
            out.line("");
        }
        out.open("init() {");
        out.open(format!(
            "BernHost.shared.start(fingerprint: 0x{fingerprint:016x}, kinds: [{}], primary: {}) {{ field, reader in",
            kinds.join(", "),
            string(&app.windows.primary)
        ));
        out.line("try BernEngine.shared.bernApply(field, &reader)");
        out.close("}");
        out.close("}");
        out.line("");
        out.open("var body: some Scene {");
        match self.platform {
            Platform::Macos => self.macos_scenes(&mut out, &order, &initial),
            Platform::Ios => self.ios_scene(&mut out, &initial),
        }
        out.close("}");
        out.close("}");
        out.finish()
    }

    /// One SwiftUI scene per window kind: `Window`, or `Settings` for the
    /// settings window.
    fn macos_scenes(&self, out: &mut Out, order: &[usize], initial: &[String]) {
        let app = self.app;
        for &index in order {
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
    }

    /// The scenes of an iOS app. The primary window's scene hosts the
    /// virtual windows: on iPhone every other window is a sheet over the
    /// primary, titled and with a close button unless its content brings
    /// its own navigation; on iPad only settings is. On iPad every other
    /// window has a scene of its own, identified by its kind, so opening it
    /// again brings the open one forward, and titled in a toolbar unless
    /// its content brings its own navigation. A settings window opens with
    /// Command-Comma, as settings do.
    fn ios_scene(&self, out: &mut Out, initial: &[String]) {
        let app = self.app;
        let window_group = |out: &mut Out, window: &WindowDef| {
            out.open(format!(
                "SwiftUI.WindowGroup(SwiftUI.Text(verbatim: BernText.string({})), id: {}, for: String.self) {{ _ in",
                string(&window.title.key),
                string(&window.id)
            ));
        };
        let scene_modifiers = |out: &mut Out, window: &WindowDef| {
            out.close(format!("}} defaultValue: {{ {} }}", string(&window.id)));
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
        };
        let Some(primary) = app
            .windows
            .windows
            .iter()
            .find(|w| w.id == app.windows.primary)
        else {
            return;
        };
        window_group(out, primary);
        out.open(format!(
            "BernVirtualHost(initial: [{}]) {{ kind in",
            initial.join(", ")
        ));
        out.open("switch kind {");
        for (index, window) in app.windows.windows.iter().enumerate() {
            out.line(format!("case {}:", string(&window.id)));
            out.indent += 1;
            if window.id == app.windows.primary || self.has_navigation(&window.content, 0) {
                out.line(format!("BernWindow{index}()"));
            } else {
                out.line(format!(
                    "BernSheetChrome(kind: {}, title: {}) {{ BernWindow{index}() }}",
                    string(&window.id),
                    string(&window.title.key)
                ));
            }
            out.indent -= 1;
        }
        out.line("default:");
        out.line("    SwiftUI.EmptyView()");
        out.close("}");
        out.close("}");
        out.line(".environment(BernEngine.shared)");
        scene_modifiers(out, primary);
        if let Some(settings) = app
            .windows
            .windows
            .iter()
            .find(|window| window.role == WindowRole::Settings)
        {
            out.open(".commands {");
            out.open("SwiftUI.CommandGroup(replacing: .appSettings) {");
            out.line(format!(
                "SwiftUI.Button(BernText.string({}) + \"…\") {{ BernHost.shared.open({}) }}",
                string(&settings.title.key),
                string(&settings.id)
            ));
            out.line("    .keyboardShortcut(\",\")");
            out.close("}");
            out.close("}");
        }
        for (index, window) in app.windows.windows.iter().enumerate() {
            if window.id == app.windows.primary || window.role == WindowRole::Settings {
                continue;
            }
            window_group(out, window);
            if self.has_navigation(&window.content, 0) {
                out.line(format!("BernWindow{index}()"));
            } else {
                out.line(format!(
                    "BernWindowChrome(title: {}) {{ BernWindow{index}() }}",
                    string(&window.title.key)
                ));
            }
            out.line("    .environment(BernEngine.shared)");
            scene_modifiers(out, window);
        }
    }

    /// Whether a window's content has a navigation container of its own,
    /// directly or through the components it calls.
    fn has_navigation(&self, node: &Node, depth: usize) -> bool {
        // Components cannot call themselves forever; the depth only guards
        // against a cycle the compiler would report.
        if depth > 64 {
            return false;
        }
        let mut found = false;
        node.walk(&mut |node| match node {
            Node::Element(element)
                if matches!(element.kind.as_str(), "SplitView" | "Navigation") =>
            {
                found = true;
            }
            Node::Component(call) => {
                if let Some(component) = self.app.component(&call.component) {
                    found |= self.has_navigation(&component.body, depth + 1);
                }
            }
            _ => {}
        });
        found
    }
}

/// `(value, label)` of every sidebar item, in declaration order.
/// `case .variant(let a, _)` for one arm of a `match`, or `default`.
fn case_pattern(def: &EnumDef, arm: &MatchArm) -> String {
    let Some(index) = arm.variant else {
        return "default".into();
    };
    let variant = &def.variants[index as usize];
    if variant.fields.is_empty() {
        return format!("case .{}", case(&variant.name));
    }
    let patterns: Vec<_> = (0..variant.fields.len())
        .map(|field| {
            arm.bindings
                .iter()
                .find(|binding| binding.field == field)
                .map(|binding| format!("let {}", member(&binding.name)))
                .unwrap_or_else(|| "_".into())
        })
        .collect();
    format!("case .{}({})", case(&variant.name), patterns.join(", "))
}

/// Whether navigation items sit in conditions or repetitions, also inside
/// a sidebar's sections.
fn dynamic_items(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) if element.kind == "ForEach" => true,
        Node::Element(element) if element.kind == "Section" => dynamic_items(&element.children),
        Node::Element(_) | Node::Component(_) => false,
        Node::If(_) | Node::IfLet(_) | Node::Match(_) => true,
    })
}
