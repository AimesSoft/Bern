//! A scrollable container whose content is assembled by another RON layout.

use crate::core::layout::{SizePolicy, Widget as LayoutWidget};
use crate::core::widget::{BuildContext, BuildError, LayoutMessage, WidgetDef, size_lengths};
use iced::{Color, Element, Length};

/// The layout name of this control.
pub const NAME: &str = "scroll_layout";

const MAX_DEPTH: u32 = 32;

/// A generic scroll viewport around an embedded layout.
#[derive(Default)]
pub struct ScrollLayout;

impl WidgetDef for ScrollLayout {
    fn name(&self) -> &'static str {
        NAME
    }

    fn validate(&self, node: &LayoutWidget) -> Result<(), BuildError> {
        if node
            .str_prop("src")
            .is_some_and(|src| !src.trim().is_empty())
        {
            Ok(())
        } else {
            Err(BuildError::MissingProp {
                widget: node.id.clone(),
                prop: "src".into(),
            })
        }
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let src = node
            .str_prop("src")
            .expect("scroll_layout src was validated before build");
        if ctx.depth() >= MAX_DEPTH {
            return error_text(format!("scroll_layout nesting too deep at `{src}`"));
        }
        let content = match ctx.store.resolve(src) {
            Some(layout) => {
                let child_ctx = ctx.with_prefix(&node.id);
                match ctx.registry.build_embedded(layout, &child_ctx) {
                    Ok(element) => element,
                    Err(error) => return error_text(format!("scroll_layout `{src}`: {error:?}")),
                }
            }
            None => return error_text(format!("scroll_layout `{src}` not found")),
        };

        let (width, height) = size_lengths(size);
        let scroll_theme = ctx.theme.clone();
        iced::widget::scrollable(content)
            .width(width.unwrap_or(Length::Fill))
            .height(height.unwrap_or(Length::Fill))
            .style(move |_theme, status| iced::widget::scrollable::default(&scroll_theme, status))
            .into()
    }
}

fn error_text(message: impl Into<String>) -> Element<'static, LayoutMessage> {
    iced::widget::text(message.into())
        .size(12)
        .color(Color::from_rgb(1.0, 0.45, 0.45))
        .into()
}
