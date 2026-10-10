//! Query subscriptions of the C ABI.
//!
//! A generated `Query` view opens a subscription for its window instance,
//! then sends its parameters whenever they change. Each request gets a
//! new parameter version and cancels the one before; the engine's provider
//! answers whenever it is ready, and only the answer for the current
//! version reaches the view (`bern_runtime::query`). Subscriptions end
//! when the view goes away or its window closes. When the engine changes a
//! state field a query declares in `#[reads(…)]`, its live subscriptions
//! ask again with the same parameters.

use super::{BernApp, BernBuffer, BernCallbacks, error_bytes, slice, with_app};
use crate::link::EngineLink;
use bern_command::{CallContext, Entry};
use bern_protocol::{
    CommandError, Invocation, ProtocolDescriptor, Reader, Value, Writer, decode, encode,
};
use bern_runtime::query::{QuerySubscriptions, Resolution, Resource, Ticket};
use std::sync::{Arc, Mutex};

pub(crate) type Subscriptions = Arc<Mutex<QuerySubscriptions>>;

/// Display state of a subscription: version, phase, then the data and
/// error the phase carries. Phases: 0 idle, 1 loading, 2 ready,
/// 3 refreshing, 4 failed (`has data` u8, data, code, message).
pub(crate) fn state_bytes(
    descriptor: &ProtocolDescriptor,
    query: usize,
    version: u64,
    state: &Resource<Value>,
) -> Vec<u8> {
    let result = &descriptor.commands[query].result;
    let mut writer = Writer::new();
    writer.u64(version);
    match state {
        Resource::Idle => writer.u8(0),
        Resource::Loading => writer.u8(1),
        Resource::Ready(data) => {
            writer.u8(2);
            encode(data, result, &descriptor.types, &mut writer);
        }
        Resource::Refreshing(data) => {
            writer.u8(3);
            encode(data, result, &descriptor.types, &mut writer);
        }
        Resource::Failed { error, previous } => {
            writer.u8(4);
            match previous {
                Some(data) => {
                    writer.u8(1);
                    encode(data, result, &descriptor.types, &mut writer);
                }
                None => writer.u8(0),
            }
            writer.text(&error.code);
            writer.text(&error.message);
        }
    }
    writer.bytes
}

/// Current state of `id` for the frontend, if the subscription is live.
fn current(
    subscriptions: &QuerySubscriptions,
    descriptor: &ProtocolDescriptor,
    id: u64,
) -> Option<Vec<u8>> {
    let entry = subscriptions.get(id)?;
    Some(state_bytes(
        descriptor,
        entry.query,
        entry.version(),
        entry.state(),
    ))
}

/// Send a request to the engine; its answer updates the subscription and,
/// when still current, the frontend.
pub(crate) fn send(
    host: &dyn EngineLink,
    subscriptions: &Subscriptions,
    callbacks: BernCallbacks,
    id: u64,
    query: usize,
    ticket: Ticket,
    params: Vec<Value>,
) {
    let descriptor = host.descriptor();
    let answers = Arc::clone(subscriptions);
    let cancellation = ticket.cancellation.clone();
    host.dispatch(
        Invocation {
            command: query,
            params,
        },
        CallContext {
            entry: Entry::Ui,
            request_id: format!("query-{id}-{}", ticket.version),
        },
        cancellation,
        Box::new(move |outcome| {
            let bytes = {
                let mut subscriptions = answers.lock().expect("query lock");
                match subscriptions.resolve(id, &ticket, outcome.result) {
                    Resolution::Applied => current(&subscriptions, descriptor, id),
                    Resolution::Discarded => None,
                }
            };
            if let Some(bytes) = bytes {
                callbacks.query(id, &bytes);
            }
        }),
    );
}

/// Engine state fields changed: ask again for every subscription whose
/// query reads one of them, and show that it is refreshing.
pub(crate) fn refresh(
    host: &dyn EngineLink,
    subscriptions: &Subscriptions,
    callbacks: BernCallbacks,
    changed: &[usize],
) {
    let descriptor = host.descriptor();
    let requests: Vec<_> = {
        let mut locked = subscriptions.lock().expect("query lock");
        locked
            .refresh(changed)
            .into_iter()
            .filter_map(|(id, ticket, params)| {
                let entry = locked.get(id)?;
                let bytes = state_bytes(descriptor, entry.query, entry.version(), entry.state());
                Some((id, entry.query, ticket, params, bytes))
            })
            .collect()
    };
    for (id, query, ticket, params, bytes) in requests {
        callbacks.query(id, &bytes);
        send(host, subscriptions, callbacks, id, query, ticket, params);
    }
}

/// Indices of the state fields a query reads.
pub(crate) fn reads(descriptor: &ProtocolDescriptor, query: usize) -> Vec<usize> {
    descriptor.commands[query]
        .reads
        .iter()
        .filter_map(|field| descriptor.state_index(field))
        .collect()
}

/// Open a subscription to catalog query `query` for a window instance.
/// Returns `0, subscription id` or `1, code, message`.
///
/// # Safety
/// `app` must be null or a live handle from `bern_app_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_query_open(
    app: *mut BernApp,
    kind: u32,
    generation: u64,
    query: u32,
) -> BernBuffer {
    with_app(app, BernBuffer::empty(), |app| {
        let descriptor = app.host.descriptor();
        let opened = (|| {
            let query = query as usize;
            if !descriptor
                .commands
                .get(query)
                .is_some_and(|spec| spec.is_query())
            {
                return Err(CommandError::new("unknown_command", "unknown query index"));
            }
            let window = app
                .session
                .lock()
                .expect("session lock")
                .open_windows()
                .into_iter()
                .find(|window| window.kind == kind as usize && window.generation == generation)
                .map(|window| window.id)
                .ok_or_else(|| {
                    CommandError::new("stale_window", "the window instance is no longer open")
                })?;
            Ok(app.queries.lock().expect("query lock").open(
                window,
                query,
                reads(descriptor, query),
            ))
        })();
        BernBuffer::from_vec(match opened {
            Ok(id) => {
                let mut writer = Writer::new();
                writer.u8(0);
                writer.u64(id);
                writer.bytes
            }
            Err(error) => error_bytes(&error),
        })
    })
}

/// Ask a subscription for new parameters, encoded in the query's
/// parameter order. The previous request is cancelled. Returns `0, state`
/// (see [`state_bytes`]) or `1, code, message`; the answer arrives later
/// through `on_query`.
///
/// # Safety
/// `params` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_query_request(
    app: *mut BernApp,
    subscription: u64,
    params: *const u8,
    len: usize,
) -> BernBuffer {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(params, len) };
    with_app(app, BernBuffer::empty(), |app| {
        let descriptor = app.host.descriptor();
        let requested = (|| {
            let query = app
                .queries
                .lock()
                .expect("query lock")
                .get(subscription)
                .map(|entry| entry.query)
                .ok_or_else(|| {
                    CommandError::new("target_not_found", "the query subscription has ended")
                })?;
            let spec = &descriptor.commands[query];
            let mut reader = Reader::new(bytes);
            let params = spec
                .params
                .iter()
                .map(|param| decode(&param.ty, &descriptor.types, &mut reader))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| CommandError::invalid_params(error.0))?;
            if !reader.is_finished() {
                return Err(CommandError::invalid_params("trailing parameter bytes"));
            }
            let (ticket, state) = {
                let mut queries = app.queries.lock().expect("query lock");
                let ticket = queries
                    .request(subscription, params.clone())
                    .ok_or_else(|| {
                        CommandError::new("target_not_found", "the query subscription has ended")
                    })?;
                (
                    ticket,
                    current(&queries, descriptor, subscription).unwrap_or_default(),
                )
            };
            send(
                app.host.as_ref(),
                &app.queries,
                app.callbacks,
                subscription,
                query,
                ticket,
                params,
            );
            Ok(state)
        })();
        BernBuffer::from_vec(match requested {
            Ok(state) => {
                let mut writer = Writer::new();
                writer.u8(0);
                writer.bytes.extend(state);
                writer.bytes
            }
            Err(error) => error_bytes(&error),
        })
    })
}

/// End a subscription: its request in flight is cancelled and later
/// answers are dropped.
///
/// # Safety
/// `app` must be null or a live handle from `bern_app_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_query_close(app: *mut BernApp, subscription: u64) {
    with_app(app, (), |app| {
        app.queries.lock().expect("query lock").close(subscription);
    })
}
