//! Pointer events any visible element takes, on the widgets it made: a
//! double click or tap, a context menu and the turns of a wheel. The
//! controllers go on those widgets themselves, so nothing about the
//! element's layout changes.

use crate::gesture_state::{Notches, Presses};
use crate::reactive::on_cleanup;
use crate::widgets::{MenuItem, bound_action};
use crate::{Act, Build, Ctx, Parent};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

type Run = Rc<dyn Fn()>;

/// Surface pixels of smooth scrolling, as from a touchpad, that make one
/// notch of a wheel.
const SURFACE_NOTCH: f64 = 50.0;

#[derive(Default)]
pub struct PointerProps {
    pub double_tap: Option<Act>,
    /// `ContextMenu`: what a secondary click or a long touch opens.
    pub menu: Vec<Option<MenuItem>>,
    pub wheel_up: Option<Act>,
    pub wheel_down: Option<Act>,
    /// `Hover(delay:)`: action run after the pointer rests for `delay` seconds.
    pub hover: Option<(Act, f64)>,
    /// The wheel reaches the element before its content, as a slider
    /// would otherwise scroll its value.
    pub capture_wheel: bool,
}

/// The element `build` makes, with its pointer events.
#[allow(non_snake_case)]
pub fn Pointer(p: &Parent, ctx: &Ctx, props: PointerProps, build: Build) {
    // A region of the same parent: the element places itself as it would
    // without events.
    let region = p.region();
    build(&region, ctx);
    let double = bound_action(ctx, &props.double_tap);
    let up = bound_action(ctx, &props.wheel_up);
    let down = bound_action(ctx, &props.wheel_down);
    let hover = props.hover.as_ref().map(|(act, delay)| {
        let at = ctx.clone();
        let act = Rc::clone(act);
        (Rc::new(move || act(&at)) as Run, *delay)
    });
    let mut menu = Some(props.menu).filter(|menu| !menu.is_empty());
    let wheels = up.is_some() || down.is_some();
    for widget in region.widgets() {
        // Content that otherwise takes no input, such as a progress ring.
        widget.set_can_target(true);
        if let Some(run) = &double {
            double_taps(&widget, Rc::clone(run));
        }
        // An element makes one widget; a menu has one place to open.
        if let Some(items) = menu.take() {
            crate::table::context_menu(ctx, &widget, items);
        }
        if wheels {
            wheel(&widget, up.clone(), down.clone(), props.capture_wheel);
        }
        if let Some((run, delay)) = &hover {
            self::hover(&widget, Rc::clone(run), *delay);
        }
    }
}

/// Whether a press at `(x, y)` in surface coordinates hits an interactive
/// control nested inside `owner` (such as a button or slider), so the
/// owner's `DoubleTap` does not hijack rapid clicks on child controls.
fn tap_in_control(owner: &gtk::Widget, x: f64, y: f64) -> bool {
    let Some(root) = owner.root() else {
        return false;
    };
    let root: gtk::Widget = root.upcast();
    let Some(point) = root.compute_point(owner, &gtk::graphene::Point::new(x as f32, y as f32))
    else {
        return false;
    };
    let Some(mut node) = owner.pick(
        f64::from(point.x()),
        f64::from(point.y()),
        gtk::PickFlags::DEFAULT,
    ) else {
        return false;
    };
    while node != *owner {
        if node.is::<gtk::Button>()
            || node.is::<gtk::ToggleButton>()
            || node.is::<gtk::Switch>()
            || node.is::<gtk::Scale>()
            || node.is::<gtk::DropDown>()
            || node.is::<gtk::Editable>()
            || node.is::<gtk::TextView>()
        {
            return true;
        }
        let Some(parent) = node.parent() else {
            break;
        };
        node = parent;
    }
    false
}

/// Run `run` on the second press of a double click or tap. Runs in the
/// bubble phase so nested `DoubleTap` owners and interactive child controls
/// take presses first; presses inside child controls are skipped.
fn double_taps(widget: &gtk::Widget, run: Run) {
    let presses = Rc::new(RefCell::new(Presses::default()));
    let events = gtk::EventControllerLegacy::new();
    events.set_propagation_phase(gtk::PropagationPhase::Bubble);
    {
        let presses = Rc::clone(&presses);
        let weak = widget.downgrade();
        events.connect_event(move |_, event| {
            use gtk::gdk::EventType;
            let Some(widget) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let settings = widget.settings();
            let distance = settings.gtk_double_click_distance().max(1) as f64;
            let (x, y) = event.position().unwrap_or_default();
            match event.event_type() {
                EventType::ButtonPress | EventType::TouchBegin => {
                    let primary = event.event_type() == EventType::TouchBegin
                        || (!event.is_pointer_emulated()
                            && event
                                .downcast_ref::<gtk::gdk::ButtonEvent>()
                                .is_some_and(|e| e.button() == gtk::gdk::BUTTON_PRIMARY));
                    if !primary {
                        return glib::Propagation::Proceed;
                    }
                    if tap_in_control(&widget, x, y) {
                        presses.borrow_mut().cancel();
                        return glib::Propagation::Proceed;
                    }
                    let double = presses.borrow_mut().press(
                        u64::from(event.time()),
                        x,
                        y,
                        settings.gtk_double_click_time().max(1) as u64,
                        distance,
                    );
                    if double {
                        run();
                        return glib::Propagation::Stop;
                    }
                }
                EventType::MotionNotify | EventType::TouchUpdate => {
                    presses.borrow_mut().moved(x, y, distance);
                }
                EventType::ButtonRelease | EventType::TouchEnd => presses.borrow_mut().release(),
                EventType::TouchCancel | EventType::GrabBroken => presses.borrow_mut().cancel(),
                _ => {}
            }
            glib::Propagation::Proceed
        });
    }
    widget.add_controller(events);
    {
        let presses = Rc::clone(&presses);
        widget.connect_unmap(move |_| presses.borrow_mut().cancel());
    }
    on_cleanup(move || presses.borrow_mut().cancel());
}

/// Run `up` or `down` once for each notch the wheel or trackpad scrolls
/// over `widget`. While it has either, vertical scrolling over it stops
/// there, so a scroll view around it stays; with Control it goes on, as
/// that zooms.
fn wheel(widget: &gtk::Widget, up: Option<Run>, down: Option<Run>, capture: bool) {
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    if capture {
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    }
    let notches = Rc::new(RefCell::new(Notches::default()));
    {
        let notches = Rc::clone(&notches);
        scroll.connect_scroll_begin(move |_| notches.borrow_mut().reset());
    }
    {
        let notches = Rc::clone(&notches);
        scroll.connect_scroll_end(move |_| notches.borrow_mut().reset());
    }
    scroll.connect_scroll(move |controller, dx, dy| {
        if dy == 0.0
            || dy.abs() < dx.abs()
            || controller
                .current_event_state()
                .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        let target = if dy < 0.0 { &up } else { &down };
        let Some(run) = target else {
            return glib::Propagation::Proceed;
        };
        let delta = match controller.unit() {
            gtk::gdk::ScrollUnit::Surface => dy / SURFACE_NOTCH,
            _ => dy,
        };
        let turned = notches.borrow_mut().scroll(delta);
        if (turned < 0 && dy < 0.0) || (turned > 0 && dy > 0.0) {
            for _ in 0..turned.unsigned_abs() {
                run();
            }
        }
        glib::Propagation::Stop
    });
    widget.add_controller(scroll);
}

/// Run `run` after the pointer rests over `widget` for `delay` seconds;
/// leaving or unmapping cancels a pending timer.
fn hover(widget: &gtk::Widget, run: Run, delay: f64) {
    let motion = gtk::EventControllerMotion::new();
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let cancel = |slot: &Rc<RefCell<Option<glib::SourceId>>>| {
        if let Some(id) = slot.borrow_mut().take() {
            id.remove();
        }
    };
    {
        let pending = Rc::clone(&pending);
        let weak = widget.downgrade();
        let wait = if delay.is_finite() && delay > 0.0 {
            Duration::from_secs_f64(delay)
        } else {
            Duration::ZERO
        };
        motion.connect_enter(move |_, _, _| {
            cancel(&pending);
            let done = Rc::clone(&pending);
            let run = Rc::clone(&run);
            let weak = weak.clone();
            *pending.borrow_mut() = Some(glib::timeout_add_local_once(wait, move || {
                done.borrow_mut().take();
                if weak.upgrade().is_some() {
                    run();
                }
            }));
        });
    }
    {
        let pending = Rc::clone(&pending);
        motion.connect_leave(move |_| cancel(&pending));
    }
    widget.add_controller(motion);
    {
        let pending = Rc::clone(&pending);
        widget.connect_unmap(move |_| cancel(&pending));
    }
    on_cleanup(move || cancel(&pending));
}
