//! Navigation: split views and their sidebars, navigation stacks and
//! their links, and tabs.

use super::values::kotlin_type;
use super::{Ctx, Generator, Out, Scope};
use crate::names::member;
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    // MARK: Navigation containers

    /// Items of a navigation container through `if`, `if let`, `match` and
    /// `ForEach`; `item` writes each one.
    pub(super) fn nav_walk(
        &self,
        out: &mut Out,
        nodes: &[Node],
        item: &dyn Fn(&mut Out, &Element),
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
                // Items stand directly in the container or in its sections;
                // other elements, such as a search's results, hold none.
                Node::Element(element) => match item(element) {
                    Some(statement) => out.line(statement),
                    None if element.kind == "Section" => {
                        self.nav_statements(out, &element.children, item)
                    }
                    None => {}
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

    pub(super) fn split_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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
        let header = sidebar.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "SidebarHeader" => Some(child),
            _ => None,
        });
        // `Detail(titled: false)`: the detail's top bar leaves out the
        // destination's name.
        let title = match detail.expr("titled") {
            Some(titled) => format!("if ({}) bernSelected else \"\"", self.expr(titled)),
            None => "bernSelected".into(),
        };
        let width = self
            .length(sidebar, "width")
            .map(|width| format!("sidebarWidth = {width}, "))
            .unwrap_or_default();
        out.open(format!(
            "BernSplitView({title}, bernShowing, {width}header = "
        ));
        match header {
            Some(header) => {
                out.open("{");
                self.nodes(out, &header.children, &ctx.within(Scope::Column));
                out.close("},");
            }
            None => out.line("null,"),
        }
        out.open("sidebar = { bernShowDetail ->");
        self.sidebar_nodes(out, &sidebar.children, &selection, ctx);
        if let Some(footer) = footer {
            out.close("}, footer = {");
            out.indent += 1;
            self.nodes(out, &footer.children, &ctx.within(Scope::Column));
        }
        out.close("}, detail = {");
        out.indent += 1;
        self.nodes(out, &detail.children, &ctx.within(Scope::Other));
        out.close("},");
        out.close(")");
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
                // A live count beside the page, hidden at zero.
                let badge = element
                    .expr("badge")
                    .map(|badge| format!(", badge = ({}).toLong()", self.expr(badge)))
                    .unwrap_or_default();
                match self.style_of(element) {
                    Some(style) => out.line(format!(
                        "BernStyledSidebarItem({label}, {}, {selection} == {value}, BernStyles.{}{badge}) {{ {selection} = {value}; bernShowDetail() }}",
                        Self::icon(element.icon("icon")),
                        style.name
                    )),
                    None => out.line(format!(
                        "BernSidebarItem({label}, {}, {selection} == {value}{badge}) {{ {selection} = {value}; bernShowDetail() }}",
                        Self::icon(element.icon("icon"))
                    )),
                }
            }
            "Search" => self.element(out, element, ctx),
            _ => {}
        });
    }

    pub(super) fn navigation(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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

    pub(super) fn tabs(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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
            if tab.kind != "Tab" {
                return;
            }
            let value = tab.expr("value").map(|v| self.expr(v)).unwrap_or_default();
            out.open(format!("if ({selection} == {value}) {{"));
            self.nodes(out, &tab.children, &ctx.within(Scope::Other));
            out.close("}");
        });
        // A `TabAccessory`: the bar above the navigation bar while shown.
        let accessory = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "TabAccessory" => Some(child),
            _ => None,
        });
        if let Some(accessory) = accessory {
            out.close("}, accessory = {");
            out.indent += 1;
            out.open(format!(
                "if ({}) BernTabAccessory {{",
                accessory
                    .expr("shown")
                    .map(|shown| self.expr(shown))
                    .unwrap_or_else(|| "true".into())
            ));
            self.nodes(out, &accessory.children, &ctx.within(Scope::Other));
            out.close("}");
        }
        out.close("}, bar = {");
        out.indent += 1;
        self.nav_walk(out, &element.children, &|out, tab| {
            if tab.kind != "Tab" {
                return;
            }
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

    pub(super) fn nav_link(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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
