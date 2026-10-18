//! Toolbars, side panels, menus and search.

use super::values::kotlin_type;
use super::{Ctx, Generator, Out, Scope};
use bern_ir::{Element, Node};
use bern_protocol::Type;

impl Generator<'_> {
    pub(super) fn toolbar(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open("BernChrome(actions = {");
        self.nav_walk(out, &element.children, &|out, item| {
            self.toolbar_item(out, item, ctx)
        });
        out.close("})");
    }

    fn toolbar_item(&self, out: &mut Out, item: &Element, ctx: &Ctx) {
        match item.kind.as_str() {
            "PopoverButton" => self.popover_button(out, item, ctx, true),
            // A top app bar parts its groups of actions with space.
            "Divider" => out.line("Spacer(Modifier.size(12.dp))"),
            "Button" => {
                let label = self.text_prop(item, "label").unwrap_or_default();
                let (action, file) = self.pressed(item);
                let enabled = self.enabled(item);
                let target = self.target(item, ctx);
                out.line(format!(
                    "BernControl({target}, \"button\", {label}, {enabled}, BernControlActions(activate = {{ {action} }}{file}))"
                ));
                self.shortcut(out, item, &action);
                match item.icon("icon") {
                    Some(icon) => out.line(format!(
                        "BernIconButton({label}, {}, {enabled}, Modifier.testTag({target})) {{ {action} }}",
                        Self::icon(Some(icon))
                    )),
                    None => out.line(format!(
                        "TextButton(onClick = {{ {action} }}, enabled = {enabled}, modifier = Modifier.testTag({target})) {{ Text({label}) }}"
                    )),
                }
            }
            "Picker" => {
                let field_ref = item.field("value").expect("picker field");
                let field = self.field(item, "value").expect("picker field");
                let label = self.text_prop(item, "label").unwrap_or_default();
                let enum_type = kotlin_type(&field_ref.ty);
                let labeled = match &field_ref.ty {
                    Type::Enum { name } => self.types().enum_def(name).is_some_and(|def| {
                        def.variants.iter().all(|variant| variant.label.is_some())
                    }),
                    _ => false,
                };
                let option_label = if labeled {
                    "it.bernLabel"
                } else {
                    "it.bernName"
                };
                let target = self.target(item, ctx);
                out.line(format!(
                    "BernControl({target}, \"picker\", {label}, actions = BernControlActions(select = {{ name -> {enum_type}.fromBernName(name)?.let {{ {field} = it; true }} ?: false }}, value = {{ {field}.bernName }}))"
                ));
                out.line(format!(
                    "BernToolbarPicker({label}, {enum_type}.entries, {field}, {{ {option_label} }}, {{ {field} = it }}, Modifier.testTag({target}))"
                ));
            }
            "Search" => {
                let field = self.field(item, "value").expect("search field");
                let prompt = self
                    .text_prop(item, "prompt")
                    .unwrap_or_else(|| "\"\"".into());
                let submit = item
                    .action
                    .as_ref()
                    .map(|a| format!("{{ {} }}", self.action(a)))
                    .unwrap_or_else(|| "null".into());
                let target = self.target(item, ctx);
                out.open(format!(
                    "BernToolbarSearch({target}, {field}, {{ {field} = it }}, {prompt}, {submit}) {{"
                ));
                self.nodes(out, &item.children, &ctx.within(Scope::Column));
                out.close("}");
            }
            _ => self.element(out, item, ctx),
        }
    }

    pub(super) fn menu(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        out.line(format!(
            "BernControl({}, \"menu\", {label}, actions = BernControlActions())",
            self.target(element, ctx)
        ));
        let items: Vec<&Element> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(item) => Some(item),
                _ => None,
            })
            .collect();
        for item in items.iter().filter(|item| item.kind == "MenuItem") {
            let item_label = self.text_prop(item, "label").unwrap_or_default();
            let (action, file) = self.pressed(item);
            out.line(format!(
                "BernControl({}, \"button\", {item_label}, {}, BernControlActions(activate = {{ {action} }}{file}))",
                self.target(item, ctx),
                self.enabled(item)
            ));
            self.shortcut(out, item, &action);
        }
        out.open(format!(
            "BernMenu({label}, {}, Modifier.testTag({})) {{ dismiss ->",
            Self::icon(element.icon("icon")),
            self.target(element, ctx)
        ));
        for item in &items {
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
        out.close("}");
    }

    pub(super) fn inspector(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "presented").expect("bool field");
        out.open(format!(
            "BernInspector({field}, onDismiss = {{ {field} = false }}) {{"
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
    }

    pub(super) fn search(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let field = self.field(element, "value").expect("search field");
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "\"\"".into());
        let action = element.action.as_ref().map(|a| self.action(a));
        let target = self.target(element, ctx);
        let submit = action
            .as_ref()
            .map(|action| format!(", submit = {{ {action} }}"))
            .unwrap_or_default();
        out.line(format!(
            "BernControl({target}, \"search\", {prompt}, actions = BernControlActions(edit = {{ {field} = it }}{submit}, value = {{ {field} }}))"
        ));
        out.line(format!(
            "BernSearchField({field}, {{ {field} = it }}, {prompt}, {}, Modifier.testTag({target}))",
            action
                .map(|action| format!("{{ {action} }}"))
                .unwrap_or_else(|| "null".into())
        ));
        // Results show below the field while there is a search.
        if !element.children.is_empty() {
            out.open(format!("if ({field}.isNotEmpty()) {{"));
            out.open("Column(Modifier.fillMaxWidth()) {");
            self.nodes(out, &element.children, &ctx.within(Scope::Column));
            out.close("}");
            out.close("}");
        }
    }
}
