//! Answering read-only queries.
//!
//! A query declared in a `#[queries]` trait receives an [`Answer`] as its
//! last parameter. The provider answers once, immediately or later and
//! from any thread, for example after a [`Clock`] delay or when an
//! external source replies. A request that is no longer wanted — its UI
//! subscription moved on to newer parameters or closed — is cancelled;
//! providers may check [`Answer::is_cancelled`] and stop early, and any
//! answer sent after cancellation is discarded.

use crate::bind::BernValue;
use bern_command::QueryAnswer;
use bern_protocol::CommandError;
use std::marker::PhantomData;

pub use bern_runtime::query::{
    Cancellation, Clock, ManualClock, Query, QuerySubscriptions, Resolution, Resource, SystemClock,
    Ticket,
};

/// The pending answer of one query request, typed by the query's result.
#[must_use = "a query is answered by sending its `Answer`; dropping it fails the request"]
pub struct Answer<T> {
    inner: QueryAnswer,
    _result: PhantomData<fn(T)>,
}

impl<T: BernValue> Answer<T> {
    #[doc(hidden)]
    pub fn new(inner: QueryAnswer) -> Self {
        Self {
            inner,
            _result: PhantomData,
        }
    }

    /// Whether the requester no longer wants this answer.
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// The request's cancellation flag, for work that runs elsewhere.
    pub fn cancellation(&self) -> Cancellation {
        self.inner.cancellation()
    }

    /// Answer with a result or a structured error.
    pub fn send(self, result: Result<T, CommandError>) {
        self.inner.send(result.map(|value| value.to_value()));
    }

    /// Answer with a result.
    pub fn ok(self, value: T) {
        self.send(Ok(value));
    }

    /// Answer with an error.
    pub fn fail(self, error: CommandError) {
        self.send(Err(error));
    }
}
