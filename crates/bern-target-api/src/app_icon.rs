//! The application's own icons: `icon: "icons/play.svg"` names a declared
//! SVG resource in place of a name from the Bern icon set. A target that
//! draws icons as filled shapes in the control's foreground colour (WinUI 3's
//! `PathIcon`) takes the picture as one outline: strokes become the shapes
//! they cover and text becomes its glyphs, all drawn at build time.

use bern_ir::{Application, Node, Prop};
use resvg::tiny_skia::{Path, PathSegment, Point, Transform};
use resvg::usvg;

/// Whether an icon name names one of the application's own SVG icons.
pub fn is_app_icon(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".svg")
}

/// The application's own icons its elements use, each once, in the order
/// first used.
pub fn used(app: &Application) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    app.walk(&mut |node: &Node, _| {
        if let Node::Element(element) = node {
            for prop in element.props.values() {
                if let Prop::Icon { name } = prop
                    && is_app_icon(name)
                    && !names.contains(name)
                {
                    names.push(name.clone());
                }
            }
        }
    });
    names
}

/// The side of the square the outline is drawn in, as Fluent's artwork.
pub const SIDE: f32 = 20.0;

/// The SVG at `path` as path data in a `SIDE`-unit square: the picture
/// fitted and centred, every fill, stroke and glyph it covers as filled
/// contours. Each shape's contours are turned to wind the same way, so with
/// the nonzero rule overlapping shapes add up instead of cutting holes.
pub fn outline(path: &std::path::Path) -> Result<String, String> {
    let data =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut options = usvg::Options {
        resources_dir: path.parent().map(|parent| parent.to_owned()),
        ..Default::default()
    };
    if String::from_utf8_lossy(&data).contains("<text") {
        options.fontdb_mut().load_system_fonts();
    }
    let tree = usvg::Tree::from_data(&data, &options)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let size = tree.size();
    let scale = SIDE / size.width().max(size.height());
    let base = Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        (SIDE - size.width() * scale) / 2.0,
        (SIDE - size.height() * scale) / 2.0,
    );
    let mut shapes = Vec::new();
    collect(tree.root(), base, &mut shapes);
    if shapes.is_empty() {
        return Err(format!("{} draws nothing", path.display()));
    }
    let mut out = String::new();
    for shape in shapes {
        let mut contours = contours(&shape);
        if contours.iter().map(Contour::area).sum::<f32>() < 0.0 {
            for contour in &mut contours {
                contour.reverse();
            }
        }
        for contour in &contours {
            contour.write(&mut out);
        }
    }
    Ok(out.trim_end().to_owned())
}

/// Every filled shape under `group`, already in the square's coordinates.
/// `outer` places the group's parent; each group adds its own transform, as
/// resvg draws them (a text's glyphs sit in its flattened group the same way).
fn collect(group: &usvg::Group, outer: Transform, shapes: &mut Vec<Path>) {
    let ts = outer.pre_concat(group.transform());
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect(group, ts, shapes),
            usvg::Node::Text(text) => collect(text.flattened(), ts, shapes),
            usvg::Node::Path(path) if path.is_visible() => {
                if path.fill().is_some()
                    && let Some(filled) = path.data().clone().transform(ts)
                {
                    shapes.push(filled);
                }
                if let Some(stroke) = path.stroke() {
                    // Stroked where the path is drawn, then moved; finer
                    // than the square's units so curves stay smooth.
                    let resolution = ts.get_scale().0 * 8.0;
                    if let Some(covered) = path.data().stroke(&stroke.to_tiny_skia(), resolution)
                        && let Some(covered) = covered.transform(ts)
                    {
                        shapes.push(covered);
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum Segment {
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
}

impl Segment {
    fn end(self) -> Point {
        match self {
            Segment::Line(to) | Segment::Quad(_, to) | Segment::Cubic(_, _, to) => to,
        }
    }
}

/// One closed contour: where it starts and the segments that follow.
struct Contour {
    start: Point,
    segments: Vec<Segment>,
}

fn contours(path: &Path) -> Vec<Contour> {
    let mut contours: Vec<Contour> = Vec::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(at) => contours.push(Contour {
                start: at,
                segments: Vec::new(),
            }),
            PathSegment::LineTo(to) => push(&mut contours, Segment::Line(to)),
            PathSegment::QuadTo(control, to) => push(&mut contours, Segment::Quad(control, to)),
            PathSegment::CubicTo(first, second, to) => {
                push(&mut contours, Segment::Cubic(first, second, to))
            }
            PathSegment::Close => {}
        }
    }
    contours.retain(|contour| !contour.segments.is_empty());
    contours
}

fn push(contours: &mut [Contour], segment: Segment) {
    if let Some(contour) = contours.last_mut() {
        contour.segments.push(segment);
    }
}

impl Contour {
    /// The signed area of the polygon through the contour's points and
    /// control points: near enough to tell which way it winds.
    fn area(&self) -> f32 {
        let mut points = vec![self.start];
        for segment in &self.segments {
            match *segment {
                Segment::Line(to) => points.push(to),
                Segment::Quad(control, to) => points.extend([control, to]),
                Segment::Cubic(first, second, to) => points.extend([first, second, to]),
            }
        }
        let mut sum = 0.0;
        for (index, a) in points.iter().enumerate() {
            let b = points[(index + 1) % points.len()];
            sum += a.x * b.y - b.x * a.y;
        }
        sum / 2.0
    }

    fn reverse(&mut self) {
        let mut ends = vec![self.start];
        ends.extend(self.segments.iter().map(|segment| segment.end()));
        let mut reversed = Vec::with_capacity(self.segments.len());
        for (index, segment) in self.segments.iter().enumerate().rev() {
            let to = ends[index];
            reversed.push(match *segment {
                Segment::Line(_) => Segment::Line(to),
                Segment::Quad(control, _) => Segment::Quad(control, to),
                Segment::Cubic(first, second, _) => Segment::Cubic(second, first, to),
            });
        }
        self.start = *ends.last().unwrap_or(&self.start);
        self.segments = reversed;
    }

    fn write(&self, out: &mut String) {
        out.push_str(&format!("M{} ", point(self.start)));
        for segment in &self.segments {
            match *segment {
                Segment::Line(to) => out.push_str(&format!("L{} ", point(to))),
                Segment::Quad(control, to) => {
                    out.push_str(&format!("Q{} {} ", point(control), point(to)))
                }
                Segment::Cubic(first, second, to) => out.push_str(&format!(
                    "C{} {} {} ",
                    point(first),
                    point(second),
                    point(to)
                )),
            }
        }
        out.push_str("Z ");
    }
}

fn point(at: Point) -> String {
    format!("{},{}", number(at.x), number(at.y))
}

fn number(value: f32) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let text = format!("{rounded:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_svg(name: &str, body: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bern-app-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn a_stroke_becomes_a_filled_outline_in_the_square() {
        let path = write_svg(
            "line.svg",
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="3 3 18 18" fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round"><path d="M8.5 6v12"/></svg>"##,
        );
        let data = outline(&path).unwrap();
        assert!(data.starts_with('M'), "{data}");
        assert!(data.ends_with('Z'), "{data}");
        // The line at x = 8.5 of 3..21 lands at (8.5 - 3) / 18 * 20 ≈ 6.1,
        // its stroke 1.5 × 20/18 wide around it.
        let xs: Vec<f32> = data
            .split(|c: char| c == ' ' || c.is_ascii_alphabetic())
            .filter_map(|pair| pair.split(',').next()?.parse().ok())
            .collect();
        let (low, high) = xs
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(
            (low - 5.28).abs() < 0.05 && (high - 6.94).abs() < 0.05,
            "{low} {high}"
        );
    }

    #[test]
    fn every_shape_winds_the_same_way() {
        // Two strokes drawn in opposite directions: both come out positive.
        let path = write_svg(
            "two.svg",
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="#000" stroke-width="2"><path d="M2 5h16"/><path d="M18 10H2"/></svg>"##,
        );
        let data = outline(&path).unwrap();
        let shapes: Vec<&str> = data
            .split('Z')
            .filter(|part| !part.trim().is_empty())
            .collect();
        assert_eq!(shapes.len(), 2);
        let areas: Vec<f32> = shapes
            .iter()
            .map(|shape| {
                let points: Vec<(f32, f32)> = shape
                    .split(|c: char| c == ' ' || c.is_ascii_alphabetic())
                    .filter_map(|pair| {
                        let (x, y) = pair.split_once(',')?;
                        Some((x.parse().ok()?, y.parse().ok()?))
                    })
                    .collect();
                (0..points.len())
                    .map(|i| {
                        let (a, b) = (points[i], points[(i + 1) % points.len()]);
                        a.0 * b.1 - b.0 * a.1
                    })
                    .sum::<f32>()
            })
            .collect();
        assert!(areas.iter().all(|&area| area > 0.0), "{areas:?}");
    }

    #[test]
    fn names_ending_in_svg_are_the_applications_own() {
        assert!(is_app_icon("icons/play.svg"));
        assert!(is_app_icon("Danmaku.SVG"));
        assert!(!is_app_icon("play.fill"));
    }
}
