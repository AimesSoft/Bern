//! GTK event adapters. Native buttons still own keyboard, accessibility,
//! focus and pressed visuals; pointer arbitration only changes their actions.
use crate::Ctx;
use crate::features::GestureProps;
use crate::gesture_state::{Drag, Tap, Taps, finish_swipe};
use crate::protocol::Value;
use crate::reactive::on_cleanup;
use crate::widgets::Bound;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

type Run = Option<Rc<dyn Fn()>>;

fn run(action: &Run) {
    if let Some(action) = action {
        action();
    }
}

fn stop_timer(timer: &RefCell<Option<glib::SourceId>>) {
    if let Some(timer) = timer.borrow_mut().take() {
        timer.remove();
    }
}

/// Cancels the native press without changing the button's keyboard activation.
fn deny_button(button: &gtk::Button) {
    let controllers = button.observe_controllers();
    for i in 0..controllers.n_items() {
        if let Some(gesture) = controllers
            .item(i)
            .and_then(|c| c.downcast::<gtk::Gesture>().ok())
        {
            gesture.reset();
        }
    }
}

pub(crate) fn button_clicks(button: &gtk::Button, single: Run, double: Run) {
    let Some(double) = double else {
        if let Some(single) = single {
            button.connect_clicked(move |_| single());
        }
        return;
    };
    let taps = Rc::new(RefCell::new(Taps::default()));
    let pointer = Rc::new(Cell::new(false));
    let contact = Rc::new(Cell::new(false));
    let sequence = Rc::new(RefCell::new(None::<gtk::gdk::EventSequence>));
    let timer = Rc::new(RefCell::new(None));
    let live = Rc::new(Cell::new(true));
    let cancel: Rc<dyn Fn()> = {
        let (taps, timer, pointer, contact) = (
            taps.clone(),
            timer.clone(),
            pointer.clone(),
            contact.clone(),
        );
        Rc::new(move || {
            taps.borrow_mut().cancel();
            pointer.set(false);
            contact.set(false);
            stop_timer(&timer);
        })
    };
    // A capture-phase observer does not compete with GTK's own click gesture.
    // It is installed after the native controller (GTK visits newest first),
    // so it sees the pointer before GtkButton emits clicked.
    let events = gtk::EventControllerLegacy::new();
    events.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let (taps, pointer, contact, sequence, timer, single, cancel) = (
            taps.clone(),
            pointer.clone(),
            contact.clone(),
            sequence.clone(),
            timer.clone(),
            single.clone(),
            cancel.clone(),
        );
        let weak = button.downgrade();
        events.connect_event(move |_, event| {
            use gtk::gdk::EventType;
            let Some(button) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let kind = event.event_type();
            let touch = matches!(
                kind,
                EventType::TouchBegin
                    | EventType::TouchUpdate
                    | EventType::TouchEnd
                    | EventType::TouchCancel
            );
            let id = touch.then(|| event.event_sequence());
            let primary = touch
                || event
                    .downcast_ref::<gtk::gdk::ButtonEvent>()
                    .is_some_and(|e| e.button() == 1);
            let (x, y) = event.position().unwrap_or_default();
            match kind {
                EventType::ButtonPress | EventType::TouchBegin if primary && !contact.get() => {
                    pointer.set(true);
                    contact.set(true);
                    *sequence.borrow_mut() = id;
                    stop_timer(&timer);
                    let settings = button.settings();
                    let flush = taps.borrow_mut().press(
                        glib::monotonic_time() as u64 / 1000,
                        x,
                        y,
                        settings.gtk_double_click_time().max(1) as u64,
                        settings.gtk_double_click_distance().max(1) as f64,
                    );
                    if flush {
                        run(&single);
                    }
                }
                EventType::MotionNotify | EventType::TouchUpdate
                    if pointer.get() && *sequence.borrow() == id =>
                {
                    let moved = taps.borrow_mut().moved(
                        x,
                        y,
                        button.settings().gtk_dnd_drag_threshold().max(1) as f64,
                    );
                    if moved {
                        stop_timer(&timer);
                        deny_button(&button);
                    }
                }
                EventType::ButtonRelease | EventType::TouchEnd
                    if primary && *sequence.borrow() == id =>
                {
                    contact.set(false);
                    let (pointer, taps) = (pointer.clone(), taps.clone());
                    let generation = taps.borrow().generation;
                    // Clear after the native release handler has emitted clicked.
                    glib::idle_add_local_once(move || {
                        if taps.borrow().generation == generation {
                            pointer.set(false);
                        }
                    });
                }
                EventType::TouchCancel if *sequence.borrow() == id => {
                    cancel();
                    pointer.set(false);
                }
                EventType::KeyPress => {
                    cancel();
                    pointer.set(false);
                }
                _ => {}
            }
            glib::Propagation::Proceed
        });
    }
    button.add_controller(events);
    // Another gesture (notably an enclosing Swipe/scroll view) can revoke
    // the press. Neither the pending single nor the second tap may survive it.
    let controllers = button.observe_controllers();
    for i in 0..controllers.n_items() {
        if let Some(gesture) = controllers
            .item(i)
            .and_then(|c| c.downcast::<gtk::GestureClick>().ok())
        {
            let cancel = cancel.clone();
            gesture.connect_cancel(move |_, _| cancel());
        }
    }
    {
        let (pointer, cancel) = (pointer.clone(), cancel.clone());
        button.connect_activate(move |_| {
            pointer.set(false);
            cancel();
        });
    }
    {
        let (taps, timer, live, pointer) =
            (taps.clone(), timer.clone(), live.clone(), pointer.clone());
        button.connect_clicked(move |button| {
            if !live.get() {
                return;
            }
            if !pointer.replace(false) {
                taps.borrow_mut().cancel();
                stop_timer(&timer);
                run(&single);
                return;
            }
            let outcome = taps.borrow_mut().click();
            match outcome {
                Tap::Double => double(),
                Tap::Pending => {
                    let generation = taps.borrow().generation;
                    let (taps, single, timer_done, live) =
                        (taps.clone(), single.clone(), timer.clone(), live.clone());
                    let weak = button.downgrade();
                    let interval = button.settings().gtk_double_click_time().max(1) as u64;
                    *timer.borrow_mut() = Some(glib::timeout_add_local_once(
                        Duration::from_millis(interval),
                        move || {
                            timer_done.borrow_mut().take();
                            let outcome = taps.borrow_mut().expire(generation);
                            if outcome == Tap::Single
                                && live.get()
                                && weak
                                    .upgrade()
                                    .is_some_and(|b| b.is_mapped() && b.is_sensitive())
                            {
                                run(&single);
                            }
                        },
                    ));
                }
                _ => {}
            }
        });
    }
    {
        let cancel = cancel.clone();
        let pointer = pointer.clone();
        button.connect_unmap(move |_| {
            cancel();
            pointer.set(false);
        });
    }
    on_cleanup(move || {
        live.set(false);
        cancel();
    });
}

pub(crate) fn swipe(area: &gtk::Box, ctx: &Ctx, props: &GestureProps, action: Run, commit: Run) {
    let bound = Bound::new(ctx, props.field);
    let active = props.active.map(|field| Bound::new(ctx, field));
    let set_active: Rc<dyn Fn(bool)> = Rc::new(move |value| {
        if let Some(active) = &active {
            active.set(Value::Bool(value));
        }
    });
    set_active(false);
    let state = Rc::new(RefCell::new(Drag::default()));
    let live = Rc::new(Cell::new(true));
    let gesture = gtk::GestureDrag::new();
    gesture.set_button(1);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let cancel: Rc<dyn Fn()> = {
        let (state, set_active) = (state.clone(), set_active.clone());
        Rc::new(move || {
            state.borrow_mut().cancel();
            set_active(false);
        })
    };
    {
        let (state, bound, set_active) = (state.clone(), bound.clone(), set_active.clone());
        gesture.connect_drag_begin(move |_, _, _| {
            state
                .borrow_mut()
                .begin(crate::value::as_f64(&bound.peek()));
            set_active(false);
        });
    }
    let update: Rc<dyn Fn(&gtk::GestureDrag, f64, f64)> = {
        let (state, live, set_active, cancel) = (
            state.clone(),
            live.clone(),
            set_active.clone(),
            cancel.clone(),
        );
        let (vertical, reach, min, max, continuous) = (
            props.vertical,
            props.distance.unwrap_or(300.0),
            props.min,
            props.max,
            props.continuous,
        );
        let weak = area.downgrade();
        let action = action.clone();
        Rc::new(move |gesture, x, y| {
            if !live.get() {
                return;
            }
            let Some(area) = weak.upgrade() else {
                return;
            };
            let was_moving = state.borrow().moving;
            let value = state.borrow_mut().update(
                (x, y),
                vertical,
                area.settings().gtk_dnd_drag_threshold().max(1) as f64,
                reach,
                min,
                max,
            );
            let moving = state.borrow().moving;
            if moving && !was_moving {
                if !gesture.set_state(gtk::EventSequenceState::Claimed) {
                    cancel();
                    return;
                }
                // Claiming cancels descendant buttons. For a Swipe *inside*
                // a Button also revoke the ancestor's native press.
                let mut parent = area.parent();
                while let Some(widget) = parent {
                    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                        deny_button(button);
                    }
                    parent = widget.parent();
                }
                set_active(true);
            }
            if let Some(value) = value {
                if !live.get() {
                    return;
                }
                bound.set(Value::Float(value));
                if continuous && live.get() {
                    run(&action);
                }
            }
        })
    };
    {
        let update = update.clone();
        gesture.connect_drag_update(move |gesture, x, y| update(gesture, x, y));
    }
    {
        let (state, live, set_active) = (state.clone(), live.clone(), set_active.clone());
        let continuous = props.continuous;
        gesture.connect_drag_end(move |gesture, x, y| {
            finish_swipe(
                continuous,
                || {
                    update(gesture, x, y);
                    state.borrow_mut().end() && live.get()
                },
                || run(&action),
                || run(&commit),
                || {
                    state.borrow_mut().cancel();
                    set_active(false);
                },
            );
        });
    }
    {
        let cancel = cancel.clone();
        gesture.connect_cancel(move |_, _| cancel());
    }
    {
        let cancel = cancel.clone();
        gesture.connect_sequence_state_changed(move |_, _, state| {
            if state == gtk::EventSequenceState::Denied {
                cancel();
            }
        });
    }
    area.add_controller(gesture);
    {
        let cancel = cancel.clone();
        area.connect_unmap(move |_| cancel());
    }
    on_cleanup(move || {
        live.set(false);
        cancel();
    });
}
