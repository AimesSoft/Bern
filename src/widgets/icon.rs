//! The `icon` control: a Material icon glyph.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "heart", kind: "icon", area: "actions",
//!        props: { "name": "favorite", "size": "18" })
//! ```
//!
//! The `name` prop accepts a Material icon name via the embedded icon
//! package; unknown names are rendered as raw text. The color follows the
//! active iced theme's text color.

use crate::core::layout::Widget;
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef};
use iced::Element;

/// The layout name of this control.
pub const NAME: &str = "icon";

/// The control itself.
#[derive(Default)]
pub struct Icon;

impl WidgetDef for Icon {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a>(
        &self,
        node: &'a Widget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a>,
    ) -> Element<'a, LayoutMessage> {
        let name = node.str_prop("name").unwrap_or("");
        let size = node
            .prop("size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(20.0);

        let content = match crate::icons::glyph(name) {
            Some(glyph) => iced::widget::text(glyph).font(crate::icons::font()),
            None => iced::widget::text(name),
        };

        content
            .size(size)
            .color(ctx.theme.palette().text)
            .into()
    }
}
