//! `Canvas`: shapes, data series and patterns drawn in semantic colors, in
//! the canvas's own coordinates (points from its top-left corner), with
//! GPUI's paths.

use crate::ctx::Ctx;
use crate::layout::{Len, color};
use crate::{Color, El};
use gpui::{
    Background, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, Rgba, Styled, canvas,
    linear_color_stop, linear_gradient, point, px,
};

/// The outline of a `Shape`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rect,
    Rounded,
    Capsule,
    Ellipse,
}

/// The edge a shape's fill fades out toward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fade {
    Up,
    Down,
    Left,
    Right,
}

/// A `Shape`: where it is (filling the canvas without a size), its fill
/// and stroke.
#[derive(Clone, Debug)]
pub struct ShapeOp {
    pub kind: ShapeKind,
    pub x: f32,
    pub y: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub corner: f32,
    pub fill: Option<Color>,
    /// Filled with the average color of the picture at this address
    /// (`fill_from`).
    pub picture: Option<String>,
    pub fade: Option<Fade>,
    /// Color and width.
    pub stroke: Option<(Color, f32)>,
    /// Dash and gap lengths of the stroke.
    pub dash: Option<(f32, f32)>,
    pub opacity: f32,
}

/// A `Series`: numbers across the canvas width, as an area or a line.
#[derive(Clone, Debug)]
pub struct SeriesOp {
    pub values: Vec<f64>,
    pub area: bool,
    pub max: Option<f64>,
    pub fill: Option<Color>,
    pub stroke: Option<(Color, f32)>,
    pub opacity: f32,
}

/// A band of a woven pattern: its width, and its color at an opacity, or
/// a gap.
#[derive(Clone, Copy, Debug)]
pub struct Band {
    pub width: f32,
    pub color: Option<Color>,
    pub opacity: f32,
}

/// A `Pattern` of `style!`: its bands across and down, and fine diagonal
/// lines with `twill`.
#[derive(Clone, Debug)]
pub struct PatternOp {
    pub bands: &'static [Band],
    pub twill: bool,
    pub opacity: f32,
}

/// One drawing element of a canvas.
#[derive(Clone, Debug)]
pub enum Op {
    Shape(ShapeOp),
    Series(SeriesOp),
    Pattern(PatternOp),
}

fn faded(color: Rgba, opacity: f32) -> Hsla {
    let mut color: Hsla = color.into();
    color.a *= opacity;
    color
}

/// A resolved drawing element: its colors in the canvas's appearance.
enum Paint {
    Shape(ShapeOp, Option<Hsla>, Option<(Hsla, f32)>),
    Series(SeriesOp, Option<Hsla>, Option<(Hsla, f32)>),
    Pattern(Vec<(f32, Option<Hsla>)>, bool, Hsla),
}

pub(crate) fn outline(
    kind: ShapeKind,
    bounds: Bounds<Pixels>,
    corner: f32,
    builder: &mut PathBuilder,
) {
    let (x, y) = (bounds.origin.x, bounds.origin.y);
    let (w, h) = (bounds.size.width, bounds.size.height);
    let radius = match kind {
        ShapeKind::Rect => px(0.0),
        ShapeKind::Rounded => px(corner).min(w / 2.0).min(h / 2.0),
        ShapeKind::Capsule => (w / 2.0).min(h / 2.0),
        ShapeKind::Ellipse => {
            let (rx, ry) = (w / 2.0, h / 2.0);
            builder.move_to(point(x, y + ry));
            builder.arc_to(point(rx, ry), px(0.0), false, true, point(x + w, y + ry));
            builder.arc_to(point(rx, ry), px(0.0), false, true, point(x, y + ry));
            builder.close();
            return;
        }
    };
    let r = radius;
    builder.move_to(point(x + r, y));
    builder.line_to(point(x + w - r, y));
    if r > px(0.0) {
        builder.arc_to(point(r, r), px(0.0), false, true, point(x + w, y + r));
    }
    builder.line_to(point(x + w, y + h - r));
    if r > px(0.0) {
        builder.arc_to(point(r, r), px(0.0), false, true, point(x + w - r, y + h));
    }
    builder.line_to(point(x + r, y + h));
    if r > px(0.0) {
        builder.arc_to(point(r, r), px(0.0), false, true, point(x, y + h - r));
    }
    builder.line_to(point(x, y + r));
    if r > px(0.0) {
        builder.arc_to(point(r, r), px(0.0), false, true, point(x + r, y));
    }
    builder.close();
}

fn paint_shape(
    shape: &ShapeOp,
    fill: Option<Hsla>,
    stroke: Option<(Hsla, f32)>,
    canvas: Bounds<Pixels>,
    window: &mut gpui::Window,
) {
    let origin: Point<Pixels> = canvas.origin + point(px(shape.x), px(shape.y));
    let size = gpui::size(
        shape.width.map(px).unwrap_or(canvas.size.width),
        shape.height.map(px).unwrap_or(canvas.size.height),
    );
    let bounds = Bounds { origin, size };
    if let Some(fill) = fill {
        let mut builder = PathBuilder::fill();
        outline(shape.kind, bounds, shape.corner, &mut builder);
        if let Ok(path) = builder.build() {
            let background: Background = match shape.fade {
                Some(edge) => {
                    let angle = match edge {
                        Fade::Up => 0.0,
                        Fade::Right => 90.0,
                        Fade::Down => 180.0,
                        Fade::Left => 270.0,
                    };
                    let mut clear = fill;
                    clear.a = 0.0;
                    linear_gradient(
                        angle,
                        linear_color_stop(fill, 0.0),
                        linear_color_stop(clear, 1.0),
                    )
                }
                None => fill.into(),
            };
            window.paint_path(path, background);
        }
    }
    if let Some((color, width)) = stroke {
        let builder = PathBuilder::stroke(px(width));
        let mut builder = match shape.dash {
            Some((dash, gap)) => builder.dash_array(&[px(dash), px(gap)]),
            None => builder,
        };
        // The stroke runs inside the shape's edge.
        let inset = px(width / 2.0);
        let inner = Bounds {
            origin: bounds.origin + point(inset, inset),
            size: gpui::size(
                (bounds.size.width - inset * 2.0).max(px(0.0)),
                (bounds.size.height - inset * 2.0).max(px(0.0)),
            ),
        };
        outline(
            shape.kind,
            inner,
            (shape.corner - width / 2.0).max(0.0),
            &mut builder,
        );
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

fn paint_series(
    series: &SeriesOp,
    fill: Option<Hsla>,
    stroke: Option<(Hsla, f32)>,
    canvas: Bounds<Pixels>,
    window: &mut gpui::Window,
) {
    let count = series.values.len();
    if count < 2 {
        return;
    }
    let peak = series
        .max
        .unwrap_or_else(|| series.values.iter().copied().fold(0.0, f64::max));
    let peak = if peak > 0.0 { peak } else { 1.0 };
    let (x, y) = (canvas.origin.x, canvas.origin.y);
    let (w, h) = (canvas.size.width, canvas.size.height);
    let points: Vec<Point<Pixels>> = series
        .values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let along = index as f32 / (count - 1) as f32;
            let up = (value / peak).clamp(0.0, 1.0) as f32;
            point(x + w * along, y + h - h * up)
        })
        .collect();
    if series.area
        && let Some(fill) = fill
    {
        let mut builder = PathBuilder::fill();
        builder.move_to(point(x, y + h));
        for at in &points {
            builder.line_to(*at);
        }
        builder.line_to(point(x + w, y + h));
        builder.close();
        if let Ok(path) = builder.build() {
            window.paint_path(path, fill);
        }
    }
    if let Some((color, width)) = stroke.or(if series.area {
        None
    } else {
        fill.map(|fill| (fill, 2.0))
    }) {
        let mut builder = PathBuilder::stroke(px(width));
        builder.move_to(points[0]);
        for at in &points[1..] {
            builder.line_to(*at);
        }
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

fn paint_pattern(
    bands: &[(f32, Option<Hsla>)],
    twill: bool,
    line: Hsla,
    canvas: Bounds<Pixels>,
    window: &mut gpui::Window,
) {
    let repeat: f32 = bands.iter().map(|(width, _)| width).sum();
    if repeat <= 0.0 {
        return;
    }
    let (x0, y0) = (canvas.origin.x, canvas.origin.y);
    let (w, h) = (f32::from(canvas.size.width), f32::from(canvas.size.height));
    // Bands down the canvas, then across it, so where two cross both show.
    for across in [false, true] {
        let length = if across { w } else { h };
        let mut at = 0.0;
        while at < length {
            for (width, color) in bands {
                if let Some(color) = color {
                    let start = at.min(length);
                    let end = (at + width).min(length);
                    if end > start {
                        let bounds = if across {
                            Bounds::new(
                                point(x0 + px(start), y0),
                                gpui::size(px(end - start), px(h)),
                            )
                        } else {
                            Bounds::new(
                                point(x0, y0 + px(start)),
                                gpui::size(px(w), px(end - start)),
                            )
                        };
                        let mut builder = PathBuilder::fill();
                        outline(ShapeKind::Rect, bounds, 0.0, &mut builder);
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, *color);
                        }
                    }
                }
                at += width;
            }
        }
    }
    if twill {
        let mut builder = PathBuilder::stroke(px(0.5));
        let mut offset = -h;
        while offset < w {
            builder.move_to(point(x0 + px(offset), y0 + px(h)));
            builder.line_to(point(x0 + px(offset + h), y0));
            offset += 4.0;
        }
        if let Ok(path) = builder.build() {
            window.paint_path(path, line);
        }
    }
}

/// `Canvas`: its drawing elements in order, at its size, or filling the
/// space its container gives without asking for any.
pub fn canvas_element(c: &Ctx, width: Len, height: Len, ops: Vec<Op>) -> El {
    // Inside a scene, the drawing is in the scene's scale too.
    let factor = crate::units::scale();
    let paints: Vec<Paint> =
        ops.into_iter()
            .map(|op| match op {
                Op::Shape(shape) => {
                    let shape = ShapeOp {
                        x: shape.x * factor,
                        y: shape.y * factor,
                        width: shape.width.map(|width| width * factor),
                        height: shape.height.map(|height| height * factor),
                        corner: shape.corner * factor,
                        stroke: shape.stroke.map(|(color, width)| (color, width * factor)),
                        dash: shape.dash.map(|(dash, gap)| (dash * factor, gap * factor)),
                        ..shape
                    };
                    let fill = match &shape.picture {
                        Some(url) => crate::pictures::average(url)
                            .map(|average| faded(average, shape.opacity)),
                        None => shape.fill.map(|fill| faded(color(c, fill), shape.opacity)),
                    };
                    let stroke = shape
                        .stroke
                        .map(|(stroke, width)| (faded(color(c, stroke), shape.opacity), width));
                    Paint::Shape(shape, fill, stroke)
                }
                Op::Series(series) => {
                    let fill = series
                        .fill
                        .map(|fill| faded(color(c, fill), series.opacity));
                    let stroke = series
                        .stroke
                        .map(|(stroke, width)| (faded(color(c, stroke), series.opacity), width));
                    Paint::Series(series, fill, stroke)
                }
                Op::Pattern(pattern) => {
                    let bands = pattern
                        .bands
                        .iter()
                        .map(|band| {
                            (
                                band.width,
                                band.color.map(|band_color| {
                                    faded(color(c, band_color), band.opacity * pattern.opacity)
                                }),
                            )
                        })
                        .collect();
                    Paint::Pattern(
                        bands,
                        pattern.twill,
                        faded(c.color("text.primary"), 0.08 * pattern.opacity),
                    )
                }
            })
            .collect();
    let element = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            for paint in &paints {
                match paint {
                    Paint::Shape(shape, fill, stroke) => {
                        paint_shape(shape, *fill, *stroke, bounds, window)
                    }
                    Paint::Series(series, fill, stroke) => {
                        paint_series(series, *fill, *stroke, bounds, window)
                    }
                    Paint::Pattern(bands, twill, line) => {
                        paint_pattern(bands, *twill, *line, bounds, window)
                    }
                }
            }
        },
    );
    let element = match width {
        Len::Fixed(width) => element.w(crate::units::px(width)).flex_none(),
        Len::Auto => element.w_full(),
    };
    let element = match height {
        Len::Fixed(height) => element.h(crate::units::px(height)),
        Len::Auto => element.h_full(),
    };
    element.into_any_element()
}
