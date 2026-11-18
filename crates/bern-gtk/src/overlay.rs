//! `bern::overlay` on Linux: an undecorated, transparent window per overlay
//! with a label per line, made on the GTK thread. While the pointer is over
//! it the bar of buttons shows and, unlocked, a plate; unlocked, dragging
//! it but by a button moves it. Locked, its input region holds only the
//! buttons that unlock it, so the pointer goes through to what lies under
//! the rest. GTK 4 asks the compositor for no place and no staying on top,
//! so where it stands and whether it stays above other windows is the
//! desktop's choice.

use bern_ffi::overlay::{
    Control, Host, Line, Overlay, Panel, Place, TextEffect, TextStyle, panel_changed,
    panel_dismissed,
};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, graphene, gsk};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) struct Overlays;

impl Host for Overlays {
    fn supports_panel(&self) -> bool {
        true
    }

    fn show(&self, id: &str, overlay: &Overlay) {
        self.show_styled(id, overlay, TextStyle::default());
    }

    fn show_styled(&self, id: &str, overlay: &Overlay, style: TextStyle) {
        let (id, overlay) = (id.to_string(), overlay.clone());
        glib::MainContext::default().invoke(move || show(&id, &overlay, style));
    }

    fn lit(&self, id: &str, overlay: &Overlay) {
        let (id, overlay) = (id.to_string(), overlay.clone());
        glib::MainContext::default().invoke(move || lit(&id, &overlay));
    }

    fn panel(&self, id: &str, panel: Option<&Panel>) {
        let (id, panel) = (id.to_string(), panel.cloned());
        glib::MainContext::default().invoke(move || {
            if let Some(parts) = WINDOWS.with(|windows| windows.borrow().get(&id).cloned()) {
                match panel {
                    Some(panel) if parts.window.is_visible() => parts.panel.show(&panel),
                    _ => parts.panel.close(),
                }
            }
        });
    }

    fn hide(&self, id: &str) {
        let id = id.to_string();
        glib::MainContext::default().invoke(move || {
            WINDOWS.with(|windows| {
                if let Some(parts) = windows.borrow().get(&id) {
                    parts.panel.close();
                    parts.window.set_visible(false);
                }
            });
        });
    }
}

/// An overlay's window, its bar and its lines.
#[derive(Clone)]
struct Parts {
    window: gtk::Window,
    bar: gtk::Box,
    lines: gtk::Box,
    panel: Rc<SettingsPanel>,
}

thread_local! {
    static WINDOWS: RefCell<HashMap<String, Parts>> = RefCell::new(HashMap::new());
}

const CSS: &str = "window.bern-overlay { background: transparent; }
window.bern-overlay box.bern-overlay-plate { padding: 12px; border-radius: 12px; }
window.bern-overlay.unlocked.over box.bern-overlay-plate { background: rgba(0, 0, 0, 0.3); }
window.bern-overlay.effect-none label { text-shadow: none; }
window.bern-overlay.effect-shadow label { text-shadow: 0 1px 3px rgba(0, 0, 0, 0.85); }
window.bern-overlay.effect-outline label { text-shadow: -1px -1px 0 rgba(0, 0, 0, 0.9), 1px -1px 0 rgba(0, 0, 0, 0.9), -1px 1px 0 rgba(0, 0, 0, 0.9), 1px 1px 0 rgba(0, 0, 0, 0.9), 0 -1.5px 0 rgba(0, 0, 0, 0.9), 0 1.5px 0 rgba(0, 0, 0, 0.9), -1.5px 0 0 rgba(0, 0, 0, 0.9), 1.5px 0 0 rgba(0, 0, 0, 0.9); }
window.bern-overlay box.bern-overlay-bar { opacity: 0; }
window.bern-overlay.over box.bern-overlay-bar { opacity: 1; }
window.bern-overlay button.bern-overlay-button { min-width: 28px; min-height: 28px; padding: 0; border-radius: 14px;
  background: rgba(0, 0, 0, 0.3); color: white; box-shadow: none; }
window.bern-overlay button.bern-overlay-button:hover { background: rgba(0, 0, 0, 0.55); }";

fn parts(id: &str) -> Parts {
    WINDOWS.with(|windows| {
        windows
            .borrow_mut()
            .entry(id.to_string())
            .or_insert_with(|| {
                if let Some(display) = gtk::gdk::Display::default() {
                    let provider = gtk::CssProvider::new();
                    provider.load_from_string(CSS);
                    gtk::style_context_add_provider_for_display(
                        &display,
                        &provider,
                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                    );
                }
                let window = gtk::Window::builder()
                    .decorated(false)
                    .resizable(false)
                    .focusable(false)
                    .title("")
                    .build();
                window.add_css_class("bern-overlay");
                window.add_css_class("effect-shadow");
                let plate = gtk::Box::new(gtk::Orientation::Vertical, 8);
                plate.add_css_class("bern-overlay-plate");
                let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                bar.add_css_class("bern-overlay-bar");
                bar.set_halign(gtk::Align::Center);
                let lines = gtk::Box::new(gtk::Orientation::Vertical, 2);
                plate.append(&bar);
                plate.append(&lines);
                window.set_child(Some(&plate));
                let panel = SettingsPanel::new(id, &window, &plate);
                // The bar and the plate show while the pointer is over it.
                let motion = gtk::EventControllerMotion::new();
                let entered = window.clone();
                motion.connect_enter(move |_, _, _| entered.add_css_class("over"));
                let left = window.clone();
                motion.connect_leave(move |_| left.remove_css_class("over"));
                window.add_controller(motion);
                // Unlocked, a press but on a button starts the compositor's
                // move.
                let press = gtk::GestureClick::new();
                let moved = window.clone();
                press.connect_pressed(move |gesture, _, x, y| {
                    if !moved.has_css_class("unlocked") {
                        return;
                    }
                    let picked = moved.pick(x, y, gtk::PickFlags::DEFAULT);
                    if picked.is_some_and(|widget| {
                        widget.is::<gtk::Button>()
                            || widget.ancestor(gtk::Button::static_type()).is_some()
                    }) {
                        return;
                    }
                    let Some(surface) = moved.surface() else {
                        return;
                    };
                    let Ok(toplevel) = surface.downcast::<gtk::gdk::Toplevel>() else {
                        return;
                    };
                    if let Some(device) = gesture.device() {
                        toplevel.begin_move(
                            &device,
                            gesture.current_button() as i32,
                            x,
                            y,
                            gesture.current_event_time(),
                        );
                    }
                });
                window.add_controller(press);
                // After each frame the input region follows the lock and
                // where the buttons are.
                let painted = window.downgrade();
                let barred = bar.downgrade();
                window.connect_realize(move |window| {
                    let Some(clock) = window.frame_clock() else {
                        return;
                    };
                    let (painted, barred) = (painted.clone(), barred.clone());
                    clock.connect_after_paint(move |_| {
                        if let (Some(window), Some(bar)) = (painted.upgrade(), barred.upgrade()) {
                            input_region(&window, &bar);
                        }
                    });
                });
                Parts {
                    window,
                    bar,
                    lines,
                    panel,
                }
            })
            .clone()
    })
}

/// Unlocked, the whole window takes the pointer; locked, only the buttons
/// that unlock it.
fn input_region(window: &gtk::Window, bar: &gtk::Box) {
    let Some(surface) = window.surface() else {
        return;
    };
    let region = if window.has_css_class("unlocked") {
        gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
            0,
            0,
            surface.width().max(1),
            surface.height().max(1),
        ))
    } else {
        let region = gtk::cairo::Region::create();
        let mut child = bar.first_child();
        while let Some(button) = child {
            if button.can_target()
                && let Some(bounds) = button.compute_bounds(window)
            {
                let _ = region.union_rectangle(&gtk::cairo::RectangleInt::new(
                    bounds.x().floor() as i32,
                    bounds.y().floor() as i32,
                    bounds.width().ceil() as i32,
                    bounds.height().ceil() as i32,
                ));
            }
            child = button.next_sibling();
        }
        region
    };
    surface.set_input_region(Some(&region));
}

fn label(line: &Line) -> gtk::Label {
    let label = gtk::Label::new(Some(&line.text));
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrSize::new_size_absolute(
        (line.size * gtk::pango::SCALE as f64) as i32,
    ));
    if line.bold {
        attributes.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
    }
    let [r, g, b, a] = line.color;
    let wide = |c: u8| c as u16 * 257;
    attributes.insert(gtk::pango::AttrColor::new_foreground(
        wide(r),
        wide(g),
        wide(b),
    ));
    attributes.insert(gtk::pango::AttrInt::new_foreground_alpha(wide(a)));
    label.set_attributes(Some(&attributes));
    label
}

/// Only how far the lines are lit changed: they draw again.
fn lit(id: &str, overlay: &Overlay) {
    let Parts { lines, .. } = parts(id);
    let mut child = lines.first_child();
    for line in overlay.lines.iter().filter(|line| !line.text.is_empty()) {
        let Some(widget) = child else { break };
        if let Some(shown) = widget.downcast_ref::<RubyLine>() {
            shown.update(line);
        } else if let Some(shown) = widget.downcast_ref::<crate::lit::Lit>() {
            if let Some(route) = &line.route {
                shown.follow(route.clone());
            } else if let Some(value) = line.lit {
                shown.set(value);
            }
        }
        child = widget.next_sibling();
    }
}

fn show(id: &str, overlay: &Overlay, style: TextStyle) {
    let Parts {
        window,
        bar,
        lines,
        panel,
    } = parts(id);
    for class in ["effect-none", "effect-shadow", "effect-outline"] {
        window.remove_css_class(class);
    }
    window.add_css_class(match style.effect {
        TextEffect::None => "effect-none",
        TextEffect::Shadow => "effect-shadow",
        TextEffect::Outline => "effect-outline",
    });
    lines.set_opacity(style.opacity);
    while let Some(child) = lines.first_child() {
        lines.remove(&child);
    }
    for line in overlay.lines.iter().filter(|line| !line.text.is_empty()) {
        let text = label(line);
        if !line.ruby_spans().is_empty() {
            lines.append(&RubyLine::new(line, &text));
        } else if line.lights() {
            let shown = crate::lit::Lit::new(&text, &text, line.lit.unwrap_or(0.0));
            if let Some(route) = &line.route {
                shown.follow(route.clone());
            }
            lines.append(&shown);
        } else {
            lines.append(&text);
        }
    }
    // Every button keeps its slot; locked, only those that unlock it show
    // and take the pointer.
    while let Some(child) = bar.first_child() {
        bar.remove(&child);
    }
    if overlay.place == Place::Free {
        for described in &overlay.buttons {
            let button = gtk::Button::from_icon_name(crate::icons::name(&described.symbol));
            button.add_css_class("bern-overlay-button");
            button.set_focusable(false);
            if overlay.locked && !described.while_locked {
                button.set_opacity(0.0);
                button.set_can_target(false);
            }
            let (overlay_id, button_id) = (id.to_string(), described.id.clone());
            button.connect_clicked(move |_| bern_ffi::overlay::press(&overlay_id, &button_id));
            bar.append(&button);
        }
    }
    bar.set_visible(bar.first_child().is_some());
    if overlay.locked {
        window.remove_css_class("unlocked");
        panel.close();
    } else {
        window.add_css_class("unlocked");
    }
    if lines.first_child().is_none() {
        panel.close();
        window.set_visible(false);
        return;
    }
    window.present();
    input_region(&window, &bar);
}

// A single source label supplies shaping, accessibility and scalar-to-glyph
// geometry. Readings are snapshot decorations, not text fragments in a row.
mod ruby_imp {
    use super::*;

    #[derive(Default)]
    pub struct RubyLine {
        pub line: RefCell<Option<Line>>,
        pub label: RefCell<Option<gtk::Label>>,
        pub spans: RefCell<Vec<(usize, usize, String)>>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
        pub band: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RubyLine {
        const NAME: &'static str = "BernOverlayRubyLine";
        type Type = super::RubyLine;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for RubyLine {
        fn dispose(&self) {
            if let Some(tick) = self.tick.borrow_mut().take() {
                tick.remove();
            }
            if let Some(label) = self.label.borrow_mut().take() {
                label.unparent();
            }
        }
    }

    impl WidgetImpl for RubyLine {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let label = self.label.borrow();
            let Some(label) = label.as_ref() else {
                return (0, 0, -1, -1);
            };
            let (min, natural, baseline, natural_baseline) = label.measure(orientation, for_size);
            let band = if orientation == gtk::Orientation::Vertical {
                self.band.get()
            } else {
                0
            };
            (
                min + band,
                natural + band,
                if baseline < 0 { -1 } else { baseline + band },
                if natural_baseline < 0 {
                    -1
                } else {
                    natural_baseline + band
                },
            )
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(label) = self.label.borrow().as_ref() {
                let band = self.band.get();
                label.allocate(
                    width,
                    (height - band).max(0),
                    if baseline < 0 { -1 } else { baseline - band },
                    Some(gsk::Transform::new().translate(&graphene::Point::new(0.0, band as f32))),
                );
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (line, label) = (self.line.borrow(), self.label.borrow());
            let (Some(line), Some(label)) = (line.as_ref(), label.as_ref()) else {
                return;
            };
            let place = line
                .route
                .as_ref()
                .map(|route| route.place())
                .or(line.lit)
                .filter(|place| *place >= 0.0);
            let scale = gtk::pango::SCALE as f32;
            let (offset, _) = label.layout_offsets();
            let layout = label.layout();
            if let Some(place) = place {
                let width = widget.width() as f32;
                let height = widget.height() as f32;
                let edge = line
                    .text
                    .char_indices()
                    .nth(place as usize)
                    .map_or(width, |(at, _)| {
                        let rect = layout.index_to_pos(at as i32);
                        offset as f32
                            + (rect.x() as f32 + rect.width() as f32 * place.fract() as f32) / scale
                    })
                    .clamp(0.0, width);
                snapshot.push_mask(gsk::MaskMode::Alpha);
                snapshot.append_color(
                    &gdk::RGBA::new(0.0, 0.0, 0.0, 1.0),
                    &graphene::Rect::new(0.0, 0.0, edge, height),
                );
                snapshot.append_color(
                    &gdk::RGBA::new(0.0, 0.0, 0.0, bern_ffi::overlay::FADED as f32),
                    &graphene::Rect::new(edge, 0.0, width - edge, height),
                );
                snapshot.pop();
            }
            widget.snapshot_child(label, snapshot);
            let offsets: Vec<_> = line
                .text
                .char_indices()
                .map(|(at, _)| at)
                .chain(std::iter::once(line.text.len()))
                .collect();
            let [r, g, b, a] = line.color;
            let color = gdk::RGBA::new(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            );
            for (start, end, reading) in self.spans.borrow().iter() {
                let left = layout.index_to_pos(offsets[*start] as i32).x() as f32 / scale;
                let right = layout.index_to_pos(offsets[*end] as i32).x() as f32 / scale;
                let available = (right - left).abs().max(1.0);
                let ruby = widget.create_pango_layout(Some(reading));
                let mut font = layout.font_description().unwrap_or_default();
                font.set_absolute_size(line.size * 0.45 * gtk::pango::SCALE as f64);
                if line.bold {
                    font.set_weight(gtk::pango::Weight::Bold);
                }
                ruby.set_font_description(Some(&font));
                let (measured, _) = ruby.pixel_size();
                font.set_absolute_size(
                    line.size
                        * 0.45
                        * gtk::pango::SCALE as f64
                        * (available as f64 / measured.max(1) as f64).min(1.0),
                );
                ruby.set_font_description(Some(&font));
                let (wide, high) = ruby.pixel_size();
                snapshot.save();
                snapshot.translate(&graphene::Point::new(
                    offset as f32 + left.min(right) + (available - wide as f32) / 2.0,
                    (self.band.get() - high).max(0) as f32,
                ));
                snapshot.push_shadow(&[gsk::Shadow::new(
                    gdk::RGBA::new(0.0, 0.0, 0.0, 0.85),
                    0.0,
                    1.0,
                    3.0,
                )]);
                snapshot.append_layout(&ruby, &color);
                snapshot.pop();
                snapshot.restore();
            }
            if place.is_some() {
                snapshot.pop();
            }
        }
    }
}

glib::wrapper! {
    pub struct RubyLine(ObjectSubclass<ruby_imp::RubyLine>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl RubyLine {
    fn new(line: &Line, label: &gtk::Label) -> Self {
        let widget: Self = glib::Object::new();
        label.set_parent(&widget);
        widget.imp().label.replace(Some(label.clone()));
        widget.imp().band.set((line.size * 0.7).ceil() as i32);
        widget.imp().spans.replace(line.ruby_spans());
        widget.update(line);
        widget
    }

    fn update(&self, line: &Line) {
        self.imp().line.replace(Some(line.clone()));
        let moving = line.route.as_ref().is_some_and(|route| route.moving());
        let mut tick = self.imp().tick.borrow_mut();
        if moving && tick.is_none() {
            *tick = Some(self.add_tick_callback(|widget, _| {
                widget.queue_draw();
                glib::ControlFlow::Continue
            }));
        } else if !moving && let Some(id) = tick.take() {
            id.remove();
        }
        self.queue_draw();
    }
}

struct SettingsPanel {
    id: String,
    window: glib::WeakRef<gtk::Window>,
    popover: gtk::Popover,
    title: gtk::Label,
    column: gtk::Box,
    rows: RefCell<Vec<PanelRow>>,
    open: Rc<Cell<bool>>,
    syncing: Rc<Cell<bool>>,
}

impl SettingsPanel {
    fn new(id: &str, window: &gtk::Window, parent: &gtk::Box) -> Rc<Self> {
        let popover = gtk::Popover::new();
        popover.set_parent(parent);
        popover.set_position(gtk::PositionType::Right);
        popover.set_autohide(false);
        popover.set_has_arrow(true);

        let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);
        outer.set_margin_top(10);
        outer.set_margin_bottom(10);
        outer.set_margin_start(12);
        outer.set_margin_end(12);
        outer.set_width_request(248);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(None);
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.add_css_class("circular");
        header.append(&title);
        header.append(&close);
        outer.append(&header);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .max_content_height(420)
            .child(&column)
            .build();
        outer.append(&scroll);
        popover.set_child(Some(&outer));

        let open = Rc::new(Cell::new(false));
        let syncing = Rc::new(Cell::new(false));
        let panel = Rc::new(Self {
            id: id.to_owned(),
            window: window.downgrade(),
            popover: popover.clone(),
            title,
            column,
            rows: RefCell::new(Vec::new()),
            open: Rc::clone(&open),
            syncing: Rc::clone(&syncing),
        });

        let pop_close = popover.clone();
        close.connect_clicked(move |_| pop_close.popdown());

        let weak = Rc::downgrade(&panel);
        popover.connect_closed(move |_| {
            if let Some(panel) = weak.upgrade() {
                if let Some(win) = panel.window.upgrade() {
                    win.set_focusable(false);
                }
                if panel.open.replace(false) && !panel.syncing.get() {
                    panel_dismissed(&panel.id);
                }
            }
        });

        panel
    }

    fn close(&self) {
        self.syncing.set(true);
        if self.open.replace(false) {
            self.popover.popdown();
        }
        if let Some(win) = self.window.upgrade() {
            win.set_focusable(false);
        }
        self.syncing.set(false);
    }

    fn show(&self, model: &Panel) {
        self.syncing.set(true);
        self.title.set_label(&model.title);
        let mut rows = self.rows.borrow_mut();
        let same_shape = rows.len() == model.controls.len()
            && rows.iter().zip(&model.controls).all(|(r, c)| r.matches(c));
        if !same_shape {
            rows.clear();
            while let Some(child) = self.column.first_child() {
                self.column.remove(&child);
            }
            for control in &model.controls {
                let row = PanelRow::new(&self.id, &self.syncing, control);
                self.column.append(&row.body);
                rows.push(row);
            }
        }
        for (row, control) in rows.iter_mut().zip(&model.controls) {
            row.sync(control);
        }
        drop(rows);
        self.syncing.set(false);
        if !self.open.replace(true) {
            if let Some(win) = self.window.upgrade() {
                win.set_focusable(true);
            }
            self.popover.popup();
        }
    }
}

struct PanelRow {
    id: String,
    body: gtk::Box,
    label: gtk::Label,
    kind: PanelKind,
}

enum PanelKind {
    Range(gtk::Scale, gtk::Label),
    Color(gtk::ColorDialogButton),
    Choice(gtk::DropDown, Vec<String>),
}

impl PanelRow {
    fn id_of(c: &Control) -> &str {
        match c {
            Control::Range { id, .. } | Control::Color { id, .. } | Control::Choice { id, .. } => {
                id
            }
        }
    }

    fn matches(&self, control: &Control) -> bool {
        self.id == Self::id_of(control)
            && matches!(
                (&self.kind, control),
                (PanelKind::Range(..), Control::Range { .. })
                    | (PanelKind::Color(..), Control::Color { .. })
                    | (PanelKind::Choice(..), Control::Choice { .. })
            )
    }

    fn new(overlay_id: &str, syncing: &Rc<Cell<bool>>, control: &Control) -> Self {
        let id = Self::id_of(control).to_owned();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);

        let kind = match control {
            Control::Range { min, max, step, .. } => {
                let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                label.set_hexpand(true);
                let readout = gtk::Label::new(None);
                readout.add_css_class("dim-label");
                top.append(&label);
                top.append(&readout);
                body.append(&top);

                let step = if *step > 0.0 && step.is_finite() {
                    *step
                } else {
                    1.0
                };
                let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, *min, *max, step);
                scale.set_draw_value(false);
                let oid = overlay_id.to_owned();
                let cid = id.clone();
                let sync = Rc::clone(syncing);
                let read = readout.clone();
                scale.connect_value_changed(move |s| {
                    let v = s.value();
                    read.set_label(&format!("{}", v.round() as i64));
                    if !sync.get() {
                        panel_changed(&oid, &cid, v, true);
                    }
                });
                body.append(&scale);
                PanelKind::Range(scale, readout)
            }
            Control::Color { .. } => {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                label.set_hexpand(true);
                let dialog = gtk::ColorDialog::new();
                dialog.set_with_alpha(false);
                let btn = gtk::ColorDialogButton::new(Some(dialog));
                let oid = overlay_id.to_owned();
                let cid = id.clone();
                let sync = Rc::clone(syncing);
                btn.connect_rgba_notify(move |b| {
                    if sync.get() {
                        return;
                    }
                    let rgba = b.rgba();
                    let byte = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u32;
                    let rgb =
                        (byte(rgba.red()) << 16) | (byte(rgba.green()) << 8) | byte(rgba.blue());
                    panel_changed(&oid, &cid, f64::from(rgb), true);
                });
                row.append(&label);
                row.append(&btn);
                body.append(&row);
                PanelKind::Color(btn)
            }
            Control::Choice { options, .. } => {
                body.append(&label);
                let refs: Vec<&str> = options.iter().map(String::as_str).collect();
                let dropdown = gtk::DropDown::from_strings(&refs);
                let oid = overlay_id.to_owned();
                let cid = id.clone();
                let sync = Rc::clone(syncing);
                dropdown.connect_selected_notify(move |d| {
                    if sync.get() {
                        return;
                    }
                    let idx = d.selected();
                    if idx != gtk::INVALID_LIST_POSITION {
                        panel_changed(&oid, &cid, f64::from(idx), true);
                    }
                });
                body.append(&dropdown);
                PanelKind::Choice(dropdown, options.clone())
            }
        };

        Self {
            id,
            body,
            label,
            kind,
        }
    }

    fn sync(&mut self, control: &Control) {
        match (&mut self.kind, control) {
            (
                PanelKind::Range(scale, readout),
                Control::Range {
                    label,
                    min,
                    max,
                    step,
                    value,
                    ..
                },
            ) => {
                self.label.set_label(label);
                scale.set_range(*min, *max);
                if *step > 0.0 && step.is_finite() {
                    scale.set_increments(*step, *step * 5.0);
                }
                scale.set_value(value.clamp(*min, *max));
                readout.set_label(&format!("{}", value.round() as i64));
            }
            (PanelKind::Color(btn), Control::Color { label, rgb, .. }) => {
                self.label.set_label(label);
                let r = ((*rgb >> 16) & 0xff) as f32 / 255.0;
                let g = ((*rgb >> 8) & 0xff) as f32 / 255.0;
                let b = (*rgb & 0xff) as f32 / 255.0;
                btn.set_rgba(&gdk::RGBA::new(r, g, b, 1.0));
            }
            (
                PanelKind::Choice(dropdown, prev),
                Control::Choice {
                    label,
                    options,
                    selected,
                    ..
                },
            ) => {
                self.label.set_label(label);
                if prev != options {
                    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
                    let model = gtk::StringList::new(&refs);
                    dropdown.set_model(Some(&model));
                    *prev = options.clone();
                }
                if *selected < options.len() {
                    dropdown.set_selected(*selected as u32);
                }
            }
            _ => {}
        }
    }
}
