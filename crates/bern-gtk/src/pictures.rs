//! Pictures from files and http(s) addresses, read in the background.
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
use std::collections::HashSet;
use std::time::Duration;

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

thread_local! {
    static SESSION: soup::Session = new_session();
    /// Addresses the server refused on this network.
    static REFUSED: RefCell<HashSet<String>> = RefCell::default();
    /// Cancelled to wake every load waiting for its next try.
    static WAKE: RefCell<gio::Cancellable> = RefCell::new(gio::Cancellable::new());
    static LISTENING: Cell<bool> = const { Cell::new(false) };
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

/// One try at the bytes at `url`: a `file://` URL or path, or an http(s)
/// address.
async fn read(url: &str) -> Reading {
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
    let Ok(message) = soup::Message::new("GET", url) else {
        return Reading::Refused;
    };
    let session = SESSION.with(soup::Session::clone);
    match session
        .send_and_read_future(&message, glib::Priority::DEFAULT)
        .await
    {
        Ok(bytes) => match message.status_code() {
            200..=299 => Reading::Bytes(bytes),
            408 | 429 | 500.. => Reading::Unreachable,
            _ => Reading::Refused,
        },
        Err(error) if error.matches(gio::TlsError::BadCertificate) => Reading::Refused,
        Err(_) => Reading::Unreachable,
    }
}

/// What `decode` makes of the bytes at `url`, once they come and it can;
/// none when there is no such file. Dropped, it stops: a view's load ends
/// with the view, and a view that appears again starts over.
pub(crate) async fn picture<T>(
    url: &str,
    mut decode: impl AsyncFnMut(glib::Bytes) -> Option<T>,
) -> Option<T> {
    listen();
    let mut tries = 0;
    loop {
        if REFUSED.with(|refused| refused.borrow().contains(url)) {
            pause(None).await;
            continue;
        }
        match read(url).await {
            Reading::Bytes(bytes) => {
                if let Some(picture) = decode(bytes).await {
                    return Some(picture);
                }
                if !remote(url) {
                    return None;
                }
                refuse(url);
            }
            Reading::Unreachable => wait(&mut tries).await,
            Reading::Refused => refuse(url),
            Reading::Absent => return None,
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
    use std::net::{SocketAddr, TcpStream};
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
}
