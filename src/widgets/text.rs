//! The `text` control: static text from a layout property.
//!
//! Color comes from the active iced theme's text color. Following the theme
//! reveal is automatic at the engine level (the registry wraps every control
//! in a reveal wrapper).

use crate::core::layout::Widget;
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef};
use iced::Element;

/// The layout name of this control.
pub const NAME: &str = "text";

/// The control itself.
#[derive(Default)]
pub struct Text;

impl WidgetDef for Text {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a, 't>(
        &self,
        node: &'a Widget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let content = node.str_prop("text").unwrap_or("");
        let size = node
            .prop("size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(16.0);

        iced::widget::text(content)
            .size(size)
            .color(ctx.theme.palette().text)
            .into()
    }
}
