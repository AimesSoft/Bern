//! Search fields and menus.

use super::{Generator, Out, some, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    pub(super) fn search(&self, out: &mut Out, element: &Element) {
        let (function, run) = if element.keyword("style") == Some("palette") {
            ("SearchPalette", "activate_target")
        } else {
            ("Search", "focus_target")
        };
        if element.children.is_empty() {
            out.line(format!(
                "b::{function}(p, c, {}, {}, None);",
                self.search_props(element),
                self.action(element),
            ));
        } else {
            self.children(
                out,
                &format!(
                    "b::{function}(p, c, {}, {}, Some(",
                    self.search_props(element),
                    self.action(element),
                ),
                &element.children,
                "));",
            );
        }
        self.shortcut(out, element, &format!("b::{run}(c, {})", self.id(element)));
    }

    pub(super) fn search_item(&self, out: &mut Out, element: &Element) {
        let props = format!(
            "b::SearchItemProps {{ id: {}, label: {}, icon: {}, detail: {}, hint: {}, navigation: {}, pick: {} }}",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            some(self.text_prop(element, "detail")),
            some(self.text_prop(element, "hint")),
            self.value(element, "navigation"),
            self.pick(element),
        );
        if element.children.is_empty() {
            out.line(format!(
                "b::SearchItem(p, c, {props}, {}, None);",
                self.action(element)
            ));
        } else {
            self.children(
                out,
                &format!(
                    "b::SearchItem(p, c, {props}, {}, Some(",
                    self.action(element)
                ),
                &element.children,
                "));",
            );
        }
    }

    pub(super) fn search_props(&self, element: &Element) -> String {
        format!(
            "b::SearchProps {{ id: {}, field: {}, prompt: {}, panel_prompt: {}, width: {}, height: {}, font_size: {}, suggest: {} }}",
            self.id(element),
            some(self.field(element, "value")),
            some(self.text_prop(element, "prompt")),
            some(self.text_prop(element, "panel_prompt")),
            Self::number(element, "width"),
            Self::number(element, "height"),
            Self::number(element, "font_size"),
            element.keyword("suggest") == Some("true"),
        )
    }

    /// A menu button and its items; dividers are `None`.
    pub(super) fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "b::Menu(p, c, b::MenuProps {{ id: {}, label: {}, icon: {}, icon_only: {}, shape: {}, hide_disabled: {} }}, vec![",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
            element.keyword("style") == Some("icon"),
            self.shape(element),
            element.keyword("hide_disabled") == Some("true"),
        ));
        let mut shortcuts = Vec::new();
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("None,");
                continue;
            }
            if item.prop("shortcut").is_some() {
                shortcuts.push(item);
            }
            out.line(format!("{},", self.menu_item(item)));
        }
        out.close("]);");
        for item in shortcuts {
            self.shortcut(
                out,
                item,
                &format!("b::activate_target(c, {})", self.id(item)),
            );
        }
    }

    /// One item of a menu or of a table's row menu.
    pub(super) fn menu_item(&self, item: &Element) -> String {
        format!(
            "Some(b::MenuItem {{ id: {}, label: {}, icon: {}, role: {}, enabled: {}, action: {}, pick: {}, paste: {} }})",
            self.id(item),
            some(self.text_prop(item, "label")),
            Self::icon(item, "icon"),
            Self::keyword(item, "role"),
            self.enabled(item),
            self.action(item),
            self.pick(item),
            some(self.field(item, "paste")),
        )
    }

    /// A `Table`: its columns, each a heading and the cell of each row, its
    /// row menu, and its primary action.
    pub(super) fn table(&self, out: &mut Out, element: &Element) {
        let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        out.open(format!(
            "b::Table(p, c, b::TableProps {{ id: {}, label: {}, selection: {} }}, {}, {field}, {}, vec![",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.field(element, "selection")),
            self.getter(items),
            string(&template.var),
        ));
        let mut menu = Vec::new();
        for node in &element.children {
            let Node::Element(part) = node else { continue };
            match part.kind.as_str() {
                "TableColumn" => {
                    out.open("b::TableColumn {");
                    out.line(format!(
                        "title: {}, width: {}, sorted: {}, descending: {}, sort: {},",
                        some(self.text_prop(part, "title")),
                        Self::number(part, "width"),
                        some(part.expr("sorted").map(|expr| self.getter(expr))),
                        some(part.expr("descending").map(|expr| self.getter(expr))),
                        self.action(part),
                    ));
                    self.children(out, "cell: ", &part.children, ",");
                    out.close("},");
                }
                "TableMenu" => menu.extend(part.children.iter().filter_map(|node| match node {
                    Node::Element(item) => Some(item),
                    _ => None,
                })),
                _ => {}
            }
        }
        out.close("], vec![");
        out.indent += 1;
        for item in &menu {
            if item.kind == "Divider" {
                out.line("None,");
            } else {
                out.line(format!("{},", self.menu_item(item)));
            }
        }
        out.close(format!("], {});", self.action(element)));
        for item in menu.iter().filter(|item| item.prop("shortcut").is_some()) {
            self.shortcut(
                out,
                item,
                &format!("b::activate_target(c, {})", self.id(item)),
            );
        }
    }
}
