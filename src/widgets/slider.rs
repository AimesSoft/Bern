//! The `slider` and `progress` controls: the video player progress bar,
//! ported from nipaplay's `VideoProgressBar`.
//!
//! Layout usage:
//!
//! ```ron
//! // 可拖动进度条：拖动/点击发布 (id, Changed(value))，value 在 0..=1。
//! Widget(id: "seek", kind: "slider", area: "root", props: { "value": "0.35" })
//!
//! // 纯展示进度条：不响应交互，只按 value 显示。
//! Widget(id: "seek_progress", kind: "progress", area: "root", props: { "value": "0.35" })
//! ```
//!
//! Appearance and animations are copied from nipaplay's `VideoProgressBar`
//! (dark mode renders exactly like nipaplay), with colors driven by the
//! active iced theme so the bar also looks right in light mode:
//!
//! - a 4 px capsule track (text color at low opacity), with the played
//!   portion in the text color;
//! - a **capsule thumb** (28 × 16, fully rounded ends, text color) with two
//!   shadows;
//! - hovering grows the thumb 8 % with a 160 ms ease-out cubic animation;
//! - pressing squeezes the thumb through an underdamped **spring**
//!   (stiffness 620 / damping 22), and releasing lets it spring back with a
//!   visible jelly overshoot (stiffness 360 / damping 7.2) — the same
//!   numbers as nipaplay.
//!
//! The layout's `value` prop is the single source of truth: the app patches
//! it after receiving `Changed` (or lets playback progress drive it), and
//! the control simply renders it.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::ui::PressOrigin;
use crate::core::widget::{
    BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent, size_lengths,
};
use iced::advanced::layout::{self, Layout, Limits, Node};
use iced::advanced::renderer::{Quad, Style};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::window;
use iced::{Border, Color, Element, Length, Point, Rectangle, Shadow, Size, Vector};
use std::time::Instant;

/// The layout name of the interactive slider.
pub const SLIDER_NAME: &str = "slider";
/// The layout name of the display-only progress bar.
pub const PROGRESS_NAME: &str = "progress";

/// Total bar height: thumb (16) + margins, like nipaplay's vertical margin.
const BAR_HEIGHT: f32 = 40.0;
/// Track height (nipaplay uses 4 px).
const TRACK_HEIGHT: f32 = 4.0;
/// Thumb base size (nipaplay desktop: 28 × 16 capsule).
const THUMB_BASE: Size = Size::new(28.0, 16.0);
/// Hover growth: 8 % (nipaplay `_thumbSizeForAnimation`).
const HOVER_SCALE: f32 = 0.08;
/// Hover animation duration (nipaplay `_thumbHoverController`: 160 ms).
const HOVER_DURATION_MS: f32 = 160.0;
/// Press spring (nipaplay `_dragThumbSpring`: mass 1, stiffness 620, damping 22).
const PRESS_STIFFNESS: f32 = 620.0;
const PRESS_DAMPING: f32 = 22.0;
/// Release spring (nipaplay `_releaseThumbSpring`: stiffness 360, damping 7.2)
/// — underdamped on purpose: the thumb overshoots and wobbles (jelly).
const RELEASE_STIFFNESS: f32 = 360.0;
const RELEASE_DAMPING: f32 = 7.2;
/// Thumb deformation target while pressed (nipaplay `_beginThumbPress`).
const PRESS_DEFORM: f32 = 0.62;

/// The interactive slider control (the [`WidgetDef`]).
#[derive(Default)]
pub struct Slider;

/// The display-only progress bar control (the [`WidgetDef`]).
#[derive(Default)]
pub struct Progress;

impl WidgetDef for Slider {
    fn name(&self) -> &'static str {
        SLIDER_NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        build_bar(node, size, ctx, true)
    }
}

impl WidgetDef for Progress {
    fn name(&self) -> &'static str {
        PROGRESS_NAME
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        build_bar(node, size, ctx, false)
    }
}

/// Shared construction for both controls.
fn build_bar<'a, 't>(
    node: &'a LayoutWidget,
    size: Option<crate::core::layout::SizePolicy>,
    ctx: &BuildContext<'a, 't>,
    interactive: bool,
) -> Element<'a, LayoutMessage> {
    let value = node
        .prop("value")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let (width, _height) = size_lengths(size);
    let text = ctx.theme.palette().text;

    SliderView {
        value,
        id: ctx.qualify(&node.id),
        interactive,
        width: width.unwrap_or(Length::Fill),
        track: Color::from_rgba(text.r, text.g, text.b, 0.28),
        played: text,
        thumb: text,
        press_origin: ctx.press_origin.clone(),
    }
    .into()
}

/// The capsule progress bar behind both controls.
pub struct SliderView {
    /// Displayed value in 0..=1 (from the layout prop).
    value: f32,
    /// Qualified event id (used only when interactive).
    id: String,
    /// Whether the bar accepts hover/drag input.
    interactive: bool,
    width: Length,
    track: Color,
    played: Color,
    thumb: Color,
    press_origin: PressOrigin,
}

/// Widget-tree state: hover/drag plus the two animations (hover growth and
/// the spring-based jelly deformation).
struct State {
    hovered: bool,
    dragging: bool,
    /// Hover growth 0..1 (eased with ease-out cubic in draw).
    hover_growth: f32,
    /// Spring deformation: >0 squeezes (narrower + taller), <0 rebounds
    /// (wider + shorter), like nipaplay's `_thumbDeformController`.
    deform: f32,
    deform_velocity: f32,
    /// Current spring parameters and target.
    spring: (f32, f32),
    target: f32,
    last: Option<Instant>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hovered: false,
            dragging: false,
            hover_growth: 0.0,
            deform: 0.0,
            deform_velocity: 0.0,
            spring: (RELEASE_STIFFNESS, RELEASE_DAMPING),
            target: 0.0,
            last: None,
        }
    }
}

impl Widget<LayoutMessage, iced::Theme, iced::Renderer> for SliderView {
    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(BAR_HEIGHT))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        layout::atomic(limits, self.width, Length::Fixed(BAR_HEIGHT))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::default()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, _tree: &mut Tree) {}

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

        // 每收到任何事件都推进动画：RedrawRequested 用事件自带时间戳
        // （测试可注入时间），其它事件用墙钟。帧间隔异常大时按 0 处理，
        // 避免空闲恢复后一步补完动画。
        let now = match event {
            Event::Window(window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };
        let dt = match state.last {
            Some(last) => {
                let elapsed = now.duration_since(last).as_secs_f32();
                if elapsed > 0.1 { 0.0 } else { elapsed }
            }
            None => 0.0,
        };
        state.last = Some(now);

        if self.interactive {
            let over = cursor.is_over(bounds);
            if over != state.hovered {
                state.hovered = over;
                shell.request_redraw();
            }

            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    if over {
                        state.dragging = true;
                        let center = bounds.center();
                        self.press_origin.record((center.x, center.y));
                        if let Some(value) = position_value(bounds, cursor) {
                            self.value = value;
                            self.publish(shell);
                        }
                        // 按下：弹簧把滑块压扁（变窄变长）。
                        state.spring = (PRESS_STIFFNESS, PRESS_DAMPING);
                        state.target = PRESS_DEFORM;
                        shell.request_redraw();
                    }
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    if state.dragging {
                        state.dragging = false;
                        // 松开：换成低阻尼弹簧弹回 0，过冲振荡 = 果冻。
                        state.spring = (RELEASE_STIFFNESS, RELEASE_DAMPING);
                        state.target = 0.0;
                        shell.request_redraw();
                    }
                }
                Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    if state.dragging
                        && let Some(value) = position_value(bounds, cursor)
                    {
                        self.value = value;
                        self.publish(shell);
                        shell.request_redraw();
                    }
                }
                _ => {}
            }
        }

        let mut animating = false;

        // 悬浮放大：0 → 1（draw 里 easeOutCubic）。
        let hover_target = if self.interactive && (state.hovered || state.dragging) {
            1.0
        } else {
            0.0
        };
        let remaining = hover_target - state.hover_growth;
        if remaining.abs() > 0.0005 {
            let step = dt / (HOVER_DURATION_MS / 1000.0);
            state.hover_growth = if remaining > 0.0 {
                (state.hover_growth + step).min(hover_target)
            } else {
                (state.hover_growth - step).max(hover_target)
            };
            animating = true;
        } else {
            state.hover_growth = hover_target;
        }

        // 弹簧积分（mass = 1）：果冻形变。
        let (stiffness, damping) = state.spring;
        let accel = -stiffness * (state.deform - state.target) - damping * state.deform_velocity;
        state.deform_velocity += accel * dt;
        state.deform += state.deform_velocity * dt;
        if (state.deform - state.target).abs() < 0.001 && state.deform_velocity.abs() < 0.001 {
            state.deform = state.target;
            state.deform_velocity = 0.0;
        } else {
            animating = true;
        }

        if animating {
            shell.request_redraw();
        }
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
        let center_y = bounds.y + bounds.height / 2.0;
        let track_rect = Rectangle::new(
            Point::new(bounds.x, center_y - TRACK_HEIGHT / 2.0),
            Size::new(bounds.width, TRACK_HEIGHT),
        );
        let played_width = (bounds.width * self.value).clamp(0.0, bounds.width);

        // 背景轨道：文字色低透明度（浅色=浅灰，深色=白灰）。
        renderer.fill_quad(
            Quad {
                bounds: track_rect,
                border: Border::default().rounded(TRACK_HEIGHT / 2.0),
                ..Default::default()
            },
            self.track,
        );
        // 已播放部分：文字色（深色=白，浅色=黑），与主题一致。
        if played_width > 0.0 {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle::new(
                        track_rect.position(),
                        Size::new(played_width, TRACK_HEIGHT),
                    ),
                    border: Border::default().rounded(TRACK_HEIGHT / 2.0),
                    ..Default::default()
                },
                self.played,
            );
        }

        // 胶囊滑块：尺寸 = 基尺寸 ×（悬浮放大 × 弹簧形变），
        // 公式照抄 nipaplay `_thumbSizeForAnimation`。
        let growth = ease_out_cubic(state.hover_growth);
        let squeeze = state.deform.max(0.0);
        let rebound = (-state.deform).max(0.0);
        let hover_scale = 1.0 + HOVER_SCALE * growth;
        let width_scale = (hover_scale * (1.0 - 0.42 * squeeze + 0.34 * rebound)).clamp(0.56, 1.38);
        let height_scale =
            (hover_scale * (1.0 + 0.40 * squeeze - 0.30 * rebound)).clamp(0.68, 1.42);
        let thumb_width = THUMB_BASE.width * width_scale;
        let thumb_height = THUMB_BASE.height * height_scale;
        let thumb_x = bounds.x + bounds.width * self.value;
        let thumb = Rectangle::new(
            Point::new(thumb_x - thumb_width / 2.0, center_y - thumb_height / 2.0),
            Size::new(thumb_width, thumb_height),
        );
        let emphasis = state.dragging || state.hovered || state.deform.abs() > 0.05;
        renderer.fill_quad(
            Quad {
                bounds: thumb,
                border: Border::default().rounded(thumb_height / 2.0),
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.39),
                    offset: Vector::new(0.0, 2.0),
                    blur_radius: if emphasis { 12.0 } else { 8.0 },
                },
                snap: false,
            },
            self.thumb,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if self.interactive {
            let state = tree.state.downcast_ref::<State>();
            if cursor.is_over(layout.bounds()) {
                return if state.dragging {
                    mouse::Interaction::Grabbing
                } else {
                    mouse::Interaction::Pointer
                };
            }
        }
        mouse::Interaction::default()
    }
}

impl SliderView {
    /// Publishes a value-change event (slider only).
    fn publish(&self, shell: &mut Shell<'_, LayoutMessage>) {
        shell.publish(LayoutMessage::Event(WidgetEvent {
            widget_id: self.id.clone(),
            kind: EventKind::Changed(self.value),
        }));
    }
}

/// Maps a cursor position inside `bounds` to a 0..=1 value.
fn position_value(bounds: Rectangle, cursor: mouse::Cursor) -> Option<f32> {
    let position = cursor.position()?;
    if bounds.width <= 0.0 {
        return None;
    }
    Some(((position.x - bounds.x) / bounds.width).clamp(0.0, 1.0))
}

/// Ease-out cubic (nipaplay uses it for the hover growth).
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl From<SliderView> for Element<'_, LayoutMessage> {
    fn from(widget: SliderView) -> Self {
        Element::new(widget)
    }
}
