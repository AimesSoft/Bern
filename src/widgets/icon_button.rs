//! The `icon_button` control: a bare icon that grows while hovered with a
//! smooth scale animation. No background container — just the icon.
//!
//! Layout usage:
//!
//! ```ron
//! Widget(id: "back", kind: "icon_button", area: "actions",
//!        props: { "icon": "←", "size": "18", "tooltip": "返回" })
//! ```
//!
//! `tooltip` is required and must be non-empty. Hovering the icon shows it
//! below the button after a short delay. Layout validation fails with
//! [`crate::BuildError::MissingProp`] when the property is absent or blank.
//!
//! The icon color follows the active iced theme's text color (white on dark,
//! black on light) — built into this control. `scale` and `duration_ms` can
//! be set per widget through layout props. While hovered, the icon is
//! repainted with the theme accent color (and restored on leave).
//!
//! The `icon` prop accepts a Material icon name (e.g. `"add_rounded"`,
//! `"favorite_rounded"`) via the embedded icon package; unknown names are
//! rendered as raw text glyphs (e.g. `"→"`), so both styles work.
//!
//! Material icons render through the engine-level morph foundation
//! ([`crate::core::morph`]): when the `icon` prop changes, the old glyph
//! jelly-distorts into the new one instead of swapping instantly. Use
//! `morph_duration_ms` to tune the morph speed (default 420ms).

use crate::core::layout::Widget as LayoutWidget;
use crate::core::ui::PressOrigin;
use crate::core::widget::{
    BuildContext, BuildError, EventKind, LayoutMessage, WidgetDef, WidgetEvent,
};
use crate::widgets::morph_icon::MorphIconView;
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::Style;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::window;
use iced::{Border, Color, Element, Length, Padding, Rectangle, Size, Transformation};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The layout name of this control.
pub const NAME: &str = "icon_button";

const TOOLTIP_RADIUS: f32 = 6.0;

/// Rebuilds the button content for a given color (used to switch the icon
/// to the theme accent color while hovered).
pub type Rebuild<'a, Message> = Arc<dyn Fn(Color) -> Element<'a, Message> + 'a>;

/// Colors and animation parameters resolved at build time.
#[derive(Debug, Clone)]
pub struct Visual {
    /// The color of the icon (and any other content).
    pub icon_color: Color,
    /// The theme accent color, used when the button is `selected`.
    pub accent: Color,
    /// Hover scale factor (>= 1.0).
    pub scale: f32,
    /// Scale animation duration in seconds.
    pub duration: f32,
}

impl Visual {
    /// Resolves visuals with the icon-button defaults (scale 1.35, 120 ms).
    fn resolve(node: &LayoutWidget, theme: &iced::Theme) -> Self {
        Self::resolve_with(node, theme, 1.35, 120.0)
    }

    /// Resolves visuals from layout props, with configurable defaults
    /// (used by `action_button`, which shares this scale/press core).
    pub fn resolve_with(
        node: &LayoutWidget,
        theme: &iced::Theme,
        default_scale: f32,
        default_duration_ms: f32,
    ) -> Self {
        Self {
            icon_color: theme.palette().text,
            accent: theme.extended_palette().primary.base.color,
            scale: node
                .prop("scale")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(default_scale),
            duration: node
                .prop("duration_ms")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(default_duration_ms)
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

    fn interactive(&self) -> bool {
        true
    }

    fn validate(&self, node: &LayoutWidget) -> Result<(), BuildError> {
        if node
            .str_prop("tooltip")
            .is_some_and(|tooltip| !tooltip.trim().is_empty())
        {
            Ok(())
        } else {
            Err(BuildError::MissingProp {
                widget: node.id.clone(),
                prop: "tooltip".into(),
            })
        }
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let icon_name = node.str_prop("icon").unwrap_or("");
        let tooltip_text = node
            .str_prop("tooltip")
            .expect("icon_button tooltip was validated before build");
        let glyph_size = node
            .prop("size")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(20.0);

        let visual = Visual::resolve(node, ctx.theme);
        let id = ctx.qualify(&node.id);

        // Material icon names render with the embedded icon font; anything
        // else falls back to a raw text glyph. Following the theme reveal is
        // automatic (the registry wraps this control). Material icons use
        // the vector morph foundation: switching icons jelly-morphs.
        let morph_duration = node
            .prop("morph_duration_ms")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(420.0)
            / 1000.0;
        // 悬浮时重建内容并换成主题强调色：图标/文字的颜色在悬浮中变成
        // 强调色，移开恢复文字色。
        let rebuild: Rebuild<'a, LayoutMessage> = Arc::new(move |color| {
            match crate::icons::glyph(icon_name) {
                Some(glyph) => {
                    MorphIconView::new(glyph, color, glyph_size, morph_duration).into()
                }
                None => iced::widget::text(icon_name).color(color).into(),
            }
        });
        let content = rebuild(visual.icon_color);

        let button: Element<'a, LayoutMessage> =
            IconButtonWidget::new(content, rebuild, visual, ctx.press_origin.clone())
                .on_press(LayoutMessage::Event(WidgetEvent {
                    widget_id: id,
                    kind: EventKind::Pressed,
                }))
                .into();
        let tooltip_style = tooltip_style(ctx.theme);
        let tooltip_content = iced::widget::container(
            iced::widget::text(tooltip_text).size(13),
        )
        // Horizontal room keeps short labels readable; vertical padding is
        // deliberately compact so the hint does not look like a button.
        .padding([3, 9])
        .style(move |_theme| tooltip_style);

        iced::widget::tooltip(
            button,
            tooltip_content,
            iced::widget::tooltip::Position::Bottom,
        )
        .gap(6)
        // The styled inner container owns its asymmetric padding.
        .padding(0)
        .delay(Duration::from_millis(350))
        .into()
    }
}

pub(crate) fn tooltip_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let (background, text, border) = if theme.extended_palette().is_dark {
        (
            Color::from_rgb8(48, 48, 52),
            Color::WHITE,
            Color::from_rgba(1.0, 1.0, 1.0, 0.12),
        )
    } else {
        (
            Color::from_rgb8(250, 250, 252),
            Color::from_rgb8(20, 20, 22),
            Color::from_rgba(0.0, 0.0, 0.0, 0.12),
        )
    };

    iced::widget::container::Style {
        text_color: Some(text),
        background: Some(background.into()),
        border: Border::default()
            .rounded(TOOLTIP_RADIUS)
            .width(1.0)
            .color(border),
        ..Default::default()
    }
}

/// The custom iced widget behind `icon_button`: draws a bare icon and scales
/// the whole content around its center while hovered. Presses record their
/// position into the shared [`PressOrigin`], so backgrounds can reveal color
/// changes from this button. While a theme reveal runs, the button subscribes
/// changes from this button. Following the theme reveal is handled
/// automatically by the engine-level wrapper.
pub struct IconButtonWidget<'a, Message> {
    content: Element<'a, Message>,
    /// Rebuilds the content with a new color (hover → accent).
    rebuild: Rebuild<'a, Message>,
    on_press: Option<Message>,
    visual: Visual,
    padding: f32,
    press_origin: PressOrigin,
}

impl<'a, Message> IconButtonWidget<'a, Message> {
    /// Creates a new hover-scale button around the given content.
    pub fn new(
        content: impl Into<Element<'a, Message>>,
        rebuild: Rebuild<'a, Message>,
        visual: Visual,
        press_origin: PressOrigin,
    ) -> Self {
        Self {
            content: content.into(),
            rebuild,
            on_press: None,
            visual,
            padding: 10.0,
            press_origin,
        }
    }

    /// Sets the message produced when the button is pressed.
    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    /// Sets the hit-area padding (visual content is not padded).
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
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

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for IconButtonWidget<'a, Message>
where
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
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
        // 走 `Tree::diff_children`（带 tag 检查），类型变化时重建子树，
        // 否则切页/换图标后 downcast 会崩溃。
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

            // Hover is derived from the cursor position, not from the
            // window-level `CursorEntered`/`CursorLeft` events (those fire
            // once when the cursor enters the window, not per widget).
            let over = cursor.is_over(layout.bounds());
            if over != state.hovered {
                state.hovered = over;
                // 悬浮中图标/文字换成主题强调色，移开恢复文字色。
                self.content = (self.rebuild)(if over {
                    self.visual.accent
                } else {
                    self.visual.icon_color
                });
                shell.request_redraw();
            }

            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    if cursor.is_over(layout.bounds()) {
                        eprintln!(
                            "[icon_button] ButtonPressed over bounds {:?}",
                            layout.bounds()
                        );
                        let center = layout.bounds().center();
                        self.press_origin.record((center.x, center.y));
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
                        // 空闲后恢复的首帧与上一个时间戳可能相隔很久；直接
                        // 用它算 dt 会一步补完动画。超过 0.1s 视为动画起点。
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

impl<'a, Message: Clone + 'a> From<IconButtonWidget<'a, Message>> for Element<'a, Message> {
    fn from(widget: IconButtonWidget<'a, Message>) -> Self {
        Element::new(widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_palette_follows_light_and_dark_modes() {
        let light = tooltip_style(&iced::Theme::Light);
        let dark = tooltip_style(&iced::Theme::Dark);

        assert_eq!(light.text_color, Some(Color::from_rgb8(20, 20, 22)));
        assert_eq!(dark.text_color, Some(Color::WHITE));
        assert_ne!(light.background, dark.background);
        assert_eq!(light.border.radius, TOOLTIP_RADIUS.into());
        assert_eq!(dark.border.radius, TOOLTIP_RADIUS.into());
    }
}
