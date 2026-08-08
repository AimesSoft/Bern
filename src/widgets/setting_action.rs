//! A settings row that exposes an action with a trailing chevron.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, EventKind, LayoutMessage, WidgetDef};
use crate::widgets::settings_shared::{self, SettingsPalette};
use iced::Element;

/// The layout name of this control.
pub const NAME: &str = "setting_action";

/// A labelled settings action.
#[derive(Default)]
pub struct SettingAction;

impl WidgetDef for SettingAction {
    fn name(&self) -> &'static str {
        NAME
    }
    fn interactive(&self) -> bool {
        true
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
        let palette = SettingsPalette::resolve(ctx.theme);
        settings_shared::action_row(
            node,
            settings_shared::chevron(palette),
            settings_shared::message(ctx.qualify(&node.id), EventKind::Pressed),
            palette,
        )
    }
}
