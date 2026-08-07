//! Rern — a runtime-driven UI framework built on top of [iced].
//!
//! The core idea: **controls, layout, and behavior are three separate
//! concerns**, exchanged only through well-defined contracts.
//!
//! - **Controls** are ordinary Rust files under [`widgets`]. Each control
//!   implements [`WidgetDef`] and declares its own style surface — exactly
//!   the fields a theme file is allowed to adjust.
//! - **Layouts** are RON files (`layouts/*.ron`) loaded at runtime. They are
//!   flat by design: two tables, `areas` (rows/columns/stacks, connected by
//!   `parent` ids) and `widgets` (controls placed into areas). Different
//!   devices can pick different layout files.
//! - **Light/dark mode** is not configured anywhere external: every control
//!   keeps its light and dark palettes in its own code and picks colors from
//!   the active `iced::Theme` (the standard interface) at build time.
//!
//! Because layouts and themes are runtime data, an application ships one
//! binary and adapts its UI per device, or swaps its look without recompiling.

pub mod core;
pub mod icons;
pub mod widgets;

pub use core::layout::{Area, AreaKind, Layout, SizePolicy, Widget};
pub use core::id::IdRegistry;
pub use core::registry::Registry;
pub use core::store::LayoutStore;
pub use core::theme::ThemeRouter;
pub use core::ui::{PressOrigin, ThemeReveal};
pub use core::widget::{
    BuildContext, BuildError, EventKind, LayoutMessage, WidgetDef, WidgetEvent,
};
pub use iced;

/// Creates a [`Registry`] pre-loaded with every built-in control.
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::new();
    widgets::register_builtins(&mut registry);
    registry
}
