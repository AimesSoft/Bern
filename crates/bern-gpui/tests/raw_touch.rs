//! Synthetic raw contacts exercise GPUI's real dispatch, not mouse emulation.
#![cfg(feature = "test-support")]
use gpui::{
    Context, InputEvent, IntoElement, Pixels, Render, TestAppContext, TouchEvent, TouchId,
    TouchPhase, Window, canvas, point, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct RawTouchListener {
    raw: Rc<RefCell<Vec<(TouchPhase, Pixels)>>>,
    scroll: Rc<RefCell<Vec<TouchPhase>>>,
    clicks: Rc<Cell<usize>>,
    claimed: Rc<Cell<bool>>,
}
impl Render for RawTouchListener {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (raw, scroll, clicks, claimed) = (
            self.raw.clone(),
            self.scroll.clone(),
            self.clicks.clone(),
            self.claimed.clone(),
        );
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                window.on_mouse_event(move |event: &TouchEvent, phase, window, cx| {
                    if phase != gpui::DispatchPhase::Capture {
                        return;
                    }
                    raw.borrow_mut().push((event.phase, event.position.x));
                    if event.phase == TouchPhase::Started {
                        claimed.set(false);
                    }
                    if event.phase == TouchPhase::Moved && event.position.x > px(10.) {
                        claimed.set(true);
                    }
                    if claimed.get() {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                });
                window.on_mouse_event(move |event: &gpui::ScrollEvent, phase, _, _| {
                    if phase == gpui::DispatchPhase::Bubble {
                        scroll.borrow_mut().push(event.touch_phase);
                    }
                });
                window.on_mouse_event(move |_: &gpui::MouseUpEvent, phase, _, _| {
                    if phase == gpui::DispatchPhase::Bubble {
                        clicks.set(clicks.get() + 1);
                    }
                });
            },
        )
    }
}

#[gpui::test]
fn raw_touch_can_claim_after_slop_without_click_or_momentum(cx: &mut TestAppContext) {
    let raw = Rc::new(RefCell::new(Vec::new()));
    let scroll = Rc::new(RefCell::new(Vec::new()));
    let clicks = Rc::new(Cell::new(0));
    let window = cx.add_window({
        let (raw, scroll, clicks) = (raw.clone(), scroll.clone(), clicks.clone());
        move |_, _| RawTouchListener {
            raw,
            scroll,
            clicks,
            claimed: Rc::new(Cell::new(false)),
        }
    });
    for (phase, x) in [
        (TouchPhase::Started, 0.),
        (TouchPhase::Moved, 5.),
        (TouchPhase::Moved, 30.),
        (TouchPhase::Ended, 40.),
    ] {
        dispatch_touch(window, cx, TouchId(1), phase, x);
    }
    assert_eq!(raw.borrow().len(), 4);
    assert!(scroll.borrow().is_empty());
    assert_eq!(clicks.get(), 0);
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        scroll.borrow().is_empty(),
        "claimed touch produces no fling"
    );
    // Merely observing raw contacts preserves normal taps.
    dispatch_touch(window, cx, TouchId(2), TouchPhase::Started, 0.);
    dispatch_touch(window, cx, TouchId(2), TouchPhase::Ended, 0.);
    assert_eq!(clicks.get(), 1);
}

#[gpui::test]
fn raw_touch_claim_cancels_an_existing_native_scroll(cx: &mut TestAppContext) {
    let scroll = Rc::new(RefCell::new(Vec::new()));
    let window = cx.add_window({
        let scroll = scroll.clone();
        move |_, _| RawTouchListener {
            raw: Rc::new(RefCell::new(Vec::new())),
            scroll,
            clicks: Rc::new(Cell::new(0)),
            claimed: Rc::new(Cell::new(false)),
        }
    });
    dispatch_touch(window, cx, TouchId(1), TouchPhase::Started, 0.);
    dispatch_touch(window, cx, TouchId(1), TouchPhase::Moved, 9.);
    dispatch_touch(window, cx, TouchId(1), TouchPhase::Moved, 30.);
    dispatch_touch(window, cx, TouchId(1), TouchPhase::Cancelled, 30.);
    assert_eq!(
        *scroll.borrow(),
        [TouchPhase::Started, TouchPhase::Cancelled]
    );
}

fn dispatch_touch<T: 'static>(
    window: gpui::WindowHandle<T>,
    cx: &mut TestAppContext,
    id: TouchId,
    phase: TouchPhase,
    x: f32,
) {
    gpui::AnyWindowHandle::from(window)
        .update(cx, |_, window, cx| {
            window.dispatch_event(
                TouchEvent {
                    id,
                    phase,
                    position: point(px(x), px(0.)),
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
