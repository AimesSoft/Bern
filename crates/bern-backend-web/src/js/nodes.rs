//! The node tree: children callbacks, elements by kind, control flow,
//! components and windows.

use super::{Generator, Out, local, number, string};
use bern_ir::{
    Action, ComponentArg, ComponentDef, Element, IfLetNode, IfNode, MatchNode, Node, ParamKind,
    WindowDef,
};

impl Generator<'_> {
    pub(super) fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            self.node(out, node);
        }
    }

    /// A children callback: `(p, ctx) => { … }`.
    pub(super) fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        self.children_with(out, prefix, &[], nodes, suffix);
    }

    /// A children callback whose first lines act on the container `p`.
    pub(super) fn children_with(
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
    pub(super) fn container_children(
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
    pub(super) fn row_label(&self, nodes: &[Node]) -> Option<String> {
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

    pub(super) fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => self.text_element(out, element),
            "Canvas" => self.canvas(out, element),
            "Label" => self.label(out, element),
            "Button" | "IconButton" => self.button(out, element),
            "TextInput" => self.text_input(out, element),
            "Search" => self.search(out, element),
            "SearchItem" => self.search_item(out, element),
            "Switch" => self.switch(out, element),
            "Slider" => self.slider(out, element),
            "Picker" => self.picker(out, element),
            "Video" => self.video(out, element),
            "Query" => self.query(out, element),
            "Form" => self.form(out, element),
            "Section" if self.in_sidebar.get() => self.sidebar_section(out, element),
            "SidebarItem" => self.sidebar_item(out, element),
            "Section" => self.section(out, element),
            "Column" | "Row" => self.column_or_row(out, element),
            "Grid" => self.grid(out, element),
            "Stack" => self.stack(out, element),
            "Marquee" => self.marquee(out, element),
            "ScaleBox" | "Positioned" => self.scale_box(out, element),
            "Modal" | "Flow" | "Accordion" => self.sized_container(out, element),
            "Timer" => self.timer(out, element),
            "Shortcut" => self.key_shortcut(out, element),
            "Svg" => self.svg(out, element),
            "Image" => self.image(out, element),
            "Progress" => self.progress(out, element),
            "Spacer" => out.line("B.Spacer(p);"),
            // A page has no window buttons of its own.
            "WindowButtons" => {}
            "Divider" => out.line("B.Divider(p);"),
            "ScrollView" => self.scroll_view(out, element),
            "Rail" => self.rail(out, element),
            "PopoverButton" => self.popover_button(out, element),
            "Toast" => self.toast(out, element),
            "Inspector" => self.inspector(out, element),
            "ControlGroup" => self.control_group(out, element),
            "SplitButton" => self.split_button(out, element),
            "Drop" => self.drop_zone(out, element),
            "Pinch" | "Swipe" => self.gesture(out, element),
            "Toolbar" => self.toolbar(out, element),
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => self.nav_link(out, element),
            "List" | "ForEach" => self.list(out, element),
            "Tabs" => self.tabs(out, element),
            "Tab" => self.tab(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "LanguagePicker" => self.language_picker(out, element),
            "Detail" | "Destination" | "TabAccessory" | "Sidebar" | "SidebarHeader"
            | "SidebarFooter" | "WindowSet" | "Window" | "Layout" | "MenuItem" | "PickerOption" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }

    pub(super) fn component(&self, out: &mut Out, component: &ComponentDef) {
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

    pub(super) fn window(&self, out: &mut Out, window: &WindowDef) {
        out.line(format!("// Root of the `{}` window.", window.id));
        out.open(format!("function w_{}(p, ctx) {{", window.id));
        self.node(out, &window.content);
        out.close("}");
        out.line("");
    }

    /// The close check of a window, evaluated against its own drafts.
    pub(super) fn close_check(&self, close: &bern_ir::CloseCheck) -> String {
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
}
