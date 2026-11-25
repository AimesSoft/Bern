//! Content & feedback controls: `Text` (13-step type scale), `Label`,
//! `Progress`, `ProgressRing`, `Skeleton`, `Banner`, `Badge`, `EmptyState`,
//! and `Avatar`.

use crate::ir::{
    Align, AriaRole, Axis, Border, BoxNode, ColorValue, Corners, Edges, IconNode, Justify, Layout,
    Length, Overflow, Rgba8, RingNode, SlotId, SurfaceNode, TextNode, TrackNode, Typography,
    VisualStyle,
};

/// Bern's 13-step semantic type scale shared identically across Web and GPUI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextStyleKind {
    LargeTitle,
    Title,
    Title2,
    Title3,
    Headline,
    Subheadline,
    #[default]
    Body,
    Callout,
    Footnote,
    Caption,
    Caption2,
    Control,
    Code,
}

/// Resolve `(font_size, line_height, weight, monospace)` for a [`TextStyleKind`].
pub const fn text_scale(style: TextStyleKind) -> Typography {
    let (size, line, weight, monospace) = match style {
        TextStyleKind::LargeTitle => (28.0, 34.0, 700, false),
        TextStyleKind::Title => (24.0, 32.0, 600, false),
        TextStyleKind::Title2 => (20.0, 28.0, 600, false),
        TextStyleKind::Title3 => (17.0, 24.0, 600, false),
        TextStyleKind::Headline => (14.0, 20.0, 600, false),
        TextStyleKind::Subheadline => (13.0, 18.0, 400, false),
        TextStyleKind::Body => (14.0, 20.0, 400, false),
        TextStyleKind::Callout => (13.0, 20.0, 400, false),
        TextStyleKind::Footnote => (12.0, 16.0, 400, false),
        TextStyleKind::Caption => (12.0, 16.0, 500, false),
        TextStyleKind::Caption2 => (11.0, 16.0, 500, false),
        TextStyleKind::Control => (13.5, 20.0, 500, false),
        TextStyleKind::Code => (13.0, 20.0, 400, true),
    };
    Typography {
        size,
        line_height: Some(line),
        weight,
        monospace,
        tabular_nums: false,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextEmphasis {
    #[default]
    Primary,
    Secondary,
    Tertiary,
}

impl TextEmphasis {
    pub const fn role(self) -> &'static str {
        match self {
            TextEmphasis::Primary => "text.primary",
            TextEmphasis::Secondary => "text.secondary",
            TextEmphasis::Tertiary => "text.placeholder",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextSpec {
    pub content: String,
    pub style: TextStyleKind,
    pub emphasis: TextEmphasis,
    pub color: Option<&'static str>,
    pub size: Option<f32>,
    pub line_height: Option<f32>,
    pub bold: bool,
    pub lines: Option<usize>,
    pub selectable: bool,
}

pub fn text(spec: TextSpec) -> SurfaceNode {
    let mut typo = text_scale(spec.style);
    if let Some(size) = spec.size {
        let base_line = typo.line_height.unwrap_or(size * 1.35);
        typo.line_height = Some((size * base_line / typo.size).round());
        typo.size = size;
    }
    if let Some(line_height) = spec.line_height {
        typo.line_height = Some(line_height);
    }
    if spec.bold {
        typo.weight = 700;
    }
    let color = ColorValue::Role(spec.color.unwrap_or_else(|| spec.emphasis.role()));
    SurfaceNode::Text(TextNode {
        content: spec.content,
        typography: typo,
        color,
        lines: spec.lines,
        truncate: spec.lines == Some(1),
        selectable: spec.selectable,
    })
}

#[derive(Clone, Debug, Default)]
pub struct LabelSpec {
    pub title: String,
    pub icon: Option<&'static str>,
    pub style: TextStyleKind,
    pub color: Option<&'static str>,
    pub size: Option<f32>,
    pub icon_only: bool,
}

pub fn label(spec: LabelSpec) -> SurfaceNode {
    let mut typo = text_scale(spec.style);
    if let Some(size) = spec.size {
        let base_line = typo.line_height.unwrap_or(size * 1.35);
        typo.line_height = Some((size * base_line / typo.size).round());
        typo.size = size;
    }
    let color = ColorValue::Role(spec.color.unwrap_or("text.primary"));
    if spec.icon_only {
        return match spec.icon {
            Some(icon) => SurfaceNode::Icon(IconNode {
                name: icon,
                size: typo.size + 2.0,
                color,
            }),
            None => BoxNode::new("bern-label").into_node(),
        };
    }
    let mut node = BoxNode::new("bern-label").with_layout(Layout {
        direction: Axis::Horizontal,
        align: Align::Center,
        gap: 6.0,
        ..Layout::default()
    });
    if let Some(icon) = spec.icon {
        node = node.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: typo.size + 2.0,
            color,
        }));
    }
    node.child(SurfaceNode::Text(TextNode {
        content: spec.title,
        typography: typo,
        color,
        lines: None,
        truncate: false,
        selectable: false,
    }))
    .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct ProgressSpec {
    pub fraction: f32,
    pub label: Option<String>,
}

pub fn progress(spec: ProgressSpec) -> SurfaceNode {
    let track = SurfaceNode::Track(TrackNode {
        part: "bern-progress",
        axis: Axis::Horizontal,
        fraction: spec.fraction.clamp(0.0, 1.0),
        hit_extent: 4.0,
        min_main_extent: None,
        thickness: 4.0,
        radius: 2.0,
        track_color: ColorValue::Role("separator"),
        fill_color: ColorValue::Role("accent"),
        disabled: false,
        thumb: None,
    });
    let mut col = BoxNode::new("bern-progress-wrap")
        .with_role(AriaRole::ProgressBar)
        .with_layout(Layout {
            direction: Axis::Vertical,
            gap: 6.0,
            width: Length::Full,
            ..Layout::default()
        });
    if let Some(label) = spec.label {
        col = col.child(SurfaceNode::Text(TextNode {
            content: label,
            typography: text_scale(TextStyleKind::Caption),
            color: ColorValue::Role("text.secondary"),
            lines: None,
            truncate: false,
            selectable: false,
        }));
    }
    col.child(track).into_node()
}

#[derive(Clone, Copy, Debug)]
pub struct ProgressRingSpec {
    pub fraction: f32,
    pub diameter: f32,
    pub stroke_width: f32,
}

impl Default for ProgressRingSpec {
    fn default() -> Self {
        Self {
            fraction: 0.0,
            diameter: 24.0,
            stroke_width: 2.5,
        }
    }
}

pub fn progress_ring(spec: ProgressRingSpec) -> SurfaceNode {
    SurfaceNode::Ring(RingNode {
        fraction: spec.fraction.clamp(0.0, 1.0),
        diameter: spec.diameter,
        stroke_width: spec.stroke_width,
        color: ColorValue::Role("accent"),
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SkeletonShape {
    #[default]
    Rect,
    Capsule,
    Circle,
}

#[derive(Clone, Copy, Debug)]
pub struct SkeletonSpec {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub corner: f32,
    pub shape: SkeletonShape,
    pub base_opacity: f32,
}

impl Default for SkeletonSpec {
    fn default() -> Self {
        Self {
            width: None,
            height: Some(12.0),
            corner: 6.0,
            shape: SkeletonShape::Rect,
            base_opacity: 0.07,
        }
    }
}

pub fn skeleton(spec: SkeletonSpec) -> SurfaceNode {
    let corners = match spec.shape {
        SkeletonShape::Capsule | SkeletonShape::Circle => Corners::PILL,
        SkeletonShape::Rect => Corners::all(spec.corner),
    };
    BoxNode::new("bern-skeleton")
        .with_layout(Layout {
            width: spec.width.map(Length::Px).unwrap_or(Length::Full),
            height: spec.height.map(Length::Px).unwrap_or(Length::Auto),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::RoleAlpha(
                "text.primary",
                spec.base_opacity.clamp(0.0, 1.0),
            )),
            corners: Some(corners),
            ..VisualStyle::default()
        })
        .into_node()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BannerSeverity {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl BannerSeverity {
    pub fn parse(s: &str) -> Self {
        match s {
            "success" => Self::Success,
            "warning" => Self::Warning,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }

    pub const fn role(self) -> &'static str {
        match self {
            Self::Success => "status.success",
            Self::Warning => "status.warning",
            Self::Error => "status.error",
            Self::Info => "accent",
        }
    }

    pub const fn default_icon(self) -> &'static str {
        match self {
            Self::Success => "checkmark.circle",
            Self::Warning => "exclamationmark.triangle",
            Self::Error => "xmark.circle.fill",
            Self::Info => "info.circle",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BannerSpec {
    pub severity: BannerSeverity,
    pub title: Option<String>,
    pub content: String,
    pub icon: Option<&'static str>,
    pub has_actions: bool,
    pub dismissible: bool,
}

pub fn banner(spec: BannerSpec) -> SurfaceNode {
    let role = spec.severity.role();
    let icon = spec.icon.unwrap_or_else(|| spec.severity.default_icon());
    let mut body = BoxNode::new("bern-banner-body").with_layout(Layout {
        direction: Axis::Vertical,
        gap: 2.0,
        flex_grow: Some(1.0),
        min_width: Some(Length::Px(0.0)),
        ..Layout::default()
    });
    if let Some(title) = spec.title.filter(|t| !t.is_empty()) {
        body = body.child(SurfaceNode::Text(TextNode {
            content: title,
            typography: Typography {
                size: 13.5,
                line_height: Some(19.0),
                weight: 600,
                ..Typography::default()
            },
            color: ColorValue::Role("text.primary"),
            lines: None,
            truncate: false,
            selectable: false,
        }));
    }
    body = body.child(SurfaceNode::Text(TextNode {
        content: spec.content,
        typography: Typography {
            size: 13.0,
            line_height: Some(18.0),
            weight: 400,
            ..Typography::default()
        },
        color: ColorValue::Role("text.secondary"),
        lines: None,
        truncate: false,
        selectable: false,
    }));

    let mut row = BoxNode::new("bern-banner")
        .with_role(AriaRole::Status)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 12.0,
            padding: Edges::xy(14.0, 10.0),
            width: Length::Full,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::RoleAlpha(role, 0.12)),
            border: Some(Border::all(1.0, ColorValue::RoleAlpha(role, 0.28))),
            corners: Some(Corners::all(10.0)),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 16.0,
            color: ColorValue::Role(role),
        }))
        .child(body.into_node());
    if spec.has_actions {
        row = row.child(SurfaceNode::Slot(SlotId("actions")));
    }
    if spec.dismissible {
        row = row.child(SurfaceNode::Slot(SlotId("dismiss")));
    }
    row.into_node()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeTone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Error,
}

impl BadgeTone {
    pub fn parse(s: &str) -> Self {
        match s {
            "accent" => Self::Accent,
            "success" => Self::Success,
            "warning" => Self::Warning,
            "error" => Self::Error,
            _ => Self::Neutral,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BadgeSpec {
    pub content: String,
    pub tone: BadgeTone,
    pub icon: Option<&'static str>,
}

pub fn badge(spec: BadgeSpec) -> SurfaceNode {
    let (fg_role, tint_role, bg_alpha, border_alpha) = match spec.tone {
        BadgeTone::Accent => ("accent", "accent", 0.14, 0.28),
        BadgeTone::Success => ("status.success", "status.success", 0.14, 0.28),
        BadgeTone::Warning => ("status.warning", "status.warning", 0.14, 0.30),
        BadgeTone::Error => ("status.error", "status.error", 0.14, 0.30),
        BadgeTone::Neutral => ("text.secondary", "text.primary", 0.08, 0.16),
    };
    let mut pill = BoxNode::new("bern-badge")
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            gap: 4.0,
            padding: Edges::xy(8.0, 2.0),
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::RoleAlpha(tint_role, bg_alpha)),
            foreground: Some(ColorValue::Role(fg_role)),
            border: Some(Border::all(
                1.0,
                ColorValue::RoleAlpha(tint_role, border_alpha),
            )),
            corners: Some(Corners::PILL),
            typography: Some(Typography {
                size: 12.0,
                line_height: Some(15.0),
                weight: 600,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        });
    if let Some(icon) = spec.icon {
        pill = pill.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 11.5,
            color: ColorValue::Role(fg_role),
        }));
    }
    pill.child(SurfaceNode::Text(TextNode {
        content: spec.content,
        typography: Typography {
            size: 12.0,
            line_height: Some(15.0),
            weight: 600,
            ..Typography::default()
        },
        color: ColorValue::Role(fg_role),
        lines: Some(1),
        truncate: false,
        selectable: false,
    }))
    .into_node()
}

#[derive(Clone, Debug, Default)]
pub struct EmptyStateSpec {
    pub title: String,
    pub message: Option<String>,
    pub icon: Option<&'static str>,
    pub has_actions: bool,
}

pub fn empty_state(spec: EmptyStateSpec) -> SurfaceNode {
    let mut col = BoxNode::new("bern-empty-state").with_layout(Layout {
        direction: Axis::Vertical,
        align: Align::Center,
        justify: Justify::Center,
        gap: 10.0,
        padding: Edges::xy(24.0, 28.0),
        width: Length::Full,
        ..Layout::default()
    });
    if let Some(icon) = spec.icon {
        col = col.child(SurfaceNode::Icon(IconNode {
            name: icon,
            size: 38.0,
            color: ColorValue::Role("icon.secondary"),
        }));
    }
    col = col.child(SurfaceNode::Text(TextNode {
        content: spec.title,
        typography: Typography {
            size: 17.0,
            line_height: Some(24.0),
            weight: 600,
            ..Typography::default()
        },
        color: ColorValue::Role("text.primary"),
        lines: None,
        truncate: false,
        selectable: false,
    }));
    if let Some(message) = spec.message.filter(|m| !m.is_empty()) {
        col = col.child(SurfaceNode::Text(TextNode {
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
    if spec.has_actions {
        col = col.child(
            BoxNode::new("bern-empty-state-actions")
                .with_layout(Layout {
                    direction: Axis::Horizontal,
                    wrap: true,
                    align: Align::Center,
                    justify: Justify::Center,
                    gap: 8.0,
                    padding: Edges::trbl(4.0, 0.0, 0.0, 0.0),
                    ..Layout::default()
                })
                .child(SurfaceNode::Slot(SlotId("actions")))
                .into_node(),
        );
    }
    col.into_node()
}

#[derive(Clone, Debug)]
pub struct AvatarSpec {
    pub label: String,
    pub size: f32,
    pub rounded: bool,
    pub has_image: bool,
}

impl Default for AvatarSpec {
    fn default() -> Self {
        Self {
            label: String::new(),
            size: 40.0,
            rounded: false,
            has_image: false,
        }
    }
}

/// Deterministic accent hue swatch for initials avatars (identical on Web and GPUI).
pub fn avatar_color(label: &str) -> (Rgba8, Rgba8) {
    const PALETTE: [(u8, u8, u8); 6] = [
        (59, 130, 246),
        (16, 185, 129),
        (245, 158, 11),
        (139, 92, 246),
        (236, 72, 153),
        (20, 184, 166),
    ];
    let mut hash: u64 = 14695981039346656037;
    for b in label.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(1099511628211);
    }
    let (r, g, b) = PALETTE[(hash as usize) % PALETTE.len()];
    (Rgba8::new(r, g, b, 255), Rgba8::new(r, g, b, 56))
}

pub fn avatar_initials(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "?".into();
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() >= 2
        && let (Some(a), Some(b)) = (words[0].chars().next(), words[1].chars().next())
    {
        return format!("{a}{b}").to_uppercase();
    }
    let chars: Vec<char> = trimmed.chars().take(2).collect();
    if chars.len() == 2 && chars[0].is_ascii() && chars[1].is_ascii() {
        return format!("{}{}", chars[0], chars[1]).to_uppercase();
    }
    chars[0].to_uppercase().to_string()
}

pub fn avatar(spec: AvatarSpec) -> SurfaceNode {
    let side = spec.size.max(8.0);
    let radius = if spec.rounded {
        (side * 0.22).max(4.0)
    } else {
        side * 0.5
    };
    let (fg, bg) = avatar_color(&spec.label);
    let mut node = BoxNode::new("bern-avatar")
        .with_role(AriaRole::Img)
        .with_layout(Layout {
            direction: Axis::Horizontal,
            align: Align::Center,
            justify: Justify::Center,
            width: Length::Px(side),
            height: Length::Px(side),
            flex_shrink: Some(0.0),
            overflow: Overflow::Hidden,
            ..Layout::default()
        })
        .with_style(VisualStyle {
            background: Some(ColorValue::Rgba(bg)),
            foreground: Some(ColorValue::Rgba(fg)),
            corners: Some(Corners::all(radius)),
            typography: Some(Typography {
                size: (side * 0.38).max(10.0),
                line_height: None,
                weight: 600,
                ..Typography::default()
            }),
            ..VisualStyle::default()
        })
        .child(SurfaceNode::Text(TextNode {
            content: avatar_initials(&spec.label),
            typography: Typography {
                size: (side * 0.38).max(10.0),
                line_height: None,
                weight: 600,
                ..Typography::default()
            },
            color: ColorValue::Rgba(fg),
            lines: Some(1),
            truncate: false,
            selectable: false,
        }));
    if spec.has_image {
        node = node.child(SurfaceNode::Slot(SlotId("image")));
    }
    node.into_node()
}
