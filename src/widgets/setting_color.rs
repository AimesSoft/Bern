//! A settings row that cycles through a declared color palette.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, BuildError, EventKind, LayoutMessage, WidgetDef};
use crate::widgets::settings_shared::{self, SettingsPalette};
use iced::widget::{Row, Space, container};
use iced::{Background, Border, Element};

/// The layout name of this control.
pub const NAME: &str = "setting_color";

/// A labelled color selection setting.
#[derive(Default)]
pub struct SettingColor;

impl WidgetDef for SettingColor {
    fn name(&self) -> &'static str {
        NAME
    }
    fn interactive(&self) -> bool {
        true
    }
    fn validate(&self, node: &LayoutWidget) -> Result<(), BuildError> {
        settings_shared::validate_common(node)?;
        if settings_shared::options(node).is_empty() {
            Err(BuildError::MissingProp {
                widget: node.id.clone(),
                prop: "options".into(),
            })
        } else {
            Ok(())
        }
    }
    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        let palette = SettingsPalette::resolve(ctx.theme);
        let options = settings_shared::options(node);
        let selected = node
            .str_prop("value")
            .unwrap_or(options[0].as_str())
            .to_string();
        let next = settings_shared::next_option(&selected, &options);
        let swatch_children: Vec<Element<'a, LayoutMessage>> = options
            .iter()
            .take(6)
            .map(|option| {
                let color = settings_shared::parse_color(option).unwrap_or(palette.accent);
                let active = option == &selected;
                container(Space::new().width(18.0).height(18.0))
                    .style(move |_theme| iced::widget::container::Style {
                        background: Some(Background::Color(color)),
                        border: Border::default()
                            .rounded(9)
                            .width(if active { 2.0 } else { 1.0 })
                            .color(if active {
                                palette.text
                            } else {
                                palette.divider
                            }),
                        ..Default::default()
                    })
                    .into()
            })
            .collect();
        let swatches = Row::with_children(swatch_children).spacing(7).into();
        settings_shared::action_row(
            node,
            swatches,
            settings_shared::message(
                ctx.qualify(&node.id),
                EventKind::Other(format!("set|{next}")),
            ),
            palette,
        )
    }
}
