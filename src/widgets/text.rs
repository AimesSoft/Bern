//! The `text` control: static text from a layout property.
//!
//! Color comes from the active iced theme's text color. Optional `bold` and
//! `opacity` properties let layouts express secondary section headings.
//! Following the theme reveal is automatic at the engine level.

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

        let opacity = node
            .str_prop("opacity")
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let base = ctx.theme.palette().text;
        let color = iced::Color::from_rgba(base.r, base.g, base.b, opacity);
        let text = iced::widget::text(content).size(size).color(color);
        if matches!(node.str_prop("bold"), Some("true" | "1" | "yes" | "on")) {
            text.font(crate::fonts::bold_font()).into()
        } else {
            text.into()
        }
    }
}
