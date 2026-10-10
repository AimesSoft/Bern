//! Asynchronous query resources and clocks for scenarios.
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
//! [`QuerySubscriptions`] holds the live subscriptions of a UI session.
//! Each belongs to a window instance and ends with it; a subscription whose
//! query reads engine state is requested again when that state changes.
//!
//! Providers schedule delayed work through a [`Clock`]. [`SystemClock`]
//! uses real time; [`ManualClock`] only moves when a scenario advances it,
//! so the order of asynchronous completions is part of the test, not an
//! accident.

pub use bern_protocol::Cancellation;
use bern_protocol::{CommandError, Value};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

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
        let cancellation = Cancellation::new();
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

/// A live subscription to one catalog query, owned by a window instance.
#[derive(Debug)]
pub struct QuerySubscription {
    /// Window instance that owns it, such as `main#1`.
    pub window: String,
    /// Catalog index of the query.
    pub query: usize,
    /// State fields the query reads.
    reads: Vec<usize>,
    resource: Query<Vec<Value>, Value>,
}

impl QuerySubscription {
    pub fn state(&self) -> &Resource<Value> {
        self.resource.state()
    }

    pub fn version(&self) -> u64 {
        self.resource.version()
    }

    pub fn params(&self) -> Option<&Vec<Value>> {
        self.resource.params()
    }

    pub fn discarded(&self) -> u64 {
        self.resource.discarded()
    }
}

/// The query subscriptions of one UI session.
#[derive(Debug, Default)]
pub struct QuerySubscriptions {
    next: u64,
    entries: BTreeMap<u64, QuerySubscription>,
}

impl QuerySubscriptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a subscription to `query` for `window`. Nothing is requested
    /// until [`request`](Self::request) names the parameters.
    pub fn open(&mut self, window: impl Into<String>, query: usize, reads: Vec<usize>) -> u64 {
        self.next += 1;
        self.entries.insert(
            self.next,
            QuerySubscription {
                window: window.into(),
                query,
                reads,
                resource: Query::new(),
            },
        );
        self.next
    }

    pub fn get(&self, id: u64) -> Option<&QuerySubscription> {
        self.entries.get(&id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Request new parameters; the previous request is cancelled. `None`
    /// when the subscription has ended.
    pub fn request(&mut self, id: u64, params: Vec<Value>) -> Option<Ticket> {
        self.entries
            .get_mut(&id)
            .map(|entry| entry.resource.request(params))
    }

    /// Apply an answer if it is still current.
    pub fn resolve(
        &mut self,
        id: u64,
        ticket: &Ticket,
        result: Result<Value, CommandError>,
    ) -> Resolution {
        match self.entries.get_mut(&id) {
            Some(entry) => entry.resource.resolve(ticket, result),
            None => Resolution::Discarded,
        }
    }

    /// End a subscription; its request in flight is cancelled.
    pub fn close(&mut self, id: u64) -> bool {
        match self.entries.remove(&id) {
            Some(mut entry) => {
                entry.resource.end();
                true
            }
            None => false,
        }
    }

    /// End every subscription of a closed window instance.
    pub fn close_window(&mut self, window: &str) -> Vec<u64> {
        let ids: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.window == window)
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            self.close(*id);
        }
        ids
    }

    /// Engine state fields changed: every requested subscription whose
    /// query reads one of them is requested again with its parameters.
    /// Returns what to send to the provider.
    pub fn refresh(&mut self, changed: &[usize]) -> Vec<(u64, Ticket, Vec<Value>)> {
        let mut requests = Vec::new();
        for (id, entry) in &mut self.entries {
            if !entry.reads.iter().any(|field| changed.contains(field)) {
                continue;
            }
            let Some(params) = entry.resource.params().cloned() else {
                continue;
            };
            let ticket = entry.resource.request(params.clone());
            requests.push((*id, ticket, params));
        }
        requests
    }
}

type Timer = Box<dyn FnOnce() + Send>;

/// Time source for providers that answer later. Scenarios inject a
/// [`ManualClock`] to decide the order of completions.
pub trait Clock: Send + Sync + 'static {
    /// Milliseconds since the clock started.
    fn now_ms(&self) -> u64;
    /// Run `task` once `delay_ms` have passed, on a thread of the clock's
    /// choosing.
    fn after_ms(&self, delay_ms: u64, task: Timer);
}

/// A clock that only moves when a scenario advances it. Timers fire in
/// due order, ties in the order they were scheduled, on the thread that
/// advances the clock.
#[derive(Clone, Default)]
pub struct ManualClock {
    inner: Arc<Mutex<ClockState>>,
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
        self.inner.lock().expect("clock lock").now
    }

    /// Run `task` once the clock has advanced by `delay_ms`.
    pub fn after(&self, delay_ms: u64, task: impl FnOnce() + Send + 'static) {
        let mut state = self.inner.lock().expect("clock lock");
        let due = state.now + delay_ms;
        state.sequence += 1;
        let sequence = state.sequence;
        state.timers.push((due, sequence, Box::new(task)));
    }

    /// Timers not yet fired.
    pub fn pending(&self) -> usize {
        self.inner.lock().expect("clock lock").timers.len()
    }

    /// Move the clock forward, firing every timer that becomes due.
    /// Returns how many fired.
    pub fn advance(&self, delta_ms: u64) -> usize {
        let target = self.inner.lock().expect("clock lock").now + delta_ms;
        let mut fired = 0;
        loop {
            let next = {
                let mut state = self.inner.lock().expect("clock lock");
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
        self.inner.lock().expect("clock lock").now = target;
        fired
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now()
    }

    fn after_ms(&self, delay_ms: u64, task: Timer) {
        self.after(delay_ms, task);
    }
}

/// Real time. One timer thread runs while timers are pending and ends when
/// none are left.
#[derive(Clone)]
pub struct SystemClock {
    inner: Arc<SystemTimers>,
}

struct SystemTimers {
    start: Instant,
    queue: Mutex<TimerQueue>,
    wake: Condvar,
}

#[derive(Default)]
struct TimerQueue {
    sequence: u64,
    timers: Vec<(Instant, u64, Timer)>,
    running: bool,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SystemTimers {
                start: Instant::now(),
                queue: Mutex::new(TimerQueue::default()),
                wake: Condvar::new(),
            }),
        }
    }

    fn run(timers: Arc<SystemTimers>) {
        let mut queue = timers.queue.lock().expect("timer lock");
        loop {
            let now = Instant::now();
            let next = queue
                .timers
                .iter()
                .enumerate()
                .min_by_key(|(_, (due, sequence, _))| (*due, *sequence))
                .map(|(index, (due, _, _))| (index, *due));
            match next {
                None => {
                    queue.running = false;
                    return;
                }
                Some((index, due)) if due <= now => {
                    let (_, _, task) = queue.timers.remove(index);
                    drop(queue);
                    // A failing task must not stop the timers behind it.
                    let _ = catch_unwind(AssertUnwindSafe(task));
                    queue = timers.queue.lock().expect("timer lock");
                }
                Some((_, due)) => {
                    queue = timers
                        .wake
                        .wait_timeout(queue, due - now)
                        .expect("timer lock")
                        .0;
                }
            }
        }
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.inner.start.elapsed().as_millis() as u64
    }

    fn after_ms(&self, delay_ms: u64, task: Timer) {
        let mut queue = self.inner.queue.lock().expect("timer lock");
        queue.sequence += 1;
        let sequence = queue.sequence;
        queue.timers.push((
            Instant::now() + Duration::from_millis(delay_ms),
            sequence,
            task,
        ));
        if !queue.running {
            queue.running = true;
            let timers = Arc::clone(&self.inner);
            if std::thread::Builder::new()
                .name("bern-timers".into())
                .spawn(move || SystemClock::run(timers))
                .is_err()
            {
                queue.running = false;
            }
        }
        self.inner.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    /// A search provider whose latency the scenario chooses per query.
    struct Search {
        clock: ManualClock,
        query: Arc<Mutex<Query<String, Vec<String>>>>,
        items: Vec<&'static str>,
        cancelled: Arc<Mutex<Vec<String>>>,
    }

    impl Search {
        fn new(clock: &ManualClock) -> Self {
            Self {
                clock: clock.clone(),
                query: Arc::new(Mutex::new(Query::new())),
                items: vec!["Documents", "Downloads", "Movies", "Music", "Photos"],
                cancelled: Arc::default(),
            }
        }

        /// Type `text` into the search field; the answer takes `latency_ms`.
        fn type_text(&self, text: &str, latency_ms: u64) {
            let ticket = self.query.lock().unwrap().request(text.to_string());
            let query = Arc::clone(&self.query);
            let cancelled = Arc::clone(&self.cancelled);
            let text = text.to_lowercase();
            let results: Vec<String> = self
                .items
                .iter()
                .filter(|item| item.to_lowercase().contains(&text))
                .map(|item| item.to_string())
                .collect();
            self.clock.after(latency_ms, move || {
                if ticket.cancellation.is_cancelled() {
                    cancelled.lock().unwrap().push(text);
                }
                // A provider that cannot stop in time still answers; the
                // query drops what is no longer current.
                query.lock().unwrap().resolve(&ticket, Ok(results));
            });
        }

        fn state(&self) -> Resource<Vec<String>> {
            self.query.lock().unwrap().state().clone()
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
        assert_eq!(search.query.lock().unwrap().discarded(), 1);
        assert_eq!(*search.cancelled.lock().unwrap(), vec!["m".to_string()]);
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

    #[test]
    fn subscriptions_end_with_their_window_and_refresh_when_read_state_changes() {
        let mut subscriptions = QuerySubscriptions::new();
        let search = subscriptions.open("main#1", 4, vec![2]);
        let other = subscriptions.open("settings#1", 5, vec![]);
        // Nothing requested yet: a state change has nothing to refresh.
        assert!(subscriptions.refresh(&[2]).is_empty());
        let first = subscriptions
            .request(search, vec![Value::Text("m".into())])
            .unwrap();
        assert_eq!(
            subscriptions.resolve(search, &first, Ok(Value::Int(1))),
            Resolution::Applied
        );
        subscriptions.request(other, vec![]).unwrap();
        // Field 2 changed: only the subscription reading it asks again,
        // with the same parameters, keeping its data visible.
        let refreshed = subscriptions.refresh(&[0, 2]);
        assert_eq!(refreshed.len(), 1);
        let (id, ticket, params) = &refreshed[0];
        assert_eq!((*id, ticket.version), (search, 2));
        assert_eq!(params, &vec![Value::Text("m".into())]);
        assert_eq!(
            subscriptions.get(search).unwrap().state(),
            &Resource::Refreshing(Value::Int(1))
        );
        // The older answer no longer applies.
        assert_eq!(
            subscriptions.resolve(search, &first, Ok(Value::Int(9))),
            Resolution::Discarded
        );
        // Closing the window ends its subscriptions and cancels requests.
        assert_eq!(subscriptions.close_window("main#1"), vec![search]);
        assert!(ticket.cancellation.is_cancelled());
        assert_eq!(
            subscriptions.resolve(search, ticket, Ok(Value::Int(2))),
            Resolution::Discarded
        );
        assert_eq!(subscriptions.len(), 1);
        assert!(subscriptions.close(other));
        assert!(subscriptions.is_empty());
    }

    #[test]
    fn the_system_clock_fires_timers_in_due_order() {
        let clock = SystemClock::new();
        let (sender, receiver) = channel();
        for (delay, name) in [(40, "late"), (5, "early"), (20, "middle")] {
            let sender = sender.clone();
            clock.after_ms(delay, Box::new(move || sender.send(name).unwrap()));
        }
        let order: Vec<_> = (0..3)
            .map(|_| receiver.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        assert_eq!(order, vec!["early", "middle", "late"]);
        assert!(clock.now_ms() >= 40);
        // The timer thread ends when idle and starts again on demand.
        clock.after_ms(1, Box::new(move || sender.send("again").unwrap()));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            "again"
        );
    }
}
