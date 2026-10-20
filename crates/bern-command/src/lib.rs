//! One engine instance, many entries.
//!
//! [`Host`] owns the application engine on a dedicated thread, validates
//! every invocation against the catalog at the receiving boundary, runs
//! commands one after another and publishes consistent, versioned state
//! updates. The UI, CLI, JSON channel and MCP server all call the same
//! host; none of them re-implements business logic.
//!
//! On WebAssembly there are no threads: the engine runs on the calling
//! thread instead, one job at a time, and requests made while a job runs
//! (from a reply or a state listener) wait in order until it finishes.
//!
//! Queries are read-only. Their providers start on the engine thread in
//! arrival order, so they see the state left by the commands before them,
//! but they answer through a [`QueryAnswer`] that may be completed later
//! and from any thread. A query request carries a [`Cancellation`]; the
//! provider can stop early, and an answer that arrives after cancellation
//! is reported as `cancelled` instead of as a result.

mod composite;
pub mod files;
pub mod outbox;
pub mod resources;
pub mod storage;
pub mod trace;

pub use composite::{Composite, ModuleEngine};

use bern_protocol::{
    Cancellation, CommandError, CommandOutcome, CommandStatus, Invocation, ProtocolDescriptor,
    StateUpdate, Value,
};
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Implemented by the code `#[bern::protocol]` generates for an engine.
pub trait EngineProtocol: Send + 'static {
    fn descriptor(&self) -> &'static ProtocolDescriptor;
    /// Run a command whose parameters already match the catalog.
    fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError>;
    /// Start a read-only query whose parameters already match the catalog.
    /// The provider answers through `answer`, now or later.
    fn query(&self, invocation: &Invocation, answer: QueryAnswer) {
        let _ = invocation;
        answer.send(Err(CommandError::new(
            "unknown_command",
            "this engine provides no queries",
        )));
    }
    /// Current value of every state field, in descriptor order.
    fn snapshot(&self) -> Vec<Value>;
    /// The optional modules the application starts with, as saved by
    /// [`EngineProtocol::modules_changed`]; `None` keeps each module's
    /// default.
    fn initial_modules(&self) -> Option<Vec<String>> {
        None
    }
    /// The optional modules running after a change, for the application to
    /// save.
    fn modules_changed(&mut self, enabled: &[String]) {
        let _ = enabled;
    }
}

/// Which entry a request came through, for diagnostics and policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Ui,
    Cli,
    Json,
    Mcp,
    Test,
}

impl Entry {
    pub fn name(self) -> &'static str {
        match self {
            Entry::Ui => "ui",
            Entry::Cli => "cli",
            Entry::Json => "json",
            Entry::Mcp => "mcp",
            Entry::Test => "test",
        }
    }
}

/// Caller information established by the host or transport, never taken
/// from command parameters.
#[derive(Clone, Debug)]
pub struct CallContext {
    pub entry: Entry,
    pub request_id: String,
}

type Reply = Box<dyn FnOnce(CommandOutcome) + Send>;
type Listener = Arc<dyn Fn(&StateUpdate) + Send + Sync>;

enum Job {
    Invoke {
        invocation: Invocation,
        context: CallContext,
        cancellation: Cancellation,
        reply: Reply,
        trace: Option<trace::JobTrace>,
    },
    Stop,
}

/// The pending answer of one query request. The provider answers exactly
/// once, from any thread; dropping it unanswered fails the request with
/// `query_dropped` (or `cancelled` when nobody waits for it any more).
pub struct QueryAnswer {
    reply: Option<Reply>,
    shared: Arc<Shared>,
    command: usize,
    cancellation: Cancellation,
    /// The traced job's sequence number and catalog.
    trace: Option<(u64, u16)>,
}

impl QueryAnswer {
    /// Whether the requester no longer wants this answer. Providers doing
    /// expensive work check it and stop early.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// The request's cancellation flag, for work that outlives a check.
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    /// Answer the query. The value is checked against the declared result.
    pub fn send(mut self, result: Result<Value, CommandError>) {
        self.finish(result);
    }

    fn finish(&mut self, result: Result<Value, CommandError>) {
        let Some(reply) = self.reply.take() else {
            return;
        };
        let descriptor = self.shared.descriptor;
        let spec = &descriptor.commands[self.command];
        let result = if self.cancellation.is_cancelled() {
            Err(cancelled())
        } else {
            result.and_then(|value| {
                value
                    .check(&spec.result, &descriptor.types)
                    .map_err(|error| {
                        CommandError::new(
                            "invalid_result",
                            format!("`{}` returned an invalid result: {error}", spec.id),
                        )
                    })?;
                Ok(value)
            })
        };
        if let Some((seq, catalog)) = self.trace {
            let at = trace::now_ns();
            trace::record(trace::Record {
                t0: at,
                t1: at,
                key: seq,
                a: match &result {
                    Ok(_) => 0,
                    Err(error) if error.code == "cancelled" => 2,
                    Err(_) => 1,
                },
                kind: trace::kind::QUERY_ANSWER,
                catalog,
                name: self.command as u32,
                ..trace::Record::default()
            });
        }
        let revision = self.shared.state.lock().map(|state| state.0).unwrap_or(0);
        reply(CommandOutcome {
            status: CommandStatus::Completed,
            revision,
            result,
        });
    }
}

impl Drop for QueryAnswer {
    fn drop(&mut self) {
        let error = if thread::panicking() {
            CommandError::new("engine_failed", "the query panicked")
        } else {
            CommandError::new(
                "query_dropped",
                "the query provider dropped the request without answering",
            )
        };
        self.finish(Err(error));
    }
}

fn stopped_outcome(revision: u64) -> CommandOutcome {
    CommandOutcome {
        status: CommandStatus::Completed,
        revision,
        result: Err(CommandError::new(
            "engine_stopped",
            "the engine has stopped",
        )),
    }
}

fn cancelled() -> CommandError {
    CommandError::new("cancelled", "the request was cancelled")
}

struct Shared {
    engine_id: String,
    descriptor: &'static ProtocolDescriptor,
    /// The id trace records name the catalog by.
    catalog: u16,
    state: Mutex<(u64, Vec<Value>)>,
    listeners: Mutex<Vec<(u64, Listener)>>,
    next_listener: AtomicU64,
}

/// Handle to a running engine. Cloning shares the same instance.
#[derive(Clone)]
pub struct Host {
    shared: Arc<Shared>,
    jobs: Jobs,
}

/// Where jobs go: the engine thread, or the caller's thread when there
/// are no threads.
#[derive(Clone)]
enum Jobs {
    Thread(Sender<Job>),
    Local(Arc<LocalJobs>),
}

struct LocalJobs {
    worker: Mutex<Box<dyn FnMut(Job) -> bool + Send>>,
    pending: Mutex<VecDeque<Job>>,
    draining: AtomicBool,
    stopped: AtomicBool,
}

impl LocalJobs {
    /// Queue a job and, unless a job is already running further up the
    /// stack, run queued jobs until none are left.
    fn push(&self, job: Job) -> Result<(), Job> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(job);
        }
        self.pending.lock().expect("job lock").push_back(job);
        if self.draining.swap(true, Ordering::AcqRel) {
            // A job is running further up the stack; it drains this one.
            return Ok(());
        }
        loop {
            let Some(job) = self.pending.lock().expect("job lock").pop_front() else {
                break;
            };
            if self.stopped.load(Ordering::Acquire) {
                // Queued behind a stop: answer as the stopped engine does.
                if let Job::Invoke { reply, .. } = job {
                    reply(stopped_outcome(0));
                }
                continue;
            }
            let keep = (self.worker.lock().expect("worker lock"))(job);
            if !keep {
                self.stopped.store(true, Ordering::Release);
            }
        }
        self.draining.store(false, Ordering::Release);
        Ok(())
    }
}

/// Keeps a state listener registered until dropped.
pub struct Subscription {
    shared: Arc<Shared>,
    id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut listeners) = self.shared.listeners.lock() {
            listeners.retain(|(id, _)| *id != self.id);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn new_engine_id() -> String {
    #[link(wasm_import_module = "bern")]
    unsafe extern "C" {
        /// A random 32-bit value from the host page.
        fn random_u32() -> u32;
    }
    // SAFETY: the page provides `bern.random_u32` with this signature.
    let (high, low) = unsafe { (random_u32(), random_u32()) };
    format!("{high:08x}{:06x}", low & 0xff_ffff)
}

#[cfg(not(target_arch = "wasm32"))]
fn new_engine_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{:08x}{:04x}{:02x}",
        nanos as u32 ^ (nanos >> 32) as u32,
        std::process::id() & 0xffff,
        sequence & 0xff
    )
}

impl Host {
    /// Start the engine on its own thread, or on the calling thread where
    /// there are no threads.
    pub fn start(engine: impl EngineProtocol) -> Host {
        if cfg!(target_arch = "wasm32") {
            Self::start_local(engine)
        } else {
            Self::start_thread(engine)
        }
    }

    fn shared(engine: &impl EngineProtocol) -> Arc<Shared> {
        let descriptor = engine.descriptor();
        let initial = engine.snapshot();
        debug_assert_eq!(initial.len(), descriptor.state.len());
        let catalog = trace::register_catalog(descriptor);
        trace::auto_start();
        Arc::new(Shared {
            engine_id: new_engine_id(),
            descriptor,
            catalog,
            state: Mutex::new((0, initial)),
            listeners: Mutex::new(Vec::new()),
            next_listener: AtomicU64::new(1),
        })
    }

    fn start_thread(engine: impl EngineProtocol) -> Host {
        let shared = Self::shared(&engine);
        let (jobs, receiver) = channel::<Job>();
        let mut worker = Worker::new(engine, Arc::clone(&shared));
        thread::Builder::new()
            .name("bern-engine".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    if !worker.handle(job) {
                        break;
                    }
                }
            })
            .expect("start engine thread");
        Host {
            shared,
            jobs: Jobs::Thread(jobs),
        }
    }

    /// Run jobs on the thread that dispatches them. Replies and listeners
    /// run before `dispatch` returns, except for requests made while a job
    /// is running, which run right after it. Blocking calls must not be
    /// made from a reply or listener in this mode.
    pub fn start_local(engine: impl EngineProtocol) -> Host {
        let shared = Self::shared(&engine);
        let mut worker = Worker::new(engine, Arc::clone(&shared));
        let local = LocalJobs {
            worker: Mutex::new(Box::new(move |job| trace::local_engine(|| worker.handle(job)))),
            pending: Mutex::new(VecDeque::new()),
            draining: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        };
        Host {
            shared,
            jobs: Jobs::Local(Arc::new(local)),
        }
    }

    fn send(&self, job: Job) -> Result<(), Job> {
        match &self.jobs {
            Jobs::Thread(sender) => sender.send(job).map_err(|error| error.0),
            Jobs::Local(local) => local.push(job),
        }
    }

    pub fn engine_id(&self) -> &str {
        &self.shared.engine_id
    }

    pub fn descriptor(&self) -> &'static ProtocolDescriptor {
        self.shared.descriptor
    }

    /// Current revision and full state.
    pub fn snapshot(&self) -> (u64, Vec<Value>) {
        self.shared.state.lock().expect("state lock").clone()
    }

    /// Find a command by its stable id.
    pub fn resolve(&self, command: &str) -> Option<usize> {
        self.shared
            .descriptor
            .command(command)
            .map(|(index, _)| index)
    }

    /// Check an invocation against the catalog without running it.
    pub fn validate(&self, invocation: &Invocation) -> Result<(), CommandError> {
        validate(self.shared.descriptor, invocation)
    }

    /// Whether the catalog entry at `command` is a read-only query.
    pub fn is_query(&self, command: usize) -> bool {
        self.shared
            .descriptor
            .commands
            .get(command)
            .is_some_and(|spec| spec.is_query())
    }

    /// Queue a command or query; `reply` runs once it finishes, on the
    /// engine thread for commands and on the answering thread for queries.
    pub fn dispatch(
        &self,
        invocation: Invocation,
        context: CallContext,
        reply: impl FnOnce(CommandOutcome) + Send + 'static,
    ) {
        self.dispatch_cancellable(invocation, context, Cancellation::new(), reply);
    }

    /// Queue a request whose query can be cancelled through `cancellation`.
    /// Commands are not cancellable and ignore it.
    pub fn dispatch_cancellable(
        &self,
        invocation: Invocation,
        context: CallContext,
        cancellation: Cancellation,
        reply: impl FnOnce(CommandOutcome) + Send + 'static,
    ) {
        let reply: Reply = Box::new(reply);
        let trace = if trace::on(trace::ENGINE) {
            trace::job(self.shared.catalog, invocation.command, &context)
        } else {
            None
        };
        if let Err(Job::Invoke { reply, .. }) = self.send(Job::Invoke {
            invocation,
            context,
            cancellation,
            reply,
            trace,
        }) {
            reply(stopped_outcome(self.snapshot().0));
        }
    }

    /// Queue a command and wait for its outcome.
    pub fn dispatch_blocking(
        &self,
        invocation: Invocation,
        context: CallContext,
    ) -> CommandOutcome {
        let (sender, receiver) = channel();
        self.dispatch(invocation, context, move |outcome| {
            let _ = sender.send(outcome);
        });
        receiver.recv().unwrap_or(stopped_outcome(0))
    }

    /// Run a query and wait at most `timeout` for its answer. A late answer
    /// is cancelled and reported as `query_timeout`; queries have no side
    /// effects, so asking again is always safe.
    pub fn query_blocking(
        &self,
        invocation: Invocation,
        context: CallContext,
        timeout: Duration,
    ) -> CommandOutcome {
        let (sender, receiver) = channel();
        let cancellation = Cancellation::new();
        self.dispatch_cancellable(invocation, context, cancellation.clone(), move |outcome| {
            let _ = sender.send(outcome);
        });
        receiver.recv_timeout(timeout).unwrap_or_else(|error| {
            cancellation.cancel();
            CommandOutcome {
                status: CommandStatus::Completed,
                revision: self.snapshot().0,
                result: Err(match error {
                    std::sync::mpsc::RecvTimeoutError::Timeout => CommandError::new(
                        "query_timeout",
                        format!(
                            "the query did not answer within {} ms; it was cancelled",
                            timeout.as_millis()
                        ),
                    ),
                    std::sync::mpsc::RecvTimeoutError::Disconnected => {
                        CommandError::new("engine_stopped", "the engine has stopped")
                    }
                }),
            }
        })
    }

    /// Receive every state update, on the engine thread, until dropped.
    pub fn subscribe(
        &self,
        listener: impl Fn(&StateUpdate) + Send + Sync + 'static,
    ) -> Subscription {
        let id = self.shared.next_listener.fetch_add(1, Ordering::Relaxed);
        self.shared
            .listeners
            .lock()
            .expect("listener lock")
            .push((id, Arc::new(listener)));
        Subscription {
            shared: Arc::clone(&self.shared),
            id,
        }
    }

    /// Stop the engine thread after already queued commands.
    pub fn stop(&self) {
        let _ = self.send(Job::Stop);
    }
}

fn validate(descriptor: &ProtocolDescriptor, invocation: &Invocation) -> Result<(), CommandError> {
    let spec = descriptor
        .commands
        .get(invocation.command)
        .ok_or_else(|| CommandError::new("unknown_command", "unknown command index"))?;
    if invocation.params.len() != spec.params.len() {
        return Err(CommandError::invalid_params(format!(
            "`{}` expects {} parameters, got {}",
            spec.id,
            spec.params.len(),
            invocation.params.len()
        )));
    }
    for (value, param) in invocation.params.iter().zip(&spec.params) {
        value.check(&param.ty, &descriptor.types).map_err(|error| {
            CommandError::invalid_params(format!("`{}`: {}", param.name, error.message))
        })?;
    }
    Ok(())
}

/// The engine and the state it leaves for the next job.
struct Worker<E> {
    engine: E,
    shared: Arc<Shared>,
    poisoned: bool,
}

impl<E: EngineProtocol> Worker<E> {
    fn new(engine: E, shared: Arc<Shared>) -> Self {
        Self {
            engine,
            shared,
            poisoned: false,
        }
    }

    /// Run one job; `false` once the engine should stop.
    fn handle(&mut self, job: Job) -> bool {
        let Job::Invoke {
            invocation,
            context: _context,
            cancellation,
            reply,
            trace: traced,
        } = job
        else {
            return false;
        };
        let _scope = traced.as_ref().map(trace::JobScope::enter);
        let job_trace = traced.map(|job| (job.seq, job.catalog));
        let (engine, shared) = (&mut self.engine, &self.shared);
        let descriptor = shared.descriptor;
        let is_query = descriptor
            .commands
            .get(invocation.command)
            .is_some_and(|spec| spec.is_query());
        if is_query && !self.poisoned {
            let answer = QueryAnswer {
                reply: Some(reply),
                shared: Arc::clone(shared),
                command: invocation.command,
                cancellation,
                trace: job_trace,
            };
            if let Err(error) = validate(descriptor, &invocation) {
                answer.send(Err(error));
            } else if answer.is_cancelled() {
                // Superseded while queued: the provider never starts.
                drop(answer);
            } else {
                let started = job_trace.map(|_| trace::now_ns());
                let panicked =
                    catch_unwind(AssertUnwindSafe(|| engine.query(&invocation, answer))).is_err();
                if let Some(started) = started {
                    trace::job_record(
                        trace::kind::QUERY_START,
                        started,
                        invocation.command as u32,
                        0,
                        0,
                        0,
                    );
                }
                if panicked {
                    // The answer was dropped while unwinding and reported the failure.
                    self.poisoned = true;
                }
            }
            return true;
        }
        let poisoned = &mut self.poisoned;
        let started = job_trace.map(|_| trace::now_ns());
        let result = if *poisoned {
            Err(CommandError::new(
                "engine_failed",
                "the engine stopped accepting commands after an internal failure",
            ))
        } else {
            match validate(descriptor, &invocation) {
                Err(error) => Err(error),
                Ok(()) => match catch_unwind(AssertUnwindSafe(|| engine.invoke(&invocation))) {
                    Ok(result) => result.and_then(|value| {
                        let spec = &descriptor.commands[invocation.command];
                        value
                            .check(&spec.result, &descriptor.types)
                            .map_err(|error| {
                                CommandError::new(
                                    "invalid_result",
                                    format!("`{}` returned an invalid result: {error}", spec.id),
                                )
                            })?;
                        Ok(value)
                    }),
                    Err(_) => {
                        *poisoned = true;
                        Err(CommandError::new("engine_failed", "the command panicked"))
                    }
                },
            }
        };
        if let Some(started) = started {
            trace::job_record(
                trace::kind::EXEC,
                started,
                invocation.command as u32,
                result.is_ok() as u64,
                0,
                0,
            );
        }
        let revision = if *poisoned {
            shared.state.lock().expect("state lock").0
        } else if job_trace.is_some() {
            publish_traced(shared, &*engine)
        } else {
            publish(shared, engine.snapshot())
        };
        reply(CommandOutcome {
            status: CommandStatus::Completed,
            revision,
            result,
        });
        true
    }
}

/// Diff the new snapshot, bump the revision and notify listeners once.
fn publish(shared: &Shared, next: Vec<Value>) -> u64 {
    publish_with(shared, next, |_, previous, value| previous != Some(value)).0
}

/// `publish` for a traced job: the snapshot, each field's comparison and
/// the whole publication, listeners included, are recorded.
fn publish_traced(shared: &Shared, engine: &impl EngineProtocol) -> u64 {
    let started = trace::now_ns();
    let next = engine.snapshot();
    trace::job_record(trace::kind::SNAPSHOT, started, 0, 0, 0, 0);
    let (revision, changed) = publish_with(shared, next, |index, previous, value| {
        let compared = trace::now_ns();
        let differs = previous != Some(value);
        trace::job_record(
            trace::kind::COMPARE_FIELD,
            compared,
            index as u32,
            differs as u64,
            0,
            0,
        );
        differs
    });
    let created = if changed { revision } else { 0 };
    trace::job_record(trace::kind::PUBLISH, started, 0, created, 0, 0);
    revision
}

/// Publish `next`: the revision afterwards, and whether it is new.
fn publish_with(
    shared: &Shared,
    next: Vec<Value>,
    mut differs: impl FnMut(usize, Option<&Value>, &Value) -> bool,
) -> (u64, bool) {
    let update = {
        let mut state = shared.state.lock().expect("state lock");
        let changes: Vec<(usize, Value)> = next
            .iter()
            .enumerate()
            .filter(|(index, value)| differs(*index, state.1.get(*index), value))
            .map(|(index, value)| (index, value.clone()))
            .collect();
        if changes.is_empty() {
            return (state.0, false);
        }
        state.0 += 1;
        state.1 = next;
        StateUpdate {
            revision: state.0,
            changes,
        }
    };
    let listeners: Vec<Listener> = shared
        .listeners
        .lock()
        .expect("listener lock")
        .iter()
        .map(|(_, listener)| Arc::clone(listener))
        .collect();
    for listener in listeners {
        listener(&update);
    }
    (update.revision, true)
}

#[cfg(test)]
mod tests;
