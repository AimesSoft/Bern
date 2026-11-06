//! A WebUI served from a native engine: a page in any browser becomes one
//! more UI session of the engine that runs here, and the application can
//! answer HTTP requests of its own on the same port.
//!
//! A build carries the page when its target names `web_ui` in `Bern.toml`; an
//! engine host always has one. The engine says what it wants with [`set`],
//! whenever it likes, and reads the outcome from [`status`]: the runtime
//! starts the server once it is up, and a change of options restarts it.
//!
//! The server listens on every interface and asks no credential, which is
//! what a browser on another device opening an address needs. An application
//! offers it as something the user turns on.

use std::any::Any;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// An HTTP request for the application's routes.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub method: String,
    /// The path, percent-encoded as it came.
    pub path: String,
    /// What followed the `?`, percent-encoded as it came.
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub peer: Option<IpAddr>,
}

impl Request {
    /// The value of a header, whatever its case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The decoded value of a query parameter.
    pub fn param(&self, name: &str) -> Option<String> {
        self.query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key) == name).then(|| decode(value))
        })
    }
}

/// `+` and percent escapes decoded, as a query or a path segment holds them.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[index], escaped) {
            (_, Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (b'+', None) => {
                out.push(b' ');
                index += 1;
            }
            (byte, None) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a response carries.
#[derive(Clone, Debug)]
pub enum Body {
    Empty,
    Bytes(Vec<u8>),
    /// Part of a file, read as it is sent. The headers are the application's,
    /// `Content-Length` among them.
    File {
        path: PathBuf,
        offset: u64,
        length: u64,
    },
}

/// An HTTP response from the application's routes. The server sends no body
/// for `HEAD` and adds `Connection: close`.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

/// The HTTP requests an application answers itself, on the page's port.
pub trait Routes: Send + Sync {
    /// The answer, or `None` for the page and its files to answer. Called on
    /// the connection's own thread, which may wait for the engine: `host`
    /// runs its commands as any entry does.
    fn handle(&self, request: &Request, host: &crate::Host) -> Option<Response>;
}

/// How the engine wants the server.
#[derive(Clone)]
pub struct Options {
    pub port: u16,
    pub routes: Option<Arc<dyn Routes>>,
}

/// Where the server stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Not wanted, or this build has no page.
    Stopped,
    Running {
        port: u16,
    },
    /// Wanted, and it could not listen.
    Failed(String),
}

/// A running server; dropping it stops it.
pub type Running = Box<dyn Any + Send>;
/// Starts the server, giving the port it listens on.
pub type Starter = Box<dyn Fn(&Options) -> Result<(Running, u16), String> + Send>;

struct State {
    starter: Option<Starter>,
    wanted: Option<Options>,
    running: Option<Running>,
    status: Status,
}

static STATE: Mutex<State> = Mutex::new(State {
    starter: None,
    wanted: None,
    running: None,
    status: Status::Stopped,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn apply(state: &mut State) {
    // The old server lets go of its port first.
    state.running = None;
    state.status = Status::Stopped;
    if let (Some(starter), Some(options)) = (&state.starter, &state.wanted) {
        match starter(options) {
            Ok((running, port)) => {
                state.running = Some(running);
                state.status = Status::Running { port };
            }
            Err(message) => state.status = Status::Failed(message),
        }
    }
}

/// The name below the app's resources of the folder a native build keeps the
/// page in.
pub const PAGE_FOLDER: &str = "__bern_web_ui";

/// The folder of the page this build carries among its resources.
pub fn page() -> Option<PathBuf> {
    let name = format!("{PAGE_FOLDER}/index.html");
    let index = crate::resources::Resources::host().path(&name).ok()?;
    index.is_file().then(|| index.parent().map(PathBuf::from))?
}

/// Called by the runtime once the engine is up: the build has a page.
pub fn install(starter: Starter) {
    let mut state = state();
    state.starter = Some(starter);
    apply(&mut state);
}

/// Whether this build carries a page. Answers once the engine is up.
pub fn available() -> bool {
    state().starter.is_some()
}

/// Serve the page with these options, or stop serving it.
pub fn set(options: Option<Options>) {
    let mut state = state();
    state.wanted = options;
    apply(&mut state);
}

pub fn status() -> Status {
    state().status.clone()
}
