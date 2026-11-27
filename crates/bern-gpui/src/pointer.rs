//! Pointer events an element declares beside its content: a double click
//! (`DoubleTap`), the menu a secondary click opens (`ContextMenu`), the
//! notches of a wheel or trackpad (`WheelUp`, `WheelDown`) and pointer
//! resting (`Hover`). A `Button` keeps its own double click and menu;
//! every other element takes them here, around what it shows.

use crate::El;
use crate::ctx::{Act, Ctx, Press};
use crate::menus::MenuEntry;
use crate::units::px;
use gpui::{
    App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, ScrollDelta, ScrollEvent, ScrollSource, Stateful, StatefulInteractiveElement,
    Styled, TouchPhase, Window, div,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

thread_local! {
    /// Generation tokens for active `Hover(delay:)` timers, keyed by element.
    static HOVER_GENERATIONS: RefCell<HashMap<String, u64>> = RefCell::new(HashMap::new());
}

/// The actions of an element's pointer events.
#[derive(Clone, Default)]
pub struct PointerEvents {
    pub double_tap: Option<Act>,
    pub wheel_up: Option<Act>,
    pub wheel_down: Option<Act>,
    pub hover: Option<(Act, f64)>,
}

/// Points of precise (trackpad) scrolling that count as one notch.
const NOTCH: f32 = 40.0;

/// `content` with its pointer events and the commands of its menu. The
/// wrapper takes the room `content` takes, so the element lays out as it
/// would without events; a single click still reaches the content.
pub fn pointer_events(
    c: &Ctx,
    id: &'static str,
    events: PointerEvents,
    menu: Option<Vec<MenuEntry>>,
    content: El,
) -> El {
    // Never wider than its container, as the columns and rows the content
    // may be: text in it still wraps.
    let wrapper = c.grow().apply(
        div()
            .id(ElementId::Name(format!("{}#pointer", c.key(id)).into()))
            .flex()
            .flex_col()
            .min_w(px(0.0))
            .max_w(gpui::relative(1.0))
            .child(content),
    );
    if !c.enabled() {
        return wrapper.into_any_element();
    }
    let wrapper = match events.double_tap {
        Some(action) => {
            let press = crate::controls::press(c, None, Some(action));
            // Use `on_click` so child buttons (which stop propagation on
            // `on_click`) keep their clicks instead of firing an enclosing
            // container's `DoubleTap`.
            wrapper.on_click(move |event: &ClickEvent, _, cx| {
                if let ClickEvent::Mouse(mouse) = event
                    && mouse.down.button == MouseButton::Left
                    && mouse.down.click_count == 2
                {
                    cx.stop_propagation();
                    press(cx);
                }
            })
        }
        None => wrapper,
    };
    let wrapper = match menu {
        Some(entries) => {
            let target = format!("{}#context", c.key(id));
            let opener = target.clone();
            let wrapper = wrapper.on_mouse_down(MouseButton::Right, move |event, _, cx| {
                cx.stop_propagation();
                crate::overlay::open_at(&opener, event.position);
            });
            if crate::overlay::is_open(&target) {
                let list = crate::menus::entry_list(c, &target, entries, false);
                wrapper.child(crate::overlay::panel(c, &target, list))
            } else {
                wrapper
            }
        }
        None => wrapper,
    };
    let wrapper = match events.hover {
        Some((action, delay)) => {
            let hover_key = format!("{}/{}#hover", c.win().id, c.key(id));
            let press = crate::controls::press(c, None, Some(action));
            wrapper.on_hover(move |inside, _window, cx| {
                let token = HOVER_GENERATIONS.with(|gens| {
                    let mut gens = gens.borrow_mut();
                    let entry = gens.entry(hover_key.clone()).or_insert(0);
                    *entry = entry.wrapping_add(1);
                    *entry
                });
                if !*inside {
                    return;
                }
                if delay <= 0.0 || !delay.is_finite() {
                    press(cx);
                    return;
                }
                let dur = Duration::from_secs_f64(delay);
                let hover_key = hover_key.clone();
                let press = Rc::clone(&press);
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(dur).await;
                    let still_valid = HOVER_GENERATIONS
                        .with(|gens| gens.borrow().get(&hover_key).copied() == Some(token));
                    if still_valid {
                        cx.update(|cx| press(cx));
                    }
                })
                .detach();
            })
        }
        None => wrapper,
    };
    if events.wheel_up.is_none() && events.wheel_down.is_none() {
        return wrapper.into_any_element();
    }
    let press =
        |action: Option<Act>| action.map(|action| crate::controls::press(c, None, Some(action)));
    Wheel {
        key: ElementId::Name(format!("{}#wheel", c.key(id)).into()),
        element: wrapper,
        up: press(events.wheel_up),
        down: press(events.wheel_down),
    }
    .into_any_element()
}

/// An element that takes the vertical scrolling over it as wheel notches.
#[derive(IntoElement)]
struct Wheel {
    key: ElementId,
    element: Stateful<Div>,
    up: Option<Press>,
    down: Option<Press>,
}

impl RenderOnce for Wheel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The part of a notch scrolled so far: up positive, down negative.
        let travel = window
            .use_keyed_state(self.key, cx, |_, _| Rc::new(Cell::new(0.0f32)))
            .read(cx)
            .clone();
        let (up, down) = (self.up, self.down);
        self.element.on_scroll(move |event: &ScrollEvent, _, cx| {
            // Control or Command with the wheel zooms; a finger's pan on a
            // touchscreen is no wheel.
            if event.source != ScrollSource::Pointer
                || event.modifiers.control
                || event.modifiers.platform
            {
                return;
            }
            let (across, along, notch) = match event.delta {
                ScrollDelta::Lines(delta) => (delta.x, delta.y, 1.0),
                ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y), NOTCH),
            };
            if event.touch_phase == TouchPhase::Started {
                travel.set(0.0);
            }
            if along == 0.0 || along.abs() < across.abs() {
                return;
            }
            // Content moving down shows what is above it: the wheel turns
            // up, as a scroll view would scroll toward its top.
            let press = if along > 0.0 { &up } else { &down };
            let Some(press) = press else {
                return;
            };
            cx.stop_propagation();
            let mut total = travel.get();
            if total * along < 0.0 {
                total = 0.0;
            }
            total += along / notch;
            while total.abs() >= 1.0 {
                total -= total.signum();
                press(cx);
            }
            travel.set(total);
        })
    }
}
