//! Built-in controls.
//!
//! Each control lives in its own file and declares its own style surface —
//! the fields a theme file may adjust for it. See
//! [`crate::core::widget::WidgetDef`].

pub mod action_button;
pub mod button;
pub mod divider;
pub mod dropdown;
pub mod h_tab;
pub mod icon;
pub mod icon_button;
pub mod layout;
pub mod morph_icon;
pub mod rect;
pub(crate) mod reveal_wrapper;
pub mod round_button;
pub mod rounded_container;
pub mod scroll_layout;
pub mod setting_action;
pub mod setting_color;
pub mod setting_hotkey;
pub mod setting_info;
pub mod setting_label;
pub(crate) mod settings_shared;
pub mod side_tab;
pub mod slider;
pub mod split_pane;
pub mod switch;
pub mod text;
pub mod text_input;
pub mod title;
pub mod virtual_window;
pub mod window_controls;

use crate::core::registry::Registry;

/// Registers every built-in control into `registry`.
pub fn register_builtins(registry: &mut Registry) {
    registry.register(action_button::ActionButton);
    registry.register(button::Button);
    registry.register(divider::Divider);
    registry.register(dropdown::Dropdown);
    registry.register(h_tab::HTab);
    registry.register(icon::Icon);
    registry.register(icon_button::IconButton);
    registry.register(layout::LayoutRef);
    registry.register(morph_icon::MorphIcon);
    registry.register(text::Text);
    registry.register(text_input::TextInput);
    registry.register(rect::Rect);
    registry.register(round_button::RoundButton);
    registry.register(rounded_container::RoundedContainer);
    registry.register(scroll_layout::ScrollLayout);
    registry.register(setting_action::SettingAction);
    registry.register(setting_color::SettingColor);
    registry.register(setting_hotkey::SettingHotkey);
    registry.register(setting_info::SettingInfo);
    registry.register(setting_label::SettingLabel);
    registry.register(side_tab::SideTab);
    registry.register(split_pane::SplitPane);
    registry.register(switch::Switch);
    registry.register(slider::Progress);
    registry.register(slider::Slider);
    registry.register(title::Title);
    registry.register(virtual_window::VirtualWindow);
    registry.register(window_controls::WindowControls);
}
