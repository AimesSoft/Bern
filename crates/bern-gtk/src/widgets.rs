//! Standard controls and containers as GTK 4 and libadwaita widgets.
//!
//! Inside a form section, controls are libadwaita rows (`AdwSwitchRow`,
//! `AdwEntryRow`, `AdwComboRow` and action rows) in a boxed list; elsewhere
//! they are the plain GTK controls. Colors and type come from the system
//! style: widgets only use libadwaita's style classes.

use crate::controls::{self, Actions};
use crate::ctx::Field;
use crate::motion::{Curve, Num, bind_number};
use crate::parent::keeps_width;
use crate::protocol::Value;
use crate::reactive::{effect, untrack};
use crate::text::{self, tr};
use crate::value::{as_f64, as_text, number};
use crate::{Act, Build, Ctx, Get, Parent, Str, icons};
use adw::prelude::*;
use bern_ffi::overlay::Route;
use std::cell::Cell;
use std::rc::Rc;

/// A text getter bound to a context.
pub(crate) fn bound_text(ctx: &Ctx, text: &Str) -> Rc<dyn Fn() -> String> {
    let ctx = ctx.clone();
    let text = Rc::clone(text);
    Rc::new(move || text(&ctx))
}

pub(crate) fn bound_enabled(ctx: &Ctx, enabled: &Option<Get>) -> Option<Rc<dyn Fn() -> bool>> {
    let enabled = enabled.clone()?;
    let ctx = ctx.clone();
    Some(Rc::new(move || crate::value::truthy(&enabled(&ctx))))
}

pub(crate) fn bound_action(ctx: &Ctx, action: &Option<Act>) -> Option<Rc<dyn Fn()>> {
    let action = action.clone()?;
    let ctx = ctx.clone();
    Some(Rc::new(move || action(&ctx)))
}

/// Keep a label's text in step with a getter.
pub(crate) fn bind_label(label: &gtk::Label, text: Rc<dyn Fn() -> String>) {
    let label = label.clone();
    effect(move || label.set_label(&text()));
}

thread_local! {
    static ACTIVATIONS: std::cell::RefCell<Vec<(glib::WeakRef<gtk::ListBoxRow>, crate::Thunk)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `activate` when the row is activated by a click or the keyboard,
/// until the current scope ends.
pub(crate) fn on_activate(row: &gtk::ListBoxRow, activate: Rc<dyn Fn()>) {
    let weak = row.downgrade();
    ACTIVATIONS.with(|rows| {
        rows.borrow_mut().retain(|(row, _)| row.upgrade().is_some());
        rows.borrow_mut().push((weak, activate));
    });
    let ended = row.downgrade();
    crate::reactive::on_cleanup(move || {
        ACTIVATIONS.with(|rows| {
            rows.borrow_mut()
                .retain(|(row, _)| row.upgrade().is_some() && row.upgrade() != ended.upgrade())
        });
    });
}

/// A boxed list whose activatable rows run their actions.
pub(crate) fn boxed_list() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.connect_row_activated(|_, row| {
        let found = ACTIVATIONS.with(|rows| {
            rows.borrow()
                .iter()
                .find(|(candidate, _)| candidate.upgrade().as_ref() == Some(row))
                .map(|(_, activate)| Rc::clone(activate))
        });
        if let Some(activate) = found {
            activate();
        }
    });
    list
}

/// A row of a boxed list holding `child`, for content that is not a
/// libadwaita row itself. Its padding is the row's, so a form's layout
/// can change it.
pub(crate) fn plain_row(child: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.add_css_class("bern-plain-row");
    row.set_child(Some(child.as_ref()));
    row
}

/// Add a widget as it fits the parent: plainly, or in a row of a list.
pub(crate) fn place(p: &Parent, widget: &impl IsA<gtk::Widget>) {
    if p.is_list() {
        p.add(&plain_row(widget));
    } else {
        p.add(widget);
    }
}

fn style_class(style: Option<&str>) -> &'static str {
    match style {
        Some("large_title") => "title-1",
        Some("title") => "title-2",
        Some("title2") => "title-3",
        Some("title3") => "title-4",
        Some("headline") | Some("subheadline") => "heading",
        Some("footnote") | Some("caption") | Some("caption2") => "caption",
        Some("code") => "monospace",
        _ => "body",
    }
}

#[derive(Default)]
pub struct TextProps {
    pub content: Option<Str>,
    /// JSON scalar-indexed [start, end, reading] ranges.
    pub ruby: Option<Str>,
    pub style: Option<&'static str>,
    pub emphasis: Option<&'static str>,
    /// The accent or a status role the text is drawn in.
    pub color: Option<&'static str>,
    /// A capsule of this color under each line.
    pub mark: Option<crate::theme::Color>,
    /// At most this many lines, the last ending in an ellipsis.
    pub lines: Option<f64>,
    /// The size of the type in points, in place of the style's.
    pub font_size: Option<Num>,
    /// The height of each line, the extra split above and below it.
    pub line_height: Option<Num>,
    /// The content is inline Markdown.
    pub markdown: bool,
    /// Bold while this holds, else in the style's weight.
    pub bold: Option<Get>,
    /// As far through its characters as this as it is, the rest faded; below
    /// zero, all of it.
    pub lit: Option<Num>,
    /// Seconds and characters: `lit` at zero or above moves along them by
    /// itself, as `lit_clock` runs from `lit_since`.
    pub lit_path: Option<Str>,
    pub lit_clock: Option<Num>,
    pub lit_since: Option<Num>,
    /// The spring a size that follows state changes with.
    pub animate: Option<Curve>,
    /// Whether the user may select and copy the text.
    pub selectable: bool,
}

/// The size and line height of a label's text, as Pango attributes.
fn text_attributes(label: &gtk::Label, size: Option<f64>, line_height: Option<f64>) {
    let attributes = gtk::pango::AttrList::new();
    let scale = f64::from(gtk::pango::SCALE);
    if let Some(size) = size {
        attributes.insert(gtk::pango::AttrSize::new_size_absolute(
            (size.max(1.0) * scale).round() as i32,
        ));
    }
    let line_height = if label.has_css_class("bern-ruby") {
        let metrics = label.pango_context().metrics(None, None);
        let em = size.unwrap_or(f64::from(metrics.ascent() + metrics.descent()) / scale);
        Some(line_height.unwrap_or(0.0).max(em * 2.1))
    } else {
        line_height
    };
    if let Some(height) = line_height {
        attributes.insert(gtk::pango::AttrInt::new_line_height_absolute(
            (height.max(0.0) * scale).round() as i32,
        ));
    }
    label.set_attributes(Some(&attributes));
}

/// Keep a label's size and line height in step with bound lengths, the
/// size springing with `curve` when given.
fn bind_text_size(
    ctx: &Ctx,
    label: &gtk::Label,
    size: Option<Num>,
    line_height: Option<Num>,
    curve: Option<Curve>,
) {
    if size.is_none() && line_height.is_none() {
        return;
    }
    let values = Rc::new(Cell::new((None::<f64>, None::<f64>)));
    if let Some(size) = &size {
        let (values, label_) = (Rc::clone(&values), label.clone());
        bind_number(ctx, size, curve, label, move |value| {
            values.set((Some(value), values.get().1));
            let (size, height) = values.get();
            text_attributes(&label_, size, height);
        });
    }
    if let Some(height) = &line_height {
        let (values, label_) = (Rc::clone(&values), label.clone());
        bind_number(ctx, height, None, label, move |value| {
            values.set((values.get().0, Some(value)));
            let (size, height) = values.get();
            text_attributes(&label_, size, height);
        });
    }
}

/// The classes of a text style: libadwaita's for its size, and Bern's for
/// the weights and sizes `text { … }` gives.
fn text_style(widget: &impl IsA<gtk::Widget>, style: Option<&str>) {
    // `control`: the font of the control it stands in, which the label
    // inherits, as a button's title does.
    if style == Some("control") {
        return;
    }
    widget.add_css_class(style_class(style));
    widget.add_css_class(&crate::style::text_class(style.unwrap_or("body")));
}

#[allow(non_snake_case)]
pub fn Text(p: &Parent, ctx: &Ctx, props: TextProps) {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    text_style(&label, props.style);
    if matches!(props.emphasis, Some("secondary") | Some("tertiary")) {
        label.add_css_class("dim-label");
    }
    // libadwaita's text colors of the accent and the status colors.
    match props.color {
        Some("accent") => label.add_css_class("accent"),
        Some("status.success") => label.add_css_class("success"),
        Some("status.warning") => label.add_css_class("warning"),
        Some("status.error") => label.add_css_class("error"),
        _ => {}
    }
    if let Some(lines) = props.lines {
        label.set_lines(lines.max(1.0) as i32);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    }
    bind_text_size(
        ctx,
        &label,
        props.font_size.clone(),
        props.line_height.clone(),
        props.animate,
    );
    if let Some(bold) = props.bold {
        let (at, shown) = (ctx.clone(), label.clone());
        effect(move || {
            if crate::value::truthy(&bold(&at)) {
                shown.add_css_class("bern-bold");
            } else {
                shown.remove_css_class("bern-bold");
            }
        });
    }
    if props.selectable {
        label.set_selectable(true);
        label.set_can_focus(true);
    }
    if let Some(content) = &props.content {
        if props.markdown {
            let text = bound_text(ctx, content);
            let shown = label.clone();
            shown.set_use_markup(true);
            effect(move || shown.set_markup(&crate::markdown::markup(&text())));
        } else {
            bind_label(&label, bound_text(ctx, content));
        }
    }
    let shown: gtk::Widget = match props.mark {
        Some(mark) => crate::canvas::Marked::new(&label, mark).upcast(),
        None => crate::layout::Words::wrap(&label).upcast(),
    };
    let shown = if let Some(lit) = &props.lit {
        let shown = crate::lit::Lit::new(&shown, &label, -1.0);
        let bound = shown.clone();
        bind_number(ctx, lit, None, &label, move |value| bound.set(value));
        if let Some(path) = &props.lit_path {
            let (at, path, bound) = (ctx.clone(), bound_text(ctx, path), shown.clone());
            let (lit, clock, since) = (
                lit.clone(),
                props.lit_clock.clone(),
                props.lit_since.clone(),
            );
            effect(move || {
                let read = |value: &Option<Num>| value.as_ref().map_or(0.0, |value| value.now(&at));
                let route = if lit.now(&at) < 0.0 {
                    Route::default()
                } else {
                    Route::new(&path(), read(&clock), read(&since))
                };
                bound.follow(route);
            });
        }
        shown.upcast::<gtk::Widget>()
    } else {
        shown
    };
    let shown: gtk::Widget = if let Some(ruby) = &props.ruby {
        let wrapped = RubyText::new(&shown, &label);
        let (at, ruby, weak, label) = (
            ctx.clone(),
            bound_text(ctx, ruby),
            wrapped.downgrade(),
            label.clone(),
        );
        let (size, height) = (props.font_size.clone(), props.line_height.clone());
        let content = props.content.clone();
        let markdown = props.markdown;
        let mapped_label = label.clone();
        effect(move || {
            let label = &mapped_label;
            let source = content
                .as_ref()
                .map_or_else(String::new, |content| content(&at));
            let mut ranges = ruby_text_ranges(&source, &ruby());
            if markdown {
                ranges = ranges
                    .into_iter()
                    .filter_map(|(range, reading)| {
                        if source.contains(['\u{fdd0}', '\u{fdd1}']) {
                            return None;
                        }
                        let marked = format!(
                            "{}\u{fdd0}{}\u{fdd1}{}",
                            &source[..range.start],
                            &source[range.clone()],
                            &source[range.end..]
                        );
                        let (_, mapped, _) =
                            gtk::pango::parse_markup(&crate::markdown::markup(&marked), '\0')
                                .ok()?;
                        if mapped.replace(['\u{fdd0}', '\u{fdd1}'], "") != label.text() {
                            return None;
                        }
                        let start = mapped.find('\u{fdd0}')?;
                        let end = mapped
                            .find('\u{fdd1}')?
                            .checked_sub('\u{fdd0}'.len_utf8())?;
                        (start < end).then_some((start..end, reading))
                    })
                    .collect();
            }
            if let Some(wrapped) = weak.upgrade() {
                use gtk::subclass::prelude::*;
                let active = !ranges.is_empty();
                *wrapped.imp().ranges.borrow_mut() = ranges;
                if active {
                    label.add_css_class("bern-ruby");
                } else {
                    label.remove_css_class("bern-ruby");
                }
                text_attributes(
                    label,
                    size.as_ref().map(|v| v.now(&at)),
                    height.as_ref().map(|v| v.now(&at)),
                );
                wrapped.queue_resize();
                wrapped.queue_draw();
            }
        });
        // Use the same bound scalar progress, Route and GTK frame clock as
        // the base Lit; only the reading's native glyph clip is different.
        if let Some(lit) = &props.lit {
            let bound = wrapped.clone();
            bind_number(ctx, lit, None, &label, move |value| bound.set_lit(value));
            if let Some(path) = &props.lit_path {
                let (at, path, bound) = (ctx.clone(), bound_text(ctx, path), wrapped.clone());
                let (lit, clock, since) = (
                    lit.clone(),
                    props.lit_clock.clone(),
                    props.lit_since.clone(),
                );
                effect(move || {
                    let read =
                        |value: &Option<Num>| value.as_ref().map_or(0.0, |value| value.now(&at));
                    bound.follow_lit(if lit.now(&at) < 0.0 {
                        Route::default()
                    } else {
                        Route::new(&path(), read(&clock), read(&since))
                    });
                });
            }
        }
        wrapped.upcast()
    } else {
        shown
    };
    place(p, &shown);
}

fn ruby_text_ranges(content: &str, source: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let Ok(ranges) = serde_json::from_str::<Vec<(usize, usize, String)>>(source) else {
        return Vec::new();
    };
    let offsets: Vec<_> = content
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(content.len()))
        .collect();
    let mut previous = 0;
    let mut result = Vec::new();
    for (start, end, reading) in ranges {
        if start < previous
            || start >= end
            || end >= offsets.len()
            || reading.is_empty()
            || reading.contains(['\n', '\r'])
        {
            continue;
        }
        let range = offsets[start]..offsets[end];
        if content[range.clone()].contains(['\n', '\r']) {
            continue;
        }
        result.push((range, reading));
        previous = end;
    }
    result
}

mod ruby_text {
    use super::*;
    use gtk::glib;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct RubyText {
        pub label: std::cell::OnceCell<gtk::Label>,
        pub ranges: std::cell::RefCell<Vec<(std::ops::Range<usize>, String)>>,
        pub lit: Cell<Option<f64>>,
        pub route: std::cell::RefCell<Route>,
        pub tick: std::cell::RefCell<Option<gtk::TickCallbackId>>,
        pub drawn: Cell<f64>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for RubyText {
        const NAME: &'static str = "BernRubyText";
        type Type = super::RubyText;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for RubyText {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for RubyText {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            self.obj().first_child().map_or((0, 0, -1, -1), |child| {
                child.measure(
                    orientation,
                    if orientation == gtk::Orientation::Horizontal {
                        -1
                    } else {
                        for_size
                    },
                )
            })
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                child.allocate(width, height, baseline, None);
            }
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(child) = self.obj().first_child() {
                self.obj().snapshot_child(&child, snapshot);
            }
            let Some(label) = self.label.get() else {
                return;
            };
            let layout = label.layout();
            let source = label.text();
            let lit = self.obj().lit_place();
            self.drawn.set(lit);
            let cut = (lit >= 0.0)
                .then(|| source.char_indices().nth(lit as usize))
                .flatten();
            let scale = f64::from(gtk::pango::SCALE);
            let (offset_x, offset_y) = label.layout_offsets();
            let offset = label
                .compute_point(&*self.obj(), &gtk::graphene::Point::new(0.0, 0.0))
                .unwrap_or_default();
            let mut iter = layout.iter();
            loop {
                if let Some(line) = iter.line_readonly() {
                    let start = line.start_index().max(0) as usize;
                    let end = start + line.length().max(0) as usize;
                    for (range, reading) in self.ranges.borrow().iter() {
                        let a = range.start.max(start);
                        let b = range.end.min(end);
                        if a >= b
                            || range.end > source.len()
                            || !source.is_char_boundary(a)
                            || !source.is_char_boundary(b)
                        {
                            continue;
                        }
                        let total = source[range.clone()].chars().count();
                        let before = source[range.start..a].chars().count();
                        let through = source[range.start..b].chars().count();
                        let chars: Vec<_> = reading.chars().collect();
                        let reading: String = chars
                            [chars.len() * before / total..chars.len() * through / total]
                            .iter()
                            .collect();
                        if reading.is_empty() {
                            continue;
                        }
                        let ink = label.create_pango_layout(Some(&reading));
                        let mut font = layout.font_description().unwrap_or_else(|| {
                            label.pango_context().font_description().unwrap_or_default()
                        });
                        // Per-label absolute font size attributes take priority over CSS.
                        let mut em = f64::from(font.size()) / scale;
                        if let Some(attrs) = label.attributes() {
                            let iterator = attrs.iterator();
                            if let Some(size) = iterator
                                .get(gtk::pango::AttrType::AbsoluteSize)
                                .and_then(|a| a.downcast::<gtk::pango::AttrSize>().ok())
                            {
                                em = f64::from(size.size()) / scale;
                            }
                        }
                        font.set_absolute_size(em.max(1.0) * scale);
                        let metrics = layout.context().metrics(Some(&font), None);
                        let height = (em * 0.5).max(1.0);
                        font.set_absolute_size(height * scale);
                        ink.set_font_description(Some(&font));
                        if let Some(attrs) = layout.attributes() {
                            let mut iterator = attrs.iterator();
                            while iterator.range().1 <= a as i32 && iterator.next_style_change() {}
                            let reading_attrs = gtk::pango::AttrList::new();
                            for kind in [
                                gtk::pango::AttrType::Weight,
                                gtk::pango::AttrType::Style,
                                gtk::pango::AttrType::Family,
                                gtk::pango::AttrType::Foreground,
                            ] {
                                if let Some(mut attr) = iterator.get(kind) {
                                    attr.set_start_index(0);
                                    attr.set_end_index(u32::MAX);
                                    reading_attrs.insert(attr);
                                }
                            }
                            ink.set_attributes(Some(&reading_attrs));
                        }
                        // Pango returns separate visual ranges for bidirectional text.
                        for pair in line.x_ranges(a as i32, b as i32).as_chunks::<2>().0 {
                            let left = f64::from(pair[0].min(pair[1])) / scale;
                            let width = f64::from((pair[1] - pair[0]).abs()) / scale;
                            let (ink_width, _) = ink.pixel_size();
                            if width <= 0.0 || ink_width <= 0 {
                                continue;
                            }
                            let fit = (width / f64::from(ink_width)).min(1.0);
                            snapshot.save();
                            snapshot.translate(&gtk::graphene::Point::new(
                                offset.x()
                                    + offset_x as f32
                                    + (left + (width - f64::from(ink_width) * fit) / 2.0) as f32,
                                offset.y()
                                    + offset_y as f32
                                    + (f64::from(iter.baseline() - metrics.ascent()) / scale
                                        - height) as f32,
                            ));
                            snapshot.scale(fit as f32, fit as f32);
                            if let Some((at, _)) = cut {
                                let alpha = if at >= b { 1.0 } else { 0.4 };
                                snapshot.push_opacity(alpha);
                                snapshot.append_layout(&ink, &label.color());
                                snapshot.pop();
                                if at >= a && at < b {
                                    let position = layout.index_to_pos(at as i32);
                                    let edge = (f64::from(position.x())
                                        + f64::from(position.width()) * lit.fract())
                                        / scale;
                                    let origin = left + (width - f64::from(ink_width) * fit) / 2.0;
                                    let lit_width =
                                        ((edge - origin) / fit).clamp(0.0, f64::from(ink_width));
                                    let (_, ink_height) = ink.pixel_size();
                                    snapshot.push_clip(&gtk::graphene::Rect::new(
                                        0.0,
                                        0.0,
                                        lit_width as f32,
                                        ink_height as f32,
                                    ));
                                    snapshot.append_layout(&ink, &label.color());
                                    snapshot.pop();
                                }
                            } else {
                                snapshot.append_layout(&ink, &label.color());
                            }
                            snapshot.restore();
                        }
                    }
                }
                if !iter.next_line() {
                    break;
                }
            }
        }
    }
}

gtk::glib::wrapper! {
    pub struct RubyText(ObjectSubclass<ruby_text::RubyText>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
impl RubyText {
    fn lit_place(&self) -> f64 {
        use gtk::subclass::prelude::*;
        let lit = self.imp().lit.get().unwrap_or(-1.0);
        let route = self.imp().route.borrow();
        if lit < 0.0 || route.points.is_empty() {
            lit
        } else {
            route.place()
        }
    }

    fn set_lit(&self, value: f64) {
        use gtk::subclass::prelude::*;
        self.imp().lit.set(Some(value));
        self.keep_lit_moving();
    }

    fn follow_lit(&self, route: Route) {
        use gtk::subclass::prelude::*;
        self.imp().route.replace(route);
        self.keep_lit_moving();
    }

    fn keep_lit_moving(&self) {
        use gtk::subclass::prelude::*;
        let moving = self.imp().lit.get().is_some_and(|lit| lit >= 0.0)
            && self.imp().route.borrow().moving();
        let mut tick = self.imp().tick.borrow_mut();
        if moving && tick.is_none() {
            *tick = Some(self.add_tick_callback(|widget, _| {
                if (widget.lit_place() - widget.imp().drawn.get()).abs() >= 1.0 / 48.0 {
                    widget.queue_draw();
                }
                gtk::glib::ControlFlow::Continue
            }));
        } else if !moving && let Some(id) = tick.take() {
            id.remove();
        }
        self.queue_draw();
    }

    fn new(child: &gtk::Widget, label: &gtk::Label) -> Self {
        use gtk::subclass::prelude::*;
        let widget: Self = gtk::glib::Object::new();
        let _ = widget.imp().label.set(label.clone());
        child.set_parent(&widget);
        widget
    }
}

#[derive(Default)]
pub struct LabelProps {
    pub title: Option<Str>,
    pub icon: Option<&'static str>,
    pub style: Option<&'static str>,
    /// The size of the title and icon in points.
    pub font_size: Option<f64>,
    pub color: Option<&'static str>,
    /// Only the icon shows; the title names it for assistive technology.
    pub icon_only: bool,
}

#[allow(non_snake_case)]
pub fn Label(p: &Parent, ctx: &Ctx, props: LabelProps) {
    let title = props.title.as_ref().map(|title| bound_text(ctx, title));
    let icon = |name: &str| {
        let image = icons::image(name);
        if let Some(size) = props.font_size {
            image.set_pixel_size(size.round().max(1.0) as i32);
        }
        image
    };
    if props.icon_only {
        // An image's role is already `img`.
        let image = icon(props.icon.unwrap_or("circle"));
        if let Some(role) = props.color {
            crate::theme::apply_role_color(image.upcast_ref(), role);
        }
        if let Some(title) = title {
            let named = image.clone();
            effect(move || {
                let text = title();
                named.set_tooltip_text(Some(&text));
                named.update_property(&[gtk::accessible::Property::Label(&text)]);
            });
        }
        place(p, &image);
        return;
    }
    if p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(name) = props.icon {
            row.add_prefix(&icon(name));
        }
        if let Some(role) = props.color {
            crate::theme::apply_role_color(row.upcast_ref(), role);
        }
        if let Some(title) = title {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&title())));
        }
        p.add(&row);
        return;
    }
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if let Some(name) = props.icon {
        container.append(&icon(name));
    }
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    text_style(&label, props.style);
    if let Some(size) = props.font_size {
        text_attributes(&label, Some(size), None);
    }
    if let Some(title) = title {
        bind_label(&label, title);
    }
    container.append(&label);
    if let Some(role) = props.color {
        crate::theme::apply_role_color(container.upcast_ref(), role);
    }
    p.add(&container);
}

#[derive(Default)]
pub struct ButtonProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub role: Option<&'static str>,
    pub prominence: Option<&'static str>,
    pub enabled: Option<Get>,
    pub style: Option<&'static crate::style::StyleDef>,
    /// `pick:`: the open panel asks for a file before the action runs.
    pub pick: Option<crate::features::Pick>,
    /// `paste:`: the clipboard's text goes into this draft first.
    pub paste: Option<crate::ctx::Field>,
    pub shape: Shape,
    /// `width: fill`: as wide as its container.
    pub fill: bool,
    /// `ContextMenu`: what a secondary click or a long touch opens.
    pub menu: Vec<Option<MenuItem>>,
    pub double_tap: Option<Act>,
    pub cursor: Option<&'static str>,
}

fn register_button(
    ctx: &Ctx,
    button: &gtk::Button,
    id: &str,
    label: Option<Rc<dyn Fn() -> String>>,
    enabled: Option<Rc<dyn Fn() -> bool>>,
    action: Option<Rc<dyn Fn()>>,
    set_file: Option<(crate::features::Give, &'static str)>,
) {
    controls::register(
        ctx,
        id,
        "button",
        label,
        enabled,
        Actions {
            activate: action,
            file_kinds: set_file.as_ref().map(|(_, kinds)| *kinds),
            set_file: set_file.map(|(give, _)| give),
            focus: Some(crate::features::focus_of(button.upcast_ref())),
            ..Actions::default()
        },
    );
}

/// The button as it goes into its container: in a motion box when a style
/// moves it.
fn styled(button: &gtk::Button, style: Option<&'static crate::style::StyleDef>) -> gtk::Widget {
    match style {
        Some(style) => {
            let motion = crate::motion::MotionBox::wrap(button);
            crate::style::apply(style, button.upcast_ref(), button.upcast_ref(), &motion);
            motion.upcast()
        }
        None => button.clone().upcast(),
    }
}

#[allow(non_snake_case)]
pub fn Button(p: &Parent, ctx: &Ctx, props: ButtonProps, action: Option<Act>) {
    let button = gtk::Button::new();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    match (props.icon, &label) {
        (Some(icon), Some(label)) => {
            let content = adw::ButtonContent::new();
            content.set_icon_name(icons::name(icon));
            let shown = content.clone();
            let label = Rc::clone(label);
            effect(move || shown.set_label(&label()));
            button.set_child(Some(&content));
        }
        (Some(icon), None) => button.set_icon_name(icons::name(icon)),
        (None, Some(label)) => {
            let shown = button.clone();
            let label = Rc::clone(label);
            effect(move || shown.set_label(&label()));
        }
        (None, None) => {}
    }
    match props.prominence {
        Some("prominent") => button.add_css_class("suggested-action"),
        Some("plain") => button.add_css_class("flat"),
        _ => {}
    }
    if props.role == Some("destructive") {
        button.add_css_class("destructive-action");
    }
    let enabled = bound_enabled(ctx, &props.enabled);
    if let Some(enabled) = enabled.clone() {
        let shown = button.clone();
        let cursor = props.cursor;
        effect(move || {
            let sensitive = enabled();
            shown.set_sensitive(sensitive);
            match cursor {
                Some("pointer") => shown.set_cursor_from_name(sensitive.then_some("pointer")),
                Some("default") => shown.set_cursor_from_name(Some("default")),
                _ => {}
            }
        });
    } else if matches!(props.cursor, Some("pointer" | "default")) {
        button.set_cursor_from_name(props.cursor);
    }
    let (run, set_file) =
        crate::features::picking(ctx, props.pick.clone(), bound_action(ctx, &action));
    let run = crate::features::pasting(ctx, props.paste, run);
    crate::gestures::button_clicks(&button, run.clone(), bound_action(ctx, &props.double_tap));
    register_button(
        ctx,
        &button,
        props.id,
        label,
        enabled,
        run,
        set_file.zip(props.pick.map(|pick| pick.kinds)),
    );
    shape(ctx, button.upcast_ref(), &props.shape);
    if !props.menu.is_empty() {
        crate::table::context_menu(ctx, button.upcast_ref(), props.menu);
    }
    if props.fill {
        button.set_hexpand(true);
        button.set_halign(gtk::Align::Fill);
    }
    if p.is_list() && !ctx.in_toolbar() {
        if !props.fill {
            button.set_halign(gtk::Align::Start);
        }
        let row = plain_row(&styled(&button, props.style));
        row.add_css_class("bern-button-row");
        p.add(&row);
    } else {
        let widget = styled(&button, props.style);
        if !props.fill {
            keeps_width(&widget);
        }
        p.add(&widget);
    }
}

#[derive(Default)]
pub struct IconButtonProps {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: Option<Str>,
    pub enabled: Option<Get>,
    pub style: Option<&'static crate::style::StyleDef>,
    pub pick: Option<crate::features::Pick>,
    /// `paste:`: the clipboard's text goes into this draft first.
    pub paste: Option<crate::ctx::Field>,
    /// The size of the icon in points.
    pub font_size: Option<f64>,
    /// A round button of this diameter.
    pub diameter: Option<Num>,
    /// `prominent`: filled with the accent color.
    pub prominence: Option<&'static str>,
    pub icon_progress: Option<Num>,
    pub badge_icon: Option<&'static str>,
    pub badge_visible: Option<Get>,
    pub color: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn IconButton(p: &Parent, ctx: &Ctx, props: IconButtonProps, action: Option<Act>) {
    let button = gtk::Button::from_icon_name(icons::name(props.icon));
    if props.prominence == Some("prominent") {
        button.add_css_class("suggested-action");
    } else {
        button.add_css_class("flat");
    }
    if let Some(size) = props.font_size
        && let Some(image) = button.child().and_downcast::<gtk::Image>()
    {
        image.set_pixel_size(size.round().max(1.0) as i32);
    }
    if props.icon_progress.is_some() || props.badge_icon.is_some() {
        let glyph =
            crate::icon_glyph::IconGlyph::new(props.icon, props.badge_icon, props.font_size);
        button.set_child(Some(&glyph));
        if let Some(progress) = &props.icon_progress {
            let shown = glyph.clone();
            bind_number(ctx, progress, None, &glyph, move |value| {
                shown.set_progress(value)
            });
        }
        if let Some(visible) = bound_enabled(ctx, &props.badge_visible) {
            let shown = glyph.clone();
            effect(move || shown.set_badge_visible(visible()));
        }
    }
    if let Some(diameter) = &props.diameter {
        button.add_css_class("circular");
        button.add_css_class("bern-sized-control");
        let sized = button.clone();
        bind_number(ctx, diameter, None, &button, move |value| {
            let side = value.round().max(0.0) as i32;
            sized.set_size_request(side, side);
        });
    }
    if let Some(role) = props.color {
        crate::theme::apply_role_color(button.upcast_ref(), role);
    }
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if let Some(label) = label.clone() {
        let shown = button.clone();
        effect(move || {
            let text = label();
            shown.set_tooltip_text(Some(&text));
            shown.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let enabled = bound_enabled(ctx, &props.enabled);
    if let Some(enabled) = enabled.clone() {
        let shown = button.clone();
        effect(move || shown.set_sensitive(enabled()));
    }
    let (run, set_file) =
        crate::features::picking(ctx, props.pick.clone(), bound_action(ctx, &action));
    let run = crate::features::pasting(ctx, props.paste, run);
    if let Some(run) = run.clone() {
        button.connect_clicked(move |_| run());
    }
    register_button(
        ctx,
        &button,
        props.id,
        label,
        enabled,
        run,
        set_file.zip(props.pick.map(|pick| pick.kinds)),
    );
    button.set_valign(gtk::Align::Center);
    let widget = styled(&button, props.style);
    keeps_width(&widget);
    place(p, &widget);
}

/// What a control shows and changes.
#[derive(Clone)]
pub enum Model {
    /// A draft field of the window; the action, if any, runs after each
    /// change.
    Field(Field, Option<Act>),
    /// A value read from state; a change runs this with the new value, as
    /// a command that sets it.
    Controlled(Get, Commit),
}

/// Runs the command that changes a controlled value to the one given.
pub type Commit = Rc<dyn Fn(&Ctx, Value)>;

/// Turn a control bound to a `bool` on or off, as its shortcut does.
pub fn toggle(ctx: &Ctx, model: &Model) {
    let bound = Bound::model(ctx, model.clone());
    bound.set(Value::Bool(bound.peek() != Value::Bool(true)));
}

/// The value a control is bound to.
#[derive(Clone)]
pub(crate) struct Bound {
    ctx: Ctx,
    model: Model,
}

impl Bound {
    pub(crate) fn new(ctx: &Ctx, field: Field) -> Self {
        Self::model(ctx, Model::Field(field, None))
    }

    pub(crate) fn model(ctx: &Ctx, model: Model) -> Self {
        Self {
            ctx: ctx.clone(),
            model,
        }
    }

    /// Whether the engine controls the value: the control previews its
    /// own changes until they arrive.
    pub(crate) fn controlled(&self) -> bool {
        matches!(self.model, Model::Controlled(..))
    }

    pub(crate) fn get(&self) -> Value {
        match &self.model {
            Model::Field(field, _) => self.ctx.read(*field),
            Model::Controlled(get, _) => get(&self.ctx),
        }
    }

    pub(crate) fn peek(&self) -> Value {
        match &self.model {
            Model::Field(field, _) => self.ctx.peek(*field),
            Model::Controlled(get, _) => untrack(|| get(&self.ctx)),
        }
    }

    pub(crate) fn set(&self, value: Value) {
        if self.peek() == value {
            return;
        }
        match &self.model {
            Model::Field(field, then) => {
                if self.ctx.write(*field, value)
                    && let Some(then) = then
                {
                    then(&self.ctx);
                }
            }
            Model::Controlled(_, commit) => commit(&self.ctx, value),
        }
    }
}

/// `shape: capsule` and `height` of a system control.
#[derive(Clone, Default)]
pub struct Shape {
    pub capsule: bool,
    pub height: Option<Num>,
}

/// Round a control's ends and give it its height: the theme's own
/// minimum height and vertical padding give way to the declared height.
pub(crate) fn shape(ctx: &Ctx, widget: &gtk::Widget, shape: &Shape) {
    if shape.capsule {
        widget.add_css_class("bern-capsule");
    }
    if let Some(height) = &shape.height {
        widget.add_css_class("bern-sized-control");
        let sized = widget.clone();
        bind_number(ctx, height, None, widget, move |value| {
            sized.set_size_request(sized.width_request(), value.round().max(0.0) as i32);
        });
        widget.set_valign(gtk::Align::Center);
    }
}

/// `padding`, `padding_x` and `padding_y` of a container.
#[derive(Clone, Default)]
pub struct Padding {
    pub all: Option<Num>,
    pub x: Option<Num>,
    pub y: Option<Num>,
}

/// Padding as the widget's margins: `all` on every side, then `x` across
/// and `y` along the vertical, which win where given.
pub(crate) fn pad(ctx: &Ctx, widget: &gtk::Widget, padding: &Padding, animate: Option<Curve>) {
    let side = |value: f64| value.round().max(0.0) as i32;
    if let Some(all) = &padding.all {
        let padded = widget.clone();
        let (x, y) = (padding.x.is_none(), padding.y.is_none());
        bind_number(ctx, all, animate, widget, move |value| {
            if x {
                padded.set_margin_start(side(value));
                padded.set_margin_end(side(value));
            }
            if y {
                padded.set_margin_top(side(value));
                padded.set_margin_bottom(side(value));
            }
        });
    }
    if let Some(across) = &padding.x {
        let padded = widget.clone();
        bind_number(ctx, across, animate, widget, move |value| {
            padded.set_margin_start(side(value));
            padded.set_margin_end(side(value));
        });
    }
    if let Some(down) = &padding.y {
        let padded = widget.clone();
        bind_number(ctx, down, animate, widget, move |value| {
            padded.set_margin_top(side(value));
            padded.set_margin_bottom(side(value));
        });
    }
}

/// A caption above a control outside forms.
pub(crate) fn stacked(
    label: Option<Rc<dyn Fn() -> String>>,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if let Some(label) = label {
        let caption = gtk::Label::new(None);
        caption.set_xalign(0.0);
        caption.add_css_class("caption-heading");
        bind_label(&caption, label);
        container.append(&caption);
    }
    container.append(control);
    container
}

/// Names `widget` for assistive technologies where no label shows.
fn accessible_name(widget: &impl IsA<gtk::Widget>, label: Option<Rc<dyn Fn() -> String>>) {
    if let Some(label) = label {
        let named = widget.clone().upcast::<gtk::Widget>();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
}

/// A label beside a control outside forms.
pub(crate) fn beside(
    label: Option<Rc<dyn Fn() -> String>>,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(label) = label {
        let caption = gtk::Label::new(None);
        caption.set_xalign(0.0);
        caption.set_hexpand(true);
        bind_label(&caption, label);
        container.append(&caption);
    }
    control.as_ref().set_valign(gtk::Align::Center);
    container.append(control);
    container
}

#[derive(Default)]
pub struct TextInputProps {
    pub id: &'static str,
    pub label: Option<Str>,
    /// `label_hidden`: the label names the field without showing.
    pub label_hidden: bool,
    /// The draft it edits (`value`), or the state it shows and the command
    /// each edit runs (`current`).
    pub model: Option<Model>,
    pub prompt: Option<Str>,
    pub kind: Option<&'static str>,
}

/// Text editing: changes update the draft; draft changes from elsewhere
/// update the text without echoing back.
pub(crate) fn editable(
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: Bound,
) -> Rc<Cell<bool>> {
    editable_unless(text, set, bound, None)
}

/// Like [`editable`]; while `editing` holds, state from elsewhere waits:
/// a field showing engine state keeps what the user types until the
/// engine has caught up with it.
fn editable_unless(
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: Bound,
    editing: Option<Rc<dyn Fn() -> bool>>,
) -> Rc<Cell<bool>> {
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    effect(move || {
        let value = as_text(&bound.get());
        if editing.as_ref().is_some_and(|editing| editing()) {
            return;
        }
        if untrack(|| text()) != value {
            guard.set(true);
            set(&value);
            guard.set(false);
        }
    });
    updating
}

/// For a field showing engine state: whether `widget` is being edited, and
/// when it stops, the engine's text again.
fn held_while_focused(
    widget: &impl IsA<gtk::Widget>,
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: &Bound,
    updating: Rc<Cell<bool>>,
) {
    let focus = gtk::EventControllerFocus::new();
    let bound = bound.clone();
    focus.connect_leave(move |_| {
        let value = as_text(&bound.peek());
        if text() != value {
            updating.set(true);
            set(&value);
            updating.set(false);
        }
    });
    widget.as_ref().add_controller(focus);
}

/// Whether the keyboard focus is in `widget`, as in the text of an entry.
fn contains_focus(widget: &gtk::Widget) -> bool {
    widget
        .root()
        .and_then(|root| root.focus())
        .is_some_and(|focus| focus == *widget || focus.is_ancestor(widget))
}

/// The text of a field bound to `bound`; a field showing engine state is
/// held while it is being edited.
fn text_binding(
    widget: &impl IsA<gtk::Widget>,
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: &Bound,
) -> Rc<Cell<bool>> {
    if !bound.controlled() {
        return editable(text, set, bound.clone());
    }
    let watched = widget.as_ref().downgrade();
    let editing: Rc<dyn Fn() -> bool> = Rc::new(move || {
        watched
            .upgrade()
            .is_some_and(|widget| contains_focus(&widget))
    });
    let updating = editable_unless(
        Rc::clone(&text),
        Rc::clone(&set),
        bound.clone(),
        Some(editing),
    );
    held_while_focused(widget, text, set, bound, Rc::clone(&updating));
    updating
}

#[allow(non_snake_case)]
pub fn TextInput(p: &Parent, ctx: &Ctx, props: TextInputProps, submit: Option<Act>) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    // The label shown above or in the field; a hidden one only names it.
    let shown = label.clone().filter(|_| !props.label_hidden);
    let prompt = props.prompt.as_ref().map(|prompt| bound_text(ctx, prompt));
    let submit = bound_action(ctx, &submit);
    let kind = props.kind.unwrap_or("text");
    let secure = kind == "secure";

    if kind == "multiline" {
        let view = gtk::TextView::new();
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_accepts_tab(false);
        view.set_top_margin(8);
        view.set_bottom_margin(8);
        view.set_left_margin(8);
        view.set_right_margin(8);
        view.set_size_request(-1, 72);
        let buffer = view.buffer();
        let read = buffer.clone();
        let write = buffer.clone();
        let updating = text_binding(
            &view,
            Rc::new(move || {
                read.text(&read.start_iter(), &read.end_iter(), false)
                    .to_string()
            }),
            Rc::new(move |value| write.set_text(value)),
            &bound,
        );
        let changed = bound.clone();
        buffer.connect_changed(move |buffer| {
            if !updating.get() {
                changed.set(Value::Text(
                    buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), false)
                        .to_string(),
                ));
            }
        });
        let frame = gtk::Frame::new(None);
        // GtkTextView has no placeholder: the prompt lies over the empty
        // view as an entry's does, dimmed, and lets the clicks through.
        match prompt {
            Some(prompt) => {
                let overlay = gtk::Overlay::new();
                overlay.set_child(Some(&view));
                let hint = gtk::Label::new(None);
                hint.add_css_class("dim-label");
                hint.set_halign(gtk::Align::Fill);
                hint.set_valign(gtk::Align::Start);
                hint.set_xalign(0.0);
                hint.set_wrap(true);
                hint.set_margin_start(8);
                hint.set_margin_end(8);
                hint.set_margin_top(8);
                hint.set_can_target(false);
                hint.set_visible(buffer.char_count() == 0);
                let text = hint.clone();
                effect(move || text.set_text(&prompt()));
                let shown = hint.clone();
                buffer.connect_changed(move |buffer| shown.set_visible(buffer.char_count() == 0));
                overlay.add_overlay(&hint);
                frame.set_child(Some(&overlay));
            }
            None => frame.set_child(Some(&view)),
        }
        accessible_name(&view, label.clone());
        if p.is_list() {
            p.add(&plain_row(&stacked(shown, &frame)));
        } else {
            p.add(&stacked(shown, &frame));
        }
    } else if p.is_list() {
        let row: adw::EntryRow = if secure {
            adw::PasswordEntryRow::new().upcast()
        } else {
            adw::EntryRow::new()
        };
        if let Some(title) = shown {
            let titled = row.clone();
            effect(move || titled.set_title(&title()));
        } else {
            accessible_name(&row, label.clone());
        }
        let read = row.clone();
        let write = row.clone();
        let updating = text_binding(
            &row,
            Rc::new(move || read.text().to_string()),
            Rc::new(move |value| write.set_text(value)),
            &bound,
        );
        let changed = bound.clone();
        row.connect_changed(move |row| {
            if !updating.get() {
                changed.set(Value::Text(row.text().to_string()));
            }
        });
        if let Some(submit) = submit.clone() {
            row.connect_entry_activated(move |_| submit());
        }
        p.add(&row);
    } else {
        let entry = gtk::Entry::new();
        entry.set_visibility(!secure);
        if secure {
            entry.set_input_purpose(gtk::InputPurpose::Password);
        }
        if let Some(prompt) = prompt.clone() {
            let shown = entry.clone();
            effect(move || shown.set_placeholder_text(Some(&prompt())));
        }
        let read = entry.clone();
        let write = entry.clone();
        let updating = text_binding(
            &entry,
            Rc::new(move || read.text().to_string()),
            Rc::new(move |value| write.set_text(value)),
            &bound,
        );
        let changed = bound.clone();
        entry.connect_changed(move |entry| {
            if !updating.get() {
                changed.set(Value::Text(entry.text().to_string()));
            }
        });
        if let Some(submit) = submit.clone() {
            entry.connect_activate(move |_| submit());
        }
        // A field takes the free width of its row, as a text field does in
        // a SwiftUI stack.
        entry.set_hexpand(true);
        if shown.is_none() {
            accessible_name(&entry, label.clone());
            p.add(&entry);
        } else {
            p.add(&stacked(shown, &entry));
        }
    }
    let set = bound.clone();
    let edit: Rc<dyn Fn(String)> = Rc::new(move |value| set.set(Value::Text(value)));
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "text_input",
        label,
        None,
        Actions {
            edit: Some(edit),
            submit,
            value: (!secure).then(|| {
                Rc::new(move || Some(as_text(&value.peek()))) as Rc<dyn Fn() -> Option<String>>
            }),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct SearchProps {
    pub id: &'static str,
    pub field: Option<Field>,
    pub prompt: Option<Str>,
    /// The prompt of a palette's own field.
    pub panel_prompt: Option<Str>,
    /// The field's size in the page, or a palette's.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// The size of the field's text in points.
    pub font_size: Option<f64>,
    /// The results show on focus, before anything is typed.
    pub suggest: bool,
    /// Show only the magnifying glass icon on a palette trigger.
    pub icon_only: bool,
}

/// A search field. Its children are results shown below it while it has
/// the focus and text, in a popover that keeps the focus in the field;
/// choosing a result closes them.
#[allow(non_snake_case)]
pub fn Search(
    p: &Parent,
    ctx: &Ctx,
    props: SearchProps,
    submit: Option<Act>,
    results: Option<Build>,
) {
    let Some(field) = props.field else { return };
    let bound = Bound::new(ctx, field);
    let prompt = props.prompt.as_ref().map(|prompt| bound_text(ctx, prompt));
    let entry = gtk::SearchEntry::new();
    if ctx.in_toolbar() {
        entry.set_hexpand(false);
        entry.set_width_chars(18);
        entry.set_valign(gtk::Align::Center);
    } else {
        // In a page: as wide as given, else as wide as its container.
        entry.set_hexpand(props.width.is_none());
        if props.width.is_some() || props.height.is_some() {
            entry.add_css_class("bern-sized-control");
            entry.set_size_request(
                props.width.map(|width| width.round() as i32).unwrap_or(-1),
                props
                    .height
                    .map(|height| height.round() as i32)
                    .unwrap_or(-1),
            );
        }
        if let Some(size) = props.font_size {
            entry.add_css_class(&crate::style::class_for(&format!("font-size: {size}px;")));
        }
    }
    if let Some(prompt) = prompt.clone() {
        let shown = entry.clone();
        effect(move || {
            let text = prompt();
            shown.set_placeholder_text(Some(&text));
            shown.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let read = entry.clone();
    let write = entry.clone();
    let updating = editable(
        Rc::new(move || read.text().to_string()),
        Rc::new(move |value| write.set_text(value)),
        bound.clone(),
    );
    let changed = bound.clone();
    entry.connect_changed(move |entry| {
        if !updating.get() {
            changed.set(Value::Text(entry.text().to_string()));
        }
    });
    let submit = bound_action(ctx, &submit);
    if let Some(submit) = submit.clone() {
        entry.connect_activate(move |_| submit());
    }
    let cleared = bound.clone();
    entry.connect_stop_search(move |_| cleared.set(Value::Text(String::new())));
    if let Some(results) = results {
        search_results(ctx, &entry, &bound, results, props.suggest);
    }
    place(p, &entry);
    let set = bound.clone();
    let value = bound.clone();
    let focused = entry.clone();
    controls::register(
        ctx,
        props.id,
        "search",
        prompt.or_else(|| Some(Rc::new(String::new))),
        None,
        Actions {
            edit: Some(Rc::new(move |text| set.set(Value::Text(text)))),
            submit,
            value: Some(Rc::new(move || Some(as_text(&value.peek())))),
            focus: Some(Rc::new(move || {
                focused.grab_focus();
            })),
            ..Actions::default()
        },
    );
}

fn search_results(
    ctx: &Ctx,
    entry: &gtk::SearchEntry,
    bound: &Bound,
    results: Build,
    suggest: bool,
) {
    let popover = gtk::Popover::new();
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.set_can_focus(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    column.set_margin_top(6);
    column.set_margin_bottom(6);
    column.set_margin_start(6);
    column.set_margin_end(6);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_max_content_height(360);
    scrolled.set_min_content_width(280);
    scrolled.set_child(Some(&column));
    popover.set_child(Some(&scrolled));
    popover.set_parent(entry);
    results(&Parent::of_box(&column), &ctx.outside_toolbar());
    let focused = crate::reactive::Cell::new(false);
    let focus = gtk::EventControllerFocus::new();
    {
        let focused = focused.clone();
        focus.connect_enter(move |_| focused.set(true));
    }
    {
        let focused = focused.clone();
        let popover = popover.clone();
        focus.connect_leave(move |_| {
            let focused = focused.clone();
            let popover = popover.clone();
            // Focus moving into the results keeps them.
            glib::idle_add_local_once(move || {
                let inside = popover
                    .root()
                    .and_then(|root| gtk::prelude::RootExt::focus(&root))
                    .is_some_and(|focus| focus.is_ancestor(&popover));
                if !inside {
                    focused.set(false);
                }
            });
        });
    }
    entry.add_controller(focus);
    // Choosing a result closes them, until the field takes the focus again.
    // The raw release is seen before the result's own click, which then
    // still runs.
    let release = gtk::EventControllerLegacy::new();
    release.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let focused = focused.clone();
        release.connect_event(move |_, event| {
            if matches!(
                event.event_type(),
                gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::TouchEnd
            ) {
                let focused = focused.clone();
                glib::idle_add_local_once(move || focused.set(false));
            }
            glib::Propagation::Proceed
        });
    }
    column.add_controller(release);
    // Typing again in the field shows them again.
    {
        let focused = focused.clone();
        entry.connect_search_changed(move |entry| {
            let inside = entry
                .root()
                .and_then(|root| gtk::prelude::RootExt::focus(&root))
                .is_some_and(|focus| {
                    &focus == entry.upcast_ref::<gtk::Widget>() || focus.is_ancestor(entry)
                });
            if inside {
                focused.set(true);
            }
        });
    }
    let read = bound.clone();
    let shown = popover.clone();
    effect(move || {
        let open = focused.get() && (suggest || !as_text(&read.get()).is_empty());
        if open && !shown.is_visible() {
            shown.popup();
        } else if !open && shown.is_visible() {
            shown.popdown();
        }
    });
    crate::reactive::on_cleanup(move || {
        popover.popdown();
        popover.unparent();
    });
}

#[derive(Default)]
pub struct SwitchProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub style: Option<&'static str>,
    pub icon: Option<&'static str>,
    /// The label names the switch for assistive technology only.
    pub label_hidden: bool,
    pub shape: Shape,
    /// `style! { Switch... { foreground: ... } }`: knob color when on.
    pub look: Option<&'static crate::style::StyleDef>,
}

#[allow(non_snake_case)]
pub fn Switch(p: &Parent, ctx: &Ctx, props: SwitchProps) {
    let Some(model) = props.model else { return };
    if props.style == Some("button") || ctx.in_toolbar() {
        crate::features::toggle_button(
            p,
            ctx,
            crate::features::ToggleProps {
                id: props.id,
                label: props.label,
                model,
                icon: props.icon,
                shape: props.shape,
            },
        );
        return;
    }
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let shown_label = label.clone().filter(|_| !props.label_hidden);
    let checkbox = props.style == Some("checkbox");
    // Every presentation exposes one boolean property, `active`.
    let control: gtk::Widget;
    let active: glib::Object;
    if p.is_list() && !checkbox {
        let row = adw::SwitchRow::new();
        if let Some(look) = props.look {
            row.add_css_class(&format!("bern-switch-{}", look.name));
        }
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&label()));
        }
        active = row.clone().upcast();
        control = row.upcast();
    } else if checkbox {
        let check = gtk::CheckButton::new();
        if p.is_list() {
            let row = adw::ActionRow::new();
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            if let Some(label) = label.clone() {
                let shown = row.clone();
                effect(move || shown.set_title(&glib::markup_escape_text(&label())));
            }
            control = row.upcast();
        } else {
            if let Some(label) = shown_label.clone() {
                let shown = check.clone();
                effect(move || shown.set_label(Some(&label())));
            }
            control = check.clone().upcast();
        }
        active = check.upcast();
    } else {
        let switch = gtk::Switch::new();
        if let Some(look) = props.look {
            switch.add_css_class(&format!("bern-switch-{}", look.name));
        }
        shape(ctx, switch.upcast_ref(), &props.shape);
        control = beside(shown_label.clone(), &switch).upcast();
        active = switch.upcast();
    }
    if props.label_hidden
        && let Some(label) = label.clone()
        && let Ok(widget) = active.clone().downcast::<gtk::Widget>()
    {
        effect(move || widget.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = active.clone();
    let read = bound.clone();
    effect(move || {
        let value = read.get() == Value::Bool(true);
        if shown.property::<bool>("active") != value {
            guard.set(true);
            shown.set_property("active", value);
            guard.set(false);
        }
    });
    let changed = bound.clone();
    active.connect_notify_local(Some("active"), move |object, _| {
        if !updating.get() {
            changed.set(Value::Bool(object.property::<bool>("active")));
        }
    });
    p.add(&control);
    let set = bound.clone();
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "switch",
        label,
        None,
        Actions {
            set_bool: Some(Rc::new(move |on| set.set(Value::Bool(on)))),
            value: Some(Rc::new(move || {
                Some((value.peek() == Value::Bool(true)).to_string())
            })),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct ColorPickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub label_hidden: bool,
}

#[allow(non_snake_case)]
pub fn ColorPicker(p: &Parent, ctx: &Ctx, props: ColorPickerProps, commit: Option<Act>) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let dialog = gtk::ColorDialog::new();
    dialog.set_with_alpha(false);
    let button = gtk::ColorDialogButton::new(Some(dialog));
    let shown_label = (!props.label_hidden).then(|| label.clone()).flatten();
    let control = beside(shown_label, &button);
    if props.label_hidden
        && let Some(label) = label.clone()
    {
        let shown = button.clone();
        effect(move || {
            shown.update_property(&[gtk::accessible::Property::Label(&label())]);
        });
    }
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = button.clone();
    let read = bound.clone();
    let as_rgb = |value: &Value| -> u32 {
        match value {
            Value::Int(n) => (*n).clamp(0, 0xff_ffff) as u32,
            _ => (as_f64(value).round() as i64).clamp(0, 0xff_ffff) as u32,
        }
    };
    effect(move || {
        let rgb = as_rgb(&read.get());
        let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
        let b = (rgb & 0xff) as f32 / 255.0;
        let target = gtk::gdk::RGBA::new(r, g, b, 1.0);
        if shown.rgba() != target {
            guard.set(true);
            shown.set_rgba(&target);
            guard.set(false);
        }
    });
    let commit = bound_action(ctx, &commit);
    let changed = bound.clone();
    let changed_commit = commit.clone();
    let guard = Rc::clone(&updating);
    fn as_value(peeked: &Value, rgb: u32) -> Value {
        if matches!(peeked, Value::Int(_)) {
            Value::Int(i64::from(rgb))
        } else {
            Value::Float(f64::from(rgb))
        }
    }
    button.connect_rgba_notify(move |b| {
        if guard.get() {
            return;
        }
        let rgba = b.rgba();
        let byte = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u32;
        let rgb = (byte(rgba.red()) << 16) | (byte(rgba.green()) << 8) | byte(rgba.blue());
        changed.set(as_value(&changed.peek(), rgb));
        if let Some(commit) = &changed_commit {
            commit();
        }
    });
    p.add(&control);
    let set = bound.clone();
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "color_picker",
        label,
        None,
        Actions {
            set_number: Some(Rc::new(move |number| {
                if number.is_finite() {
                    let clamped = (number.round() as i64).clamp(0, 0xff_ffff) as u32;
                    set.set(as_value(&set.peek(), clamped));
                    if let Some(commit) = &commit {
                        commit();
                    }
                }
            })),
            value: Some(Rc::new(move || Some(as_rgb(&value.peek()).to_string()))),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct SliderProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub min_icon: Option<&'static str>,
    pub max_icon: Option<&'static str>,
    /// The label names the slider for assistive technology only.
    pub label_hidden: bool,
    pub continuous: bool,
    /// `axis: vertical`: min at the bottom, max at the top.
    pub vertical: bool,
    /// `hover`: where the pointer is along the track while it is over the
    /// slider, from 0 at `min` to 1 at `max`, and `None` otherwise.
    pub hover: Option<crate::ctx::Field>,
}

/// A slider. Bound to a draft, dragging only updates this window's draft
/// as a preview and releasing commits once; keyboard and scroll changes
/// commit at once. Bound to a value the engine controls, dragging moves
/// only the slider, and releasing sets the value.
#[allow(non_snake_case)]
pub fn Slider(p: &Parent, ctx: &Ctx, props: SliderProps, commit: Option<Act>) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let controlled = bound.controlled();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let (min, max) = (props.min.unwrap_or(0.0), props.max.unwrap_or(1.0));
    let increment = props.step.unwrap_or((max - min) / 100.0).max(f64::EPSILON);
    let orientation = if props.vertical {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    };
    let scale = gtk::Scale::with_range(orientation, min, max, increment);
    scale.set_draw_value(false);
    if props.vertical {
        scale.set_inverted(true);
        scale.set_vexpand(true);
        scale.set_hexpand(false);
    } else {
        scale.set_hexpand(true);
    }
    if let Some(step) = props.step {
        scale.set_round_digits(if step.fract() == 0.0 { 0 } else { -1 });
    }
    let commit = bound_action(ctx, &commit);
    let snap = move |value: f64| match props.step {
        Some(step) if step > 0.0 => (min + ((value - min) / step).round() * step).clamp(min, max),
        _ => value.clamp(min, max),
    };
    let updating = Rc::new(Cell::new(false));
    let dragging = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = scale.clone();
    let read = bound.clone();
    let held = Rc::clone(&dragging);
    effect(move || {
        let value = as_f64(&read.get());
        // A drag in progress keeps the slider where the pointer has it.
        if held.get() && controlled {
            return;
        }
        if (shown.value() - value).abs() > f64::EPSILON {
            guard.set(true);
            shown.set_value(value);
            guard.set(false);
        }
    });
    let pressed = Rc::clone(&dragging);
    let released_commit = commit.clone();
    let released = bound.clone();
    let released_scale = scale.clone();
    let legacy = gtk::EventControllerLegacy::new();
    legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
    legacy.connect_event(move |_, event| {
        match event.event_type() {
            gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => pressed.set(true),
            gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::TouchEnd
                if pressed.replace(false) =>
            {
                let commit = released_commit.clone();
                let bound = released.clone();
                let scale = released_scale.clone();
                // After the scale handled the release.
                glib::idle_add_local_once(move || {
                    if controlled {
                        bound.set(Value::Float(snap(scale.value())));
                    }
                    if let Some(commit) = commit {
                        commit();
                    }
                });
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    scale.add_controller(legacy);
    let changed = bound.clone();
    let keyed_commit = commit.clone();
    scale.connect_value_changed(move |scale| {
        if updating.get() {
            return;
        }
        let value = snap(scale.value());
        if dragging.get() {
            // A preview: the draft follows, a controlled value waits.
            if !controlled || props.continuous {
                changed.set(Value::Float(value));
            }
            return;
        }
        changed.set(Value::Float(value));
        if let Some(commit) = &keyed_commit {
            commit();
        }
    });
    if let Some(field) = props.hover {
        hover_track(ctx, &scale, field, props.vertical);
    }
    let track = if props.vertical {
        let track = gtk::Box::new(gtk::Orientation::Vertical, 6);
        track.set_vexpand(true);
        track.set_halign(gtk::Align::Center);
        if let Some(icon) = props.max_icon {
            let image = icons::image(icon);
            image.add_css_class("dim-label");
            track.append(&image);
        }
        track.append(&scale);
        if let Some(icon) = props.min_icon {
            let image = icons::image(icon);
            image.add_css_class("dim-label");
            track.append(&image);
        }
        track
    } else {
        let track = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        if let Some(icon) = props.min_icon {
            let image = icons::image(icon);
            image.add_css_class("dim-label");
            track.append(&image);
        }
        track.append(&scale);
        if let Some(icon) = props.max_icon {
            let image = icons::image(icon);
            image.add_css_class("dim-label");
            track.append(&image);
        }
        track
    };
    if let Some(label) = label.clone() {
        let shown = scale.clone();
        effect(move || {
            shown.update_property(&[gtk::accessible::Property::Label(&label())]);
        });
    }
    let content = stacked(label.clone().filter(|_| !props.label_hidden), &track);
    if p.is_list() {
        p.add(&plain_row(&content));
    } else {
        p.add(&content);
    }
    let set = bound.clone();
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "slider",
        label,
        None,
        Actions {
            set_number: Some(Rc::new(move |number| {
                set.set(Value::Float(snap(number)));
                if let Some(commit) = &commit {
                    commit();
                }
            })),
            value: Some(Rc::new(move || Some(number(as_f64(&value.peek()))))),
            ..Actions::default()
        },
    );
}

/// Keep `field` at where the pointer is along the scale's track while it
/// is over the scale, and `None` once it leaves. It is this window's draft
/// only: the value, its preview and the commit are the slider's own.
fn hover_track(ctx: &Ctx, scale: &gtk::Scale, field: crate::ctx::Field, vertical: bool) {
    let bound = Bound::new(ctx, field);
    let weak = scale.downgrade();
    type HoverPoint = Rc<dyn Fn(Option<(f64, f64)>)>;
    let point: HoverPoint = Rc::new(move |pos| {
        let fraction = pos.and_then(|(x, y)| {
            let scale = weak.upgrade()?;
            let trough = scale.range_rect();
            let (start, end) = scale.slider_range();
            let (coord, origin, span, flipped) = if vertical {
                (
                    y,
                    f64::from(trough.y()),
                    f64::from(trough.height()),
                    scale.is_inverted(),
                )
            } else {
                (
                    x,
                    f64::from(trough.x()),
                    f64::from(trough.width()),
                    scale.is_inverted() != (scale.direction() == gtk::TextDirection::Rtl),
                )
            };
            Some(crate::gesture_state::track_fraction(
                coord,
                origin,
                span,
                f64::from((end - start).max(0)),
                flipped,
            ))
        });
        bound.set(match fraction {
            Some(fraction) => Value::some(Value::Float(fraction)),
            None => Value::none(),
        });
    });
    let motion = gtk::EventControllerMotion::new();
    {
        let point = Rc::clone(&point);
        motion.connect_enter(move |_, x, y| point(Some((x, y))));
    }
    {
        let point = Rc::clone(&point);
        motion.connect_motion(move |_, x, y| point(Some((x, y))));
    }
    {
        let point = Rc::clone(&point);
        motion.connect_leave(move |_| point(None));
    }
    scale.add_controller(motion);
    scale.connect_unmap(move |_| point(None));
}

/// One option of a picker: its value, the name of its variant and its
/// label.
pub struct PickOption {
    pub value: Value,
    pub name: &'static str,
    pub label: Str,
}

pub fn option(value: Value, name: &'static str, label: Str) -> PickOption {
    PickOption { value, name, label }
}

#[derive(Default)]
pub struct PickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub options: Vec<PickOption>,
    pub style: Option<&'static str>,
    /// The label names the picker for assistive technology only.
    pub label_hidden: bool,
    pub shape: Shape,
}

/// A list of strings that follows the options' labels.
fn label_model(ctx: &Ctx, labels: Vec<Str>, apply: impl Fn(&[String]) + 'static) {
    let ctx = ctx.clone();
    effect(move || {
        let texts: Vec<String> = labels.iter().map(|label| label(&ctx)).collect();
        apply(&texts);
    });
}

#[allow(non_snake_case)]
pub fn Picker(p: &Parent, ctx: &Ctx, props: PickerProps) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let values: Rc<Vec<Value>> = Rc::new(
        props
            .options
            .iter()
            .map(|option| option.value.clone())
            .collect(),
    );
    let names: Rc<Vec<&'static str>> =
        Rc::new(props.options.iter().map(|option| option.name).collect());
    let labels: Vec<Str> = props
        .options
        .iter()
        .map(|option| Rc::clone(&option.label))
        .collect();
    let index_of = {
        let values = Rc::clone(&values);
        move |value: &Value| values.iter().position(|candidate| candidate == value)
    };
    let select_index: Rc<dyn Fn(usize)> = {
        let values = Rc::clone(&values);
        let bound = bound.clone();
        Rc::new(move |index| {
            if let Some(value) = values.get(index) {
                bound.set(value.clone());
            }
        })
    };
    let segmented = matches!(
        props.style,
        Some("segmented") | Some("radio") | Some("inline")
    );
    // Chips without a label of their own in a flow are items of the flow;
    // the picker then has no widget of its own.
    let control: Option<gtk::Widget> = if props.style == Some("chips") {
        // A toggle button per option, one of a group, the chosen one
        // active; elsewhere than in a flow they wrap in a flow of their own.
        let flat = p.is_flow() && props.label_hidden && !ctx.in_toolbar();
        let holder = (!flat).then(|| crate::scene::flow_of(8.0));
        let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
        for index in 0..props.options.len() {
            let button = gtk::ToggleButton::new();
            button.set_valign(gtk::Align::Center);
            if props.shape.capsule {
                button.add_css_class("bern-capsule");
            }
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            let select = Rc::clone(&select_index);
            button.connect_toggled(move |button| {
                if button.is_active() {
                    select(index);
                }
            });
            match &holder {
                Some(flow) => button.set_parent(flow),
                None => p.add(&button),
            }
            buttons.push(button);
        }
        let shown = buttons.clone();
        label_model(ctx, labels, move |texts| {
            for (button, text) in shown.iter().zip(texts) {
                button.set_label(text);
            }
        });
        let read = bound.clone();
        let index_of = index_of.clone();
        effect(move || {
            if let Some(index) = index_of(&read.get())
                && let Some(button) = buttons.get(index)
                && !button.is_active()
            {
                button.set_active(true);
            }
        });
        holder.map(|flow| {
            if let Some(label) = label.clone()
                && !props.label_hidden
            {
                let shown = flow.clone();
                effect(move || {
                    shown.update_property(&[gtk::accessible::Property::Label(&label())])
                });
            }
            flow.upcast()
        })
    } else if segmented {
        let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        group.add_css_class("linked");
        group.set_valign(gtk::Align::Center);
        shape(ctx, group.upcast_ref(), &props.shape);
        let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
        for (index, _) in props.options.iter().enumerate() {
            let button = gtk::ToggleButton::new();
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            let select = Rc::clone(&select_index);
            button.connect_toggled(move |button| {
                if button.is_active() {
                    select(index);
                }
            });
            group.append(&button);
            buttons.push(button);
        }
        let shown = buttons.clone();
        label_model(ctx, labels, move |texts| {
            for (button, text) in shown.iter().zip(texts) {
                button.set_label(text);
            }
        });
        let read = bound.clone();
        let index_of = index_of.clone();
        effect(move || {
            if let Some(index) = index_of(&read.get())
                && let Some(button) = buttons.get(index)
                && !button.is_active()
            {
                button.set_active(true);
            }
        });
        if let Some(label) = label.clone() {
            let shown = group.clone();
            effect(move || shown.update_property(&[gtk::accessible::Property::Label(&label())]));
        }
        Some(group.upcast())
    } else {
        let model = gtk::StringList::new(&[]);
        let updating = Rc::new(Cell::new(false));
        let searchable = props.style == Some("combobox");
        let (selected_owner, widget): (glib::Object, gtk::Widget) =
            if p.is_list() && !ctx.in_toolbar() {
                let row = adw::ComboRow::new();
                row.set_model(Some(&model));
                if searchable {
                    row.set_enable_search(true);
                    row.set_expression(Some(&gtk::PropertyExpression::new(
                        gtk::StringObject::static_type(),
                        None::<gtk::Expression>,
                        "string",
                    )));
                }
                if let Some(label) = label.clone() {
                    let shown = row.clone();
                    effect(move || shown.set_title(&label()));
                }
                (row.clone().upcast(), row.upcast())
            } else {
                let expr = searchable.then(|| {
                    gtk::PropertyExpression::new(
                        gtk::StringObject::static_type(),
                        None::<gtk::Expression>,
                        "string",
                    )
                    .upcast()
                });
                let dropdown = gtk::DropDown::new(Some(model.clone()), expr);
                if searchable {
                    dropdown.set_enable_search(true);
                }
                dropdown.set_valign(gtk::Align::Center);
                shape(ctx, dropdown.upcast_ref(), &props.shape);
                (dropdown.clone().upcast(), dropdown.upcast())
            };
        let reselect = {
            let owner = selected_owner.clone();
            let read = bound.clone();
            let index_of = index_of.clone();
            let guard = Rc::clone(&updating);
            move || {
                if let Some(index) = index_of(&read.peek())
                    && owner.property::<u32>("selected") != index as u32
                {
                    guard.set(true);
                    owner.set_property("selected", index as u32);
                    guard.set(false);
                }
            }
        };
        let relabel = reselect.clone();
        let guard = Rc::clone(&updating);
        label_model(ctx, labels, move |texts| {
            guard.set(true);
            let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
            model.splice(0, model.n_items(), &texts);
            guard.set(false);
            relabel();
        });
        let read = bound.clone();
        effect(move || {
            read.get();
            reselect();
        });
        let select = Rc::clone(&select_index);
        selected_owner.connect_notify_local(Some("selected"), move |owner, _| {
            if !updating.get() {
                select(owner.property::<u32>("selected") as usize);
            }
        });
        Some(widget)
    };
    if let Some(control) = control {
        if props.label_hidden
            && let Some(label) = label.clone()
        {
            let named = control.clone();
            effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
        }
        if ctx.in_toolbar() || props.label_hidden {
            if p.is_list() {
                p.add(&plain_row(&control));
            } else {
                keeps_width(&control);
                p.add(&control);
            }
        } else if (segmented || props.style == Some("chips")) && p.is_list() {
            let row = adw::ActionRow::new();
            if let Some(label) = label.clone() {
                let shown = row.clone();
                effect(move || shown.set_title(&glib::markup_escape_text(&label())));
            }
            row.add_suffix(&control);
            p.add(&row);
        } else if p.is_list() {
            p.add(&control);
        } else {
            p.add(&beside(label.clone(), &control));
        }
    }
    let select = {
        let names = Rc::clone(&names);
        let select_index = Rc::clone(&select_index);
        Rc::new(
            move |name: &str| match names.iter().position(|candidate| *candidate == name) {
                Some(index) => {
                    select_index(index);
                    true
                }
                None => false,
            },
        )
    };
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "picker",
        label,
        None,
        Actions {
            select: Some(select),
            value: Some(Rc::new(move || {
                index_of(&value.peek()).map(|index| names[index].to_string())
            })),
            ..Actions::default()
        },
    );
}

pub struct DynamicPickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub items: Get,
    pub key_field: usize,
    pub label_field: usize,
    pub label_enum: Option<&'static str>,
    pub prompt: Option<Str>,
    pub style: Option<&'static str>,
    pub label_hidden: bool,
    pub shape: Shape,
}

#[allow(non_snake_case)]
pub fn DynamicPicker(p: &Parent, ctx: &Ctx, props: DynamicPickerProps) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let items_get = props.items;
    let key_field = props.key_field;
    let label_field = props.label_field;
    let label_enum = props.label_enum;
    let at_items = ctx.clone();
    let read_items: Rc<dyn Fn() -> Vec<(Value, String, String)>> = Rc::new(move || {
        crate::value::as_list(items_get(&at_items))
            .into_iter()
            .map(|row| {
                let k = crate::value::at(row.clone(), &[key_field]);
                let lv = crate::value::at(row, &[label_field]);
                let kt = crate::value::key_text(&k);
                let lt = display(&lv, label_enum);
                (k, kt, lt)
            })
            .collect()
    });
    let snapshot: Rc<std::cell::RefCell<Vec<(Value, String, String)>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));
    let choose_val: Rc<dyn Fn(Value)> = {
        let bound = bound.clone();
        Rc::new(move |val| {
            if bound.peek() != val {
                bound.set(val);
            }
        })
    };
    let segmented = matches!(
        props.style,
        Some("segmented") | Some("radio") | Some("inline")
    );
    let searchable = props.style == Some("combobox");
    let control: gtk::Widget = if props.style == Some("chips") || segmented {
        let holder = if props.style == Some("chips") {
            let flow = crate::scene::flow_of(8.0);
            flow.upcast::<gtk::Widget>()
        } else {
            let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            group.add_css_class("linked");
            group.set_valign(gtk::Align::Center);
            shape(ctx, group.upcast_ref(), &props.shape);
            group.upcast::<gtk::Widget>()
        };
        let holder_ref = holder.clone();
        let read_items = Rc::clone(&read_items);
        let snapshot = Rc::clone(&snapshot);
        let read_val = bound.clone();
        let choose = Rc::clone(&choose_val);
        let is_chips = props.style == Some("chips");
        let capsule_chips = is_chips && props.shape.capsule;
        effect(move || {
            let cur_items = read_items();
            let cur_val = read_val.get();
            *snapshot.borrow_mut() = cur_items.clone();
            while let Some(child) = holder_ref.first_child() {
                child.unparent();
            }
            let mut first_btn: Option<gtk::ToggleButton> = None;
            for (key, _, lbl) in &cur_items {
                let btn = gtk::ToggleButton::with_label(lbl);
                btn.set_valign(gtk::Align::Center);
                if capsule_chips {
                    btn.add_css_class("bern-capsule");
                }
                if let Some(first) = &first_btn {
                    btn.set_group(Some(first));
                } else {
                    first_btn = Some(btn.clone());
                }
                if *key == cur_val {
                    btn.set_active(true);
                }
                let k = key.clone();
                let choose = Rc::clone(&choose);
                btn.connect_toggled(move |b| {
                    if b.is_active() {
                        choose(k.clone());
                    }
                });
                if is_chips {
                    btn.set_parent(&holder_ref);
                } else if let Ok(bx) = holder_ref.clone().downcast::<gtk::Box>() {
                    bx.append(&btn);
                }
            }
        });
        holder
    } else {
        let list_model = gtk::StringList::new(&[]);
        let updating = Rc::new(Cell::new(false));
        let (selected_owner, widget): (glib::Object, gtk::Widget) =
            if p.is_list() && !ctx.in_toolbar() {
                let row = adw::ComboRow::new();
                row.set_model(Some(&list_model));
                if searchable {
                    row.set_enable_search(true);
                    row.set_expression(Some(&gtk::PropertyExpression::new(
                        gtk::StringObject::static_type(),
                        None::<gtk::Expression>,
                        "string",
                    )));
                }
                if let Some(label) = label.clone() {
                    let shown = row.clone();
                    effect(move || shown.set_title(&label()));
                }
                (row.clone().upcast(), row.upcast())
            } else {
                let expr = searchable.then(|| {
                    gtk::PropertyExpression::new(
                        gtk::StringObject::static_type(),
                        None::<gtk::Expression>,
                        "string",
                    )
                    .upcast()
                });
                let dropdown = gtk::DropDown::new(Some(list_model.clone()), expr);
                if searchable {
                    dropdown.set_enable_search(true);
                }
                dropdown.set_valign(gtk::Align::Center);
                shape(ctx, dropdown.upcast_ref(), &props.shape);
                (dropdown.clone().upcast(), dropdown.upcast())
            };
        let owner_sync = selected_owner.clone();
        let snap_sync = Rc::clone(&snapshot);
        let read_items_sync = Rc::clone(&read_items);
        let read_val = bound.clone();
        let guard = Rc::clone(&updating);
        effect(move || {
            let cur_items = read_items_sync();
            let cur_val = read_val.get();
            let labels: Vec<&str> = cur_items.iter().map(|(_, _, l)| l.as_str()).collect();
            *snap_sync.borrow_mut() = cur_items.clone();
            guard.set(true);
            list_model.splice(0, list_model.n_items(), &labels);
            if let Some(idx) = cur_items.iter().position(|(k, _, _)| *k == cur_val) {
                owner_sync.set_property("selected", idx as u32);
            } else {
                owner_sync.set_property("selected", gtk::INVALID_LIST_POSITION);
            }
            guard.set(false);
        });
        let snap_sel = Rc::clone(&snapshot);
        let choose_sel = Rc::clone(&choose_val);
        let guard_sel = Rc::clone(&updating);
        selected_owner.connect_notify_local(Some("selected"), move |owner, _| {
            if !guard_sel.get() {
                let idx = owner.property::<u32>("selected") as usize;
                if let Some((key, _, _)) = snap_sel.borrow().get(idx) {
                    choose_sel(key.clone());
                }
            }
        });
        widget
    };
    if props.label_hidden
        && let Some(label) = label.clone()
    {
        let named = control.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
    if ctx.in_toolbar() || props.label_hidden {
        if p.is_list() {
            p.add(&plain_row(&control));
        } else {
            keeps_width(&control);
            p.add(&control);
        }
    } else if (segmented || props.style == Some("chips")) && p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&label())));
        }
        row.add_suffix(&control);
        p.add(&row);
    } else if p.is_list() {
        p.add(&control);
    } else {
        p.add(&beside(label.clone(), &control));
    }
    let select = {
        let read_items = Rc::clone(&read_items);
        let choose = Rc::clone(&choose_val);
        Rc::new(move |name: &str| {
            let cur = untrack(|| read_items());
            if let Some((key, _, _)) = cur.into_iter().find(|(_, kt, _)| kt == name) {
                choose(key);
                true
            } else {
                false
            }
        })
    };
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "picker",
        label,
        None,
        Actions {
            select: Some(select),
            value: Some(Rc::new(move || Some(crate::value::key_text(&value.peek())))),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct TooltipProps {
    pub content: Option<Str>,
    pub keys: Option<&'static str>,
    pub bound_keys: Option<Str>,
    pub placement: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn Tooltip(p: &Parent, ctx: &Ctx, props: TooltipProps, build: Build) {
    let region = p.region();
    build(&region, ctx);
    let _ = props.placement;
    let text = props.content.as_ref().map(|c| bound_text(ctx, c));
    let keys_fn: Option<Rc<dyn Fn() -> String>> = match (&props.bound_keys, props.keys) {
        (Some(bound), _) => Some(bound_text(ctx, bound)),
        (None, Some(lit)) => Some(Rc::new(move || lit.to_string())),
        (None, None) => None,
    };
    if let Some(keys_fn) = keys_fn {
        for widget in region.widgets() {
            widget.set_has_tooltip(true);
            let t = text.clone();
            let k = Rc::clone(&keys_fn);
            widget.connect_query_tooltip(move |_, _, _, _, tooltip| {
                let body = crate::reactive::untrack(|| t.as_ref().map(|f| f()).unwrap_or_default());
                let raw_keys = crate::reactive::untrack(|| k());
                let formatted = crate::windows::shortcut_text(&raw_keys);
                if body.is_empty() && formatted.is_empty() {
                    return false;
                }
                if formatted.is_empty() {
                    tooltip.set_text(Some(&body));
                    return true;
                }
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                row.set_valign(gtk::Align::Center);
                if !body.is_empty() {
                    let lbl = gtk::Label::new(Some(&body));
                    row.append(&lbl);
                }
                let caps = gtk::Box::new(gtk::Orientation::Horizontal, 3);
                for part in formatted.split('+') {
                    let cap = gtk::Label::new(Some(part));
                    cap.add_css_class("bern-keycap");
                    caps.append(&cap);
                }
                row.append(&caps);
                tooltip.set_custom(Some(&row));
                true
            });
        }
        return;
    }
    let Some(text) = text else { return };
    for widget in region.widgets() {
        let w = widget.clone();
        let t = Rc::clone(&text);
        effect(move || w.set_tooltip_text(Some(&t())));
    }
}

#[derive(Default)]
pub struct BannerProps {
    pub id: Option<&'static str>,
    pub severity: Option<&'static str>,
    pub title: Option<Str>,
    pub content: Option<Str>,
    pub icon: Option<&'static str>,
    pub dismiss_label: Option<Str>,
}

#[allow(non_snake_case)]
pub fn Banner(
    p: &Parent,
    ctx: &Ctx,
    props: BannerProps,
    dismiss: Option<Act>,
    actions: Option<Build>,
) {
    let severity = props.severity.unwrap_or("info");
    let default_icon = match severity {
        "success" => "checkmark.circle",
        "warning" => "exclamationmark.triangle",
        "error" => "xmark.circle.fill",
        _ => "info.circle",
    };
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("bern-banner");
    row.add_css_class(&format!("bern-banner-{severity}"));
    row.set_hexpand(true);
    row.set_valign(gtk::Align::Center);
    let icon = icons::image(props.icon.unwrap_or(default_icon));
    icon.set_valign(gtk::Align::Center);
    row.append(&icon);
    let text_col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_col.set_hexpand(true);
    if let Some(title) = props.title.as_ref().map(|t| bound_text(ctx, t)) {
        let lbl = gtk::Label::new(None);
        lbl.add_css_class("heading");
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        let l = lbl.clone();
        effect(move || l.set_text(&title()));
        text_col.append(&lbl);
    }
    let content_text = props.content.as_ref().map(|t| bound_text(ctx, t));
    if let Some(content) = &content_text {
        let lbl = gtk::Label::new(None);
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        let l = lbl.clone();
        let c = Rc::clone(content);
        effect(move || l.set_text(&c()));
        text_col.append(&lbl);
    }
    row.append(&text_col);
    if let Some(build_actions) = actions {
        let act_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        act_box.set_valign(gtk::Align::Center);
        build_actions(&Parent::of_box(&act_box), ctx);
        row.append(&act_box);
    }
    if let Some(on_dismiss) = bound_action(ctx, &dismiss) {
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.add_css_class("circular");
        close.set_valign(gtk::Align::Center);
        let close_label = props
            .dismiss_label
            .as_ref()
            .map(|l| bound_text(ctx, l))
            .or_else(|| content_text.clone());
        if let Some(lbl) = &close_label {
            let btn = close.clone();
            let l = Rc::clone(lbl);
            effect(move || btn.set_tooltip_text(Some(&l())));
        }
        let run = Rc::clone(&on_dismiss);
        close.connect_clicked(move |_| run());
        row.append(&close);
        if let Some(id) = props.id {
            controls::register(
                ctx,
                id,
                "button",
                close_label,
                None,
                Actions {
                    activate: Some(on_dismiss),
                    ..Actions::default()
                },
            );
        }
    }
    p.add(&row);
}

pub struct DisclosureProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub model: Option<Model>,
    pub then: Option<Act>,
}

#[allow(non_snake_case)]
pub fn Disclosure(p: &Parent, ctx: &Ctx, props: DisclosureProps, children: Build) {
    let label = props.label.as_ref().map(|l| bound_text(ctx, l));
    let bound = props.model.map(|m| Bound::model(ctx, m));
    let local = Rc::new(crate::reactive::Cell::new(false));
    let then = bound_action(ctx, &props.then);
    let read_open: Rc<dyn Fn() -> bool> = {
        let bound = bound.clone();
        let local = Rc::clone(&local);
        Rc::new(move || match &bound {
            Some(b) => crate::value::truthy(&b.get()),
            None => local.get(),
        })
    };
    let peek_open: Rc<dyn Fn() -> bool> = {
        let bound = bound.clone();
        let local = Rc::clone(&local);
        Rc::new(move || match &bound {
            Some(b) => crate::value::truthy(&b.peek()),
            None => local.peek(),
        })
    };
    let write_open: Rc<dyn Fn(bool)> = {
        let bound = bound.clone();
        let local = Rc::clone(&local);
        let then = then.clone();
        Rc::new(move |next| match &bound {
            Some(b) => {
                if crate::value::truthy(&b.peek()) != next {
                    b.set(Value::Bool(next));
                }
            }
            None => {
                if local.peek() != next {
                    local.set(next);
                    if let Some(run) = &then {
                        run();
                    }
                }
            }
        })
    };
    let expander = gtk::Expander::new(None);
    expander.set_hexpand(true);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if let Some(icon) = props.icon {
        header.append(&icons::image(icon));
    }
    if let Some(lbl) = label.clone() {
        let text = gtk::Label::new(None);
        let shown = text.clone();
        effect(move || shown.set_text(&lbl()));
        header.append(&text);
    }
    expander.set_label_widget(Some(&header));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_top(6);
    body.set_margin_start(16);
    children(&Parent::of_box(&body), ctx);
    expander.set_child(Some(&body));
    let updating = Rc::new(Cell::new(false));
    {
        let exp = expander.clone();
        let read_open = Rc::clone(&read_open);
        let guard = Rc::clone(&updating);
        effect(move || {
            let open = read_open();
            if exp.is_expanded() != open {
                guard.set(true);
                exp.set_expanded(open);
                guard.set(false);
            }
        });
    }
    {
        let write_open = Rc::clone(&write_open);
        let guard = Rc::clone(&updating);
        expander.connect_expanded_notify(move |exp| {
            if !guard.get() {
                write_open(exp.is_expanded());
            }
        });
    }
    p.add(&expander);
    let peek_toggle = Rc::clone(&peek_open);
    let write_toggle = Rc::clone(&write_open);
    let write_bool = Rc::clone(&write_open);
    let peek_val = Rc::clone(&peek_open);
    controls::register(
        ctx,
        props.id,
        "disclosure",
        label,
        None,
        Actions {
            activate: Some(Rc::new(move || write_toggle(!peek_toggle()))),
            set_bool: Some(Rc::new(move |on| write_bool(on))),
            value: Some(Rc::new(move || {
                Some(if peek_val() { "true" } else { "false" }.to_string())
            })),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct NumberInputProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub precision: u32,
    pub enabled: Option<Get>,
    pub label_hidden: bool,
}

#[allow(non_snake_case)]
pub fn NumberInput(p: &Parent, ctx: &Ctx, props: NumberInputProps) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|l| bound_text(ctx, l));
    let enabled = bound_enabled(ctx, &props.enabled);
    let min = props.min.unwrap_or(-1e9);
    let max = props.max.unwrap_or(1e9);
    let step = props.step.unwrap_or(1.0);
    let precision = props.precision.min(6);
    let clamp_round = move |raw: f64| -> f64 {
        let clamped = raw.clamp(min, max);
        let factor = 10_f64.powi(precision as i32);
        (clamped * factor).round() / factor
    };
    let initial = clamp_round(as_f64(&bound.peek()));
    let adj = gtk::Adjustment::new(initial, min, max, step, step * 10.0, 0.0);
    let spin = gtk::SpinButton::new(Some(&adj), step, precision);
    spin.set_numeric(true);
    spin.set_valign(gtk::Align::Center);
    let updating = Rc::new(Cell::new(false));
    {
        let spin_r = spin.clone();
        let read = bound.clone();
        let guard = Rc::clone(&updating);
        effect(move || {
            let v = as_f64(&read.get());
            if (spin_r.value() - v).abs() > 1e-12 {
                guard.set(true);
                spin_r.set_value(v);
                guard.set(false);
            }
        });
    }
    if let Some(en) = enabled.clone() {
        let s = spin.clone();
        effect(move || s.set_sensitive(en()));
    }
    let commit: Rc<dyn Fn(f64)> = {
        let bound = bound.clone();
        Rc::new(move |raw| {
            if !raw.is_finite() {
                return;
            }
            let next = clamp_round(raw);
            if (as_f64(&bound.peek()) - next).abs() > 1e-12 {
                bound.set(Value::Float(next));
            }
        })
    };
    {
        let commit = Rc::clone(&commit);
        let guard = Rc::clone(&updating);
        spin.connect_value_changed(move |s| {
            if !guard.get() {
                commit(s.value());
            }
        });
    }
    if props.label_hidden
        && let Some(lbl) = label.clone()
    {
        let named = spin.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&lbl())]));
    }
    if ctx.in_toolbar() || props.label_hidden {
        if p.is_list() {
            p.add(&plain_row(&spin));
        } else {
            keeps_width(&spin);
            p.add(&spin);
        }
    } else if p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(lbl) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&lbl())));
        }
        row.add_suffix(&spin);
        p.add(&row);
    } else {
        p.add(&beside(label.clone(), &spin));
    }
    let commit_edit = Rc::clone(&commit);
    let commit_num = Rc::clone(&commit);
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "number_input",
        label,
        enabled,
        Actions {
            edit: Some(Rc::new(move |s| {
                if let Ok(v) = s.trim().parse::<f64>() {
                    commit_edit(v);
                }
            })),
            set_number: Some(Rc::new(move |v| commit_num(v))),
            value: Some(Rc::new(move || Some(number(as_f64(&value.peek()))))),
            ..Actions::default()
        },
    );
}

fn date_ymd(value: &Value) -> (i64, i64, i64) {
    if let Value::Record(fields) = value
        && fields.len() >= 3
        && let (Value::Int(y), Value::Int(m), Value::Int(d)) = (&fields[0], &fields[1], &fields[2])
    {
        let y = if (1..=9999).contains(y) { *y } else { 2026 };
        let m = (*m).clamp(1, 12);
        let d = (*d).clamp(1, 31);
        return (y, m, d);
    }
    (2026, 1, 1)
}

fn date_value((y, m, d): (i64, i64, i64)) -> Value {
    Value::Record(vec![Value::Int(y), Value::Int(m), Value::Int(d)])
}

fn format_iso_date((y, m, d): (i64, i64, i64)) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

fn parse_iso_date(raw: &str) -> Option<(i64, i64, i64)> {
    let mut parts = raw.trim().split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some()
        || !(1..=9999).contains(&y)
        || !(1..=12).contains(&m)
        || !(1..=31).contains(&d)
    {
        return None;
    }
    let dt = glib::DateTime::from_local(y as i32, m as i32, d as i32, 12, 0, 0.0).ok()?;
    Some((
        i64::from(dt.year()),
        i64::from(dt.month()),
        i64::from(dt.day_of_month()),
    ))
}

#[derive(Default)]
pub struct DatePickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub min: Option<Get>,
    pub max: Option<Get>,
    pub enabled: Option<Get>,
    pub label_hidden: bool,
}

#[allow(non_snake_case)]
pub fn DatePicker(p: &Parent, ctx: &Ctx, props: DatePickerProps) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|l| bound_text(ctx, l));
    let enabled = bound_enabled(ctx, &props.enabled);
    let min_get = props.min.clone();
    let max_get = props.max.clone();
    let clamp_date = {
        let at = ctx.clone();
        move |mut ymd: (i64, i64, i64)| -> (i64, i64, i64) {
            if let Some(lo_fn) = &min_get {
                let lo = date_ymd(&lo_fn(&at));
                if ymd < lo {
                    ymd = lo;
                }
            }
            if let Some(hi_fn) = &max_get {
                let hi = date_ymd(&hi_fn(&at));
                if ymd > hi {
                    ymd = hi;
                }
            }
            ymd
        }
    };
    let commit: Rc<dyn Fn((i64, i64, i64))> = {
        let bound = bound.clone();
        let clamp = clamp_date.clone();
        Rc::new(move |raw| {
            let next = clamp(raw);
            if date_ymd(&bound.peek()) != next {
                bound.set(date_value(next));
            }
        })
    };

    let calendar = gtk::Calendar::new();
    let popover = gtk::Popover::new();
    popover.set_child(Some(&calendar));
    let button = gtk::MenuButton::new();
    button.set_popover(Some(&popover));
    button.set_valign(gtk::Align::Center);
    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    btn_box.append(&icons::image("calendar"));
    let text_lbl = gtk::Label::new(None);
    btn_box.append(&text_lbl);
    button.set_child(Some(&btn_box));

    let updating = Rc::new(Cell::new(false));
    {
        let cal = calendar.clone();
        let lbl = text_lbl.clone();
        let read = bound.clone();
        let guard = Rc::clone(&updating);
        effect(move || {
            let ymd = date_ymd(&read.get());
            lbl.set_text(&format_iso_date(ymd));
            if let Ok(dt) =
                glib::DateTime::from_local(ymd.0 as i32, ymd.1 as i32, ymd.2 as i32, 12, 0, 0.0)
            {
                guard.set(true);
                cal.select_day(&dt);
                guard.set(false);
            }
        });
    }
    if let Some(en) = enabled.clone() {
        let b = button.clone();
        effect(move || b.set_sensitive(en()));
    }
    {
        let commit = Rc::clone(&commit);
        let guard = Rc::clone(&updating);
        let pop = popover.clone();
        calendar.connect_day_selected(move |cal| {
            if !guard.get() {
                let dt = cal.date();
                commit((
                    i64::from(dt.year()),
                    i64::from(dt.month()),
                    i64::from(dt.day_of_month()),
                ));
                pop.popdown();
            }
        });
    }
    if props.label_hidden
        && let Some(lbl) = label.clone()
    {
        let named = button.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&lbl())]));
    }
    if ctx.in_toolbar() || props.label_hidden {
        if p.is_list() {
            p.add(&plain_row(&button));
        } else {
            keeps_width(&button);
            p.add(&button);
        }
    } else if p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(lbl) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&lbl())));
        }
        row.add_suffix(&button);
        p.add(&row);
    } else {
        p.add(&beside(label.clone(), &button));
    }
    let commit_edit = Rc::clone(&commit);
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "date_picker",
        label,
        enabled,
        Actions {
            edit: Some(Rc::new(move |s| {
                if let Some(parsed) = parse_iso_date(&s) {
                    commit_edit(parsed);
                }
            })),
            value: Some(Rc::new(move || {
                Some(format_iso_date(date_ymd(&value.peek())))
            })),
            ..Actions::default()
        },
    );
}

fn clamp_time_minutes(raw: f64, min: f64, max: f64, step: f64) -> f64 {
    if !raw.is_finite() {
        return min;
    }
    let s = step.round().max(1.0);
    let clamped = raw.clamp(min, max);
    let snapped = min + ((clamped - min) / s).round() * s;
    snapped.round().clamp(min, max)
}

fn format_time_minutes(mins: f64) -> String {
    let total = if mins.is_finite() {
        mins.round().clamp(0.0, 1439.0) as i32
    } else {
        0
    };
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn parse_time_minutes(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if let Some((h, m)) = trimmed.split_once(':') {
        let hh: i32 = h.trim().parse().ok()?;
        let mm: i32 = m.trim().parse().ok()?;
        if (0..=23).contains(&hh) && (0..=59).contains(&mm) {
            return Some(f64::from(hh * 60 + mm));
        }
    }
    trimmed.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[derive(Default)]
pub struct TimePickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub enabled: Option<Get>,
    pub label_hidden: bool,
}

#[allow(non_snake_case)]
pub fn TimePicker(p: &Parent, ctx: &Ctx, props: TimePickerProps) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|l| bound_text(ctx, l));
    let enabled = bound_enabled(ctx, &props.enabled);
    let min = props.min.unwrap_or(0.0).clamp(0.0, 1439.0);
    let max = props.max.unwrap_or(1439.0).clamp(min, 1439.0);
    let step = props.step.unwrap_or(1.0).round().clamp(1.0, 60.0);
    let commit: Rc<dyn Fn(f64)> = {
        let bound = bound.clone();
        Rc::new(move |raw| {
            if !raw.is_finite() {
                return;
            }
            let next = clamp_time_minutes(raw, min, max, step);
            if (as_f64(&bound.peek()) - next).abs() > 1e-6 {
                bound.set(Value::Float(next));
            }
        })
    };
    let initial = clamp_time_minutes(as_f64(&bound.peek()), min, max, step) as i32;
    let h_adj = gtk::Adjustment::new(f64::from(initial / 60), 0.0, 23.0, 1.0, 4.0, 0.0);
    let m_adj = gtk::Adjustment::new(f64::from(initial % 60), 0.0, 59.0, step, step * 5.0, 0.0);
    let h_spin = gtk::SpinButton::new(Some(&h_adj), 1.0, 0);
    let m_spin = gtk::SpinButton::new(Some(&m_adj), step, 0);
    h_spin.set_wrap(true);
    m_spin.set_wrap(true);
    h_spin.set_numeric(true);
    m_spin.set_numeric(true);
    h_spin.set_width_chars(2);
    m_spin.set_width_chars(2);
    h_spin.connect_output(|s| {
        s.set_text(&format!("{:02}", s.value_as_int()));
        glib::Propagation::Stop
    });
    m_spin.connect_output(|s| {
        s.set_text(&format!("{:02}", s.value_as_int()));
        glib::Propagation::Stop
    });
    let sep = gtk::Label::new(Some(":"));
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row_box.set_valign(gtk::Align::Center);
    row_box.append(&h_spin);
    row_box.append(&sep);
    row_box.append(&m_spin);

    let updating = Rc::new(Cell::new(false));
    {
        let h_r = h_spin.clone();
        let m_r = m_spin.clone();
        let read = bound.clone();
        let guard = Rc::clone(&updating);
        effect(move || {
            let mins = clamp_time_minutes(as_f64(&read.get()), min, max, step) as i32;
            let hh = f64::from(mins / 60);
            let mm = f64::from(mins % 60);
            guard.set(true);
            if (h_r.value() - hh).abs() > 0.5 {
                h_r.set_value(hh);
            }
            if (m_r.value() - mm).abs() > 0.5 {
                m_r.set_value(mm);
            }
            guard.set(false);
        });
    }
    if let Some(en) = enabled.clone() {
        let bx = row_box.clone();
        effect(move || bx.set_sensitive(en()));
    }
    {
        let commit = Rc::clone(&commit);
        let guard = Rc::clone(&updating);
        let m_ref = m_spin.clone();
        h_spin.connect_value_changed(move |h| {
            if !guard.get() {
                commit(f64::from(h.value_as_int() * 60 + m_ref.value_as_int()));
            }
        });
    }
    {
        let commit = Rc::clone(&commit);
        let guard = Rc::clone(&updating);
        let h_ref = h_spin.clone();
        m_spin.connect_value_changed(move |m| {
            if !guard.get() {
                commit(f64::from(h_ref.value_as_int() * 60 + m.value_as_int()));
            }
        });
    }
    if props.label_hidden
        && let Some(lbl) = label.clone()
    {
        let named = row_box.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&lbl())]));
    }
    if ctx.in_toolbar() || props.label_hidden {
        if p.is_list() {
            p.add(&plain_row(&row_box));
        } else {
            keeps_width(&row_box);
            p.add(&row_box);
        }
    } else if p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(lbl) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&lbl())));
        }
        row.add_suffix(&row_box);
        p.add(&row);
    } else {
        p.add(&beside(label.clone(), &row_box));
    }
    let commit_edit = Rc::clone(&commit);
    let commit_num = Rc::clone(&commit);
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "time_picker",
        label,
        enabled,
        Actions {
            edit: Some(Rc::new(move |s| {
                if let Some(mins) = parse_time_minutes(&s) {
                    commit_edit(mins);
                }
            })),
            set_number: Some(Rc::new(move |v| commit_num(v))),
            value: Some(Rc::new(move || {
                Some(format_time_minutes(as_f64(&value.peek())))
            })),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct EmptyStateProps {
    pub title: Option<Str>,
    pub message: Option<Str>,
    pub icon: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn EmptyState(p: &Parent, ctx: &Ctx, props: EmptyStateProps, actions: Option<Build>) {
    let page = adw::StatusPage::new();
    page.set_hexpand(true);
    if let Some(icon) = props.icon {
        page.set_icon_name(Some(icons::name(icon)));
    }
    if let Some(title) = props.title.as_ref().map(|t| bound_text(ctx, t)) {
        let shown = page.clone();
        effect(move || shown.set_title(&title()));
    }
    if let Some(msg) = props.message.as_ref().map(|m| bound_text(ctx, m)) {
        let shown = page.clone();
        effect(move || {
            let text = msg();
            shown.set_description((!text.is_empty()).then_some(text.as_str()));
        });
    }
    if let Some(build) = actions {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_halign(gtk::Align::Center);
        build(&Parent::of_box(&row), ctx);
        if row.first_child().is_some() {
            page.set_child(Some(&row));
        }
    }
    p.add(&page);
}

#[derive(Default)]
pub struct BadgeProps {
    pub content: Option<Str>,
    pub tone: Option<&'static str>,
    pub icon: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn Badge(p: &Parent, ctx: &Ctx, props: BadgeProps) {
    let tone = props.tone.unwrap_or("neutral");
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    pill.add_css_class("bern-badge");
    pill.add_css_class(&format!("bern-badge-{tone}"));
    pill.set_halign(gtk::Align::Start);
    pill.set_valign(gtk::Align::Center);
    if let Some(icon) = props.icon {
        let img = icons::image(icon);
        img.set_pixel_size(12);
        pill.append(&img);
    }
    let lbl = gtk::Label::new(None);
    lbl.set_single_line_mode(true);
    if let Some(content) = props.content.as_ref().map(|c| bound_text(ctx, c)) {
        bind_label(&lbl, content);
    }
    pill.append(&lbl);
    keeps_width(&pill);
    p.add(&pill);
}

#[derive(Default)]
pub struct AvatarProps {
    pub label: Option<Str>,
    pub source: Option<&'static str>,
    pub url: Option<Get>,
    pub size: Option<Num>,
    pub shape: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn Avatar(p: &Parent, ctx: &Ctx, props: AvatarProps) {
    let initial_size = props
        .size
        .as_ref()
        .map_or(40, |n| n.peek(ctx).round().max(8.0) as i32);
    let avatar = adw::Avatar::new(initial_size, None, true);
    avatar.set_halign(gtk::Align::Start);
    avatar.set_valign(gtk::Align::Center);
    if props.shape == Some("rounded") {
        avatar.add_css_class("bern-avatar-rounded");
    }
    if let Some(label) = props.label.as_ref().map(|l| bound_text(ctx, l)) {
        let shown = avatar.clone();
        effect(move || {
            let text = label();
            shown.set_text(Some(&text));
            shown
                .upcast_ref::<gtk::Widget>()
                .update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    if let Some(size) = &props.size {
        let shown = avatar.clone();
        bind_number(ctx, size, None, &avatar, move |v| {
            shown.set_size(v.round().max(8.0) as i32);
        });
    }
    if let Some(source) = props.source {
        let path = std::path::PathBuf::from(crate::host::resources_dir()).join(source);
        if let Some(texture) = read_picture(&path, None, 0.0, (None, None)) {
            avatar.set_custom_image(Some(&texture));
        }
    } else if let Some(url_get) = props.url {
        let at = ctx.clone();
        let shown = avatar.clone();
        let loading: Rc<std::cell::RefCell<Option<glib::JoinHandle<()>>>> = Rc::default();
        let stop_on_drop = Rc::clone(&loading);
        crate::reactive::on_cleanup(move || {
            if let Some(handle) = stop_on_drop.take() {
                handle.abort();
            }
        });
        effect(move || {
            if let Some(handle) = loading.take() {
                handle.abort();
            }
            let raw = url_get(&at);
            let Some(url) = crate::canvas::url(&raw).filter(|u| !u.is_empty()) else {
                shown.set_custom_image(None::<&gtk::gdk::Paintable>);
                return;
            };
            let target = shown.clone();
            let size = f64::from(target.size().max(40));
            let slot = Rc::clone(&loading);
            let handle = glib::spawn_future_local(async move {
                let texture = crate::features::load_avatar_texture(&url, size).await;
                slot.take();
                target.set_custom_image(texture.as_ref());
            });
            *loading.borrow_mut() = Some(handle);
        });
    }
    keeps_width(&avatar);
    p.add(&avatar);
}

pub struct TreeViewProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub items: Get,
    pub key: usize,
    pub parent: usize,
    pub var: &'static str,
    pub selection: Option<Field>,
    pub expanded: Option<Field>,
    pub action: Option<Act>,
}

#[allow(non_snake_case)]
pub fn TreeView(p: &Parent, ctx: &Ctx, props: TreeViewProps, row: Build) {
    let label = props.label.as_ref().map(|l| bound_text(ctx, l));
    let activate = bound_action(ctx, &props.action);
    let local_expanded: Rc<crate::reactive::Cell<Vec<String>>> =
        Rc::new(crate::reactive::Cell::new(Vec::new()));
    let last_items: Rc<std::cell::RefCell<Vec<Value>>> = Rc::default();

    type ReadExpanded = Rc<dyn Fn() -> Vec<String>>;
    type SetExpanded = Rc<dyn Fn(&str, bool)>;

    let read_expanded: ReadExpanded = {
        let ctx = ctx.clone();
        let ext = props.expanded;
        let local = Rc::clone(&local_expanded);
        Rc::new(move || {
            if let Some(field) = ext {
                crate::value::as_list(ctx.session(field.session, field.field))
                    .into_iter()
                    .map(|v| crate::value::key_text(&v))
                    .collect()
            } else {
                local.get()
            }
        })
    };
    let set_expanded: SetExpanded = {
        let win = Rc::clone(ctx.win());
        let ext = props.expanded;
        let local = Rc::clone(&local_expanded);
        let last = Rc::clone(&last_items);
        let key_idx = props.key;
        Rc::new(move |wanted: &str, open: bool| {
            if let Some(field) = ext {
                let current = crate::value::as_list(win.cell(field).peek());
                let mut next = Vec::new();
                let mut present = false;
                for v in current {
                    if crate::value::key_text(&v) == wanted {
                        present = true;
                        if open {
                            next.push(v);
                        }
                    } else {
                        next.push(v);
                    }
                }
                if open
                    && !present
                    && let Some(found) = last.borrow().iter().find_map(|item| {
                        let kv = crate::value::at(item.clone(), &[key_idx]);
                        (crate::value::key_text(&kv) == wanted).then_some(kv)
                    })
                {
                    next.push(found);
                }
                win.write(field, Value::List(next));
            } else {
                let mut cur = local.peek();
                if open {
                    if !cur.iter().any(|k| k == wanted) {
                        cur.push(wanted.to_string());
                    }
                } else {
                    cur.retain(|k| k != wanted);
                }
                local.set(cur);
            }
        })
    };
    let select_key: Rc<dyn Fn(&str) -> bool> = {
        let win = Rc::clone(ctx.win());
        let sel = props.selection;
        let last = Rc::clone(&last_items);
        let key_idx = props.key;
        Rc::new(move |wanted: &str| {
            let found = last.borrow().iter().find_map(|item| {
                let kv = crate::value::at(item.clone(), &[key_idx]);
                (crate::value::key_text(&kv) == wanted).then_some(kv)
            });
            let Some(kv) = found else {
                return false;
            };
            if let Some(field) = sel {
                win.write(field, kv);
            }
            true
        })
    };

    let host = gtk::Box::new(gtk::Orientation::Vertical, 2);
    host.set_hexpand(true);
    let scopes: Rc<std::cell::RefCell<Vec<crate::reactive::Scope>>> = Rc::default();
    let drop_scopes = Rc::clone(&scopes);
    crate::reactive::on_cleanup(move || {
        for scope in drop_scopes.borrow_mut().drain(..) {
            scope.dispose();
        }
    });

    {
        let host = host.clone();
        let scopes = Rc::clone(&scopes);
        let ctx = ctx.clone();
        let items = props.items;
        let key_idx = props.key;
        let parent_idx = props.parent;
        let var = props.var;
        let sel = props.selection;
        let read_expanded = Rc::clone(&read_expanded);
        let set_expanded = Rc::clone(&set_expanded);
        let activate = activate.clone();
        let last_items = Rc::clone(&last_items);
        effect(move || {
            let list = crate::value::as_list(items(&ctx));
            let expanded = read_expanded();
            let selected_key =
                sel.map(|f| crate::value::key_text(&ctx.session(f.session, f.field)));
            crate::reactive::untrack(|| {
                *last_items.borrow_mut() = list.clone();
                for scope in scopes.borrow_mut().drain(..) {
                    scope.dispose();
                }
                while let Some(child) = host.first_child() {
                    host.remove(&child);
                }
                let mut known = std::collections::HashSet::new();
                let mut keys = Vec::with_capacity(list.len());
                for item in &list {
                    let k = crate::value::key_text(&crate::value::at(item.clone(), &[key_idx]));
                    known.insert(k.clone());
                    keys.push(k);
                }
                let mut children_map: std::collections::HashMap<String, Vec<usize>> =
                    std::collections::HashMap::new();
                let mut roots = Vec::new();
                for (i, item) in list.iter().enumerate() {
                    if let Value::Optional(Some(inner)) =
                        crate::value::at(item.clone(), &[parent_idx])
                    {
                        let pk = crate::value::key_text(&inner);
                        if known.contains(&pk) && pk != keys[i] {
                            children_map.entry(pk).or_default().push(i);
                            continue;
                        }
                    }
                    roots.push(i);
                }
                let exp_set: std::collections::HashSet<&str> =
                    expanded.iter().map(String::as_str).collect();
                let mut visited = std::collections::HashSet::new();
                let mut stack: Vec<(usize, i32)> =
                    roots.into_iter().rev().map(|idx| (idx, 0)).collect();
                while let Some((idx, depth)) = stack.pop() {
                    let k = keys[idx].clone();
                    if !visited.insert(k.clone()) {
                        continue;
                    }
                    let kids = children_map.get(&k);
                    let has_kids = kids.is_some_and(|v| !v.is_empty());
                    let is_open = exp_set.contains(k.as_str());
                    let is_sel = selected_key.as_deref() == Some(k.as_str());

                    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    line.set_hexpand(true);
                    line.set_margin_start(8 + depth * 18);
                    line.set_margin_end(8);
                    line.set_margin_top(2);
                    line.set_margin_bottom(2);
                    line.add_css_class("bern-tree-row");
                    if is_sel {
                        line.add_css_class("bern-tree-selected");
                    }
                    if has_kids {
                        let chev = gtk::Button::new();
                        chev.add_css_class("flat");
                        chev.set_valign(gtk::Align::Center);
                        chev.set_child(Some(&icons::image(if is_open {
                            "chevron.down"
                        } else {
                            "chevron.right"
                        })));
                        let set_exp = Rc::clone(&set_expanded);
                        let key_click = k.clone();
                        chev.connect_clicked(move |_| set_exp(&key_click, !is_open));
                        line.append(&chev);
                    } else {
                        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                        spacer.set_size_request(24, 1);
                        line.append(&spacer);
                    }
                    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    content.set_hexpand(true);
                    content.set_valign(gtk::Align::Center);
                    let scope = crate::reactive::Scope::detached();
                    let item_val = list[idx].clone();
                    let k_for_ctx = k.clone();
                    let row_ctx = ctx
                        .with_key(Rc::new(move || k_for_ctx.clone()))
                        .with_local(var, Rc::new(move || item_val.clone()));
                    let build = Rc::clone(&row);
                    let parent_box = Parent::of_box(&content);
                    scope.run(|| build(&parent_box, &row_ctx));
                    scopes.borrow_mut().push(scope);
                    line.append(&content);

                    let click = gtk::GestureClick::new();
                    let key_val = crate::value::at(list[idx].clone(), &[key_idx]);
                    let win = Rc::clone(ctx.win());
                    let act = activate.clone();
                    click.connect_released(move |_, n_press, _, _| {
                        if let Some(field) = sel {
                            win.write(field, key_val.clone());
                        }
                        if n_press >= 2
                            && let Some(run) = &act
                        {
                            run();
                        }
                    });
                    line.add_controller(click);
                    host.append(&line);

                    if has_kids
                        && is_open
                        && let Some(child_indices) = kids
                    {
                        for &cidx in child_indices.iter().rev() {
                            stack.push((cidx, depth + 1));
                        }
                    }
                }
            });
        });
    }

    if let Some(lbl) = label.clone() {
        let named = host.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&lbl())]));
    }
    p.add(&host);

    let set_exp_cli = Rc::clone(&set_expanded);
    let read_exp_cli = Rc::clone(&read_expanded);
    let win_val = Rc::clone(ctx.win());
    let sel_val = props.selection;
    controls::register(
        ctx,
        props.id,
        "tree_view",
        label,
        None,
        Actions {
            activate,
            edit: Some(Rc::new(move |cmd| {
                if let Some((op, k)) = cmd.trim().split_once(':') {
                    match op {
                        "expand" => set_exp_cli(k, true),
                        "collapse" => set_exp_cli(k, false),
                        "toggle" => {
                            let open = read_exp_cli().iter().any(|x| x == k);
                            set_exp_cli(k, !open);
                        }
                        _ => {}
                    }
                }
            })),
            select: Some(select_key),
            value: Some(Rc::new(move || {
                sel_val.map(|f| crate::value::key_text(&win_val.cell(f).peek()))
            })),
            ..Actions::default()
        },
    );
}

pub struct SplitPaneProps {
    pub id: Option<&'static str>,
    pub vertical: bool,
    pub ratio: Option<Field>,
    pub min_first: Option<Num>,
    pub min_second: Option<Num>,
}

#[allow(non_snake_case)]
pub fn SplitPane(p: &Parent, ctx: &Ctx, props: SplitPaneProps, first: Build, second: Build) {
    let orientation = if props.vertical {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    };
    let paned = gtk::Paned::new(orientation);
    paned.set_hexpand(true);
    paned.set_vexpand(true);
    paned.set_resize_start_child(true);
    paned.set_resize_end_child(true);
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let box2 = gtk::Box::new(gtk::Orientation::Vertical, 0);
    box1.set_hexpand(true);
    box1.set_vexpand(true);
    box2.set_hexpand(true);
    box2.set_vexpand(true);
    first(&Parent::of_box(&box1), ctx);
    second(&Parent::of_box(&box2), ctx);
    paned.set_start_child(Some(&box1));
    paned.set_end_child(Some(&box2));

    let min_first = Rc::new(Cell::new(120.0_f64));
    let min_second = Rc::new(Cell::new(120.0_f64));
    if let Some(m1) = &props.min_first {
        let b = box1.clone();
        let slot = Rc::clone(&min_first);
        let vert = props.vertical;
        bind_number(ctx, m1, None, &box1, move |v| {
            let px = v.max(24.0);
            slot.set(px);
            if vert {
                b.set_size_request(-1, px.round() as i32);
            } else {
                b.set_size_request(px.round() as i32, -1);
            }
        });
    }
    if let Some(m2) = &props.min_second {
        let b = box2.clone();
        let slot = Rc::clone(&min_second);
        let vert = props.vertical;
        bind_number(ctx, m2, None, &box2, move |v| {
            let px = v.max(24.0);
            slot.set(px);
            if vert {
                b.set_size_request(-1, px.round() as i32);
            } else {
                b.set_size_request(px.round() as i32, -1);
            }
        });
    }

    let local_ratio = Rc::new(crate::reactive::Cell::new(0.5_f64));
    let syncing = Rc::new(Cell::new(false));
    let read_ratio: Rc<dyn Fn() -> f64> = {
        let ctx = ctx.clone();
        let ext = props.ratio;
        let local = Rc::clone(&local_ratio);
        Rc::new(move || {
            let v = match ext {
                Some(f) => crate::value::as_f64(&ctx.session(f.session, f.field)),
                None => local.get(),
            };
            if v.is_finite() {
                v.clamp(0.05, 0.95)
            } else {
                0.5
            }
        })
    };
    let write_ratio: Rc<dyn Fn(f64)> = {
        let win = Rc::clone(ctx.win());
        let ext = props.ratio;
        let local = Rc::clone(&local_ratio);
        Rc::new(move |raw: f64| {
            let v = if raw.is_finite() {
                raw.clamp(0.05, 0.95)
            } else {
                0.5
            };
            if let Some(f) = ext {
                win.write(f, Value::Float(v));
            } else {
                local.set(v);
            }
        })
    };
    {
        let paned_eff = paned.clone();
        let read = Rc::clone(&read_ratio);
        let syncing = Rc::clone(&syncing);
        let vert = props.vertical;
        effect(move || {
            let r = read();
            let total = if vert {
                paned_eff.height()
            } else {
                paned_eff.width()
            };
            if total > 10 {
                syncing.set(true);
                paned_eff.set_position((f64::from(total) * r).round() as i32);
                syncing.set(false);
            }
        });
    }
    {
        let write = Rc::clone(&write_ratio);
        let syncing = Rc::clone(&syncing);
        let vert = props.vertical;
        paned.connect_position_notify(move |pw| {
            if syncing.get() {
                return;
            }
            let total = if vert { pw.height() } else { pw.width() };
            if total > 10 {
                write(f64::from(pw.position()) / f64::from(total));
            }
        });
    }
    p.add(&paned);
    if let Some(id) = props.id {
        let write_cli = Rc::clone(&write_ratio);
        let win_val = Rc::clone(ctx.win());
        let ext = props.ratio;
        let local = Rc::clone(&local_ratio);
        controls::register(
            ctx,
            id,
            "split_pane",
            Some(Rc::new(move || id.to_string())),
            None,
            Actions {
                set_number: Some(Rc::new(move |v| write_cli(v))),
                value: Some(Rc::new(move || {
                    let v = match ext {
                        Some(f) => crate::value::as_f64(&win_val.cell(f).peek()),
                        None => local.peek(),
                    };
                    Some(format!("{:.3}", v.clamp(0.05, 0.95)))
                })),
                ..Actions::default()
            },
        );
    }
}

pub struct ReorderableProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub items: Get,
    pub key: usize,
    pub var: &'static str,
    pub from: Field,
    pub to: Field,
    pub action: Option<Act>,
}

#[allow(non_snake_case)]
pub fn Reorderable(p: &Parent, ctx: &Ctx, props: ReorderableProps, row: Build) {
    let label = props.label.as_ref().map(|l| bound_text(ctx, l));
    let run_action = bound_action(ctx, &props.action);
    let count_cell = Rc::new(Cell::new(0usize));
    let commit_move: Rc<dyn Fn(i64, i64)> = {
        let win = Rc::clone(ctx.win());
        let from = props.from;
        let to = props.to;
        let run_action = run_action.clone();
        let count_cell = Rc::clone(&count_cell);
        Rc::new(move |src: i64, dst: i64| {
            let n = count_cell.get() as i64;
            if src < 0 || src >= n || dst < 0 || dst >= n || src == dst {
                return;
            }
            win.write(from, Value::Int(src));
            win.write(to, Value::Int(dst));
            if let Some(act) = &run_action {
                act();
            }
        })
    };

    let host = gtk::Box::new(gtk::Orientation::Vertical, 4);
    host.set_hexpand(true);
    let scopes: Rc<std::cell::RefCell<Vec<crate::reactive::Scope>>> = Rc::default();
    let drop_scopes = Rc::clone(&scopes);
    crate::reactive::on_cleanup(move || {
        for scope in drop_scopes.borrow_mut().drain(..) {
            scope.dispose();
        }
    });

    {
        let host = host.clone();
        let scopes = Rc::clone(&scopes);
        let ctx = ctx.clone();
        let items = props.items;
        let key_idx = props.key;
        let var = props.var;
        let commit_move = Rc::clone(&commit_move);
        let count_cell = Rc::clone(&count_cell);
        effect(move || {
            let list = crate::value::as_list(items(&ctx));
            crate::reactive::untrack(|| {
                count_cell.set(list.len());
                for scope in scopes.borrow_mut().drain(..) {
                    scope.dispose();
                }
                while let Some(child) = host.first_child() {
                    host.remove(&child);
                }
                let total = list.len();
                for (i, item) in list.iter().enumerate() {
                    let k = crate::value::key_text(&crate::value::at(item.clone(), &[key_idx]));
                    let card = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    card.set_hexpand(true);
                    card.add_css_class("card");
                    card.set_margin_top(2);
                    card.set_margin_bottom(2);

                    let grip = icons::image("line.3.horizontal");
                    grip.set_margin_start(8);
                    grip.set_valign(gtk::Align::Center);
                    card.append(&grip);

                    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    content.set_hexpand(true);
                    content.set_valign(gtk::Align::Center);
                    let scope = crate::reactive::Scope::detached();
                    let item_val = item.clone();
                    let row_ctx = ctx
                        .with_key(Rc::new(move || k.clone()))
                        .with_local(var, Rc::new(move || item_val.clone()));
                    let build = Rc::clone(&row);
                    let parent_box = Parent::of_box(&content);
                    scope.run(|| build(&parent_box, &row_ctx));
                    scopes.borrow_mut().push(scope);
                    card.append(&content);

                    let up = gtk::Button::new();
                    up.add_css_class("flat");
                    up.set_child(Some(&icons::image("chevron.up")));
                    up.set_sensitive(i > 0);
                    let cm_up = Rc::clone(&commit_move);
                    up.connect_clicked(move |_| cm_up(i as i64, i as i64 - 1));
                    card.append(&up);

                    let down = gtk::Button::new();
                    down.add_css_class("flat");
                    down.set_child(Some(&icons::image("chevron.down")));
                    down.set_sensitive(i + 1 < total);
                    let cm_down = Rc::clone(&commit_move);
                    down.connect_clicked(move |_| cm_down(i as i64, i as i64 + 1));
                    card.append(&down);

                    // Native GTK4 drag-and-drop reordering on each row.
                    let drag = gtk::DragSource::new();
                    drag.set_actions(gtk::gdk::DragAction::MOVE);
                    let idx_payload = i.to_string();
                    drag.connect_prepare(move |_, _, _| {
                        Some(gtk::gdk::ContentProvider::for_value(
                            &idx_payload.to_value(),
                        ))
                    });
                    card.add_controller(drag);

                    let drop =
                        gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
                    let cm_drop = Rc::clone(&commit_move);
                    let dst = i as i64;
                    drop.connect_drop(move |_, value, _, _| {
                        if let Ok(raw) = value.get::<String>()
                            && let Ok(src) = raw.parse::<i64>()
                        {
                            cm_drop(src, dst);
                            return true;
                        }
                        false
                    });
                    card.add_controller(drop);

                    host.append(&card);
                }
            });
        });
    }

    if let Some(lbl) = label.clone() {
        let named = host.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&lbl())]));
    }
    p.add(&host);
    let cm_cli = Rc::clone(&commit_move);
    let count_cli = Rc::clone(&count_cell);
    controls::register(
        ctx,
        props.id,
        "reorderable",
        label,
        None,
        Actions {
            edit: Some(Rc::new(move |s| {
                let normalized = s.trim().replace("->", ":");
                if let Some((a, b)) = normalized.split_once(':')
                    && let (Ok(from), Ok(to)) = (a.trim().parse::<i64>(), b.trim().parse::<i64>())
                {
                    cm_cli(from, to);
                }
            })),
            value: Some(Rc::new(move || Some(count_cli.get().to_string()))),
            ..Actions::default()
        },
    );
}

pub struct BottomSheetProps {
    pub detents: Option<&'static str>,
    pub drag_indicator: bool,
    pub dismiss: Option<Act>,
}

#[allow(non_snake_case)]
pub fn BottomSheet(_p: &Parent, ctx: &Ctx, props: BottomSheetProps, children: Build) {
    let win = Rc::clone(ctx.win());
    let dialog = adw::Dialog::new();
    dialog.set_presentation_mode(adw::DialogPresentationMode::BottomSheet);
    let height = match props.detents {
        Some("large") => 640,
        _ => 420,
    };
    dialog.set_content_height(height);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.set_margin_top(12);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);
    children(&Parent::of_box(&body), ctx);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&body));
    scroller.set_vexpand(true);
    dialog.set_child(Some(&scroller));

    let closed_from_cleanup = Rc::new(Cell::new(false));
    let dismiss = bound_action(ctx, &props.dismiss);
    {
        let cleanup_flag = Rc::clone(&closed_from_cleanup);
        dialog.connect_closed(move |_| {
            if !cleanup_flag.get()
                && let Some(act) = &dismiss
            {
                act();
            }
        });
    }
    dialog.present(Some(&win.window));
    let d_clean = dialog.clone();
    crate::reactive::on_cleanup(move || {
        closed_from_cleanup.set(true);
        d_clean.force_close();
    });
}

pub struct RefreshableProps {
    pub id: &'static str,
    pub refreshing: Option<Get>,
    pub action: Option<Act>,
}

#[allow(non_snake_case)]
pub fn Refreshable(p: &Parent, ctx: &Ctx, props: RefreshableProps, children: Build) {
    let on_refresh = bound_action(ctx, &props.action);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 4);
    outer.set_hexpand(true);
    outer.set_vexpand(true);

    let revealer = gtk::Revealer::new();
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_halign(gtk::Align::Center);
    bar.set_margin_top(4);
    bar.set_margin_bottom(4);
    let spinner = gtk::Spinner::new();
    bar.append(&spinner);
    revealer.set_child(Some(&bar));
    revealer.set_reveal_child(false);
    if let Some(ref_get) = props.refreshing.clone() {
        let rev = revealer.clone();
        let spin = spinner.clone();
        let at = ctx.clone();
        effect(move || {
            let active = crate::value::truthy(&ref_get(&at));
            spin.set_spinning(active);
            rev.set_reveal_child(active);
        });
    }
    outer.append(&revealer);
    children(&Parent::of_box(&outer), ctx);
    p.add(&outer);

    let ref_val = props.refreshing;
    let at_val = ctx.clone();
    let id = props.id;
    controls::register(
        ctx,
        id,
        "refreshable",
        Some(Rc::new(move || id.to_string())),
        None,
        Actions {
            activate: on_refresh,
            value: Some(Rc::new(move || {
                let active = ref_val
                    .as_ref()
                    .is_some_and(|g| crate::value::truthy(&g(&at_val)));
                Some(if active { "refreshing" } else { "idle" }.into())
            })),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct StackProps {
    /// `column` or `row`.
    pub axis: &'static str,
    pub gap: Option<Num>,
    pub padding: Padding,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub align: Option<&'static str>,
    /// The spring bound lengths change with.
    pub animate: Option<Curve>,
    /// Children slide to new places with this spring.
    pub reflow: Option<Curve>,
}

/// `width` and `height` fix a side of `widget`: it neither shrinks below
/// nor stretches past it.
pub(crate) fn bind_size(
    ctx: &Ctx,
    widget: &gtk::Widget,
    width: &Option<Num>,
    height: &Option<Num>,
    animate: Option<Curve>,
) {
    let size = Rc::new(Cell::new((-1, -1)));
    if let Some(width) = width {
        widget.set_hexpand(false);
        let sized = widget.clone();
        let size = Rc::clone(&size);
        bind_number(ctx, width, animate, widget, move |value| {
            let (_, height) = size.get();
            let width = value.round().max(0.0) as i32;
            size.set((width, height));
            sized.set_size_request(width, height);
        });
    }
    if let Some(height) = height {
        widget.set_vexpand(false);
        let sized = widget.clone();
        bind_number(ctx, height, animate, widget, move |value| {
            let (width, _) = size.get();
            let height = value.round().max(0.0) as i32;
            size.set((width, height));
            sized.set_size_request(width, height);
        });
    }
}

/// How a column or row aligns its children across its axis. A column
/// without `align` is a leading one: what fills across, such as a field, a
/// divider or a grid, fills it, and the rest keeps its own width.
pub(crate) fn stack_cross(align: Option<&str>, column: bool) -> gtk::Align {
    match (align, column) {
        (Some("center"), _) => gtk::Align::Center,
        (Some("trailing") | Some("bottom"), _) => gtk::Align::End,
        (Some("leading") | Some("top"), _) => gtk::Align::Start,
        // A row's items share the baseline of their first lines.
        (Some("baseline"), false) => gtk::Align::BaselineCenter,
        (Some("fill"), _) => gtk::Align::Fill,
        (None, true) => gtk::Align::Start,
        (None, false) => gtk::Align::Center,
        _ => gtk::Align::Fill,
    }
}

#[allow(non_snake_case)]
pub fn Stack(p: &Parent, ctx: &Ctx, props: StackProps, children: Build) {
    let column = props.axis == "column";
    let orientation = if column {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    };
    let cross = stack_cross(props.align, column);
    // A column or row whose children slide takes a container of its own;
    // others are GTK boxes.
    let (container, inner): (gtk::Widget, Parent) = match props.reflow {
        Some(curve) => {
            let reflow = crate::layout::ReflowBox::new(orientation, 8, curve);
            if let Some(gap) = &props.gap {
                let spaced = reflow.clone();
                bind_number(ctx, gap, props.animate, &reflow, move |value| {
                    spaced.set_spacing(value.round() as i32);
                });
            }
            let inner = Parent::of_reflow(&reflow, cross);
            (reflow.upcast(), inner)
        }
        None => {
            let container = crate::layout::ClearBox::new(orientation, 8);
            if let Some(gap) = &props.gap {
                let spaced = container.clone();
                bind_number(ctx, gap, props.animate, &container, move |value| {
                    spaced.set_spacing(value.round().max(0.0) as i32);
                });
            }
            let inner = Parent::of_aligned_box(container.upcast_ref(), cross);
            (container.upcast(), inner)
        }
    };
    pad(ctx, &container, &props.padding, props.animate);
    // A declared side is the size the column or row asks for, as wide as
    // its content would like or not.
    let outer: gtk::Widget = if props.width.is_some() || props.height.is_some() {
        crate::layout::Sized::wrap(&container).upcast()
    } else {
        container
    };
    bind_size(ctx, &outer, &props.width, &props.height, props.animate);
    children(&inner, ctx);
    place(p, &outer);
}

#[derive(Default)]
pub struct GridProps {
    pub min_width: Option<Num>,
    pub gap: Option<Num>,
    /// The space between rows, where it differs from `gap`.
    pub row_gap: Option<Num>,
    pub padding: Padding,
    pub animate: Option<Curve>,
    /// Rows as tall as the column width over the ratio, plus the extra.
    pub cells: Option<(f64, Num)>,
    /// Cells slide to new places with this spring.
    pub reflow: Option<Curve>,
}

/// Equal columns, as many as fit at `min_width` or wider: a flow box whose
/// cells share each line. A bound `min_width` springs with `animate`, and
/// the cells move to their new places as the columns change. With
/// `cells`, every row is as tall as the columns' width gives, its items
/// from the top of their cells.
#[allow(non_snake_case)]
pub fn Grid(p: &Parent, ctx: &Ctx, props: GridProps, children: Build) {
    if let Some((ratio, extra)) = props.cells.clone() {
        crate::scene::cell_grid(p, ctx, props, ratio, extra, children);
        return;
    }
    let flow = gtk::FlowBox::new();
    flow.set_homogeneous(true);
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_activate_on_single_click(false);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(64);
    // The columns share the width it is given, as many as fit.
    flow.set_hexpand(true);
    flow.set_valign(gtk::Align::Start);
    flow.set_row_spacing(8);
    flow.set_column_spacing(8);
    if let Some(gap) = &props.gap {
        let spaced = flow.clone();
        let rows = props.row_gap.is_none();
        bind_number(ctx, gap, props.animate, &flow, move |value| {
            let gap = value.round().max(0.0) as u32;
            if rows {
                spaced.set_row_spacing(gap);
            }
            spaced.set_column_spacing(gap);
        });
    }
    if let Some(gap) = &props.row_gap {
        let spaced = flow.clone();
        bind_number(ctx, gap, props.animate, &flow, move |value| {
            spaced.set_row_spacing(value.round().max(0.0) as u32);
        });
    }
    pad(ctx, flow.upcast_ref(), &props.padding, props.animate);
    let first = props
        .min_width
        .as_ref()
        .map(|width| width.peek(ctx))
        .unwrap_or(120.0);
    let cells = Parent::of_flow(&flow, Rc::new(Cell::new(first.round() as i32)));
    // A cell is never wider than the grid has room for: with less room
    // than `min_width`, one column fills it, as on the other targets. A
    // probe with no height above the cells measures the room; until it
    // has, and while the room is short, the cells ask for no width, so
    // the probe measures the room the grid is given, not the width its
    // cells would force.
    let wanted = Rc::new(Cell::new(first));
    let room = Rc::new(Cell::new(0.0));
    let fit = {
        let (cells, wanted, room, flow) = (
            cells.clone(),
            Rc::clone(&wanted),
            Rc::clone(&room),
            flow.clone(),
        );
        Rc::new(move || {
            let inside = room.get() - f64::from(flow.margin_start() + flow.margin_end());
            if wanted.get() > inside {
                flow.set_max_children_per_line(1);
                cells.set_cell_width(-1);
            } else {
                flow.set_max_children_per_line(64);
                cells.set_cell_width(wanted.get().round().max(1.0) as i32);
            }
        })
    };
    fit();
    if let Some(min_width) = &props.min_width {
        let (wanted, fit) = (Rc::clone(&wanted), Rc::clone(&fit));
        bind_number(ctx, min_width, props.animate, &flow, move |value| {
            wanted.set(value);
            fit();
        });
    }
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_hexpand(true);
    holder.set_valign(gtk::Align::Start);
    let probe = gtk::DrawingArea::new();
    probe.set_content_height(0);
    probe.set_hexpand(true);
    probe.connect_resize(move |_, width, _| {
        if width > 0 && f64::from(width) != room.get() {
            room.set(f64::from(width));
            // After this layout, which a new size request would upset.
            let fit = Rc::clone(&fit);
            glib::idle_add_local_once(move || fit());
        }
    });
    holder.append(&probe);
    holder.append(&flow);
    children(&cells, ctx);
    place(p, &holder);
}

#[derive(Default)]
pub struct LayersProps {
    pub align: Option<&'static str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub animate: Option<Curve>,
    /// The surface a style gives the stack.
    pub style: Option<&'static crate::style::StyleDef>,
    /// While false, nothing in the stack takes input.
    pub enabled: Option<Get>,
    /// Layers slide to new places with this spring.
    pub reflow: Option<Curve>,
    /// `under_bars`: while shown, the stack fills the window up to its top
    /// edge, under the header bar.
    pub under_bars: bool,
}

/// `Stack`: layers in one place, later ones above earlier ones, the stack
/// as large as its largest layer.
#[allow(non_snake_case)]
pub fn Layers(p: &Parent, ctx: &Ctx, props: LayersProps, children: Build) {
    use gtk::Align::{Center, End, Start};
    let (valign, halign) = match props.align.unwrap_or("center") {
        "top_leading" => (Start, Start),
        "top" => (Start, Center),
        "top_trailing" => (Start, End),
        "leading" => (Center, Start),
        "trailing" => (Center, End),
        "bottom_leading" => (End, Start),
        "bottom" => (End, Center),
        "bottom_trailing" => (End, End),
        _ => (Center, Center),
    };
    let layers = crate::layout::Layers::new();
    layers.set_reflow(props.reflow);
    layers.set_block(!p.is_horizontal() && props.width.is_none());
    bind_size(
        ctx,
        layers.upcast_ref(),
        &props.width,
        &props.height,
        props.animate,
    );
    if let Some(style) = props.style {
        crate::style::surface(style, &layers);
    }
    if let Some(enabled) = bound_enabled(ctx, &props.enabled) {
        // Disabled content keeps its look: it only stops taking input and
        // leaves the accessibility tree.
        let shown = layers.clone();
        effect(move || {
            let on = enabled();
            shown.set_can_target(on);
            shown.set_can_focus(on);
            shown.update_state(&[gtk::accessible::State::Hidden(!on)]);
        });
    }
    if props.under_bars {
        under_bars(layers.upcast_ref());
    } else if p.is_window() && props.width.is_none() && props.height.is_none() {
        // Written directly in a window, a stack without a size fills it and
        // places its smaller layers there by its alignment.
        layers.set_hexpand(true);
        layers.set_vexpand(true);
    }
    children(&Parent::of_layers(&layers, halign, valign), ctx);
    place(p, &layers);
}

/// While `widget` is shown it fills the window, and the window's toolbar
/// view lays the content up to its top edge, under the header bar.
fn under_bars(widget: &gtk::Widget) {
    widget.set_hexpand(true);
    widget.set_vexpand(true);
    let extend = |widget: &gtk::Widget, on: bool| {
        if let Some(view) = widget
            .ancestor(adw::ToolbarView::static_type())
            .and_then(|view| view.downcast::<adw::ToolbarView>().ok())
        {
            view.set_extend_content_to_top_edge(on);
        }
    };
    widget.connect_map(move |widget| extend(widget, true));
    widget.connect_unmap(move |widget| extend(widget, false));
}

#[derive(Default)]
pub struct ImageProps {
    /// Name of the declared resource.
    pub source: &'static str,
    /// The declared resource shown in place of `source` where the picture
    /// is dark: in a dark part, or a dark window.
    pub dark: Option<&'static str>,
    /// Its sides never change: it is double-sampled.
    pub one_size: bool,
    pub label: Option<Str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub fit: Option<&'static str>,
    pub animate: Option<Curve>,
    /// Rounded corners of this radius.
    pub corner: Option<Num>,
    /// Drawn in the foreground color, as a symbol: only the picture's
    /// shape shows.
    pub template: bool,
    /// Width over height, in place of the picture's own.
    pub aspect: Option<f64>,
    /// Upper bound in points on the longer side needed when decoded.
    pub decode_max: Option<f64>,
    /// Color saturation from 0 (grayscale) to 1 (full color).
    pub saturation: Option<Num>,
    /// Optional placeholder built while a picture loads.
    pub placeholder: Option<Build>,
    /// Static blur radius (>= 4) baked into the decoded bitmap.
    pub blur: f64,
}

/// The content fit of `fit`.
pub(crate) fn content_fit(fit: Option<&str>) -> gtk::ContentFit {
    match fit {
        Some("cover") => gtk::ContentFit::Cover,
        Some("fill") => gtk::ContentFit::Fill,
        _ => gtk::ContentFit::Contain,
    }
}

/// How a picture is sized and shaped.
pub(crate) struct Sizing<'a> {
    pub(crate) width: &'a Option<Num>,
    pub(crate) height: &'a Option<Num>,
    pub(crate) animate: Option<Curve>,
    pub(crate) corner: &'a Option<Num>,
    pub(crate) aspect: Option<f64>,
    pub(crate) saturation: &'a Option<Num>,
}

/// A picture of a declared size and shape, holding `picture`: with one
/// side given, the other follows `ratio`; with only `aspect`, as wide as
/// its container and as tall as the aspect makes it; with no size, as
/// large as its container (it covers it). Returns the widget to place and
/// what lays it out again when the ratio changes.
pub(crate) fn framed(
    ctx: &Ctx,
    picture: &gtk::Widget,
    sizing: Sizing<'_>,
    ratio: Rc<Cell<f64>>,
) -> (gtk::Widget, crate::picture::Frame, Rc<dyn Fn()>) {
    let Sizing {
        width,
        height,
        animate,
        corner,
        aspect,
        saturation,
    } = sizing;
    let frame = crate::picture::Frame::new(picture, 0, 0);
    if let Some(saturation) = saturation {
        let shown = frame.clone();
        bind_number(ctx, saturation, animate, &frame, move |value| {
            shown.set_saturation(value);
        });
    }
    let sides: Rc<Cell<(Option<f64>, Option<f64>)>> = Rc::new(Cell::new((None, None)));
    let resize: Rc<dyn Fn()> = {
        let frame = frame.clone();
        let sides = Rc::clone(&sides);
        let ratio = Rc::clone(&ratio);
        Rc::new(move || {
            let ratio = ratio.get();
            let (width, height) = sides.get();
            let w = width.or(height.map(|height| height * ratio)).unwrap_or(0.0);
            let h = height.or(width.map(|width| width / ratio)).unwrap_or(0.0);
            frame.set_size(w.round() as i32, h.round() as i32);
        })
    };
    match (width, height, aspect) {
        (None, None, Some(aspect)) => frame.fill_width(aspect),
        (None, None, None) => frame.cover(),
        _ => {
            if let Some(width) = width {
                let sides = Rc::clone(&sides);
                let resize = Rc::clone(&resize);
                bind_number(ctx, width, animate, &frame, move |value| {
                    sides.set((Some(value), sides.get().1));
                    resize();
                });
            }
            if let Some(height) = height {
                let sides = Rc::clone(&sides);
                let resize = Rc::clone(&resize);
                bind_number(ctx, height, animate, &frame, move |value| {
                    sides.set((sides.get().0, Some(value)));
                    resize();
                });
            }
        }
    }
    let outer: gtk::Widget = match corner {
        Some(corner) => crate::features::rounded(ctx, &frame, corner, animate).upcast(),
        None => frame.clone().upcast(),
    };
    (outer, frame, resize)
}

/// A picture from a declared resource. With one side given, the other
/// follows the picture's aspect ratio. Without a label it is decorative.
#[allow(non_snake_case)]
pub fn Image(p: &Parent, ctx: &Ctx, props: ImageProps) {
    let _ = &props.placeholder;
    let resources = std::path::PathBuf::from(crate::host::resources_dir());
    let light = resources.join(props.source);
    let dark = props.dark.map(|dark| resources.join(dark));
    // The window's appearance until the picture is placed in its part.
    let path = match &dark {
        Some(dark) if crate::theme::is_dark() => dark.clone(),
        _ => light.clone(),
    };
    let fit = content_fit(props.fit);
    let blur = props.blur;
    let sides = (props.one_size || props.decode_max.is_some() || blur >= 0.2).then(|| {
        let side = |num: &Option<Num>| match (num.as_ref().map(|n| n.peek(ctx)), props.decode_max) {
            (Some(s), Some(m)) => Some(s.min(m)),
            (Some(s), None) if props.one_size => Some(s),
            (None, Some(m)) => Some(m),
            _ => None,
        };
        (side(&props.width), side(&props.height))
    });
    let fixed_display = (
        props.width.as_ref().map(|n| n.peek(ctx)),
        props.height.as_ref().map(|n| n.peek(ctx)),
    );
    let initial_pixels = (blur >= 0.2)
        .then(|| sides.and_then(|(w, h)| twice_pixels(&path, w, h, fit, 1.0)))
        .flatten();
    let texture = read_picture(&path, initial_pixels, blur, fixed_display);
    let (natural_width, natural_height) = texture
        .as_ref()
        .map(|paintable| (paintable.intrinsic_width(), paintable.intrinsic_height()))
        .unwrap_or((0, 0));
    let ratio = props
        .aspect
        .unwrap_or(if natural_width > 0 && natural_height > 0 {
            f64::from(natural_width) / f64::from(natural_height)
        } else {
            1.0
        });
    let picture: gtk::Widget = if props.template {
        let template = crate::picture::Template::new(fit);
        template.set_paintable(texture.as_ref());
        template.upcast()
    } else {
        let picture = match &texture {
            Some(texture) => gtk::Picture::for_paintable(texture),
            None => gtk::Picture::for_filename(&path),
        };
        picture.set_can_shrink(true);
        picture.set_content_fit(fit);
        picture.upcast()
    };
    match &props.label {
        Some(label) => {
            let text = bound_text(ctx, label);
            let described = picture.clone();
            effect(move || match described.downcast_ref::<gtk::Picture>() {
                Some(picture) => picture.set_alternative_text(Some(&text())),
                None => described.update_property(&[gtk::accessible::Property::Label(&text())]),
            });
        }
        None => picture.update_state(&[gtk::accessible::State::Hidden(true)]),
    }
    if dark.is_some() || props.one_size || props.decode_max.is_some() || blur >= 0.2 {
        follow_picture(&picture, path, light, dark, sides, fit, blur, fixed_display);
    }
    let (outer, _, _) = framed(
        ctx,
        &picture,
        Sizing {
            width: &props.width,
            height: &props.height,
            animate: props.animate,
            corner: &props.corner,
            aspect: props.aspect,
            saturation: &props.saturation,
        },
        Rc::new(Cell::new(ratio)),
    );
    place(p, &outer);
}

/// The declared picture at `path`: in HDR where GTK presents it, upright,
/// and with `pixels`, its longer side that many pixels, drawn down by
/// gdk-pixbuf, which averages the pixels each one covers.
fn read_picture(
    path: &std::path::Path,
    pixels: Option<i32>,
    blur: f64,
    display: (Option<f64>, Option<f64>),
) -> Option<gtk::gdk::Paintable> {
    if blur >= 0.2 {
        let pixbuf = match pixels {
            Some(pixels) => gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, pixels, pixels, true),
            None => gtk::gdk_pixbuf::Pixbuf::from_file(path),
        }
        .ok()?;
        let upright = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
        let sigma = crate::features::blur_sigma(
            blur,
            upright.width(),
            upright.height(),
            display.0,
            display.1,
        );
        return crate::features::premultiplied_texture_blurred(&upright, sigma)
            .map(|texture| texture.upcast());
    }
    if let Some(texture) = crate::hdr::texture(path) {
        return Some(texture.upcast());
    }
    if let Some(pixels) = pixels
        && let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, pixels, pixels, true)
    {
        let upright = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
        return crate::features::premultiplied_texture(&upright).map(|texture| texture.upcast());
    }
    oriented(path).map(|texture| texture.upcast()).or_else(|| {
        gtk::gdk::Texture::from_filename(path)
            .ok()
            .map(|texture| texture.upcast())
    })
}

/// Keep in `picture` (a `gtk::Picture` or a template) the declared picture
/// for where it is, starting from `shown`: `dark` in a dark part or window,
/// else `light`; and with `sides` that never change (points, `None` for a
/// side that follows the picture) double-sampled once it is placed:
/// decoded at twice the pixels it covers, which GTK halves when it draws
/// it. Squeezed down from a much larger picture by GTK alone, its edges
/// would break up.
fn follow_picture(
    picture: &gtk::Widget,
    shown: std::path::PathBuf,
    light: std::path::PathBuf,
    dark: Option<std::path::PathBuf>,
    sides: Option<(Option<f64>, Option<f64>)>,
    fit: gtk::ContentFit,
    blur: f64,
    display: (Option<f64>, Option<f64>),
) {
    // Placed in a window, or on a display of another scale.
    let placed = crate::reactive::Version::new();
    let bump = placed.clone();
    picture.connect_map(move |_| bump.bump());
    let bump = placed.clone();
    picture.connect_scale_factor_notify(move |_| bump.bump());
    let current = Rc::new(std::cell::RefCell::new((shown, None::<i32>)));
    let weak = picture.downgrade();
    effect(move || {
        crate::theme::track();
        placed.track();
        let Some(widget) = weak.upgrade() else {
            return;
        };
        let path = match &dark {
            Some(dark) if crate::theme::part_of(&widget).unwrap_or_else(crate::theme::is_dark) => {
                dark
            }
            _ => &light,
        };
        let scale = widget
            .native()
            .and_then(|native| native.surface())
            .map(|surface| surface.scale())
            .unwrap_or_else(|| f64::from(widget.scale_factor()));
        let pixels = sides
            .filter(|_| widget.is_mapped())
            .and_then(|(width, height)| twice_pixels(path, width, height, fit, scale));
        let next = (path.clone(), pixels);
        if *current.borrow() == next {
            return;
        }
        let texture = read_picture(path, pixels, blur, display);
        current.replace(next);
        if let Some(template) = widget.downcast_ref::<crate::picture::Template>() {
            template.set_paintable(texture.as_ref());
        } else if let Some(picture) = widget.downcast_ref::<gtk::Picture>() {
            picture.set_paintable(texture.as_ref());
        }
    });
}

/// The longer side, in pixels, of the picture at `path` decoded at twice
/// the pixels it covers shown `width` × `height` points (`None` for a side
/// that follows the picture) as `fit` places it, at `scale` pixels per
/// point; `None` when it has no more pixels than that.
fn twice_pixels(
    path: &std::path::Path,
    width: Option<f64>,
    height: Option<f64>,
    fit: gtk::ContentFit,
    scale: f64,
) -> Option<i32> {
    let (_, w, h) = gtk::gdk_pixbuf::Pixbuf::file_info(path)?;
    let (w, h) = (f64::from(w), f64::from(h));
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (shown_width, shown_height) = match (width, height) {
        (Some(width), Some(height)) => match fit {
            gtk::ContentFit::Fill => (width, height),
            _ => {
                let (fx, fy) = (width / w, height / h);
                let s = if fit == gtk::ContentFit::Cover {
                    fx.max(fy)
                } else {
                    fx.min(fy)
                };
                (w * s, h * s)
            }
        },
        (Some(width), None) => (width, width * h / w),
        (None, Some(height)) => (height * w / h, height),
        (None, None) => return None,
    };
    let factor = (2.0 * shown_width * scale / w).max(2.0 * shown_height * scale / h);
    (factor < 1.0).then(|| (w.max(h) * factor).ceil().max(1.0) as i32)
}

/// A JPEG turned upright by its EXIF orientation, which GTK's own loader
/// ignores. gdk-pixbuf reads it; for JPEG it decodes the same 8-bit
/// picture GTK would. Other formats are left to GTK's loaders.
fn oriented(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if extension != "jpg" && extension != "jpeg" {
        return None;
    }
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    crate::features::premultiplied_texture(&pixbuf.apply_embedded_orientation().unwrap_or(pixbuf))
}

#[derive(Default)]
pub struct ProgressProps {
    pub value: Option<Get>,
    pub total: Option<f64>,
    pub label: Option<Str>,
    /// `style: ring`, `width` across and `stroke_width` thick.
    pub ring: bool,
    pub width: Option<f64>,
    pub stroke_width: Option<f64>,
}

/// A progress bar of `value` out of `total` under its label, or without a
/// value a spinner beside it.
#[allow(non_snake_case)]
pub fn Progress(p: &Parent, ctx: &Ctx, props: ProgressProps) {
    if props.ring
        && let Some(value) = props.value.clone()
    {
        return progress_ring(p, ctx, value, &props);
    }
    let label = props.label.as_ref().map(|text| {
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_wrap(true);
        bind_label(&label, bound_text(ctx, text));
        label
    });
    let container = match props.value {
        Some(value) => {
            let total = props.total.unwrap_or(1.0);
            let bar = gtk::ProgressBar::new();
            let shown = bar.clone();
            let ctx = ctx.clone();
            effect(move || {
                let current = as_f64(&value(&ctx));
                shown.set_fraction((current / total).clamp(0.0, 1.0));
            });
            let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
            column.set_hexpand(true);
            if let Some(label) = &label {
                bar.update_relation(&[gtk::accessible::Relation::LabelledBy(
                    &[label.upcast_ref()],
                )]);
                column.append(label);
            }
            column.append(&bar);
            column
        }
        None => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let spinner = gtk::Spinner::new();
            spinner.start();
            row.append(&spinner);
            if let Some(label) = &label {
                label.add_css_class("dim-label");
                spinner.update_relation(&[gtk::accessible::Relation::LabelledBy(&[
                    label.upcast_ref()
                ])]);
                row.append(label);
            }
            row
        }
    };
    place(p, &container);
}

/// `Progress(style: ring)`: an arc in the accent from the top, clockwise,
/// with round ends; at nothing yet, a dot at the top. It lies over what it
/// rings and takes no input.
fn progress_ring(p: &Parent, ctx: &Ctx, value: Get, props: &ProgressProps) {
    let size = props.width.unwrap_or(24.0);
    let stroke = props.stroke_width.unwrap_or(3.0);
    let total = props.total.unwrap_or(1.0);
    let area: gtk::DrawingArea = gtk::glib::Object::builder()
        .property("accessible-role", gtk::AccessibleRole::ProgressBar)
        .build();
    area.set_content_width(size.round() as i32);
    area.set_content_height(size.round() as i32);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.set_can_target(false);
    let fraction = Rc::new(Cell::new(0.0_f64));
    let drawn = Rc::clone(&fraction);
    area.set_draw_func(move |area, cr, width, height| {
        let color = crate::theme::rgba(&crate::theme::Color::Role("accent"), area.upcast_ref());
        cr.set_source_rgba(
            color.red().into(),
            color.green().into(),
            color.blue().into(),
            color.alpha().into(),
        );
        cr.set_line_width(stroke);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        let (w, h) = (f64::from(width), f64::from(height));
        let start = -std::f64::consts::FRAC_PI_2;
        let sweep = drawn.get().max(0.0001) * std::f64::consts::TAU;
        cr.arc(
            w / 2.0,
            h / 2.0,
            (w.min(h) - stroke) / 2.0,
            start,
            start + sweep,
        );
        let _ = cr.stroke();
    });
    let shown = area.clone();
    let at = ctx.clone();
    effect(move || {
        crate::theme::track();
        let current = if total > 0.0 {
            (as_f64(&value(&at)) / total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        fraction.set(current);
        shown.update_property(&[
            gtk::accessible::Property::ValueMin(0.0),
            gtk::accessible::Property::ValueMax(1.0),
            gtk::accessible::Property::ValueNow(current),
        ]);
        shown.queue_draw();
    });
    if let Some(label) = props.label.as_ref() {
        let label = bound_text(ctx, label);
        let named = area.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
    place(p, &area);
}

#[allow(non_snake_case)]
pub fn Spacer(p: &Parent) {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.add_css_class(crate::scene::SPACER);
    // Empty room: what lies under it takes the press.
    spacer.set_can_target(false);
    if p.is_horizontal() {
        spacer.set_hexpand(true);
    } else {
        spacer.set_vexpand(true);
    }
    p.add(&spacer);
}

#[allow(non_snake_case)]
pub fn Divider(p: &Parent) {
    let orientation = if p.is_horizontal() {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    };
    let line = gtk::Separator::new(orientation);
    // A line across what holds it, in a column or a stack alike.
    if !p.is_horizontal() {
        line.set_hexpand(true);
    }
    place(p, &line);
}

#[derive(Default)]
pub struct FormProps {
    pub title: Option<Str>,
    /// The space between sections.
    pub gap: Option<f64>,
    /// The least height of a row.
    pub row_height: Option<f64>,
    /// The padding of every row across and down, unless its section says.
    pub padding_x: Option<f64>,
    pub padding_y: Option<f64>,
}

/// The layout a form gives the rows of its sections.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowLayout {
    pub row_height: Option<f64>,
    pub padding_x: Option<f64>,
    pub padding_y: Option<f64>,
}

impl RowLayout {
    /// The rules that lay out the rows of a list carrying their class: a
    /// plain row's padding, a libadwaita row's header, the least height.
    fn class(&self) -> Option<String> {
        if *self == RowLayout::default() {
            return None;
        }
        let mut rules = String::new();
        let padding = match (self.padding_y, self.padding_x) {
            (None, None) => None,
            (y, x) => Some(format!(
                "padding: {}px {}px;",
                y.unwrap_or(10.0),
                x.unwrap_or(12.0)
            )),
        };
        if let Some(padding) = padding {
            rules.push_str(&format!(
                "& > row.bern-plain-row {{ {padding} }} & > row > box.header {{ {padding} }} "
            ));
        }
        if let Some(height) = self.row_height {
            rules.push_str(&format!(
                "& > row {{ min-height: {height}px; }} & > row > box.header {{ min-height: {height}px; }} "
            ));
        }
        Some(crate::style::class_with(&rules))
    }
}

/// A grouped form: sections of boxed lists in a clamped, scrolling page,
/// as libadwaita preference pages lay them out. `gap` spaces the sections;
/// a form that opens with a plain section starts at the top of the page.
#[allow(non_snake_case)]
pub fn Form(p: &Parent, ctx: &Ctx, props: FormProps, children: Build) {
    if let (Some(title), Some(page)) = (&props.title, ctx.page()) {
        page.push_title(bound_text(ctx, title));
    }
    let content = gtk::Box::new(
        gtk::Orientation::Vertical,
        props
            .gap
            .map(|gap| gap.round().max(0.0) as i32)
            .unwrap_or(24),
    );
    // The room above the first section, unless that is a plain one: a
    // rule on the first child, so it follows a heading a condition gives
    // or takes away later.
    content.add_css_class("bern-form-rows");
    content.set_margin_bottom(24);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(680);
    clamp.set_tightening_threshold(480);
    clamp.set_child(Some(&content));
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    // A form takes the room it is given both ways, as on the other
    // targets; the clamp keeps its rows readable.
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_child(Some(&clamp));
    crate::features::natural_scrolling(&scrolled, false);
    let layout = RowLayout {
        row_height: props.row_height,
        padding_x: props.padding_x,
        padding_y: props.padding_y,
    };
    children(&Parent::of_box(&content), &ctx.with_form(Rc::new(layout)));
    p.add(&scrolled);
}

#[derive(Default)]
pub struct SectionProps {
    pub title: Option<Str>,
    pub footer: Option<Str>,
    /// Rows without a background, padding or separators, as a heading
    /// that scrolls with the form.
    pub plain: bool,
    /// The padding of the section's rows, in place of the form's.
    pub padding_x: Option<f64>,
    pub padding_y: Option<f64>,
}

/// A titled group of rows with an optional footnote.
#[allow(non_snake_case)]
pub fn Section(p: &Parent, ctx: &Ctx, props: SectionProps, children: Build) {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if let Some(title) = &props.title {
        let heading = gtk::Label::new(None);
        heading.set_xalign(0.0);
        heading.set_wrap(true);
        heading.add_css_class("heading");
        heading.set_margin_bottom(6);
        bind_label(&heading, bound_text(ctx, title));
        group.append(&heading);
    }
    let list = boxed_list();
    // A plain section's rows keep the page's layout, whatever the form
    // gives its groups: only its own insets apply.
    let form = ctx
        .form()
        .filter(|_| !props.plain)
        .map(|form| **form)
        .unwrap_or_default();
    let layout = RowLayout {
        row_height: form.row_height,
        padding_x: props.padding_x.or(form.padding_x),
        padding_y: props.padding_y.or(form.padding_y),
    };
    if props.plain {
        list.remove_css_class("boxed-list");
        list.add_css_class("bern-plain-list");
        group.add_css_class("bern-plain-section");
    }
    if let Some(class) = layout.class() {
        list.add_css_class(&class);
    }
    group.append(&list);
    if let Some(footer) = &props.footer {
        let note = gtk::Label::new(None);
        note.set_xalign(0.0);
        note.set_wrap(true);
        note.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        note.add_css_class("dim-label");
        note.add_css_class("caption");
        note.set_margin_top(6);
        bind_label(&note, bound_text(ctx, footer));
        group.append(&note);
    }
    children(&Parent::of_list(&list), ctx);
    p.add(&group);
}

/// Toolbar items for the enclosing page's header.
#[allow(non_snake_case)]
pub fn Toolbar(p: &Parent, ctx: &Ctx, children: Build) {
    tools(p, ctx, children, false);
}

/// Toolbar items at the leading end of the page's header, after the way
/// back: those before a toolbar's `Spacer`.
#[allow(non_snake_case)]
pub fn LeadingToolbar(p: &Parent, ctx: &Ctx, children: Build) {
    tools(p, ctx, children, true);
}

fn tools(_p: &Parent, ctx: &Ctx, children: Build, leading: bool) {
    let Some(page) = ctx.page().cloned() else {
        return;
    };
    let holder = page.add_tools(leading);
    holder.set_valign(gtk::Align::Center);
    let shown = holder.clone();
    let at = ctx.clone();
    effect(move || shown.set_visible(at.shown()));
    children(
        &Parent::of_aligned_box(&holder, gtk::Align::Center),
        &ctx.toolbar(),
    );
}

#[derive(Default)]
pub struct LanguagePickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    /// In a list, the menu alone, without the row's title.
    pub label_hidden: bool,
}

#[allow(non_snake_case)]
pub fn LanguagePicker(p: &Parent, ctx: &Ctx, props: LanguagePickerProps) {
    let supported: Rc<Vec<String>> = Rc::new(text::with(|language| language.supported.clone()));
    let initial_system = text::tr("bern.language.system");
    let mut names: Vec<String> = Vec::with_capacity(supported.len() + 1);
    names.push(initial_system);
    names.extend(supported.iter().map(|tag| text::autonym(tag)));
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&name_refs);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let (owner, widget): (glib::Object, gtk::Widget) = if p.is_list() && !props.label_hidden {
        let row = adw::ComboRow::new();
        row.set_model(Some(&model));
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&label()));
        }
        (row.clone().upcast(), row.upcast())
    } else {
        let dropdown = gtk::DropDown::new(Some(model.clone()), None::<gtk::Expression>);
        (dropdown.clone().upcast(), dropdown.upcast())
    };
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = owner.clone();
    let tags = Rc::clone(&supported);
    let list_model = model.clone();
    effect(move || {
        let current = text::language();
        let system_label = text::tr("bern.language.system");
        let follows = text::follows_system();
        let target = if follows {
            Some(0u32)
        } else {
            tags.iter()
                .position(|tag| *tag == current)
                .map(|i| (i + 1) as u32)
        };
        guard.set(true);
        if list_model.string(0).as_deref() != Some(system_label.as_str()) {
            list_model.splice(0, 1, &[system_label.as_str()]);
        }
        if let Some(index) = target
            && shown.property::<u32>("selected") != index
        {
            shown.set_property("selected", index);
        }
        guard.set(false);
    });
    let tags = Rc::clone(&supported);
    owner.connect_notify_local(Some("selected"), move |owner, _| {
        if updating.get() {
            return;
        }
        let index = owner.property::<u32>("selected") as usize;
        let tag = if index == 0 {
            Some("system".to_string())
        } else {
            tags.get(index - 1).cloned()
        };
        if let Some(tag) = tag {
            // Text bindings update after the selection change finishes.
            glib::idle_add_local_once(move || {
                text::set_language(&tag);
            });
        }
    });
    if p.is_list() {
        p.add(&widget);
    } else {
        p.add(&beside(label.clone(), &widget));
    }
    controls::register(
        ctx,
        props.id,
        "picker",
        label,
        None,
        Actions {
            select: Some(Rc::new(|tag: &str| text::set_language(tag))),
            value: Some(Rc::new(|| {
                Some(text::with(|language| language.current.peek()))
            })),
            ..Actions::default()
        },
    );
}

/// An item of a menu; `None` entries are dividers.
pub struct MenuItem {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub role: Option<&'static str>,
    pub enabled: Option<Get>,
    pub checked: Option<Get>,
    pub action: Option<Act>,
    pub pick: Option<crate::features::Pick>,
    /// `paste:`: the clipboard's text goes into this draft before the
    /// action runs.
    pub paste: Option<crate::ctx::Field>,
}

#[derive(Default)]
pub struct MenuProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    /// `style: icon`: a round button showing only the icon, the label as
    /// its tooltip and accessible name; without an icon, shows its text
    /// trigger without a dropdown arrow.
    pub icon_only: bool,
    pub hide_disabled: bool,
    pub shape: Shape,
    /// Preferred side (`above` / `below`) for the popup menu.
    pub placement: Option<&'static str>,
    /// `bold: true`: bold trigger label.
    pub bold: bool,
}

/// A menu button. Each item is registered on the button, so external
/// entries reach the items while the menu is closed.
#[allow(non_snake_case)]
pub fn Menu(p: &Parent, ctx: &Ctx, props: MenuProps, items: Vec<Option<MenuItem>>) {
    let button = gtk::MenuButton::new();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if props.bold {
        button.add_css_class("bern-bold");
    }
    match props.placement {
        Some("above") => button.set_direction(gtk::ArrowType::Up),
        Some("below") => button.set_direction(gtk::ArrowType::Down),
        _ => {}
    }
    if props.icon_only {
        if let Some(icon) = props.icon {
            button.set_icon_name(icons::name(icon));
            button.add_css_class("flat");
            button.add_css_class("circular");
            if let Some(label) = label.clone() {
                let named = button.clone();
                effect(move || {
                    let text = label();
                    named.set_tooltip_text(Some(&text));
                    named.update_property(&[gtk::accessible::Property::Label(&text)]);
                });
            }
        } else {
            button.add_css_class("flat");
            button.set_always_show_arrow(false);
            if let Some(label) = label.clone() {
                let shown = button.clone();
                effect(move || shown.set_label(&label()));
            }
        }
    } else {
        match (props.icon, label.clone()) {
            (Some(icon), Some(label)) => {
                let content = adw::ButtonContent::new();
                content.set_icon_name(icons::name(icon));
                let shown = content.clone();
                effect(move || shown.set_label(&label()));
                button.set_child(Some(&content));
            }
            (Some(icon), None) => button.set_icon_name(icons::name(icon)),
            (None, Some(label)) => {
                let shown = button.clone();
                effect(move || shown.set_label(&label()));
            }
            (None, None) => {}
        }
    }
    shape(ctx, button.upcast_ref(), &props.shape);
    button.set_valign(gtk::Align::Center);
    let group = gio::SimpleActionGroup::new();
    let mut entries = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Some(item) = item else {
            entries.push(None);
            continue;
        };
        let name = format!("i{index}");
        let checked = bound_enabled(ctx, &item.checked);
        let action = match &checked {
            Some(_) => gio::SimpleAction::new_stateful(&name, None, &false.to_variant()),
            None => gio::SimpleAction::new(&name, None),
        };
        let (run, set_file) =
            crate::features::picking(ctx, item.pick.clone(), bound_action(ctx, &item.action));
        let run = crate::features::pasting(ctx, item.paste, run);
        if let Some(checked) = checked.clone() {
            let shown = action.clone();
            let check_after = Rc::clone(&checked);
            effect(move || shown.set_state(&checked().to_variant()));
            let run_act = run.clone();
            action.connect_activate(move |act, _| {
                if let Some(run) = &run_act {
                    run();
                }
                act.set_state(&crate::reactive::untrack(&*check_after).to_variant());
            });
        } else if let Some(run) = run.clone() {
            action.connect_activate(move |_, _| run());
        }
        let enabled = bound_enabled(ctx, &item.enabled);
        if let Some(enabled) = enabled.clone() {
            let shown = action.clone();
            effect(move || shown.set_enabled(enabled()));
        }
        group.add_action(&action);
        let item_label = item
            .label
            .as_ref()
            .map(|label| bound_text(ctx, label))
            .unwrap_or_else(|| Rc::new(String::new));
        let value = checked.map(|checked| {
            Rc::new(move || {
                Some(
                    if crate::reactive::untrack(&*checked) {
                        "checked"
                    } else {
                        "unchecked"
                    }
                    .to_string(),
                )
            }) as Rc<dyn Fn() -> Option<String>>
        });
        controls::register(
            ctx,
            item.id,
            "button",
            Some(Rc::clone(&item_label)),
            enabled.clone(),
            Actions {
                activate: run,
                set_file,
                file_kinds: item.pick.map(|pick| pick.kinds),
                value,
                ..Actions::default()
            },
        );
        let _ = (item.icon, item.role);
        entries.push(Some((format!("menu.{name}"), item_label, enabled)));
    }
    button.insert_action_group("menu", Some(&group));
    let shown = button.clone();
    effect(move || {
        let menu = gio::Menu::new();
        let mut section = gio::Menu::new();
        for entry in &entries {
            match entry {
                Some((action, label, enabled)) => {
                    if !props.hide_disabled || enabled.as_ref().is_none_or(|enabled| enabled()) {
                        section.append(Some(&label()), Some(action));
                    }
                }
                None => {
                    menu.append_section(None, &section);
                    section = gio::Menu::new();
                }
            }
        }
        menu.append_section(None, &section);
        shown.set_menu_model(Some(&menu));
    });
    let open = button.clone();
    controls::register(
        ctx,
        props.id,
        "menu",
        label,
        None,
        Actions {
            value: Some(Rc::new(move || {
                Some(if open.is_active() { "open" } else { "closed" }.to_string())
            })),
            ..Actions::default()
        },
    );
    keeps_width(&button);
    place(p, &button);
}

/// Text of a value for `Text(content: …)` with runtime data: integers,
/// floats and texts as Bern formats them, enums by their label.
pub fn display(value: &Value, enum_name: Option<&str>) -> String {
    match (value, enum_name) {
        (Value::Enum { variant, fields }, Some(name)) => label_of(name, *variant, fields),
        (value, _) => crate::value::plain(value),
    }
}

/// The localized label of a variant without fields.
pub fn label(enum_name: &str, variant: u32) -> String {
    label_of(enum_name, variant, &[])
}

/// The localized label of an enum value. A label that shows the
/// variant's fields writes them as its text says.
pub fn label_of(enum_name: &str, variant: u32, fields: &[Value]) -> String {
    let host = crate::host::host();
    let Some(def) = host.def.types.enum_def(enum_name) else {
        return String::new();
    };
    let Some(variant) = def.variants.get(variant as usize) else {
        return String::new();
    };
    let key = crate::protocol::label_key(enum_name, &variant.name);
    let Some(label) = variant
        .label
        .as_ref()
        .filter(|label| !label.params.is_empty())
    else {
        return tr(&key);
    };
    let args: Vec<(&str, crate::protocol::format::Arg)> = label
        .params
        .iter()
        .filter_map(|param| {
            let at = variant
                .fields
                .iter()
                .position(|field| field.name == *param)?;
            let value = fields.get(at)?;
            let arg = match &variant.fields[at].ty {
                crate::protocol::Type::Record { name }
                    if name == crate::protocol::format::CALENDAR_DATE =>
                {
                    text::date(value)
                }
                crate::protocol::Type::Enum { name } => text::arg(value, Some(name)),
                _ => text::arg(value, None),
            };
            Some((param.as_str(), arg))
        })
        .collect();
    text::fmt(&key, &args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::tests::{app, start, unit_enum};

    #[test]
    fn ruby_ranges_use_scalars_and_leave_plain_text_for_empty_input() {
        assert_eq!(
            ruby_text_ranges("字𠮷文", r#"[[1,2,"よし"]]"#),
            vec![(3..7, "よし".into())]
        );
        for source in ["", "[]", "null", r#"[[2,1,"x"]]"#, r#"[[0,4,"x"]]"#] {
            assert!(ruby_text_ranges("字𠮷文", source).is_empty());
        }
        assert_eq!(
            ruby_text_ranges("文字", r#"[[0,1,"も"],[0,2,"じ"]]"#),
            vec![(0..3, "も".into())]
        );
    }

    #[test]
    fn a_column_without_align_is_a_leading_one() {
        assert_eq!(stack_cross(None, true), gtk::Align::Start);
        assert_eq!(stack_cross(Some("leading"), true), stack_cross(None, true));
        assert_eq!(stack_cross(Some("center"), true), gtk::Align::Center);
        assert_eq!(stack_cross(Some("trailing"), true), gtk::Align::End);
        // A row without `align` centers its items vertically.
        assert_eq!(stack_cross(None, false), gtk::Align::Center);
        assert_eq!(
            stack_cross(Some("baseline"), false),
            gtk::Align::BaselineCenter
        );
    }

    #[test]
    fn text_styles_map_to_libadwaita_classes() {
        for (style, class) in [
            (Some("large_title"), "title-1"),
            (Some("title"), "title-2"),
            (Some("title2"), "title-3"),
            (Some("title3"), "title-4"),
            (Some("headline"), "heading"),
            (Some("subheadline"), "heading"),
            (Some("footnote"), "caption"),
            (Some("caption2"), "caption"),
            (Some("code"), "monospace"),
            (Some("body"), "body"),
            (None, "body"),
        ] {
            assert_eq!(style_class(style), class, "{style:?}");
        }
    }

    #[test]
    fn values_show_as_text_or_by_their_enum_label() {
        let mut def = app();
        def.types.enums = vec![unit_enum("Mood", &["Calm", "Busy"])];
        def.strings = &[("en", &[("Mood.Busy", "Busy now")])];
        start(def);
        assert_eq!(display(&Value::unit_variant(1), Some("Mood")), "Busy now");
        // Without a translation, the label is its key.
        assert_eq!(display(&Value::unit_variant(0), Some("Mood")), "Mood.Calm");
        assert_eq!(display(&Value::Float(2.0), Some("Mood")), "2");
        assert_eq!(display(&Value::unit_variant(1), None), "");
        assert_eq!(label("Mood", 7), "");
        assert_eq!(label("Weather", 0), "");
    }
}
