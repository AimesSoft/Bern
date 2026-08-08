//! A scrollable container whose content is assembled by another RON layout.

use crate::core::frame_clock::animation_frame_interval;
use crate::core::layout::{SizePolicy, Widget as LayoutWidget};
use crate::core::widget::{BuildContext, BuildError, LayoutMessage, WidgetDef, size_lengths};
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::Style;
use iced::advanced::widget::Operation;
use iced::advanced::widget::operation::scrollable::{self as scroll_operation, AbsoluteOffset};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget, mouse, overlay};
use iced::event::Event;
use iced::widget::Id;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::window;
use iced::{Background, Color, Element, Length, Rectangle, Size, Vector};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "scroll_layout";

const MAX_DEPTH: u32 = 32;
const LINE_SCROLL_PIXELS: f32 = 60.0;
const LINE_SMOOTH_TIME_CONSTANT: f32 = 0.055;
const PIXEL_SMOOTH_TIME_CONSTANT: f32 = 0.025;
const MAX_PENDING_SCROLL: f32 = 1200.0;
const SETTLED_DISTANCE: f32 = 0.1;

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
        let id = Id::from(ctx.qualify(&node.id));
        let scrollable: Element<'a, LayoutMessage> = iced::widget::scrollable(content)
            .id(id.clone())
            .width(width.unwrap_or(Length::Fill))
            .height(height.unwrap_or(Length::Fill))
            .direction(Direction::Vertical(capsule_scrollbar()))
            .style(move |_theme, status| neutral_capsule_style(&scroll_theme, status))
            .into();

        SmoothScrollable::new(id, scrollable).into()
    }
}

/// Adds frame-driven interpolation to iced's existing scrollable. There is
/// still exactly one scroll state and one scrollbar: this wrapper only turns
/// sparse wheel/trackpad input into small offsets applied on redraw frames.
struct SmoothScrollable<'a> {
    id: Id,
    child: Element<'a, LayoutMessage>,
}

impl<'a> SmoothScrollable<'a> {
    fn new(id: Id, child: Element<'a, LayoutMessage>) -> Self {
        Self { id, child }
    }
}

struct SmoothState {
    pending_y: f32,
    time_constant: f32,
    last_frame: Option<Instant>,
}

impl Default for SmoothState {
    fn default() -> Self {
        Self {
            pending_y: 0.0,
            time_constant: LINE_SMOOTH_TIME_CONSTANT,
            last_frame: None,
        }
    }
}

impl Widget<LayoutMessage, iced::Theme, iced::Renderer> for SmoothScrollable<'_> {
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.child.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SmoothState>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(SmoothState::default()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.child.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.child));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.child
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, LayoutMessage>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SmoothState>();

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && cursor.is_over(layout.bounds())
        {
            match *delta {
                // Discrete mouse-wheel notches need interpolation between
                // sparse events.
                mouse::ScrollDelta::Lines { y, .. } => {
                    let delta_y = -y * LINE_SCROLL_PIXELS;
                    if delta_y != 0.0 {
                        state.pending_y = (state.pending_y + delta_y)
                            .clamp(-MAX_PENDING_SCROLL, MAX_PENDING_SCROLL);
                        state.time_constant = LINE_SMOOTH_TIME_CONSTANT;
                        shell.capture_event();
                        shell.request_redraw_at(Instant::now());
                        return;
                    }
                }
                // Pixel deltas already contain platform momentum, so use a
                // much shorter interpolation window than wheel notches. This
                // fills the gaps between ~30 Hz input samples without adding
                // the delayed catch-up of the previous implementation.
                mouse::ScrollDelta::Pixels { y, .. } => {
                    let delta_y = -y;
                    if delta_y != 0.0 {
                        state.pending_y = (state.pending_y + delta_y)
                            .clamp(-MAX_PENDING_SCROLL, MAX_PENDING_SCROLL);
                        state.time_constant = PIXEL_SMOOTH_TIME_CONSTANT;
                        shell.capture_event();
                        shell.request_redraw_at(Instant::now());
                        return;
                    }
                }
            }
        }

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(_)) | Event::Touch(_)
        ) {
            state.pending_y = 0.0;
            state.last_frame = None;
        }

        let mut animation_redraw_at = None;
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && state.pending_y.abs() > SETTLED_DISTANCE
        {
            let dt = state
                .last_frame
                .map(|last| now.duration_since(last).as_secs_f32().clamp(0.0, 0.05))
                .unwrap_or(1.0 / 60.0);
            state.last_frame = Some(*now);

            let response = 1.0 - (-dt / state.time_constant).exp();
            let step = state.pending_y * response;
            state.pending_y -= step;

            let mut operation = scroll_operation::scroll_by::<()>(
                self.id.clone(),
                AbsoluteOffset { x: 0.0, y: step },
            );
            self.child.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                &mut operation,
            );

            if state.pending_y.abs() > SETTLED_DISTANCE {
                animation_redraw_at = Some(*now);
            } else {
                state.pending_y = 0.0;
                state.last_frame = None;
            }
        }

        self.child.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if let Some(at) = animation_redraw_at {
            // `NextFrame` requested from inside a macOS redraw callback can be
            // coalesced until the following vsync. Use the shared software
            // 60 Hz deadline instead of relying on that callback chain.
            Shell::replace_redraw_request(
                shell,
                window::RedrawRequest::At(at + animation_frame_interval()),
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.child.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, LayoutMessage, iced::Theme, iced::Renderer>> {
        self.child.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a> From<SmoothScrollable<'a>> for Element<'a, LayoutMessage> {
    fn from(scrollable: SmoothScrollable<'a>) -> Self {
        Element::new(scrollable)
    }
}

fn capsule_scrollbar() -> Scrollbar {
    Scrollbar::new().width(8).scroller_width(6).margin(1)
}

fn neutral_capsule_style(
    theme: &iced::Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let opacity = match status {
        iced::widget::scrollable::Status::Active { .. } => 0.20,
        iced::widget::scrollable::Status::Hovered { .. } => 0.30,
        iced::widget::scrollable::Status::Dragged { .. } => 0.40,
    };
    let text = theme.palette().text;
    let thumb = Background::Color(Color::from_rgba(text.r, text.g, text.b, opacity));
    let mut style = iced::widget::scrollable::default(theme, status);
    let capsule = iced::border::rounded(u32::MAX);
    style.vertical_rail.background = None;
    style.vertical_rail.border = capsule;
    style.vertical_rail.scroller.background = thumb;
    style.vertical_rail.scroller.border = capsule;
    style.horizontal_rail.background = None;
    style.horizontal_rail.border = capsule;
    style.horizontal_rail.scroller.background = thumb;
    style.horizontal_rail.scroller.border = capsule;
    style
}

fn error_text(message: impl Into<String>) -> Element<'static, LayoutMessage> {
    iced::widget::text(message.into())
        .size(12)
        .color(Color::from_rgb(1.0, 0.45, 0.45))
        .into()
}
