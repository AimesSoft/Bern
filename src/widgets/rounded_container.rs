//! A theme-aware rounded rectangular container that embeds a RON layout.
//!
//! It is intended for grouped settings and similar surfaces:
//!
//! ```ron
//! Widget(id: "playback_card", kind: "rounded_container", area: "root",
//!        props: { "src": "playback_settings", "radius": "8", "padding": "0" })
//! ```

use crate::core::layout::{SizePolicy, Widget as LayoutWidget};
use crate::core::widget::{BuildContext, BuildError, LayoutMessage, WidgetDef, size_lengths};
use iced::{Background, Border, Color, Element};

/// The layout name of this control.
pub const NAME: &str = "rounded_container";

const MAX_DEPTH: u32 = 32;

/// A rounded surface which owns an embedded layout.
#[derive(Default)]
pub struct RoundedContainer;

impl WidgetDef for RoundedContainer {
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
            .expect("rounded_container src was validated before build");
        if ctx.depth() >= MAX_DEPTH {
            return error_text(format!("rounded_container nesting too deep at `{src}`"));
        }
        let child: Element<'a, LayoutMessage> = match ctx.store.resolve(src) {
            Some(layout) => {
                let child_ctx = ctx.with_prefix(&node.id);
                match ctx.registry.build_embedded(layout, &child_ctx) {
                    Ok(element) => element,
                    Err(error) => error_text(format!(
                        "rounded_container `{}`: layout `{src}`: {error:?}",
                        node.id
                    )),
                }
            }
            None => error_text(format!(
                "rounded_container `{}`: layout `{src}` not found",
                node.id
            )),
        };

        let radius = float_prop(node, "radius", 8.0, 0.0, 64.0);
        let padding = float_prop(node, "padding", 0.0, 0.0, 128.0);
        let background_opacity = float_prop(node, "background_opacity", 0.30, 0.0, 1.0);
        let border_opacity = float_prop(node, "border_opacity", 0.20, 0.0, 1.0);
        let text = ctx.theme.palette().text;
        let surface = if ctx.theme.extended_palette().is_dark {
            Color::BLACK
        } else {
            Color::WHITE
        };
        let background = Color::from_rgba(surface.r, surface.g, surface.b, background_opacity);
        let border_color = Color::from_rgba(text.r, text.g, text.b, border_opacity);
        let (width, height) = size_lengths(size);

        let mut container = iced::widget::container(child)
            .padding(padding)
            .style(move |_theme| iced::widget::container::Style {
                background: Some(Background::Color(background)),
                border: Border::default()
                    .rounded(radius)
                    .width(0.5)
                    .color(border_color),
                ..Default::default()
            });
        if let Some(width) = width {
            container = container.width(width);
        }
        if let Some(height) = height {
            container = container.height(height);
        }
        container.into()
    }
}

fn float_prop(node: &LayoutWidget, key: &str, fallback: f32, min: f32, max: f32) -> f32 {
    node.prop(key)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn error_text(message: impl Into<String>) -> Element<'static, LayoutMessage> {
    iced::widget::text(message.into())
        .size(12)
        .color(Color::from_rgb(1.0, 0.45, 0.45))
        .into()
}
