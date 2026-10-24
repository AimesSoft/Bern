//! Search fields and menus.

use super::{Generator, Out, some};
use bern_ir::{Element, Node};

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
