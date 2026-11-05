//! A local server that fails on purpose, for the tests of how each target
//! fetches pictures. A test includes it with `#[path]`.
//!
//! The first part of a path says how it behaves, and the rest tells pictures
//! apart: `/breaks/1` and `/breaks/2` are counted separately.
//!
//! - `/breaks/…`: the connection closes twice, then the picture comes.
//! - `/stalls/…`: the first connection stays open and silent, as a stalled
//!   one does, then the picture comes.
//! - `/busy/…`: 503 twice, then the picture comes.
//! - `/down/…`: the connection closes until the test heals it.
//! - `/gone/…`: 404 until the test heals it.
//! - `/page/…`: a page, not a picture, until the test heals it.
//! - anything else: the picture.
//!
//! The test's own questions count for nothing: `/asked<path>` answers how
//! often `<path>` was asked for, `/heal<path>` heals it and `/listen/<port>`
//! starts listening on a port nothing listened on before.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One pixel.
pub const PICTURE: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f,
    0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Default)]
struct Server {
    /// Requests seen, by path.
    asked: HashMap<String, usize>,
    /// Paths that have recovered.
    healed: HashSet<String>,
}

/// Starts the server: where it listens, and a port nothing listens on until
/// the test says so.
pub fn start() -> (SocketAddr, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let silent = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    serve(listener, Arc::default());
    (address, silent)
}

fn serve(listener: TcpListener, server: Arc<Mutex<Server>>) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let server = server.clone();
            std::thread::spawn(move || answer(stream, &server));
        }
    });
}

fn answer(mut stream: TcpStream, server: &Arc<Mutex<Server>>) {
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    while !request.windows(4).any(|end| end == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
    let respond = |mut stream: TcpStream, status: &str, kind: &str, body: &[u8]| {
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body);
    };
    if let Some(asked) = path.strip_prefix("/asked") {
        let count = server
            .lock()
            .unwrap()
            .asked
            .get(asked)
            .copied()
            .unwrap_or(0);
        return respond(stream, "200 OK", "text/plain", count.to_string().as_bytes());
    }
    if let Some(healed) = path.strip_prefix("/heal") {
        server.lock().unwrap().healed.insert(healed.to_string());
        return respond(stream, "200 OK", "text/plain", b"");
    }
    if let Some(port) = path.strip_prefix("/listen/") {
        let late = TcpListener::bind(("127.0.0.1", port.parse().unwrap())).unwrap();
        serve(late, server.clone());
        return respond(stream, "200 OK", "text/plain", b"");
    }
    let (asked, healed) = {
        let mut server = server.lock().unwrap();
        let asked = server.asked.entry(path.clone()).or_insert(0);
        *asked += 1;
        (*asked, server.healed.contains(&path))
    };
    let kind = path.split('/').nth(1).unwrap_or("");
    match kind {
        "breaks" if asked <= 2 => {}
        "stalls" if asked <= 1 => std::thread::sleep(Duration::from_secs(20)),
        "busy" if asked <= 2 => respond(stream, "503 Busy", "text/plain", b"busy"),
        "down" if !healed => {}
        "gone" if !healed => respond(stream, "404 Not Found", "text/plain", b"gone"),
        "page" if !healed => respond(stream, "200 OK", "text/html", b"<html></html>"),
        _ => respond(stream, "200 OK", "image/png", PICTURE),
    }
}
