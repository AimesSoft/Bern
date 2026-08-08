//! The `action_button` control: icon + label with **no container** —
//! just the icon and the text, hovered by scaling the whole content
//! (same animation core as `icon_button`).
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "lib_sort_btn", kind: "action_button", area: "root",
//!        props: { "icon": "sort_by_alpha_rounded", "label": "排序" })
//! ```
//!
//! Appearance follows nipaplay's action buttons: a 21 px icon + an 8 px
//! gap + a 15 px w800 label (dark: white / light: black87). Hovering scales
//! the content around its center (`scale` prop, default 1.08, 140 ms
//! ease-out-cubic); pressing records the press origin and publishes
//! `(id, Pressed)`.
//!
//! While hovered, the icon and label are repainted with the theme accent
//! color (and restored on leave).
//!
//! The `icon` prop accepts a Material icon name via the embedded icon
//! package (with the vector morph foundation); unknown names render as raw
//! text glyphs.

use crate::core::layout::Widget;
use crate::core::widget::{BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent};
use crate::widgets::icon_button::{IconButtonWidget, Visual};
use crate::widgets::morph_icon::MorphIconView;
use iced::Element;
use std::sync::Arc;

/// The layout name of this control.
pub const NAME: &str = "action_button";

/// The control itself.
#[derive(Default)]
pub struct ActionButton;

impl WidgetDef for ActionButton {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn build<'a, 't>(
        &self,
        node: &'a Widget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let label = node.str_prop("label").unwrap_or("");
        let icon_name = node.str_prop("icon").unwrap_or("");
        let icon_size = node
            .prop("icon_size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(21.0);
        let font_size = node
            .prop("font_size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(15.0);
        let morph_duration = node
            .prop("morph_duration_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(420.0)
            / 1000.0;
        let id = ctx.qualify(&node.id);

        let visual = Visual::resolve_with(node, ctx.theme, 1.08, 140.0);
        // 悬浮时重建内容并换成主题强调色（图标 + 文字一起变）。
        let rebuild: Arc<dyn Fn(iced::Color) -> Element<'a, LayoutMessage>> =
            Arc::new(move |color| {
                // 图标 + 文字（nipaplay：21px 图标 + 8px 间距 + 15px 粗体标签）。
                let mut children: Vec<Element<'_, LayoutMessage>> = Vec::new();
                match crate::icons::glyph(icon_name) {
                    Some(glyph) => children.push(
                        MorphIconView::new(glyph, color, icon_size, morph_duration).into(),
                    ),
                    None if !icon_name.is_empty() => children.push(
                        iced::widget::text(icon_name)
                            .size(icon_size)
                            .color(color)
                            .into(),
                    ),
                    None => {}
                }
                children.push(
                    iced::widget::text(label)
                        .size(font_size)
                        .font(crate::fonts::bold_font())
                        .color(color)
                        .into(),
                );
                iced::widget::Row::with_children(children).spacing(8).into()
            });
        let content = rebuild(visual.icon_color);

        IconButtonWidget::new(content, rebuild, visual, ctx.press_origin.clone())
            .padding(6.0)
            .on_press(LayoutMessage::Event(WidgetEvent {
                widget_id: id,
                kind: EventKind::Pressed,
            }))
            .into()
    }
}
