//! The minimal HTTP/1.1 the engine host needs: static files of the page,
//! with byte ranges for media, and the WebSocket upgrade. Every response
//! closes the connection.

use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};

const MAX_HEAD: usize = 16 * 1024;

pub struct Request {
    pub method: String,
    pub path: String,
    headers: Vec<(String, String)>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Read the request line and headers.
pub fn read_request(reader: &mut BufReader<TcpStream>) -> io::Result<Request> {
    let mut lines = Vec::new();
    let mut total = 0;
    loop {
        let mut line = String::new();
        let read = reader.by_ref().take(MAX_HEAD as u64).read_line(&mut line)?;
        if read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "closed"));
        }
        total += read;
        if total > MAX_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
        let line = line.trim_end_matches(['\r', '\n']).to_string();
        if line.is_empty() {
            break;
        }
        lines.push(line);
    }
    let mut parts = lines
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty request"))?
        .split(' ');
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default();
    let path = target.split(['?', '#']).next().unwrap_or("/").to_string();
    let headers = lines[1..]
        .iter()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    Ok(Request {
        method,
        path,
        headers,
    })
}

pub fn respond(
    stream: &mut TcpStream,
    status: &str,
    extra: &[(&str, String)],
    body: &[u8],
) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n",
        body.len()
    );
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

pub fn error(stream: &mut TcpStream, status: &str) -> io::Result<()> {
    respond(
        stream,
        status,
        &[("Content-Type", "text/plain; charset=utf-8".into())],
        status.as_bytes(),
    )
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("webmanifest") => "application/manifest+json",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("m3u8") => "application/vnd.apple.mpegurl",
        Some("ts") => "video/mp2t",
        Some("vtt") => "text/vtt",
        _ => "application/octet-stream",
    }
}

/// The file a request path names below `root`, or `None` for paths that
/// leave it or are not plain.
pub fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
    let relative = path.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    if relative.contains(['%', '\\']) {
        return None;
    }
    let relative = Path::new(relative);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(relative))
}

/// Serve a file, honouring a single `Range: bytes=a-b`.
pub fn serve_file(
    stream: &mut TcpStream,
    file: &Path,
    range: Option<&str>,
    head_only: bool,
) -> io::Result<()> {
    let Ok(mut handle) = std::fs::File::open(file) else {
        return error(stream, "404 Not Found");
    };
    let length = handle.metadata()?.len();
    if !handle.metadata()?.is_file() {
        return error(stream, "404 Not Found");
    }
    let kind = content_type(file);
    let mut headers = vec![
        ("Content-Type", kind.to_string()),
        ("Accept-Ranges", "bytes".to_string()),
        ("Cache-Control", "no-cache".to_string()),
    ];
    let span = range.and_then(|range| parse_range(range, length));
    if range.is_some() && span.is_none() {
        headers.push(("Content-Range", format!("bytes */{length}")));
        return respond(stream, "416 Range Not Satisfiable", &headers, b"");
    }
    let (status, start, end) = match span {
        Some((start, end)) => {
            headers.push(("Content-Range", format!("bytes {start}-{end}/{length}")));
            ("206 Partial Content", start, end)
        }
        None => ("200 OK", 0, length.saturating_sub(1)),
    };
    let size = if length == 0 { 0 } else { end - start + 1 };
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {size}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n"
    );
    for (name, value) in &headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if !head_only && size > 0 {
        handle.seek(SeekFrom::Start(start))?;
        io::copy(&mut handle.take(size), stream)?;
    }
    stream.flush()
}

fn parse_range(range: &str, length: u64) -> Option<(u64, u64)> {
    let spec = range.strip_prefix("bytes=")?;
    if spec.contains(',') || length == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: u64 = suffix.parse().ok()?;
            (length.saturating_sub(suffix), length - 1)
        }
        (start, "") => (start.parse().ok()?, length - 1),
        (start, end) => (
            start.parse().ok()?,
            end.parse::<u64>().ok()?.min(length - 1),
        ),
    };
    (start <= end && start < length).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_below_the_root() {
        let root = Path::new("/srv/page");
        assert_eq!(resolve(root, "/"), Some(root.join("index.html")));
        assert_eq!(resolve(root, "/app.js"), Some(root.join("app.js")));
        assert_eq!(
            resolve(root, "/resources/media/clip.mp4"),
            Some(root.join("resources/media/clip.mp4"))
        );
        // Repeated slashes still name a file below the root.
        assert_eq!(resolve(root, "//app.js"), Some(root.join("app.js")));
        for bad in ["/../etc/passwd", "/a/../../b", "/%2e%2e/x", "/a\\b"] {
            assert_eq!(resolve(root, bad), None, "{bad}");
        }
    }

    #[test]
    fn ranges_follow_rfc_7233() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=900-2000", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=5-1", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }
}
