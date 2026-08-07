//! The `icon_button` control: a bare icon that grows while hovered with a
//! smooth scale animation. No background container — just the icon.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "back", kind: "icon_button", area: "actions",
//!        props: { "icon": "←", "size": "18" })
//! ```
//!
//! The icon color follows the active iced theme's text color (white on dark,
//! black on light) — built into this control. `scale` and `duration_ms` can
//! be set per widget through layout props.
//!
//! The `icon` prop accepts a Material icon name (e.g. `"add"`, `"favorite"`)
//! via the embedded icon package; unknown names are rendered as raw text
//! glyphs (e.g. `"→"`), so both styles work.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{
    BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent,
};
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::Style;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{mouse, Clipboard, Renderer, Shell, Widget};
use iced::event::Event;
use iced::window;
use iced::{Color, Element, Length, Padding, Rectangle, Size, Transformation};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "icon_button";

/// Colors and animation parameters resolved at build time.
#[derive(Debug, Clone)]
struct Visual {
    icon_color: Color,
    scale: f32,
    duration: f32,
}

impl Visual {
    fn resolve(node: &LayoutWidget, theme: &iced::Theme) -> Self {
        Self {
            icon_color: theme.palette().text,
            scale: node
                .prop("scale")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(1.35),
            duration: node
                .prop("duration_ms")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(120.0)
                / 1000.0,
        }
    }
}

/// The framework control (the [`WidgetDef`]).
#[derive(Default)]
pub struct IconButton;

impl WidgetDef for IconButton {
    fn name(&self) -> &'static str {
        NAME
    }

    fn build<'a>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a>,
    ) -> Element<'a, LayoutMessage> {
        let icon_name = node.str_prop("icon").unwrap_or("");
        let glyph_size = node
            .prop("size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(20.0);

        let visual = Visual::resolve(node, ctx.theme);
        let id = ctx.qualify(&node.id);

        // Material icon names render with the embedded icon font; anything
        // else falls back to a raw text glyph.
        let content = match crate::icons::glyph(icon_name) {
            Some(glyph) => iced::widget::text(glyph).font(crate::icons::font()),
            None => iced::widget::text(icon_name),
        };
        let content = content.size(glyph_size).color(visual.icon_color);

        IconButtonWidget::new(
            content,
            visual,
        )
        .on_press(LayoutMessage::Event(WidgetEvent {
            widget_id: id,
            kind: EventKind::Pressed,
        }))
        .into()
    }
}

/// The custom iced widget behind `icon_button`: draws a rounded background
/// and scales the whole content around its center while hovered.
pub struct IconButtonWidget<'a, Message> {
    content: Element<'a, Message>,
    on_press: Option<Message>,
    visual: Visual,
    padding: f32,
}

impl<'a, Message> IconButtonWidget<'a, Message> {
    /// Creates a new icon button around the given content.
    fn new(content: impl Into<Element<'a, Message>>, visual: Visual) -> Self {
        Self {
            content: content.into(),
            on_press: None,
            visual,
            padding: 10.0,
        }
    }

    /// Sets the message produced when the button is pressed.
    fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }
}

/// Hover/animation state stored in the widget tree.
#[derive(Default)]
struct State {
    hovered: bool,
    progress: f32,
    last: Option<Instant>,
}

impl State {
    /// Smoothstep easing of the animation progress.
    fn eased(&self) -> f32 {
        let t = self.progress.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer>
    for IconButtonWidget<'a, Message>
where
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::container(node, Padding::from(self.padding))
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
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let center = bounds.center();
        // Never smaller than 1.0: resting/shrunk state is the natural size.
        let scale = (1.0 + (self.visual.scale - 1.0) * state.eased()).max(1.0);

        renderer.with_transformation(
            Transformation::translate(center.x, center.y)
                * Transformation::scale(scale)
                * Transformation::translate(-center.x, -center.y),
            |renderer| {
                if let Some(child) = layout.children().next() {
                    self.content.as_widget().draw(
                        &tree.children[0],
                        renderer,
                        theme,
                        style,
                        child,
                        cursor,
                        viewport,
                    );
                }
            },
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::new(State::default()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(&mut tree.children[0]);
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
        {
            let state = tree.state.downcast_mut::<State>();

            // Hover is derived from the cursor position, not from the
            // window-level `CursorEntered`/`CursorLeft` events (those fire
            // once when the cursor enters the window, not per widget).
            let over = cursor.is_over(layout.bounds());
            if over != state.hovered {
                state.hovered = over;
                shell.request_redraw();
            }

            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    if cursor.is_over(layout.bounds()) {
                        eprintln!(
                            "[icon_button] ButtonPressed over bounds {:?}",
                            layout.bounds()
                        );
                        if let Some(message) = &self.on_press {
                            eprintln!("[icon_button] publishing press message");
                            shell.publish(message.clone());
                        }
                    } else {
                        eprintln!(
                            "[icon_button] ButtonPressed NOT over bounds {:?}",
                            layout.bounds()
                        );
                    }
                }
                Event::Window(window::Event::RedrawRequested(now)) => {
                    let target = if state.hovered { 1.0 } else { 0.0 };
                    let remaining = target - state.progress;
                    if remaining.abs() > 0.0005 {
                        let dt = if let Some(last) = state.last {
                            now.duration_since(last).as_secs_f32()
                        } else {
                            state.last = Some(*now);
                            0.0
                        };
                        if dt > 0.0 {
                            state.last = Some(*now);
                            let step = dt / self.visual.duration;
                            state.progress = if remaining > 0.0 {
                                (state.progress + step).min(target)
                            } else {
                                (state.progress - step).max(target)
                            };
                        };
                        shell.request_redraw();
                    } else {
                        state.progress = target;
                        state.last = None;
                    }
                }
                _ => {}
            }
        }

        // Keep the inner content tree in sync.
        if let Some(child) = layout.children().next() {
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                child,
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
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

impl<'a, Message: Clone + 'a> From<IconButtonWidget<'a, Message>>
    for Element<'a, Message>
{
    fn from(widget: IconButtonWidget<'a, Message>) -> Self {
        Element::new(widget)
    }
}
