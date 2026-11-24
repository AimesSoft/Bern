//! File-kind acceptance rules for `ui.set_file` and base64 encoding for
//! window screenshots.

use crate::protocol::Value;

/// Base64-encode `bytes` with standard padding.
pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Extension of a `file://` or `http(s)://` URL, lowercased without the dot.
fn extension_of(url: &str) -> String {
    let stripped = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');
    let segment = stripped.rsplit('/').next().unwrap_or(stripped);
    match segment.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Whether `url` matches the declared file kinds (such as `"any"`, `"folder"`,
/// `".txt"`, `"image"`, `"video"`).
pub(crate) fn accepts(url: &str, kinds: &str) -> bool {
    let mut words = kinds
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty());
    if words.clone().any(|word| word == "any") {
        return true;
    }
    if url.starts_with("file:")
        && let Ok(parsed) = url::Url::parse(url)
        && let Ok(path) = parsed.to_file_path()
        && path.is_dir()
    {
        return words.any(|word| word == "folder");
    }
    let extension = extension_of(url);
    let table = crate::host::host().def.file_kinds;
    words.any(|word| match word {
        "any" => true,
        "folder" => false,
        word if word.starts_with('.') => extension == word[1..].to_ascii_lowercase(),
        kind => table
            .iter()
            .find(|(name, _)| *name == kind)
            .is_some_and(|(_, extensions)| extensions.contains(&extension.as_str())),
    })
}

/// Turn a local filesystem path into a `file://` URL string.
pub(crate) fn path_to_file_url(path: &std::path::Path) -> String {
    url::Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", path.display()))
}

/// Convert a validated `file://` or `http(s)://` URL into the `Value` expected
/// by a file-picking draft field (`Text`, `MediaSource`, or `List<MediaSource>`).
pub(crate) fn url_value(ty: Option<&crate::protocol::Type>, url: &str) -> Value {
    let path_str = if url.starts_with("file:") {
        url::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.to_file_path().ok())
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| url.to_string())
    } else {
        url.to_string()
    };
    let media_source = Value::Record(vec![Value::Text(url.to_string())]);
    match ty {
        Some(crate::protocol::Type::Text) => Value::Text(path_str),
        Some(crate::protocol::Type::List { inner }) => match inner.as_ref() {
            crate::protocol::Type::Text => Value::List(vec![Value::Text(path_str)]),
            _ => Value::List(vec![media_source]),
        },
        _ => media_source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_standard_vectors() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert_eq!(encode_base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn extension_strips_query_and_fragment() {
        assert_eq!(extension_of("https://example.com/a/b.PNG?v=1#top"), "png");
        assert_eq!(extension_of("file:///tmp/movie.mkv"), "mkv");
        assert_eq!(extension_of("file:///tmp/noext"), "");
    }
}
