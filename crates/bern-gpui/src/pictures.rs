//! Pictures from http(s) addresses, and the average color of a picture for
//! `Shape(fill_from:)`.
//!
//! GPUI's image loader keeps what a load gave, a failure too, and would
//! wait for ever on a stalled connection. So the application's HTTP client
//! is `Patient`: its answer comes once there is something to answer.
//!
//! An address the network could not reach, or that stays silent, is tried
//! again, each wait longer than the one before, for as long as a view shows
//! it. After the last wait it rests until the network changes or a window
//! comes to the front; a view that appears again starts over at once. An
//! address the server answered without a picture is not asked again on the
//! same network.

use futures::AsyncReadExt;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use http_client::{AsyncBody, HttpClient, Response, StatusCode, http};
use reqwest_client::ReqwestClient;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Once};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Seconds before each further try, give or take a quarter, so that
/// pictures that failed together do not all return together.
#[cfg(not(test))]
const WAITS: [f64; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
#[cfg(test)]
const WAITS: [f64; 2] = [0.02, 0.02];
/// How long a try may hear nothing before it counts as failed. A stalled
/// connection reports no error.
#[cfg(not(test))]
const SILENCE: Duration = Duration::from_secs(15);
#[cfg(test)]
const SILENCE: Duration = Duration::from_secs(1);
/// How often the network's addresses are compared once a picture has
/// failed: GPUI has no word on the network.
const NETWORK_CHECK: Duration = Duration::from_secs(5);

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
    Picture(Vec<u8>),
    /// Transport failure or timeout: renew the shared HTTP client.
    Unreachable,
    /// Server answered with a retryable status (408, 429, 5xx); keep the pool.
    Retry,
    /// The server answered, and not with a picture.
    Refused(StatusCode, Vec<u8>),
}

/// Who shows an address.
#[derive(Default)]
struct Views {
    count: usize,
    /// How many times it went from no views to one.
    appeared: u64,
}

type FetchResult = (StatusCode, Arc<Vec<u8>>);

struct MemoryEntry {
    bytes: Arc<Vec<u8>>,
    last_used: u64,
}

/// Bounded in-memory LRU cache storing validated raw picture bytes.
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

    fn get(&mut self, url: &str) -> Option<Arc<Vec<u8>>> {
        let entry = self.entries.get_mut(url)?;
        self.clock += 1;
        entry.last_used = self.clock;
        let bytes = entry.bytes.clone();
        self.order.push_back((url.to_string(), self.clock));
        self.compact_order();
        Some(bytes)
    }

    fn insert(&mut self, url: &str, bytes: Arc<Vec<u8>>) {
        let len = bytes.len();
        if len > self.limit {
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

/// Persistent disk cache backed by a platform cache directory.
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
        if bytes.len() > MAX_CACHE_FILE_BYTES || !picture(&bytes) {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&path) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    fn write(&self, url: &str, bytes: &[u8]) {
        if bytes.len() > MAX_CACHE_FILE_BYTES || !picture(bytes) {
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
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(|home| PathBuf::from(home).join("Library/Caches/bern-pictures"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .filter(|value| !value.is_empty())
                    .map(|profile| {
                        PathBuf::from(profile)
                            .join("AppData/Local")
                            .into_os_string()
                    })
            })
            .map(|base| PathBuf::from(base).join("bern-pictures"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|value| !value.is_empty())
                    .map(|home| PathBuf::from(home).join(".cache"))
            })
            .map(|base| base.join("bern-pictures"))
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
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    match parsed.host() {
        Some(url::Host::Domain(domain)) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost")
        }
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// Executor-agnostic async semaphore bounding concurrent remote downloads.
struct Semaphore {
    state: Mutex<SemaphoreState>,
}

struct SemaphoreState {
    available: usize,
    waiters: VecDeque<oneshot::Sender<()>>,
}

struct Permit<'a> {
    sem: &'a Semaphore,
}

struct AcquireGuard<'a> {
    sem: &'a Semaphore,
    acquired: bool,
}

impl Semaphore {
    const fn new(limit: usize) -> Self {
        Self {
            state: Mutex::new(SemaphoreState {
                available: limit,
                waiters: VecDeque::new(),
            }),
        }
    }

    fn wake_next(state: &mut SemaphoreState) {
        while let Some(tx) = state.waiters.pop_front() {
            if tx.send(()).is_ok() {
                break;
            }
        }
    }

    async fn acquire(&self) -> Permit<'_> {
        let mut guard = AcquireGuard {
            sem: self,
            acquired: false,
        };
        loop {
            let rx = {
                let mut state = self.state.lock().unwrap();
                if state.available > 0 {
                    state.available -= 1;
                    guard.acquired = true;
                    return Permit { sem: self };
                }
                let (tx, rx) = oneshot::channel();
                state.waiters.push_back(tx);
                rx
            };
            let _ = rx.await;
        }
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut state = self.sem.state.lock().unwrap();
        state.available += 1;
        Semaphore::wake_next(&mut state);
    }
}

impl Drop for AcquireGuard<'_> {
    fn drop(&mut self) {
        if !self.acquired {
            let mut state = self.sem.state.lock().unwrap();
            if state.available > 0 {
                Semaphore::wake_next(&mut state);
            }
        }
    }
}

/// Shared state for an in-flight download deduplicated by URL.
struct InFlight {
    state: Mutex<InFlightState>,
    view_waiters: AtomicUsize,
    viewless_waiters: AtomicUsize,
}

struct InFlightState {
    result: Option<FetchResult>,
    waiters: Vec<oneshot::Sender<FetchResult>>,
}

impl InFlight {
    fn new() -> Self {
        Self {
            state: Mutex::new(InFlightState {
                result: None,
                waiters: Vec::new(),
            }),
            view_waiters: AtomicUsize::new(0),
            viewless_waiters: AtomicUsize::new(0),
        }
    }

    fn has_viewless_waiters(&self) -> bool {
        self.viewless_waiters.load(Ordering::Acquire) > 0
    }

    fn total_waiters(&self) -> usize {
        self.view_waiters.load(Ordering::Acquire) + self.viewless_waiters.load(Ordering::Acquire)
    }
}

struct WaiterGuard {
    url: String,
    entry: Arc<InFlight>,
    for_views: bool,
}

impl WaiterGuard {
    fn new(url: &str, entry: Arc<InFlight>, for_views: bool) -> Self {
        if for_views {
            entry.view_waiters.fetch_add(1, Ordering::AcqRel);
        } else {
            entry.viewless_waiters.fetch_add(1, Ordering::AcqRel);
        }
        Self {
            url: url.to_string(),
            entry,
            for_views,
        }
    }
}

impl Drop for WaiterGuard {
    fn drop(&mut self) {
        if self.for_views {
            self.entry.view_waiters.fetch_sub(1, Ordering::AcqRel);
        } else {
            self.entry.viewless_waiters.fetch_sub(1, Ordering::AcqRel);
        }
        if self.entry.total_waiters() == 0 {
            wake_url(&self.url);
        }
    }
}

static CLIENT: LazyLock<Mutex<Arc<ReqwestClient>>> = LazyLock::new(|| Mutex::new(new_client()));
/// Addresses the server refused on this network.
static REFUSED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);
static VIEWS: LazyLock<Mutex<HashMap<String, Views>>> = LazyLock::new(Mutex::default);
/// An address waiting for its next try, and what wakes it.
type Pause = (String, oneshot::Sender<()>);
static PAUSES: LazyLock<Mutex<Vec<Pause>>> = LazyLock::new(Mutex::default);
static MEMORY_CACHE: LazyLock<Mutex<MemoryCache>> =
    LazyLock::new(|| Mutex::new(MemoryCache::new(MEMORY_CACHE_LIMIT)));
static DISK_CACHE: LazyLock<Option<DiskCache>> =
    LazyLock::new(|| default_cache_dir().map(DiskCache::new));
static IN_FLIGHT: LazyLock<Mutex<HashMap<String, Arc<InFlight>>>> = LazyLock::new(Mutex::default);
static DOWNLOAD_SEMAPHORE: Semaphore = Semaphore::new(MAX_CONCURRENT_REQUESTS);

fn new_client() -> Arc<ReqwestClient> {
    Arc::new(
        ReqwestClient::proxy_user_agent_and_read_timeout(None, "Bern", Some(SILENCE))
            .expect("Failed to initialize HTTP client"),
    )
}

/// The client GPUI's image loader asks, for pictures alone: nothing else in
/// a Bern application uses it.
pub(crate) struct Patient;

impl HttpClient for Patient {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&http_client::Url> {
        None
    }

    fn send(
        &self,
        request: http::Request<AsyncBody>,
    ) -> BoxFuture<'static, http_client::Result<Response<AsyncBody>>> {
        let url = request.uri().to_string();
        Box::pin(async move {
            let (status, bytes) = fetch(&url, true).await;
            Ok(Response::builder()
                .status(status)
                .body(AsyncBody::from(bytes))?)
        })
    }
}

/// One try at `url`, on the client's runtime: its read timeout runs on
/// that runtime's timers.
async fn read(client: Arc<ReqwestClient>, url: String) -> Reading {
    let _permit = DOWNLOAD_SEMAPHORE.acquire().await;
    let reading = reqwest_client::runtime().spawn(async move {
        let Ok(mut response) = client.get(&url, AsyncBody::empty(), true).await else {
            return Reading::Unreachable;
        };
        let mut bytes = Vec::new();
        if response.body_mut().read_to_end(&mut bytes).await.is_err() {
            return Reading::Unreachable;
        }
        let status = response.status();
        match status.as_u16() {
            200..=299 => Reading::Picture(bytes),
            408 | 429 | 500.. => Reading::Retry,
            _ => Reading::Refused(status, bytes),
        }
    });
    reading.await.unwrap_or(Reading::Unreachable)
}

fn cache_get(url: &str) -> Option<Vec<u8>> {
    if is_loopback(url) {
        return None;
    }
    if let Some(bytes) = MEMORY_CACHE.lock().unwrap().get(url) {
        return Some((*bytes).clone());
    }
    let bytes = DISK_CACHE.as_ref()?.read(url)?;
    MEMORY_CACHE
        .lock()
        .unwrap()
        .insert(url, Arc::new(bytes.clone()));
    Some(bytes)
}

fn cache_put(url: &str, bytes: Arc<Vec<u8>>) {
    if is_loopback(url) || !picture(&bytes) {
        return;
    }
    MEMORY_CACHE.lock().unwrap().insert(url, bytes.clone());
    if let Some(disk) = DISK_CACHE.as_ref() {
        disk.write(url, &bytes);
    }
}

/// The answer at `url`, once there is one worth giving; `for_views`, only
/// while a view shows it.
async fn fetch(url: &str, for_views: bool) -> (StatusCode, Vec<u8>) {
    if let Some(bytes) = cache_get(url) {
        return (StatusCode::OK, bytes);
    }

    let (entry, guard, rx, is_leader) = {
        let mut in_flight = IN_FLIGHT.lock().unwrap();
        let (entry, is_leader) = if let Some(existing) = in_flight.get(url).cloned() {
            (existing, false)
        } else {
            let created = Arc::new(InFlight::new());
            in_flight.insert(url.to_string(), created.clone());
            (created, true)
        };
        let guard = WaiterGuard::new(url, entry.clone(), for_views);
        let rx = {
            let mut state = entry.state.lock().unwrap();
            if let Some((status, bytes)) = &state.result {
                return (*status, (**bytes).clone());
            }
            let (tx, rx) = oneshot::channel();
            state.waiters.push(tx);
            rx
        };
        (entry, guard, rx, is_leader)
    };

    if is_leader {
        let url_owned = url.to_string();
        let entry_cloned = entry.clone();
        drop(reqwest_client::runtime().spawn(async move {
            drive_fetch(url_owned, entry_cloned).await;
        }));
    } else if !for_views {
        wake_url(url);
    }

    let result = rx.await;
    drop(guard);
    match result {
        Ok((status, bytes)) => (status, (*bytes).clone()),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Vec::new()),
    }
}

async fn drive_fetch(url: String, entry: Arc<InFlight>) {
    let mut tries = 0;
    let mut seen = appeared(&url);
    loop {
        if !entry.has_viewless_waiters() {
            while !shown(&url) && !entry.has_viewless_waiters() {
                if abandon_if_unused(&url, &entry) {
                    return;
                }
                pause_while_unshown(&url, &entry).await;
            }
            if !entry.has_viewless_waiters() {
                let now = appeared(&url);
                if now != seen {
                    seen = now;
                    tries = 0;
                }
            }
        }
        if abandon_if_unused(&url, &entry) {
            return;
        }
        if let Some(bytes) = cache_get(&url) {
            finish_in_flight(&url, &entry, (StatusCode::OK, Arc::new(bytes)));
            return;
        }

        let client = CLIENT.lock().unwrap().clone();
        match read(client.clone(), url.clone()).await {
            Reading::Picture(bytes) => {
                let valid = picture(&bytes);
                if !valid {
                    refuse(&url);
                }
                let shared = Arc::new(bytes);
                if valid {
                    cache_put(&url, shared.clone());
                }
                finish_in_flight(&url, &entry, (StatusCode::OK, shared));
                crate::host::redraw();
                return;
            }
            Reading::Refused(status, bytes) => {
                refuse(&url);
                let shared = Arc::new(bytes);
                finish_in_flight(&url, &entry, (status, shared));
                crate::host::redraw();
                return;
            }
            Reading::Unreachable => {
                listen();
                renew(&client);
                wait_and_retry(&url, &mut tries).await;
            }
            Reading::Retry => {
                listen();
                wait_and_retry(&url, &mut tries).await;
            }
        }
    }
}

async fn wait_and_retry(url: &str, tries: &mut usize) {
    let wait = WAITS.get(*tries).copied();
    pause(url, wait.map(|seconds| seconds * jitter())).await;
    *tries = if wait.is_some() { *tries + 1 } else { 0 };
    crate::host::redraw();
}

fn abandon_if_unused(url: &str, entry: &Arc<InFlight>) -> bool {
    if entry.total_waiters() > 0 {
        return false;
    }
    let mut in_flight = IN_FLIGHT.lock().unwrap();
    if entry.total_waiters() > 0 {
        return false;
    }
    if in_flight
        .get(url)
        .is_some_and(|current| Arc::ptr_eq(current, entry))
    {
        in_flight.remove(url);
    }
    true
}

fn finish_in_flight(url: &str, entry: &Arc<InFlight>, result: FetchResult) {
    {
        let mut in_flight = IN_FLIGHT.lock().unwrap();
        if in_flight
            .get(url)
            .is_some_and(|current| Arc::ptr_eq(current, entry))
        {
            in_flight.remove(url);
        }
    }
    let waiters = {
        let mut state = entry.state.lock().unwrap();
        state.result = Some(result.clone());
        std::mem::take(&mut state.waiters)
    };
    for waiter in waiters {
        let _ = waiter.send(result.clone());
    }
}

/// Requests to one server share a connection, and the next try would join
/// one that stalled. A client shares nothing with another.
fn renew(used: &Arc<ReqwestClient>) {
    let mut current = CLIENT.lock().unwrap();
    if Arc::ptr_eq(&current, used) {
        *current = new_client();
    }
}

/// Whether GPUI's image loader can make a picture of `bytes`: one of the
/// formats it knows, or an SVG, which has no signature to look for.
fn picture(bytes: &[u8]) -> bool {
    image::guess_format(bytes).is_ok() || bytes.windows(4).any(|part| part == b"<svg")
}

fn refuse(url: &str) {
    listen();
    REFUSED.lock().unwrap().insert(url.to_string());
}

/// A number between 0.75 and 1.25, from the seed of a new hasher.
fn jitter() -> f64 {
    use std::hash::{BuildHasher, Hasher};
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    0.75 + (random % 1000) as f64 / 2000.0
}

fn appeared(url: &str) -> u64 {
    VIEWS
        .lock()
        .unwrap()
        .get(url)
        .map_or(0, |views| views.appeared)
}

fn shown(url: &str) -> bool {
    VIEWS
        .lock()
        .unwrap()
        .get(url)
        .is_some_and(|views| views.count > 0)
}

fn wake_url(url: &str) {
    let woken = {
        let mut pauses = PAUSES.lock().unwrap();
        let (woken, kept): (Vec<_>, Vec<_>) =
            pauses.drain(..).partition(|(paused, _)| paused == url);
        *pauses = kept;
        woken
    };
    for (_, wake) in woken {
        let _ = wake.send(());
    }
}

async fn pause_while_unshown(url: &str, entry: &InFlight) {
    let (wake, woken) = oneshot::channel();
    {
        let mut pauses = PAUSES.lock().unwrap();
        pauses.retain(|(_, wake)| !wake.is_canceled());
        pauses.push((url.to_string(), wake));
    }
    if shown(url) || entry.has_viewless_waiters() || entry.total_waiters() == 0 {
        return;
    }
    let _ = woken.await;
}

/// Waits `seconds`, or with none for as long as it takes, until the network
/// changes, a window comes to the front or a view of `url` appears.
async fn pause(url: &str, seconds: Option<f64>) {
    let (wake, woken) = oneshot::channel();
    {
        let mut pauses = PAUSES.lock().unwrap();
        pauses.retain(|(_, wake)| !wake.is_canceled());
        pauses.push((url.to_string(), wake));
    }
    match seconds {
        Some(seconds) => {
            let delay = futures_timer::Delay::new(Duration::from_secs_f64(seconds));
            futures::future::select(delay, woken).await;
        }
        None => {
            let _ = woken.await;
        }
    }
}

/// Whether `url` waits for its next try.
pub(crate) fn paused(url: &str) -> bool {
    PAUSES
        .lock()
        .unwrap()
        .iter()
        .any(|(paused, wake)| paused == url && !wake.is_canceled())
}

/// Every load waiting for its next try goes now.
pub(crate) fn try_again() {
    for (_, wake) in PAUSES.lock().unwrap().drain(..) {
        let _ = wake.send(());
    }
}

/// A view of a picture from the network, for as long as it is there. The
/// first view of an address starts it over.
pub(crate) struct Watch(String);

impl Watch {
    pub(crate) fn new(url: &str) -> Self {
        let mut all = VIEWS.lock().unwrap();
        let views = all.entry(url.to_string()).or_default();
        views.count += 1;
        if views.count == 1 {
            views.appeared += 1;
            drop(all);
            wake_url(url);
        }
        Watch(url.to_string())
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some(views) = VIEWS.lock().unwrap().get_mut(&self.0) {
            views.count -= 1;
        }
    }
}

/// The picture at the http(s) address `url`, kept as long as a view is.
pub(crate) fn source(url: &str) -> gpui::ImageSource {
    let url = url.to_string();
    let resource = gpui::Resource::Uri(url.clone().into());
    gpui::ImageSource::Custom(Arc::new(move |window, cx| {
        window.use_keyed_state(gpui::ElementId::Name(url.clone().into()), cx, |_, _| {
            Watch::new(&url)
        });
        window.use_asset::<gpui::ImgResourceLoader>(&resource, cx)
    }))
}

/// The network's addresses, in an order to compare.
fn addresses() -> Vec<String> {
    let mut addresses: Vec<String> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .map(|interface| format!("{} {}", interface.name, interface.ip()))
        .collect();
    addresses.sort();
    addresses
}

/// Starts watching the network, on the first failure.
fn listen() {
    static LISTENING: Once = Once::new();
    LISTENING.call_once(|| {
        std::thread::spawn(|| {
            let mut last = addresses();
            loop {
                std::thread::sleep(NETWORK_CHECK);
                let now = addresses();
                if now != last && !now.is_empty() {
                    network_changed();
                }
                last = now;
            }
        });
    });
}

/// Once it is there after a change, a new way out may answer what the last
/// one could not.
fn network_changed() {
    crate::host::pictures_changed();
    try_again();
}

/// What was refused may come on this network: GPUI's loader forgets it, and
/// fills ask again.
pub(crate) fn forget_refused(cx: &mut gpui::App) {
    for url in REFUSED.lock().unwrap().drain() {
        cx.remove_asset::<gpui::ImgResourceLoader>(&gpui::Resource::Uri(url.into()));
    }
    if let Ok(mut averages) = AVERAGES.lock() {
        averages.retain(|_, average| !matches!(average, Average::Known(None)));
    }
}

enum Average {
    Pending,
    Known(Option<[u8; 3]>),
}

static AVERAGES: LazyLock<Mutex<HashMap<String, Average>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The average color of the picture at `url` (a file or an http(s)
/// address); none until it is read, or when it cannot be.
pub(crate) fn average(url: &str) -> Option<gpui::Rgba> {
    if url.is_empty() {
        return None;
    }
    let mut averages = AVERAGES.lock().ok()?;
    match averages.get(url) {
        Some(Average::Known(color)) => {
            return color.map(|[r, g, b]| gpui::Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: 1.0,
            });
        }
        Some(Average::Pending) => return None,
        None => {}
    }
    averages.insert(url.to_string(), Average::Pending);
    let url = url.to_string();
    std::thread::spawn(move || {
        let color = read_all(&url).and_then(|bytes| mean(&bytes));
        if let Ok(mut averages) = AVERAGES.lock() {
            averages.insert(url, Average::Known(color));
        }
        crate::host::redraw();
    });
    None
}

/// The bytes of a file, or of an http(s) address once they come.
fn read_all(url: &str) -> Option<Vec<u8>> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() == "file" {
        return std::fs::read(parsed.to_file_path().ok()?).ok();
    }
    let (status, bytes) = futures::executor::block_on(fetch(url, false));
    status.is_success().then_some(bytes)
}

fn mean(bytes: &[u8]) -> Option<[u8; 3]> {
    let picture = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(24, 24)
        .into_rgb8();
    let count = u64::from(picture.width()) * u64::from(picture.height());
    if count == 0 {
        return None;
    }
    let mut sum = [0u64; 3];
    for pixel in picture.pixels() {
        for (total, channel) in sum.iter_mut().zip(pixel.0) {
            *total += u64::from(channel);
        }
    }
    Some(sum.map(|total| (total / count) as u8))
}

#[cfg(test)]
#[path = "../../bern-target-api/tests/faulty_server/mod.rs"]
mod faulty_server;

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::thread::{JoinHandle, sleep, spawn};
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

    /// How often `path` was asked for, once that has stopped changing.
    fn settled(address: SocketAddr, path: &str) -> usize {
        let mut count = asked(address, path);
        let mut still = 0;
        while still < 4 || count == 0 {
            sleep(Duration::from_millis(250));
            let now = asked(address, path);
            still = if now == count { still + 1 } else { 0 };
            count = now;
        }
        count
    }

    fn load(url: String) -> JoinHandle<u16> {
        spawn(move || {
            block_on(Patient.get(&url, AsyncBody::empty(), true))
                .unwrap()
                .status()
                .as_u16()
        })
    }

    fn until_paused(url: &str) {
        while !paused(url) {
            sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn waits_differ_by_up_to_a_quarter() {
        let waits: Vec<f64> = (0..100).map(|_| jitter()).collect();
        assert!(waits.iter().all(|wait| (0.75..=1.25).contains(wait)));
        assert!(waits.iter().any(|wait| *wait != waits[0]));
    }

    #[test]
    fn pictures_come_after_failures_and_refusals_are_not_repeated() {
        let (address, silent) = faulty_server::start();
        let url = |path: &str| format!("http://{address}{path}");
        let shown = |path: &str| Watch::new(&url(path));
        spawn(|| {
            sleep(Duration::from_secs(60));
            eprintln!("timed out");
            std::process::abort();
        });

        // A failure on the way, and the picture still comes.
        let view = shown("/breaks/1");
        assert_eq!(load(url("/breaks/1")).join().unwrap(), 200);
        assert!(asked(address, "/breaks/1") >= 3);
        drop(view);
        let before_busy = CLIENT.lock().unwrap().clone();
        let view = shown("/busy/1");
        assert_eq!(load(url("/busy/1")).join().unwrap(), 200);
        assert_eq!(asked(address, "/busy/1"), 3);
        assert!(Arc::ptr_eq(&before_busy, &CLIENT.lock().unwrap()));
        drop(view);

        // A connection that stalls reports nothing, and is given up on: the
        // server stays silent for 20 seconds, and a try may wait for 1.
        let stalled = Instant::now();
        let first = CLIENT.lock().unwrap().clone();
        let view = shown("/stalls/1");
        assert_eq!(load(url("/stalls/1")).join().unwrap(), 200);
        assert_eq!(asked(address, "/stalls/1"), 2);
        assert!(stalled.elapsed() < Duration::from_secs(10));
        assert!(!Arc::ptr_eq(&first, &CLIENT.lock().unwrap()));
        drop(view);

        // Nothing listens at first, as while a proxy starts.
        let late = format!("http://127.0.0.1:{silent}/picture");
        let view = Watch::new(&late);
        let starting = load(late.clone());
        until_paused(&late);
        text(address, &format!("/listen/{silent}"));
        try_again();
        assert_eq!(starting.join().unwrap(), 200);
        drop(view);

        // Tries end, and a load rests until something changes.
        let view = shown("/down/1");
        let down = load(url("/down/1"));
        let tried = settled(address, "/down/1");
        assert!(tried >= 3, "{tried}");
        try_again();
        assert!(settled(address, "/down/1") > tried);
        text(address, "/heal/down/1");
        try_again();
        assert_eq!(down.join().unwrap(), 200);
        drop(view);

        // A view that goes away stops the tries, and one that appears
        // again starts over.
        let view = shown("/down/2");
        let going = load(url("/down/2"));
        until_paused(&url("/down/2"));
        drop(view);
        let left = settled(address, "/down/2");
        sleep(Duration::from_millis(300));
        assert_eq!(asked(address, "/down/2"), left);
        try_again();
        sleep(Duration::from_millis(300));
        assert_eq!(asked(address, "/down/2"), left, "no view, no try");
        text(address, "/heal/down/2");
        let view = shown("/down/2");
        assert_eq!(going.join().unwrap(), 200);
        drop(view);

        // An answer that is no picture ends the load, and GPUI's loader
        // keeps it: only a changed network asks again.
        let view = shown("/gone/1");
        assert_eq!(load(url("/gone/1")).join().unwrap(), 404);
        assert_eq!(settled(address, "/gone/1"), 1);
        assert!(REFUSED.lock().unwrap().contains(&url("/gone/1")));
        drop(view);
        let view = shown("/page/1");
        assert_eq!(load(url("/page/1")).join().unwrap(), 200);
        assert_eq!(settled(address, "/page/1"), 1);
        assert!(REFUSED.lock().unwrap().contains(&url("/page/1")));
        drop(view);

        // A fill has no view, and waits as long as it takes.
        let color = url("/busy/2");
        assert_eq!(average(&color), None);
        while matches!(AVERAGES.lock().unwrap().get(&color), Some(Average::Pending)) {
            sleep(Duration::from_millis(10));
        }
        assert_eq!(asked(address, "/busy/2"), 3);
        assert!(average(&color).is_some());
    }

    #[test]
    fn loopback_and_cache_key_behave_deterministically() {
        assert!(is_loopback("http://127.0.0.1:8080/a.png"));
        assert!(is_loopback("http://localhost:3000/a.png"));
        assert!(is_loopback("http://[::1]:9000/a.png"));
        assert!(!is_loopback("https://example.com/a.png"));

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
        cache.insert("a", Arc::new(vec![1, 2, 3, 4]));
        cache.insert("b", Arc::new(vec![5, 6, 7, 8]));
        assert!(cache.get("a").is_some());
        cache.insert("c", Arc::new(vec![9, 10, 11, 12]));

        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
        assert!(cache.get("c").is_some());
        assert_eq!(cache.total_bytes, 8);

        cache.insert("huge", Arc::new(vec![0; 11]));
        assert!(cache.get("huge").is_none());
        assert_eq!(cache.total_bytes, 8);
    }

    #[test]
    fn disk_cache_validates_writes_atomically_and_prunes() {
        let dir = std::env::temp_dir().join(format!(
            "bern-pictures-test-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let disk = DiskCache::new(dir.clone());

        disk.write("https://example.com/bad", b"<html>not an image</html>");
        assert!(disk.read("https://example.com/bad").is_none());

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

        // Populate extra files with distinct mtimes and verify pruning down to target.
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
    fn retry_statuses_do_not_return_unreachable_for_pool_renewal() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        spawn(move || {
            for stream in listener.incoming().flatten() {
                spawn(move || {
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
                    let status = match path {
                        "/408" => "408 Request Timeout",
                        "/429" => "429 Too Many Requests",
                        "/502" => "502 Bad Gateway",
                        _ => "200 OK",
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(head.as_bytes());
                });
            }
        });

        let client = new_client();
        for path in ["/408", "/429", "/502"] {
            let result = block_on(read(client.clone(), format!("http://{addr}{path}")));
            assert!(matches!(result, Reading::Retry), "unexpected for {path}");
        }
    }

    #[test]
    fn inflight_deduplicates_patient_and_average_and_limits_concurrency() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let hits_srv = hits.clone();
        let active_srv = active.clone();
        let max_srv = max_active.clone();
        spawn(move || {
            for stream in listener.incoming().flatten() {
                let hits = hits_srv.clone();
                let active = active_srv.clone();
                let max_active = max_srv.clone();
                spawn(move || {
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
                    sleep(Duration::from_millis(120));
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

        let shared_url = format!("http://{addr}/shared");
        let _view = Watch::new(&shared_url);
        let p1 = load(shared_url.clone());
        let p2 = load(shared_url.clone());
        assert_eq!(average(&shared_url), None);
        assert_eq!(p1.join().unwrap(), 200);
        assert_eq!(p2.join().unwrap(), 200);
        while matches!(
            AVERAGES.lock().unwrap().get(&shared_url),
            Some(Average::Pending)
        ) {
            sleep(Duration::from_millis(10));
        }
        assert!(average(&shared_url).is_some());
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        let mut handles = Vec::new();
        let mut views = Vec::new();
        for idx in 0..16 {
            let item_url = format!("http://{addr}/item/{idx}");
            views.push(Watch::new(&item_url));
            handles.push(load(item_url));
        }
        for handle in handles {
            assert_eq!(handle.join().unwrap(), 200);
        }
        drop(views);
        let peak = max_active.load(Ordering::SeqCst);
        assert!(peak <= MAX_CONCURRENT_REQUESTS, "peak={peak}");
    }
}
