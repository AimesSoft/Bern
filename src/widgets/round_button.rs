//! The `round_button` control: a rounded-rectangle button, ported from
//! nipaplay's large-screen focusable action.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "lib_sync", kind: "round_button", area: "root",
//!        props: { "icon": "sync_rounded", "label": "同步" })
//! ```
//!
//! Appearance follows nipaplay:
//!
//! - a fixed rounded-8 surface: white 82% (light) / white 10% (dark) with a
//!   1 px text-color stroke, so the container reads as a button;
//! - hovering swaps the stroke for a 2 px accent one while the **content**
//!   (icon + label) scales 1.035 inside the fixed surface (140 ms
//!   ease-out-cubic);
//! - content color is white (dark) / black87 (light); label is w800;
//! - pressing records the press origin and publishes `(id, Pressed)`.
//!
//! While hovered, the icon and label are repainted with the theme accent
//! color (and restored on leave).
//!
//! The `icon` prop accepts a Material icon name (with the vector morph
//! foundation); unknown names render as raw text glyphs.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::ui::PressOrigin;
use crate::core::widget::{
    BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent, size_lengths,
};
use crate::widgets::morph_icon::MorphIconView;
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::Style;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::window;
use iced::{Background, Border, Color, Element, Length, Padding, Rectangle, Size, Transformation};
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "round_button";

/// Colors and animation parameters resolved at build time.
#[derive(Debug, Clone)]
struct Visual {
    fill: Color,
    border_idle: Color,
    accent: Color,
    content_color: Color,
    scale: f32,
    duration: f32,
    font_size: f32,
    icon_size: f32,
    /// [vertical, horizontal] padding of the fixed surface.
    padding: [f32; 2],
}

impl Visual {
    fn resolve(node: &LayoutWidget, theme: &iced::Theme) -> Self {
        let is_dark = theme.extended_palette().is_dark;
        let with_alpha = |c: Color, a: f32| Color::from_rgba(c.r, c.g, c.b, a);
        Self {
            fill: if is_dark {
                with_alpha(Color::WHITE, 0.10)
            } else {
                with_alpha(Color::WHITE, 0.82)
            },
            border_idle: with_alpha(theme.palette().text, 0.12),
            accent: theme.extended_palette().primary.base.color,
            content_color: if is_dark {
                Color::WHITE
            } else {
                with_alpha(Color::BLACK, 0.87)
            },
            scale: node
                .prop("scale")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(1.035),
            duration: node
                .prop("duration_ms")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(140.0)
                / 1000.0,
            font_size: node
                .prop("font_size")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(15.0),
            icon_size: node
                .prop("icon_size")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(21.0),
            padding: [13.0, 16.0],
        }
    }
}

/// The framework control (the [`WidgetDef`]).
#[derive(Default)]
pub struct RoundButton;

impl WidgetDef for RoundButton {
    fn name(&self) -> &'static str {
        NAME
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
        let label = node.str_prop("label").unwrap_or("");
        let icon_name = node.str_prop("icon").unwrap_or("");
        let visual = Visual::resolve(node, ctx.theme);
        let morph_duration = node
            .prop("morph_duration_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(420.0)
            / 1000.0;
        let id = ctx.qualify(&node.id);

        // 悬浮时重建内容并换成主题强调色（图标 + 文字一起变）。
        let rebuild: crate::widgets::icon_button::Rebuild<'a, LayoutMessage> =
            std::sync::Arc::new(move |color| {
                // 内容：可选图标 + 粗体标签（nipaplay 的 21px 图标 + 8px + 15px）。
                let mut children: Vec<Element<'_, LayoutMessage>> = Vec::new();
                match crate::icons::glyph(icon_name) {
                    Some(glyph) => children.push(
                        MorphIconView::new(glyph, color, visual.icon_size, morph_duration).into(),
                    ),
                    None if !icon_name.is_empty() => children.push(
                        iced::widget::text(icon_name)
                            .size(visual.icon_size)
                            .color(color)
                            .into(),
                    ),
                    None => {}
                }
                children.push(
                    iced::widget::text(label)
                        .size(visual.font_size)
                        .font(crate::fonts::bold_font())
                        .color(color)
                        .into(),
                );
                iced::widget::Row::with_children(children).spacing(8).into()
            });
        let content = rebuild(visual.content_color);

        let (width, _height) = size_lengths(size);
        RoundButtonWidget::new(content, rebuild, visual, ctx.press_origin.clone())
            .width(width)
            .on_press(LayoutMessage::Event(WidgetEvent {
                widget_id: id,
                kind: EventKind::Pressed,
            }))
            .into()
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
    /// Ease-out-cubic easing of the animation progress.
    fn eased(&self) -> f32 {
        let t = self.progress.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}

/// The custom iced widget behind `round_button`: a fixed rounded-8 surface
/// whose content scales on hover while an accent stroke appears.
pub struct RoundButtonWidget<'a, Message> {
    content: Element<'a, Message>,
    /// Rebuilds the content with a new color (hover → accent).
    rebuild: crate::widgets::icon_button::Rebuild<'a, Message>,
    on_press: Option<Message>,
    visual: Visual,
    width: Option<Length>,
    press_origin: PressOrigin,
}

impl<'a, Message> RoundButtonWidget<'a, Message> {
    fn new(
        content: impl Into<Element<'a, Message>>,
        rebuild: crate::widgets::icon_button::Rebuild<'a, Message>,
        visual: Visual,
        press_origin: PressOrigin,
    ) -> Self {
        Self {
            content: content.into(),
            rebuild,
            on_press: None,
            visual,
            width: None,
            press_origin,
        }
    }

    /// Sets an explicit width for the surface.
    fn width(mut self, width: Option<Length>) -> Self {
        self.width = width;
        self
    }

    /// Sets the message produced when the button is pressed.
    fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for RoundButtonWidget<'a, Message>
where
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        Size::new(
            self.width.unwrap_or(Length::Shrink),
            Length::Shrink,
        )
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let padding = Padding::from(self.visual.padding);
        match self.width {
            Some(width) => {
                // 固定宽度表面：内容靠左，padding 提供内边距。
                let content = Node::container(node, padding);
                let size = limits.resolve(width, Length::Shrink, content.size());
                Node::with_children(Size::new(size.width, content.size().height), vec![content])
            }
            None => Node::container(node, padding),
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        // 固定表面：圆角 8 填充；默认一条细描边让「有容器」一眼可辨，
        // 悬浮时换成 2px 强调色描边。
        renderer.fill_quad(
            iced::advanced::renderer::Quad {
                bounds,
                border: Border::default()
                    .rounded(8)
                    .width(if state.hovered { 2.0 } else { 1.0 })
                    .color(if state.hovered {
                        self.visual.accent
                    } else {
                        self.visual.border_idle
                    }),
                ..Default::default()
            },
            Background::Color(self.visual.fill),
        );

        // 只有内容缩放，表面固定（nipaplay 的 AnimatedScale 在表面内部）。
        if let Some(child) = layout.children().next() {
            let center = child.bounds().center();
            let scale = (1.0 + (self.visual.scale - 1.0) * state.eased()).max(1.0);
            renderer.with_transformation(
                Transformation::translate(center.x, center.y)
                    * Transformation::scale(scale)
                    * Transformation::translate(-center.x, -center.y),
                |renderer| {
                    self.content.as_widget().draw(
                        &tree.children[0],
                        renderer,
                        _theme,
                        &Style {
                            text_color: if state.hovered {
                                self.visual.accent
                            } else {
                                self.visual.content_color
                            },
                        },
                        child,
                        cursor,
                        viewport,
                    );
                },
            );
        }
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
        {
            let state = tree.state.downcast_mut::<State>();
            let over = cursor.is_over(layout.bounds());
            if over != state.hovered {
                state.hovered = over;
                // 悬浮中图标/文字换成主题强调色，移开恢复内容色。
                self.content = (self.rebuild)(if over {
                    self.visual.accent
                } else {
                    self.visual.content_color
                });
                shell.request_redraw();
            }

            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    if cursor.is_over(layout.bounds()) {
                        let center = layout.bounds().center();
                        self.press_origin.record((center.x, center.y));
                        if let Some(message) = &self.on_press {
                            shell.publish(message.clone());
                        }
                    }
                }
                Event::Window(window::Event::RedrawRequested(now)) => {
                    let target = if state.hovered { 1.0 } else { 0.0 };
                    let remaining = target - state.progress;
                    if remaining.abs() > 0.0005 {
                        let dt = match state.last {
                            Some(last) => {
                                let elapsed = now.duration_since(last).as_secs_f32();
                                if elapsed > 0.1 { 0.0 } else { elapsed }
                            }
                            None => 0.0,
                        };
                        state.last = Some(*now);
                        let step = dt / self.visual.duration;
                        state.progress = if remaining > 0.0 {
                            (state.progress + step).min(target)
                        } else {
                            (state.progress - step).max(target)
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a> From<RoundButtonWidget<'a, Message>> for Element<'a, Message> {
    fn from(widget: RoundButtonWidget<'a, Message>) -> Self {
        Element::new(widget)
    }
}
