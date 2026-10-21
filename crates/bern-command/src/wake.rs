//! Work the engine started elsewhere, coming back to it.
//!
//! A command that starts slow work, such as a network request or a scan of
//! a folder, gives it to another thread and returns at once. When the work
//! is done, that thread calls [`Waker::wake`]; the host then runs the
//! engine's `wake` on the engine thread, where the engine takes the results
//! into its state, and publishes the state as it does after a command.
//!
//! The engine finds its waker with [`waker`] while it runs a command, a
//! query or `wake` itself. The host also wakes every engine once as it
//! starts, so work the engine needs from the beginning can start there.
//! Wakes that arrive while one is waiting are merged into it: `wake` takes
//! every result that is ready, not one result per call.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Wakes one engine from any thread. Cloning shares it.
#[derive(Clone)]
pub struct Waker {
    /// Set while a wake is queued and the engine has not run it yet.
    pending: Arc<AtomicBool>,
    send: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl Waker {
    pub(crate) fn new(send: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            send: Arc::new(send),
        }
    }

    /// Ask the engine to take in what is ready. Returns `false` once the
    /// engine has stopped and nothing will run.
    pub fn wake(&self) -> bool {
        if self.pending.swap(true, Ordering::AcqRel) {
            return true;
        }
        let queued = (self.send)();
        if !queued {
            self.pending.store(false, Ordering::Release);
        }
        queued
    }

    /// Called by the host right before the engine's `wake` runs, so a wake
    /// sent while it runs queues another.
    pub(crate) fn taken(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Waker>> = const { RefCell::new(None) };
}

/// The waker of the engine whose command, query or `wake` is running on
/// this thread; `None` anywhere else.
pub fn waker() -> Option<Waker> {
    CURRENT.with(|current| current.borrow().clone())
}

/// Make `waker` the current one until the returned guard drops.
pub(crate) fn enter(waker: &Waker) -> Entered {
    let previous = CURRENT.with(|current| current.replace(Some(waker.clone())));
    Entered { previous }
}

pub(crate) struct Entered {
    previous: Option<Waker>,
}

impl Drop for Entered {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|current| *current.borrow_mut() = previous);
    }
}
