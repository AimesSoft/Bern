//! The result of an asynchronous query. The subscription belongs to the
//! window instance and ends with this view; parameters are sent again
//! only when they change, and only the current answer is shown.

use crate::controls::{self, Actions};
use crate::host::{host, status, take, with_bytes};
use crate::protocol::{Reader, Type, Value, Writer, decode, encode};
use crate::reactive::{Cell, effect, on_cleanup, untrack};
use crate::text::tr;
use crate::widgets::plain_row;
use crate::{Branch, Build, BytesFn, Ctx, Get, Parent, Str, icons};
use adw::prelude::*;
use std::rc::Rc;

#[derive(Default)]
pub struct QueryProps {
    pub id: Option<&'static str>,
    /// Catalog index of the query.
    pub query: usize,
    pub params: Vec<Get>,
    pub loading: Option<Str>,
    pub failed: Option<Str>,
    pub empty: Option<Str>,
    pub icon: Option<&'static str>,
    /// The template variable.
    pub var: &'static str,
}

#[derive(Clone, PartialEq)]
struct State {
    version: u64,
    /// 0 idle, 1 loading, 2 ready, 3 refreshing, 4 failed.
    phase: u8,
    data: Option<Value>,
    error: Option<(String, String)>,
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

fn spinner() -> gtk::Widget {
    let spinner = gtk::Spinner::new();
    spinner.start();
    spinner.upcast()
}

#[allow(non_snake_case)]
pub fn Query(p: &Parent, ctx: &Ctx, props: QueryProps, template: Build) {
    let host = host();
    let Some(spec) = host.def.descriptor.commands.get(props.query) else {
        return;
    };
    let result = spec.result.clone();
    let param_types: Vec<Type> = spec.params.iter().map(|param| param.ty.clone()).collect();
    let has_empty = props.empty.is_some();
    let state = Cell::new(State {
        version: 0,
        phase: 0,
        data: None,
        error: None,
    });
    let receive: BytesFn = {
        let state = state.clone();
        let result = result.clone();
        Rc::new(move |bytes| {
            if let Some(next) = read_state(bytes, &result)
                && next.version >= state.peek().version
            {
                state.set(next);
            }
        })
    };
    let win = ctx.win();
    // SAFETY: the handle is live while the application runs.
    let opened = take(unsafe {
        bern_ffi::query::bern_query_open(
            host.app(),
            win.kind as u32,
            win.generation,
            props.query as u32,
        )
    });
    let subscription = match status(&opened) {
        Ok(mut reader) => reader.u64().ok(),
        Err(failure) => {
            state.set(State {
                version: 0,
                phase: 4,
                data: None,
                error: Some((failure.code, failure.message)),
            });
            None
        }
    };
    if let Some(subscription) = subscription {
        host.open_query(subscription, Rc::clone(&receive));
        on_cleanup(move || {
            let host = crate::host::host();
            host.close_query(subscription);
            // SAFETY: the handle is live while the application runs.
            unsafe { bern_ffi::query::bern_query_close(host.app(), subscription) };
        });
    }
    let request: BytesFn = {
        let receive = Rc::clone(&receive);
        Rc::new(move |bytes| {
            let Some(subscription) = subscription else {
                return;
            };
            let reply = with_bytes(bytes, |ptr, len| {
                // SAFETY: the handle is live; the bytes are valid for this call.
                take(unsafe {
                    bern_ffi::query::bern_query_request(
                        crate::host::host().app(),
                        subscription,
                        ptr,
                        len,
                    )
                })
            });
            if status(&reply).is_ok() {
                receive(&reply[1..]);
            }
        })
    };
    let params: Rc<dyn Fn() -> Vec<u8>> = {
        let at = ctx.clone();
        let args = props.params.clone();
        Rc::new(move || {
            let types = &crate::host::host().def.descriptor.types;
            let mut writer = Writer::new();
            for (arg, ty) in args.iter().zip(&param_types) {
                encode(&arg(&at), ty, types, &mut writer);
            }
            writer.bytes
        })
    };
    let sent: Rc<std::cell::RefCell<Option<Vec<u8>>>> = Rc::new(std::cell::RefCell::new(None));
    {
        let params = Rc::clone(&params);
        let sent = Rc::clone(&sent);
        let request = Rc::clone(&request);
        effect(move || {
            let bytes = params();
            if sent.borrow().as_ref() == Some(&bytes) {
                return;
            }
            *sent.borrow_mut() = Some(bytes.clone());
            untrack(|| request(&bytes));
        });
    }
    let retry: Rc<dyn Fn()> = {
        let params = Rc::clone(&params);
        Rc::new(move || {
            let bytes = untrack(|| params());
            *sent.borrow_mut() = Some(bytes.clone());
            request(&bytes);
        })
    };

    // Which status to show, and whether the template shows data.
    let key: Rc<dyn Fn() -> &'static str> = {
        let state = state.clone();
        let result = result.clone();
        Rc::new(move || {
            let current = state.get();
            let visible = current
                .data
                .as_ref()
                .and_then(|data| shown(data, &result, has_empty));
            if current.phase == 1 || (current.phase == 0 && current.data.is_none()) {
                "loading"
            } else if current.phase == 4 && current.data.is_none() {
                "failed"
            } else if visible.is_none() && has_empty {
                "empty"
            } else {
                "data"
            }
        })
    };
    let text_of = |text: &Option<Str>, fallback: &'static str| -> Rc<dyn Fn() -> String> {
        match text {
            Some(text) => {
                let at = ctx.clone();
                let text = Rc::clone(text);
                Rc::new(move || text(&at))
            }
            None => Rc::new(move || tr(fallback)),
        }
    };
    let loading = text_of(&props.loading, "bern.query.loading");
    let failed = text_of(&props.failed, "bern.query.failed");
    let empty = props.empty.as_ref().map(|_| text_of(&props.empty, ""));
    let empty_icon = props.icon.unwrap_or("tray");
    let in_list = p.is_list();
    let wrap = move |widget: gtk::Widget| -> gtk::Widget {
        if in_list {
            plain_row(&widget).upcast()
        } else {
            widget
        }
    };
    let bound_label = |text: Rc<dyn Fn() -> String>| {
        let label = gtk::Label::new(None);
        label.set_wrap(true);
        let shown = label.clone();
        effect(move || shown.set_label(&text()));
        label
    };
    let retry_button = |retry: Rc<dyn Fn()>| {
        let button = gtk::Button::new();
        let shown = button.clone();
        effect(move || shown.set_label(&tr("bern.query.retry")));
        button.connect_clicked(move |_| retry());
        button
    };
    let status_branch: Branch = {
        let key = Rc::clone(&key);
        let state = state.clone();
        Rc::new(move |_| {
            let phase = state.get().phase;
            match (key(), phase) {
                ("loading", _) => Some(0),
                ("failed", _) => Some(1),
                ("empty", _) => Some(2),
                ("data", 3) => Some(3),
                ("data", 4) => Some(4),
                _ => None,
            }
        })
    };
    let arms: Vec<Build> = vec![
        {
            let loading = Rc::clone(&loading);
            Rc::new(move |p: &Parent, _: &Ctx| {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                line.set_halign(gtk::Align::Center);
                line.set_margin_top(18);
                line.set_margin_bottom(18);
                line.append(&spinner());
                let label = bound_label(Rc::clone(&loading));
                label.add_css_class("dim-label");
                line.append(&label);
                p.add(&wrap(line.upcast()));
            })
        },
        {
            let failed = Rc::clone(&failed);
            let retry = Rc::clone(&retry);
            Rc::new(move |p: &Parent, _: &Ctx| {
                let page = adw::StatusPage::new();
                page.set_icon_name(Some("dialog-warning-symbolic"));
                page.add_css_class("compact");
                let shown = page.clone();
                let failed = Rc::clone(&failed);
                effect(move || shown.set_title(&failed()));
                page.set_child(Some(&retry_button(Rc::clone(&retry))));
                p.add(&wrap(page.upcast()));
            })
        },
        {
            let empty = empty.clone();
            Rc::new(move |p: &Parent, _: &Ctx| {
                let page = adw::StatusPage::new();
                page.set_icon_name(Some(icons::name(empty_icon)));
                page.add_css_class("compact");
                if let Some(empty) = empty.clone() {
                    let shown = page.clone();
                    effect(move || shown.set_title(&empty()));
                }
                p.add(&wrap(page.upcast()));
            })
        },
        {
            let loading = Rc::clone(&loading);
            Rc::new(move |p: &Parent, _: &Ctx| {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                line.append(&spinner());
                let label = bound_label(Rc::clone(&loading));
                label.add_css_class("dim-label");
                line.append(&label);
                p.add(&wrap(line.upcast()));
            })
        },
        {
            let failed = Rc::clone(&failed);
            let retry = Rc::clone(&retry);
            Rc::new(move |p: &Parent, _: &Ctx| {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
                icon.add_css_class("warning");
                line.append(&icon);
                let label = bound_label(Rc::clone(&failed));
                label.set_hexpand(true);
                label.set_xalign(0.0);
                line.append(&label);
                line.append(&retry_button(Rc::clone(&retry)));
                p.add(&wrap(line.upcast()));
            })
        },
    ];
    crate::When(p, ctx, status_branch, arms);

    let data_key = Rc::clone(&key);
    let data_state = state.clone();
    let data_result = result.clone();
    let inner = ctx.with_local(
        props.var,
        Rc::new(move || {
            data_state
                .get()
                .data
                .and_then(|data| shown(&data, &data_result, has_empty))
                .unwrap_or(Value::Unit)
        }),
    );
    crate::When(
        p,
        ctx,
        Rc::new(move |_| (data_key() == "data").then_some(0)),
        vec![Rc::new(move |p: &Parent, _: &Ctx| template(p, &inner))],
    );

    if let Some(id) = props.id {
        let read = state.clone();
        controls::register(
            ctx,
            id,
            "query",
            Some(Rc::new(String::new)),
            None,
            Actions {
                activate: Some(retry),
                value: Some(Rc::new(move || {
                    let current = read.peek();
                    let name = ["idle", "loading", "ready", "refreshing", "failed"]
                        [current.phase.min(4) as usize];
                    Some(match current.data.as_ref().and_then(count) {
                        Some(n) => format!("{name} v{} ({n})", current.version),
                        None => format!("{name} v{}", current.version),
                    })
                })),
                ..Actions::default()
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::tests::{app, start};
    use crate::protocol::TypeTable;

    /// A query state: its version and phase, then what `rest` writes.
    fn answer(version: u64, phase: u8, rest: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.u64(version);
        writer.u8(phase);
        rest(&mut writer);
        writer.bytes
    }

    #[test]
    fn query_states_carry_data_by_phase() {
        start(app());
        let list = Type::list(Type::Int);
        let items = Value::List(vec![Value::Int(4), Value::Int(2)]);
        let write_items = |writer: &mut Writer| {
            encode(&items, &list, &TypeTable::default(), writer);
        };
        let loading = read_state(&answer(1, 1, |_| {}), &list).expect("loading");
        assert_eq!(
            (loading.version, loading.phase, loading.data, loading.error),
            (1, 1, None, None)
        );
        let ready = read_state(&answer(2, 2, write_items), &list).expect("ready");
        assert_eq!((ready.phase, ready.data), (2, Some(items.clone())));
        let refreshing = read_state(&answer(3, 3, write_items), &list).expect("refreshing");
        assert_eq!(refreshing.data, Some(items.clone()));
        // A failure keeps the data it had, if any.
        let failed = read_state(
            &answer(4, 4, |writer| {
                writer.u8(1);
                write_items(writer);
                writer.text("timeout");
                writer.text("The server did not answer");
            }),
            &list,
        )
        .expect("failed");
        assert_eq!(
            (failed.data, failed.error),
            (
                Some(items.clone()),
                Some(("timeout".into(), "The server did not answer".into()))
            )
        );
        let offline = read_state(
            &answer(5, 4, |writer| {
                writer.u8(0);
                writer.text("offline");
                writer.text("No network");
            }),
            &list,
        )
        .expect("failed");
        assert_eq!(offline.data, None);
    }

    #[test]
    fn truncated_query_states_are_dropped() {
        start(app());
        let list = Type::list(Type::Int);
        assert!(read_state(&answer(1, 2, |_| {}), &list).is_none());
        assert!(read_state(&answer(1, 4, |writer| writer.u8(0)), &list).is_none());
        assert!(read_state(&[], &list).is_none());
    }

    #[test]
    fn empty_results_show_the_empty_state() {
        let list = Type::list(Type::Int);
        let empty = Value::List(Vec::new());
        assert_eq!(shown(&empty, &list, true), None);
        assert_eq!(shown(&empty, &list, false), Some(empty.clone()));
        let optional = Type::optional(Type::Int);
        assert_eq!(shown(&Value::none(), &optional, false), None);
        assert_eq!(
            shown(&Value::some(Value::Int(3)), &optional, true),
            Some(Value::Int(3))
        );
        assert_eq!(shown(&Value::Int(0), &Type::Int, true), Some(Value::Int(0)));
        assert_eq!(count(&Value::List(vec![Value::Unit; 3])), Some(3));
        assert_eq!(count(&Value::none()), Some(0));
        assert_eq!(count(&Value::some(Value::Unit)), Some(1));
        assert_eq!(count(&Value::Int(3)), None);
    }
}
