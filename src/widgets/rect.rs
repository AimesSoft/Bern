//! The `rect` control: a plain filled rectangle.
//!
//! Typically used as a page background: place it in a [`AreaKind::Stack`]
//! root with `z: -1` and `size: Fill`, and everything else draws on top.
//! Its fill color follows the active iced theme (light background on light,
//! dark background on dark) — built into this control.

use crate::core::layout::Widget;
use crate::core::widget::{size_lengths, BuildContext, LayoutMessage, WidgetDef};
use iced::widget::container;
use iced::{Background, Border, Element};

/// The layout name of this control.
pub const NAME: &str = "rect";

/// The control itself.
#[derive(Default)]
pub struct Rect;

impl WidgetDef for Rect {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a>(
        &self,
        _node: &'a Widget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a>,
    ) -> Element<'a, LayoutMessage> {
        let background = ctx.theme.palette().background;

        let (width, height) = size_lengths(size);
        let mut rect = iced::widget::container(iced::widget::space());
        if let Some(width) = width {
            rect = rect.width(width);
        }
        if let Some(height) = height {
            rect = rect.height(height);
        }

        rect
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(background)),
                border: Border::default(),
                ..Default::default()
            })
            .into()
    }
}
