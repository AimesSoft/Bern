//! The application's own icons: `icon: "icons/play.svg"` names a declared
//! SVG resource in place of a name from the Bern icon set. Every target
//! draws them in the control's foreground colour from one build-time outline
//! in a 20-unit square: fills, strokes and text glyphs become nonzero-winding
//! filled contours.

use bern_ir::{Application, Node, Prop};
use resvg::tiny_skia::{Path, PathBuilder, PathSegment, Point, Transform};
use resvg::usvg;
use std::sync::{Arc, OnceLock};

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

/// One segment of an outlined icon contour, in the 20-unit square.
#[derive(Clone, Copy, Debug)]
pub enum Segment {
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

/// One closed contour of an outlined icon, normalized to nonzero winding.
#[derive(Clone, Debug)]
pub struct Contour {
    pub start: Point,
    pub segments: Vec<Segment>,
}

/// Cached system font database so multiple `<text>` icons do not rescan the
/// host's fonts on every call.
fn system_fonts() -> &'static Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
}

fn has_text_tag(data: &[u8]) -> bool {
    let lower = String::from_utf8_lossy(data).to_ascii_lowercase();
    lower.contains("<text") || lower.contains(":text")
}

/// The SVG at `path` as normalized nonzero-winding contours in a `SIDE`-unit
/// square: fitted and centred, with fills, strokes and glyphs expanded.
pub fn outline_contours(path: &std::path::Path) -> Result<Vec<Contour>, String> {
    let data =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    outline_bytes(&path.display().to_string(), &data, path.parent())
}

/// Same as [`outline_contours`], operating on raw SVG bytes.
pub fn outline_bytes(
    label: &str,
    data: &[u8],
    resources_dir: Option<&std::path::Path>,
) -> Result<Vec<Contour>, String> {
    let uses_text = has_text_tag(data);
    let mut options = usvg::Options {
        resources_dir: resources_dir.map(|parent| parent.to_owned()),
        ..Default::default()
    };
    if uses_text {
        options.fontdb = Arc::clone(system_fonts());
    }
    let tree =
        usvg::Tree::from_data(data, &options).map_err(|error| format!("{label}: {error}"))?;
    let size = tree.size();
    let max_dim = size.width().max(size.height());
    if !max_dim.is_finite() || max_dim <= 0.0 {
        return Err(format!("{label} has an empty viewBox"));
    }
    let scale = SIDE / max_dim;
    let base = Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        (SIDE - size.width() * scale) / 2.0,
        (SIDE - size.height() * scale) / 2.0,
    );
    let mut shapes = Vec::new();
    let mut missing_text_glyphs = false;
    collect(tree.root(), base, &mut shapes, &mut missing_text_glyphs);
    if missing_text_glyphs {
        return Err(format!(
            "{label}: <text> produced no glyph outlines (this machine may lack a matching font; convert text to paths in your SVG editor)"
        ));
    }
    if shapes.is_empty() {
        if uses_text {
            return Err(format!(
                "{label} draws nothing (if it uses <text>, this machine may lack a matching font; convert text to paths in your SVG editor)"
            ));
        }
        return Err(format!("{label} draws nothing"));
    }
    let mut all = Vec::new();
    for (shape, even_odd) in shapes {
        let mut list = extract_contours(&shape);
        normalize_contours(&mut list, even_odd);
        all.extend(list);
    }
    if all.is_empty() {
        return Err(format!("{label} draws nothing"));
    }
    Ok(all)
}

/// The SVG at `path` as SVG/XAML path data (`M… Z …`) in a `SIDE`-unit square.
pub fn outline(path: &std::path::Path) -> Result<String, String> {
    let contours = outline_contours(path)?;
    Ok(format_contours(&contours))
}

/// Format normalized contours as SVG/XAML path data (`M… Z …`).
pub fn format_contours(contours: &[Contour]) -> String {
    let mut out = String::new();
    for contour in contours {
        contour.write(&mut out);
    }
    out.trim_end().to_owned()
}

/// Wrap outlined path data in a standalone monochrome SVG using `currentColor`
/// (usable by Web, GPUI, and GTK symbolic icons).
pub fn svg_document(path_data: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><path fill="currentColor" fill-rule="nonzero" d="{}"/></svg>"#,
        path_data.trim()
    )
}

/// Every filled shape under `group`, paired with whether its fill rule is
/// `evenodd`, already in the square's coordinates.
fn collect(
    group: &usvg::Group,
    outer: Transform,
    shapes: &mut Vec<(Path, bool)>,
    missing_text_glyphs: &mut bool,
) {
    let ts = outer.pre_concat(group.transform());
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect(group, ts, shapes, missing_text_glyphs),
            usvg::Node::Text(text) => {
                let flattened = text.flattened();
                if flattened.children().is_empty() {
                    *missing_text_glyphs = true;
                } else {
                    collect(flattened, ts, shapes, missing_text_glyphs);
                }
            }
            usvg::Node::Path(path) if path.is_visible() => {
                if let Some(fill) = path.fill()
                    && let Some(filled) = path.data().clone().transform(ts)
                {
                    shapes.push((filled, fill.rule() == usvg::FillRule::EvenOdd));
                }
                if let Some(stroke) = path.stroke() {
                    // Stroke each subpath independently so opposite-direction
                    // strokes inside one `<path>` (such as a cross) get
                    // normalized to the same winding and never cut holes.
                    let (sx, sy) = ts.get_scale();
                    let resolution = sx.max(sy).max(0.01) * 8.0;
                    let skia_stroke = stroke.to_tiny_skia();
                    for subpath in split_subpaths(path.data()) {
                        if let Some(covered) = subpath.stroke(&skia_stroke, resolution)
                            && let Some(covered) = covered.transform(ts)
                        {
                            shapes.push((covered, false));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Split a path into its individual `MoveTo` subpaths.
fn split_subpaths(path: &Path) -> Vec<Path> {
    let mut result = Vec::new();
    let mut builder: Option<PathBuilder> = None;
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(at) => {
                if let Some(prev) = builder.take().and_then(PathBuilder::finish) {
                    result.push(prev);
                }
                let mut next = PathBuilder::new();
                next.move_to(at.x, at.y);
                builder = Some(next);
            }
            PathSegment::LineTo(to) => {
                if let Some(b) = builder.as_mut() {
                    b.line_to(to.x, to.y);
                }
            }
            PathSegment::QuadTo(c, to) => {
                if let Some(b) = builder.as_mut() {
                    b.quad_to(c.x, c.y, to.x, to.y);
                }
            }
            PathSegment::CubicTo(c1, c2, to) => {
                if let Some(b) = builder.as_mut() {
                    b.cubic_to(c1.x, c1.y, c2.x, c2.y, to.x, to.y);
                }
            }
            PathSegment::Close => {
                if let Some(b) = builder.as_mut() {
                    b.close();
                }
            }
        }
    }
    if let Some(last) = builder.and_then(PathBuilder::finish) {
        result.push(last);
    }
    result
}

fn extract_contours(path: &Path) -> Vec<Contour> {
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

/// Normalize a shape's contours so outer contours wind positively (`area > 0`)
/// and holes wind negatively (`area < 0`) under the nonzero rule, converting
/// `evenodd` shapes into equivalent nonzero contours as well.
fn normalize_contours(contours: &mut [Contour], even_odd: bool) {
    let count = contours.len();
    if count == 0 {
        return;
    }
    let polys: Vec<Vec<Point>> = contours.iter().map(Contour::sampled_points).collect();
    let areas: Vec<f32> = polys.iter().map(|poly| polygon_area(poly)).collect();

    // `contains[outer][inner]` when `outer` strictly encloses `inner`.
    let mut depths = vec![0usize; count];
    let mut outer_root: Vec<Option<usize>> = vec![None; count];
    for i in 0..count {
        let Some(probe) = polys[i].first().copied() else {
            continue;
        };
        let mut largest_encloser: Option<(usize, f32)> = None;
        for j in 0..count {
            if i == j || areas[j].abs() <= areas[i].abs() + 1e-4 {
                continue;
            }
            if point_in_polygon(probe, &polys[j]) {
                depths[i] += 1;
                if largest_encloser.is_none_or(|(_, area)| areas[j].abs() > area) {
                    largest_encloser = Some((j, areas[j].abs()));
                }
            }
        }
        outer_root[i] = largest_encloser.map(|(j, _)| j);
    }

    for i in 0..count {
        let flip = if even_odd {
            let want_positive = depths[i].is_multiple_of(2);
            (want_positive && areas[i] < 0.0) || (!want_positive && areas[i] > 0.0)
        } else {
            let root = outer_root[i].unwrap_or(i);
            areas[root] < 0.0
        };
        if flip {
            contours[i].reverse();
        }
    }
}

fn polygon_area(points: &[Point]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0f32;
    for (index, a) in points.iter().enumerate() {
        let b = points[(index + 1) % points.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum / 2.0
}

fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        let intersects =
            ((a.y > p.y) != (b.y > p.y)) && (p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl Contour {
    /// Approximate the contour with a dense polyline for accurate area and
    /// containment tests on curved segments.
    fn sampled_points(&self) -> Vec<Point> {
        const STEPS: usize = 8;
        let mut points = vec![self.start];
        let mut cur = self.start;
        for segment in &self.segments {
            match *segment {
                Segment::Line(to) => {
                    points.push(to);
                    cur = to;
                }
                Segment::Quad(c, to) => {
                    for s in 1..=STEPS {
                        let t = s as f32 / STEPS as f32;
                        let mt = 1.0 - t;
                        let x = mt * mt * cur.x + 2.0 * mt * t * c.x + t * t * to.x;
                        let y = mt * mt * cur.y + 2.0 * mt * t * c.y + t * t * to.y;
                        points.push(Point::from_xy(x, y));
                    }
                    cur = to;
                }
                Segment::Cubic(c1, c2, to) => {
                    for s in 1..=STEPS {
                        let t = s as f32 / STEPS as f32;
                        let mt = 1.0 - t;
                        let x = mt * mt * mt * cur.x
                            + 3.0 * mt * mt * t * c1.x
                            + 3.0 * mt * t * t * c2.x
                            + t * t * t * to.x;
                        let y = mt * mt * mt * cur.y
                            + 3.0 * mt * mt * t * c1.y
                            + 3.0 * mt * t * t * c2.y
                            + t * t * t * to.y;
                        points.push(Point::from_xy(x, y));
                    }
                    cur = to;
                }
            }
        }
        points
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

    struct TempSvg(std::path::PathBuf);

    impl Drop for TempSvg {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn write_svg(name: &str, body: &str) -> TempSvg {
        let dir = std::env::temp_dir().join(format!("bern-app-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        TempSvg(path)
    }

    #[test]
    fn a_stroke_becomes_a_filled_outline_in_the_square() {
        let file = write_svg(
            "line.svg",
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="3 3 18 18" fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round"><path d="M8.5 6v12"/></svg>"##,
        );
        let data = outline(&file.0).unwrap();
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
        // Two strokes drawn in opposite directions inside a single `<path>`
        // (such as a cross): both come out positive so overlapping strokes
        // never cut a hole under nonzero winding.
        let file = write_svg(
            "cross.svg",
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="#000" stroke-width="2"><path d="M4 4L16 16M16 4L4 16"/></svg>"##,
        );
        let contours = outline_contours(&file.0).unwrap();
        assert_eq!(contours.len(), 2);
        let areas: Vec<f32> = contours
            .iter()
            .map(|c| polygon_area(&c.sampled_points()))
            .collect();
        assert!(areas.iter().all(|&area| area > 0.0), "{areas:?}");
    }

    #[test]
    fn evenodd_holes_become_opposite_winding_for_nonzero() {
        // Same-direction outer and inner squares with `fill-rule="evenodd"`:
        // outer must be positive and the inner hole must flip negative.
        let file = write_svg(
            "ring.svg",
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path fill="#000" fill-rule="evenodd" d="M2 2H18V18H2ZM6 6H14V14H6Z"/></svg>"##,
        );
        let contours = outline_contours(&file.0).unwrap();
        assert_eq!(contours.len(), 2);
        let areas: Vec<f32> = contours
            .iter()
            .map(|c| polygon_area(&c.sampled_points()))
            .collect();
        assert!(areas[0] > 0.0 && areas[1] < 0.0, "{areas:?}");
    }

    #[test]
    fn names_ending_in_svg_are_the_applications_own() {
        assert!(is_app_icon("icons/play.svg"));
        assert!(is_app_icon("Danmaku.SVG"));
        assert!(!is_app_icon("play.fill"));
    }
}
