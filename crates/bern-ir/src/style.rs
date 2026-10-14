//! Style overrides and motion tokens declared with `style!`.
//!
//! Standard controls keep their platform look unless the application
//! declares otherwise. A rule overrides one kind of control, or one named
//! variant of it, on every platform or on the platforms it names. Colors
//! are semantic roles or explicit pairs for light and dark; motion refers
//! to spring tokens by name.

use crate::{SourceLocation, Srgb};
use serde::{Deserialize, Serialize};

/// Spring tokens every application has: name, period in seconds and
/// damping ratio. An application may change their values.
pub const BUILT_IN_MOTION: [(&str, f64, f64); 6] = [
    ("press", 0.12, 1.0),
    ("snappy", 0.22, 0.85),
    ("smooth", 0.45, 1.0),
    ("bouncy", 0.50, 0.62),
    ("jelly", 0.42, 0.42),
    ("hero", 0.48, 0.86),
];

/// Semantic color roles a style may name. Native targets map them to their
/// system colors; Bern-drawn targets take them from the theme kernel.
pub const COLOR_ROLES: &[&str] = &[
    "surface.canvas",
    "surface.panel",
    "surface.raised",
    "text.primary",
    "text.secondary",
    "text.placeholder",
    "icon.primary",
    "icon.secondary",
    "accent",
    "on_accent",
    "status.success",
    "status.warning",
    "status.error",
    "control.border",
    "control.track",
    "control.thumb",
    "focus.indicator",
    "selection.background",
    "selection.foreground",
    "disabled.foreground",
    "disabled.surface",
    "fill.subtle",
    "fill.strong",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleSheet {
    /// Every motion token: the built-in ones, then the application's own.
    pub motion: Vec<MotionToken>,
    pub rules: Vec<StyleRule>,
    /// Every transition: the built-in ones, then the application's own.
    #[serde(default)]
    pub transitions: Vec<TransitionDef>,
    /// Patterns `Pattern(name:)` tiles over a canvas.
    #[serde(default)]
    pub patterns: Vec<PatternDef>,
    /// Accent colors the application chooses among at run time with
    /// `WindowSet(accent:)`.
    #[serde(default)]
    pub accents: Vec<AccentDef>,
    /// The weights the application gives text styles, such as a heavy
    /// `large_title`; the sizes stay the platform's.
    #[serde(default)]
    pub text_weights: Vec<TextWeight>,
}

/// The weight, and the size when the application's design gives one, of
/// one text style.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextWeight {
    /// A text style, such as `large_title`.
    pub style: String,
    /// One of [`TEXT_WEIGHTS`].
    pub weight: String,
    /// The size in points on desktops; the platform's size of the style
    /// when absent.
    #[serde(default)]
    pub size: Option<f64>,
}

/// The weights a text style can take.
pub const TEXT_WEIGHTS: &[&str] = &[
    "ultralight", "thin", "light", "regular", "medium", "semibold", "bold", "heavy", "black",
];

/// A named accent color.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccentDef {
    pub name: String,
    pub color: ColorRef,
}

/// A woven check: the bands, repeated across and down, colored where they
/// cross; with `twill`, fine diagonal lines over it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternDef {
    pub name: String,
    pub bands: Vec<Band>,
    pub twill: bool,
}

/// A band of a pattern: `width` logical units of `color` at `opacity`, or
/// a gap of the canvas's own color when `color` is `None`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub color: Option<ColorRef>,
    pub width: f64,
    pub opacity: f64,
}

/// How an element appears or disappears, moved by a motion token.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransitionDef {
    pub name: String,
    pub kind: TransitionKind,
    pub motion: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionKind {
    /// Fades in or out.
    Fade,
    /// Grows from (or shrinks to) `from` of its size while fading.
    Pop { from: f64 },
    /// Slides from (or to) `distance` beyond `edge` while fading.
    Slide { edge: Edge, distance: f64 },
}

/// Transitions every application has.
pub fn built_in_transitions() -> Vec<TransitionDef> {
    vec![
        TransitionDef {
            name: "fade".into(),
            kind: TransitionKind::Fade,
            motion: "smooth".into(),
        },
        TransitionDef {
            name: "pop".into(),
            kind: TransitionKind::Pop { from: 0.8 },
            motion: "bouncy".into(),
        },
        TransitionDef {
            name: "slide".into(),
            kind: TransitionKind::Slide {
                edge: Edge::Bottom,
                distance: 16.0,
            },
            motion: "smooth".into(),
        },
    ]
}

impl StyleSheet {
    pub fn token(&self, name: &str) -> Option<&MotionToken> {
        self.motion.iter().find(|token| token.name == name)
    }

    /// The accent named like an enum variant: `Crimson` and `crimson`
    /// name the same accent.
    pub fn accent(&self, name: &str) -> Option<&AccentDef> {
        let wanted = name.replace('_', "").to_lowercase();
        self.accents
            .iter()
            .find(|accent| accent.name.replace('_', "").to_lowercase() == wanted)
    }

    pub fn pattern(&self, name: &str) -> Option<&PatternDef> {
        self.patterns.iter().find(|pattern| pattern.name == name)
    }

    pub fn transition(&self, name: &str) -> Option<&TransitionDef> {
        self.transitions
            .iter()
            .find(|transition| transition.name == name)
    }

    /// The rules for `platform`, in declaration order: later rules override
    /// earlier ones property by property.
    pub fn rules_for<'a>(&'a self, platform: &'a str) -> impl Iterator<Item = &'a StyleRule> {
        self.rules.iter().filter(move |rule| {
            rule.targets.is_empty() || rule.targets.iter().any(|t| t == platform)
        })
    }
}

/// A spring: undamped period `period` in seconds and damping ratio
/// `damping`. Stiffness is (2π / period)² and damping 4π·damping / period,
/// for a mass of 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionToken {
    pub name: String,
    pub period: f64,
    pub damping: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleRule {
    /// The control kind, such as `SidebarItem`.
    pub component: String,
    /// A named variant that instances select with `variant:`.
    pub variant: Option<String>,
    /// Platforms the rule applies to (`macos`, `ios`, `windows`, `linux`,
    /// `browser`); empty for all.
    pub targets: Vec<String>,
    /// Platforms that keep their default look for effects they cannot show.
    pub exempt: Vec<String>,
    /// Effects in every state.
    pub base: Vec<StyleEffect>,
    pub states: Vec<StateStyle>,
    /// Motion back to the resting state.
    pub settle: Option<String>,
    pub source: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateStyle {
    pub state: StyleState,
    pub effects: Vec<StyleEffect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleState {
    Hover,
    Pressed,
    Selected,
    Focus,
    Disabled,
}

impl StyleState {
    pub const ALL: [StyleState; 5] = [
        StyleState::Hover,
        StyleState::Pressed,
        StyleState::Selected,
        StyleState::Focus,
        StyleState::Disabled,
    ];

    pub fn name(self) -> &'static str {
        match self {
            StyleState::Hover => "hover",
            StyleState::Pressed => "pressed",
            StyleState::Selected => "selected",
            StyleState::Focus => "focus",
            StyleState::Disabled => "disabled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StyleEffect {
    Background {
        color: ColorRef,
    },
    Foreground {
        color: ColorRef,
    },
    Border {
        color: ColorRef,
        width: f64,
    },
    Corner {
        radius: f64,
    },
    Padding {
        length: f64,
    },
    Size {
        size: ControlSize,
    },
    /// A selected item filled with `color`; its text and icons use the
    /// color's foreground pair.
    Fill {
        color: ColorRef,
    },
    /// A selected item marked by a bar on one edge.
    Indicator {
        color: ColorRef,
        edge: Edge,
        width: f64,
    },
    Scale {
        x: f64,
        y: f64,
        motion: String,
    },
    Offset {
        x: f64,
        y: f64,
        motion: String,
    },
    Opacity {
        value: f64,
        motion: String,
    },
    /// A shadow below the control, as if lifted off the page: `radius` of
    /// blur, `y` down, black at `opacity`.
    Shadow {
        radius: f64,
        y: f64,
        opacity: f64,
    },
    /// The platform's own look for this state.
    System,
    /// No feedback in this state.
    Nothing,
}

impl StyleEffect {
    /// The effect's name in `style!` and in target capabilities.
    pub fn name(&self) -> &'static str {
        match self {
            StyleEffect::Background { .. } => "background",
            StyleEffect::Foreground { .. } => "foreground",
            StyleEffect::Border { .. } => "border",
            StyleEffect::Corner { .. } => "corner",
            StyleEffect::Padding { .. } => "padding",
            StyleEffect::Size { .. } => "size",
            StyleEffect::Fill { .. } => "fill",
            StyleEffect::Indicator { .. } => "indicator",
            StyleEffect::Scale { .. } => "scale",
            StyleEffect::Offset { .. } => "offset",
            StyleEffect::Opacity { .. } => "opacity",
            StyleEffect::Shadow { .. } => "shadow",
            StyleEffect::System => "system",
            StyleEffect::Nothing => "none",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColorRef {
    Role { role: String },
    Fixed { light: Srgb, dark: Srgb },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlSize {
    Small,
    Regular,
    Large,
    ExtraLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Leading,
    Trailing,
    Top,
    Bottom,
}
