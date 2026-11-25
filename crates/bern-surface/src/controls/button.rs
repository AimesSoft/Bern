//! `Button` and `IconButton` lowering.

use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Cursor, Edges, IconNode, Justify,
    Layout, Length, SlotId, StateOverrides, SurfaceNode, TextNode, Typography, VisualStyle,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonProminence {
    #[default]
    Standard,
    Prominent,
    Plain,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonRole {
    #[default]
    Normal,
    Destructive,
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonShape {
    #[default]
    Default,
    Capsule,
    Chip,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    #[default]
    Regular,
    Large,
    ExtraLarge,
}

/// Height, horizontal padding, text size, and icon size for a button variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonMetrics {
    pub height: f32,
    pub padding_x: f32,
    pub font_size: f32,
    pub icon_size: f32,
    pub corner: f32,
}

/// Canonical button metrics shared by Web and GPUI.
pub fn button_metrics(
    size: Option<ButtonSize>,
    shape: ButtonShape,
    height_override: Option<f32>,
) -> ButtonMetrics {
    let (default_height, padding_x, font_size) = match (shape, size) {
        (ButtonShape::Chip, _) => (30.0, 12.0, 13.0),
        (_, Some(ButtonSize::Small)) => (28.0, 10.0, 12.5),
        (_, Some(ButtonSize::Large)) => (42.0, 20.0, 14.5),
        (_, Some(ButtonSize::ExtraLarge)) => (50.0, 24.0, 16.0),
        _ => (36.0, 14.0, 13.5),
    };
    let height = height_override.unwrap_or(default_height);
    let corner = match shape {
        ButtonShape::Capsule | ButtonShape::Chip => height / 2.0,
        ButtonShape::Default => 8.0,
    };
    ButtonMetrics {
        height,
        padding_x,
        font_size,
        icon_size: font_size + 2.5,
        corner,
    }
}

#[derive(Clone, Debug, Default)]
pub struct ButtonSpec {
    pub label: String,
    pub icon: Option<&'static str>,
    pub prominence: ButtonProminence,
    pub role: ButtonRole,
    pub shape: ButtonShape,
    pub size: Option<ButtonSize>,
    pub height: Option<f32>,
    pub enabled: bool,
    pub custom_content: bool,
    pub fill_width: bool,
}

impl ButtonSpec {
    pub fn recipe(&self) -> &'static str {
        match (self.role, self.prominence) {
            (ButtonRole::Destructive, _) => "button.destructive",
            (_, ButtonProminence::Prominent) => "button.prominent",
            (_, ButtonProminence::Plain) => "button.plain",
            _ => "button.standard",
        }
    }
}

pub fn button(spec: ButtonSpec) -> SurfaceNode {
    let recipe = spec.recipe();
    let bordered = recipe == "button.standard";
    let metrics = button_metrics(spec.size, spec.shape, spec.height);
    let state = if spec.enabled { "rest" } else { "disabled" };

    let state_style = |st: &'static str| VisualStyle {
        background: Some(ColorValue::Recipe {
            recipe,
            state: st,
            slot: "background",
        }),
        foreground: Some(ColorValue::Recipe {
            recipe,
            state: st,
            slot: "foreground",
        }),
        border: bordered.then_some(Border::all(
            1.0,
            ColorValue::Recipe {
                recipe,
                state: st,
                slot: "border",
            },
        )),
        corners: Some(Corners::all(metrics.corner)),
        cursor: Some(if spec.enabled {
            Cursor::Pointer
        } else {
            Cursor::Default
        }),
        typography: Some(Typography {
            size: metrics.font_size,
            line_height: Some((metrics.font_size * 1.4).round()),
            weight: 500,
            ..Typography::default()
        }),
        ..VisualStyle::default()
    };

    let part = match (spec.role, spec.prominence) {
        (ButtonRole::Destructive, _) => "bern-button bern-button-destructive",
        (_, ButtonProminence::Prominent) => "bern-button bern-button-prominent",
        (_, ButtonProminence::Plain) => "bern-button bern-button-plain",
        _ => "bern-button bern-button-standard",
    };

    let mut node = BoxNode::new(part)
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            gap: 6.0,
            padding: Edges::xy(metrics.padding_x, 0.0),
            width: if spec.fill_width {
                Length::Full
            } else {
                Length::Auto
            },
            height: Length::Px(metrics.height),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(state_style(state))
        .with_states(StateOverrides {
            hover: spec.enabled.then(|| state_style("hover")),
            pressed: spec.enabled.then(|| state_style("pressed")),
            disabled: Some(state_style("disabled")),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        });

    if spec.custom_content {
        node = node.child(SurfaceNode::Slot(SlotId("content")));
    } else {
        let fg = ColorValue::Recipe {
            recipe,
            state,
            slot: "foreground",
        };
        if let Some(icon) = spec.icon {
            node = node.child(SurfaceNode::Icon(IconNode {
                name: icon,
                size: metrics.icon_size,
                color: fg,
            }));
        }
        node = node.child(SurfaceNode::Text(TextNode {
            content: spec.label,
            typography: Typography {
                size: metrics.font_size,
                line_height: Some((metrics.font_size * 1.4).round()),
                weight: 500,
                ..Typography::default()
            },
            color: fg,
            lines: Some(1),
            truncate: true,
            selectable: false,
        }));
    }
    node.into_node()
}

#[derive(Clone, Copy, Debug)]
pub struct IconButtonSpec {
    pub icon: &'static str,
    pub prominence: ButtonProminence,
    pub color: Option<&'static str>,
    pub diameter: Option<f32>,
    pub glyph_size: Option<f32>,
    pub enabled: bool,
}

impl Default for IconButtonSpec {
    fn default() -> Self {
        Self {
            icon: "circle",
            prominence: ButtonProminence::Plain,
            color: None,
            diameter: None,
            glyph_size: None,
            enabled: true,
        }
    }
}

pub fn icon_button(spec: IconButtonSpec) -> SurfaceNode {
    let round = spec.diameter.is_some();
    let side = spec.diameter.unwrap_or(36.0);
    let corner = if round { side / 2.0 } else { 8.0 };
    let recipe = match (spec.prominence, round) {
        (ButtonProminence::Prominent, _) => "button.prominent",
        (ButtonProminence::Standard, true) => "button.standard",
        _ => "icon_button",
    };
    let bordered = recipe == "button.standard";
    let state = if spec.enabled { "rest" } else { "disabled" };
    let fg = match spec.color {
        Some(role) if spec.enabled => ColorValue::Role(role),
        _ => ColorValue::Recipe {
            recipe,
            state,
            slot: "foreground",
        },
    };
    let style = VisualStyle {
        background: Some(ColorValue::Recipe {
            recipe,
            state,
            slot: "background",
        }),
        foreground: Some(fg),
        border: bordered.then_some(Border::all(
            1.0,
            ColorValue::Recipe {
                recipe,
                state,
                slot: "border",
            },
        )),
        corners: Some(Corners::all(corner)),
        cursor: Some(if spec.enabled {
            Cursor::Pointer
        } else {
            Cursor::Default
        }),
        ..VisualStyle::default()
    };
    BoxNode::new("bern-icon-button")
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            width: Length::Px(side),
            height: Length::Px(side),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(style)
        .with_states(StateOverrides {
            hover: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                foreground: Some(ColorValue::Recipe {
                    recipe,
                    state: "hover",
                    slot: "foreground",
                }),
                ..VisualStyle::default()
            }),
            pressed: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Role("pressed.overlay")),
                foreground: Some(ColorValue::Recipe {
                    recipe,
                    state: "pressed",
                    slot: "foreground",
                }),
                ..VisualStyle::default()
            }),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Icon(IconNode {
            name: spec.icon,
            size: spec.glyph_size.unwrap_or(18.0),
            color: fg,
        }))
        .into_node()
}
