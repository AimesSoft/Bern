//! Navigation: split views and their sidebars, navigation stacks and
//! their links, adaptive layouts and tabs.

use super::{BUILDER, Generator, Out, designated, string};
use bern_ir::{Element, Expr, ExprKind, IfLetNode, IfNode, MatchNode, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// The navigation view: its sidebar's items may come and go with `if`,
    /// `match` and `ForEach`, each registering while it is mounted.
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
        let mut footer = None;
        let mut content = Vec::new();
        for node in &sidebar.children {
            match node {
                Node::Element(child) if child.kind == "Search" => search = Some(child),
                Node::Element(child) if child.kind == "SidebarFooter" => footer = Some(child),
                other => content.push(other.clone()),
            }
        }
        let search = search
            .filter(|search| search.field("value").is_some())
            .map(|search| format!("B::SearchProps{}", self.search_props(search)));
        out.open(format!(
            "B::SplitView(p, c, {},",
            designated(&[
                ("id", sidebar.id.as_ref().map(|_| self.id(sidebar))),
                ("selection", Some(selection)),
                (
                    "fallback",
                    sidebar.expr("fallback").map(|value| self.getter(value))
                ),
                ("search", search),
            ])
        ));
        match footer {
            Some(footer) => self.children(out, "", &footer.children, ","),
            None => out.line("nullptr,"),
        }
        self.in_sidebar.set(true);
        self.children(out, "", &content, ",");
        self.in_sidebar.set(false);
        self.children(out, "", &detail.children, ");");
        out.indent -= 1;
    }

    pub(super) fn sidebar_item(&self, out: &mut Out, element: &Element) {
        let Some(value) = element.expr("value") else {
            return;
        };
        out.line(format!(
            "B::SidebarItem(p, c, {});",
            designated(&[
                ("value", Some(self.getter(value))),
                ("name", Some(self.variant_name(value))),
                ("label", self.text_prop(element, "label")),
                ("icon", Self::icon(element, "icon")),
                (
                    "style",
                    self.style("SidebarItem", element.keyword("variant"))
                ),
            ])
        ));
    }

    /// Name of an enum variant value, for selections: the literal's, or
    /// looked up when the value is computed.
    fn variant_name(&self, expr: &Expr) -> String {
        match (&expr.kind, &expr.ty) {
            (ExprKind::Variant { variant, .. }, Type::Enum { name }) => self
                .types()
                .enum_def(name)
                .map(|def| string(&def.variants[*variant as usize].name))
                .unwrap_or_else(|| "\"\"".into()),
            (_, Type::Enum { name }) => format!(
                "[](Ctx const& c) {{ return R::VariantName({}, {}); }}",
                self.expr(expr),
                string(name)
            ),
            _ => "\"\"".into(),
        }
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
            "B::Navigation(p, c, {}, {path},",
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

    /// A `NavLink`: a row that pushes a route, named by the text it shows
    /// first.
    pub(super) fn nav_link(&self, out: &mut Out, element: &Element) {
        let route = element
            .expr("route")
            .map(|route| self.getter(route))
            .unwrap_or_else(|| "[](Ctx const&) { return Value::Unit(); }".into());
        let label = self
            .row_label(&element.children)
            .unwrap_or_else(|| "winrt::hstring()".into());
        self.call(
            out,
            &format!(
                "B::NavLink(p, c, {}, {route}, [](Ctx const& c) {{ return {label}; }}, ",
                self.id(element)
            ),
            &element.children,
        );
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
                            Some(variant) => format!("if (tag == {variant}u) return {label}; "),
                            None => format!("return {label}; "),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "[&]() -> winrt::hstring {{ auto tag = R::Tag({}); {}return winrt::hstring(); }}()",
                    self.expr(&node.value),
                    arms.concat()
                ))
            }
            Node::Component(_) | Node::If(_) | Node::IfLet(_) => None,
        })
    }

    /// One builder per layout size; the runtime picks the size by the width
    /// available and the breakpoints of `[layouts]`, and shows the layout of
    /// a size without one of its own as the next larger, else smaller.
    pub(super) fn adaptive(&self, out: &mut Out, element: &Element) {
        let layout = |size: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size) => Some(layout),
                _ => None,
            })
        };
        out.open(format!(
            "B::Adaptive(p, c, {}, std::array<Build, 4>{{",
            self.optional_id(element)
        ));
        for size in ["compact", "medium", "expanded", "large"] {
            match layout(size) {
                Some(layout) => self.children(out, "", &layout.children, ","),
                None => out.line("nullptr,"),
            }
        }
        out.close("});");
    }

    /// Tabs whose items may come and go: each `Tab` registers while it is
    /// mounted.
    pub(super) fn tabs(&self, out: &mut Out, element: &Element) {
        let Some(selection) = self.field(element, "selection") else {
            return;
        };
        out.open(format!(
            "B::Tabs(p, c, {}, {BUILDER}",
            designated(&[
                ("id", element.id.as_ref().map(|_| self.id(element))),
                ("selection", Some(selection)),
                (
                    "fallback",
                    element.expr("fallback").map(|value| self.getter(value))
                ),
            ])
        ));
        self.tab_items(out, &element.children);
        out.close("});");
    }

    fn tab_items(&self, out: &mut Out, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element(tab) if tab.kind == "Tab" => self.tab(out, tab),
                Node::Element(other) if other.kind == "ForEach" => self.tab_each(out, other),
                Node::If(branch) => self.tab_when(out, branch),
                Node::IfLet(branch) => self.tab_if_let(out, branch),
                Node::Match(branch) => self.tab_match(out, branch),
                _ => {}
            }
        }
    }

    /// A `Tab`: its spec, and its content as the builder.
    fn tab(&self, out: &mut Out, tab: &Element) {
        let Some(value) = tab.expr("value") else {
            return;
        };
        let head = designated(&[
            ("value", Some(self.getter(value))),
            ("name", Some(self.variant_name(value))),
            ("label", self.text_prop(tab, "label")),
            ("icon", Self::icon(tab, "icon")),
        ]);
        // The builder is the last field: leave the braces open.
        let head = &head[..head.len() - 1];
        let separator = if head.ends_with('{') { "" } else { ", " };
        self.children(
            out,
            &format!("B::Tab(p, c, B::TabSpec{head}{separator}.build = "),
            &tab.children,
            "});",
        );
    }

    /// Tabs repeated with `ForEach`.
    fn tab_each(&self, out: &mut Out, each: &Element) {
        if let (Some(items), Some(template), Some(Prop::Key { field, .. })) =
            (each.expr("items"), &each.template, each.prop("key"))
        {
            out.open(format!(
                "B::Each(p, c, {}, {BUILDER}",
                designated(&[
                    ("items", Some(self.getter(items))),
                    ("key", Some(field.to_string())),
                    ("variable", Some(string(&template.var))),
                ])
            ));
            self.tab_items(out, &each.children);
            out.close("});");
        }
    }

    /// Tabs shown under `if`.
    fn tab_when(&self, out: &mut Out, branch: &IfNode) {
        let mut cases = String::from("[](Ctx const& c) -> std::optional<int> { ");
        for (index, (condition, _)) in branch.branches.iter().enumerate() {
            cases.push_str(&format!(
                "if ({}) return {index}; ",
                self.condition(condition)
            ));
        }
        if branch.otherwise.is_empty() {
            cases.push_str("return std::nullopt; }");
        } else {
            cases.push_str(&format!("return {}; }}", branch.branches.len()));
        }
        out.open(format!("B::When(p, c, {cases}, std::vector<Build>{{"));
        for body in branch
            .branches
            .iter()
            .map(|(_, body)| body)
            .chain((!branch.otherwise.is_empty()).then_some(&branch.otherwise))
        {
            out.open(BUILDER);
            self.tab_items(out, body);
            out.close("},");
        }
        out.close("});");
    }

    /// Tabs shown under `if let`.
    fn tab_if_let(&self, out: &mut Out, branch: &IfLetNode) {
        out.open(format!(
            "B::IfLet(p, c, {}, {}, {BUILDER}",
            string(&branch.var),
            self.getter(&branch.value)
        ));
        self.tab_items(out, &branch.then);
        if branch.otherwise.is_empty() {
            out.close("});");
        } else {
            out.close(format!("}}, {BUILDER}"));
            out.indent += 1;
            self.tab_items(out, &branch.otherwise);
            out.close("});");
        }
    }

    /// Tabs shown by the arms of a `match`.
    fn tab_match(&self, out: &mut Out, branch: &MatchNode) {
        out.open(format!(
            "B::Match(p, c, {}, std::vector<B::MatchArm>{{",
            self.getter(&branch.value)
        ));
        for arm in &branch.arms {
            let bindings: Vec<String> = arm
                .bindings
                .iter()
                .map(|binding| format!("{{{}, {}}}", string(&binding.name), binding.field))
                .collect();
            out.open(format!(
                "B::Arm({}, {BUILDER}",
                arm.variant
                    .map(|variant| format!("{variant}u"))
                    .unwrap_or_else(|| "std::nullopt".into())
            ));
            self.tab_items(out, &arm.body);
            if bindings.is_empty() {
                out.close("}),");
            } else {
                out.close(format!("}}, {{{}}}),", bindings.join(", ")));
            }
        }
        out.close("});");
    }
}
