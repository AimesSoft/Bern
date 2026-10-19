//! Springs and the motion of elements.
//!
//! A spring is libadwaita's `AdwSpringAnimation` with Bern's period and
//! damping ratio (mass 1, stiffness (2π / T)²): a new target starts from
//! the current value and velocity, so motion can be interrupted without a
//! jump. An element that moves is wrapped in a [`MotionBox`], which lays
//! out exactly as its child and gives the child a transform when it
//! allocates it, so the motion changes neither the child's drawing nor
//! its neighbours' places. The transform is the sum of the moves and the
//! product of the scales and opacities of independent layers — a style's
//! state, an appearing transition, a shared element, a nudge, scroll
//! effects — so each springs on its own.

use crate::protocol::Value;
use crate::reactive::{effect, on_cleanup, untrack};
use crate::{Build, Ctx, Get, Parent};
use adw::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

/// A spring: period in seconds and damping ratio.
pub type Curve = (f64, f64);

/// The quick fade transitions become with less motion.
const QUICK_FADE: Curve = (0.15, 1.0);
/// The push of a nudge.
const NUDGE_OUT: Curve = (0.12, 1.0);

/// Development aid: `BERN_MOTION_LOG` prints every transform an element
/// takes, with the time in milliseconds, to check motion frame by frame.
pub(crate) fn log_motion() -> bool {
    thread_local! {
        static LOG: bool = std::env::var_os("BERN_MOTION_LOG").is_some();
    }
    LOG.with(|log| *log)
}

thread_local! {
    /// `reduce_motion` of the elements showing now: any that is on
    /// reduces motion everywhere.
    static PREFERENCES: RefCell<Vec<(u64, bool)>> = const { RefCell::new(Vec::new()) };
    static NEXT_PREFERENCE: Cell<u64> = const { Cell::new(1) };
}

/// Whether motion is reduced: `BERN_MOTION=reduce`, animations off in the
/// GTK settings (libadwaita then skips every animation), or an element's
/// `reduce_motion` on.
pub fn reduced() -> bool {
    std::env::var("BERN_MOTION").as_deref() == Ok("reduce")
        || gtk::Settings::default().is_some_and(|settings| !settings.is_gtk_enable_animations())
        || PREFERENCES.with(|preferences| preferences.borrow().iter().any(|(_, on)| *on))
}

/// `reduce_motion`: while `when` holds, motion is reduced, until the
/// current scope ends.
fn prefer_reduced(ctx: &Ctx, when: Get) {
    let token = NEXT_PREFERENCE.with(|next| {
        let token = next.get();
        next.set(token + 1);
        token
    });
    PREFERENCES.with(|preferences| preferences.borrow_mut().push((token, false)));
    let at = ctx.clone();
    effect(move || {
        let on = crate::value::truthy(&when(&at));
        PREFERENCES.with(|preferences| {
            if let Some(entry) = preferences
                .borrow_mut()
                .iter_mut()
                .find(|(id, _)| *id == token)
            {
                entry.1 = on;
            }
        });
    });
    on_cleanup(move || {
        PREFERENCES.with(|preferences| preferences.borrow_mut().retain(|(id, _)| *id != token));
    });
}

struct SpringInner {
    value: Cell<f64>,
    target: Cell<f64>,
    widget: glib::WeakRef<gtk::Widget>,
    apply: Box<dyn Fn(f64)>,
    running: RefCell<Option<(adw::SpringAnimation, glib::SignalHandlerId)>>,
    done: RefCell<Option<Box<dyn FnOnce()>>>,
}

/// A value that springs to its targets, timed by a widget's frame clock.
#[derive(Clone)]
pub(crate) struct Spring(Rc<SpringInner>);

impl Spring {
    pub(crate) fn new(
        value: f64,
        widget: &impl IsA<gtk::Widget>,
        apply: impl Fn(f64) + 'static,
    ) -> Self {
        Self(Rc::new(SpringInner {
            value: Cell::new(value),
            target: Cell::new(value),
            widget: widget.as_ref().downgrade(),
            apply: Box::new(apply),
            running: RefCell::new(None),
            done: RefCell::new(None),
        }))
    }

    pub(crate) fn value(&self) -> f64 {
        self.0.value.get()
    }

    pub(crate) fn target(&self) -> f64 {
        self.0.target.get()
    }

    /// End any motion; returns the velocity it had.
    fn stop(&self) -> f64 {
        let running = self.0.running.borrow_mut().take();
        match running {
            Some((animation, handler)) => {
                let velocity = animation.velocity();
                animation.disconnect(handler);
                animation.pause();
                velocity
            }
            None => 0.0,
        }
    }

    fn finish(&self) {
        let done = self.0.done.borrow_mut().take();
        if let Some(done) = done {
            done();
        }
    }

    /// Change the value without applying it, ending any motion; for a
    /// spring its owner reads while laying out.
    pub(crate) fn set_quietly(&self, value: f64) {
        self.stop();
        self.0.value.set(value);
        self.0.target.set(value);
    }

    /// Jump to `value`, ending any motion.
    pub(crate) fn set(&self, value: f64) {
        self.stop();
        self.0.value.set(value);
        self.0.target.set(value);
        (self.0.apply)(value);
        self.finish();
    }

    /// Spring to `target` with `curve`, or jump there without one; `done`
    /// runs when it arrives or another target replaces this one.
    pub(crate) fn to(&self, target: f64, curve: Option<Curve>, done: Option<Box<dyn FnOnce()>>) {
        self.finish();
        let velocity = self.stop();
        self.0.target.set(target);
        *self.0.done.borrow_mut() = done;
        let widget = self.0.widget.upgrade();
        let still = (target - self.value()).abs() < 1e-4 && velocity.abs() < 1e-3;
        let (Some((period, damping)), Some(widget), false) = (curve, widget, still) else {
            self.0.value.set(target);
            (self.0.apply)(target);
            self.finish();
            return;
        };
        let period = period.max(0.01);
        let stiffness = (2.0 * std::f64::consts::PI / period).powi(2);
        let params = adw::SpringParams::new(damping.max(0.0), 1.0, stiffness);
        // The running animation keeps the spring, so a spring nothing else
        // holds still arrives; the animation is dropped when it ends.
        let inner = Rc::clone(&self.0);
        let animation = adw::SpringAnimation::new(
            &widget,
            self.value(),
            target,
            params,
            adw::CallbackAnimationTarget::new(move |value| {
                inner.value.set(value);
                (inner.apply)(value);
            }),
        );
        animation.set_initial_velocity(velocity);
        animation.set_epsilon(0.0005);
        let weak = Rc::downgrade(&self.0);
        let handler = animation.connect_done(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.running.borrow_mut().take();
                let spring = Spring(inner);
                let target = spring.target();
                spring.0.value.set(target);
                (spring.0.apply)(target);
                spring.finish();
            }
        });
        *self.0.running.borrow_mut() = Some((animation.clone(), handler));
        animation.play();
    }

    /// A push, as a nudge gives: out with a quick spring, back with `curve`.
    pub(crate) fn bump(&self, to: f64, curve: Curve) {
        let back = self.clone();
        let rest = self.target();
        self.to(
            to,
            Some(NUDGE_OUT),
            Some(Box::new(move || back.to(rest, Some(curve), None))),
        );
    }
}

/// The five springs of one layer of an element's transform, and the
/// point of the element its scale keeps in place, as fractions of its
/// size: the center unless a transition grows from an edge.
#[derive(Clone)]
pub struct Layer {
    pub(crate) x: Spring,
    pub(crate) y: Spring,
    pub(crate) sx: Spring,
    pub(crate) sy: Spring,
    pub(crate) opacity: Spring,
    pub(crate) pivot: Rc<Cell<(f64, f64)>>,
}

impl Layer {
    fn springs(&self) -> [&Spring; 5] {
        [&self.x, &self.y, &self.sx, &self.sy, &self.opacity]
    }

    fn set(&self, values: Pose) {
        self.x.set(values.x);
        self.y.set(values.y);
        self.sx.set(values.sx);
        self.sy.set(values.sy);
        self.opacity.set(values.opacity);
    }
}

/// Values of a layer.
#[derive(Clone, Copy)]
struct Pose {
    x: f64,
    y: f64,
    sx: f64,
    sy: f64,
    opacity: f64,
}

const REST: Pose = Pose {
    x: 0.0,
    y: 0.0,
    sx: 1.0,
    sy: 1.0,
    opacity: 1.0,
};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MotionBox {
        pub layers: RefCell<Vec<(&'static str, Layer)>>,
        /// The move and scale the child gets.
        pub transform: Cell<(f64, f64, f64, f64)>,
        pub allocating: Cell<bool>,
        /// Run after the next allocation, with the box laid out.
        pub laid_out: RefCell<Vec<Box<dyn FnOnce()>>>,
        /// Run after every allocation.
        pub watchers: RefCell<Vec<Rc<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MotionBox {
        const NAME: &'static str = "BernMotion";
        type Type = super::MotionBox;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Generic);
        }
    }

    impl ObjectImpl for MotionBox {
        fn constructed(&self) {
            self.parent_constructed();
            self.transform.set((0.0, 0.0, 1.0, 1.0));
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for MotionBox {
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
            self.allocating.set(true);
            let pending: Vec<_> = self.laid_out.borrow_mut().drain(..).collect();
            for run in pending {
                run();
            }
            let watchers: Vec<_> = self.watchers.borrow().clone();
            for watch in watchers {
                watch();
            }
            self.allocating.set(false);
            let Some(child) = self.obj().first_child() else {
                return;
            };
            if !child.should_layout() {
                return;
            }
            let (x, y, sx, sy) = self.obj().transform_at(f64::from(width), f64::from(height));
            self.transform.set((x, y, sx, sy));
            let (w, h) = (width as f32 / 2.0, height as f32 / 2.0);
            let transform = if x == 0.0 && y == 0.0 && sx == 1.0 && sy == 1.0 {
                None
            } else {
                Some(
                    gtk::gsk::Transform::new()
                        .translate(&gtk::graphene::Point::new(w + x as f32, h + y as f32))
                        .scale(sx as f32, sy as f32)
                        .translate(&gtk::graphene::Point::new(-w, -h)),
                )
            };
            child.allocate(width, height, baseline, transform);
        }
    }
}

glib::wrapper! {
    /// One child, laid out as the child is, moved, scaled and faded by
    /// springs.
    pub struct MotionBox(ObjectSubclass<imp::MotionBox>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MotionBox {
    /// Wrap `child`, taking over its alignment so the transform's center is
    /// the child's own.
    pub fn wrap(child: &impl IsA<gtk::Widget>) -> Self {
        let child = child.as_ref();
        let motion: Self = glib::Object::new();
        motion.set_halign(child.halign());
        motion.set_valign(child.valign());
        child.set_halign(gtk::Align::Fill);
        child.set_valign(gtk::Align::Fill);
        child.set_parent(&motion);
        motion
    }

    /// The named layer of this element's transform, made at rest.
    pub(crate) fn layer(&self, name: &'static str) -> Layer {
        if let Some((_, layer)) = self
            .imp()
            .layers
            .borrow()
            .iter()
            .find(|(existing, _)| *existing == name)
        {
            return layer.clone();
        }
        let spring = |value: f64| {
            let weak = self.downgrade();
            Spring::new(value, self, move |_| {
                if let Some(motion) = weak.upgrade() {
                    motion.write();
                }
            })
        };
        let layer = Layer {
            x: spring(0.0),
            y: spring(0.0),
            sx: spring(1.0),
            sy: spring(1.0),
            opacity: spring(1.0),
            pivot: Rc::new(Cell::new((0.5, 0.5))),
        };
        self.imp().layers.borrow_mut().push((name, layer.clone()));
        layer
    }

    /// The move and scale of the child at this size: each layer's scale
    /// keeps its pivot in place.
    fn transform_at(&self, width: f64, height: f64) -> (f64, f64, f64, f64) {
        let (mut x, mut y, mut sx, mut sy) = (0.0, 0.0, 1.0, 1.0);
        for (_, layer) in self.imp().layers.borrow().iter() {
            let (lsx, lsy) = (layer.sx.value(), layer.sy.value());
            let (px, py) = layer.pivot.get();
            x += layer.x.value() + (1.0 - lsx) * (px - 0.5) * width;
            y += layer.y.value() + (1.0 - lsy) * (py - 0.5) * height;
            sx *= lsx;
            sy *= lsy;
        }
        (x, y, sx, sy)
    }

    fn write(&self) {
        let mut opacity = 1.0;
        for (_, layer) in self.imp().layers.borrow().iter() {
            opacity *= layer.opacity.value();
        }
        let (x, y, sx, sy) = self.transform_at(f64::from(self.width()), f64::from(self.height()));
        self.imp().transform.set((x, y, sx, sy));
        self.set_opacity(opacity.clamp(0.0, 1.0));
        if log_motion() {
            let child = self
                .first_child()
                .map(|child| child.type_().name().to_string())
                .unwrap_or_default();
            eprintln!(
                "bern-motion {} {child} {:?} move ({x:.1}, {y:.1}) scale ({sx:.3}, {sy:.3}) opacity {opacity:.2}",
                glib::monotonic_time() / 1000,
                glib::translate::ToGlibPtr::<*mut glib::gobject_ffi::GObject>::to_glib_none(
                    self.upcast_ref::<glib::Object>()
                )
                .0
            );
        }
        if !self.imp().allocating.get() {
            self.queue_allocate();
        }
    }

    /// Run `body` once, after the next allocation.
    pub(crate) fn after_layout(&self, body: impl FnOnce() + 'static) {
        self.imp().laid_out.borrow_mut().push(Box::new(body));
        self.queue_allocate();
    }

    /// Run `body` after every allocation until the current scope ends.
    pub(crate) fn watch_layout(&self, body: Rc<dyn Fn()>) {
        self.imp().watchers.borrow_mut().push(Rc::clone(&body));
        let weak = self.downgrade();
        on_cleanup(move || {
            if let Some(motion) = weak.upgrade() {
                motion
                    .imp()
                    .watchers
                    .borrow_mut()
                    .retain(|watcher| !Rc::ptr_eq(watcher, &body));
            }
        });
    }
}

// MARK: - Structure

thread_local! {
    /// Whether what is being built appears in an update, not the first
    /// display.
    static ENTERING: Cell<bool> = const { Cell::new(false) };
    /// The stagger clock of the container whose direct children are being
    /// built.
    static CLOCK: RefCell<Option<Rc<Clock>>> = const { RefCell::new(None) };
}

pub(crate) fn entering() -> bool {
    ENTERING.with(Cell::get)
}

pub(crate) fn with_entering<R>(entering: bool, body: impl FnOnce() -> R) -> R {
    let saved = ENTERING.with(|cell| cell.replace(entering));
    let result = body();
    ENTERING.with(|cell| cell.set(saved));
    result
}

/// A container's turns: its direct children that appear take turns,
/// `step` seconds apart, counted from 0 in each update.
pub struct Clock {
    step: f64,
    next: Cell<u32>,
    resetting: Cell<bool>,
    /// Whether the container itself appeared in an update, so its children
    /// play `appear` with it.
    play: bool,
}

impl Clock {
    fn turn(self: &Rc<Self>) -> f64 {
        let delay = self.next.get() as f64 * self.step;
        self.next.set(self.next.get() + 1);
        if !self.resetting.replace(true) {
            let clock = Rc::clone(self);
            glib::idle_add_local_once(move || {
                clock.next.set(0);
                clock.resetting.set(false);
            });
        }
        delay
    }
}

pub(crate) fn clock() -> Option<Rc<Clock>> {
    CLOCK.with(|clock| clock.borrow().clone())
}

pub(crate) fn with_clock<R>(clock: Option<Rc<Clock>>, body: impl FnOnce() -> R) -> R {
    let saved = CLOCK.with(|cell| cell.replace(clock));
    let result = body();
    CLOCK.with(|cell| *cell.borrow_mut() = saved);
    result
}

/// A clock for the children a container builds now.
pub(crate) fn new_clock(step: f64) -> Rc<Clock> {
    Rc::new(Clock {
        step,
        next: Cell::new(0),
        resetting: Cell::new(false),
        play: entering(),
    })
}

/// The children `body` builds are the staggering children of a container.
pub fn staggered(step: f64, body: impl FnOnce()) {
    if reduced() {
        body();
        return;
    }
    with_clock(Some(new_clock(step)), body);
}

// MARK: - Transitions

#[derive(Clone, Copy, Debug)]
pub enum Edge {
    Leading,
    Trailing,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub enum TransitionKind {
    Fade,
    /// Grows from this fraction of its size.
    Pop(f64),
    /// Slides from this far beyond the edge.
    Slide(Edge, f64),
    /// Extends from nothing along one axis, the edge staying in place,
    /// without fading.
    Grow(Edge),
}

/// How an element appears or disappears, moved by a spring.
#[derive(Clone, Copy, Debug)]
pub struct Transition {
    pub kind: TransitionKind,
    pub motion: Curve,
}

fn start_of(transition: &Transition, widget: &gtk::Widget) -> Pose {
    let mut from = Pose {
        opacity: 0.0,
        ..REST
    };
    match transition.kind {
        TransitionKind::Fade => {}
        TransitionKind::Pop(scale) => {
            from.sx = scale;
            from.sy = scale;
        }
        TransitionKind::Slide(edge, distance) => {
            let rtl = widget.direction() == gtk::TextDirection::Rtl;
            match edge {
                Edge::Top => from.y = -distance,
                Edge::Bottom => from.y = distance,
                Edge::Leading => from.x = if rtl { distance } else { -distance },
                Edge::Trailing => from.x = if rtl { -distance } else { distance },
            }
        }
        TransitionKind::Grow(edge) => {
            from.opacity = 1.0;
            match edge {
                Edge::Top | Edge::Bottom => from.sy = 0.0,
                Edge::Leading | Edge::Trailing => from.sx = 0.0,
            }
        }
    }
    from
}

/// The point a transition's scale keeps in place: the edge it grows from,
/// else the center.
fn pivot_of(transition: &Transition, widget: &gtk::Widget) -> (f64, f64) {
    let TransitionKind::Grow(edge) = transition.kind else {
        return (0.5, 0.5);
    };
    let rtl = widget.direction() == gtk::TextDirection::Rtl;
    match edge {
        Edge::Top => (0.5, 0.0),
        Edge::Bottom => (0.5, 1.0),
        Edge::Leading => (if rtl { 1.0 } else { 0.0 }, 0.5),
        Edge::Trailing => (if rtl { 0.0 } else { 1.0 }, 0.5),
    }
}

/// Play `transition` on `motion`, in or out; `done` runs once it ends.
fn play(
    motion: &MotionBox,
    transition: &Transition,
    entering: bool,
    done: Option<Box<dyn FnOnce()>>,
    delay: f64,
) {
    let quick = reduced();
    let layer = motion.layer(if entering { "enter" } else { "exit" });
    let away = if quick {
        Pose {
            opacity: 0.0,
            ..REST
        }
    } else {
        start_of(transition, motion.upcast_ref())
    };
    let (start, end) = if entering { (away, REST) } else { (REST, away) };
    let curve = if quick { QUICK_FADE } else { transition.motion };
    layer.pivot.set(pivot_of(transition, motion.upcast_ref()));
    layer.set(start);
    let left = Rc::new(Cell::new(5));
    let done = Rc::new(RefCell::new(done));
    let run = move || {
        let values = [end.x, end.y, end.sx, end.sy, end.opacity];
        for (spring, value) in layer.springs().into_iter().zip(values) {
            let left = Rc::clone(&left);
            let done = Rc::clone(&done);
            spring.to(
                value,
                Some(curve),
                Some(Box::new(move || {
                    left.set(left.get() - 1);
                    if left.get() == 0
                        && let Some(done) = done.borrow_mut().take()
                    {
                        done();
                    }
                })),
            );
        }
    };
    let start = move || {
        if delay > 0.0 && !quick {
            glib::timeout_add_local_once(Duration::from_secs_f64(delay), run);
        } else {
            run();
        }
    };
    when_mapped(motion.upcast_ref(), start);
}

/// Run `body` once `widget` is on screen: an animation started before
/// that would end at once.
fn when_mapped(widget: &gtk::Widget, body: impl FnOnce() + 'static) {
    if widget.is_mapped() {
        body();
        return;
    }
    let body = RefCell::new(Some(body));
    let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let held = Rc::clone(&handler);
    let id = widget.connect_map(move |widget| {
        if let Some(id) = held.borrow_mut().take() {
            widget.disconnect(id);
        }
        if let Some(body) = body.borrow_mut().take() {
            body();
        }
    });
    *handler.borrow_mut() = Some(id);
}

// MARK: - Shared elements

struct Departed {
    rect: gtk::graphene::Rect,
    ghost: Option<gtk::gdk::Paintable>,
    stamp: u64,
}

thread_local! {
    static HEROES: RefCell<HashMap<String, glib::WeakRef<MotionBox>>> = RefCell::new(HashMap::new());
    static DEPARTED: RefCell<HashMap<String, Departed>> = RefCell::new(HashMap::new());
    static STAMP: Cell<u64> = const { Cell::new(0) };
}

/// Where `motion` lies in the window's ghost layer, without its own
/// transform.
fn rect_in(motion: &MotionBox, layer: &gtk::Fixed) -> Option<gtk::graphene::Rect> {
    let rect = motion.compute_bounds(layer)?;
    (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect)
}

/// The element with `key` leaves: remember its place and a picture of it
/// for the next one with the key to grow from.
fn hero_leave(key: &str, motion: &MotionBox, layer: &gtk::Fixed) {
    HEROES.with(|heroes| {
        let mut heroes = heroes.borrow_mut();
        if heroes
            .get(key)
            .and_then(glib::WeakRef::upgrade)
            .is_some_and(|current| &current == motion)
        {
            heroes.remove(key);
        }
    });
    if !motion.is_mapped() {
        return;
    }
    let Some(rect) = rect_in(motion, layer) else {
        return;
    };
    let ghost = (!contains_media(motion.upcast_ref()))
        .then(|| gtk::WidgetPaintable::new(Some(motion)).current_image());
    let stamp = STAMP.with(|stamp| {
        stamp.set(stamp.get() + 1);
        stamp.get()
    });
    DEPARTED.with(|departed| {
        departed
            .borrow_mut()
            .insert(key.to_string(), Departed { rect, ghost, stamp })
    });
    let key = key.to_string();
    glib::timeout_add_local_once(Duration::from_millis(600), move || {
        DEPARTED.with(|departed| {
            let mut departed = departed.borrow_mut();
            if departed.get(&key).is_some_and(|entry| entry.stamp == stamp) {
                departed.remove(&key);
            }
        });
    });
}

/// Videos and drawings change on their own; their pictures go stale, so
/// they get no ghost.
fn contains_media(widget: &gtk::Widget) -> bool {
    if widget.is::<gtk::Picture>() || widget.is::<gtk::DrawingArea>() {
        return true;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if contains_media(&current) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

/// The element with `key` appears: once laid out, it grows from where the
/// last one with the key was, whose picture moves along and fades.
fn hero_enter(key: &str, motion: &MotionBox, layer: &gtk::Fixed, curve: Curve) {
    let previous = HEROES.with(|heroes| {
        heroes
            .borrow_mut()
            .insert(key.to_string(), motion.downgrade())
    });
    let previous = previous
        .and_then(|weak| weak.upgrade())
        .filter(|previous| previous != motion && previous.is_mapped());
    let from = DEPARTED
        .with(|departed| departed.borrow_mut().remove(key))
        .or_else(|| {
            let previous = previous?;
            Some(Departed {
                rect: rect_in(&previous, layer)?,
                ghost: None,
                stamp: 0,
            })
        });
    let Some(from) = from else { return };
    if reduced() {
        return;
    }
    let target = motion.clone();
    let layer = layer.clone();
    motion.after_layout(move || grow(&target, from, &layer, curve));
}

fn grow(motion: &MotionBox, from: Departed, layer: &gtk::Fixed, curve: Curve) {
    let Some(to) = rect_in(motion, layer) else {
        return;
    };
    let a = from.rect;
    let dx = (a.x() + a.width() / 2.0 - (to.x() + to.width() / 2.0)) as f64;
    let dy = (a.y() + a.height() / 2.0 - (to.y() + to.height() / 2.0)) as f64;
    let sx = (a.width() / to.width()) as f64;
    let sy = (a.height() / to.height()) as f64;
    let hero = motion.layer("hero");
    let ghost = from.ghost.map(|paintable| {
        let picture = gtk::Picture::for_paintable(&paintable);
        picture.set_can_target(false);
        picture.set_size_request(a.width() as i32, a.height() as i32);
        picture.set_content_fit(gtk::ContentFit::Fill);
        layer.put(&picture, a.x() as f64, a.y() as f64);
        picture
    });
    let place = {
        let layer = layer.clone();
        let ghost = ghost.clone();
        move |t: f64| {
            let u = 1.0 - t;
            hero.x.set(dx * u);
            hero.y.set(dy * u);
            hero.sx.set(1.0 + (sx - 1.0) * u);
            hero.sy.set(1.0 + (sy - 1.0) * u);
            hero.opacity.set(t.clamp(0.0, 1.0));
            if let Some(ghost) = &ghost {
                let (w, h) = (a.width() / 2.0, a.height() / 2.0);
                let transform = gtk::gsk::Transform::new()
                    .translate(&gtk::graphene::Point::new(
                        a.x() + w - (dx * t) as f32,
                        a.y() + h - (dy * t) as f32,
                    ))
                    .scale(
                        (1.0 + (1.0 / sx - 1.0) * t) as f32,
                        (1.0 + (1.0 / sy - 1.0) * t) as f32,
                    )
                    .translate(&gtk::graphene::Point::new(-w, -h));
                layer.set_child_transform(ghost, Some(&transform));
                ghost.set_opacity((1.0 - t).clamp(0.0, 1.0));
            }
        }
    };
    place(0.0);
    let progress = Spring::new(0.0, motion, place);
    let removed = layer.clone();
    progress.to(
        1.0,
        Some(curve),
        Some(Box::new(move || {
            if let Some(ghost) = ghost {
                removed.remove(&ghost);
            }
        })),
    );
}

// MARK: - Decoration

/// The motion and appearance props any element takes.
#[derive(Default)]
pub struct Decor {
    pub appear: Option<&'static Transition>,
    pub disappear: Option<&'static Transition>,
    /// The key of a shared element, and its spring.
    pub hero: Option<(Get, Curve)>,
    pub reveal: bool,
    pub appearance: Option<&'static str>,
    /// A nudge each time this turns true, back with the spring.
    pub nudge: Option<(Get, Curve)>,
    pub scroll_scale: Option<f64>,
    pub scroll_opacity: Option<f64>,
    /// The point of the element the scroll scale grows from, as
    /// fractions of its size; the center when not given.
    pub scroll_origin: Option<(f64, f64)>,
    /// Seconds before `appear` plays; with a delay it plays also when the
    /// element shows first.
    pub appear_delay: Option<f64>,
    /// A nudge each time this value changes, back with the spring.
    pub nudge_key: Option<(Get, Curve)>,
    /// How large a nudge makes the element.
    pub nudge_scale: Option<f64>,
    /// While true, motion is reduced everywhere.
    pub reduce_motion: Option<Get>,
}

/// Wrap a widget an element made in a motion box: inside a list row the
/// row's content moves, as rows are the list's to lay out.
fn wrap(widget: &gtk::Widget) -> (gtk::Widget, MotionBox) {
    if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>()
        && let Some(child) = row.child()
    {
        row.set_child(None::<&gtk::Widget>);
        let motion = MotionBox::wrap(&child);
        row.set_child(Some(&motion));
        return (widget.clone(), motion);
    }
    let motion = MotionBox::wrap(widget);
    (motion.clone().upcast(), motion)
}

/// Apply the motion props to the element `build` makes.
#[allow(non_snake_case)]
pub fn Decorate(p: &Parent, ctx: &Ctx, decor: Decor, build: Build) {
    // Elements inside this one enter with it, not on their own; a
    // staggering container's clock is only for its direct children.
    if let Some(when) = &decor.reduce_motion {
        prefer_reduced(ctx, Rc::clone(when));
    }
    let animated = entering();
    let turns = clock();
    let (capture, made) = Parent::capture(p);
    with_entering(false, || with_clock(None, || build(&capture, ctx)));
    let made: Vec<gtk::Widget> = made.borrow().clone();
    for widget in made {
        let (outer, motion) = wrap(&widget);
        if let Some(appearance) = decor.appearance {
            crate::theme::use_part(appearance == "dark");
            motion.add_css_class(&format!("bern-appearance-{appearance}"));
        }
        if let Some(disappear) = decor.disappear {
            let leaving = motion.downgrade();
            crate::parent::on_exit(
                &outer,
                Rc::new(move |done| match leaving.upgrade() {
                    Some(motion) => play(&motion, disappear, false, Some(done), 0.0),
                    None => done(),
                }),
            );
        }
        if let Some(appear) = decor.appear {
            let staggered = turns.as_ref().filter(|turns| turns.play || animated);
            if animated || staggered.is_some() || decor.appear_delay.is_some() {
                let delay = staggered.map(Clock::turn).unwrap_or(0.0)
                    + decor.appear_delay.unwrap_or(0.0).max(0.0);
                if log_motion() {
                    eprintln!(
                        "bern-motion appear delay {delay} {:?}",
                        glib::translate::ToGlibPtr::<*mut glib::gobject_ffi::GObject>::to_glib_none(
                            motion.upcast_ref::<glib::Object>()
                        )
                        .0
                    );
                }
                play(&motion, appear, true, None, delay);
            }
        }
        if let Some((key, curve)) = &decor.hero {
            hero(ctx, &motion, Rc::clone(key), *curve);
        }
        if decor.reveal {
            reveal(&outer, &motion);
        }
        let scale = decor.nudge_scale.unwrap_or(1.06);
        if let Some((when, curve)) = &decor.nudge {
            nudge(ctx, &motion, Rc::clone(when), *curve, scale, false);
        }
        if let Some((key, curve)) = &decor.nudge_key {
            nudge(ctx, &motion, Rc::clone(key), *curve, scale, true);
        }
        if decor.scroll_scale.is_some() || decor.scroll_opacity.is_some() {
            follow_scroll(
                ctx,
                &motion,
                decor.scroll_scale.unwrap_or(1.0),
                decor.scroll_opacity.unwrap_or(1.0),
                decor.scroll_origin.unwrap_or((0.5, 0.5)),
            );
        }
        p.add(&outer);
    }
}

fn key_text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        other => crate::value::key_text(other),
    }
}

fn hero(ctx: &Ctx, motion: &MotionBox, key: Get, curve: Curve) {
    let layer = ctx.win().ghosts.clone();
    let window = ctx.win().id.clone();
    let current: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let at = ctx.clone();
    let shown = motion.clone();
    let held = Rc::clone(&current);
    let ghosts = layer.clone();
    effect(move || {
        // Shared elements pair within one window.
        let next = format!("{window}/{}", key_text(&key(&at)));
        untrack(|| {
            let previous = held.borrow_mut().replace(next.clone());
            if let Some(previous) = previous {
                if previous == next {
                    return;
                }
                hero_leave(&previous, &shown, &ghosts);
            }
            hero_enter(&next, &shown, &ghosts, curve);
        });
    });
    let leaving = motion.clone();
    on_cleanup(move || {
        let key = current.borrow_mut().take();
        if let Some(key) = key {
            hero_leave(&key, &leaving, &layer);
        }
    });
}

/// Shown while the pointer is over the container it is in, or it has the
/// keyboard focus; always on devices without a pointer.
fn reveal(outer: &gtk::Widget, motion: &MotionBox) {
    let pointer = gtk::gdk::Display::default()
        .and_then(|display| display.default_seat())
        .is_none_or(|seat| {
            seat.capabilities()
                .contains(gtk::gdk::SeatCapabilities::POINTER)
        });
    if !pointer {
        return;
    }
    let layer = motion.layer("reveal");
    layer.opacity.set(0.0);
    let hovering = Rc::new(Cell::new(false));
    let focused = Rc::new(Cell::new(false));
    let update = {
        let hovering = Rc::clone(&hovering);
        let focused = Rc::clone(&focused);
        let layer = layer.clone();
        Rc::new(move || {
            let shown = hovering.get() || focused.get();
            layer
                .opacity
                .to(if shown { 1.0 } else { 0.0 }, Some((0.22, 0.85)), None);
        })
    };
    let focus = gtk::EventControllerFocus::new();
    {
        let focused = Rc::clone(&focused);
        let update = Rc::clone(&update);
        focus.connect_contains_focus_notify(move |controller| {
            focused.set(controller.contains_focus());
            update();
        });
    }
    motion.add_controller(focus);
    // The container is known once the element is in it.
    let watched = Rc::new(RefCell::new(
        None::<(gtk::Widget, gtk::EventControllerMotion)>,
    ));
    let attach = {
        let outer = outer.clone();
        let watched = Rc::clone(&watched);
        move || {
            let Some(container) = outer.parent() else {
                return;
            };
            if watched
                .borrow()
                .as_ref()
                .is_some_and(|(current, _)| current == &container)
            {
                return;
            }
            if let Some((previous, controller)) = watched.borrow_mut().take() {
                previous.remove_controller(&controller);
            }
            let pointer = gtk::EventControllerMotion::new();
            let entered = Rc::clone(&hovering);
            let update_enter = Rc::clone(&update);
            pointer.connect_enter(move |_, _, _| {
                entered.set(true);
                update_enter();
            });
            let left = Rc::clone(&hovering);
            let update_leave = Rc::clone(&update);
            pointer.connect_leave(move |_| {
                left.set(false);
                update_leave();
            });
            container.add_controller(pointer.clone());
            *watched.borrow_mut() = Some((container, pointer));
        }
    };
    let attach = Rc::new(attach);
    let on_map = Rc::clone(&attach);
    outer.connect_map(move |_| on_map());
    if outer.is_mapped() {
        attach();
    }
    on_cleanup(move || {
        if let Some((container, controller)) = watched.borrow_mut().take() {
            container.remove_controller(&controller);
        }
    });
}

/// A short nudge each time `when` turns true, or with `keyed` each time
/// it changes.
fn nudge(ctx: &Ctx, motion: &MotionBox, when: Get, curve: Curve, scale: f64, keyed: bool) {
    let at = ctx.clone();
    let before = RefCell::new(untrack(|| when(&at)));
    let layer = motion.layer("nudge");
    effect(move || {
        let now = when(&at);
        let fires = if keyed {
            now != *before.borrow()
        } else {
            crate::value::truthy(&now) && !crate::value::truthy(&before.borrow())
        };
        if fires && !reduced() {
            untrack(|| nudge_layer(&layer, curve, scale));
        }
        *before.borrow_mut() = now;
    });
}

fn nudge_layer(layer: &Layer, curve: Curve, scale: f64) {
    layer.sx.bump(scale, curve);
    layer.sy.bump(scale, curve);
}

/// Give `widget` a nudge, when it moves.
pub(crate) fn nudge_widget(widget: &gtk::Widget, curve: Curve) {
    if reduced() {
        return;
    }
    let motion = widget
        .downcast_ref::<MotionBox>()
        .cloned()
        .or_else(|| widget.first_child().and_then(|child| child.downcast().ok()));
    if let Some(motion) = motion {
        nudge_layer(&motion.layer("nudge"), curve, 1.06);
    }
}

/// Scale and opacity by the distance of the element's center from the
/// middle of its scroll view: `scale` and full opacity at the middle, plain
/// size and `opacity` half a view plus half its size away.
fn follow_scroll(ctx: &Ctx, motion: &MotionBox, scale: f64, opacity: f64, origin: (f64, f64)) {
    if reduced() {
        return;
    }
    let Some(view) = ctx.scroll().map(|scroll| scroll.view.clone()) else {
        return;
    };
    let horizontal = view.hscrollbar_policy() != gtk::PolicyType::Never;
    let layer = motion.layer("scroll");
    // The scale keeps the origin in place.
    layer.pivot.set(origin);
    let update: Rc<dyn Fn()> = {
        let motion = motion.downgrade();
        let view = view.downgrade();
        Rc::new(move || {
            let (Some(motion), Some(view)) = (motion.upgrade(), view.upgrade()) else {
                return;
            };
            let Some(rect) = motion.compute_bounds(&view) else {
                return;
            };
            let (port, size, center) = if horizontal {
                (
                    view.width() as f64,
                    rect.width() as f64,
                    (rect.x() + rect.width() / 2.0) as f64,
                )
            } else {
                (
                    view.height() as f64,
                    rect.height() as f64,
                    (rect.y() + rect.height() / 2.0) as f64,
                )
            };
            let reach = (port + size) / 2.0;
            let near = if reach > 0.0 {
                1.0 - ((center - port / 2.0).abs() / reach).min(1.0)
            } else {
                1.0
            };
            let factor = 1.0 + (scale - 1.0) * near;
            layer.sx.set(factor);
            layer.sy.set(factor);
            layer.opacity.set(opacity + (1.0 - opacity) * near);
        })
    };
    let adjustment = if horizontal {
        view.hadjustment()
    } else {
        view.vadjustment()
    };
    let scrolled = Rc::clone(&update);
    let handler = adjustment.connect_value_changed(move |_| scrolled());
    motion.watch_layout(Rc::clone(&update));
    on_cleanup(move || adjustment.disconnect(handler));
}

// MARK: - Bound numbers

/// A length or other number: written in the declaration, or bound to an
/// expression.
#[derive(Clone)]
pub enum Num {
    Fixed(f64),
    Bound(Get),
}

impl Num {
    /// The value now, without recording a dependency.
    pub(crate) fn peek(&self, ctx: &Ctx) -> f64 {
        match self {
            Num::Fixed(value) => *value,
            Num::Bound(get) => untrack(|| crate::value::as_f64(&get(ctx))),
        }
    }
}

/// Write `value` with `write`, and again whenever a bound value changes,
/// springing to it with `curve` when given. `widget` times the spring.
pub(crate) fn bind_number(
    ctx: &Ctx,
    value: &Num,
    curve: Option<Curve>,
    widget: &impl IsA<gtk::Widget>,
    write: impl Fn(f64) + 'static,
) {
    match value {
        Num::Fixed(value) => write(*value),
        Num::Bound(get) => {
            let at = ctx.clone();
            let get = Rc::clone(get);
            let first = untrack(|| crate::value::as_f64(&get(&at)));
            write(first);
            let spring = Spring::new(first, widget, write);
            let started = Rc::new(Cell::new(false));
            effect(move || {
                let next = crate::value::as_f64(&get(&at));
                if !started.replace(true) {
                    return;
                }
                untrack(|| spring.to(next, curve, None));
            });
        }
    }
}
