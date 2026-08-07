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

use crate::core::layout::Widget;
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef};
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

    fn build<'a>(
        &self,
        node: &'a Widget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a>,
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
        match ctx.registry.build_embedded(sub, &child_ctx) {
            Ok(element) => element,
            Err(error) => error_text(format!("layout `{src}`: {error:?}")),
        }
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
