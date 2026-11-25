//! Overlays, popovers, menus, dialogs, alerts, sheets, tooltips, toasts, and
//! disclosures.

use crate::ir::{
    Align, AriaRole, Axis, Border, BorderSides, BoxNode, ColorValue, Corners, Cursor, Edges,
    IconNode, Justify, Layout, Length, Overflow, Shadow, SlotId, StateOverrides, SurfaceNode,
    TextNode, Typography, VisualStyle,
};

pub const MODAL_CORNER: f32 = 12.0;
pub const ALERT_CORNER: f32 = 14.0;
pub const POPUP_CORNER: f32 = 10.0;
pub const SHEET_CORNER: f32 = 18.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct MenuPanelSpec {
    pub min_width: f32,
}

pub fn menu_panel(spec: MenuPanelSpec) -> SurfaceNode {
    BoxNode::new("bern-menu")
        .with_role(AriaRole::Menu)
        .with_layout(Layout {
            direction: Axis::Vertical,
            min_width: Some(Length::Px(spec.min_width.max(160.0))),
            padding: Edges::all(4.0),
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
            corners: Some(Corners::all(POPUP_CORNER)),
            shadows: vec![Shadow::drop(12.0, 32.0, ColorValue::Role("scrim"))],
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Slot(SlotId("items")))
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct MenuItemSpec {
    pub label: String,
    pub icon: Option<&'static str>,
    pub checked: Option<bool>,
    pub reserve_check_column: bool,
    pub destructive: bool,
    pub enabled: bool,
}

pub fn menu_item(spec: MenuItemSpec) -> SurfaceNode {
    let fg = match (spec.enabled, spec.destructive) {
        (false, _) => ColorValue::Role("disabled.foreground"),
        (true, true) => ColorValue::Role("status.error"),
        (true, false) => ColorValue::Role("text.primary"),
    };
    let check_color = if spec.enabled {
        ColorValue::Recipe {
            recipe: "menu",
            state: "selected",
            slot: "check",
        }
    } else {
        ColorValue::Role("disabled.foreground")
    };
    let mut node = BoxNode::new("bern-menu-item")
        .with_role(AriaRole::MenuItem)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            height: Length::Px(32.0),
            padding: Edges::xy(10.0, 0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            foreground: Some(fg),
            corners: Some(Corners::all(6.0)),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            typography: Some(Typography::default()),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "menu",
                    state: "hover",
                    slot: "background",
                }),
                ..VisualStyle::default()
            }),
            pressed: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Role("pressed.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        });
    if spec.reserve_check_column {
        let mut mark = BoxNode::new("bern-menu-check").with_layout(Layout {
            width: Length::Px(14.0),
            height: Length::Px(14.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        });
        if spec.checked == Some(true) {
            mark = mark.child(SurfaceNode::Icon(IconNode {
                name: "checkmark",
                size: 14.0,
                color: check_color,
            }));
        }
        node = node.child(mark.into_node());
    }
    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 16.0,
            color: fg,
        }));
    }
    node.child(SurfaceNode::Text(TextNode {
        content: spec.label,
        typography: Typography::default(),
        color: fg,
        lines: Some(1),
        truncate: true,
        selectable: false,
    }))
    .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PopoverPanelSpec {
    pub background: Option<ColorValue>,
    pub foreground: Option<ColorValue>,
}

pub fn popover_panel(spec: PopoverPanelSpec) -> SurfaceNode {
    BoxNode::new("bern-popover-panel")
        .with_role(AriaRole::Dialog)
        .with_layout(Layout {
            direction: Axis::Vertical,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(spec.background.unwrap_or(ColorValue::Recipe {
                recipe: "menu",
                state: "popup",
                slot: "background",
            })),
            foreground: Some(spec.foreground.unwrap_or(ColorValue::Role("text.primary"))),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "menu",
                    state: "popup",
                    slot: "border",
                },
            )),
            corners: Some(Corners::all(POPUP_CORNER)),
            shadows: vec![Shadow::drop(12.0, 32.0, ColorValue::Role("scrim"))],
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Slot(SlotId("content")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SplitButtonSpec {
    pub height: f32,
    pub open: bool,
}

pub fn split_button(spec: SplitButtonSpec) -> SurfaceNode {
    let h = if spec.height > 0.0 { spec.height } else { 36.0 };
    let state = if spec.open { "pressed" } else { "rest" };
    BoxNode::new("bern-split-button")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 2.0,
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("toggle")))
        .child(
            BoxNode::new("bern-split-chevron")
                .with_role(AriaRole::Button)
                .with_layout(Layout {
                    direction: Axis::Horizontal,
                    align: Align::Center,
                    justify: Justify::Center,
                    width: Length::Px(h * 0.8),
                    height: Length::Px(h),
                    flex_shrink: Some(0.0),
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Recipe {
                        recipe: "button.standard",
                        state,
                        slot: "background",
                    }),
                    border: Some(Border::all(
                        1.0,
                        ColorValue::Recipe {
                            recipe: "button.standard",
                            state: "rest",
                            slot: "border",
                        },
                    )),
                    corners: Some(Corners::all(h / 2.0)),
                    cursor: Some(Cursor::Pointer),
                    ..VisualStyle::default()
                })
                .with_states(StateOverrides {
                    hover: Some(VisualStyle {
                        background: Some(ColorValue::Role("hover.overlay")),
                        ..VisualStyle::default()
                    }),
                    ..StateOverrides::default()
                })
                .child(SurfaceNode::Icon(IconNode {
                    name: "chevron.down",
                    size: 12.0,
                    color: ColorValue::Recipe {
                        recipe: "button.standard",
                        state: "rest",
                        slot: "foreground",
                    },
                }))
                .into_node(),
        )
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ModalSpec {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub cover: bool,
}

pub fn modal(spec: ModalSpec) -> SurfaceNode {
    BoxNode::new("bern-window-dialog")
        .with_role(AriaRole::Dialog)
        .with_layout(Layout {
            direction: Axis::Vertical,
            width: if spec.cover {
                Length::Full
            } else {
                Length::Px(spec.width.unwrap_or(560.0))
            },
            height: if spec.cover {
                Length::Full
            } else {
                spec.height.map(Length::Px).unwrap_or(Length::Auto)
            },
            overflow: Overflow::Hidden,
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "dialog",
                state: "open",
                slot: "background",
            }),
            foreground: Some(ColorValue::Recipe {
                recipe: "dialog",
                state: "open",
                slot: "foreground",
            }),
            border: (!spec.cover).then_some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "dialog",
                    state: "open",
                    slot: "border",
                },
            )),
            corners: Some(if spec.cover {
                Corners::ZERO
            } else {
                Corners::all(MODAL_CORNER)
            }),
            shadows: if spec.cover {
                Vec::new()
            } else {
                vec![Shadow::drop(16.0, 48.0, ColorValue::Role("scrim"))]
            },
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Slot(SlotId("content")))
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct AlertSpec {
    pub title: String,
    pub message: Option<String>,
    pub width: f32,
}

pub fn alert(spec: AlertSpec) -> SurfaceNode {
    let mut card = BoxNode::new("bern-alert")
        .with_role(AriaRole::AlertDialog)
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 12.0,
            width: Length::Px(if spec.width > 0.0 { spec.width } else { 380.0 }),
            padding: Edges::all(20.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "dialog",
                state: "open",
                slot: "background",
            }),
            foreground: Some(ColorValue::Recipe {
                recipe: "dialog",
                state: "open",
                slot: "foreground",
            }),
            border: Some(Border::all(
                1.0,
                ColorValue::Recipe {
                    recipe: "dialog",
                    state: "open",
                    slot: "border",
                },
            )),
            corners: Some(Corners::all(ALERT_CORNER)),
            shadows: vec![Shadow::drop(20.0, 44.0, ColorValue::Role("scrim"))],
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.title,
            typography: Typography {
                size: 16.0,
                line_height: Some(22.0),
                weight: 600,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: None,
            truncate: false,
            selectable: false,
        }));
    if let Some(message) = spec.message.filter(|m| !m.is_empty()) {
        card = card.child(SurfaceNode::Text(TextNode {
            content: message,
            typography: Typography {
                size: 13.0,
                line_height: Some(19.0),
                weight: 400,
                ..Typography::default()
            },
            color: ColorValue::Role("text.secondary"),
            lines: None,
            truncate: false,
            selectable: false,
        }));
    }
    card.child(
        BoxNode::new("bern-alert-actions")
            .with_layout(Layout {
                direction: Axis::Horizontal,
                justify: Justify::End,
                gap: 8.0,
                padding: Edges::trbl(6.0, 0.0, 0.0, 0.0),
                ..Layout::default()
            })
            .child(SurfaceNode::Slot(SlotId("actions")))
            .into_node(),
    )
    .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BottomSheetSpec {
    pub width: f32,
    pub max_height: f32,
    pub drag_indicator: bool,
}

pub fn bottom_sheet(spec: BottomSheetSpec) -> SurfaceNode {
    let mut sheet = BoxNode::new("bern-bottom-sheet")
        .with_role(AriaRole::Dialog)
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 10.0,
            width: Length::Px(if spec.width > 0.0 { spec.width } else { 640.0 }),
            max_height: (spec.max_height > 0.0).then_some(Length::Px(spec.max_height)),
            padding: Edges::trbl(10.0, 18.0, 20.0, 18.0),
            overflow: Overflow::ScrollY,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Recipe {
                recipe: "dialog",
                state: "open",
                slot: "background",
            }),
            foreground: Some(ColorValue::Recipe {
                recipe: "dialog",
                state: "open",
                slot: "foreground",
            }),
            border: Some(Border::sides(
                1.0,
                ColorValue::Recipe {
                    recipe: "dialog",
                    state: "open",
                    slot: "border",
                },
                BorderSides::SHEET_TOP,
            )),
            corners: Some(Corners::top(SHEET_CORNER)),
            shadows: vec![Shadow::drop(-12.0, 36.0, ColorValue::Role("scrim"))],
            ..VisualStyle::default()
        });
    if spec.drag_indicator {
        sheet = sheet.child(
            BoxNode::new("bern-bottom-sheet-handle")
                .with_layout(Layout {
                    width: Length::Px(36.0),
                    height: Length::Px(4.0),
                    margin: Edges::trbl(0.0, 0.0, 4.0, 0.0),
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Role("control.border")),
                    corners: Some(Corners::PILL),
                    ..VisualStyle::default()
                })
                .into_node(),
        );
    }
    sheet
        .child(SurfaceNode::Slot(SlotId("content")))
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct TooltipSpec {
    pub content: String,
    pub keys: Option<String>,
}

pub fn tooltip(spec: TooltipSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-tooltip")
        .with_role(AriaRole::Tooltip)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 6.0,
            padding: Edges::xy(8.0, 4.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            foreground: Some(ColorValue::Role("text.primary")),
            border: Some(Border::all(1.0, ColorValue::Role("separator"))),
            corners: Some(Corners::all(6.0)),
            shadows: vec![Shadow::drop(6.0, 20.0, ColorValue::Role("scrim"))],
            typography: Some(Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 400,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: spec.content,
            typography: Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 400,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: Some(1),
            truncate: false,
            selectable: false,
        }));
    if let Some(keys) = spec.keys.filter(|k| !k.is_empty()) {
        node = node.child(
            BoxNode::new("bern-tooltip-kbd")
                .with_layout(Layout {
                    direction: Axis::Horizontal,
                    align: Align::Center,
                    padding: Edges::xy(5.0, 1.0),
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Role("surface.panel")),
                    foreground: Some(ColorValue::Role("text.secondary")),
                    border: Some(Border::all(1.0, ColorValue::Role("separator"))),
                    corners: Some(Corners::all(4.0)),
                    ..VisualStyle::default()
                })
                .child(SurfaceNode::Text(TextNode {
                    content: keys,
                    typography: Typography {
                        size: 11.0,
                        line_height: Some(14.0),
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

#[derive(Clone, Debug, Default)]
pub struct ToastSpec {
    pub content: String,
    pub icon: Option<&'static str>,
}

pub fn toast(spec: ToastSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-toast")
        .with_role(AriaRole::Status)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            padding: Edges::xy(14.0, 10.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            foreground: Some(ColorValue::Role("text.primary")),
            border: Some(Border::all(1.0, ColorValue::Role("separator"))),
            corners: Some(Corners::all(10.0)),
            shadows: vec![Shadow::drop(12.0, 28.0, ColorValue::Role("scrim"))],
            typography: Some(Typography {
                size: 13.0,
                line_height: Some(18.0),
                weight: 400,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        });
    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 16.0,
            color: ColorValue::Role("text.primary"),
        }));
    }
    node.child(SurfaceNode::Text(TextNode {
        content: spec.content,
        typography: Typography {
            size: 13.0,
            line_height: Some(18.0),
            weight: 400,
            ..Typography::default()
        },
        color: ColorValue::Role("text.primary"),
        lines: Some(2),
        truncate: true,
        selectable: false,
    }))
    .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct DisclosureSpec {
    pub label: String,
    pub icon: Option<&'static str>,
    pub expanded: bool,
}

pub fn disclosure(spec: DisclosureSpec) -> SurfaceNode {
    let mut header = BoxNode::new("bern-disclosure-summary")
        .with_role(AriaRole::Button)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            padding: Edges::xy(6.0, 6.0),
            width: Length::Full,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            corners: Some(Corners::all(6.0)),
            cursor: Some(Cursor::Pointer),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: Some(VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Icon(IconNode {
            name: if spec.expanded {
                "chevron.down"
            } else {
                "chevron.right"
            },
            size: 12.0,
            color: ColorValue::Role("icon.secondary"),
        }));
    if let Some(icon) = spec.icon {
        header = header.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 15.0,
            color: ColorValue::Role("icon.primary"),
        }));
    }
    header = header.child(SurfaceNode::Text(TextNode {
        content: spec.label,
        typography: Typography {
            size: 13.5,
            line_height: Some(20.0),
            weight: 500,
            ..Typography::default()
        },
        color: ColorValue::Role("text.primary"),
        lines: Some(1),
        truncate: true,
        selectable: false,
    }));

    let mut root = BoxNode::new("bern-disclosure")
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 4.0,
            width: Length::Full,
            ..Layout::default()
        })
        .child(header.into_node());
    if spec.expanded {
        root = root.child(
            BoxNode::new("bern-disclosure-body")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    gap: 8.0,
                    padding: Edges::trbl(4.0, 0.0, 6.0, 20.0),
                    width: Length::Full,
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("content")))
                .into_node(),
        );
    }
    root.into_node()
}
