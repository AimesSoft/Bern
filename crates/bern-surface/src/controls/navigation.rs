//! Navigation & chrome containers: `SplitView`, `Sidebar`, `SidebarItem`,
//! `Detail`, `NavigationBar`, `NavLink`, `Tabs`, `TabItem`, `TabAccessory`,
//! and `Inspector`.

use crate::ir::{
    Align, AriaRole, Axis, Border, BorderSides, BoxNode, ColorValue, Corners, Cursor, Edges,
    IconNode, Justify, Layout, Length, Overflow, Shadow, SlotId, StateOverrides, SurfaceNode,
    TextNode, Typography, VisualStyle,
};

pub const SIDEBAR_DEFAULT_WIDTH: f32 = 232.0;
pub const SIDEBAR_ITEM_HEIGHT: f32 = 34.0;
pub const SIDEBAR_ITEM_CORNER: f32 = 8.0;
pub const SIDEBAR_ITEM_PADDING_X: f32 = 10.0;
pub const NAV_BAR_HEIGHT: f32 = 44.0;
pub const INSPECTOR_DEFAULT_WIDTH: f32 = 280.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct SplitViewSpec {
    pub collapsed: bool,
}

pub fn split_view(spec: SplitViewSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-split").with_layout(Layout {
        direction: Axis::Horizontal,
        width: Length::Full,
        height: Length::Full,
        overflow: Overflow::Hidden,
        ..Layout::default()
    });
    if !spec.collapsed {
        node = node.child(SurfaceNode::Slot(SlotId("sidebar")));
    }
    node.child(SurfaceNode::Slot(SlotId("detail"))).into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SidebarSpec {
    pub width: Option<f32>,
    /// Explicit background override from `style!`; defaults to translucent
    /// `sidebar.surface` so backdrops flow seamlessly behind the sidebar.
    pub background: Option<ColorValue>,
    pub has_header: bool,
    pub has_footer: bool,
}

pub fn sidebar(spec: SidebarSpec) -> SurfaceNode {
    let bg = spec
        .background
        .unwrap_or(ColorValue::Role("sidebar.surface"));
    let mut node = BoxNode::new("bern-sidebar")
        .with_role(AriaRole::Navigation)
        .with_layout(Layout {
            direction: Axis::Vertical,
            width: Length::Px(spec.width.unwrap_or(SIDEBAR_DEFAULT_WIDTH)),
            height: Length::Full,
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(bg),
            border: Some(Border::sides(
                1.0,
                ColorValue::Role("separator"),
                BorderSides::RIGHT,
            )),
            ..VisualStyle::default()
        });
    if spec.has_header {
        node = node.child(
            BoxNode::new("bern-sidebar-header")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    padding: Edges::trbl(12.0, 12.0, 0.0, 12.0),
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("header")))
                .into_node(),
        );
    }
    node = node.child(
        BoxNode::new("bern-sidebar-list")
            .with_layout(Layout {
                direction: Axis::Vertical,
                gap: 2.0,
                padding: Edges::all(8.0),
                flex_grow: Some(1.0),
                min_height: Some(Length::Px(0.0)),
                overflow: Overflow::ScrollY,
                ..Layout::default()
            })
            .child(SurfaceNode::Slot(SlotId("items")))
            .into_node(),
    );
    if spec.has_footer {
        node = node.child(
            BoxNode::new("bern-sidebar-footer")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    padding: Edges::all(8.0),
                    gap: 4.0,
                    flex_shrink: Some(0.0),
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("footer")))
                .into_node(),
        );
    }
    node.into_node()
}

#[derive(Clone, Debug, Default)]
pub struct SidebarItemSpec {
    pub label: String,
    pub icon: Option<&'static str>,
    pub badge: Option<i64>,
    pub selected: bool,
    pub enabled: bool,
    pub corner: Option<f32>,
    pub padding_x: Option<f32>,
}

pub fn sidebar_item(spec: SidebarItemSpec) -> SurfaceNode {
    let state = if spec.selected { "selected" } else { "rest" };
    let fg = ColorValue::Recipe {
        recipe: "nav_item",
        state,
        slot: "foreground",
    };
    let bg = ColorValue::Recipe {
        recipe: "nav_item",
        state,
        slot: "background",
    };
    let mut node = BoxNode::new("bern-sidebar-item")
        .with_role(AriaRole::Option)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 10.0,
            height: Length::Px(SIDEBAR_ITEM_HEIGHT),
            padding: Edges::xy(spec.padding_x.unwrap_or(SIDEBAR_ITEM_PADDING_X), 0.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(bg),
            foreground: Some(fg),
            corners: Some(Corners::all(spec.corner.unwrap_or(SIDEBAR_ITEM_CORNER))),
            opacity: (!spec.enabled).then_some(0.5),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            typography: Some(Typography {
                size: 13.5,
                line_height: Some(20.0),
                weight: if spec.selected { 600 } else { 500 },
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: (spec.enabled && !spec.selected).then(|| VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "nav_item",
                    state: "hover",
                    slot: "background",
                }),
                foreground: Some(ColorValue::Recipe {
                    recipe: "nav_item",
                    state: "hover",
                    slot: "foreground",
                }),
                ..VisualStyle::default()
            }),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        });
    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 16.0,
            color: fg,
        }));
    }
    node = node.child(SurfaceNode::Text(TextNode {
        content: spec.label,
        typography: Typography {
            size: 13.5,
            line_height: Some(20.0),
            weight: if spec.selected { 600 } else { 500 },
            ..Typography::default()
        },
        color: fg,
        lines: Some(1),
        truncate: true,
        selectable: false,
    }));
    if let Some(count) = spec.badge.filter(|c| *c > 0) {
        node = node.child(SurfaceNode::Spacer);
        node = node.child(SurfaceNode::Text(TextNode {
            content: count.to_string(),
            typography: Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 500,
                tabular_nums: true,
                ..Typography::default()
            },
            color: fg,
            lines: Some(1),
            truncate: false,
            selectable: false,
        }));
    }
    node.into_node()
}

#[derive(Clone, Debug, Default)]
pub struct DetailSpec {
    pub title: Option<String>,
    pub has_toggle: bool,
    pub has_footer: bool,
    pub background: Option<ColorValue>,
}

pub fn detail(spec: DetailSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-detail")
        .with_layout(Layout {
            direction: Axis::Vertical,
            flex_grow: Some(1.0),
            min_width: Some(Length::Px(0.0)),
            height: Length::Full,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: spec.background,
            ..VisualStyle::default()
        });
    if spec.title.is_some() || spec.has_toggle {
        let mut header = BoxNode::new("bern-page-header").with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            padding: if spec.title.is_some() {
                Edges::trbl(16.0, 24.0, 8.0, 24.0)
            } else {
                Edges::trbl(8.0, 12.0, 4.0, 12.0)
            },
            flex_shrink: Some(0.0),
            ..Layout::default()
        });
        if spec.has_toggle {
            header = header.child(SurfaceNode::Slot(SlotId("toggle")));
        }
        if let Some(title) = spec.title {
            header = header.child(SurfaceNode::Text(TextNode {
                content: title,
                typography: Typography {
                    size: 20.0,
                    line_height: Some(28.0),
                    weight: 600,
                    ..Typography::default()
                },
                color: ColorValue::Role("text.primary"),
                lines: Some(1),
                truncate: true,
                selectable: false,
            }));
        }
        node = node.child(header.into_node());
    }
    node = node.child(
        BoxNode::new("bern-detail-body")
            .with_layout(Layout {
                direction: Axis::Vertical,
                flex_grow: Some(1.0),
                min_height: Some(Length::Px(0.0)),
                ..Layout::default()
            })
            .child(SurfaceNode::Slot(SlotId("content")))
            .into_node(),
    );
    if spec.has_footer {
        node = node.child(
            BoxNode::new("bern-detail-footer")
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    flex_shrink: Some(0.0),
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("footer")))
                .into_node(),
        );
    }
    node.into_node()
}

#[derive(Clone, Debug, Default)]
pub struct NavigationBarSpec {
    pub title: Option<String>,
}

pub fn navigation_bar(spec: NavigationBarSpec) -> SurfaceNode {
    let mut bar = BoxNode::new("bern-nav-bar")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 4.0,
            height: Length::Px(NAV_BAR_HEIGHT),
            padding: Edges::xy(8.0, 0.0),
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
        .child(SurfaceNode::Slot(SlotId("back")));
    if let Some(title) = spec.title {
        bar = bar.child(SurfaceNode::Text(TextNode {
            content: title,
            typography: Typography {
                size: 17.0,
                line_height: Some(24.0),
                weight: 600,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: Some(1),
            truncate: true,
            selectable: false,
        }));
    }
    bar.into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NavLinkSpec {
    /// Inside a grouped `Section`, which already provides card row padding.
    pub in_section: bool,
    pub enabled: bool,
}

pub fn nav_link(spec: NavLinkSpec) -> SurfaceNode {
    BoxNode::new("bern-navlink")
        .with_role(AriaRole::Link)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            padding: if spec.in_section {
                Edges::ZERO
            } else {
                Edges::xy(16.0, 12.0)
            },
            width: Length::Full,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            corners: Some(Corners::all(if spec.in_section { 6.0 } else { 8.0 })),
            cursor: Some(if spec.enabled {
                Cursor::Pointer
            } else {
                Cursor::Default
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "row",
                    state: "hover",
                    slot: "background",
                }),
                ..VisualStyle::default()
            }),
            pressed: spec.enabled.then(|| VisualStyle {
                background: Some(ColorValue::Recipe {
                    recipe: "row",
                    state: "pressed",
                    slot: "background",
                }),
                ..VisualStyle::default()
            }),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Slot(SlotId("content")))
        .child(SurfaceNode::Icon(IconNode {
            name: "chevron.right",
            size: 14.0,
            color: ColorValue::Role("icon.secondary"),
        }))
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct TabItemSpec {
    pub label: String,
    pub icon: Option<&'static str>,
    pub icon_only: bool,
    pub selected: bool,
}

pub fn tab_item(spec: TabItemSpec) -> SurfaceNode {
    let state = if spec.selected { "selected" } else { "rest" };
    let fg = ColorValue::Recipe {
        recipe: "tab",
        state,
        slot: "foreground",
    };
    let mut node = BoxNode::new("bern-tab")
        .with_role(AriaRole::Tab)
        .with_layout(Layout {
            direction: Axis::Vertical,
            align: Align::Center,
            justify: Justify::Center,
            gap: 2.0,
            flex_grow: Some(1.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            foreground: Some(fg),
            cursor: Some(Cursor::Pointer),
            typography: Some(Typography {
                size: 11.0,
                line_height: Some(14.0),
                weight: if spec.selected { 600 } else { 500 },
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: (!spec.selected).then(|| VisualStyle {
                foreground: Some(ColorValue::Recipe {
                    recipe: "tab",
                    state: "hover",
                    slot: "foreground",
                }),
                ..VisualStyle::default()
            }),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        });
    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 22.0,
            color: fg,
        }));
    }
    if !(spec.icon_only && spec.icon.is_some()) {
        node = node.child(SurfaceNode::Text(TextNode {
            content: spec.label,
            typography: Typography {
                size: 11.0,
                line_height: Some(14.0),
                weight: if spec.selected { 600 } else { 500 },
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

#[derive(Clone, Copy, Debug, Default)]
pub struct TabsSpec {
    pub has_accessory: bool,
}

pub fn tabs(spec: TabsSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-tabs")
        .with_layout(Layout {
            direction: Axis::Vertical,
            width: Length::Full,
            height: Length::Full,
            ..Layout::default()
        })
        .child(
            BoxNode::new("bern-tabpanels")
                .with_role(AriaRole::TabPanel)
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    flex_grow: Some(1.0),
                    min_height: Some(Length::Px(0.0)),
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("content")))
                .into_node(),
        );
    if spec.has_accessory {
        node = node.child(tab_accessory(TabAccessorySpec));
    }
    node.child(
        BoxNode::new("bern-tabbar")
            .with_role(AriaRole::TabList)
            .with_layout(Layout {
                direction: Axis::Horizontal,
                width: Length::Full,
                height: Length::Px(56.0),
                flex_shrink: Some(0.0),
                ..Layout::default()
            })
            .with_style(VisualStyle {
                background: Some(ColorValue::Role("sidebar.surface")),
                border: Some(Border::sides(
                    1.0,
                    ColorValue::Role("separator"),
                    BorderSides::TOP,
                )),
                ..VisualStyle::default()
            })
            .child(SurfaceNode::Slot(SlotId("tabs")))
            .into_node(),
    )
    .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TabAccessorySpec;

pub fn tab_accessory(_spec: TabAccessorySpec) -> SurfaceNode {
    BoxNode::new("bern-tab-accessory")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            min_height: Some(Length::Px(48.0)),
            padding: Edges::xy(6.0, 0.0),
            margin: Edges::xy(12.0, 6.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            border: Some(Border::all(1.0, ColorValue::Role("separator"))),
            corners: Some(Corners::all(24.0)),
            shadows: vec![Shadow::drop(8.0, 24.0, ColorValue::Role("scrim"))],
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Slot(SlotId("accessory")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InspectorSpec {
    pub presented: bool,
    pub width: Option<f32>,
}

pub fn inspector(spec: InspectorSpec) -> SurfaceNode {
    let mut node = BoxNode::new("bern-inspector-host")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            width: Length::Full,
            height: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("body")));
    if spec.presented {
        node = node.child(
            BoxNode::new("bern-inspector")
                .with_role(AriaRole::Region)
                .with_layout(Layout {
                    direction: Axis::Vertical,
                    width: Length::Px(spec.width.unwrap_or(INSPECTOR_DEFAULT_WIDTH)),
                    height: Length::Full,
                    padding: Edges::all(12.0),
                    gap: 8.0,
                    flex_shrink: Some(0.0),
                    overflow: Overflow::Hidden,
                    ..Layout::default()
                })
                .with_style(VisualStyle {
                    background: Some(ColorValue::Role("sidebar.surface")),
                    border: Some(Border::sides(
                        1.0,
                        ColorValue::Role("separator"),
                        BorderSides::LEFT,
                    )),
                    ..VisualStyle::default()
                })
                .child(SurfaceNode::Slot(SlotId("panel")))
                .into_node(),
        );
    }
    node.into_node()
}
