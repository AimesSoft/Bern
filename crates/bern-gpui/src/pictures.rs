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
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, Once};
use std::time::Duration;

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

/// What one try at an address gave.
enum Reading {
    Picture(Vec<u8>),
    /// The network failed, or the server could not answer now.
    Unreachable,
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

static CLIENT: LazyLock<Mutex<Arc<ReqwestClient>>> = LazyLock::new(|| Mutex::new(new_client()));
/// Addresses the server refused on this network.
static REFUSED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);
static VIEWS: LazyLock<Mutex<HashMap<String, Views>>> = LazyLock::new(Mutex::default);
/// An address waiting for its next try, and what wakes it.
type Pause = (String, oneshot::Sender<()>);
static PAUSES: LazyLock<Mutex<Vec<Pause>>> = LazyLock::new(Mutex::default);

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
            408 | 429 | 500.. => Reading::Unreachable,
            _ => Reading::Refused(status, bytes),
        }
    });
    reading.await.unwrap_or(Reading::Unreachable)
}

/// The answer at `url`, once there is one worth giving; `for_views`, only
/// while a view shows it.
async fn fetch(url: &str, for_views: bool) -> (StatusCode, Vec<u8>) {
    let mut tries = 0;
    let mut seen = appeared(url);
    loop {
        if for_views {
            while !shown(url) {
                pause(url, None).await;
            }
            let now = appeared(url);
            if now != seen {
                seen = now;
                tries = 0;
            }
        }
        let client = CLIENT.lock().unwrap().clone();
        match read(client.clone(), url.to_string()).await {
            Reading::Picture(bytes) => {
                if !picture(&bytes) {
                    refuse(url);
                }
                crate::host::redraw();
                return (StatusCode::OK, bytes);
            }
            Reading::Refused(status, bytes) => {
                refuse(url);
                crate::host::redraw();
                return (status, bytes);
            }
            Reading::Unreachable => {
                listen();
                renew(&client);
                let wait = WAITS.get(tries).copied();
                pause(url, wait.map(|seconds| seconds * jitter())).await;
                tries = if wait.is_some() { tries + 1 } else { 0 };
                crate::host::redraw();
            }
        }
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
            let mut pauses = PAUSES.lock().unwrap();
            let (woken, kept): (Vec<_>, Vec<_>) =
                pauses.drain(..).partition(|(paused, _)| paused == url);
            *pauses = kept;
            for (_, wake) in woken {
                let _ = wake.send(());
            }
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
        let view = shown("/busy/1");
        assert_eq!(load(url("/busy/1")).join().unwrap(), 200);
        assert_eq!(asked(address, "/busy/1"), 3);
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
}
