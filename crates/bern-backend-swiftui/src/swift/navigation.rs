//! Navigation: split views and their sidebars, navigation stacks and
//! their links, and tabs.

use super::values::swift_type;
use super::{Ctx, Generator, Out, Platform, case_pattern};
use crate::names::{member, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
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
        if let Some(rows) = &rows {
            let ty = sidebar
                .field("selection")
                .map(|field| swift_type(&field.ty))
                .unwrap_or_default();
            out.line(format!(
                ".bernSidebarNavigation(selection: {rows}, values: {})",
                self.nav_values(&sidebar.children, "SidebarItem", &ty)
            ));
        }
        // tvOS's sidebar is a floating panel of the system's width.
        if self.platform == Platform::Tvos {
        } else if let Some(width) = self.length(sidebar, "width") {
            out.line(format!(
                ".navigationSplitViewColumnWidth(min: {width}, ideal: {width}, max: {width})"
            ));
            out.line(format!(".frame(minWidth: {width}, maxWidth: {width})"));
        } else {
            out.line(".navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 320)");
        }
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
            // tvOS has no sidebar placement; its search takes the screen.
            let placement = if self.platform == Platform::Tvos {
                ".automatic"
            } else {
                ".sidebar"
            };
            self.searchable(out, search, placement, ctx);
        }
        // The header stays at the top of the sidebar, below the window's
        // buttons; the list scrolls below it.
        if let Some(header) = sidebar.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "SidebarHeader" => Some(child),
            _ => None,
        }) {
            out.open(".safeAreaInset(edge: .top, spacing: 0) {");
            out.open("SwiftUI.VStack(alignment: .leading, spacing: 8) {");
            self.nodes(out, &header.children, ctx);
            out.close("}");
            out.line(".frame(maxWidth: .infinity, alignment: .leading)");
            out.line(".padding(.horizontal, 10)");
            out.line(".padding(.vertical, 8)");
            self.motion_modifiers(out, header);
            out.close("}");
        }
        // The footer stays at the bottom of the sidebar; the list scrolls
        // above it.
        if let Some(footer) = sidebar.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "SidebarFooter" => Some(child),
            _ => None,
        }) {
            out.open(".safeAreaInset(edge: .bottom, spacing: 0) {");
            out.open("SwiftUI.VStack(alignment: .leading, spacing: 8) {");
            self.nodes(out, &footer.children, ctx);
            out.close("}");
            out.line(".frame(maxWidth: .infinity, alignment: .leading)");
            out.line(".padding(.horizontal, 10)");
            out.line(".padding(.vertical, 8)");
            self.motion_modifiers(out, footer);
            out.close("}");
        }
        // The detail column and the sidebar control are both named after
        // the selected destination.
        let title = labeled.then(|| self.nav_label(&selection, &sidebar.children, "SidebarItem"));
        // A compact iOS split view shows the sidebar as the first screen of
        // a stack, titled like the window.
        if self.platform.uikit() {
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
            // The page shows its own title: the title bar does not repeat it.
            if detail
                .expr("titled")
                .map(|titled| self.expr(titled))
                .as_deref()
                == Some("false")
            {
                out.line(".bernToolbarTitleHidden()");
            }
        }
        self.toolbar(out, &detail.children, ctx);
        self.inspector(out, &detail.children, ctx);
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
        self.nav_walk(out, nodes, &|out, element| {
            self.sidebar_child(out, element, rows)
        });
    }

    /// Items of a navigation container through `if`, `if let`, `match` and
    /// `ForEach`, as the container's own builder takes them; `item` writes
    /// each one.
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
                    let each = format!(
                        "SwiftUI.ForEach({}, id: \\.{}) {{ {var} in",
                        self.expr(items),
                        member(name)
                    );
                    out.open(each);
                    self.nav_walk(out, &element.children, item);
                    out.close("}");
                }
                Node::Element(element) => item(out, element),
                Node::If(node) => {
                    for (index, (condition, body)) in node.branches.iter().enumerate() {
                        if index > 0 {
                            out.indent -= 1;
                        }
                        let keyword = if index == 0 { "if" } else { "} else if" };
                        out.open(format!("{keyword} {} {{", self.expr(condition)));
                        self.nav_walk(out, body, item);
                    }
                    if !node.otherwise.is_empty() {
                        out.indent -= 1;
                        out.open("} else {");
                        self.nav_walk(out, &node.otherwise, item);
                    }
                    out.close("}");
                }
                Node::IfLet(node) => {
                    out.open(format!(
                        "if let {} = {} {{",
                        member(&node.var),
                        self.expr(&node.value)
                    ));
                    self.nav_walk(out, &node.then, item);
                    if !node.otherwise.is_empty() {
                        out.indent -= 1;
                        out.open("} else {");
                        self.nav_walk(out, &node.otherwise, item);
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
                        self.nav_walk(out, &arm.body, item);
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
                Node::Element(element)
                    if matches!(
                        element.kind.as_str(),
                        "Search" | "SidebarHeader" | "SidebarFooter" | "Toolbar"
                    ) => {}
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
        // Without a selected item the window's title stands in: an empty
        // title on macOS leaves the toolbar without its title for good.
        format!("{{ () -> String in {statements}; return bernWindow.title }}()")
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
                let badge = element
                    .expr("badge")
                    .map(|e| format!("Int({})", self.expr(e)))
                    .unwrap_or_else(|| "0".into());
                out.line(format!(
                    "BernSidebarRow(label: {label}, systemImage: {icon}, value: {value}, selection: {}, style: {style}, badge: {badge})",
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
                        "BernSidebarLabel(title: {label}, systemImage: {}).tag({value})",
                        string(icon)
                    )),
                    None => out.line(format!("SwiftUI.Text(verbatim: {label}).tag({value})")),
                }
                if let Some(badge) = element.expr("badge") {
                    out.line(format!(".badge(Int({}))", self.expr(badge)));
                }
            }
            _ => {}
        }
    }

    pub(super) fn navigation(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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

    pub(super) fn tabs(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let selection = self.field(element, "selection").expect("selection field");
        // A `TabAccessory` is built once and shown by the tab view (iOS
        // 26.1's bottom accessory) or, before that and on macOS, at the
        // bottom of every page.
        let accessory = element.children.iter().find_map(|node| match node {
            Node::Element(child) if child.kind == "TabAccessory" => Some(child),
            _ => None,
        });
        if let Some(accessory) = accessory {
            out.open("SwiftUI.Group {");
            out.open("let bernAccessory = SwiftUI.AnyView(SwiftUI.Group {");
            self.nodes(out, &accessory.children, ctx);
            out.close("})");
            out.line(format!(
                "let bernAccessoryShown: Bool = {}",
                accessory
                    .expr("shown")
                    .map(|shown| self.expr(shown))
                    .unwrap_or_else(|| "true".into())
            ));
        }
        out.open(format!(
            "SwiftUI.TabView(selection: {}) {{",
            self.binding_of(&selection)
        ));
        self.nav_walk(out, &element.children, &|out, tab| {
            if tab.kind != "Tab" {
                return;
            }
            let label = self.text_prop(tab, "label").unwrap_or_default();
            let value = tab.expr("value").map(|v| self.expr(v)).unwrap_or_default();
            out.open("SwiftUI.Group {");
            self.nodes(out, &tab.children, ctx);
            out.close("}");
            // A tab's page is a focus scope of its own on tvOS, where the
            // page's `default_focus` takes the focus as it comes down from
            // the tabs.
            if self.platform == Platform::Tvos {
                out.line(".modifier(BernFocusScope())");
            }
            out.open(".tabItem {");
            match tab.icon("icon") {
                // `display: icon`: the tab shows the symbol alone, as tvOS
                // shows its search tab; the label names it. A Mac's tab
                // picker shows a label's title only, and its symbol only for
                // an icon-only label where it stands in the toolbar; an
                // image alone leaves the tab blank (macOS 27).
                Some(icon) if tab.keyword("display") == Some("icon") => {
                    if self.platform == Platform::Macos {
                        out.open("if bernTabShowsIcon(bernWindow) {");
                        out.line(format!(
                            "SwiftUI.Label({label}, systemImage: {}).labelStyle(.iconOnly)",
                            string(icon)
                        ));
                        out.close("} else {");
                        out.indent += 1;
                        out.line(format!(
                            "SwiftUI.Label({label}, systemImage: {})",
                            string(icon)
                        ));
                        out.close("}");
                    } else {
                        out.line(format!(
                            "SwiftUI.Image(systemName: {}).accessibilityLabel(SwiftUI.Text(verbatim: {label}))",
                            string(icon)
                        ));
                    }
                }
                Some(icon) => out.line(format!(
                    "SwiftUI.Label({label}, systemImage: {})",
                    string(icon)
                )),
                None => out.line(format!("SwiftUI.Text(verbatim: {label})")),
            }
            out.close("}");
            out.line(format!(".tag({value})"));
            if accessory.is_some() {
                out.line(".bernTabAccessoryInset(bernAccessoryShown, bernAccessory)");
            }
            self.toolbar(out, &tab.children, ctx);
        });
        out.close("}");
        if accessory.is_some() {
            out.line(".bernTabAccessory(bernAccessoryShown, bernAccessory)");
        }
        // The selected tab is one of the system parts the application's
        // accent colors (design 8.1).
        out.line(".bernAccentTint()");
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
        if accessory.is_some() {
            out.close("}");
        }
    }

    pub(super) fn nav_link(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
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
