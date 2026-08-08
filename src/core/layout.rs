//! Runtime layout files (RON): a flat description of areas and widgets.
//!
//! The file is intentionally flat — two tables, `areas` and `widgets` —
//! while the tree shape lives in the `parent` pointers of [`Area`]. This
//! keeps layouts readable at any depth, and easy to diff, validate, and
//! generate.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A whole layout file: one page description.
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    /// Optional human-readable name, e.g. `"phone login"`.
    #[serde(default)]
    pub name: Option<String>,
    /// Layout containers. Tree shape is expressed with [`Area::parent`] ids.
    #[serde(default)]
    pub areas: Vec<Area>,
    /// The widgets placed inside areas.
    #[serde(default)]
    pub widgets: Vec<Widget>,
}

impl Layout {
    /// Parse a RON layout source.
    pub fn parse(source: &str) -> Result<Self, ron::error::SpannedError> {
        // `IMPLICIT_SOME` lets optional fields be written as plain values
        // (`parent: "root"`, `size: Fill`) instead of `Some(...)`.
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(source)
    }

    /// Load and parse a layout file from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let source =
            std::fs::read_to_string(path.as_ref()).map_err(|e| format!("read layout: {e}"))?;
        Self::parse(&source).map_err(|e| format!("parse layout: {e}"))
    }
}

/// A layout container (row, column, or stack).
#[derive(Debug, Clone, Deserialize)]
pub struct Area {
    /// Unique id; referenced by `parent` and by widgets' `area`.
    pub id: String,
    /// How children are arranged.
    #[serde(rename = "kind")]
    pub kind: AreaKind,
    /// Parent area id. Absent means this is a root area.
    #[serde(default)]
    pub parent: Option<String>,
    /// Inner padding in logical pixels.
    #[serde(default)]
    pub padding: Option<f32>,
    /// Gap between children.
    #[serde(default)]
    pub spacing: Option<f32>,
    /// Optional horizontal sizing policy for this container.
    #[serde(default)]
    pub width: Option<SizePolicy>,
    /// Optional vertical sizing policy for this container.
    #[serde(default)]
    pub height: Option<SizePolicy>,
}

/// How an area arranges its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AreaKind {
    /// Children side by side.
    Row,
    /// Children stacked vertically.
    Column,
    /// Children drawn on top of each other (use `z` for ordering).
    Stack,
}

/// A widget placed in an area.
#[derive(Debug, Clone, Deserialize)]
pub struct Widget {
    /// Unique id; events from this widget carry this id.
    pub id: String,
    /// Registered widget type name, e.g. `"title"`.
    #[serde(rename = "kind")]
    pub kind: String,
    /// The area this widget belongs to.
    pub area: String,
    /// Draw order inside a [`AreaKind::Stack`]. Defaults to `0`.
    #[serde(default)]
    pub z: i32,
    /// Sizing policy. `Auto` by default.
    #[serde(default)]
    pub size: Option<SizePolicy>,
    /// Widget-specific properties (label, text, placeholder, ...).
    #[serde(default)]
    pub props: HashMap<String, String>,
}

impl Widget {
    /// Raw property value, if present.
    pub fn prop(&self, key: &str) -> Option<&String> {
        self.props.get(key)
    }

    /// Property value as `&str`, if present.
    pub fn str_prop(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }
}

/// How a widget is sized.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum SizePolicy {
    /// Size itself to its content.
    Auto,
    /// Fill all available space in both axes.
    Fill,
    /// Fixed size in logical pixels (both axes).
    Fixed(f32),
    /// Share the free space of a `Row`/`Column` proportionally,
    /// like Flutter's flex factor.
    Weight(f32),
}
