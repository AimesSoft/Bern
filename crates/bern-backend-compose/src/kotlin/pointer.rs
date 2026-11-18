//! Pointer events of elements: double taps, context menus and wheel turns.

use super::{Ctx, Generator, Out, Scope};
use bern_ir::Element;

impl Generator<'_> {
    /// The Kotlin of the action of the pointer event `kind` an element
    /// declares.
    pub(super) fn event_action(&self, element: &Element, kind: &str) -> Option<String> {
        let action = element.pointer_event(kind)?.action.as_ref()?;
        Some(self.action(action))
    }

    /// `.bernWheel(…)` for the wheel events an element declares. A watch
    /// has no wheel; its crown scrolls instead.
    pub(super) fn wheel_modifier(&self, element: &Element) -> String {
        let up = self.event_action(element, "WheelUp");
        let down = self.event_action(element, "WheelDown");
        if self.watch || (up.is_none() && down.is_none()) {
            return String::new();
        }
        let lambda =
            |action: Option<String>| action.map_or_else(|| "null".into(), |a| format!("{{ {a} }}"));
        format!(".bernWheel({}, {})", lambda(up), lambda(down))
    }

    /// The share of the row or column it stands in that an element takes
    /// there, which a box around it takes in its place.
    fn flex(&self, element: &Element, ctx: &Ctx) -> &'static str {
        match element.kind.as_str() {
            "Column" | "Row" => self.line_flex(element, ctx),
            "Stack" if self.stack_weighted(element, ctx) => ".weight(1f)",
            "Slider" => Self::slider_flex(ctx),
            _ => "",
        }
    }

    /// An element other than a button with pointer events, inside
    /// `BernPointerEvents`, which adds no layout: it takes the element's
    /// share of the row or column, and the element its constraints. The
    /// events are not content, so the element is realized without them.
    pub(super) fn with_pointer_events(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let content = Element {
            children: element.content().cloned().collect(),
            ..element.clone()
        };
        let mut args = vec![format!("Modifier{}", self.flex(&content, ctx))];
        if let Some(action) = self.event_action(element, "DoubleTap") {
            args.push(format!("onDoubleTap = {{ {action} }}"));
        }
        if !self.watch {
            for (kind, name) in [("WheelUp", "onWheelUp"), ("WheelDown", "onWheelDown")] {
                if let Some(action) = self.event_action(element, kind) {
                    args.push(format!("{name} = {{ {action} }}"));
                }
            }
        }
        match element.context_menu() {
            Some(items) => {
                self.menu_controls(out, &items, ctx);
                out.open(format!(
                    "BernPointerEvents({}, menu = {{ dismiss ->",
                    args.join(", ")
                ));
                self.menu_items(out, &items);
                out.close("}) {");
                out.indent += 1;
            }
            None => out.open(format!("BernPointerEvents({}) {{", args.join(", "))),
        }
        // The box takes the element's share of the row or column, so
        // inside it the element is out of that scope: a weight there
        // has no receiver.
        self.element_view(out, &content, &ctx.within(Scope::Other));
        out.close("}");
    }
}
