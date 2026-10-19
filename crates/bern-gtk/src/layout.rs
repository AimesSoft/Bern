//! Containers GTK has no widget for: the layers of a `Stack`, a `Rail`
//! that places children at fractions of its length, and a clip with
//! rounded corners.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn children(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut children = Vec::new();
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        children.push(current);
    }
    children
}

fn translate(x: f32, y: f32) -> Option<gtk::gsk::Transform> {
    Some(gtk::gsk::Transform::new().translate(&gtk::graphene::Point::new(x, y)))
}

/// Where the children of a container were laid out, and the springs of
/// their way to new places: a child the layout moves slides there from
/// where it was, carrying on from where a slide in progress has it.
#[derive(Default)]
pub(crate) struct Slides {
    places: RefCell<Vec<SlidePlace>>,
}

struct SlidePlace {
    widget: glib::WeakRef<gtk::Widget>,
    at: (f64, f64),
    moving: Option<(crate::motion::Spring, crate::motion::Spring)>,
}

impl Slides {
    /// Forget children that left `container`.
    pub(crate) fn forget_gone(&self, container: &gtk::Widget) {
        self.places.borrow_mut().retain(|place| {
            place
                .widget
                .upgrade()
                .is_some_and(|widget| widget.parent().as_ref() == Some(container))
        });
    }

    /// The offset of `child`, laid out now at `now`, on its way there.
    pub(crate) fn offset(
        &self,
        container: &gtk::Widget,
        child: &gtk::Widget,
        now: (f64, f64),
        curve: crate::motion::Curve,
    ) -> (f64, f64) {
        let mut places = self.places.borrow_mut();
        let index = places
            .iter()
            .position(|place| place.widget.upgrade().as_ref() == Some(child));
        let Some(index) = index else {
            places.push(SlidePlace {
                widget: child.downgrade(),
                at: now,
                moving: None,
            });
            return (0.0, 0.0);
        };
        let place = &mut places[index];
        let (dx, dy) = (place.at.0 - now.0, place.at.1 - now.1);
        place.at = now;
        if (dx.abs() > 0.5 || dy.abs() > 0.5) && !crate::motion::reduced() {
            let weak = container.downgrade();
            let relayout = move |_| {
                if let Some(container) = weak.upgrade() {
                    container.queue_allocate();
                }
            };
            let (x, y) = place.moving.get_or_insert_with(|| {
                (
                    crate::motion::Spring::new(0.0, container, relayout.clone()),
                    crate::motion::Spring::new(0.0, container, relayout),
                )
            });
            x.set_quietly(x.value() + dx);
            y.set_quietly(y.value() + dy);
            x.to(0.0, Some(curve), None);
            y.to(0.0, Some(curve), None);
        }
        place
            .moving
            .as_ref()
            .map(|(x, y)| (x.value(), y.value()))
            .unwrap_or((0.0, 0.0))
    }
}

mod layers {
    use super::*;

    #[derive(Default)]
    pub struct Layers {
        /// With `reflow`, the spring layers slide to new places with.
        pub curve: Cell<Option<crate::motion::Curve>>,
        pub(crate) slides: Slides,
        /// Where layers that do not fill the stack stand in it.
        pub alignment: Cell<Option<(gtk::Align, gtk::Align)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Layers {
        const NAME: &'static str = "BernLayers";
        type Type = super::Layers;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Layers {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Layers {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (mut minimum, mut natural) = (0, 0);
            for child in children(self.obj().upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                let (child_minimum, child_natural, _, _) = child.measure(orientation, for_size);
                minimum = minimum.max(child_minimum);
                natural = natural.max(child_natural);
            }
            declared(
                self.obj().upcast_ref(),
                orientation,
                (minimum, natural, -1, -1),
            )
        }

        /// A stack fills where a layer fills, except a rail: it covers the
        /// size the other layers give the stack and makes it no larger. A
        /// stack of rails alone fills along them.
        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            let all = children(self.obj().upcast_ref());
            let content: Vec<&gtk::Widget> = all.iter().filter(|child| !is_rail(child)).collect();
            let counted: Vec<&gtk::Widget> = if content.is_empty() {
                all.iter().collect()
            } else {
                content
            };
            for child in counted {
                *hexpand |= child.compute_expand(gtk::Orientation::Horizontal);
                *vexpand |= child.compute_expand(gtk::Orientation::Vertical);
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let widget = self.obj();
            // A stack is as large as its largest layer, or its declared
            // size; given more room than that, it keeps its size at the
            // leading edge unless a layer fills it.
            let (_, natural_width, _, _) = self.measure(gtk::Orientation::Horizontal, -1);
            let area_width = if widget.compute_expand(gtk::Orientation::Horizontal) {
                width
            } else {
                width.min(natural_width.max(widget.width_request()).max(0))
            };
            let (_, natural_height, _, _) = self.measure(gtk::Orientation::Vertical, area_width);
            let area_height = if widget.compute_expand(gtk::Orientation::Vertical) {
                height
            } else {
                height.min(natural_height.max(widget.height_request()).max(0))
            };
            let curve = self.curve.get();
            if curve.is_some() {
                self.slides.forget_gone(widget.upcast_ref());
            }
            let (halign, valign) = self
                .alignment
                .get()
                .unwrap_or((gtk::Align::Center, gtk::Align::Center));
            for child in children(widget.upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                // A layer that fills in a direction, such as a drawing
                // without a size, fills the stack that way. A placed element
                // takes the whole size the other layers give the stack,
                // which it does not make larger. A rail takes the stack's
                // length along its axis; across it, it is as thick as its
                // children with a size of their own and stands where the
                // stack aligns its layers, as a progress bar at the foot of
                // a picture, or without such children covers the stack.
                let positioned = crate::scene::is_positioned(&child);
                let rail = rail_of(&child);
                let fills = |orientation: gtk::Orientation| {
                    if positioned {
                        return true;
                    }
                    match &rail {
                        Some(rail) => {
                            let along = if rail.vertical() {
                                gtk::Orientation::Vertical
                            } else {
                                gtk::Orientation::Horizontal
                            };
                            orientation == along || child.measure(orientation, -1).1 <= 0
                        }
                        None => child.compute_expand(orientation),
                    }
                };
                let along =
                    |fill: bool, align: gtk::Align| if fill { gtk::Align::Fill } else { align };
                let h = along(fills(gtk::Orientation::Horizontal), halign);
                let v = along(fills(gtk::Orientation::Vertical), valign);
                if child.halign() != h {
                    child.set_halign(h);
                }
                if child.valign() != v {
                    child.set_valign(v);
                }
                child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(area_width, area_height, -1, None);
                // A layer the layout moved slides from where it was.
                if let Some(curve) = curve
                    && let Some(point) =
                        child.compute_point(&*widget, &gtk::graphene::Point::zero())
                {
                    let (x, y) = (f64::from(point.x()), f64::from(point.y()));
                    let (dx, dy) = self
                        .slides
                        .offset(widget.upcast_ref(), &child, (x, y), curve);
                    if dx != 0.0 || dy != 0.0 {
                        child.allocate(
                            area_width,
                            area_height,
                            -1,
                            translate(dx as f32, dy as f32),
                        );
                    }
                }
            }
        }
    }
}

glib::wrapper! {
    /// The layers of a `Stack` in one place, later ones above earlier
    /// ones; each aligns by its own alignment.
    pub struct Layers(ObjectSubclass<layers::Layers>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Layers {
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Layers the layout moves slide to their new places with `curve`.
    pub(crate) fn set_reflow(&self, curve: Option<crate::motion::Curve>) {
        self.imp().curve.set(curve);
    }

    /// Where layers that do not fill the stack stand in it.
    pub(crate) fn set_alignment(&self, halign: gtk::Align, valign: gtk::Align) {
        if self.imp().alignment.replace(Some((halign, valign))) != Some((halign, valign)) {
            self.queue_allocate();
        }
    }

    pub(crate) fn insert_after(&self, widget: &gtk::Widget, previous: Option<&gtk::Widget>) {
        if widget.parent().as_ref() == Some(self.upcast_ref()) {
            widget.unparent();
        }
        widget.insert_after(self, previous);
    }

    pub(crate) fn remove(&self, widget: &gtk::Widget) {
        if widget.parent().as_ref() == Some(self.upcast_ref()) {
            widget.unparent();
        }
    }
}

impl Default for Layers {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a child stands on a rail: at a fraction of its length, or from
/// one fraction to another.
#[derive(Default)]
pub struct Placement {
    pub(crate) at: Cell<Option<f64>>,
    pub(crate) from: Cell<f64>,
    pub(crate) to: Cell<f64>,
}

/// The start and length along a rail of `length` of a child placed by
/// `placement`, whose own length is `natural`; the scale runs `past` beyond
/// both ends, and `anchor` is the point of an `at` child on its fraction. A
/// child without a placement spans the rail.
fn span(
    placement: Option<&Placement>,
    length: f64,
    past: f64,
    anchor: f64,
    natural: i32,
) -> (f64, f64) {
    let fraction = |value: f64| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let scale = length + 2.0 * past;
    match placement {
        Some(placement) => match placement.at.get() {
            Some(at) => {
                let size = natural.max(0) as f64;
                (fraction(at) * scale - past - anchor * size, size)
            }
            None => {
                let (a, b) = (fraction(placement.from.get()), fraction(placement.to.get()));
                (a.min(b) * scale - past, (b - a).abs() * scale)
            }
        },
        None => (0.0, length),
    }
}

mod rail {
    use super::*;

    #[derive(Default)]
    pub struct Rail {
        pub vertical: Cell<bool>,
        /// The point of an `at` child on its fraction: 0 its start, 0.5 its
        /// center, 1 its end.
        pub anchor: Cell<f64>,
        /// How far the scale runs past each end.
        pub overhang: Cell<f64>,
        pub placements: RefCell<Vec<(glib::WeakRef<gtk::Widget>, Rc<Placement>)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Rail {
        const NAME: &'static str = "BernRail";
        type Type = super::Rail;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Rail {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl Rail {
        fn placement(&self, child: &gtk::Widget) -> Option<Rc<Placement>> {
            self.placements
                .borrow()
                .iter()
                .find(|(widget, _)| widget.upgrade().as_ref() == Some(child))
                .map(|(_, placement)| Rc::clone(placement))
        }
    }

    impl WidgetImpl for Rail {
        /// Along its axis a rail fills; across it, children without a size
        /// of their own fill the thickness the rail gets, but do not ask the
        /// containers around it for more.
        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = !self.vertical.get();
            *vexpand = self.vertical.get();
        }

        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let along = if self.vertical.get() {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            };
            // Along its axis a rail takes the length it gets.
            if orientation == along {
                return (0, 0, -1, -1);
            }
            let (mut minimum, mut natural) = (0, 0);
            for child in children(self.obj().upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                let (child_minimum, child_natural, _, _) = child.measure(orientation, -1);
                minimum = minimum.max(child_minimum);
                natural = natural.max(child_natural);
            }
            (minimum, natural, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let vertical = self.vertical.get();
            let (length, thickness) = if vertical {
                (height as f64, height_or(width))
            } else {
                (width as f64, height_or(height))
            };
            let (along, across) = if vertical {
                (gtk::Orientation::Vertical, gtk::Orientation::Horizontal)
            } else {
                (gtk::Orientation::Horizontal, gtk::Orientation::Vertical)
            };
            for child in children(self.obj().upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                let placement = self.placement(&child);
                let (_, natural_along, _, _) = child.measure(along, -1);
                let (_, natural_across, _, _) = child.measure(across, -1);
                // Across, a child without a size of its own fills the rail.
                let size_across = if natural_across <= 0 || child.compute_expand(across) {
                    thickness
                } else {
                    natural_across as f64
                };
                let start_across = (thickness - size_across) / 2.0;
                let (start, size) = span(
                    placement.as_deref(),
                    length,
                    self.overhang.get(),
                    self.anchor.get(),
                    natural_along,
                );
                let (x, y, w, h) = if vertical {
                    (start_across, start, size_across, size)
                } else {
                    (start, start_across, size, size_across)
                };
                child.allocate(
                    w.round().max(0.0) as i32,
                    h.round().max(0.0) as i32,
                    -1,
                    translate(x as f32, y as f32),
                );
            }
        }
    }

    fn height_or(value: i32) -> f64 {
        value.max(0) as f64
    }
}

glib::wrapper! {
    /// A strip that places each child at a fraction of its length or
    /// between two. Children may reach past its ends; it takes no clicks
    /// of its own.
    pub struct Rail(ObjectSubclass<rail::Rail>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// Whether `widget` is a rail, or an element wrapping one in its motion.
pub(crate) fn is_rail(widget: &gtk::Widget) -> bool {
    rail_of(widget).is_some()
}

/// The rail `widget` is, or holds as the box that moves it.
fn rail_of(widget: &gtk::Widget) -> Option<Rail> {
    if let Ok(rail) = widget.clone().downcast::<Rail>() {
        return Some(rail);
    }
    if widget.is::<crate::motion::MotionBox>() {
        return widget.first_child().and_then(|child| child.downcast::<Rail>().ok());
    }
    None
}

impl Rail {
    pub fn new(vertical: bool, anchor: f64) -> Self {
        let rail: Self = glib::Object::new();
        rail.imp().vertical.set(vertical);
        rail.imp().anchor.set(anchor);
        if vertical {
            rail.set_vexpand(true);
        } else {
            rail.set_hexpand(true);
        }
        rail
    }

    pub(crate) fn vertical(&self) -> bool {
        self.imp().vertical.get()
    }

    /// How far the scale runs past each end.
    pub(crate) fn set_overhang(&self, overhang: f64) {
        if self.imp().overhang.replace(overhang) != overhang {
            self.queue_allocate();
        }
    }

    pub(crate) fn insert_after(&self, widget: &gtk::Widget, previous: Option<&gtk::Widget>) {
        if widget.parent().as_ref() == Some(self.upcast_ref()) {
            widget.unparent();
        }
        widget.insert_after(self, previous);
    }

    pub(crate) fn remove(&self, widget: &gtk::Widget) {
        if widget.parent().as_ref() == Some(self.upcast_ref()) {
            widget.unparent();
        }
        self.imp()
            .placements
            .borrow_mut()
            .retain(|(candidate, _)| candidate.upgrade().is_some_and(|kept| &kept != widget));
    }

    /// Where `widget` stands; it moves when the placement changes and the
    /// rail lays out again.
    pub(crate) fn place(&self, widget: &gtk::Widget, placement: Rc<Placement>) {
        let mut placements = self.imp().placements.borrow_mut();
        placements.retain(|(candidate, _)| candidate.upgrade().is_some());
        placements.push((widget.downgrade(), placement));
    }
}

mod clip {
    use super::*;

    #[derive(Default)]
    pub struct Clip {
        pub radius: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Clip {
        const NAME: &'static str = "BernClip";
        type Type = super::Clip;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Clip {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Clip {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            self.obj()
                .first_child()
                .map(|child| child.request_mode())
                .unwrap_or(gtk::SizeRequestMode::ConstantSize)
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match self.obj().first_child() {
                Some(child) if child.should_layout() => child.measure(orientation, for_size),
                _ => (0, 0, -1, -1),
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            if let Some(child) = self.obj().first_child() {
                *hexpand = child.compute_expand(gtk::Orientation::Horizontal);
                *vexpand = child.compute_expand(gtk::Orientation::Vertical);
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child()
                && child.should_layout()
            {
                child.allocate(width, height, baseline, None);
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let radius = self.radius.get().max(0.0) as f32;
            if radius <= 0.0 {
                self.parent_snapshot(snapshot);
                return;
            }
            let bounds =
                gtk::graphene::Rect::new(0.0, 0.0, widget.width() as f32, widget.height() as f32);
            snapshot.push_rounded_clip(&gtk::gsk::RoundedRect::from_rect(bounds, radius));
            self.parent_snapshot(snapshot);
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    /// Its child, clipped to a rectangle with rounded corners.
    pub struct Clip(ObjectSubclass<clip::Clip>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Clip {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let clip: Self = glib::Object::new();
        child.as_ref().set_parent(&clip);
        clip
    }

    pub fn set_radius(&self, radius: f64) {
        if (self.imp().radius.get() - radius).abs() > f64::EPSILON {
            self.imp().radius.set(radius);
            self.queue_draw();
        }
    }
}

mod reflow {
    use super::*;
    use crate::motion::{Curve, Spring};

    /// Where a child was laid out last, and the springs of its way there.
    pub(crate) struct Place {
        pub(crate) widget: glib::WeakRef<gtk::Widget>,
        pub(crate) at: (f64, f64),
        pub(crate) moving: Option<(Spring, Spring)>,
    }

    #[derive(Default)]
    pub struct ReflowBox {
        /// The layout of a box: GTK's own, used for this widget.
        pub(crate) layout: std::cell::OnceCell<gtk::BoxLayout>,
        pub(crate) curve: Cell<Option<Curve>>,
        pub(crate) places: RefCell<Vec<Place>>,
        pub(crate) allocating: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReflowBox {
        const NAME: &'static str = "BernReflowBox";
        type Type = super::ReflowBox;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_css_name("box");
        }
    }

    impl ObjectImpl for ReflowBox {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl ReflowBox {
        pub(crate) fn layout(&self) -> &gtk::BoxLayout {
            self.layout
                .get_or_init(|| gtk::BoxLayout::new(gtk::Orientation::Vertical))
        }
    }

    impl WidgetImpl for ReflowBox {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            let height_for_width = children(self.obj().upcast_ref())
                .iter()
                .any(|child| child.request_mode() != gtk::SizeRequestMode::ConstantSize);
            if height_for_width {
                gtk::SizeRequestMode::HeightForWidth
            } else {
                gtk::SizeRequestMode::ConstantSize
            }
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            declared(
                self.obj().upcast_ref(),
                orientation,
                self.layout().measure(&*self.obj(), orientation, for_size),
            )
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.layout()
                .allocate(&*self.obj(), width, height, baseline);
            let Some(curve) = self.curve.get() else {
                return;
            };
            self.allocating.set(true);
            self.slide(curve);
            self.allocating.set(false);
        }
    }

    impl ReflowBox {
        /// Children the layout moved slide from where they were, carrying
        /// on from where a slide in progress has them.
        fn slide(&self, curve: Curve) {
            let obj = self.obj();
            let mut places = self.places.borrow_mut();
            places.retain(|place| {
                place
                    .widget
                    .upgrade()
                    .is_some_and(|widget| widget.parent().as_ref() == Some(obj.upcast_ref()))
            });
            for child in children(obj.upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                let Some(point) = child.compute_point(&*obj, &gtk::graphene::Point::zero()) else {
                    continue;
                };
                let now = (f64::from(point.x()), f64::from(point.y()));
                let index = places
                    .iter()
                    .position(|place| place.widget.upgrade().as_ref() == Some(&child));
                let Some(index) = index else {
                    places.push(Place {
                        widget: child.downgrade(),
                        at: now,
                        moving: None,
                    });
                    continue;
                };
                let place = &mut places[index];
                let (dx, dy) = (place.at.0 - now.0, place.at.1 - now.1);
                place.at = now;
                if (dx.abs() > 0.5 || dy.abs() > 0.5) && !crate::motion::reduced() {
                    let weak = obj.downgrade();
                    let relayout = move |_| {
                        if let Some(reflow) = weak.upgrade()
                            && !reflow.imp().allocating.get()
                        {
                            reflow.queue_allocate();
                        }
                    };
                    let (x, y) = place.moving.get_or_insert_with(|| {
                        (
                            Spring::new(0.0, &*obj, relayout.clone()),
                            Spring::new(0.0, &*obj, relayout),
                        )
                    });
                    x.set_quietly(x.value() + dx);
                    y.set_quietly(y.value() + dy);
                    x.to(0.0, Some(curve), None);
                    y.to(0.0, Some(curve), None);
                }
                if let Some((x, y)) = &place.moving {
                    let (ox, oy) = (x.value(), y.value());
                    if ox != 0.0 || oy != 0.0 {
                        if crate::motion::log_motion() {
                            eprintln!(
                                "bern-reflow {} {} offset ({ox:.1}, {oy:.1})",
                                glib::monotonic_time() / 1000,
                                child.type_().name()
                            );
                        }
                        child.allocate(
                            child.width(),
                            child.height(),
                            -1,
                            translate((now.0 + ox) as f32, (now.1 + oy) as f32),
                        );
                    }
                }
            }
        }
    }
}

glib::wrapper! {
    /// A column or row whose children slide to their new places when the
    /// layout moves them: laid out by GTK's box layout, then each moved
    /// child offset by a spring that returns to zero.
    pub struct ReflowBox(ObjectSubclass<reflow::ReflowBox>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ReflowBox {
    pub fn new(orientation: gtk::Orientation, spacing: i32, curve: crate::motion::Curve) -> Self {
        let reflow: Self = glib::Object::new();
        let layout = reflow.imp().layout();
        layout.set_orientation(orientation);
        layout.set_spacing(spacing.max(0) as u32);
        reflow.imp().curve.set(Some(curve));
        reflow
    }

    pub(crate) fn horizontal(&self) -> bool {
        self.imp().layout().orientation() == gtk::Orientation::Horizontal
    }

    pub(crate) fn set_spacing(&self, spacing: i32) {
        self.imp().layout().set_spacing(spacing.max(0) as u32);
        self.queue_resize();
    }

    pub(crate) fn insert_after(&self, widget: &gtk::Widget, previous: Option<&gtk::Widget>) {
        if widget.parent().as_ref() == Some(self.upcast_ref()) {
            widget.unparent();
        }
        widget.insert_after(self, previous);
    }

    pub(crate) fn remove(&self, widget: &gtk::Widget) {
        if widget.parent().as_ref() == Some(self.upcast_ref()) {
            widget.unparent();
        }
    }
}

mod offered {
    use super::*;

    /// What learns the width offered.
    pub type Changed = Rc<dyn Fn(i32)>;

    #[derive(Default)]
    pub struct Offered {
        pub width: Cell<i32>,
        pub changed: RefCell<Option<Changed>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Offered {
        const NAME: &'static str = "BernOffered";
        type Type = super::Offered;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Offered {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Offered {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        /// Across, it asks for nothing: the width it gets is what its
        /// container offers, not what the layout it shows needs.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self
                .obj()
                .first_child()
                .filter(|child| child.should_layout())
            else {
                return (0, 0, -1, -1);
            };
            match orientation {
                gtk::Orientation::Horizontal => {
                    let (_, natural, _, _) = child.measure(orientation, -1);
                    (0, natural, -1, -1)
                }
                _ => {
                    let (least, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                    let width = if for_size < 0 {
                        -1
                    } else {
                        for_size.max(least)
                    };
                    let (minimum, natural, _, _) = child.measure(orientation, width);
                    (minimum, natural, -1, -1)
                }
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = true;
            if let Some(child) = self.obj().first_child() {
                *vexpand = child.compute_expand(gtk::Orientation::Vertical);
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child()
                && child.should_layout()
            {
                // A layout wider than the room it gets overflows, clipped,
                // until the width chooses another.
                let (least, _, _, _) = child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(width.max(least), height, baseline, None);
            }
            if width > 0 && self.width.replace(width) != width {
                let changed = self.changed.borrow().clone();
                if let Some(changed) = changed {
                    // After this layout pass, which the change may redo.
                    glib::idle_add_local_once(move || changed(width));
                }
            }
        }
    }
}

glib::wrapper! {
    /// One child, laid out in the width its container offers: it tells
    /// that width, which layouts choose by, and never asks for the width
    /// its child needs.
    pub struct Offered(ObjectSubclass<offered::Offered>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Offered {
    pub fn new(child: &impl IsA<gtk::Widget>, changed: impl Fn(i32) + 'static) -> Self {
        let offered: Self = glib::Object::new();
        offered.set_overflow(gtk::Overflow::Hidden);
        child.as_ref().set_parent(&offered);
        *offered.imp().changed.borrow_mut() = Some(Rc::new(changed));
        offered
    }
}

/// A measurement of a container with a declared side: it never asks for
/// more than that side, though what it holds may need more.
pub(crate) fn declared(
    widget: &gtk::Widget,
    orientation: gtk::Orientation,
    (minimum, natural, minimum_baseline, natural_baseline): (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let side = match orientation {
        gtk::Orientation::Horizontal => widget.width_request(),
        _ => widget.height_request(),
    };
    if side < 0 {
        return (minimum, natural, minimum_baseline, natural_baseline);
    }
    (
        minimum,
        natural.min(side).max(minimum),
        minimum_baseline,
        natural_baseline,
    )
}

mod sized {
    use super::*;

    #[derive(Default)]
    pub struct Sized;

    #[glib::object_subclass]
    impl ObjectSubclass for Sized {
        const NAME: &'static str = "BernSized";
        type Type = super::Sized;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Sized {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Sized {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            self.obj()
                .first_child()
                .map(|child| child.request_mode())
                .unwrap_or(gtk::SizeRequestMode::ConstantSize)
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let measured = match self.obj().first_child() {
                Some(child) if child.should_layout() => child.measure(orientation, for_size),
                _ => (0, 0, -1, -1),
            };
            declared(self.obj().upcast_ref(), orientation, measured)
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            if let Some(child) = self.obj().first_child() {
                *hexpand = child.compute_expand(gtk::Orientation::Horizontal);
                *vexpand = child.compute_expand(gtk::Orientation::Vertical);
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child()
                && child.should_layout()
            {
                child.allocate(width, height, baseline, None);
            }
        }
    }
}

glib::wrapper! {
    /// One child at a declared width or height: what it asks for is that
    /// side, not only the least it takes, whatever the child would like.
    pub struct Sized(ObjectSubclass<sized::Sized>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Sized {
    pub fn wrap(child: &impl IsA<gtk::Widget>) -> Self {
        let sized: Self = glib::Object::new();
        child.as_ref().set_parent(&sized);
        sized
    }
}

mod words {
    use super::*;

    #[derive(Default)]
    pub struct Words;

    #[glib::object_subclass]
    impl ObjectSubclass for Words {
        const NAME: &'static str = "BernWords";
        type Type = super::Words;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Generic);
        }
    }

    impl ObjectImpl for Words {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Words {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        /// The width text would like is that of its lines unwrapped,
        /// whatever height a row beside it offers; it wraps only where
        /// the width it gets is less.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match self.obj().first_child() {
                Some(child) if child.should_layout() => match orientation {
                    gtk::Orientation::Horizontal => child.measure(orientation, -1),
                    _ => child.measure(orientation, for_size),
                },
                _ => (0, 0, -1, -1),
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            if let Some(child) = self.obj().first_child() {
                *hexpand = child.compute_expand(gtk::Orientation::Horizontal);
                *vexpand = child.compute_expand(gtk::Orientation::Vertical);
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child()
                && child.should_layout()
            {
                child.allocate(width, height, baseline, None);
            }
        }
    }
}

glib::wrapper! {
    /// Text that takes the width of its lines before it wraps.
    pub struct Words(ObjectSubclass<words::Words>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Words {
    pub fn wrap(child: &impl IsA<gtk::Widget>) -> Self {
        let words: Self = glib::Object::new();
        child.as_ref().set_parent(&words);
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(fraction: f64) -> Placement {
        let placement = Placement::default();
        placement.at.set(Some(fraction));
        placement
    }

    fn between(from: f64, to: f64) -> Placement {
        let placement = Placement::default();
        placement.from.set(from);
        placement.to.set(to);
        placement
    }

    #[test]
    fn children_without_a_placement_span_the_rail() {
        assert_eq!(span(None, 300.0, 0.0, 0.5, 40), (0.0, 300.0));
    }

    #[test]
    fn children_at_a_fraction_stand_on_the_anchor() {
        assert_eq!(span(Some(&at(0.25)), 200.0, 0.0, 0.0, 20), (50.0, 20.0));
        assert_eq!(span(Some(&at(0.25)), 200.0, 0.0, 0.5, 20), (40.0, 20.0));
        assert_eq!(span(Some(&at(1.0)), 200.0, 0.0, 1.0, 20), (180.0, 20.0));
        // Fractions past the ends clamp to them, one that is not a number
        // is the start; the child may reach past the rail.
        assert_eq!(span(Some(&at(1.5)), 200.0, 0.0, 0.5, 20), (190.0, 20.0));
        assert_eq!(
            span(Some(&at(f64::NAN)), 200.0, 0.0, 0.5, 20),
            (-10.0, 20.0)
        );
        // A child that measures nothing has no length.
        assert_eq!(span(Some(&at(0.5)), 200.0, 0.0, 0.5, -1), (100.0, 0.0));
    }

    #[test]
    fn an_overhang_runs_the_scale_past_both_ends() {
        // 0 stands 50 before the start and 1 50 past the end.
        assert_eq!(span(Some(&at(0.0)), 200.0, 50.0, 0.0, 20), (-50.0, 20.0));
        assert_eq!(span(Some(&at(0.5)), 200.0, 50.0, 0.5, 20), (90.0, 20.0));
        assert_eq!(span(Some(&at(1.0)), 200.0, 50.0, 1.0, 20), (230.0, 20.0));
        assert_eq!(
            span(Some(&between(0.25, 0.75)), 200.0, 50.0, 0.5, 20),
            (25.0, 150.0)
        );
        // A child without a placement still spans the rail itself.
        assert_eq!(span(None, 200.0, 50.0, 0.5, 20), (0.0, 200.0));
    }

    #[test]
    fn children_between_fractions_fill_the_range() {
        assert_eq!(
            span(Some(&between(0.25, 0.75)), 200.0, 0.0, 0.5, 20),
            (50.0, 100.0)
        );
        assert_eq!(
            span(Some(&between(0.75, 0.25)), 200.0, 0.0, 0.5, 20),
            (50.0, 100.0)
        );
        assert_eq!(
            span(Some(&between(-0.5, 2.0)), 200.0, 0.0, 0.5, 20),
            (0.0, 200.0)
        );
    }
}
