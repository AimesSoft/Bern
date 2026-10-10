//! Local IPC control channel.
//!
//! An application host assembles this channel explicitly; nothing listens
//! by default. The server accepts newline-delimited JSON control messages on
//! a Unix domain socket inside a per-user directory (mode 0700, socket 0600)
//! and only serves peers running as the same user. Every request reaches the
//! same [`bern_command::Host`] and the same live UI sessions as the UI.

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
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_LINE: usize = 1 << 20;

/// How long a media command waits for the executor before its outcome is
/// reported as unknown.
const MEDIA_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a query may take to answer. A late query is cancelled; it has
/// no side effects, so the caller can simply ask again.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Directory holding the sockets of one application, unique per user.
pub fn socket_dir(bundle_id: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join(format!(
        "bern-{:016x}",
        fingerprint_bytes(bundle_id.as_bytes())
    ))
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
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
    #[cfg(target_os = "linux")]
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

fn current_uid() -> u32 {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() }
}

/// A running control channel. Dropping it stops accepting and removes the socket.
pub struct Server {
    path: PathBuf,
    stop: Arc<AtomicBool>,
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
        let dir = socket_dir(bundle_id);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
        let metadata = std::fs::metadata(&dir)?;
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != current_uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the control channel directory belongs to another user",
            ));
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
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
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
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
                    match peer_uid(&stream) {
                        Ok(uid) if uid == current_uid() => {}
                        _ => continue,
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
        Ok(Server { path, stop })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wake the accept loop so it observes the flag.
        let _ = UnixStream::connect(&self.path);
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Remove sockets left behind by engines that ended without cleanup, such
/// as a process killed by a signal. Live sockets accept connections.
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
        let stream = UnixStream::connect(path).map_err(ConnectError::Io)?;
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
            })
            .map_err(ConnectError::Io)?;
        match client.read().map_err(ConnectError::Io)? {
            ServerMessage::Welcome {
                engine_id,
                namespace,
                ..
            } => {
                client.engine_id = engine_id;
                client.namespace = namespace;
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
