//! Pictures from files, http(s) addresses and what the engine serves, read
//! in the background.
//!
//! An address the network could not reach, or that stays silent, is tried
//! again, each wait longer than the one before, for as long as something
//! waits for its picture. After the last wait it rests until the network
//! changes or a window comes to the front; a view that appears again starts
//! over at once. An address the server answered without a picture is not
//! asked again on the same network.

use gio::prelude::*;
use soup::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::poll_fn;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::task::{Poll, Waker};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Seconds before each further try, give or take a quarter, so that
/// pictures that failed together do not all return together.
#[cfg(not(test))]
const WAITS: [f64; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
#[cfg(test)]
const WAITS: [f64; 2] = [0.02, 0.02];
/// Seconds a try may hear nothing before it counts as failed. A stalled
/// connection reports no error. libsoup then closes it, HTTP/2 too, so the
/// next try has another.
#[cfg(not(test))]
const SILENCE: u32 = 15;
#[cfg(test)]
const SILENCE: u32 = 1;

/// Maximum in-memory raw byte LRU capacity (32 MiB).
const MEMORY_CACHE_LIMIT: usize = 32 * 1024 * 1024;
/// Maximum on-disk picture cache capacity (256 MiB).
const DISK_CACHE_LIMIT: u64 = 256 * 1024 * 1024;
/// Target size after background disk pruning (192 MiB).
const DISK_CACHE_TARGET: u64 = 192 * 1024 * 1024;
/// Maximum size of a single cached file (16 MiB).
const MAX_CACHE_FILE_BYTES: usize = 16 * 1024 * 1024;
/// Trigger background disk pruning once every this many writes.
const DISK_PRUNE_INTERVAL: u64 = 16;
/// Maximum concurrent remote picture downloads.
const MAX_CONCURRENT_REQUESTS: usize = 8;

/// What one try at an address gave.
enum Reading {
    Bytes(glib::Bytes),
    /// The network failed or stayed silent, or the server could not answer
    /// now.
    Unreachable,
    /// The server answered, and not with a picture.
    Refused,
    /// No such file. Looking again costs nothing, so nothing is remembered.
    Absent,
}

#[derive(Clone)]
enum InFlightOutcome {
    Bytes {
        bytes: glib::Bytes,
        from_cache: bool,
    },
    Refused,
    Absent,
}

struct MemoryEntry {
    bytes: glib::Bytes,
    last_used: u64,
}

/// Bounded in-memory LRU cache storing raw picture bytes.
struct MemoryCache {
    entries: HashMap<String, MemoryEntry>,
    order: VecDeque<(String, u64)>,
    total_bytes: usize,
    clock: u64,
    limit: usize,
}

impl MemoryCache {
    fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            clock: 0,
            limit,
        }
    }

    fn get(&mut self, url: &str) -> Option<glib::Bytes> {
        let entry = self.entries.get_mut(url)?;
        self.clock += 1;
        entry.last_used = self.clock;
        let bytes = entry.bytes.clone();
        self.order.push_back((url.to_string(), self.clock));
        self.compact_order();
        Some(bytes)
    }

    fn remove(&mut self, url: &str) {
        if let Some(prev) = self.entries.remove(url) {
            self.total_bytes = self.total_bytes.saturating_sub(prev.bytes.len());
        }
    }

    fn insert(&mut self, url: &str, bytes: glib::Bytes) {
        let len = bytes.len();
        if len == 0 || len > self.limit {
            return;
        }
        if let Some(prev) = self.entries.remove(url) {
            self.total_bytes = self.total_bytes.saturating_sub(prev.bytes.len());
        }
        self.clock += 1;
        self.total_bytes += len;
        self.entries.insert(
            url.to_string(),
            MemoryEntry {
                bytes,
                last_used: self.clock,
            },
        );
        self.order.push_back((url.to_string(), self.clock));
        while self.total_bytes > self.limit {
            let Some((key, stamp)) = self.order.pop_front() else {
                break;
            };
            if self
                .entries
                .get(&key)
                .is_some_and(|entry| entry.last_used == stamp)
                && let Some(removed) = self.entries.remove(&key)
            {
                self.total_bytes = self.total_bytes.saturating_sub(removed.bytes.len());
            }
        }
        self.compact_order();
    }

    fn compact_order(&mut self) {
        let threshold = self.entries.len().max(64) * 2;
        if self.order.len() <= threshold {
            return;
        }
        let entries = &self.entries;
        self.order.retain(|(key, stamp)| {
            entries
                .get(key)
                .is_some_and(|entry| entry.last_used == *stamp)
        });
    }
}

/// Persistent disk cache backed by `glib::user_cache_dir()/bern-pictures`.
struct DiskCache {
    dir: PathBuf,
    writes: AtomicU64,
    pruning: Arc<AtomicBool>,
}

struct PruneGuard(Arc<AtomicBool>);

impl Drop for PruneGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl DiskCache {
    fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            writes: AtomicU64::new(0),
            pruning: Arc::new(AtomicBool::new(false)),
        }
    }

    fn path_for(&self, url: &str) -> PathBuf {
        self.dir.join(cache_key(url))
    }

    fn read(&self, url: &str) -> Option<Vec<u8>> {
        let path = self.path_for(url);
        let bytes = std::fs::read(&path).ok()?;
        if bytes.is_empty() || bytes.len() > MAX_CACHE_FILE_BYTES {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    fn remove(&self, url: &str) {
        let _ = std::fs::remove_file(self.path_for(url));
    }

    fn write(&self, url: &str, bytes: &[u8]) {
        if bytes.is_empty() || bytes.len() > MAX_CACHE_FILE_BYTES {
            return;
        }
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let key = cache_key(url);
        let final_path = self.dir.join(&key);
        let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = self
            .dir
            .join(format!(".tmp-{key}-{}-{seq}", std::process::id()));
        let wrote = (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(&tmp_path)?;
            file.write_all(bytes)?;
            file.flush()?;
            std::fs::rename(&tmp_path, &final_path)?;
            Ok(())
        })();
        if wrote.is_err() {
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }
        let count = self.writes.fetch_add(1, Ordering::Relaxed) + 1;
        if count.is_multiple_of(DISK_PRUNE_INTERVAL)
            && self
                .pruning
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            let dir = self.dir.clone();
            let flag = self.pruning.clone();
            std::thread::spawn(move || {
                let _guard = PruneGuard(flag);
                prune_dir(&dir, DISK_CACHE_LIMIT, DISK_CACHE_TARGET);
            });
        }
    }
}

fn default_cache_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        Some(gtk::glib::user_cache_dir().join("bern-pictures"))
    }
}

fn cache_key(url: &str) -> String {
    const FNV1A_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV1A_PRIME: u64 = 0x100000001b3;

    let mut low = FNV1A_OFFSET;
    for &byte in url.as_bytes() {
        low ^= u64::from(byte);
        low = low.wrapping_mul(FNV1A_PRIME);
    }

    let mut high = FNV1A_OFFSET;
    for &byte in url.as_bytes().iter().rev() {
        high ^= u64::from(byte);
        high = high.wrapping_mul(FNV1A_PRIME);
    }
    high ^= url.len() as u64;
    high = high.wrapping_mul(FNV1A_PRIME);

    format!("{low:016x}{high:016x}")
}

fn prune_dir(dir: &Path, limit: u64, target: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files = Vec::new();
    let mut total = 0u64;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with(".tmp-") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let len = metadata.len();
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        total = total.saturating_add(len);
        files.push((modified, entry.path(), len));
    }
    if total <= limit {
        return;
    }
    files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    for (_, path, len) in files {
        if total <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

fn is_loopback(url: &str) -> bool {
    let rest = if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, right)| right);
    let host = if let Some(bracketed) = host_port.strip_prefix('[') {
        bracketed
            .split(']')
            .next()
            .unwrap_or("")
            .split('%')
            .next()
            .unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Download priority for bounding and ordering remote picture requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) enum Priority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
}

struct SemaphoreWaiter {
    id: u64,
    priority: Priority,
    waker: Waker,
    source: Option<glib::Source>,
    granted: bool,
}

/// Asynchronous semaphore bounding concurrent remote HTTP downloads.
struct SemaphoreState {
    available: usize,
    waiters: VecDeque<SemaphoreWaiter>,
    next_id: u64,
}

impl SemaphoreState {
    const fn new(limit: usize) -> Self {
        Self {
            available: limit,
            waiters: VecDeque::new(),
            next_id: 0,
        }
    }

    fn prune_destroyed(&mut self) {
        self.waiters
            .retain(|w| w.source.as_ref().is_none_or(|s| !s.is_destroyed()));
    }

    /// Hand a permit directly to the highest-priority un-granted waiter
    /// (FIFO on ties), or return it to `available`.
    fn release_one(&mut self) -> Option<Waker> {
        self.prune_destroyed();
        let best_idx = self
            .waiters
            .iter()
            .enumerate()
            .filter(|(_, w)| !w.granted)
            .max_by_key(|(idx, w)| (w.priority, std::cmp::Reverse(*idx)))
            .map(|(idx, _)| idx);
        if let Some(idx) = best_idx {
            let waiter = &mut self.waiters[idx];
            waiter.granted = true;
            Some(waiter.waker.clone())
        } else {
            self.available += 1;
            None
        }
    }
}

struct Permit {
    sem: Rc<RefCell<SemaphoreState>>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let waker = self.sem.borrow_mut().release_one();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

struct AcquireFuture {
    sem: Rc<RefCell<SemaphoreState>>,
    waiter_id: Option<u64>,
    entry: Option<Rc<InFlightEntry>>,
}

impl Drop for AcquireFuture {
    fn drop(&mut self) {
        if let Some(id) = self.waiter_id.take() {
            if let Some(entry) = &self.entry
                && entry.sem_waiter_id.get() == Some(id)
            {
                entry.sem_waiter_id.set(None);
            }
            let waker = {
                let mut state = self.sem.borrow_mut();
                let was_granted = state
                    .waiters
                    .iter()
                    .position(|w| w.id == id)
                    .and_then(|pos| state.waiters.remove(pos))
                    .is_some_and(|w| w.granted);
                if was_granted {
                    state.release_one()
                } else {
                    None
                }
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }
}

async fn acquire_download_permit(priority: Priority, entry: Option<Rc<InFlightEntry>>) -> Permit {
    let sem = DOWNLOAD_SEMAPHORE.with(Rc::clone);
    let mut future = AcquireFuture {
        sem,
        waiter_id: None,
        entry,
    };
    poll_fn(|cx| {
        let mut state = future.sem.borrow_mut();
        if let Some(id) = future.waiter_id
            && let Some(pos) = state.waiters.iter().position(|w| w.id == id)
        {
            if state.waiters[pos].granted {
                state.waiters.remove(pos);
                future.waiter_id = None;
                if let Some(entry) = &future.entry
                    && entry.sem_waiter_id.get() == Some(id)
                {
                    entry.sem_waiter_id.set(None);
                }
                return Poll::Ready(Permit {
                    sem: Rc::clone(&future.sem),
                });
            }
            if state.available > 0 {
                state.waiters.remove(pos);
                state.available -= 1;
                future.waiter_id = None;
                if let Some(entry) = &future.entry
                    && entry.sem_waiter_id.get() == Some(id)
                {
                    entry.sem_waiter_id.set(None);
                }
                return Poll::Ready(Permit {
                    sem: Rc::clone(&future.sem),
                });
            }
            let waker = cx.waker().clone();
            if !state.waiters[pos].waker.will_wake(&waker) {
                state.waiters[pos].waker = waker;
            }
            if let Some(entry) = &future.entry {
                let promoted = entry.priority.get();
                if promoted > state.waiters[pos].priority {
                    state.waiters[pos].priority = promoted;
                }
            }
            return Poll::Pending;
        }
        if state.available > 0 {
            state.available -= 1;
            return Poll::Ready(Permit {
                sem: Rc::clone(&future.sem),
            });
        }
        state.next_id += 1;
        let id = state.next_id;
        future.waiter_id = Some(id);
        if let Some(entry) = &future.entry {
            entry.sem_waiter_id.set(Some(id));
        }
        let eff_priority = future
            .entry
            .as_ref()
            .map_or(priority, |e| priority.max(e.priority.get()));
        state.waiters.push_back(SemaphoreWaiter {
            id,
            priority: eff_priority,
            waker: cx.waker().clone(),
            source: glib::main_current_source(),
            granted: false,
        });
        Poll::Pending
    })
    .await
}

struct InFlightWaiter {
    id: u64,
    waker: Option<Waker>,
    source: Option<glib::Source>,
}

struct InFlightState {
    waiters: Vec<InFlightWaiter>,
    outcome: Option<InFlightOutcome>,
    next_id: u64,
}

struct InFlightEntry {
    state: RefCell<InFlightState>,
    leader: RefCell<Option<glib::JoinHandle<()>>>,
    priority: Cell<Priority>,
    sem_waiter_id: Cell<Option<u64>>,
}

impl InFlightEntry {
    fn new(priority: Priority) -> Self {
        Self {
            state: RefCell::new(InFlightState {
                waiters: Vec::new(),
                outcome: None,
                next_id: 0,
            }),
            leader: RefCell::new(None),
            priority: Cell::new(priority),
            sem_waiter_id: Cell::new(None),
        }
    }

    fn prune_dead_waiters(&self) -> bool {
        let mut state = self.state.borrow_mut();
        state
            .waiters
            .retain(|waiter| waiter.source.as_ref().is_none_or(|s| !s.is_destroyed()));
        !state.waiters.is_empty()
    }
}

/// Promote any active in-flight download (and its pending semaphore waiter)
/// for `url` to at least `priority`.
pub(crate) fn promote_download(url: &str, priority: Priority) {
    IN_FLIGHT.with(|map| {
        let map = map.borrow();
        if let Some(entry) = map.get(url)
            && priority > entry.priority.get()
        {
            entry.priority.set(priority);
            if let Some(waiter_id) = entry.sem_waiter_id.get() {
                DOWNLOAD_SEMAPHORE.with(|sem| {
                    let mut state = sem.borrow_mut();
                    if let Some(waiter) = state.waiters.iter_mut().find(|w| w.id == waiter_id)
                        && priority > waiter.priority
                    {
                        waiter.priority = priority;
                    }
                });
            }
        }
    });
}

struct WaiterGuard {
    url: String,
    entry: Rc<InFlightEntry>,
    waiter_id: u64,
}

impl Drop for WaiterGuard {
    fn drop(&mut self) {
        let should_cancel = {
            let mut state = self.entry.state.borrow_mut();
            state.waiters.retain(|waiter| {
                waiter.id != self.waiter_id
                    && waiter.source.as_ref().is_none_or(|s| !s.is_destroyed())
            });
            state.waiters.is_empty() && state.outcome.is_none()
        };
        if should_cancel {
            cancel_in_flight(&self.url, &self.entry);
        }
    }
}

fn cancel_in_flight(url: &str, entry: &Rc<InFlightEntry>) {
    IN_FLIGHT.with(|map| {
        let mut map = map.borrow_mut();
        if map
            .get(url)
            .is_some_and(|current| Rc::ptr_eq(current, entry))
        {
            map.remove(url);
        }
    });
    if let Some(id) = entry.sem_waiter_id.take() {
        let waker = DOWNLOAD_SEMAPHORE.with(|sem| {
            let mut state = sem.borrow_mut();
            let was_granted = state
                .waiters
                .iter()
                .position(|w| w.id == id)
                .and_then(|pos| state.waiters.remove(pos))
                .is_some_and(|w| w.granted);
            if was_granted {
                state.release_one()
            } else {
                None
            }
        });
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    if let Some(handle) = entry.leader.borrow_mut().take() {
        handle.abort();
    }
}

static DISK_CACHE: LazyLock<Option<DiskCache>> =
    LazyLock::new(|| default_cache_dir().map(DiskCache::new));

thread_local! {
    static SESSION: soup::Session = new_session();
    /// Addresses the server refused on this network.
    static REFUSED: RefCell<HashSet<String>> = RefCell::default();
    /// Cancelled to wake every load waiting for its next try.
    static WAKE: RefCell<gio::Cancellable> = RefCell::new(gio::Cancellable::new());
    static LISTENING: Cell<bool> = const { Cell::new(false) };
    static MEMORY_CACHE: RefCell<MemoryCache> = RefCell::new(MemoryCache::new(MEMORY_CACHE_LIMIT));
    static IN_FLIGHT: RefCell<HashMap<String, Rc<InFlightEntry>>> = RefCell::default();
    static DOWNLOAD_SEMAPHORE: Rc<RefCell<SemaphoreState>> =
        Rc::new(RefCell::new(SemaphoreState::new(MAX_CONCURRENT_REQUESTS)));
}

fn new_session() -> soup::Session {
    let session = soup::Session::new();
    session.set_timeout(SILENCE);
    session.set_user_agent("Bern");
    session
}

fn remote(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// One try at the bytes at `url`: a `file://` URL or path, an http(s)
/// address, or a URL the engine serves.
async fn read(url: &str, entry: Option<Rc<InFlightEntry>>) -> Reading {
    if bern_ffi::files::served(url) {
        let address = url.to_string();
        let bytes = gio::spawn_blocking(move || bern_ffi::files::read_served(&address)).await;
        return match bytes {
            Ok(Some(bytes)) => Reading::Bytes(glib::Bytes::from_owned(bytes)),
            _ => Reading::Absent,
        };
    }
    if !remote(url) {
        let file = if url.contains("://") {
            gio::File::for_uri(url)
        } else {
            gio::File::for_path(url)
        };
        return match file.load_bytes_future().await {
            Ok((bytes, _)) => Reading::Bytes(bytes),
            Err(_) => Reading::Absent,
        };
    }
    let priority = entry
        .as_ref()
        .map_or(Priority::Normal, |e| e.priority.get());
    let _permit = acquire_download_permit(priority, entry.clone()).await;
    let Ok(message) = soup::Message::new("GET", url) else {
        return Reading::Refused;
    };
    let glib_priority = match entry.as_ref().map_or(priority, |e| e.priority.get()) {
        Priority::High => glib::Priority::HIGH,
        Priority::Normal => glib::Priority::DEFAULT,
        Priority::Low => glib::Priority::LOW,
    };
    let session = SESSION.with(soup::Session::clone);
    match session.send_and_read_future(&message, glib_priority).await {
        Ok(bytes) => match message.status_code() {
            200..=299 => Reading::Bytes(bytes),
            408 | 429 | 500.. => Reading::Unreachable,
            _ => Reading::Refused,
        },
        Err(error) if error.matches(gio::TlsError::BadCertificate) => Reading::Refused,
        Err(_) => Reading::Unreachable,
    }
}

fn finish_in_flight(url: &str, entry: &Rc<InFlightEntry>, outcome: InFlightOutcome) {
    IN_FLIGHT.with(|map| {
        let mut map = map.borrow_mut();
        if map
            .get(url)
            .is_some_and(|current| Rc::ptr_eq(current, entry))
        {
            map.remove(url);
        }
    });
    let wakers: Vec<Waker> = {
        let mut state = entry.state.borrow_mut();
        state.outcome = Some(outcome);
        state
            .waiters
            .iter_mut()
            .filter_map(|waiter| waiter.waker.take())
            .collect()
    };
    *entry.leader.borrow_mut() = None;
    for waker in wakers {
        waker.wake();
    }
}

async fn drive_in_flight(url: String, entry: Rc<InFlightEntry>, use_disk_cache: bool) {
    if !entry.prune_dead_waiters() {
        cancel_in_flight(&url, &entry);
        return;
    }

    if use_disk_cache
        && remote(&url)
        && !is_loopback(&url)
        && let Some(disk) = DISK_CACHE.as_ref()
    {
        let key_url = url.clone();
        if let Ok(Some(raw)) = gio::spawn_blocking(move || disk.read(&key_url)).await {
            let bytes = glib::Bytes::from_owned(raw);
            MEMORY_CACHE.with(|cache| cache.borrow_mut().insert(&url, bytes.clone()));
            finish_in_flight(
                &url,
                &entry,
                InFlightOutcome::Bytes {
                    bytes,
                    from_cache: true,
                },
            );
            return;
        }
    }

    let mut tries = 0;
    loop {
        if !entry.prune_dead_waiters() {
            cancel_in_flight(&url, &entry);
            return;
        }
        if REFUSED.with(|refused| refused.borrow().contains(&url)) {
            finish_in_flight(&url, &entry, InFlightOutcome::Refused);
            return;
        }
        match read(&url, Some(Rc::clone(&entry))).await {
            Reading::Bytes(bytes) => {
                finish_in_flight(
                    &url,
                    &entry,
                    InFlightOutcome::Bytes {
                        bytes,
                        from_cache: false,
                    },
                );
                return;
            }
            Reading::Unreachable => {
                if !entry.prune_dead_waiters() {
                    cancel_in_flight(&url, &entry);
                    return;
                }
                wait(&mut tries).await;
            }
            Reading::Refused => {
                refuse(&url);
                finish_in_flight(&url, &entry, InFlightOutcome::Refused);
                return;
            }
            Reading::Absent => {
                finish_in_flight(&url, &entry, InFlightOutcome::Absent);
                return;
            }
        }
    }
}

async fn fetch_in_flight(url: &str, use_disk_cache: bool, priority: Priority) -> InFlightOutcome {
    let caller_source = glib::main_current_source();
    let (entry, guard) = IN_FLIGHT.with(|map| {
        let mut map = map.borrow_mut();
        let existing = map.get(url).cloned().filter(|entry| {
            if entry.prune_dead_waiters() {
                true
            } else {
                if let Some(handle) = entry.leader.borrow_mut().take() {
                    handle.abort();
                }
                false
            }
        });
        let (entry, is_leader) = if let Some(existing) = existing {
            (existing, false)
        } else {
            let created = Rc::new(InFlightEntry::new(priority));
            map.insert(url.to_string(), Rc::clone(&created));
            (created, true)
        };
        let waiter_id = {
            let mut state = entry.state.borrow_mut();
            state.next_id += 1;
            let id = state.next_id;
            state.waiters.push(InFlightWaiter {
                id,
                waker: None,
                source: caller_source,
            });
            id
        };
        if is_leader {
            let url_owned = url.to_string();
            let entry_cloned = Rc::clone(&entry);
            let handle =
                glib::spawn_future_local(drive_in_flight(url_owned, entry_cloned, use_disk_cache));
            if entry.state.borrow().outcome.is_none() {
                *entry.leader.borrow_mut() = Some(handle);
            }
        }
        let guard = WaiterGuard {
            url: url.to_string(),
            entry: Rc::clone(&entry),
            waiter_id,
        };
        (entry, guard)
    });
    promote_download(url, priority);

    let outcome = poll_fn(|cx| {
        let mut state = entry.state.borrow_mut();
        if let Some(outcome) = state.outcome.clone() {
            return Poll::Ready(outcome);
        }
        let waker = cx.waker().clone();
        if let Some(waiter) = state
            .waiters
            .iter_mut()
            .find(|waiter| waiter.id == guard.waiter_id)
        {
            if !waiter
                .waker
                .as_ref()
                .is_some_and(|existing| existing.will_wake(&waker))
            {
                waiter.waker = Some(waker);
            }
        }
        Poll::Pending
    })
    .await;
    drop(guard);
    outcome
}

fn cache_put(url: &str, bytes: glib::Bytes) {
    if !remote(url) || is_loopback(url) || bytes.is_empty() {
        return;
    }
    let already_cached = MEMORY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.get(url).is_some() {
            true
        } else {
            cache.insert(url, bytes.clone());
            false
        }
    });
    if !already_cached
        && bytes.len() <= MAX_CACHE_FILE_BYTES
        && let Some(disk) = DISK_CACHE.as_ref()
    {
        let url = url.to_string();
        gio::spawn_blocking(move || disk.write(&url, &bytes));
    }
}

fn cache_evict(url: &str) {
    MEMORY_CACHE.with(|cache| cache.borrow_mut().remove(url));
    if let Some(disk) = DISK_CACHE.as_ref() {
        let url = url.to_string();
        gio::spawn_blocking(move || disk.remove(&url));
    }
}

/// What `decode` makes of the bytes at `url`, once they come and it can;
/// none when there is no such file. Dropped, it stops: a view's load ends
/// with the view, and a view that appears again starts over.
pub(crate) async fn picture<T>(
    url: &str,
    decode: impl AsyncFnMut(glib::Bytes) -> Option<T>,
) -> Option<T> {
    picture_with_priority(url, Priority::Normal, decode).await
}

/// Same as [`picture`], with an explicit download [`Priority`].
pub(crate) async fn picture_with_priority<T>(
    url: &str,
    priority: Priority,
    mut decode: impl AsyncFnMut(glib::Bytes) -> Option<T>,
) -> Option<T> {
    listen();
    let mut skip_cache = false;
    loop {
        if REFUSED.with(|refused| refused.borrow().contains(url)) {
            pause(None).await;
            continue;
        }
        if !skip_cache
            && remote(url)
            && !is_loopback(url)
            && let Some(bytes) = MEMORY_CACHE.with(|cache| cache.borrow_mut().get(url))
        {
            if let Some(picture) = decode(bytes).await {
                return Some(picture);
            }
            cache_evict(url);
            skip_cache = true;
            continue;
        }
        match fetch_in_flight(url, !skip_cache, priority).await {
            InFlightOutcome::Bytes { bytes, from_cache } => {
                if let Some(picture) = decode(bytes.clone()).await {
                    if !from_cache {
                        cache_put(url, bytes);
                    }
                    return Some(picture);
                }
                if from_cache {
                    cache_evict(url);
                    skip_cache = true;
                    continue;
                }
                if !remote(url) {
                    return None;
                }
                refuse(url);
            }
            InFlightOutcome::Refused => refuse(url),
            InFlightOutcome::Absent => return None,
        }
    }
}

fn refuse(url: &str) {
    REFUSED.with(|refused| refused.borrow_mut().insert(url.to_string()));
}

/// The wait before the next of `tries`, or the rest after the last.
async fn wait(tries: &mut usize) {
    let wait = WAITS.get(*tries).copied();
    pause(wait.map(|seconds| seconds * glib::random_double_range(0.75, 1.25))).await;
    *tries = if wait.is_some() { *tries + 1 } else { 0 };
}

/// Waits `seconds`, or with none for as long as it takes, until the network
/// changes or a window comes to the front.
async fn pause(seconds: Option<f64>) {
    let wake = WAKE.with(|wake| wake.borrow().clone());
    match seconds {
        Some(seconds) => {
            let _ =
                glib::future_with_timeout(Duration::from_secs_f64(seconds), wake.future()).await;
        }
        None => wake.future().await,
    }
}

/// Every load waiting for its next try goes now.
pub(crate) fn try_again() {
    WAKE.with(|wake| wake.replace(gio::Cancellable::new()))
        .cancel();
}

/// Once it is there after a change, a new way out may answer what the last
/// one could not.
fn network_changed(available: bool) {
    if available {
        REFUSED.with(|refused| refused.borrow_mut().clear());
        try_again();
    }
}

fn listen() {
    if !LISTENING.replace(true) {
        gio::NetworkMonitor::default()
            .connect_network_changed(|_, available| network_changed(available));
    }
}

#[cfg(test)]
#[path = "../../bern-target-api/tests/faulty_server/mod.rs"]
mod faulty_server;

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    fn text(address: SocketAddr, path: &str) -> String {
        let mut stream = TcpStream::connect(address).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\n\r\n").unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        answer.split("\r\n\r\n").nth(1).unwrap().to_string()
    }

    fn asked(address: SocketAddr, path: &str) -> usize {
        text(address, &format!("/asked{path}")).parse().unwrap()
    }

    async fn settle(milliseconds: u64) {
        glib::timeout_future(Duration::from_millis(milliseconds)).await;
    }

    /// How often `path` was asked for, once that has stopped changing.
    async fn settled(address: SocketAddr, path: &str) -> usize {
        let mut count = asked(address, path);
        let mut still = 0;
        while still < 4 || count == 0 {
            settle(250).await;
            let now = asked(address, path);
            still = if now == count { still + 1 } else { 0 };
            count = now;
        }
        count
    }

    /// The length of the picture at `url`, in the background.
    fn load(url: String) -> glib::JoinHandle<Option<usize>> {
        glib::spawn_future_local(async move {
            picture(&url, async |bytes: glib::Bytes| {
                (bytes.first() == Some(&0x89)).then_some(bytes.len())
            })
            .await
        })
    }

    fn within<T>(future: impl Future<Output = T>) -> T {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                context.block_on(async {
                    glib::future_with_timeout(Duration::from_secs(60), future)
                        .await
                        .expect("timed out")
                })
            })
            .unwrap()
    }

    #[test]
    fn pictures_come_after_failures_and_refusals_are_not_repeated() {
        let (address, silent) = faulty_server::start();
        let url = |path: &str| format!("http://{address}{path}");
        within(async {
            // A failure on the way, and the picture still comes.
            assert!(load(url("/breaks/1")).await.unwrap().is_some());
            assert!(asked(address, "/breaks/1") >= 3);
            assert!(load(url("/busy/1")).await.unwrap().is_some());
            assert_eq!(asked(address, "/busy/1"), 3);

            // A connection that stalls reports nothing, and is given up on:
            // the server stays silent for 20 seconds, and a try may wait
            // for 1.
            let stalled = Instant::now();
            assert!(load(url("/stalls/1")).await.unwrap().is_some());
            assert_eq!(asked(address, "/stalls/1"), 2);
            assert!(stalled.elapsed() < Duration::from_secs(10));

            // Nothing listens at first, as while a proxy starts.
            let starting = load(format!("http://127.0.0.1:{silent}/picture"));
            settle(300).await;
            text(address, &format!("/listen/{silent}"));
            try_again();
            assert!(starting.await.unwrap().is_some());

            // Tries end, and a load rests until something changes.
            let down = load(url("/down/1"));
            let tried = settled(address, "/down/1").await;
            assert!(tried >= 3, "{tried}");
            try_again();
            assert!(settled(address, "/down/1").await > tried);
            text(address, "/heal/down/1");
            network_changed(false);
            settle(50).await;
            network_changed(true);
            assert!(down.await.unwrap().is_some());

            // A view that goes away stops the tries, and one that appears
            // again starts over.
            let gone = load(url("/down/2"));
            settle(10).await;
            gone.abort();
            let left = settled(address, "/down/2").await;
            settle(300).await;
            assert_eq!(asked(address, "/down/2"), left);
            text(address, "/heal/down/2");
            assert!(load(url("/down/2")).await.unwrap().is_some());

            // An answer that is no picture is not asked for again: not by
            // the same view, a new one or a window coming to the front.
            for path in ["/gone/1", "/page/1"] {
                let first = load(url(path));
                assert_eq!(settled(address, path).await, 1, "{path}");
                first.abort();
                let second = load(url(path));
                try_again();
                assert_eq!(settled(address, path).await, 1, "{path}");
                // On another network it may have one after all.
                text(address, &format!("/heal{path}"));
                network_changed(true);
                assert!(second.await.unwrap().is_some(), "{path}");
                assert_eq!(asked(address, path), 2, "{path}");
            }

            // A file is looked for once, and looked for again by the next
            // view.
            assert_eq!(load("file:///nothing/here.png".into()).await.unwrap(), None);
            assert_eq!(load("/nothing/here.png".into()).await.unwrap(), None);
            let path = std::env::temp_dir().join("bern-picture-retry.png");
            std::fs::write(&path, faulty_server::PICTURE).unwrap();
            assert!(load(path.display().to_string()).await.unwrap().is_some());
        });
    }

    #[test]
    fn loopback_and_cache_key_behave_deterministically() {
        assert!(is_loopback("http://127.0.0.1:8080/a.png"));
        assert!(is_loopback("http://localhost:3000/a.png"));
        assert!(is_loopback("http://[::1]:9000/a.png"));
        assert!(is_loopback("https://sub.localhost/a.png"));
        assert!(!is_loopback("https://example.com/a.png"));
        assert!(!is_loopback("file:///tmp/a.png"));

        let k1 = cache_key("https://example.com/a.png");
        let k2 = cache_key("https://example.com/a.png");
        let k3 = cache_key("https://example.com/b.png");
        assert_eq!(k1.len(), 32);
        assert!(k1.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn memory_lru_evicts_least_recently_used() {
        let mut cache = MemoryCache::new(10);
        cache.insert("a", glib::Bytes::from_static(&[1, 2, 3, 4]));
        cache.insert("b", glib::Bytes::from_static(&[5, 6, 7, 8]));
        assert!(cache.get("a").is_some());
        cache.insert("c", glib::Bytes::from_static(&[9, 10, 11, 12]));

        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
        assert!(cache.get("c").is_some());
        assert_eq!(cache.total_bytes, 8);

        cache.insert("huge", glib::Bytes::from_owned(vec![0; 11]));
        assert!(cache.get("huge").is_none());
        assert_eq!(cache.total_bytes, 8);
    }

    #[test]
    fn disk_cache_writes_atomically_enforces_limit_and_prunes() {
        let dir = std::env::temp_dir().join(format!(
            "bern-gtk-pictures-test-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let disk = DiskCache::new(dir.clone());

        disk.write("https://example.com/ok.png", faulty_server::PICTURE);
        assert_eq!(
            disk.read("https://example.com/ok.png").as_deref(),
            Some(faulty_server::PICTURE)
        );
        let has_tmp = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with(".tmp-"));
        assert!(!has_tmp);

        let oversized = vec![0u8; MAX_CACHE_FILE_BYTES + 1];
        disk.write("https://example.com/big.png", &oversized);
        assert!(disk.read("https://example.com/big.png").is_none());

        let now = SystemTime::now();
        for idx in 0..4u64 {
            let path = dir.join(format!("extra-{idx}"));
            std::fs::write(&path, [idx as u8; 100]).unwrap();
            let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            let _ = file.set_modified(now - Duration::from_secs(100 - idx * 10));
        }
        let tmp_keep = dir.join(".tmp-inflight");
        std::fs::write(&tmp_keep, [0u8; 500]).unwrap();

        prune_dir(&dir, 300, 150);
        let remaining: u64 = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with(".tmp-"))
            .map(|e| e.metadata().unwrap().len())
            .sum();
        assert!(remaining <= 150, "remaining={remaining}");
        assert!(tmp_keep.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inflight_deduplicates_and_limits_concurrency() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let shared_hits = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let hits_srv = Arc::clone(&shared_hits);
        let active_srv = Arc::clone(&active);
        let max_srv = Arc::clone(&max_active);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let hits = Arc::clone(&hits_srv);
                let active = Arc::clone(&active_srv);
                let max_active = Arc::clone(&max_srv);
                std::thread::spawn(move || {
                    let mut stream = stream;
                    let mut buf = [0u8; 1024];
                    let mut req = Vec::new();
                    while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => req.extend_from_slice(&buf[..n]),
                        }
                    }
                    let req_str = String::from_utf8_lossy(&req);
                    let path = req_str.split_whitespace().nth(1).unwrap_or("/");
                    if path == "/shared" {
                        hits.fetch_add(1, Ordering::SeqCst);
                    }
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(cur, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(120));
                    active.fetch_sub(1, Ordering::SeqCst);
                    let body = faulty_server::PICTURE;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(body);
                });
            }
        });

        within(async {
            let shared_url = format!("http://{addr}/shared");
            let a = load(shared_url.clone());
            let b = load(shared_url.clone());
            let c = load(shared_url);
            // Cancelling one waiter leaves the remaining shared request intact.
            a.abort();
            assert_eq!(b.await.unwrap(), Some(faulty_server::PICTURE.len()));
            assert_eq!(c.await.unwrap(), Some(faulty_server::PICTURE.len()));
            assert_eq!(shared_hits.load(Ordering::SeqCst), 1);

            let mut handles = Vec::new();
            for idx in 0..16 {
                handles.push(load(format!("http://{addr}/item/{idx}")));
            }
            for handle in handles {
                assert_eq!(handle.await.unwrap(), Some(faulty_server::PICTURE.len()));
            }
            let peak = max_active.load(Ordering::SeqCst);
            assert!(peak <= MAX_CONCURRENT_REQUESTS, "peak={peak}");
        });
    }

    #[test]
    fn priority_semaphore_grants_highest_and_promotes_inflight() {
        within(async {
            // 1-slot local semaphore: test directed priority handoff & cancellation transfer.
            let sem = Rc::new(RefCell::new(SemaphoreState::new(1)));
            let order = Rc::new(RefCell::new(Vec::new()));

            let p0 = Permit {
                sem: Rc::clone(&sem),
            };
            sem.borrow_mut().available = 0;

            let make_acquire = |prio: Priority, sem: &Rc<RefCell<SemaphoreState>>| {
                let mut future = AcquireFuture {
                    sem: Rc::clone(sem),
                    waiter_id: None,
                    entry: None,
                };
                Box::pin(async move {
                    poll_fn(|cx| {
                        let mut state = future.sem.borrow_mut();
                        if let Some(id) = future.waiter_id
                            && let Some(pos) = state.waiters.iter().position(|w| w.id == id)
                        {
                            if state.waiters[pos].granted {
                                state.waiters.remove(pos);
                                future.waiter_id = None;
                                return Poll::Ready(Permit {
                                    sem: Rc::clone(&future.sem),
                                });
                            }
                            return Poll::Pending;
                        }
                        if state.available > 0 {
                            state.available -= 1;
                            return Poll::Ready(Permit {
                                sem: Rc::clone(&future.sem),
                            });
                        }
                        state.next_id += 1;
                        let id = state.next_id;
                        future.waiter_id = Some(id);
                        state.waiters.push_back(SemaphoreWaiter {
                            id,
                            priority: prio,
                            waker: cx.waker().clone(),
                            source: None,
                            granted: false,
                        });
                        Poll::Pending
                    })
                    .await
                })
            };

            let spawn_waiter = |label: &'static str, prio: Priority| {
                let mut fut = make_acquire(prio, &sem);
                let order = Rc::clone(&order);
                glib::spawn_future_local(async move {
                    let _permit = fut.as_mut().await;
                    order.borrow_mut().push(label);
                })
            };

            let low = spawn_waiter("low", Priority::Low);
            let normal = spawn_waiter("normal", Priority::Normal);
            // Poll `high_cancelled` manually so dropping it deterministically runs AcquireFuture::drop.
            let mut high_cancelled = make_acquire(Priority::High, &sem);
            let noop = futures_task_noop_waker();
            let mut cx = std::task::Context::from_waker(&noop);
            assert!(high_cancelled.as_mut().poll(&mut cx).is_pending());
            let high = spawn_waiter("high", Priority::High);
            settle(5).await;

            // Drop initial permit: `high_cancelled` is granted first; dropping it
            // invokes AcquireFuture::drop and transfers the permit to `high`, then `normal`, then `low`.
            drop(p0);
            drop(high_cancelled);

            glib::future_with_timeout(Duration::from_secs(2), async {
                high.await.unwrap();
                normal.await.unwrap();
                low.await.unwrap();
            })
            .await
            .expect("waiters did not complete in 2s");
            assert_eq!(*order.borrow(), vec!["high", "normal", "low"]);
            assert_eq!(sem.borrow().available, 1);

            // Verify promote_download raises both InFlightEntry and queued semaphore waiter.
            let url = "https://example.com/promote-test.png";
            let entry = Rc::new(InFlightEntry::new(Priority::Normal));
            IN_FLIGHT.with(|map| {
                map.borrow_mut().insert(url.to_string(), Rc::clone(&entry));
            });
            DOWNLOAD_SEMAPHORE.with(|dsem| {
                let mut state = dsem.borrow_mut();
                state.next_id += 1;
                let id = state.next_id;
                entry.sem_waiter_id.set(Some(id));
                state.waiters.push_back(SemaphoreWaiter {
                    id,
                    priority: Priority::Normal,
                    waker: futures_task_noop_waker(),
                    source: None,
                    granted: false,
                });
            });
            promote_download(url, Priority::High);
            assert_eq!(entry.priority.get(), Priority::High);
            DOWNLOAD_SEMAPHORE.with(|dsem| {
                let id = entry.sem_waiter_id.get().unwrap();
                let mut state = dsem.borrow_mut();
                let w = state.waiters.iter().find(|w| w.id == id).unwrap();
                assert_eq!(w.priority, Priority::High);
                state.waiters.retain(|w| w.id != id);
            });
            IN_FLIGHT.with(|map| {
                map.borrow_mut().remove(url);
            });
        });
    }

    fn futures_task_noop_waker() -> Waker {
        use std::task::{RawWaker, RawWakerVTable};
        const VTABLE: RawWakerVTable =
            RawWakerVTable::new(|p| RawWaker::new(p, &VTABLE), |_| {}, |_| {}, |_| {});
        unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
    }
}
