//! The non-interactive icon, title and subtitle block used by settings rows.
//!
//! Layouts place this beside existing controls such as `switch`, `dropdown`
//! and `slider`; it deliberately owns no interaction or setting state.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, BuildError, LayoutMessage, WidgetDef};
use crate::widgets::settings_shared::{self, SettingsPalette};
use iced::{Element, Length};

/// The layout name of this control.
pub const NAME: &str = "setting_label";

/// A static settings-row label.
#[derive(Default)]
pub struct SettingLabel;

impl WidgetDef for SettingLabel {
    fn name(&self) -> &'static str {
        NAME
    }

    fn validate(&self, node: &LayoutWidget) -> Result<(), BuildError> {
        settings_shared::validate_common(node)
    }

    fn build<'a, 't>(
        &self,
        node: &'a LayoutWidget,
        _size: Option<crate::SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage> {
        settings_shared::base_row(
            node,
            iced::widget::Space::new().width(Length::Fixed(0.0)).into(),
            SettingsPalette::resolve(ctx.theme),
        )
    }
}
