//! Persistent JavaScript contexts. Builds select Boa on native targets or a
//! Web Worker on the browser target. Values cross the boundary as JSON, each
//! context has independent globals, and `globalThis.bern.cli(...)` invokes the
//! application's in-process command catalog according to the context's
//! [`CliPolicy`].

mod cli;

pub use cli::{
    CliCall, CliDispatcher, CliError, CliFilter, CliInput, CliPolicy, CliTargetKind,
    DEFAULT_CLI_TIMEOUT_MS, DEFAULT_MAX_IN_FLIGHT, split_line,
};
use serde::{Deserialize, Serialize};
pub use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock, RwLock},
    task::{Context as TaskContext, Poll, Waker},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
type Result = std::result::Result<Value, Error>;

struct State {
    result: Option<Result>,
    waker: Option<Waker>,
}
type Shared = Arc<Mutex<State>>;

fn finish(state: &Shared, value: Result) {
    let waker = {
        let mut state = state.lock().unwrap();
        if state.result.is_some() {
            return;
        }
        state.result = Some(value);
        state.waker.take()
    };
    if let Some(waker) = waker {
        waker.wake();
    }
}

/// Await this without blocking the frontend's UI thread.
pub struct Evaluation {
    state: Shared,
}
impl Future for Evaluation {
    type Output = Result;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result> {
        let mut state = self.state.lock().unwrap();
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Evaluation {
    pub fn try_result(&self) -> Option<Result> {
        self.state.lock().unwrap().result.take()
    }
}

#[cfg(not(feature = "boa"))]
static NEXT: AtomicU64 = AtomicU64::new(1);
#[cfg(not(feature = "boa"))]
fn next() -> String {
    NEXT.fetch_add(1, Ordering::Relaxed).to_string()
}

/// One isolated, persistent JavaScript context.
pub struct Context {
    policy: Arc<RwLock<CliPolicy>>,
    #[cfg(not(feature = "boa"))]
    id: String,
    #[cfg(feature = "boa")]
    worker: std::sync::mpsc::Sender<boa::Message>,
}

impl Context {
    /// Create a context with the default [`CliPolicy::allow_all`] policy.
    pub fn new() -> std::result::Result<Self, Error> {
        Self::with_policy(CliPolicy::allow_all())
    }

    /// Create a context whose `bern.cli(...)` calls are governed by `policy`.
    pub fn with_policy(policy: CliPolicy) -> std::result::Result<Self, Error> {
        let policy = Arc::new(RwLock::new(policy));
        #[cfg(feature = "boa")]
        let worker = boa::worker(Arc::clone(&policy))?;
        #[cfg(not(feature = "boa"))]
        let id = {
            let mut hub = hub().lock().unwrap();
            if !hub.available {
                return Err(Error(
                    "JavaScript host is not running; enable javascript in the target".into(),
                ));
            }
            let id = next();
            hub.contexts.insert(id.clone(), Arc::clone(&policy));
            id
        };
        Ok(Self {
            policy,
            #[cfg(not(feature = "boa"))]
            id,
            #[cfg(feature = "boa")]
            worker,
        })
    }

    /// Replace the active [`CliPolicy`] for future `bern.cli(...)` calls in this context.
    pub fn set_policy(&self, policy: CliPolicy) {
        *self.policy.write().unwrap() = policy;
    }

    /// Snapshot of the current [`CliPolicy`] for this context.
    pub fn policy(&self) -> CliPolicy {
        self.policy.read().unwrap().clone()
    }

    pub fn eval(&self, source: impl Into<String>) -> Evaluation {
        let state = Arc::new(Mutex::new(State {
            result: None,
            waker: None,
        }));
        let source = source.into();
        #[cfg(feature = "boa")]
        if self
            .worker
            .send(boa::Message::Eval {
                source,
                state: state.clone(),
            })
            .is_err()
        {
            finish(&state, Err(Error("JavaScript context stopped".into())));
        }
        #[cfg(not(feature = "boa"))]
        {
            let mut hub = hub().lock().unwrap();
            if hub.available {
                let id = next();
                hub.pending
                    .insert(id.clone(), (self.id.clone(), None, state.clone()));
                hub.queue.push_back(Outbound::Eval {
                    id,
                    context: self.id.clone(),
                    source: Some(source),
                });
            } else {
                finish(&state, Err(Error("JavaScript host stopped".into())));
            }
        }
        Evaluation { state }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        #[cfg(feature = "boa")]
        {
            let _ = self.worker.send(boa::Message::Stop);
        }
        #[cfg(not(feature = "boa"))]
        {
            let states = {
                let mut hub = hub().lock().unwrap();
                hub.contexts.remove(&self.id);
                hub.pending_cli.retain(|(ctx, _), _| ctx != &self.id);
                hub.queue.retain(|item| item.context() != self.id);
                if hub.available {
                    hub.queue.push_back(Outbound::Eval {
                        id: next(),
                        context: self.id.clone(),
                        source: None,
                    });
                }
                let ids: Vec<_> = hub
                    .pending
                    .iter()
                    .filter(|(_, p)| p.0 == self.id)
                    .map(|(id, _)| id.clone())
                    .collect();
                ids.into_iter()
                    .filter_map(|id| hub.pending.remove(&id).map(|p| p.2))
                    .collect::<Vec<_>>()
            };
            for state in states {
                finish(&state, Err(Error("JavaScript context disposed".into())));
            }
        }
    }
}

#[derive(Serialize)]
struct CliReplyPayload {
    call_id: String,
    context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<CliError>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Outbound {
    #[cfg(not(feature = "boa"))]
    Eval {
        id: String,
        context: String,
        source: Option<String>,
    },
    CliReply {
        cli_reply: CliReplyPayload,
    },
}

#[cfg(not(feature = "boa"))]
impl Outbound {
    fn context(&self) -> &str {
        match self {
            Outbound::Eval { context, .. } => context,
            Outbound::CliReply { cli_reply } => &cli_reply.context,
        }
    }
}

#[derive(Default)]
struct Hub {
    available: bool,
    queue: VecDeque<Outbound>,
    pending: BTreeMap<String, (String, Option<f64>, Shared)>,
    contexts: BTreeMap<String, Arc<RwLock<CliPolicy>>>,
    pending_cli: BTreeMap<(String, String), (Option<f64>, u64)>,
}

fn hub() -> &'static Mutex<Hub> {
    static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();
    HUB.get_or_init(Default::default)
}

type ActiveDispatcher = Option<(u64, Arc<dyn CliDispatcher>)>;

fn cli_registry() -> &'static Mutex<ActiveDispatcher> {
    static REGISTRY: OnceLock<Mutex<ActiveDispatcher>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

static NEXT_CLI_REGISTRATION: AtomicU64 = AtomicU64::new(1);

/// Keeps a [`CliDispatcher`] registered until dropped.
pub struct CliRegistration {
    token: u64,
}

impl Drop for CliRegistration {
    fn drop(&mut self) {
        if let Ok(mut guard) = cli_registry().lock()
            && guard
                .as_ref()
                .is_some_and(|(token, _)| *token == self.token)
        {
            *guard = None;
        }
    }
}

/// Frontend and runtime host API; application code uses [`Context`] and [`CliPolicy`].
pub mod host {
    use super::*;

    /// Install the in-process CLI dispatcher used by `bern.cli(...)`.
    pub fn install_cli(dispatcher: Arc<dyn CliDispatcher>) -> CliRegistration {
        let token = NEXT_CLI_REGISTRATION.fetch_add(1, Ordering::Relaxed);
        *cli_registry().lock().unwrap() = Some((token, dispatcher));
        CliRegistration { token }
    }

    /// Dispatch a CLI call through the currently installed dispatcher.
    pub fn dispatch_cli(
        input: CliInput,
        policy: Arc<CliPolicy>,
        done: Box<dyn FnOnce(std::result::Result<Value, CliError>) + Send>,
    ) {
        let dispatcher = cli_registry()
            .lock()
            .unwrap()
            .as_ref()
            .map(|(_, dispatcher)| Arc::clone(dispatcher));
        match dispatcher {
            Some(dispatcher) => dispatcher.dispatch(input, policy, done),
            None => done(Err(CliError::new(
                "cli_unavailable",
                "no application CLI host is attached to bern.cli",
            ))),
        }
    }

    pub fn start() {
        hub().lock().unwrap().available = true;
    }

    pub fn stop() {
        let states = {
            let mut hub = hub().lock().unwrap();
            hub.available = false;
            hub.queue.clear();
            hub.contexts.clear();
            hub.pending_cli.clear();
            std::mem::take(&mut hub.pending)
        };
        for (_, (_, _, state)) in states {
            finish(&state, Err(Error("JavaScript host stopped".into())));
        }
    }

    /// A JSON batch, or an empty string. Called on the UI thread.
    pub fn poll(now_ms: f64) -> String {
        let (requests, expired) = {
            let mut hub = hub().lock().unwrap();
            let ids: Vec<_> = hub
                .pending
                .iter_mut()
                .filter_map(|(id, p)| {
                    let deadline = p.1.get_or_insert(now_ms + 30_000.0);
                    (now_ms >= *deadline).then(|| id.clone())
                })
                .collect();
            let expired: Vec<_> = ids
                .into_iter()
                .filter_map(|id| hub.pending.remove(&id))
                .collect();

            let expired_cli: Vec<_> = hub
                .pending_cli
                .iter_mut()
                .filter_map(|(key, (deadline, timeout_ms))| {
                    let limit = deadline.get_or_insert(now_ms + *timeout_ms as f64);
                    (now_ms >= *limit).then(|| (key.clone(), *timeout_ms))
                })
                .collect();
            for ((context, call_id), timeout_ms) in expired_cli {
                hub.pending_cli.remove(&(context.clone(), call_id.clone()));
                hub.queue.push_back(Outbound::CliReply {
                    cli_reply: CliReplyPayload {
                        call_id,
                        context,
                        value: None,
                        error: Some(CliError::new(
                            "cli_timeout",
                            format!("bern.cli timed out after {timeout_ms} ms"),
                        )),
                    },
                });
            }

            (hub.queue.drain(..).collect::<Vec<_>>(), expired)
        };
        for (_, _, state) in expired {
            finish(&state, Err(Error("JavaScript evaluation timed out".into())));
        }
        if requests.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&requests).unwrap()
        }
    }

    pub fn complete(json: &str) {
        #[derive(Deserialize)]
        struct Reply {
            id: String,
            #[serde(default)]
            value: Value,
            error: Option<String>,
        }
        let Ok(reply) = serde_json::from_str::<Reply>(json) else {
            return;
        };
        let pending = hub().lock().unwrap().pending.remove(&reply.id);
        if let Some((_, _, state)) = pending {
            finish(
                &state,
                reply.error.map_or(Ok(reply.value), |e| Err(Error(e))),
            );
        }
    }

    /// Accept one `bern.cli(...)` request from an external JS host (such as a browser Worker).
    pub fn cli(json: &str) {
        #[derive(Deserialize)]
        struct Request {
            context: String,
            call_id: String,
            input: CliInput,
        }
        let Ok(request) = serde_json::from_str::<Request>(json) else {
            return;
        };
        let policy = {
            let mut hub = hub().lock().unwrap();
            let Some(policy_lock) = hub.contexts.get(&request.context) else {
                hub.queue.push_back(Outbound::CliReply {
                    cli_reply: CliReplyPayload {
                        call_id: request.call_id,
                        context: request.context,
                        value: None,
                        error: Some(CliError::new(
                            "cli_unavailable",
                            "JavaScript context is not active",
                        )),
                    },
                });
                return;
            };
            let policy = policy_lock.read().unwrap().clone();
            let in_flight = hub
                .pending_cli
                .keys()
                .filter(|(ctx, _)| ctx == &request.context)
                .count();
            if in_flight >= policy.max_in_flight() {
                hub.queue.push_back(Outbound::CliReply {
                    cli_reply: CliReplyPayload {
                        call_id: request.call_id,
                        context: request.context,
                        value: None,
                        error: Some(CliError::new(
                            "too_many_requests",
                            format!(
                                "too many concurrent bern.cli calls (limit {})",
                                policy.max_in_flight()
                            ),
                        )),
                    },
                });
                return;
            }
            hub.pending_cli.insert(
                (request.context.clone(), request.call_id.clone()),
                (None, policy.timeout_ms()),
            );
            Arc::new(policy)
        };
        let context = request.context;
        let call_id = request.call_id;
        dispatch_cli(
            request.input,
            policy,
            Box::new(move |result| {
                let mut hub = hub().lock().unwrap();
                if hub
                    .pending_cli
                    .remove(&(context.clone(), call_id.clone()))
                    .is_none()
                {
                    return;
                }
                let payload = match result {
                    Ok(value) => CliReplyPayload {
                        call_id,
                        context,
                        value: Some(value),
                        error: None,
                    },
                    Err(error) => CliReplyPayload {
                        call_id,
                        context,
                        value: None,
                        error: Some(error),
                    },
                };
                hub.queue
                    .push_back(Outbound::CliReply { cli_reply: payload });
            }),
        );
    }
}

#[cfg(feature = "boa")]
mod boa {
    use super::*;
    use boa_engine::{
        Context as JsContext, JsArgs, JsNativeError, JsValue, NativeFunction, Source,
        builtins::promise::{PromiseState, ResolvingFunctions},
        js_string,
        object::{ObjectInitializer, builtins::JsPromise},
        property::Attribute,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::{Duration, Instant};

    pub(crate) enum Message {
        Eval {
            source: String,
            state: Shared,
        },
        CliReply {
            call_id: u64,
            result: std::result::Result<Value, CliError>,
        },
        Stop,
    }

    struct PendingCli {
        resolvers: ResolvingFunctions,
        deadline: Instant,
        timeout_ms: u64,
    }

    type PendingCliMap = Rc<RefCell<BTreeMap<u64, PendingCli>>>;
    type PendingEvalList = Vec<(JsPromise, Shared)>;

    pub(crate) fn worker(
        policy: Arc<RwLock<CliPolicy>>,
    ) -> std::result::Result<mpsc::Sender<Message>, Error> {
        let (tx, rx) = mpsc::channel::<Message>();
        let reply_tx = tx.clone();
        std::thread::Builder::new()
            .name("bern-js".into())
            .spawn(move || run_worker(rx, reply_tx, policy))
            .map_err(|e| Error(e.to_string()))?;
        Ok(tx)
    }

    fn run_worker(
        rx: mpsc::Receiver<Message>,
        reply_tx: mpsc::Sender<Message>,
        policy: Arc<RwLock<CliPolicy>>,
    ) {
        let mut context = JsContext::default();
        context
            .runtime_limits_mut()
            .set_loop_iteration_limit(1_000_000);
        context.runtime_limits_mut().set_recursion_limit(512);

        let pending_cli: PendingCliMap = Rc::new(RefCell::new(BTreeMap::new()));
        let next_call_id = Rc::new(Cell::new(1u64));
        install_bern_global(
            &mut context,
            reply_tx,
            policy,
            Rc::clone(&pending_cli),
            next_call_id,
        );

        let mut pending_evals: PendingEvalList = Vec::new();
        let mut runs: u32 = 0;

        loop {
            let next_deadline = pending_cli
                .borrow()
                .values()
                .map(|entry| entry.deadline)
                .min();
            let message = match next_deadline {
                Some(deadline) => {
                    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        Ok(message) => Some(message),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                None => match rx.recv() {
                    Ok(message) => Some(message),
                    Err(_) => break,
                },
            };

            if let Some(message) = message {
                match message {
                    Message::Stop => break,
                    Message::Eval { source, state } => {
                        handle_eval(
                            &source,
                            state,
                            &mut context,
                            &pending_cli,
                            &mut pending_evals,
                        );
                        runs = runs.wrapping_add(1);
                    }
                    Message::CliReply { call_id, result } => {
                        let entry = pending_cli.borrow_mut().remove(&call_id);
                        if let Some(entry) = entry {
                            settle_cli_promise(entry.resolvers, result, &mut context);
                            let _ = context.run_jobs();
                        }
                    }
                }
            }

            expire_timed_out_cli(&pending_cli, &mut context);
            settle_pending_evals(&mut pending_evals, &pending_cli, &mut context);

            if runs > 0
                && runs.is_multiple_of(64)
                && pending_cli.borrow().is_empty()
                && pending_evals.is_empty()
            {
                boa_gc::force_collect();
            }
        }

        pending_cli.borrow_mut().clear();
        for (_, state) in pending_evals.drain(..) {
            finish(&state, Err(Error("JavaScript context stopped".into())));
        }
        drop(context);
        boa_gc::force_collect();
    }

    fn install_bern_global(
        context: &mut JsContext,
        reply_tx: mpsc::Sender<Message>,
        policy: Arc<RwLock<CliPolicy>>,
        pending_cli: PendingCliMap,
        next_call_id: Rc<Cell<u64>>,
    ) {
        // SAFETY: Captured values are non-GC Rust handles (`mpsc::Sender`, `Arc`, `Rc<Cell>`) plus
        // `pending_cli`, which is explicitly cleared before `context` is dropped on worker shutdown.
        let cli_fn = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let input = match parse_cli_args(args, ctx) {
                    Ok(input) => input,
                    Err(error) => return Ok(reject_promise(&error, ctx)),
                };
                let current_policy = Arc::new(policy.read().unwrap().clone());
                if pending_cli.borrow().len() >= current_policy.max_in_flight() {
                    return Ok(reject_promise(
                        &CliError::new(
                            "too_many_requests",
                            format!(
                                "too many concurrent bern.cli calls (limit {})",
                                current_policy.max_in_flight()
                            ),
                        ),
                        ctx,
                    ));
                }
                let call_id = next_call_id.get();
                next_call_id.set(call_id.wrapping_add(1));
                let (promise, resolvers) = JsPromise::new_pending(ctx);
                let timeout_ms = current_policy.timeout_ms();
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                pending_cli.borrow_mut().insert(
                    call_id,
                    PendingCli {
                        resolvers,
                        deadline,
                        timeout_ms,
                    },
                );
                let tx = reply_tx.clone();
                host::dispatch_cli(
                    input,
                    current_policy,
                    Box::new(move |result| {
                        let _ = tx.send(Message::CliReply { call_id, result });
                    }),
                );
                Ok(promise.into())
            })
        };

        let bern_object = ObjectInitializer::new(context)
            .function(cli_fn, js_string!("cli"), 1)
            .build();
        let _ = context.register_global_property(
            js_string!("bern"),
            bern_object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        );
    }

    fn parse_cli_args(
        args: &[JsValue],
        context: &mut JsContext,
    ) -> std::result::Result<CliInput, CliError> {
        let first = args.get_or_undefined(0);
        if first.is_string() {
            let target = first
                .to_string(context)
                .map_err(|e| CliError::invalid_params(e.to_string()))?
                .to_std_string_escaped();
            let second = args.get_or_undefined(1);
            if second.is_undefined() || second.is_null() {
                return Ok(CliInput::Line { line: target });
            }
            if !second.is_object() || second.as_object().is_some_and(|obj| obj.is_array()) {
                return Err(CliError::invalid_params(
                    "second argument to bern.cli must be a parameter object",
                ));
            }
            let json = second
                .to_json(context)
                .map_err(|e| CliError::invalid_params(e.to_string()))?
                .unwrap_or(Value::Null);
            let Value::Object(params) = json else {
                return Err(CliError::invalid_params(
                    "second argument to bern.cli must be a plain JSON object",
                ));
            };
            let third = args.get_or_undefined(2);
            let session = if third.is_undefined() || third.is_null() {
                None
            } else if third.is_string() {
                let s = third
                    .to_string(context)
                    .map_err(|e| CliError::invalid_params(e.to_string()))?
                    .to_std_string_escaped();
                (!s.is_empty()).then_some(s)
            } else if third.is_object() {
                let opts = third
                    .to_json(context)
                    .map_err(|e| CliError::invalid_params(e.to_string()))?
                    .unwrap_or(Value::Null);
                match opts {
                    Value::Object(map) => match map.get("session") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(s)) => (!s.is_empty()).then_some(s.clone()),
                        Some(_) => {
                            return Err(CliError::invalid_params(
                                "options.session must be a string",
                            ));
                        }
                    },
                    _ => {
                        return Err(CliError::invalid_params(
                            "third argument to bern.cli must be an options object or session id",
                        ));
                    }
                }
            } else {
                return Err(CliError::invalid_params(
                    "third argument to bern.cli must be an options object or session id",
                ));
            };
            Ok(CliInput::Command {
                target,
                params,
                session,
            })
        } else if first.as_object().is_some_and(|obj| obj.is_array()) {
            let second = args.get_or_undefined(1);
            if !second.is_undefined() && !second.is_null() {
                return Err(CliError::invalid_params(
                    "array form of bern.cli does not take a second argument",
                ));
            }
            let json = first
                .to_json(context)
                .map_err(|e| CliError::invalid_params(e.to_string()))?
                .unwrap_or(Value::Null);
            let Value::Array(items) = json else {
                return Err(CliError::invalid_params(
                    "bern.cli argument list must be an array",
                ));
            };
            let mut parsed = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(s) => parsed.push(s),
                    Value::Number(n) => parsed.push(n.to_string()),
                    Value::Bool(b) => parsed.push(b.to_string()),
                    _ => {
                        return Err(CliError::invalid_params(
                            "bern.cli argument array elements must be strings, numbers or booleans",
                        ));
                    }
                }
            }
            Ok(CliInput::Args { args: parsed })
        } else {
            Err(CliError::invalid_params(
                "bern.cli expects a command string or an argument array",
            ))
        }
    }

    fn make_cli_error(error: &CliError, context: &mut JsContext) -> JsValue {
        let object = JsNativeError::error()
            .with_message(error.message.clone())
            .into_opaque(context);
        let _ = object.set(
            js_string!("code"),
            JsValue::from(js_string!(error.code.as_str())),
            false,
            context,
        );
        object.into()
    }

    fn reject_promise(error: &CliError, context: &mut JsContext) -> JsValue {
        let (promise, resolvers) = JsPromise::new_pending(context);
        let err = make_cli_error(error, context);
        let _ = resolvers
            .reject
            .call(&JsValue::undefined(), &[err], context);
        promise.into()
    }

    fn settle_cli_promise(
        resolvers: ResolvingFunctions,
        result: std::result::Result<Value, CliError>,
        context: &mut JsContext,
    ) {
        match result {
            Ok(value) => match JsValue::from_json(&value, context) {
                Ok(js_val) => {
                    let _ = resolvers
                        .resolve
                        .call(&JsValue::undefined(), &[js_val], context);
                }
                Err(error) => {
                    let err = make_cli_error(
                        &CliError::new("invalid_result", error.to_string()),
                        context,
                    );
                    let _ = resolvers
                        .reject
                        .call(&JsValue::undefined(), &[err], context);
                }
            },
            Err(error) => {
                let err = make_cli_error(&error, context);
                let _ = resolvers
                    .reject
                    .call(&JsValue::undefined(), &[err], context);
            }
        }
    }

    fn expire_timed_out_cli(pending_cli: &PendingCliMap, context: &mut JsContext) {
        let now = Instant::now();
        let expired_ids: Vec<u64> = pending_cli
            .borrow()
            .iter()
            .filter_map(|(&id, entry)| (entry.deadline <= now).then_some(id))
            .collect();
        if expired_ids.is_empty() {
            return;
        }
        let expired: Vec<PendingCli> = {
            let mut map = pending_cli.borrow_mut();
            expired_ids
                .into_iter()
                .filter_map(|id| map.remove(&id))
                .collect()
        };
        for entry in expired {
            settle_cli_promise(
                entry.resolvers,
                Err(CliError::new(
                    "cli_timeout",
                    format!("bern.cli timed out after {} ms", entry.timeout_ms),
                )),
                context,
            );
        }
        let _ = context.run_jobs();
    }

    fn value_to_result(value: JsValue, context: &mut JsContext) -> Result {
        if value.is_undefined() {
            return Ok(Value::Null);
        }
        value
            .to_json(context)
            .map(|v| v.unwrap_or(Value::Null))
            .map_err(|e| Error(e.to_string()))
    }

    fn format_rejection(error: JsValue, context: &mut JsContext) -> Error {
        Error(
            error
                .to_string(context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|e| e.to_string()),
        )
    }

    fn handle_eval(
        source: &str,
        state: Shared,
        context: &mut JsContext,
        pending_cli: &PendingCliMap,
        pending_evals: &mut PendingEvalList,
    ) {
        let value = match context.eval(Source::from_bytes(source)) {
            Ok(value) => value,
            Err(error) => {
                finish(&state, Err(Error(error.to_string())));
                return;
            }
        };
        if let Err(error) = context.run_jobs() {
            finish(&state, Err(Error(error.to_string())));
            return;
        }
        if let Some(object) = value.as_object()
            && let Ok(promise) = JsPromise::from_object(object.clone())
        {
            match promise.state() {
                PromiseState::Fulfilled(val) => finish(&state, value_to_result(val, context)),
                PromiseState::Rejected(err) => finish(&state, Err(format_rejection(err, context))),
                PromiseState::Pending => {
                    if pending_cli.borrow().is_empty() {
                        finish(
                            &state,
                            Err(Error(
                                "Promise did not settle; no external async host is installed"
                                    .into(),
                            )),
                        );
                    } else {
                        pending_evals.push((promise, state));
                    }
                }
            }
            return;
        }
        finish(&state, value_to_result(value, context));
    }

    fn settle_pending_evals(
        pending_evals: &mut PendingEvalList,
        pending_cli: &PendingCliMap,
        context: &mut JsContext,
    ) {
        let no_more_cli = pending_cli.borrow().is_empty();
        let mut index = 0;
        while index < pending_evals.len() {
            match pending_evals[index].0.state() {
                PromiseState::Fulfilled(val) => {
                    let (_, state) = pending_evals.remove(index);
                    finish(&state, value_to_result(val, context));
                }
                PromiseState::Rejected(err) => {
                    let (_, state) = pending_evals.remove(index);
                    finish(&state, Err(format_rejection(err, context)));
                }
                PromiseState::Pending if no_more_cli => {
                    let (_, state) = pending_evals.remove(index);
                    finish(
                        &state,
                        Err(Error(
                            "Promise did not settle; no external async host is installed".into(),
                        )),
                    );
                }
                PromiseState::Pending => index += 1,
            }
        }
    }
}

/// HTML runtime shared by native WebView adapters.
pub const HOST_RUNTIME: &str = include_str!("runtime.js");
