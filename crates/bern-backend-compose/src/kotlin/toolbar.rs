//! Toolbars, side panels, menus and search.

use super::{Ctx, Generator, Out, Scope};
use crate::names::{double, float, string};
use bern_ir::{Element, Node};

impl Generator<'_> {
    pub(super) fn toolbar(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        // Items before a `Spacer` stand at the leading end of the
        // top bar, after its way back; the rest at the trailing end.
        let split = element
            .children
            .iter()
            .position(|node| matches!(node, Node::Element(child) if child.kind == "Spacer"));
        let (leading, trailing) = match split {
            Some(index) => (&element.children[..index], &element.children[index + 1..]),
            None => (&element.children[..0], &element.children[..]),
        };
        out.open("BernChrome(");
        if !leading.is_empty() {
            out.open("leading = {");
            self.nav_walk(out, leading, &|out, item| self.toolbar_item(out, item, ctx));
            out.close("},");
        }
        out.open("actions = {");
        self.nav_walk(out, trailing, &|out, item| {
            self.toolbar_item(out, item, ctx)
        });
        out.close("},");
        out.close(")");
    }

    /// `Search(style: palette)`: a button opening the search over the
    /// window, its results below the field; a `SearchItem` chosen closes
    /// it.
    pub(super) fn search_palette(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let Some(field) = self.field(element, "value") else {
            return;
        };
        let prompt = self
            .text_prop(element, "prompt")
            .unwrap_or_else(|| "\"\"".into());
        let panel = self
            .text_prop(element, "panel_prompt")
            .unwrap_or_else(|| prompt.clone());
        let submit = element
            .action
            .as_ref()
            .map(|a| format!("{{ {} }}", self.action(a)))
            .unwrap_or_else(|| "null".into());
        let size = |name, default: f64| double(element.number(name).unwrap_or(default));
        let target = self.target(element, ctx);
        out.open(format!(
            "BernSearchPalette({target}, {field}, {{ {field} = it }}, {prompt}, {panel}, ({}).dp, ({}).dp, {submit}) {{",
            size("width", 640.0),
            size("height", 480.0)
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Column));
        out.close("}");
        if let Some(shortcut) = element.keyword("shortcut") {
            out.line(format!(
                "BernShortcut({}) {{ BernSearchPalettes.open({target}) }}",
                string(shortcut)
            ));
        }
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
            "Picker" => self.picker_in(out, item, ctx, true),
            "Search" if item.keyword("style") == Some("palette") => {
                self.search_palette(out, item, ctx)
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
        // `style: icon` shows only the icon, a round button with `height`.
        let icon_only = if element.keyword("style") == Some("icon") {
            ", iconOnly = true"
        } else {
            ""
        };
        out.open(format!(
            "BernMenu({label}, {}, Modifier.testTag({}){icon_only}{}) {{ dismiss ->",
            Self::icon(element.icon("icon")),
            self.target(element, ctx),
            self.shape_arg(element)
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
        // The field's own size and text size, standing in the page.
        let mut extra = String::new();
        if let Some(width) = element.number("width") {
            extra.push_str(&format!(
                ", width = {}.dp",
                float(width).trim_end_matches('f')
            ));
        }
        if let Some(height) = element.number("height") {
            extra.push_str(&format!(
                ", height = {}.dp",
                float(height).trim_end_matches('f')
            ));
        }
        if let Some(size) = element.number("font_size") {
            extra.push_str(&format!(", fontSize = {}", float(size)));
        }
        out.line(format!(
            "BernSearchField({field}, {{ {field} = it }}, {prompt}, {}, Modifier.testTag({target}){extra})",
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
