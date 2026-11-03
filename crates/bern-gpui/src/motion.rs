//! Motion: values that spring between the frames a window draws.
//!
//! A window builds its content again for every frame, so an element in
//! motion keeps, by its key, a spring that moves toward the value it should
//! show, and each frame shows where the spring is by then; while any
//! spring moves, the window asks for the next frame. Springs follow Bern's
//! motion tokens: a period and a damping ratio, mass 1, stiffness
//! (2π / T)², damping 4πζ / T. A new target starts from the current value
//! and velocity, so motion can be interrupted without a jump.
//!
//! An element appears when its key was not drawn in the window's previous
//! frame, but not in the window's first frame, nor inside another element
//! that appears with it; the children of a staggering container take
//! turns. GPUI draws no element transformed, so an element in motion is
//! drawn in a box of its own while it moves: the box fades and moves it,
//! and for a scale it keeps the place the element took at rest while the
//! element is built again with every length scaled.

use crate::El;
use crate::ctx::Ctx;
use crate::protocol::Value;
use crate::units::px;
use gpui::{Bounds, IntoElement, ParentElement, Pixels, Styled, div};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// A spring: period in seconds and damping ratio.
pub type Curve = (f32, f32);

/// The quick fade transitions become with less motion.
const QUICK_FADE: Curve = (0.15, 1.0);
/// The push of a nudge, before it springs back.
const NUDGE_OUT: Curve = (0.12, 1.0);
/// How long a nudge pushes before it springs back.
const NUDGE_HOLD: Duration = Duration::from_millis(90);
/// The spring a nudge comes back with.
const JELLY: Curve = (0.42, 0.42);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Leading,
    Trailing,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransitionKind {
    Fade,
    /// Grows from this fraction of its size.
    Pop(f32),
    /// Slides from this far beyond the edge.
    Slide(Edge, f32),
    /// Extends from nothing along one axis, the edge staying in place,
    /// without fading.
    Grow(Edge),
    /// Comes in from beyond the edge, its whole size away, without fading.
    Move(Edge),
}

/// How an element appears or disappears, moved by a spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub kind: TransitionKind,
    pub motion: Curve,
}

/// The motion props of an element.
#[derive(Clone, Debug)]
pub struct Motion {
    pub appear: Option<&'static Transition>,
    /// Seconds before `appear` plays; with a delay it plays also when the
    /// element shows first.
    pub appear_delay: Option<f32>,
    pub disappear: Option<&'static Transition>,
    /// A nudge each time this turns true.
    pub nudge: Option<bool>,
    /// A nudge each time this value changes.
    pub nudge_key: Option<Value>,
    /// How large a nudge makes the element.
    pub nudge_scale: Option<f32>,
    /// While true, motion is reduced everywhere.
    pub reduce_motion: Option<bool>,
    /// A shared element: its name and key (`hero_key`), and the spring it
    /// moves from where the last one was with.
    pub hero: Option<(String, Curve)>,
    /// Following scrolling: the scale and the opacity at the middle and at
    /// the edge of the scroll view the element is in, and the point of it
    /// the scale grows from, as fractions of its size
    /// (`scroll_scale`, `scroll_opacity`, `scroll_origin_x`, `_y`).
    pub scroll: Option<(f32, f32, (f32, f32))>,
}

impl Motion {
    pub const NONE: Motion = Motion {
        appear: None,
        appear_delay: None,
        disappear: None,
        nudge: None,
        nudge_key: None,
        nudge_scale: None,
        reduce_motion: None,
        hero: None,
        scroll: None,
    };
}

/// The key of the shared element `name` showing `key` (`hero`,
/// `hero_key`).
pub fn hero_key(name: &str, key: Option<Value>) -> String {
    match key {
        Some(key) => format!("{name}#{}", crate::value::key_text(&key)),
        None => name.to_string(),
    }
}

/// A shared element on its way from where the last one with its key was.
struct Flight {
    from: Bounds<Pixels>,
    progress: Spring,
    curve: Curve,
}

/// Where an element in motion is drawn: moved by `x` and `y` points,
/// scaled about its center, faded, and for a transition that grows from an
/// edge, the fraction of it that shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub grow: Option<(Edge, f32)>,
    /// The point the scale keeps in place, as fractions of the size.
    pub origin: (f32, f32),
}

impl Default for Pose {
    fn default() -> Self {
        Pose::REST
    }
}

impl Pose {
    pub const REST: Pose = Pose {
        x: 0.0,
        y: 0.0,
        scale: 1.0,
        opacity: 1.0,
        grow: None,
        origin: (0.5, 0.5),
    };

    fn at_rest(&self) -> bool {
        self.x.abs() < 0.01
            && self.y.abs() < 0.01
            && (self.scale - 1.0).abs() < 0.0005
            && self.opacity >= 0.999
            && self.grow.is_none_or(|(_, shown)| shown >= 0.9995)
    }
}

/// A value moving toward its target.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spring {
    pub(crate) value: f32,
    velocity: f32,
    pub(crate) target: f32,
    last: Instant,
}

impl Spring {
    /// A spring at `from`, on its way to `to`.
    pub(crate) fn toward(from: f32, to: f32, now: Instant) -> Self {
        Spring {
            target: to,
            ..Spring::at(from, now)
        }
    }

    pub(crate) fn at(value: f32, now: Instant) -> Self {
        Spring {
            value,
            velocity: 0.0,
            target: value,
            last: now,
        }
    }

    /// Move the value on to `now`, in steps short enough to stay stable
    /// with the stiffest token.
    pub(crate) fn advance(&mut self, (period, damping): Curve, now: Instant) {
        let mut left = now
            .saturating_duration_since(self.last)
            .as_secs_f32()
            .min(0.1);
        self.last = now;
        let period = period.max(0.01);
        let stiffness = (std::f32::consts::TAU / period).powi(2);
        let friction = 4.0 * std::f32::consts::PI * damping / period;
        while left > 0.0 {
            let step = left.min(1.0 / 480.0);
            let force = -stiffness * (self.value - self.target) - friction * self.velocity;
            self.velocity += force * step;
            self.value += self.velocity * step;
            left -= step;
        }
    }

    /// Whether it has come to rest, within `unit / 1000` of its target;
    /// then it stands at the target.
    pub(crate) fn settle(&mut self, unit: f32) -> bool {
        let settled =
            (self.value - self.target).abs() <= unit * 1e-3 && self.velocity.abs() <= unit * 1e-2;
        if settled {
            self.value = self.target;
            self.velocity = 0.0;
        }
        settled
    }
}

/// An appearing transition under way.
struct Entering {
    transition: Transition,
    /// From 0 to 1, from where the transition starts to rest.
    progress: Spring,
    /// When it starts moving, after its delay.
    begins: Instant,
    /// With less motion, a quick fade.
    quick: bool,
}

/// A nudge under way.
struct Nudge {
    scale: Spring,
    /// What set it off last: the condition, or the key.
    last: Value,
    /// When it springs back, after pushing.
    back_at: Option<Instant>,
}

/// The rows a list showed in its last frame, and those leaving: by key,
/// with their item and where they were.
#[derive(Default)]
struct Rows {
    before: Vec<String>,
    items: HashMap<String, Value>,
    leaving: Vec<(String, Value, usize)>,
}

/// What a window keeps of the motion in it.
#[derive(Default)]
struct Scene {
    /// Frames drawn so far.
    drawn: u64,
    /// The frame each key was last drawn in.
    seen: HashMap<String, u64>,
    entering: HashMap<String, Entering>,
    nudges: HashMap<String, Nudge>,
    /// Springs of `animate` and of control states, by key.
    springs: HashMap<String, Spring>,
    /// The size of elements about to scale, as they were drawn last.
    sizes: Sizes,
    /// What each condition shows and what of it leaves.
    branches: HashMap<String, Branch>,
    /// What each list showed and which of its rows leave.
    lists: HashMap<String, Rows>,
    /// Elements leaving: from 0 to 1, from rest to gone.
    leaving: HashMap<String, Spring>,
    /// Children of reflowing containers.
    slides: HashMap<String, Slide>,
    /// Where each shared element was drawn, by its key, and in which
    /// frame.
    heroes: HashMap<String, (Bounds<Pixels>, u64)>,
    /// Shared elements under way, by the key of the element.
    flights: HashMap<String, Flight>,
    /// Something moves in the frame being drawn.
    moving: bool,
    /// An element asked for less motion in the frame being drawn, and in
    /// the one before it.
    asked_less: bool,
    asked_less_before: bool,
}

thread_local! {
    static SCENES: RefCell<HashMap<Rc<str>, Scene>> = RefCell::new(HashMap::new());
    /// The time the frame being drawn shows.
    static NOW: Cell<Option<Instant>> = const { Cell::new(None) };
    /// Whether motion is reduced by the system in the frame being drawn.
    static SYSTEM_LESS: Cell<bool> = const { Cell::new(false) };
    /// Whether what is being built stands in an element that appears
    /// with it, in this frame.
    static WITHIN: Cell<bool> = const { Cell::new(false) };
    /// The turns of the staggering container whose direct children are
    /// being built.
    static CLOCK: RefCell<Option<Rc<Clock>>> = const { RefCell::new(None) };
}

/// A container's turns: its direct children that appear take turns, `step`
/// seconds apart, counted from 0 in each frame.
struct Clock {
    step: f32,
    next: Cell<u32>,
    /// The container appears in this frame, so its children play `appear`
    /// with it.
    play: bool,
}

pub(crate) fn now() -> Instant {
    NOW.with(Cell::get).unwrap_or_else(clock_time)
}

fn with_scene<R>(c: &Ctx, body: impl FnOnce(&mut Scene) -> R) -> R {
    SCENES.with(|scenes| {
        let mut scenes = scenes.borrow_mut();
        let scene = scenes.entry(Rc::clone(&c.win().id)).or_default();
        body(scene)
    })
}

impl Scene {
    /// Note that `key` is drawn in this frame; whether it was not in the
    /// previous one.
    fn fresh(&mut self, key: &str) -> bool {
        let drawn = self.drawn;
        let before = self.seen.insert(key.to_string(), drawn);
        !before.is_some_and(|frame| frame + 1 >= drawn)
    }

    fn reduced(&self) -> bool {
        SYSTEM_LESS.with(Cell::get) || self.asked_less || self.asked_less_before
    }
}

/// Development aid: `BERN_MOTION_LOG` prints what starts moving, with the
/// time in milliseconds, to check motion frame by frame.
pub(crate) fn log(what: impl FnOnce() -> String) {
    thread_local! {
        static LOG: bool = std::env::var_os("BERN_MOTION_LOG").is_some();
        static START: Instant = Instant::now();
    }
    if LOG.with(|log| *log) {
        let at = START.with(|start| start.elapsed().as_millis());
        eprintln!("bern-motion {at} {}", what());
    }
}

/// The time motion goes by: `BERN_MOTION=slow` makes it go ten times
/// slower, to check motion frame by frame.
fn clock_time() -> Instant {
    thread_local! {
        static SLOW: Option<Instant> =
            (std::env::var("BERN_MOTION").as_deref() == Ok("slow")).then(Instant::now);
    }
    let real = Instant::now();
    match SLOW.with(|slow| *slow) {
        Some(start) => start + real.saturating_duration_since(start) / 10,
        None => real,
    }
}

/// A window starts drawing a frame.
pub(crate) fn begin(win: &str) {
    NOW.with(|now| now.set(Some(clock_time())));
    SYSTEM_LESS.with(|less| less.set(crate::native::reduce_motion()));
    SCENES.with(|scenes| {
        let mut scenes = scenes.borrow_mut();
        let scene = scenes.entry(win.into()).or_default();
        scene.drawn += 1;
        scene.moving = false;
        scene.asked_less_before = scene.asked_less;
        scene.asked_less = false;
    });
}

/// A window has built its frame: forget what it no longer draws. Whether
/// something moves, so the window draws the next frame.
pub(crate) fn end(win: &str) -> bool {
    NOW.with(|now| now.set(None));
    SCENES.with(|scenes| {
        let mut scenes = scenes.borrow_mut();
        let Some(scene) = scenes.get_mut(win) else {
            return false;
        };
        let drawn = scene.drawn;
        scene.seen.retain(|_, frame| *frame == drawn);
        let seen = &scene.seen;
        scene.entering.retain(|key, _| seen.contains_key(key));
        scene.nudges.retain(|key, _| seen.contains_key(key));
        scene.springs.retain(|key, _| seen.contains_key(key));
        scene.branches.retain(|key, _| seen.contains_key(key));
        scene.lists.retain(|key, _| seen.contains_key(key));
        scene.leaving.retain(|key, _| seen.contains_key(key));
        scene.slides.retain(|key, _| seen.contains_key(key));
        scene.flights.retain(|key, _| seen.contains_key(key));
        scene.heroes.retain(|_, (_, frame)| *frame + 1 >= drawn);
        scene
            .sizes
            .borrow_mut()
            .retain(|key, _| seen.contains_key(key));
        scene.moving
    })
}

/// A window closed.
pub(crate) fn forget(win: &str) {
    SCENES.with(|scenes| scenes.borrow_mut().remove(win));
}

/// Whether motion is reduced where `c` builds.
pub(crate) fn reduced(c: &Ctx) -> bool {
    with_scene(c, |scene| scene.reduced())
}

/// The children `build` makes are those of the staggering container `id`:
/// the ones that appear take turns, `step` seconds apart.
pub fn staggered<R>(c: &Ctx, id: &str, step: f32, build: impl FnOnce() -> R) -> R {
    if c.listing() || reduced(c) {
        return build();
    }
    let play = with_scene(c, |scene| {
        let fresh = scene.fresh(&format!("{}#turns", c.key(id)));
        fresh && scene.drawn > 1
    });
    let clock = Rc::new(Clock {
        step: step.max(0.0),
        next: Cell::new(0),
        play,
    });
    let saved = CLOCK.with(|cell| cell.replace(Some(clock)));
    let result = build();
    CLOCK.with(|cell| *cell.borrow_mut() = saved);
    result
}

/// Where a transition starts, with less motion or not; `size` is the
/// element's (the window's before it was laid out), for `Move`.
fn start_of(transition: &Transition, quick: bool, size: (f32, f32)) -> Pose {
    let mut from = Pose {
        opacity: 0.0,
        ..Pose::REST
    };
    if quick {
        return from;
    }
    match transition.kind {
        TransitionKind::Fade => {}
        TransitionKind::Pop(scale) => from.scale = scale,
        TransitionKind::Slide(edge, distance) => match edge {
            Edge::Top => from.y = -distance,
            Edge::Bottom => from.y = distance,
            Edge::Leading => from.x = -distance,
            Edge::Trailing => from.x = distance,
        },
        TransitionKind::Grow(edge) => {
            from.opacity = 1.0;
            from.grow = Some((edge, 0.0));
        }
        TransitionKind::Move(edge) => {
            from.opacity = 1.0;
            match edge {
                Edge::Top => from.y = -size.1,
                Edge::Bottom => from.y = size.1,
                Edge::Leading => from.x = -size.0,
                Edge::Trailing => from.x = size.0,
            }
        }
    }
    from
}

/// The pose `progress` of the way from `from` to rest; opacity and the
/// part shown stay within bounds as a bouncy spring overshoots.
fn between(from: Pose, progress: f32) -> Pose {
    let mix = |start: f32, end: f32| start + (end - start) * progress;
    Pose {
        x: mix(from.x, 0.0),
        y: mix(from.y, 0.0),
        scale: mix(from.scale, 1.0).max(0.0),
        opacity: mix(from.opacity, 1.0).clamp(0.0, 1.0),
        grow: from
            .grow
            .map(|(edge, shown)| (edge, mix(shown, 1.0).clamp(0.0, 1.0))),
        origin: from.origin,
    }
}

/// Apply the motion props of the element `id` to what `build` makes. An
/// element that draws itself moved (`own`: `Positioned`, `Modal`) takes the
/// pose and is only built scaled; any other is drawn in a box of its own
/// while it moves.
pub fn moving(
    c: &Ctx,
    id: &str,
    motion: Motion,
    own: bool,
    build: impl FnOnce(&Ctx, Pose) -> El,
) -> El {
    if c.listing() {
        return build(c, Pose::REST);
    }
    let key = c.key(id);
    let now = now();
    let within = WITHIN.with(Cell::get);
    let clock = CLOCK.with(|cell| cell.borrow_mut().take());
    let (fresh, pose, sizes, measure) = with_scene(c, |scene| {
        if motion.reduce_motion == Some(true) {
            scene.asked_less = true;
        }
        let fresh = scene.fresh(&key);
        let quick = scene.reduced();
        // Appearing.
        if fresh && let Some(appear) = motion.appear {
            let animated = scene.drawn > 1 && !within;
            let staggered = clock.as_ref().filter(|clock| clock.play || animated);
            if animated || staggered.is_some() || motion.appear_delay.is_some() {
                log(|| format!("appear {key}"));
                let turn = staggered
                    .map(|clock| {
                        let turn = clock.next.get();
                        clock.next.set(turn + 1);
                        turn as f32 * clock.step
                    })
                    .unwrap_or(0.0);
                let delay = if quick {
                    0.0
                } else {
                    turn + motion.appear_delay.unwrap_or(0.0).max(0.0)
                };
                scene.sizes.borrow_mut().remove(&key);
                scene.entering.insert(
                    key.clone(),
                    Entering {
                        transition: *appear,
                        progress: Spring {
                            value: 0.0,
                            velocity: 0.0,
                            target: 1.0,
                            last: now,
                        },
                        begins: now + Duration::from_secs_f32(delay),
                        quick,
                    },
                );
            }
        }
        let mut pose = Pose::REST;
        let size = scene
            .sizes
            .borrow()
            .get(&key)
            .copied()
            .unwrap_or((c.win().width.get(), c.win().height.get()));
        if let Some(entering) = scene.entering.get_mut(&key) {
            let from = start_of(&entering.transition, entering.quick, size);
            if now < entering.begins {
                entering.progress.last = now;
                pose = from;
                scene.moving = true;
            } else {
                let curve = if entering.quick {
                    QUICK_FADE
                } else {
                    entering.transition.motion
                };
                entering.progress.advance(curve, now);
                if entering.progress.settle(1.0) {
                    scene.entering.remove(&key);
                } else {
                    pose = between(from, entering.progress.value);
                    scene.moving = true;
                }
            }
        }
        // Nudging.
        let trigger = match (&motion.nudge, &motion.nudge_key) {
            (Some(on), _) => Some((Value::Bool(*on), false)),
            (None, Some(value)) => Some((value.clone(), true)),
            (None, None) => None,
        };
        if let Some((value, keyed)) = trigger {
            let push = motion.nudge_scale.unwrap_or(1.06);
            let nudge = scene.nudges.entry(key.clone()).or_insert_with(|| Nudge {
                scale: Spring::at(1.0, now),
                last: value.clone(),
                back_at: None,
            });
            let set_off = if keyed {
                nudge.last != value
            } else {
                value == Value::Bool(true) && nudge.last != Value::Bool(true)
            };
            nudge.last = value;
            if set_off && !quick {
                log(|| format!("nudge {key}"));
                nudge.scale.target = push;
                nudge.scale.last = now;
                nudge.back_at = Some(now + NUDGE_HOLD);
                scene.sizes.borrow_mut().remove(&key);
            }
            if let Some(back_at) = nudge.back_at {
                nudge.scale.advance(NUDGE_OUT, now);
                if now >= back_at {
                    nudge.scale.target = 1.0;
                    nudge.back_at = None;
                }
                scene.moving = true;
            } else if nudge.scale.value != 1.0 || nudge.scale.velocity != 0.0 {
                nudge.scale.advance(JELLY, now);
                if !nudge.scale.settle(1.0) {
                    scene.moving = true;
                }
            }
            pose.scale *= nudge.scale.value;
        }
        // Shared elements: one that appears where another with its key was
        // drawn in the last frame grows from there. Its own place is known
        // once it is laid out, so it is hidden for a frame and measured.
        let mut measure = false;
        let hero = motion.hero.as_ref().filter(|_| !GOING.with(Cell::get));
        if let Some((hero, curve)) = hero {
            let last = |scene: &Scene| {
                scene
                    .heroes
                    .get(hero)
                    .filter(|(_, frame)| *frame + 1 == scene.drawn)
                    .map(|(bounds, _)| *bounds)
            };
            if fresh
                && scene.drawn > 1
                && !quick
                && let Some(from) = last(scene)
            {
                scene.flights.insert(
                    key.clone(),
                    Flight {
                        from,
                        progress: Spring {
                            value: 0.0,
                            velocity: 0.0,
                            target: 1.0,
                            last: now,
                        },
                        curve: *curve,
                    },
                );
                pose.opacity = 0.0;
                measure = true;
                scene.moving = true;
            } else if scene.flights.contains_key(&key) {
                let to = last(scene);
                let outer = crate::units::scale();
                if let Some(flight) = scene.flights.get_mut(&key) {
                    match to {
                        Some(to) if f32::from(to.size.width) > 0.0 => {
                            flight.progress.advance(flight.curve, now);
                            if flight.progress.settle(1.0) {
                                scene.flights.remove(&key);
                            } else {
                                let left = 1.0 - flight.progress.value;
                                let from = flight.from;
                                let delta = from.center() - to.center();
                                let ratio = f32::from(from.size.width) / f32::from(to.size.width);
                                pose.x += f32::from(delta.x) / outer * left;
                                pose.y += f32::from(delta.y) / outer * left;
                                pose.scale *= 1.0 + (ratio - 1.0) * left;
                                scene.moving = true;
                            }
                        }
                        _ => {
                            scene.flights.remove(&key);
                        }
                    }
                }
            }
        }
        // Following scrolling: larger and more opaque nearer the middle
        // of the scroll view, by where the element was last laid out and
        // how far the view has scrolled since.
        if let Some((scale, opacity, origin)) = motion.scroll
            && !quick
            && let Some(near) = c
                .scroller()
                .and_then(|scroller| crate::scroll::nearness(&scroller, &key))
        {
            pose.scale *= 1.0 + (scale - 1.0) * near;
            pose.opacity *= opacity + (1.0 - opacity) * near;
            pose.origin = origin;
        }
        (fresh, pose, Rc::clone(&scene.sizes), measure)
    });
    // Inside an element that appears, elements do not play their own
    // `appear`, but staggering containers inside still take turns; a
    // container's turns are only for its direct children.
    let around = WITHIN.with(|cell| cell.replace(within || fresh));
    let marks = Marks {
        measure,
        follow: motion
            .scroll
            .and_then(|_| c.scroller())
            .map(|(scroller, _)| (scroller, key.clone())),
        record: motion
            .hero
            .as_ref()
            .filter(|_| !GOING.with(Cell::get))
            .map(|(hero, _)| (Rc::clone(&c.win().id), hero.clone())),
    };
    let element = posed(c, &key, pose, own, &sizes, marks, build);
    WITHIN.with(|cell| cell.set(around));
    CLOCK.with(|cell| *cell.borrow_mut() = clock);
    element
}

/// What an element in motion notes as it is laid out: its size, for the
/// frames that scale it, and where a shared element is, by its key.
#[derive(Default)]
struct Marks {
    measure: bool,
    record: Option<(Rc<str>, String)>,
    /// An element following scrolling: its scroll view and its key.
    follow: Option<(Rc<str>, String)>,
}

/// What `build` makes, drawn in `pose`.
fn posed(
    c: &Ctx,
    key: &str,
    pose: Pose,
    own: bool,
    sizes: &Sizes,
    marks: Marks,
    build: impl FnOnce(&Ctx, Pose) -> El,
) -> El {
    if pose.at_rest() && !marks.measure {
        let element = build(c, Pose::REST);
        if marks.record.is_none() && marks.follow.is_none() {
            return element;
        }
        let measure = marks
            .follow
            .as_ref()
            .map(|_| (key.to_string(), Rc::clone(sizes), crate::units::scale()));
        Shifted {
            follow: marks.follow,
            measure,
            ..Shifted::around(element, marks.record)
        }
        .into_any_element()
    } else if own {
        let element = crate::units::scaled(pose.scale, || build(c, pose));
        Shifted {
            follow: marks.follow,
            ..Shifted::around(element, marks.record)
        }
        .into_any_element()
    } else {
        boxed(c, key, pose, sizes, marks, build)
    }
}

// MARK: - Leaving

/// What a condition showed, and what of it is still leaving.
struct Branch {
    shown: usize,
    value: Value,
    leaving: Option<(usize, Value)>,
}

thread_local! {
    /// How many elements built in the current `left` are still leaving.
    static LEAVES: Cell<u32> = const { Cell::new(0) };
    /// Whether what is being built is leaving: shared elements in it are
    /// no longer where their key is.
    static GOING: Cell<bool> = const { Cell::new(false) };
}

/// The branch of the condition `id` that is leaving, with the value its
/// bindings had: the branch shown before, while elements of it play their
/// `disappear`. `shown` is the branch shown now, with its value; `leaves`
/// says for each branch whether elements of it play `disappear`.
pub fn leaving(
    c: &Ctx,
    id: &str,
    shown: usize,
    value: Value,
    leaves: &[bool],
) -> Option<(usize, Value)> {
    if c.listing() {
        return None;
    }
    let key = format!("{}#branch", c.key(id));
    with_scene(c, |scene| {
        let fresh = scene.fresh(&key);
        let branch = scene.branches.entry(key).or_insert(Branch {
            shown,
            value: Value::Unit,
            leaving: None,
        });
        if fresh {
            *branch = Branch {
                shown,
                value,
                leaving: None,
            };
            return None;
        }
        if branch.shown != shown {
            let before = std::mem::replace(&mut branch.value, value);
            branch.leaving = leaves
                .get(branch.shown)
                .copied()
                .unwrap_or(false)
                .then_some((branch.shown, before));
            branch.shown = shown;
        } else {
            branch.value = value;
        }
        // A branch shown again stops leaving.
        if branch
            .leaving
            .as_ref()
            .is_some_and(|(leaving, _)| *leaving == shown)
        {
            branch.leaving = None;
        }
        branch.leaving.clone()
    })
}

/// Build what leaves of the condition `id`; once none of it moves, it is
/// gone.
pub fn left(c: &Ctx, id: &str, build: impl FnOnce()) {
    let around = LEAVES.with(|leaves| leaves.replace(0));
    build();
    let still = LEAVES.with(|leaves| leaves.replace(around));
    if still == 0 {
        let key = format!("{}#branch", c.key(id));
        with_scene(c, |scene| {
            if let Some(branch) = scene.branches.get_mut(&key) {
                branch.leaving = None;
            }
        });
    }
}

/// The rows of the list `id`: its `items` now, by the key `key_of` gives,
/// and among them, where they were, the rows that left while elements of
/// them play their `disappear`, marked `true`.
pub fn rows(
    c: &Ctx,
    id: &str,
    items: Vec<Value>,
    leaves: bool,
    key_of: impl Fn(&Value) -> Value,
) -> Vec<(Value, bool)> {
    if c.listing() || !leaves {
        return items.into_iter().map(|item| (item, false)).collect();
    }
    let key = format!("{}#rows", c.key(id));
    let keyed: Vec<(String, Value)> = items
        .into_iter()
        .map(|item| (crate::value::key_text(&key_of(&item)), item))
        .collect();
    with_scene(c, |scene| {
        let fresh = scene.fresh(&key);
        let list = scene.lists.entry(key).or_default();
        if fresh {
            list.before = keyed.iter().map(|(key, _)| key.clone()).collect();
            list.items = keyed.iter().cloned().collect();
            list.leaving.clear();
            return keyed.into_iter().map(|(_, item)| (item, false)).collect();
        }
        // Rows gone since the last frame start leaving where they were.
        let now: std::collections::HashSet<&str> =
            keyed.iter().map(|(key, _)| key.as_str()).collect();
        for (index, gone) in list.before.iter().enumerate() {
            if !now.contains(gone.as_str())
                && !list.leaving.iter().any(|(key, _, _)| key == gone)
                && let Some(item) = list.items.get(gone)
            {
                list.leaving.push((gone.clone(), item.clone(), index));
            }
        }
        // Rows back again stop leaving.
        list.leaving
            .retain(|(key, _, _)| !now.contains(key.as_str()));
        list.before = keyed.iter().map(|(key, _)| key.clone()).collect();
        list.items = keyed.iter().cloned().collect();
        for (key, item, _) in &list.leaving {
            list.items.insert(key.clone(), item.clone());
        }
        let mut rows: Vec<(Value, bool)> =
            keyed.into_iter().map(|(_, item)| (item, false)).collect();
        let mut leaving = list.leaving.clone();
        leaving.sort_by_key(|(_, _, index)| *index);
        for (_, item, index) in leaving {
            rows.insert(index.min(rows.len()), (item, true));
        }
        rows
    })
}

/// Build a row of the list `id` that is leaving, by its key; once none of
/// it moves, it is gone.
pub fn left_row(c: &Ctx, id: &str, row: &Value, build: impl FnOnce()) {
    let around = LEAVES.with(|leaves| leaves.replace(0));
    build();
    let still = LEAVES.with(|leaves| leaves.replace(around));
    if still == 0 {
        let key = format!("{}#rows", c.key(id));
        let row = crate::value::key_text(row);
        with_scene(c, |scene| {
            if let Some(list) = scene.lists.get_mut(&key) {
                list.leaving.retain(|(key, _, _)| *key != row);
            }
        });
    }
}

/// The element `id`, leaving with `transition`: drawn as `build` makes it
/// on its way out, then not at all.
pub fn leave(
    c: &Ctx,
    id: &str,
    transition: &'static Transition,
    own: bool,
    build: impl FnOnce(&Ctx, Pose) -> El,
) -> El {
    if c.listing() {
        return div().into_any_element();
    }
    let key = format!("{}#leave", c.key(id));
    // Its size while it was shown, measured under its own key.
    let shown = c.key(id);
    let now = now();
    let (pose, gone, sizes) = with_scene(c, |scene| {
        let fresh = scene.fresh(&key);
        let quick = scene.reduced();
        if fresh {
            scene.leaving.insert(
                key.clone(),
                Spring {
                    value: 0.0,
                    velocity: 0.0,
                    target: 1.0,
                    last: now,
                },
            );
        }
        let Some(progress) = scene.leaving.get_mut(&key) else {
            return (Pose::REST, true, Rc::clone(&scene.sizes));
        };
        progress.advance(if quick { QUICK_FADE } else { transition.motion }, now);
        // Gone once it no longer shows, without waiting for a bounce.
        let size = scene
            .sizes
            .borrow()
            .get(&shown)
            .copied()
            .unwrap_or((c.win().width.get(), c.win().height.get()));
        let away = start_of(transition, quick, size);
        let pose = between(away, 1.0 - progress.value);
        // Gone once nothing of it shows, without waiting for the spring to
        // come to rest.
        let invisible = pose.opacity <= 0.01 || pose.grow.is_some_and(|(_, shown)| shown <= 0.01);
        let gone = progress.settle(1.0) || (invisible && progress.value >= 0.99);
        if !gone {
            scene.moving = true;
        }
        (pose, gone, Rc::clone(&scene.sizes))
    });
    if gone {
        return div().into_any_element();
    }
    LEAVES.with(|leaves| leaves.set(leaves.get() + 1));
    // Nothing inside what leaves appears.
    let around = WITHIN.with(|cell| cell.replace(true));
    let going = GOING.with(|cell| cell.replace(true));
    let clock = CLOCK.with(|cell| cell.borrow_mut().take());
    let element = posed(c, &key, pose, own, &sizes, Marks::default(), build);
    GOING.with(|cell| cell.set(going));
    WITHIN.with(|cell| cell.set(around));
    CLOCK.with(|cell| *cell.borrow_mut() = clock);
    element
}

/// An element drawn in a box of its own in `pose`: the box fades and moves
/// it, and where it scales or grows from an edge, keeps its size at rest,
/// measured the frame before.
fn boxed(
    c: &Ctx,
    key: &str,
    pose: Pose,
    sizes: &Sizes,
    marks: Marks,
    build: impl FnOnce(&Ctx, Pose) -> El,
) -> El {
    let outer = crate::units::scale();
    let scaled = (pose.scale - 1.0).abs() >= 0.0005;
    let hidden = pose.opacity <= 0.001 || pose.grow.is_some_and(|(_, shown)| shown <= 0.0);
    let size = sizes.borrow().get(key).copied();
    let (content, measuring) = match size {
        Some((width, height)) if (scaled || pose.grow.is_some()) && !marks.measure => {
            let content = if scaled {
                crate::units::scaled(pose.scale, || build(c, pose))
            } else {
                build(c, pose)
            };
            let (shown_width, shown_height) = (width * pose.scale, height * pose.scale);
            let face = div()
                .absolute()
                .left(px((width - shown_width) * pose.origin.0))
                .top(px((height - shown_height) * pose.origin.1))
                .w(px(shown_width))
                .h(px(shown_height))
                .flex()
                .flex_col()
                .child(content);
            let face = match pose.grow {
                Some((edge, shown)) => clipped(face.into_any_element(), edge, shown, width, height),
                None => face.into_any_element(),
            };
            let element = div()
                .relative()
                .flex_none()
                .w(px(width))
                .h(px(height))
                .opacity(pose.opacity)
                .child(face)
                .into_any_element();
            (element, false)
        }
        _ => {
            // Faded by its container's own box where the container takes
            // it, else by a box around it; moved without a box, as it lies
            // at rest. Not measured yet, it is drawn at its size this once,
            // where it does not show yet, and measured.
            let fading = !hidden && pose.opacity < 0.999;
            let (content, fade) = if fading {
                let (faded, fade) = c.with_fade(pose.opacity);
                (build(&faded, pose), Some(fade))
            } else {
                (build(c, pose), None)
            };
            let content = match fade {
                Some(fade) if !fade.taken() => c
                    .grow()
                    .apply(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(0.0))
                            .max_w(gpui::relative(1.0)),
                    )
                    .opacity(pose.opacity)
                    .child(content)
                    .into_any_element(),
                _ => content,
            };
            (content, scaled || pose.grow.is_some() || marks.measure)
        }
    };
    Shifted {
        child: content,
        offset: gpui::point(px(pose.x), px(pose.y)),
        hidden,
        measure: measuring.then(|| (key.to_string(), Rc::clone(sizes), outer)),
        record: marks.record,
        follow: marks.follow,
        origin: false,
        reflow: None,
    }
    .into_any_element()
}

/// An element moved by `offset` and, while `hidden`, not drawn, laid out
/// as it is: it takes no box of its own. With `measure`, its size at rest
/// is noted for the frames that scale it.
struct Shifted {
    child: El,
    offset: gpui::Point<Pixels>,
    hidden: bool,
    measure: Option<(String, Sizes, f32)>,
    /// A shared element, whose place is noted by its key in its window.
    record: Option<(Rc<str>, String)>,
    /// An element following scrolling, whose place its scroll view notes.
    follow: Option<(Rc<str>, String)>,
    /// A reflowing container: its children's places are measured from
    /// its own.
    origin: bool,
    /// A child of a reflowing container, which slides to its new place.
    reflow: Option<Reflow>,
}

/// A child of a reflowing container: its key, its window, the spring it
/// slides with, and the window's size as it was built.
struct Reflow {
    key: String,
    win: Rc<str>,
    curve: Curve,
    window: (f32, f32),
}

/// Where a reflowing child was, and how far it still is from its place.
struct Slide {
    at: gpui::Point<Pixels>,
    window: (f32, f32),
    x: Spring,
    y: Spring,
}

thread_local! {
    /// Where the reflowing container whose children are being prepainted
    /// lies.
    static ORIGIN: Cell<Option<gpui::Point<Pixels>>> = const { Cell::new(None) };
}

impl Shifted {
    /// `child` where it lies, noting where it is when it is a shared
    /// element.
    fn around(child: El, record: Option<(Rc<str>, String)>) -> Self {
        Shifted {
            child,
            offset: gpui::Point::default(),
            hidden: false,
            measure: None,
            record,
            follow: None,
            origin: false,
            reflow: None,
        }
    }
}

/// `element`, hidden: laid out and prepainted as ever, not drawn.
pub(crate) fn unseen(element: El) -> El {
    Shifted {
        hidden: true,
        ..Shifted::around(element, None)
    }
    .into_any_element()
}

/// The children of the container `element` slide to their new places
/// when their places change (`reflow`).
pub fn reflow_origin(element: El) -> El {
    Shifted {
        origin: true,
        ..Shifted::around(element, None)
    }
    .into_any_element()
}

/// The child `id` of a reflowing container, sliding to its new place with
/// `curve` when it changes, unless the window changed size.
pub fn reflowed(c: &Ctx, id: &str, curve: Curve, element: El) -> El {
    if c.listing() {
        return element;
    }
    let key = format!("{}#slide", c.key(id));
    with_scene(c, |scene| scene.fresh(&key));
    Shifted {
        reflow: Some(Reflow {
            key,
            win: Rc::clone(&c.win().id),
            curve,
            window: (c.win().width.get(), c.win().height.get()),
        }),
        ..Shifted::around(element, None)
    }
    .into_any_element()
}

impl Reflow {
    /// How far the child at `bounds` is from its place.
    fn offset(&self, bounds: Bounds<Pixels>, window: &mut gpui::Window) -> gpui::Point<Pixels> {
        let at = bounds.origin - ORIGIN.with(Cell::get).unwrap_or_default();
        let now = now();
        let moving = SCENES.with(|scenes| {
            let mut scenes = scenes.borrow_mut();
            let scene = scenes.get_mut(&self.win)?;
            let reduced = scene.reduced();
            let slide = scene
                .slides
                .entry(self.key.clone())
                .or_insert_with(|| Slide {
                    at,
                    window: self.window,
                    x: Spring::at(0.0, now),
                    y: Spring::at(0.0, now),
                });
            if slide.at != at {
                if slide.window == self.window && !reduced {
                    // From where it showed, on to its place.
                    slide.x.value += f32::from(slide.at.x - at.x);
                    slide.y.value += f32::from(slide.at.y - at.y);
                    slide.x.last = now;
                    slide.y.last = now;
                }
                slide.at = at;
            }
            slide.window = self.window;
            slide.x.advance(self.curve, now);
            slide.y.advance(self.curve, now);
            let settled = slide.x.settle(1.0) & slide.y.settle(1.0);
            Some((
                gpui::point(gpui::px(slide.x.value), gpui::px(slide.y.value)),
                !settled,
            ))
        });
        match moving {
            Some((offset, moving)) => {
                if moving {
                    window.request_animation_frame();
                }
                offset
            }
            None => gpui::Point::default(),
        }
    }
}

type Sizes = Rc<RefCell<HashMap<String, (f32, f32)>>>;

impl IntoElement for Shifted {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for Shifted {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        if let Some((key, sizes, outer)) = &self.measure {
            sizes.borrow_mut().insert(
                key.clone(),
                (
                    f32::from(bounds.size.width) / outer,
                    f32::from(bounds.size.height) / outer,
                ),
            );
        }
        if let Some((scroller, key)) = &self.follow {
            crate::scroll::follow(scroller, key, bounds);
        }
        if let Some((win, hero)) = &self.record {
            SCENES.with(|scenes| {
                if let Some(scene) = scenes.borrow_mut().get_mut(win) {
                    let frame = scene.drawn;
                    scene.heroes.insert(hero.clone(), (bounds, frame));
                }
            });
        }
        let offset = match &self.reflow {
            Some(reflow) => self.offset + reflow.offset(bounds, window),
            None => self.offset,
        };
        let around = self
            .origin
            .then(|| ORIGIN.with(|origin| origin.replace(Some(bounds.origin))));
        // Laid out and prepainted even while hidden, so what measures
        // itself there is ready when it shows.
        window.with_element_offset(offset, |window| {
            self.child.prepaint(window, cx);
        });
        if let Some(around) = around {
            ORIGIN.with(|origin| origin.set(around));
        }
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        if !self.hidden {
            self.child.paint(window, cx);
        }
    }
}

/// `face`, `width` by `height`, showing `shown` of itself from `edge`.
fn clipped(face: El, edge: Edge, shown: f32, width: f32, height: f32) -> El {
    let clip = div().absolute().overflow_hidden();
    let clip = match edge {
        Edge::Top => clip
            .top(px(0.0))
            .left(px(0.0))
            .w(px(width))
            .h(px(height * shown)),
        Edge::Bottom => clip
            .bottom(px(0.0))
            .left(px(0.0))
            .w(px(width))
            .h(px(height * shown)),
        Edge::Leading => clip
            .top(px(0.0))
            .left(px(0.0))
            .w(px(width * shown))
            .h(px(height)),
        Edge::Trailing => clip
            .top(px(0.0))
            .right(px(0.0))
            .w(px(width * shown))
            .h(px(height)),
    };
    let inside = div().absolute().w(px(width)).h(px(height));
    let inside = match edge {
        Edge::Top | Edge::Leading => inside.top(px(0.0)).left(px(0.0)),
        Edge::Bottom => inside.bottom(px(0.0)).left(px(0.0)),
        Edge::Trailing => inside.top(px(0.0)).right(px(0.0)),
    };
    clip.child(inside.child(face)).into_any_element()
}

/// `value` of the element's length `id`, springing to it with `curve`
/// when it changes (`animate`); as it is the first time it shows.
pub fn eased(c: &Ctx, id: &str, curve: Curve, value: f32) -> f32 {
    if c.listing() {
        return value;
    }
    let key = format!("{}~", c.key(id));
    spring_to(c, &key, curve, value, value.abs().max(1.0))
}

/// The value of the spring `key` on its way to `target`, moving with
/// `curve`; it starts at the target. `unit` is the size of a change.
pub(crate) fn spring_to(c: &Ctx, key: &str, curve: Curve, target: f32, unit: f32) -> f32 {
    spring_from(c, key, None, curve, target, unit)
}

/// The same, starting at `start` where given rather than at the target.
pub(crate) fn spring_from(
    c: &Ctx,
    key: &str,
    start: Option<f32>,
    curve: Curve,
    target: f32,
    unit: f32,
) -> f32 {
    if c.listing() {
        return target;
    }
    let now = now();
    with_scene(c, |scene| {
        scene.fresh(key);
        let reduced = scene.reduced();
        let spring = scene
            .springs
            .entry(key.to_string())
            .or_insert_with(|| Spring::toward(start.unwrap_or(target), target, now));
        if (spring.target - target).abs() > f32::EPSILON {
            spring.target = target;
        }
        if reduced {
            *spring = Spring::at(target, now);
            return target;
        }
        spring.advance(curve, now);
        if !spring.settle(unit.max(1.0)) {
            scene.moving = true;
        }
        spring.value
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_reaches_its_target_and_rests() {
        let start = Instant::now();
        let mut spring = Spring {
            value: 0.0,
            velocity: 0.0,
            target: 1.0,
            last: start,
        };
        let mut overshot = false;
        let mut at = start;
        for _ in 0..120 {
            at += Duration::from_millis(16);
            spring.advance((0.42, 0.42), at);
            overshot |= spring.value > 1.0;
        }
        assert!(overshot, "jelly bounces past its target");
        assert!(spring.settle(1.0), "and comes to rest within two seconds");
        assert_eq!(spring.value, 1.0);
    }

    #[test]
    fn a_damped_spring_does_not_overshoot() {
        let start = Instant::now();
        let mut spring = Spring {
            value: 0.0,
            velocity: 0.0,
            target: 1.0,
            last: start,
        };
        let mut at = start;
        for _ in 0..90 {
            at += Duration::from_millis(16);
            spring.advance((0.45, 1.0), at);
            assert!(spring.value <= 1.0 + 1e-4);
        }
        assert!(spring.settle(1.0));
    }

    #[test]
    fn transitions_start_where_they_say() {
        let pop = Transition {
            kind: TransitionKind::Pop(0.4),
            motion: (0.5, 0.62),
        };
        let from = start_of(&pop, false, (100.0, 100.0));
        assert_eq!((from.scale, from.opacity), (0.4, 0.0));
        let slide = Transition {
            kind: TransitionKind::Slide(Edge::Bottom, 12.0),
            motion: (0.45, 1.0),
        };
        assert_eq!(start_of(&slide, false, (100.0, 100.0)).y, 12.0);
        assert_eq!(start_of(&slide, true, (100.0, 100.0)).y, 0.0, "less motion only fades");
        let grow = Transition {
            kind: TransitionKind::Grow(Edge::Leading),
            motion: (0.45, 1.0),
        };
        let from = start_of(&grow, false, (100.0, 100.0));
        assert_eq!(from.opacity, 1.0);
        assert_eq!(from.grow, Some((Edge::Leading, 0.0)));
        let half = between(from, 0.5);
        assert_eq!(half.grow, Some((Edge::Leading, 0.5)));
        assert!(between(from, 1.2).grow.unwrap().1 <= 1.0);
    }
}
