//! Form, section, divider, list, drop area, and control group lowering.

use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Edges, Layout, Length, Overflow,
    SlotId, StateOverrides, SurfaceNode, TextNode, Typography, VisualStyle,
};

pub const SECTION_CARD_CORNER: f32 = 12.0;
pub const SECTION_ROW_MIN_HEIGHT: f32 = 44.0;
pub const SECTION_ROW_PADDING_X: f32 = 16.0;
pub const SECTION_ROW_PADDING_Y: f32 = 10.0;

#[derive(Clone, Debug, Default)]
pub struct FormSpec {
    pub title: Option<String>,
    pub gap: f32,
}

pub fn form(spec: FormSpec) -> SurfaceNode {
    let mut inner = BoxNode::new("bern-form").with_layout(Layout {
        direction: Axis::Vertical,
        gap: spec.gap,
        padding: Edges::all(16.0),
        overflow: Overflow::ScrollY,
        ..Layout::default()
    });
    if let Some(title) = spec.title {
        inner = inner.child(SurfaceNode::Text(TextNode {
            content: title,
            typography: Typography {
                size: 24.0,
                line_height: Some(32.0),
                weight: 600,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: None,
            truncate: false,
            selectable: false,
        }));
    }
    inner
        .child(SurfaceNode::Slot(SlotId("content")))
        .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct SectionSpec {
    pub title: Option<String>,
    pub footer: Option<String>,
    pub plain: bool,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub row_height: Option<f32>,
}

pub fn section(spec: SectionSpec) -> SurfaceNode {
    let mut body = BoxNode::new(if spec.plain {
        "bern-group bern-group-plain"
    } else {
        "bern-group"
    })
    .with_layout(Layout {
        direction: Axis::Vertical,
        width: Length::Full,
        overflow: if spec.plain {
            Overflow::Visible
        } else {
            Overflow::Hidden
        },
        ..Layout::default()
    });
    if !spec.plain {
        body = body.with_style(VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            border: Some(Border::all(1.0, ColorValue::Role("separator"))),
            corners: Some(Corners::all(SECTION_CARD_CORNER)),
            ..VisualStyle::default()
        });
    }
    body = body.child(SurfaceNode::Slot(SlotId("rows")));

    let caption = |text: String| {
        SurfaceNode::Text(TextNode {
            content: text,
            typography: Typography {
                size: 12.0,
                line_height: Some(16.0),
                weight: 500,
                ..Typography::default()
            },
            color: ColorValue::Role("text.secondary"),
            lines: None,
            truncate: false,
            selectable: false,
        })
    };

    let mut outer = BoxNode::new("bern-section").with_layout(Layout {
        direction: Axis::Vertical,
        gap: 6.0,
        width: Length::Full,
        ..Layout::default()
    });
    if let Some(title) = spec.title {
        outer = outer.child(caption(title));
    }
    outer = outer.child(body.into_node());
    if let Some(footer) = spec.footer {
        outer = outer.child(caption(footer));
    }
    outer.into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DividerSpec;

pub fn divider(_spec: DividerSpec) -> SurfaceNode {
    BoxNode::new("bern-divider")
        .with_role(AriaRole::Separator)
        .with_layout(Layout {
            width: Length::Full,
            height: Length::Px(1.0),
            flex_shrink: Some(0.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Role("separator")),
            ..VisualStyle::default()
        })
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ListSpec {
    pub gap: f32,
}

pub fn list(spec: ListSpec) -> SurfaceNode {
    BoxNode::new("bern-list")
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: spec.gap,
            width: Length::Full,
            ..Layout::default()
        })
        .child(SurfaceNode::Slot(SlotId("rows")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DropAreaSpec {
    pub active: bool,
}

pub fn drop_area(spec: DropAreaSpec) -> SurfaceNode {
    BoxNode::new("bern-drop")
        .with_layout(Layout {
            direction: Axis::Vertical,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: spec.active.then_some(ColorValue::Role("hover.overlay")),
            corners: Some(Corners::all(8.0)),
            ..VisualStyle::default()
        })
        .with_states(StateOverrides {
            hover: Some(VisualStyle {
                background: Some(ColorValue::Role("hover.overlay")),
                ..VisualStyle::default()
            }),
            ..StateOverrides::default()
        })
        .child(SurfaceNode::Slot(SlotId("content")))
        .into_node()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ControlGroupSpec {
    pub height: Option<f32>,
}

pub fn control_group(spec: ControlGroupSpec) -> SurfaceNode {
    BoxNode::new("bern-control-group")
        .with_role(AriaRole::Group)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 2.0,
            padding: Edges::all(2.0),
            height: spec.height.map(Length::Px).unwrap_or(Length::Auto),
            flex_shrink: Some(0.0),
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
            corners: Some(Corners::PILL),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Slot(SlotId("content")))
        .into_node()
}
