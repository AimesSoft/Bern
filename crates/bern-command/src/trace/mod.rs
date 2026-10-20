//! Framework-level performance tracing.
//!
//! Every point that records first reads one category mask ([`on`]); with
//! tracing off that relaxed load is all it costs. With tracing on, a point
//! takes timestamps from the platform's monotonic clock ([`now_ns`]) and
//! appends a fixed-size record to its thread's ring. A collector — the
//! `bern-trace` thread, or the page's timer on WebAssembly — reads the
//! rings, matches records across threads by command token, job and state
//! revision, and turns them into summary lines, a timeline for
//! ui.perfetto.dev and the data `trace.report` answers with. Nothing on
//! the recording threads writes files or terminals.
//!
//! The engine side records here (queueing, running commands and queries,
//! the snapshot, comparison and encoding of state); the bridge adds what
//! crosses into the frontend, and frontends add the UI thread's side
//! through `bern_trace_ui`. Application code adds its own spans with
//! [`span!`](crate::trace_span).

mod clock;
mod collect;
mod report;
mod ring;
pub mod rules;
mod text;
mod timeline;

pub use clock::now_ns;
pub use report::{Distribution, Report};
pub use rules::Rules;
pub use ring::Record;

use crate::{CallContext, Entry};
use bern_protocol::{ProtocolDescriptor, Value};
use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

// ---------------------------------------------------------------------------
// Categories

/// Commands and queries in the engine: queueing, running, publishing.
pub const ENGINE: u32 = 1 << 0;
/// The UI thread's side: dispatching, receiving, applying, interactions.
pub const UI: u32 = 1 << 1;
/// Frame ticks and long frames.
pub const FRAME: u32 = 1 << 2;
/// `span!` in application code (also recorded with `ENGINE`).
pub const SPAN: u32 = 1 << 3;
/// Summaries of spring motion.
pub const MOTION: u32 = 1 << 4;
/// Every transform motion writes, printed line by line.
pub const MOTION_VALUES: u32 = 1 << 5;
/// Style dictionaries a frontend parses.
pub const STYLE: u32 = 1 << 6;
/// Generated views recomputed (reserved).
pub const VIEW: u32 = 1 << 7;
/// What `1` and `on` turn on.
pub const DEFAULT: u32 = ENGINE | UI | FRAME | SPAN;
/// What `all` turns on: everything but the line-by-line motion values.
pub const ALL: u32 = ENGINE | UI | FRAME | SPAN | MOTION | STYLE | VIEW;

static MASK: AtomicU32 = AtomicU32::new(0);

/// Whether any of `categories` is being recorded. The only cost of a trace
/// point while tracing is off.
#[inline(always)]
pub fn on(categories: u32) -> bool {
    MASK.load(Ordering::Relaxed) & categories != 0
}

/// The categories being recorded.
pub fn mask() -> u32 {
    MASK.load(Ordering::Relaxed)
}

/// Record kinds. The UI kinds are part of the C interface: frontends pass
/// these numbers to `bern_trace_ui`.
pub mod kind {
    /// A control's action ran: `key` interaction, name the control.
    pub const ACT: u16 = 1;
    /// The frontend encoded and dispatched a command: `key` token, `a`
    /// command index, `b` interaction.
    pub const DISPATCH: u16 = 2;
    /// The UI thread handled a state update: `key` revision, `a` bytes.
    pub const RECEIVE_STATE: u16 = 3;
    /// One field of it decoded and written: `key` revision, `a` field
    /// index, `b` bytes.
    pub const APPLY_FIELD: u16 = 4;
    /// The UI thread handled a command's reply: `key` token.
    pub const RECEIVE_REPLY: u16 = 5;
    /// The UI thread handled a query state: `key` subscription, `a`
    /// version.
    pub const RECEIVE_QUERY: u16 = 6;
    /// A frame: `t0` its tick, `t1` when its work ended, `a` the refresh
    /// interval and `b` when it is shown, both in ns (0 unknown).
    pub const FRAME: u16 = 7;
    /// Other UI-thread work: name it, `a` its category bit.
    pub const UI_SPAN: u16 = 8;

    /// `bern_app_dispatch` decoded the parameters: `key` token, `a`
    /// command.
    pub const DECODE: u16 = 20;
    /// A job waited in the queue: `t0` queued, `t1` started.
    pub const QUEUE: u16 = 21;
    pub const EXEC: u16 = 22;
    pub const QUERY_START: u16 = 23;
    /// A query answered: `a` 0 with a result, 1 with an error, 2
    /// cancelled.
    pub const QUERY_ANSWER: u16 = 24;
    pub const SNAPSHOT: u16 = 25;
    pub const SNAPSHOT_FIELD: u16 = 26;
    /// `a` 1 when the field changed.
    pub const COMPARE_FIELD: u16 = 27;
    /// `a` the new revision, 0 when nothing changed.
    pub const PUBLISH: u16 = 28;
    /// `a` revision, `b` bytes, `extra` list items + 1 (0 not a list).
    pub const ENCODE_FIELD: u16 = 29;
    /// The bridge called the frontend: `extra` which (`CALLBACK_*`), `key`
    /// revision, token or subscription, `a` version, `b` bytes.
    pub const CALLBACK: u16 = 30;
    /// A command a module sent ran inside the command: name its index.
    pub const MODULE: u16 = 31;
    /// A `span!`: `a`/`b` a static name's address and length, or name an
    /// interned one.
    pub const SPAN: u16 = 32;
    /// A request reached the engine host through a control channel.
    pub const ENTRY: u16 = 33;
}

pub const CALLBACK_STATE: u32 = 0;
pub const CALLBACK_REPLY: u32 = 1;
pub const CALLBACK_QUERY: u32 = 2;
/// A query answer the subscription no longer wanted.
pub const CALLBACK_DISCARDED: u32 = 3;

/// Where a request came from, as records carry it.
pub mod entry {
    pub const UI: u32 = 0;
    pub const CLI: u32 = 1;
    pub const JSON: u32 = 2;
    pub const MCP: u32 = 3;
    pub const TEST: u32 = 4;
    /// A query subscription of the UI: `origin` subscription, `aux`
    /// version.
    pub const QUERY: u32 = 5;

    pub fn name(entry: u32) -> &'static str {
        match entry {
            UI => "ui",
            CLI => "cli",
            JSON => "json",
            MCP => "mcp",
            TEST => "test",
            QUERY => "query",
            _ => "other",
        }
    }
}

/// The track of an engine that runs on its caller's thread, such as on a
/// web page, so the timeline shows it apart from the UI.
pub const LOCAL_ENGINE_TRACK: u32 = 0x4000_0000;

#[cfg(test)]
thread_local! {
    /// Clock reads on this thread, for tests of the off path.
    pub(crate) static CLOCK_READS: Cell<u64> = const { Cell::new(0) };
}

#[inline]
fn now() -> u64 {
    #[cfg(test)]
    CLOCK_READS.with(|reads| reads.set(reads.get() + 1));
    now_ns()
}

/// Append a record; the collector is woken once a ring is half full.
pub fn record(record: Record) {
    if ring::push(record) {
        wake();
    }
}

// ---------------------------------------------------------------------------
// Catalogs and names

static CATALOGS: RwLock<Vec<&'static ProtocolDescriptor>> = RwLock::new(Vec::new());

/// The id records name `descriptor` by; the same descriptor keeps its id.
pub fn register_catalog(descriptor: &'static ProtocolDescriptor) -> u16 {
    let mut catalogs = CATALOGS.write().unwrap_or_else(|error| error.into_inner());
    if let Some(index) = catalogs
        .iter()
        .position(|known| std::ptr::eq(*known, descriptor))
    {
        return index as u16 + 1;
    }
    catalogs.push(descriptor);
    let id = catalogs.len() as u16;
    drop(catalogs);
    filter::catalog_added(id);
    id
}

pub(crate) fn catalog(id: u16) -> Option<&'static ProtocolDescriptor> {
    let catalogs = CATALOGS.read().unwrap_or_else(|error| error.into_inner());
    catalogs.get((id as usize).checked_sub(1)?).copied()
}

struct Names {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

/// Names of controls and dynamic spans, so records carry a number.
static NAMES: Mutex<Option<Names>> = Mutex::new(None);
/// Distinct names kept; later ones share one.
const MAX_NAMES: usize = 65_536;

/// The number standing for `name`.
pub fn intern(name: &str) -> u32 {
    let mut names = NAMES.lock().unwrap_or_else(|error| error.into_inner());
    let names = names.get_or_insert_with(|| Names {
        ids: HashMap::new(),
        names: vec!["…".into()],
    });
    if let Some(id) = names.ids.get(name) {
        return *id;
    }
    if names.names.len() >= MAX_NAMES {
        return 0;
    }
    let id = names.names.len() as u32;
    names.names.push(name.to_string());
    names.ids.insert(name.to_string(), id);
    id
}

pub(crate) fn interned(id: u32) -> String {
    NAMES
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .and_then(|names| names.names.get(id as usize).cloned())
        .unwrap_or_default()
}

/// FNV-1a, for request ids that records carry as numbers.
pub fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Feature contracts by `Contract.Feature`, with their command ids, for
/// `feature:` rules.
static FEATURES: Mutex<Vec<(String, Vec<String>)>> = Mutex::new(Vec::new());

/// Make the application's feature contracts known to `feature:` rules.
pub fn register_features(features: Vec<(String, Vec<String>)>) {
    FEATURES
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .extend(features);
    filter::rebuild();
}

// ---------------------------------------------------------------------------
// Command filters

/// Which commands are recorded, by catalog, while rules filter them.
mod filter {
    use super::rules::{Rules, glob_matches};
    use super::{CATALOGS, FEATURES};
    use std::sync::RwLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    static FILTERING: AtomicBool = AtomicBool::new(false);
    static RULES: RwLock<Option<Rules>> = RwLock::new(None);
    /// Per catalog id - 1, whether each command is recorded.
    static TABLE: RwLock<Vec<Vec<bool>>> = RwLock::new(Vec::new());

    pub fn set(rules: Option<&Rules>) {
        *RULES.write().unwrap_or_else(|error| error.into_inner()) =
            rules.filter(|rules| rules.filters_commands()).cloned();
        rebuild();
    }

    pub fn rebuild() {
        let rules = RULES.read().unwrap_or_else(|error| error.into_inner());
        let Some(rules) = rules.as_ref() else {
            FILTERING.store(false, Ordering::Release);
            return;
        };
        let features = FEATURES.lock().unwrap_or_else(|error| error.into_inner());
        let featured: Vec<&String> = features
            .iter()
            .filter(|(name, _)| {
                rules.features.iter().any(|wanted| {
                    glob_matches(wanted, name)
                        || name
                            .rsplit_once('.')
                            .is_some_and(|(_, feature)| glob_matches(wanted, feature))
                })
            })
            .flat_map(|(_, commands)| commands)
            .collect();
        let catalogs = CATALOGS.read().unwrap_or_else(|error| error.into_inner());
        let table = catalogs
            .iter()
            .map(|descriptor| {
                descriptor
                    .commands
                    .iter()
                    .map(|spec| {
                        rules.commands.iter().any(|glob| glob_matches(glob, &spec.id))
                            || (spec.is_query()
                                && rules.queries.iter().any(|glob| glob_matches(glob, &spec.id)))
                            || featured.iter().any(|id| **id == spec.id)
                    })
                    .collect()
            })
            .collect();
        *TABLE.write().unwrap_or_else(|error| error.into_inner()) = table;
        FILTERING.store(true, Ordering::Release);
    }

    pub fn catalog_added(_: u16) {
        if FILTERING.load(Ordering::Acquire) {
            rebuild();
        }
    }

    pub fn wants(catalog: u16, command: usize) -> bool {
        if !FILTERING.load(Ordering::Acquire) {
            return true;
        }
        let table = TABLE.read().unwrap_or_else(|error| error.into_inner());
        (catalog as usize)
            .checked_sub(1)
            .and_then(|index| table.get(index))
            .and_then(|commands| commands.get(command))
            .copied()
            .unwrap_or(false)
    }
}

/// Whether a command of `catalog` is recorded under the current rules.
pub fn traces_command(catalog: u16, command: usize) -> bool {
    filter::wants(catalog, command)
}

// ---------------------------------------------------------------------------
// Jobs: what the engine records about one request

/// Where the next dispatch on this thread comes from, when the bridge
/// knows better than the call context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A UI command with its reply token.
    Token(u64),
    /// A request of a UI query subscription.
    Query { subscription: u64, version: u64 },
}

thread_local! {
    static ORIGIN: Cell<Option<Origin>> = const { Cell::new(None) };
    /// The job running on this thread: sequence number and catalog.
    static JOB: Cell<(u64, u16)> = const { Cell::new((0, 0)) };
    /// Nesting of snapshots: a module's state inside a composite is timed
    /// as the one field it forms.
    static SNAPSHOT_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Dispatch inside `body` as coming from `origin`.
pub fn with_origin<T>(origin: Origin, body: impl FnOnce() -> T) -> T {
    if !on(ENGINE) {
        return body();
    }
    let previous = ORIGIN.replace(Some(origin));
    let result = body();
    ORIGIN.set(previous);
    result
}

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

/// What a queued request carries for tracing.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JobTrace {
    pub seq: u64,
    pub catalog: u16,
    pub command: u32,
    pub queued: u64,
    pub entry: u32,
    pub origin: u64,
    pub aux: u64,
}

/// Start tracing a request being queued, if its command is recorded.
pub(crate) fn job(catalog: u16, command: usize, context: &CallContext) -> Option<JobTrace> {
    if !filter::wants(catalog, command) {
        return None;
    }
    let (entry, origin, aux) = match ORIGIN.get() {
        Some(Origin::Token(token)) => (entry::UI, token, 0),
        Some(Origin::Query {
            subscription,
            version,
        }) => (entry::QUERY, subscription, version),
        None => (
            match context.entry {
                Entry::Ui => entry::UI,
                Entry::Cli => entry::CLI,
                Entry::Json => entry::JSON,
                Entry::Mcp => entry::MCP,
                Entry::Test => entry::TEST,
            },
            hash(&context.request_id),
            0,
        ),
    };
    Some(JobTrace {
        seq: NEXT_JOB.fetch_add(1, Ordering::Relaxed),
        catalog,
        command: command as u32,
        queued: now(),
        entry,
        origin,
        aux,
    })
}

/// While a traced job runs on this thread; records the queueing on entry.
pub(crate) struct JobScope {
    previous: (u64, u16),
}

impl JobScope {
    pub fn enter(job: &JobTrace) -> Self {
        record(Record {
            t0: job.queued,
            t1: now(),
            key: job.seq,
            a: job.origin,
            b: job.aux,
            kind: kind::QUEUE,
            catalog: job.catalog,
            name: job.command,
            extra: job.entry,
            ..Record::default()
        });
        Self {
            previous: JOB.replace((job.seq, job.catalog)),
        }
    }
}

impl Drop for JobScope {
    fn drop(&mut self) {
        JOB.set(self.previous);
    }
}

/// The traced job running on this thread, if any: sequence and catalog.
pub fn current_job() -> Option<(u64, u16)> {
    let job = JOB.get();
    (job.0 != 0).then_some(job)
}

/// Record a span of the current job: `kind`, `name`, `a`, `b` and
/// `extra` as that kind defines them.
pub fn job_record(kind: u16, t0: u64, name: u32, a: u64, b: u64, extra: u32) {
    let (seq, catalog) = JOB.get();
    record(Record {
        t0,
        t1: now(),
        key: seq,
        a,
        b,
        kind,
        catalog,
        name,
        extra,
        ..Record::default()
    });
}

/// The engine's snapshot of field `index`, timed when the engine is
/// traced. Generated snapshots call it for each field.
pub fn snapshot_field(index: usize, value: impl FnOnce() -> Value) -> Value {
    let depth = SNAPSHOT_DEPTH.get();
    if depth > 0 || current_job().is_none() {
        return value();
    }
    let started = now();
    SNAPSHOT_DEPTH.set(depth + 1);
    let value = value();
    SNAPSHOT_DEPTH.set(depth);
    job_record(kind::SNAPSHOT_FIELD, started, index as u32, 0, 0, 0);
    value
}

/// Run a command a module sent, recorded inside the command that sent it.
pub fn module_call<T>(command: usize, body: impl FnOnce() -> T) -> T {
    if current_job().is_none() {
        return body();
    }
    let started = now();
    let result = body();
    job_record(kind::MODULE, started, command as u32, 0, 0, 0);
    result
}

/// Run an engine that works on its caller's thread, on its own track.
pub fn local_engine<T>(body: impl FnOnce() -> T) -> T {
    if !on(ENGINE | SPAN) {
        return body();
    }
    ring::on_track(LOCAL_ENGINE_TRACK, body)
}

// ---------------------------------------------------------------------------
// Spans

enum SpanName {
    Static(&'static str),
    Interned(u32),
}

/// A span of application code, recorded when dropped. Made by
/// [`span!`](crate::trace_span).
#[must_use = "a span measures until it is dropped; bind it with `let _span = …`"]
pub struct Span {
    active: Option<(u64, SpanName)>,
    kind: u16,
}

impl Span {
    #[inline]
    pub fn enter(name: &'static str) -> Span {
        if !on(ENGINE | SPAN) {
            return Span {
                active: None,
                kind: kind::SPAN,
            };
        }
        Span {
            active: Some((now(), SpanName::Static(name))),
            kind: kind::SPAN,
        }
    }

    #[inline]
    pub fn enter_with(name: impl FnOnce() -> String) -> Span {
        if !on(ENGINE | SPAN) {
            return Span {
                active: None,
                kind: kind::SPAN,
            };
        }
        Span {
            active: Some((now(), SpanName::Interned(intern(&name())))),
            kind: kind::SPAN,
        }
    }

    /// A request a control channel handles, from reading it to writing
    /// its answer: `name` says which entry and what.
    pub fn entry(name: impl FnOnce() -> String) -> Span {
        if !on(ENGINE) {
            return Span {
                active: None,
                kind: kind::ENTRY,
            };
        }
        Span {
            active: Some((now(), SpanName::Interned(intern(&name())))),
            kind: kind::ENTRY,
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        let Some((started, name)) = self.active.take() else {
            return;
        };
        let (a, b, name) = match name {
            SpanName::Static(text) => (text.as_ptr() as u64, text.len() as u64, 0),
            SpanName::Interned(id) => (0, 0, id),
        };
        record(Record {
            t0: started,
            t1: now(),
            key: JOB.get().0,
            a,
            b,
            kind: self.kind,
            name,
            ..Record::default()
        });
    }
}

/// Measure application code until the end of the scope:
/// `let _span = bern::trace::span!("reindex");`, or with a formatted name,
/// `span!("load {}", path)`, formatted only while tracing.
#[macro_export]
macro_rules! trace_span {
    ($name:literal) => {
        $crate::trace::Span::enter($name)
    };
    ($($format:tt)+) => {
        $crate::trace::Span::enter_with(|| ::std::format!($($format)+))
    };
}

/// A static span name from its record. The address and length came from a
/// `&'static str` in this process.
pub(crate) fn static_name(record: &Record) -> String {
    if record.a == 0 {
        return interned(record.name);
    }
    // SAFETY: `Span` stores the address and length of a `&'static str`,
    // which stays valid for the whole process.
    let bytes = unsafe { std::slice::from_raw_parts(record.a as *const u8, record.b as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

/// A UI-thread record from a frontend; `name` is interned for the kinds
/// that carry one.
#[allow(clippy::too_many_arguments)]
pub fn ui(kind: u16, key: u64, a: u64, b: u64, t0: u64, t1: u64, name: Option<&str>) {
    let name = name.map(intern).unwrap_or(0);
    record(Record {
        t0,
        t1: t1.max(t0),
        key,
        a,
        b,
        kind,
        name,
        ..Record::default()
    });
}

/// The bridge handed something to the frontend: `which` is a
/// `CALLBACK_*`, `key` the revision, token or subscription, `a` the query
/// version.
pub fn callback(which: u32, key: u64, a: u64, bytes: usize) {
    let at = now();
    record(Record {
        t0: at,
        t1: at,
        key,
        a,
        b: bytes as u64,
        kind: kind::CALLBACK,
        extra: which,
        ..Record::default()
    });
}

/// A line the collector prints as it is, such as a motion value.
pub fn text(line: impl Into<String>) {
    let mut texts = TEXTS.lock().unwrap_or_else(|error| error.into_inner());
    if texts.len() < 65_536 {
        texts.push((now(), line.into()));
    }
}

static TEXTS: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());

// ---------------------------------------------------------------------------
// Sessions

/// Where summary lines go: standard error unless the host says otherwise
/// (logcat on Android, the console on a web page).
pub type Sink = Arc<dyn Fn(&str) + Send + Sync>;

struct State {
    session: Option<collect::Collector>,
    /// The last session after it stopped, so `trace.report` still answers.
    last: Option<collect::Collector>,
    /// Rules a host set before the engine started.
    configured: Option<String>,
    dir: Option<PathBuf>,
    sink: Option<Sink>,
    listeners: Vec<Arc<dyn Fn(u32) + Send + Sync>>,
    sessions: u32,
}

static STATE: Mutex<State> = Mutex::new(State {
    session: None,
    last: None,
    configured: None,
    dir: None,
    sink: None,
    listeners: Vec::new(),
    sessions: 0,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

/// What a session that started records and where its timeline goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub rules: String,
    pub file: Option<String>,
}

/// What a session that stopped left.
#[derive(Clone, Debug, PartialEq)]
pub struct Stopped {
    pub files: Vec<String>,
    pub events: u64,
    pub dropped: u64,
    /// The timeline itself, when asked for.
    pub timeline: Option<String>,
}

impl Started {
    /// As a `TraceStarted` record.
    pub fn to_value(&self) -> Value {
        Value::Record(vec![
            Value::Text(self.rules.clone()),
            Value::Optional(self.file.clone().map(|file| Box::new(Value::Text(file)))),
        ])
    }
}

impl Stopped {
    /// As a `TraceStopped` record.
    pub fn to_value(&self) -> Value {
        Value::Record(vec![
            Value::List(self.files.iter().cloned().map(Value::Text).collect()),
            Value::Int(self.events.min(i64::MAX as u64) as i64),
            Value::Int(self.dropped.min(i64::MAX as u64) as i64),
            Value::Optional(
                self.timeline
                    .clone()
                    .map(|timeline| Box::new(Value::Text(timeline))),
            ),
        ])
    }
}

/// Rules and a directory for timelines, from a host that cannot set
/// environment variables (an Android intent extra, a page address).
/// Applied when the engine starts, or at once if it has.
pub fn configure(rules: Option<&str>, dir: Option<&str>) {
    let start_now = {
        let mut state = state();
        if let Some(dir) = dir.filter(|dir| !dir.is_empty()) {
            state.dir = Some(PathBuf::from(dir));
        }
        let rules = rules.map(str::trim).filter(|rules| !rules.is_empty());
        if AUTO_STARTED.load(Ordering::Acquire) {
            rules.map(str::to_string)
        } else {
            state.configured = rules.map(str::to_string);
            None
        }
    };
    if let Some(rules) = start_now {
        report_start(start(&rules));
    }
}

/// Send summary lines to `sink` instead of standard error.
pub fn set_sink(sink: Sink) {
    state().sink = Some(sink);
}

/// Call `listener` with the new mask whenever tracing starts or stops, so
/// frontends can mirror it.
pub fn on_mask_change(listener: Arc<dyn Fn(u32) + Send + Sync>) {
    state().listeners.push(listener);
}

static AUTO_STARTED: AtomicBool = AtomicBool::new(false);

/// Start tracing as the host configured or the environment asks, once per
/// process. Engines call it as they start.
pub fn auto_start() {
    if AUTO_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let configured = state().configured.take();
    let rules = configured.or_else(|| {
        rules::from_env(|name| std::env::var(name).ok()).or_else(system_property)
    });
    if let Some(rules) = rules {
        report_start(start(&rules));
    }
}

fn report_start(started: Result<Started, String>) {
    if let Err(error) = started {
        print(&format!("bern-trace: {error}"));
    }
}

/// `debug.bern.trace`, which `adb shell setprop` sets for apps started
/// without `bern run`.
#[cfg(target_os = "android")]
fn system_property() -> Option<String> {
    use std::os::raw::{c_char, c_int};
    unsafe extern "C" {
        fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
    }
    let mut value = [0u8; 92];
    // SAFETY: the name is NUL-terminated and `value` holds PROP_VALUE_MAX
    // bytes.
    let len = unsafe {
        __system_property_get(c"debug.bern.trace".as_ptr(), value.as_mut_ptr().cast())
    };
    let text = String::from_utf8_lossy(&value[..len.clamp(0, 91) as usize]).into_owned();
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(not(target_os = "android"))]
fn system_property() -> Option<String> {
    None
}

fn print(line: &str) {
    let sink = state().sink.clone();
    emit(sink.as_ref(), &[line.to_string()]);
}

fn emit(sink: Option<&Sink>, lines: &[String]) {
    match sink {
        Some(sink) => lines.iter().for_each(|line| sink(line)),
        None => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                use std::io::Write;
                let mut stderr = std::io::stderr().lock();
                for line in lines {
                    let _ = writeln!(stderr, "{line}");
                }
            }
        }
    }
}

fn notify(listeners: Vec<Arc<dyn Fn(u32) + Send + Sync>>, mask: u32) {
    for listener in listeners {
        listener(mask);
    }
}

fn default_dir(state: &State) -> PathBuf {
    if let Some(dir) = &state.dir {
        return dir.clone();
    }
    // A page has no files; its timeline stays in memory under this name.
    if cfg!(target_arch = "wasm32") {
        return PathBuf::new();
    }
    std::env::var_os("BERN_IPC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// Start a session with `rules`, ending the one running.
pub fn start(rules: &str) -> Result<Started, String> {
    let parsed = Rules::parse(rules)?;
    let finished = stop_session(false);
    let mut state = state();
    state.sessions += 1;
    let file = match &parsed.file {
        rules::TimelineFile::None => None,
        rules::TimelineFile::Path(path) => Some(PathBuf::from(path)),
        rules::TimelineFile::Default => {
            let name = catalog(1)
                .map(|descriptor| descriptor.namespace.clone())
                .unwrap_or_else(|| "app".into());
            #[cfg(not(target_arch = "wasm32"))]
            let pid = std::process::id();
            #[cfg(target_arch = "wasm32")]
            let pid = 0;
            let suffix = if state.sessions > 1 {
                format!("-{}", state.sessions)
            } else {
                String::new()
            };
            Some(default_dir(&state).join(format!("bern-trace-{name}-{pid}{suffix}.json")))
        }
    };
    ring::begin(parsed.buffer);
    TEXTS.lock().unwrap_or_else(|error| error.into_inner()).clear();
    filter::set(Some(&parsed));
    let started_at = now();
    let mut lines = Vec::new();
    let timeline = file.as_ref().and_then(|path| match timeline::Timeline::create(path) {
        Ok(timeline) => Some(timeline),
        Err(error) => {
            lines.push(format!(
                "bern-trace: cannot write the timeline {}: {error}",
                path.display()
            ));
            None
        }
    });
    let written = timeline
        .as_ref()
        .and_then(|timeline| timeline.path().map(|path| path.display().to_string()));
    let normal = parsed.to_string();
    if !parsed.quiet {
        lines.insert(
            0,
            match &written {
                Some(path) => format!("bern-trace: on {normal}, timeline {path}"),
                None => format!("bern-trace: on {normal}"),
            },
        );
    }
    let mask = parsed.mask;
    state.session = Some(collect::Collector::new(parsed, started_at, timeline));
    let listeners = state.listeners.clone();
    let sink = state.sink.clone();
    drop(state);
    if let Some(finished) = finished {
        emit(sink.as_ref(), &finished.1);
    }
    emit(sink.as_ref(), &lines);
    MASK.store(mask, Ordering::Release);
    start_collector();
    notify(listeners, mask);
    Ok(Started {
        rules: normal,
        file: written,
    })
}

/// The rules of the running session, in their normal spelling.
pub fn rules() -> Option<String> {
    state().session.as_ref().map(|collector| collector.rules().to_string())
}

/// Stop the session; with `timeline`, the result carries the timeline.
pub fn stop(timeline: bool) -> Stopped {
    let finished = stop_session(timeline);
    let (listeners, sink) = {
        let state = state();
        (state.listeners.clone(), state.sink.clone())
    };
    let Some((stopped, lines)) = finished else {
        return Stopped {
            files: Vec::new(),
            events: 0,
            dropped: 0,
            timeline: None,
        };
    };
    emit(sink.as_ref(), &lines);
    notify(listeners, 0);
    stopped
}

/// End the running session: read what is left, finish what is open and
/// close the timeline. Returns the result and the lines to print.
fn stop_session(timeline: bool) -> Option<(Stopped, Vec<String>)> {
    MASK.store(0, Ordering::Release);
    let mut state = state();
    let mut collector = state.session.take()?;
    pump_into(&mut collector);
    let mut lines = collector.finish();
    let (files, inline) = collector.close_timeline(timeline);
    let stopped = Stopped {
        files,
        events: collector.events(),
        dropped: collector.dropped(),
        timeline: inline,
    };
    if !collector.quiet() {
        lines.push(format!(
            "bern-trace: off after {} events{}",
            stopped.events,
            if stopped.dropped > 0 {
                format!(", {} dropped (raise buffer:)", stopped.dropped)
            } else {
                String::new()
            }
        ));
    }
    lines.extend(collector.take_lines());
    filter::set(None);
    state.last = Some(collector);
    Some((stopped, lines))
}

/// What the running (or last) session measured. `top` bounds the slowest
/// interactions listed; `reset` starts the statistics over.
pub fn report(top: usize, reset: bool) -> Report {
    pump();
    let mut state = state();
    let state = &mut *state;
    let collector = state.session.as_mut().or(state.last.as_mut());
    match collector {
        Some(collector) => {
            let report = collector.report(top, now());
            if reset {
                collector.reset();
            }
            report
        }
        None => Report::default(),
    }
}

/// Read the rings and process what they hold. The collector thread calls
/// it; on a web page the page's timer does.
pub fn pump() {
    let mut state = state();
    let sink = state.sink.clone();
    let Some(collector) = state.session.as_mut() else {
        return;
    };
    pump_into(collector);
    let lines = collector.take_lines();
    drop(state);
    emit(sink.as_ref(), &lines);
}

fn pump_into(collector: &mut collect::Collector) {
    let mut records = Vec::new();
    let (dropped, threads) = ring::drain(&mut records);
    let texts = std::mem::take(&mut *TEXTS.lock().unwrap_or_else(|error| error.into_inner()));
    collector.process(records, texts, dropped, &threads, now());
}

#[cfg(not(target_arch = "wasm32"))]
static COLLECTOR: std::sync::OnceLock<std::thread::Thread> = std::sync::OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn start_collector() {
    let thread = COLLECTOR.get_or_init(|| {
        std::thread::Builder::new()
            .name("bern-trace".into())
            .spawn(|| {
                loop {
                    if MASK.load(Ordering::Acquire) == 0 {
                        std::thread::park();
                    } else {
                        std::thread::park_timeout(std::time::Duration::from_millis(20));
                    }
                    pump();
                }
            })
            .expect("start the trace collector")
            .thread()
            .clone()
    });
    thread.unpark();
}

#[cfg(target_arch = "wasm32")]
fn start_collector() {}

fn wake() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(thread) = COLLECTOR.get() {
        thread.unpark();
    }
}

#[cfg(test)]
mod tests;
