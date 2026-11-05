//! Persistent JavaScript contexts. Builds select the system host or Boa.
//! Values cross the boundary as JSON. Each context has independent globals.
use serde::{Deserialize, Serialize};
pub use serde_json::Value;
#[cfg(not(feature = "boa"))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
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

pub struct Context {
    #[cfg(not(feature = "boa"))]
    id: String,
    #[cfg(feature = "boa")]
    worker: std::sync::mpsc::Sender<(String, Shared)>,
}
impl Context {
    pub fn new() -> std::result::Result<Self, Error> {
        #[cfg(feature = "boa")]
        let worker = boa::worker()?;
        #[cfg(not(feature = "boa"))]
        if !hub().lock().unwrap().available {
            return Err(Error(
                "JavaScript host is not running; enable javascript in the target".into(),
            ));
        }
        Ok(Self {
            #[cfg(not(feature = "boa"))]
            id: next(),
            #[cfg(feature = "boa")]
            worker,
        })
    }
    pub fn eval(&self, source: impl Into<String>) -> Evaluation {
        let state = Arc::new(Mutex::new(State {
            result: None,
            waker: None,
        }));
        let source = source.into();
        #[cfg(feature = "boa")]
        if self.worker.send((source, state.clone())).is_err() {
            finish(&state, Err(Error("JavaScript context stopped".into())));
        }
        #[cfg(not(feature = "boa"))]
        {
            let mut hub = hub().lock().unwrap();
            if hub.available {
                let id = next();
                hub.pending
                    .insert(id.clone(), (self.id.clone(), None, state.clone()));
                hub.queue.push_back(Request {
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
        #[cfg(not(feature = "boa"))]
        {
            let states = {
                let mut hub = hub().lock().unwrap();
                hub.queue.retain(|r| r.context != self.id);
                if hub.available {
                    hub.queue.push_back(Request {
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
struct Request {
    id: String,
    context: String,
    source: Option<String>,
}
#[derive(Default)]
struct Hub {
    available: bool,
    queue: VecDeque<Request>,
    pending: BTreeMap<String, (String, Option<f64>, Shared)>,
}
fn hub() -> &'static Mutex<Hub> {
    static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();
    HUB.get_or_init(Default::default)
}
/// Frontend adapter API; application code uses `Context`.
pub mod host {
    use super::*;
    pub fn start() {
        hub().lock().unwrap().available = true;
    }
    pub fn stop() {
        let states = {
            let mut hub = hub().lock().unwrap();
            hub.available = false;
            hub.queue.clear();
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
}
#[cfg(feature = "boa")]
mod boa {
    use super::*;
    use boa_engine::{
        Context as JsContext, Source, builtins::promise::PromiseState, object::builtins::JsPromise,
    };
    pub fn worker() -> std::result::Result<std::sync::mpsc::Sender<(String, Shared)>, Error> {
        let (tx, rx) = std::sync::mpsc::channel::<(String, Shared)>();
        std::thread::Builder::new()
            .name("bern-js".into())
            .spawn(move || {
                let mut context = JsContext::default();
                context
                    .runtime_limits_mut()
                    .set_loop_iteration_limit(1_000_000);
                context.runtime_limits_mut().set_recursion_limit(512);
                for (source, state) in rx {
                    let result = evaluate(&source, &mut context).map_err(Error);
                    finish(&state, result);
                }
            })
            .map_err(|e| Error(e.to_string()))?;
        Ok(tx)
    }
    fn evaluate(source: &str, context: &mut JsContext) -> std::result::Result<Value, String> {
        let value = context
            .eval(Source::from_bytes(source))
            .map_err(|e| e.to_string())?;
        context.run_jobs().map_err(|e| e.to_string())?;
        let value = if let Some(object) = value.as_object() {
            if let Ok(promise) = JsPromise::from_object(object.clone()) {
                match promise.state() {
                    PromiseState::Fulfilled(value) => value,
                    PromiseState::Rejected(error) => {
                        return Err(error
                            .to_string(context)
                            .map(|s| s.to_std_string_escaped())
                            .unwrap_or_else(|e| e.to_string()));
                    }
                    PromiseState::Pending => {
                        return Err(
                            "Promise did not settle; no external async host is installed".into(),
                        );
                    }
                }
            } else {
                value
            }
        } else {
            value
        };
        if value.is_undefined() {
            return Ok(Value::Null);
        }
        value
            .to_json(context)
            .map(|v| v.unwrap_or(Value::Null))
            .map_err(|e| e.to_string())
    }
}

/// HTML runtime shared by native WebView adapters.
pub const HOST_RUNTIME: &str = include_str!("runtime.js");
