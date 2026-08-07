//! The `button` control: a labeled button that emits [`EventKind::Pressed`].
//!
//! Colors come from the active iced theme's palette, so the light/dark
//! appearance is built into this control — no external theme file needed.

use crate::core::layout::Widget;
use crate::core::widget::{
    BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent, size_lengths,
};
use iced::widget::button;
use iced::{Background, Border, Element};

/// The layout name of this control.
pub const NAME: &str = "button";

/// The control itself. Stateless at build time; events carry the widget id.
#[derive(Default)]
pub struct Button;

impl WidgetDef for Button {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn build<'a, 't>(
        &self,
        node: &'a Widget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let id = ctx.qualify(&node.id);
        let label = node.str_prop("label").unwrap_or("");

        let primary = &ctx.theme.extended_palette().primary;
        let background = primary.base.color;
        let hover = primary.strong.color;
        let pressed = primary.weak.color;
        let text_color = primary.base.text;
        let radius = 6.0;

        let (width, _height) = size_lengths(size);
        let mut button = iced::widget::button(iced::widget::text(label));
        if let Some(width) = width {
            button = button.width(width);
        }

        button
            .on_press(LayoutMessage::Event(WidgetEvent {
                widget_id: id,
                kind: EventKind::Pressed,
            }))
            .style(move |_theme: &iced::Theme, status: button::Status| {
                let color = match status {
                    button::Status::Active => background,
                    button::Status::Hovered => hover,
                    button::Status::Pressed => pressed,
                    button::Status::Disabled => background,
                };
                button::Style {
                    background: Some(Background::Color(color)),
                    text_color,
                    border: Border::default().rounded(radius),
                    ..Default::default()
                }
            })
            .into()
    }
}
