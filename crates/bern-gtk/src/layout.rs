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

mod layers {
    use super::*;

    #[derive(Default)]
    pub struct Layers;

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
            (minimum, natural, -1, -1)
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            for child in children(self.obj().upcast_ref()) {
                *hexpand |= child.compute_expand(gtk::Orientation::Horizontal);
                *vexpand |= child.compute_expand(gtk::Orientation::Vertical);
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let widget = self.obj();
            // A stack is as large as its largest layer; given more room than
            // that, it keeps its size at the leading edge unless a layer
            // fills it.
            let (_, natural_width, _, _) = self.measure(gtk::Orientation::Horizontal, -1);
            let area_width = if widget.compute_expand(gtk::Orientation::Horizontal) {
                width
            } else {
                width.min(natural_width.max(0))
            };
            let (_, natural_height, _, _) = self.measure(gtk::Orientation::Vertical, area_width);
            let area_height = if widget.compute_expand(gtk::Orientation::Vertical) {
                height
            } else {
                height.min(natural_height.max(0))
            };
            for child in children(widget.upcast_ref()) {
                if !child.should_layout() {
                    continue;
                }
                child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(area_width, area_height, -1, None);
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

mod rail {
    use super::*;

    #[derive(Default)]
    pub struct Rail {
        pub vertical: Cell<bool>,
        /// The point of an `at` child on its fraction: 0 its start, 0.5 its
        /// center, 1 its end.
        pub anchor: Cell<f64>,
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
            let fraction = |value: f64| {
                if value.is_finite() {
                    value.clamp(0.0, 1.0)
                } else {
                    0.0
                }
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
                let (start, size) = match placement.as_deref() {
                    Some(placement) => match placement.at.get() {
                        Some(at) => {
                            let size = natural_along.max(0) as f64;
                            (fraction(at) * length - self.anchor.get() * size, size)
                        }
                        None => {
                            let (a, b) =
                                (fraction(placement.from.get()), fraction(placement.to.get()));
                            (a.min(b) * length, (b - a).abs() * length)
                        }
                    },
                    None => (0.0, length),
                };
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
    widget.is::<Rail>()
        || (widget.is::<crate::motion::MotionBox>()
            && widget.first_child().is_some_and(|child| child.is::<Rail>()))
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
            self.layout().measure(&*self.obj(), orientation, for_size)
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
