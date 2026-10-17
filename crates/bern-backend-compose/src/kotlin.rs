//! Kotlin generation: schema types, engine state, session drafts, one
//! composable per component and per window, and the activity.

use crate::names::{double, float, member, pascal, string};
use bern_ir::{
    Action, Application, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind, IfLetNode,
    IfNode, LayoutSize, MatchArm, MatchNode, MediaOp, Node, ParamKind, Prop, Text, WindowDef,
    WindowOp, WindowRole,
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

pub fn kotlin_type(ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".into(),
        Type::Bool => "Boolean".into(),
        Type::Int => "Long".into(),
        Type::Float => "Double".into(),
        Type::Text => "String".into(),
        Type::Enum { name } | Type::Record { name } => name.clone(),
        Type::Optional { inner } => {
            let inner = kotlin_type(inner);
            if inner.ends_with('?') {
                inner
            } else {
                format!("{inner}?")
            }
        }
        Type::List { inner } => format!("List<{}>", kotlin_type(inner)),
    }
}

/// An expression reading a value of `ty` from `reader`.
fn decode(ty: &Type, reader: &str) -> String {
    match ty {
        Type::Unit => "Unit".into(),
        Type::Bool => format!("{reader}.bool()"),
        Type::Int => format!("{reader}.i64()"),
        Type::Float => format!("{reader}.f64()"),
        Type::Text => format!("{reader}.text()"),
        Type::Enum { name } | Type::Record { name } => format!("{name}.decode({reader})"),
        Type::Optional { inner } => format!("{reader}.optional {{ {} }}", decode(inner, reader)),
        Type::List { inner } => format!("{reader}.list {{ {} }}", decode(inner, reader)),
    }
}

/// A statement writing `value` of `ty` to `writer`.
fn encode(ty: &Type, value: &str, writer: &str) -> String {
    match ty {
        Type::Unit => String::new(),
        Type::Bool => format!("{writer}.bool({value})"),
        Type::Int => format!("{writer}.i64({value})"),
        Type::Float => format!("{writer}.f64({value})"),
        Type::Text => format!("{writer}.text({value})"),
        Type::Enum { .. } | Type::Record { .. } => format!("{value}.encode({writer})"),
        Type::Optional { inner } => format!(
            "{writer}.optional({value}) {{ {} }}",
            encode(inner, "it", writer)
        ),
        Type::List { inner } => format!(
            "{writer}.list({value}) {{ {} }}",
            encode(inner, "it", writer)
        ),
    }
}

/// Kotlin expression for a schema value.
fn value_literal(value: &Value, ty: &Type, types: &TypeTable) -> String {
    match (value, ty) {
        (Value::Bool(value), _) => value.to_string(),
        (Value::Int(value), _) => format!("{value}L"),
        (Value::Float(value), _) => double(*value),
        (Value::Text(value), _) => string(value),
        (Value::Optional(None), _) => "null".into(),
        (Value::Optional(Some(value)), Type::Optional { inner }) => {
            value_literal(value, inner, types)
        }
        (Value::List(items), Type::List { inner }) => format!(
            "listOf<{}>({})",
            kotlin_type(inner),
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
                format!("{name}.{}", variant_def.name)
            } else {
                let args: Vec<_> = fields
                    .iter()
                    .zip(&variant_def.fields)
                    .map(|(value, field)| {
                        format!(
                            "{} = {}",
                            member(&field.name),
                            value_literal(value, &field.ty, types)
                        )
                    })
                    .collect();
                format!("{name}.{}({})", variant_def.name, args.join(", "))
            }
        }
        (Value::Record(values), Type::Record { name }) => {
            let def = types.record_def(name).expect("known record");
            let args: Vec<_> = values
                .iter()
                .zip(&def.fields)
                .map(|(value, field)| {
                    format!(
                        "{} = {}",
                        member(&field.name),
                        value_literal(value, &field.ty, types)
                    )
                })
                .collect();
            format!("{name}({})", args.join(", "))
        }
        _ => "Unit".into(),
    }
}

fn default_literal(ty: &Type, types: &TypeTable) -> String {
    let value = Value::default_for(ty, types).unwrap_or(Value::Unit);
    value_literal(&value, ty, types)
}

fn generate_enum(out: &mut Out, def: &EnumDef) {
    let name = &def.name;
    let labeled = def.variants.iter().all(|variant| variant.label.is_some());
    let supertypes = if labeled { " : BernLabeled" } else { "" };
    let label = |out: &mut Out, prefix: &str| {
        if labeled {
            out.open("override val bernLabel: String");
            out.open("get() = when (this) {");
            for variant in &def.variants {
                out.line(format!(
                    "{prefix}{} -> BernText.string({})",
                    variant.name,
                    string(&variant.label.as_ref().unwrap().key)
                ));
            }
            out.close("}");
            out.indent -= 1;
            out.line("");
        }
    };
    if def.is_unit_only() {
        out.open(format!("enum class {name}{supertypes} {{"));
        let names: Vec<_> = def.variants.iter().map(|v| v.name.clone()).collect();
        out.line(format!("{};", names.join(", ")));
        out.line("");
        label(out, "");
        out.line("/// The declared variant name, used by external entries.");
        out.line("val bernName: String get() = name");
        out.line("");
        out.line("fun encode(writer: BernWriter) = writer.u32(ordinal)");
        out.line("");
        out.open("companion object {");
        out.open(format!(
            "fun decode(reader: BernReader): {name} = when (reader.u32()) {{"
        ));
        for (index, variant) in def.variants.iter().enumerate() {
            out.line(format!("{index} -> {}", variant.name));
        }
        out.line(format!(
            "else -> throw BernDecodeError({})",
            string(&format!("invalid {name}"))
        ));
        out.close("}");
        out.line("");
        out.line(format!(
            "fun fromBernName(name: String): {name}? = entries.firstOrNull {{ it.name == name }}"
        ));
        out.close("}");
        out.close("}");
        out.line("");
        return;
    }
    out.open(format!("sealed class {name}{supertypes} {{"));
    for variant in &def.variants {
        if variant.fields.is_empty() {
            out.line(format!("data object {} : {name}()", variant.name));
        } else {
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("val {}: {}", member(&field.name), kotlin_type(&field.ty)))
                .collect();
            out.line(format!(
                "data class {}({}) : {name}()",
                variant.name,
                fields.join(", ")
            ));
        }
    }
    out.line("");
    label(out, "is ");
    out.open("val bernName: String");
    out.open("get() = when (this) {");
    for variant in &def.variants {
        out.line(format!("is {} -> {}", variant.name, string(&variant.name)));
    }
    out.close("}");
    out.indent -= 1;
    out.line("");
    out.open("fun encode(writer: BernWriter) {");
    out.open("when (this) {");
    for (index, variant) in def.variants.iter().enumerate() {
        out.open(format!("is {} -> {{", variant.name));
        out.line(format!("writer.u32({index})"));
        for field in &variant.fields {
            out.line(encode(&field.ty, &member(&field.name), "writer"));
        }
        out.close("}");
    }
    out.close("}");
    out.close("}");
    out.line("");
    out.open("companion object {");
    out.open(format!(
        "fun decode(reader: BernReader): {name} = when (reader.u32()) {{"
    ));
    for (index, variant) in def.variants.iter().enumerate() {
        if variant.fields.is_empty() {
            out.line(format!("{index} -> {}", variant.name));
        } else {
            let args: Vec<_> = variant
                .fields
                .iter()
                .map(|field| format!("{} = {}", member(&field.name), decode(&field.ty, "reader")))
                .collect();
            out.line(format!("{index} -> {}({})", variant.name, args.join(", ")));
        }
    }
    out.line(format!(
        "else -> throw BernDecodeError({})",
        string(&format!("invalid {name}"))
    ));
    out.close("}");
    out.close("}");
    out.close("}");
    out.line("");
}

fn generate_record(out: &mut Out, def: &RecordDef) {
    let name = &def.name;
    let fields: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("val {}: {}", member(&field.name), kotlin_type(&field.ty)))
        .collect();
    out.open(format!("data class {name}({}) {{", fields.join(", ")));
    out.open("fun encode(writer: BernWriter) {");
    for field in &def.fields {
        out.line(encode(&field.ty, &member(&field.name), "writer"));
    }
    out.close("}");
    out.line("");
    out.open("companion object {");
    let args: Vec<_> = def
        .fields
        .iter()
        .map(|field| format!("{} = {}", member(&field.name), decode(&field.ty, "reader")))
        .collect();
    out.line(format!(
        "fun decode(reader: BernReader): {name} = {name}({})",
        args.join(", ")
    ));
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
    out.open("object BernEngine {");
    for field in &protocol.state {
        if !field.doc.is_empty() {
            out.line(format!("/// {}", field.doc.replace('\n', " ")));
        }
        out.line(format!(
            "var {}: {} by mutableStateOf({})",
            member(&field.name),
            kotlin_type(&field.ty),
            default_literal(&field.ty, &app.types)
        ));
        out.line("    private set");
    }
    out.line("");
    out.open("fun bernApply(field: Int, reader: BernReader) {");
    out.open("when (field) {");
    for (index, field) in protocol.state.iter().enumerate() {
        out.line(format!(
            "{index} -> {} = {}",
            member(&field.name),
            decode(&field.ty, "reader")
        ));
    }
    out.line("else -> throw BernDecodeError(\"unknown state field $field\")");
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
            .map(|param| format!("{}: {}", member(&param.name), kotlin_type(&param.ty)))
            .collect();
        params.push("bernWindow: BernWindowContext?".into());
        params.push("bernCompletion: ((BernCommandFailure?) -> Unit)? = null".into());
        out.open(format!(
            "fun {}({}) {{",
            member(&app.protocol.command_symbol(command)),
            params.join(", ")
        ));
        out.open(format!(
            "BernHost.dispatch({index}, bernWindow?.id, {{ writer ->"
        ));
        for param in &command.params {
            out.line(encode(&param.ty, &member(&param.name), "writer"));
        }
        out.close("}, completion = bernCompletion)");
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
        out.open(format!("class {class} : BernSessionState({index}) {{"));
        for (field_index, field) in session.fields.iter().enumerate() {
            let name = member(&field.name);
            let storage = format!("bern_{}", field.name);
            let ty = kotlin_type(&field.ty);
            out.line(format!(
                "private var {storage}: {ty} by mutableStateOf({})",
                value_literal(&field.initial, &field.ty, &app.types)
            ));
            out.open(format!("var {name}: {ty}"));
            out.line(format!("get() = {storage}"));
            out.open("set(value) {");
            out.line(format!(
                "if (window?.set({index}, {field_index}) {{ writer -> {} }} != true) return",
                encode(&field.ty, "value", "writer")
            ));
            out.line(format!("{storage} = value"));
            out.close("}");
            out.indent -= 1;
            out.line("");
        }
        out.open("override fun load(reader: BernReader) {");
        for field in &session.fields {
            out.line(format!(
                "bern_{} = {}",
                field.name,
                decode(&field.ty, "reader")
            ));
        }
        out.close("}");
        out.close("}");
        out.line("");
    }
}

/// The layout a generated statement sits in, which decides whether
/// `Spacer` can take the free space.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Scope {
    #[default]
    Other,
    Column,
    Row,
}

/// What a composable body can refer to while generating.
#[derive(Clone, Default)]
struct Ctx {
    component: String,
    /// Kotlin expression appended to control targets inside rows.
    key: Option<String>,
    /// The enclosing navigation path session field.
    path: Option<String>,
    scope: Scope,
    /// Inside a branch whose condition animates its appearing and
    /// disappearing.
    animated: bool,
    /// Inside a scroll view that rests on its items or follows a
    /// position: the first repetition's items report where they are.
    scroll_items: bool,
    /// The nodes are direct children of a container that declares
    /// `stagger`, or the root of a component, which may be one: those that
    /// appear take turns on the clock in `LocalBernStagger`.
    stagger: bool,
    /// The nodes are layers of a `Stack`.
    layer: bool,
}

impl Ctx {
    fn within(&self, scope: Scope) -> Ctx {
        Ctx {
            scope,
            ..self.clone()
        }
    }
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether a container anywhere declares `stagger`. Only then do the
    /// children of containers and the roots of components take part in
    /// the turns.
    staggers: bool,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        let mut staggers = false;
        let mut find = |node: &Node| {
            staggers |= matches!(node, Node::Element(element) if element.prop("stagger").is_some())
        };
        for component in &app.components {
            component.body.walk(&mut find);
        }
        for window in &app.windows.windows {
            window.content.walk(&mut find);
        }
        Self { app, staggers }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    /// The style of a control from `style!`, if any.
    fn style_of(&self, element: &Element) -> Option<crate::style::Resolved> {
        crate::style::resolve(
            self.app,
            PLATFORM,
            &element.kind,
            element.keyword("variant"),
        )
    }

    /// `, style = BernStyles.x` for a styled control.
    fn style_arg(&self, element: &Element) -> String {
        self.style_of(element)
            .map(|style| format!(", style = BernStyles.{}", style.name))
            .unwrap_or_default()
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

    fn expr(&self, expr: &Expr) -> String {
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
                        "BernText.format({}, listOf({}))",
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

    /// The control identity as a Kotlin string expression.
    fn target(&self, element: &Element, ctx: &Ctx) -> String {
        let base = string(&format!(
            "{}.{}",
            ctx.component,
            element.id.as_deref().unwrap_or(&element.path)
        ));
        // The keys of the rows around it are added where it registers.
        base
    }

    /// The session field a control writes, readable and assignable.
    fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        let session = self.app.session(&field.session)?;
        Some(format!(
            "{}.{}",
            session_var(&field.session),
            member(&session.fields[field.field].name)
        ))
    }

    fn action(&self, action: &Action) -> String {
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

    fn icon(name: Option<&str>) -> String {
        match name {
            Some(name) => format!("BernIcons.get({})", string(name)),
            None => "null".into(),
        }
    }

    fn enabled(&self, element: &Element) -> String {
        element
            .expr("enabled")
            .map(|expr| self.expr(expr))
            .unwrap_or_else(|| "true".into())
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node], ctx: &Ctx) {
        for node in nodes {
            self.node(out, node, ctx);
        }
    }

    fn node(&self, out: &mut Out, node: &Node, ctx: &Ctx) {
        match node {
            Node::Element(element) => {
                // What the place the element stands in says about it ends
                // here; its own children start afresh. A repetition adds no
                // layout: its rows stand where it does.
                let repeats = element.kind == "ForEach";
                let own = if repeats {
                    ctx.clone()
                } else {
                    Ctx {
                        stagger: false,
                        layer: false,
                        ..ctx.clone()
                    }
                };
                // On a rail, the element sits at its fraction or between
                // two.
                let placement = self.placement(element);
                if let Some(place) = &placement {
                    out.open(format!("BernPlace({place}) {{"));
                }
                // Added later, as an item of a list, the element plays its
                // appearing transition; a direct child of a staggering
                // container waits for its turn.
                let appear = element.keyword("appear").filter(|_| !ctx.animated);
                let turns = ctx.stagger && self.staggers;
                let unstaggered =
                    turns && !repeats && appear.is_none() && Self::appears_below(element);
                if unstaggered {
                    out.open("BernUnstaggered {");
                }
                if let Some(name) = appear {
                    let staggered = if turns { ", staggered = true" } else { "" };
                    out.open(format!(
                        "BernAppear({}{staggered}) {{",
                        self.transition_args(Some(name))
                    ));
                }
                // Effects drawn on the element as a whole: a nudge and the
                // look that follows scrolling.
                let mut effects = String::new();
                if let Some(when) = element.expr("nudge") {
                    effects.push_str(&format!(
                        ".bernNudge({}, {})",
                        self.expr(when),
                        crate::style::motion(self.app, "jelly")
                    ));
                }
                let (scale, opacity) = (
                    element.number("scroll_scale"),
                    element.number("scroll_opacity"),
                );
                if scale.is_some() || opacity.is_some() {
                    effects.push_str(&format!(
                        ".bernScrollEffect({}, {})",
                        float(scale.unwrap_or(1.0)),
                        float(opacity.unwrap_or(1.0))
                    ));
                }
                if !effects.is_empty() {
                    out.open(format!("Box(Modifier{effects}) {{"));
                }
                // A part fixed to one appearance, with what opens from it.
                let appearance = element.keyword("appearance");
                if let Some(appearance) = appearance {
                    out.open(format!(
                        "BernAppearance(dark = {}) {{",
                        appearance == "dark"
                    ));
                }
                // A rail that is a layer of a stack, directly, and holds
                // only spans without a thickness of their own takes the
                // stack's size.
                let direct = appear.is_none()
                    && effects.is_empty()
                    && appearance.is_none()
                    && element.keyword("hero").is_none();
                let own = Ctx {
                    layer: if repeats {
                        own.layer
                    } else {
                        ctx.layer && direct && element.kind == "Rail" && Self::rail_fills(element)
                    },
                    ..own
                };
                match element.keyword("hero") {
                    Some(hero) => {
                        let key = match element.expr("hero_key") {
                            Some(key) => format!("\"{hero}-${{{}}}\"", self.expr(key)),
                            None => string(hero),
                        };
                        out.open(format!(
                            "BernHero({key}, {}) {{",
                            crate::style::motion(self.app, "hero")
                        ));
                        self.element(out, element, &own);
                        out.close("}");
                    }
                    None => self.element(out, element, &own),
                }
                if appearance.is_some() {
                    out.close("}");
                }
                if !effects.is_empty() {
                    out.close("}");
                }
                if appear.is_some() {
                    out.close("}");
                }
                if unstaggered {
                    out.close("}");
                }
                if placement.is_some() {
                    out.close("}");
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
                                    Some(format!("{} = {}", member(&param.name), self.expr(expr)))
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
            Node::If(node) if Self::branches_move(node) => self.animated_if(out, node, ctx),
            Node::If(node) => self.if_node(out, node, ctx, &|out, body| self.nodes(out, body, ctx)),
            Node::IfLet(node) => self.if_let(out, node, &|out, body| self.nodes(out, body, ctx)),
            Node::Match(node) => {
                self.match_node(out, node, &|out, body| self.nodes(out, body, ctx))
            }
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode, _ctx: &Ctx, body: &dyn Fn(&mut Out, &[Node])) {
        for (index, (condition, nodes)) in node.branches.iter().enumerate() {
            if index == 0 {
                out.open(format!("if ({}) {{", self.expr(condition)));
            } else {
                out.close(format!("}} else if ({}) {{", self.expr(condition)));
                out.indent += 1;
            }
            body(out, nodes);
        }
        if !node.otherwise.is_empty() {
            out.close("} else {");
            out.indent += 1;
            body(out, &node.otherwise);
        }
        out.close("}");
    }

    /// Whether a branch of the condition holds an element that appears or
    /// disappears with a transition.
    fn branches_move(node: &IfNode) -> bool {
        node.branches
            .iter()
            .map(|(_, body)| body)
            .chain(std::iter::once(&node.otherwise))
            .flatten()
            .any(|node| {
                matches!(node, Node::Element(element)
                    if element.keyword("appear").is_some()
                        || element.keyword("disappear").is_some()
                        || element.keyword("hero").is_some())
            })
    }

    /// `bernEnter(…)` for a transition name.
    fn enter(&self, name: Option<&str>) -> String {
        format!("bernEnter({})", self.transition_args(name))
    }

    /// The transition and its spring for a transition name, as arguments.
    fn transition_args(&self, name: Option<&str>) -> String {
        match name.and_then(|name| crate::style::transition(self.app, name)) {
            Some((transition, motion)) => format!("{transition}, {motion}"),
            None => "null, null".into(),
        }
    }

    /// Whether an element below this one, or a component, may appear and
    /// so take a turn it should not: only direct children of a staggering
    /// container take turns.
    fn appears_below(element: &Element) -> bool {
        let mut found = false;
        for child in &element.children {
            child.walk(&mut |node| {
                found |= match node {
                    Node::Component(_) => true,
                    Node::Element(element) => element.keyword("appear").is_some(),
                    _ => false,
                }
            });
        }
        found
    }

    /// The arguments of `BernPlace` for an element on a rail: its fraction
    /// `at`, or `from` and `to`, and the spring it moves with.
    fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| match element.prop(name)? {
            Prop::Expr { expr } => Some(format!("({}).toDouble()", self.expr(expr))),
            Prop::Number { value } => Some(double(*value)),
            _ => None,
        };
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        if at.is_none() && from.is_none() && to.is_none() {
            return None;
        }
        let motion = element
            .keyword("animate")
            .map(|token| crate::style::motion(self.app, token))
            .unwrap_or_else(|| "null".into());
        let or_null = |value: Option<String>| value.unwrap_or_else(|| "null".into());
        Some(format!(
            "{}, {}, {}, {motion}",
            or_null(at),
            or_null(from),
            or_null(to)
        ))
    }

    /// Whether every element a rail places spans between two fractions
    /// without a size of its own across the rail, so that the rail has no
    /// thickness but what its parent gives it.
    fn rail_fills(rail: &Element) -> bool {
        let across = if rail.keyword("axis") == Some("vertical") {
            "width"
        } else {
            "height"
        };
        let mut placed = Vec::new();
        Self::placed(&rail.children, &mut placed)
            && !placed.is_empty()
            && placed.iter().all(|element| {
                element.prop("from").is_some()
                    && element.prop("at").is_none()
                    && element.prop(across).is_none()
            })
    }

    /// The elements a rail places among `nodes`, through conditions and
    /// repetitions, which add no layout of their own; false when one is a
    /// component, whose size is not known here.
    fn placed<'n>(nodes: &'n [Node], found: &mut Vec<&'n Element>) -> bool {
        nodes.iter().all(|node| match node {
            Node::Element(element) if element.kind == "ForEach" => {
                Self::placed(&element.children, found)
            }
            Node::Element(element) => {
                found.push(element);
                true
            }
            Node::Component(_) => false,
            Node::If(node) => {
                node.branches
                    .iter()
                    .all(|(_, body)| Self::placed(body, found))
                    && Self::placed(&node.otherwise, found)
            }
            Node::IfLet(node) => {
                Self::placed(&node.then, found) && Self::placed(&node.otherwise, found)
            }
            Node::Match(node) => node.arms.iter().all(|arm| Self::placed(&arm.body, found)),
        })
    }

    /// `bernExit(…)` for a transition name.
    fn exit(&self, name: Option<&str>) -> String {
        match name.and_then(|name| crate::style::transition(self.app, name)) {
            Some((transition, motion)) => format!("bernExit({transition}, {motion})"),
            None => "bernExit(null, null)".into(),
        }
    }

    /// A condition whose branches appear and disappear with their
    /// elements' transitions: the branch shown replaces the previous one in
    /// the same place, and shared elements pair up across the two.
    fn animated_if(&self, out: &mut Out, node: &IfNode, ctx: &Ctx) {
        out.open("run {");
        let mut branch = String::new();
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            branch.push_str(&format!("if ({}) {index} else ", self.expr(condition)));
        }
        branch.push_str(&node.branches.len().to_string());
        out.line(format!("val bernBranch = {branch}"));
        let bodies: Vec<&Vec<Node>> = node
            .branches
            .iter()
            .map(|(_, body)| body)
            .chain(std::iter::once(&node.otherwise))
            .collect();
        // Each branch enters with its first element's appearing transition
        // and leaves with its disappearing one; shared elements alone
        // cross-fade.
        let mut enters = Vec::new();
        let mut exits = Vec::new();
        for body in &bodies {
            let first = body.iter().find_map(|node| match node {
                Node::Element(element) => Some(element),
                _ => None,
            });
            let hero = body.iter().any(
                |node| matches!(node, Node::Element(element) if element.keyword("hero").is_some()),
            );
            let appear = first.and_then(|element| element.keyword("appear"));
            let disappear = first.and_then(|element| element.keyword("disappear"));
            let fade = |name: Option<&str>, enter: bool| match (name, hero) {
                (None, true) => format!(
                    "{}(BernTransition.Fade, {})",
                    if enter { "bernEnter" } else { "bernExit" },
                    crate::style::motion(self.app, "hero")
                ),
                (name, _) if enter => self.enter(name),
                (name, _) => self.exit(name),
            };
            enters.push(fade(appear, true));
            exits.push(fade(disappear, false));
        }
        out.line(format!("val bernEnters = listOf({})", enters.join(", ")));
        out.line(format!("val bernExits = listOf({})", exits.join(", ")));
        out.open("AnimatedContent(");
        out.line("targetState = bernBranch,");
        out.line("transitionSpec = { (bernEnters[targetState] togetherWith bernExits[initialState]).using(SizeTransform(clip = false)) },");
        out.line("label = \"branch\",");
        out.close(") { bernShown ->");
        out.indent += 1;
        let inner = Ctx {
            animated: true,
            ..ctx.clone()
        };
        out.open("CompositionLocalProvider(LocalBernVisibility provides this) {");
        out.open("when (bernShown) {");
        for (index, body) in bodies.iter().enumerate() {
            if body.is_empty() {
                continue;
            }
            out.open(format!("{index} -> {{"));
            if body.len() > 1 {
                out.open("Column {");
                self.nodes(out, body, &inner.within(Scope::Column));
                out.close("}");
            } else {
                self.nodes(out, body, &inner);
            }
            out.close("}");
        }
        out.line("else -> {}");
        out.close("}");
        out.close("}");
        out.close("}");
        out.close("}");
    }

    /// The direct children of a container: with `stagger`, those that
    /// appear take turns on the container's clock.
    fn children(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!(
                    "BernStaggered(rememberBernStagger({})) {{",
                    double(step)
                ));
                let turns = Ctx {
                    stagger: true,
                    ..ctx.clone()
                };
                self.nodes(out, &element.children, &turns);
                out.close("}");
            }
            None => self.nodes(out, &element.children, ctx),
        }
    }

    /// `.bernReflow(…)` for a container that declares `reflow`.
    fn reflow(&self, element: &Element) -> String {
        element
            .keyword("reflow")
            .map(|token| format!(".bernReflow({})", crate::style::motion(self.app, token)))
            .unwrap_or_default()
    }

    /// A length in dp: a literal, or an expression it follows, moving with
    /// the element's `animate` spring. Compose refuses negative padding and
    /// sizes, so a value below zero, or a spring overshooting zero, stops at
    /// zero.
    fn length(&self, element: &Element, name: &str) -> Option<String> {
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

    /// Whether an element below follows scrolling, also inside the
    /// components it places.
    fn scroll_effects(&self, nodes: &[Node]) -> bool {
        let mut found = false;
        let mut seen = std::collections::BTreeSet::new();
        let mut pending: Vec<&Node> = nodes.iter().collect();
        while let Some(node) = pending.pop() {
            node.walk(&mut |node| match node {
                Node::Element(element)
                    if element.prop("scroll_scale").is_some()
                        || element.prop("scroll_opacity").is_some() =>
                {
                    found = true;
                }
                Node::Component(reference) if seen.insert(reference.component.clone()) => {
                    if let Some(component) = self.app.component(&reference.component) {
                        pending.push(&component.body);
                    }
                }
                _ => {}
            });
        }
        found
    }

    /// `.width(…)` and `.height(…)` of a container that declares them.
    fn size_modifiers(&self, element: &Element) -> String {
        let mut modifiers = String::new();
        if let Some(width) = self.length(element, "width") {
            modifiers.push_str(&format!(".width({width})"));
        }
        if let Some(height) = self.length(element, "height") {
            modifiers.push_str(&format!(".height({height})"));
        }
        modifiers
    }

    /// The file a control takes (`pick:`, or a `Drop`'s `value:`): the
    /// lambda that stores it and runs `action`, and the control actions
    /// that read it and take one from an external entry.
    fn file_target(&self, element: &Element, name: &str, action: &str) -> Option<(String, String)> {
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
    fn pressed(&self, element: &Element) -> (String, String) {
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

    /// The element's keyboard shortcut running `action`.
    fn shortcut(&self, out: &mut Out, element: &Element, action: &str) {
        if let Some(shortcut) = element.keyword("shortcut") {
            out.line(format!("BernShortcut({}) {{ {action} }}", string(shortcut)));
        }
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode, body: &dyn Fn(&mut Out, &[Node])) {
        let var = member(&node.var);
        out.open(format!("({}).let {{ {var} ->", self.expr(&node.value)));
        out.open(format!("if ({var} != null) {{"));
        body(out, &node.then);
        if !node.otherwise.is_empty() {
            out.close("} else {");
            out.indent += 1;
            body(out, &node.otherwise);
        }
        out.close("}");
        out.close("}");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode, body: &dyn Fn(&mut Out, &[Node])) {
        let def = self.types().enum_def(&node.enum_name).expect("enum");
        out.open(format!(
            "when (val bernValue = {}) {{",
            self.expr(&node.value)
        ));
        for arm in &node.arms {
            out.open(format!("{} -> {{", case_pattern(def, arm)));
            for line in arm_bindings(def, arm) {
                out.line(line);
            }
            body(out, &arm.body);
            out.close("}");
        }
        out.close("}");
    }

    fn style_of_text(element: &Element) -> &'static str {
        match element.keyword("style") {
            Some("large_title") => "headlineLarge",
            Some("title") => "headlineSmall",
            Some("title2") => "titleLarge",
            Some("title3") => "titleMedium",
            Some("headline") => "titleSmall",
            Some("subheadline") => "bodyMedium",
            Some("callout") => "bodyMedium",
            Some("footnote") => "bodySmall",
            Some("caption") => "labelMedium",
            Some("caption2") => "labelSmall",
            _ => "bodyLarge",
        }
    }

    fn element(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        match element.kind.as_str() {
            "Text" => {
                let content = self.text_prop(element, "content").unwrap_or_default();
                let color = match element.keyword("emphasis") {
                    Some("secondary") => "MaterialTheme.colorScheme.onSurfaceVariant",
                    Some("tertiary") => {
                        "MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)"
                    }
                    _ => "Color.Unspecified",
                };
                let style = format!("MaterialTheme.typography.{}", Self::style_of_text(element));
                match element.keyword("mark") {
                    Some(role) => out.line(format!(
                        "BernMarkedText({content}, {style}, {color}, BernColor.Role({}))",
                        string(role)
                    )),
                    None => out.line(format!("Text({content}, style = {style}, color = {color})")),
                }
            }
            "Canvas" => self.canvas(out, element),
            "Label" => {
                let title = self.text_prop(element, "title").unwrap_or_default();
                out.line(format!(
                    "BernLabel({title}, {}, MaterialTheme.typography.{})",
                    Self::icon(element.icon("icon")),
                    Self::style_of_text(element)
                ));
            }
            "Button" | "IconButton" => self.button(out, element, ctx),
            "TextInput" => self.text_input(out, element, ctx),
            "Search" => {
                let field = self.field(element, "value").expect("search field");
                let prompt = self
                    .text_prop(element, "prompt")
                    .unwrap_or_else(|| "\"\"".into());
                let action = element.action.as_ref().map(|a| self.action(a));
                let target = self.target(element, ctx);
                let submit = action
                    .as_ref()
                    .map(|action| format!(", submit = {{ {action} }}"))
                    .unwrap_or_default();
                out.line(format!(
                    "BernControl({target}, \"search\", {prompt}, actions = BernControlActions(edit = {{ {field} = it }}{submit}, value = {{ {field} }}))"
                ));
                out.line(format!(
                    "BernSearchField({field}, {{ {field} = it }}, {prompt}, {}, Modifier.testTag({target}))",
                    action
                        .map(|action| format!("{{ {action} }}"))
                        .unwrap_or_else(|| "null".into())
                ));
                // Results show below the field while there is a search.
                if !element.children.is_empty() {
                    out.open(format!("if ({field}.isNotEmpty()) {{"));
                    out.open("Column(Modifier.fillMaxWidth()) {");
                    self.nodes(out, &element.children, &ctx.within(Scope::Column));
                    out.close("}");
                    out.close("}");
                }
            }
            "Toast" => {
                let content = self.text_prop(element, "content").unwrap_or_default();
                let key = element
                    .expr("key")
                    .map(|key| self.expr(key))
                    .unwrap_or_else(|| content.clone());
                out.line(format!(
                    "BernToast({content}, {}, {key}, {}, {})",
                    Self::icon(element.icon("icon")),
                    double(element.number("duration").unwrap_or(3.0)),
                    crate::style::motion(self.app, "bouncy")
                ));
            }
            "Inspector" => {
                let field = self.field(element, "presented").expect("bool field");
                out.open(format!(
                    "BernInspector({field}, onDismiss = {{ {field} = false }}) {{"
                ));
                self.nodes(out, &element.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            "SplitButton" => {
                let field = self.field(element, "value").expect("bool field");
                let label = self.text_prop(element, "label").unwrap_or_default();
                let more = self
                    .text_prop(element, "more_label")
                    .unwrap_or_else(|| "BernText.string(\"bern.split.more\")".into());
                self.shortcut(out, element, &format!("{field} = !{field}"));
                out.open(format!(
                    "BernSplitButton({}, {label}, {}, {more}, {field}, {{ {field} = it }}) {{",
                    self.target(element, ctx),
                    Self::icon(element.icon("icon"))
                ));
                self.nodes(out, &element.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            "PopoverButton" => self.popover_button(out, element, ctx, false),
            // Shown by its sidebar.
            "SidebarFooter" => {}
            "Drop" => {
                let label = self.text_prop(element, "label").unwrap_or_default();
                let action = element
                    .action
                    .as_ref()
                    .map(|a| self.action(a))
                    .unwrap_or_else(|| "Unit".into());
                let (give, actions) = self
                    .file_target(element, "value", &action)
                    .expect("file field");
                let over = self
                    .field(element, "over")
                    .map(|field| format!("{{ over: Boolean -> {field} = over }}"))
                    .unwrap_or_else(|| "null".into());
                out.line(format!(
                    "BernControl({}, \"drop\", {label}, actions = BernControlActions({actions}))",
                    self.target(element, ctx)
                ));
                out.open(format!(
                    "BernDrop({}, {over}, {}, {give}) {{",
                    string(element.keyword("files").unwrap_or("any")),
                    element.keyword("opens") == Some("true")
                ));
                self.nodes(out, &element.children, &ctx.within(Scope::Other));
                out.close("}");
            }
            "Switch" => {
                let field = self.field(element, "value").expect("bool field");
                let label = self.text_prop(element, "label").unwrap_or_default();
                let target = self.target(element, ctx);
                out.line(format!(
                    "BernControl({target}, \"switch\", {label}, actions = BernControlActions(setBool = {{ {field} = it }}, value = {{ if ({field}) \"true\" else \"false\" }}))"
                ));
                self.shortcut(out, element, &format!("{field} = !{field}"));
                if element.keyword("style") == Some("button") {
                    out.line(format!(
                        "BernToggleButton({label}, {}, {field}, {{ {field} = it }}, Modifier.testTag({target}))",
                        Self::icon(element.icon("icon"))
                    ));
                } else {
                    out.line(format!(
                        "BernSwitch({label}, {field}, {{ {field} = it }}, checkbox = {}, modifier = Modifier.testTag({target}))",
                        element.keyword("style") == Some("checkbox")
                    ));
                }
            }
            "Slider" => self.slider(out, element, ctx),
            "Picker" => self.picker(out, element, ctx),
            "Progress" => {
                let label = self
                    .text_prop(element, "label")
                    .unwrap_or_else(|| "null".into());
                let value = element
                    .expr("value")
                    .map(|value| format!("({}).toDouble()", self.expr(value)))
                    .unwrap_or_else(|| "null".into());
                out.line(format!(
                    "BernProgress({value}, {}, {label})",
                    double(element.number("total").unwrap_or(1.0))
                ));
            }
            "Form" => {
                if let Some(title) = self.text_prop(element, "title") {
                    out.line(format!("BernChrome(title = {title})"));
                }
                out.open("Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {");
                self.nodes(out, &element.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            "Section" => {
                let title = self
                    .text_prop(element, "title")
                    .unwrap_or_else(|| "null".into());
                let footer = self
                    .text_prop(element, "footer")
                    .unwrap_or_else(|| "null".into());
                out.open(format!("BernSection({title}, {footer}) {{"));
                self.nodes(out, &element.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            "Column" | "Row" => {
                let column = element.kind == "Column";
                let gap = self
                    .length(element, "gap")
                    .map(|gap| format!("Arrangement.spacedBy({gap})"))
                    .unwrap_or_else(|| "Arrangement.spacedBy(8.dp)".into());
                let mut modifier = format!(
                    "Modifier{}{}",
                    self.reflow(element),
                    self.size_modifiers(element)
                );
                if let Some(padding) = self.length(element, "padding") {
                    modifier.push_str(&format!(".padding({padding})"));
                }
                if column {
                    let alignment = match element.keyword("align") {
                        Some("center") => "Alignment.CenterHorizontally",
                        Some("trailing") => "Alignment.End",
                        _ => "Alignment.Start",
                    };
                    out.open(format!(
                        "Column({modifier}, verticalArrangement = {gap}, horizontalAlignment = {alignment}) {{"
                    ));
                    self.children(out, element, &ctx.within(Scope::Column));
                } else {
                    let alignment = match element.keyword("align") {
                        Some("top") => "Alignment.Top",
                        Some("bottom") => "Alignment.Bottom",
                        _ => "Alignment.CenterVertically",
                    };
                    out.open(format!(
                        "Row({modifier}, horizontalArrangement = {gap}, verticalAlignment = {alignment}) {{"
                    ));
                    self.children(out, element, &ctx.within(Scope::Row));
                }
                out.close("}");
            }
            "Grid" => {
                out.open(format!(
                    "BernGrid(minWidth = {}, gap = {}{}) {{",
                    self.length(element, "min_width")
                        .unwrap_or_else(|| "120.dp".into()),
                    self.length(element, "gap").unwrap_or_else(|| "8.dp".into()),
                    self.length(element, "padding")
                        .map(|padding| format!(", modifier = Modifier.padding({padding})"))
                        .unwrap_or_default()
                ));
                self.children(out, element, &ctx.within(Scope::Other));
                out.close("}");
            }
            "Stack" => {
                let alignment = match element.keyword("align") {
                    Some("top_leading") => "Alignment.TopStart",
                    Some("top") => "Alignment.TopCenter",
                    Some("top_trailing") => "Alignment.TopEnd",
                    Some("leading") => "Alignment.CenterStart",
                    Some("trailing") => "Alignment.CenterEnd",
                    Some("bottom_leading") => "Alignment.BottomStart",
                    Some("bottom") => "Alignment.BottomCenter",
                    Some("bottom_trailing") => "Alignment.BottomEnd",
                    _ => "Alignment.Center",
                };
                out.open(format!(
                    "Box(Modifier{}{}, contentAlignment = {alignment}) {{",
                    self.reflow(element),
                    self.size_modifiers(element)
                ));
                let layers = Ctx {
                    layer: true,
                    ..ctx.within(Scope::Other)
                };
                self.children(out, element, &layers);
                out.close("}");
            }
            "Rail" => {
                let anchor = match element.keyword("anchor") {
                    Some("start") => "BernRailAnchor.Start",
                    Some("end") => "BernRailAnchor.End",
                    _ => "BernRailAnchor.Center",
                };
                // A layer of a stack holding only spans without a
                // thickness of their own is as large as the stack.
                let modifier = if ctx.layer {
                    ", Modifier.matchParentSize()"
                } else {
                    ""
                };
                out.open(format!(
                    "BernRail({}, {anchor}{modifier}) {{",
                    element.keyword("axis") == Some("vertical")
                ));
                let placed = Ctx {
                    layer: false,
                    ..ctx.within(Scope::Other)
                };
                self.nodes(out, &element.children, &placed);
                out.close("}");
            }
            "Svg" => {
                let value = |name, default: &str| {
                    element
                        .expr(name)
                        .map(|e| self.expr(e))
                        .unwrap_or_else(|| default.into())
                };
                out.line(format!(
                    "BernSvg({}, {}, {}, {}, {}, {}, ({}).toDouble(), ({}).toDouble(), {})",
                    string(element.resource("source").unwrap_or_default()),
                    self.text_prop(element, "label")
                        .unwrap_or_else(|| "null".into()),
                    self.length(element, "width")
                        .unwrap_or_else(|| "0.dp".into()),
                    self.length(element, "height")
                        .unwrap_or_else(|| "0.dp".into()),
                    string(element.keyword("fit").unwrap_or("contain")),
                    value("playing", "true"),
                    value("speed", "1"),
                    value("restart", "0"),
                    element.keyword("interactive").unwrap_or("true"),
                ));
            }
            "Image" => {
                let length = |name| self.length(element, name).unwrap_or_else(|| "null".into());
                let fit = match element.keyword("fit") {
                    Some("cover") => "ContentScale.Crop",
                    Some("fill") => "ContentScale.FillBounds",
                    _ => "ContentScale.Fit",
                };
                // A picture the engine gives, read at run time.
                if let Some(source) = element.expr("source") {
                    let url = if matches!(source.ty, Type::Optional { .. }) {
                        format!("({})?.url", self.expr(source))
                    } else {
                        format!("({}).url", self.expr(source))
                    };
                    out.line(format!(
                        "BernImageURL({url}, {}, {}, {}, {fit})",
                        self.text_prop(element, "label")
                            .unwrap_or_else(|| "null".into()),
                        length("width"),
                        length("height"),
                    ));
                    return;
                }
                out.line(format!(
                    "BernImage({}, {}, {}, {}, {fit})",
                    string(element.resource("source").unwrap_or_default()),
                    self.text_prop(element, "label")
                        .unwrap_or_else(|| "null".into()),
                    length("width"),
                    length("height"),
                ));
            }
            "Spacer" => match ctx.scope {
                Scope::Column | Scope::Row => out.line("Spacer(Modifier.weight(1f))"),
                Scope::Other => out.line("Spacer(Modifier.size(8.dp))"),
            },
            "Divider" => match ctx.scope {
                Scope::Row => out.line("VerticalDivider()"),
                _ => out.line("HorizontalDivider()"),
            },
            "ScrollView"
                if element.prop("snap").is_some()
                    || element.prop("position").is_some()
                    || self.scroll_effects(&element.children) =>
            {
                let horizontal = element.keyword("axis") == Some("horizontal");
                let snap = element
                    .keyword("snap")
                    .map(string)
                    .unwrap_or_else(|| "null".into());
                let (position, on_position) =
                    match (self.field(element, "position"), element.field("position")) {
                        (Some(field), Some(field_ref)) => (
                            field.clone(),
                            format!("{{ {field} = it as {} }}", kotlin_type(&field_ref.ty)),
                        ),
                        _ => ("null".into(), "null".into()),
                    };
                let arrival = if element.keyword("arrival").is_some() {
                    crate::style::motion(self.app, "jelly")
                } else {
                    "null".into()
                };
                out.open(format!(
                    "BernScroll({horizontal}, {snap}, {position}, {on_position}, {}, {arrival}, 0.dp) {{",
                    crate::style::motion(self.app, "smooth")
                ));
                let inner = Ctx {
                    scroll_items: true,
                    ..ctx.within(if horizontal {
                        Scope::Row
                    } else {
                        Scope::Column
                    })
                };
                self.nodes(out, &element.children, &inner);
                out.close("}");
            }
            "Pinch" | "Swipe" => {
                let field = self.field(element, "value").expect("number field");
                let label = self.text_prop(element, "label").unwrap_or_default();
                let action = element
                    .action
                    .as_ref()
                    .map(|a| self.action(a))
                    .unwrap_or_else(|| "Unit".into());
                let min = double(element.number("min").unwrap_or(0.0));
                let max = double(element.number("max").unwrap_or(1.0));
                out.line(format!(
                    "BernControl({}, {}, {label}, actions = BernControlActions(setNumber = {{ {field} = it.coerceIn({min}, {max}); {action} }}, value = {{ {field}.toString() }}))",
                    self.target(element, ctx),
                    string(&element.kind.to_lowercase())
                ));
                if element.kind == "Pinch" {
                    out.open(format!(
                        "BernPinch({label}, {field}, {{ {field} = it }}, {min}, {max}, {{ {action} }}) {{"
                    ));
                } else {
                    out.open(format!(
                        "BernSwipe({label}, {field}, {{ {field} = it }}, {min}, {max}, {}, {}, {{ {action} }}) {{",
                        element.keyword("axis") == Some("horizontal"),
                        double(element.number("distance").unwrap_or(300.0))
                    ));
                }
                self.nodes(out, &element.children, &ctx.within(Scope::Other));
                out.close("}");
            }
            "ScrollView" => {
                if element.keyword("axis") == Some("horizontal") {
                    out.open("Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {");
                    self.nodes(out, &element.children, &ctx.within(Scope::Row));
                } else {
                    out.open(
                        "Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {",
                    );
                    self.nodes(out, &element.children, &ctx.within(Scope::Column));
                }
                out.close("}");
            }
            "List" | "ForEach" => self.list(out, element, ctx),
            "Query" => self.query(out, element, ctx),
            "Video" => {
                let label = self
                    .text_prop(element, "label")
                    .unwrap_or_else(|| "null".into());
                let binding = match (element.expr("source"), element.prop("player")) {
                    (Some(source), _) => {
                        let value = self.expr(source);
                        if matches!(source.ty, Type::Optional { .. }) {
                            format!("BernVideoBinding.Source(({value})?.url)")
                        } else {
                            format!("BernVideoBinding.Source(({value}).url)")
                        }
                    }
                    (None, Some(Prop::Player { id })) => {
                        format!("BernVideoBinding.Player({})", string(id))
                    }
                    _ => "BernVideoBinding.Source(null)".into(),
                };
                let fit = match element.keyword("fit") {
                    Some("cover") => "BernVideoFit.Cover",
                    Some("fill") => "BernVideoFit.Fill",
                    _ => "BernVideoFit.Contain",
                };
                out.line(format!(
                    "BernVideo({}, {label}, {binding}, {}, {fit}{})",
                    self.target(element, ctx),
                    element.keyword("controls") != Some("none"),
                    self.length(element, "corner")
                        .map(|corner| format!(", corner = {corner}"))
                        .unwrap_or_default()
                ));
            }
            "SplitView" => self.split_view(out, element, ctx),
            "Navigation" => self.navigation(out, element, ctx),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| self.expr(route))
                    .unwrap_or_default();
                let Some(path) = &ctx.path else { return };
                let target = self.target(element, ctx);
                out.line(format!(
                    "BernControl({target}, \"link\", {}, actions = BernControlActions(activate = {{ {path} = {path} + {route} }}))",
                    self.row_label(&element.children).unwrap_or_else(|| "\"\"".into())
                ));
                out.open(format!(
                    "BernNavLink(onClick = {{ {path} = {path} + {route} }}, modifier = Modifier.testTag({target})) {{"
                ));
                self.nodes(out, &element.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            "Tabs" => self.tabs(out, element, ctx),
            "Toolbar" => {
                out.open("BernChrome(actions = {");
                self.nav_walk(out, &element.children, &|out, item| {
                    self.toolbar_item(out, item, ctx)
                });
                out.close("})");
            }
            "Menu" => self.menu(out, element, ctx),
            "Adaptive" => self.adaptive(out, element, ctx),
            "LanguagePicker" => {
                let label = self.text_prop(element, "label").unwrap_or_default();
                out.line(format!(
                    "BernLanguagePicker({label}, {})",
                    self.target(element, ctx)
                ));
            }
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "WindowSet"
            | "Window" | "Layout" | "MenuItem" => {
                // Realized by their parents.
            }
            other => out.line(format!("// unsupported {other}")),
        }
    }

    /// A `Canvas`: shapes, series and patterns in order, in canvas dp.
    fn canvas(&self, out: &mut Out, element: &Element) {
        let color = |element: &Element, name: &str| {
            element
                .keyword(name)
                .map(|role| format!("BernColor.Role({})", string(role)))
                .unwrap_or_else(|| "null".into())
        };
        let number = |element: &Element, name: &str, default: f64| {
            double(element.number(name).unwrap_or(default))
        };
        let length = |name| self.length(element, name).unwrap_or_else(|| "null".into());
        let semantics = match self.text_prop(element, "label") {
            Some(label) => {
                format!(".semantics {{ contentDescription = {label}; role = Role.Image }}")
            }
            None => ".clearAndSetSemantics {}".into(),
        };
        out.open("run {");
        out.line("val bernPalette = bernPalette()");
        // The average colors of the pictures shapes are filled from, read
        // while composing so the drawing follows them once known.
        for (index, node) in element.children.iter().enumerate() {
            let Node::Element(child) = node else { continue };
            let url = match (child.expr("fill_from"), child.resource("fill_from")) {
                (Some(expr), _) if matches!(expr.ty, Type::Optional { .. }) => {
                    format!("({})?.url", self.expr(expr))
                }
                (Some(expr), _) => format!("({}).url", self.expr(expr)),
                (None, Some(name)) => format!("BernPictures.resource({})", string(name)),
                (None, None) => continue,
            };
            out.line(format!(
                "val bernFill{index} = BernPictures.average(LocalContext.current, {url})"
            ));
        }
        let mut values = Vec::new();
        for node in &element.children {
            if let Node::Element(child) = node
                && child.kind == "Series"
                && let Some(list) = child.expr("values")
            {
                let index = values.len();
                let read = match &list.ty {
                    Type::List { inner } if matches!(**inner, Type::Int) => {
                        format!("({}).map {{ it.toDouble() }}", self.expr(list))
                    }
                    _ => self.expr(list),
                };
                out.line(format!("val bernValues{index} = {read}"));
                let max = child
                    .expr("max")
                    .map(|max| format!("({}).toDouble()", self.expr(max)))
                    .unwrap_or_else(|| "null".into());
                out.line(format!("val bernMax{index}: Double? = {max}"));
                values.push(index);
            }
        }
        out.open(format!(
            "Canvas(Modifier.bernFrame({}, {}){semantics}) {{",
            length("width"),
            length("height")
        ));
        let mut series = 0;
        for (index, node) in element.children.iter().enumerate() {
            // A canvas holds its drawing elements directly.
            let Node::Element(child) = node else { continue };
            match child.kind.as_str() {
                "Shape" => {
                    let filled = child.prop("fill_from").is_some();
                    let fill = match (filled, color(child, "fill")) {
                        (false, fill) => fill,
                        (true, fallback) if fallback == "null" => format!("bernFill{index}"),
                        (true, fallback) => format!("(bernFill{index} ?: {fallback})"),
                    };
                    let fade = child
                        .keyword("fade")
                        .map(|edge| format!(", fade = {}", string(edge)))
                        .unwrap_or_default();
                    let optional = |name: &str| {
                        child
                            .number(name)
                            .map(double)
                            .unwrap_or_else(|| "null".into())
                    };
                    out.line(format!(
                        "BernDraw.shape(this, bernPalette, {}, {}, {}, {}, {}, {}, {fill}, {}, {}, {}{fade})",
                        string(child.keyword("kind").unwrap_or("rect")),
                        number(child, "x", 0.0),
                        number(child, "y", 0.0),
                        optional("width"),
                        optional("height"),
                        number(child, "corner", 8.0),
                        color(child, "stroke"),
                        number(child, "stroke_width", 1.0),
                        number(child, "opacity", 1.0),
                    ));
                }
                "Series" => {
                    out.line(format!(
                        "BernDraw.series(this, bernPalette, bernValues{series}, bernMax{series}, {}, {}, {}, {}, {})",
                        child.keyword("kind") == Some("area"),
                        color(child, "fill"),
                        color(child, "stroke"),
                        number(child, "stroke_width", 2.0),
                        number(child, "opacity", 1.0),
                    ));
                    series += 1;
                }
                "Pattern" => out.line(format!(
                    "BernDraw.pattern(this, bernPalette, BernPatterns.{}, {})",
                    member(child.keyword("name").expect("pattern name")),
                    number(child, "opacity", 1.0),
                )),
                _ => {}
            }
        }
        out.close("}");
        out.close("}");
    }

    fn button(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, file) = self.pressed(element);
        let enabled = self.enabled(element);
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"button\", {label}, {enabled}, BernControlActions(activate = {{ {action} }}{file}))"
        ));
        self.shortcut(out, element, &action);
        if element.kind == "Button" && !element.children.is_empty() {
            // The content is what the button shows; the label names it.
            let style = self
                .style_of(element)
                .map(|style| format!("BernStyles.{}", style.name))
                .unwrap_or_else(|| "null".into());
            out.open(format!(
                "BernContentButton({label}, {enabled}, {style}, Modifier.testTag({target}), onClick = {{ {action} }}) {{"
            ));
            self.nodes(out, &element.children, &ctx.within(Scope::Other));
            out.close("}");
            return;
        }
        let style = self.style_arg(element);
        if element.kind == "IconButton" {
            out.line(format!(
                "BernIconButton({label}, {}, {enabled}, Modifier.testTag({target}){style}) {{ {action} }}",
                Self::icon(element.icon("icon"))
            ));
            return;
        }
        let prominence = match element.keyword("prominence") {
            Some("prominent") => "BernProminence.Prominent",
            Some("plain") => "BernProminence.Plain",
            _ => "BernProminence.Standard",
        };
        out.line(format!(
            "BernButton({label}, {}, {prominence}, destructive = {}, enabled = {enabled}{style}, modifier = Modifier.testTag({target})) {{ {action} }}",
            Self::icon(element.icon("icon")),
            element.keyword("role") == Some("destructive")
        ));
    }

    fn text_input(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("text field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "null".into());
        let kind = element.keyword("kind").unwrap_or("plain");
        let action = element.action.as_ref().map(|a| self.action(a));
        let target = self.target(element, ctx);
        let submit = action
            .as_ref()
            .map(|action| format!(", submit = {{ {action} }}"))
            .unwrap_or_default();
        // Secure text is never exposed to external entries.
        let value = if kind == "secure" {
            String::new()
        } else {
            format!(", value = {{ {field} }}")
        };
        out.line(format!(
            "BernControl({target}, \"text_input\", {label}, actions = BernControlActions(edit = {{ {field} = it }}{submit}{value}))"
        ));
        out.line(format!(
            "BernTextInput({field}, {{ {field} = it }}, {label}, {prompt}, {}, {}, Modifier.testTag({target}))",
            string(kind),
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into())
        ));
    }

    fn slider(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("number field");
        let integer = matches!(
            element.field("value").map(|field| &field.ty),
            Some(Type::Int)
        );
        let label = self.text_prop(element, "label").unwrap_or_default();
        let min = double(element.number("min").unwrap_or(0.0));
        let max = double(element.number("max").unwrap_or(1.0));
        let step = element
            .number("step")
            .map(double)
            .unwrap_or_else(|| "null".into());
        let action = element.action.as_ref().map(|a| self.action(a));
        let target = self.target(element, ctx);
        let (read, write) = if integer {
            (
                format!("{field}.toDouble()"),
                format!("{field} = Math.round(it)"),
            )
        } else {
            (field.clone(), format!("{field} = it"))
        };
        let commit = action
            .as_ref()
            .map(|action| format!("; {action}"))
            .unwrap_or_default();
        out.line(format!(
            "BernControl({target}, \"slider\", {label}, actions = BernControlActions(setNumber = {{ {write}{commit} }}, value = {{ BernText.text({field}) }}))"
        ));
        out.line(format!(
            "BernSlider({label}, {read}, {{ {write} }}, {min}, {max}, {step}, {}, {}, {}, Modifier.testTag({target}))",
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into()),
            Self::icon(element.icon("min_icon")),
            Self::icon(element.icon("max_icon"))
        ));
    }

    fn picker(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field_ref = element.field("value").expect("picker field");
        let field = self.field(element, "value").expect("picker field");
        let label = self.text_prop(element, "label").unwrap_or_default();
        let enum_type = kotlin_type(&field_ref.ty);
        let labeled = match &field_ref.ty {
            Type::Enum { name } => self
                .types()
                .enum_def(name)
                .is_some_and(|def| def.variants.iter().all(|variant| variant.label.is_some())),
            _ => false,
        };
        let option_label = if labeled {
            "it.bernLabel"
        } else {
            "it.bernName"
        };
        let style = element.keyword("style").unwrap_or("menu");
        let target = self.target(element, ctx);
        out.line(format!(
            "BernControl({target}, \"picker\", {label}, actions = BernControlActions(select = {{ name -> {enum_type}.fromBernName(name)?.let {{ {field} = it; true }} ?: false }}, value = {{ {field}.bernName }}))"
        ));
        out.line(format!(
            "BernPicker({label}, {enum_type}.entries, {field}, {{ {option_label} }}, {{ {field} = it }}, {}, Modifier.testTag({target}))",
            string(style)
        ));
    }

    fn list(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let lazy = element.kind == "List";
        // With `stagger`, the rows take turns appearing; without, rows of a
        // repetition are direct children of the container it stands in.
        let stagger = element.number("stagger");
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                let var = member(&template.var);
                let key = format!("{var}.{}", member(name));
                let mut inner = ctx.clone();
                inner.key = Some(key.clone());
                if stagger.is_some() {
                    inner.stagger = true;
                }
                // The clock reaches the rows of a lazy list too: they are
                // composed with the list's locals.
                if let Some(step) = stagger {
                    out.open(format!(
                        "BernStaggered(rememberBernStagger({})) {{",
                        double(step)
                    ));
                }
                if lazy {
                    out.open("LazyColumn(Modifier.fillMaxSize()) {");
                    out.open(format!(
                        "items({}, key = {{ {var} -> BernKey.of({key}) }}) {{ {var} ->",
                        self.expr(items)
                    ));
                    out.open(format!(
                        "CompositionLocalProvider(LocalBernItemKeys provides LocalBernItemKeys.current + \"[${{{key}}}]\") {{"
                    ));
                    out.open("Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {");
                    self.nodes(out, &element.children, &inner.within(Scope::Column));
                    out.close("}");
                    out.close("}");
                    out.line("HorizontalDivider()");
                    out.close("}");
                    out.close("}");
                } else {
                    out.open(format!("for ({var} in {}) {{", self.expr(items)));
                    out.open(format!("key(BernKey.of({key})) {{"));
                    out.open(format!(
                        "CompositionLocalProvider(LocalBernItemKeys provides LocalBernItemKeys.current + \"[${{{key}}}]\") {{"
                    ));
                    if ctx.scroll_items {
                        inner.scroll_items = false;
                        out.open(format!("Box(Modifier.bernScrollItem({key})) {{"));
                        self.nodes(out, &element.children, &inner.within(Scope::Other));
                        out.close("}");
                    } else {
                        self.nodes(out, &element.children, &inner);
                    }
                    out.close("}");
                    out.close("}");
                    out.close("}");
                }
                if stagger.is_some() {
                    out.close("}");
                }
            }
            _ => {
                if lazy {
                    out.open(
                        "Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {",
                    );
                    self.children(out, element, &ctx.within(Scope::Column));
                    out.close("}");
                } else {
                    self.children(out, element, ctx);
                }
            }
        }
    }

    /// A `Query`: the runtime view owns the subscription and shows the
    /// loading, refreshing, failed and empty states; the template shows the
    /// data. Its parameters are encoded on every composition; the runtime
    /// requests them only when they change.
    fn query(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let data = kotlin_type(&binding.result);
        let encode: Vec<_> = binding
            .args
            .iter()
            .map(|arg| encode(&arg.ty, &format!("({})", self.expr(arg)), "writer"))
            .collect();
        let params = format!(
            "run {{ val writer = BernWriter(); {}; writer.bytes() }}",
            encode.join("; ")
        );
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
        let (shown, count) = match &binding.result {
            Type::List { .. } if empty.is_some() => (
                "if (data.isEmpty()) null else data",
                format!("{{ data: {data} -> data.size }}"),
            ),
            Type::List { .. } => ("data", format!("{{ data: {data} -> data.size }}")),
            Type::Optional { .. } => (
                "data",
                format!("{{ data: {data} -> if (data == null) 0 else 1 }}"),
            ),
            _ => ("data", "null".to_string()),
        };
        out.open("BernQueryView(");
        out.line(format!("{}, {},", binding.query, self.target(element, ctx)));
        out.line(format!("{params},"));
        out.line(format!(
            "{loading}, {}, {}, {failed},",
            empty.unwrap_or_else(|| "null".into()),
            string(icon)
        ));
        out.line(format!(
            "{{ reader -> {} }},",
            decode(&binding.result, "reader")
        ));
        out.line(format!("{count},"));
        out.line(format!("{{ data: {data} -> {shown} }},"));
        out.close(format!(") {{ {} ->", member(&template.var)));
        out.indent += 1;
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    fn menu(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        out.line(format!(
            "BernControl({}, \"menu\", {label}, actions = BernControlActions())",
            self.target(element, ctx)
        ));
        let items: Vec<&Element> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(item) => Some(item),
                _ => None,
            })
            .collect();
        for item in items.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, file) = self.pressed(item);
            out.line(format!(
                "BernControl({}, \"button\", {item_label}, {}, BernControlActions(activate = {{ {action} }}{file}))",
                self.target(item, ctx),
                self.enabled(item)
            ));
            self.shortcut(out, item, &action);
        }
        out.open(format!(
            "BernMenu({label}, {}, Modifier.testTag({})) {{ dismiss ->",
            Self::icon(element.icon("icon")),
            self.target(element, ctx)
        ));
        for item in &items {
            if item.kind == "Divider" {
                out.line("HorizontalDivider()");
                continue;
            }
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, _) = self.pressed(item);
            out.line(format!(
                "BernMenuItem({item_label}, {}, {}, destructive = {}) {{ dismiss(); {action} }}",
                Self::icon(item.icon("icon")),
                self.enabled(item),
                item.keyword("role") == Some("destructive")
            ));
        }
        out.close("}");
    }

    /// A button that shows its children in a popover; in a top bar a
    /// label shows as a text button.
    fn popover_button(&self, out: &mut Out, element: &Element, ctx: &Ctx, plain: bool) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (presented, on_presented) = match self.field(element, "presented") {
            Some(field) => (field.clone(), format!("{{ {field} = it }}")),
            None => ("null".into(), "null".into()),
        };
        let shortcut = element
            .keyword("shortcut")
            .map(string)
            .unwrap_or_else(|| "null".into());
        out.open(format!(
            "BernPopoverButton({}, {label}, {}, {presented}, {on_presented}, {shortcut}, {plain}) {{",
            self.target(element, ctx),
            Self::icon(element.icon("icon"))
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    fn toolbar_item(&self, out: &mut Out, item: &Element, ctx: &Ctx) {
        match item.kind.as_str() {
            "PopoverButton" => self.popover_button(out, item, ctx, true),
            // A top app bar parts its groups of actions with space.
            "Divider" => out.line("Spacer(Modifier.size(12.dp))"),
            "Button" => {
                let label = self.text_prop(item, "label").unwrap_or_default();
                let (action, file) = self.pressed(item);
                let enabled = self.enabled(item);
                let target = self.target(item, ctx);
                out.line(format!(
                    "BernControl({target}, \"button\", {label}, {enabled}, BernControlActions(activate = {{ {action} }}{file}))"
                ));
                self.shortcut(out, item, &action);
                match item.icon("icon") {
                    Some(icon) => out.line(format!(
                        "BernIconButton({label}, {}, {enabled}, Modifier.testTag({target})) {{ {action} }}",
                        Self::icon(Some(icon))
                    )),
                    None => out.line(format!(
                        "TextButton(onClick = {{ {action} }}, enabled = {enabled}, modifier = Modifier.testTag({target})) {{ Text({label}) }}"
                    )),
                }
            }
            "Picker" => {
                let field_ref = item.field("value").expect("picker field");
                let field = self.field(item, "value").expect("picker field");
                let label = self.text_prop(item, "label").unwrap_or_default();
                let enum_type = kotlin_type(&field_ref.ty);
                let labeled = match &field_ref.ty {
                    Type::Enum { name } => self.types().enum_def(name).is_some_and(|def| {
                        def.variants.iter().all(|variant| variant.label.is_some())
                    }),
                    _ => false,
                };
                let option_label = if labeled {
                    "it.bernLabel"
                } else {
                    "it.bernName"
                };
                let target = self.target(item, ctx);
                out.line(format!(
                    "BernControl({target}, \"picker\", {label}, actions = BernControlActions(select = {{ name -> {enum_type}.fromBernName(name)?.let {{ {field} = it; true }} ?: false }}, value = {{ {field}.bernName }}))"
                ));
                out.line(format!(
                    "BernToolbarPicker({label}, {enum_type}.entries, {field}, {{ {option_label} }}, {{ {field} = it }}, Modifier.testTag({target}))"
                ));
            }
            "Search" => {
                let field = self.field(item, "value").expect("search field");
                let prompt = self
                    .text_prop(item, "prompt")
                    .unwrap_or_else(|| "\"\"".into());
                let submit = item
                    .action
                    .as_ref()
                    .map(|a| format!("{{ {} }}", self.action(a)))
                    .unwrap_or_else(|| "null".into());
                let target = self.target(item, ctx);
                out.open(format!(
                    "BernToolbarSearch({target}, {field}, {{ {field} = it }}, {prompt}, {submit}) {{"
                ));
                self.nodes(out, &item.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            _ => self.element(out, item, ctx),
        }
    }

    fn adaptive(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let target = if element.id.is_some() {
            self.target(element, ctx)
        } else {
            "null".into()
        };
        // The breakpoints are the application's, in `BernHost.layouts`; a
        // size without its own layout shows another one.
        out.open(format!("BernAdaptive({target},"));
        for size in LayoutSize::ALL {
            let layout = element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size.name()) => {
                    Some(layout)
                }
                _ => None,
            });
            match layout {
                Some(layout) => {
                    out.open(format!("{} = {{", size.name()));
                    self.nodes(out, &layout.children, &ctx.within(Scope::Other));
                    out.close("},");
                }
                None => out.line(format!("{} = null,", size.name())),
            }
        }
        out.close(")");
    }

    // MARK: Navigation containers

    /// Items of a navigation container through `if`, `if let`, `match` and
    /// `ForEach`; `item` writes each one.
    fn nav_walk(&self, out: &mut Out, nodes: &[Node], item: &dyn Fn(&mut Out, &Element)) {
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
                    out.open(format!("for ({var} in {}) {{", self.expr(items)));
                    out.open(format!("key(BernKey.of({var}.{})) {{", member(name)));
                    self.nav_walk(out, &element.children, item);
                    out.close("}");
                    out.close("}");
                }
                Node::Element(element) => item(out, element),
                Node::If(node) => self.if_node(out, node, &Ctx::default(), &|out, body| {
                    self.nav_walk(out, body, item)
                }),
                Node::IfLet(node) => {
                    self.if_let(out, node, &|out, body| self.nav_walk(out, body, item))
                }
                Node::Match(node) => {
                    self.match_node(out, node, &|out, body| self.nav_walk(out, body, item))
                }
                Node::Component(_) => {}
            }
        }
    }

    /// Kotlin statements that run `item` for each item a navigation
    /// container shows now; elements that are not items are looked into.
    fn nav_statements(
        &self,
        out: &mut Out,
        nodes: &[Node],
        item: &dyn Fn(&Element) -> Option<String>,
    ) {
        for node in nodes {
            match node {
                Node::Element(element) if element.kind == "ForEach" => {
                    let (Some(items), Some(template)) = (element.expr("items"), &element.template)
                    else {
                        continue;
                    };
                    out.open(format!(
                        "for ({} in {}) {{",
                        member(&template.var),
                        self.expr(items)
                    ));
                    self.nav_statements(out, &element.children, item);
                    out.close("}");
                }
                Node::Element(element) => match item(element) {
                    Some(statement) => out.line(statement),
                    None => self.nav_statements(out, &element.children, item),
                },
                Node::If(node) => self.if_node(out, node, &Ctx::default(), &|out, body| {
                    self.nav_statements(out, body, item)
                }),
                Node::IfLet(node) => {
                    self.if_let(out, node, &|out, body| self.nav_statements(out, body, item))
                }
                Node::Match(node) => {
                    self.match_node(out, node, &|out, body| self.nav_statements(out, body, item))
                }
                Node::Component(_) => {}
            }
        }
    }

    /// `val name = buildList { … }` of the values of the items of `kind`
    /// shown now.
    fn nav_values(&self, out: &mut Out, name: &str, nodes: &[Node], kind: &str, ty: &str) {
        out.open(format!("val {name} = buildList<{ty}> {{"));
        self.nav_statements(out, nodes, &|element| {
            (element.kind == kind).then(|| {
                let value = element
                    .expr("value")
                    .map(|v| self.expr(v))
                    .unwrap_or_default();
                format!("add({value})")
            })
        });
        out.close("}");
    }

    /// `val name = run { … }`: the label of the selected item of `kind`.
    fn nav_label(&self, out: &mut Out, name: &str, selection: &str, nodes: &[Node], kind: &str) {
        out.open(format!("val {name} = run bernLabel@{{"));
        self.nav_statements(out, nodes, &|element| {
            (element.kind == kind).then(|| {
                let value = element
                    .expr("value")
                    .map(|v| self.expr(v))
                    .unwrap_or_default();
                let label = self.text_prop(element, "label").unwrap_or_default();
                format!("if ({selection} == {value}) return@bernLabel {label}")
            })
        });
        out.line("\"\"");
        out.close("}");
    }

    /// When the selected item goes away, select the container's
    /// `fallback`, or else the first item shown.
    fn nav_fallback(&self, out: &mut Out, element: &Element, selection: &str, visible: &str) {
        if !dynamic_items(&element.children) {
            return;
        }
        let target = element
            .expr("fallback")
            .map(|value| self.expr(value))
            .unwrap_or_else(|| format!("{visible}.firstOrNull()"));
        out.open(format!("LaunchedEffect({visible}) {{"));
        out.line(format!(
            "if ({selection} !in {visible}) ({target})?.let {{ {selection} = it }}"
        ));
        out.close("}");
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
        let ty = kotlin_type(&sidebar.field("selection").expect("selection field").ty);
        out.open("run {");
        self.nav_values(out, "bernVisible", &sidebar.children, "SidebarItem", &ty);
        self.nav_label(
            out,
            "bernSelected",
            &selection,
            &sidebar.children,
            "SidebarItem",
        );
        self.nav_fallback(out, sidebar, &selection, "bernVisible");
        // Where only one column fits, choosing a destination opens it, for
        // the user and for external entries alike.
        out.line("val bernShowing = rememberSaveable { mutableStateOf(false) }");
        if sidebar.id.is_some() {
            out.line(format!(
                "BernControl({}, \"sidebar\", bernSelected, actions = BernControlActions(select = {{ name -> bernVisible.firstOrNull {{ it.bernName == name }}?.let {{ {selection} = it; bernShowing.value = true; true }} ?: false }}, value = {{ {selection}.bernName }}))",
                self.target(sidebar, ctx)
            ));
        }
        let footer = sidebar.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "SidebarFooter" => Some(child),
            _ => None,
        });
        out.open("BernSplitView(bernSelected, bernShowing, sidebar = { bernShowDetail ->");
        self.sidebar_nodes(out, &sidebar.children, &selection, ctx);
        if let Some(footer) = footer {
            out.close("}, footer = {");
            out.indent += 1;
            self.nodes(out, &footer.children, &ctx.within(Scope::Column));
        }
        out.close("}, detail = {");
        out.indent += 1;
        self.nodes(out, &detail.children, &ctx.within(Scope::Other));
        out.close("})");
        out.close("}");
    }

    fn sidebar_nodes(&self, out: &mut Out, nodes: &[Node], selection: &str, ctx: &Ctx) {
        self.nav_walk(out, nodes, &|out, element| match element.kind.as_str() {
            "Section" => {
                let title = self.text_prop(element, "title").unwrap_or_else(|| "null".into());
                out.open(format!("BernSidebarSection({title}) {{"));
                self.sidebar_nodes(out, &element.children, selection, ctx);
                out.close("}");
            }
            "SidebarItem" => {
                let label = self.text_prop(element, "label").unwrap_or_default();
                let value = element.expr("value").map(|v| self.expr(v)).unwrap_or_default();
                // Items without a style of their own keep the Material
                // highlight.
                match self.style_of(element) {
                    Some(style) => out.line(format!(
                        "BernStyledSidebarItem({label}, {}, {selection} == {value}, BernStyles.{}) {{ {selection} = {value}; bernShowDetail() }}",
                        Self::icon(element.icon("icon")),
                        style.name
                    )),
                    None => out.line(format!(
                        "BernSidebarItem({label}, {}, {selection} == {value}) {{ {selection} = {value}; bernShowDetail() }}",
                        Self::icon(element.icon("icon"))
                    )),
                }
            }
            "Search" => self.element(out, element, ctx),
            _ => {}
        });
    }

    fn navigation(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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
        let mut inner = ctx.within(Scope::Other);
        inner.path = Some(path.clone());
        if element.id.is_some() {
            out.line(format!(
                "BernControl({}, \"navigation\", \"\", actions = BernControlActions(back = {{ if ({path}.isEmpty()) false else {{ {path} = {path}.dropLast(1); true }} }}, value = {{ {path}.size.toString() }}))",
                self.target(element, ctx)
            ));
        }
        out.open(format!("if ({path}.isEmpty()) {{"));
        self.nodes(out, &content, &inner);
        out.close("} else {");
        out.indent += 1;
        let var = destination
            .and_then(|destination| destination.template.as_ref())
            .map(|template| member(&template.var))
            .unwrap_or_else(|| "route".into());
        out.line(format!("val {var} = {path}.last()"));
        out.line(format!(
            "BernChrome(back = {{ {path} = {path}.dropLast(1) }})"
        ));
        if let Some(destination) = destination {
            self.nodes(out, &destination.children, &inner);
        }
        out.close("}");
    }

    fn tabs(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let selection = self.field(element, "selection").expect("selection field");
        let ty = kotlin_type(&element.field("selection").expect("selection field").ty);
        out.open("run {");
        self.nav_values(out, "bernVisible", &element.children, "Tab", &ty);
        self.nav_fallback(out, element, &selection, "bernVisible");
        if element.id.is_some() {
            self.nav_label(out, "bernSelected", &selection, &element.children, "Tab");
            out.line(format!(
                "BernControl({}, \"tabs\", bernSelected, actions = BernControlActions(select = {{ name -> bernVisible.firstOrNull {{ it.bernName == name }}?.let {{ {selection} = it; true }} ?: false }}, value = {{ {selection}.bernName }}))",
                self.target(element, ctx)
            ));
        }
        out.open("BernTabs(content = {");
        self.nav_walk(out, &element.children, &|out, tab| {
            let value = tab.expr("value").map(|v| self.expr(v)).unwrap_or_default();
            out.open(format!("if ({selection} == {value}) {{"));
            self.nodes(out, &tab.children, &ctx.within(Scope::Other));
            out.close("}");
        });
        out.close("}, bar = {");
        out.indent += 1;
        self.nav_walk(out, &element.children, &|out, tab| {
            let value = tab.expr("value").map(|v| self.expr(v)).unwrap_or_default();
            let label = self.text_prop(tab, "label").unwrap_or_default();
            out.line(format!(
                "BernTabItem({label}, {}, {selection} == {value}) {{ {selection} = {value} }}",
                Self::icon(tab.icon("icon"))
            ));
        });
        out.close("})");
        out.close("}");
    }

    /// The text a row shows first: the first `Text` or `Label`.
    fn row_label(&self, nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            Node::Element(element) => match element.kind.as_str() {
                "Text" => self.text_prop(element, "content"),
                "Label" => self.text_prop(element, "title"),
                _ => self.row_label(&element.children),
            },
            _ => None,
        })
    }

    // MARK: Components and windows

    /// Lines binding the window and the sessions a body reads.
    fn locals(&self, out: &mut Out, sessions: &[String], window: &str) {
        for session in sessions {
            out.line(format!(
                "val {} = {window}.session({}::class.java)",
                session_var(session),
                session_class(session)
            ));
        }
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        let name = format!("{}View", pascal(&component.name));
        let mut sessions = Vec::new();
        let mut params = Vec::new();
        for param in &component.params {
            match &param.kind {
                ParamKind::Value { ty } => {
                    params.push(format!("{}: {}", member(&param.name), kotlin_type(ty)))
                }
                ParamKind::Session { session } => {
                    if !sessions.contains(session) {
                        sessions.push(session.clone());
                    }
                }
                ParamKind::Client { .. } | ParamKind::Windows | ParamKind::Players => {}
            }
        }
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.line("@Composable");
        out.open(format!("fun {name}({}) {{", params.join(", ")));
        out.line("val bernWindow = LocalBernWindow.current");
        self.locals(out, &sessions, "bernWindow");
        // The component may be a direct child of a staggering container.
        let ctx = Ctx {
            component: component.name.clone(),
            stagger: self.staggers,
            ..Ctx::default()
        };
        self.node(out, &component.body, &ctx);
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, index: usize, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.line("@Composable");
        out.open(format!("fun BernWindow{index}() {{"));
        let sessions: Vec<String> = window
            .sessions
            .iter()
            .map(|session| format!("{}()", session_class(session)))
            .collect();
        out.line(format!(
            "val context = remember {{ BernWindowContext({index}, {}, {}, listOf({})) }}",
            string(&window.id),
            window.role == WindowRole::Settings,
            sessions.join(", ")
        ));
        out.line("val bernWindow = context");
        self.locals(out, &window.sessions, "context");
        if let Some(close) = &window.close {
            self.close_check(out, close);
        }
        let windows = &self.app.windows;
        if windows.accent.is_some() || windows.appearance.is_some() {
            let accent = windows
                .accent
                .as_ref()
                .map(|expr| format!("BernAccents.color(({}).bernName)", self.expr(expr)))
                .unwrap_or_else(|| "null".into());
            let appearance = windows
                .appearance
                .as_ref()
                .map(|expr| format!("({}).bernName", self.expr(expr)))
                .unwrap_or_else(|| "null".into());
            out.line(format!("BernThemeChoice({accent}, {appearance})"));
        }
        // `chrome` has nothing to change on a phone, whose windows have no
        // title bar; immersion hides the top bar and the system bars.
        let mut args = Vec::new();
        if let Some(idle) = window.idle_after {
            args.push(format!("idleAfter = {}", double(idle)));
        }
        if let Some(immersive) = &window.immersive {
            args.push(format!("immersive = {}", self.expr(immersive)));
        }
        let args = if args.is_empty() {
            String::new()
        } else {
            format!(", {}", args.join(", "))
        };
        out.open(format!("BernWindowRoot(context{args}) {{"));
        let ctx = Ctx {
            component: window.id.clone(),
            ..Ctx::default()
        };
        self.node(out, &window.content, &ctx);
        out.close("}");
        out.close("}");
        out.line("");
    }

    /// The window's close check: evaluated against its own drafts when
    /// closing is requested, with the declared save command.
    fn close_check(&self, out: &mut Out, close: &bern_ir::CloseCheck) {
        out.open("context.closeCheck = BernCloseCheck(");
        out.line(format!(
            "needsConfirmation = {{ {} }},",
            self.expr(&close.when)
        ));
        out.line(format!("message = {{ {} }},", self.text(&close.message)));
        match &close.save {
            Some(Action::Engine { command, args }) => {
                let spec = &self.app.protocol.commands[*command];
                let mut values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                values.push("context".into());
                values.push("done".into());
                out.line(format!(
                    "save = {{ done -> BernEngine.{}({}) }},",
                    member(&self.app.protocol.command_symbol(spec)),
                    values.join(", ")
                ));
            }
            _ => out.line("save = null,"),
        }
        out.close(")");
    }

    pub fn generate(&self, package: &str, fingerprint: u64) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("@file:Suppress(\"RedundantSuppression\", \"UNUSED_VARIABLE\", \"UNUSED_ANONYMOUS_PARAMETER\", \"unused\", \"NAME_SHADOWING\")");
        out.line("");
        out.line(format!("package {package}"));
        out.line("");
        for import in IMPORTS {
            out.line(format!("import {import}"));
        }
        out.line("");
        out.line("// MARK: - Values");
        out.line("");
        for def in &app.types.enums {
            generate_enum(&mut out, def);
        }
        for def in &app.types.records {
            generate_record(&mut out, def);
        }
        let styles = crate::style::used(app, PLATFORM);
        if !styles.is_empty() {
            out.line("// MARK: - Styles");
            out.line("");
            out.line("/// The style overrides of `style!` on this platform.");
            out.open("object BernStyles {");
            for style in &styles {
                out.line(crate::style::declaration(app, style));
            }
            out.close("}");
            out.line("");
        }
        if !app.style.patterns.is_empty() {
            out.line("/// The patterns of `style!`.");
            out.open("object BernPatterns {");
            for pattern in &app.style.patterns {
                let bands = pattern
                    .bands
                    .iter()
                    .map(|band| {
                        format!(
                            "BernBand({}, {}, {})",
                            band.color
                                .as_ref()
                                .map(crate::style::color)
                                .unwrap_or_else(|| "null".into()),
                            double(band.width),
                            double(band.opacity)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.line(format!(
                    "val {} = BernPatternDef(listOf({bands}), {})",
                    member(&pattern.name),
                    pattern.twill
                ));
            }
            out.close("}");
            out.line("");
        }
        if !app.style.accents.is_empty() {
            out.line("/// The accents of `style!`, by the enum variant that chooses one.");
            out.open("object BernAccents {");
            out.open("fun color(name: String): BernColor? = when (name.replace(\"_\", \"\").lowercase()) {");
            for accent in &app.style.accents {
                out.line(format!(
                    "{} -> {}",
                    string(&accent.name.replace('_', "").to_lowercase()),
                    crate::style::color(&accent.color)
                ));
            }
            out.line("else -> null");
            out.close("}");
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
        for (index, window) in app.windows.windows.iter().enumerate() {
            self.window(&mut out, index, window);
        }
        let kinds: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                let dialog = if window.role == WindowRole::Dialog {
                    format!(
                        ", isDialog = true, width = {}, height = {}",
                        window.width.map(double).unwrap_or_else(|| "null".into()),
                        window.height.map(double).unwrap_or_else(|| "null".into())
                    )
                } else {
                    String::new()
                };
                format!(
                    "BernWindowKind({}, {}, {}{dialog})",
                    string(&window.id),
                    string(&window.title.key),
                    window.role == WindowRole::Settings
                )
            })
            .collect();
        let initial: Vec<String> = app
            .windows
            .windows
            .iter()
            .filter(|window| {
                window.initial == bern_ir::WindowInitial::Open && window.id != app.windows.primary
            })
            .map(|window| string(&window.id))
            .collect();
        let languages: Vec<String> = app.languages.supported.iter().map(|l| string(l)).collect();
        out.line("// MARK: - Activity");
        out.line("");
        out.line("/// The application's one activity: it starts the engine and shows the");
        out.line("/// primary window with the windows open over it.");
        out.open("class MainActivity : ComponentActivity() {");
        out.open("override fun onCreate(savedInstanceState: Bundle?) {");
        out.line("super.onCreate(savedInstanceState)");
        out.line("enableEdgeToEdge()");
        out.open("BernHost.start(");
        out.line("applicationContext,");
        out.line(format!("{}L,", fingerprint as i64));
        out.line(format!("listOf({}),", languages.join(", ")));
        out.line(format!("{},", string(&app.languages.default)));
        out.line(format!("listOf({}),", kinds.join(", ")));
        out.line(format!("{},", string(&app.windows.primary)));
        out.close(") { field, reader -> BernEngine.bernApply(field, reader) }");
        out.line("BernFiles.register(this)");
        if let Some(layouts) = &app.layouts {
            let or_null = |value: Option<f64>| value.map(double).unwrap_or_else(|| "null".into());
            out.line(format!(
                "BernHost.layouts = BernLayouts({}, {}, {})",
                double(layouts.compact_below),
                or_null(layouts.medium_below),
                or_null(layouts.expanded_below)
            ));
        }
        out.open("setContent {");
        out.open("BernTheme {");
        out.line(format!(
            "LaunchedEffect(Unit) {{ BernHost.openInitial(listOf({})) }}",
            initial.join(", ")
        ));
        out.open("BernVirtualHost { kind ->");
        out.open("when (kind) {");
        for (index, window) in app.windows.windows.iter().enumerate() {
            out.line(format!("{} -> BernWindow{index}()", string(&window.id)));
        }
        out.close("}");
        out.close("}");
        out.close("}");
        out.close("}");
        out.close("}");
        out.line("");
        out.line("/// A file another app opens with this one while it runs.");
        out.open("override fun onNewIntent(intent: android.content.Intent) {");
        out.line("super.onNewIntent(intent)");
        out.line("BernFiles.opened(intent)");
        out.close("}");
        out.line("");
        out.line("/// Keyboard shortcuts come first, whether or not a control has focus:");
        out.line("/// Compose sends keys only to the focused control and its parents.");
        out.open("override fun dispatchKeyEvent(event: android.view.KeyEvent): Boolean {");
        out.line("BernShortcuts.pressed(event)");
        out.line("return BernShortcuts.handle(androidx.compose.ui.input.key.KeyEvent(event)) || super.dispatchKeyEvent(event)");
        out.close("}");
        out.close("}");
        out.finish()
    }
}

/// The platform's name in `style!`.
const PLATFORM: &str = "android";

const IMPORTS: &[&str] = &[
    "androidx.compose.animation.AnimatedContent",
    "androidx.compose.animation.AnimatedVisibility",
    "androidx.compose.animation.SizeTransform",
    "androidx.compose.animation.togetherWith",
    "androidx.compose.foundation.Canvas",
    "androidx.compose.ui.graphics.Color",
    "androidx.compose.ui.semantics.Role",
    "androidx.compose.ui.semantics.clearAndSetSemantics",
    "androidx.compose.ui.semantics.contentDescription",
    "androidx.compose.ui.semantics.role",
    "androidx.compose.ui.semantics.semantics",
    "android.os.Bundle",
    "androidx.activity.ComponentActivity",
    "androidx.activity.compose.setContent",
    "androidx.activity.enableEdgeToEdge",
    "androidx.compose.foundation.horizontalScroll",
    "androidx.compose.foundation.layout.Arrangement",
    "androidx.compose.foundation.layout.Box",
    "androidx.compose.foundation.layout.Column",
    "androidx.compose.foundation.layout.Row",
    "androidx.compose.foundation.layout.Spacer",
    "androidx.compose.foundation.layout.fillMaxSize",
    "androidx.compose.foundation.layout.fillMaxWidth",
    "androidx.compose.foundation.layout.padding",
    "androidx.compose.foundation.layout.size",
    "androidx.compose.foundation.layout.width",
    "androidx.compose.foundation.layout.height",
    "androidx.compose.foundation.lazy.LazyColumn",
    "androidx.compose.foundation.lazy.items",
    "androidx.compose.foundation.rememberScrollState",
    "androidx.compose.foundation.verticalScroll",
    "androidx.compose.material3.HorizontalDivider",
    "androidx.compose.material3.MaterialTheme",
    "androidx.compose.material3.Text",
    "androidx.compose.material3.TextButton",
    "androidx.compose.material3.VerticalDivider",
    "androidx.compose.runtime.Composable",
    "androidx.compose.runtime.CompositionLocalProvider",
    "androidx.compose.runtime.LaunchedEffect",
    "androidx.compose.runtime.getValue",
    "androidx.compose.runtime.key",
    "androidx.compose.runtime.mutableStateOf",
    "androidx.compose.runtime.remember",
    "androidx.compose.runtime.saveable.rememberSaveable",
    "androidx.compose.runtime.setValue",
    "androidx.compose.ui.Alignment",
    "androidx.compose.ui.Modifier",
    "androidx.compose.ui.layout.ContentScale",
    "androidx.compose.ui.platform.testTag",
    "androidx.compose.ui.platform.LocalContext",
    "androidx.compose.ui.unit.dp",
    "dev.bern.runtime.*",
];

/// The pattern of one `match` arm: a variant, or `else` for `_`.
fn case_pattern(def: &EnumDef, arm: &MatchArm) -> String {
    let Some(index) = arm.variant else {
        return "else".into();
    };
    let variant = &def.variants[index as usize];
    // Unit variants are values; variants with fields are classes.
    if variant.fields.is_empty() {
        format!("{}.{}", def.name, variant.name)
    } else {
        format!("is {}.{}", def.name, variant.name)
    }
}

/// `val name = bernValue.field` for the bindings of one arm.
fn arm_bindings(def: &EnumDef, arm: &MatchArm) -> Vec<String> {
    let Some(index) = arm.variant else {
        return Vec::new();
    };
    let variant = &def.variants[index as usize];
    arm.bindings
        .iter()
        .map(|binding| {
            format!(
                "val {} = bernValue.{}",
                member(&binding.name),
                member(&variant.fields[binding.field].name)
            )
        })
        .collect()
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
