//! The node tree: each element inside its placement and decoration,
//! components where they are used, `if`, `if let` and `match`, and the
//! call each kind of element makes.

use super::{BUILDER, Generator, Out, designated, float, ident, motion_ref, string};
use bern_ir::{ComponentArg, ComponentRef, Element, IfLetNode, IfNode, MatchNode, Node, ParamKind};

impl Generator<'_> {
    pub(super) fn nodes(&self, out: &mut Out, nodes: &[Node]) {
        // An inspector stands beside the other content of its container.
        let inspector = nodes.iter().find_map(|node| match node {
            Node::Element(element) if element.kind == "Inspector" => Some(element),
            _ => None,
        });
        if let Some(inspector) = inspector {
            let others: Vec<Node> = nodes
                .iter()
                .filter(
                    |node| !matches!(node, Node::Element(element) if element.kind == "Inspector"),
                )
                .cloned()
                .collect();
            out.open(format!(
                "B::Inspector(p, c, {},",
                designated(&[
                    ("presented", self.field(inspector, "presented")),
                    ("width", Self::number(inspector, "width")),
                ])
            ));
            self.children(out, "", &others, ",");
            self.children(out, "", &inspector.children, ");");
            out.indent -= 1;
            return;
        }
        for node in nodes {
            self.node(out, node);
        }
    }

    /// A children builder, `[](Parent const& p, Ctx const& c) { … }`,
    /// between `prefix` and `suffix`.
    pub(super) fn children(&self, out: &mut Out, prefix: &str, nodes: &[Node], suffix: &str) {
        out.open(format!("{prefix}{BUILDER}"));
        self.nodes(out, nodes);
        out.close(format!("}}{suffix}"));
    }

    /// A call whose last argument is a children builder.
    pub(super) fn call(&self, out: &mut Out, head: &str, nodes: &[Node]) {
        self.children(out, head, nodes, ");");
    }

    /// A container's call: with `stagger`, its children take turns
    /// appearing.
    pub(super) fn container(&self, out: &mut Out, element: &Element, head: &str) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!("{head}[](Parent const& p, Ctx const& outer) {{"));
                out.line(format!(
                    "auto const c = B::Staggered(outer, {});",
                    float(step)
                ));
                self.nodes(out, &element.children);
                out.close("});");
            }
            None => self.call(out, head, &element.children),
        }
    }

    /// Where an element sits on its `Rail`, for `B::Place`.
    fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| self.len(element, name);
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        (at.is_some() || from.is_some()).then(|| {
            designated(&[
                ("at", at),
                ("from", from),
                ("to", to),
                ("animate", Self::animate(element)),
            ])
        })
    }

    /// The motion and appearance props of an element, for `B::Decorate`.
    fn decoration(&self, element: &Element) -> Option<String> {
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("&X_{}", ident(transition)))
        };
        let hero = element.keyword("hero").map(string);
        let hero_key = hero
            .as_ref()
            .and(element.expr("hero_key"))
            .map(|key| self.getter(key));
        let fields = [
            ("appear", transition("appear")),
            ("disappear", transition("disappear")),
            ("hero", hero.clone()),
            ("heroKey", hero_key),
            ("heroMotion", hero.as_ref().map(|_| motion_ref("hero"))),
            (
                "reveal",
                (element.keyword("reveal") == Some("hover")).then(|| "true".to_string()),
            ),
            ("appearance", Self::keyword(element, "appearance")),
            ("tint", Self::keyword(element, "tint")),
            ("nudge", element.expr("nudge").map(|when| self.getter(when))),
            (
                "nudgeMotion",
                (element.expr("nudge").is_some() || element.expr("nudge_key").is_some())
                    .then(|| motion_ref("jelly")),
            ),
            (
                "nudgeKey",
                element.expr("nudge_key").map(|key| self.getter(key)),
            ),
            ("nudgeScale", Self::number(element, "nudge_scale")),
            ("appearDelay", Self::number(element, "appear_delay")),
            (
                "reduceMotion",
                element
                    .expr("reduce_motion")
                    .map(|reduce| self.getter(reduce)),
            ),
            ("scrollScale", Self::number(element, "scroll_scale")),
            ("scrollOpacity", Self::number(element, "scroll_opacity")),
            ("scrollOriginX", Self::number(element, "scroll_origin_x")),
            ("scrollOriginY", Self::number(element, "scroll_origin_y")),
            ("idleArea", Self::number(element, "idle_area")),
        ];
        fields
            .iter()
            .any(|(_, value)| value.is_some())
            .then(|| designated(&fields))
    }

    pub(super) fn node(&self, out: &mut Out, node: &Node) {
        match node {
            Node::Element(element) => {
                let placement = self.placement(element);
                if let Some(spec) = &placement {
                    out.open(format!("B::Place(p, c, {spec}, {BUILDER}"));
                }
                // A modal's motion is its card's, above the window.
                let decoration = if element.kind == "Modal" {
                    None
                } else {
                    self.decoration(element)
                };
                match decoration {
                    Some(spec) => {
                        out.open(format!("B::Decorate(p, c, {spec}, {BUILDER}"));
                        self.element(out, element);
                        out.close("});");
                    }
                    None => self.element(out, element),
                }
                if placement.is_some() {
                    out.close("});");
                }
            }
            Node::Component(reference) => self.component_call(out, reference),
            Node::If(node) => self.if_node(out, node),
            Node::IfLet(node) => self.if_let(out, node),
            Node::Match(node) => self.match_node(out, node),
        }
    }

    /// A use of a component: its function, called with the values the
    /// use gives its parameters.
    fn component_call(&self, out: &mut Out, reference: &ComponentRef) {
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
                            "{{{}, {}}}",
                            string(&param.name),
                            self.getter(expr)
                        )),
                        _ => None,
                    }),
                _ => None,
            })
            .collect();
        let child = if args.is_empty() {
            format!("c.Child({})", string(&component.name))
        } else {
            format!(
                "c.Child({}, {{{}}})",
                string(&component.name),
                args.join(", ")
            )
        };
        out.line(format!("c_{}(p, {child});", component.name));
    }

    fn if_node(&self, out: &mut Out, node: &IfNode) {
        let mut branch = String::from("[](Ctx const& c) -> std::optional<int> { ");
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            branch.push_str(&format!(
                "if ({}) return {index}; ",
                self.condition(condition)
            ));
        }
        if node.otherwise.is_empty() {
            branch.push_str("return std::nullopt; }");
        } else {
            branch.push_str(&format!("return {}; }}", node.branches.len()));
        }
        out.open(format!("B::When(p, c, {branch}, std::vector<Build>{{"));
        for (_, body) in &node.branches {
            self.children(out, "", body, ",");
        }
        if !node.otherwise.is_empty() {
            self.children(out, "", &node.otherwise, ",");
        }
        out.close("});");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode) {
        out.open(format!(
            "B::IfLet(p, c, {}, {},",
            string(&node.var),
            self.getter(&node.value)
        ));
        if node.otherwise.is_empty() {
            self.children(out, "", &node.then, ");");
        } else {
            self.children(out, "", &node.then, ",");
            self.children(out, "", &node.otherwise, ");");
        }
        out.indent -= 1;
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode) {
        out.open(format!(
            "B::Match(p, c, {}, std::vector<B::MatchArm>{{",
            self.getter(&node.value)
        ));
        for arm in &node.arms {
            let bindings: Vec<String> = arm
                .bindings
                .iter()
                .map(|binding| format!("{{{}, {}}}", string(&binding.name), binding.field))
                .collect();
            let suffix = if bindings.is_empty() {
                "),".to_string()
            } else {
                format!(", {{{}}}),", bindings.join(", "))
            };
            self.children(
                out,
                &format!(
                    "B::Arm({}, ",
                    arm.variant
                        .map(|variant| format!("{variant}u"))
                        .unwrap_or_else(|| "std::nullopt".into())
                ),
                &arm.body,
                &suffix,
            );
        }
        out.close("});");
    }

    fn element(&self, out: &mut Out, element: &Element) {
        match element.kind.as_str() {
            "Text" => self.text_element(out, element),
            "Label" => self.label(out, element),
            "Button" | "IconButton" => self.button(out, element),
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
            "Column" | "Row" => self.stack(out, element),
            "Grid" => self.grid(out, element),
            "Stack" => self.layers(out, element),
            "FocusRegion" => self.call(out, "B::FocusRegion(p, c, ", &element.children),
            "ScaleBox" => self.scale_box(out, element),
            "Positioned" => self.positioned(out, element),
            "Marquee" => self.marquee(out, element),
            "Accordion" => self.accordion(out, element),
            "Flow" => self.flow(out, element),
            "Modal" => self.modal(out, element),
            "Timer" => self.timer(out, element),
            "Watch" => self.watch(out, element),
            "Shortcut" => self.key_shortcut(out, element),
            "ShortcutInput" => self.shortcut_input(out, element),
            "ControlGroup" => self.control_group(out, element),
            "Rail" => self.rail(out, element),
            "Svg" => self.svg(out, element),
            "Image" => self.image(out, element),
            "Progress" => self.progress(out, element),
            "Canvas" => self.canvas(out, element),
            "Spacer" => out.line("B::Spacer(p);"),
            // The caption buttons stand at the trailing end of the title bar.
            "WindowButtons" => {}
            "Divider" => out.line("B::Divider(p, c);"),
            "ScrollView" => self.scroll_view(out, element),
            // Items before a `Spacer` stand at the leading end of the
            // header, after its way back; the rest at the trailing end.
            "Toolbar" => match element
                .children
                .iter()
                .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"))
            {
                Some(index) => {
                    self.call(out, "B::LeadingToolbar(p, c, ", &element.children[..index]);
                    self.call(out, "B::Toolbar(p, c, ", &element.children[index + 1..]);
                }
                None => self.call(out, "B::Toolbar(p, c, ", &element.children),
            },
            "SplitView" => self.split_view(out, element),
            "Navigation" => self.navigation(out, element),
            "NavLink" => self.nav_link(out, element),
            "List" => self.list(out, element),
            "ForEach" => self.for_each(out, element),
            "PreviewList" => self.preview_list(out, element),
            "Tabs" => self.tabs(out, element),
            "Adaptive" => self.adaptive(out, element),
            "Menu" => self.menu(out, element),
            "Table" => self.table(out, element),
            "PopoverButton" => self.popover_button(out, element),
            "SplitButton" => self.split_button(out, element),
            "Toast" => self.toast(out, element),
            "Drop" => self.drop_area(out, element),
            "Pinch" | "Swipe" => self.gesture(out, element),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarHeader" | "SidebarFooter"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" | "Inspector" | "TabAccessory"
            // Windows applications of this kind keep no menu bar.
            | "MenuBar" | "CommandMenu" | "CommandItem"
            // The application builds the status item's menu when it opens.
            | "StatusItem"
            | "PickerOption" | "TableColumn" | "TableMenu" => {
                // Realized by their parents.
            }
            other => out.line(format!("/* unsupported {other} */")),
        }
    }
}
