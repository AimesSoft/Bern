//! NipaPlay-style native window caption controls.
//!
//! The control is a direct port of Flutter NipaPlay's
//! `WindowControlButtons`: each rectangular button is 46×40 px, the full
//! group is 138×40 px, and background changes animate for 90 ms with an
//! ease-out-cubic curve. The close button uses Windows' familiar
//! `#E81123` hover red and `#C50F1F` pressed red.
//!
//! ```ron
//! Widget(id: "window_controls", kind: "window_controls", area: "chrome")
//! Widget(id: "dialog_close", kind: "window_controls", area: "title",
//!        props: { "close_only": "true" })
//! Widget(id: "app_chrome", kind: "window_controls", area: "chrome",
//!        props: {
//!            "leading_items": "dark_mode_rounded:切换主题:theme_toggle:20,settings_rounded:设置:settings:20",
//!        })
//! ```
//!
//! Presses publish [`EventKind::WindowControl`] with the widget's qualified
//! id. Applications can return [`crate::perform_window_control_action`] for
//! native app windows, or interpret `Close` locally when using the
//! close-only variant in a virtual window.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::ui::PressOrigin;
use crate::core::widget::{
    BuildContext, BuildError, EventKind, LayoutMessage, WidgetDef, WidgetEvent,
};
use crate::core::window::WindowControlAction;
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{Quad, Style};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer, Shell, Widget, mouse};
use iced::event::Event;
use iced::window;
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Size};
use std::collections::HashSet;
use std::time::Instant;

/// The layout name of this control.
pub const NAME: &str = "window_controls";

/// Width of one NipaPlay caption button.
pub const BUTTON_WIDTH: f32 = 46.0;
/// Height of one NipaPlay caption button.
pub const BUTTON_HEIGHT: f32 = 40.0;
/// Width of the complete minimize/maximize/close group.
pub const TOTAL_WIDTH: f32 = BUTTON_WIDTH * 3.0;

const ANIMATION_DURATION: f32 = 0.09;
const CLOSE_HOVER: Color = Color::from_rgb8(0xE8, 0x11, 0x23);
const CLOSE_PRESSED: Color = Color::from_rgb8(0xC5, 0x0F, 0x1F);

/// The runtime-layout control definition.
#[derive(Default)]
pub struct WindowControls;

impl WidgetDef for WindowControls {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn validate(&self, node: &LayoutWidget) -> Result<(), BuildError> {
        validate_bool_prop(node, "close_only")?;
        validate_bool_prop(node, "maximized")?;
        parse_leading_items(node.str_prop("leading_items").unwrap_or("")).map_err(|reason| {
            BuildError::BadProp {
                widget: node.id.clone(),
                prop: "leading_items".into(),
                value: reason,
            }
        })?;
        Ok(())
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::core::layout::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let close_only = bool_prop(node, "close_only", false);
        let maximized = bool_prop(node, "maximized", false);
        let visual = Visual::resolve(ctx.theme);
        let tooltip_style = crate::widgets::icon_button::tooltip_style(ctx.theme);
        let id = ctx.qualify(&node.id);
        let leading_items = if close_only {
            Vec::new()
        } else {
            parse_leading_items(node.str_prop("leading_items").unwrap_or(""))
                .expect("window_controls leading_items were validated before build")
        };
        let mut buttons = Vec::with_capacity(if close_only {
            1
        } else {
            leading_items.len() + 3
        });

        if !close_only {
            for item in &leading_items {
                buttons.push(caption_button(
                    icon_glyph(&item.icon),
                    item.icon_size,
                    item.tooltip.clone(),
                    false,
                    visual,
                    tooltip_style,
                    ctx.press_origin.clone(),
                    LayoutMessage::Event(WidgetEvent {
                        widget_id: ctx.qualify(&item.key),
                        kind: EventKind::Pressed,
                    }),
                ));
            }

            buttons.push(caption_button(
                crate::icons::glyph("remove_rounded")
                    .expect("remove_rounded is in the Material icon table"),
                22.0,
                "最小化",
                false,
                visual,
                tooltip_style,
                ctx.press_origin.clone(),
                LayoutMessage::Event(WidgetEvent {
                    widget_id: id.clone(),
                    kind: EventKind::WindowControl(WindowControlAction::Minimize),
                }),
            ));

            let (icon, icon_size, tooltip) = if maximized {
                ("filter_none_rounded", 18.0, "还原")
            } else {
                ("crop_square_rounded", 22.0, "最大化")
            };
            buttons.push(caption_button(
                crate::icons::glyph(icon).expect("caption icon is in the Material icon table"),
                icon_size,
                tooltip,
                false,
                visual,
                tooltip_style,
                ctx.press_origin.clone(),
                LayoutMessage::Event(WidgetEvent {
                    widget_id: id.clone(),
                    kind: EventKind::WindowControl(WindowControlAction::ToggleMaximize),
                }),
            ));
        }

        buttons.push(caption_button(
            crate::icons::glyph("close_rounded")
                .expect("close_rounded is in the Material icon table"),
            22.0,
            "关闭",
            true,
            visual,
            tooltip_style,
            ctx.press_origin.clone(),
            LayoutMessage::Event(WidgetEvent {
                widget_id: id,
                kind: EventKind::WindowControl(WindowControlAction::Close),
            }),
        ));

        let button_count = if close_only {
            1
        } else {
            leading_items.len() + 3
        };
        iced::widget::Row::with_children(buttons)
            .spacing(0)
            .width(BUTTON_WIDTH * button_count as f32)
            .height(BUTTON_HEIGHT)
            .into()
    }
}

fn caption_button<'a>(
    glyph: char,
    icon_size: f32,
    tooltip: impl Into<String>,
    is_close: bool,
    visual: Visual,
    tooltip_style: iced::widget::container::Style,
    press_origin: PressOrigin,
    message: LayoutMessage,
) -> Element<'a, LayoutMessage> {
    let icon = iced::widget::text(glyph)
        .font(crate::icons::font())
        .size(icon_size);
    let button: Element<'a, LayoutMessage> =
        CaptionButton::new(icon, visual, is_close, press_origin, message).into();

    let tooltip_content = iced::widget::container(iced::widget::text(tooltip.into()).size(13))
        .padding([3, 9])
        .style(move |_theme| tooltip_style);

    iced::widget::tooltip(
        button,
        tooltip_content,
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(0)
    .delay(std::time::Duration::from_millis(350))
    .into()
}

#[derive(Debug, Clone, Copy)]
struct Visual {
    idle_icon: Color,
    hover_background: Color,
    pressed_background: Color,
}

impl Visual {
    fn resolve(theme: &iced::Theme) -> Self {
        let dark = theme.extended_palette().is_dark;
        Self {
            idle_icon: if dark {
                Color::WHITE
            } else {
                Color::from_rgba(0.0, 0.0, 0.0, 0.87)
            },
            hover_background: if dark {
                Color::from_rgba(1.0, 1.0, 1.0, 0.12)
            } else {
                Color::from_rgba(0.0, 0.0, 0.0, 0.08)
            },
            pressed_background: if dark {
                Color::from_rgba(1.0, 1.0, 1.0, 0.18)
            } else {
                Color::from_rgba(0.0, 0.0, 0.0, 0.14)
            },
        }
    }

    fn background(self, is_close: bool, hovered: bool, pressed: bool) -> Color {
        if is_close && (hovered || pressed) {
            if pressed { CLOSE_PRESSED } else { CLOSE_HOVER }
        } else if pressed {
            self.pressed_background
        } else if hovered {
            self.hover_background
        } else {
            Color::TRANSPARENT
        }
    }

    fn icon(self, is_close: bool, active: bool) -> Color {
        if is_close && active {
            Color::WHITE
        } else {
            self.idle_icon
        }
    }
}

/// One 46×40 caption button with NipaPlay's animated rectangular background.
struct CaptionButton<'a, Message> {
    content: Element<'a, Message>,
    visual: Visual,
    is_close: bool,
    press_origin: PressOrigin,
    on_press: Message,
}

impl<'a, Message> CaptionButton<'a, Message> {
    fn new(
        content: impl Into<Element<'a, Message>>,
        visual: Visual,
        is_close: bool,
        press_origin: PressOrigin,
        on_press: Message,
    ) -> Self {
        Self {
            content: content.into(),
            visual,
            is_close,
            press_origin,
            on_press,
        }
    }
}

#[derive(Debug)]
struct State {
    hovered: bool,
    pressed: bool,
    background: Color,
    from: Color,
    target: Color,
    progress: f32,
    last_frame: Option<Instant>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hovered: false,
            pressed: false,
            background: Color::TRANSPARENT,
            from: Color::TRANSPARENT,
            target: Color::TRANSPARENT,
            progress: 1.0,
            last_frame: None,
        }
    }
}

impl State {
    fn retarget(&mut self, target: Color) {
        if self.target == target {
            return;
        }
        self.from = self.background;
        self.target = target;
        self.progress = 0.0;
        self.last_frame = None;
    }

    fn update_target(&mut self, visual: Visual, is_close: bool) {
        self.retarget(visual.background(is_close, self.hovered, self.pressed));
    }

    fn animate(&mut self, now: Instant) -> bool {
        if self.progress >= 1.0 {
            self.background = self.target;
            self.last_frame = None;
            return false;
        }

        let dt = match self.last_frame {
            Some(last) => {
                let elapsed = now.duration_since(last).as_secs_f32();
                if elapsed > 0.1 { 0.0 } else { elapsed }
            }
            None => 0.0,
        };
        self.last_frame = Some(now);
        self.progress = (self.progress + dt / ANIMATION_DURATION).min(1.0);
        self.background = lerp_color(self.from, self.target, ease_out_cubic(self.progress));
        self.progress < 1.0
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for CaptionButton<'a, Message>
where
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(BUTTON_WIDTH), Length::Fixed(BUTTON_HEIGHT))
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let size = limits.resolve(
            Length::Fixed(BUTTON_WIDTH),
            Length::Fixed(BUTTON_HEIGHT),
            Size::new(BUTTON_WIDTH, BUTTON_HEIGHT),
        );
        let child_limits = Limits::new(Size::ZERO, size).loose();
        let mut child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &child_limits);
        child.move_to_mut(Point::new(
            ((size.width - child.size().width) / 2.0).max(0.0),
            ((size.height - child.size().height) / 2.0).max(0.0),
        ));
        Node::with_children(size, vec![child])
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
        let state = tree.state.downcast_mut::<State>();
        let over = cursor.is_over(layout.bounds());
        if over != state.hovered {
            state.hovered = over;
            state.update_target(self.visual, self.is_close);
            shell.request_redraw();
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if over => {
                state.pressed = true;
                state.update_target(self.visual, self.is_close);
                let center = layout.bounds().center();
                self.press_origin.record((center.x, center.y));
                shell.capture_event();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.pressed => {
                state.pressed = false;
                state.update_target(self.visual, self.is_close);
                if over {
                    shell.publish(self.on_press.clone());
                }
                shell.capture_event();
                shell.request_redraw();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if state.animate(*now) {
                    shell.request_redraw();
                }
            }
            _ => {}
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
        renderer.fill_quad(
            Quad {
                bounds: layout.bounds(),
                border: Border::default(),
                ..Default::default()
            },
            Background::Color(state.background),
        );

        if let Some(child) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &Style {
                    text_color: self
                        .visual
                        .icon(self.is_close, state.hovered || state.pressed),
                },
                child,
                cursor,
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
            layout.children().next()?,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a> From<CaptionButton<'a, Message>> for Element<'a, Message> {
    fn from(button: CaptionButton<'a, Message>) -> Self {
        Element::new(button)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LeadingItem {
    icon: String,
    tooltip: String,
    key: String,
    icon_size: f32,
}

/// Returns the interaction ids declared by `leading_items`.
///
/// Each comma-separated item uses `icon:tooltip:key[:size]`. Malformed input
/// returns no keys here; the control's layout validation reports the precise
/// [`BuildError::BadProp`] before a widget tree is built.
pub fn leading_item_keys(value: &str) -> Vec<String> {
    parse_leading_items(value)
        .map(|items| items.into_iter().map(|item| item.key).collect())
        .unwrap_or_default()
}

fn parse_leading_items(value: &str) -> Result<Vec<LeadingItem>, String> {
    let mut items = Vec::new();
    let mut keys = HashSet::new();

    for (index, raw) in value.split(',').enumerate() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let fields: Vec<&str> = raw.split(':').map(str::trim).collect();
        if !matches!(fields.len(), 3 | 4) {
            return Err(format!(
                "item {} must use icon:tooltip:key[:size]",
                index + 1
            ));
        }
        let (icon, tooltip, key) = (fields[0], fields[1], fields[2]);
        if icon.is_empty() || tooltip.is_empty() || key.is_empty() {
            return Err(format!(
                "item {} requires a non-empty icon, tooltip, and key",
                index + 1
            ));
        }
        if !keys.insert(key.to_string()) {
            return Err(format!("duplicate leading item key `{key}`"));
        }
        let icon_size = if let Some(value) = fields.get(3) {
            value
                .parse::<f32>()
                .ok()
                .filter(|size| size.is_finite() && *size > 0.0 && *size <= 128.0)
                .ok_or_else(|| format!("item {} has an invalid icon size", index + 1))?
        } else {
            22.0
        };
        items.push(LeadingItem {
            icon: icon.into(),
            tooltip: tooltip.into(),
            key: key.into(),
            icon_size,
        });
    }

    Ok(items)
}

fn icon_glyph(icon: &str) -> char {
    crate::icons::glyph(icon)
        .or_else(|| icon.chars().next())
        .expect("leading item icons are validated as non-empty")
}

fn bool_prop(node: &LayoutWidget, key: &str, default: bool) -> bool {
    node.str_prop(key).and_then(parse_bool).unwrap_or(default)
}

fn validate_bool_prop(node: &LayoutWidget, key: &str) -> Result<(), BuildError> {
    let Some(value) = node.str_prop(key) else {
        return Ok(());
    };
    if parse_bool(value).is_some() {
        Ok(())
    } else {
        Err(BuildError::BadProp {
            widget: node.id.clone(),
            prop: key.into(),
            value: value.into(),
        })
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    Color::from_rgba(
        from.r + (to.r - from.r) * t,
        from.g + (to.g - from.g) * t,
        from.b + (to.b - from.b) * t,
        from.a + (to.a - from.a) * t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn node(props: &[(&str, &str)]) -> LayoutWidget {
        LayoutWidget {
            id: "caption".into(),
            kind: NAME.into(),
            area: "root".into(),
            z: 0,
            size: None,
            props: props
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn close_only_and_maximized_are_strict_boolean_props() {
        let control = WindowControls;
        assert!(control.validate(&node(&[])).is_ok());
        assert!(
            control
                .validate(&node(&[("close_only", "true"), ("maximized", "1")]))
                .is_ok()
        );
        assert!(matches!(
            control.validate(&node(&[("close_only", "sometimes")])),
            Err(BuildError::BadProp { prop, .. }) if prop == "close_only"
        ));
    }

    #[test]
    fn palette_matches_flutter_nipaplay() {
        let light = Visual::resolve(&iced::Theme::Light);
        let dark = Visual::resolve(&iced::Theme::Dark);

        assert_eq!(light.background(true, true, false), CLOSE_HOVER);
        assert_eq!(dark.background(true, true, true), CLOSE_PRESSED);
        assert_eq!(light.background(false, true, false).a, 0.08);
        assert_eq!(light.background(false, true, true).a, 0.14);
        assert_eq!(dark.background(false, true, false).a, 0.12);
        assert_eq!(dark.background(false, true, true).a, 0.18);
        assert_eq!(light.icon(true, true), Color::WHITE);
    }

    #[test]
    fn caption_metrics_match_flutter_nipaplay() {
        assert_eq!(BUTTON_WIDTH, 46.0);
        assert_eq!(BUTTON_HEIGHT, 40.0);
        assert_eq!(TOTAL_WIDTH, 138.0);
        assert_eq!(ANIMATION_DURATION, 0.09);
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
    }

    #[test]
    fn leading_items_parse_icons_tooltips_keys_and_optional_sizes() {
        let items = parse_leading_items(
            "dark_mode_rounded:切换主题:theme_toggle:20,settings_rounded:设置:settings",
        )
        .expect("valid leading items");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].key, "theme_toggle");
        assert_eq!(items[0].icon_size, 20.0);
        assert_eq!(items[1].key, "settings");
        assert_eq!(items[1].icon_size, 22.0);
        assert_eq!(
            leading_item_keys("dark_mode_rounded:切换主题:theme_toggle"),
            vec!["theme_toggle"]
        );
        assert!(parse_leading_items("icon:tip:duplicate,icon:tip:duplicate").is_err());
    }
}
