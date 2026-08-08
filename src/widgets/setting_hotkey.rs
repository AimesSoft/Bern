//! A settings row that displays and edits a keyboard shortcut shell.

use crate::core::layout::Widget as LayoutWidget;
use crate::core::widget::{BuildContext, EventKind, LayoutMessage, WidgetDef};
use crate::widgets::settings_shared::{self, SettingsPalette};
use iced::Element;

/// The layout name of this control.
pub const NAME: &str = "setting_hotkey";

/// A labelled shortcut setting.
#[derive(Default)]
pub struct SettingHotkey;

impl WidgetDef for SettingHotkey {
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
        let value = node
            .str_prop("value")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("未设置");
        let trailing = settings_shared::selection_chip(value, false, palette);
        settings_shared::action_row(
            node,
            trailing,
            settings_shared::message(ctx.qualify(&node.id), EventKind::Pressed),
            palette,
        )
    }
}
