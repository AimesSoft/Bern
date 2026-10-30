//! The result of an asynchronous query. Each `Query` a window shows holds
//! a subscription of that window instance; parameters go to the engine
//! only when they change, only the current answer is shown, and a
//! subscription ends when its window draws without it.

use crate::ctx::Ctx;
use crate::host::{host, status, take};
use crate::protocol::{Reader, Type, Value, Writer, decode, encode};
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons, text};
use gpui::{
    FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The texts and icon of a query's states.
#[derive(Default)]
pub struct QueryProps {
    pub loading: Option<String>,
    pub failed: Option<String>,
    pub empty: Option<String>,
    pub icon: Option<&'static str>,
}

#[derive(Clone, Default, PartialEq)]
struct State {
    version: u64,
    /// 0 idle, 1 loading, 2 ready, 3 refreshing, 4 failed.
    phase: u8,
    data: Option<Value>,
    error: Option<(String, String)>,
}

struct Subscription {
    id: u64,
    query: usize,
    /// The window it belongs to.
    window: String,
    sent: Option<Vec<u8>>,
    state: State,
    /// Drawn in the window's latest frame.
    seen: bool,
}

thread_local! {
    /// Subscriptions by window and target.
    static SUBSCRIPTIONS: RefCell<HashMap<String, Subscription>> = RefCell::new(HashMap::new());
}

fn read_state(bytes: &[u8], result: &Type) -> Option<State> {
    let types = &host().def.descriptor.types;
    let mut reader = Reader::new(bytes);
    let version = reader.u64().ok()?;
    let phase = reader.u8().ok()?;
    let mut data = None;
    let mut error = None;
    if phase == 2 || phase == 3 {
        data = Some(decode(result, types, &mut reader).ok()?);
    }
    if phase == 4 {
        if reader.u8().ok()? == 1 {
            data = Some(decode(result, types, &mut reader).ok()?);
        }
        error = Some((reader.text().ok()?, reader.text().ok()?));
    }
    Some(State {
        version,
        phase,
        data,
        error,
    })
}

/// What the template shows of the data: `None` when the result is empty
/// and the declaration has an empty state.
fn shown(data: &Value, result: &Type, has_empty: bool) -> Option<Value> {
    match (data, result) {
        (Value::List(items), Type::List { .. }) if has_empty && items.is_empty() => None,
        (Value::Optional(None), _) => None,
        (Value::Optional(Some(inner)), _) => Some((**inner).clone()),
        (data, _) => Some(data.clone()),
    }
}

fn count(data: &Value) -> Option<usize> {
    match data {
        Value::List(items) => Some(items.len()),
        Value::Optional(value) => Some(value.is_some() as usize),
        _ => None,
    }
}

/// Take a state for subscription `id` unless an answer to a later request
/// is already shown.
fn store(key: &str, bytes: &[u8]) -> bool {
    SUBSCRIPTIONS.with(|subscriptions| {
        let mut subscriptions = subscriptions.borrow_mut();
        let Some(subscription) = subscriptions.get_mut(key) else {
            return false;
        };
        let result = &host().def.descriptor.commands[subscription.query].result;
        match read_state(bytes, result) {
            Some(next)
                if next.version >= subscription.state.version && next != subscription.state =>
            {
                subscription.state = next;
                true
            }
            _ => false,
        }
    })
}

/// An answer from the engine; true when a window has to draw again.
pub(crate) fn receive(id: u64, bytes: &[u8]) -> bool {
    let key = SUBSCRIPTIONS.with(|subscriptions| {
        subscriptions
            .borrow()
            .iter()
            .find(|(_, subscription)| subscription.id == id)
            .map(|(key, _)| key.clone())
    });
    key.is_some_and(|key| store(&key, bytes))
}

/// Send parameters to subscription `key`; the reply is its state now.
fn request(key: &str, bytes: Vec<u8>) {
    let Some(id) = SUBSCRIPTIONS.with(|subscriptions| {
        let mut subscriptions = subscriptions.borrow_mut();
        let subscription = subscriptions.get_mut(key)?;
        subscription.sent = Some(bytes.clone());
        Some(subscription.id)
    }) else {
        return;
    };
    // SAFETY: the handle is live while the application runs; the bytes
    // are valid for this call.
    let reply = take(unsafe {
        bern_ffi::query::bern_query_request(host().app(), id, bytes.as_ptr(), bytes.len())
    });
    if status(&reply).is_ok() {
        store(key, &reply[1..]);
    }
}

/// Mark the subscriptions of `window` as not yet drawn in this frame.
pub(crate) fn begin(window: &str) {
    SUBSCRIPTIONS.with(|subscriptions| {
        for subscription in subscriptions.borrow_mut().values_mut() {
            if subscription.window == window {
                subscription.seen = false;
            }
        }
    });
}

/// End the subscriptions of `window` its frame did not draw; all of them
/// with `closed`.
pub(crate) fn end(window: &str, closed: bool) {
    let ended: Vec<u64> = SUBSCRIPTIONS.with(|subscriptions| {
        let mut ended = Vec::new();
        subscriptions.borrow_mut().retain(|_, subscription| {
            let keep = subscription.window != window || (subscription.seen && !closed);
            if !keep {
                ended.push(subscription.id);
            }
            keep
        });
        ended
    });
    for id in ended {
        // SAFETY: the handle is live while the application runs.
        unsafe { bern_ffi::query::bern_query_close(host().app(), id) };
    }
}

/// `Query`: its template over the query's data, or what the query is
/// doing while there is none to show.
pub fn query(
    c: &Ctx,
    id: &'static str,
    query: usize,
    params: Vec<Value>,
    props: QueryProps,
    template: impl FnOnce(&Ctx, Value) -> Vec<El>,
) -> El {
    let host = host();
    let Some(spec) = host.def.descriptor.commands.get(query) else {
        return div().into_any_element();
    };
    let target = c.target(id);
    let window = c.win().id.clone();
    let key = format!("{window}/{target}");
    let opened = SUBSCRIPTIONS.with(|subscriptions| subscriptions.borrow().contains_key(&key));
    // Looking for controls opens nothing.
    if !opened && c.listing() {
        return div().into_any_element();
    }
    if !opened {
        let win = c.win();
        // SAFETY: the handle is live while the application runs.
        let reply = take(unsafe {
            bern_ffi::query::bern_query_open(
                host.app(),
                win.kind as u32,
                win.generation,
                query as u32,
            )
        });
        let (id, state) = match status(&reply) {
            Ok(mut reader) => (reader.u64().unwrap_or(0), State::default()),
            Err(failure) => (
                0,
                State {
                    phase: 4,
                    error: Some((failure.code, failure.message)),
                    ..State::default()
                },
            ),
        };
        SUBSCRIPTIONS.with(|subscriptions| {
            subscriptions.borrow_mut().insert(
                key.clone(),
                Subscription {
                    id,
                    query,
                    window: window.to_string(),
                    sent: None,
                    state,
                    seen: true,
                },
            )
        });
    }
    let types = &host.def.descriptor.types;
    let mut writer = Writer::new();
    for (value, param) in params.iter().zip(&spec.params) {
        encode(value, &param.ty, types, &mut writer);
    }
    let bytes = writer.bytes;
    let (fresh, live) = SUBSCRIPTIONS.with(|subscriptions| {
        let mut subscriptions = subscriptions.borrow_mut();
        let subscription = subscriptions.get_mut(&key).expect("opened above");
        subscription.seen = true;
        (
            subscription.sent.as_ref() != Some(&bytes),
            subscription.id != 0,
        )
    });
    if fresh && live && !c.listing() {
        request(&key, bytes.clone());
    }
    let state = SUBSCRIPTIONS
        .with(|subscriptions| {
            subscriptions
                .borrow()
                .get(&key)
                .map(|subscription| subscription.state.clone())
        })
        .unwrap_or_default();

    let retry: crate::ctx::Press = {
        let key = key.clone();
        Rc::new(move |_| request(&key, bytes.clone()))
    };
    let name = ["idle", "loading", "ready", "refreshing", "failed"][state.phase.min(4) as usize];
    c.list(Control {
        target,
        window,
        role: "query",
        label: String::new(),
        value: Some(match state.data.as_ref().and_then(count) {
            Some(n) => format!("{name} v{} ({n})", state.version),
            None => format!("{name} v{}", state.version),
        }),
        enabled: true,
        actions: Actions {
            activate: Some(Rc::clone(&retry)),
            ..Actions::default()
        },
    });

    let has_empty = props.empty.is_some();
    let visible = state
        .data
        .as_ref()
        .and_then(|data| shown(data, &spec.result, has_empty));
    let loading = props
        .loading
        .unwrap_or_else(|| text::tr("bern.query.loading"));
    let failed = props
        .failed
        .unwrap_or_else(|| text::tr("bern.query.failed"));
    let secondary = c.color("text.secondary");
    let retry_button = |c: &Ctx| {
        let retry = Rc::clone(&retry);
        let hover = c.slot("button.standard", "hover", "background");
        div()
            .id(c.element_id("retry"))
            .flex_none()
            .px(px(12.0))
            .h(px(30.0))
            .flex()
            .items_center()
            .rounded(px(8.0))
            .bg(c.slot("button.standard", "rest", "background"))
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .hover(move |style| style.bg(hover))
            .child(text::tr("bern.query.retry"))
            .on_click(move |_, _, cx| retry(cx))
    };
    let line = |content: Vec<El>| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .py(px(18.0))
            .text_size(px(13.0))
            .text_color(secondary)
            .children(content)
            .into_any_element()
    };
    let status = if state.phase == 1 || (state.phase == 0 && state.data.is_none()) {
        Some(line(vec![div().child(loading.clone()).into_any_element()]))
    } else if state.phase == 4 && state.data.is_none() {
        Some(line(vec![
            icons::icon("exclamationmark.triangle", 16.0, c.color("status.warning")),
            div().child(failed.clone()).into_any_element(),
            retry_button(c).into_any_element(),
        ]))
    } else if visible.is_none() && has_empty {
        Some(line(vec![
            icons::icon(props.icon.unwrap_or("tray"), 16.0, secondary),
            div()
                .child(props.empty.clone().unwrap_or_default())
                .into_any_element(),
        ]))
    } else {
        None
    };
    if let Some(status) = status {
        return c
            .grow()
            .apply(div().flex().flex_col().child(status))
            .into_any_element();
    }
    // Data, with what the query is doing again above it.
    let note = match state.phase {
        3 => Some(line(vec![div().child(loading).into_any_element()])),
        4 => Some(line(vec![
            icons::icon("exclamationmark.triangle", 16.0, c.color("status.warning")),
            div().child(failed).into_any_element(),
            retry_button(c).into_any_element(),
        ])),
        _ => None,
    };
    let content = template(c, visible.unwrap_or(Value::Unit));
    c.grow()
        .apply(div().flex().flex_col().children(note).children(content))
        .into_any_element()
}
