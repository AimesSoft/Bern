//! A draggable virtual window inspired by Nipaplay's `NipaplayWindow`.
//!
//! The control fills its layout area with a modal surface and places an
//! embedded RON layout inside a floating window. The title bar can be dragged;
//! double-clicking it toggles between windowed and filled-screen modes.
//!
//! ```ron
//! Widget(id: "settings_window", kind: "virtual_window", area: "root", size: Fill,
//!        props: {
//!            "src": "settings_content",
//!            "title": "设置",
//!            "width": "850",
//!            "height_factor": "0.8",
//!            // Prefer this when the window is already a high-z child of the
//!            // root Stack. Nested windows can omit it and use an overlay.
//!            "presentation": "inline",
//!        })
//! ```
//!
//! Closing the window (with the close control or by clicking the scrim)
//! publishes `EventKind::Other("close")` using the virtual window's id.

use crate::core::frame_clock::animation_frame_interval;
use crate::core::layout::{SizePolicy, Widget as LayoutWidget};
use crate::core::widget::{
    BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent, size_lengths,
};
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{Quad, Style};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Overlay, Renderer, Shell, Widget, mouse, overlay};
use iced::event::Event;
use iced::widget::{Column, Row, Space, button, container};
use iced::window;
use iced::{
    Background, Border, Color, Element, Length, Padding, Point, Rectangle, Shadow, Size,
    Transformation, Vector,
};
use std::cell::Cell;
use std::time::{Duration, Instant};

/// The layout name of this control.
pub const NAME: &str = "virtual_window";

const DEFAULT_WIDTH: f32 = 850.0;
const DEFAULT_HEIGHT_FACTOR: f32 = 0.8;
const DEFAULT_MARGIN: f32 = 20.0;
const DEFAULT_FILLED_MARGIN: f32 = 10.0;
const DEFAULT_RADIUS: f32 = 15.0;
const DEFAULT_TITLE_HEIGHT: f32 = 48.0;
const DEFAULT_TITLE_SIZE: f32 = 20.0;
const DEFAULT_ANIMATION_MS: f32 = 250.0;
const DOUBLE_CLICK_MS: u128 = 350;
const DOUBLE_CLICK_DISTANCE: f32 = 8.0;
const MIN_VISIBLE_TITLE_WIDTH: f32 = 96.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseStyle {
    Mac,
    Fluent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    Overlay,
    Inline,
}

impl Presentation {
    fn label(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Inline => "inline",
        }
    }

    fn resolve(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("inline" | "root" | "stack") => Self::Inline,
            _ => Self::Overlay,
        }
    }
}

impl CloseStyle {
    fn resolve(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("mac" | "macos" | "traffic_light" | "traffic-light") => Self::Mac,
            Some("fluent" | "windows") => Self::Fluent,
            _ if cfg!(target_os = "macos") => Self::Mac,
            _ => Self::Fluent,
        }
    }
}

#[derive(Debug, Clone)]
struct Visual {
    window_width: f32,
    window_height: Option<f32>,
    height_factor: f32,
    margin: f32,
    filled_margin: f32,
    radius: f32,
    title_height: f32,
    background: Color,
    border: Color,
    scrim: Color,
    dismiss_on_scrim: bool,
    animation_duration: f32,
    initially_filled: bool,
}

/// The virtual-window control.
#[derive(Default)]
pub struct VirtualWindow;

impl WidgetDef for VirtualWindow {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let build_started = Instant::now();
        let id = ctx.qualify(&node.id);
        let close_message = LayoutMessage::Event(WidgetEvent {
            widget_id: id,
            kind: EventKind::Other("close".into()),
        });
        let title = node.str_prop("title").unwrap_or("窗口");
        let title_height = float_prop(node, "title_height", DEFAULT_TITLE_HEIGHT, 32.0, 96.0);
        let title_size = float_prop(node, "title_size", DEFAULT_TITLE_SIZE, 12.0, 48.0);
        let show_close = bool_prop(node, "show_close", true);
        let close_style = CloseStyle::resolve(node.str_prop("close_style"));
        let text_color = ctx.theme.palette().text;
        let separator_color = with_alpha(text_color, 0.12);

        let title_text: Element<'a, LayoutMessage> = iced::widget::text(title)
            .size(title_size)
            .font(crate::fonts::bold_font())
            .color(text_color)
            .into();
        let spacer: Element<'a, LayoutMessage> = Space::new().width(Length::Fill).into();
        let close =
            show_close.then(|| close_button(close_style, close_message.clone(), text_color));

        let mut title_children = Vec::with_capacity(3);
        if close_style == CloseStyle::Mac {
            if let Some(close) = close {
                title_children.push(close);
            }
            title_children.push(title_text);
            title_children.push(spacer);
        } else {
            title_children.push(title_text);
            title_children.push(spacer);
            if let Some(close) = close {
                title_children.push(close);
            }
        }

        let title_padding = if close_style == CloseStyle::Mac {
            Padding {
                top: 5.0,
                right: 12.0,
                bottom: 5.0,
                left: 5.0,
            }
        } else {
            Padding {
                top: 5.0,
                right: 5.0,
                bottom: 5.0,
                left: 16.0,
            }
        };
        let title_bar: Element<'a, LayoutMessage> = container(
            Row::with_children(title_children).align_y(iced::alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .height(title_height)
        .padding(title_padding)
        .align_y(iced::alignment::Vertical::Center)
        .into();
        let separator: Element<'a, LayoutMessage> =
            container(Space::new().width(Length::Fill).height(1.0))
                .style(move |_theme| iced::widget::container::Style {
                    background: Some(Background::Color(separator_color)),
                    ..Default::default()
                })
                .into();

        let content: Element<'a, LayoutMessage> = match node.str_prop("src") {
            Some(src) => match ctx.store.resolve(src) {
                Some(layout) => {
                    let child_ctx = ctx.with_prefix(&node.id);
                    match ctx.registry.build_embedded(layout, &child_ctx) {
                        Ok(element) => element,
                        Err(error) => error_text(
                            format!("virtual_window `{}`: layout `{src}`: {error:?}", node.id),
                            ctx.theme.extended_palette().danger.base.color,
                        ),
                    }
                }
                None => error_text(
                    format!("virtual_window `{}`: layout `{src}` not found", node.id),
                    ctx.theme.extended_palette().danger.base.color,
                ),
            },
            None => error_text(
                format!("virtual_window `{}`: missing `src` prop", node.id),
                ctx.theme.extended_palette().danger.base.color,
            ),
        };
        let content_padding = float_prop(node, "content_padding", 0.0, 0.0, 128.0);
        let body: Element<'a, LayoutMessage> = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(content_padding)
            .into();
        let window_content: Element<'a, LayoutMessage> =
            Column::with_children([title_bar, separator, body])
                .width(Length::Fill)
                .height(Length::Fill)
                .into();

        let is_dark = ctx.theme.extended_palette().is_dark;
        let background = if is_dark {
            Color::from_rgb8(0x2C, 0x2C, 0x2C)
        } else {
            Color::WHITE
        };
        let scrim_alpha = float_prop(node, "scrim_alpha", 0.0, 0.0, 0.8);
        let visual = Visual {
            window_width: node
                .prop("width")
                .or_else(|| node.prop("max_width"))
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite())
                .unwrap_or(DEFAULT_WIDTH)
                .clamp(160.0, 4096.0),
            window_height: node
                .prop("height")
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value > 0.0),
            height_factor: node
                .prop("height_factor")
                .or_else(|| node.prop("max_height_factor"))
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite())
                .unwrap_or(DEFAULT_HEIGHT_FACTOR)
                .clamp(0.2, 1.0),
            margin: float_prop(node, "margin", DEFAULT_MARGIN, 0.0, 256.0),
            filled_margin: float_prop(node, "filled_margin", DEFAULT_FILLED_MARGIN, 0.0, 128.0),
            radius: float_prop(node, "radius", DEFAULT_RADIUS, 0.0, 64.0),
            title_height,
            background,
            border: with_alpha(text_color, 0.10),
            scrim: Color::from_rgba(0.0, 0.0, 0.0, scrim_alpha),
            dismiss_on_scrim: bool_prop(node, "dismiss_on_scrim", true),
            animation_duration: float_prop(node, "animation_ms", DEFAULT_ANIMATION_MS, 1.0, 5000.0)
                / 1000.0,
            initially_filled: matches!(
                node.str_prop("mode"),
                Some("filled" | "filled_screen" | "filled-screen" | "maximized")
            ),
        };
        let (root_width, root_height) = size_lengths(size);

        VirtualWindowWidget {
            content: window_content,
            close_message,
            visual,
            presentation: Presentation::resolve(node.str_prop("presentation")),
            build_duration: build_started.elapsed(),
            width: root_width.unwrap_or(Length::Fill),
            height: root_height.unwrap_or(Length::Fill),
        }
        .into()
    }
}

fn close_button<'a>(
    style: CloseStyle,
    message: LayoutMessage,
    text_color: Color,
) -> Element<'a, LayoutMessage> {
    match style {
        CloseStyle::Mac => {
            let dot = container(Space::new())
                .width(14.0)
                .height(14.0)
                .style(|_theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgb8(0xFF, 0x5F, 0x57))),
                    border: Border::default().rounded(7),
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
                        offset: Vector::new(0.0, 1.0),
                        blur_radius: 2.0,
                    },
                    ..Default::default()
                });
            button(dot)
                .on_press(message)
                .width(28.0)
                .height(28.0)
                .padding(7)
                .style(|_theme, _status| iced::widget::button::Style {
                    background: None,
                    border: Border::default(),
                    ..Default::default()
                })
                .into()
        }
        CloseStyle::Fluent => button(iced::widget::text("×").size(18))
            .on_press(message)
            .width(32.0)
            .height(28.0)
            .padding(0)
            .style(move |_theme, status| {
                let hovered = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                iced::widget::button::Style {
                    background: hovered
                        .then_some(Background::Color(Color::from_rgb8(0xC4, 0x2B, 0x1C))),
                    text_color: if hovered { Color::WHITE } else { text_color },
                    border: Border::default().rounded(5),
                    ..Default::default()
                }
            })
            .into(),
    }
}

struct State {
    offset: Vector,
    maximized: bool,
    dragging: bool,
    last_cursor: Option<Point>,
    last_title_click: Option<(Instant, Point)>,
    open_progress: f32,
    opened_at: Instant,
    perf: PerfStats,
}

impl State {
    fn new(maximized: bool, presentation: Presentation) -> Self {
        Self {
            offset: Vector::ZERO,
            maximized,
            dragging: false,
            last_cursor: None,
            last_title_click: None,
            open_progress: 0.0,
            opened_at: Instant::now(),
            perf: PerfStats::new(presentation),
        }
    }
}

#[derive(Default)]
struct StageStats {
    calls: Cell<u64>,
    total_ns: Cell<u128>,
    max_ns: Cell<u128>,
}

impl StageStats {
    fn record(&self, duration: Duration) {
        let elapsed = duration.as_nanos();
        self.calls.set(self.calls.get() + 1);
        self.total_ns.set(self.total_ns.get() + elapsed);
        self.max_ns.set(self.max_ns.get().max(elapsed));
    }

    fn take(&self) -> StageSnapshot {
        let snapshot = StageSnapshot {
            calls: self.calls.replace(0),
            total_ns: self.total_ns.replace(0),
            max_ns: self.max_ns.replace(0),
        };
        snapshot
    }
}

struct StageSnapshot {
    calls: u64,
    total_ns: u128,
    max_ns: u128,
}

impl StageSnapshot {
    fn summary(&self) -> String {
        if self.calls == 0 {
            return "0x".into();
        }
        let total_ms = self.total_ns as f64 / 1_000_000.0;
        let average_ms = total_ms / self.calls as f64;
        let max_ms = self.max_ns as f64 / 1_000_000.0;
        format!(
            "{}x total={total_ms:.2}ms avg={average_ms:.3}ms max={max_ms:.3}ms",
            self.calls
        )
    }
}

struct PerfStats {
    presentation: Presentation,
    interval_started: Cell<Instant>,
    build: StageStats,
    layout: StageStats,
    draw: StageStats,
    content_update: StageStats,
    mouse_interaction: StageStats,
    active_frame_gap: StageStats,
    opening_frame_gap: StageStats,
    last_draw: Cell<Option<Instant>>,
    opening_draws: Cell<u64>,
    events: Cell<u64>,
    redraw_events: Cell<u64>,
    wheel_events: Cell<u64>,
    line_wheel_events: Cell<u64>,
    pixel_wheel_events: Cell<u64>,
    cursor_events: Cell<u64>,
}

impl PerfStats {
    fn new(presentation: Presentation) -> Self {
        Self {
            presentation,
            interval_started: Cell::new(Instant::now()),
            build: StageStats::default(),
            layout: StageStats::default(),
            draw: StageStats::default(),
            content_update: StageStats::default(),
            mouse_interaction: StageStats::default(),
            active_frame_gap: StageStats::default(),
            opening_frame_gap: StageStats::default(),
            last_draw: Cell::new(None),
            opening_draws: Cell::new(0),
            events: Cell::new(0),
            redraw_events: Cell::new(0),
            wheel_events: Cell::new(0),
            line_wheel_events: Cell::new(0),
            pixel_wheel_events: Cell::new(0),
            cursor_events: Cell::new(0),
        }
    }

    fn record_event(&self, event: &Event) {
        self.events.set(self.events.get() + 1);
        match event {
            Event::Window(window::Event::RedrawRequested(_)) => {
                self.redraw_events.set(self.redraw_events.get() + 1);
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                self.wheel_events.set(self.wheel_events.get() + 1);
                match delta {
                    mouse::ScrollDelta::Lines { .. } => {
                        self.line_wheel_events.set(self.line_wheel_events.get() + 1);
                    }
                    mouse::ScrollDelta::Pixels { .. } => {
                        self.pixel_wheel_events
                            .set(self.pixel_wheel_events.get() + 1);
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                self.cursor_events.set(self.cursor_events.get() + 1);
            }
            _ => {}
        }
    }

    fn record_draw_frame(&self, opening: bool) {
        let now = Instant::now();
        if let Some(last) = self.last_draw.replace(Some(now)) {
            let gap = now.duration_since(last);
            // Ignore idle time. Only consecutive animation/scrolling frames
            // are useful for judging cadence.
            if gap <= Duration::from_millis(100) {
                self.active_frame_gap.record(gap);
                if opening {
                    self.opening_frame_gap.record(gap);
                }
            }
        }
        if opening {
            self.opening_draws.set(self.opening_draws.get() + 1);
        }
    }

    fn report_if_due(&self) {
        let now = Instant::now();
        let interval = now.duration_since(self.interval_started.get());
        if interval < Duration::from_secs(1) {
            return;
        }
        self.interval_started.set(now);

        let events = self.events.replace(0);
        let redraw = self.redraw_events.replace(0);
        let wheel = self.wheel_events.replace(0);
        let line_wheel = self.line_wheel_events.replace(0);
        let pixel_wheel = self.pixel_wheel_events.replace(0);
        let cursor = self.cursor_events.replace(0);
        let other = events.saturating_sub(redraw + wheel + cursor);
        let build = self.build.take();
        let layout = self.layout.take();
        let draw = self.draw.take();
        let content_update = self.content_update.take();
        let mouse_interaction = self.mouse_interaction.take();
        let active_frame_gap = self.active_frame_gap.take();
        let opening_frame_gap = self.opening_frame_gap.take();
        let opening_draws = self.opening_draws.replace(0);
        let fps = draw.calls as f64 / interval.as_secs_f64();

        eprintln!(
            "[bern-perf][virtual_window] mode={} interval={:.0}ms draw_rate={fps:.1}/s events={events} (redraw={redraw} wheel={wheel}[line={line_wheel},pixel={pixel_wheel}] cursor={cursor} other={other}) opening_draws={opening_draws}\n  build: {}\n  layout: {}\n  draw: {}\n  active_frame_gap: {}\n  opening_frame_gap: {}\n  content_update: {}\n  mouse_interaction: {}",
            self.presentation.label(),
            interval.as_secs_f64() * 1000.0,
            build.summary(),
            layout.summary(),
            draw.summary(),
            active_frame_gap.summary(),
            opening_frame_gap.summary(),
            content_update.summary(),
            mouse_interaction.summary(),
        );
    }
}

struct VirtualWindowWidget<'a, Message> {
    content: Element<'a, Message>,
    close_message: Message,
    visual: Visual,
    presentation: Presentation,
    build_duration: Duration,
    width: Length,
    height: Length,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for VirtualWindowWidget<'a, Message>
where
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let bounds = limits.resolve(self.width, self.height, Size::ZERO);
        if self.presentation == Presentation::Inline {
            let state = tree.state.downcast_ref::<State>();
            let content_tree = tree
                .children
                .first_mut()
                .expect("virtual window content tree must exist");
            layout_window(
                &mut self.content,
                content_tree,
                state,
                &self.visual,
                renderer,
                bounds,
            )
        } else {
            // The inline node is only an overlay anchor. The actual window is
            // laid out by `VirtualWindowOverlay` against the full application
            // viewport, so nested windows can cover their ancestors.
            Node::new(bounds)
        }
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
        if self.presentation == Presentation::Inline {
            draw_window(
                &self.content,
                &tree.children[0],
                tree.state.downcast_ref::<State>(),
                &self.visual,
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            );
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        let state = State::new(self.visual.initially_filled, self.presentation);
        state.perf.build.record(self.build_duration);
        eprintln!(
            "[bern-perf][virtual_window] created mode={} initial_build={:.3}ms",
            self.presentation.label(),
            self.build_duration.as_secs_f64() * 1000.0,
        );
        tree::State::Some(Box::new(state))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.state
            .downcast_ref::<State>()
            .perf
            .build
            .record(self.build_duration);
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.presentation == Presentation::Inline {
            let state = tree.state.downcast_mut::<State>();
            let content_tree = &mut tree.children[0];
            update_window(
                &mut self.content,
                content_tree,
                state,
                &self.close_message,
                &self.visual,
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
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
        if self.presentation != Presentation::Inline {
            return mouse::Interaction::None;
        }
        let interaction = window_mouse_interaction(
            &self.content,
            &tree.children[0],
            tree.state.downcast_ref::<State>(),
            &self.visual,
            layout,
            cursor,
            viewport,
            renderer,
        );
        if interaction == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            interaction
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        if self.presentation == Presentation::Inline {
            let child_layout = layout.children().next()?;
            return self.content.as_widget_mut().overlay(
                &mut tree.children[0],
                child_layout,
                renderer,
                viewport,
                translation,
            );
        }
        let state = tree.state.downcast_mut::<State>();
        let content_tree = tree.children.first_mut()?;
        Some(overlay::Element::new(Box::new(VirtualWindowOverlay {
            content: &mut self.content,
            content_tree,
            state,
            close_message: self.close_message.clone(),
            visual: self.visual.clone(),
            viewport: *viewport,
        })))
    }
}

/// Full-viewport overlay for the virtual window. Keeping the modal surface
/// here (instead of in the widget's inline `draw`) lets a window declared in
/// a nested page cover sibling UI such as the application's top Tab bar.
struct VirtualWindowOverlay<'borrow, 'element, Message> {
    content: &'borrow mut Element<'element, Message>,
    content_tree: &'borrow mut Tree,
    state: &'borrow mut State,
    close_message: Message,
    visual: Visual,
    viewport: Rectangle,
}

fn layout_window<Message>(
    content: &mut Element<'_, Message>,
    content_tree: &mut Tree,
    state: &State,
    visual: &Visual,
    renderer: &iced::Renderer,
    bounds: Size,
) -> Node {
    let started = Instant::now();
    let window_size = resolve_window_size(bounds, visual, state.maximized);
    let position =
        resolve_window_position(bounds, window_size, visual, state.maximized, state.offset);
    let child_limits = Limits::new(window_size, window_size);
    let child = content
        .as_widget_mut()
        .layout(content_tree, renderer, &child_limits)
        .move_to(position);
    let node = Node::with_children(bounds, vec![child]);
    state.perf.layout.record(started.elapsed());
    node
}

#[allow(clippy::too_many_arguments)]
fn draw_window<Message>(
    content: &Element<'_, Message>,
    content_tree: &Tree,
    state: &State,
    visual: &Visual,
    renderer: &mut iced::Renderer,
    theme: &iced::Theme,
    style: &Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
) {
    let started = Instant::now();
    state.perf.record_draw_frame(state.open_progress < 1.0);
    let root_bounds = layout.bounds();
    if visual.scrim.a > 0.0 {
        renderer.fill_quad(
            Quad {
                bounds: root_bounds,
                ..Default::default()
            },
            visual.scrim,
        );
    }

    let Some(window_layout) = layout.children().next() else {
        state.perf.draw.record(started.elapsed());
        state.perf.report_if_due();
        return;
    };
    let window_bounds = window_layout.bounds();
    // Moving a fully populated widget tree with a small vertical offset keeps
    // the opening motion composited without repeatedly resampling the whole
    // modal. Scaling the tree used to interact badly with nested clipping on
    // macOS: the untransformed clip and the overshooting scale briefly exposed
    // stale pixels from the background.
    let translate_y = 14.0 * (1.0 - ease_out_cubic(state.open_progress));
    renderer.with_transformation(Transformation::translate(0.0, translate_y), |renderer| {
        renderer.fill_quad(
            Quad {
                bounds: window_bounds,
                border: Border::default()
                    .rounded(visual.radius)
                    .width(1.0)
                    .color(visual.border),
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
                    offset: Vector::new(0.0, 5.0),
                    blur_radius: 15.0,
                },
                ..Default::default()
            },
            visual.background,
        );
        renderer.with_layer(window_bounds, |renderer| {
            content.as_widget().draw(
                content_tree,
                renderer,
                theme,
                style,
                window_layout,
                cursor,
                viewport,
            );
        });
    });
    state.perf.draw.record(started.elapsed());
    state.perf.report_if_due();
}

#[allow(clippy::too_many_arguments)]
fn update_window<Message: Clone>(
    content: &mut Element<'_, Message>,
    content_tree: &mut Tree,
    state: &mut State,
    close_message: &Message,
    visual: &Visual,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &iced::Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    viewport: &Rectangle,
) {
    state.perf.record_event(event);
    let Some(window_layout) = layout.children().next() else {
        return;
    };

    if let Event::Window(window::Event::RedrawRequested(now)) = event
        && state.open_progress < 1.0
    {
        let elapsed = now
            .checked_duration_since(state.opened_at)
            .unwrap_or_default()
            .as_secs_f32();
        state.open_progress = (elapsed / visual.animation_duration).min(1.0);
        if state.open_progress < 1.0 {
            // Keep deadlines anchored to the start of the animation. Adding a
            // full interval to `now` here makes the compositor wait once to
            // deliver this frame and then makes Iced wait a second time,
            // effectively reducing a 60 Hz opening animation to about 30 Hz.
            Shell::replace_redraw_request(
                shell,
                window::RedrawRequest::At(next_animation_frame(
                    state.opened_at,
                    *now,
                    animation_frame_interval(),
                )),
            );
        }
        // This frame only advances the virtual window's opening transform.
        // Avoid dispatching it through every control in a large settings page
        // or letting an inline modal's obscured siblings process it as well.
        shell.capture_event();
        return;
    }

    let content_started = Instant::now();
    content.as_widget_mut().update(
        content_tree,
        event,
        window_layout,
        cursor,
        renderer,
        clipboard,
        shell,
        viewport,
    );
    state.perf.content_update.record(content_started.elapsed());

    if shell.is_event_captured() {
        return;
    }

    let window_bounds = window_layout.bounds();
    let title_bounds = Rectangle {
        height: visual.title_height,
        ..window_bounds
    };
    match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            let Some(position) = cursor.position() else {
                return;
            };
            if title_bounds.contains(position) {
                let now = Instant::now();
                let is_double = state.last_title_click.is_some_and(|(last, point)| {
                    now.duration_since(last).as_millis() <= DOUBLE_CLICK_MS
                        && point.distance(position) <= DOUBLE_CLICK_DISTANCE
                });
                if is_double {
                    state.maximized = !state.maximized;
                    state.offset = Vector::ZERO;
                    state.dragging = false;
                    state.last_cursor = None;
                    state.last_title_click = None;
                } else {
                    state.dragging = !state.maximized;
                    state.last_cursor = Some(position);
                    state.last_title_click = Some((now, position));
                }
                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();
            } else if window_bounds.contains(position) {
                shell.capture_event();
            } else if layout.bounds().contains(position) {
                if visual.dismiss_on_scrim {
                    shell.publish(close_message.clone());
                }
                shell.capture_event();
            }
        }
        Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
            if let Some(last) = state.last_cursor {
                state.offset += *position - last;
            }
            state.last_cursor = Some(*position);
            shell.capture_event();
            shell.invalidate_layout();
            shell.request_redraw();
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
            state.dragging = false;
            state.last_cursor = None;
            shell.capture_event();
        }
        _ => {}
    }
}

fn window_mouse_interaction<Message>(
    content: &Element<'_, Message>,
    content_tree: &Tree,
    state: &State,
    visual: &Visual,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &iced::Renderer,
) -> mouse::Interaction {
    let started = Instant::now();
    let Some(window_layout) = layout.children().next() else {
        state.perf.mouse_interaction.record(started.elapsed());
        return mouse::Interaction::None;
    };
    let child_interaction = content.as_widget().mouse_interaction(
        content_tree,
        window_layout,
        cursor,
        viewport,
        renderer,
    );
    let interaction = if child_interaction != mouse::Interaction::None {
        child_interaction
    } else {
        let title_bounds = Rectangle {
            height: visual.title_height,
            ..window_layout.bounds()
        };
        if cursor.is_over(title_bounds) && !state.maximized {
            if state.dragging {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Grab
            }
        } else {
            mouse::Interaction::None
        }
    };
    state.perf.mouse_interaction.record(started.elapsed());
    interaction
}

impl<Message> Overlay<Message, iced::Theme, iced::Renderer>
    for VirtualWindowOverlay<'_, '_, Message>
where
    Message: Clone,
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        layout_window(
            self.content,
            self.content_tree,
            self.state,
            &self.visual,
            renderer,
            bounds,
        )
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        draw_window(
            self.content,
            self.content_tree,
            self.state,
            &self.visual,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &self.viewport,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        update_window(
            self.content,
            self.content_tree,
            self.state,
            &self.close_message,
            &self.visual,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        window_mouse_interaction(
            self.content,
            self.content_tree,
            self.state,
            &self.visual,
            layout,
            cursor,
            &self.viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let child_layout = layout.children().next()?;
        self.content.as_widget_mut().overlay(
            self.content_tree,
            child_layout,
            renderer,
            &self.viewport,
            Vector::ZERO,
        )
    }

    fn index(&self) -> f32 {
        10.0
    }
}

impl<'a, Message: Clone + 'a> From<VirtualWindowWidget<'a, Message>> for Element<'a, Message> {
    fn from(widget: VirtualWindowWidget<'a, Message>) -> Self {
        Element::new(widget)
    }
}

fn resolve_window_size(outer: Size, visual: &Visual, maximized: bool) -> Size {
    let margin = if maximized {
        visual.filled_margin
    } else {
        visual.margin
    };
    let available = Size::new(
        (outer.width - margin * 2.0).max(0.0),
        (outer.height - margin * 2.0).max(0.0),
    );
    if maximized {
        available
    } else {
        Size::new(
            visual.window_width.min(available.width),
            visual
                .window_height
                .unwrap_or(outer.height * visual.height_factor)
                .min(available.height),
        )
    }
}

fn resolve_window_position(
    outer: Size,
    window: Size,
    visual: &Visual,
    maximized: bool,
    offset: Vector,
) -> Point {
    if maximized {
        return Point::new(visual.filled_margin, visual.filled_margin);
    }
    let centered = Point::new(
        (outer.width - window.width) / 2.0 + offset.x,
        (outer.height - window.height) / 2.0 + offset.y,
    );
    Point::new(
        centered.x.clamp(
            -window.width + MIN_VISIBLE_TITLE_WIDTH,
            outer.width - MIN_VISIBLE_TITLE_WIDTH,
        ),
        centered
            .y
            .clamp(0.0, (outer.height - visual.title_height).max(0.0)),
    )
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn next_animation_frame(opened_at: Instant, now: Instant, interval: Duration) -> Instant {
    let interval_ns = interval.as_nanos().max(1);
    let elapsed_ns = now
        .checked_duration_since(opened_at)
        .unwrap_or_default()
        .as_nanos();
    let next_frame = elapsed_ns / interval_ns + 1;
    let deadline_ns = interval_ns.saturating_mul(next_frame);
    let deadline = Duration::from_nanos(deadline_ns.min(u128::from(u64::MAX)) as u64);
    opened_at + deadline
}

fn float_prop(node: &LayoutWidget, key: &str, default: f32, min: f32, max: f32) -> f32 {
    node.prop(key)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(default)
        .clamp(min, max)
}

fn bool_prop(node: &LayoutWidget, key: &str, default: bool) -> bool {
    node.str_prop(key)
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

fn error_text(message: String, color: Color) -> Element<'static, LayoutMessage> {
    container(iced::widget::text(message).size(12).color(color))
        .padding(16)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::Layout;
    use crate::core::store::LayoutStore;
    use crate::core::theme::ThemeRouter;

    fn test_visual() -> Visual {
        Visual {
            window_width: 850.0,
            window_height: None,
            height_factor: 0.8,
            margin: 20.0,
            filled_margin: 10.0,
            radius: 15.0,
            title_height: 48.0,
            background: Color::WHITE,
            border: Color::BLACK,
            scrim: Color::TRANSPARENT,
            dismiss_on_scrim: true,
            animation_duration: 0.25,
            initially_filled: false,
        }
    }

    #[test]
    fn windowed_and_filled_sizes_follow_reference_margins() {
        let outer = Size::new(1200.0, 800.0);
        let visual = test_visual();
        assert_eq!(
            resolve_window_size(outer, &visual, false),
            Size::new(850.0, 640.0)
        );
        assert_eq!(
            resolve_window_size(outer, &visual, true),
            Size::new(1180.0, 780.0)
        );
    }

    #[test]
    fn ease_out_cubic_stays_inside_bounds() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!((0.0..=1.0).contains(&ease_out_cubic(0.8)));
    }

    #[test]
    fn builds_embedded_layout_from_ron() {
        let content = Layout::parse(
            r#"
            Layout(
                areas: [Area(id: "root", kind: Column)],
                widgets: [Widget(id: "label", kind: "text", area: "root",
                                 props: { "text": "window content" })],
            )
            "#,
        )
        .expect("content parses");
        let page = Layout::parse(
            r#"
            Layout(
                areas: [Area(id: "root", kind: Stack)],
                widgets: [Widget(id: "window", kind: "virtual_window", area: "root",
                                 size: Fill, props: { "src": "content", "title": "Demo" })],
            )
            "#,
        )
        .expect("page parses");

        let mut store = LayoutStore::new();
        store.insert("content", content);
        let registry = crate::builtin_registry();
        registry.ids().register("window");
        let router = ThemeRouter::new(iced::Theme::Light);
        let _element = registry
            .build(&page, &router, &store)
            .expect("virtual window builds");
    }
}
