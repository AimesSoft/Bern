//! The `split_pane` control: a Windows-like workspace split into panes.
//!
//! It can divide a page from left to right (`horizontal`) or top to bottom
//! (`vertical`). Every boundary is drawn as a thin, theme-aware line.
//!
//! The control can either host layouts directly:
//!
//! ```ron
//! Widget(id: "workspace", kind: "split_pane", area: "root", size: Fill,
//!        props: {
//!            "direction": "horizontal",
//!            "panes": "navigation,editor,inspector",
//!            "weights": "1,4,2",
//!        })
//! ```
//!
//! or act only as a divided page background by omitting `panes` and setting
//! `sections`. A pane layout may contain another `split_pane`, allowing a
//! workspace to be divided along both axes.

use crate::core::layout::{SizePolicy, Widget as LayoutWidget};
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef, size_lengths};
use crate::widgets::rect::Rect;
use iced::widget::{Column, Row, Space, Stack, container};
use iced::{Background, Border, Color, Element, Length, Padding};

/// The layout name of this control.
pub const NAME: &str = "split_pane";

const DEFAULT_SECTIONS: usize = 2;
const MAX_SECTIONS: usize = 64;
const DEFAULT_DIVIDER_WIDTH: f32 = 1.0;
const DEFAULT_DIVIDER_INSET: f32 = 12.0;

/// The direction in which panes are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Panes run from left to right; boundaries are vertical lines.
    Horizontal,
    /// Panes run from top to bottom; boundaries are horizontal lines.
    Vertical,
}

impl Direction {
    fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("vertical" | "column" | "top_bottom" | "top-bottom" | "上下") => Self::Vertical,
            // Unknown values deliberately fall back to the most common
            // desktop arrangement: left-to-right panes.
            _ => Self::Horizontal,
        }
    }
}

/// A static split-pane workspace.
#[derive(Default)]
pub struct SplitPane;

impl WidgetDef for SplitPane {
    fn name(&self) -> &'static str {
        NAME
    }

    /// This control owns a real `rect` background, which is the body of the
    /// circular theme reveal. Embedded controls still receive their normal
    /// per-control reveal wrappers.
    fn follows_theme_reveal(&self) -> bool {
        false
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let direction =
            Direction::parse(node.str_prop("direction").or_else(|| node.str_prop("axis")));
        let pane_sources = parse_panes(node.str_prop("panes"));
        let pane_count = if pane_sources.is_empty() {
            node.prop("sections")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(DEFAULT_SECTIONS)
                .clamp(2, MAX_SECTIONS)
        } else {
            pane_sources.len().min(MAX_SECTIONS)
        };
        let portions = parse_portions(node.str_prop("weights"), pane_count);
        let divider_width = node
            .prop("divider_width")
            .or_else(|| node.prop("line_width"))
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(DEFAULT_DIVIDER_WIDTH)
            .clamp(0.0, 16.0);
        let divider_inset = node
            .prop("divider_inset")
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(DEFAULT_DIVIDER_INSET)
            .clamp(0.0, 256.0);

        let palette = ctx.theme.palette();
        let divider_alpha = if ctx.theme.extended_palette().is_dark {
            0.18
        } else {
            0.12
        };
        let divider_color = with_alpha(palette.text, divider_alpha);
        let (requested_width, requested_height) = size_lengths(size);
        let width = requested_width.unwrap_or(Length::Fill);
        let height = requested_height.unwrap_or(Length::Fill);

        let mut children = Vec::with_capacity(pane_count.saturating_mul(2).saturating_sub(1));
        for (index, portion) in portions.iter().copied().enumerate() {
            let content: Element<'a, LayoutMessage> = match pane_sources.get(index) {
                Some(src) => match ctx.store.resolve(src) {
                    Some(layout) => {
                        let pane_ctx = ctx.with_prefix(&format!("{}.pane{index}", node.id));
                        match ctx.registry.build_embedded(layout, &pane_ctx) {
                            Ok(element) => element,
                            Err(error) => error_text(
                                format!("split_pane `{}`: layout `{src}`: {error:?}", node.id),
                                ctx.theme.extended_palette().danger.base.color,
                            ),
                        }
                    }
                    None => error_text(
                        format!("split_pane `{}`: layout `{src}` not found", node.id),
                        ctx.theme.extended_palette().danger.base.color,
                    ),
                },
                None => Space::new().width(Length::Fill).height(Length::Fill).into(),
            };

            let pane = match direction {
                Direction::Horizontal => container(content)
                    .width(Length::FillPortion(portion))
                    .height(Length::Fill),
                Direction::Vertical => container(content)
                    .width(Length::Fill)
                    .height(Length::FillPortion(portion)),
            };
            children.push(pane.into());

            if index + 1 < pane_count && divider_width > 0.0 {
                children.push(divider(
                    direction,
                    divider_width,
                    divider_inset,
                    divider_color,
                ));
            }
        }

        let panes: Element<'a, LayoutMessage> = match direction {
            Direction::Horizontal => Row::with_children(children)
                .width(width)
                .height(height)
                .into(),
            Direction::Vertical => Column::with_children(children)
                .width(width)
                .height(height)
                .into(),
        };

        // Reuse the standard page background so split workspaces keep the
        // same light/dark circular reveal behavior as `rect`.
        let background = Rect.build(node, size, ctx);
        Stack::with_children([background, panes])
            .width(width)
            .height(height)
            .into()
    }
}

fn divider<'a>(
    direction: Direction,
    width: f32,
    inset: f32,
    color: Color,
) -> Element<'a, LayoutMessage> {
    let line = container(
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_theme| {
            iced::widget::container::Style {
                background: Some(Background::Color(color)),
                border: Border::default().rounded(width / 2.0),
                ..Default::default()
            }
        }),
    );

    match direction {
        Direction::Horizontal => line
            .width(width)
            .height(Length::Fill)
            .padding(Padding {
                top: inset,
                right: 0.0,
                bottom: inset,
                left: 0.0,
            })
            .into(),
        Direction::Vertical => line
            .width(Length::Fill)
            .height(width)
            .padding(Padding {
                top: 0.0,
                right: inset,
                bottom: 0.0,
                left: inset,
            })
            .into(),
    }
}

fn parse_panes(value: Option<&str>) -> Vec<&str> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(MAX_SECTIONS)
        .collect()
}

/// Converts arbitrary positive weights to stable `FillPortion` values.
/// Invalid or mismatched lists fall back to equal panes.
fn parse_portions(value: Option<&str>, count: usize) -> Vec<u16> {
    let Some(value) = value else {
        return vec![1; count];
    };
    let weights: Vec<f32> = value
        .split(',')
        .filter_map(|part| {
            let weight = part.trim().parse::<f32>().ok()?;
            (weight.is_finite() && weight > 0.0).then_some(weight)
        })
        .collect();
    if weights.len() != count {
        return vec![1; count];
    }

    let total: f32 = weights.iter().sum();
    weights
        .into_iter()
        .map(|weight| ((weight / total) * 1000.0).round().clamp(1.0, 1000.0) as u16)
        .collect()
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

fn error_text(message: String, color: Color) -> Element<'static, LayoutMessage> {
    container(iced::widget::text(message).size(12).color(color))
        .padding(12)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::Layout;
    use crate::core::store::LayoutStore;
    use crate::core::theme::ThemeRouter;

    #[test]
    fn parses_both_split_directions() {
        assert_eq!(Direction::parse(Some("horizontal")), Direction::Horizontal);
        assert_eq!(Direction::parse(Some("left_right")), Direction::Horizontal);
        assert_eq!(Direction::parse(Some("vertical")), Direction::Vertical);
        assert_eq!(Direction::parse(Some("top_bottom")), Direction::Vertical);
        assert_eq!(Direction::parse(Some("上下")), Direction::Vertical);
    }

    #[test]
    fn converts_weights_to_fill_portions() {
        assert_eq!(parse_portions(Some("1,2,1"), 3), vec![250, 500, 250]);
        assert_eq!(parse_portions(Some("1,2"), 3), vec![1, 1, 1]);
        assert_eq!(parse_portions(Some("1,nope,1"), 3), vec![1, 1, 1]);
    }

    #[test]
    fn parses_and_trims_pane_layout_names() {
        assert_eq!(
            parse_panes(Some(" navigation, editor , inspector ")),
            vec!["navigation", "editor", "inspector"]
        );
        assert!(parse_panes(None).is_empty());
    }

    #[test]
    fn builds_embedded_pane_layouts_from_ron() {
        let pane = Layout::parse(
            r#"
            Layout(
                areas: [Area(id: "root", kind: Column)],
                widgets: [
                    Widget(id: "label", kind: "text", area: "root",
                           props: { "text": "pane" }),
                ],
            )
            "#,
        )
        .expect("pane layout parses");
        let workspace = Layout::parse(
            r#"
            Layout(
                areas: [Area(id: "root", kind: Stack)],
                widgets: [
                    Widget(id: "workspace", kind: "split_pane", area: "root", size: Fill,
                           props: {
                               "direction": "horizontal",
                               "panes": "left,right",
                               "weights": "1,3",
                           }),
                ],
            )
            "#,
        )
        .expect("workspace layout parses");

        let mut store = LayoutStore::new();
        store.insert("left", pane.clone());
        store.insert("right", pane);
        let registry = crate::builtin_registry();
        let router = ThemeRouter::new(iced::Theme::Light);

        let _element = registry
            .build(&workspace, &router, &store)
            .expect("split pane builds embedded layouts");
    }
}
