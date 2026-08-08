//! A Fluent-style settings switch using the active theme accent.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, EventKind, LayoutMessage, WidgetDef, WidgetEvent};
use iced::widget::toggler;
use iced::{Background, Color, Element};

/// The layout name of this control.
pub const NAME: &str = "switch";

/// A standalone boolean switch.
#[derive(Default)]
pub struct Switch;

impl WidgetDef for Switch {
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
        let value = bool_prop(node.str_prop("value"), false);
        let id = ctx.qualify(&node.id);
        themed_toggler(
            value,
            move |next| {
                LayoutMessage::Event(WidgetEvent {
                    widget_id: id.clone(),
                    kind: EventKind::Toggled(next),
                })
            },
            ctx.theme,
        )
    }
}

/// Builds the shared NipaPlay-style switch used by settings rows.
pub(crate) fn themed_toggler<'a>(
    value: bool,
    on_toggle: impl Fn(bool) -> LayoutMessage + 'a,
    theme: &iced::Theme,
) -> Element<'a, LayoutMessage> {
    let is_dark = theme.extended_palette().is_dark;
    let primary = &theme.extended_palette().primary;
    let accent = primary.base.color;
    let accent_hover = primary.strong.color;
    let off = if is_dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.18)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.16)
    };
    let border = if is_dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.30)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.28)
    };

    toggler(value)
        .on_toggle(on_toggle)
        .size(22)
        .style(move |_theme, status| {
            let (is_toggled, hovered) = match status {
                toggler::Status::Active { is_toggled } => (is_toggled, false),
                toggler::Status::Hovered { is_toggled } => (is_toggled, true),
                toggler::Status::Disabled { is_toggled } => (is_toggled, false),
            };
            let background = if is_toggled {
                if hovered { accent_hover } else { accent }
            } else if hovered {
                Color::from_rgba(off.r, off.g, off.b, (off.a + 0.08).min(1.0))
            } else {
                off
            };
            toggler::Style {
                background: Background::Color(background),
                background_border_width: 1.0,
                background_border_color: if is_toggled { background } else { border },
                foreground: Background::Color(Color::WHITE),
                foreground_border_width: 0.0,
                foreground_border_color: Color::TRANSPARENT,
                text_color: None,
                border_radius: None,
                padding_ratio: 0.14,
            }
        })
        .into()
}

fn bool_prop(value: Option<&str>, fallback: bool) -> bool {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("true" | "1" | "yes" | "on") => true,
        Some("false" | "0" | "no" | "off") => false,
        _ => fallback,
    }
}
