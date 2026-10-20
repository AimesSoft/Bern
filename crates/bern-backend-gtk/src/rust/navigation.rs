//! Navigation: split views and their sidebars, navigation stacks and
//! links, and tabs.

use super::{Generator, Out, some, string};
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
        let Some(selection) = self.field(sidebar, "selection") else {
            return;
        };
        let mut search = None;
        let mut header = None;
        let mut footer = None;
        let mut content = Vec::new();
        for node in &sidebar.children {
            match node {
                Node::Element(child) if child.kind == "Search" => search = Some(child),
                Node::Element(child) if child.kind == "SidebarHeader" => header = Some(child),
                Node::Element(child) if child.kind == "SidebarFooter" => footer = Some(child),
                other => content.push(other.clone()),
            }
        }
        out.open("b::SplitView(p, c, b::SplitSpec {");
        out.line(format!(
            "id: {},",
            some(sidebar.id.as_ref().map(|_| self.id(sidebar)))
        ));
        out.line(format!("selection: {selection},"));
        out.line(format!(
            "fallback: {},",
            some(sidebar.expr("fallback").map(|value| self.getter(value)))
        ));
        match search {
            Some(search) => out.line(format!(
                "search: Some(({}, {})),",
                self.search_props(search),
                self.action(search)
            )),
            None => out.line("search: None,"),
        }
        match header {
            Some(header) => self.children(out, "header: Some(", &header.children, "),"),
            None => out.line("header: None,"),
        }
        match footer {
            Some(footer) => self.children(out, "footer: Some(", &footer.children, "),"),
            None => out.line("footer: None,"),
        }
        out.line(format!("width: {},", Self::number(sidebar, "width")));
        out.line(format!("titled: {},", self.value(detail, "titled")));
        out.close("},");
        out.indent += 1;
        self.in_sidebar.set(true);
        self.children(out, "", &content, ",");
        self.in_sidebar.set(false);
        self.children(out, "", &detail.children, "");
        out.indent -= 1;
        out.line(");");
    }

    /// A `Section` among a sidebar's items.
    pub(super) fn sidebar_section(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::SidebarSection(p, c, {}, ",
                some(self.text_prop(element, "title"))
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn sidebar_item(&self, out: &mut Out, element: &Element) {
        let Some(value) = element.expr("value") else {
            return;
        };
        out.line(format!(
            "b::SidebarItem(p, c, b::SidebarItemProps {{ value: {}, enum_name: {}, label: {}, icon: {}, style: {}, badge: {} }});",
            self.getter(value),
            Self::enum_name(value),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            self.style("SidebarItem", element),
            some(self.num(element, "badge")),
        ));
    }

    pub(super) fn navigation(&self, out: &mut Out, element: &Element) {
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

    pub(super) fn nav_link(&self, out: &mut Out, element: &Element) {
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

    pub(super) fn tabs(&self, out: &mut Out, element: &Element) {
        let Some(selection) = self.field(element, "selection") else {
            return;
        };
        let accessory = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "TabAccessory" => Some(child),
            _ => None,
        });
        self.children(
            out,
            &format!(
                "b::Tabs(p, c, {}, {selection}, ",
                some(element.id.as_ref().map(|_| self.id(element)))
            ),
            &element.children,
            ",",
        );
        match accessory {
            Some(accessory) => self.children(
                out,
                &format!(
                    "Some(b::TabAccessory {{ shown: {}, build: ",
                    some(accessory.expr("shown").map(|shown| self.getter(shown)))
                ),
                &accessory.children,
                " }));",
            ),
            None => out.line("None);"),
        }
    }

    pub(super) fn tab(&self, out: &mut Out, tab: &Element) {
        let Some(value) = tab.expr("value") else {
            return;
        };
        self.children(
            out,
            &format!(
                "b::Tab(p, c, b::TabProps {{ value: {}, enum_name: {}, label: {}, icon: {}, icon_only: {} }}, ",
                self.getter(value),
                Self::enum_name(value),
                self.text_prop(tab, "label")
                    .unwrap_or_else(|| "b::str(|_| String::new())".into()),
                Self::icon(tab, "icon"),
                tab.keyword("display") == Some("icon"),
            ),
            &tab.children,
            ");",
        );
    }
}
