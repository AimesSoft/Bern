//! `Picker` lowering: dropdown menu, segmented control, and chips.

use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Cursor, Edges, IconNode, Justify,
    Layout, Length, Shadow, StateOverrides, SurfaceNode, TextNode, Typography, VisualStyle,
};

#[derive(Clone, Debug, Default)]
pub struct PickerMenuSpec {
    pub label: String,
    pub label_hidden: bool,
    pub selected_label: String,
    pub open: bool,
    pub enabled: bool,
}

pub fn picker_menu(spec: PickerMenuSpec) -> SurfaceNode {
    let state = if !spec.enabled {
        "disabled"
    } else if spec.open {
        "pressed"
    } else {
        "rest"
    };
    let trigger = BoxNode::new("bern-dropdown")
        .with_role(AriaRole::Combobox)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::SpaceBetween,
            gap: 6.0,
            height: Length::Px(36.0),
            padding: Edges::trbl(0.0, 10.0, 0.0, 12.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "button.standard",
                state,
                slot: "background",
            }),
            foreground: Some(ColorValue::Recipe {
                recipe: "button.standard",
                state,
                slot: "foreground",
            }),
            border: Some(Border::all(
                1.0,
                if spec.open {
                    ColorValue::Role("accent")
                } else {
                    ColorValue::Recipe {
                        recipe: "button.standard",
                        state,
                        slot: "border",
                    }
                },
            )),
            corners: Some(Corners::all(8.0)),
            opacity: (!spec.enabled).then_some(0.5),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            typography: Some(Typography {
                size: 13.5,
                line_height: Some(20.0),
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
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.selected_label,
            typography: Typography {
                size: 13.5,
                line_height: Some(20.0),
                weight: 500,
                ..Typography::default()
            },
            color: ColorValue::Recipe {
                recipe: "button.standard",
                state,
                slot: "foreground",
            },
            lines: Some(1),
            truncate: true,
            selectable: false,
        }))
        .child(SurfaceNode::Icon(IconNode {
            name: "chevron.down",
            size: 12.0,
            color: ColorValue::Role("icon.secondary"),
        }))
        .into_node();

    wrap_picker_label(spec.label, spec.label_hidden, trigger)
}

#[derive(Clone, Debug)]
pub struct SegmentOptionSpec {
    pub label: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PickerSegmentedSpec {
    pub label: String,
    pub label_hidden: bool,
    pub options: Vec<SegmentOptionSpec>,
    pub capsule: bool,
    pub height: Option<f32>,
    pub enabled: bool,
}

pub fn picker_segmented(spec: PickerSegmentedSpec) -> SurfaceNode {
    let seg_height = spec.height.map(|h| (h - 6.0).max(16.0)).unwrap_or(30.0);
    let track_corner = if spec.capsule {
        Corners::PILL
    } else {
        Corners::all(8.0)
    };
    let seg_corner = if spec.capsule {
        Corners::PILL
    } else {
        Corners::all(6.0)
    };

    let segments = spec.options.into_iter().map(|opt| {
        let state = if opt.selected { "selected" } else { "rest" };
        BoxNode::new("bern-segment")
            .with_role(AriaRole::Option)
            .with_layout(Layout {
                direction: Axis::Horizontal,
                align: Align::Center,
                justify: Justify::Center,
                height: Length::Px(seg_height),
                padding: Edges::xy(10.0, 0.0),
                flex_grow: Some(1.0),
                ..Layout::default()
            })
            .with_style(VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "segmented",
                    state,
                    slot: "background",
                }),
                foreground: Some(ColorValue::Recipe {
                    recipe: "segmented",
                    state,
                    slot: "foreground",
                }),
                border: opt.selected.then_some(Border::all(
                    1.0,
                    ColorValue::Recipe {
                        recipe: "segmented",
                        state: "selected",
                        slot: "border",
                    },
                )),
                corners: Some(seg_corner),
                shadows: if opt.selected {
                    vec![Shadow::drop(1.0, 2.0, ColorValue::Role("scrim"))]
                } else {
                    Vec::new()
                },
                cursor: Some(if spec.enabled && !opt.selected {
                    Cursor::Pointer
                } else {
                    Cursor::Default
                }),
                typography: Some(Typography {
                    size: 13.0,
                    line_height: Some(18.0),
                    weight: 500,
                    ..Typography::default()
                }),
                ..VisualStyle::default()
            })
            .with_states(StateOverrides {
                hover: (spec.enabled && !opt.selected).then(|| VisualStyle {
                    background: Some(ColorValue::Recipe {
                        recipe: "segmented",
                        state: "hover",
                        slot: "background",
                    }),
                    foreground: Some(ColorValue::Recipe {
                        recipe: "segmented",
                        state: "hover",
                        slot: "foreground",
                    }),
                    ..VisualStyle::default()
                }),
                focus_ring: Some(ColorValue::Role("focus.indicator")),
                ..StateOverrides::default()
            })
            .child(SurfaceNode::Text(TextNode {
                content: opt.label,
                typography: Typography {
                    size: 13.0,
                    line_height: Some(18.0),
                    weight: 500,
                    ..Typography::default()
                },
                color: ColorValue::Recipe {
                    recipe: "segmented",
                    state,
                    slot: "foreground",
                },
                lines: Some(1),
                truncate: true,
                selectable: false,
            }))
            .into_node()
    });

    let group = BoxNode::new("bern-segmented")
        .with_role(AriaRole::Group)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 2.0,
            padding: Edges::all(2.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "segmented",
                state: "rest",
                slot: "track",
            }),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "segmented",
                    state: "rest",
                    slot: "border",
                },
            )),
            corners: Some(track_corner),
            ..VisualStyle::default()
        })
        .children(segments)
        .into_node();

    wrap_picker_label(spec.label, spec.label_hidden, group)
}

#[derive(Clone, Debug)]
pub struct ChipOptionSpec {
    pub label: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PickerChipsSpec {
    pub label: String,
    pub label_hidden: bool,
    pub options: Vec<ChipOptionSpec>,
    pub enabled: bool,
}

/// `Picker(style: chips)` using the canonical `chip` recipe (selected option
/// inverted to `text.primary` / `surface.canvas`, matching both Web & `bern-theme`).
pub fn picker_chips(spec: PickerChipsSpec) -> SurfaceNode {
    let chips = spec.options.into_iter().map(|opt| {
        let state = if !spec.enabled {
            "disabled"
        } else if opt.selected {
            "selected"
        } else {
            "rest"
        };
        BoxNode::new("bern-chip")
            .with_role(AriaRole::Option)
            .with_layout(Layout {
                direction: Axis::Horizontal,
                align: Align::Center,
                justify: Justify::Center,
                height: Length::Px(30.0),
                padding: Edges::xy(12.0, 0.0),
                flex_shrink: Some(0.0),
                ..Layout::default()
            })
            .with_style(VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "chip",
                    state,
                    slot: "background",
                }),
                foreground: Some(ColorValue::Recipe {
                    recipe: "chip",
                    state,
                    slot: "foreground",
                }),
                border: (!opt.selected).then_some(Border::all(
                    1.0,
                    ColorValue::Recipe {
                        recipe: "chip",
                        state,
                        slot: "border",
                    },
                )),
                corners: Some(Corners::PILL),
                cursor: Some(if spec.enabled && !opt.selected {
                    Cursor::Pointer
                } else {
                    Cursor::Default
                }),
                typography: Some(Typography {
                    size: 13.0,
                    line_height: Some(18.0),
                    weight: 500,
                    ..Typography::default()
                }),
                ..VisualStyle::default()
            })
            .with_states(StateOverrides {
                hover: (spec.enabled && !opt.selected).then(|| VisualStyle {
                    background: Some(ColorValue::Recipe {
                        recipe: "chip",
                        state: "hover",
                        slot: "background",
                    }),
                    foreground: Some(ColorValue::Recipe {
                        recipe: "chip",
                        state: "hover",
                        slot: "foreground",
                    }),
                    ..VisualStyle::default()
                }),
                focus_ring: Some(ColorValue::Role("focus.indicator")),
                ..StateOverrides::default()
            })
            .child(SurfaceNode::Text(TextNode {
                content: opt.label,
                typography: Typography {
                    size: 13.0,
                    line_height: Some(18.0),
                    weight: 500,
                    ..Typography::default()
                },
                color: ColorValue::Recipe {
                    recipe: "chip",
                    state,
                    slot: "foreground",
                },
                lines: Some(1),
                truncate: false,
                selectable: false,
            }))
            .into_node()
    });

    let wrap = BoxNode::new("bern-chips")
        .with_role(AriaRole::Group)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            wrap: true,
            align: Align::Center,
            gap: 8.0,
            ..Layout::default()
        })
        .children(chips)
        .into_node();

    wrap_picker_label(spec.label, spec.label_hidden, wrap)
}

fn wrap_picker_label(label: String, hidden: bool, field: SurfaceNode) -> SurfaceNode {
    if hidden {
        return field;
    }
    BoxNode::new("bern-row")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::SpaceBetween,
            gap: 12.0,
            min_height: Some(Length::Px(36.0)),
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: label,
            typography: Typography::default(),
            color: ColorValue::Role("text.primary"),
            lines: None,
            truncate: false,
            selectable: false,
        }))
        .child(field)
        .into_node()
}
