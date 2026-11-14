//! Binary `BSVG` codec: encodes compiled scenes at build time and decodes them
//! at runtime without pulling in XML or CSS parsers.

use crate::model::*;

#[cfg(feature = "svg")]
pub fn encode(scene: &SceneData) -> Vec<u8> {
    let mut w = Encoder {
        buf: Vec::with_capacity(4096),
    };
    w.buf.extend_from_slice(MAGIC);
    w.view_box(scene.view_box);
    w.vec(&scene.root_children, |w, &v| w.u16(v));
    w.vec(&scene.gradients, Encoder::gradient);
    w.vec(&scene.clips, Encoder::clip);
    w.vec(&scene.nodes, Encoder::node);
    w.vec(&scene.rules, Encoder::rule);
    w.vec(&scene.keyframes, Encoder::keyframes);
    w.interaction(&scene.interaction);
    w.buf
}

pub fn decode(bytes: &[u8]) -> Result<SceneData, String> {
    let rest = bytes
        .strip_prefix(MAGIC)
        .ok_or_else(|| "invalid BSVG header".to_string())?;
    let mut r = Decoder {
        bytes: rest,
        pos: 0,
    };
    let view_box = r.view_box()?;
    let root_children = r.vec(Decoder::u16)?;
    let gradients = r.vec(Decoder::gradient)?;
    let clips = r.vec(Decoder::clip)?;
    let nodes = r.vec(Decoder::node)?;
    let rules = r.vec(Decoder::rule)?;
    let keyframes = r.vec(Decoder::keyframes)?;
    let interaction = r.interaction()?;
    if r.pos != r.bytes.len() {
        return Err("trailing bytes in BSVG stream".into());
    }
    Ok(SceneData {
        view_box,
        nodes,
        root_children,
        gradients,
        clips,
        rules,
        keyframes,
        interaction,
    })
}

#[cfg(feature = "svg")]
struct Encoder {
    buf: Vec<u8>,
}

#[cfg(feature = "svg")]
impl Encoder {
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
    }
    fn opt<T>(&mut self, v: &Option<T>, f: impl FnOnce(&mut Self, &T)) {
        match v {
            None => self.u8(0),
            Some(x) => {
                self.u8(1);
                f(self, x);
            }
        }
    }
    fn vec<T>(&mut self, items: &[T], mut f: impl FnMut(&mut Self, &T)) {
        self.u32(items.len() as u32);
        for item in items {
            f(self, item);
        }
    }
    fn matrix(&mut self, m: &[f32; 6]) {
        for &v in m {
            self.f32(v);
        }
    }
    fn view_box(&mut self, vb: ViewBox) {
        self.f32(vb.x);
        self.f32(vb.y);
        self.f32(vb.width);
        self.f32(vb.height);
    }
    fn bbox(&mut self, b: BBox) {
        self.bool(b.valid);
        if b.valid {
            self.f32(b.x);
            self.f32(b.y);
            self.f32(b.width);
            self.f32(b.height);
        }
    }
    fn color(&mut self, c: &ColorSpec) {
        match c {
            ColorSpec::None => self.u8(0),
            ColorSpec::Rgba(rgba) => {
                self.u8(1);
                self.buf.extend_from_slice(rgba);
            }
            ColorSpec::CurrentColor => self.u8(2),
            ColorSpec::LightDark(pair) => {
                self.u8(3);
                self.color(&pair.0);
                self.color(&pair.1);
            }
            ColorSpec::Gradient(idx) => {
                self.u8(4);
                self.u16(*idx);
            }
        }
    }
    fn gradient(&mut self, g: &GradientDef) {
        match g.kind {
            GradientKind::Linear { x1, y1, x2, y2 } => {
                self.u8(0);
                self.f32(x1);
                self.f32(y1);
                self.f32(x2);
                self.f32(y2);
            }
            GradientKind::Radial { cx, cy, r, fx, fy } => {
                self.u8(1);
                self.f32(cx);
                self.f32(cy);
                self.f32(r);
                self.f32(fx);
                self.f32(fy);
            }
        }
        self.u8(match g.units {
            GradientUnits::ObjectBoundingBox => 0,
            GradientUnits::UserSpaceOnUse => 1,
        });
        self.matrix(&g.transform);
        self.vec(&g.stops, |w, s| {
            w.f32(s.offset);
            w.color(&s.color);
            w.f32(s.opacity);
        });
    }
    fn shape(&mut self, s: &ShapeKind) {
        match s {
            ShapeKind::Group => self.u8(0),
            ShapeKind::Path(segs) => {
                self.u8(1);
                self.vec(segs, |w, seg| match *seg {
                    PathSeg::Move(x, y) => {
                        w.u8(0);
                        w.f32(x);
                        w.f32(y);
                    }
                    PathSeg::Line(x, y) => {
                        w.u8(1);
                        w.f32(x);
                        w.f32(y);
                    }
                    PathSeg::Quad(x1, y1, x, y) => {
                        w.u8(2);
                        w.f32(x1);
                        w.f32(y1);
                        w.f32(x);
                        w.f32(y);
                    }
                    PathSeg::Cubic(x1, y1, x2, y2, x, y) => {
                        w.u8(3);
                        w.f32(x1);
                        w.f32(y1);
                        w.f32(x2);
                        w.f32(y2);
                        w.f32(x);
                        w.f32(y);
                    }
                    PathSeg::Close => w.u8(4),
                });
            }
            &ShapeKind::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
            } => {
                self.u8(2);
                self.f32(x);
                self.f32(y);
                self.f32(width);
                self.f32(height);
                self.f32(rx);
                self.f32(ry);
            }
            &ShapeKind::Circle { cx, cy, r } => {
                self.u8(3);
                self.f32(cx);
                self.f32(cy);
                self.f32(r);
            }
            &ShapeKind::Ellipse { cx, cy, rx, ry } => {
                self.u8(4);
                self.f32(cx);
                self.f32(cy);
                self.f32(rx);
                self.f32(ry);
            }
            &ShapeKind::Line { x1, y1, x2, y2 } => {
                self.u8(5);
                self.f32(x1);
                self.f32(y1);
                self.f32(x2);
                self.f32(y2);
            }
        }
    }
    fn fill_rule(&mut self, r: FillRule) {
        self.u8(match r {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        });
    }
    fn clip(&mut self, c: &ClipDef) {
        self.vec(&c.items, |w, item| {
            w.shape(&item.shape);
            w.matrix(&item.transform);
            w.fill_rule(item.fill_rule);
        });
    }
    fn transform_op(&mut self, op: &TransformOp) {
        match *op {
            TransformOp::Translate(x, y) => {
                self.u8(0);
                self.f32(x);
                self.f32(y);
            }
            TransformOp::Scale(x, y) => {
                self.u8(1);
                self.f32(x);
                self.f32(y);
            }
            TransformOp::Rotate(a) => {
                self.u8(2);
                self.f32(a);
            }
            TransformOp::SkewX(a) => {
                self.u8(3);
                self.f32(a);
            }
            TransformOp::SkewY(a) => {
                self.u8(4);
                self.f32(a);
            }
            TransformOp::Matrix(m) => {
                self.u8(5);
                self.matrix(&m);
            }
        }
    }
    fn coord(&mut self, c: CoordVal) {
        match c {
            CoordVal::Px(v) => {
                self.u8(0);
                self.f32(v);
            }
            CoordVal::Percent(p) => {
                self.u8(1);
                self.f32(p);
            }
        }
    }
    fn easing(&mut self, e: Easing) {
        match e {
            Easing::Linear => self.u8(0),
            Easing::CubicBezier(a, b, c, d) => {
                self.u8(1);
                self.f32(a);
                self.f32(b);
                self.f32(c);
                self.f32(d);
            }
            Easing::Steps(n, jump_start) => {
                self.u8(2);
                self.u16(n);
                self.bool(jump_start);
            }
        }
    }
    fn iterations(&mut self, it: Iterations) {
        match it {
            Iterations::Count(n) => {
                self.u8(0);
                self.f32(n);
            }
            Iterations::Infinite => self.u8(1),
        }
    }
    fn style_prop(&mut self, p: &StyleProp) {
        match p {
            StyleProp::Fill(c) => {
                self.u8(0);
                self.color(c);
            }
            StyleProp::FillOpacity(v) => {
                self.u8(1);
                self.f32(*v);
            }
            StyleProp::FillRule(r) => {
                self.u8(2);
                self.fill_rule(*r);
            }
            StyleProp::Stroke(c) => {
                self.u8(3);
                self.color(c);
            }
            StyleProp::StrokeWidth(v) => {
                self.u8(4);
                self.f32(*v);
            }
            StyleProp::StrokeOpacity(v) => {
                self.u8(5);
                self.f32(*v);
            }
            StyleProp::StrokeLinecap(c) => {
                self.u8(6);
                self.u8(match c {
                    LineCap::Butt => 0,
                    LineCap::Round => 1,
                    LineCap::Square => 2,
                });
            }
            StyleProp::StrokeLinejoin(j) => {
                self.u8(7);
                self.u8(match j {
                    LineJoin::Miter => 0,
                    LineJoin::Round => 1,
                    LineJoin::Bevel => 2,
                });
            }
            StyleProp::StrokeDasharray(a) => {
                self.u8(8);
                self.vec(a, |w, &v| w.f32(v));
            }
            StyleProp::StrokeDashoffset(v) => {
                self.u8(9);
                self.f32(*v);
            }
            StyleProp::Opacity(v) => {
                self.u8(10);
                self.f32(*v);
            }
            StyleProp::DisplayNone(v) => {
                self.u8(11);
                self.bool(*v);
            }
            StyleProp::Transform(ops) => {
                self.u8(12);
                self.vec(ops, Self::transform_op);
            }
            StyleProp::TransformBox(b) => {
                self.u8(13);
                self.u8(match b {
                    TransformBox::ViewBox => 0,
                    TransformBox::FillBox => 1,
                });
            }
            StyleProp::TransformOrigin(o) => {
                self.u8(14);
                self.coord(o.x);
                self.coord(o.y);
            }
            StyleProp::AnimName(n) => {
                self.u8(15);
                self.opt(n, |w, s| w.str(s));
            }
            StyleProp::AnimDuration(v) => {
                self.u8(16);
                self.f32(*v);
            }
            StyleProp::AnimDelay(v) => {
                self.u8(17);
                self.f32(*v);
            }
            StyleProp::AnimTiming(e) => {
                self.u8(18);
                self.easing(*e);
            }
            StyleProp::AnimIterations(it) => {
                self.u8(19);
                self.iterations(*it);
            }
            StyleProp::AnimDirection(d) => {
                self.u8(20);
                self.u8(match d {
                    AnimDirection::Normal => 0,
                    AnimDirection::Reverse => 1,
                    AnimDirection::Alternate => 2,
                    AnimDirection::AlternateReverse => 3,
                });
            }
            StyleProp::AnimFill(f) => {
                self.u8(21);
                self.u8(match f {
                    AnimFill::None => 0,
                    AnimFill::Forwards => 1,
                    AnimFill::Backwards => 2,
                    AnimFill::Both => 3,
                });
            }
            StyleProp::Transitions(ts) => {
                self.u8(22);
                self.vec(ts, |w, t| {
                    w.u8(match t.prop {
                        TransitionProp::All => 0,
                        TransitionProp::Fill => 1,
                        TransitionProp::Stroke => 2,
                        TransitionProp::Opacity => 3,
                        TransitionProp::Transform => 4,
                        TransitionProp::StrokeWidth => 5,
                        TransitionProp::StrokeDashoffset => 6,
                    });
                    w.f32(t.duration);
                    w.f32(t.delay);
                    w.easing(t.easing);
                });
            }
        }
    }
    fn smil(&mut self, s: &SmilAnim) {
        self.u8(match s.attr {
            SmilAttr::R => 0,
            SmilAttr::Rx => 1,
            SmilAttr::Ry => 2,
            SmilAttr::Cx => 3,
            SmilAttr::Cy => 4,
            SmilAttr::X => 5,
            SmilAttr::Y => 6,
            SmilAttr::Width => 7,
            SmilAttr::Height => 8,
            SmilAttr::Opacity => 9,
            SmilAttr::StrokeWidth => 10,
            SmilAttr::StrokeDashoffset => 11,
            SmilAttr::Fill => 12,
            SmilAttr::Stroke => 13,
            SmilAttr::TransformTranslate => 14,
            SmilAttr::TransformScale => 15,
            SmilAttr::TransformRotate => 16,
        });
        self.vec(&s.values, |w, v| match v {
            SmilValue::Scalar(x) => {
                w.u8(0);
                w.f32(*x);
            }
            SmilValue::Pair(x, y) => {
                w.u8(1);
                w.f32(*x);
                w.f32(*y);
            }
            SmilValue::Rotate(a, pivot) => {
                w.u8(2);
                w.f32(*a);
                w.opt(pivot, |w, &(cx, cy)| {
                    w.f32(cx);
                    w.f32(cy);
                });
            }
            SmilValue::Color(c) => {
                w.u8(3);
                w.color(c);
            }
        });
        self.vec(&s.key_times, |w, &t| w.f32(t));
        self.vec(&s.splines, |w, &e| w.easing(e));
        self.bool(s.discrete);
        self.f32(s.begin);
        self.f32(s.dur);
        self.iterations(s.iterations);
        self.bool(s.freeze);
    }
    fn node(&mut self, n: &SceneNode) {
        self.opt(&n.parent, |w, &p| w.u16(p));
        self.vec(&n.children, |w, &c| w.u16(c));
        self.str(&n.tag);
        self.opt(&n.id, |w, s| w.str(s));
        self.vec(&n.classes, |w, s| w.str(s));
        self.shape(&n.shape);
        self.bbox(n.fill_box);
        self.opt(&n.clip, |w, &c| w.u16(c));
        self.vec(&n.svg_transform, Self::transform_op);
        self.vec(&n.base_props, |w, (p, inline, imp)| {
            w.style_prop(p);
            w.bool(*inline);
            w.bool(*imp);
        });
        self.vec(&n.smil, Self::smil);
    }
    fn rule(&mut self, r: &StyleRule) {
        self.u8(match r.media {
            MediaCond::Always => 0,
            MediaCond::SchemeDark => 1,
            MediaCond::SchemeLight => 2,
            MediaCond::ReducedMotion => 3,
        });
        self.vec(&r.steps, |w, step| {
            w.opt(&step.compound.tag, |w, s| w.str(s));
            w.opt(&step.compound.id, |w, s| w.str(s));
            w.vec(&step.compound.classes, |w, s| w.str(s));
            w.bool(step.compound.hover);
            w.bool(step.compound.active);
            w.bool(step.direct_parent);
        });
        self.u16(r.specificity.0);
        self.u16(r.specificity.1);
        self.u16(r.specificity.2);
        self.u32(r.order);
        self.vec(&r.declarations, |w, (p, imp)| {
            w.style_prop(p);
            w.bool(*imp);
        });
    }
    fn keyframes(&mut self, kf: &Keyframes) {
        self.str(&kf.name);
        self.vec(&kf.tracks, |w, tr| {
            w.u8(match tr.prop {
                TrackProp::Transform => 0,
                TrackProp::Opacity => 1,
                TrackProp::Fill => 2,
                TrackProp::Stroke => 3,
                TrackProp::StrokeWidth => 4,
                TrackProp::StrokeDashoffset => 5,
            });
            w.vec(&tr.stops, |w, st| {
                w.f32(st.offset);
                match &st.value {
                    AnimValue::Transform(ops) => {
                        w.u8(0);
                        w.vec(ops, Self::transform_op);
                    }
                    AnimValue::Scalar(v) => {
                        w.u8(1);
                        w.f32(*v);
                    }
                    AnimValue::Color(c) => {
                        w.u8(2);
                        w.color(c);
                    }
                }
                w.opt(&st.easing, |w, &e| w.easing(e));
            });
        });
    }
    fn timed_class(&mut self, tc: &TimedClass) {
        self.opt(&tc.class, |w, s| w.str(s));
        self.opt(&tc.duration, |w, &d| w.f32(d));
    }
    fn action(&mut self, a: &InteractionAction) {
        match a {
            InteractionAction::Toggle(c) => {
                self.u8(0);
                self.str(c);
            }
            InteractionAction::Cycle(list) => {
                self.u8(1);
                self.vec(list, Self::timed_class);
            }
            InteractionAction::Trigger(tc) => {
                self.u8(2);
                self.timed_class(tc);
            }
            InteractionAction::Random(list) => {
                self.u8(3);
                self.vec(list, Self::timed_class);
            }
        }
    }
    fn interaction(&mut self, i: &Interaction) {
        self.f32(i.ready_after);
        self.opt(&i.on_click, Self::action);
        self.opt(&i.on_dblclick, Self::action);
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| "truncated BSVG stream".to_string())?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, String> {
        Ok(self.u8()? != 0)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        let s = self.take(4)?;
        Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn str(&mut self) -> Result<String, String> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| "invalid UTF-8 in BSVG".into())
    }
    fn opt<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(f(self)?)),
            _ => Err("invalid option tag in BSVG".into()),
        }
    }
    fn vec<T>(
        &mut self,
        mut f: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let len = self.u32()? as usize;
        if len > 1_000_000 {
            return Err("excessive BSVG vector length".into());
        }
        let mut out = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            out.push(f(self)?);
        }
        Ok(out)
    }
    fn matrix(&mut self) -> Result<[f32; 6], String> {
        Ok([
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ])
    }
    fn view_box(&mut self) -> Result<ViewBox, String> {
        Ok(ViewBox {
            x: self.f32()?,
            y: self.f32()?,
            width: self.f32()?,
            height: self.f32()?,
        })
    }
    fn bbox(&mut self) -> Result<BBox, String> {
        if !self.bool()? {
            return Ok(BBox::default());
        }
        Ok(BBox {
            x: self.f32()?,
            y: self.f32()?,
            width: self.f32()?,
            height: self.f32()?,
            valid: true,
        })
    }
    fn color(&mut self) -> Result<ColorSpec, String> {
        match self.u8()? {
            0 => Ok(ColorSpec::None),
            1 => {
                let s = self.take(4)?;
                Ok(ColorSpec::Rgba([s[0], s[1], s[2], s[3]]))
            }
            2 => Ok(ColorSpec::CurrentColor),
            3 => {
                let light = self.color()?;
                let dark = self.color()?;
                Ok(ColorSpec::LightDark(Box::new((light, dark))))
            }
            4 => Ok(ColorSpec::Gradient(self.u16()?)),
            _ => Err("invalid BSVG color tag".into()),
        }
    }
    fn gradient(&mut self) -> Result<GradientDef, String> {
        let kind = match self.u8()? {
            0 => GradientKind::Linear {
                x1: self.f32()?,
                y1: self.f32()?,
                x2: self.f32()?,
                y2: self.f32()?,
            },
            1 => GradientKind::Radial {
                cx: self.f32()?,
                cy: self.f32()?,
                r: self.f32()?,
                fx: self.f32()?,
                fy: self.f32()?,
            },
            _ => return Err("invalid BSVG gradient kind".into()),
        };
        let units = match self.u8()? {
            0 => GradientUnits::ObjectBoundingBox,
            1 => GradientUnits::UserSpaceOnUse,
            _ => return Err("invalid BSVG gradient units".into()),
        };
        let transform = self.matrix()?;
        let stops = self.vec(|r| {
            Ok(GradientStop {
                offset: r.f32()?,
                color: r.color()?,
                opacity: r.f32()?,
            })
        })?;
        Ok(GradientDef {
            kind,
            units,
            transform,
            stops,
        })
    }
    fn shape(&mut self) -> Result<ShapeKind, String> {
        match self.u8()? {
            0 => Ok(ShapeKind::Group),
            1 => {
                let segs = self.vec(|r| match r.u8()? {
                    0 => Ok(PathSeg::Move(r.f32()?, r.f32()?)),
                    1 => Ok(PathSeg::Line(r.f32()?, r.f32()?)),
                    2 => Ok(PathSeg::Quad(r.f32()?, r.f32()?, r.f32()?, r.f32()?)),
                    3 => Ok(PathSeg::Cubic(
                        r.f32()?,
                        r.f32()?,
                        r.f32()?,
                        r.f32()?,
                        r.f32()?,
                        r.f32()?,
                    )),
                    4 => Ok(PathSeg::Close),
                    _ => Err("invalid BSVG path segment".into()),
                })?;
                Ok(ShapeKind::Path(segs))
            }
            2 => Ok(ShapeKind::Rect {
                x: self.f32()?,
                y: self.f32()?,
                width: self.f32()?,
                height: self.f32()?,
                rx: self.f32()?,
                ry: self.f32()?,
            }),
            3 => Ok(ShapeKind::Circle {
                cx: self.f32()?,
                cy: self.f32()?,
                r: self.f32()?,
            }),
            4 => Ok(ShapeKind::Ellipse {
                cx: self.f32()?,
                cy: self.f32()?,
                rx: self.f32()?,
                ry: self.f32()?,
            }),
            5 => Ok(ShapeKind::Line {
                x1: self.f32()?,
                y1: self.f32()?,
                x2: self.f32()?,
                y2: self.f32()?,
            }),
            _ => Err("invalid BSVG shape tag".into()),
        }
    }
    fn fill_rule(&mut self) -> Result<FillRule, String> {
        match self.u8()? {
            0 => Ok(FillRule::NonZero),
            1 => Ok(FillRule::EvenOdd),
            _ => Err("invalid BSVG fill rule".into()),
        }
    }
    fn clip(&mut self) -> Result<ClipDef, String> {
        let items = self.vec(|r| {
            Ok(ClipItem {
                shape: r.shape()?,
                transform: r.matrix()?,
                fill_rule: r.fill_rule()?,
            })
        })?;
        Ok(ClipDef { items })
    }
    fn transform_op(&mut self) -> Result<TransformOp, String> {
        match self.u8()? {
            0 => Ok(TransformOp::Translate(self.f32()?, self.f32()?)),
            1 => Ok(TransformOp::Scale(self.f32()?, self.f32()?)),
            2 => Ok(TransformOp::Rotate(self.f32()?)),
            3 => Ok(TransformOp::SkewX(self.f32()?)),
            4 => Ok(TransformOp::SkewY(self.f32()?)),
            5 => Ok(TransformOp::Matrix(self.matrix()?)),
            _ => Err("invalid BSVG transform op".into()),
        }
    }
    fn coord(&mut self) -> Result<CoordVal, String> {
        match self.u8()? {
            0 => Ok(CoordVal::Px(self.f32()?)),
            1 => Ok(CoordVal::Percent(self.f32()?)),
            _ => Err("invalid BSVG coord".into()),
        }
    }
    fn easing(&mut self) -> Result<Easing, String> {
        match self.u8()? {
            0 => Ok(Easing::Linear),
            1 => Ok(Easing::CubicBezier(
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
            )),
            2 => Ok(Easing::Steps(self.u16()?, self.bool()?)),
            _ => Err("invalid BSVG easing".into()),
        }
    }
    fn iterations(&mut self) -> Result<Iterations, String> {
        match self.u8()? {
            0 => Ok(Iterations::Count(self.f32()?)),
            1 => Ok(Iterations::Infinite),
            _ => Err("invalid BSVG iterations".into()),
        }
    }
    fn style_prop(&mut self) -> Result<StyleProp, String> {
        match self.u8()? {
            0 => Ok(StyleProp::Fill(self.color()?)),
            1 => Ok(StyleProp::FillOpacity(self.f32()?)),
            2 => Ok(StyleProp::FillRule(self.fill_rule()?)),
            3 => Ok(StyleProp::Stroke(self.color()?)),
            4 => Ok(StyleProp::StrokeWidth(self.f32()?)),
            5 => Ok(StyleProp::StrokeOpacity(self.f32()?)),
            6 => Ok(StyleProp::StrokeLinecap(match self.u8()? {
                0 => LineCap::Butt,
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => return Err("invalid linecap".into()),
            })),
            7 => Ok(StyleProp::StrokeLinejoin(match self.u8()? {
                0 => LineJoin::Miter,
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => return Err("invalid linejoin".into()),
            })),
            8 => Ok(StyleProp::StrokeDasharray(self.vec(Self::f32)?)),
            9 => Ok(StyleProp::StrokeDashoffset(self.f32()?)),
            10 => Ok(StyleProp::Opacity(self.f32()?)),
            11 => Ok(StyleProp::DisplayNone(self.bool()?)),
            12 => Ok(StyleProp::Transform(self.vec(Self::transform_op)?)),
            13 => Ok(StyleProp::TransformBox(match self.u8()? {
                0 => TransformBox::ViewBox,
                1 => TransformBox::FillBox,
                _ => return Err("invalid transform-box".into()),
            })),
            14 => Ok(StyleProp::TransformOrigin(OriginSpec {
                x: self.coord()?,
                y: self.coord()?,
            })),
            15 => Ok(StyleProp::AnimName(self.opt(Self::str)?)),
            16 => Ok(StyleProp::AnimDuration(self.f32()?)),
            17 => Ok(StyleProp::AnimDelay(self.f32()?)),
            18 => Ok(StyleProp::AnimTiming(self.easing()?)),
            19 => Ok(StyleProp::AnimIterations(self.iterations()?)),
            20 => Ok(StyleProp::AnimDirection(match self.u8()? {
                0 => AnimDirection::Normal,
                1 => AnimDirection::Reverse,
                2 => AnimDirection::Alternate,
                3 => AnimDirection::AlternateReverse,
                _ => return Err("invalid animation-direction".into()),
            })),
            21 => Ok(StyleProp::AnimFill(match self.u8()? {
                0 => AnimFill::None,
                1 => AnimFill::Forwards,
                2 => AnimFill::Backwards,
                3 => AnimFill::Both,
                _ => return Err("invalid animation-fill-mode".into()),
            })),
            22 => Ok(StyleProp::Transitions(self.vec(|r| {
                let prop = match r.u8()? {
                    0 => TransitionProp::All,
                    1 => TransitionProp::Fill,
                    2 => TransitionProp::Stroke,
                    3 => TransitionProp::Opacity,
                    4 => TransitionProp::Transform,
                    5 => TransitionProp::StrokeWidth,
                    6 => TransitionProp::StrokeDashoffset,
                    _ => return Err("invalid transition property".into()),
                };
                Ok(TransitionSpec {
                    prop,
                    duration: r.f32()?,
                    delay: r.f32()?,
                    easing: r.easing()?,
                })
            })?)),
            _ => Err("invalid BSVG style prop".into()),
        }
    }
    fn smil(&mut self) -> Result<SmilAnim, String> {
        let attr = match self.u8()? {
            0 => SmilAttr::R,
            1 => SmilAttr::Rx,
            2 => SmilAttr::Ry,
            3 => SmilAttr::Cx,
            4 => SmilAttr::Cy,
            5 => SmilAttr::X,
            6 => SmilAttr::Y,
            7 => SmilAttr::Width,
            8 => SmilAttr::Height,
            9 => SmilAttr::Opacity,
            10 => SmilAttr::StrokeWidth,
            11 => SmilAttr::StrokeDashoffset,
            12 => SmilAttr::Fill,
            13 => SmilAttr::Stroke,
            14 => SmilAttr::TransformTranslate,
            15 => SmilAttr::TransformScale,
            16 => SmilAttr::TransformRotate,
            _ => return Err("invalid BSVG SMIL attr".into()),
        };
        let values = self.vec(|r| match r.u8()? {
            0 => Ok(SmilValue::Scalar(r.f32()?)),
            1 => Ok(SmilValue::Pair(r.f32()?, r.f32()?)),
            2 => Ok(SmilValue::Rotate(
                r.f32()?,
                r.opt(|r| Ok((r.f32()?, r.f32()?)))?,
            )),
            3 => Ok(SmilValue::Color(r.color()?)),
            _ => Err("invalid BSVG SMIL value".into()),
        })?;
        let key_times = self.vec(Self::f32)?;
        let splines = self.vec(Self::easing)?;
        let discrete = self.bool()?;
        let begin = self.f32()?;
        let dur = self.f32()?;
        let iterations = self.iterations()?;
        let freeze = self.bool()?;
        Ok(SmilAnim {
            attr,
            values,
            key_times,
            splines,
            discrete,
            begin,
            dur,
            iterations,
            freeze,
        })
    }
    fn node(&mut self) -> Result<SceneNode, String> {
        Ok(SceneNode {
            parent: self.opt(Self::u16)?,
            children: self.vec(Self::u16)?,
            tag: self.str()?,
            id: self.opt(Self::str)?,
            classes: self.vec(Self::str)?,
            shape: self.shape()?,
            fill_box: self.bbox()?,
            clip: self.opt(Self::u16)?,
            svg_transform: self.vec(Self::transform_op)?,
            base_props: self.vec(|r| Ok((r.style_prop()?, r.bool()?, r.bool()?)))?,
            smil: self.vec(Self::smil)?,
        })
    }
    fn rule(&mut self) -> Result<StyleRule, String> {
        let media = match self.u8()? {
            0 => MediaCond::Always,
            1 => MediaCond::SchemeDark,
            2 => MediaCond::SchemeLight,
            3 => MediaCond::ReducedMotion,
            _ => return Err("invalid BSVG media condition".into()),
        };
        let steps = self.vec(|r| {
            Ok(SelectorStep {
                compound: CompoundSel {
                    tag: r.opt(Self::str)?,
                    id: r.opt(Self::str)?,
                    classes: r.vec(Self::str)?,
                    hover: r.bool()?,
                    active: r.bool()?,
                },
                direct_parent: r.bool()?,
            })
        })?;
        let specificity = (self.u16()?, self.u16()?, self.u16()?);
        let order = self.u32()?;
        let declarations = self.vec(|r| Ok((r.style_prop()?, r.bool()?)))?;
        Ok(StyleRule {
            media,
            steps,
            specificity,
            order,
            declarations,
        })
    }
    fn keyframes(&mut self) -> Result<Keyframes, String> {
        let name = self.str()?;
        let tracks = self.vec(|r| {
            let prop = match r.u8()? {
                0 => TrackProp::Transform,
                1 => TrackProp::Opacity,
                2 => TrackProp::Fill,
                3 => TrackProp::Stroke,
                4 => TrackProp::StrokeWidth,
                5 => TrackProp::StrokeDashoffset,
                _ => return Err("invalid BSVG track prop".into()),
            };
            let stops = r.vec(|r| {
                let offset = r.f32()?;
                let value = match r.u8()? {
                    0 => AnimValue::Transform(r.vec(Self::transform_op)?),
                    1 => AnimValue::Scalar(r.f32()?),
                    2 => AnimValue::Color(r.color()?),
                    _ => return Err("invalid BSVG anim value".into()),
                };
                let easing = r.opt(Self::easing)?;
                Ok(KeyframeStop {
                    offset,
                    value,
                    easing,
                })
            })?;
            Ok(KeyframeTrack { prop, stops })
        })?;
        Ok(Keyframes { name, tracks })
    }
    fn timed_class(&mut self) -> Result<TimedClass, String> {
        Ok(TimedClass {
            class: self.opt(Self::str)?,
            duration: self.opt(Self::f32)?,
        })
    }
    fn action(&mut self) -> Result<InteractionAction, String> {
        match self.u8()? {
            0 => Ok(InteractionAction::Toggle(self.str()?)),
            1 => Ok(InteractionAction::Cycle(self.vec(Self::timed_class)?)),
            2 => Ok(InteractionAction::Trigger(self.timed_class()?)),
            3 => Ok(InteractionAction::Random(self.vec(Self::timed_class)?)),
            _ => Err("invalid BSVG interaction action".into()),
        }
    }
    fn interaction(&mut self) -> Result<Interaction, String> {
        Ok(Interaction {
            ready_after: self.f32()?,
            on_click: self.opt(Self::action)?,
            on_dblclick: self.opt(Self::action)?,
        })
    }
}
