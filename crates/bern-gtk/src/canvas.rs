//! `Canvas` and `Text(mark:)`: drawings in semantic colors.
//!
//! A canvas is a `GtkDrawingArea` drawn with cairo in its own coordinates.
//! Its drawing elements are read in one binding, so it redraws when the
//! state they show changes, as it does when it resizes or the colors behind
//! the roles change. Patterns are drawn band by band each time; nothing is
//! cached as a bitmap.

use crate::motion::{Curve, Num, bind_number};
use crate::reactive::{Version, effect};
use crate::theme::{self, Color};
use crate::{Ctx, Parent, Str};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

/// A picture a shape takes its average color from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A declared resource, by name.
    Resource(&'static str),
    /// A `file://` or http(s) URL; none while the state has none.
    Url(Option<String>),
}

/// A `Shape`: a rectangle, rounded rectangle, capsule or ellipse.
#[derive(Clone, Debug)]
pub struct ShapeOp {
    /// `rect`, `rounded`, `capsule` or `ellipse`.
    pub kind: &'static str,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub corner: Option<f64>,
    pub fill: Option<Color>,
    pub fill_from: Option<Source>,
    /// `up`, `down`, `left` or `right`: the fill fades out that way.
    pub fade: Option<&'static str>,
    pub stroke: Option<Color>,
    pub stroke_width: Option<f64>,
    pub opacity: Option<f64>,
}

impl ShapeOp {
    pub const NONE: ShapeOp = ShapeOp {
        kind: "rect",
        x: None,
        y: None,
        width: None,
        height: None,
        corner: None,
        fill: None,
        fill_from: None,
        fade: None,
        stroke: None,
        stroke_width: None,
        opacity: None,
    };
}

/// A `Series`: values across the full width.
#[derive(Clone, Debug)]
pub struct SeriesOp {
    pub values: Vec<f64>,
    pub max: Option<f64>,
    pub area: bool,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: Option<f64>,
    pub opacity: Option<f64>,
}

/// A band of a pattern: a width of a color at an opacity, or a gap.
#[derive(Clone, Copy, Debug)]
pub struct Band {
    pub color: Option<Color>,
    pub width: f64,
    pub opacity: f64,
}

/// A pattern of `style!`.
#[derive(Debug)]
pub struct PatternDef {
    pub bands: &'static [Band],
    pub twill: bool,
}

/// One drawing element.
#[derive(Clone, Debug)]
pub enum Op {
    Shape(ShapeOp),
    Series(SeriesOp),
    Pattern(&'static PatternDef, Option<f64>),
}

/// The values of a list, as numbers.
pub fn numbers(value: crate::protocol::Value) -> Vec<f64> {
    crate::value::as_list(value)
        .iter()
        .map(crate::value::as_f64)
        .collect()
}

/// The URL of a `MediaSource` value, or of an optional one.
pub fn url(value: &crate::protocol::Value) -> Option<String> {
    crate::media::url_of(value)
}

/// The drawing elements of a canvas, read in one binding.
pub type Ops = Rc<dyn Fn(&Ctx) -> Vec<Op>>;

pub struct CanvasProps {
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub label: Option<Str>,
    pub animate: Option<Curve>,
}

// MARK: - Pictures

/// A color as red, green and blue from 0 to 1.
type Rgb = (f64, f64, f64);

/// Average colors of pictures by URL.
type Averages = HashMap<String, Option<Rgb>>;

thread_local! {
    /// Average colors of pictures, by URL; `None` while reading or when the
    /// picture cannot be read.
    static AVERAGES: RefCell<Averages> = RefCell::new(HashMap::new());
    static READ: Version = Version::new();
}

fn average_of(pixbuf: &gtk::gdk_pixbuf::Pixbuf) -> Option<Rgb> {
    // The mean over the whole picture: a single pixel samples its middle.
    let small = pixbuf.scale_simple(32, 32, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    let channels = small.n_channels() as usize;
    let stride = small.rowstride() as usize;
    let bytes = small.read_pixel_bytes();
    let (mut red, mut green, mut blue, mut count) = (0.0, 0.0, 0.0, 0.0);
    for y in 0..small.height() as usize {
        for x in 0..small.width() as usize {
            let at = y * stride + x * channels;
            if at + 2 >= bytes.len() {
                continue;
            }
            red += f64::from(bytes[at]);
            green += f64::from(bytes[at + 1]);
            blue += f64::from(bytes[at + 2]);
            count += 1.0;
        }
    }
    (count > 0.0).then(|| {
        (
            red / count / 255.0,
            green / count / 255.0,
            blue / count / 255.0,
        )
    })
}

fn read_picture(url: String) {
    let file = if url.contains("://") {
        gio::File::for_uri(&url)
    } else {
        gio::File::for_path(&url)
    };
    let finished = url.clone();
    file.load_bytes_async(None::<&gio::Cancellable>, move |result| {
        let average = result.ok().and_then(|(bytes, _)| {
            let stream = gio::MemoryInputStream::from_bytes(&bytes);
            let pixbuf =
                gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, None::<&gio::Cancellable>).ok()?;
            let upright = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
            average_of(&upright)
        });
        AVERAGES.with(|averages| averages.borrow_mut().insert(finished, average));
        READ.with(Version::bump);
    });
}

/// The average color of a picture once read, recording a dependency on
/// pictures being read.
fn average(source: &Source) -> Option<Rgb> {
    let url = match source {
        Source::Resource(name) => std::path::Path::new(&crate::host::resources_dir())
            .join(name)
            .display()
            .to_string(),
        Source::Url(Some(url)) => url.clone(),
        Source::Url(None) => return None,
    };
    READ.with(Version::track);
    let known = AVERAGES.with(|averages| averages.borrow().get(&url).cloned());
    match known {
        Some(average) => average,
        None => {
            AVERAGES.with(|averages| averages.borrow_mut().insert(url.clone(), None));
            read_picture(url);
            None
        }
    }
}

// MARK: - Drawing

fn set_color(cr: &gtk::cairo::Context, color: &gtk::gdk::RGBA, opacity: f64) {
    cr.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()) * opacity,
    );
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let r = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + width - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + width - r, y + height - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + height - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
    cr.close_path();
}

/// A drawn element with the colors it shows, resolved when drawing.
enum Drawn {
    Shape(ShapeOp, Option<(f64, f64, f64)>),
    Series(SeriesOp),
    Pattern(&'static PatternDef, Option<f64>),
}

fn draw_shape(
    cr: &gtk::cairo::Context,
    widget: &gtk::Widget,
    op: &ShapeOp,
    average: Option<(f64, f64, f64)>,
    w: f64,
    h: f64,
) {
    let x = op.x.unwrap_or(0.0);
    let y = op.y.unwrap_or(0.0);
    let width = op.width.unwrap_or(w - x);
    let height = op.height.unwrap_or(h - y);
    let path = |cr: &gtk::cairo::Context| match op.kind {
        "ellipse" => {
            cr.save().ok();
            cr.translate(x + width / 2.0, y + height / 2.0);
            cr.scale((width / 2.0).max(0.001), (height / 2.0).max(0.001));
            cr.new_path();
            cr.arc(0.0, 0.0, 1.0, 0.0, 2.0 * PI);
            cr.restore().ok();
        }
        "rounded" => rounded(cr, x, y, width, height, op.corner.unwrap_or(8.0)),
        "capsule" => rounded(cr, x, y, width, height, width.min(height) / 2.0),
        _ => {
            cr.new_path();
            cr.rectangle(x, y, width, height);
        }
    };
    let opacity = op.opacity.unwrap_or(1.0);
    let fill = match average {
        Some((r, g, b)) => Some(gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, 1.0)),
        None if op.fill.is_some() || op.stroke.is_none() => Some(theme::rgba(
            &op.fill.unwrap_or(Color::Role("fill.strong")),
            widget,
        )),
        None => None,
    };
    if let Some(fill) = fill {
        path(cr);
        match op.fade {
            Some(fade) => {
                let (x0, y0, x1, y1) = match fade {
                    "up" => (x, y + height, x, y),
                    "left" => (x + width, y, x, y),
                    "right" => (x, y, x + width, y),
                    _ => (x, y, x, y + height),
                };
                let gradient = gtk::cairo::LinearGradient::new(x0, y0, x1, y1);
                let (r, g, b, a) = (
                    f64::from(fill.red()),
                    f64::from(fill.green()),
                    f64::from(fill.blue()),
                    f64::from(fill.alpha()) * opacity,
                );
                gradient.add_color_stop_rgba(0.0, r, g, b, a);
                gradient.add_color_stop_rgba(1.0, r, g, b, 0.0);
                cr.set_source(&gradient).ok();
            }
            None => set_color(cr, &fill, opacity),
        }
        cr.fill().ok();
    }
    if let Some(stroke) = &op.stroke {
        path(cr);
        set_color(cr, &theme::rgba(stroke, widget), opacity);
        cr.set_line_width(op.stroke_width.unwrap_or(1.0));
        cr.stroke().ok();
    }
}

/// The points of a series' values in `w` by `h`: spread across the width,
/// and up from the bottom by their share of `max` or of the largest value.
fn series_points(values: &[f64], max: Option<f64>, w: f64, h: f64) -> Vec<(f64, f64)> {
    let peak = max.unwrap_or_else(|| values.iter().copied().fold(f64::MIN, f64::max));
    let scale = if peak > 0.0 { peak } else { 1.0 };
    let count = values.len();
    let step = if count > 1 {
        w / (count - 1) as f64
    } else {
        0.0
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                if count > 1 {
                    index as f64 * step
                } else {
                    w / 2.0
                },
                h - h * (value / scale).clamp(0.0, 1.0),
            )
        })
        .collect()
}

fn draw_series(cr: &gtk::cairo::Context, widget: &gtk::Widget, op: &SeriesOp, w: f64, h: f64) {
    if op.values.is_empty() {
        return;
    }
    let count = op.values.len();
    let points = series_points(&op.values, op.max, w, h);
    let line = |cr: &gtk::cairo::Context| {
        cr.new_path();
        cr.move_to(points[0].0, points[0].1);
        for (x, y) in &points[1..] {
            cr.line_to(*x, *y);
        }
    };
    let opacity = op.opacity.unwrap_or(1.0);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_width(op.stroke_width.unwrap_or(2.0));
    if op.area {
        line(cr);
        cr.line_to(points[count - 1].0, h);
        cr.line_to(points[0].0, h);
        cr.close_path();
        set_color(
            cr,
            &theme::rgba(&op.fill.unwrap_or(Color::Role("accent")), widget),
            opacity,
        );
        cr.fill().ok();
        if let Some(stroke) = &op.stroke {
            line(cr);
            set_color(cr, &theme::rgba(stroke, widget), opacity);
            cr.stroke().ok();
        }
    } else {
        line(cr);
        set_color(
            cr,
            &theme::rgba(&op.stroke.unwrap_or(Color::Role("accent")), widget),
            opacity,
        );
        cr.stroke().ok();
    }
}

/// A woven check: the bands repeat across and down, and where colored
/// bands cross both show; with `twill`, fine diagonal lines over it.
fn draw_pattern(
    cr: &gtk::cairo::Context,
    widget: &gtk::Widget,
    pattern: &PatternDef,
    opacity: Option<f64>,
    w: f64,
    h: f64,
) {
    let sett: f64 = pattern.bands.iter().map(|band| band.width.max(0.0)).sum();
    if sett <= 0.0 {
        return;
    }
    let alpha = opacity.unwrap_or(1.0);
    for across in [true, false] {
        let extent = if across { w } else { h };
        let mut offset = 0.0;
        while offset < extent {
            for band in pattern.bands {
                if band.width <= 0.0 {
                    continue;
                }
                if let Some(color) = &band.color {
                    set_color(cr, &theme::rgba(color, widget), alpha * band.opacity);
                    if across {
                        cr.rectangle(offset, 0.0, band.width, h);
                    } else {
                        cr.rectangle(0.0, offset, w, band.width);
                    }
                    cr.fill().ok();
                }
                offset += band.width;
                if offset >= extent {
                    break;
                }
            }
        }
    }
    if pattern.twill {
        set_color(
            cr,
            &theme::rgba(&Color::Role("text.primary"), widget),
            alpha * 0.05,
        );
        cr.set_line_width(0.5);
        cr.new_path();
        let mut start = -h;
        while start < w {
            cr.move_to(start, h);
            cr.line_to(start + h, 0.0);
            start += 3.0;
        }
        cr.stroke().ok();
    }
}

/// `Canvas`: its drawing elements in order, in its own coordinates; it
/// fills the space where it has no size.
#[allow(non_snake_case)]
pub fn Canvas(p: &Parent, ctx: &Ctx, props: CanvasProps, ops: Ops) {
    let area = gtk::DrawingArea::new();
    match &props.width {
        Some(width) => {
            let sized = area.clone();
            bind_number(ctx, width, props.animate, &area, move |value| {
                sized.set_content_width(value.round().max(0.0) as i32);
            });
        }
        None => area.set_hexpand(true),
    }
    match &props.height {
        Some(height) => {
            let sized = area.clone();
            bind_number(ctx, height, props.animate, &area, move |value| {
                sized.set_content_height(value.round().max(0.0) as i32);
            });
        }
        None => area.set_vexpand(true),
    }
    match &props.label {
        Some(label) => {
            let at = ctx.clone();
            let label = Rc::clone(label);
            let named = area.clone();
            effect(move || named.update_property(&[gtk::accessible::Property::Label(&label(&at))]));
        }
        None => area.update_state(&[gtk::accessible::State::Hidden(true)]),
    }
    let drawn: Rc<RefCell<Vec<Drawn>>> = Rc::new(RefCell::new(Vec::new()));
    let shown = Rc::clone(&drawn);
    area.set_draw_func(move |area, cr, w, h| {
        let (w, h) = (f64::from(w), f64::from(h));
        let widget = area.upcast_ref::<gtk::Widget>();
        for item in shown.borrow().iter() {
            match item {
                Drawn::Shape(op, average) => draw_shape(cr, widget, op, *average, w, h),
                Drawn::Series(op) => draw_series(cr, widget, op, w, h),
                Drawn::Pattern(pattern, opacity) => {
                    draw_pattern(cr, widget, pattern, *opacity, w, h)
                }
            }
        }
    });
    let at = ctx.clone();
    let redrawn = area.clone();
    effect(move || {
        let list = ops(&at);
        theme::track();
        let items = list
            .into_iter()
            .map(|op| match op {
                Op::Shape(shape) => {
                    let average = shape.fill_from.as_ref().and_then(average);
                    Drawn::Shape(shape, average)
                }
                Op::Series(series) => Drawn::Series(series),
                Op::Pattern(pattern, opacity) => Drawn::Pattern(pattern, opacity),
            })
            .collect();
        *drawn.borrow_mut() = items;
        redrawn.queue_draw();
    });
    if p.is_list() {
        p.add(&crate::widgets::plain_row(&area));
    } else {
        p.add(&area);
    }
}

// MARK: - Text marks

mod marked {
    use super::*;

    #[derive(Default)]
    pub struct Marked {
        pub color: std::cell::Cell<Option<Color>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Marked {
        const NAME: &'static str = "BernMarkedText";
        type Type = super::Marked;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Marked {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Marked {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        /// As unmarked text: the width of its lines before it wraps,
        /// whatever height a row beside it offers.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match self.obj().first_child() {
                Some(child) if orientation == gtk::Orientation::Horizontal => {
                    child.measure(orientation, -1)
                }
                Some(child) => child.measure(orientation, for_size),
                None => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                child.allocate(width, height, baseline, None);
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            if let (Some(color), Some(label)) = (
                self.color.get(),
                widget
                    .first_child()
                    .and_then(|child| child.downcast::<gtk::Label>().ok()),
            ) {
                let rgba = theme::rgba(&color, widget.upcast_ref());
                let layout = label.layout();
                let (offset_x, offset_y) = label.layout_offsets();
                let mut lines = layout.iter();
                loop {
                    let (_, logical) = lines.line_extents();
                    let scale = f64::from(gtk::pango::SCALE);
                    let width = f64::from(logical.width()) / scale;
                    let height = f64::from(logical.height()) / scale;
                    if width > 0.0 {
                        let x = f64::from(offset_x) + f64::from(logical.x()) / scale;
                        let y = f64::from(offset_y) + f64::from(logical.y()) / scale;
                        // From 74 % to 98 % of the line, 0.4 em longer.
                        let band = height * 0.24;
                        let em = height / 1.2;
                        let rect = gtk::graphene::Rect::new(
                            x as f32,
                            (y + height * 0.74) as f32,
                            (width + em * 0.4) as f32,
                            band as f32,
                        );
                        let radius = (band / 2.0) as f32;
                        snapshot.push_rounded_clip(&gtk::gsk::RoundedRect::from_rect(rect, radius));
                        snapshot.append_color(&rgba, &rect);
                        snapshot.pop();
                    }
                    if !lines.next_line() {
                        break;
                    }
                }
            }
            self.parent_snapshot(snapshot);
        }
    }
}

glib::wrapper! {
    /// A label with a capsule of a color under each of its lines.
    pub struct Marked(ObjectSubclass<marked::Marked>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Marked {
    pub(crate) fn new(label: &gtk::Label, color: Color) -> Self {
        let marked: Self = glib::Object::new();
        marked.imp().color.set(Some(color));
        label.set_parent(&marked);
        // Lines move when the text changes or wraps differently.
        let redraw = marked.downgrade();
        label.connect_label_notify(move |_| {
            if let Some(marked) = redraw.upgrade() {
                marked.queue_draw();
            }
        });
        let themed = marked.downgrade();
        effect(move || {
            theme::track();
            if let Some(marked) = themed.upgrade() {
                marked.queue_draw();
            }
        });
        marked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Value;
    use gtk::cairo;

    #[test]
    fn series_values_read_as_numbers() {
        let values = Value::List(vec![
            Value::Int(1),
            Value::Float(2.5),
            Value::Text("x".into()),
        ]);
        assert_eq!(numbers(values), [1.0, 2.5, 0.0]);
        assert!(numbers(Value::Int(1)).is_empty());
    }

    #[test]
    fn series_rise_by_their_share_of_the_peak() {
        assert_eq!(
            series_points(&[0.0, 5.0, 10.0], None, 100.0, 50.0),
            [(0.0, 50.0), (50.0, 25.0), (100.0, 0.0)]
        );
        // Against a declared maximum, values past it or below zero stay
        // inside the area.
        assert_eq!(
            series_points(&[-1.0, 20.0, 40.0], Some(20.0), 10.0, 10.0),
            [(0.0, 10.0), (5.0, 0.0), (10.0, 0.0)]
        );
        // One value stands in the middle.
        assert_eq!(series_points(&[0.5], Some(1.0), 80.0, 10.0), [(40.0, 5.0)]);
        // Without a positive peak, values are shares of 1.
        assert_eq!(
            series_points(&[-2.0, -1.0], None, 10.0, 10.0),
            [(0.0, 10.0), (10.0, 10.0)]
        );
    }

    #[test]
    fn corners_round_at_most_to_half_the_shorter_side() {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).expect("a surface");
        let cr = cairo::Context::new(&surface).expect("a context");
        let inside = |x, y| cr.in_fill(x, y).expect("a path");
        // 40 by 30 with a radius of 100: the ends are half circles of 15.
        rounded(&cr, 10.0, 20.0, 40.0, 30.0, 100.0);
        assert!(!inside(11.0, 21.0));
        assert!(inside(30.0, 21.0));
        assert!(inside(10.5, 35.0) && inside(49.5, 35.0));
        assert!(!inside(51.0, 35.0));
        // A negative radius draws square corners.
        cr.new_path();
        rounded(&cr, 10.0, 20.0, 40.0, 30.0, -4.0);
        assert!(inside(10.5, 20.5) && inside(49.5, 49.5));
    }
}
