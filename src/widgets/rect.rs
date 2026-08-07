//! The `rect` control: the page background.
//!
//! Typically used as a page background: place it in a [`AreaKind::Stack`]
//! root with `z: -1` and `size: Fill`, and everything else draws on top.
//! Its fill color follows the active iced theme (light background on light,
//! dark background on dark) — built into this control.
//!
//! When the color changes because of an interactive press (e.g. a theme
//! toggle button), the new color reveals with a circular wipe that starts at
//! the pressed widget's position and grows until it covers the control — the
//! same effect nipaplay uses for its light/dark switch. If the change is not
//! press-driven, it switches instantly.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::ui::ThemeReveal;
use crate::core::widget::{size_lengths, BuildContext, LayoutMessage, WidgetDef};
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{Quad, Style};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{mouse, Clipboard, Renderer, Shell, Widget};
use iced::event::Event;
use iced::window;
use iced::{Border, Color, Element, Length, Rectangle, Size};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "rect";

/// Default reveal duration, matching nipaplay (420 ms).
const DEFAULT_DURATION_MS: f32 = 420.0;

/// The control itself (the [`WidgetDef`]).
#[derive(Default)]
pub struct Rect;

impl WidgetDef for Rect {
    fn name(&self) -> &'static str {
        NAME
    }

    /// The background is the animation body itself and does not follow the
    /// reveal wrapper.
    fn follows_theme_reveal(&self) -> bool {
        false
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let color = ctx.theme.palette().background;
        // During a two-phase reveal the theme has not switched yet: reveal
        // the *target* palette from the coordinator, keeping the current
        // color as the base.
        let (next_color, origin) = if ctx.theme_reveal.is_active() {
            (
                ctx.theme_reveal
                    .target()
                    .map(|target| target.palette().background)
                    .unwrap_or(color),
                ctx.theme_reveal.origin(),
            )
        } else {
            (color, None)
        };
        let (width, height) = size_lengths(size);
        let duration = node
            .prop("duration_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(DEFAULT_DURATION_MS)
            / 1000.0;

        BackgroundWidget {
            color: next_color,
            origin,
            width: width.unwrap_or(Length::Shrink),
            height: height.unwrap_or(Length::Shrink),
            duration,
            reveal: ctx.theme_reveal.clone(),
        }
        .into()
    }
}

/// The custom background widget behind `rect`.
pub struct BackgroundWidget {
    color: Color,
    /// Where the color-changing press happened (window coordinates).
    origin: Option<(f32, f32)>,
    width: Length,
    height: Length,
    /// Reveal duration in seconds.
    duration: f32,
    /// The theme-reveal notification hub.
    reveal: ThemeReveal,
}

/// Animation state stored in the widget tree.
#[derive(Debug, Clone, Copy)]
struct State {
    /// The color currently covering the whole background.
    shown: Color,
    /// The color being revealed.
    next: Color,
    origin: (f32, f32),
    progress: f32,
    last: Option<Instant>,
    animating: bool,
    /// Reverse direction: the old color contracts inward from the edges
    /// (e.g. switching back to light), instead of the new color expanding
    /// outward from the origin.
    reverse: bool,
    /// Whether the "all buttons covered" completion was already published.
    done_published: bool,
    initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            shown: Color::TRANSPARENT,
            next: Color::TRANSPARENT,
            origin: (0.0, 0.0),
            progress: 1.0,
            last: None,
            animating: false,
            reverse: false,
            done_published: false,
            initialized: false,
        }
    }
}

impl Widget<LayoutMessage, iced::Theme, iced::Renderer> for BackgroundWidget {
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Node {
        Node::new(limits.resolve(self.width, self.height, Size::ZERO))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        if state.animating && state.progress < 1.0 {
            let max_radius = max_corner_distance(bounds, state.origin);
            let eased = ease_out_cubic(state.progress);

            if state.reverse {
                // New color everywhere; the old color contracts inward from
                // the edges toward the origin (switching back to light).
                renderer.fill_quad(
                    Quad {
                        bounds,
                        border: Border::default(),
                        ..Default::default()
                    },
                    state.next,
                );
                let radius = max_radius * (1.0 - eased);
                if radius > 0.0 {
                    renderer.fill_quad(
                        Quad {
                            bounds: circle_rect(state.origin, radius),
                            // A square with corner radius == half its side is a circle.
                            border: Border::default().rounded(radius),
                            ..Default::default()
                        },
                        state.shown,
                    );
                }
            } else {
                // Old color everywhere; the new color expands outward from
                // the origin until it covers everything.
                renderer.fill_quad(
                    Quad {
                        bounds,
                        border: Border::default(),
                        ..Default::default()
                    },
                    state.shown,
                );
                let radius = max_radius * eased;
                if radius > 0.0 {
                    renderer.fill_quad(
                        Quad {
                            bounds: circle_rect(state.origin, radius),
                            border: Border::default().rounded(radius),
                            ..Default::default()
                        },
                        state.next,
                    );
                }
            }
        } else {
            // Rest state: the full background shows the current color.
            renderer.fill_quad(
                Quad {
                    bounds,
                    border: Border::default(),
                    ..Default::default()
                },
                state.shown,
            );
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::default()))
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();

        if !state.initialized {
            state.shown = self.color;
            state.next = self.color;
            state.progress = 1.0;
            state.initialized = true;
            return;
        }

        if self.color == state.next {
            return;
        }

        match self.origin {
            Some(origin) => {
                // Interactive color change: reveal from the pressed widget.
                let old = state.next;
                let new = self.color;
                state.shown = state.next;
                state.next = self.color;
                state.origin = origin;
                state.progress = 0.0;
                state.last = None;
                state.animating = true;
                state.done_published = false;
                // Switching back to a lighter color contracts the old color
                // inward; switching to a darker color expands the new one
                // outward — like nipaplay's light/dark switch.
                state.reverse = luminance(new) > luminance(old);
            }
            None => {
                // Non-interactive change: switch instantly.
                state.shown = self.color;
                state.next = self.color;
                state.progress = 1.0;
                state.animating = false;
            }
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && state.animating
        {
            let dt = if let Some(last) = state.last {
                now.duration_since(last).as_secs_f32()
            } else {
                state.last = Some(*now);
                0.0
            };
            if dt > 0.0 {
                state.last = Some(*now);
                state.progress = (state.progress + dt / self.duration).min(1.0);
            }

            if state.progress >= 1.0 {
                state.animating = false;
                state.shown = state.next;
            } else {
                shell.request_redraw();
            }

            // Per-button commands: as the circle sweeps, each subscribed
            // button receives a one-shot command when the sweep reaches it.
            if self.reveal.subscriber_count() > 0 {
                let radius = max_corner_distance(layout.bounds(), state.origin)
                    * ease_out_cubic(state.progress);
                self.reveal
                    .notify_covered(state.origin, radius, state.reverse);
            }

            // The mode switch happens only after the full sweep completes
            // (which covers every button), so the animation is always fully
            // visible before the theme flips.
            if !state.done_published && state.progress >= 1.0 {
                state.done_published = true;
                shell.publish(LayoutMessage::ThemeRevealDone);
            }

            if std::env::var("RERN_DEBUG_REVEAL").is_ok() {
                eprintln!(
                    "[bg-reveal] progress={:.3} eased={:.3} subscribers={} covered={} pos={:?}",
                    state.progress,
                    ease_out_cubic(state.progress),
                    self.reveal.subscriber_count(),
                    self.reveal.covered_count(),
                    self.reveal.positions(),
                );
            }
        }
    }
}

/// A square centered at `origin` with the given `radius`.
fn circle_rect(origin: (f32, f32), radius: f32) -> Rectangle {
    Rectangle {
        x: origin.0 - radius,
        y: origin.1 - radius,
        width: radius * 2.0,
        height: radius * 2.0,
    }
}

/// Perceived brightness of a color (Rec. 601 luma).
fn luminance(color: Color) -> f32 {
    0.299 * color.r + 0.587 * color.g + 0.114 * color.b
}

impl From<BackgroundWidget> for Element<'static, LayoutMessage> {
    fn from(widget: BackgroundWidget) -> Self {
        Element::new(widget)
    }
}

/// The distance from `origin` to the farthest corner of `bounds`.
fn max_corner_distance(bounds: Rectangle, origin: (f32, f32)) -> f32 {
    let (ox, oy) = origin;
    let corners = [
        (bounds.x, bounds.y),
        (bounds.x + bounds.width, bounds.y),
        (bounds.x, bounds.y + bounds.height),
        (bounds.x + bounds.width, bounds.y + bounds.height),
    ];
    corners
        .iter()
        .map(|(x, y)| {
            let dx = x - ox;
            let dy = y - oy;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0, f32::max)
}

/// Cubic ease-out, matching nipaplay's `Curves.easeOutCubic`.
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_radius_reaches_farthest_corner() {
        let bounds = Rectangle::new(iced::Point::new(0.0, 0.0), iced::Size::new(100.0, 80.0));
        // Origin at the center; the farthest corner distance is sqrt(50^2+40^2).
        let expected = (50.0f32 * 50.0 + 40.0 * 40.0).sqrt();
        let got = max_corner_distance(bounds, (50.0, 40.0));
        assert!((got - expected).abs() < 0.001);
        // Origin in a corner: the opposite corner.
        let got = max_corner_distance(bounds, (0.0, 0.0));
        assert!((got - (100.0f32 * 100.0 + 80.0 * 80.0).sqrt()).abs() < 0.001);
    }

    #[test]
    fn ease_out_cubic_bounds() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        let mid = ease_out_cubic(0.5);
        assert!(mid > 0.5 && mid < 1.0, "ease-out should be fast early");
    }

    #[test]
    fn luminance_orders_light_dark() {
        assert!(luminance(Color::WHITE) > luminance(Color::from_rgb(0.1, 0.1, 0.1)));
    }
}
