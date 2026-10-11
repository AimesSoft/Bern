//! C# generation from the IR.
//!
//! The generated `App.cs` builds every component and window from the
//! runtime's standard components (the `Bern` namespace). Expressions become
//! C# expressions over schema values evaluated in a context `c`; locals
//! (component parameters, row variables, pattern bindings) live in the
//! context, so every lambda is `static`: it captures nothing, and each
//! binding re-runs only when a cell it read changes.

use bern_ir::{
    Action, Application, BinaryOp, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind,
    IfLetNode, IfNode, MatchNode, MediaOp, Node, ParamKind, Prop, Text, WindowDef, WindowInitial,
    WindowOp, WindowRole,
};
use bern_protocol::{
    EnumDef, OperationKind, SessionCommand, Type, TypeTable, Value, session_catalog,
};

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

/// A C# string literal.
pub fn string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' || c == '\u{85}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn float(value: f64) -> String {
    if value.is_nan() {
        return "double.NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "double.PositiveInfinity".into()
        } else {
            "double.NegativeInfinity".into()
        };
    }
    let text = format!("{value:?}");
    if text.contains('.') || text.contains('e') {
        text
    } else {
        format!("{text}.0")
    }
}

fn null(value: Option<String>) -> String {
    value.unwrap_or_else(|| "null".into())
}

/// A schema value as a C# expression.
fn literal(value: &Value) -> String {
    let list = |values: &[Value]| values.iter().map(literal).collect::<Vec<_>>();
    let with = |head: String, values: &[Value]| {
        std::iter::once(head)
            .chain(list(values))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match value {
        Value::Unit => "V.Unit".into(),
        Value::Bool(value) => format!("V.Bool({value})"),
        Value::Int(value) => format!("V.Int({value}L)"),
        Value::Float(value) => format!("V.Float({})", float(*value)),
        Value::Text(value) => format!("V.Text({})", string(value)),
        Value::Enum { variant, fields } => {
            format!("V.Enum({})", with(format!("{variant}u"), fields))
        }
        Value::Optional(None) => "V.None".into(),
        Value::Optional(Some(value)) => format!("V.Some({})", literal(value)),
        Value::Record(values) => format!("V.Record({})", list(values).join(", ")),
        Value::List(values) => format!("V.List({})", list(values).join(", ")),
    }
}

/// A schema type as a C# `BType` expression.
pub fn type_expr(ty: &Type) -> String {
    match ty {
        Type::Unit => "BType.Unit".into(),
        Type::Bool => "BType.Bool".into(),
        Type::Int => "BType.Int".into(),
        Type::Float => "BType.Float".into(),
        Type::Text => "BType.Text".into(),
        Type::Enum { name } => format!("BType.Enum({})", string(name)),
        Type::Record { name } => format!("BType.Record({})", string(name)),
        Type::Optional { inner } => format!("BType.Optional({})", type_expr(inner)),
        Type::List { inner } => format!("BType.List({})", type_expr(inner)),
    }
}

fn types_list(types: &[Type]) -> String {
    if types.is_empty() {
        "System.Array.Empty<BType>()".into()
    } else {
        format!(
            "new BType[] {{ {} }}",
            types.iter().map(type_expr).collect::<Vec<_>>().join(", ")
        )
    }
}

/// Every type the runtime decodes: the application's (protocol and UI
/// types), the media catalog's and the session catalog's, first
/// definition winning.
fn merged_types(app: &Application) -> TypeTable {
    let mut merged = TypeTable::default();
    for table in [
        app.types.clone(),
        app.protocol.types.clone(),
        bern_media::media_types(),
        session_catalog().0,
    ] {
        for def in table.enums {
            if !merged.enums.iter().any(|known| known.name == def.name) {
                merged.enums.push(def);
            }
        }
        for def in table.records {
            if !merged.records.iter().any(|known| known.name == def.name) {
                merged.records.push(def);
            }
        }
    }
    merged
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
            format!("R.At({expr}, {})", indices.join(", "))
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
        }
    }

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value),
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let values = std::iter::once(format!("{variant}u"))
                    .chain(fields.iter().map(|field| self.expr(field)))
                    .collect::<Vec<_>>();
                format!("R.Variant({})", values.join(", "))
            }
            ExprKind::Not { expr } => format!("R.Not({})", self.expr(expr)),
            ExprKind::Neg { expr } => format!("R.Neg({})", self.expr(expr)),
            ExprKind::Binary { op, lhs, rhs } => {
                let (left, right) = (self.expr(lhs), self.expr(rhs));
                match op {
                    BinaryOp::And => format!("R.And({left}, () => {right})"),
                    BinaryOp::Or => format!("R.Or({left}, () => {right})"),
                    other => {
                        let name = match other {
                            BinaryOp::Eq => "Eq",
                            BinaryOp::Ne => "Ne",
                            BinaryOp::Lt => "Lt",
                            BinaryOp::Le => "Le",
                            BinaryOp::Gt => "Gt",
                            _ => "Ge",
                        };
                        format!("R.{name}({left}, {right})")
                    }
                }
            }
        }
    }

    fn condition(&self, expr: &Expr) -> String {
        format!("R.Truthy({})", self.expr(expr))
    }

    /// Display text of a value, by type.
    fn display(&self, expr: &Expr) -> String {
        let value = self.expr(expr);
        match &expr.ty {
            Type::Enum { name } => format!("Text.Display({value}, {})", string(name)),
            _ => format!("R.Plain({value})"),
        }
    }

    fn text(&self, text: &Text) -> String {
        match text {
            Text::Localized { text, args } => {
                if text.params.is_empty() {
                    format!("Text.Tr({})", string(&text.key))
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
                            format!("({}, {value})", string(param))
                        })
                        .collect();
                    format!("Text.Fmt({}, {})", string(&text.key), values.join(", "))
                }
            }
            Text::Dynamic { expr } => self.display(expr),
        }
    }

    fn text_getter(&self, text: &Text) -> String {
        format!("static c => {}", self.text(text))
    }

    fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text_getter(text))
    }

    fn getter(&self, expr: &Expr) -> String {
        format!("static c => {}", self.expr(expr))
    }

    fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    fn optional_id(&self, element: &Element) -> String {
        null(element.id.as_ref().map(|_| self.id(element)))
    }

    fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        Some(format!(
            "new Field({}, {})",
            self.session_index(&field.session),
            field.field
        ))
    }

    fn values(&self, args: &[Expr]) -> String {
        if args.is_empty() {
            "System.Array.Empty<V>()".into()
        } else {
            let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
            format!("new V[] {{ {} }}", values.join(", "))
        }
    }

    fn action_body(&self, action: &Action) -> String {
        match action {
            Action::Engine { command, args } => {
                format!("Host.Dispatch(c, {command}, {})", self.values(args))
            }
            Action::Window { op, window } => match op {
                WindowOp::Open | WindowOp::Activate => {
                    format!("WindowOps.Open(c, {})", string(window))
                }
                WindowOp::Close => format!("WindowOps.Close(c, {})", string(window)),
            },
            Action::Media { player, op, args } => {
                let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                let player = string(player);
                match op {
                    MediaOp::Open => format!(
                        "Media.Open(c, {player}, {})",
                        values.first().cloned().unwrap_or_else(|| "V.Unit".into())
                    ),
                    other => {
                        let name = match other {
                            MediaOp::Play => "Play",
                            MediaOp::Pause => "Pause",
                            MediaOp::Seek => "Seek",
                            MediaOp::SetVolume => "SetVolume",
                            _ => "SetMuted",
                        };
                        let args = std::iter::once(format!("MediaOp.{name}, {player}"))
                            .chain(values)
                            .collect::<Vec<_>>();
                        format!("Media.Send(c, {})", args.join(", "))
                    }
                }
            }
        }
    }

    fn action(&self, element: &Element) -> String {
        null(
            element
                .action
                .as_ref()
                .map(|action| format!("static c => {}", self.action_body(action))),
        )
    }

    fn keyword(element: &Element, name: &str) -> String {
        null(element.keyword(name).map(string))
    }

    fn icon(element: &Element, name: &str) -> String {
        null(element.icon(name).map(string))
    }

    fn number(element: &Element, name: &str) -> String {
        null(element.number(name).map(float))
    }

    fn enabled(&self, element: &Element) -> String {
        null(element.expr("enabled").map(|expr| self.getter(expr)))
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            self.node(out, node);
        }
    }

    /// A children builder: `static (p, c) => { … }`.
    fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        out.open(format!("{prefix}static (p, c) =>"));
        out.indent -= 1;
        out.open("{");
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
                                ComponentArg::Value { expr } => Some(format!(
                                    "({}, {})",
                                    string(&param.name),
                                    self.getter(expr)
                                )),
                                _ => None,
                            }),
                        _ => None,
                    })
                    .collect();
                let args = std::iter::once(string(&component.name))
                    .chain(args)
                    .collect::<Vec<_>>();
                out.line(format!(
                    "c_{}(p, c.Child({}));",
                    component.name,
                    args.join(", ")
                ));
            }
            Node::If(node) => self.if_node(out, node),
            Node::IfLet(node) => self.if_let(out, node),
            Node::Match(node) => self.match_node(out, node),
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode) {
        let mut branch = String::from("static c => { ");
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            branch.push_str(&format!(
                "if ({}) return {index}; ",
                self.condition(condition)
            ));
        }
        if node.otherwise.is_empty() {
            branch.push_str("return null; }");
        } else {
            branch.push_str(&format!("return {}; }}", node.branches.len()));
        }
        out.open(format!("B.When(p, c, {branch},"));
        let count = node.branches.len() + usize::from(!node.otherwise.is_empty());
        let mut written = 0;
        let mut separator = || {
            written += 1;
            if written == count { ");" } else { "," }
        };
        for (_, body) in &node.branches {
            self.children(out, "", body, separator());
        }
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, separator());
        }
        out.indent -= 1;
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode) {
        out.open(format!(
            "B.IfLet(p, c, {}, {},",
            string(&node.var),
            self.getter(&node.value)
        ));
        if node.otherwise.is_empty() {
            self.children(out, "", &node.then, ", null);");
        } else {
            self.children(out, "", &node.then, ",");
            self.children(out, "", &node.otherwise, ");");
        }
        out.indent -= 1;
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode) {
        if node.arms.is_empty() {
            out.line(format!("B.Match(p, c, {});", self.getter(&node.value)));
            return;
        }
        out.open(format!("B.Match(p, c, {},", self.getter(&node.value)));
        for (index, arm) in node.arms.iter().enumerate() {
            let bindings: String = arm
                .bindings
                .iter()
                .map(|binding| format!(", ({}, {})", string(&binding.name), binding.field))
                .collect();
            let last = index + 1 == node.arms.len();
            self.children(
                out,
                &format!(
                    "B.Arm({}, ",
                    null(arm.variant.map(|variant| format!("{variant}u")))
                ),
                &arm.body,
                &format!("{bindings}){}", if last { ");" } else { "," }),
            );
        }
        out.indent -= 1;
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
                let arms = node
                    .arms
                    .iter()
                    .map(|arm| {
                        let label = self.row_label(&arm.body)?;
                        Some(match arm.variant {
                            Some(variant) => format!("{variant}u => {label},"),
                            None => format!("_ => {label},"),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "(R.Tag({}) switch {{ {} _ => \"\" }})",
                    self.expr(&node.value),
                    arms.join(" ")
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// Name of an enum variant value, for selections.
    fn variant_name(&self, expr: &Expr) -> String {
        match (&expr.kind, &expr.ty) {
            (ExprKind::Variant { variant, .. }, Type::Enum { name }) => self
                .types()
                .enum_def(name)
                .map(|def| string(&def.variants[*variant as usize].name))
                .unwrap_or_else(|| "\"\"".into()),
            _ => "\"\"".into(),
        }
    }

    /// The label getter of a variant: its declared label, or its name.
    fn variant_label(&self, def: &EnumDef, index: usize) -> String {
        let variant = &def.variants[index];
        if variant.label.is_some() {
            format!("static c => Text.Label({}, {index})", string(&def.name))
        } else {
            format!("static c => {}", string(&variant.name))
        }
    }

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => out.line(format!(
                "B.Text(p, c, content: {}, style: {}, emphasis: {});",
                null(self.text_prop(element, "content")),
                Self::keyword(element, "style"),
                Self::keyword(element, "emphasis"),
            )),
            "Label" => out.line(format!(
                "B.Label(p, c, title: {}, icon: {}, style: {});",
                null(self.text_prop(element, "title")),
                Self::icon(element, "icon"),
                Self::keyword(element, "style"),
            )),
            "Button" => out.line(format!(
                "B.Button(p, c, {}, label: {}, icon: {}, role: {}, prominence: {}, enabled: {}, action: {});",
                self.id(element),
                null(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                Self::keyword(element, "role"),
                Self::keyword(element, "prominence"),
                self.enabled(element),
                self.action(element),
            )),
            "IconButton" => out.line(format!(
                "B.IconButton(p, c, {}, {}, label: {}, enabled: {}, action: {});",
                self.id(element),
                string(element.icon("icon").unwrap_or("circle")),
                null(self.text_prop(element, "label")),
                self.enabled(element),
                self.action(element),
            )),
            "TextInput" => out.line(format!(
                "B.TextInput(p, c, {}, label: {}, field: {}, prompt: {}, kind: {}, submit: {});",
                self.id(element),
                null(self.text_prop(element, "label")),
                null(self.field(element, "value")),
                null(self.text_prop(element, "prompt")),
                Self::keyword(element, "kind"),
                self.action(element),
            )),
            "Search" => out.line(format!(
                "B.Search(p, c, {}, field: {}, prompt: {}, submit: {});",
                self.id(element),
                null(self.field(element, "value")),
                null(self.text_prop(element, "prompt")),
                self.action(element),
            )),
            "Switch" => out.line(format!(
                "B.Switch(p, c, {}, label: {}, field: {}, style: {});",
                self.id(element),
                null(self.text_prop(element, "label")),
                null(self.field(element, "value")),
                Self::keyword(element, "style"),
            )),
            "Slider" => out.line(format!(
                "B.Slider(p, c, {}, label: {}, field: {}, min: {}, max: {}, step: {}, minIcon: {}, maxIcon: {}, commit: {});",
                self.id(element),
                null(self.text_prop(element, "label")),
                null(self.field(element, "value")),
                Self::number(element, "min"),
                Self::number(element, "max"),
                Self::number(element, "step"),
                Self::icon(element, "min_icon"),
                Self::icon(element, "max_icon"),
                self.action(element),
            )),
            "Picker" => self.picker(out, element),
            "LanguagePicker" => out.line(format!(
                "B.LanguagePicker(p, c, {}, label: {});",
                self.id(element),
                null(self.text_prop(element, "label")),
            )),
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => self.children(
                out,
                &format!(
                    "B.Form(p, c, {}, ",
                    null(self.text_prop(element, "title"))
                ),
                &element.children,
                ");",
            ),
            "Section" => self.children(
                out,
                &format!(
                    "B.Section(p, c, {}, {}, ",
                    null(self.text_prop(element, "title")),
                    null(self.text_prop(element, "footer"))
                ),
                &element.children,
                ");",
            ),
            "Column" | "Row" => self.children(
                out,
                &format!(
                    "B.Stack(p, c, {}, {}, {}, {}, ",
                    string(if element.kind == "Column" { "column" } else { "row" }),
                    Self::number(element, "gap"),
                    Self::number(element, "padding"),
                    Self::keyword(element, "align"),
                ),
                &element.children,
                ");",
            ),
            "Spacer" => out.line("B.Spacer(p);"),
            "Divider" => out.line("B.Divider(p);"),
            "ScrollView" => self.children(
                out,
                &format!("B.ScrollView(p, c, {}, ", Self::keyword(element, "axis")),
                &element.children,
                ");",
            ),
            "Toolbar" => self.children(out, "B.Toolbar(p, c, ", &element.children, ");"),
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| self.getter(route))
                    .unwrap_or_else(|| "static c => V.Unit".into());
                let label = self
                    .row_label(&element.children)
                    .unwrap_or_else(|| "\"\"".into());
                self.children(
                    out,
                    &format!(
                        "B.NavLink(p, c, {}, {route}, static c => {label}, ",
                        self.id(element)
                    ),
                    &element.children,
                    ");",
                );
            }
            "List" => self.children(out, "B.List(p, c, ", &element.children, ");"),
            "ForEach" => self.for_each(out, element),
            "Tabs" => self.tabs(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "WindowSet"
            | "Window" | "Layout" | "MenuItem" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }

    fn picker(&self, out: &mut Out, element: &Element) {
        let Some(field) = element.field("value") else {
            return;
        };
        let Type::Enum { name } = &field.ty else {
            return;
        };
        let def = self.types().enum_def(name).expect("enum");
        let options: Vec<_> = def
            .variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                format!(
                    "B.Option({index}, {}, {})",
                    string(&variant.name),
                    self.variant_label(def, index)
                )
            })
            .collect();
        out.line(format!(
            "B.Picker(p, c, {}, label: {}, field: {}, options: new[] {{ {} }}, style: {});",
            self.id(element),
            null(self.text_prop(element, "label")),
            null(self.field(element, "value")),
            options.join(", "),
            Self::keyword(element, "style"),
        ));
    }

    fn for_each(&self, out: &mut Out, element: &Element) {
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { field, .. })) => self.children(
                out,
                &format!(
                    "B.Each(p, c, {}, {field}, {}, ",
                    self.getter(items),
                    string(&template.var)
                ),
                &element.children,
                ");",
            ),
            _ => self.nodes(out, &element.children),
        }
    }

    fn video(&self, out: &mut Out, element: &Element) {
        let (source, player) = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => (Some(self.getter(source)), None),
            (None, Some(Prop::Player { id })) => (None, Some(string(id))),
            _ => (None, None),
        };
        out.line(format!(
            "B.Video(p, c, {}, source: {}, player: {}, label: {}, controls: {}, fit: {});",
            self.id(element),
            null(source),
            null(player),
            null(self.text_prop(element, "label")),
            element.keyword("controls") != Some("none"),
            Self::keyword(element, "fit"),
        ));
    }

    fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let params: Vec<_> = binding.args.iter().map(|arg| self.getter(arg)).collect();
        let params = if params.is_empty() {
            "System.Array.Empty<Get>()".to_string()
        } else {
            format!("new Get[] {{ {} }}", params.join(", "))
        };
        self.children(
            out,
            &format!(
                "B.Query(p, c, {}, {}, {params}, {}, {}, {}, {}, {}, ",
                self.optional_id(element),
                binding.query,
                null(self.text_prop(element, "loading")),
                null(self.text_prop(element, "failed")),
                null(self.text_prop(element, "empty")),
                Self::icon(element, "icon"),
                string(&template.var),
            ),
            &element.children,
            ");",
        );
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
        let Some(selection) = self.field(sidebar, "selection") else {
            return;
        };
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
        let search = match search
            .and_then(|search| self.field(search, "value").map(|field| (search, field)))
        {
            Some((search, field)) => format!(
                "new B.SidebarSearch({}, {field}, {}, {})",
                self.id(search),
                null(self.text_prop(search, "prompt")),
                self.action(search)
            ),
            None => "null".into(),
        };
        out.open(format!(
            "B.SplitView(p, c, {}, {selection}, {search}, new B.SidebarSection[]",
            self.optional_id(sidebar)
        ));
        out.open("{");
        for (title, items) in &sections {
            out.line(format!("new({}, new B.SidebarItem[]", null(title.clone())));
            out.open("{");
            for item in items {
                let Some(value) = item.expr("value") else {
                    continue;
                };
                out.line(format!(
                    "new({}, {}, {}, {}),",
                    self.getter(value),
                    self.variant_name(value),
                    null(self.text_prop(item, "label")),
                    Self::icon(item, "icon"),
                ));
            }
            out.close("}),");
        }
        out.close("},");
        self.children(out, "", &detail.children, ");");
        out.indent -= 1;
    }

    fn navigation(&self, out: &mut Out, element: &Element) {
        let Some(path) = self.field(element, "path") else {
            return;
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
        let var = destination
            .and_then(|destination| destination.template.as_ref())
            .map(|template| template.var.clone())
            .unwrap_or_else(|| "route".into());
        out.open(format!(
            "B.Navigation(p, c, {}, {path},",
            self.optional_id(element)
        ));
        self.children(out, "", &content, ",");
        out.line(format!("{},", string(&var)));
        let destination_nodes = destination
            .map(|destination| destination.children.clone())
            .unwrap_or_default();
        self.children(out, "", &destination_nodes, ");");
        out.indent -= 1;
    }

    /// One builder per layout size; the runtime picks one by the width
    /// available to it and the breakpoint from `[layouts]`.
    fn adaptive(&self, out: &mut Out, element: &Element) {
        let layout = |size: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size) => Some(layout),
                _ => None,
            })
        };
        out.open(format!("B.Adaptive(p, c, {},", self.optional_id(element)));
        for (size, suffix) in [("compact", ","), ("expanded", ");")] {
            match layout(size) {
                Some(layout) => self.children(out, "", &layout.children, suffix),
                None => out.line(format!("null{suffix}")),
            }
        }
        out.indent -= 1;
    }

    /// A menu button and its items; dividers are `null`.
    fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "B.Menu(p, c, {}, {}, {}, new B.MenuItem?[]",
            self.id(element),
            null(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
        ));
        out.open("{");
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("null,");
                continue;
            }
            out.line(format!(
                "new({}, {}, {}, {}, {}, {}),",
                self.id(item),
                null(self.text_prop(item, "label")),
                Self::icon(item, "icon"),
                Self::keyword(item, "role"),
                self.enabled(item),
                self.action(item),
            ));
        }
        out.close("});");
        out.indent -= 1;
    }

    fn tabs(&self, out: &mut Out, element: &Element) {
        let Some(selection) = self.field(element, "selection") else {
            return;
        };
        let tabs: Vec<(&Element, &Expr)> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(tab) => tab.expr("value").map(|value| (tab, value)),
                _ => None,
            })
            .collect();
        out.open(format!(
            "B.Tabs(p, c, {}, {selection}{}",
            self.optional_id(element),
            if tabs.is_empty() { ");" } else { "," }
        ));
        for (index, (tab, value)) in tabs.iter().enumerate() {
            let last = index + 1 == tabs.len();
            self.children(
                out,
                &format!(
                    "new B.TabSpec({}, {}, {}, {}, ",
                    self.getter(value),
                    self.variant_name(value),
                    self.text_prop(tab, "label")
                        .unwrap_or_else(|| "static c => \"\"".into()),
                    Self::icon(tab, "icon"),
                ),
                &tab.children,
                if last { "));" } else { ")," },
            );
        }
        out.indent -= 1;
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!("static void c_{}(Parent p, Ctx c)", component.name));
        out.indent -= 1;
        out.open("{");
        self.node(out, &component.body);
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.line(format!("static void w_{}(Parent p, Ctx c)", window.id));
        out.open("{");
        self.node(out, &window.content);
        out.close("}");
        out.line("");
        if let Some(close) = &window.close {
            let save = match &close.save {
                Some(Action::Engine { command, args }) => format!(
                    "static (c, done) => Host.Dispatch(c, {command}, {}, done)",
                    self.values(args)
                ),
                _ => "null".into(),
            };
            out.line(format!(
                "/// The close check of the `{}` window, read against its own drafts.",
                window.id
            ));
            out.line(format!(
                "static CloseCheck close_{}(Ctx c) => new()",
                window.id
            ));
            out.open("{");
            out.line(format!(
                "NeedsConfirmation = {},",
                self.condition(&close.when)
            ));
            out.line(format!("Message = {},", self.text(&close.message)));
            out.line(format!("Save = {save},"));
            out.close("};");
            out.line("");
        }
    }

    fn type_table(out: &mut Out, types: &TypeTable) {
        out.open("static TypeTable Types() => new TypeTable()");
        for def in &types.enums {
            let variants: Vec<_> = def
                .variants
                .iter()
                .map(|variant| {
                    let fields: Vec<_> = variant
                        .fields
                        .iter()
                        .map(|field| field.ty.clone())
                        .collect();
                    format!(
                        "new VariantDef({}, {})",
                        string(&variant.name),
                        types_list(&fields)
                    )
                })
                .collect();
            let args = std::iter::once(string(&def.name))
                .chain(variants)
                .collect::<Vec<_>>();
            out.line(format!(".Enum({})", args.join(", ")));
        }
        for def in &types.records {
            let args = std::iter::once(string(&def.name))
                .chain(def.fields.iter().map(|field| type_expr(&field.ty)))
                .collect::<Vec<_>>();
            out.line(format!(".Record({})", args.join(", ")));
        }
        out.line(";");
        out.indent -= 1;
        out.line("");
    }

    /// The application's `App.cs`.
    pub fn generate(&self, fingerprint: u64) -> String {
        let app = self.app;
        let protocol = &app.protocol;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("#pragma warning disable CS8321, IDE0039");
        out.line("");
        out.line("using System;");
        out.line("using System.Collections.Generic;");
        out.line("using Bern;");
        out.line("using V = Bern.Value;");
        out.line("");
        out.line("namespace BernApp;");
        out.line("");
        out.line("static class Ui");
        out.open("{");
        out.line("// Components.");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }

        out.line("/// Strings of every supported language.");
        out.line("static Dictionary<string, Dictionary<string, string>> Strings() => new()");
        out.open("{");
        for (language, table) in crate::strings::tables(app) {
            out.open(format!("[{}] = new()", string(&language)));
            out.indent -= 1;
            out.open("{");
            for (key, text) in table {
                out.line(format!("[{}] = {},", string(&key), string(&text)));
            }
            out.close("},");
        }
        out.close("};");
        out.line("");

        out.line("/// Protocol, UI, media and session catalog types.");
        Self::type_table(&mut out, &merged_types(app));

        let (_, session_specs) = session_catalog();
        let session_results: Vec<Type> = SessionCommand::ALL
            .iter()
            .map(|command| {
                session_specs
                    .iter()
                    .find(|spec| spec.id == command.id())
                    .map(|spec| spec.result.clone())
                    .unwrap_or(Type::Unit)
            })
            .collect();

        out.line("/// The application, as the runtime sees it.");
        out.line("public static AppDef App() => new()");
        out.open("{");
        out.line(format!("Id = {},", string(&app.app.id)));
        out.line(format!("BundleId = {},", string(&app.app.bundle_id)));
        out.line("Name = \"app.name\",");
        out.line(format!("Version = {},", string(&app.app.version)));
        out.line(format!("Fingerprint = 0x{fingerprint:016x}UL,"));
        let languages: Vec<_> = app
            .languages
            .supported
            .iter()
            .map(|language| string(language))
            .collect();
        out.line(format!("Languages = new[] {{ {} }},", languages.join(", ")));
        out.line(format!(
            "DefaultLanguage = {},",
            string(&app.languages.default)
        ));
        out.line("Strings = Strings(),");
        out.line("Types = Types(),");
        let state: Vec<Type> = protocol
            .state
            .iter()
            .map(|field| field.ty.clone())
            .collect();
        out.line(format!("State = {},", types_list(&state)));
        out.open("Commands = new CommandDef[]");
        out.indent -= 1;
        out.open("{");
        for command in &protocol.commands {
            let params: Vec<Type> = command
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect();
            out.line(format!(
                "new({}, {}, {}, {}),",
                string(&command.id),
                types_list(&params),
                type_expr(&command.result),
                command.kind == OperationKind::Query,
            ));
        }
        out.close("},");
        out.open("Sessions = new SessionDef[]");
        out.indent -= 1;
        out.open("{");
        for session in &app.sessions {
            let fields: Vec<_> = session
                .fields
                .iter()
                .map(|field| format!("({}, {})", string(&field.name), type_expr(&field.ty)))
                .collect();
            out.line(format!(
                "new({}, new (string, BType)[] {{ {} }}),",
                string(&session.name),
                fields.join(", ")
            ));
        }
        out.close("},");
        out.line(format!(
            "SessionResults = {},",
            types_list(&session_results)
        ));
        out.open("Windows = new WindowDef[]");
        out.indent -= 1;
        out.open("{");
        for window in &app.windows.windows {
            let optional = |value: Option<f64>| null(value.map(float));
            let sessions: Vec<_> = window
                .sessions
                .iter()
                .map(|name| self.session_index(name).to_string())
                .collect();
            out.line(format!(
                "new() {{ Id = {}, Title = {}, Settings = {}, Width = {}, Height = {}, MinWidth = {}, MinHeight = {}, InitialOpen = {}, Sessions = new int[] {{ {} }}, Render = w_{}, Close = {} }},",
                string(&window.id),
                string(&window.title.key),
                window.role == WindowRole::Settings,
                optional(window.width),
                optional(window.height),
                optional(window.min_width),
                optional(window.min_height),
                window.initial == WindowInitial::Open,
                sessions.join(", "),
                window.id,
                if window.close.is_some() {
                    format!("close_{}", window.id)
                } else {
                    "null".into()
                },
            ));
        }
        out.close("},");
        out.line(format!("Primary = {},", string(&app.windows.primary)));
        let players: Vec<_> = app
            .players
            .iter()
            .map(|player| string(&player.id))
            .collect();
        out.line(format!(
            "Players = new string[] {{ {} }},",
            players.join(", ")
        ));
        out.line(format!(
            "CompactBelow = {},",
            null(app.layouts.map(|layouts| float(layouts.compact_below)))
        ));
        if let Some(seed) = app.theme.brand_seed {
            out.line(format!(
                "BrandSeed = ((byte){}, (byte){}, (byte){}),",
                seed.red, seed.green, seed.blue
            ));
        }
        out.close("};");
        out.close("}");
        out.line("");
        out.line("static class Program");
        out.open("{");
        out.line("[STAThread]");
        out.line(format!(
            "static void Main() => Launcher.Run(Ui.App, {});",
            string(&app.app.bundle_id)
        ));
        out.close("}");
        out.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_escape_for_csharp() {
        assert_eq!(string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        assert_eq!(string("中文"), "\"中文\"");
        assert_eq!(string("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn floats_are_double_literals() {
        assert_eq!(float(1.0), "1.0");
        assert_eq!(float(0.25), "0.25");
        assert_eq!(float(f64::INFINITY), "double.PositiveInfinity");
    }

    #[test]
    fn literals_build_values() {
        let value = Value::Enum {
            variant: 2,
            fields: vec![Value::Text("x".into()), Value::Optional(None)],
        };
        assert_eq!(literal(&value), "V.Enum(2u, V.Text(\"x\"), V.None)");
        assert_eq!(literal(&Value::List(vec![])), "V.List()");
    }
}
