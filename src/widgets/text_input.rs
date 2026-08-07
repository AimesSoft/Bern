//! The `text_input` control: editable text that emits
//! [`EventKind::TextChanged`] events.
//!
//! Colors come from the active iced theme's palette, built into this control.

use crate::core::layout::Widget;
use crate::core::widget::{
    size_lengths, BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent,
};
use iced::widget::text_input;
use iced::{Background, Border, Color, Element};

/// The layout name of this control.
pub const NAME: &str = "text_input";

/// The control itself.
#[derive(Default)]
pub struct TextInput;

impl WidgetDef for TextInput {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a>(
        &self,
        node: &'a Widget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a>,
    ) -> Element<'a, LayoutMessage> {
        let id = ctx.qualify(&node.id);
        let placeholder = node.str_prop("placeholder").unwrap_or("");
        let value = node.str_prop("value").unwrap_or("");

        let theme = ctx.theme;
        let text_color = theme.palette().text;
        let background = theme.palette().background;
        let border_color = Color {
            r: text_color.r,
            g: text_color.g,
            b: text_color.b,
            a: 0.35,
        };
        let placeholder_color = Color {
            r: text_color.r,
            g: text_color.g,
            b: text_color.b,
            a: 0.5,
        };
        let radius = 6.0;

        let (width, _height) = size_lengths(size);
        let mut input = iced::widget::text_input(placeholder, value);
        if let Some(width) = width {
            input = input.width(width);
        }

        input
            .on_input(move |input| {
                LayoutMessage::Event(WidgetEvent {
                    widget_id: id.clone(),
                    kind: EventKind::TextChanged(input),
                })
            })
            .padding(8u16)
            .style(move |_theme: &iced::Theme, _status: text_input::Status| {
                text_input::Style {
                    background: Background::Color(background),
                    border: Border::default().rounded(radius).color(border_color),
                    icon: text_color,
                    placeholder: placeholder_color,
                    value: text_color,
                    selection: border_color,
                }
            })
            .into()
    }
}
