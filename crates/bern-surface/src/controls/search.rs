//! Search fields, search palettes, search items, and toolbars.

use crate::ir::{
    Align, AriaRole, Axis, Border, BorderSides, BoxNode, ColorValue, Corners, Cursor, Edges,
    IconNode, Justify, Layout, Length, Overflow, Shadow, SlotId, StateOverrides, SurfaceNode,
    TextNode, Typography, VisualStyle,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchSpec {
    pub width: Option<f32>,
    pub has_results: bool,
}

pub fn search(spec: SearchSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-search-wrap")
        .with_layout(Layout {
            direction: Axis::Vertical,
            width: spec.width.map(Length::Px).unwrap_or(Length::Full),
            flex_shrink: spec.width.map(|_| 0.0),
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("input")));
    if spec.has_results {
        node = node.child(
            BoxNode::new("bern-search-results")
                .with_role(AriaRole::Region)
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    min_width: Some(Length::Px(280.0)),
                    max_height: Some(Length::Px(420.0)),
                    padding: Edges::all(6.0),
                    overflow: Overflow::ScrollY,
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Recipe {
                        recipe: "menu",
                        state: "popup",
                        slot: "background",
                    }),
                    foreground: Some(ColorValue::Role("text.primary")),
                    border: Some(Border::all(
                        1.0,
                        ColorValue::Recipe {
                            recipe: "menu",
                            state: "popup",
                            slot: "border",
                        },
                    )),
                    corners: Some(Corners::all(10.0)),
                    shadows: vec![Shadow::drop(12.0, 32.0, ColorValue::Role("scrim"))],
                    ..VisualStyle::default()
                })
                .child(SurfaceNode::Slot(SlotId("results")))
                .into_node(),
        );
    }
    node.into_node()
}

#[derive(Clone, Debug, Default)]
pub struct SearchPaletteSpec {
    pub prompt: String,
    pub shortcut_hint: Option<String>,
    pub icon_only: bool,
    pub width: f32,
    pub height: f32,
    pub enabled: bool,
}

pub fn search_palette(spec: SearchPaletteSpec) -> SurfaceNode {
    let trigger = if spec.icon_only {
        BoxNode::new("bern-search-palette-trigger")
            .with_role(AriaRole::Searchbox)
            .with_layout(Layout {
                direction: Axis::Horizontal,
                align: Align::Center,
                justify: Justify::Center,
                width: Length::Px(32.0),
                height: Length::Px(32.0),
                flex_shrink: Some(0.0),
                ..Layout::default()
            })
            .with_style(VisualStyle {
                corners: Some(Corners::all(16.0)),
                opacity: (!spec.enabled).then_some(0.5),
                cursor: Some(if spec.enabled {
                    Cursor::Pointer
                } else {
                    Cursor::Default
                }),
                ..VisualStyle::default()
            })
            .child(SurfaceNode::Icon(IconNode {
                name: "magnifyingglass",
                size: 16.0,
                color: ColorValue::Role("text.primary"),
            }))
            .into_node()
    } else {
        let mut node = BoxNode::new("bern-search-palette-trigger")
            .with_role(AriaRole::Searchbox)
            .with_layout(Layout {
                direction: Axis::Horizontal,
                align: Align::Center,
                gap: 6.0,
                width: Length::Px(260.0),
                height: Length::Px(32.0),
                padding: Edges::xy(10.0, 0.0),
                flex_shrink: Some(0.0),
                ..Layout::default()
            })
            .with_style(VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "field",
                    state: "rest",
                    slot: "background",
                }),
                foreground: Some(ColorValue::Role("text.secondary")),
                border: Some(Border::all(
                    1.0,
                    ColorValue::Recipe {
                        recipe: "field",
                        state: "rest",
                        slot: "border",
                    },
                )),
                corners: Some(Corners::all(16.0)),
                opacity: (!spec.enabled).then_some(0.5),
                cursor: Some(if spec.enabled {
                    Cursor::Pointer
                } else {
                    Cursor::Default
                }),
                ..VisualStyle::default()
            })
            .child(SurfaceNode::Icon(IconNode {
                name: "magnifyingglass",
                size: 14.0,
                color: ColorValue::Role("text.secondary"),
            }))
            .child(SurfaceNode::Text(TextNode {
                content: spec.prompt,
                typography: Typography {
                    size: 13.0,
                    line_height: Some(18.0),
                    weight: 400,
                    ..Typography::default()
                },
                color: ColorValue::Role("text.secondary"),
                lines: Some(1),
                truncate: true,
                selectable: false,
            }))
            .child(SurfaceNode::Spacer);
        if let Some(hint) = spec.shortcut_hint {
            node = node.child(
                BoxNode::new("bern-search-palette-key")
                    .with_layout(Layout {
                        padding: Edges::xy(5.0, 0.0),
                        flex_shrink: Some(0.0),
                        ..Layout::default()
                    })
                    .with_style(VisualStyle {
                        border: Some(Border::all(1.0, ColorValue::Role("separator"))),
                        corners: Some(Corners::all(4.0)),
                        ..VisualStyle::default()
                    })
                    .child(SurfaceNode::Text(TextNode {
                        content: hint,
                        typography: Typography {
                            size: 11.0,
                            line_height: Some(15.0),
                            weight: 500,
                            ..Typography::default()
                        },
                        color: ColorValue::Role("text.secondary"),
                        lines: Some(1),
                        truncate: false,
                        selectable: false,
                    }))
                    .into_node(),
            );
        }
        node.into_node()
    };

    let panel = BoxNode::new("bern-search-palette")
        .with_role(AriaRole::Dialog)
        .with_layout(Layout {
            direction: Axis::Vertical,
            width: Length::Px(if spec.width > 0.0 { spec.width } else { 640.0 }),
            height: Length::Px(if spec.height > 0.0 {
                spec.height
            } else {
                480.0
            }),
            overflow: Overflow::Hidden,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "menu",
                state: "popup",
                slot: "background",
            }),
            foreground: Some(ColorValue::Role("text.primary")),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "menu",
                    state: "popup",
                    slot: "border",
                },
            )),
            corners: Some(Corners::all(14.0)),
            shadows: vec![Shadow::drop(16.0, 48.0, ColorValue::Role("scrim"))],
            ..VisualStyle::default()
        })
        .child(
            BoxNode::new("bern-search-palette-header")
                .with_layout(Layout {
                    direction: Axis::Horizontal,
                    align: Align::Center,
                    gap: 4.0,
                    padding: Edges::xy(6.0, 0.0),
                    flex_shrink: Some(0.0),
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    border: Some(Border::sides(
                        1.0,
                        ColorValue::Role("separator"),
                        BorderSides::BOTTOM,
                    )),
                    ..VisualStyle::default()
                })
                .child(SurfaceNode::Slot(SlotId("input")))
                .child(SurfaceNode::Slot(SlotId("close")))
                .into_node(),
        )
        .child(
            BoxNode::new("bern-search-palette-results")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    flex_grow: Some(1.0),
                    min_height: Some(Length::Px(0.0)),
                    padding: Edges::all(6.0),
                    overflow: Overflow::ScrollY,
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("results")))
                .into_node(),
        )
        .into_node();

    BoxNode::new("bern-search-palette-host")
        .child(trigger)
        .child(panel)
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct SearchItemSpec {
    pub label: String,
    pub icon: Option<&'static str>,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub highlighted: bool,
    pub custom_content: bool,
}

pub fn search_item(spec: SearchItemSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-search-item")
        .with_role(AriaRole::Button)
        .with_layout(if spec.custom_content {
            Layout {
                direction: Axis::Vertical,
                width: Length::Full,
                padding: Edges::all(4.0),
                ..Layout::default()
            }
        } else {
            Layout {
                direction: Axis::Horizontal,
                align: Align::Center,
                gap: 10.0,
                width: Length::Full,
                height: Length::Px(36.0),
                padding: Edges::xy(10.0, 0.0),
                ..Layout::default()
            }
        })
        .with_style(VisualStyle {
            background: spec
                .highlighted
                .then_some(ColorValue::Role("pressed.overlay")),
            corners: Some(Corners::all(8.0)),
            cursor: Some(Cursor::Pointer),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: (!spec.highlighted).then(|| VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        });

    if spec.custom_content {
        return node.child(SurfaceNode::Slot(SlotId("content"))).into_node();
    }

    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 16.0,
            color: ColorValue::Role("text.secondary"),
        }));
    }
    node = node.child(SurfaceNode::Text(TextNode {
        content: spec.label,
        typography: Typography::default(),
        color: ColorValue::Role("text.primary"),
        lines: Some(1),
        truncate: true,
        selectable: false,
    }));
    node = node.child(SurfaceNode::Spacer);
    if let Some(detail) = spec.detail {
        node = node.child(SurfaceNode::Text(TextNode {
            content: detail,
            typography: Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 400,
                ..Typography::default()
            },
            color: ColorValue::Role("text.secondary"),
            lines: Some(1),
            truncate: true,
            selectable: false,
        }));
    }
    if let Some(hint) = spec.hint {
        node = node.child(
            BoxNode::new("bern-search-item-hint")
                .with_layout(Layout {
                    padding: Edges::xy(5.0, 0.0),
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    border: Some(Border::all(1.0, ColorValue::Role("separator"))),
                    corners: Some(Corners::all(4.0)),
                    ..VisualStyle::default()
                })
                .child(SurfaceNode::Text(TextNode {
                    content: hint,
                    typography: Typography {
                        size: 11.0,
                        line_height: Some(15.0),
                        weight: 500,
                        ..Typography::default()
                    },
                    color: ColorValue::Role("text.secondary"),
                    lines: Some(1),
                    truncate: false,
                    selectable: false,
                }))
                .into_node(),
        );
    }
    node.into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ToolbarSpec {
    pub height: f32,
}

pub fn toolbar(spec: ToolbarSpec) -> SurfaceNode {
    BoxNode::new("bern-toolbar")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            width: Length::Full,
            height: Length::Px(if spec.height > 0.0 { spec.height } else { 52.0 }),
            padding: Edges::xy(12.0, 0.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("leading")))
        .child(SurfaceNode::Spacer)
        .child(SurfaceNode::Slot(SlotId("trailing")))
        .into_node()
}
