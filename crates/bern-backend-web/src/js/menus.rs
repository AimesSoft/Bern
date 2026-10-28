//! Search fields and their items, menus and toolbars.

use super::{Generator, Out, local, number, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    pub(super) fn search(&self, out: &mut Out, element: &Element) {
        let palette = element.keyword("style") == Some("palette");
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("field", self.field(element, "value")),
            ("prompt", self.text_prop(element, "prompt")),
            ("panelPrompt", self.text_prop(element, "panel_prompt")),
            ("width", element.number("width").map(number)),
            ("height", element.number("height").map(number)),
            ("fontSize", element.number("font_size").map(number)),
            // The palette's field shows the key that opens it.
            (
                "shortcut",
                palette
                    .then(|| self.shortcut_keys(element, "shortcut"))
                    .flatten(),
            ),
            (
                "suggest",
                (element.keyword("suggest") == Some("true")).then(|| "true".into()),
            ),
        ]);
        let submit = self.action_fn(element);
        let function = if palette {
            "B.SearchPalette"
        } else {
            "B.Search"
        };
        if element.children.is_empty() {
            out.line(format!("{function}(p, ctx, {props}, {submit});"));
        } else {
            self.children(
                out,
                &format!("{function}(p, ctx, {props}, {submit}, "),
                &element.children,
                ");",
            );
        }
        self.shortcut(
            out,
            element,
            &format!(
                "() => F.{}(ctx, {})",
                if palette {
                    "activateTarget"
                } else {
                    "focusTarget"
                },
                self.id(element)
            ),
        );
    }

    pub(super) fn search_item(&self, out: &mut Out, element: &Element) {
        let (action, set_file, kinds) = self.file_actions(element);
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("icon", element.icon("icon").map(string)),
            ("detail", self.text_prop(element, "detail")),
            ("hint", self.text_prop(element, "hint")),
            ("navigation", self.value_prop(element, "navigation")),
            ("setFile", set_file),
            ("fileKinds", kinds),
        ]);
        if element.children.is_empty() {
            out.line(format!("B.SearchItem(p, ctx, {props}, {action});"));
        } else {
            self.children(
                out,
                &format!("B.SearchItem(p, ctx, {props}, {action}, "),
                &element.children,
                ");",
            );
        }
    }

    /// A menu button and its items; dividers are `null`.
    pub(super) fn menu(&self, out: &mut Out, element: &Element) {
        let mut shortcuts = Vec::new();
        out.open(format!(
            "B.Menu(p, ctx, {}, [",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "hideDisabled",
                    (element.keyword("hide_disabled") == Some("true")).then(|| "true".into()),
                ),
                ("icon", element.icon("icon").map(string)),
                ("style", Self::keyword(element, "style")),
                ("shape", Self::keyword(element, "shape")),
                ("height", self.length(element, "height")),
            ])
        ));
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("null,");
                continue;
            }
            let enabled = item
                .expr("enabled")
                .map(|expr| format!("() => {}", self.expr(expr)));
            let (action, set_file, kinds) = self.file_actions(item);
            if item.prop("shortcut").is_some() {
                shortcuts.push((item, action.clone()));
            }
            out.line(format!(
                "{},",
                Self::props(vec![
                    ("id", Some(self.id(item))),
                    ("label", self.text_prop(item, "label")),
                    ("icon", item.icon("icon").map(string)),
                    ("role", Self::keyword(item, "role")),
                    ("enabled", enabled),
                    ("action", Some(action)),
                    ("setFile", set_file),
                    ("fileKinds", kinds),
                ])
            ));
        }
        out.close("]);");
        for (item, action) in shortcuts {
            self.shortcut(out, item, &action);
        }
    }

    /// Toolbar children before the spacer occupy the leading title-bar
    /// slot; children after it occupy the ordinary trailing slot.
    pub(super) fn toolbar(&self, out: &mut Out, element: &Element) {
        if let Some(index) = element
            .children
            .iter()
            .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"))
        {
            self.children(
                out,
                "B.tools(ctx, ",
                &element.children[..index],
                ", { placement: \"leading\" });",
            );
            self.children(out, "B.tools(ctx, ", &element.children[index + 1..], ");");
        } else {
            self.children(out, "B.tools(ctx, ", &element.children, ");");
        }
    }

    /// A `Table`: its columns, each a heading and a cell builder taking
    /// the row, its row menu, and its primary action.
    pub(super) fn table(&self, out: &mut Out, element: &Element) {
        let (Some(items), Some(template), Some(Prop::Key { name, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        out.open(format!(
            "B.Table(p, ctx, {}, [",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                ("selection", self.field(element, "selection")),
                ("items", Some(format!("() => {}", self.expr(items)))),
                ("key", Some(format!("(v) => v.{name}"))),
            ])
        ));
        let mut menu = Vec::new();
        for node in &element.children {
            let Node::Element(part) = node else { continue };
            match part.kind.as_str() {
                "TableColumn" => {
                    out.open("{");
                    out.line(format!(
                        "...{},",
                        Self::props(vec![
                            ("title", self.text_prop(part, "title")),
                            ("width", part.number("width").map(number)),
                            ("sorted", self.value_prop(part, "sorted")),
                            ("descending", self.value_prop(part, "descending")),
                            ("sort", part.action.as_ref().map(|_| self.action_fn(part))),
                        ])
                    ));
                    out.open(format!("cell: (p, ctx, {}) => {{", local(&template.var)));
                    self.nodes(out, &part.children);
                    out.close("},");
                    out.close("},");
                }
                "TableMenu" => menu.extend(part.children.iter().filter_map(|node| match node {
                    Node::Element(item) => Some(item),
                    _ => None,
                })),
                _ => {}
            }
        }
        out.close("], [");
        out.indent += 1;
        let mut shortcuts = Vec::new();
        for item in menu {
            if item.kind == "Divider" {
                out.line("null,");
                continue;
            }
            let enabled = item
                .expr("enabled")
                .map(|expr| format!("() => {}", self.expr(expr)));
            let (action, set_file, kinds) = self.file_actions(item);
            if item.prop("shortcut").is_some() {
                shortcuts.push((item, action.clone()));
            }
            out.line(format!(
                "{},",
                Self::props(vec![
                    ("id", Some(self.id(item))),
                    ("label", self.text_prop(item, "label")),
                    ("icon", item.icon("icon").map(string)),
                    ("role", Self::keyword(item, "role")),
                    ("enabled", enabled),
                    ("action", Some(action)),
                    ("setFile", set_file),
                    ("fileKinds", kinds),
                ])
            ));
        }
        out.close(format!(
            "], {});",
            element
                .action
                .as_ref()
                .map(|_| self.action_fn(element))
                .unwrap_or_else(|| "null".into())
        ));
        for (item, action) in shortcuts {
            self.shortcut(out, item, &action);
        }
    }
}
