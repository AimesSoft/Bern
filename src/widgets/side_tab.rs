//! A NipaPlay desktop-style vertical settings tab list.
//!
//! All choices are declared directly on the control as
//! `material_icon:label:number`, separated by commas. A press publishes the
//! chosen number through `EventKind::Other` using the parent control id.
//!
//! This control only renders the tab list. Layouts that need scrolling must
//! compose it inside `scroll_layout` instead of creating another scrolling
//! implementation here.

use crate::core::layout::{SizePolicy, Widget as LayoutWidget};
use crate::core::widget::{
    BuildContext, BuildError, EventKind, LayoutMessage, WidgetDef, WidgetEvent, size_lengths,
};
use iced::widget::{Column, Row, Space, button, container};
use iced::{Background, Border, Color, Element, Length};

/// The layout name of this control.
pub const NAME: &str = "side_tab";

#[derive(Debug, Clone)]
struct TabItem {
    icon: String,
    label: String,
    number: i32,
}

/// A configurable vertical navigation list.
#[derive(Default)]
pub struct SideTab;

impl WidgetDef for SideTab {
    fn name(&self) -> &'static str {
        NAME
    }

    fn interactive(&self) -> bool {
        true
    }

    fn validate(&self, node: &LayoutWidget) -> Result<(), BuildError> {
        parse_items(node.str_prop("items").unwrap_or(""))
            .map(|_| ())
            .map_err(|value| BuildError::BadProp {
                widget: node.id.clone(),
                prop: "items".into(),
                value,
            })
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let items = parse_items(node.str_prop("items").unwrap_or(""))
            .expect("side_tab items were validated before build");
        let selected = node
            .prop("selected")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or_else(|| items.first().map(|item| item.number).unwrap_or(0));
        let id = ctx.qualify(&node.id);
        let is_dark = ctx.theme.extended_palette().is_dark;
        let inactive = if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.72)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.54)
        };
        let primary = &ctx.theme.extended_palette().primary;
        let accent = primary.base.color;
        let accent_text = primary.base.text;
        let hover = if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.05)
        };

        let mut children = Vec::with_capacity(items.len());
        for item in items {
            let is_selected = item.number == selected;
            let foreground = if is_selected { accent_text } else { inactive };
            let background = if is_selected {
                accent
            } else {
                Color::TRANSPARENT
            };
            let icon: Element<'a, LayoutMessage> = match crate::icons::glyph(&item.icon) {
                Some(glyph) => iced::widget::text(glyph)
                    .font(crate::icons::font())
                    .size(20)
                    .color(foreground)
                    .into(),
                None => iced::widget::text(item.icon)
                    .size(18)
                    .color(foreground)
                    .into(),
            };
            let label: Element<'a, LayoutMessage> = iced::widget::text(item.label)
                .size(15)
                .font(crate::fonts::bold_font())
                .color(foreground)
                .into();
            let mut row_children = vec![icon, Space::new().width(11.0).into(), label];
            row_children.push(Space::new().width(Length::Fill).into());
            if is_selected {
                row_children.push(
                    iced::widget::text(
                        crate::icons::glyph("chevron_right_rounded")
                            .expect("chevron_right_rounded is in the Material icon table"),
                    )
                    .font(crate::icons::font())
                    .size(20)
                    .color(accent_text)
                    .into(),
                );
            }
            let content = Row::with_children(row_children)
                .align_y(iced::alignment::Vertical::Center)
                .width(Length::Fill);
            let message = LayoutMessage::Event(WidgetEvent {
                widget_id: id.clone(),
                kind: EventKind::Other(item.number.to_string()),
            });
            let tab = button(content)
                .on_press(message)
                .width(Length::Fill)
                .padding([12, 14])
                .style(move |_theme, status| {
                    let fill = match status {
                        button::Status::Hovered if !is_selected => hover,
                        button::Status::Pressed if !is_selected => {
                            Color::from_rgba(hover.r, hover.g, hover.b, (hover.a + 0.04).min(1.0))
                        }
                        _ => background,
                    };
                    iced::widget::button::Style {
                        background: Some(Background::Color(fill)),
                        text_color: foreground,
                        border: Border::default().rounded(8),
                        ..Default::default()
                    }
                });
            children.push(tab.into());
        }

        let list = Column::with_children(children)
            .spacing(4)
            .padding([8, 8])
            .width(Length::Fill);
        let (width, height) = size_lengths(size);
        let mut host = container(list).width(width.unwrap_or(Length::Fill));
        if let Some(height) = height {
            host = host.height(height);
        }
        host.into()
    }
}

fn parse_items(value: &str) -> Result<Vec<TabItem>, String> {
    let mut result = Vec::new();
    for raw in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let mut fields = raw.splitn(3, ':').map(str::trim);
        let icon = fields.next().unwrap_or_default();
        let label = fields.next().unwrap_or_default();
        let number = fields
            .next()
            .ok_or_else(|| format!("`{raw}` must be icon:label:number"))?
            .parse::<i32>()
            .map_err(|_| format!("`{raw}` has an invalid number"))?;
        if icon.is_empty() || label.is_empty() {
            return Err(format!("`{raw}` must be icon:label:number"));
        }
        if result.iter().any(|item: &TabItem| item.number == number) {
            return Err(format!("duplicate tab number `{number}`"));
        }
        result.push(TabItem {
            icon: icon.into(),
            label: label.into(),
            number,
        });
    }
    if result.is_empty() {
        Err("at least one tab item is required".into())
    } else {
        Ok(result)
    }
}
