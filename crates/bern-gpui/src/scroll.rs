//! `ScrollView`: a scrolling container that can open at a point of its
//! content, come to rest on an item (`snap`), and follow and set the item
//! it stands at (`position`). The items are the rows of the first lists
//! inside it; each notes where it was laid out, the view where it is.

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
}

thread_local! {
    static SCROLLS: RefCell<HashMap<Rc<str>, State>> = RefCell::new(HashMap::new());
}

/// The key the items of the scroll view `id` note themselves under.
pub fn scroll_key(c: &Ctx, id: &str) -> Rc<str> {
    format!("{}/{}", c.win().id, c.target(id)).into()
}

/// The elements of one row of a list: inside a scroll view that rests on
/// items or follows one, wrapped to note where they are laid out.
pub fn scroll_items(c: &Ctx, key: Value, items: Vec<El>, v: &mut Vec<El>) {
    let Some(list) = c.item_of() else {
        v.extend(items);
        return;
    };
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
    v.push(
        div()
            .relative()
            .flex()
            .flex_col()
            .children(items)
            .child(probe)
            .into_any_element(),
    );
}

fn along(point: Point<Pixels>, horizontal: bool) -> f32 {
    f32::from(if horizontal { point.x } else { point.y })
}

/// The offset that brings `item` to the start or the middle of the view.
fn offset_for(
    state: &State,
    item: Bounds<Pixels>,
    center: bool,
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
    let shift = if center {
        (start(item) + length(item) / 2.0) - (start(view) + length(view) / 2.0)
    } else {
        start(item) - start(view)
    };
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

/// The item nearest the start or the middle of the view.
fn nearest(state: &State, center: bool, horizontal: bool) -> Option<(Value, Bounds<Pixels>)> {
    let view = state.viewport?;
    let anchor = |bounds: Bounds<Pixels>| {
        let start = along(bounds.origin, horizontal);
        let length = f32::from(if horizontal {
            bounds.size.width
        } else {
            bounds.size.height
        });
        if center { start + length / 2.0 } else { start }
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
    let center = props.snap == Snap::Center;
    // A position set elsewhere is scrolled to once its item is laid out.
    let position = props.position.map(|field| c.read(field));
    let handle = SCROLLS.with(|scrolls| {
        let mut scrolls = scrolls.borrow_mut();
        let state = scrolls.entry(key.clone()).or_default();
        if !c.listing() {
            state.items.clear();
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
    let resting = props.snap != Snap::None || props.position.is_some();
    let content = if resting {
        let c = c.clone();
        let key = key.clone();
        content.on_scroll(move |_, window, cx| {
            let count = SCROLLS.with(|scrolls| {
                let mut scrolls = scrolls.borrow_mut();
                let state = scrolls.get_mut(&key)?;
                state.scrolled += 1;
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
                    let pending = state.pending.clone()?;
                    let item = state.items.iter().find(|(item, _)| *item == pending)?.1;
                    state.pending = None;
                    let offset = offset_for(state, item, center, horizontal);
                    state.handle.set_offset(offset);
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
    c.grow().apply(element).into_any_element()
}

/// After scrolling stops: come to rest on the nearest item, and note it as
/// the position.
fn rest(
    c: &Ctx,
    key: &Rc<str>,
    count: u64,
    props: ScrollProps,
    window: gpui::AnyWindowHandle,
    cx: &mut gpui::App,
) {
    let center = props.snap == Snap::Center;
    let found = SCROLLS.with(|scrolls| {
        let mut scrolls = scrolls.borrow_mut();
        let state = scrolls.get_mut(key)?;
        if state.scrolled != count {
            return None;
        }
        let (item, bounds) = nearest(state, center, props.horizontal)?;
        if props.snap != Snap::None {
            let offset = offset_for(state, bounds, center, props.horizontal);
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
