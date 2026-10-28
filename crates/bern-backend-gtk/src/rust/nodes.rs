//! The node tree: children builders, elements by kind, control flow,
//! components and windows.

use super::{Generator, Out, float, some, string};
use bern_ir::{
    Action, ComponentArg, ComponentDef, Element, IfLetNode, IfNode, MatchNode, Node, ParamKind,
    WindowDef,
};

impl Generator<'_> {
    /// Nodes in order; an `Inspector` among them takes the others as the
    /// content it pushes aside.
    pub(super) fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        let inspector = nodes.iter().find_map(|node| match node {
            Node::Element(element) if element.kind == "Inspector" => Some(element),
            _ => None,
        });
        let Some(inspector) = inspector else {
            for node in nodes {
                self.node(out, node);
            }
            return;
        };
        let rest: Vec<Node> = nodes
            .iter()
            .filter(|node| !matches!(node, Node::Element(element) if element.kind == "Inspector"))
            .cloned()
            .collect();
        let props = format!(
            "b::InspectorProps {{ presented: {}, width: {} }}",
            self.field(inspector, "presented")
                .unwrap_or_else(|| "b::Field { session: 0, field: 0 }".into()),
            Self::number(inspector, "width")
        );
        out.open(format!("b::Inspector(p, c, {props},"));
        self.children(out, "", &rest, ",");
        self.children(out, "", &inspector.children, ",");
        out.close(");");
    }

    /// A children builder: `b::build(|p, c| { … })`.
    pub(super) fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        out.open(format!("{prefix}b::build(|p, c| {{"));
        self.nodes(out, nodes);
        out.close(format!("}}){suffix}"));
    }

    /// A container's children builder; with `stagger`, its children take
    /// turns appearing.
    pub(super) fn container_children(
        &self,
        out: &mut Out,
        prefix: &str,
        element: &Element,
        suffix: &str,
    ) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!("{prefix}b::build(|p, c| {{"));
                out.open(format!("b::staggered({}, || {{", float(step)));
                self.nodes(out, &element.children);
                out.close("});");
                out.close(format!("}}){suffix}"));
            }
            None => self.children(out, prefix, &element.children, suffix),
        }
    }

    fn node(&self, out: &mut Out, node: &Node) {
        match node {
            Node::Element(element) => {
                let placement = self.placement(element);
                if let Some(spec) = &placement {
                    out.open(format!("b::Place(p, c, {spec}, b::build(|p, c| {{"));
                }
                // A modal's motion is its card's, above the window.
                let decoration = if element.kind == "Modal" {
                    None
                } else {
                    self.decoration(element)
                };
                match decoration {
                    Some(decor) => {
                        out.open(format!("b::Decorate(p, c, {decor}, b::build(|p, c| {{"));
                        self.element(out, element);
                        out.close("}));");
                    }
                    None => self.element(out, element),
                }
                if placement.is_some() {
                    out.close("}));");
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
    pub(super) fn row_label(&self, nodes: &[Node]) -> Option<String> {
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

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => self.text_element(out, element),
            "Label" => self.label(out, element),
            "Button" if !element.children.is_empty() => self.content_button(out, element),
            "Button" => self.button(out, element),
            "IconButton" => self.icon_button(out, element),
            "TextInput" => self.text_input(out, element),
            "Search" => self.search(out, element),
            "SearchItem" => self.search_item(out, element),
            "Switch" => self.switch(out, element),
            "Slider" => self.slider(out, element),
            "Picker" => self.picker(out, element),
            "LanguagePicker" => self.language_picker(out, element),
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => self.form(out, element),
            "Section" if self.in_sidebar.get() => self.sidebar_section(out, element),
            "Section" => self.section(out, element),
            "SidebarItem" => self.sidebar_item(out, element),
            "Column" | "Row" => self.column_or_row(out, element),
            "Grid" => self.grid(out, element),
            "Stack" => self.stack(out, element),
            "FocusRegion" => self.children(out, "b::FocusRegion(p, c, ", &element.children, ");"),
            "ScaleBox" => self.scale_box(out, element),
            "Positioned" => self.positioned(out, element),
            "Marquee" => self.marquee(out, element),
            "Accordion" => self.accordion(out, element),
            "Flow" => self.flow(out, element),
            "Modal" => self.modal(out, element),
            "Timer" => self.timer(out, element),
            "Shortcut" => self.key_shortcut(out, element),
            "ShortcutInput" => self.shortcut_input(out, element),
            "ControlGroup" => self.control_group(out, element),
            "Svg" => self.svg(out, element),
            "Image" => self.image(out, element),
            "Progress" => self.progress(out, element),
            "Spacer" => out.line("b::Spacer(p);"),
            // A header bar keeps its buttons at its ends, never over content.
            "WindowButtons" => {}
            "Divider" => out.line("b::Divider(p);"),
            "ScrollView" => self.scroll_view(out, element),
            "Canvas" => self.canvas(out, element),
            "Rail" => self.rail(out, element),
            "PopoverButton" => self.popover_button(out, element),
            "SplitButton" => self.split_button(out, element),
            "Toast" => self.toast(out, element),
            "Drop" => self.drop_zone(out, element),
            "Pinch" | "Swipe" => self.gesture(out, element),
            // Items before a `Spacer` stand at the leading end of the header.
            "Toolbar" => match element
                .children
                .iter()
                .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"))
            {
                Some(index) => {
                    self.children(
                        out,
                        "b::LeadingToolbar(p, c, ",
                        &element.children[..index],
                        ");",
                    );
                    self.children(
                        out,
                        "b::Toolbar(p, c, ",
                        &element.children[index + 1..],
                        ");",
                    );
                }
                None => self.children(out, "b::Toolbar(p, c, ", &element.children, ");"),
            },
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => self.nav_link(out, element),
            "List" => self.children(out, "b::List(p, c, ", &element.children, ");"),
            "ForEach" => self.for_each(out, element),
            "PreviewList" => self.preview_list(out, element),
            "Tabs" => self.tabs(out, element),
            "Tab" => self.tab(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "Table" => self.table(out, element),
            "Detail" | "Destination" | "Sidebar" | "SidebarHeader" | "SidebarFooter"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" | "Inspector" | "Shape" | "Series"
            // GNOME applications keep no menu bar.
            | "MenuBar" | "CommandMenu" | "CommandItem"
            // The application builds the status item's menu.
            | "StatusItem"
            | "Pattern" | "TabAccessory" | "PickerOption" | "TableColumn" | "TableMenu" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }

    pub(super) fn component(&self, out: &mut Out, component: &ComponentDef) {
        out.line(format!(
            "/// Generated from the `{}` component.",
            component.name
        ));
        out.open(format!(
            "fn c_{}(p: &b::Parent, c: &b::Ctx) {{",
            component.name
        ));
        self.nodes(out, std::slice::from_ref(&component.body));
        out.close("}");
        out.line("");
    }

    pub(super) fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("/// Root of the `{}` window.", window.id));
        out.open(format!("fn w_{}(p: &b::Parent, c: &b::Ctx) {{", window.id));
        self.nodes(out, std::slice::from_ref(&window.content));
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
}
