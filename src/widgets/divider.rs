//! A theme-aware one-pixel divider for layout composition.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef};
use iced::{Background, Color, Element, Length};

/// The layout name of this control.
pub const NAME: &str = "divider";

/// A horizontal divider that fills the available width.
#[derive(Default)]
pub struct Divider;

impl WidgetDef for Divider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let thickness = float_prop(node, "thickness", 1.0, 0.5, 8.0);
        let opacity = float_prop(node, "opacity", 0.12, 0.0, 1.0);
        let text = ctx.theme.palette().text;
        let color = Color::from_rgba(text.r, text.g, text.b, opacity);

        iced::widget::container(
            iced::widget::Space::new()
                .width(Length::Fill)
                .height(thickness),
        )
        .width(Length::Fill)
        .height(thickness)
        .style(move |_theme| iced::widget::container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
    }
}

fn float_prop(node: &LayoutWidget, key: &str, fallback: f32, min: f32, max: f32) -> f32 {
    node.str_prop(key)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
        .clamp(min, max)
}
