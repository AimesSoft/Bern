//! `Table`: a Material data table, headings above the rows of a lazy
//! column, as Android has no table of its own.

use super::{Ctx, Generator, Out, Scope};
use crate::names::{float, member};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    pub(super) fn table(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(items), Some(template), Some(Prop::Key { name, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            return;
        };
        let var = member(&template.var);
        let key = member(name);
        let target = self.target(element, ctx);
        let label = self.text_prop(element, "label").unwrap_or_default();
        let selection = self.field(element, "selection");
        let primary = element.action.as_ref().map(|action| self.action(action));
        let columns = parts(element, "TableColumn");
        let menu: Vec<&Element> = parts(element, "TableMenu")
            .into_iter()
            .flat_map(|menu| {
                menu.children.iter().filter_map(|node| match node {
                    Node::Element(item) => Some(item),
                    _ => None,
                })
            })
            .collect();
        out.open("run {");
        out.line(format!("val tableRows = {}", self.expr(items)));
        out.line(format!("val tableKeys = tableRows.map {{ it.{key} }}"));
        let mut actions = Vec::new();
        if let Some(primary) = &primary {
            actions.push(format!("activate = {{ {primary} }}"));
        }
        if let Some(field) = &selection {
            actions.push(format!(
                "select = {{ text -> val wanted = text.split(\",\").filter {{ it.isNotEmpty() }}; val known = tableKeys.map {{ it.toString() }}; if (wanted.all {{ it in known }}) {{ {field} = tableKeys.filter {{ it.toString() in wanted }}; true }} else false }}"
            ));
            actions.push(format!("value = {{ {field}.joinToString(\",\") }}"));
        }
        out.line(format!(
            "BernControl({target}, \"table\", {label}, actions = BernControlActions({}))",
            actions.join(", ")
        ));
        // The row menu's items are controls too, reached while it is closed.
        for item in menu.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, file) = self.pressed(item);
            out.line(format!(
                "BernControl({}, \"button\", {item_label}, {}, BernControlActions(activate = {{ {action} }}{file}))",
                self.target(item, ctx),
                self.enabled(item)
            ));
            self.shortcut(out, item, &action);
        }
        // It takes the free room of the column or row it is in, as a list
        // does.
        let flexible = match ctx.scope {
            Scope::Column | Scope::Row => "Modifier.weight(1f)",
            Scope::Other | Scope::Flow => "Modifier.bernFlexible().fillMaxHeight()",
        };
        out.open(format!(
            "Column({flexible}.fillMaxWidth().testTag({target})) {{"
        ));
        let (chosen, choose_all) = match &selection {
            Some(field) => (
                format!(
                    "when {{ {field}.isEmpty() -> ToggleableState.Off; tableKeys.all {{ it in {field} }} -> ToggleableState.On; else -> ToggleableState.Indeterminate }}"
                ),
                format!("{{ all -> {field} = if (all) tableKeys else emptyList() }}"),
            ),
            None => ("null".into(), "{ _ -> }".into()),
        };
        out.open(format!("BernTableHeadings({chosen}, {choose_all}) {{"));
        for column in &columns {
            let title = self.text_prop(column, "title").unwrap_or_default();
            let sorted = column
                .expr("sorted")
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "false".into());
            let descending = column
                .expr("descending")
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "false".into());
            let sort = column
                .action
                .as_ref()
                .map(|action| format!("{{ {} }}", self.action(action)))
                .unwrap_or_else(|| "null".into());
            out.line(format!(
                "BernTableHeading({title}, {sorted}, {descending}, {sort}, {})",
                width(column)
            ));
        }
        out.close("}");
        out.line("HorizontalDivider()");
        out.open("LazyColumn(Modifier.fillMaxWidth().weight(1f)) {");
        out.open(format!(
            "items(tableRows, key = {{ {var} -> BernKey.of({var}.{key}) }}) {{ {var} ->"
        ));
        let row_key = format!("{var}.{key}");
        out.open(format!(
            "CompositionLocalProvider(LocalBernItemKeys provides LocalBernItemKeys.current + \"[${{{row_key}}}]\") {{"
        ));
        let (state, toggle, only) = match &selection {
            Some(field) => (
                format!("{row_key} in {field}"),
                format!(
                    "{{ on -> val next = if (on) {field} + {row_key} else {field} - {row_key}; {field} = tableKeys.filter {{ it in next }} }}"
                ),
                format!("{{ {field} = listOf({row_key}) }}"),
            ),
            None => ("null".into(), "{ _ -> }".into(), "{}".into()),
        };
        let primary_arg = primary
            .as_ref()
            .map(|action| format!("{{ {action} }}"))
            .unwrap_or_else(|| "null".into());
        let menu_arg = if menu.is_empty() {
            "null"
        } else {
            "{ dismiss ->"
        };
        out.open(format!(
            "BernTableRow({state}, {toggle}, {only}, {primary_arg}, {menu_arg}"
        ));
        if !menu.is_empty() {
            for item in &menu {
                if item.kind == "Divider" {
                    out.line("HorizontalDivider()");
                    continue;
                }
                let item_label = self.text_prop(item, "label").unwrap_or_default();
                let (action, _) = self.pressed(item);
                out.line(format!(
                    "BernMenuItem({item_label}, {}, {}, destructive = {}) {{ dismiss(); {action} }}",
                    Self::icon(item.icon("icon")),
                    self.enabled(item),
                    item.keyword("role") == Some("destructive")
                ));
            }
            out.close("}) {");
        } else {
            out.indent -= 1;
            out.open(") {");
        }
        let mut inner = ctx.clone();
        inner.key = Some(row_key.clone());
        for column in &columns {
            out.open(format!(
                "Row({}, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {{",
                width(column)
            ));
            self.nodes(out, &column.children, &inner.within(Scope::Row));
            out.close("}");
        }
        out.close("}");
        out.close("}");
        out.line("HorizontalDivider()");
        out.close("}");
        out.close("}");
        out.close("}");
        out.close("}");
    }
}

/// A column's width: its own, or a share of what the others leave.
fn width(column: &Element) -> String {
    match column.number("width") {
        Some(width) => format!("Modifier.width({}.dp)", float(width).trim_end_matches('f')),
        None => "Modifier.weight(1f)".into(),
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
