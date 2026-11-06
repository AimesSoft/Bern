//! The engine host's browser channel for WebUI pages.
//!
//! A WebUI engine host runs the engine natively and serves its page over
//! HTTP on the loopback interface, together with a WebSocket channel that
//! carries the same JSON control messages as the local IPC channel. The CLI,
//! JSON clients and the MCP server keep using the IPC channel; every entry
//! reaches the same engine instance and the same live UI sessions.
//!
//! Access rules:
//! * The server listens on 127.0.0.1 only.
//! * Every request must name the server itself in `Host`, so a page from
//!   another site cannot reach it through DNS rebinding.
//! * The WebSocket upgrade requires an `Origin` equal to that host: only the
//!   page this server serves may connect.
//! * The first message must carry the token generated at start. The token
//!   travels in the address fragment, which browsers never send to servers,
//!   so the static files reveal nothing; the address is the credential.
//!
//! [`FileServer`] serves a page alone, without a channel, for builds whose
//! engine runs in the page itself.
//!
//! An engine may also ask for the page to reach other devices (see
//! `bern_command::web_ui`). [`install`] lets it: that server listens on every
//! interface and takes any `Host`, since a browser opens it by whatever address
//! it found, and asks no token, since the address alone must open it. The
//! upgrade still requires the `Origin` of the server itself, so a page of
//! another site cannot drive the engine. The application's routes answer
//! first, and the page answers the rest.

mod channel;
mod digest;
mod frame;
mod http;

pub use channel::{Connection, Env, Outbound};

use bern_adapter_json::ServerMessage;
use bern_command::{Host, web_ui};
use bern_media::MediaHub;
use bern_runtime::Sessions;
use std::io::{self, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// Path of the WebSocket endpoint.
pub const ENDPOINT: &str = "/bern";
/// Where a page reads a file of the app by its `file:` URL, such as a picture
/// the engine named.
const FILES: &str = "/__bern/file";

/// A running browser channel. Dropping it stops accepting connections.
pub struct Server {
    address: SocketAddr,
    token: Option<String>,
    stop: Arc<AtomicBool>,
}

/// How one server treats a request.
#[derive(Clone)]
struct Policy {
    /// Any `Host` is welcome.
    open: bool,
    routes: Option<Arc<dyn web_ui::Routes>>,
}

/// Lets the engine serve this page to other devices, through
/// `bern_command::web_ui`, for as long as it wants to.
pub fn install(host: Host, sessions: Sessions, media: MediaHub, root: PathBuf) {
    web_ui::install(Box::new(move |options| {
        let server = Server::start_open(
            host.clone(),
            sessions.clone(),
            media.clone(),
            root.clone(),
            options,
        )
        .map_err(|error| match error.kind() {
            io::ErrorKind::AddrInUse => format!("port {} is in use", options.port),
            io::ErrorKind::PermissionDenied => {
                format!("no permission to listen on port {}", options.port)
            }
            _ => error.to_string(),
        })?;
        let port = server.address().port();
        Ok((Box::new(server) as web_ui::Running, port))
    }));
}

impl Server {
    /// Serve the page in `root` and the WebSocket channel for `host`, its
    /// media players and live UI `sessions`, on 127.0.0.1 at `port` (0
    /// picks a free port).
    pub fn start(
        host: Host,
        sessions: Sessions,
        media: MediaHub,
        root: PathBuf,
        port: u16,
    ) -> io::Result<Server> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let token = digest::random_token(16)?;
        let env = Env::new(host, sessions, media, Some(token.clone()));
        let policy = Policy {
            open: false,
            routes: None,
        };
        Self::accept(listener, env, root, policy, Some(token))
    }

    /// The same for other devices: on every interface, with no token, and
    /// the application's routes before the page.
    pub fn start_open(
        host: Host,
        sessions: Sessions,
        media: MediaHub,
        root: PathBuf,
        options: &web_ui::Options,
    ) -> io::Result<Server> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, options.port))?;
        let env = Env::new(host, sessions, media, None);
        let policy = Policy {
            open: true,
            routes: options.routes.clone(),
        };
        Self::accept(listener, env, root, policy, None)
    }

    fn accept(
        listener: TcpListener,
        env: Arc<Env>,
        root: PathBuf,
        policy: Policy,
        token: Option<String>,
    ) -> io::Result<Server> {
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let root = Arc::new(root);
        thread::Builder::new()
            .name("bern-ws".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let env = Arc::clone(&env);
                    let root = Arc::clone(&root);
                    let policy = policy.clone();
                    let _ = thread::Builder::new()
                        .name("bern-ws-connection".into())
                        .spawn(move || {
                            let _ = serve(stream, env, &root, address.port(), &policy);
                        });
                }
            })?;
        Ok(Server {
            address,
            token,
            stop,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The page address, with its credential in the fragment if it has one.
    pub fn url(&self) -> String {
        let port = self.address.port();
        match &self.token {
            Some(token) => format!("http://127.0.0.1:{port}/#token={token}"),
            None => format!("http://127.0.0.1:{port}/"),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // The accepting thread wakes by a connection of its own.
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.address.port()));
    }
}

/// Whether `Host` names this server.
fn own_host(value: &str, port: u16) -> bool {
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| value.eq_ignore_ascii_case(&format!("{name}:{port}")))
}

fn serve(
    stream: TcpStream,
    env: Arc<Env>,
    root: &std::path::Path,
    port: u16,
    policy: &Policy,
) -> io::Result<()> {
    let peer = stream.peer_addr().ok().map(|address| address.ip());
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let request = http::read_request(&mut reader)?;
    let Some(host) = request
        .header("Host")
        .filter(|host| policy.open || own_host(host, port))
    else {
        return http::error(&mut writer, "403 Forbidden");
    };
    if request.path == ENDPOINT {
        let upgrade = request
            .header("Upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
        let origin = request.header("Origin") == Some(&format!("http://{host}"));
        let (Some(key), true, true, Some("13")) = (
            request.header("Sec-WebSocket-Key"),
            upgrade,
            request.method == "GET",
            request.header("Sec-WebSocket-Version"),
        ) else {
            return http::error(&mut writer, "400 Bad Request");
        };
        if !origin {
            return http::error(&mut writer, "403 Forbidden");
        }
        let accept = digest::accept_key(key);
        writer.write_all(
            format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            )
            .as_bytes(),
        )?;
        writer.flush()?;
        return websocket(reader, writer, env);
    }
    if request.path == FILES {
        return match app_file(&request.query) {
            Some(file) => http::serve_file(
                &mut writer,
                &file,
                request.header("Range"),
                request.method == "HEAD",
            ),
            None => http::error(&mut writer, "404 Not Found"),
        };
    }
    if let Some(routes) = &policy.routes {
        let asked = web_ui::Request {
            body: http::read_body(&mut reader, &request)?,
            method: request.method.clone(),
            path: request.path.clone(),
            query: request.query.clone(),
            headers: request.headers.clone(),
            peer,
        };
        if let Some(response) = routes.handle(&asked, &env.host) {
            return http::send(&mut writer, &response, request.method == "HEAD");
        }
    }
    serve_files(&mut writer, &request, root)
}

/// The file `url=` names, if it is in the app's own folders: a page sees what
/// the app keeps, and nothing else of the device.
fn app_file(query: &str) -> Option<PathBuf> {
    let url = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("url="))
        .map(web_ui::decode)?;
    let path = web_ui::decode(&url.strip_prefix("file://")?.replace('+', "%2B"));
    #[cfg(windows)]
    let path = path.trim_start_matches('/').to_string();
    let file = std::fs::canonicalize(path).ok()?;
    [
        bern_command::storage::cache_dir(),
        bern_command::storage::data_dir(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|folder| std::fs::canonicalize(folder).ok())
    .any(|folder| file.starts_with(folder))
    .then_some(file)
}

/// Answer a request for a file below `root`.
fn serve_files(
    writer: &mut TcpStream,
    request: &http::Request,
    root: &std::path::Path,
) -> io::Result<()> {
    match request.method.as_str() {
        "GET" | "HEAD" => match http::resolve(root, &request.path) {
            Some(file) => http::serve_file(
                writer,
                &file,
                request.header("Range"),
                request.method == "HEAD",
            ),
            None => http::error(writer, "404 Not Found"),
        },
        _ => http::error(writer, "405 Method Not Allowed"),
    }
}

/// Serves the files of a page whose engine runs in the page itself, such
/// as a `web_app` build, for `bern run`: 127.0.0.1 only, with the same
/// `Host` rule as [`Server`], and no WebSocket channel.
pub struct FileServer {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl FileServer {
    /// Serve `root` on 127.0.0.1 at `port` (0 picks a free port).
    pub fn start(root: PathBuf, port: u16) -> io::Result<FileServer> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let root = Arc::new(root);
        thread::Builder::new()
            .name("bern-files".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let root = Arc::clone(&root);
                    let _ = thread::Builder::new()
                        .name("bern-files-connection".into())
                        .spawn(move || {
                            let _ = serve_page(stream, &root, address.port());
                        });
                }
            })?;
        Ok(FileServer { address, stop })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The page address.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.address.port())
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
    }
}

fn serve_page(stream: TcpStream, root: &std::path::Path, port: u16) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let request = http::read_request(&mut reader)?;
    if !request
        .header("Host")
        .is_some_and(|host| own_host(host, port))
    {
        return http::error(&mut writer, "403 Forbidden");
    }
    serve_files(&mut writer, &request, root)
}

fn websocket(reader: BufReader<TcpStream>, writer: TcpStream, env: Arc<Env>) -> io::Result<()> {
    let writer = Arc::new(Mutex::new(writer));
    let out_writer = Arc::clone(&writer);
    let out: Outbound = Arc::new(move |message: &ServerMessage| {
        if let Ok(text) = serde_json::to_string(message) {
            let mut stream = out_writer.lock().expect("socket lock");
            let _ = frame::write_frame(&mut *stream, 0x1, text.as_bytes());
        }
    });
    let connection = Connection::new(env, out);
    // Reads use the buffered reader; pongs and close replies go through
    // the shared writer so they never interleave with messages.
    let mut duplex = Duplex {
        reader,
        writer: Arc::clone(&writer),
    };
    let result = loop {
        match frame::read_message(&mut duplex) {
            Ok(frame::Incoming::Text(text)) => {
                if !connection.receive(&text) {
                    let mut stream = writer.lock().expect("socket lock");
                    let _ = frame::write_frame(&mut *stream, 0x8, &[0x03, 0xf0]);
                    break Ok(());
                }
            }
            Ok(frame::Incoming::Close) => break Ok(()),
            Err(problem) => break Err(problem),
        }
    };
    connection.closed();
    result
}

struct Duplex {
    reader: BufReader<TcpStream>,
    writer: Arc<Mutex<TcpStream>>,
}

impl io::Read for Duplex {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buffer)
    }
}

impl io::Write for Duplex {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.writer.lock().expect("socket lock").write(buffer)
    }

    fn write_all(&mut self, buffer: &[u8]) -> io::Result<()> {
        self.writer.lock().expect("socket lock").write_all(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.lock().expect("socket lock").flush()
    }
}

#[cfg(test)]
mod tests;
