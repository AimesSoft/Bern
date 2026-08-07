//! The widget contract: what every control must provide to be usable from a
//! layout file, and how events flow back to the application.

use crate::core::layout::{SizePolicy, Widget};
use crate::core::registry::Registry;
use crate::core::store::LayoutStore;
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
    /// Anything else, as a string payload.
    Other(String),
}

/// Everything a widget needs while building itself from a layout node.
///
/// The `prefix` makes layouts modular: when a layout embeds another layout
/// as a widget, every id inside the embedded tree is qualified with the
/// embedding widget's id, so reused building blocks never collide.
pub struct BuildContext<'a> {
    /// The active iced theme — the standard interface for light/dark mode.
    /// Controls keep their light and dark palettes in their own code and
    /// pick colors from this theme.
    pub theme: &'a iced::Theme,
    /// The widget registry, so containers can build their children.
    pub registry: &'a Registry,
    /// The layout store, so `layout` widgets can resolve embedded layouts.
    pub store: &'a LayoutStore,
    prefix: String,
    depth: u32,
}

impl<'a> BuildContext<'a> {
    /// Creates the context for a top-level layout build.
    pub fn root(
        theme: &'a iced::Theme,
        registry: &'a Registry,
        store: &'a LayoutStore,
    ) -> Self {
        Self {
            theme,
            registry,
            store,
            prefix: String::new(),
            depth: 0,
        }
    }

    /// A child context for an embedded layout, extending the id prefix.
    pub fn with_prefix(&self, extra: &str) -> Self {
        let prefix = if self.prefix.is_empty() {
            extra.to_string()
        } else {
            format!("{}.{}", self.prefix, extra)
        };
        Self {
            theme: self.theme,
            registry: self.registry,
            store: self.store,
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
    fn build<'a>(
        &self,
        node: &'a Widget,
        size: Option<SizePolicy>,
        ctx: &BuildContext<'a>,
    ) -> Element<'a, LayoutMessage>;
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
    MissingProp {
        widget: String,
        prop: String,
    },
    /// A property has an invalid value.
    BadProp {
        widget: String,
        prop: String,
        value: String,
    },
    /// The layout itself is structurally invalid (bad references, duplicate
    /// ids, wrong root count, ...).
    InvalidLayout(String),
}
