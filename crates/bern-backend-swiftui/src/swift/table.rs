//! `Table`: the system table on macOS and iOS, a list of row buttons on
//! tvOS, which has none.

use super::values::swift_type;
use super::{Ctx, Generator, Out, Platform};
use crate::names::{member, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    pub(super) fn table(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(items), Some(template), Some(Prop::Key { field, name })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        let Type::Record { name: record } = &template.ty else {
            return;
        };
        let key_type = self
            .types()
            .record_def(record)
            .and_then(|def| def.fields.get(*field))
            .map(|field| swift_type(&field.ty))
            .unwrap_or_else(|| "String".into());
        let row_type = swift_type(&template.ty);
        let var = member(&template.var);
        let key = member(name);
        let items = self.expr(items);
        let selection = self.field(element, "selection");
        let columns: Vec<&Element> = parts(element, "TableColumn");
        let menu: Vec<&Element> = parts(element, "TableMenu")
            .into_iter()
            .flat_map(|menu| {
                menu.children.iter().filter_map(|node| match node {
                    Node::Element(item) => Some(item),
                    _ => None,
                })
            })
            .collect();
        let label = self.text_prop(element, "label").unwrap_or_default();
        let primary = element.action.as_ref().map(|action| self.action(action));
        let keys = format!("({items}).map(\\.{key})");
        // Each action first makes the rows it acts on the selection.
        let choose = |ids: &str| match &selection {
            Some(field) => format!("{field} = bernChosen({ids}, in: {keys}); "),
            None => String::new(),
        };

        if self.platform == Platform::Tvos {
            out.open("SwiftUI.List {");
            out.open(format!(
                "SwiftUI.ForEach({items}, id: \\.{key}) {{ {var} in"
            ));
            let only = format!("[{var}.{key}]");
            let set = match &selection {
                Some(field) => format!("{field} = {only}; "),
                None => String::new(),
            };
            out.open(format!(
                "SwiftUI.Button(action: {{ {set}{} }}) {{",
                primary.clone().unwrap_or_default()
            ));
            out.open("SwiftUI.HStack(spacing: 24) {");
            for column in &columns {
                self.nodes(out, &column.children, ctx);
            }
            out.close("}");
            out.close("}");
            if !menu.is_empty() {
                out.open(".contextMenu {");
                self.table_menu(out, &menu, &set);
                out.close("}");
            }
            out.line(format!(".bernItemKey({var}.{key})"));
            out.close("}");
            out.close("}");
        } else {
            let order = format!("BernColumnOrder<BernTableRow<{row_type}, {key_type}>>");
            let sortable = columns
                .iter()
                .any(|column| column.action.is_some() || column.expr("sorted").is_some());
            let mut head = format!("SwiftUI.Table(of: BernTableRow<{row_type}, {key_type}>.self");
            if let Some(field) = &selection {
                head.push_str(&format!(
                    ", selection: SwiftUI.Binding(get: {{ Set({field}) }}, set: {{ {field} = bernChosen($0, in: {keys}) }})"
                ));
            }
            if sortable {
                // The order shows the column state says the rows are sorted
                // by; a click on a heading runs that column's action.
                let mut get = String::from("{ ");
                let mut set = String::from("{ order in switch order.first?.column { ");
                for (index, column) in columns.iter().enumerate() {
                    if let Some(sorted) = column.expr("sorted") {
                        let down = column
                            .expr("descending")
                            .map(|expr| self.expr(expr))
                            .unwrap_or_else(|| "false".into());
                        get.push_str(&format!(
                            "if {} {{ return [{order}(column: {index}, order: ({down}) ? .reverse : .forward)] }}; ",
                            self.expr(sorted)
                        ));
                    }
                    if let Some(action) = &column.action {
                        set.push_str(&format!("case {index}?: {}; ", self.action(action)));
                    }
                }
                get.push_str("return [] }");
                set.push_str("default: break } }");
                head.push_str(&format!(
                    ", sortOrder: SwiftUI.Binding(get: {get}, set: {set})"
                ));
            }
            out.open(format!("{head}) {{"));
            for (index, column) in columns.iter().enumerate() {
                let title = self.text_prop(column, "title").unwrap_or_default();
                let sorts = column.action.is_some() || column.expr("sorted").is_some();
                let using = if sorts {
                    format!(", sortUsing: {order}(column: {index})")
                } else {
                    String::new()
                };
                out.open(format!(
                    "SwiftUI.TableColumn(SwiftUI.Text(verbatim: {title}){using}) {{ bernRow in"
                ));
                out.line(format!("let {var} = bernRow.value"));
                out.open("SwiftUI.Group {");
                self.nodes(out, &column.children, ctx);
                out.close("}");
                out.line(".bernItemKey(bernRow.id)");
                out.close("}");
                if let Some(width) = column.number("width") {
                    out.line(format!(
                        ".width(min: 40, ideal: {})",
                        crate::names::double(width)
                    ));
                }
            }
            out.close("} rows: {");
            out.indent += 1;
            out.open(format!(
                "SwiftUI.ForEach(({items}).map {{ BernTableRow(id: $0.{key}, value: $0) }}) {{ bernRow in"
            ));
            out.line("SwiftUI.TableRow(bernRow)");
            out.close("}");
            out.close("}");
            let primary_line = primary
                .clone()
                .map(|action| format!(", primaryAction: {{ ids in {}{action} }}", choose("ids")))
                .unwrap_or_default();
            if !menu.is_empty() || primary.is_some() {
                out.open(format!(
                    ".contextMenu(forSelectionType: {key_type}.self, menu: {{ ids in"
                ));
                if !menu.is_empty() {
                    out.open("if !ids.isEmpty {");
                    self.table_menu(out, &menu, &choose("ids"));
                    out.close("}");
                }
                out.close(format!("}}{primary_line})"));
            }
        }
        let activate = primary
            .map(|action| format!("activate: {{ {action} }}, "))
            .unwrap_or_default();
        let select = match &selection {
            Some(field) => {
                let parse = if key_type == "Int64" {
                    "Int64($0)"
                } else {
                    "String($0)"
                };
                format!(
                    "select: {{ text in let wanted = text.split(separator: \",\").compactMap {{ {parse} }}; let known = {keys}; guard wanted.allSatisfy(known.contains) else {{ return false }}; {field} = bernChosen(Set(wanted), in: known); return true }}, value: {{ {field}.map {{ String($0) }}.joined(separator: \",\") }}"
                )
            }
            None => String::new(),
        };
        out.line(format!(
            ".bernControl({}, role: \"table\", label: {label}, actions: BernControlActions({activate}{select}))",
            self.target(element, ctx)
        ));
        // The row menu's items are controls too, reached while it is closed.
        for item in menu.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, file) = self.pressed(item);
            let enabled = item
                .expr("enabled")
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "true".into());
            out.line(format!(
                ".bernControl({}, role: \"button\", label: {item_label}, enabled: {enabled}, actions: BernControlActions(activate: {{ {action} }}{file}))",
                self.target(item, ctx)
            ));
        }
    }

    /// The buttons of a row menu; `choose` makes the rows it opened over
    /// the selection first.
    fn table_menu(&self, out: &mut Out, items: &[&Element], choose: &str) {
        for item in items {
            if item.kind == "Divider" {
                out.line("SwiftUI.Divider()");
                continue;
            }
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, _) = self.pressed(item);
            let role = if item.keyword("role") == Some("destructive") {
                "role: .destructive, "
            } else {
                ""
            };
            out.open(format!(
                "SwiftUI.Button({role}action: {{ {choose}{action} }}) {{"
            ));
            match item.icon("icon") {
                Some(icon) => out.line(format!(
                    "SwiftUI.Label({item_label}, systemImage: {})",
                    string(icon)
                )),
                None => out.line(format!("SwiftUI.Text(verbatim: {item_label})")),
            }
            out.close("}");
            if let Some(enabled) = item.expr("enabled") {
                out.line(format!(".disabled(!({}))", self.expr(enabled)));
            }
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
