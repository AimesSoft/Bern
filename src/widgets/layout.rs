//! The `layout` control: embeds another layout file as a widget.
//!
//! Building blocks live in their own `.ron` files and are pulled in with:
//!
//! ```ron
//! Widget(id: "card", kind: "layout", area: "root", props: { "src": "hello_card" })
//! ```
//!
//! The embedded tree is drawn in place. Every id inside it is qualified with
//! the embedding widget's id (`card.title`), so a block can be reused several
//! times on one page without id collisions. `src` names are resolved through
//! the [`crate::core::store::LayoutStore`]: the device folder first, then
//! `common`.
//!
//! Embedded layouts can also occupy remaining row/column space and align their
//! contents inside it. This is useful for reusable trailing action groups:
//!
//! ```ron
//! Widget(id: "actions", kind: "layout", area: "topbar", size: Weight(1),
//!        props: { "src": "topbar_actions", "align_x": "right" })
//! ```

use crate::core::layout::{SizePolicy, Widget};
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef, size_lengths};
use iced::alignment::{Horizontal, Vertical};
use iced::{Color, Element};

/// The layout name of this control.
pub const NAME: &str = "layout";

/// Maximum embedding depth, as a recursion guard against cyclic references.
const MAX_DEPTH: u32 = 32;

/// The control itself.
#[derive(Default)]
pub struct LayoutRef;

impl WidgetDef for LayoutRef {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a, 't>(
        &self,
        node: &'a Widget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let Some(src) = node.str_prop("src") else {
            return error_text("layout widget: missing `src` prop");
        };
        if ctx.depth() >= MAX_DEPTH {
            return error_text(format!("layout nesting too deep at `{src}`"));
        }
        let Some(sub) = ctx.store.resolve(src) else {
            return error_text(format!("layout `{src}` not found in the store"));
        };

        let prefix = node.str_prop("prefix").unwrap_or(&node.id);
        let child_ctx = ctx.with_prefix(prefix);
        let element = match ctx.registry.build_embedded(sub, &child_ctx) {
            Ok(element) => element,
            Err(error) => error_text(format!("layout `{src}`: {error:?}")),
        };

        if size.is_none()
            && node.str_prop("align_x").is_none()
            && node.str_prop("align_y").is_none()
        {
            return element;
        }

        let (width, height) = size_lengths(size);
        let mut container = iced::widget::container(element);
        if let Some(width) = width {
            container = container.width(width);
        }
        if let Some(height) = height {
            container = container.height(height);
        }
        if let Some(alignment) = horizontal_alignment(node.str_prop("align_x")) {
            container = container.align_x(alignment);
        }
        if let Some(alignment) = vertical_alignment(node.str_prop("align_y")) {
            container = container.align_y(alignment);
        }
        container.into()
    }
}

fn horizontal_alignment(value: Option<&str>) -> Option<Horizontal> {
    match value {
        Some("left" | "start") => Some(Horizontal::Left),
        Some("center") => Some(Horizontal::Center),
        Some("right" | "end") => Some(Horizontal::Right),
        _ => None,
    }
}

fn vertical_alignment(value: Option<&str>) -> Option<Vertical> {
    match value {
        Some("top" | "start") => Some(Vertical::Top),
        Some("center") => Some(Vertical::Center),
        Some("bottom" | "end") => Some(Vertical::Bottom),
        _ => None,
    }
}

/// Renders a visible error instead of failing the whole build. A real app
/// might replace this with a fallback widget or a logged diagnostic.
fn error_text(message: impl Into<String>) -> Element<'static, LayoutMessage> {
    iced::widget::text(message.into())
        .size(12)
        .color(Color::from_rgb(1.0, 0.45, 0.45))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_embedded_layout_alignment() {
        assert!(matches!(
            horizontal_alignment(Some("right")),
            Some(Horizontal::Right)
        ));
        assert!(matches!(
            horizontal_alignment(Some("end")),
            Some(Horizontal::Right)
        ));
        assert!(matches!(
            vertical_alignment(Some("center")),
            Some(Vertical::Center)
        ));
        assert!(horizontal_alignment(Some("unknown")).is_none());
    }
}
