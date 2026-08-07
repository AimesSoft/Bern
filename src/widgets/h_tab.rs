//! The `h_tab` control: a horizontal tab bar, ported from the top-left
//! navigation of nipaplay (bold labels, hover zoom, accent-colored capsule
//! indicator under the selected tab).
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "nav", kind: "h_tab", area: "topbar",
//!        props: { "items": "首页:tab_home,视频:tab_video,媒体库:tab_library" })
//! ```
//!
//! `items` is a comma-separated list of `label:key` pairs. Each `key` is an
//! interaction id of that tab — declared in the app's central `ids.rs` and
//! validated by the registry at build time. Pressing a tab emits
//! [`EventKind::Pressed`] with that tab's key as `widget_id`.
//!
//! Appearance follows nipaplay and is fully theme-driven (light/dark palettes
//! live inside this control):
//!
//! - labels are bold, unselected ones at reduced opacity (white 60% on dark,
//!   black 54% on light), selected one in the theme accent color;
//! - hovering zooms the label to `hover_scale` (default 1.1) with a smooth
//!   200 ms ease-out animation;
//! - a rounded capsule (default 3 px tall) sits at the bottom of the selected
//!   label and slides between tabs (300 ms) when the selection changes.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::ui::PressOrigin;
use crate::core::widget::{BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent};
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{Quad, Style};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::window;
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Size, Transformation};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "h_tab";

/// Parses the `items` prop into `(label, key)` pairs.
///
/// Format: `"label:key,label:key"`. Malformed entries are skipped; the
/// registry validates the format before any control is built.
pub fn parse_items(value: &str) -> Vec<(String, String)> {
    value
        .split(',')
        .filter_map(|part| {
            let (label, key) = part.trim().split_once(':')?;
            let (label, key) = (label.trim(), key.trim());
            if label.is_empty() || key.is_empty() {
                return None;
            }
            Some((label.to_string(), key.to_string()))
        })
        .collect()
}

/// The interaction keys declared by an `items` prop.
pub fn item_keys(value: &str) -> Vec<String> {
    parse_items(value).into_iter().map(|(_, key)| key).collect()
}

/// Validates the `items` prop format; returns the item keys on success.
pub fn validate_items(value: &str) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((label, key)) = part.split_once(':') else {
            return Err(format!("item `{part}` must be `label:key`"));
        };
        let (label, key) = (label.trim(), key.trim());
        if label.is_empty() {
            return Err(format!("item `{part}` has an empty label"));
        }
        if key.is_empty() {
            return Err(format!("item `{part}` has an empty key"));
        }
        keys.push(key.to_string());
    }
    if keys.is_empty() {
        return Err("`items` must declare at least one tab".into());
    }
    Ok(keys)
}

/// The control itself (the [`WidgetDef`]).
#[derive(Default)]
pub struct HTab;

impl WidgetDef for HTab {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let items: Vec<(String, String)> = parse_items(node.str_prop("items").unwrap_or(""))
            .into_iter()
            .map(|(label, key)| (label, ctx.qualify(&key)))
            .collect();
        let font_size = node
            .prop("font_size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(20.0);
        let hover_scale = node
            .prop("hover_scale")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.1);
        let hover_duration = node
            .prop("duration_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(200.0)
            / 1000.0;
        let indicator_duration = node
            .prop("indicator_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(300.0)
            / 1000.0;
        let indicator_height = node
            .prop("indicator_height")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(3.0);
        let indicator_radius = node
            .prop("indicator_radius")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(30.0);
        let item_padding = node
            .prop("item_padding")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(8.0);

        let is_dark = ctx.theme.extended_palette().is_dark;
        let accent = ctx.theme.extended_palette().primary.base.color;
        let text_color = ctx.theme.palette().text;
        // nipaplay: white 60% on dark, black 54% on light for unselected.
        let unselected_alpha = if is_dark { 0.6 } else { 0.54 };

        let labels = items
            .iter()
            .map(|(label, _)| iced::widget::text(label.clone()).size(font_size).into())
            .collect();

        HTabView {
            items,
            labels,
            hover_scale,
            hover_duration,
            indicator_duration,
            indicator_height,
            indicator_radius,
            item_padding,
            accent,
            text_color,
            unselected_alpha,
            press_origin: ctx.press_origin.clone(),
        }
        .into()
    }
}

/// The custom tab-bar widget: a row of hover-zooming labels with a sliding
/// capsule indicator. Each tab is an independent interactive target that
/// publishes its own `(id, Pressed)` event.
pub struct HTabView<'a> {
    /// `(label, qualified event id)` for every tab.
    items: Vec<(String, String)>,
    /// The label elements (colors are applied per-item at draw time).
    labels: Vec<Element<'a, LayoutMessage>>,
    hover_scale: f32,
    hover_duration: f32,
    indicator_duration: f32,
    indicator_height: f32,
    indicator_radius: f32,
    item_padding: f32,
    accent: Color,
    text_color: Color,
    unselected_alpha: f32,
    press_origin: PressOrigin,
}

/// Space between the labels and the capsule indicator.
const INDICATOR_GAP: f32 = 12.0;

/// Widget-tree state: hover per item, selected index, and the sliding
/// capsule animation.
#[derive(Default)]
struct State {
    /// The hovered item (if any).
    hovered: Option<usize>,
    /// Per-item hover animation progress (0 = rest, 1 = fully hovered).
    hover_progress: Vec<f32>,
    /// The selected item.
    selected: usize,
    /// Capsule animation: it slides from `from` to `to`.
    from: usize,
    to: usize,
    /// Raw capsule progress 0..1 (before easing).
    progress: f32,
    last: Option<Instant>,
}

impl State {
    fn initial(item_count: usize) -> Self {
        Self {
            hover_progress: vec![0.0; item_count],
            // The capsule rests on the first tab at startup.
            progress: 1.0,
            ..Default::default()
        }
    }
}

/// Ease-out cubic (the same curve the hover/indicator animations use).
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl<'a> Widget<LayoutMessage, iced::Theme, iced::Renderer> for HTabView<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let mut nodes = Vec::with_capacity(self.items.len());
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for i in 0..self.items.len() {
            let node =
                self.labels[i]
                    .as_widget_mut()
                    .layout(&mut tree.children[i], renderer, limits);
            width += node.size().width + self.item_padding * 2.0;
            height = height.max(node.size().height);
            nodes.push(node);
        }
        let total_height = height + INDICATOR_GAP + self.indicator_height;
        let mut x = 0.0;
        for node in &mut nodes {
            node.move_to_mut(Point::new(x + self.item_padding, 0.0));
            x += node.size().width + self.item_padding * 2.0;
        }
        Node::with_children(Size::new(width, total_height), nodes)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::initial(self.items.len())))
    }

    fn children(&self) -> Vec<Tree> {
        self.labels.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.labels);
        let state = tree.state.downcast_mut::<State>();
        state.hover_progress.resize(self.items.len(), 0.0);
        if state.selected >= self.items.len() {
            state.selected = 0;
            state.from = 0;
            state.to = 0;
            state.progress = 1.0;
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        // Each tab's clickable area: its label box padded horizontally and
        // stretched to the full bar height (generous, like nipaplay).
        let item_rects: Vec<Rectangle> = layout
            .children()
            .map(|child| {
                let label = child.bounds();
                Rectangle::new(
                    Point::new(label.x - self.item_padding, bounds.y),
                    Size::new(label.width + self.item_padding * 2.0, bounds.height),
                )
            })
            .collect();

        let hovered = item_rects.iter().position(|rect| cursor.is_over(*rect));
        if hovered != state.hovered {
            state.hovered = hovered;
            shell.request_redraw();
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(index) = hovered {
                    let center = item_rects[index].center();
                    self.press_origin.record((center.x, center.y));
                    if index != state.selected {
                        state.from = state.selected;
                        state.to = index;
                        state.progress = 0.0;
                        state.selected = index;
                    }
                    shell.publish(LayoutMessage::Event(WidgetEvent {
                        widget_id: self.items[index].1.clone(),
                        kind: EventKind::Pressed,
                    }));
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                // 帧间隔：空闲后恢复的第一个 RedrawRequested 与上一个时间
                // 戳可能相隔很久（好几秒），直接把 dt 算进去会一步补完
                // 动画，看起来就是“瞬间放大”。超过正常帧间隔就当作动画
                // 起点（dt=0），下一帧起再按真实帧率推进。
                let dt = match state.last {
                    Some(last) => {
                        let elapsed = now.duration_since(last).as_secs_f32();
                        if elapsed > 0.1 { 0.0 } else { elapsed }
                    }
                    None => 0.0,
                };
                state.last = Some(*now);

                let mut animating = false;
                for i in 0..state.hover_progress.len() {
                    let target = if state.hovered == Some(i) { 1.0 } else { 0.0 };
                    let remaining = target - state.hover_progress[i];
                    if remaining.abs() > 0.0005 {
                        animating = true;
                        let step = dt / self.hover_duration;
                        state.hover_progress[i] = if remaining > 0.0 {
                            (state.hover_progress[i] + step).min(target)
                        } else {
                            (state.hover_progress[i] - step).max(target)
                        };
                    } else {
                        state.hover_progress[i] = target;
                    }
                }

                if state.from != state.to {
                    animating = true;
                    state.progress += dt / self.indicator_duration;
                    if state.progress >= 1.0 {
                        state.progress = 1.0;
                        state.from = state.to;
                    }
                }

                if animating {
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        _style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let child_layouts: Vec<Layout<'_>> = layout.children().collect();

        // The capsule indicator: a rounded pill under the selected label,
        // sliding from the old label to the new one.
        if !child_layouts.is_empty() {
            let eased = ease_out_cubic(state.progress);
            let from = child_layouts[state.from].bounds();
            let to = child_layouts[state.to].bounds();
            let capsule = Rectangle::new(
                Point::new(
                    from.x + (to.x - from.x) * eased,
                    bounds.y + bounds.height - self.indicator_height,
                ),
                Size::new(
                    from.width + (to.width - from.width) * eased,
                    self.indicator_height,
                ),
            );
            renderer.fill_quad(
                Quad {
                    bounds: capsule,
                    border: Border::default().rounded(self.indicator_radius),
                    ..Default::default()
                },
                Background::Color(self.accent),
            );
        }

        for (i, child) in child_layouts.iter().enumerate() {
            let label_bounds = child.bounds();
            let center = label_bounds.center();
            let selected = state.selected == i;
            let hovered = state.hovered == Some(i);
            let scale = 1.0 + (self.hover_scale - 1.0) * ease_out_cubic(state.hover_progress[i]);
            let color = if selected {
                self.accent
            } else if hovered {
                self.text_color
            } else {
                Color::from_rgba(
                    self.text_color.r,
                    self.text_color.g,
                    self.text_color.b,
                    self.unselected_alpha,
                )
            };

            renderer.with_transformation(
                Transformation::translate(center.x, center.y)
                    * Transformation::scale(scale)
                    * Transformation::translate(-center.x, -center.y),
                |renderer| {
                    self.labels[i].as_widget().draw(
                        &tree.children[i],
                        renderer,
                        theme,
                        &Style { text_color: color },
                        *child,
                        cursor,
                        viewport,
                    );
                },
            );
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let over = layout.children().any(|child| {
            let label = child.bounds();
            cursor.is_over(Rectangle::new(
                Point::new(label.x - self.item_padding, bounds.y),
                Size::new(label.width + self.item_padding * 2.0, bounds.height),
            ))
        });
        if over {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> From<HTabView<'a>> for Element<'a, LayoutMessage> {
    fn from(widget: HTabView<'a>) -> Self {
        Element::new(widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_label_key_items() {
        let items = parse_items("首页:tab_home, 视频:tab_video ,媒体库:tab_library");
        assert_eq!(
            items,
            vec![
                ("首页".to_string(), "tab_home".to_string()),
                ("视频".to_string(), "tab_video".to_string()),
                ("媒体库".to_string(), "tab_library".to_string()),
            ]
        );
    }

    #[test]
    fn validates_items() {
        assert!(validate_items("首页:tab_home,视频:tab_video").is_ok());
        assert!(validate_items("").is_err(), "empty items must fail");
        assert!(
            validate_items("首页").is_err(),
            "label without a key must fail"
        );
        assert!(validate_items("首页:").is_err(), "empty key must fail");
    }

    #[test]
    fn ease_out_cubic_bounds() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5, "ease-out is fast at first");
    }
}
