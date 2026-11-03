//! Files the user hands an application, read by its engine.
//!
//! Pickers, drop areas and the system hand the engine a URL. A `file:` URL
//! names a local file, which the engine opens itself. The URLs of some
//! platforms only their host can resolve: an Android document
//! (`content:`) and a file a page was given (`blob:`). The host installs an
//! [`Opener`] for such a scheme, so an engine reads every file the same
//! way: [`name`] for the name the user knows it by, [`open`] for its bytes.
//! A folder is read the same way too: [`list`] for what it holds, [`walk`]
//! for that and what its folders hold, each entry with its own URL.

use bern_protocol::CommandError;
use std::collections::HashSet;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// A file opened for reading, with its length in bytes.
pub struct SourceFile {
    reader: Box<dyn ReadSeek>,
    len: u64,
}

/// What a [`SourceFile`] reads through.
pub trait ReadSeek: Read + Seek + Send {}

impl<T: Read + Seek + Send> ReadSeek for T {}

impl SourceFile {
    pub fn new(reader: impl Read + Seek + Send + 'static, len: u64) -> Self {
        Self {
            reader: Box::new(reader),
            len,
        }
    }

    /// The file's length in bytes when it was opened.
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Read for SourceFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

impl Seek for SourceFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.reader.seek(position)
    }
}

impl std::fmt::Debug for SourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceFile")
            .field("len", &self.len)
            .finish()
    }
}

/// A file or folder in a folder the user handed the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its own URL, which [`open`], [`name`] and [`list`] take.
    pub url: String,
    /// The name the user knows it by.
    pub name: String,
    /// Where it is in the folder listed or walked: its name, or for
    /// [`walk`] the names of the folders on the way and its own, joined by
    /// `/`, such as `Live/01 Intro.mp3`.
    pub path: String,
    pub folder: bool,
    /// Its length in bytes; 0 for a folder.
    pub size: u64,
    /// When it last changed, when the system says.
    pub modified: Option<SystemTime>,
}

/// How a host resolves the URLs of one scheme.
pub trait Opener: Send + Sync {
    /// The name the user knows the file by, when the host knows it.
    fn name(&self, url: &str) -> Option<String>;
    /// The file, opened for reading.
    fn open(&self, url: &str) -> Result<SourceFile, CommandError>;
    /// What the folder at `url` holds, each entry's `path` its name. A
    /// scheme without folders answers `not_a_folder`.
    fn list(&self, url: &str) -> Result<Vec<Entry>, CommandError> {
        Err(not_a_folder(url))
    }
}

fn not_a_folder(url: &str) -> CommandError {
    CommandError::new("not_a_folder", format!("`{url}` is not a folder"))
}

type Openers = Vec<(String, Arc<dyn Opener>)>;

static OPENERS: RwLock<Openers> = RwLock::new(Vec::new());

/// Resolve the URLs of `scheme` through `opener` from now on. Hosts install
/// their openers before the engine handles its first command; a later
/// install for the same scheme replaces the earlier one.
pub fn install(scheme: &str, opener: impl Opener + 'static) {
    let scheme = scheme.to_ascii_lowercase();
    let mut openers = OPENERS.write().unwrap_or_else(|error| error.into_inner());
    openers.retain(|(existing, _)| *existing != scheme);
    openers.push((scheme, Arc::new(opener)));
}

fn installed(scheme: &str) -> Option<Arc<dyn Opener>> {
    let openers = OPENERS.read().unwrap_or_else(|error| error.into_inner());
    openers
        .iter()
        .find(|(existing, _)| existing == scheme)
        .map(|(_, opener)| opener.clone())
}

/// The scheme of `url`, lowercased: the letters before the first `:`.
fn scheme(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once(':')?;
    let valid = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// The name the user knows the file at `url` by: the last part of a local
/// path or web address, or the name the system or the page gave a document.
/// `None` when there is none, for example a web address ending in `/`.
pub fn name(url: &str) -> Option<String> {
    let scheme = scheme(url)?;
    if let Some(opener) = installed(&scheme) {
        return opener.name(url);
    }
    match scheme.as_str() {
        "file" => file_path(url)?
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        "http" | "https" => {
            let path = url.split(['?', '#']).next().unwrap_or(url);
            let after_host = path.splitn(4, '/').nth(3)?;
            let last = after_host.rsplit('/').next()?;
            let name = String::from_utf8(percent_decode(last)).ok()?;
            (!name.is_empty()).then_some(name)
        }
        #[cfg(target_arch = "wasm32")]
        "blob" => web::name(url),
        _ => None,
    }
}

/// The file at `url`, opened for reading. Local files and the documents a
/// host resolves can be read; a web address cannot (`unreadable_source`),
/// the engine fetches it with its own client. A file that is gone or no
/// longer readable fails with `source_unavailable`.
pub fn open(url: &str) -> Result<SourceFile, CommandError> {
    let unreadable = || {
        CommandError::new(
            "unreadable_source",
            format!(
                "`{url}` cannot be read here: an engine reads local files and the documents the system or the page handed the application"
            ),
        )
    };
    let scheme = scheme(url)
        .ok_or_else(|| CommandError::new("invalid_url", format!("`{url}` is not a URL")))?;
    if let Some(opener) = installed(&scheme) {
        return opener.open(url);
    }
    match scheme.as_str() {
        "file" => {
            let path = file_path(url).ok_or_else(|| {
                CommandError::new("invalid_url", format!("`{url}` is not a local file URL"))
            })?;
            let unavailable = |error: io::Error| {
                CommandError::new("source_unavailable", format!("{}: {error}", path.display()))
            };
            let file = std::fs::File::open(&path).map_err(unavailable)?;
            let metadata = file.metadata().map_err(unavailable)?;
            if metadata.is_dir() {
                return Err(CommandError::new(
                    "source_unavailable",
                    format!("{} is a folder", path.display()),
                ));
            }
            Ok(SourceFile::new(file, metadata.len()))
        }
        #[cfg(target_arch = "wasm32")]
        "blob" => web::open(url).ok_or_else(|| {
            CommandError::new(
                "source_unavailable",
                format!("the page no longer holds the file `{url}`"),
            )
        }),
        _ => Err(unreadable()),
    }
}

/// What the folder at `url` holds, in no particular order, each entry's
/// `path` its name. Local folders and the folders the system or the page
/// handed the application can be listed: on Android a folder from the
/// system's picker (a `content:` tree) and the folders inside it, in a
/// page a folder the user picked. A file answers `not_a_folder`, a folder
/// that is gone or no longer readable `source_unavailable`, and a web
/// address `unreadable_source`. A local link to a file or folder is listed
/// as what it points to; one that points nowhere is left out, and so is a
/// name that is not valid Unicode.
pub fn list(url: &str) -> Result<Vec<Entry>, CommandError> {
    let scheme = scheme(url)
        .ok_or_else(|| CommandError::new("invalid_url", format!("`{url}` is not a URL")))?;
    if let Some(opener) = installed(&scheme) {
        return opener.list(url);
    }
    match scheme.as_str() {
        "file" => {
            let path = file_path(url).ok_or_else(|| {
                CommandError::new("invalid_url", format!("`{url}` is not a local file URL"))
            })?;
            let unavailable = |error: io::Error| {
                CommandError::new("source_unavailable", format!("{}: {error}", path.display()))
            };
            if !std::fs::metadata(&path).map_err(unavailable)?.is_dir() {
                return Err(not_a_folder(url));
            }
            let mut entries = Vec::new();
            for entry in std::fs::read_dir(&path).map_err(unavailable)?.flatten() {
                let Ok(name) = entry.file_name().into_string() else {
                    continue;
                };
                let Ok(metadata) = std::fs::metadata(entry.path()) else {
                    continue;
                };
                let folder = metadata.is_dir();
                entries.push(Entry {
                    url: crate::resources::file_url(&entry.path()),
                    path: name.clone(),
                    name,
                    folder,
                    size: if folder { 0 } else { metadata.len() },
                    modified: metadata.modified().ok(),
                });
            }
            Ok(entries)
        }
        #[cfg(target_arch = "wasm32")]
        "blob" => web::list(url).ok_or_else(|| not_a_folder(url)),
        "http" | "https" => Err(CommandError::new(
            "unreadable_source",
            format!(
                "`{url}` cannot be listed here: an engine lists local folders and the folders the system or the page handed the application"
            ),
        )),
        _ => Err(not_a_folder(url)),
    }
}

/// Folders inside folders deeper than this are left out of a walk.
const WALK_DEPTH: usize = 64;

/// What the folder at `url` holds and what the folders in it hold, as
/// [`list`] answers it, each entry's `path` its place in the folder at
/// `url`. A folder comes before what it holds. `keep` sees each entry
/// first: what it turns down is left out, and a folder it turns down is not
/// gone into, for example hidden ones (`!entry.name.starts_with('.')`).
/// Only the folder at `url` itself must be readable: a folder inside that
/// cannot be listed is there without its content. Each folder is gone into
/// once, even when links lead to it again, and at most 64 deep.
pub fn walk(url: &str, mut keep: impl FnMut(&Entry) -> bool) -> Result<Vec<Entry>, CommandError> {
    let top = list(url)?;
    let mut entries = Vec::new();
    let mut seen = HashSet::from([folder_identity(url)]);
    walk_into(top, "", 1, &mut keep, &mut seen, &mut entries);
    Ok(entries)
}

fn walk_into(
    listed: Vec<Entry>,
    prefix: &str,
    depth: usize,
    keep: &mut impl FnMut(&Entry) -> bool,
    seen: &mut HashSet<String>,
    entries: &mut Vec<Entry>,
) {
    for mut entry in listed {
        if !prefix.is_empty() {
            entry.path = format!("{prefix}/{}", entry.path);
        }
        if !keep(&entry) {
            continue;
        }
        let inside =
            (entry.folder && depth < WALK_DEPTH && seen.insert(folder_identity(&entry.url)))
                .then(|| list(&entry.url).ok())
                .flatten();
        let path = entry.path.clone();
        entries.push(entry);
        if let Some(inside) = inside {
            walk_into(inside, &path, depth + 1, keep, seen, entries);
        }
    }
}

/// What tells two folders apart: the real path of a local one, so that
/// links leading to a folder again are seen through; the URL of any other.
fn folder_identity(url: &str) -> String {
    file_path(url)
        .and_then(|path| std::fs::canonicalize(path).ok())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| url.to_string())
}

/// A time that hosts give as milliseconds since 1970.
fn from_millis(millis: i64) -> Option<SystemTime> {
    let millis = u64::try_from(millis).ok()?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(millis))
}

/// Entries as hosts hand them over, for their openers: for each entry its
/// URL, name, `d` for a folder or `f` for a file, size in bytes, and last
/// change in milliseconds since 1970 or nothing, each followed by a NUL.
#[doc(hidden)]
pub fn parse_entries(text: &str) -> Option<Vec<Entry>> {
    let fields: Vec<&str> = text.split('\0').collect();
    // The last NUL ends the last field, leaving nothing after it.
    let (last, fields) = fields.split_last()?;
    if !last.is_empty() || fields.len() % 5 != 0 {
        return None;
    }
    fields
        .chunks(5)
        .map(|entry| {
            let folder = match entry[2] {
                "d" => true,
                "f" => false,
                _ => return None,
            };
            Some(Entry {
                url: entry[0].to_string(),
                name: entry[1].to_string(),
                path: entry[1].to_string(),
                folder,
                size: if folder { 0 } else { entry[3].parse().ok()? },
                modified: entry[4].parse().ok().and_then(from_millis),
            })
        })
        .collect()
}

/// The local path a `file:` URL names, percent-decoded. On Windows
/// `file:///C:/dir/a%20b.mkv` is `C:\dir\a b.mkv` and `file://server/share/x`
/// the share `\\server\share\x`.
pub fn file_path(url: &str) -> Option<PathBuf> {
    file_path_text(url, cfg!(windows)).map(PathBuf::from)
}

fn file_path_text(url: &str, windows: bool) -> Option<String> {
    if scheme(url)? != "file" {
        return None;
    }
    let rest = &url[5..];
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (host, path) = match rest.strip_prefix("//") {
        Some(authority) => match authority.find('/') {
            Some(slash) => (&authority[..slash], &authority[slash..]),
            None => (authority, ""),
        },
        None => ("", rest),
    };
    if !path.starts_with('/') {
        return None;
    }
    let path = String::from_utf8(percent_decode(path)).ok()?;
    let local = host.is_empty() || host.eq_ignore_ascii_case("localhost");
    if !windows {
        return local.then_some(path);
    }
    let path = path.replace('/', "\\");
    if !local {
        return Some(format!(r"\\{host}{path}"));
    }
    // `\C:\dir` loses the root slash before the drive.
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        Some(path[1..].to_string())
    } else {
        Some(path)
    }
}

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                i += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    decoded
}

/// The files a page was given, which the page reads for the engine.
#[cfg(target_arch = "wasm32")]
mod web {
    use super::SourceFile;
    use std::io::{self, Read, Seek, SeekFrom};

    #[link(wasm_import_module = "bern")]
    unsafe extern "C" {
        /// The length of the name of the page's file at `url`, written to
        /// `out` when it fits in `capacity`; -1 when the page holds none.
        fn file_name(url: *const u8, url_len: usize, out: *mut u8, capacity: usize) -> i32;
        /// The size of the page's file at `url`, or -1.
        fn file_size(url: *const u8, url_len: usize) -> f64;
        /// Copies up to `len` bytes from `offset` of the file to `out`;
        /// answers the count copied, or -1.
        fn file_read(url: *const u8, url_len: usize, offset: f64, out: *mut u8, len: usize) -> i32;
        /// The length of what the page's folder at `url` holds, in the
        /// layout of `parse_entries`, written to `out` when it fits in
        /// `capacity`; -1 when the page holds no such folder.
        fn folder_list(url: *const u8, url_len: usize, out: *mut u8, capacity: usize) -> i32;
    }

    /// The text a page import answers for `url`, asked again with a larger
    /// buffer when it did not fit.
    fn text(
        url: &str,
        ask: unsafe extern "C" fn(*const u8, usize, *mut u8, usize) -> i32,
    ) -> Option<String> {
        let mut buffer = vec![0u8; 256];
        loop {
            // SAFETY: the page provides the import with this signature and
            // writes at most `capacity` bytes.
            let len = unsafe { ask(url.as_ptr(), url.len(), buffer.as_mut_ptr(), buffer.len()) };
            let len = usize::try_from(len).ok()?;
            if len <= buffer.len() {
                buffer.truncate(len);
                return String::from_utf8(buffer).ok();
            }
            buffer.resize(len, 0);
        }
    }

    pub fn name(url: &str) -> Option<String> {
        text(url, file_name)
    }

    pub fn list(url: &str) -> Option<Vec<super::Entry>> {
        super::parse_entries(&text(url, folder_list)?)
    }

    pub fn open(url: &str) -> Option<SourceFile> {
        // SAFETY: the page provides `bern.file_size` with this signature.
        let size = unsafe { file_size(url.as_ptr(), url.len()) };
        (size >= 0.0).then(|| {
            let len = size as u64;
            SourceFile::new(
                PageFile {
                    url: url.to_string(),
                    position: 0,
                    len,
                },
                len,
            )
        })
    }

    /// A file the page holds, read a slice at a time.
    struct PageFile {
        url: String,
        position: u64,
        len: u64,
    }

    impl Read for PageFile {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let left = self.len.saturating_sub(self.position);
            let want = (buf.len() as u64).min(left) as usize;
            if want == 0 {
                return Ok(0);
            }
            // SAFETY: the page provides `bern.file_read` with this signature
            // and writes at most `want` bytes.
            let read = unsafe {
                file_read(
                    self.url.as_ptr(),
                    self.url.len(),
                    self.position as f64,
                    buf.as_mut_ptr(),
                    want,
                )
            };
            let read = usize::try_from(read)
                .map_err(|_| io::Error::other(format!("the page could not read `{}`", self.url)))?;
            self.position += read as u64;
            Ok(read)
        }
    }

    impl Seek for PageFile {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            let target = match position {
                SeekFrom::Start(offset) => Some(offset),
                SeekFrom::End(delta) => self.len.checked_add_signed(delta),
                SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            };
            let target = target.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "seek before the start of the file",
                )
            })?;
            self.position = target;
            Ok(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_become_local_paths() {
        assert_eq!(
            file_path_text("file:///no%20such%20dir/clip%20%231.mp4", false).as_deref(),
            Some("/no such dir/clip #1.mp4")
        );
        assert_eq!(
            file_path_text("file://localhost/tmp/a.mkv", false).as_deref(),
            Some("/tmp/a.mkv")
        );
        assert_eq!(
            file_path_text("FILE:/tmp/%E5%BD%B1.mkv", false).as_deref(),
            Some("/tmp/影.mkv")
        );
        assert_eq!(file_path_text("file://server/share/a.mkv", false), None);
        assert_eq!(file_path_text("https://example.com/a.mkv", false), None);
        assert_eq!(
            file_path_text("file:///C:/Videos/a%20b.mkv", true).as_deref(),
            Some(r"C:\Videos\a b.mkv")
        );
        assert_eq!(
            file_path_text("file://server/share/a.mkv", true).as_deref(),
            Some(r"\\server\share\a.mkv")
        );
    }

    #[test]
    fn file_urls_round_trip_through_file_url() {
        let path = std::env::temp_dir().join("bern files #1 影.txt");
        let url = crate::resources::file_url(&path);
        assert_eq!(file_path(&url), Some(path));
    }

    #[test]
    fn names_come_from_the_url_or_the_host() {
        assert_eq!(
            name("file:///tmp/Clip%20%231.mkv").as_deref(),
            Some("Clip #1.mkv")
        );
        assert_eq!(
            name("https://example.com/v/%E5%BD%B1.mp4?t=1#x").as_deref(),
            Some("影.mp4")
        );
        assert_eq!(name("https://example.com/"), None);
        assert_eq!(name("blob:https://example.com/0f1e"), None);

        struct Documents;
        impl Opener for Documents {
            fn name(&self, url: &str) -> Option<String> {
                Some(format!("named {url}"))
            }
            fn open(&self, _: &str) -> Result<SourceFile, CommandError> {
                Ok(SourceFile::new(io::Cursor::new(b"document".to_vec()), 8))
            }
        }
        install("x-bern-test", Documents);
        assert_eq!(
            name("X-Bern-Test://doc/1").as_deref(),
            Some("named X-Bern-Test://doc/1")
        );
        let mut text = String::new();
        open("x-bern-test://doc/1")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "document");
    }

    #[test]
    fn local_files_open_and_others_say_why_not() {
        let dir = std::env::temp_dir().join(format!("bern-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clip #1.bin");
        std::fs::write(&path, b"0123456789").unwrap();
        let url = crate::resources::file_url(&path);

        let mut file = open(&url).unwrap();
        assert_eq!(file.len(), 10);
        file.seek(SeekFrom::Start(4)).unwrap();
        let mut rest = Vec::new();
        file.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, b"456789");

        assert_eq!(
            open(&crate::resources::file_url(&dir)).unwrap_err().code,
            "source_unavailable"
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(open(&url).unwrap_err().code, "source_unavailable");
        assert_eq!(
            open("https://example.com/a.mkv").unwrap_err().code,
            "unreadable_source"
        );
        assert_eq!(
            open("content://media/1").unwrap_err().code,
            "unreadable_source"
        );
        assert_eq!(open("no url").unwrap_err().code, "invalid_url");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_folders_are_listed_and_walked() {
        let dir = std::env::temp_dir().join(format!("bern-walk-{} 影", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Live #1/deep")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::write(dir.join("a.mp3"), b"12345").unwrap();
        std::fs::write(dir.join("Live #1/b.mp3"), b"1").unwrap();
        std::fs::write(dir.join("Live #1/deep/c.lrc"), b"").unwrap();
        std::fs::write(dir.join(".hidden/d.mp3"), b"").unwrap();
        // A link back up is listed as the folder it leads to, and gone
        // into once only.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dir, dir.join("Live #1/up")).unwrap();
        let url = crate::resources::file_url(&dir);

        let mut top = list(&url).unwrap();
        top.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<(&str, bool, u64)> = top
            .iter()
            .map(|entry| (entry.name.as_str(), entry.folder, entry.size))
            .collect();
        assert_eq!(
            names,
            [
                (".hidden", true, 0),
                ("Live #1", true, 0),
                ("a.mp3", false, 5)
            ]
        );
        assert!(top[2].modified.is_some());
        assert_eq!(file_path(&top[1].url), Some(dir.join("Live #1")));

        let mut walked = walk(&url, |entry| !entry.name.starts_with('.')).unwrap();
        walked.sort_by(|a, b| a.path.cmp(&b.path));
        let paths: Vec<&str> = walked.iter().map(|entry| entry.path.as_str()).collect();
        let mut expected = vec![
            "Live #1",
            "Live #1/b.mp3",
            "Live #1/deep",
            "Live #1/deep/c.lrc",
            "a.mp3",
        ];
        if cfg!(unix) {
            expected.insert(4, "Live #1/up");
        }
        assert_eq!(paths, expected);
        let lyrics = walked.iter().find(|entry| entry.name == "c.lrc").unwrap();
        assert_eq!(file_path(&lyrics.url), Some(dir.join("Live #1/deep/c.lrc")));

        assert_eq!(list(&top[2].url).unwrap_err().code, "not_a_folder");
        assert_eq!(
            walk(&top[2].url, |_| true).unwrap_err().code,
            "not_a_folder"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(list(&url).unwrap_err().code, "source_unavailable");
        assert_eq!(
            list("https://example.com/music/").unwrap_err().code,
            "unreadable_source"
        );
        assert_eq!(list("no url").unwrap_err().code, "invalid_url");
    }

    #[test]
    fn host_folders_are_walked_through_their_opener() {
        // A tree as a document provider has it: each folder lists its
        // children by their own URLs.
        struct Tree;
        impl Opener for Tree {
            fn name(&self, _: &str) -> Option<String> {
                None
            }
            fn open(&self, _: &str) -> Result<SourceFile, CommandError> {
                Ok(SourceFile::new(io::Cursor::new(Vec::new()), 0))
            }
            fn list(&self, url: &str) -> Result<Vec<Entry>, CommandError> {
                let text = match url {
                    "x-bern-tree://root" => {
                        "x-bern-tree://a\0a.mp3\0f\x003\x001700000000000\0x-bern-tree://sub\0Sub\0d\0\0\0"
                    }
                    "x-bern-tree://sub" => "x-bern-tree://b\0b.flac\0f\x007\0\0",
                    _ => return Err(not_a_folder(url)),
                };
                parse_entries(text).ok_or_else(|| CommandError::new("bad", "bad"))
            }
        }
        install("x-bern-tree", Tree);
        let walked = walk("x-bern-tree://root", |_| true).unwrap();
        let paths: Vec<(&str, &str, u64)> = walked
            .iter()
            .map(|entry| (entry.path.as_str(), entry.url.as_str(), entry.size))
            .collect();
        assert_eq!(
            paths,
            [
                ("a.mp3", "x-bern-tree://a", 3),
                ("Sub", "x-bern-tree://sub", 0),
                ("Sub/b.flac", "x-bern-tree://b", 7)
            ]
        );
        assert_eq!(
            walked[0].modified,
            SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(1_700_000_000_000))
        );
        assert_eq!(walked[2].modified, None);
        assert_eq!(list("x-bern-tree://a").unwrap_err().code, "not_a_folder");
        // An opener that has no folders says so.
        assert_eq!(
            list("x-bern-test://doc/1").unwrap_err().code,
            "not_a_folder"
        );
    }

    #[test]
    fn host_entries_parse_or_are_refused_whole() {
        assert_eq!(parse_entries(""), Some(Vec::new()));
        assert_eq!(parse_entries("u\0n\0f\x001\0"), None);
        assert_eq!(parse_entries("u\0n\0x\x001\0\0"), None);
        assert_eq!(parse_entries("u\0n\0f\0big\0\0"), None);
        let folder = parse_entries("u\0名\0d\0\0-5\0").unwrap();
        assert_eq!(folder[0].name, "名");
        assert!(folder[0].folder && folder[0].modified.is_none());
    }
}
