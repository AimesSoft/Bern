//! `Table`: headings above the rows of the system list.

use super::{BUILDER, Generator, Out, designated, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    pub(super) fn table(&self, out: &mut Out, element: &Element) {
        let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        let props = designated(&[
            ("id", Some(self.id(element))),
            ("label", self.text_prop(element, "label")),
            ("items", Some(self.getter(items))),
            ("key", Some(field.to_string())),
            ("variable", Some(string(&template.var))),
            ("selection", self.field(element, "selection")),
            ("primary", self.action(element)),
        ]);
        out.open(format!(
            "B::Table(p, c, {props}, std::vector<B::TableColumnDef>{{"
        ));
        for column in parts(element, "TableColumn") {
            let fields = designated(&[
                ("title", self.text_prop(column, "title")),
                ("width", Self::number(column, "width")),
                (
                    "sorted",
                    column.expr("sorted").map(|expr| self.getter(expr)),
                ),
                (
                    "descending",
                    column.expr("descending").map(|expr| self.getter(expr)),
                ),
                ("sort", self.action(column)),
            ]);
            // The cell builder is the last of the column's fields.
            let head = fields.strip_suffix('}').unwrap_or(&fields);
            let comma = if head.len() > 1 { ", " } else { "" };
            out.open(format!("B::TableColumnDef{head}{comma}.cell = {BUILDER}"));
            self.nodes(out, &column.children);
            out.close("}},");
        }
        out.close("},");
        out.indent += 1;
        out.open("std::vector<std::optional<B::MenuItemDef>>{");
        let mut shortcuts = Vec::new();
        for menu in parts(element, "TableMenu") {
            for node in &menu.children {
                let Node::Element(item) = node else { continue };
                if item.kind == "Divider" {
                    out.line("std::nullopt,");
                    continue;
                }
                if item.prop("shortcut").is_some() {
                    shortcuts.push(item);
                }
                out.line(format!("B::MenuItemDef{},", self.menu_item_def(item)));
            }
        }
        out.close("});");
        out.indent -= 1;
        for item in shortcuts {
            self.shortcut(out, item, "activate");
        }
    }
}

/// The children of `element` of `kind`.
fn parts<'a>(element: &'a Element, kind: &str) -> Vec<&'a Element> {
    element
        .children
        .iter()
        .filter_map(|node| match node {
            Node::Element(part) if part.kind == kind => Some(part),
            _ => None,
        })
        .collect()
}
