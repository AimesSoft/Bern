use crate::{CommandError, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A validated command call addressed by catalog index.
#[derive(Clone, Debug, PartialEq)]
pub struct Invocation {
    pub command: usize,
    pub params: Vec<Value>,
}

/// Lifecycle of a command request. The first version runs commands to
/// completion before answering; the enum keeps room for long operations
/// that answer `Accepted` with a handle first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandStatus {
    Accepted,
    Completed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandOutcome {
    pub status: CommandStatus,
    /// State revision after the command finished, so callers can relate the
    /// result to subscription updates.
    pub revision: u64,
    pub result: Result<Value, CommandError>,
}

/// Changed state fields within one consistent revision.
#[derive(Clone, Debug, PartialEq)]
pub struct StateUpdate {
    pub revision: u64,
    pub changes: Vec<(usize, Value)>,
}

/// Signals a query provider that its answer is no longer wanted: the
/// subscription moved on to newer parameters or ended. Clones share the
/// same flag, so the requester and the provider see one request.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
