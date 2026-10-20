//! Local IPC control channel.
//!
//! An application host assembles this channel explicitly; nothing listens
//! by default. The server accepts newline-delimited JSON control messages on
//! a Unix domain socket inside a per-user directory and only serves the same
//! user: on Unix the directory is mode 0700, the socket 0600 and peers are
//! checked by user id; on Windows (AF_UNIX, Windows 10 1803 and later) the
//! directory is in the user's own temporary directory, which only that user
//! can open. Every request reaches the same [`bern_command::Host`] and the
//! same live UI sessions as the UI.

use bern_adapter_json::{
    self as json, ClientMessage, ServerMessage, SessionInfo, media_players_json, params_from_json,
    state_json, to_json,
};
use bern_command::{CallContext, Entry, Host, Subscription};
use bern_media::{MediaCommand, MediaHub, MediaSubscription, media_catalog};
use bern_protocol::{
    CommandError, Invocation, PROTOCOL_VERSION, SessionCommand, fingerprint_bytes, session_catalog,
};
use bern_runtime::Sessions;
use platform::{UnixListener, UnixStream};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_LINE: usize = 1 << 20;

/// How long a media command waits for the executor before its outcome is
/// reported as unknown.
const MEDIA_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a client waits for the handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a query may take to answer. A late query is cancelled; it has
/// no side effects, so the caller can simply ask again.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// The variable naming the directory the per-application socket
/// directories go in, instead of the temporary directory. `bern run` sets
/// it for an app in the iOS simulator to the Mac's temporary directory,
/// where the Mac's command line tools look.
pub const DIR_VARIABLE: &str = "BERN_IPC_DIR";

/// The abstract socket an Android app listens on: an app cannot share a
/// directory with the developer's tools, so the tools reach it through
/// `adb forward` to this name.
pub fn android_socket(bundle_id: &str) -> String {
    format!("bern.{bundle_id}")
}

/// The socket in [`socket_dir`] that `bern run` forwards to the Android
/// app's [`android_socket`], named like an engine.
pub const ANDROID_FORWARD: &str = "android.sock";

/// Directory holding the sockets of one application, unique per user.
pub fn socket_dir(bundle_id: &str) -> PathBuf {
    std::env::var_os(DIR_VARIABLE)
        .map(PathBuf::from)
        .unwrap_or_else(platform::temp_dir)
        .join(format!(
            "bern-{:016x}",
            fingerprint_bytes(bundle_id.as_bytes())
        ))
}

#[cfg(unix)]
mod platform {
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::io::AsRawFd;
    pub(crate) use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    pub(crate) fn temp_dir() -> PathBuf {
        std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }

    fn current_uid() -> u32 {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() }
    }

    /// Create the socket directory, owned by this user and private to it.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) fn prepare_dir(dir: &Path) -> io::Result<()> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        if std::fs::metadata(dir)?.uid() != current_uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the control channel directory belongs to another user",
            ));
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
    }

    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) fn secure_socket(path: &Path) -> io::Result<()> {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
    }

    fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "freebsd"
        ))]
        {
            let mut uid: libc::uid_t = 0;
            let mut gid: libc::gid_t = 0;
            // SAFETY: the descriptor is a connected Unix socket owned by `stream`.
            let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(uid)
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let mut credentials = libc::ucred {
                pid: 0,
                uid: 0,
                gid: 0,
            };
            let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
            // SAFETY: valid socket and correctly sized output buffer.
            let result = unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_PEERCRED,
                    (&mut credentials as *mut libc::ucred).cast(),
                    &mut length,
                )
            };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(credentials.uid)
        }
    }

    /// Whether a connected peer runs as this user. On Android the app is
    /// its own user, and the developer's tools arrive through `adb`, which
    /// runs as the shell user (or as root on development builds).
    pub(crate) fn peer_allowed(stream: &UnixStream) -> bool {
        peer_uid(stream).is_ok_and(|uid| {
            uid == current_uid() || (cfg!(target_os = "android") && (uid == 2000 || uid == 0))
        })
    }
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::path::{Path, PathBuf};
    pub(crate) use uds_windows::{UnixListener, UnixStream};

    /// The user's own temporary directory, which other users cannot open.
    pub(crate) fn temp_dir() -> PathBuf {
        std::env::temp_dir()
    }

    pub(crate) fn prepare_dir(dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)
    }

    /// The socket inherits the access of the user's temporary directory.
    pub(crate) fn secure_socket(_: &Path) -> io::Result<()> {
        Ok(())
    }

    /// Only this user can reach the socket file, so every peer is this
    /// user (or an administrator).
    pub(crate) fn peer_allowed(_: &UnixStream) -> bool {
        true
    }
}

/// A running control channel. Dropping it stops accepting and removes the socket.
pub struct Server {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    /// The abstract address to connect to when stopping.
    #[cfg(target_os = "android")]
    wake: std::os::unix::net::SocketAddr,
}

impl Server {
    /// Start serving `host`, its media players and live UI `sessions` for
    /// the application `bundle_id`.
    pub fn start(
        host: Host,
        sessions: Sessions,
        media: MediaHub,
        bundle_id: &str,
    ) -> io::Result<Server> {
        #[cfg(target_os = "android")]
        let (listener, path, wake) = {
            use std::os::android::net::SocketAddrExt;
            let name = android_socket(bundle_id);
            let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())?;
            let listener = UnixListener::bind_addr(&address)?;
            (listener, PathBuf::from(format!("@{name}")), address)
        };
        #[cfg(not(target_os = "android"))]
        let (listener, path) = Self::bind(bundle_id, &host)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        thread::Builder::new()
            .name("bern-ipc".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    if !platform::peer_allowed(&stream) {
                        continue;
                    }
                    let host = host.clone();
                    let sessions = sessions.clone();
                    let media = media.clone();
                    let _ = thread::Builder::new()
                        .name("bern-ipc-connection".into())
                        .spawn(move || {
                            let _ = serve(stream, host, sessions, media);
                        });
                }
            })?;
        Ok(Server {
            path,
            stop,
            #[cfg(target_os = "android")]
            wake,
        })
    }

    /// The socket in the application's directory.
    #[cfg(not(target_os = "android"))]
    fn bind(bundle_id: &str, host: &Host) -> io::Result<(UnixListener, PathBuf)> {
        let dir = socket_dir(bundle_id);
        platform::prepare_dir(&dir)?;
        remove_stale_sockets(&dir);
        let path = dir.join(format!("{}.sock", host.engine_id()));
        if path.as_os_str().len() > 100 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "control channel path is too long for a Unix socket",
            ));
        }
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        platform::secure_socket(&path)?;
        Ok((listener, path))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wake the accept loop so it observes the flag.
        #[cfg(target_os = "android")]
        let _ = UnixStream::connect_addr(&self.wake);
        #[cfg(not(target_os = "android"))]
        {
            let _ = UnixStream::connect(&self.path);
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Remove sockets left behind by engines that ended without cleanup, such
/// as a process killed by a signal. Live sockets accept connections.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn remove_stale_sockets(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "sock")
            && UnixStream::connect(&path).is_err()
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

type Writer = Arc<Mutex<UnixStream>>;

fn send(writer: &Writer, message: &ServerMessage) -> io::Result<()> {
    let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
    line.push(b'\n');
    let mut stream = writer.lock().expect("writer lock");
    stream.write_all(&line)?;
    stream.flush()
}

fn error(request_id: Option<String>, code: &str, message: impl Into<String>) -> ServerMessage {
    ServerMessage::Error {
        request_id,
        error: CommandError::new(code, message),
    }
}

fn read_line(reader: &mut BufReader<UnixStream>, line: &mut Vec<u8>) -> io::Result<bool> {
    line.clear();
    let read = reader
        .by_ref()
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', line)?;
    if read == 0 {
        return Ok(false);
    }
    if line.len() > MAX_LINE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    Ok(true)
}

fn serve(stream: UnixStream, host: Host, sessions: Sessions, media: MediaHub) -> io::Result<()> {
    let writer: Writer = Arc::new(Mutex::new(stream.try_clone()?));
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    let descriptor = host.descriptor();

    // Handshake: protocol version and, when given, catalog fingerprint.
    if !read_line(&mut reader, &mut line)? {
        return Ok(());
    }
    let entry = match serde_json::from_slice::<ClientMessage>(&line) {
        Ok(ClientMessage::Hello {
            protocol_version,
            entry,
            catalog_fingerprint,
            ..
        }) => {
            if protocol_version != PROTOCOL_VERSION {
                return send(
                    &writer,
                    &error(
                        None,
                        "protocol_mismatch",
                        format!(
                            "this engine speaks protocol {PROTOCOL_VERSION}, the client {protocol_version}"
                        ),
                    ),
                );
            }
            if let Some(fingerprint) = catalog_fingerprint
                && fingerprint != descriptor.fingerprint().to_string()
            {
                return send(
                    &writer,
                    &error(
                        None,
                        "catalog_mismatch",
                        "the client was built for a different command catalog; rebuild it",
                    ),
                );
            }
            match entry.as_str() {
                "cli" => Entry::Cli,
                "mcp" => Entry::Mcp,
                _ => Entry::Json,
            }
        }
        _ => {
            return send(
                &writer,
                &error(None, "handshake_required", "send a hello message first"),
            );
        }
    };
    send(&writer, &json::welcome(host.engine_id(), descriptor))?;

    let (session_types, session_specs) = session_catalog();
    let (media_types, media_specs) = media_catalog();
    let mut subscription: Option<(Subscription, MediaSubscription)> = None;
    while read_line(&mut reader, &mut line)? {
        let message: ClientMessage = match serde_json::from_slice(&line) {
            Ok(message) => message,
            Err(problem) => {
                send(
                    &writer,
                    &error(None, "invalid_message", problem.to_string()),
                )?;
                continue;
            }
        };
        // The request on the timeline, from handling it to its answer.
        let _request = bern_command::trace::Span::entry(|| match &message {
            ClientMessage::Command { command, .. } | ClientMessage::SessionCommand { command, .. } => {
                format!("{} {command}", entry.name())
            }
            _ => format!("{} request", entry.name()),
        });
        let reply = match message {
            ClientMessage::Hello { .. } => error(None, "invalid_message", "already connected"),
            ClientMessage::Command {
                request_id,
                engine_id,
                command,
                params,
            } => {
                if engine_id != host.engine_id() {
                    error(
                        Some(request_id),
                        "engine_mismatch",
                        "this request names a different engine instance",
                    )
                } else if let Some((index, spec)) = descriptor.command(&command) {
                    let outcome =
                        params_from_json(spec, &params, &descriptor.types).map(|params| {
                            let invocation = Invocation {
                                command: index,
                                params,
                            };
                            let context = CallContext {
                                entry,
                                request_id: request_id.clone(),
                            };
                            if spec.is_query() {
                                host.query_blocking(invocation, context, QUERY_TIMEOUT)
                            } else {
                                host.dispatch_blocking(invocation, context)
                            }
                        });
                    match outcome {
                        Ok(outcome) => ServerMessage::Result {
                            request_id,
                            status: "completed".into(),
                            revision: outcome.revision.to_string(),
                            ok: outcome.result.is_ok(),
                            result: outcome
                                .result
                                .as_ref()
                                .ok()
                                .map(|value| to_json(value, &spec.result, &descriptor.types)),
                            error: outcome.result.err(),
                        },
                        Err(problem) => ServerMessage::Result {
                            request_id,
                            status: "completed".into(),
                            revision: host.snapshot().0.to_string(),
                            ok: false,
                            result: None,
                            error: Some(problem),
                        },
                    }
                } else if let Some(media_command) = MediaCommand::from_id(&command) {
                    let spec = &media_specs[media_command.index() as usize];
                    let result = params_from_json(spec, &params, &media_types)
                        .and_then(|params| {
                            media.dispatch_blocking(media_command, params, MEDIA_TIMEOUT)
                        })
                        .map(|value| to_json(&value, &spec.result, &media_types));
                    ServerMessage::Result {
                        request_id,
                        status: "completed".into(),
                        revision: media.snapshot().0.to_string(),
                        ok: result.is_ok(),
                        error: result.as_ref().err().cloned(),
                        result: result.ok(),
                    }
                } else {
                    ServerMessage::Result {
                        request_id,
                        status: "completed".into(),
                        revision: host.snapshot().0.to_string(),
                        ok: false,
                        result: None,
                        error: Some(CommandError::unknown_command(&command)),
                    }
                }
            }
            ClientMessage::SessionCommand {
                request_id,
                session_id,
                command,
                params,
            } => {
                let result = match (
                    SessionCommand::from_id(&command),
                    session_specs.iter().find(|spec| spec.id == command),
                ) {
                    (Some(session_command), Some(spec)) => {
                        params_from_json(spec, &params, &session_types)
                            .and_then(|params| {
                                sessions.handle(session_id.as_deref(), session_command, params)
                            })
                            .map(|value| to_json(&value, &spec.result, &session_types))
                    }
                    _ => Err(CommandError::unknown_command(&command)),
                };
                ServerMessage::Result {
                    request_id,
                    status: "completed".into(),
                    revision: host.snapshot().0.to_string(),
                    ok: result.is_ok(),
                    error: result.as_ref().err().cloned(),
                    result: result.ok(),
                }
            }
            ClientMessage::State { request_id } => {
                let (revision, values) = host.snapshot();
                let values: Vec<(usize, _)> = values.into_iter().enumerate().collect();
                ServerMessage::State {
                    request_id,
                    revision: revision.to_string(),
                    values: state_json(descriptor, &values),
                }
            }
            ClientMessage::Catalog { request_id } => ServerMessage::Catalog {
                request_id,
                protocol: Box::new(descriptor.clone()),
                session_commands: session_specs.clone(),
                session_types: session_types.clone(),
                media_commands: media_specs.clone(),
                media_types: media_types.clone(),
            },
            ClientMessage::Sessions { request_id } => ServerMessage::Sessions {
                request_id,
                sessions: sessions
                    .list()
                    .iter()
                    .map(|session| SessionInfo {
                        session_id: session.session_id(),
                        windows: session.windows(),
                    })
                    .collect(),
            },
            // Pages register sessions and run players over the WebSocket
            // channel of a WebUI engine host; this channel serves tools.
            ClientMessage::Cancel { .. }
            | ClientMessage::OpenSession { .. }
            | ClientMessage::SessionWindows { .. }
            | ClientMessage::SessionResult { .. }
            | ClientMessage::MediaAttach { .. }
            | ClientMessage::MediaCreate { .. }
            | ClientMessage::MediaBind { .. }
            | ClientMessage::MediaRelease { .. }
            | ClientMessage::MediaReleaseWindow { .. }
            | ClientMessage::MediaView { .. }
            | ClientMessage::MediaComplete { .. }
            | ClientMessage::MediaEvent { .. } => error(
                None,
                "invalid_message",
                "this channel serves tools; pages connect through the engine host's WebSocket channel",
            ),
            ClientMessage::Subscribe { request_id } => {
                let updates = Arc::clone(&writer);
                let engine = host.subscribe(move |update| {
                    let _ = send(
                        &updates,
                        &ServerMessage::Update {
                            revision: update.revision.to_string(),
                            changes: state_json(descriptor, &update.changes),
                        },
                    );
                });
                let updates = Arc::clone(&writer);
                let players = media.subscribe(move |update| {
                    let _ = send(
                        &updates,
                        &ServerMessage::Media {
                            revision: update.revision.to_string(),
                            players: media_players_json(&update.players),
                        },
                    );
                });
                subscription = Some((engine, players));
                let (revision, values) = host.snapshot();
                let values: Vec<(usize, _)> = values.into_iter().enumerate().collect();
                ServerMessage::State {
                    request_id,
                    revision: revision.to_string(),
                    values: state_json(descriptor, &values),
                }
            }
        };
        send(&writer, &reply)?;
    }
    drop(subscription);
    Ok(())
}

/// Why a client could not reach an engine.
#[derive(Debug)]
pub enum ConnectError {
    NoEngine,
    Ambiguous(Vec<String>),
    NotFound(String),
    Io(io::Error),
    Rejected(CommandError),
    /// The engine took the connection but did not answer the handshake.
    NoAnswer,
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::NoEngine => f.write_str(
                "no running engine was found; start the application with its control channel enabled",
            ),
            ConnectError::Ambiguous(engines) => write!(
                f,
                "several engines are running ({}); choose one with --engine",
                engines.join(", ")
            ),
            ConnectError::NotFound(engine) => write!(f, "engine `{engine}` is not running"),
            ConnectError::Io(error) => write!(f, "connection failed: {error}"),
            ConnectError::Rejected(error) => write!(f, "the engine refused the connection: {error}"),
            ConnectError::NoAnswer => f.write_str(
                "the engine does not answer; an app the system suspended, such as an iOS app in the background, answers once it runs again",
            ),
        }
    }
}

impl std::error::Error for ConnectError {}

/// Running engines of an application that accept connections.
pub fn discover(bundle_id: &str) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(socket_dir(bundle_id)) else {
        return Vec::new();
    };
    let mut engines: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let engine = path.file_stem()?.to_str()?.to_string();
            (path.extension()? == "sock" && UnixStream::connect(&path).is_ok())
                .then_some((engine, path))
        })
        .collect();
    engines.sort();
    engines
}

/// Blocking control channel client.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    pub engine_id: String,
    pub namespace: String,
    next: u64,
}

impl Client {
    /// Connect to the only running engine, or to the one named `engine`.
    pub fn connect(
        bundle_id: &str,
        engine: Option<&str>,
        entry: &str,
        fingerprint: Option<u64>,
    ) -> Result<Client, ConnectError> {
        let engines = discover(bundle_id);
        let path = match engine {
            Some(engine) => engines
                .into_iter()
                .find(|(id, _)| id == engine)
                .map(|(_, path)| path)
                .ok_or_else(|| ConnectError::NotFound(engine.to_string()))?,
            None => match engines.len() {
                0 => return Err(ConnectError::NoEngine),
                1 => engines.into_iter().next().unwrap().1,
                _ => {
                    return Err(ConnectError::Ambiguous(
                        engines.into_iter().map(|(id, _)| id).collect(),
                    ));
                }
            },
        };
        Self::connect_path(&path, entry, fingerprint)
    }

    pub fn connect_path(
        path: &Path,
        entry: &str,
        fingerprint: Option<u64>,
    ) -> Result<Client, ConnectError> {
        Self::connect_within(path, entry, fingerprint, HANDSHAKE_TIMEOUT)
    }

    fn connect_within(
        path: &Path,
        entry: &str,
        fingerprint: Option<u64>,
        handshake: Duration,
    ) -> Result<Client, ConnectError> {
        let stream = UnixStream::connect(path).map_err(ConnectError::Io)?;
        // An engine answers the handshake at once unless it is not running
        // at all, such as an iOS app suspended in the background; commands
        // afterwards may take their time.
        stream
            .set_read_timeout(Some(handshake))
            .map_err(ConnectError::Io)?;
        let mut client = Client {
            reader: BufReader::new(stream.try_clone().map_err(ConnectError::Io)?),
            writer: stream,
            engine_id: String::new(),
            namespace: String::new(),
            next: 1,
        };
        client
            .write(&ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                entry: entry.to_string(),
                catalog_fingerprint: fingerprint.map(|value| value.to_string()),
                token: None,
            })
            .map_err(ConnectError::Io)?;
        let reply = client.read().map_err(|error| match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ConnectError::NoAnswer,
            _ => ConnectError::Io(error),
        })?;
        match reply {
            ServerMessage::Welcome {
                engine_id,
                namespace,
                ..
            } => {
                client.engine_id = engine_id;
                client.namespace = namespace;
                client
                    .writer
                    .set_read_timeout(None)
                    .map_err(ConnectError::Io)?;
                Ok(client)
            }
            ServerMessage::Error { error, .. } => Err(ConnectError::Rejected(error)),
            _ => Err(ConnectError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected handshake reply",
            ))),
        }
    }

    fn write(&mut self, message: &ClientMessage) -> io::Result<()> {
        let mut line = serde_json::to_vec(message).map_err(io::Error::other)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()
    }

    /// Read the next server message.
    pub fn read(&mut self) -> io::Result<ServerMessage> {
        let mut line = Vec::new();
        if self.reader.read_until(b'\n', &mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the engine closed the connection",
            ));
        }
        serde_json::from_slice(&line)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn request_id(&mut self) -> String {
        let id = self.next;
        self.next += 1;
        id.to_string()
    }

    /// Send a request and wait for its reply, skipping state updates.
    pub fn request(
        &mut self,
        build: impl FnOnce(String) -> ClientMessage,
    ) -> io::Result<ServerMessage> {
        let id = self.request_id();
        self.write(&build(id.clone()))?;
        loop {
            let message = self.read()?;
            let matches = match &message {
                ServerMessage::Result { request_id, .. }
                | ServerMessage::State { request_id, .. }
                | ServerMessage::Catalog { request_id, .. }
                | ServerMessage::Sessions { request_id, .. } => *request_id == id,
                ServerMessage::Error { request_id, .. } => {
                    request_id.as_deref().is_none_or(|request| request == id)
                }
                _ => false,
            };
            if matches {
                return Ok(message);
            }
        }
    }
}

#[cfg(test)]
mod tests;
