//! Navigation: split views and their sidebars, navigation stacks and
//! links, tabs, and the inspector.

use super::{Generator, Out, local, number, string};
use bern_ir::{Element, Node};

impl Generator<'_> {
    pub(super) fn split_view(&self, out: &mut Out, element: &Element) {
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
        let mut search = None;
        let mut footer = None;
        let mut header = None;
        let mut content = Vec::new();
        for node in &sidebar.children {
            match node {
                Node::Element(child) if child.kind == "Search" => search = Some(child),
                Node::Element(child) if child.kind == "SidebarFooter" => footer = Some(child),
                Node::Element(child) if child.kind == "SidebarHeader" => header = Some(child),
                other => content.push(other.clone()),
            }
        }
        out.open("F.SplitView(p, ctx, {");
        if sidebar.id.is_some() {
            out.line(format!("id: {},", self.id(sidebar)));
        }
        out.line(format!("selection: {selection},"));
        if let Some(width) = self.length(sidebar, "width") {
            out.line(format!("width: {width},"));
        }
        if let Some(titled) = self.value_prop(detail, "titled") {
            out.line(format!("titled: {titled},"));
        }
        if let Some(fallback) = sidebar.expr("fallback") {
            out.line(format!("fallback: {},", self.expr(fallback)));
        }
        if let Some(search) = search {
            out.open("search: (p, ctx) => {");
            self.element(out, search);
            out.close("},");
        }
        if let Some(header) = header {
            self.children(out, "header: ", &header.children, ",");
        }
        if let Some(footer) = footer {
            self.children(out, "footer: ", &footer.children, ",");
        }
        out.close("}, (p, ctx) => {");
        out.indent += 1;
        self.in_sidebar.set(true);
        self.nodes(out, &content);
        self.in_sidebar.set(false);
        out.close("}, (p, ctx) => {");
        out.indent += 1;
        self.nodes(out, &detail.children);
        out.close("});");
    }

    /// A `Section` among a sidebar's items.
    pub(super) fn sidebar_section(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![("title", self.text_prop(element, "title"))]);
        self.children(
            out,
            &format!("F.SidebarSection(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }

    pub(super) fn sidebar_item(&self, out: &mut Out, element: &Element) {
        let Some(value) = element.expr("value") else {
            return;
        };
        let label = element.text("label").map(|text| self.text(text));
        out.line(format!(
            "F.SidebarItem(p, ctx, {});",
            Self::props(vec![
                ("value", Some(self.expr(value))),
                ("name", Some(self.variant_name(value))),
                ("label", label.map(|label| format!("() => {label}"))),
                ("icon", element.icon("icon").map(string)),
                ("badge", self.length(element, "badge")),
                (
                    "style",
                    crate::style::reference(self.app, "SidebarItem", element.keyword("variant"))
                ),
            ])
        ));
    }

    pub(super) fn navigation(&self, out: &mut Out, element: &Element) {
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

    pub(super) fn nav_link(&self, out: &mut Out, element: &Element) {
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

    pub(super) fn tabs(&self, out: &mut Out, element: &Element) {
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

    pub(super) fn inspector(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("presented", self.field(element, "presented")),
            ("width", element.number("width").map(number)),
            ("motion", Some(self.motion("bouncy"))),
        ]);
        self.children(
            out,
            &format!("F.Inspector(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }
}
