//! `Slider` and `MediaScrubber` lowering.
//!
//! Both horizontal and vertical sliders render a sleek 4 pt rail with a solid
//! white perpendicular capsule thumb (`7 × 18 pt` on horizontal tracks,
//! `18 × 7 pt` on vertical tracks), a subtle hairline border, and a soft
//! elevation shadow in both light and dark modes.

use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Cursor, Edges, IconNode, Justify,
    Layout, Length, Shadow, StateOverrides, SurfaceNode, TextNode, TrackNode, Typography,
    VisualStyle,
};

/// Rail thickness in logical points.
pub const SLIDER_TRACK_THICKNESS: f32 = 4.0;
/// Rail corner radius (full pill for 4 pt track).
pub const SLIDER_TRACK_RADIUS: f32 = 2.0;
/// Interactive hit-target height (or width when vertical) in points.
pub const SLIDER_HIT_EXTENT: f32 = 24.0;
/// Minimum length of a vertical slider track in points.
pub const SLIDER_VERTICAL_MIN_LENGTH: f32 = 120.0;
/// Thumb narrow dimension (width on horizontal track, height on vertical track).
pub const SLIDER_THUMB_THICKNESS: f32 = 7.0;
/// Thumb long dimension (height on horizontal track, width on vertical track).
pub const SLIDER_THUMB_LENGTH: f32 = 18.0;
/// Full capsule corner radius for the thumb.
pub const SLIDER_THUMB_RADIUS: f32 = 999.0;
/// Hover / active drag scale factor for the thumb.
pub const SLIDER_THUMB_SCALE_ACTIVE: f32 = 1.12;

/// Input specification for lowering a `Slider`.
#[derive(Clone, Debug)]
pub struct SliderSpec {
    pub label: Option<String>,
    pub label_hidden: bool,
    pub axis: Axis,
    pub fraction: f32,
    pub enabled: bool,
    pub tint: Option<&'static str>,
    pub min_icon: Option<&'static str>,
    pub max_icon: Option<&'static str>,
}

impl Default for SliderSpec {
    fn default() -> Self {
        Self {
            label: None,
            label_hidden: false,
            axis: Axis::Horizontal,
            fraction: 0.0,
            enabled: true,
            tint: None,
            min_icon: None,
            max_icon: None,
        }
    }
}

/// Canonical white perpendicular capsule thumb box (`7 × 18` horizontal,
/// `18 × 7` vertical) with subtle border and soft drop shadow.
pub fn slider_thumb(axis: Axis, enabled: bool) -> BoxNode {
    let (width, height) = match axis {
        Axis::Horizontal => (SLIDER_THUMB_THICKNESS, SLIDER_THUMB_LENGTH),
        Axis::Vertical => (SLIDER_THUMB_LENGTH, SLIDER_THUMB_THICKNESS),
    };
    let style = if enabled {
        VisualStyle {
            background: Some(ColorValue::Role("slider.thumb")),
            border: Some(Border::all(1.0, ColorValue::Role("slider.thumb.border"))),
            corners: Some(Corners::PILL),
            shadows: vec![Shadow::drop(
                1.0,
                4.0,
                ColorValue::Role("slider.thumb.shadow"),
            )],
            cursor: Some(Cursor::Pointer),
            ..VisualStyle::default()
        }
    } else {
        VisualStyle {
            background: Some(ColorValue::Role("surface.raised")),
            border: Some(Border::all(1.0, ColorValue::Role("disabled.foreground"))),
            corners: Some(Corners::PILL),
            shadows: Vec::new(),
            opacity: Some(0.85),
            ..VisualStyle::default()
        }
    };
    let hover = VisualStyle {
        scale: Some((SLIDER_THUMB_SCALE_ACTIVE, SLIDER_THUMB_SCALE_ACTIVE)),
        ..VisualStyle::default()
    };
    BoxNode::new("bern-slider-thumb")
        .with_layout(Layout {
            width: Length::Px(width),
            height: Length::Px(height),
            ..Layout::default()
        })
        .with_style(style)
        .with_states(StateOverrides {
            hover: enabled.then_some(hover.clone()),
            pressed: enabled.then_some(hover),
            focus_ring: Some(ColorValue::Role("focus.indicator")),
            ..StateOverrides::default()
        })
}

/// Lower a `Slider` into a [`SurfaceNode`].
pub fn slider(spec: SliderSpec) -> SurfaceNode {
    let fraction = spec.fraction.clamp(0.0, 1.0);
    let fill_color = if spec.enabled {
        ColorValue::Role(spec.tint.unwrap_or("accent"))
    } else {
        ColorValue::Role("disabled.foreground")
    };
    let track_color = if spec.enabled {
        ColorValue::Role("separator")
    } else {
        ColorValue::Role("disabled.surface")
    };
    let track = SurfaceNode::Track(TrackNode {
        part: if spec.axis == Axis::Vertical {
            "bern-slider-vertical"
        } else {
            "bern-slider"
        },
        axis: spec.axis,
        fraction,
        hit_extent: SLIDER_HIT_EXTENT,
        min_main_extent: (spec.axis == Axis::Vertical).then_some(SLIDER_VERTICAL_MIN_LENGTH),
        thickness: SLIDER_TRACK_THICKNESS,
        radius: SLIDER_TRACK_RADIUS,
        track_color,
        fill_color,
        disabled: !spec.enabled,
        thumb: Some(slider_thumb(spec.axis, spec.enabled)),
    });

    let icon_color = ColorValue::Role("icon.secondary");
    if spec.axis == Axis::Vertical {
        let mut col = BoxNode::new("bern-slider-track-vertical")
            .with_role(AriaRole::Slider)
            .with_layout(Layout {
                direction: Axis::Vertical,
                align: Align::Center,
                justify: Justify::Center,
                gap: 8.0,
                height: Length::Full,
                ..Layout::default()
            });
        if !spec.label_hidden
            && let Some(label) = spec.label
        {
            col = col.child(SurfaceNode::Text(TextNode {
                content: label,
                typography: Typography {
                    size: 13.0,
                    line_height: Some(18.0),
                    weight: 400,
                    ..Typography::default()
                },
                color: ColorValue::Role("text.primary"),
                lines: None,
                truncate: false,
                selectable: false,
            }));
        }
        if let Some(max_icon) = spec.max_icon {
            col = col.child(SurfaceNode::Icon(IconNode {
                name: max_icon,
                size: 16.0,
                color: icon_color,
            }));
        }
        col = col.child(track);
        if let Some(min_icon) = spec.min_icon {
            col = col.child(SurfaceNode::Icon(IconNode {
                name: min_icon,
                size: 16.0,
                color: icon_color,
            }));
        }
        return col.into_node();
    }

    let mut row = BoxNode::new("bern-slider-track")
        .with_role(AriaRole::Slider)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 8.0,
            width: Length::Full,
            ..Layout::default()
        });
    if let Some(min_icon) = spec.min_icon {
        row = row.child(SurfaceNode::Icon(IconNode {
            name: min_icon,
            size: 16.0,
            color: icon_color,
        }));
    }
    row = row.child(track);
    if let Some(max_icon) = spec.max_icon {
        row = row.child(SurfaceNode::Icon(IconNode {
            name: max_icon,
            size: 16.0,
            color: icon_color,
        }));
    }

    if spec.label_hidden || spec.label.is_none() {
        row.into_node()
    } else {
        BoxNode::new("bern-slider-field")
            .with_layout(Layout {
                direction: Axis::Vertical,
                gap: 6.0,
                width: Length::Full,
                padding: Edges::ZERO,
                ..Layout::default()
            })
            .child(SurfaceNode::Text(TextNode {
                content: spec.label.unwrap_or_default(),
                typography: Typography {
                    size: 14.0,
                    line_height: Some(20.0),
                    weight: 400,
                    ..Typography::default()
                },
                color: ColorValue::Role("text.primary"),
                lines: None,
                truncate: false,
                selectable: false,
            }))
            .child(row.into_node())
            .into_node()
    }
}

/// Neutral media playback scrubber (shared by video control bars).
#[derive(Clone, Copy, Debug)]
pub struct MediaScrubberSpec {
    pub fraction: f32,
    pub enabled: bool,
    pub tint: Option<&'static str>,
}

impl Default for MediaScrubberSpec {
    fn default() -> Self {
        Self {
            fraction: 0.0,
            enabled: true,
            tint: None,
        }
    }
}

pub fn media_scrubber(spec: MediaScrubberSpec) -> SurfaceNode {
    SurfaceNode::Track(TrackNode {
        part: "bern-media-scrubber",
        axis: Axis::Horizontal,
        fraction: spec.fraction.clamp(0.0, 1.0),
        hit_extent: 22.0,
        min_main_extent: None,
        thickness: SLIDER_TRACK_THICKNESS,
        radius: SLIDER_TRACK_RADIUS,
        track_color: ColorValue::Role("media.track"),
        fill_color: ColorValue::Role(spec.tint.unwrap_or("media.played")),
        disabled: !spec.enabled,
        thumb: Some(slider_thumb(Axis::Horizontal, spec.enabled)),
    })
}
