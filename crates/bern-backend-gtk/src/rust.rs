//! Rust generation from the IR.
//!
//! The generated module builds every component and window from the
//! runtime's standard components (`bern-gtk`). Expressions become Rust
//! expressions over schema values evaluated in a context `c`; locals
//! (component parameters, row variables, pattern bindings) live in the
//! context, so closures capture nothing and each binding re-runs only when
//! a cell it read changes.

use bern_ir::{
    Action, Application, BinaryOp, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind,
    IfLetNode, IfNode, MatchNode, MediaOp, Node, ParamKind, Prop, Text, WindowDef, WindowInitial,
    WindowOp, WindowRole,
};
use bern_protocol::{EnumDef, Type, TypeTable, Value};

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

/// A Rust string literal.
pub fn string(value: &str) -> String {
    format!("{value:?}")
}

fn float(value: f64) -> String {
    let text = format!("{value:?}");
    if text.contains('.') || text.contains('e') || text.contains("inf") || text.contains("NaN") {
        text
    } else {
        format!("{text}.0")
    }
}

fn some(value: Option<String>) -> String {
    value
        .map(|value| format!("Some({value})"))
        .unwrap_or_else(|| "None".into())
}

/// A schema value as a Rust expression.
fn literal(value: &Value) -> String {
    let list = |values: &[Value]| values.iter().map(literal).collect::<Vec<_>>().join(", ");
    match value {
        Value::Unit => "V::Unit".into(),
        Value::Bool(value) => format!("V::Bool({value})"),
        Value::Int(value) => format!("V::Int({value})"),
        Value::Float(value) => format!("V::Float({})", float(*value)),
        Value::Text(value) => format!("V::Text({}.into())", string(value)),
        Value::Enum { variant, fields } => {
            format!(
                "V::Enum {{ variant: {variant}, fields: vec![{}] }}",
                list(fields)
            )
        }
        Value::Optional(None) => "V::Optional(None)".into(),
        Value::Optional(Some(value)) => format!("V::some({})", literal(value)),
        Value::Record(values) => format!("V::Record(vec![{}])", list(values)),
        Value::List(values) => format!("V::List(vec![{}])", list(values)),
    }
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
            // The GTK runtime keeps no window idle state yet.
            Binding::Window { .. } => literal(&bern_protocol::Value::Bool(false)),
        }
    }

    fn expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => literal(value),
            ExprKind::Binding { binding } => self.binding(binding),
            ExprKind::Variant { variant, fields } => {
                let values: Vec<_> = fields.iter().map(|field| self.expr(field)).collect();
                format!("rt::variant({variant}, vec![{}])", values.join(", "))
            }
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

    fn condition(&self, expr: &Expr) -> String {
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

    fn text(&self, text: &Text) -> String {
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
                                .map(|(_, expr)| self.display(expr))
                                .unwrap_or_else(|| "String::new()".into());
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

    fn text_prop(&self, element: &Element, name: &str) -> Option<String> {
        element.text(name).map(|text| self.text_getter(text))
    }

    fn getter(&self, expr: &Expr) -> String {
        format!("b::get(|c| {})", self.expr(expr))
    }

    fn id(&self, element: &Element) -> String {
        string(element.id.as_deref().unwrap_or(&element.path))
    }

    fn field(&self, element: &Element, name: &str) -> Option<String> {
        let field = element.field(name)?;
        Some(format!(
            "b::Field {{ session: {}, field: {} }}",
            self.session_index(&field.session),
            field.field
        ))
    }

    fn action_body(&self, action: &Action) -> String {
        match action {
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

    fn action(&self, element: &Element) -> String {
        some(
            element
                .action
                .as_ref()
                .map(|action| format!("b::act(|c| {})", self.action_body(action))),
        )
    }

    fn keyword(element: &Element, name: &str) -> String {
        some(element.keyword(name).map(string))
    }

    fn icon(element: &Element, name: &str) -> String {
        some(element.icon(name).map(string))
    }

    fn number(element: &Element, name: &str) -> String {
        some(element.number(name).map(float))
    }

    fn enabled(&self, element: &Element) -> String {
        some(element.expr("enabled").map(|expr| self.getter(expr)))
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            self.node(out, node);
        }
    }

    /// A children builder: `b::build(|p, c| { … })`.
    fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        out.open(format!("{prefix}b::build(|p, c| {{"));
        self.nodes(out, nodes);
        out.close(format!("}}){suffix}"));
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
                out.line(format!(
                    "c_{}(p, &c.child({}, vec![{}]));",
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
            branch.push_str(&format!(
                "if {} {{ Some({index}) }} else ",
                self.condition(condition)
            ));
        }
        if node.otherwise.is_empty() {
            branch.push_str("{ None }");
        } else {
            branch.push_str(&format!("{{ Some({}) }}", node.branches.len()));
        }
        out.open(format!(
            "b::When(p, c, Rc::new(|c: &b::Ctx| {branch}), vec!["
        ));
        for (_, body) in &node.branches {
            self.children(out, "", body, ",");
        }
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, ",");
        }
        out.close("]);");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode) {
        out.open(format!(
            "b::IfLet(p, c, {}, {},",
            string(&node.var),
            self.getter(&node.value)
        ));
        self.children(out, "", &node.then, ",");
        if node.otherwise.is_empty() {
            out.line("None,");
        } else {
            self.children(out, "Some(", &node.otherwise, "),");
        }
        out.close(");");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode) {
        out.open(format!(
            "b::Match(p, c, {}, vec![",
            self.getter(&node.value)
        ));
        for arm in &node.arms {
            let bindings: Vec<_> = arm
                .bindings
                .iter()
                .map(|binding| format!("({}, {})", string(&binding.name), binding.field))
                .collect();
            out.open(format!(
                "b::MatchArm {{ variant: {}, bindings: vec![{}], build:",
                some(arm.variant.map(|variant| variant.to_string())),
                bindings.join(", ")
            ));
            self.children(out, "", &arm.body, ",");
            out.close("},");
        }
        out.close("]);");
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
                            Some(variant) => format!("Some({variant}) => {label},"),
                            None => format!("_ => {label},"),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "match rt::tag(&{}) {{ {} _ => String::new() }}",
                    self.expr(&node.value),
                    arms.join(" ")
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// Name and label of an enum variant value, for selections.
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
            format!("b::str(|_| b::label({}, {index}))", string(&def.name))
        } else {
            format!("b::str(|_| {}.to_string())", string(&variant.name))
        }
    }

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => out.line(format!(
                "b::Text(p, c, b::TextProps {{ content: {}, style: {}, emphasis: {} }});",
                some(self.text_prop(element, "content")),
                Self::keyword(element, "style"),
                Self::keyword(element, "emphasis"),
            )),
            "Label" => out.line(format!(
                "b::Label(p, c, b::LabelProps {{ title: {}, icon: {}, style: {} }});",
                some(self.text_prop(element, "title")),
                Self::icon(element, "icon"),
                Self::keyword(element, "style"),
            )),
            "Button" => out.line(format!(
                "b::Button(p, c, b::ButtonProps {{ id: {}, label: {}, icon: {}, role: {}, prominence: {}, enabled: {} }}, {});",
                self.id(element),
                some(self.text_prop(element, "label")),
                Self::icon(element, "icon"),
                Self::keyword(element, "role"),
                Self::keyword(element, "prominence"),
                self.enabled(element),
                self.action(element),
            )),
            "IconButton" => out.line(format!(
                "b::IconButton(p, c, b::IconButtonProps {{ id: {}, icon: {}, label: {}, enabled: {} }}, {});",
                self.id(element),
                string(element.icon("icon").unwrap_or("circle")),
                some(self.text_prop(element, "label")),
                self.enabled(element),
                self.action(element),
            )),
            "TextInput" => out.line(format!(
                "b::TextInput(p, c, b::TextInputProps {{ id: {}, label: {}, field: {}, prompt: {}, kind: {} }}, {});",
                self.id(element),
                some(self.text_prop(element, "label")),
                some(self.field(element, "value")),
                some(self.text_prop(element, "prompt")),
                Self::keyword(element, "kind"),
                self.action(element),
            )),
            "Search" => out.line(format!(
                "b::Search(p, c, {}, {});",
                self.search_props(element),
                self.action(element),
            )),
            "Switch" => out.line(format!(
                "b::Switch(p, c, b::SwitchProps {{ id: {}, label: {}, field: {}, style: {} }});",
                self.id(element),
                some(self.text_prop(element, "label")),
                some(self.field(element, "value")),
                Self::keyword(element, "style"),
            )),
            "Slider" => out.line(format!(
                "b::Slider(p, c, b::SliderProps {{ id: {}, label: {}, field: {}, min: {}, max: {}, step: {}, min_icon: {}, max_icon: {} }}, {});",
                self.id(element),
                some(self.text_prop(element, "label")),
                some(self.field(element, "value")),
                Self::number(element, "min"),
                Self::number(element, "max"),
                Self::number(element, "step"),
                Self::icon(element, "min_icon"),
                Self::icon(element, "max_icon"),
                self.action(element),
            )),
            "Picker" => self.picker(out, element),
            "LanguagePicker" => out.line(format!(
                "b::LanguagePicker(p, c, b::LanguagePickerProps {{ id: {}, label: {} }});",
                self.id(element),
                some(self.text_prop(element, "label")),
            )),
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => self.children(
                out,
                &format!(
                    "b::Form(p, c, b::FormProps {{ title: {} }}, ",
                    some(self.text_prop(element, "title"))
                ),
                &element.children,
                ");",
            ),
            "Section" => self.children(
                out,
                &format!(
                    "b::Section(p, c, b::SectionProps {{ title: {}, footer: {} }}, ",
                    some(self.text_prop(element, "title")),
                    some(self.text_prop(element, "footer"))
                ),
                &element.children,
                ");",
            ),
            "Column" | "Row" => self.children(
                out,
                &format!(
                    "b::Stack(p, c, b::StackProps {{ axis: {}, gap: {}, padding: {}, align: {} }}, ",
                    string(if element.kind == "Column" { "column" } else { "row" }),
                    Self::number(element, "gap"),
                    Self::number(element, "padding"),
                    Self::keyword(element, "align"),
                ),
                &element.children,
                ");",
            ),
            "Grid" => self.children(
                out,
                &format!(
                    "b::Grid(p, c, b::GridProps {{ min_width: {}, gap: {}, padding: {} }}, ",
                    float(element.number("min_width").unwrap_or(120.0)),
                    Self::number(element, "gap"),
                    Self::number(element, "padding"),
                ),
                &element.children,
                ");",
            ),
            "Stack" => self.children(
                out,
                &format!(
                    "b::Layers(p, c, b::LayersProps {{ align: {} }}, ",
                    Self::keyword(element, "align")
                ),
                &element.children,
                ");",
            ),
            "Image" => out.line(format!(
                "b::Image(p, c, b::ImageProps {{ source: {}, label: {}, width: {}, height: {}, fit: {} }});",
                string(element.resource("source").unwrap_or_default()),
                some(self.text_prop(element, "label")),
                Self::number(element, "width"),
                Self::number(element, "height"),
                Self::keyword(element, "fit"),
            )),
            "Progress" => out.line(format!(
                "b::Progress(p, c, b::ProgressProps {{ value: {}, total: {}, label: {} }});",
                some(element.expr("value").map(|value| self.getter(value))),
                Self::number(element, "total"),
                some(self.text_prop(element, "label")),
            )),
            "Spacer" => out.line("b::Spacer(p);"),
            "Divider" => out.line("b::Divider(p);"),
            "ScrollView" => self.children(
                out,
                &format!(
                    "b::ScrollView(p, c, b::ScrollProps {{ axis: {} }}, ",
                    Self::keyword(element, "axis")
                ),
                &element.children,
                ");",
            ),
            "Toolbar" => self.children(out, "b::Toolbar(p, c, ", &element.children, ");"),
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => {
                let route = element
                    .expr("route")
                    .map(|route| self.getter(route))
                    .unwrap_or_else(|| "b::get(|_| V::Unit)".into());
                let label = self
                    .row_label(&element.children)
                    .unwrap_or_else(|| "String::new()".into());
                self.children(
                    out,
                    &format!(
                        "b::NavLink(p, c, {}, {route}, b::str(|c| {label}), ",
                        self.id(element)
                    ),
                    &element.children,
                    ");",
                );
            }
            "List" => self.children(out, "b::List(p, c, ", &element.children, ");"),
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

    fn search_props(&self, element: &Element) -> String {
        format!(
            "b::SearchProps {{ id: {}, field: {}, prompt: {} }}",
            self.id(element),
            some(self.field(element, "value")),
            some(self.text_prop(element, "prompt")),
        )
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
                    "b::option({index}, {}, {})",
                    string(&variant.name),
                    self.variant_label(def, index)
                )
            })
            .collect();
        out.line(format!(
            "b::Picker(p, c, b::PickerProps {{ id: {}, label: {}, field: {}, options: vec![{}], style: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.field(element, "value")),
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
                    "b::Each(p, c, {}, {field}, {}, ",
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
            "b::Video(p, c, b::VideoProps {{ id: {}, source: {}, player: {}, label: {}, controls: {}, fit: {} }});",
            self.id(element),
            some(source),
            some(player),
            some(self.text_prop(element, "label")),
            element.keyword("controls") != Some("none"),
            Self::keyword(element, "fit"),
        ));
    }

    fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let params: Vec<_> = binding.args.iter().map(|arg| self.getter(arg)).collect();
        self.children(
            out,
            &format!(
                "b::Query(p, c, b::QueryProps {{ id: {}, query: {}, params: vec![{}], loading: {}, failed: {}, empty: {}, icon: {}, var: {} }}, ",
                some(element.id.as_ref().map(|_| self.id(element))),
                binding.query,
                params.join(", "),
                some(self.text_prop(element, "loading")),
                some(self.text_prop(element, "failed")),
                some(self.text_prop(element, "empty")),
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
        out.open("b::SplitView(p, c, b::SplitSpec {");
        out.line(format!(
            "id: {},",
            some(sidebar.id.as_ref().map(|_| self.id(sidebar)))
        ));
        out.line(format!("selection: {selection},"));
        match search {
            Some(search) => out.line(format!(
                "search: Some(({}, {})),",
                self.search_props(search),
                self.action(search)
            )),
            None => out.line("search: None,"),
        }
        out.open("sections: vec![");
        for (title, items) in &sections {
            out.open(format!(
                "b::SidebarSection {{ title: {}, items: vec![",
                some(title.clone())
            ));
            for item in items {
                let Some(value) = item.expr("value") else {
                    continue;
                };
                out.line(format!(
                    "b::SidebarItem {{ value: {}, name: {}, label: {}, icon: {} }},",
                    self.getter(value),
                    self.variant_name(value),
                    some(self.text_prop(item, "label")),
                    Self::icon(item, "icon"),
                ));
            }
            out.close("] },");
        }
        out.close("],");
        out.close("},");
        out.indent += 1;
        self.children(out, "", &detail.children, "");
        out.indent -= 1;
        out.line(");");
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
            "b::Navigation(p, c, {}, {path},",
            some(element.id.as_ref().map(|_| self.id(element)))
        ));
        self.children(out, "", &content, ",");
        out.line(format!("{},", string(&var)));
        let destination_nodes = destination
            .map(|destination| destination.children.clone())
            .unwrap_or_default();
        self.children(out, "", &destination_nodes, ",");
        out.close(");");
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
        out.open(format!(
            "b::Adaptive(p, c, {},",
            some(element.id.as_ref().map(|_| self.id(element)))
        ));
        for size in ["compact", "expanded"] {
            match layout(size) {
                Some(layout) => self.children(out, "Some(", &layout.children, "),"),
                None => out.line("None,"),
            }
        }
        out.close(");");
    }

    /// A menu button and its items; dividers are `None`.
    fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "b::Menu(p, c, b::MenuProps {{ id: {}, label: {}, icon: {} }}, vec![",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
        ));
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("None,");
                continue;
            }
            out.line(format!(
                "Some(b::MenuItem {{ id: {}, label: {}, icon: {}, role: {}, enabled: {}, action: {} }}),",
                self.id(item),
                some(self.text_prop(item, "label")),
                Self::icon(item, "icon"),
                Self::keyword(item, "role"),
                self.enabled(item),
                self.action(item),
            ));
        }
        out.close("]);");
    }

    fn tabs(&self, out: &mut Out, element: &Element) {
        let Some(selection) = self.field(element, "selection") else {
            return;
        };
        out.open(format!(
            "b::Tabs(p, c, {}, {selection}, vec![",
            some(element.id.as_ref().map(|_| self.id(element)))
        ));
        for node in &element.children {
            let Node::Element(tab) = node else { continue };
            let Some(value) = tab.expr("value") else {
                continue;
            };
            out.open(format!(
                "b::TabSpec {{ value: {}, name: {}, label: {}, icon: {}, build:",
                self.getter(value),
                self.variant_name(value),
                self.text_prop(tab, "label")
                    .unwrap_or_else(|| "b::str(|_| String::new())".into()),
                Self::icon(tab, "icon"),
            ));
            self.children(out, "", &tab.children, ",");
            out.close("},");
        }
        out.close("]);");
    }

    fn component(&self, out: &mut Out, component: &ComponentDef) {
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!(
            "fn c_{}(p: &b::Parent, c: &b::Ctx) {{",
            component.name
        ));
        self.node(out, &component.body);
        out.close("}");
        out.line("");
    }

    fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!("fn w_{}(p: &b::Parent, c: &b::Ctx) {{", window.id));
        self.node(out, &window.content);
        out.close("}");
        out.line("");
        if let Some(close) = &window.close {
            let save = match &close.save {
                Some(Action::Engine { command, args }) => {
                    let values: Vec<_> = args.iter().map(|arg| self.expr(arg)).collect();
                    format!(
                        "Some(Rc::new(|c: &b::Ctx, done: b::Done| b::dispatch(c, {command}, vec![{}], Some(done))))",
                        values.join(", ")
                    )
                }
                _ => "None".into(),
            };
            out.line(format!(
                "/// The close check of the `{}` window, read against its own drafts.",
                window.id
            ));
            out.open(format!(
                "fn close_{}(c: &b::Ctx) -> b::CloseCheck {{",
                window.id
            ));
            out.open("b::CloseCheck {");
            out.line(format!(
                "needs_confirmation: {},",
                self.condition(&close.when)
            ));
            out.line(format!("message: {},", self.text(&close.message)));
            out.line(format!("save: {save},"));
            out.close("}");
            out.close("}");
            out.line("");
        }
    }

    /// The application's `ui.rs`.
    pub fn generate(&self, fingerprint: u64, descriptor: &str) -> String {
        let app = self.app;
        let mut out = Out::default();
        out.line("// Generated by Bern from the Rust declarations. Do not edit.");
        out.line("#![allow(unused_imports, unused_variables, clippy::all)]");
        out.line("");
        out.line("use bern_gtk as b;");
        out.line("use bern_gtk::protocol::Value as V;");
        out.line("use bern_gtk::value as rt;");
        out.line("use std::rc::Rc;");
        out.line("");
        out.line("// Components.");
        for component in &app.components {
            self.component(&mut out, component);
        }
        out.line("// Windows.");
        for window in &app.windows.windows {
            self.window(&mut out, window);
        }

        out.line("/// Strings of every supported language.");
        out.open("static STRINGS: b::text::StringTable = &[");
        for (language, table) in crate::strings::tables(app) {
            out.open(format!("({}, &[", string(&language)));
            for (key, text) in table {
                out.line(format!("({}, {}),", string(&key), string(&text)));
            }
            out.close("]),");
        }
        out.close("];");
        out.line("");

        out.line("/// The application, as the runtime sees it.");
        out.open("pub fn app() -> b::AppDef {");
        out.open("b::AppDef {");
        out.line(format!("id: {},", string(&app.app.id)));
        out.line(format!("bundle_id: {},", string(&app.app.bundle_id)));
        out.line("name: \"app.name\",");
        out.line(format!("version: {},", string(&app.app.version)));
        out.line(format!("fingerprint: 0x{fingerprint:016x},"));
        let languages: Vec<_> = app
            .languages
            .supported
            .iter()
            .map(|language| string(language))
            .collect();
        out.line(format!("languages: &[{}],", languages.join(", ")));
        out.line(format!(
            "default_language: {},",
            string(&app.languages.default)
        ));
        out.line("strings: STRINGS,");
        out.line(format!("descriptor: ({descriptor}).clone(),"));
        out.line(format!(
            "types: {},",
            bern_target_api::glue::types_expr(&app.types)
        ));
        out.open("sessions: vec![");
        for session in &app.sessions {
            let fields: Vec<_> = session
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "({}, {}, {})",
                        string(&field.name),
                        bern_target_api::glue::type_expr(&field.ty),
                        literal(&field.initial)
                    )
                })
                .collect();
            out.line(format!(
                "b::SessionDef {{ name: {}, fields: vec![{}] }},",
                string(&session.name),
                fields.join(", ")
            ));
        }
        out.close("],");
        out.open("windows: vec![");
        for window in &app.windows.windows {
            let optional = |value: Option<f64>| some(value.map(float));
            let sessions: Vec<_> = window
                .sessions
                .iter()
                .map(|name| self.session_index(name).to_string())
                .collect();
            out.line(format!(
                "b::WindowDef {{ id: {}, title: {}, settings: {}, width: {}, height: {}, min_width: {}, min_height: {}, initial_open: {}, sessions: vec![{}], render: w_{}, close: {} }},",
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
                    format!("Some(close_{})", window.id)
                } else {
                    "None".into()
                },
            ));
        }
        out.close("],");
        out.line(format!("primary: {},", string(&app.windows.primary)));
        let players: Vec<_> = app
            .players
            .iter()
            .map(|player| string(&player.id))
            .collect();
        out.line(format!("players: vec![{}],", players.join(", ")));
        out.line(format!(
            "compact_below: {},",
            some(app.layouts.map(|layouts| float(layouts.compact_below)))
        ));
        out.line(format!(
            "brand_seed: {},",
            some(
                app.theme
                    .brand_seed
                    .map(|seed| format!("({}, {}, {})", seed.red, seed.green, seed.blue))
            )
        ));
        out.close("}");
        out.close("}");
        out.finish()
    }
}
