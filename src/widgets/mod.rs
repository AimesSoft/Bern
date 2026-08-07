//! Built-in controls.
//!
//! Each control lives in its own file and declares its own style surface —
//! the fields a theme file may adjust for it. See
//! [`crate::core::widget::WidgetDef`].

pub mod button;
pub mod icon;
pub mod icon_button;
pub mod layout;
pub mod rect;
pub mod text;
pub mod text_input;
pub mod title;

use crate::core::registry::Registry;

/// Registers every built-in control into `registry`.
pub fn register_builtins(registry: &mut Registry) {
    registry.register(button::Button);
    registry.register(icon::Icon);
    registry.register(icon_button::IconButton);
    registry.register(layout::LayoutRef);
    registry.register(text::Text);
    registry.register(text_input::TextInput);
    registry.register(rect::Rect);
    registry.register(title::Title);
}
