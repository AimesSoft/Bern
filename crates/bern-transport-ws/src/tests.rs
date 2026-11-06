//! The server over real TCP connections: static files, host and origin
//! checks, and the WebSocket handshake.

use super::*;
use std::io::{Read, Write};

fn host() -> Host {
    struct Empty;
    impl bern_command::EngineProtocol for Empty {
        fn descriptor(&self) -> &'static bern_protocol::ProtocolDescriptor {
            static DESCRIPTOR: std::sync::OnceLock<bern_protocol::ProtocolDescriptor> =
                std::sync::OnceLock::new();
            DESCRIPTOR.get_or_init(|| bern_protocol::ProtocolDescriptor {
                modules: Vec::new(),
                events: Vec::new(),
                descriptor_version: bern_protocol::DESCRIPTOR_VERSION,
                namespace: "empty".into(),
                version: 1,
                types: Default::default(),
                state: vec![],
                commands: vec![],
            })
        }
        fn invoke(
            &mut self,
            _: &bern_protocol::Invocation,
        ) -> Result<bern_protocol::Value, bern_protocol::CommandError> {
            unreachable!()
        }
        fn snapshot(&self) -> Vec<bern_protocol::Value> {
            vec![]
        }
    }
    Host::start(Empty)
}

fn start() -> (Server, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "bern-ws-{}-{:?}",
        std::process::id(),
        thread::current().id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("index.html"), "<!doctype html><p>page</p>").unwrap();
    std::fs::write(root.join("clip.mp4"), (0u8..=255).collect::<Vec<_>>()).unwrap();
    let server =
        Server::start(host(), Sessions::new(), MediaHub::new(&[]), root.clone(), 0).unwrap();
    (server, root)
}

fn request(server: &Server, head: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(server.address()).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8_lossy(&response[..split]).into_owned(),
        response[split + 4..].to_vec(),
    )
}

#[test]
fn files_are_served_only_to_requests_naming_this_host() {
    let (server, root) = start();
    let port = server.address().port();
    let (head, body) = request(
        &server,
        &format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("text/html"));
    assert_eq!(body, b"<!doctype html><p>page</p>");
    // DNS rebinding: another name for this address is refused.
    let (head, _) = request(
        &server,
        &format!("GET / HTTP/1.1\r\nHost: evil.example:{port}\r\n\r\n"),
    );
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");
    let (head, _) = request(
        &server,
        &format!("GET /../secret HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    // Byte ranges, which video seeking uses.
    let (head, body) = request(
        &server,
        &format!("GET /clip.mp4 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nRange: bytes=10-19\r\n\r\n"),
    );
    assert!(head.starts_with("HTTP/1.1 206"), "{head}");
    assert!(head.contains("Content-Range: bytes 10-19/256"), "{head}");
    assert_eq!(body, (10u8..20).collect::<Vec<_>>());
    assert!(
        server
            .url()
            .starts_with(&format!("http://127.0.0.1:{port}/#token="))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn only_the_served_page_may_open_the_channel() {
    let (server, root) = start();
    let port = server.address().port();
    let upgrade = |origin: &str| {
        format!(
            "GET /bern HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: {origin}\r\n\r\n"
        )
    };
    let (head, _) = request(&server, &upgrade("http://evil.example"));
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");

    let mut stream = TcpStream::connect(server.address()).unwrap();
    stream
        .write_all(upgrade(&format!("http://127.0.0.1:{port}")).as_bytes())
        .unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).unwrap();
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    assert!(
        head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
        "{head}"
    );

    // A hello without the token closes the channel with an explanation.
    let hello = serde_json::json!({
        "type": "hello", "protocol_version": bern_protocol::PROTOCOL_VERSION, "entry": "webui"
    })
    .to_string();
    let mask = [1u8, 2, 3, 4];
    let mut frame = vec![0x81, 0x80 | 126];
    frame.extend_from_slice(&(hello.len() as u16).to_be_bytes());
    frame.extend_from_slice(&mask);
    frame.extend(hello.bytes().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    stream.write_all(&frame).unwrap();
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).unwrap();
    assert_eq!(header[0], 0x81);
    let length = match header[1] {
        126 => {
            let mut bytes = [0u8; 2];
            stream.read_exact(&mut bytes).unwrap();
            u16::from_be_bytes(bytes) as usize
        }
        short => short as usize,
    };
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(reply["error"]["code"], "unauthorized");
    let _ = std::fs::remove_dir_all(root);
}

struct Hello;

impl bern_command::web_ui::Routes for Hello {
    fn handle(
        &self,
        request: &bern_command::web_ui::Request,
        _host: &bern_command::Host,
    ) -> Option<bern_command::web_ui::Response> {
        (request.path == "/api/hello").then(|| bern_command::web_ui::Response {
            status: 200,
            headers: vec![("Content-Type".into(), "text/plain".into())],
            body: bern_command::web_ui::Body::Bytes(
                format!(
                    "{} {}",
                    request.param("name").unwrap_or_default(),
                    request.body.len()
                )
                .into_bytes(),
            ),
        })
    }
}

#[test]
fn an_open_server_takes_any_host_and_routes_answer_before_the_page() {
    let (first, root) = start();
    drop(first);
    let options = bern_command::web_ui::Options {
        port: 0,
        routes: Some(std::sync::Arc::new(Hello)),
    };
    let server = Server::start_open(
        host(),
        Sessions::new(),
        MediaHub::new(&[]),
        root.clone(),
        &options,
    )
    .unwrap();
    assert_eq!(
        server.url(),
        format!("http://127.0.0.1:{}/", server.address().port())
    );

    let (head, body) = request(
        &server,
        "POST /api/hello?name=a%20b HTTP/1.1\r\nHost: 192.168.1.5:1180\r\nContent-Length: 3\r\n\r\nabc",
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert_eq!(body, b"a b 3");

    let (head, _) = request(
        &server,
        "HEAD /api/hello?name=a HTTP/1.1\r\nHost: nas.local\r\n\r\n",
    );
    assert!(head.contains("Content-Length: 3"), "{head}");

    // What the routes leave is the page's, by whatever name it was reached.
    let (head, _) = request(&server, "GET / HTTP/1.1\r\nHost: nas.local:1180\r\n\r\n");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_page_reads_the_apps_own_files_and_no_others() {
    assert_eq!(app_file("url=file%3A%2F%2F%2Fetc%2Fhosts"), None);
    assert_eq!(app_file("other=1"), None);
}
