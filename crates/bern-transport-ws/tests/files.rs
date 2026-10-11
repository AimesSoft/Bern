//! The page-only file server `bern run` uses for builds whose engine runs
//! in the page.

use bern_transport_ws::FileServer;
use std::io::{Read, Write};
use std::net::TcpStream;

fn get(server: &FileServer, path: &str, host: &str, extra: &str) -> String {
    let mut stream = TcpStream::connect(server.address()).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {host}\r\n{extra}\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn a_page_is_served_from_its_directory_only() {
    let root = std::env::temp_dir().join(format!("bern-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("resources")).unwrap();
    std::fs::write(root.join("index.html"), "<!doctype html>").unwrap();
    std::fs::write(root.join("resources/clip.txt"), "0123456789").unwrap();
    let server = FileServer::start(root.clone(), 0).unwrap();
    let host = format!("127.0.0.1:{}", server.address().port());
    assert_eq!(server.url(), format!("http://{host}/"));

    let page = get(&server, "/", &host, "");
    assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
    assert!(page.contains("Content-Type: text/html"), "{page}");
    assert!(page.ends_with("<!doctype html>"), "{page}");

    let part = get(
        &server,
        "/resources/clip.txt",
        &host,
        "Range: bytes=2-4\r\n",
    );
    assert!(part.starts_with("HTTP/1.1 206 Partial Content"), "{part}");
    assert!(part.ends_with("234"), "{part}");

    // Another site reaching the port through DNS rebinding.
    let foreign = get(
        &server,
        "/",
        &format!("evil.example:{}", server.address().port()),
        "",
    );
    assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
    let outside = get(&server, "/../secret", &host, "");
    assert!(outside.starts_with("HTTP/1.1 404"), "{outside}");
    let _ = std::fs::remove_dir_all(&root);
}
