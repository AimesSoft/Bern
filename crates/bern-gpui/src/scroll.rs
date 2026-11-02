//! `ScrollView`: a scrolling container that can open at a point of its
//! content, come to rest on an item (`snap`), follow and set the item it
//! stands at (`position`, at a fraction of the view with `anchor`), and
//! tell when it is scrolled by hand (`browsing`). The items are the rows of
//! the first lists inside it; each notes where it was laid out, the view
//! where it is.

use crate::El;
use crate::ctx::{Ctx, Field};
use crate::protocol::Value;
use gpui::{
    Bounds, InteractiveElement, IntoElement, ParentElement, Pixels, Point, ScrollHandle,
    StatefulInteractiveElement, Styled, div, point,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Where scrolling comes to rest (`snap`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Snap {
    #[default]
    None,
    Start,
    Center,
}

/// How a `ScrollView` scrolls.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScrollProps {
    pub horizontal: bool,
    pub snap: Snap,
    /// The session field holding the key of the item the view stands at.
    pub position: Option<Field>,
    /// The point at this fraction of the content at the same fraction of
    /// the view, when the view opens.
    pub open_at: Option<f32>,
    /// The point at this fraction of the content in the middle of the
    /// view, when the view opens.
    pub open_center: Option<f32>,
    /// The item a changed `position` scrolls to nudges once there
    /// (`arrival: nudge`).
    pub arrival: bool,
    /// The item of `position` stands this fraction of the way along the
    /// view, item and view meeting at the same fraction (`anchor`).
    pub anchor: Option<f32>,
    /// The session field set while the view is scrolled by hand, until it
    /// rests (`browsing`).
    pub browsing: Option<Field>,
}

impl ScrollProps {
    /// Where along an item and the view they meet.
    fn fraction(&self) -> f32 {
        match self.snap {
            Snap::Center => 0.5,
            _ => self.anchor.unwrap_or(0.0),
        }
    }
}

#[derive(Default)]
struct State {
    handle: ScrollHandle,
    viewport: Option<Bounds<Pixels>>,
    /// Keys and where each item was last laid out.
    items: Vec<(Value, Bounds<Pixels>)>,
    /// Opened at `open_at` or `open_center`.
    opened: bool,
    /// The item to scroll to once it is laid out.
    pending: Option<Value>,
    /// The position last shown or written, so a change of it is a request.
    shown: Option<Value>,
    /// Counts scrolling, so only the last of a gesture's events snaps.
    scrolled: u64,
    /// Laid out before: a position set now is scrolled to smoothly.
    ready: bool,
    /// A changed position being scrolled to: from where, to where, how far,
    /// and its item.
    glide: Option<Glide>,
    /// Items nudge when a changed position has brought the view to them.
    arrival: bool,
    /// How many times the view has arrived at each item, by its key.
    arrivals: HashMap<String, i64>,
    /// Elements following scrolling, by key: where each was laid out, and
    /// the view's offset then.
    followers: HashMap<String, (Bounds<Pixels>, Point<Pixels>)>,
}

/// Scrolling smoothly to an item a position names.
struct Glide {
    from: Point<Pixels>,
    to: Point<Pixels>,
    progress: crate::motion::Spring,
    item: Value,
}

/// The spring a changed position scrolls with (`smooth`).
const GLIDE: crate::Curve = (0.45, 1.0);

thread_local! {
    static SCROLLS: RefCell<HashMap<Rc<str>, State>> = RefCell::new(HashMap::new());
}

/// The key the items of the scroll view `id` note themselves under.
pub fn scroll_key(c: &Ctx, id: &str) -> Rc<str> {
    format!("{}/{}", c.win().id, c.key(id)).into()
}

/// Where the element `key` following scrolling in the view `scroller` is
/// laid out.
pub(crate) fn follow(scroller: &Rc<str>, key: &str, bounds: Bounds<Pixels>) {
    SCROLLS.with(|scrolls| {
        if let Some(state) = scrolls.borrow_mut().get_mut(scroller) {
            let offset = state.handle.offset();
            state.followers.insert(key.to_string(), (bounds, offset));
        }
    });
}

/// How near the middle of its scroll view the element `key` is: 1 at the
/// middle, 0 half the view and half its own length away or more; by where
/// it was last laid out, moved as far as the view has scrolled since.
pub(crate) fn nearness((scroller, horizontal): &(Rc<str>, bool), key: &str) -> Option<f32> {
    SCROLLS.with(|scrolls| {
        let scrolls = scrolls.borrow();
        let state = scrolls.get(scroller)?;
        let view = state.viewport?;
        let (bounds, then) = *state.followers.get(key)?;
        let moved = state.handle.offset() - then;
        let length = |bounds: Bounds<Pixels>| {
            f32::from(if *horizontal {
                bounds.size.width
            } else {
                bounds.size.height
            })
        };
        let middle =
            |bounds: Bounds<Pixels>| along(bounds.origin, *horizontal) + length(bounds) / 2.0;
        let center = middle(bounds) + along(moved, *horizontal);
        let reach = (length(view) + length(bounds)) / 2.0;
        Some(if reach > 0.0 {
            1.0 - ((center - middle(view)).abs() / reach).min(1.0)
        } else {
            1.0
        })
    })
}

/// The elements of one row of a list: inside a scroll view that rests on
/// items or follows one, wrapped to note where they are laid out; and when
/// a changed position has brought the view to the row, nudged.
pub fn scroll_items(c: &Ctx, key: Value, build: impl FnOnce(&Ctx) -> Vec<El>, v: &mut Vec<El>) {
    let Some(list) = c.item_of() else {
        v.extend(build(c));
        return;
    };
    let arrived = SCROLLS.with(|scrolls| {
        let scrolls = scrolls.borrow();
        let state = scrolls.get(&list)?;
        state.arrival.then(|| {
            state
                .arrivals
                .get(&crate::value::key_text(&key))
                .copied()
                .unwrap_or(0)
        })
    });
    let Some(arrived) = arrived else {
        v.push(noted(list, key, build(c)));
        return;
    };
    let motion = crate::Motion {
        nudge_key: Some(Value::Int(arrived)),
        ..crate::Motion::NONE
    };
    v.push(crate::motion::moving(
        c,
        "arrival",
        motion,
        false,
        |c, _| noted(list, key, build(c)),
    ));
}

/// `items` in a box that notes where it is laid out, as the item `key` of
/// the view `list`.
fn noted(list: Rc<str>, key: Value, items: Vec<El>) -> El {
    let probe = gpui::canvas(
        move |bounds, _, _| {
            SCROLLS.with(|scrolls| {
                if let Some(state) = scrolls.borrow_mut().get_mut(&list) {
                    state.items.push((key.clone(), bounds));
                }
            })
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    div()
        .relative()
        .flex()
        .flex_col()
        .children(items)
        .child(probe)
        .into_any_element()
}

fn along(point: Point<Pixels>, horizontal: bool) -> f32 {
    f32::from(if horizontal { point.x } else { point.y })
}

/// The offset that brings `item` to the view, the point `fraction` of the
/// way along each at the same place.
fn offset_for(
    state: &State,
    item: Bounds<Pixels>,
    fraction: f32,
    horizontal: bool,
) -> Point<Pixels> {
    let Some(view) = state.viewport else {
        return state.handle.offset();
    };
    let start = |bounds: Bounds<Pixels>| along(bounds.origin, horizontal);
    let length = |bounds: Bounds<Pixels>| {
        f32::from(if horizontal {
            bounds.size.width
        } else {
            bounds.size.height
        })
    };
    let shift = (start(item) + length(item) * fraction) - (start(view) + length(view) * fraction);
    let offset = state.handle.offset();
    let max = state.handle.max_offset();
    let limit = -along(max, horizontal);
    let moved = (along(offset, horizontal) - shift).clamp(limit.min(0.0), 0.0);
    if horizontal {
        point(gpui::px(moved), offset.y)
    } else {
        point(offset.x, gpui::px(moved))
    }
}

/// The item nearest the point `fraction` of the way along the view.
fn nearest(state: &State, fraction: f32, horizontal: bool) -> Option<(Value, Bounds<Pixels>)> {
    let view = state.viewport?;
    let anchor = |bounds: Bounds<Pixels>| {
        let start = along(bounds.origin, horizontal);
        let length = f32::from(if horizontal {
            bounds.size.width
        } else {
            bounds.size.height
        });
        start + length * fraction
    };
    let at = anchor(view);
    state
        .items
        .iter()
        .min_by(|a, b| {
            (anchor(a.1) - at)
                .abs()
                .total_cmp(&(anchor(b.1) - at).abs())
        })
        .cloned()
}

/// `ScrollView`.
pub fn scroll_view(c: &Ctx, id: &'static str, props: ScrollProps, children: Vec<El>) -> El {
    let key = scroll_key(c, id);
    let horizontal = props.horizontal;
    let fraction = props.fraction();
    // A position set elsewhere is scrolled to once its item is laid out.
    let position = props.position.map(|field| c.read(field));
    let handle = SCROLLS.with(|scrolls| {
        let mut scrolls = scrolls.borrow_mut();
        let state = scrolls.entry(key.clone()).or_default();
        if !c.listing() {
            state.items.clear();
            state.arrival = props.arrival;
            if let Some(position) = &position
                && state.shown.as_ref() != Some(position)
            {
                state.pending = Some(position.clone());
                state.shown = Some(position.clone());
            }
        }
        state.handle.clone()
    });
    // The content keeps its own length along the axis: in a container that
    // scrolls, nothing shrinks to fit.
    let inner = div().flex().flex_none().children(children);
    let inner = if horizontal {
        inner.flex_row()
    } else {
        inner.flex_col().w_full()
    };
    let content = div()
        .id(c.element_id(id))
        .flex()
        .size_full()
        .track_scroll(&handle)
        .child(inner);
    let content = if horizontal {
        content.flex_row().overflow_x_scroll()
    } else {
        content.flex_col().overflow_y_scroll()
    };
    let resting = props.snap != Snap::None || props.position.is_some() || props.browsing.is_some();
    let content = if resting {
        let c = c.clone();
        let key = key.clone();
        content.on_scroll(move |_, window, cx| {
            // Wheel and trackpad events, momentum included, are the
            // person's; positions set by the view itself send none.
            if let Some(field) = props.browsing
                && !crate::value::truthy(&c.read(field))
            {
                c.write(field, Value::Bool(true));
            }
            let count = SCROLLS.with(|scrolls| {
                let mut scrolls = scrolls.borrow_mut();
                let state = scrolls.get_mut(&key)?;
                state.scrolled += 1;
                // Scrolling by hand ends scrolling to a position.
                state.glide = None;
                Some(state.scrolled)
            });
            let Some(count) = count else { return };
            let c = c.clone();
            let key = key.clone();
            let window = window.window_handle();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(160))
                    .await;
                cx.update(|cx| rest(&c, &key, count, props, window, cx));
            })
            .detach();
        })
    } else {
        content
    };
    // Where the view is, after its items: it opens at its point and goes
    // to a requested item.
    let probe = {
        let key = key.clone();
        gpui::canvas(
            move |bounds, window, _| {
                let moved = SCROLLS.with(|scrolls| {
                    let mut scrolls = scrolls.borrow_mut();
                    let state = scrolls.get_mut(&key)?;
                    state.viewport = Some(bounds);
                    if !state.opened {
                        let max = along(state.handle.max_offset(), horizontal);
                        let view = f32::from(if horizontal {
                            bounds.size.width
                        } else {
                            bounds.size.height
                        });
                        let fraction_at = |content: f32| {
                            props
                                .open_center
                                .map(|fraction| fraction * content - view / 2.0)
                                .or(props.open_at.map(|fraction| fraction * (content - view)))
                        };
                        state.opened = true;
                        if let Some(target) = fraction_at(max + view) {
                            let moved = -target.clamp(0.0, max.max(0.0));
                            let offset = state.handle.offset();
                            state.handle.set_offset(if horizontal {
                                point(gpui::px(moved), offset.y)
                            } else {
                                point(offset.x, gpui::px(moved))
                            });
                            return Some(());
                        }
                    }
                    let ready = std::mem::replace(&mut state.ready, true);
                    let now = crate::motion::now();
                    if let Some(glide) = &mut state.glide {
                        glide.progress.advance(GLIDE, now);
                        let done = glide.progress.settle(1.0);
                        let p = glide.progress.value;
                        let at = glide.from + (glide.to - glide.from) * p;
                        state.handle.set_offset(at);
                        if done {
                            let item = crate::value::key_text(&glide.item);
                            crate::motion::log(|| format!("arrived {key} at {item}"));
                            state.glide = None;
                            *state.arrivals.entry(item).or_insert(0) += 1;
                        }
                        // The content is built again for the next frame:
                        // on the way, and to nudge the item on arrival.
                        window.request_animation_frame();
                        return Some(());
                    }
                    let pending = state.pending.clone()?;
                    let Some(item) = state
                        .items
                        .iter()
                        .find(|(item, _)| *item == pending)
                        .map(|(_, bounds)| *bounds)
                    else {
                        // A row of a grid that builds only the rows near
                        // the view: near where it would be first, then to
                        // where it is once it is built there.
                        let near = crate::layout::lazy_reach(&key, &pending)?;
                        let offset = offset_for(state, near, fraction, horizontal);
                        state.handle.set_offset(offset);
                        window.request_animation_frame();
                        return Some(());
                    };
                    state.pending = None;
                    let offset = offset_for(state, item, fraction, horizontal);
                    // A position changed after the view was laid out is
                    // scrolled to smoothly; the first one at once.
                    if ready && !crate::native::reduce_motion() {
                        state.glide = Some(Glide {
                            from: state.handle.offset(),
                            to: offset,
                            progress: crate::motion::Spring::toward(0.0, 1.0, now),
                            item: pending,
                        });
                        window.request_animation_frame();
                    } else {
                        state.handle.set_offset(offset);
                        if ready {
                            *state
                                .arrivals
                                .entry(crate::value::key_text(&pending))
                                .or_insert(0) += 1;
                            window.request_animation_frame();
                        }
                    }
                    Some(())
                });
                if moved.is_some() {
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    };
    let element = div()
        .relative()
        .flex()
        .min_w(gpui::px(0.0))
        .min_h(gpui::px(0.0))
        .child(content)
        .child(probe);
    c.faded(c.grow().apply(element)).into_any_element()
}

/// After scrolling stops: no longer browsing, come to rest on the nearest
/// item, and note it as the position.
fn rest(
    c: &Ctx,
    key: &Rc<str>,
    count: u64,
    props: ScrollProps,
    window: gpui::AnyWindowHandle,
    cx: &mut gpui::App,
) {
    let fraction = props.fraction();
    let last = SCROLLS.with(|scrolls| {
        scrolls
            .borrow()
            .get(key)
            .is_some_and(|state| state.scrolled == count)
    });
    if !last {
        return;
    }
    if let Some(field) = props.browsing
        && crate::value::truthy(&c.read(field))
    {
        c.write(field, Value::Bool(false));
    }
    let found = SCROLLS.with(|scrolls| {
        let mut scrolls = scrolls.borrow_mut();
        let state = scrolls.get_mut(key)?;
        if props.position.is_none() && props.snap == Snap::None {
            return None;
        }
        let (item, bounds) = nearest(state, fraction, props.horizontal)?;
        if props.snap != Snap::None {
            let offset = offset_for(state, bounds, fraction, props.horizontal);
            state.handle.set_offset(offset);
        }
        state.shown = Some(item.clone());
        Some(item)
    });
    let Some(item) = found else { return };
    if let Some(field) = props.position
        && c.read(field) != item
    {
        c.write(field, item);
    }
    let _ = window.update(cx, |_, window, _| window.refresh());
}
