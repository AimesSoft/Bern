//! Compact intermediate representation of an animated vector scene.

/// Binary header of a precompiled `BSVG` scene (`BSVG` + version 1).
pub const MAGIC: &[u8; 8] = b"BSVG\x00\x01\x00\x00";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub valid: bool,
}

impl BBox {
    pub fn from_min_max(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        if min_x.is_finite() && min_y.is_finite() && max_x >= min_x && max_y >= min_y {
            Self {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
                valid: true,
            }
        } else {
            Self::default()
        }
    }

    pub fn union(self, other: Self) -> Self {
        match (self.valid, other.valid) {
            (false, _) => other,
            (_, false) => self,
            (true, true) => {
                let min_x = self.x.min(other.x);
                let min_y = self.y.min(other.y);
                let max_x = (self.x + self.width).max(other.x + other.width);
                let max_y = (self.y + self.height).max(other.y + other.height);
                Self::from_min_max(min_x, min_y, max_x, max_y)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColorSpec {
    None,
    Rgba([u8; 4]),
    CurrentColor,
    LightDark(Box<(ColorSpec, ColorSpec)>),
    Gradient(u16),
}

impl ColorSpec {
    pub fn resolve_rgba(&self, dark: bool, current: [u8; 4]) -> Option<[u8; 4]> {
        match self {
            Self::None | Self::Gradient(_) => None,
            Self::Rgba(c) => Some(*c),
            Self::CurrentColor => Some(current),
            Self::LightDark(pair) => {
                if dark {
                    pair.1.resolve_rgba(dark, current)
                } else {
                    pair.0.resolve_rgba(dark, current)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientKind {
    Linear {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    Radial {
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: ColorSpec,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientDef {
    pub kind: GradientKind,
    pub units: GradientUnits,
    pub transform: [f32; 6],
    pub stops: Vec<GradientStop>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSeg {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShapeKind {
    Group,
    Path(Vec<PathSeg>),
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rx: f32,
        ry: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipItem {
    pub shape: ShapeKind,
    pub transform: [f32; 6],
    pub fill_rule: FillRule,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipDef {
    pub items: Vec<ClipItem>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32),
    SkewX(f32),
    SkewY(f32),
    Matrix([f32; 6]),
}

impl TransformOp {
    pub fn identity_like(&self) -> Self {
        match self {
            Self::Translate(..) => Self::Translate(0.0, 0.0),
            Self::Scale(..) => Self::Scale(1.0, 1.0),
            Self::Rotate(..) => Self::Rotate(0.0),
            Self::SkewX(..) => Self::SkewX(0.0),
            Self::SkewY(..) => Self::SkewY(0.0),
            Self::Matrix(..) => Self::Matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
        }
    }

    pub fn same_kind(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub fn lerp(self, other: Self, t: f32) -> Option<Self> {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        match (self, other) {
            (Self::Translate(ax, ay), Self::Translate(bx, by)) => {
                Some(Self::Translate(mix(ax, bx), mix(ay, by)))
            }
            (Self::Scale(ax, ay), Self::Scale(bx, by)) => {
                Some(Self::Scale(mix(ax, bx), mix(ay, by)))
            }
            (Self::Rotate(a), Self::Rotate(b)) => Some(Self::Rotate(mix(a, b))),
            (Self::SkewX(a), Self::SkewX(b)) => Some(Self::SkewX(mix(a, b))),
            (Self::SkewY(a), Self::SkewY(b)) => Some(Self::SkewY(mix(a, b))),
            _ => None,
        }
    }

    pub fn to_matrix(self) -> [f32; 6] {
        match self {
            Self::Translate(tx, ty) => [1.0, 0.0, 0.0, 1.0, tx, ty],
            Self::Scale(sx, sy) => [sx, 0.0, 0.0, sy, 0.0, 0.0],
            Self::Rotate(deg) => {
                let rad = deg.to_radians();
                let (s, c) = rad.sin_cos();
                [c, s, -s, c, 0.0, 0.0]
            }
            Self::SkewX(deg) => [1.0, 0.0, deg.to_radians().tan(), 1.0, 0.0, 0.0],
            Self::SkewY(deg) => [1.0, deg.to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            Self::Matrix(m) => m,
        }
    }
}

/// Multiply two 2D affine matrices `[a, b, c, d, tx, ty]` (`self * rhs`).
pub fn mul_affine(lhs: [f32; 6], rhs: [f32; 6]) -> [f32; 6] {
    [
        lhs[0] * rhs[0] + lhs[2] * rhs[1],
        lhs[1] * rhs[0] + lhs[3] * rhs[1],
        lhs[0] * rhs[2] + lhs[2] * rhs[3],
        lhs[1] * rhs[2] + lhs[3] * rhs[3],
        lhs[0] * rhs[4] + lhs[2] * rhs[5] + lhs[4],
        lhs[1] * rhs[4] + lhs[3] * rhs[5] + lhs[5],
    ]
}

pub fn ops_to_matrix(ops: &[TransformOp]) -> [f32; 6] {
    let mut m = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    for op in ops {
        m = mul_affine(m, op.to_matrix());
    }
    m
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformBox {
    ViewBox,
    FillBox,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CoordVal {
    Px(f32),
    Percent(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OriginSpec {
    pub x: CoordVal,
    pub y: CoordVal,
}

impl Default for OriginSpec {
    fn default() -> Self {
        Self {
            x: CoordVal::Px(0.0),
            y: CoordVal::Px(0.0),
        }
    }
}

impl OriginSpec {
    pub fn resolve(self, box_mode: TransformBox, fill_box: BBox, view_box: ViewBox) -> (f32, f32) {
        let (rx, ry, rw, rh) = match box_mode {
            TransformBox::FillBox if fill_box.valid => {
                (fill_box.x, fill_box.y, fill_box.width, fill_box.height)
            }
            _ => (0.0, 0.0, view_box.width, view_box.height),
        };
        let ox = match self.x {
            CoordVal::Px(v) => rx + v,
            CoordVal::Percent(p) => rx + (p * 0.01) * rw,
        };
        let oy = match self.y {
            CoordVal::Px(v) => ry + v,
            CoordVal::Percent(p) => ry + (p * 0.01) * rh,
        };
        (ox, oy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    CubicBezier(f32, f32, f32, f32),
    Steps(u16, bool),
}

impl Easing {
    pub const EASE: Self = Self::CubicBezier(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: Self = Self::CubicBezier(0.42, 0.0, 1.0, 1.0);
    pub const EASE_OUT: Self = Self::CubicBezier(0.0, 0.0, 0.58, 1.0);
    pub const EASE_IN_OUT: Self = Self::CubicBezier(0.42, 0.0, 0.58, 1.0);

    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::Steps(steps, jump_start) => {
                let n = steps.max(1) as f32;
                if jump_start {
                    ((t * n).ceil() / n).clamp(0.0, 1.0)
                } else {
                    ((t * n).floor() / n).clamp(0.0, 1.0)
                }
            }
            Self::CubicBezier(x1, y1, x2, y2) => sample_cubic_bezier(x1, y1, x2, y2, t),
        }
    }
}

fn sample_cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ax = 3.0 * x1 - 3.0 * x2 + 1.0;
    let bx = 3.0 * x2 - 6.0 * x1;
    let cx = 3.0 * x1;
    let ay = 3.0 * y1 - 3.0 * y2 + 1.0;
    let by = 3.0 * y2 - 6.0 * y1;
    let cy = 3.0 * y1;

    let mut u = x;
    for _ in 0..6 {
        let xu = ((ax * u + bx) * u + cx) * u - x;
        if xu.abs() < 1e-6 {
            return ((ay * u + by) * u + cy) * u;
        }
        let dx = (3.0 * ax * u + 2.0 * bx) * u + cx;
        if dx.abs() < 1e-6 {
            break;
        }
        u = (u - xu / dx).clamp(0.0, 1.0);
    }
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    u = x;
    for _ in 0..12 {
        let xu = ((ax * u + bx) * u + cx) * u;
        if (xu - x).abs() < 1e-6 {
            break;
        }
        if x > xu {
            lo = u;
        } else {
            hi = u;
        }
        u = 0.5 * (lo + hi);
    }
    ((ay * u + by) * u + cy) * u
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Iterations {
    Count(f32),
    Infinite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimFill {
    None,
    Forwards,
    Backwards,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionProp {
    All,
    Fill,
    Stroke,
    Opacity,
    Transform,
    StrokeWidth,
    StrokeDashoffset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionSpec {
    pub prop: TransitionProp,
    pub duration: f32,
    pub delay: f32,
    pub easing: Easing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StyleProp {
    Fill(ColorSpec),
    FillOpacity(f32),
    FillRule(FillRule),
    Stroke(ColorSpec),
    StrokeWidth(f32),
    StrokeOpacity(f32),
    StrokeLinecap(LineCap),
    StrokeLinejoin(LineJoin),
    StrokeDasharray(Vec<f32>),
    StrokeDashoffset(f32),
    Opacity(f32),
    DisplayNone(bool),
    Transform(Vec<TransformOp>),
    TransformBox(TransformBox),
    TransformOrigin(OriginSpec),
    AnimName(Option<String>),
    AnimDuration(f32),
    AnimDelay(f32),
    AnimTiming(Easing),
    AnimIterations(Iterations),
    AnimDirection(AnimDirection),
    AnimFill(AnimFill),
    Transitions(Vec<TransitionSpec>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCond {
    Always,
    SchemeDark,
    SchemeLight,
    ReducedMotion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompoundSel {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub hover: bool,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorStep {
    pub compound: CompoundSel,
    /// True for `>`, false for descendant combinator ` `.
    pub direct_parent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleRule {
    pub media: MediaCond,
    pub steps: Vec<SelectorStep>,
    pub specificity: (u16, u16, u16),
    pub order: u32,
    pub declarations: Vec<(StyleProp, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackProp {
    Transform,
    Opacity,
    Fill,
    Stroke,
    StrokeWidth,
    StrokeDashoffset,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnimValue {
    Transform(Vec<TransformOp>),
    Scalar(f32),
    Color(ColorSpec),
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyframeStop {
    pub offset: f32,
    pub value: AnimValue,
    pub easing: Option<Easing>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyframeTrack {
    pub prop: TrackProp,
    pub stops: Vec<KeyframeStop>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes {
    pub name: String,
    pub tracks: Vec<KeyframeTrack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmilAttr {
    R,
    Rx,
    Ry,
    Cx,
    Cy,
    X,
    Y,
    Width,
    Height,
    Opacity,
    StrokeWidth,
    StrokeDashoffset,
    Fill,
    Stroke,
    TransformTranslate,
    TransformScale,
    TransformRotate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SmilValue {
    Scalar(f32),
    Pair(f32, f32),
    Rotate(f32, Option<(f32, f32)>),
    Color(ColorSpec),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmilAnim {
    pub attr: SmilAttr,
    pub values: Vec<SmilValue>,
    pub key_times: Vec<f32>,
    pub splines: Vec<Easing>,
    pub discrete: bool,
    pub begin: f32,
    pub dur: f32,
    pub iterations: Iterations,
    pub freeze: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimedClass {
    pub class: Option<String>,
    pub duration: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionAction {
    Toggle(String),
    Cycle(Vec<TimedClass>),
    Trigger(TimedClass),
    Random(Vec<TimedClass>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Interaction {
    pub ready_after: f32,
    pub on_click: Option<InteractionAction>,
    pub on_dblclick: Option<InteractionAction>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    pub parent: Option<u16>,
    pub children: Vec<u16>,
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub shape: ShapeKind,
    pub fill_box: BBox,
    pub clip: Option<u16>,
    /// Static SVG `transform="..."` attribute on this element.
    pub svg_transform: Vec<TransformOp>,
    /// Presentation attributes and inline `style="..."` (`(prop, is_inline, important)`).
    pub base_props: Vec<(StyleProp, bool, bool)>,
    pub smil: Vec<SmilAnim>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneData {
    pub view_box: ViewBox,
    pub nodes: Vec<SceneNode>,
    pub root_children: Vec<u16>,
    pub gradients: Vec<GradientDef>,
    pub clips: Vec<ClipDef>,
    pub rules: Vec<StyleRule>,
    pub keyframes: Vec<Keyframes>,
    pub interaction: Interaction,
}
