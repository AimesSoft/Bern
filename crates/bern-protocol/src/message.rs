use crate::{CommandError, Value};

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
