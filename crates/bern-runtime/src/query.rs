//! Asynchronous query resources and a controllable clock for scenarios.
//!
//! A [`Query`] is the display state of one query subscription: first load,
//! data, refresh and failure, so controls do not stitch booleans together.
//! Every request gets a new parameter version; only the result for the
//! current version is accepted. A slower answer to an older request — the
//! first of two searches typed in a row — is discarded instead of
//! overwriting the newer result, and the older request is cancelled. This
//! "current result only" rule is for queries; engine commands keep their own
//! retry and idempotency policies.
//!
//! [`ManualClock`] lets scenarios decide when each provider answers, so the
//! order of asynchronous completions is part of the test, not an accident.

use bern_protocol::CommandError;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// What a query shows. An empty result is `Ready` with empty data: success,
/// not failure.
#[derive(Clone, Debug, PartialEq)]
pub enum Resource<T> {
    /// Nothing requested yet.
    Idle,
    /// First load, no data to show.
    Loading,
    Ready(T),
    /// New parameters are loading; the previous data stays visible.
    Refreshing(T),
    Failed {
        error: CommandError,
        /// Data from the last success, still shown next to the error.
        previous: Option<T>,
    },
}

impl<T> Resource<T> {
    /// The data the UI can show, if any.
    pub fn data(&self) -> Option<&T> {
        match self {
            Resource::Ready(data) | Resource::Refreshing(data) => Some(data),
            Resource::Failed { previous, .. } => previous.as_ref(),
            Resource::Idle | Resource::Loading => None,
        }
    }
}

/// Signals a provider that its result is no longer wanted.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Identity of one request: the parameter version it was made for.
#[derive(Clone, Debug)]
pub struct Ticket {
    pub version: u64,
    pub cancellation: Cancellation,
}

/// Whether a result changed what the query shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Applied,
    /// The result belonged to an older version or an ended subscription.
    Discarded,
}

/// One query subscription with its parameter versions.
#[derive(Debug)]
pub struct Query<P, T> {
    params: Option<P>,
    version: u64,
    in_flight: Option<Cancellation>,
    state: Resource<T>,
    ended: bool,
    discarded: u64,
}

impl<P, T> Default for Query<P, T> {
    fn default() -> Self {
        Self {
            params: None,
            version: 0,
            in_flight: None,
            state: Resource::Idle,
            ended: false,
            discarded: 0,
        }
    }
}

impl<P: Clone, T: Clone> Query<P, T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &Resource<T> {
        &self.state
    }

    pub fn params(&self) -> Option<&P> {
        self.params.as_ref()
    }

    /// Version of the current parameters.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Results that arrived too late and were dropped.
    pub fn discarded(&self) -> u64 {
        self.discarded
    }

    /// Ask for `params`. The previous request is cancelled; data already
    /// shown stays visible while the new request loads.
    pub fn request(&mut self, params: P) -> Ticket {
        if let Some(previous) = self.in_flight.take() {
            previous.cancel();
        }
        self.version += 1;
        self.params = Some(params);
        let previous = self.state.data().cloned();
        self.state = match previous {
            Some(data) => Resource::Refreshing(data),
            None => Resource::Loading,
        };
        let cancellation = Cancellation::default();
        self.in_flight = Some(cancellation.clone());
        Ticket {
            version: self.version,
            cancellation,
        }
    }

    /// A provider answered `ticket`. Only the current version is applied.
    pub fn resolve(&mut self, ticket: &Ticket, result: Result<T, CommandError>) -> Resolution {
        if self.ended || ticket.version != self.version || ticket.cancellation.is_cancelled() {
            self.discarded += 1;
            return Resolution::Discarded;
        }
        self.in_flight = None;
        self.state = match result {
            Ok(data) => Resource::Ready(data),
            Err(error) => Resource::Failed {
                error,
                previous: self.state.data().cloned(),
            },
        };
        Resolution::Applied
    }

    /// The owner of the subscription ended, for example its page closed.
    /// The request in flight is cancelled and later results are dropped.
    pub fn end(&mut self) {
        if let Some(previous) = self.in_flight.take() {
            previous.cancel();
        }
        self.ended = true;
    }
}

type Timer = Box<dyn FnOnce()>;

/// A clock that only moves when a scenario advances it. Timers fire in
/// due order, ties in the order they were scheduled.
#[derive(Clone, Default)]
pub struct ManualClock {
    inner: Rc<RefCell<ClockState>>,
}

#[derive(Default)]
struct ClockState {
    now: u64,
    sequence: u64,
    timers: Vec<(u64, u64, Timer)>,
}

impl ManualClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Milliseconds since the scenario started.
    pub fn now(&self) -> u64 {
        self.inner.borrow().now
    }

    /// Run `task` once the clock has advanced by `delay_ms`.
    pub fn after(&self, delay_ms: u64, task: impl FnOnce() + 'static) {
        let mut state = self.inner.borrow_mut();
        let due = state.now + delay_ms;
        state.sequence += 1;
        let sequence = state.sequence;
        state.timers.push((due, sequence, Box::new(task)));
    }

    /// Move the clock forward, firing every timer that becomes due.
    /// Returns how many fired.
    pub fn advance(&self, delta_ms: u64) -> usize {
        let target = self.inner.borrow().now + delta_ms;
        let mut fired = 0;
        loop {
            let next = {
                let mut state = self.inner.borrow_mut();
                let due = state
                    .timers
                    .iter()
                    .enumerate()
                    .filter(|(_, (due, _, _))| *due <= target)
                    .min_by_key(|(_, (due, sequence, _))| (*due, *sequence))
                    .map(|(index, _)| index);
                due.map(|index| {
                    let (due, _, task) = state.timers.remove(index);
                    state.now = due;
                    task
                })
            };
            match next {
                Some(task) => {
                    task();
                    fired += 1;
                }
                None => break,
            }
        }
        self.inner.borrow_mut().now = target;
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A search provider whose latency the scenario chooses per query.
    struct Search {
        clock: ManualClock,
        query: Rc<RefCell<Query<String, Vec<String>>>>,
        items: Vec<&'static str>,
        cancelled: Rc<RefCell<Vec<String>>>,
    }

    impl Search {
        fn new(clock: &ManualClock) -> Self {
            Self {
                clock: clock.clone(),
                query: Rc::new(RefCell::new(Query::new())),
                items: vec!["Documents", "Downloads", "Movies", "Music", "Photos"],
                cancelled: Rc::default(),
            }
        }

        /// Type `text` into the search field; the answer takes `latency_ms`.
        fn type_text(&self, text: &str, latency_ms: u64) {
            let ticket = self.query.borrow_mut().request(text.to_string());
            let query = Rc::clone(&self.query);
            let cancelled = Rc::clone(&self.cancelled);
            let text = text.to_lowercase();
            let results: Vec<String> = self
                .items
                .iter()
                .filter(|item| item.to_lowercase().contains(&text))
                .map(|item| item.to_string())
                .collect();
            self.clock.after(latency_ms, move || {
                if ticket.cancellation.is_cancelled() {
                    cancelled.borrow_mut().push(text);
                }
                // A provider that cannot stop in time still answers; the
                // query drops what is no longer current.
                query.borrow_mut().resolve(&ticket, Ok(results));
            });
        }

        fn state(&self) -> Resource<Vec<String>> {
            self.query.borrow().state().clone()
        }
    }

    #[test]
    fn a_slow_older_search_never_overwrites_a_newer_one() {
        let clock = ManualClock::new();
        let search = Search::new(&clock);
        // "m" is slow, "mo" is fast: the answers arrive out of order.
        search.type_text("m", 300);
        search.type_text("mo", 100);
        assert_eq!(search.state(), Resource::Loading);
        assert_eq!(clock.advance(100), 1);
        assert_eq!(search.state(), Resource::Ready(vec!["Movies".to_string()]));
        assert_eq!(clock.advance(200), 1);
        // The late answer for "m" was dropped, and its request was cancelled.
        assert_eq!(search.state(), Resource::Ready(vec!["Movies".to_string()]));
        assert_eq!(search.query.borrow().discarded(), 1);
        assert_eq!(*search.cancelled.borrow(), vec!["m".to_string()]);
        assert_eq!(clock.now(), 300);
    }

    #[test]
    fn in_order_answers_still_show_only_the_current_search() {
        let clock = ManualClock::new();
        let search = Search::new(&clock);
        search.type_text("d", 100);
        clock.advance(100);
        assert_eq!(
            search.state(),
            Resource::Ready(vec!["Documents".to_string(), "Downloads".to_string()])
        );
        // While the next search loads, the previous results stay visible.
        search.type_text("do", 200);
        search.type_text("dow", 300);
        assert_eq!(
            search.state(),
            Resource::Refreshing(vec!["Documents".to_string(), "Downloads".to_string()])
        );
        clock.advance(200);
        assert!(matches!(search.state(), Resource::Refreshing(_)));
        clock.advance(100);
        assert_eq!(
            search.state(),
            Resource::Ready(vec!["Downloads".to_string()])
        );
        // An empty result is a successful answer, not a failure.
        search.type_text("zzz", 50);
        clock.advance(50);
        assert_eq!(search.state(), Resource::Ready(vec![]));
    }

    #[test]
    fn failures_keep_previous_data_and_ended_subscriptions_drop_results() {
        let mut query: Query<String, Vec<i64>> = Query::new();
        let first = query.request("a".into());
        assert_eq!(query.resolve(&first, Ok(vec![1])), Resolution::Applied);
        let second = query.request("b".into());
        query.resolve(&second, Err(CommandError::new("offline", "no network")));
        assert_eq!(query.state().data(), Some(&vec![1]));
        assert!(matches!(query.state(), Resource::Failed { .. }));
        // The page that owned the subscription closed.
        let third = query.request("c".into());
        query.end();
        assert!(third.cancellation.is_cancelled());
        assert_eq!(query.resolve(&third, Ok(vec![3])), Resolution::Discarded);
    }
}
