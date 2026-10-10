//! One engine instance, many entries.
//!
//! [`Host`] owns the application engine on a dedicated thread, validates
//! every invocation against the catalog at the receiving boundary, runs
//! commands one after another and publishes consistent, versioned state
//! updates. The UI, CLI, JSON channel and MCP server all call the same
//! host; none of them re-implements business logic.
//!
//! Queries are read-only. Their providers start on the engine thread in
//! arrival order, so they see the state left by the commands before them,
//! but they answer through a [`QueryAnswer`] that may be completed later
//! and from any thread. A query request carries a [`Cancellation`]; the
//! provider can stop early, and an answer that arrives after cancellation
//! is reported as `cancelled` instead of as a result.

use bern_protocol::{
    Cancellation, CommandError, CommandOutcome, CommandStatus, Invocation, ProtocolDescriptor,
    StateUpdate, Value,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
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

fn cancelled() -> CommandError {
    CommandError::new("cancelled", "the request was cancelled")
}

struct Shared {
    engine_id: String,
    descriptor: &'static ProtocolDescriptor,
    state: Mutex<(u64, Vec<Value>)>,
    listeners: Mutex<Vec<(u64, Listener)>>,
    next_listener: AtomicU64,
}

/// Handle to a running engine. Cloning shares the same instance.
#[derive(Clone)]
pub struct Host {
    shared: Arc<Shared>,
    jobs: Sender<Job>,
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
    /// Start the engine on its own thread.
    pub fn start(engine: impl EngineProtocol) -> Host {
        let descriptor = engine.descriptor();
        let initial = engine.snapshot();
        debug_assert_eq!(initial.len(), descriptor.state.len());
        let shared = Arc::new(Shared {
            engine_id: new_engine_id(),
            descriptor,
            state: Mutex::new((0, initial)),
            listeners: Mutex::new(Vec::new()),
            next_listener: AtomicU64::new(1),
        });
        let (jobs, receiver) = channel::<Job>();
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("bern-engine".into())
            .spawn(move || run(engine, worker, receiver))
            .expect("start engine thread");
        Host { shared, jobs }
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
        if let Err(error) = self.jobs.send(Job::Invoke {
            invocation,
            context,
            cancellation,
            reply,
        }) && let Job::Invoke { reply, .. } = error.0
        {
            reply(CommandOutcome {
                status: CommandStatus::Completed,
                revision: self.snapshot().0,
                result: Err(CommandError::new(
                    "engine_stopped",
                    "the engine has stopped",
                )),
            });
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
        receiver.recv().unwrap_or(CommandOutcome {
            status: CommandStatus::Completed,
            revision: 0,
            result: Err(CommandError::new(
                "engine_stopped",
                "the engine has stopped",
            )),
        })
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
        let _ = self.jobs.send(Job::Stop);
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

fn run(
    mut engine: impl EngineProtocol,
    shared: Arc<Shared>,
    receiver: std::sync::mpsc::Receiver<Job>,
) {
    let descriptor = shared.descriptor;
    let mut poisoned = false;
    while let Ok(job) = receiver.recv() {
        let Job::Invoke {
            invocation,
            context: _context,
            cancellation,
            reply,
        } = job
        else {
            break;
        };
        let is_query = descriptor
            .commands
            .get(invocation.command)
            .is_some_and(|spec| spec.is_query());
        if is_query && !poisoned {
            let answer = QueryAnswer {
                reply: Some(reply),
                shared: Arc::clone(&shared),
                command: invocation.command,
                cancellation,
            };
            if let Err(error) = validate(descriptor, &invocation) {
                answer.send(Err(error));
            } else if answer.is_cancelled() {
                // Superseded while queued: the provider never starts.
                drop(answer);
            } else if catch_unwind(AssertUnwindSafe(|| engine.query(&invocation, answer))).is_err()
            {
                // The answer was dropped while unwinding and reported the failure.
                poisoned = true;
            }
            continue;
        }
        let result = if poisoned {
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
                        poisoned = true;
                        Err(CommandError::new("engine_failed", "the command panicked"))
                    }
                },
            }
        };
        let revision = if poisoned {
            shared.state.lock().expect("state lock").0
        } else {
            publish(&shared, engine.snapshot())
        };
        reply(CommandOutcome {
            status: CommandStatus::Completed,
            revision,
            result,
        });
    }
}

/// Diff the new snapshot, bump the revision and notify listeners once.
fn publish(shared: &Shared, next: Vec<Value>) -> u64 {
    let update = {
        let mut state = shared.state.lock().expect("state lock");
        let changes: Vec<(usize, Value)> = next
            .iter()
            .enumerate()
            .filter(|(index, value)| state.1.get(*index) != Some(*value))
            .map(|(index, value)| (index, value.clone()))
            .collect();
        if changes.is_empty() {
            return state.0;
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
    update.revision
}

#[cfg(test)]
mod tests;
