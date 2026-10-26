//! Scenes and containers with a layout of their own: a scene of fixed
//! logical size scaled to fit (`ScaleBox`), elements placed in it
//! (`Positioned`), a line that runs across (`Marquee`), columns that share
//! a width (`Accordion`), wrapping rows (`Flow`), a grid of cells sized by
//! their width, a modal above the window, timers, and control groups.

use crate::layout::Slides;
use crate::motion::{Curve, Decor, Num, Spring, bind_number, reduced};
use crate::reactive::{effect, on_cleanup, untrack};
use crate::widgets::{GridProps, bound_text, pad, place};
use crate::{Act, Build, Ctx, Get, Parent, Str};
use adw::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// The base look of these containers and of the controls' shapes.
pub(crate) const SHEET: &str = "\
.bern-capsule, .bern-capsule > button, .bern-capsule > .linked > button { border-radius: 9999px; }\n\
.linked.bern-capsule > :first-child { border-top-left-radius: 9999px; border-bottom-left-radius: 9999px; }\n\
.linked.bern-capsule > :last-child { border-top-right-radius: 9999px; border-bottom-right-radius: 9999px; }\n\
.bern-sized-control, .bern-sized-control > button, .bern-sized-control > box > button { min-height: 0; padding-top: 0; padding-bottom: 0; }\n\
.bern-sized-control.circular, .bern-sized-control.circular > button { min-width: 0; padding: 0; }\n\
row.bern-plain-row { padding: 10px 12px; }\n\
row.bern-plain-row.bern-button-row { padding: 8px 12px; }\n\
list.bern-plain-list, list.bern-plain-list > row { background: none; border: none; box-shadow: none; padding: 0; }\n\
list.bern-plain-list > row > box.header { padding: 0; min-height: 0; }\n\
.bern-form-rows > :first-child:not(.bern-plain-section) { margin-top: 24px; }\n\
.bern-badge { padding: 0 7px; min-width: 10px; border-radius: 9999px; background-color: alpha(currentColor, 0.12); font-size: smaller; font-weight: 600; }\n\
.bern-marquee label { text-shadow: 0 1px 1px black; }\n\
.bern-control-group { border-radius: 9999px; }\n\
.bern-modal-backdrop { background-color: transparent; transition: background-color 200ms ease-out; }\n\
.bern-modal-backdrop.bern-shown { background-color: rgba(0, 0, 0, 0.24); }\n\
.bern-modal { background-color: @dialog_bg_color; color: @dialog_fg_color; border-radius: 24px; border: 1px solid alpha(currentColor, 0.12); box-shadow: 0 20px 40px rgba(0, 0, 0, 0.28); }\n\
.bern-modal.bern-cover { border-radius: 0; border: none; box-shadow: none; background-color: @window_bg_color; color: @window_fg_color; }\n\
.bern-palette-trigger { padding: 4px 12px; }\n\
.bern-search-item { padding: 8px 12px; border-radius: 10px; }\n\
.bern-search-item.bern-active { background-color: alpha(@accent_bg_color, 0.18); }\n\
.bern-search-item-card { padding: 0; }\n";

fn children(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut children = Vec::new();
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        children.push(current);
    }
    children
}

fn translate(x: f64, y: f64) -> Option<gtk::gsk::Transform> {
    Some(gtk::gsk::Transform::new().translate(&gtk::graphene::Point::new(x as f32, y as f32)))
}

fn side(value: f64) -> i32 {
    value.round().max(0.0) as i32
}

// MARK: - ScaleBox

/// A `ScaleBox` scene's size and the room the box has around it on each
/// side, in scene points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Room {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

impl Room {
    /// The room around a `scene` scaled by `scale` and centered in a box
    /// of `area`.
    pub(crate) fn around(scene: (f64, f64), area: (f64, f64), scale: f64) -> Self {
        let fits = scale > 0.0 && scale.is_finite();
        let room = |area: f64, scene: f64| {
            if fits {
                (area / scale - scene).max(0.0) / 2.0
            } else {
                0.0
            }
        };
        Self {
            width: scene.0,
            height: scene.1,
            x: room(area.0, scene.0),
            y: room(area.1, scene.1),
        }
    }

    /// How far each side of a layer at `frame` (x, y, width, height)
    /// reaches past the scene when it bleeds — left, right, top, bottom:
    /// a side at an edge of the scene reaches the edge of the room, with
    /// half a point of slack so a layer laid out to the edge reaches it.
    pub(crate) fn reach(&self, (x, y, width, height): (f64, f64, f64, f64)) -> [f64; 4] {
        let at = |reached: bool, room: f64| if reached { room } else { 0.0 };
        [
            at(x <= 0.5, self.x),
            at(x + width >= self.width - 0.5, self.x),
            at(y <= 0.5, self.y),
            at(y + height >= self.height - 0.5, self.y),
        ]
    }
}

mod scale_box {
    use super::*;

    #[derive(Default)]
    pub struct ScaleBox {
        /// The scene's logical size.
        pub size: Cell<(f64, f64)>,
        /// The room around the scene as last laid out.
        pub room: Cell<Room>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ScaleBox {
        const NAME: &'static str = "BernScaleBox";
        type Type = super::ScaleBoxWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ScaleBox {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for ScaleBox {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (width, height) = self.size.get();
            let natural = match orientation {
                gtk::Orientation::Horizontal => width,
                _ => height,
            };
            (0, side(natural), -1, -1)
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = true;
            *vexpand = true;
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let (w, h) = self.size.get();
            let (w, h) = (w.max(1.0), h.max(1.0));
            let scale = (f64::from(width) / w).min(f64::from(height) / h).max(0.0);
            let x = (f64::from(width) - w * scale) / 2.0;
            let y = (f64::from(height) - h * scale) / 2.0;
            let room = Room::around((w, h), (f64::from(width), f64::from(height)), scale);
            if self.room.replace(room) != room {
                // A new shape of the box changes only the scene's
                // transform, and GTK keeps the layout of a child given the
                // same size: the layers that bleed lay out again for the
                // new room.
                bleeding_layers(self.obj().upcast_ref(), &mut |layer| layer.queue_allocate());
            }
            for child in children(self.obj().upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                child.measure(gtk::Orientation::Horizontal, -1);
                let transform = gtk::gsk::Transform::new()
                    .translate(&gtk::graphene::Point::new(x as f32, y as f32))
                    .scale(scale as f32, scale as f32);
                child.allocate(side(w), side(h), -1, Some(transform));
            }
        }
    }
}

glib::wrapper! {
    /// A scene of fixed logical size, scaled as one to fit the room it
    /// gets and centered in it, so pictures, type and hit targets stay in
    /// register at any size.
    pub struct ScaleBoxWidget(ObjectSubclass<scale_box::ScaleBox>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[derive(Default)]
pub struct ScaleBoxProps {
    pub width: Option<Num>,
    pub height: Option<Num>,
}

/// The layers of a scene, from its top leading corner.
fn scene_layers() -> crate::layout::Layers {
    let layers = crate::layout::Layers::new();
    layers.set_hexpand(true);
    layers.set_vexpand(true);
    layers
}

#[allow(non_snake_case)]
pub fn ScaleBox(p: &Parent, ctx: &Ctx, props: ScaleBoxProps, children: Build) {
    let scaled: ScaleBoxWidget = glib::Object::new();
    scaled.set_overflow(gtk::Overflow::Hidden);
    for (index, length) in [props.width, props.height].into_iter().enumerate() {
        let Some(length) = length else { continue };
        let sized = scaled.clone();
        bind_number(ctx, &length, None, &scaled, move |value| {
            let (width, height) = sized.imp().size.get();
            sized.imp().size.set(if index == 0 {
                (value, height)
            } else {
                (width, value)
            });
            sized.queue_resize();
        });
    }
    let scene = scene_layers();
    scene.set_parent(&scaled);
    children(
        &Parent::of_layers(&scene, gtk::Align::Start, gtk::Align::Start),
        ctx,
    );
    place(p, &scaled);
}

// MARK: - Positioned

/// The sides of a layer that bleeds: whether it bleeds, where it is going
/// (its place before any motion), the room of its scene as it last laid
/// out, and how far each side reaches past the scene — left, right, top,
/// bottom — on the layer's motion.
pub struct Bleed {
    on: Cell<bool>,
    target: Cell<(f64, f64, f64, f64)>,
    room: Cell<Option<Room>>,
    sides: [Spring; 4],
    curve: Option<Curve>,
}

impl Bleed {
    /// Move the sides to where they reach at the layer's target: on the
    /// layer's motion, or with `jump` at once, for a box of a new shape.
    fn follow(&self, jump: bool) {
        let reach = match (self.on.get(), self.room.get()) {
            (true, Some(room)) => room.reach(self.target.get()),
            _ => [0.0; 4],
        };
        for (spring, to) in self.sides.iter().zip(reach) {
            if jump {
                spring.set_quietly(to);
            } else if spring.target() != to {
                spring.to(to, self.curve, None);
            }
        }
    }

    fn reach(&self) -> [f64; 4] {
        self.sides.each_ref().map(|side| side.value().max(0.0))
    }
}

/// The room around the scene `widget` is a layer of; none for a layer of
/// a layer, which is placed in that layer's space.
fn scene_room(widget: &gtk::Widget) -> Option<Room> {
    let mut parent = widget.parent();
    while let Some(current) = parent {
        if let Some(scene) = current.downcast_ref::<ScaleBoxWidget>() {
            return Some(scene.imp().room.get());
        }
        if current.is::<PositionedWidget>() {
            return None;
        }
        parent = current.parent();
    }
    None
}

/// Each layer of `widget`'s scene that bleeds, as `found` gets it.
fn bleeding_layers(widget: &gtk::Widget, found: &mut impl FnMut(&PositionedWidget)) {
    for child in children(widget) {
        match child.downcast_ref::<PositionedWidget>() {
            Some(layer) if layer.imp().bleed.borrow().is_some() => found(layer),
            Some(_) => {}
            None if child.is::<ScaleBoxWidget>() => {}
            None => bleeding_layers(&child, found),
        }
    }
}

mod positioned {
    use super::*;

    #[derive(Default)]
    pub struct Positioned {
        /// Where the content stands in its container, and its size.
        pub frame: Cell<(f64, f64, f64, f64)>,
        /// With `bleed`, the sides that reach past the scene.
        pub bleed: RefCell<Option<Rc<Bleed>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Positioned {
        const NAME: &'static str = "BernPositioned";
        type Type = super::PositionedWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Positioned {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Positioned {
        /// As large as reaching its content's far corner: a stack of
        /// placed elements is the size of all of them.
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (x, y, width, height) = self.frame.get();
            let reach = match orientation {
                gtk::Orientation::Horizontal => x + width,
                _ => y + height,
            };
            (0, side(reach), -1, -1)
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = false;
            *vexpand = false;
        }

        /// Only the content takes input. The layer itself spans from its
        /// container's corner to the content's far corner, so a layer
        /// placed low in a scene would otherwise catch every press above
        /// it and hide the layers beneath.
        fn contains(&self, _x: f64, _y: f64) -> bool {
            false
        }

        fn size_allocate(&self, _width: i32, _height: i32, _baseline: i32) {
            let (x, y, width, height) = self.frame.get();
            let [left, right, top, bottom] = match self.bleed.borrow().as_ref() {
                Some(bleed) => {
                    let room = scene_room(self.obj().upcast_ref());
                    if bleed.room.replace(room) != room {
                        bleed.follow(true);
                    }
                    bleed.reach()
                }
                None => [0.0; 4],
            };
            for child in children(self.obj().upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(
                    side(width + left + right),
                    side(height + top + bottom),
                    -1,
                    translate(x - left, y - top),
                );
            }
        }
    }
}

glib::wrapper! {
    /// Content at a point of its container, at a fixed size.
    pub struct PositionedWidget(ObjectSubclass<positioned::Positioned>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// Whether `widget` is a placed element, or wraps one in its motion: it
/// takes the whole of the stack it stands in.
pub(crate) fn is_positioned(widget: &gtk::Widget) -> bool {
    widget.is::<PositionedWidget>()
        || (widget.is::<crate::motion::MotionBox>()
            && widget
                .first_child()
                .is_some_and(|child| child.is::<PositionedWidget>()))
}

#[derive(Default)]
pub struct PositionedProps {
    pub x: Option<Num>,
    pub y: Option<Num>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub animate: Option<Curve>,
    /// Whether the layer continues past each edge of its scene it reaches,
    /// to the edge of the `ScaleBox`.
    pub bleed: Option<Get>,
}

#[allow(non_snake_case)]
pub fn Positioned(p: &Parent, ctx: &Ctx, props: PositionedProps, children: Build) {
    let placed: PositionedWidget = glib::Object::new();
    let frame = [props.x, props.y, props.width, props.height];
    if let Some(on) = props.bleed {
        // The sides follow where the layer is going, on its motion.
        let weak = placed.downgrade();
        let sides = std::array::from_fn(|_| {
            let weak = weak.clone();
            Spring::new(0.0, &placed, move |_| {
                if let Some(placed) = weak.upgrade() {
                    placed.queue_allocate();
                }
            })
        });
        let bleed = Rc::new(Bleed {
            on: Cell::new(false),
            target: Cell::new((0.0, 0.0, 0.0, 0.0)),
            room: Cell::new(None),
            sides,
            curve: props.animate,
        });
        *placed.imp().bleed.borrow_mut() = Some(Rc::clone(&bleed));
        let at = ctx.clone();
        let frame = frame.clone();
        effect(move || {
            let [x, y, width, height] = frame.each_ref().map(|value| match value {
                Some(Num::Fixed(value)) => *value,
                Some(Num::Bound(get)) => crate::value::as_f64(&get(&at)),
                None => 0.0,
            });
            bleed.on.set(crate::value::truthy(&on(&at)));
            bleed.target.set((x, y, width, height));
            untrack(|| bleed.follow(false));
        });
    }
    for (index, value) in frame.into_iter().enumerate() {
        let Some(value) = value else { continue };
        let moved = placed.clone();
        bind_number(ctx, &value, props.animate, &placed, move |value| {
            let (mut x, mut y, mut width, mut height) = moved.imp().frame.get();
            match index {
                0 => x = value,
                1 => y = value,
                2 => width = value,
                _ => height = value,
            }
            moved.imp().frame.set((x, y, width, height));
            moved.queue_resize();
        });
    }
    let content = scene_layers();
    content.set_parent(&placed);
    children(
        &Parent::of_layers(&content, gtk::Align::Start, gtk::Align::Start),
        ctx,
    );
    place(p, &placed);
}

// MARK: - Marquee

mod marquee {
    use super::*;

    #[derive(Default)]
    pub struct Marquee {
        pub duration: Cell<f64>,
        pub phase: Cell<f64>,
        pub paused: Cell<bool>,
        /// Seconds run so far, and the frame time they were counted at.
        pub elapsed: Cell<f64>,
        pub last: Cell<Option<i64>>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Marquee {
        const NAME: &'static str = "BernMarquee";
        type Type = super::MarqueeWidget;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_css_name("marquee");
        }
    }

    impl ObjectImpl for Marquee {
        fn dispose(&self) {
            if let Some(tick) = self.tick.borrow_mut().take() {
                tick.remove();
            }
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Marquee {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match (orientation, self.obj().first_child()) {
                (gtk::Orientation::Vertical, Some(child)) => {
                    let (minimum, natural, _, _) = child.measure(orientation, -1);
                    (minimum, natural, -1, -1)
                }
                _ => (0, 0, -1, -1),
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = true;
            *vexpand = false;
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let Some(child) = self.obj().first_child() else {
                return;
            };
            let (_, line, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
            let room = f64::from(width);
            // From beyond the trailing edge to beyond the leading one; with
            // less motion it stands still a little way in.
            let x = if reduced() {
                room * 0.28
            } else {
                let progress = (self.phase.get()
                    + self.elapsed.get() / self.duration.get().max(1.0))
                .rem_euclid(1.0);
                room - (room + f64::from(line)) * progress
            };
            child.allocate(line, height, -1, translate(x, 0.0));
        }

        fn map(&self) {
            self.parent_map();
            self.obj().run();
        }

        fn unmap(&self) {
            if let Some(tick) = self.tick.borrow_mut().take() {
                tick.remove();
            }
            self.last.set(None);
            self.parent_unmap();
        }
    }
}

glib::wrapper! {
    /// Content that runs across the room it gets, from the trailing edge
    /// to the leading one, every `duration` seconds.
    pub struct MarqueeWidget(ObjectSubclass<marquee::Marquee>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MarqueeWidget {
    /// Run while shown and not paused; stand still otherwise.
    fn run(&self) {
        let imp = self.imp();
        let running = self.is_mapped() && !imp.paused.get() && !reduced();
        if !running {
            if let Some(tick) = imp.tick.borrow_mut().take() {
                tick.remove();
            }
            imp.last.set(None);
            self.queue_allocate();
            return;
        }
        if imp.tick.borrow().is_some() {
            return;
        }
        let tick = self.add_tick_callback(|marquee, clock| {
            let imp = marquee.imp();
            let now = clock.frame_time();
            if let Some(last) = imp.last.replace(Some(now)) {
                imp.elapsed
                    .set(imp.elapsed.get() + (now - last).max(0) as f64 / 1_000_000.0);
            }
            marquee.queue_allocate();
            glib::ControlFlow::Continue
        });
        *imp.tick.borrow_mut() = Some(tick);
    }
}

#[derive(Default)]
pub struct MarqueeProps {
    pub duration: Option<Num>,
    pub phase: f64,
    pub paused: Option<Get>,
    pub opacity: Option<Num>,
}

#[allow(non_snake_case)]
pub fn Marquee(p: &Parent, ctx: &Ctx, props: MarqueeProps, children: Build) {
    let marquee: MarqueeWidget = glib::Object::new();
    marquee.add_css_class("bern-marquee");
    marquee.set_overflow(gtk::Overflow::Hidden);
    marquee.imp().duration.set(12.0);
    marquee.imp().phase.set(props.phase);
    if let Some(duration) = &props.duration {
        let timed = marquee.clone();
        bind_number(ctx, duration, None, &marquee, move |value| {
            timed.imp().duration.set(value.max(1.0));
        });
    }
    if let Some(paused) = props.paused {
        let at = ctx.clone();
        let held = marquee.clone();
        effect(move || {
            let on = crate::value::truthy(&paused(&at));
            untrack(|| {
                held.imp().paused.set(on);
                held.run();
            });
        });
    }
    if let Some(opacity) = &props.opacity {
        let faded = marquee.clone();
        bind_number(ctx, opacity, None, &marquee, move |value| {
            faded.set_opacity(value.clamp(0.0, 1.0));
        });
    }
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.set_parent(&marquee);
    children(&Parent::of_box(&line), ctx);
    place(p, &marquee);
}

// MARK: - Accordion

/// Fixed collapsed columns share the width with one expanded column, by
/// weights that move between them; the total stays the width throughout.
pub(crate) fn accordion_widths(total: f64, collapsed: f64, gap: f64, weights: &[f64]) -> Vec<f64> {
    if weights.is_empty() {
        return Vec::new();
    }
    let count = weights.len() as f64;
    let available = (total - gap.max(0.0) * (count - 1.0)).max(0.0);
    let collapsed = collapsed.max(0.0).min(available / count);
    let extra = (available - collapsed * count).max(0.0);
    let widths: Vec<f64> = weights
        .iter()
        .map(|weight| (collapsed + extra * weight).max(0.0))
        .collect();
    let sum: f64 = widths.iter().sum();
    if sum > 0.0 {
        widths.iter().map(|width| width * available / sum).collect()
    } else {
        widths
    }
}

mod accordion {
    use super::*;

    #[derive(Default)]
    pub struct Accordion {
        pub weights: RefCell<Vec<f64>>,
        pub collapsed: Cell<f64>,
        pub gap: Cell<f64>,
        pub height: Cell<Option<f64>>,
        pub corner: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Accordion {
        const NAME: &'static str = "BernAccordion";
        type Type = super::AccordionWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Accordion {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Accordion {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => (0, 0, -1, -1),
                _ => match self.height.get() {
                    Some(height) => {
                        let height = side(height);
                        (height, height, -1, -1)
                    }
                    None => (0, 560, -1, -1),
                },
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = true;
            *vexpand = self.height.get().is_none();
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let columns: Vec<gtk::Widget> = children(self.obj().upcast_ref())
                .into_iter()
                .filter(|child| child.should_layout())
                .collect();
            let mut weights = self.weights.borrow().clone();
            weights.resize(columns.len(), 0.0);
            let widths = accordion_widths(
                f64::from(width),
                self.collapsed.get(),
                self.gap.get(),
                &weights,
            );
            let mut x = 0.0;
            for (child, column) in columns.iter().zip(widths) {
                child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(side(column), height, -1, translate(x, 0.0));
                x += column + self.gap.get();
            }
        }

        /// Each column in its own rounded corners.
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let radius = self.corner.get() as f32;
            for child in children(widget.upcast_ref()) {
                let Some(bounds) = child.compute_bounds(&*widget) else {
                    continue;
                };
                if radius > 0.0 {
                    snapshot.push_rounded_clip(&gtk::gsk::RoundedRect::from_rect(bounds, radius));
                }
                widget.snapshot_child(&child, snapshot);
                if radius > 0.0 {
                    snapshot.pop();
                }
            }
        }
    }
}

glib::wrapper! {
    /// Columns side by side, one expanded and the others collapsed to a
    /// fixed width; the widths spring as the expanded one changes.
    pub struct AccordionWidget(ObjectSubclass<accordion::Accordion>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[derive(Default)]
pub struct AccordionProps {
    pub count: Option<Num>,
    pub selected: Option<Num>,
    pub collapsed_width: Option<Num>,
    pub gap: Option<Num>,
    pub height: Option<Num>,
    pub corner: Option<Num>,
    pub animate: Option<Curve>,
}

#[allow(non_snake_case)]
pub fn Accordion(p: &Parent, ctx: &Ctx, props: AccordionProps, children: Build) {
    let accordion: AccordionWidget = glib::Object::new();
    let imp = accordion.imp();
    imp.collapsed.set(86.0);
    imp.gap.set(8.0);
    imp.height.set(None);
    let lengths = [
        (props.collapsed_width.clone(), 0),
        (props.gap.clone(), 1),
        (props.height.clone(), 2),
        (props.corner.clone(), 3),
    ];
    for (length, which) in lengths {
        let Some(length) = length else { continue };
        let shown = accordion.clone();
        bind_number(ctx, &length, None, &accordion, move |value| {
            let imp = shown.imp();
            match which {
                0 => imp.collapsed.set(value),
                1 => imp.gap.set(value),
                2 => imp.height.set(Some(value)),
                _ => imp.corner.set(value),
            }
            shown.queue_resize();
        });
    }
    // One spring per column, from collapsed (0) to expanded (1).
    let springs: Rc<RefCell<Vec<Spring>>> = Rc::new(RefCell::new(Vec::new()));
    let at = ctx.clone();
    let shown = accordion.clone();
    let (count, selected, animate) = (props.count, props.selected, props.animate);
    effect(move || {
        let read = |value: &Option<Num>| match value {
            Some(Num::Fixed(value)) => *value,
            Some(Num::Bound(get)) => crate::value::as_f64(&get(&at)),
            None => 0.0,
        };
        let count = read(&count).floor().max(0.0) as usize;
        let chosen = (read(&selected).round().max(0.0) as usize).min(count.saturating_sub(1));
        untrack(|| {
            let mut springs = springs.borrow_mut();
            if springs.len() != count {
                // A new set of columns starts where it rests.
                springs.clear();
                *shown.imp().weights.borrow_mut() = (0..count)
                    .map(|at| f64::from(u8::from(at == chosen)))
                    .collect();
                for index in 0..count {
                    let weighed = shown.clone();
                    let start = f64::from(u8::from(index == chosen));
                    springs.push(Spring::new(start, &shown, move |value| {
                        if let Some(weight) = weighed.imp().weights.borrow_mut().get_mut(index) {
                            *weight = value;
                        }
                        weighed.queue_allocate();
                    }));
                }
                shown.queue_allocate();
                return;
            }
            let curve = if reduced() { None } else { animate };
            for (index, spring) in springs.iter().enumerate() {
                let target = f64::from(u8::from(index == chosen));
                if (spring.target() - target).abs() > f64::EPSILON {
                    spring.to(target, curve, None);
                }
            }
        });
    });
    children(&Parent::of_children(&accordion, true), ctx);
    place(p, &accordion);
}

// MARK: - Flow

mod flow {
    use super::*;

    /// A child and where it stands: x, y, width and height.
    type Placed = (gtk::Widget, f64, f64, f64, f64);

    #[derive(Default)]
    pub struct Flow {
        pub gap: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Flow {
        const NAME: &'static str = "BernFlow";
        type Type = super::FlowWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Flow {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl Flow {
        /// Where each child stands in lines `width` wide, and the height of
        /// them all.
        fn lines(&self, width: Option<f64>) -> (Vec<Placed>, f64) {
            let mut widgets = Vec::new();
            let mut items = Vec::new();
            for child in children(self.obj().upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                let spacer = child.has_css_class(SPACER);
                let (w, h) = if spacer {
                    (0.0, 0.0)
                } else {
                    let (_, natural, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                    let w = match width {
                        Some(width) => f64::from(natural).min(width),
                        None => f64::from(natural),
                    };
                    let (_, tall, _, _) = child.measure(gtk::Orientation::Vertical, side(w));
                    (w, f64::from(tall))
                };
                widgets.push(child);
                items.push(FlowItem {
                    width: w,
                    height: h,
                    spacer,
                });
            }
            let (frames, height) = flow_lines(&items, width, self.gap.get());
            let placed = widgets
                .into_iter()
                .zip(frames)
                .map(|(child, (x, y, w, h))| (child, x, y, w, h))
                .collect();
            (placed, height)
        }
    }

    impl WidgetImpl for Flow {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => {
                    let (mut minimum, mut natural) = (0, 0);
                    let gap = side(self.gap.get());
                    for child in children(self.obj().upcast_ref()) {
                        if !child.should_layout() {
                            continue;
                        }
                        let (child_minimum, child_natural, _, _) = child.measure(orientation, -1);
                        minimum = minimum.max(child_minimum);
                        natural += child_natural + if natural > 0 { gap } else { 0 };
                    }
                    (minimum, natural, -1, -1)
                }
                _ => {
                    let width = (for_size >= 0).then_some(f64::from(for_size));
                    let (_, height) = self.lines(width);
                    (side(height), side(height), -1, -1)
                }
            }
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            let rtl = self.obj().direction() == gtk::TextDirection::Rtl;
            let (placed, _) = self.lines(Some(f64::from(width)));
            for (child, x, y, w, h) in placed {
                let x = if rtl { f64::from(width) - x - w } else { x };
                child.allocate(side(w), side(h), -1, translate(x, y));
            }
        }
    }
}

/// The style class of a `Spacer`, which takes the room left on its line of
/// a `Flow`.
pub(crate) const SPACER: &str = "bern-spacer";

/// One child of a `Flow`: its size, or a spacer.
#[derive(Clone, Copy)]
struct FlowItem {
    width: f64,
    height: f64,
    spacer: bool,
}

/// Each item's frame (x, y, width, height) in lines `width` wide, and the
/// height of them all: lines broken at the width, a spacer taking no room
/// there; then the spacers of a line share the room left on it, and each
/// item is centred in its line's height.
fn flow_lines(
    items: &[FlowItem],
    width: Option<f64>,
    gap: f64,
) -> (Vec<(f64, f64, f64, f64)>, f64) {
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut x = 0.0;
    for (index, item) in items.iter().enumerate() {
        if let Some(width) = width
            && lines.last().is_some_and(|line| !line.is_empty())
            && x + item.width > width
        {
            lines.push(Vec::new());
            x = 0.0;
        }
        if let Some(line) = lines.last_mut() {
            line.push(index);
        }
        x += item.width + gap;
    }
    let mut frames = vec![(0.0, 0.0, 0.0, 0.0); items.len()];
    let mut y = 0.0;
    let count = lines.len();
    for (number, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let used = line.iter().map(|&index| items[index].width).sum::<f64>()
            + gap * (line.len() - 1) as f64;
        let tall = line
            .iter()
            .map(|&index| items[index].height)
            .fold(0.0, f64::max);
        let spacers = line.iter().filter(|&&index| items[index].spacer).count();
        let share = match width {
            Some(width) if spacers > 0 => (width - used).max(0.0) / spacers as f64,
            _ => 0.0,
        };
        let mut at = 0.0;
        for &index in line {
            let item = items[index];
            let (w, h) = if item.spacer {
                (share, tall)
            } else {
                (item.width, item.height)
            };
            frames[index] = (at, y + (tall - h) / 2.0, w, h);
            at += w + gap;
        }
        y += tall + if number + 1 < count { gap } else { 0.0 };
    }
    (frames, y)
}

glib::wrapper! {
    /// Children in lines, each at its own width, wrapping onto the next
    /// line when the room ends.
    pub struct FlowWidget(ObjectSubclass<flow::Flow>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[derive(Default)]
pub struct FlowProps {
    pub gap: Option<Num>,
}

/// A flow whose items are `gap` apart, filling the width it is given.
pub(crate) fn flow_of(gap: f64) -> FlowWidget {
    let flow: FlowWidget = glib::Object::new();
    flow.set_hexpand(true);
    flow.imp().gap.set(gap);
    flow
}

#[allow(non_snake_case)]
pub fn Flow(p: &Parent, ctx: &Ctx, props: FlowProps, children: Build) {
    let flow = flow_of(8.0);
    if let Some(gap) = &props.gap {
        let spaced = flow.clone();
        bind_number(ctx, gap, None, &flow, move |value| {
            spaced.imp().gap.set(value.max(0.0));
            spaced.queue_resize();
        });
    }
    children(&Parent::of_children(&flow, true), ctx);
    place(p, &flow);
}

// MARK: - Grid of cells

mod cells {
    use super::*;

    #[derive(Default)]
    pub struct Cells {
        pub min_width: Cell<f64>,
        pub gap: Cell<f64>,
        pub row_gap: Cell<f64>,
        pub ratio: Cell<f64>,
        pub extra: Cell<f64>,
        /// With `reflow`, the spring cells slide to new places with.
        pub curve: Cell<Option<Curve>>,
        pub(crate) slides: Slides,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Cells {
        const NAME: &'static str = "BernCellGrid";
        type Type = super::CellGrid;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Cells {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl Cells {
        /// Columns, cell width and cell height at `width`.
        pub fn cells(&self, width: f64) -> (usize, f64, f64) {
            let (min, gap) = (self.min_width.get().max(1.0), self.gap.get().max(0.0));
            let columns = (((width + gap) / (min + gap)).floor() as usize).max(1);
            let cell = ((width - gap * (columns as f64 - 1.0)) / columns as f64).max(0.0);
            (
                columns,
                cell,
                cell / self.ratio.get().max(f64::EPSILON) + self.extra.get(),
            )
        }

        fn visible(&self) -> Vec<gtk::Widget> {
            children(self.obj().upcast_ref())
                .into_iter()
                .filter(|child| child.should_layout())
                .collect()
        }
    }

    impl WidgetImpl for Cells {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => {
                    let min = side(self.min_width.get());
                    (min, min, -1, -1)
                }
                _ => {
                    let width = if for_size >= 0 {
                        f64::from(for_size)
                    } else {
                        self.min_width.get()
                    };
                    let (columns, _, cell) = self.cells(width);
                    let count = self.visible().len();
                    let rows = count.div_ceil(columns);
                    let height = if rows == 0 {
                        0.0
                    } else {
                        rows as f64 * cell + (rows as f64 - 1.0) * self.row_gap.get()
                    };
                    (side(height), side(height), -1, -1)
                }
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = true;
            *vexpand = false;
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            let obj = self.obj();
            let rtl = obj.direction() == gtk::TextDirection::Rtl;
            let (columns, cell_width, cell_height) = self.cells(f64::from(width));
            let (gap, row_gap) = (self.gap.get(), self.row_gap.get());
            let curve = self.curve.get();
            self.slides.forget_gone(obj.upcast_ref());
            for (index, child) in self.visible().into_iter().enumerate() {
                let (column, row) = (index % columns, index / columns);
                let mut x = column as f64 * (cell_width + gap);
                if rtl {
                    x = f64::from(width) - x - cell_width;
                }
                let y = row as f64 * (cell_height + row_gap);
                // Content taller than its cell reaches into the gap below.
                let (least, _, _, _) = child.measure(gtk::Orientation::Vertical, side(cell_width));
                let height = cell_height.max(f64::from(least));
                let (dx, dy) = match curve {
                    Some(curve) => self.slides.offset(obj.upcast_ref(), &child, (x, y), curve),
                    None => (0.0, 0.0),
                };
                child.allocate(
                    side(cell_width),
                    side(height),
                    -1,
                    translate(x + dx, y + dy),
                );
            }
        }
    }
}

glib::wrapper! {
    /// Equal columns, as many as fit at the least width, every row as
    /// tall as the column width over the ratio plus the extra; items stand
    /// from the top of their cells.
    pub struct CellGrid(ObjectSubclass<cells::Cells>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// `Grid(cell_ratio:, cell_extra:)`.
pub(crate) fn cell_grid(
    p: &Parent,
    ctx: &Ctx,
    props: GridProps,
    ratio: f64,
    extra: Num,
    children: Build,
) {
    let grid: CellGrid = glib::Object::new();
    let imp = grid.imp();
    imp.min_width.set(120.0);
    imp.gap.set(8.0);
    imp.row_gap.set(8.0);
    imp.ratio.set(ratio);
    imp.curve.set(props.reflow);
    let lengths = [
        (props.min_width.clone(), 0),
        (props.gap.clone(), 1),
        (props.row_gap.clone(), 2),
        (Some(extra), 3),
    ];
    let own_rows = props.row_gap.is_some();
    for (length, which) in lengths {
        let Some(length) = length else { continue };
        let shown = grid.clone();
        bind_number(ctx, &length, props.animate, &grid, move |value| {
            let imp = shown.imp();
            match which {
                0 => imp.min_width.set(value),
                1 => {
                    imp.gap.set(value);
                    if !own_rows {
                        imp.row_gap.set(value);
                    }
                }
                2 => imp.row_gap.set(value),
                _ => imp.extra.set(value),
            }
            shown.queue_resize();
        });
    }
    pad(ctx, grid.upcast_ref(), &props.padding, props.animate);
    children(&Parent::of_children(&grid, false), ctx);
    place(p, &grid);
}

// MARK: - Modal

mod modal {
    use super::*;

    #[derive(Default)]
    pub struct Modal {
        /// The declared width and height; without a height, the card is
        /// as tall as its content.
        pub size: Cell<(f64, Option<f64>)>,
        pub cover: Cell<bool>,
        /// The modal's content has left and the frame goes with it.
        pub closing: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Modal {
        const NAME: &'static str = "BernModal";
        type Type = super::ModalFrame;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Dialog);
        }
    }

    impl ObjectImpl for Modal {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Modal {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        /// The card centered, as large as declared (without a height, as
        /// tall as its content at its width) and 16 points in from the
        /// edges at most, or the whole window when it covers it.
        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let obj = self.obj();
            let all = children(obj.upcast_ref());
            let (w, h) = self.size.get();
            let covers = self.cover.get();
            let card_width = if covers {
                f64::from(width)
            } else {
                w.min(f64::from(width) - 32.0).max(0.0)
            };
            for child in &all {
                if !child.should_layout() {
                    continue;
                }
                child.measure(gtk::Orientation::Horizontal, -1);
                let card_height = if covers {
                    f64::from(height)
                } else {
                    let wanted = h.unwrap_or_else(|| {
                        f64::from(
                            child
                                .measure(gtk::Orientation::Vertical, side(card_width))
                                .1,
                        )
                    });
                    wanted.min(f64::from(height) - 32.0).max(0.0)
                };
                let x = (f64::from(width) - card_width) / 2.0;
                let y = (f64::from(height) - card_height) / 2.0;
                child.allocate(side(card_width), side(card_height), -1, translate(x, y));
            }
            // Once the card has gone, so does the frame.
            if self.closing.get() && all.is_empty() {
                let frame = obj.clone();
                glib::idle_add_local_once(move || frame.remove());
            }
        }
    }
}

glib::wrapper! {
    /// A modal above the window: a backdrop over the whole window that
    /// takes its input, and the card on it.
    pub struct ModalFrame(ObjectSubclass<modal::Modal>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ModalFrame {
    /// A frame of `width` by `height` above `win`'s content, with its
    /// backdrop; the content below takes no focus while it stands.
    pub(crate) fn above(win: &crate::windows::Win, width: f64, height: Option<f64>) -> Self {
        let frame: Self = glib::Object::new();
        frame.imp().size.set((width, height));
        frame.add_css_class("bern-modal-backdrop");
        win.overlay.add_overlay(&frame);
        win.below.set_can_focus(false);
        let shown = frame.clone();
        glib::idle_add_local_once(move || shown.add_css_class("bern-shown"));
        frame
    }

    /// Whether a point of the frame lies outside its card.
    pub(crate) fn outside(&self, x: f64, y: f64) -> bool {
        self.pick(x, y, gtk::PickFlags::DEFAULT).as_ref() == Some(self.upcast_ref())
    }

    /// Leave the window's overlay; the content below takes the focus
    /// again when no other modal stands.
    pub(crate) fn remove(&self) {
        let Some(overlay) = self.parent().and_downcast::<gtk::Overlay>() else {
            return;
        };
        overlay.remove_overlay(self);
        let others = children(overlay.upcast_ref())
            .into_iter()
            .any(|child| child.is::<ModalFrame>());
        if !others && let Some(below) = overlay.child() {
            below.set_can_focus(true);
        }
    }
}

#[derive(Default)]
pub struct ModalProps {
    pub width: Option<Num>,
    pub height: Option<Num>,
    /// The whole window, edge to edge, where the window is compact.
    pub cover: bool,
    /// The motion of the card.
    pub decor: Option<Decor>,
}

/// A modal of the window, distinct from another window: the content below
/// takes no input while it stands. It stands while it is declared.
#[allow(non_snake_case)]
pub fn Modal(_p: &Parent, ctx: &Ctx, props: ModalProps, children: Build) {
    let win = Rc::clone(ctx.win());
    // Without a height, as tall as its content.
    let fits = props.height.is_none();
    let frame = ModalFrame::above(&win, 960.0, None);
    for (length, index) in [(props.width, 0), (props.height, 1)] {
        let Some(length) = length else { continue };
        let sized = frame.clone();
        bind_number(ctx, &length, None, &frame, move |value| {
            let (width, height) = sized.imp().size.get();
            sized.imp().size.set(if index == 0 {
                (value, height)
            } else {
                (width, Some(value))
            });
            sized.queue_allocate();
        });
    }
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("bern-modal");
    card.set_overflow(gtk::Overflow::Hidden);
    let name_key = crate::host::host().def.name;
    let named = frame.clone();
    effect(move || {
        named.update_property(&[gtk::accessible::Property::Label(&crate::text::tr(name_key))])
    });
    if props.cover {
        let at = ctx.clone();
        let covered = frame.clone();
        let shaped = card.clone();
        effect(move || {
            let on = crate::value::truthy(&at.window("compact"));
            covered.imp().cover.set(on);
            if on {
                shaped.add_css_class("bern-cover");
            } else {
                shaped.remove_css_class("bern-cover");
            }
            covered.queue_allocate();
        });
    }
    let inside = ctx.outside_toolbar();
    let inside = if fits { inside.fitting() } else { inside };
    children(&Parent::of_box(&card), &inside);
    let cards = Parent::of_children(&frame, false);
    let holder = card.clone();
    let add: Build = Rc::new(move |p, _| p.add(&holder));
    match props.decor {
        Some(decor) => crate::motion::Decorate(&cards, ctx, decor, add),
        None => add(&cards, ctx),
    }
    let focused = card.clone();
    glib::idle_add_local_once(move || {
        focused.child_focus(gtk::DirectionType::TabForward);
    });
    on_cleanup(move || {
        frame.remove_css_class("bern-shown");
        frame.imp().closing.set(true);
        // The card plays its disappearing transition, if it has one, and
        // the frame leaves after it.
        cards.clear();
        frame.queue_allocate();
    });
}

// MARK: - Timer

#[derive(Default)]
pub struct TimerProps {
    pub interval: f64,
    pub enabled: Option<Get>,
}

/// Runs its action every `interval` seconds while enabled and shown;
/// turning off or leaving cancels the next run.
#[allow(non_snake_case)]
pub fn Timer(_p: &Parent, ctx: &Ctx, props: TimerProps, action: Option<Act>) {
    let Some(action) = action else { return };
    let at = ctx.clone();
    let interval = props.interval;
    let enabled = props.enabled;
    effect(move || {
        if let Some(enabled) = &enabled
            && !crate::value::truthy(&enabled(&at))
        {
            return;
        }
        let delay = Duration::from_secs_f64(interval.max(0.05));
        let run = Rc::clone(&action);
        let ctx = at.clone();
        let source = glib::timeout_add_local(delay, move || {
            untrack(|| run(&ctx));
            glib::ControlFlow::Continue
        });
        let source = RefCell::new(Some(source));
        on_cleanup(move || {
            if let Some(source) = source.borrow_mut().take() {
                source.remove();
            }
        });
    });
}

// MARK: - Control groups

#[derive(Default)]
pub struct ControlGroupProps {
    pub label: Option<Str>,
    pub height: Option<Num>,
}

/// Controls joined into one capsule, as linked buttons.
#[allow(non_snake_case)]
pub fn ControlGroup(p: &Parent, ctx: &Ctx, props: ControlGroupProps, children: Build) {
    let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    group.add_css_class("linked");
    group.add_css_class("bern-control-group");
    group.set_valign(gtk::Align::Center);
    if let Some(label) = &props.label {
        let text = bound_text(ctx, label);
        let named = group.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&text())]));
    }
    crate::widgets::shape(
        ctx,
        group.upcast_ref(),
        &crate::widgets::Shape {
            capsule: true,
            height: Some(props.height.unwrap_or(Num::Fixed(40.0))),
        },
    );
    children(&Parent::of_aligned_box(&group, gtk::Align::Fill), ctx);
    place(p, &group);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(width: f64, height: f64) -> FlowItem {
        FlowItem {
            width,
            height,
            spacer: false,
        }
    }

    const SPACER_ITEM: FlowItem = FlowItem {
        width: 0.0,
        height: 0.0,
        spacer: true,
    };

    /// The plugin market's tool strip on a phone: a 123-wide segmented
    /// control, a spacer and five 54-wide chips in 358, 10 apart.
    fn strip() -> Vec<FlowItem> {
        let mut items = vec![item(123.0, 32.0), SPACER_ITEM];
        items.extend(std::iter::repeat_n(item(54.0, 30.0), 5));
        items
    }

    #[test]
    fn a_spacer_takes_the_rest_of_its_line_and_the_others_wrap() {
        let (frames, height) = flow_lines(&strip(), Some(358.0), 10.0);
        // The first line ends with three chips at its end, the spacer
        // taking the room before them; the other two start the next line.
        assert_eq!(frames[1], (133.0, 0.0, 33.0, 32.0));
        let chips: Vec<(f64, f64)> = frames[2..].iter().map(|f| (f.0, f.1)).collect();
        assert_eq!(
            chips,
            [
                (176.0, 1.0),
                (240.0, 1.0),
                (304.0, 1.0),
                (0.0, 42.0),
                (64.0, 42.0)
            ]
        );
        assert_eq!(height, 72.0);
    }

    #[test]
    fn a_wide_line_keeps_everything_on_it() {
        let (frames, height) = flow_lines(&strip(), Some(705.0), 10.0);
        assert_eq!(frames[1].2, 705.0 - 123.0 - 5.0 * 54.0 - 6.0 * 10.0);
        assert_eq!(frames[6].0 + frames[6].2, 705.0);
        assert_eq!(height, 32.0);
    }

    #[test]
    fn without_a_width_a_spacer_takes_nothing() {
        let (frames, height) = flow_lines(&strip(), None, 10.0);
        assert_eq!(frames[1].2, 0.0);
        assert_eq!(frames[6].0, 123.0 + 10.0 + 10.0 + 4.0 * 64.0);
        assert_eq!(height, 32.0);
        assert_eq!(flow_lines(&[], Some(100.0), 10.0), (vec![], 0.0));
    }

    #[test]
    fn a_scene_in_a_wider_box_has_room_at_its_sides() {
        // 1280 × 800 in 1920 × 800: the scene stands at its size, 320 to
        // each side; in 960 × 800 it shrinks to 0.75 and the room is above
        // and below.
        let wide = Room::around((1280.0, 800.0), (1920.0, 800.0), 1.0);
        assert_eq!((wide.x, wide.y), (320.0, 0.0));
        let tall = Room::around((1280.0, 800.0), (960.0, 800.0), 0.75);
        assert_eq!((tall.x, tall.y), (0.0, (800.0 / 0.75 - 800.0) / 2.0));
        let unscaled = Room::around((1280.0, 800.0), (0.0, 0.0), 0.0);
        assert_eq!((unscaled.x, unscaled.y), (0.0, 0.0));
    }

    #[test]
    fn a_layer_reaches_past_the_edges_it_touches() {
        let room = Room::around((1280.0, 800.0), (1920.0, 800.0), 1.0);
        // The whole scene reaches past both sides; the room above and
        // below is none.
        assert_eq!(
            room.reach((0.0, 0.0, 1280.0, 800.0)),
            [320.0, 320.0, 0.0, 0.0]
        );
        // Half a point short of an edge still reaches it.
        assert_eq!(
            room.reach((0.4, 0.0, 1279.2, 64.0)),
            [320.0, 320.0, 0.0, 0.0]
        );
        // A panel inside the scene stays where it is.
        assert_eq!(room.reach((24.0, 114.0, 896.0, 504.0)), [0.0; 4]);
        // A bar along the trailing edge reaches past it alone.
        assert_eq!(
            room.reach((1180.0, 0.0, 100.0, 800.0)),
            [0.0, 320.0, 0.0, 0.0]
        );
    }
}
