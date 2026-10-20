//! Per-thread rings of trace records.
//!
//! Each thread that records gets its own single-producer ring the first
//! time it records in a trace session; the collector is the only reader.
//! A full ring drops the new record and counts it, so recording never
//! waits. Rings belong to a session: starting a new one gives every thread
//! a fresh ring of the configured size.

use std::cell::{Cell, RefCell, UnsafeCell};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One event, fixed size. What `key`, `a`, `b`, `name` and `extra` mean
/// depends on `kind` (see `super::kind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub t0: u64,
    pub t1: u64,
    pub key: u64,
    pub a: u64,
    pub b: u64,
    pub kind: u16,
    /// The catalog `name` indexes, for commands and fields.
    pub catalog: u16,
    pub name: u32,
    /// The timeline track, normally the recording thread's.
    pub track: u32,
    pub extra: u32,
}

pub struct Ring {
    slots: Box<[UnsafeCell<MaybeUninit<Record>>]>,
    /// Next slot the producer writes, counting from the start.
    head: AtomicUsize,
    /// Next slot the collector reads.
    tail: AtomicUsize,
    dropped: AtomicU64,
    pub track: u32,
    pub thread: String,
}

// SAFETY: slots between `tail` and `head` are only read by the collector
// and slots outside them only written by the owning thread; the release
// and acquire on `head` and `tail` order those accesses.
unsafe impl Sync for Ring {}
unsafe impl Send for Ring {}

impl Ring {
    pub fn new(capacity: usize, track: u32, thread: String) -> Self {
        Self {
            slots: (0..capacity.max(2))
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicU64::new(0),
            track,
            thread,
        }
    }

    /// Append from the owning thread; `true` once the ring is half full.
    pub fn push(&self, record: Record) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let used = head - tail;
        if used >= self.slots.len() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        // SAFETY: the slot is outside `tail..head`, so the collector does
        // not read it, and only this thread writes.
        unsafe { (*self.slots[head % self.slots.len()].get()).write(record) };
        self.head.store(head + 1, Ordering::Release);
        used + 1 >= self.slots.len() / 2
    }

    /// Move every record written so far to `out`; collector only.
    pub fn drain(&self, out: &mut Vec<Record>) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        for index in tail..head {
            // SAFETY: slots in `tail..head` were written and published by
            // the release store of `head`.
            out.push(unsafe { (*self.slots[index % self.slots.len()].get()).assume_init() });
        }
        self.tail.store(head, Ordering::Release);
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// The rings of the current session.
struct Registry {
    session: u64,
    capacity: usize,
    rings: Vec<Arc<Ring>>,
    /// Records dropped by rings of ended threads in this session.
    retired: u64,
    next_track: u32,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    session: 0,
    capacity: super::rules::DEFAULT_BUFFER,
    rings: Vec::new(),
    retired: 0,
    next_track: 1,
});
/// The current session; 0 before the first.
static SESSION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static LOCAL: RefCell<Option<(u64, Arc<Ring>)>> = const { RefCell::new(None) };
    /// A track standing in for the thread's own, such as the engine's on a
    /// page where it runs on the page's thread.
    static TRACK: Cell<Option<u32>> = const { Cell::new(None) };
    /// The thread's track id, kept across sessions so the timeline names
    /// each thread once.
    static OWN_TRACK: Cell<u32> = const { Cell::new(0) };
}

/// Start a session whose rings hold `capacity` records each. Rings of the
/// session before are dropped; the collector read them when it stopped.
pub fn begin(capacity: usize) -> u64 {
    let mut registry = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    registry.session += 1;
    registry.capacity = capacity;
    registry.rings.clear();
    registry.retired = 0;
    SESSION.store(registry.session, Ordering::Release);
    registry.session
}

fn register(session: u64) -> Option<Arc<Ring>> {
    let mut registry = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    if registry.session != session {
        return None;
    }
    let track = OWN_TRACK.with(|own| {
        if own.get() == 0 {
            own.set(registry.next_track);
            registry.next_track += 1;
        }
        own.get()
    });
    let current = std::thread::current();
    let thread = current
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("thread {track}"));
    let ring = Arc::new(Ring::new(registry.capacity, track, thread));
    registry.rings.push(Arc::clone(&ring));
    Some(ring)
}

/// Append to this thread's ring; `true` once it is half full.
pub fn push(mut record: Record) -> bool {
    let session = SESSION.load(Ordering::Acquire);
    LOCAL
        .try_with(|local| {
            let mut local = local.borrow_mut();
            if local.as_ref().map(|(owner, _)| *owner) != Some(session) {
                *local = register(session).map(|ring| (session, ring));
            }
            let Some((_, ring)) = local.as_ref() else {
                return false;
            };
            record.track = TRACK.get().unwrap_or(ring.track);
            ring.push(record)
        })
        .unwrap_or(false)
}

/// Run `body` with records on `track` instead of the thread's own.
pub fn on_track<T>(track: u32, body: impl FnOnce() -> T) -> T {
    let previous = TRACK.replace(Some(track));
    let result = body();
    TRACK.set(previous);
    result
}

/// Every record of the current session not read yet, the number dropped
/// so far, and the threads that recorded, by track.
pub fn drain(out: &mut Vec<Record>) -> (u64, Vec<(u32, String)>) {
    let rings: Vec<Arc<Ring>> = {
        let registry = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
        registry.rings.clone()
    };
    let mut dropped = 0;
    let mut threads = Vec::with_capacity(rings.len());
    for ring in &rings {
        ring.drain(out);
        dropped += ring.dropped();
        threads.push((ring.track, ring.thread.clone()));
    }
    // Threads that ended leave their ring to the registry and `rings`
    // alone; once read it can go.
    let mut registry = REGISTRY.lock().unwrap_or_else(|error| error.into_inner());
    let mut retired = 0;
    registry.rings.retain(|ring| {
        let keep = Arc::strong_count(ring) > 2
            || ring.head.load(Ordering::Acquire) != ring.tail.load(Ordering::Relaxed);
        if !keep {
            retired += ring.dropped();
        }
        keep
    });
    registry.retired += retired;
    (dropped - retired + registry.retired, threads)
}

/// Whether this thread has a ring in the current session, for tests of
/// the off path.
#[cfg(test)]
pub fn has_ring() -> bool {
    let session = SESSION.load(Ordering::Acquire);
    LOCAL.with(|local| local.borrow().as_ref().is_some_and(|(owner, _)| *owner == session))
}
