//! The `text_input` control: a search-style text field, ported from
//! nipaplay's media-library search box.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "lib_search", kind: "text_input", area: "root",
//!        props: { "placeholder": "搜索媒体库", "value": "" })
//! ```
//!
//! - the placeholder text comes from the layout file (`placeholder` prop);
//! - typing publishes `(id, TextChanged(text))`; the application stores the
//!   new value back into the layout `value` prop (single source of truth),
//!   like the slider;
//! - appearance follows nipaplay: white 82% / 9% fill, rounded-8 border
//!   (text color 10%, accent 2px while focused), bold text, and a
//!   `search_rounded` Material icon on the left.

use crate::core::layout::Widget;
use crate::core::widget::{
    BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent, size_lengths,
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
        let placeholder = node.str_prop("placeholder").unwrap_or("");
        let value = node.str_prop("value").unwrap_or("");

        // nipaplay 的配色全部写进控件内部：深浅色模式各自取值，不依赖
        // 外部主题文件。
        let is_dark = ctx.theme.extended_palette().is_dark;
        let text = ctx.theme.palette().text;
        let accent = ctx.theme.extended_palette().primary.base.color;
        let with_alpha = |c: Color, a: f32| Color::from_rgba(c.r, c.g, c.b, a);

        let background = if is_dark {
            with_alpha(Color::WHITE, 0.09)
        } else {
            with_alpha(Color::WHITE, 0.82)
        };
        let border_idle = with_alpha(text, 0.10);
        let placeholder_color = with_alpha(text, 0.48);
        let icon_color = with_alpha(text, 0.58);

        // 左侧搜索图标（nipaplay 的 prefixIcon: Icons.search_rounded）。
        let glyph = crate::icons::glyph("search_rounded")
            .expect("search_rounded is in the icon table");

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
            .padding([16, 16])
            .size(16.0)
            .font(crate::fonts::bold_font())
            .icon(text_input::Icon {
                font: crate::icons::font(),
                code_point: glyph,
                size: Some(iced::Pixels(18.0)),
                spacing: 10.0,
                side: text_input::Side::Left,
            })
            .style(
                move |_theme: &iced::Theme, status: text_input::Status| {
                    let focused = matches!(status, text_input::Status::Focused { .. });
                    text_input::Style {
                        background: Background::Color(background),
                        border: Border::default()
                            .rounded(8)
                            .width(if focused { 2.0 } else { 1.0 })
                            .color(if focused { accent } else { border_idle }),
                        icon: icon_color,
                        placeholder: placeholder_color,
                        value: text,
                        selection: accent,
                    }
                },
            )
            .into()
    }
}
