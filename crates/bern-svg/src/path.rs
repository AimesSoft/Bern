//! SVG path parser, shape path builder, and exact Bezier bounding boxes.

use crate::model::{BBox, PathSeg, ShapeKind};
use tiny_skia::{Path, PathBuilder, Transform};

/// Build a `tiny_skia::Path` from `PathSeg`s.
pub fn build_segments(segs: &[PathSeg]) -> Option<Path> {
    if segs.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::with_capacity(segs.len(), segs.len() * 2);
    for &seg in segs {
        match seg {
            PathSeg::Move(x, y) => pb.move_to(x, y),
            PathSeg::Line(x, y) => pb.line_to(x, y),
            PathSeg::Quad(x1, y1, x, y) => pb.quad_to(x1, y1, x, y),
            PathSeg::Cubic(x1, y1, x2, y2, x, y) => pb.cubic_to(x1, y1, x2, y2, x, y),
            PathSeg::Close => pb.close(),
        }
    }
    pb.finish()
}

/// Build a `tiny_skia::Path` for any `ShapeKind`.
pub fn build_shape(shape: &ShapeKind) -> Option<Path> {
    match shape {
        ShapeKind::Group => None,
        ShapeKind::Path(segs) => build_segments(segs),
        &ShapeKind::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => build_rect(x, y, width, height, rx, ry),
        &ShapeKind::Circle { cx, cy, r } => {
            if r <= 0.0 {
                None
            } else {
                PathBuilder::from_circle(cx, cy, r)
            }
        }
        &ShapeKind::Ellipse { cx, cy, rx, ry } => build_ellipse(cx, cy, rx, ry),
        &ShapeKind::Line { x1, y1, x2, y2 } => {
            let mut pb = PathBuilder::with_capacity(2, 2);
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            pb.finish()
        }
    }
}

pub fn build_rect(x: f32, y: f32, width: f32, height: f32, rx: f32, ry: f32) -> Option<Path> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let (mut rx, mut ry) = match (rx > 0.0, ry > 0.0) {
        (false, false) => {
            let rect = tiny_skia::Rect::from_xywh(x, y, width, height)?;
            return Some(PathBuilder::from_rect(rect));
        }
        (true, false) => (rx, rx),
        (false, true) => (ry, ry),
        (true, true) => (rx, ry),
    };
    rx = rx.min(width * 0.5);
    ry = ry.min(height * 0.5);
    const K: f32 = 0.552_284_8;
    let kx = rx * K;
    let ky = ry * K;
    let mut pb = PathBuilder::new();
    pb.move_to(x + rx, y);
    pb.line_to(x + width - rx, y);
    pb.cubic_to(
        x + width - rx + kx,
        y,
        x + width,
        y + ry - ky,
        x + width,
        y + ry,
    );
    pb.line_to(x + width, y + height - ry);
    pb.cubic_to(
        x + width,
        y + height - ry + ky,
        x + width - rx + kx,
        y + height,
        x + width - rx,
        y + height,
    );
    pb.line_to(x + rx, y + height);
    pb.cubic_to(
        x + rx - kx,
        y + height,
        x,
        y + height - ry + ky,
        x,
        y + height - ry,
    );
    pb.line_to(x, y + ry);
    pb.cubic_to(x, y + ry - ky, x + rx - kx, y, x + rx, y);
    pb.close();
    pb.finish()
}

pub fn build_ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    const K: f32 = 0.552_284_8;
    let kx = rx * K;
    let ky = ry * K;
    let mut pb = PathBuilder::with_capacity(6, 13);
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
    pb.close();
    pb.finish()
}

/// Tight geometric bounding box of `path` after applying `matrix`.
pub fn path_bbox(path: &Path, matrix: [f32; 6]) -> BBox {
    let ts = Transform::from_row(
        matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
    );
    let transformed;
    let target = if ts.is_identity() {
        path
    } else {
        let Some(p) = path.clone().transform(ts) else {
            return BBox::default();
        };
        transformed = p;
        &transformed
    };
    match target.compute_tight_bounds() {
        Some(r) => BBox::from_min_max(r.left(), r.top(), r.right(), r.bottom()),
        None => BBox::default(),
    }
}

/// Parse an SVG `d` path attribute into normalized `PathSeg`s, converting
/// smooth curves and elliptical arcs (`A`/`a`) into cubic Beziers per SVG 1.1
/// F.6.5 / F.6.6.
#[cfg(feature = "svg")]
pub fn parse_svg_path(d: &str) -> Result<Vec<PathSeg>, String> {
    let mut s = PathScanner::new(d);
    let mut out = Vec::new();
    let (mut cx, mut cy) = (0.0_f32, 0.0_f32);
    let (mut sx, mut sy) = (0.0_f32, 0.0_f32);
    let mut prev_cubic: Option<(f32, f32)> = None;
    let mut prev_quad: Option<(f32, f32)> = None;
    let mut cmd = b'\0';

    while {
        s.skip_ws_comma();
        !s.eof()
    } {
        if let Some(next_cmd) = s.take_command() {
            cmd = next_cmd;
        } else if cmd == b'\0' {
            return Err(format!(
                "expected SVG path command near `{}`",
                s.rem_preview()
            ));
        }

        let rel = cmd.is_ascii_lowercase();
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let mut x = s.num()?;
                let mut y = s.num()?;
                if rel {
                    x += cx;
                    y += cy;
                }
                out.push(PathSeg::Move(x, y));
                cx = x;
                cy = y;
                sx = x;
                sy = y;
                prev_cubic = None;
                prev_quad = None;
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let mut x = s.num()?;
                let mut y = s.num()?;
                if rel {
                    x += cx;
                    y += cy;
                }
                out.push(PathSeg::Line(x, y));
                cx = x;
                cy = y;
                prev_cubic = None;
                prev_quad = None;
            }
            b'H' => {
                let mut x = s.num()?;
                if rel {
                    x += cx;
                }
                out.push(PathSeg::Line(x, cy));
                cx = x;
                prev_cubic = None;
                prev_quad = None;
            }
            b'V' => {
                let mut y = s.num()?;
                if rel {
                    y += cy;
                }
                out.push(PathSeg::Line(cx, y));
                cy = y;
                prev_cubic = None;
                prev_quad = None;
            }
            b'C' => {
                let (mut x1, mut y1) = (s.num()?, s.num()?);
                let (mut x2, mut y2) = (s.num()?, s.num()?);
                let (mut x, mut y) = (s.num()?, s.num()?);
                if rel {
                    x1 += cx;
                    y1 += cy;
                    x2 += cx;
                    y2 += cy;
                    x += cx;
                    y += cy;
                }
                out.push(PathSeg::Cubic(x1, y1, x2, y2, x, y));
                prev_cubic = Some((x2, y2));
                prev_quad = None;
                cx = x;
                cy = y;
            }
            b'S' => {
                let (mut x2, mut y2) = (s.num()?, s.num()?);
                let (mut x, mut y) = (s.num()?, s.num()?);
                if rel {
                    x2 += cx;
                    y2 += cy;
                    x += cx;
                    y += cy;
                }
                let (x1, y1) = match prev_cubic {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                out.push(PathSeg::Cubic(x1, y1, x2, y2, x, y));
                prev_cubic = Some((x2, y2));
                prev_quad = None;
                cx = x;
                cy = y;
            }
            b'Q' => {
                let (mut x1, mut y1) = (s.num()?, s.num()?);
                let (mut x, mut y) = (s.num()?, s.num()?);
                if rel {
                    x1 += cx;
                    y1 += cy;
                    x += cx;
                    y += cy;
                }
                out.push(PathSeg::Quad(x1, y1, x, y));
                prev_quad = Some((x1, y1));
                prev_cubic = None;
                cx = x;
                cy = y;
            }
            b'T' => {
                let (mut x, mut y) = (s.num()?, s.num()?);
                if rel {
                    x += cx;
                    y += cy;
                }
                let (x1, y1) = match prev_quad {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                out.push(PathSeg::Quad(x1, y1, x, y));
                prev_quad = Some((x1, y1));
                prev_cubic = None;
                cx = x;
                cy = y;
            }
            b'A' => {
                let rx = s.num()?;
                let ry = s.num()?;
                let x_axis_rotation = s.num()?;
                let large_arc = s.flag()?;
                let sweep = s.flag()?;
                let (mut x, mut y) = (s.num()?, s.num()?);
                if rel {
                    x += cx;
                    y += cy;
                }
                arc_to_cubics(
                    cx,
                    cy,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x,
                    y,
                    &mut out,
                );
                cx = x;
                cy = y;
                prev_cubic = None;
                prev_quad = None;
            }
            b'Z' => {
                out.push(PathSeg::Close);
                cx = sx;
                cy = sy;
                prev_cubic = None;
                prev_quad = None;
                cmd = b'\0';
            }
            other => return Err(format!("unsupported SVG path command `{}`", other as char)),
        }
    }
    Ok(out)
}

#[cfg(feature = "svg")]
#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(
    x1: f32,
    y1: f32,
    rx_in: f32,
    ry_in: f32,
    phi_deg: f32,
    large_arc: bool,
    sweep: bool,
    x2: f32,
    y2: f32,
    out: &mut Vec<PathSeg>,
) {
    let (x1, y1, x2, y2) = (f64::from(x1), f64::from(y1), f64::from(x2), f64::from(y2));
    if (x1 - x2).abs() < 1e-9 && (y1 - y2).abs() < 1e-9 {
        return;
    }
    let mut rx = f64::from(rx_in.abs());
    let mut ry = f64::from(ry_in.abs());
    if rx < 1e-9 || ry < 1e-9 {
        out.push(PathSeg::Line(x2 as f32, y2 as f32));
        return;
    }
    let phi = f64::from(phi_deg).to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    let dx2 = (x1 - x2) * 0.5;
    let dy2 = (y1 - y2) * 0.5;
    let x1p = cos_phi * dx2 + sin_phi * dy2;
    let y1p = -sin_phi * dx2 + cos_phi * dy2;

    let rx_sq = rx * rx;
    let ry_sq = ry * ry;
    let x1p_sq = x1p * x1p;
    let y1p_sq = y1p * y1p;

    let lambda = x1p_sq / rx_sq + y1p_sq / ry_sq;
    if lambda > 1.0 {
        let root = lambda.sqrt();
        rx *= root;
        ry *= root;
    }
    let rx_sq = rx * rx;
    let ry_sq = ry * ry;

    let denom = rx_sq * y1p_sq + ry_sq * x1p_sq;
    let num = (rx_sq * ry_sq - denom).max(0.0);
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let coef = if denom > 0.0 {
        sign * (num / denom).sqrt()
    } else {
        0.0
    };
    let cxp = coef * (rx * y1p / ry);
    let cyp = coef * (-ry * x1p / rx);

    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) * 0.5;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) * 0.5;

    let vec_angle = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
        if len == 0.0 {
            return 0.0;
        }
        let c = (dot / len).clamp(-1.0, 1.0);
        let a = c.acos();
        if ux * vy - uy * vx < 0.0 { -a } else { a }
    };

    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;

    let theta1 = vec_angle(1.0, 0.0, ux, uy);
    let mut dtheta = vec_angle(ux, uy, vx, vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }

    let segments = ((dtheta.abs() / (std::f64::consts::FRAC_PI_2 + 1e-6)).ceil() as usize).max(1);
    let delta = dtheta / segments as f64;
    let alpha = (4.0 / 3.0) * (delta * 0.25).tan();

    let map_pt = |px: f64, py: f64| -> (f32, f32) {
        let sx = px * rx;
        let sy = py * ry;
        (
            (cos_phi * sx - sin_phi * sy + cx) as f32,
            (sin_phi * sx + cos_phi * sy + cy) as f32,
        )
    };

    let mut t = theta1;
    for _ in 0..segments {
        let t2 = t + delta;
        let (s1, c1) = t.sin_cos();
        let (s2, c2) = t2.sin_cos();
        let (p1x, p1y) = map_pt(c1 - alpha * s1, s1 + alpha * c1);
        let (p2x, p2y) = map_pt(c2 + alpha * s2, s2 - alpha * c2);
        let (px, py) = map_pt(c2, s2);
        out.push(PathSeg::Cubic(p1x, p1y, p2x, p2y, px, py));
        t = t2;
    }
}

#[cfg(feature = "svg")]
struct PathScanner<'a> {
    bytes: &'a [u8],
    src: &'a str,
    pos: usize,
}

#[cfg(feature = "svg")]
impl<'a> PathScanner<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            bytes: src.as_bytes(),
            src,
            pos: 0,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn rem_preview(&self) -> &str {
        let end = (self.pos + 16).min(self.bytes.len());
        self.src.get(self.pos..end).unwrap_or("")
    }

    fn skip_ws_comma(&mut self) {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b.is_ascii_whitespace() || b == b',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn take_command(&mut self) -> Option<u8> {
        self.skip_ws_comma();
        let b = *self.bytes.get(self.pos)?;
        if matches!(
            b.to_ascii_uppercase(),
            b'M' | b'L' | b'H' | b'V' | b'C' | b'S' | b'Q' | b'T' | b'A' | b'Z'
        ) {
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    fn flag(&mut self) -> Result<bool, String> {
        self.skip_ws_comma();
        match self.bytes.get(self.pos) {
            Some(b'0') => {
                self.pos += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.pos += 1;
                Ok(true)
            }
            _ => Err(format!(
                "expected SVG arc flag (0 or 1) near `{}`",
                self.rem_preview()
            )),
        }
    }

    fn num(&mut self) -> Result<f32, String> {
        self.skip_ws_comma();
        let start = self.pos;
        if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'+' | b'-') {
            self.pos += 1;
        }
        let mut digits = false;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
            digits = true;
        }
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
                digits = true;
            }
        }
        if !digits {
            return Err(format!("expected SVG number near `{}`", self.rem_preview()));
        }
        if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'e' | b'E') {
            let save = self.pos;
            self.pos += 1;
            if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'+' | b'-') {
                self.pos += 1;
            }
            let exp_start = self.pos;
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            if self.pos == exp_start {
                self.pos = save;
            }
        }
        self.src[start..self.pos]
            .parse::<f32>()
            .map_err(|e| format!("invalid SVG number `{}`: {e}", &self.src[start..self.pos]))
    }
}
