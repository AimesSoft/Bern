//! Pointer arbitration shared by semantic buttons and value swipes. Window
//! capture listeners keep a recognized drag alive outside its original bounds.

use crate::El;
use crate::ctx::{Act, Ctx, Field, Press};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use gpui::{
    App, ClickEvent, DispatchPhase, Div, ElementId, HitboxBehavior, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, RenderOnce, Stateful, Subscription, Task,
    TouchEvent, TouchId, TouchPhase, Window, canvas, div, prelude::*,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

const SLOP: f64 = 10.0;
type Position = (f64, f64);
fn position(p: gpui::Point<gpui::Pixels>) -> Position {
    (p.x.into(), p.y.into())
}
fn moved(from: Position, to: Position) -> bool {
    (from.0 - to.0).hypot(from.1 - to.1) > SLOP
}

fn double_click_interval() -> Duration {
    #[cfg(target_os = "macos")]
    {
        // GPUI reports AppKit's click count; use the same system interval.
        let seconds: f64 = unsafe { objc2::msg_send![objc2::class!(NSEvent), doubleClickInterval] };
        return Duration::from_secs_f64(seconds.max(0.001));
    }
    #[cfg(target_os = "windows")]
    {
        return Duration::from_millis(unsafe {
            win32::Win32::UI::WindowsAndMessaging::GetDoubleClickTime()
        } as u64);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    // Matches gpui-pre-linux 0.3.10 (Wayland and X11).
    Duration::from_millis(400)
}

#[derive(Debug, PartialEq, Eq)]
enum TapClick {
    Ignore,
    Main,
    Double,
    Wait,
}

#[derive(Default)]
struct TapState {
    down: Option<Position>,
    touch: Option<TouchId>,
    dragged: bool,
    waiting: bool,
    second: bool,
    task: Option<Task<()>>,
}
impl TapState {
    fn cancel(&mut self) {
        self.task = None;
        self.waiting = false;
        self.second = false;
        self.down = None;
        self.dragged = true;
    }
    fn down(&mut self, at: Position, count: usize) {
        self.second = self.waiting && count >= 2;
        if self.second {
            self.task = None;
            self.waiting = false;
        }
        self.down = Some(at);
        self.dragged = false;
    }
    fn click(&mut self, keyboard: bool, count: usize) -> TapClick {
        if keyboard {
            self.cancel();
            return TapClick::Main;
        }
        if self.dragged {
            return TapClick::Ignore;
        }
        self.down = None;
        let double = self.second || (self.waiting && count >= 2);
        self.task = None;
        self.waiting = !double;
        self.second = false;
        if double {
            TapClick::Double
        } else {
            TapClick::Wait
        }
    }
    fn touch_down(&mut self, id: TouchId, at: Position) {
        self.touch = Some(id);
        self.down = Some(at);
        self.dragged = false;
        // GPUI synthesizes mouse down/up only after a touch tap ends.
        // Stop the first tap's timer now, not after the second finger lifts.
        self.task = None;
    }
    fn expire(&mut self) -> bool {
        std::mem::take(&mut self.waiting)
    }
    fn motion(&mut self, at: Position) {
        if self.down.is_some_and(|down| moved(down, at)) {
            self.cancel();
        }
    }
}
struct TapMount {
    state: Rc<RefCell<TapState>>,
    _activation: Subscription,
}
impl Drop for TapMount {
    fn drop(&mut self) {
        self.state.borrow_mut().cancel();
    }
}

#[derive(IntoElement)]
pub(crate) struct ButtonGesture {
    pub key: ElementId,
    pub element: Stateful<Div>,
    pub press: Press,
    pub double: Press,
}
impl RenderOnce for ButtonGesture {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let entity = window.use_keyed_state(self.key, cx, |window, cx| {
            let state = Rc::new(RefCell::new(TapState::default()));
            let activation =
                cx.observe_window_activation(window, |mount: &mut TapMount, window, _| {
                    if !window.is_window_active() {
                        let mut tap = mount.state.borrow_mut();
                        tap.cancel();
                        tap.touch = None;
                    }
                });
            TapMount {
                state,
                _activation: activation,
            }
        });
        let state = entity.read(cx).state.clone();
        let tracking = state.clone();
        let listeners = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let state = tracking.clone();
                let touch_hitbox = hitbox.clone();
                window.on_mouse_event(move |event: &TouchEvent, phase, window, _| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let mut tap = state.borrow_mut();
                    if event.phase == TouchPhase::Started {
                        // A parent Swipe may have consumed the previous
                        // contact's move/end before this observer saw it.
                        if touch_hitbox.is_hovered(window) {
                            tap.touch_down(event.id, position(event.position));
                        }
                    } else if tap.touch == Some(event.id) {
                        tap.motion(position(event.position));
                        if event.phase == TouchPhase::Cancelled {
                            tap.cancel();
                        }
                        if matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                            tap.touch = None;
                        }
                    }
                });
                let state = tracking.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Capture
                        && event.button == MouseButton::Left
                        && hitbox.is_hovered(window)
                    {
                        state
                            .borrow_mut()
                            .down(position(event.position), event.click_count);
                    }
                });
                let state = tracking.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        let mut state = state.borrow_mut();
                        if state.down.is_some() && event.pressed_button != Some(MouseButton::Left) {
                            state.cancel();
                        } else {
                            state.motion(position(event.position));
                        }
                    }
                });
                let state = tracking.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        state.borrow_mut().motion(position(event.position));
                    }
                });
                let state = tracking.clone();
                window.on_key_event(move |event: &KeyDownEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture && event.keystroke.key == "escape" {
                        state.borrow_mut().cancel();
                    }
                });
            },
        )
        .absolute()
        .size_full();
        self.element
            .relative()
            .child(listeners)
            .on_click(move |event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                let outcome = state
                    .borrow_mut()
                    .click(event.is_keyboard(), event.click_count());
                match outcome {
                    TapClick::Ignore => {}
                    TapClick::Main => (self.press)(cx),
                    TapClick::Double => (self.double)(cx),
                    TapClick::Wait => {
                        let weak = Rc::downgrade(&state);
                        let press = self.press.clone();
                        let interval = if window.last_input_was_touch() {
                            Duration::from_millis(400)
                        } else {
                            double_click_interval()
                        };
                        let task = cx.spawn(async move |cx| {
                            cx.background_executor().timer(interval).await;
                            cx.update(|cx| {
                                let Some(state) = weak.upgrade() else {
                                    return;
                                };
                                let run = state.borrow_mut().expire();
                                if run {
                                    press(cx);
                                }
                            });
                        });
                        state.borrow_mut().task = Some(task);
                    }
                }
            })
    }
}

/// How a Swipe maps a pointer displacement onto its draft field.
#[derive(Clone, Copy, Debug)]
pub struct SwipeProps {
    pub min: f64,
    pub max: f64,
    pub horizontal: bool,
    pub distance: f64,
    pub continuous: bool,
    pub active: Option<Field>,
}
#[derive(Default, Debug)]
struct SwipeState {
    down: Option<Position>,
    start: f64,
    claimed: bool,
    moved: bool,
    cancelled: bool,
    touch: Option<TouchId>,
}
impl SwipeState {
    fn begin(&mut self, at: Position, value: f64) {
        self.down = Some(at);
        self.start = value;
        self.claimed = false;
        self.moved = false;
        self.cancelled = false;
        self.touch = None;
    }
    fn change(&mut self, at: Position, props: SwipeProps) -> Option<f64> {
        let down = self.down?;
        self.moved |= moved(down, at);
        let (main, cross) = if props.horizontal {
            (at.0 - down.0, at.1 - down.1)
        } else {
            (down.1 - at.1, at.0 - down.0)
        };
        if !self.claimed && main.abs() > SLOP && main.abs() > cross.abs() {
            self.claimed = true;
        }
        self.claimed.then(|| {
            (self.start + main / props.distance * (props.max - props.min))
                .clamp(props.min, props.max)
        })
    }
    fn cancel(&mut self) -> bool {
        self.cancelled |= self.down.is_some();
        self.end()
    }
    fn end(&mut self) -> bool {
        self.down = None;
        std::mem::take(&mut self.claimed)
    }
}
struct SwipeMount {
    state: Rc<RefCell<SwipeState>>,
    active: Rc<dyn Fn(bool)>,
    _activation: Subscription,
}
impl SwipeMount {
    fn cancel(&self) {
        if self.state.borrow_mut().cancel() {
            (self.active)(false);
        }
    }
}
impl Drop for SwipeMount {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// A drag end commits its value once; continuous previews have already run
/// the primary action. Direct edits use the non-continuous path.
fn commit_swipe(
    continuous: bool,
    action: impl FnOnce(),
    commit: impl FnOnce(),
    release: impl FnOnce(),
) {
    if !continuous {
        action();
    }
    commit();
    release();
}

#[derive(Clone)]
struct SwipeBinding {
    c: Ctx,
    field: Field,
    props: SwipeProps,
    action: Option<Act>,
    commit: Option<Act>,
}
impl SwipeBinding {
    fn active(&self, active: bool) {
        if let Some(field) = self.props.active
            && self.c.read(field) != Value::Bool(active)
        {
            self.c.write(field, Value::Bool(active));
        }
    }
    fn preview(&self, value: f64) {
        let old = self.c.read(self.field);
        let value = crate::behaviors::like(&old, value);
        if old != value && self.c.write(self.field, value) && self.props.continuous {
            self.action();
        }
    }
    fn action(&self) {
        if let Some(action) = &self.action {
            action(&self.c);
        }
    }
    fn finish(&self) {
        // Keep Watch(!active) from restoring a stale engine value between
        // the final preview and its actions. The recognizer was ended first,
        // so actions that synchronously rebuild cannot commit twice.
        self.commit(self.props.continuous, || self.active(false));
    }
    fn commit(&self, continuous: bool, release: impl FnOnce()) {
        commit_swipe(
            continuous,
            || self.action(),
            || {
                if let Some(commit) = &self.commit {
                    commit(&self.c);
                }
            },
            release,
        );
    }
    fn set_number(&self, value: f64) {
        if value.is_finite() {
            let value = crate::behaviors::like(
                &self.c.read(self.field),
                value.clamp(self.props.min, self.props.max),
            );
            if self.c.write(self.field, value) {
                // Programmatic changes are a complete edit, not a drag preview.
                self.commit(false, || {});
            }
        }
    }
}
#[derive(IntoElement)]
struct Swipe {
    key: ElementId,
    binding: SwipeBinding,
    content: Vec<El>,
}
impl RenderOnce for Swipe {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let grow = self.binding.c.grow();
        let binding = self.binding;
        let initial = binding.clone();
        let entity = window.use_keyed_state(self.key.clone(), cx, move |window, cx| {
            initial.active(false);
            let activation =
                cx.observe_window_activation(window, |mount: &mut SwipeMount, window, _| {
                    if !window.is_window_active() {
                        mount.cancel();
                    }
                });
            SwipeMount {
                state: Rc::new(RefCell::new(SwipeState::default())),
                active: Rc::new(move |active| initial.active(active)),
                _activation: activation,
            }
        });
        let state = entity.read(cx).state.clone();
        if !binding.c.enabled() {
            entity.read(cx).cancel();
        }
        let enabled = binding.c.enabled();
        let listeners = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                if !enabled {
                    return;
                }
                let touch_state = state.clone();
                let touch_binding = binding.clone();
                let touch_hitbox = hitbox.clone();
                window.on_mouse_event(move |event: &TouchEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    if event.phase == TouchPhase::Started {
                        if touch_state.borrow().touch.is_none() && touch_hitbox.is_hovered(window) {
                            let mut drag = touch_state.borrow_mut();
                            drag.begin(
                                position(event.position),
                                crate::value::as_f64(&touch_binding.c.read(touch_binding.field)),
                            );
                            drag.touch = Some(event.id);
                        }
                        // Observe the contact; do not claim a tap or a cross-axis scroll.
                        return;
                    }
                    if touch_state.borrow().touch != Some(event.id) {
                        return;
                    }
                    if touch_state.borrow().cancelled {
                        if matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                            let mut drag = touch_state.borrow_mut();
                            drag.touch = None;
                            drag.cancelled = false;
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if event.phase == TouchPhase::Cancelled {
                        let claimed = touch_state.borrow_mut().cancel();
                        touch_state.borrow_mut().touch = None;
                        touch_state.borrow_mut().cancelled = false;
                        if claimed {
                            touch_binding.active(false);
                        }
                        // Let GPUI send the native scroll cancellation, never a commit.
                        return;
                    }
                    let value = touch_state
                        .borrow_mut()
                        .change(position(event.position), touch_binding.props);
                    let ended = event.phase == TouchPhase::Ended;
                    let completed = if ended {
                        let completed = touch_state.borrow_mut().end();
                        touch_state.borrow_mut().touch = None;
                        completed
                    } else {
                        false
                    };
                    if let Some(value) = value {
                        window.prevent_default();
                        cx.stop_propagation();
                        // Some platforms coalesce all movement into the release.
                        // Lock Watch before the final draft write in that case too.
                        touch_binding.active(true);
                        touch_binding.preview(value);
                    }
                    if completed {
                        touch_binding.finish();
                    }
                });
                let (down, moving, up, escape) =
                    (state.clone(), state.clone(), state.clone(), state.clone());
                let (on_down, on_move, on_up, on_escape) = (
                    binding.clone(),
                    binding.clone(),
                    binding.clone(),
                    binding.clone(),
                );
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                        return;
                    }
                    if down.borrow_mut().end() {
                        on_down.active(false);
                    }
                    down.borrow_mut().cancelled = false;
                    if hitbox.is_hovered(window) {
                        down.borrow_mut().begin(
                            position(event.position),
                            crate::value::as_f64(&on_down.c.read(on_down.field)),
                        );
                    }
                });
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    if moving.borrow().touch.is_some() {
                        return;
                    }
                    if event.pressed_button != Some(MouseButton::Left) {
                        if moving.borrow_mut().cancel() {
                            on_move.active(false);
                        }
                        return;
                    }
                    let value = moving
                        .borrow_mut()
                        .change(position(event.position), on_move.props);
                    if let Some(value) = value {
                        cx.stop_propagation();
                        on_move.active(true);
                        on_move.preview(value);
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                        return;
                    }
                    if up.borrow().down.is_none() && !up.borrow().cancelled {
                        return;
                    }
                    // Escape, lost buttons and deactivation cancel the drag,
                    // not turn its eventual release into a descendant's click.
                    if up.borrow().cancelled {
                        up.borrow_mut().cancelled = false;
                        cx.stop_propagation();
                        return;
                    }
                    let value = up
                        .borrow_mut()
                        .change(position(event.position), on_up.props);
                    let claimed = up.borrow_mut().end();
                    if up.borrow().moved {
                        cx.stop_propagation();
                    }
                    if claimed {
                        cx.stop_propagation();
                        on_up.active(true);
                        if let Some(value) = value {
                            on_up.preview(value);
                        }
                        on_up.finish();
                    }
                });
                window.on_key_event(move |event: &KeyDownEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.keystroke.key == "escape" {
                        if escape.borrow_mut().cancel() {
                            on_escape.active(false);
                            cx.stop_propagation();
                        }
                    }
                });
            },
        )
        .absolute()
        .size_full();
        // Register capture before descendants: a claimed swipe consumes the
        // release before a nested Button can synthesize a click.
        let element = div()
            .id(self.key)
            .relative()
            .flex()
            .flex_col()
            .child(listeners)
            .children(self.content);
        grow.apply(element)
    }
}

/// A session-backed, axis-locked drag with an optional end-only action.
#[allow(clippy::too_many_arguments)]
pub fn swipe(
    c: &Ctx,
    id: &'static str,
    label: String,
    field: Field,
    props: SwipeProps,
    action: Option<Act>,
    commit: Option<Act>,
    content: Vec<El>,
) -> El {
    let binding = SwipeBinding {
        c: c.clone(),
        field,
        props,
        action,
        commit,
    };
    let set = binding.clone();
    let edit = binding.clone();
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "swipe",
        label,
        value: Some(crate::value::number(crate::value::as_f64(&c.read(field)))),
        enabled: c.enabled(),
        actions: Actions {
            set_number: Some(Rc::new(move |value| set.set_number(value))),
            edit: Some(Rc::new(move |text| {
                if let Ok(value) = text.parse() {
                    edit.set_number(value);
                }
            })),
            ..Actions::default()
        },
    });
    Swipe {
        key: c.element_id(id),
        binding,
        content,
    }
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn props() -> SwipeProps {
        SwipeProps {
            min: 0.0,
            max: 100.0,
            distance: 300.0,
            horizontal: false,
            continuous: false,
            active: None,
        }
    }
    #[test]
    fn tap_waits_but_double_never_fires_main() {
        let mut tap = TapState::default();
        tap.down((0.0, 0.0), 1);
        assert_eq!(tap.click(false, 1), TapClick::Wait);
        tap.down((0.0, 0.0), 2);
        assert!(!tap.expire(), "second down cancels the pending single");
        assert_eq!(tap.click(false, 2), TapClick::Double);
        assert!(!tap.expire());
        tap.down((0.0, 0.0), 3);
        assert_eq!(tap.click(false, 3), TapClick::Wait);
        assert!(tap.expire());
        assert!(!tap.expire());
    }
    #[test]
    fn drag_cancel_and_keyboard_do_not_wait() {
        let mut tap = TapState::default();
        tap.down((0.0, 0.0), 1);
        tap.motion((11.0, 0.0));
        tap.motion((0.0, 0.0));
        assert_eq!(tap.click(false, 1), TapClick::Ignore);
        assert_eq!(tap.click(true, 1), TapClick::Main);
        tap.down((0.0, 0.0), 1);
        assert_eq!(tap.click(false, 1), TapClick::Wait);
        tap.cancel();
        assert!(!tap.expire());
    }
    #[test]
    fn swipe_threshold_dominance_and_axis_signs() {
        let mut drag = SwipeState::default();
        drag.begin((0.0, 0.0), 50.0);
        assert_eq!(drag.change((0.0, -10.0), props()), None);
        assert_eq!(drag.change((20.0, -20.0), props()), None);
        assert_eq!(drag.change((40.0, -20.0), props()), None);
        assert_eq!(drag.change((0.0, -30.0), props()), Some(60.0));
        assert!(drag.claimed);
        assert_eq!(drag.change((900.0, -60.0), props()), Some(70.0));
        assert!(drag.end());
        assert!(!drag.end(), "cancel/end is idempotent");
        let horizontal = SwipeProps {
            horizontal: true,
            ..props()
        };
        drag.begin((0.0, 0.0), 50.0);
        assert_eq!(drag.change((30.0, 0.0), horizontal), Some(60.0));
        assert_eq!(drag.change((-30.0, 0.0), horizontal), Some(40.0));
    }
    #[test]
    fn swipe_locked_origin_clamps_and_cancels_without_commit() {
        let mut drag = SwipeState::default();
        drag.begin((100.0, 100.0), 30.0);
        assert_eq!(drag.change((100.0, -200.0), props()), Some(100.0));
        assert_eq!(drag.change((100.0, 400.0), props()), Some(0.0));
        assert_eq!(drag.change((100.0, 70.0), props()), Some(40.0));
        assert!(drag.end());
        assert_eq!(drag.change((100.0, 0.0), props()), None);
        drag.begin((0.0, 0.0), 70.0);
        assert!(!drag.end(), "an unrecognized touch never commits");
    }
    #[test]
    fn commit_policy_keeps_continuous_end_separate_from_direct_edits() {
        let calls = RefCell::new(Vec::new());
        commit_swipe(
            true,
            || calls.borrow_mut().push("action"),
            || calls.borrow_mut().push("commit"),
            || calls.borrow_mut().push("release"),
        );
        assert_eq!(*calls.borrow(), ["commit", "release"]);
        calls.borrow_mut().clear();
        commit_swipe(
            false,
            || calls.borrow_mut().push("action"),
            || calls.borrow_mut().push("commit"),
            || calls.borrow_mut().push("release"),
        );
        assert_eq!(*calls.borrow(), ["action", "commit", "release"]);
    }
    #[test]
    fn unmount_and_cancel_clear_active_exactly_once() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let mount = SwipeMount {
            state: Rc::new(RefCell::new(SwipeState::default())),
            active: Rc::new(move |active| observed.borrow_mut().push(active)),
            _activation: Subscription::new(|| {}),
        };
        mount.state.borrow_mut().begin((0.0, 0.0), 50.0);
        mount.state.borrow_mut().change((0.0, -30.0), props());
        mount.cancel();
        mount.cancel();
        assert_eq!(*events.borrow(), [false]);
        mount.state.borrow_mut().begin((0.0, 0.0), 50.0);
        mount.state.borrow_mut().change((0.0, -30.0), props());
        drop(mount);
        assert_eq!(*events.borrow(), [false, false]);
    }
    #[test]
    fn cancelled_and_cross_axis_drags_cannot_become_nested_clicks() {
        let mut drag = SwipeState::default();
        drag.begin((0.0, 0.0), 50.0);
        assert_eq!(drag.change((40.0, 1.0), props()), None);
        assert!(drag.moved);
        assert!(!drag.end());
        drag.begin((0.0, 0.0), 50.0);
        drag.change((0.0, -30.0), props());
        assert!(drag.cancel());
        assert!(drag.cancelled);
        assert_eq!(drag.change((0.0, 0.0), props()), None);
        drag.begin((0.0, 0.0), 50.0);
        assert!(!drag.cancelled);
        assert!(!drag.moved);
    }
}

#[cfg(all(test, feature = "test-support"))]
mod touch_dispatch_tests {
    use super::*;
    use gpui::{Context, InputEvent, Render, TestAppContext, WindowHandle, point, px};
    use std::cell::Cell;

    struct ButtonHarness {
        main: Rc<Cell<usize>>,
        double: Rc<Cell<usize>>,
    }
    impl Render for ButtonHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let main = self.main.clone();
            let double = self.double.clone();
            ButtonGesture {
                key: "tap-state".into(),
                element: div().id("tap-button").w(px(200.)).h(px(100.)),
                press: Rc::new(move |_| main.set(main.get() + 1)),
                double: Rc::new(move |_| double.set(double.get() + 1)),
            }
        }
    }
    fn contact(
        window: WindowHandle<ButtonHarness>,
        cx: &mut TestAppContext,
        id: u64,
        phase: TouchPhase,
        x: f32,
    ) {
        // Match the platform callback: do not hold the root entity's update
        // lease while native click listeners update that same entity.
        gpui::AnyWindowHandle::from(window)
            .update(cx, |_, window, cx| {
                window.dispatch_event(
                    TouchEvent {
                        id: TouchId(id),
                        phase,
                        position: point(px(x), px(20.)),
                        predicted_position: None,
                        force: None,
                    }
                    .to_platform_input(),
                    cx,
                );
            })
            .unwrap();
        cx.run_until_parked();
    }
    fn clock(cx: &mut TestAppContext, millis: u64) {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(millis));
        cx.run_until_parked();
    }
    #[gpui::test]
    fn raw_touch_button_waits_and_double_excludes_main(cx: &mut TestAppContext) {
        let main = Rc::new(Cell::new(0));
        let double = Rc::new(Cell::new(0));
        let window = cx.add_window({
            let (main, double) = (main.clone(), double.clone());
            move |_, _| ButtonHarness { main, double }
        });
        contact(window, cx, 1, TouchPhase::Started, 20.);
        contact(window, cx, 1, TouchPhase::Ended, 20.);
        assert_eq!(main.get(), 0);
        clock(cx, 100);
        contact(window, cx, 2, TouchPhase::Started, 20.);
        clock(cx, 100);
        contact(window, cx, 2, TouchPhase::Ended, 20.);
        clock(cx, 500);
        assert_eq!((main.get(), double.get()), (0, 1));
    }
    #[gpui::test]
    fn raw_touch_button_drag_preserves_native_stop_fling_tap(cx: &mut TestAppContext) {
        let main = Rc::new(Cell::new(0));
        let double = Rc::new(Cell::new(0));
        let window = cx.add_window({
            let (main, double) = (main.clone(), double.clone());
            move |_, _| ButtonHarness { main, double }
        });
        contact(window, cx, 1, TouchPhase::Started, 20.);
        contact(window, cx, 1, TouchPhase::Moved, 50.);
        contact(window, cx, 1, TouchPhase::Ended, 20.);
        clock(cx, 500);
        assert_eq!((main.get(), double.get()), (0, 0));
        // This harness contains only a Button, no Swipe recognizer. Its
        // unclaimed pan uses GPUI's normal fling; the next contact stops it.
        contact(window, cx, 2, TouchPhase::Started, 20.);
        contact(window, cx, 2, TouchPhase::Ended, 20.);
        clock(cx, 500);
        assert_eq!((main.get(), double.get()), (0, 0));
        contact(window, cx, 3, TouchPhase::Started, 20.);
        contact(window, cx, 3, TouchPhase::Ended, 20.);
        clock(cx, 500);
        assert_eq!((main.get(), double.get()), (1, 0));
    }
    #[gpui::test]
    fn raw_touch_single_button_fires_only_after_the_interval(cx: &mut TestAppContext) {
        let main = Rc::new(Cell::new(0));
        let double = Rc::new(Cell::new(0));
        let window = cx.add_window({
            let (main, double) = (main.clone(), double.clone());
            move |_, _| ButtonHarness { main, double }
        });
        contact(window, cx, 1, TouchPhase::Started, 20.);
        contact(window, cx, 1, TouchPhase::Ended, 20.);
        assert_eq!(main.get(), 0);
        clock(cx, 399);
        assert_eq!(main.get(), 0);
        clock(cx, 2);
        assert_eq!((main.get(), double.get()), (1, 0));
    }
}
