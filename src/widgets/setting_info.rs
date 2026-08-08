//! A non-interactive informational settings row.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, LayoutMessage, WidgetDef};
use crate::widgets::settings_shared::{self, SettingsPalette};
use iced::{Element, Length};

/// The layout name of this control.
pub const NAME: &str = "setting_info";

/// A labelled informational row.
#[derive(Default)]
pub struct SettingInfo;

impl WidgetDef for SettingInfo {
    fn name(&self) -> &'static str {
        NAME
    }
    fn validate(&self, node: &LayoutWidget) -> Result<(), crate::BuildError> {
        settings_shared::validate_common(node)
    }
    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        settings_shared::static_row(
            node,
            iced::widget::Space::new().width(Length::Fixed(0.0)).into(),
            SettingsPalette::resolve(ctx.theme),
        )
    }
}
