//! Data & composite controls: `Table`, `TreeView`, `SplitPane`, `Reorderable`,
//! `Refreshable`, and `MediaBar`.

use crate::ir::{
    Align, AriaRole, Axis, Border, BorderSides, BoxNode, ColorValue, Corners, Cursor, Edges,
    IconNode, Justify, Layout, Length, Overflow, Rgba8, SlotId, StateOverrides, SurfaceNode,
    TextNode, Typography, VisualStyle,
};

pub const TABLE_HEADER_HEIGHT: f32 = 32.0;
pub const TABLE_ROW_HEIGHT: f32 = 32.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct TableSpec;

pub fn table(_spec: TableSpec) -> SurfaceNode {
    BoxNode::new("bern-table-scroll")
        .with_role(AriaRole::Table)
        .with_layout(Layout {
            direction: Axis::Vertical,
            width: Length::Full,
            overflow: Overflow::Hidden,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            border: Some(Border::all(1.0, ColorValue::Role("separator"))),
            corners: Some(Corners::all(8.0)),
            ..VisualStyle::default()
        })
        .child(
            BoxNode::new("bern-table-header")
                .with_layout(Layout {
                    direction: Axis::Horizontal,
                    align: Align::Center,
                    width: Length::Full,
                    min_height: Some(Length::Px(TABLE_HEADER_HEIGHT)),
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Role("surface.panel")),
                    border: Some(Border::sides(
                        1.0,
                        ColorValue::Role("separator"),
                        BorderSides::BOTTOM,
                    )),
                    ..VisualStyle::default()
                })
                .child(SurfaceNode::Slot(SlotId("columns")))
                .into_node(),
        )
        .child(
            BoxNode::new("bern-table-body")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    flex_grow: Some(1.0),
                    min_height: Some(Length::Px(0.0)),
                    overflow: Overflow::ScrollY,
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("rows")))
                .into_node(),
        )
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct TableColumnSpec {
    pub title: String,
    pub width: Option<f32>,
    pub sorted: bool,
    pub descending: bool,
    pub sortable: bool,
}

pub fn table_column_header(spec: TableColumnSpec) -> SurfaceNode {
    let mut cell = BoxNode::new("bern-table-heading")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 4.0,
            padding: Edges::xy(10.0, 0.0),
            width: spec.width.map(Length::Px).unwrap_or(Length::Auto),
            min_width: spec.width.is_none().then_some(Length::Px(60.0)),
            height: Length::Px(TABLE_HEADER_HEIGHT),
            flex_grow: spec.width.is_none().then_some(1.0),
            flex_shrink: spec.width.map(|_| 0.0),
            overflow: Overflow::Hidden,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            foreground: Some(ColorValue::Role("text.secondary")),
            cursor: Some(if spec.sortable {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            typography: Some(Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 600,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: spec.sortable.then(|| VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.title,
            typography: Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 600,
                ..Typography::default()
            },
            color: ColorValue::Role("text.secondary"),
            lines: Some(1),
            truncate: true,
            selectable: false,
        }));
    if spec.sorted {
        cell = cell.child(SurfaceNode::Icon(IconNode {
            name: if spec.descending {
                "chevron.down"
            } else {
                "chevron.up"
            },
            size: 11.0,
            color: ColorValue::Role("text.secondary"),
        }));
    }
    cell.into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TableRowSpec {
    pub selected: bool,
    pub enabled: bool,
}

pub fn table_row(spec: TableRowSpec) -> SurfaceNode {
    BoxNode::new("bern-table-row")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            width: Length::Full,
            min_height: Some(Length::Px(TABLE_ROW_HEIGHT)),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: spec
                .selected
                .then_some(ColorValue::RoleAlpha("selection.background", 0.18)),
            foreground: Some(ColorValue::Role("text.primary")),
            border: Some(Border::sides(
                1.0,
                ColorValue::RoleAlpha("separator", 0.06),
                BorderSides::BOTTOM,
            )),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: (spec.enabled && !spec.selected).then(|| VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "row",
                    state: "hover",
                    slot: "background",
                }),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Slot(SlotId("cells")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TreeViewSpec;

pub fn tree_view(_spec: TreeViewSpec) -> SurfaceNode {
    BoxNode::new("bern-tree")
        .with_role(AriaRole::Tree)
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 2.0,
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("rows")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TreeRowSpec {
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
}

pub fn tree_row(spec: TreeRowSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-tree-row")
        .with_role(AriaRole::TreeItem)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 6.0,
            width: Length::Full,
            min_height: Some(Length::Px(30.0)),
            padding: Edges::trbl(4.0, 8.0, 4.0, 8.0 + spec.depth as f32 * 18.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: spec
                .selected
                .then_some(ColorValue::Role("selection.background")),
            foreground: Some(ColorValue::Role(if spec.selected {
                "selection.foreground"
            } else {
                "text.primary"
            })),
            corners: Some(Corners::all(6.0)),
            cursor: Some(Cursor::Pointer),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: (!spec.selected).then(|| VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        });
    if spec.has_children {
        node = node.child(SurfaceNode::Slot(SlotId("toggle")));
    } else {
        node = node.child(
            BoxNode::new("bern-tree-spacer")
                .with_layout(Layout {
                    width: Length::Px(20.0),
                    height: Length::Px(20.0),
                    flex_shrink: Some(0.0),
                    ..Layout::default()
                })
                .into_node(),
        );
    }
    node.child(SurfaceNode::Slot(SlotId("content"))).into_node()
}

#[derive(Clone, Copy, Debug)]
pub struct SplitPaneSpec {
    pub axis: Axis,
    pub ratio: f32,
    pub min_first: f32,
    pub min_second: f32,
}

impl Default for SplitPaneSpec {
    fn default() -> Self {
        Self {
            axis: Axis::Horizontal,
            ratio: 0.5,
            min_first: 120.0,
            min_second: 120.0,
        }
    }
}

pub fn split_pane(spec: SplitPaneSpec) -> SurfaceNode {
    let vertical = spec.axis == Axis::Vertical;
    let handle = BoxNode::new("bern-split-handle")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            width: if vertical {
                Length::Full
            } else {
                Length::Px(8.0)
            },
            height: if vertical {
                Length::Px(8.0)
            } else {
                Length::Full
            },
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            cursor: Some(if vertical {
                Cursor::RowResize
            } else {
                Cursor::ColResize
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: Some(VisualStyle {
                background: Some(ColorValue::RoleAlpha("accent", 0.14)),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        })
        .child(
            BoxNode::new("bern-split-rule")
                .with_layout(Layout {
                    width: if vertical {
                        Length::Full
                    } else {
                        Length::Px(1.0)
                    },
                    height: if vertical {
                        Length::Px(1.0)
                    } else {
                        Length::Full
                    },
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Role("separator")),
                    ..VisualStyle::default()
                })
                .into_node(),
        )
        .into_node();

    BoxNode::new("bern-split-pane")
        .with_layout(Layout {
            direction: spec.axis,
            width: Length::Full,
            height: Length::Full,
            ..Layout::default()
        })
        .child(
            BoxNode::new("bern-split-first")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    width: if vertical {
                        Length::Full
                    } else {
                        Length::Fraction(spec.ratio.clamp(0.05, 0.95))
                    },
                    height: if vertical {
                        Length::Fraction(spec.ratio.clamp(0.05, 0.95))
                    } else {
                        Length::Full
                    },
                    min_width: (!vertical).then_some(Length::Px(spec.min_first)),
                    min_height: vertical.then_some(Length::Px(spec.min_first)),
                    flex_shrink: Some(0.0),
                    overflow: Overflow::Hidden,
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("first")))
                .into_node(),
        )
        .child(handle)
        .child(
            BoxNode::new("bern-split-second")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    min_width: (!vertical).then_some(Length::Px(spec.min_second)),
                    min_height: vertical.then_some(Length::Px(spec.min_second)),
                    flex_grow: Some(1.0),
                    overflow: Overflow::Hidden,
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("second")))
                .into_node(),
        )
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReorderableSpec;

pub fn reorderable(_spec: ReorderableSpec) -> SurfaceNode {
    BoxNode::new("bern-reorderable")
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 4.0,
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("rows")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReorderRowSpec {
    pub is_source: bool,
    pub is_target: bool,
}

pub fn reorder_row(spec: ReorderRowSpec) -> SurfaceNode {
    BoxNode::new("bern-reorder-row")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            padding: Edges::xy(10.0, 6.0),
            width: Length::Full,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.panel")),
            border: Some(Border::all(
                1.0,
                ColorValue::Role(if spec.is_target {
                    "accent"
                } else {
                    "control.border"
                }),
            )),
            corners: Some(Corners::all(8.0)),
            opacity: spec.is_source.then_some(0.6),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Icon(IconNode {
            name: "chevron.up.chevron.down",
            size: 13.0,
            color: ColorValue::Role("icon.secondary"),
        }))
        .child(SurfaceNode::Slot(SlotId("content")))
        .child(SurfaceNode::Slot(SlotId("actions")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RefreshableSpec {
    pub refreshing: bool,
}

pub fn refreshable(spec: RefreshableSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-refreshable").with_layout(Layout {
        direction: Axis::Vertical,
        width: Length::Full,
        min_width: Some(Length::Px(0.0)),
        ..Layout::default()
    });
    if spec.refreshing {
        node = node.child(
            BoxNode::new("bern-refreshable-bar")
                .with_layout(Layout {
                    direction: Axis::Horizontal,
                    align: Align::Center,
                    justify: Justify::Center,
                    gap: 6.0,
                    padding: Edges::xy(0.0, 6.0),
                    width: Length::Full,
                    ..Layout::default()
                })
                .child(SurfaceNode::Icon(IconNode {
                    name: "arrow.clockwise",
                    size: 14.0,
                    color: ColorValue::Role("accent"),
                }))
                .into_node(),
        );
    }
    node.child(SurfaceNode::Slot(SlotId("body"))).into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaBarSpec;

pub fn media_bar(_spec: MediaBarSpec) -> SurfaceNode {
    BoxNode::new("bern-media-bar")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 10.0,
            padding: Edges::xy(12.0, 8.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Rgba(Rgba8::new(20, 20, 24, 204))),
            border: Some(Border::all(
                1.0,
                ColorValue::Rgba(Rgba8::new(255, 255, 255, 31)),
            )),
            corners: Some(Corners::all(10.0)),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Slot(SlotId("play")))
        .child(SurfaceNode::Slot(SlotId("scrubber")))
        .child(SurfaceNode::Slot(SlotId("time")))
        .child(SurfaceNode::Slot(SlotId("mute")))
        .into_node()
}
