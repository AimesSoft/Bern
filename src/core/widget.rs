//! The widget contract: what every control must provide to be usable from a
//! layout file, and how events flow back to the application.

use crate::core::id::IdRegistry;
use crate::core::layout::{SizePolicy, Widget};
use crate::core::registry::Registry;
use crate::core::store::LayoutStore;
use crate::core::ui::{PressOrigin, ThemeReveal};
use iced::{Element, Length};

/// The message produced by every layout-driven widget.
///
/// All widgets speak the same language at runtime: events tagged with the id
/// of the widget that produced them. The application translates these generic
/// events into its own typed messages.
#[derive(Debug, Clone)]
pub enum LayoutMessage {
    /// A runtime event from a widget inside the layout tree.
    Event(WidgetEvent),
    /// Published by the background when every subscribed button has confirmed
    /// that the color beneath it changed; the app then completes the deferred
    /// theme switch.
    ThemeRevealDone,
}

/// An event produced by a widget.
#[derive(Debug, Clone)]
pub struct WidgetEvent {
    /// The `id` of the widget from the layout file.
    pub widget_id: String,
    /// What happened.
    pub kind: EventKind,
}

/// The kinds of events the built-in widgets can produce.
#[derive(Debug, Clone)]
pub enum EventKind {
    /// The widget was activated (e.g. a button press).
    Pressed,
    /// The text of a text input changed.
    TextChanged(String),
    /// A boolean control was toggled.
    Toggled(bool),
    /// A range control (e.g. a slider) reported a new value in 0..=1.
    Changed(f32),
    /// Anything else, as a string payload.
    Other(String),
}

/// Everything a widget needs while building itself from a layout node.
///
/// The `prefix` makes layouts modular: when a layout embeds another layout
/// as a widget, every id inside the embedded tree is qualified with the
/// embedding widget's id, so reused building blocks never collide.
pub struct BuildContext<'a, 't> {
    /// The active iced theme — the standard interface for light/dark mode.
    /// Controls keep their light and dark palettes in their own code and
    /// pick colors from this theme. Its lifetime is independent so the
    /// reveal wrapper can rebuild a control with a temporary target theme.
    pub theme: &'t iced::Theme,
    /// The widget registry, so containers can build their children.
    pub registry: &'a Registry,
    /// The layout store, so `layout` widgets can resolve embedded layouts.
    pub store: &'a LayoutStore,
    /// The shared press-origin store, so backgrounds can reveal color changes
    /// from the button that triggered them.
    pub press_origin: &'a PressOrigin,
    /// The theme-reveal notification hub (two-phase theme switching).
    pub theme_reveal: &'a ThemeReveal,
    /// The central widget-id registry.
    pub ids: &'a IdRegistry,
    prefix: String,
    depth: u32,
}

impl<'a, 't> BuildContext<'a, 't> {
    /// Creates the context for a top-level layout build.
    pub fn root(
        theme: &'t iced::Theme,
        registry: &'a Registry,
        store: &'a LayoutStore,
        press_origin: &'a PressOrigin,
        theme_reveal: &'a ThemeReveal,
        ids: &'a IdRegistry,
    ) -> Self {
        Self {
            theme,
            registry,
            store,
            press_origin,
            theme_reveal,
            ids,
            prefix: String::new(),
            depth: 0,
        }
    }

    /// A child context for an embedded layout, extending the id prefix.
    pub fn with_prefix(&self, extra: &str) -> BuildContext<'a, 't> {
        let prefix = if self.prefix.is_empty() {
            extra.to_string()
        } else {
            format!("{}.{}", self.prefix, extra)
        };
        Self {
            theme: self.theme,
            registry: self.registry,
            store: self.store,
            press_origin: self.press_origin,
            theme_reveal: self.theme_reveal,
            ids: self.ids,
            prefix,
            depth: self.depth + 1,
        }
    }

    /// The full event id of a widget, including the embedding prefix chain.
    pub fn qualify(&self, id: &str) -> String {
        if self.prefix.is_empty() {
            id.to_string()
        } else {
            format!("{}.{}", self.prefix, id)
        }
    }

    /// Current embedding depth (used as a recursion guard).
    pub fn depth(&self) -> u32 {
        self.depth
    }
}

/// The trait every control must implement.
///
/// This is the "one control = one rs file" contract: a control declares its
/// layout name, and how to turn a [`Widget`] into an iced [`Element`].
pub trait WidgetDef: Send + Sync {
    /// The name used in layout files and theme files, e.g. `"button"`.
    fn name(&self) -> &'static str;

    /// Build an iced element for this control from a layout node.
    fn build<'a, 't>(
        &self,
        node: &'a Widget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a, 't>,
    ) -> Element<'a, LayoutMessage>;

    /// Validates control-specific required properties before any widget tree
    /// is built. The default accepts every property set.
    fn validate(&self, _node: &Widget) -> Result<(), BuildError> {
        Ok(())
    }

    /// Whether this control produces events (and therefore needs a declared
    /// interaction id in the central [`IdRegistry`]).
    fn interactive(&self) -> bool {
        false
    }

    /// Whether the engine should automatically follow the theme reveal for
    /// this control — rebuild it with the target theme the moment the sweep
    /// covers its position. The background control (the animation body)
    /// opts out.
    fn follows_theme_reveal(&self) -> bool {
        true
    }
}

/// Converts a [`SizePolicy`] into optional width/height [`Length`]s.
///
/// Controls apply these to their own concrete widget types, so background
/// drawing (rect, button, ...) uses the widget's real bounds. `Weight` only
/// affects the main axis of a `Row`/`Column`.
pub fn size_lengths(size: Option<SizePolicy>) -> (Option<Length>, Option<Length>) {
    match size {
        None | Some(SizePolicy::Auto) => (None, None),
        Some(SizePolicy::Fill) => (Some(Length::Fill), Some(Length::Fill)),
        Some(SizePolicy::Fixed(px)) => (Some(Length::Fixed(px)), Some(Length::Fixed(px))),
        Some(SizePolicy::Weight(weight)) => {
            (Some(Length::FillPortion(weight.max(1.0) as u16)), None)
        }
    }
}

/// Errors produced while turning a layout into a widget tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// The layout referenced a widget type that is not registered.
    UnknownWidget(String),
    /// A required property is missing on the node.
    MissingProp { widget: String, prop: String },
    /// A property has an invalid value.
    BadProp {
        widget: String,
        prop: String,
        value: String,
    },
    /// The layout itself is structurally invalid (bad references, duplicate
    /// ids, wrong root count, ...).
    InvalidLayout(String),
    /// An interactive widget has no declared id in the central registry.
    UnregisteredId(String),
}
