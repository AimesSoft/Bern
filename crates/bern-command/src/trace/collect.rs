//! The collector: it matches records across threads — a UI command by its
//! token, a job by its sequence number, a state update by its revision, a
//! query answer by subscription and version — and turns them into summary
//! lines, the timeline and the report.
//!
//! Records of one request arrive in any order and in different batches:
//! each thread has its own ring, read one after another. The collector
//! keeps what it learns in maps and decides whether a request is complete
//! only after a whole batch, so the order within a batch does not matter.

use super::report::{
    CommandReport, Distribution, FieldReport, FrameReport, FramesReport, InteractionReport,
    QueryReport, Report, Segment, SpanReport,
};
use super::ring::Record;
use super::rules::{Rules, glob_matches};
use super::text::{at, bytes, ms, parts};
use super::timeline::{Arg, Timeline};
use super::{
    CALLBACK_DISCARDED, CALLBACK_QUERY, CALLBACK_REPLY, CALLBACK_STATE, FRAME, LOCAL_ENGINE_TRACK,
    UI, catalog, entry, interned, kind, static_name,
};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

const SECOND: u64 = 1_000_000_000;
/// How long the UI side of a request may take to report once the engine
/// finished it.
const TIMEOUT: u64 = 2 * SECOND;
/// How long anything waits at all; a request whose records were dropped
/// is given up after this.
const ABANDON: u64 = 60 * SECOND;
/// How much history long frames are attributed from.
const RECENT: u64 = 4 * SECOND;
/// Samples kept per distribution.
const SAMPLES: usize = 4096;
const KEPT_FRAMES: usize = 50;
const KEPT_SLOWEST: usize = 50;
/// Blocks of one command printed per second before the rest are folded.
const FOLD_AFTER: usize = 10;
/// The long-frame threshold when the frontend does not know the refresh
/// interval.
const DEFAULT_LONG: u64 = 25_000_000;
/// A changed field this big, or this slow, is flagged.
const LARGE_BYTES: u64 = 64 * 1024;
const LARGE_NS: u64 = 1_000_000;
/// Tracks for frames are offset from the UI thread's.
const FRAMES_TRACK: u32 = 0x2000_0000;

#[derive(Clone, Default)]
struct Samples {
    recent: VecDeque<u64>,
    count: u64,
    max: u64,
}

impl Samples {
    fn add(&mut self, value: u64) {
        self.count += 1;
        self.max = self.max.max(value);
        if self.recent.len() == SAMPLES {
            self.recent.pop_front();
        }
        self.recent.push_back(value);
    }

    /// In `unit`s: 1000 for microseconds from nanoseconds, 1 for bytes.
    fn distribution(&self, unit: u64) -> Distribution {
        let mut sorted: Vec<u64> = self.recent.iter().copied().collect();
        sorted.sort_unstable();
        let rank = |quantile: f64| {
            if sorted.is_empty() {
                return 0;
            }
            let index = ((quantile * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
            sorted[index - 1]
        };
        Distribution {
            count: self.count,
            p50: rank(0.5) / unit,
            p95: rank(0.95) / unit,
            max: self.max / unit,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Seg {
    t0: u64,
    t1: u64,
    track: u32,
}

impl Seg {
    fn of(record: &Record) -> Seg {
        Seg {
            t0: record.t0,
            t1: record.t1,
            track: record.track,
        }
    }
    fn dur(&self) -> u64 {
        self.t1.saturating_sub(self.t0)
    }
    fn mid(&self) -> u64 {
        self.t0 + self.dur() / 2
    }
}

#[derive(Clone, Default)]
struct FieldTimes {
    snapshot: u64,
    compare: u64,
    encode: u64,
    changed: bool,
    sent: bool,
    bytes: u64,
    items: Option<u64>,
}

/// What the engine side recorded about one request.
#[derive(Clone, Default)]
struct Job {
    seq: u64,
    catalog: u16,
    command: u32,
    entry: u32,
    origin: u64,
    aux: u64,
    queued: Option<(u64, u64)>,
    exec: Option<Seg>,
    ok: bool,
    snapshot: Option<Seg>,
    publish: Option<Seg>,
    revision: u64,
    published: bool,
    answer: Option<(u64, u64)>,
    fields: BTreeMap<u32, FieldTimes>,
    modules: Vec<(u32, u64)>,
    spans: Vec<(String, u64)>,
    /// When the collector first heard of it, for giving up.
    seen: u64,
}

#[derive(Clone, Default)]
struct UiState {
    seg: Option<Seg>,
    /// Field, time, bytes.
    fields: Vec<(u32, u64, u64)>,
}

#[derive(Clone, Copy)]
struct Tick {
    track: u32,
    t1: u64,
    shown: u64,
}

impl Tick {
    /// When the frame is on screen, as well as it is known.
    fn on_screen(&self) -> u64 {
        if self.shown > 0 { self.shown } else { self.t1 }
    }
}

/// A finished request with everything matched to it.
#[derive(Clone, Default)]
struct Done {
    job: Job,
    dispatch: Option<Seg>,
    decode: Option<Seg>,
    interaction: u64,
    state_callback: Option<(u64, u64)>,
    reply_callback: Option<u64>,
    query_callback: Option<(u64, bool)>,
    ui_state: Option<UiState>,
    ui_reply: Option<Seg>,
    ui_query: Option<Seg>,
    frame: Option<Tick>,
    /// Bern's own UI-thread time between applying and the frame.
    bern_after: u64,
}

impl Done {
    fn start(&self) -> u64 {
        self.dispatch
            .map(|seg| seg.t0)
            .or(self.job.queued.map(|(queued, _)| queued))
            .unwrap_or(0)
    }

    fn engine_end(&self) -> u64 {
        if let Some((answered, _)) = self.job.answer {
            return answered;
        }
        self.job
            .publish
            .map(|seg| seg.t1)
            .or(self.job.exec.map(|seg| seg.t1))
            .unwrap_or(0)
    }

    fn applied(&self) -> Option<u64> {
        [
            self.ui_state.as_ref().and_then(|state| state.seg).map(|seg| seg.t1),
            self.ui_reply.map(|seg| seg.t1),
            self.ui_query.map(|seg| seg.t1),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    fn end(&self) -> u64 {
        self.frame
            .map(|tick| tick.on_screen())
            .or(self.applied())
            .unwrap_or_else(|| self.engine_end())
    }

    fn total(&self) -> u64 {
        self.end().saturating_sub(self.start())
    }

    fn queue(&self) -> Option<u64> {
        self.job.queued.map(|(queued, started)| started.saturating_sub(queued))
    }

    fn to_ui(&self) -> Option<u64> {
        if self.job.answer.is_some() {
            let (called, _) = self.query_callback?;
            return Some(self.ui_query?.t0.saturating_sub(called));
        }
        let (called, _) = self.state_callback?;
        Some(self.ui_state.as_ref()?.seg?.t0.saturating_sub(called))
    }

    fn apply(&self) -> Option<u64> {
        if self.job.answer.is_some() {
            return self.ui_query.map(|seg| seg.dur());
        }
        self.ui_state.as_ref()?.seg.map(|seg| seg.dur())
    }

    fn to_frame(&self) -> Option<u64> {
        Some(self.frame?.on_screen().saturating_sub(self.applied()?))
    }

    fn bytes(&self) -> u64 {
        self.job.fields.values().map(|field| field.bytes).sum()
    }
}

struct Interaction {
    control: Option<String>,
    act: Option<Seg>,
    tokens: Vec<u64>,
    done: Vec<Done>,
    frame: Option<Tick>,
    bern_after: u64,
    seen: u64,
}

struct PendingLong {
    track: u32,
    from: u64,
    to: u64,
    refresh: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Recent {
    track: u32,
    t0: u64,
    t1: u64,
    kind: u16,
    catalog: u16,
    name: u32,
    key: u64,
    a: u64,
}

#[derive(Default)]
struct CommandStats {
    total: Samples,
    queue: Samples,
    exec: Samples,
    publish: Samples,
    apply: Samples,
    bytes: Samples,
}

#[derive(Default)]
struct QueryStats {
    cancelled: u64,
    queue: Samples,
    provider: Samples,
    round_trip: Samples,
}

#[derive(Default)]
struct FieldStats {
    sent: u64,
    bytes_max: u64,
    items_max: Option<u64>,
    snapshot: Samples,
    compare: Samples,
    encode: Samples,
    apply: Samples,
}

struct Fold {
    since: u64,
    totals: Samples,
}

#[derive(Default)]
struct Stats {
    commands: BTreeMap<(u16, u32, u32), CommandStats>,
    queries: BTreeMap<(u16, u32), QueryStats>,
    fields: BTreeMap<(u16, u32), FieldStats>,
    spans: BTreeMap<String, Samples>,
    spacing: Samples,
    long: u64,
    long_frames: VecDeque<FrameReport>,
    slowest: Vec<InteractionReport>,
}

pub(super) struct Collector {
    rules: Rules,
    started: u64,
    events: u64,
    dropped: u64,
    timeline: Option<Timeline>,
    tracks: HashSet<u32>,
    thread_names: HashMap<u32, String>,
    ui_seen: bool,
    frames_seen: bool,
    jobs: BTreeMap<u64, Job>,
    by_token: HashMap<u64, u64>,
    by_revision: HashMap<u64, u64>,
    dispatches: HashMap<u64, (Seg, u64)>,
    /// The bridge's decoding of a UI command, and whether a job of it is
    /// traced in this process.
    decodes: HashMap<u64, (Seg, bool)>,
    state_callbacks: HashMap<u64, (u64, u64)>,
    reply_callbacks: HashMap<u64, u64>,
    query_callbacks: HashMap<(u64, u64), (u64, bool)>,
    ui_states: HashMap<u64, UiState>,
    ui_replies: HashMap<u64, Seg>,
    ui_queries: HashMap<(u64, u64), Seg>,
    interactions: BTreeMap<u64, Interaction>,
    ticks: VecDeque<Tick>,
    last_tick: HashMap<u32, u64>,
    pending_long: Vec<PendingLong>,
    recent: VecDeque<Recent>,
    stats: Stats,
    printed: HashMap<(u16, u32), VecDeque<u64>>,
    folds: BTreeMap<(u16, u32), Fold>,
    lines: Vec<String>,
}

fn command_id(catalog_id: u16, command: u32) -> String {
    catalog(catalog_id)
        .and_then(|descriptor| descriptor.commands.get(command as usize))
        .map(|spec| spec.id.clone())
        .unwrap_or_else(|| format!("command {command}"))
}

fn field_name(catalog_id: u16, field: u32) -> String {
    catalog(catalog_id)
        .and_then(|descriptor| descriptor.state.get(field as usize))
        .map(|field| field.name.clone())
        .unwrap_or_else(|| format!("field {field}"))
}

impl Collector {
    pub fn new(rules: Rules, started: u64, timeline: Option<Timeline>) -> Self {
        Self {
            rules,
            started,
            events: 0,
            dropped: 0,
            timeline,
            tracks: HashSet::new(),
            thread_names: HashMap::new(),
            ui_seen: false,
            frames_seen: false,
            jobs: BTreeMap::new(),
            by_token: HashMap::new(),
            by_revision: HashMap::new(),
            dispatches: HashMap::new(),
            decodes: HashMap::new(),
            state_callbacks: HashMap::new(),
            reply_callbacks: HashMap::new(),
            query_callbacks: HashMap::new(),
            ui_states: HashMap::new(),
            ui_replies: HashMap::new(),
            ui_queries: HashMap::new(),
            interactions: BTreeMap::new(),
            ticks: VecDeque::new(),
            last_tick: HashMap::new(),
            pending_long: Vec::new(),
            recent: VecDeque::new(),
            stats: Stats::default(),
            printed: HashMap::new(),
            folds: BTreeMap::new(),
            lines: Vec::new(),
        }
    }

    pub fn events(&self) -> u64 {
        self.events
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn quiet(&self) -> bool {
        self.rules.quiet
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    pub fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    fn rel(&self, t: u64) -> u64 {
        t.saturating_sub(self.started)
    }

    fn print(&mut self, lines: Vec<String>) {
        if !self.rules.quiet {
            self.lines.extend(lines);
        }
    }

    // -----------------------------------------------------------------------
    // Reading records

    pub fn process(
        &mut self,
        mut records: Vec<Record>,
        texts: Vec<(u64, String)>,
        dropped: u64,
        threads: &[(u32, String)],
        now: u64,
    ) {
        self.dropped = dropped;
        for (track, name) in threads {
            self.thread_names.insert(*track, name.clone());
        }
        // Lines asked for one by one, such as motion values, print even
        // when summaries are quiet.
        self.lines.extend(texts.into_iter().map(|(_, line)| line));
        records.sort_by_key(|record| (record.t0, record.t1));
        self.events += records.len() as u64;
        for record in &records {
            self.handle(record);
            self.draw(record);
        }
        self.sweep(now, false);
    }

    fn job(&mut self, seq: u64, now: u64) -> &mut Job {
        self.jobs.entry(seq).or_insert_with(|| Job {
            seq,
            seen: now,
            ..Job::default()
        })
    }

    fn remember(&mut self, record: &Record) {
        self.recent.push_back(Recent {
            track: record.track,
            t0: record.t0,
            t1: record.t1,
            kind: record.kind,
            catalog: record.catalog,
            name: record.name,
            key: record.key,
            a: record.a,
        });
    }

    fn handle(&mut self, record: &Record) {
        let seg = Seg::of(record);
        let seen = record.t1;
        match record.kind {
            kind::QUEUE => {
                let job = self.job(record.key, seen);
                job.catalog = record.catalog;
                job.command = record.name;
                job.entry = record.extra;
                job.origin = record.a;
                job.aux = record.b;
                job.queued = Some((record.t0, record.t1));
                if record.extra == entry::UI {
                    self.by_token.insert(record.a, record.key);
                }
            }
            kind::EXEC => {
                let job = self.job(record.key, seen);
                job.exec = Some(seg);
                job.ok = record.a == 1;
                self.remember(record);
            }
            kind::QUERY_START => {
                self.job(record.key, seen).exec = Some(seg);
                self.remember(record);
            }
            kind::QUERY_ANSWER => {
                self.job(record.key, seen).answer = Some((record.t1, record.a));
            }
            kind::SNAPSHOT => self.job(record.key, seen).snapshot = Some(seg),
            kind::SNAPSHOT_FIELD => {
                self.job(record.key, seen)
                    .fields
                    .entry(record.name)
                    .or_default()
                    .snapshot += seg.dur();
            }
            kind::COMPARE_FIELD => {
                let field = self
                    .job(record.key, seen)
                    .fields
                    .entry(record.name)
                    .or_default();
                field.compare += seg.dur();
                field.changed |= record.a == 1;
            }
            kind::ENCODE_FIELD => {
                let field = self
                    .job(record.key, seen)
                    .fields
                    .entry(record.name)
                    .or_default();
                field.encode += seg.dur();
                field.sent = true;
                field.bytes += record.b;
                field.items = record.extra.checked_sub(1).map(u64::from);
                if record.a > 0 {
                    self.by_revision.insert(record.a, record.key);
                }
            }
            kind::PUBLISH => {
                let job = self.job(record.key, seen);
                job.publish = Some(seg);
                job.revision = record.a;
                job.published = true;
                if record.a > 0 {
                    self.by_revision.insert(record.a, record.key);
                }
                self.remember(record);
            }
            kind::MODULE => {
                self.job(record.key, seen)
                    .modules
                    .push((record.name, seg.dur()));
            }
            kind::SPAN => {
                let name = static_name(record);
                self.stats
                    .spans
                    .entry(name.clone())
                    .or_default()
                    .add(seg.dur());
                if record.key != 0 {
                    self.job(record.key, seen).spans.push((name, seg.dur()));
                }
                self.remember(record);
            }
            kind::CALLBACK => match record.extra {
                CALLBACK_STATE => {
                    self.state_callbacks.insert(record.key, (record.t0, record.b));
                }
                CALLBACK_REPLY => {
                    self.reply_callbacks.insert(record.key, record.t0);
                }
                CALLBACK_QUERY | CALLBACK_DISCARDED => {
                    self.query_callbacks.insert(
                        (record.key, record.a),
                        (record.t0, record.extra == CALLBACK_DISCARDED),
                    );
                }
                _ => {}
            },
            kind::DECODE => {
                self.decodes.insert(record.key, (seg, record.b == 1));
            }
            kind::ENTRY => self.remember(record),
            kind::ACT => {
                self.ui_seen = true;
                let control = interned(record.name);
                let interaction = self.interaction(record.key, seen);
                interaction.act = Some(seg);
                interaction.control = Some(control);
                self.remember(record);
            }
            kind::DISPATCH => {
                self.ui_seen = true;
                self.dispatches.insert(record.key, (seg, record.b));
                if record.b != 0 {
                    self.interaction(record.b, seen).tokens.push(record.key);
                }
                self.remember(record);
            }
            kind::RECEIVE_STATE => {
                self.ui_seen = true;
                self.ui_states.entry(record.key).or_default().seg = Some(seg);
                self.remember(record);
            }
            kind::APPLY_FIELD => {
                self.ui_states.entry(record.key).or_default().fields.push((
                    record.a as u32,
                    seg.dur(),
                    record.b,
                ));
            }
            kind::RECEIVE_REPLY => {
                self.ui_seen = true;
                self.ui_replies.insert(record.key, seg);
                self.remember(record);
            }
            kind::RECEIVE_QUERY => {
                self.ui_seen = true;
                self.ui_queries.insert((record.key, record.a), seg);
                self.remember(record);
            }
            kind::UI_SPAN => {
                let name = interned(record.name);
                self.stats.spans.entry(name).or_default().add(seg.dur());
                self.remember(record);
            }
            kind::FRAME => self.frame(record),
            _ => {}
        }
    }

    fn interaction(&mut self, id: u64, seen: u64) -> &mut Interaction {
        self.interactions.entry(id).or_insert_with(|| Interaction {
            control: None,
            act: None,
            tokens: Vec::new(),
            done: Vec::new(),
            frame: None,
            bern_after: 0,
            seen,
        })
    }

    fn long_threshold(&self, refresh: u64) -> u64 {
        self.rules.long_ns.unwrap_or(if refresh > 0 {
            refresh * 3 / 2
        } else {
            DEFAULT_LONG
        })
    }

    fn frame(&mut self, record: &Record) {
        self.frames_seen = true;
        self.ticks.push_back(Tick {
            track: record.track,
            t1: record.t1,
            shown: record.b,
        });
        if let Some(previous) = self.last_tick.insert(record.track, record.t0) {
            let spacing = record.t0.saturating_sub(previous);
            self.stats.spacing.add(spacing);
            if spacing > self.long_threshold(record.a) {
                self.stats.long += 1;
                self.pending_long.push(PendingLong {
                    track: record.track,
                    from: previous,
                    to: record.t0,
                    refresh: record.a,
                });
            }
        }
    }

    // -----------------------------------------------------------------------
    // Matching and finishing

    fn sweep(&mut self, now: u64, finishing: bool) {
        self.resolve_long_frames(now, finishing);
        let seqs: Vec<u64> = self.jobs.keys().copied().collect();
        for seq in seqs {
            match self.readiness(seq, now, finishing) {
                Readiness::Wait => {}
                Readiness::Ready => self.complete(seq),
                Readiness::Abandon => {
                    self.jobs.remove(&seq);
                }
            }
        }
        let ids: Vec<u64> = self.interactions.keys().copied().collect();
        for id in ids {
            self.complete_interaction(id, now, finishing);
        }
        self.prune(now, finishing);
        self.flush_folds(now, finishing);
    }

    /// The first frame whose work ended after `after`, if frames are
    /// being watched.
    fn frame_after(&self, after: u64) -> Option<Tick> {
        self.ticks.iter().find(|tick| tick.t1 >= after).copied()
    }

    fn wants_frames(&self) -> bool {
        self.rules.mask & FRAME != 0 && self.frames_seen
    }

    fn wants_ui(&self) -> bool {
        self.rules.mask & UI != 0 && self.ui_seen
    }

    /// Everything matched to job `seq` so far.
    fn view(&self, seq: u64) -> Option<Done> {
        let job = self.jobs.get(&seq)?.clone();
        let token = (job.entry == entry::UI).then_some(job.origin);
        let query_key = (job.entry == entry::QUERY).then_some((job.origin, job.aux));
        let dispatch = token.and_then(|token| self.dispatches.get(&token).copied());
        let mut done = Done {
            dispatch: dispatch.map(|(seg, _)| seg),
            interaction: dispatch.map(|(_, interaction)| interaction).unwrap_or(0),
            decode: token.and_then(|token| self.decodes.get(&token).map(|(seg, _)| *seg)),
            state_callback: (job.revision > 0)
                .then(|| self.state_callbacks.get(&job.revision).copied())
                .flatten(),
            reply_callback: token.and_then(|token| self.reply_callbacks.get(&token).copied()),
            query_callback: query_key.and_then(|key| self.query_callbacks.get(&key).copied()),
            ui_state: (job.revision > 0)
                .then(|| self.ui_states.get(&job.revision).cloned())
                .flatten(),
            ui_reply: token.and_then(|token| self.ui_replies.get(&token).copied()),
            ui_query: query_key.and_then(|key| self.ui_queries.get(&key).copied()),
            job,
            ..Done::default()
        };
        if let Some(applied) = done.applied()
            && self.wants_frames()
        {
            done.frame = self.frame_after(applied);
            if let Some(frame) = done.frame {
                done.bern_after = self.bern_time(frame.track, applied, frame.t1);
            }
        }
        Some(done)
    }

    fn readiness(&self, seq: u64, now: u64, finishing: bool) -> Readiness {
        let Some(done) = self.view(seq) else {
            return Readiness::Abandon;
        };
        let job = &done.job;
        let engine_done = if job.answer.is_some() {
            true
        } else {
            job.published
        };
        if job.queued.is_none() || !engine_done {
            return if finishing || now.saturating_sub(job.seen) > ABANDON {
                Readiness::Abandon
            } else {
                Readiness::Wait
            };
        }
        if finishing {
            return Readiness::Ready;
        }
        let latest = [
            Some(done.engine_end()),
            done.state_callback.map(|(t, _)| t),
            done.reply_callback,
            done.applied(),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0);
        if now.saturating_sub(latest) > TIMEOUT {
            return Readiness::Ready;
        }
        if self.wants_ui() {
            let query = job.answer.is_some();
            if !query && job.revision > 0 && done.ui_state.as_ref().and_then(|s| s.seg).is_none() {
                return Readiness::Wait;
            }
            if !query && job.entry == entry::UI && done.ui_reply.is_none() {
                return Readiness::Wait;
            }
            if query
                && job.entry == entry::QUERY
                && !done.query_callback.is_some_and(|(_, discarded)| discarded)
                && done.ui_query.is_none()
            {
                return Readiness::Wait;
            }
        }
        if self.wants_frames() && done.applied().is_some() && done.frame.is_none() {
            return Readiness::Wait;
        }
        Readiness::Ready
    }

    /// Take job `seq` and what was matched to it out of the maps.
    fn complete(&mut self, seq: u64) {
        let Some(done) = self.view(seq) else { return };
        self.jobs.remove(&seq);
        let job = &done.job;
        if job.entry == entry::UI {
            self.by_token.remove(&job.origin);
            self.dispatches.remove(&job.origin);
            self.decodes.remove(&job.origin);
            self.reply_callbacks.remove(&job.origin);
            self.ui_replies.remove(&job.origin);
        }
        if job.entry == entry::QUERY {
            self.query_callbacks.remove(&(job.origin, job.aux));
            self.ui_queries.remove(&(job.origin, job.aux));
        }
        if job.revision > 0 {
            self.by_revision.remove(&job.revision);
            self.state_callbacks.remove(&job.revision);
            self.ui_states.remove(&job.revision);
        }
        self.flows(&done);
        if done.interaction != 0
            && let Some(interaction) = self.interactions.get_mut(&done.interaction)
        {
            interaction.done.push(done);
            return;
        }
        if !self.rules.controls.is_empty() {
            return;
        }
        self.finish_alone(done);
    }

    fn complete_interaction(&mut self, id: u64, now: u64, finishing: bool) {
        let Some(interaction) = self.interactions.get(&id) else {
            return;
        };
        let wants_ui = self.wants_ui();
        let mut applied = interaction.act.map(|seg| seg.t1);
        // A traced command still on its way, and a command without a job
        // here (filtered out, or run by a remote engine) whose reply the UI
        // has not handled yet.
        let mut engine_pending = false;
        let mut reply_pending = false;
        for token in &interaction.tokens {
            if let Some(done) = interaction.done.iter().find(|done| done.job.origin == *token) {
                applied = applied.max(done.applied());
                continue;
            }
            match self.decodes.get(token) {
                Some((_, true)) => engine_pending = true,
                _ => match self.ui_replies.get(token) {
                    Some(reply) => applied = applied.max(Some(reply.t1)),
                    None => reply_pending |= wants_ui,
                },
            }
        }
        let frame = applied.and_then(|applied| {
            self.wants_frames()
                .then(|| self.frame_after(applied))
                .flatten()
        });
        let latest = applied.unwrap_or(interaction.seen);
        let ready = finishing
            || now.saturating_sub(interaction.seen) > ABANDON
            || (!engine_pending && now.saturating_sub(latest) > TIMEOUT)
            || (interaction.act.is_some()
                && !engine_pending
                && !reply_pending
                && (!self.wants_frames() || frame.is_some()));
        if !ready {
            return;
        }
        let mut interaction = self.interactions.remove(&id).expect("interaction");
        interaction.frame = frame;
        if let (Some(frame), Some(applied)) = (frame, applied) {
            interaction.bern_after = self.bern_time(frame.track, applied, frame.t1);
        }
        let control_ok = self.rules.controls.is_empty()
            || interaction.control.as_deref().is_some_and(|control| {
                self.rules
                    .controls
                    .iter()
                    .any(|glob| glob_matches(glob, control))
            });
        // An action whose commands were all filtered out says nothing.
        let filtered_out = self.rules.filters_commands() && interaction.done.is_empty();
        if !control_ok || filtered_out || interaction.act.is_none() {
            return;
        }
        self.finish_interaction(interaction, applied);
    }

    fn prune(&mut self, now: u64, finishing: bool) {
        let keep = |t: u64| !finishing && now.saturating_sub(t) <= ABANDON;
        self.dispatches.retain(|_, (seg, _)| keep(seg.t1));
        self.decodes.retain(|_, (seg, _)| keep(seg.t1));
        self.state_callbacks.retain(|_, (t, _)| keep(*t));
        self.reply_callbacks.retain(|_, t| keep(*t));
        self.query_callbacks.retain(|_, (t, _)| keep(*t));
        self.ui_states
            .retain(|_, state| state.seg.is_none_or(|seg| keep(seg.t1)));
        self.ui_replies.retain(|_, seg| keep(seg.t1));
        self.ui_queries.retain(|_, seg| keep(seg.t1));
        let horizon = now.saturating_sub(RECENT);
        while self.recent.front().is_some_and(|recent| recent.t1 < horizon) {
            self.recent.pop_front();
        }
        // Frames stay as long as a request may still wait for one.
        let horizon = self
            .jobs
            .values()
            .map(|job| job.seen)
            .chain(self.interactions.values().map(|interaction| interaction.seen))
            .min()
            .unwrap_or(now)
            .min(now.saturating_sub(RECENT));
        while self.ticks.front().is_some_and(|tick| tick.t1 < horizon) {
            self.ticks.pop_front();
        }
    }

    /// Bern's UI-thread time on `track` between `from` and `to`: the union
    /// of its segments there.
    fn bern_time(&self, track: u32, from: u64, to: u64) -> u64 {
        let mut spans: Vec<(u64, u64)> = self
            .recent
            .iter()
            .filter(|recent| recent.track == track && is_ui_kind(recent.kind))
            .map(|recent| (recent.t0.max(from), recent.t1.min(to)))
            .filter(|(t0, t1)| t1 > t0)
            .collect();
        spans.sort_unstable();
        let mut total = 0;
        let mut reach = 0;
        for (t0, t1) in spans {
            let t0 = t0.max(reach);
            if t1 > t0 {
                total += t1 - t0;
            }
            reach = reach.max(t1);
        }
        total
    }

    fn label(&self, recent: &Recent) -> String {
        match recent.kind {
            kind::ACT => format!("act {}", interned(recent.name)),
            kind::DISPATCH => format!("dispatch token {}", recent.key),
            kind::RECEIVE_STATE => format!("apply rev {}", recent.key),
            kind::RECEIVE_REPLY => format!("reply token {}", recent.key),
            kind::RECEIVE_QUERY => format!("query subscription {}", recent.key),
            kind::UI_SPAN => interned(recent.name),
            kind::PUBLISH => format!("publish rev {}", recent.a),
            kind::SPAN => format!("span {}", recent.key),
            kind::EXEC | kind::QUERY_START => self
                .jobs
                .get(&recent.key)
                .map(|job| command_id(job.catalog, job.command))
                .unwrap_or_else(|| "command".into()),
            kind::ENTRY => interned(recent.name),
            _ => "work".into(),
        }
    }

    /// The outermost segments on `track` within the window, longest
    /// first.
    fn top_segments(&self, track_ok: impl Fn(&Recent) -> bool, from: u64, to: u64) -> Vec<Segment> {
        let inside: Vec<&Recent> = self
            .recent
            .iter()
            .filter(|recent| track_ok(recent) && recent.t1 > from && recent.t0 < to)
            .collect();
        let mut segments: Vec<Segment> = inside
            .iter()
            .filter(|recent| {
                !inside.iter().any(|outer| {
                    outer.track == recent.track
                        && *outer != **recent
                        && outer.t0 <= recent.t0
                        && outer.t1 >= recent.t1
                        && (outer.t0, outer.t1) != (recent.t0, recent.t1)
                })
            })
            .map(|recent| Segment {
                name: self.label(recent),
                us: (recent.t1.min(to).saturating_sub(recent.t0.max(from))) / 1000,
            })
            .filter(|segment| segment.us > 0)
            .collect();
        segments.sort_by(|a, b| b.us.cmp(&a.us));
        segments.truncate(6);
        segments
    }

    fn resolve_long_frames(&mut self, now: u64, finishing: bool) {
        let pending = std::mem::take(&mut self.pending_long);
        for long in pending {
            // Engine records of the window may still be on their way.
            if !finishing && now < long.to + 50_000_000 {
                self.pending_long.push(long);
                continue;
            }
            let duration = long.to - long.from;
            let bern = self.bern_time(long.track, long.from, long.to);
            let ui_track = long.track;
            let segments = self.top_segments(
                |recent| recent.track == ui_track && is_ui_kind(recent.kind),
                long.from,
                long.to,
            );
            let engine = self.top_segments(
                |recent| recent.track != ui_track && !is_ui_kind(recent.kind),
                long.from,
                long.to,
            );
            let frame = FrameReport {
                at_ms: self.rel(long.to) / 1_000_000,
                us: duration / 1000,
                refresh_us: long.refresh / 1000,
                bern_us: bern / 1000,
                outside_us: duration.saturating_sub(bern) / 1000,
                segments,
                engine,
            };
            let list = |segments: &[Segment]| {
                segments
                    .iter()
                    .map(|segment| format!("{} {}", segment.name, ms(segment.us * 1000)))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut line = format!(
                "bern-trace: long frame {} ms at {}{}: Bern {}",
                ms(duration),
                at(self.rel(long.to)),
                if long.refresh > 0 {
                    format!(" (refresh {} ms)", ms(long.refresh))
                } else {
                    String::new()
                },
                ms(bern),
            );
            if !frame.segments.is_empty() {
                line.push_str(&format!(" ({})", list(&frame.segments)));
            }
            line.push_str(&format!(", outside Bern {}", ms(duration.saturating_sub(bern))));
            if !frame.engine.is_empty() {
                line.push_str(&format!("; engine meanwhile: {}", list(&frame.engine)));
            }
            self.print(vec![line]);
            if self.stats.long_frames.len() == KEPT_FRAMES {
                self.stats.long_frames.pop_front();
            }
            self.stats.long_frames.push_back(frame);
        }
    }

    // -----------------------------------------------------------------------
    // Statistics and summaries

    fn count(&mut self, done: &Done) {
        let job = &done.job;
        if job.answer.is_some() {
            let stats = self
                .stats
                .queries
                .entry((job.catalog, job.command))
                .or_default();
            if job.answer.is_some_and(|(_, outcome)| outcome == 2)
                || done.query_callback.is_some_and(|(_, discarded)| discarded)
            {
                stats.cancelled += 1;
            }
            if let Some(queue) = done.queue() {
                stats.queue.add(queue);
            }
            if let (Some((answered, _)), Some((_, started))) = (job.answer, job.queued) {
                stats.provider.add(answered.saturating_sub(started));
            }
            stats.round_trip.add(done.total());
            return;
        }
        let stats = self
            .stats
            .commands
            .entry((job.catalog, job.command, job.entry))
            .or_default();
        stats.total.add(done.total());
        if let Some(queue) = done.queue() {
            stats.queue.add(queue);
        }
        if let Some(exec) = job.exec {
            stats.exec.add(exec.dur());
        }
        if let Some(publish) = job.publish {
            stats.publish.add(publish.dur());
        }
        if let Some(apply) = done.apply() {
            stats.apply.add(apply);
        }
        stats.bytes.add(done.bytes());
        let applied: HashMap<u32, u64> = done
            .ui_state
            .iter()
            .flat_map(|state| state.fields.iter())
            .map(|(field, time, _)| (*field, *time))
            .collect();
        for (index, field) in &job.fields {
            if !field.changed && !field.sent {
                continue;
            }
            let stats = self.stats.fields.entry((job.catalog, *index)).or_default();
            stats.sent += 1;
            stats.bytes_max = stats.bytes_max.max(field.bytes);
            if let Some(items) = field.items {
                stats.items_max = Some(stats.items_max.unwrap_or(0).max(items));
            }
            stats.snapshot.add(field.snapshot);
            stats.compare.add(field.compare);
            stats.encode.add(field.encode);
            if let Some(apply) = applied.get(index) {
                stats.apply.add(*apply);
            }
        }
    }

    fn field_shown(&self, catalog_id: u16, field: u32) -> bool {
        self.rules.fields.is_empty() || {
            let name = field_name(catalog_id, field);
            self.rules.fields.iter().any(|glob| glob_matches(glob, &name))
        }
    }

    /// The lines of one job: its engine part and the fields it sent.
    fn job_lines(&self, done: &Done, indent: &str, head: String, with_frame: bool) -> Vec<String> {
        let job = &done.job;
        let mut lines = Vec::new();
        let query = job.answer.is_some();
        let mut first = head;
        let engine = parts(&[
            ("queue", done.queue()),
            (if query { "provider" } else { "exec" }, job.exec.map(|seg| seg.dur())),
            ("publish", job.publish.filter(|_| job.revision > 0).map(|seg| seg.dur())),
        ]);
        first.push_str(&engine);
        if job.revision > 0 {
            first.push_str(&format!(" -> rev {}", job.revision));
        }
        if !job.ok && job.exec.is_some() && !query {
            first.push_str(" (failed)");
        }
        lines.push(first);
        for (command, time) in &job.modules {
            lines.push(format!(
                "{indent}  then {} {}",
                command_id(job.catalog, *command),
                ms(*time)
            ));
        }
        for (name, time) in &job.spans {
            lines.push(format!("{indent}  span {name} {}", ms(*time)));
        }
        if job.revision > 0 {
            let applied: HashMap<u32, u64> = done
                .ui_state
                .iter()
                .flat_map(|state| state.fields.iter())
                .map(|(field, time, _)| (*field, *time))
                .collect();
            if let Some(snapshot) = job.snapshot {
                let publish = job.publish.map(|seg| seg.dur()).unwrap_or(0);
                let compare: u64 = job.fields.values().map(|field| field.compare).sum();
                let encode: u64 = job.fields.values().map(|field| field.encode).sum();
                lines.push(format!(
                    "{indent}  publish: snapshot {}, compare {}, encode {}, other {}",
                    ms(snapshot.dur()),
                    ms(compare),
                    ms(encode),
                    ms(publish.saturating_sub(snapshot.dur() + compare + encode))
                ));
            }
            let mut quiet = (0usize, 0u64, 0u64);
            let mut hidden = 0usize;
            let mut sent: Vec<(&u32, &FieldTimes)> = job
                .fields
                .iter()
                .filter(|(_, field)| field.changed || field.sent)
                .collect();
            sent.sort_by(|a, b| b.1.bytes.cmp(&a.1.bytes));
            for (index, field) in sent {
                if !self.field_shown(job.catalog, *index) {
                    hidden += 1;
                    continue;
                }
                let apply = applied.get(index).copied();
                let cost = field.snapshot + field.compare + field.encode + apply.unwrap_or(0);
                let mut line = format!("{indent}  {}", field_name(job.catalog, *index));
                if field.sent {
                    line.push_str(&format!(" {}", bytes(field.bytes)));
                }
                if let Some(items) = field.items {
                    line.push_str(&format!(", {items} items"));
                }
                if field.bytes >= LARGE_BYTES || cost >= LARGE_NS {
                    line.push_str(", sent whole (large)");
                }
                line.push_str(": ");
                line.push_str(&parts(&[
                    ("snapshot", Some(field.snapshot)),
                    ("compare", Some(field.compare)),
                    ("encode", Some(field.encode)),
                    ("apply", apply),
                ]));
                lines.push(line);
            }
            for field in job.fields.values().filter(|field| !field.changed && !field.sent) {
                quiet.0 += 1;
                quiet.1 += field.snapshot;
                quiet.2 += field.compare;
            }
            if hidden > 0 {
                lines.push(format!(
                    "{indent}  {hidden} more changed field{} (not in field:)",
                    if hidden == 1 { "" } else { "s" }
                ));
            }
            if quiet.0 > 0 {
                lines.push(format!(
                    "{indent}  {} unchanged field{}: snapshot {}, compare {}",
                    quiet.0,
                    if quiet.0 == 1 { "" } else { "s" },
                    ms(quiet.1),
                    ms(quiet.2)
                ));
            }
        }
        let ui = parts(&[
            ("to UI", done.to_ui()),
            ("apply", done.apply()),
        ]);
        let mut tail = ui;
        if let Some(to_frame) = done.to_frame().filter(|_| with_frame) {
            if !tail.is_empty() {
                tail.push_str(", ");
            }
            tail.push_str(&format!(
                "on screen +{} ({} outside Bern)",
                ms(to_frame),
                ms(to_frame.saturating_sub(done.bern_after))
            ));
        }
        if !tail.is_empty() {
            lines.push(format!("{indent}  {tail}"));
        }
        lines
    }

    fn slow_enough(&self, total: u64) -> bool {
        total >= self.rules.slow_ns
    }

    /// Whether a block of `key` should be folded into a per-second line.
    fn fold(&mut self, key: (u16, u32), at: u64, total: u64) -> bool {
        if let Some(fold) = self.folds.get_mut(&key) {
            fold.totals.add(total);
            return true;
        }
        let printed = self.printed.entry(key).or_default();
        while printed.front().is_some_and(|t| at.saturating_sub(*t) > SECOND) {
            printed.pop_front();
        }
        if printed.len() >= FOLD_AFTER {
            let mut totals = Samples::default();
            totals.add(total);
            self.folds.insert(key, Fold { since: at, totals });
            return true;
        }
        printed.push_back(at);
        false
    }

    fn flush_folds(&mut self, now: u64, finishing: bool) {
        let due: Vec<(u16, u32)> = self
            .folds
            .iter()
            .filter(|(_, fold)| finishing || now.saturating_sub(fold.since) >= SECOND)
            .map(|(key, _)| *key)
            .collect();
        for key in due {
            let fold = self.folds.remove(&key).expect("fold");
            let totals = fold.totals.distribution(1);
            let line = format!(
                "bern-trace: {} x{} more within a second, total p50 {}, p95 {}, max {} ms",
                command_id(key.0, key.1),
                totals.count,
                ms(totals.p50),
                ms(totals.p95),
                ms(totals.max)
            );
            self.print(vec![line]);
            self.printed.remove(&key);
        }
    }

    fn segments(dones: &[Done], interaction: Option<&Interaction>) -> Vec<Segment> {
        let sum = |value: &dyn Fn(&Done) -> Option<u64>| -> Option<u64> {
            let values: Vec<u64> = dones.iter().filter_map(|done| value(done)).collect();
            (!values.is_empty()).then(|| values.iter().sum())
        };
        let to_frame = match interaction {
            Some(interaction) => interaction.frame.and_then(|frame| {
                let applied = dones
                    .iter()
                    .filter_map(Done::applied)
                    .chain(interaction.act.map(|seg| seg.t1))
                    .max()?;
                Some(frame.on_screen().saturating_sub(applied))
            }),
            None => sum(&Done::to_frame),
        };
        [
            ("act", interaction.and_then(|i| i.act).map(|seg| seg.dur())),
            ("dispatch", sum(&|done| done.dispatch.or(done.decode).map(|seg| seg.dur()))),
            ("queue", sum(&Done::queue)),
            ("exec", sum(&|done| done.job.exec.map(|seg| seg.dur()))),
            (
                "publish",
                sum(&|done| done.job.publish.filter(|_| done.job.revision > 0).map(|seg| seg.dur())),
            ),
            ("to_ui", sum(&Done::to_ui)),
            ("apply", sum(&Done::apply)),
            ("to_frame", to_frame),
        ]
        .into_iter()
        .filter_map(|(name, ns)| {
            ns.map(|ns| Segment {
                name: name.into(),
                us: ns / 1000,
            })
        })
        .collect()
    }

    fn keep_slowest(&mut self, report: InteractionReport) {
        let slowest = &mut self.stats.slowest;
        let at = slowest
            .iter()
            .position(|kept| kept.total_us < report.total_us)
            .unwrap_or(slowest.len());
        if at < KEPT_SLOWEST {
            slowest.insert(at, report);
            slowest.truncate(KEPT_SLOWEST);
        }
    }

    fn finish_alone(&mut self, done: Done) {
        self.count(&done);
        let job = &done.job;
        let id = command_id(job.catalog, job.command);
        let total = done.total();
        let query = job.answer.is_some();
        self.keep_slowest(InteractionReport {
            control: None,
            entry: entry::name(job.entry).into(),
            commands: vec![id.clone()],
            at_ms: self.rel(done.start()) / 1_000_000,
            total_us: total / 1000,
            revision: (job.revision > 0).then_some(job.revision),
            segments: Self::segments(std::slice::from_ref(&done), None),
        });
        if !self.slow_enough(total) || self.fold((job.catalog, job.command), done.end(), total) {
            return;
        }
        let what = if query { "query" } else { "command" };
        let mut head = format!(
            "bern-trace: {what} {id} ({}) {} ms: ",
            entry::name(job.entry),
            ms(total)
        );
        if query && job.entry == entry::QUERY {
            head = format!(
                "bern-trace: query {id} (subscription {}, version {}) {} ms: ",
                job.origin,
                job.aux,
                ms(total)
            );
        }
        let lines = self.job_lines(&done, "", head, true);
        self.print(lines);
    }

    fn finish_interaction(&mut self, interaction: Interaction, applied: Option<u64>) {
        for done in &interaction.done {
            self.count(done);
        }
        let start = interaction
            .act
            .map(|seg| seg.t0)
            .into_iter()
            .chain(interaction.done.iter().map(Done::start))
            .min()
            .unwrap_or(0);
        let applied = applied.unwrap_or(start);
        let end = interaction
            .frame
            .map(|frame| frame.on_screen())
            .unwrap_or(applied)
            .max(interaction.done.iter().map(Done::engine_end).max().unwrap_or(0));
        let total = end.saturating_sub(start);
        let control = interaction.control.clone().unwrap_or_default();
        let commands: Vec<String> = interaction
            .done
            .iter()
            .map(|done| command_id(done.job.catalog, done.job.command))
            .collect();
        self.keep_slowest(InteractionReport {
            control: Some(control.clone()),
            entry: "ui".into(),
            commands: commands.clone(),
            at_ms: self.rel(start) / 1_000_000,
            total_us: total / 1000,
            revision: interaction
                .done
                .iter()
                .map(|done| done.job.revision)
                .max()
                .filter(|revision| *revision > 0),
            segments: Self::segments(&interaction.done, Some(&interaction)),
        });
        if !self.slow_enough(total) {
            return;
        }
        let mut lines = vec![format!(
            "bern-trace: interaction {control} {} ms{}",
            ms(total),
            interaction
                .act
                .map(|act| format!(" (action {})", ms(act.dur())))
                .unwrap_or_default()
        )];
        for done in &interaction.done {
            let job = &done.job;
            let head = format!(
                "  {}: {}",
                command_id(job.catalog, job.command),
                parts(&[(
                    "dispatch",
                    done.dispatch.or(done.decode).map(|seg| seg.dur())
                )])
            );
            let head = if head.ends_with(": ") {
                head
            } else {
                format!("{head}, ")
            };
            // The frame belongs to the whole interaction.
            lines.extend(self.job_lines(done, "  ", head, false));
        }
        if let Some(frame) = interaction.frame {
            let to_frame = frame.on_screen().saturating_sub(applied);
            lines.push(format!(
                "  on screen +{} ({} outside Bern)",
                ms(to_frame),
                ms(to_frame.saturating_sub(interaction.bern_after))
            ));
        }
        let untraced = interaction.tokens.len() - interaction.done.len();
        if untraced > 0 {
            lines.push(format!(
                "  {untraced} command{} without engine records here",
                if untraced == 1 { "" } else { "s" }
            ));
        }
        self.print(lines);
    }

    /// Finish everything open, as the session ends.
    pub fn finish(&mut self) -> Vec<String> {
        self.sweep(u64::MAX / 2, true);
        self.take_lines()
    }

    pub fn reset(&mut self) {
        self.stats = Stats::default();
        self.events = 0;
    }

    // -----------------------------------------------------------------------
    // Report

    pub fn report(&self, top: usize, now: u64) -> Report {
        let mut commands: Vec<CommandReport> = self
            .stats
            .commands
            .iter()
            .map(|((catalog_id, command, entry_code), stats)| CommandReport {
                id: command_id(*catalog_id, *command),
                entry: entry::name(*entry_code).into(),
                total_us: stats.total.distribution(1000),
                queue_us: stats.queue.distribution(1000),
                exec_us: stats.exec.distribution(1000),
                publish_us: stats.publish.distribution(1000),
                apply_us: stats.apply.distribution(1000),
                bytes: stats.bytes.distribution(1),
            })
            .collect();
        commands.sort_by(|a, b| b.total_us.p95.cmp(&a.total_us.p95));
        let mut queries: Vec<QueryReport> = self
            .stats
            .queries
            .iter()
            .map(|((catalog_id, command), stats)| QueryReport {
                id: command_id(*catalog_id, *command),
                cancelled: stats.cancelled,
                queue_us: stats.queue.distribution(1000),
                provider_us: stats.provider.distribution(1000),
                round_trip_us: stats.round_trip.distribution(1000),
            })
            .collect();
        queries.sort_by(|a, b| b.round_trip_us.p95.cmp(&a.round_trip_us.p95));
        let mut fields: Vec<FieldReport> = self
            .stats
            .fields
            .iter()
            .filter(|((catalog_id, field), _)| self.field_shown(*catalog_id, *field))
            .map(|((catalog_id, field), stats)| FieldReport {
                name: field_name(*catalog_id, *field),
                sent: stats.sent,
                bytes_max: stats.bytes_max,
                items_max: stats.items_max,
                snapshot_us: stats.snapshot.distribution(1000),
                compare_us: stats.compare.distribution(1000),
                encode_us: stats.encode.distribution(1000),
                apply_us: stats.apply.distribution(1000),
            })
            .collect();
        fields.sort_by(|a, b| b.bytes_max.cmp(&a.bytes_max));
        let mut spans: Vec<SpanReport> = self
            .stats
            .spans
            .iter()
            .map(|(name, samples)| SpanReport {
                name: name.clone(),
                us: samples.distribution(1000),
            })
            .collect();
        spans.sort_by(|a, b| b.us.p95.cmp(&a.us.p95));
        Report {
            running: super::mask() != 0,
            window_ms: self.rel(now) / 1_000_000,
            events: self.events,
            dropped: self.dropped,
            commands,
            queries,
            fields,
            interactions: self.stats.slowest.iter().take(top).cloned().collect(),
            frames: FramesReport {
                spacing_us: self.stats.spacing.distribution(1000),
                long: self.stats.long,
            },
            long_frames: self.stats.long_frames.iter().cloned().collect(),
            spans,
        }
    }

    // -----------------------------------------------------------------------
    // Timeline

    fn track_name(&self, track: u32) -> String {
        if track == LOCAL_ENGINE_TRACK {
            return "engine (on the page thread)".into();
        }
        if track >= FRAMES_TRACK && track < LOCAL_ENGINE_TRACK {
            return format!("{} frames", self.track_name(track - FRAMES_TRACK));
        }
        self.thread_names
            .get(&track)
            .cloned()
            .unwrap_or_else(|| format!("thread {track}"))
    }

    fn ensure_track(&mut self, track: u32) {
        if self.timeline.is_none() || !self.tracks.insert(track) {
            return;
        }
        let name = self.track_name(track);
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.thread(track, &name, track as i64);
        }
    }

    fn draw(&mut self, record: &Record) {
        if self.timeline.is_none() {
            return;
        }
        let track = if record.kind == kind::FRAME {
            record.track + FRAMES_TRACK
        } else {
            record.track
        };
        self.ensure_track(track);
        let ts = self.rel(record.t0);
        let dur = record.t1.saturating_sub(record.t0);
        let job_command = |collector: &Collector| {
            collector
                .jobs
                .get(&record.key)
                .map(|job| command_id(job.catalog, job.command))
                .unwrap_or_else(|| "command".into())
        };
        let job_catalog = |collector: &Collector| {
            collector
                .jobs
                .get(&record.key)
                .map(|job| job.catalog)
                .unwrap_or(record.catalog)
        };
        let (name, category, args): (String, &str, Vec<(&str, String, bool)>) = match record.kind {
            kind::ACT => (format!("act {}", interned(record.name)), "ui", vec![]),
            kind::DISPATCH => (
                "dispatch".into(),
                "ui",
                vec![
                    ("token", record.key.to_string(), true),
                    ("command", record.a.to_string(), true),
                ],
            ),
            kind::RECEIVE_STATE => (
                format!("apply rev {}", record.key),
                "ui",
                vec![("bytes", record.a.to_string(), true)],
            ),
            kind::APPLY_FIELD => (
                format!("apply field {}", record.a),
                "ui",
                vec![("bytes", record.b.to_string(), true)],
            ),
            kind::RECEIVE_REPLY => (format!("reply token {}", record.key), "ui", vec![]),
            kind::RECEIVE_QUERY => (
                format!("query subscription {}", record.key),
                "ui",
                vec![("version", record.a.to_string(), true)],
            ),
            kind::UI_SPAN => (interned(record.name), "ui", vec![]),
            kind::FRAME => {
                let long = self
                    .pending_long
                    .iter()
                    .any(|long| long.track == record.track && long.to == record.t0);
                (
                    if long { "long frame" } else { "frame" }.into(),
                    "frame",
                    vec![
                        ("refresh_ns", record.a.to_string(), true),
                        ("shown_ns", self.rel(record.b).to_string(), true),
                    ],
                )
            }
            kind::DECODE => (
                format!("decode {}", command_id(record.catalog, record.a as u32)),
                "engine",
                vec![("token", record.key.to_string(), true)],
            ),
            kind::EXEC | kind::QUERY_START => {
                let job = self.jobs.get(&record.key);
                let queue = job
                    .and_then(|job| job.queued)
                    .map(|(queued, started)| started.saturating_sub(queued) / 1000)
                    .unwrap_or(0);
                let entry_name = job.map(|job| entry::name(job.entry)).unwrap_or("other");
                let origin = job.map(|job| job.origin).unwrap_or(0);
                (
                    job_command(self),
                    "engine",
                    vec![
                        ("queue_us", queue.to_string(), true),
                        ("entry", entry_name.to_string(), false),
                        ("origin", origin.to_string(), true),
                    ],
                )
            }
            kind::SNAPSHOT => ("snapshot".into(), "engine", vec![]),
            kind::SNAPSHOT_FIELD => (
                format!("snapshot {}", field_name(job_catalog(self), record.name)),
                "engine",
                vec![],
            ),
            kind::COMPARE_FIELD => (
                format!("compare {}", field_name(job_catalog(self), record.name)),
                "engine",
                vec![("changed", record.a.to_string(), true)],
            ),
            kind::ENCODE_FIELD => (
                format!("encode {}", field_name(job_catalog(self), record.name)),
                "engine",
                vec![
                    ("bytes", record.b.to_string(), true),
                    (
                        "items",
                        record
                            .extra
                            .checked_sub(1)
                            .map(|items| items as i64)
                            .unwrap_or(-1)
                            .to_string(),
                        true,
                    ),
                ],
            ),
            kind::PUBLISH => (
                if record.a > 0 {
                    format!("publish rev {}", record.a)
                } else {
                    "publish (unchanged)".into()
                },
                "engine",
                vec![],
            ),
            kind::MODULE => (command_id(job_catalog(self), record.name), "engine", vec![]),
            kind::SPAN => (static_name(record), "span", vec![]),
            kind::ENTRY => (interned(record.name), "engine", vec![]),
            kind::CALLBACK => {
                let name = match record.extra {
                    CALLBACK_STATE => format!("state rev {} to the UI", record.key),
                    CALLBACK_REPLY => format!("reply token {} to the UI", record.key),
                    CALLBACK_QUERY => format!("query subscription {} to the UI", record.key),
                    _ => format!("query subscription {} discarded", record.key),
                };
                let timeline = self.timeline.as_mut().expect("timeline");
                timeline.instant(
                    track,
                    ts,
                    &name,
                    "engine",
                    &[("bytes", Arg::Int(record.b as i64))],
                );
                return;
            }
            kind::QUERY_ANSWER => {
                let name = format!(
                    "answer {}",
                    match record.a {
                        0 => "ok",
                        1 => "error",
                        _ => "cancelled",
                    }
                );
                let ts = self.rel(record.t1);
                let timeline = self.timeline.as_mut().expect("timeline");
                timeline.instant(track, ts, &name, "engine", &[]);
                return;
            }
            // Queue time shows as the arrow into the command and as its
            // `queue_us`: the wait is not work on any thread.
            _ => return,
        };
        let args: Vec<(&str, Arg<'_>)> = args
            .iter()
            .map(|(key, value, number)| {
                (
                    *key,
                    if *number {
                        Arg::Int(value.parse().unwrap_or(0))
                    } else {
                        Arg::Text(value)
                    },
                )
            })
            .collect();
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.slice(track, ts, dur, &name, category, &args);
        }
    }

    /// Arrows from the dispatch through the command to where the UI
    /// handled its result.
    fn flows(&mut self, done: &Done) {
        if self.timeline.is_none() {
            return;
        }
        let mut points: Vec<(u32, u64)> = Vec::new();
        if let Some(dispatch) = done.dispatch.or(done.decode) {
            points.push((dispatch.track, dispatch.mid()));
        }
        if let Some(exec) = done.job.exec {
            points.push((exec.track, exec.mid()));
        }
        if let Some(seg) = done
            .ui_reply
            .or(done.ui_query)
            .or(done.ui_state.as_ref().and_then(|state| state.seg))
        {
            points.push((seg.track, seg.mid()));
        }
        if points.len() < 2 {
            return;
        }
        let started = self.started;
        let last = points.len() - 1;
        let timeline = self.timeline.as_mut().expect("timeline");
        for (index, (track, ts)) in points.into_iter().enumerate() {
            let phase = match index {
                0 => 's',
                i if i == last => 'f',
                _ => 't',
            };
            timeline.flow(phase, done.job.seq, track, ts.saturating_sub(started));
        }
    }

    pub fn close_timeline(&mut self, contents: bool) -> (Vec<String>, Option<String>) {
        let Some(timeline) = self.timeline.take() else {
            return (Vec::new(), None);
        };
        let (path, text) = timeline.close(contents);
        (
            path.map(|path| path.display().to_string()).into_iter().collect(),
            text,
        )
    }
}

enum Readiness {
    Wait,
    Ready,
    Abandon,
}

fn is_ui_kind(kind: u16) -> bool {
    matches!(
        kind,
        kind::ACT
            | kind::DISPATCH
            | kind::RECEIVE_STATE
            | kind::RECEIVE_REPLY
            | kind::RECEIVE_QUERY
            | kind::UI_SPAN
    )
}
