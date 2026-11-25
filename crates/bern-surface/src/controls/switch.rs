//! `Switch`, `Checkbox`, and `ToggleButton` lowering.

use crate::controls::button::{ButtonShape, button_metrics};
use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Cursor, Edges, IconNode, Justify,
    Layout, Length, StateOverrides, SurfaceNode, TextNode, Typography, VisualStyle,
};

pub const SWITCH_WIDTH: f32 = 36.0;
pub const SWITCH_HEIGHT: f32 = 20.0;
pub const SWITCH_KNOB: f32 = 14.0;
pub const SWITCH_PADDING: f32 = 2.0;

#[derive(Clone, Debug, Default)]
pub struct SwitchSpec {
    pub label: String,
    pub label_hidden: bool,
    pub checked: bool,
    pub enabled: bool,
    pub knob_on_color: Option<ColorValue>,
}

pub fn switch(spec: SwitchSpec) -> SurfaceNode {
    let state = match (spec.enabled, spec.checked) {
        (false, _) => "disabled",
        (true, true) => "on",
        (true, false) => "off",
    };
    let thumb_color = if spec.checked && spec.enabled {
        spec.knob_on_color.unwrap_or(ColorValue::Recipe {
            recipe: "switch",
            state: "on",
            slot: "thumb",
        })
    } else {
        ColorValue::Recipe {
            recipe: "switch",
            state,
            slot: "thumb",
        }
    };
    let knob = BoxNode::new("bern-switch-thumb")
        .with_layout(Layout {
            width: Length::Px(SWITCH_KNOB),
            height: Length::Px(SWITCH_KNOB),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(thumb_color),
            corners: Some(Corners::PILL),
            ..VisualStyle::default()
        })
        .into_node();

    let track = BoxNode::new("bern-switch")
        .with_role(AriaRole::Switch)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: if spec.checked {
                Justify::End
            } else {
                Justify::Start
            },
            width: Length::Px(SWITCH_WIDTH),
            height: Length::Px(SWITCH_HEIGHT),
            padding: Edges::all(SWITCH_PADDING),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "switch",
                state,
                slot: "track",
            }),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "switch",
                    state,
                    slot: "border",
                },
            )),
            corners: Some(Corners::PILL),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: spec.enabled.then(|| {
                let hover_state = if spec.checked {
                    "on.hover"
                } else {
                    "off.hover"
                };
                VisualStyle {
                    background: Some(ColorValue::Recipe {
                        recipe: "switch",
                        state: hover_state,
                        slot: "track",
                    }),
                    border: Some(Border::all(
                        1.0,
                        ColorValue::Recipe {
                            recipe: "switch",
                            state: hover_state,
                            slot: "border",
                        },
                    )),
                    ..VisualStyle::default()
                }
            }),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        })
        .child(knob)
        .into_node();

    if spec.label_hidden {
        return track;
    }

    BoxNode::new("bern-row")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::SpaceBetween,
            gap: 10.0,
            min_height: Some(Length::Px(28.0)),
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.label,
            typography: Typography::default(),
            color: ColorValue::Role(if spec.enabled {
                "text.primary"
            } else {
                "disabled.foreground"
            }),
            lines: None,
            truncate: false,
            selectable: false,
        }))
        .child(track)
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct CheckboxSpec {
    pub label: String,
    pub label_hidden: bool,
    pub checked: bool,
    pub enabled: bool,
}

pub fn checkbox(spec: CheckboxSpec) -> SurfaceNode {
    let mut box_node = BoxNode::new("bern-checkbox")
        .with_role(AriaRole::Checkbox)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            width: Length::Px(16.0),
            height: Length::Px(16.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role(if spec.checked {
                "accent"
            } else {
                "surface.raised"
            })),
            border: Some(Border::all(
                1.0,
                ColorValue::Role(if spec.checked {
                    "accent"
                } else {
                    "control.border"
                }),
            )),
            corners: Some(Corners::all(4.0)),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        });
    if spec.checked {
        box_node = box_node.child(SurfaceNode::Icon(IconNode {
            name: "checkmark",
            size: 12.0,
            color: ColorValue::Role("on_accent"),
        }));
    }
    if spec.label_hidden {
        return box_node.into_node();
    }
    BoxNode::new("bern-checkbox-row")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 10.0,
            min_height: Some(Length::Px(28.0)),
            ..Layout::default()
        })
        .child(box_node.into_node())
        .child(SurfaceNode::Text(TextNode {
            content: spec.label,
            typography: Typography::default(),
            color: ColorValue::Role(if spec.enabled {
                "text.primary"
            } else {
                "disabled.foreground"
            }),
            lines: None,
            truncate: false,
            selectable: false,
        }))
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct ToggleButtonSpec {
    pub label: String,
    pub label_hidden: bool,
    pub icon: Option<&'static str>,
    pub checked: bool,
    pub enabled: bool,
    pub shape: ButtonShape,
    pub height: Option<f32>,
}

pub fn toggle_button(spec: ToggleButtonSpec) -> SurfaceNode {
    let metrics = button_metrics(None, spec.shape, spec.height);
    let state = if spec.enabled { "rest" } else { "disabled" };
    let bg = if spec.checked {
        ColorValue::Role("pressed.overlay")
    } else {
        ColorValue::Recipe {
            recipe: "button.standard",
            state,
            slot: "background",
        }
    };
    let fg = if spec.checked {
        ColorValue::Role("accent.text")
    } else {
        ColorValue::Recipe {
            recipe: "button.standard",
            state,
            slot: "foreground",
        }
    };
    let mut node = BoxNode::new("bern-toggle-button")
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            gap: 6.0,
            padding: Edges::xy(metrics.padding_x, 0.0),
            height: Length::Px(metrics.height),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(bg),
            foreground: Some(fg),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "button.standard",
                    state,
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
        })
        .with_states(StateOverrides {
            hover: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        });
    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: metrics.icon_size,
            color: fg,
        }));
    }
    if !spec.label_hidden || spec.icon.is_none() {
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
