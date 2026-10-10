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

mod channel;
mod digest;
mod frame;
mod http;

pub use channel::{Connection, Env, Outbound};

use bern_adapter_json::ServerMessage;
use bern_command::Host;
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

/// A running browser channel. Dropping it stops accepting connections.
pub struct Server {
    address: SocketAddr,
    token: String,
    stop: Arc<AtomicBool>,
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
        let address = listener.local_addr()?;
        let token = digest::random_token(16)?;
        let env = Env::new(host, sessions, media, token.clone());
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
                    let _ = thread::Builder::new()
                        .name("bern-ws-connection".into())
                        .spawn(move || {
                            let _ = serve(stream, env, &root, address.port());
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

    /// The page address with its credential in the fragment.
    pub fn url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/#token={}",
            self.address.port(),
            self.token
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.address);
    }
}

/// Whether `Host` names this server.
fn own_host(value: &str, port: u16) -> bool {
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| value.eq_ignore_ascii_case(&format!("{name}:{port}")))
}

fn serve(stream: TcpStream, env: Arc<Env>, root: &std::path::Path, port: u16) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let request = http::read_request(&mut reader)?;
    let Some(host) = request.header("Host").filter(|host| own_host(host, port)) else {
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
    match request.method.as_str() {
        "GET" | "HEAD" => match http::resolve(root, &request.path) {
            Some(file) => http::serve_file(
                &mut writer,
                &file,
                request.header("Range"),
                request.method == "HEAD",
            ),
            None => http::error(&mut writer, "404 Not Found"),
        },
        _ => http::error(&mut writer, "405 Method Not Allowed"),
    }
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
