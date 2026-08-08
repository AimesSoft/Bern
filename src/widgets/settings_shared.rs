//! Shared drawing primitives for the granular settings-row controls.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildError, EventKind, LayoutMessage, WidgetEvent};
use iced::widget::{Column, Row, Space, button, container};
use iced::{Background, Border, Color, Element, Length};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SettingsPalette {
    pub(crate) text: Color,
    pub(crate) secondary: Color,
    pub(crate) icon: Color,
    pub(crate) hover: Color,
    pub(crate) chip: Color,
    pub(crate) divider: Color,
    pub(crate) accent: Color,
}

impl SettingsPalette {
    pub(crate) fn resolve(theme: &iced::Theme) -> Self {
        let dark = theme.extended_palette().is_dark;
        let text = if dark {
            Color::WHITE
        } else {
            Color::from_rgb8(0x17, 0x1A, 0x22)
        };
        let alpha = |color: Color, value: f32| Color::from_rgba(color.r, color.g, color.b, value);
        Self {
            text,
            secondary: alpha(text, 0.70),
            icon: alpha(text, 0.70),
            hover: alpha(text, if dark { 0.06 } else { 0.045 }),
            chip: if dark {
                Color::from_rgba(1.0, 1.0, 1.0, 0.08)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.72)
            },
            divider: alpha(text, 0.12),
            accent: theme.extended_palette().primary.base.color,
        }
    }
}

pub(crate) fn validate_common(node: &LayoutWidget) -> Result<(), BuildError> {
    if node
        .str_prop("title")
        .is_some_and(|title| !title.trim().is_empty())
    {
        Ok(())
    } else {
        Err(BuildError::MissingProp {
            widget: node.id.clone(),
            prop: "title".into(),
        })
    }
}

pub(crate) fn static_row<'a>(
    node: &'a LayoutWidget,
    trailing: Element<'a, LayoutMessage>,
    palette: SettingsPalette,
) -> Element<'a, LayoutMessage> {
    container(base_row(node, trailing, palette))
        .padding([14, 16])
        .width(Length::Fill)
        .into()
}

pub(crate) fn action_row<'a>(
    node: &'a LayoutWidget,
    trailing: Element<'a, LayoutMessage>,
    message: LayoutMessage,
    palette: SettingsPalette,
) -> Element<'a, LayoutMessage> {
    button(base_row(node, trailing, palette))
        .on_press(message)
        .padding([14, 16])
        .width(Length::Fill)
        .style(move |_theme, status| iced::widget::button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(palette.hover)),
                button::Status::Pressed => Some(Background::Color(Color::from_rgba(
                    palette.text.r,
                    palette.text.g,
                    palette.text.b,
                    (palette.hover.a + 0.04).min(1.0),
                ))),
                _ => None,
            },
            text_color: palette.text,
            border: Border::default(),
            ..Default::default()
        })
        .into()
}

pub(crate) fn base_row<'a>(
    node: &'a LayoutWidget,
    trailing: Element<'a, LayoutMessage>,
    palette: SettingsPalette,
) -> Element<'a, LayoutMessage> {
    let icon_name = node.str_prop("icon").unwrap_or("");
    let icon: Element<'a, LayoutMessage> = match crate::icons::glyph(icon_name) {
        Some(glyph) => iced::widget::text(glyph)
            .font(crate::icons::font())
            .size(24)
            .color(palette.icon)
            .into(),
        None if icon_name.trim().is_empty() => Space::new().width(0.0).into(),
        None => iced::widget::text(icon_name)
            .size(18)
            .color(palette.icon)
            .into(),
    };
    let mut labels: Vec<Element<'a, LayoutMessage>> = vec![
        iced::widget::text(node.str_prop("title").unwrap_or(""))
            .size(16)
            .font(crate::fonts::bold_font())
            .color(palette.text)
            .into(),
    ];
    if let Some(subtitle) = node
        .str_prop("subtitle")
        .filter(|value| !value.trim().is_empty())
    {
        labels.push(
            iced::widget::text(subtitle)
                .size(13)
                .color(palette.secondary)
                .into(),
        );
    }
    Row::with_children([
        icon,
        Space::new().width(14.0).into(),
        Column::with_children(labels).spacing(4).into(),
        Space::new().width(Length::Fill).into(),
        trailing,
    ])
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

pub(crate) fn selection_chip<'a>(
    label: impl Into<String>,
    chevron: bool,
    palette: SettingsPalette,
) -> Element<'a, LayoutMessage> {
    let mut children: Vec<Element<'a, LayoutMessage>> = vec![
        iced::widget::text(label.into())
            .size(13)
            .font(crate::fonts::bold_font())
            .color(palette.text)
            .into(),
    ];
    if chevron {
        children.push(Space::new().width(5.0).into());
        children.push(
            iced::widget::text(
                crate::icons::glyph("expand_more_rounded")
                    .expect("expand_more_rounded is in the Material icon table"),
            )
            .font(crate::icons::font())
            .size(17)
            .color(palette.secondary)
            .into(),
        );
    }
    container(Row::with_children(children).align_y(iced::alignment::Vertical::Center))
        .padding([7, 11])
        .style(move |_theme| iced::widget::container::Style {
            background: Some(Background::Color(palette.chip)),
            border: Border::default().rounded(8).width(1).color(palette.divider),
            ..Default::default()
        })
        .into()
}

pub(crate) fn chevron<'a>(palette: SettingsPalette) -> Element<'a, LayoutMessage> {
    iced::widget::text(
        crate::icons::glyph("chevron_right_rounded")
            .expect("chevron_right_rounded is in the Material icon table"),
    )
    .font(crate::icons::font())
    .size(24)
    .color(palette.secondary)
    .into()
}

pub(crate) fn message(id: String, kind: EventKind) -> LayoutMessage {
    LayoutMessage::Event(WidgetEvent {
        widget_id: id,
        kind,
    })
}

pub(crate) fn options(node: &LayoutWidget) -> Vec<String> {
    node.str_prop("options")
        .unwrap_or("")
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn next_option(current: &str, options: &[String]) -> String {
    if options.is_empty() {
        return current.to_string();
    }
    let index = options
        .iter()
        .position(|option| option == current)
        .unwrap_or(0);
    options[(index + 1) % options.len()].clone()
}

pub(crate) fn bool_prop(value: Option<&str>, fallback: bool) -> bool {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("true" | "1" | "yes" | "on") => true,
        Some("false" | "0" | "no" | "off") => false,
        _ => fallback,
    }
}

pub(crate) fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    let raw = u32::from_str_radix(value, 16).ok()?;
    Some(Color::from_rgb8(
        ((raw >> 16) & 0xFF) as u8,
        ((raw >> 8) & 0xFF) as u8,
        (raw & 0xFF) as u8,
    ))
}
