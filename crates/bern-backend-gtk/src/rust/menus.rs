//! Search fields and menus.

use super::{Generator, Out, some};
use bern_ir::{Element, Node};

impl Generator<'_> {
    pub(super) fn search(&self, out: &mut Out, element: &Element) {
        if element.children.is_empty() {
            out.line(format!(
                "b::Search(p, c, {}, {}, None);",
                self.search_props(element),
                self.action(element),
            ));
        } else {
            self.children(
                out,
                &format!(
                    "b::Search(p, c, {}, {}, Some(",
                    self.search_props(element),
                    self.action(element),
                ),
                &element.children,
                "));",
            );
        }
        self.shortcut(
            out,
            element,
            &format!("b::focus_target(c, {})", self.id(element)),
        );
    }

    pub(super) fn search_props(&self, element: &Element) -> String {
        format!(
            "b::SearchProps {{ id: {}, field: {}, prompt: {} }}",
            self.id(element),
            some(self.field(element, "value")),
            some(self.text_prop(element, "prompt")),
        )
    }

    /// A menu button and its items; dividers are `None`.
    pub(super) fn menu(&self, out: &mut Out, element: &Element) {
        out.open(format!(
            "b::Menu(p, c, b::MenuProps {{ id: {}, label: {}, icon: {} }}, vec![",
            self.id(element),
            some(self.text_prop(element, "label")),
            Self::icon(element, "icon"),
        ));
        let mut shortcuts = Vec::new();
        for node in &element.children {
            let Node::Element(item) = node else { continue };
            if item.kind == "Divider" {
                out.line("None,");
                continue;
            }
            if item.keyword("shortcut").is_some() {
                shortcuts.push(item);
            }
            out.line(format!(
                "Some(b::MenuItem {{ id: {}, label: {}, icon: {}, role: {}, enabled: {}, action: {}, pick: {} }}),",
                self.id(item),
                some(self.text_prop(item, "label")),
                Self::icon(item, "icon"),
                Self::keyword(item, "role"),
                self.enabled(item),
                self.action(item),
                self.pick(item),
            ));
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
}
