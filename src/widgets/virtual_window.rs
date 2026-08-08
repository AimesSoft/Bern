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
//!        })
//! ```
//!
//! Closing the window (with the close control or by clicking the scrim)
//! publishes `EventKind::Other("close")` using the virtual window's id.

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
use std::time::Instant;

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
    last_frame: Option<Instant>,
    content_tree: Tree,
}

impl State {
    fn new(maximized: bool) -> Self {
        Self {
            offset: Vector::ZERO,
            maximized,
            dragging: false,
            last_cursor: None,
            last_title_click: None,
            open_progress: 0.0,
            last_frame: None,
            content_tree: Tree::empty(),
        }
    }
}

struct VirtualWindowWidget<'a, Message> {
    content: Element<'a, Message>,
    close_message: Message,
    visual: Visual,
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

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        // The inline node is only an overlay anchor. The actual window is
        // laid out by `VirtualWindowOverlay` against the full application
        // viewport, so it can cover ancestors such as a top Tab bar.
        Node::new(limits.resolve(self.width, self.height, Size::ZERO))
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::new(self.visual.initially_filled)))
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        _layout: Layout<'b>,
        _renderer: &iced::Renderer,
        viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        state.content_tree.diff(self.content.as_widget());
        Some(overlay::Element::new(Box::new(VirtualWindowOverlay {
            content: &mut self.content,
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
    state: &'borrow mut State,
    close_message: Message,
    visual: Visual,
    viewport: Rectangle,
}

impl<Message> Overlay<Message, iced::Theme, iced::Renderer>
    for VirtualWindowOverlay<'_, '_, Message>
where
    Message: Clone,
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        let window_size = resolve_window_size(bounds, &self.visual, self.state.maximized);
        let position = resolve_window_position(
            bounds,
            window_size,
            &self.visual,
            self.state.maximized,
            self.state.offset,
        );
        let child_limits = Limits::new(window_size, window_size);
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut self.state.content_tree, renderer, &child_limits)
            .move_to(position);
        Node::with_children(bounds, vec![child])
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let root_bounds = layout.bounds();
        if self.visual.scrim.a > 0.0 {
            renderer.fill_quad(
                Quad {
                    bounds: root_bounds,
                    ..Default::default()
                },
                self.visual.scrim,
            );
        }

        let Some(window_layout) = layout.children().next() else {
            return;
        };
        let window_bounds = window_layout.bounds();
        let scale = 0.8 + 0.2 * ease_out_back(self.state.open_progress);
        let center = window_bounds.center();
        renderer.with_transformation(
            Transformation::translate(center.x, center.y)
                * Transformation::scale(scale)
                * Transformation::translate(-center.x, -center.y),
            |renderer| {
                renderer.fill_quad(
                    Quad {
                        bounds: window_bounds,
                        border: Border::default()
                            .rounded(self.visual.radius)
                            .width(1.0)
                            .color(self.visual.border),
                        shadow: Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
                            offset: Vector::new(0.0, 5.0),
                            blur_radius: 15.0,
                        },
                        ..Default::default()
                    },
                    self.visual.background,
                );
                renderer.with_layer(window_bounds, |renderer| {
                    self.content.as_widget().draw(
                        &self.state.content_tree,
                        renderer,
                        theme,
                        style,
                        window_layout,
                        cursor,
                        &self.viewport,
                    );
                });
            },
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
        let Some(window_layout) = layout.children().next() else {
            return;
        };

        self.content.as_widget_mut().update(
            &mut self.state.content_tree,
            event,
            window_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );

        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && self.state.open_progress < 1.0
        {
            let dt = self
                .state
                .last_frame
                .map(|last| now.duration_since(last).as_secs_f32().min(0.1))
                .unwrap_or(0.0);
            self.state.last_frame = Some(*now);
            self.state.open_progress =
                (self.state.open_progress + dt / self.visual.animation_duration).min(1.0);
            shell.request_redraw();
        }

        if shell.is_event_captured() {
            return;
        }

        let window_bounds = window_layout.bounds();
        let title_bounds = Rectangle {
            height: self.visual.title_height,
            ..window_bounds
        };
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position() else {
                    return;
                };
                if title_bounds.contains(position) {
                    let now = Instant::now();
                    let is_double = self.state.last_title_click.is_some_and(|(last, point)| {
                        now.duration_since(last).as_millis() <= DOUBLE_CLICK_MS
                            && point.distance(position) <= DOUBLE_CLICK_DISTANCE
                    });
                    if is_double {
                        self.state.maximized = !self.state.maximized;
                        self.state.offset = Vector::ZERO;
                        self.state.dragging = false;
                        self.state.last_cursor = None;
                        self.state.last_title_click = None;
                    } else {
                        self.state.dragging = !self.state.maximized;
                        self.state.last_cursor = Some(position);
                        self.state.last_title_click = Some((now, position));
                    }
                    shell.capture_event();
                    shell.invalidate_layout();
                    shell.request_redraw();
                } else if window_bounds.contains(position) {
                    shell.capture_event();
                } else if layout.bounds().contains(position) {
                    if self.visual.dismiss_on_scrim {
                        shell.publish(self.close_message.clone());
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if self.state.dragging => {
                if let Some(last) = self.state.last_cursor {
                    self.state.offset += *position - last;
                }
                self.state.last_cursor = Some(*position);
                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.state.dragging =>
            {
                self.state.dragging = false;
                self.state.last_cursor = None;
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let Some(window_layout) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        let child_interaction = self.content.as_widget().mouse_interaction(
            &self.state.content_tree,
            window_layout,
            cursor,
            &self.viewport,
            renderer,
        );
        if child_interaction != mouse::Interaction::None {
            return child_interaction;
        }

        let title_bounds = Rectangle {
            height: self.visual.title_height,
            ..window_layout.bounds()
        };
        if cursor.is_over(title_bounds) && !self.state.maximized {
            if self.state.dragging {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Grab
            }
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let child_layout = layout.children().next()?;
        self.content.as_widget_mut().overlay(
            &mut self.state.content_tree,
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

fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
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
    fn ease_out_back_starts_and_finishes_at_bounds() {
        assert_eq!(ease_out_back(0.0), 0.0);
        assert_eq!(ease_out_back(1.0), 1.0);
        assert!(ease_out_back(0.8) > 1.0, "reference curve should overshoot");
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
