//! What a module engine sends while it handles a command.
//!
//! A module engine reaches other modules only through this outbox, which
//! the code `#[bern::protocol]` generates writes to: commands for the
//! modules it depends on (`calls::…`), events for the modules depending on
//! it (`emit::…`), and the current state of the modules it depends on
//! (`current_state()`). The application engine runs what was sent once the
//! command that sent it has finished, one after another, so a command
//! never runs inside another.

use bern_protocol::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

/// A command or event waiting to run.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    /// Command `command` (its index in the module's own catalog) of the
    /// module `namespace`.
    Call {
        namespace: String,
        command: usize,
        params: Vec<Value>,
    },
    /// Event `event` of the module `namespace`.
    Event {
        namespace: String,
        event: String,
        params: Vec<Value>,
    },
}

#[derive(Default)]
struct Outbox {
    pending: VecDeque<Pending>,
    /// State of the modules the running module may read, by namespace.
    states: BTreeMap<String, Vec<Value>>,
}

thread_local! {
    static OUTBOX: RefCell<Outbox> = RefCell::new(Outbox::default());
}

/// Queue a command of `namespace` to run after the current one.
pub fn call(namespace: &str, command: usize, params: Vec<Value>) {
    OUTBOX.with(|outbox| {
        outbox.borrow_mut().pending.push_back(Pending::Call {
            namespace: namespace.to_string(),
            command,
            params,
        })
    });
}

/// Report `event` of `namespace` to the modules that answer it.
pub fn emit(namespace: &str, event: &str, params: Vec<Value>) {
    OUTBOX.with(|outbox| {
        outbox.borrow_mut().pending.push_back(Pending::Event {
            namespace: namespace.to_string(),
            event: event.to_string(),
            params,
        })
    });
}

/// The state fields of module `namespace`, while a module that may read it
/// handles a command; `None` elsewhere or while that module is off.
pub fn state_of(namespace: &str) -> Option<Vec<Value>> {
    OUTBOX.with(|outbox| outbox.borrow().states.get(namespace).cloned())
}

/// Make `states` readable while the next command runs.
pub(crate) fn begin(states: BTreeMap<String, Vec<Value>>) {
    OUTBOX.with(|outbox| outbox.borrow_mut().states = states);
}

/// Everything sent since the last call, which also ends state access.
pub(crate) fn take() -> Vec<Pending> {
    OUTBOX.with(|outbox| {
        let mut outbox = outbox.borrow_mut();
        outbox.states.clear();
        outbox.pending.drain(..).collect()
    })
}
