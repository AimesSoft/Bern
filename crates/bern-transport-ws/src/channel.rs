//! The engine host's side of a WebUI page connection, independent of the
//! transport.
//!
//! A page authenticates with the channel token, then calls commands and
//! queries without waiting for earlier answers, subscribes to state and
//! media, registers itself as a live UI session that external entries can
//! address, and may run players as a media executor. When it goes away,
//! its session ends and the players of its windows are released.

use bern_adapter_json::{
    self as json, ClientMessage, MediaRequestMessage, ServerMessage, SessionInfo, from_json,
    media_players_json, params_from_json, params_json, state_json, to_json,
};
use bern_command::{CallContext, Entry, Host};
use bern_media::{
    ExecRequest, MediaCapabilities, MediaCommand, MediaExecutor, MediaHub, media_catalog,
};
use bern_protocol::{
    Cancellation, CommandError, Invocation, PROTOCOL_VERSION, SessionCommand, Value,
    session_catalog,
};
use bern_runtime::{SessionHandler, Sessions};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Sends one message to the page.
pub type Outbound = Arc<dyn Fn(&ServerMessage) + Send + Sync>;

/// How long an external session command waits for the page.
const SESSION_TIMEOUT: Duration = Duration::from_secs(10);

/// What every connection of one engine host shares.
pub struct Env {
    pub host: Host,
    pub sessions: Sessions,
    pub media: MediaHub,
    pub token: String,
    router: Arc<Router>,
    next: AtomicU64,
}

impl Env {
    pub fn new(host: Host, sessions: Sessions, media: MediaHub, token: String) -> Arc<Self> {
        Arc::new(Self {
            host,
            sessions,
            media,
            token,
            router: Arc::new(Router::default()),
            next: AtomicU64::new(1),
        })
    }
}

/// Routes player requests to the page that holds each player: the page
/// that created it, or for application players the page that last
/// showed it, else the page that attached most recently.
#[derive(Default)]
struct Router {
    state: Mutex<RouterState>,
}

#[derive(Default)]
struct RouterState {
    executors: Vec<(u64, Outbound)>,
    players: BTreeMap<String, u64>,
}

impl Router {
    /// Add a page; returns whether it is the first one.
    fn attach(&self, connection: u64, out: Outbound) -> bool {
        let mut state = self.state.lock().expect("router lock");
        state.executors.retain(|(id, _)| *id != connection);
        state.executors.push((connection, out));
        state.executors.len() == 1
    }

    /// Remove a page; returns whether none is left.
    fn detach(&self, connection: u64) -> bool {
        let mut state = self.state.lock().expect("router lock");
        state.executors.retain(|(id, _)| *id != connection);
        state.players.retain(|_, owner| *owner != connection);
        state.executors.is_empty()
    }

    fn claim(&self, player: &str, connection: u64) {
        self.state
            .lock()
            .expect("router lock")
            .players
            .insert(player.to_string(), connection);
    }

    fn forget(&self, player: &str) {
        self.state
            .lock()
            .expect("router lock")
            .players
            .remove(player);
    }
}

impl MediaExecutor for Router {
    fn execute(&self, request: ExecRequest) {
        let out = {
            let state = self.state.lock().expect("router lock");
            let owner = state.players.get(&request.player).copied();
            owner
                .and_then(|owner| state.executors.iter().find(|(id, _)| *id == owner))
                .or_else(|| state.executors.last())
                .map(|(_, out)| Arc::clone(out))
        };
        if let Some(out) = out {
            out(&ServerMessage::MediaRequest(
                MediaRequestMessage::from_request(&request),
            ));
        }
    }
}

/// An external session command waiting for the page's answer.
type Waiter = (SessionCommand, Sender<Result<Value, CommandError>>);

/// A page registered as a live UI session. External entries reach its
/// real controls through `session_request` messages.
struct WebSession {
    id: String,
    out: Outbound,
    windows: Mutex<Vec<String>>,
    waiting: Mutex<BTreeMap<String, Waiter>>,
    next: AtomicU64,
}

impl SessionHandler for WebSession {
    fn session_id(&self) -> String {
        self.id.clone()
    }

    fn windows(&self) -> Vec<String> {
        self.windows.lock().expect("windows lock").clone()
    }

    fn handle(&self, command: SessionCommand, params: Vec<Value>) -> Result<Value, CommandError> {
        if command.is_trace() {
            return self.trace(command, params);
        }
        self.forward(command, params)
    }
}

impl WebSession {
    /// The trace runs on both sides: the engine host records the engine,
    /// the page the UI. The host answers with both.
    fn trace(&self, command: SessionCommand, params: Vec<Value>) -> Result<Value, CommandError> {
        use bern_command::trace;
        let field = |value: &Value, index: usize| match value {
            Value::Record(fields) => fields.get(index).cloned(),
            _ => None,
        };
        match command {
            SessionCommand::TraceStart => {
                let Some(Value::Text(rules)) = params.first() else {
                    return Err(CommandError::invalid_params("`rules` must be text"));
                };
                let started = trace::start(rules)
                    .map_err(|message| CommandError::new("invalid_rules", message))?;
                // The page records its side with the same rules.
                let _ = self.forward(command, params);
                Ok(started.to_value())
            }
            SessionCommand::TraceStop => {
                let inline = matches!(params.first(), Some(Value::Optional(Some(value))) if **value == Value::Bool(true));
                let mut stopped = trace::stop(inline);
                // The page keeps its timeline in memory; the host writes it
                // next to its own.
                let page = self.forward(
                    command,
                    vec![Value::Optional(Some(Box::new(Value::Bool(true))))],
                );
                if let Ok(page) = page {
                    if let Some(Value::Int(events)) = field(&page, 1) {
                        stopped.events += events.max(0) as u64;
                    }
                    if let Some(Value::Int(dropped)) = field(&page, 2) {
                        stopped.dropped += dropped.max(0) as u64;
                    }
                    if let Some(Value::Optional(Some(timeline))) = field(&page, 3)
                        && let Value::Text(timeline) = *timeline
                    {
                        let path = match stopped.files.first() {
                            Some(file) => std::path::PathBuf::from(file.trim_end_matches(".json").to_string() + "-page.json"),
                            None => std::env::temp_dir()
                                .join(format!("bern-trace-page-{}.json", std::process::id())),
                        };
                        if std::fs::write(&path, timeline).is_ok() {
                            stopped.files.push(path.display().to_string());
                        }
                    }
                }
                Ok(stopped.to_value())
            }
            SessionCommand::TraceReport => {
                let top = match params.first() {
                    Some(Value::Optional(Some(value))) => match **value {
                        Value::Int(top) => top.clamp(0, 1000) as usize,
                        _ => 10,
                    },
                    _ => 10,
                };
                let reset = matches!(params.get(1), Some(Value::Optional(Some(value))) if **value == Value::Bool(true));
                let host = trace::report(top, reset).to_value();
                let Ok(Value::Record(page)) = self.forward(command, params) else {
                    return Ok(host);
                };
                let Value::Record(mut merged) = host else {
                    return Ok(host);
                };
                // Commands, queries and fields are the engine's; interactions
                // and frames the page's; spans and counts from both.
                for index in [2, 3] {
                    if let (Value::Int(host), Some(Value::Int(page))) = (&merged[index], page.get(index)) {
                        merged[index] = Value::Int(host + page);
                    }
                }
                for index in [7, 8, 9] {
                    if let Some(value) = page.get(index) {
                        merged[index] = value.clone();
                    }
                }
                if let (Value::List(host), Some(Value::List(page))) = (&mut merged[10], page.get(10)) {
                    host.extend(page.iter().cloned());
                }
                Ok(Value::Record(merged))
            }
            _ => self.forward(command, params),
        }
    }

    /// Run a session command on the page and wait for its answer.
    fn forward(&self, command: SessionCommand, params: Vec<Value>) -> Result<Value, CommandError> {
        let (types, specs) = session_catalog();
        let spec = specs
            .iter()
            .find(|spec| spec.id == command.id())
            .expect("catalog entry");
        let request_id = format!("s{}", self.next.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = channel();
        self.waiting
            .lock()
            .expect("waiting lock")
            .insert(request_id.clone(), (command, sender));
        (self.out)(&ServerMessage::SessionRequest {
            request_id: request_id.clone(),
            command: command.id().into(),
            params: params_json(spec, &params, &types),
        });
        let result = receiver.recv_timeout(SESSION_TIMEOUT);
        self.waiting
            .lock()
            .expect("waiting lock")
            .remove(&request_id);
        result.unwrap_or_else(|_| {
            Err(CommandError::new(
                "ui_timeout",
                "the page did not answer in time; the outcome is unknown",
            ))
        })
    }

    fn answer(&self, request_id: &str, result: Result<Option<serde_json::Value>, CommandError>) {
        let Some((command, sender)) = self
            .waiting
            .lock()
            .expect("waiting lock")
            .remove(request_id)
        else {
            return;
        };
        let (types, specs) = session_catalog();
        let spec = specs
            .iter()
            .find(|spec| spec.id == command.id())
            .expect("catalog entry");
        let value = result.and_then(|json| {
            from_json(&json.unwrap_or_default(), &spec.result, &types)
                .map_err(|problem| CommandError::new("ui_failed", problem.message))
        });
        let _ = sender.send(value);
    }

    /// The page went away: waiting entries learn the outcome is unknown.
    fn end(&self) {
        let waiting = std::mem::take(&mut *self.waiting.lock().expect("waiting lock"));
        for (_, (_, sender)) in waiting {
            let _ = sender.send(Err(CommandError::new(
                "session_ended",
                "the page disconnected before it answered; the outcome is unknown",
            )));
        }
    }
}

/// One page connection.
pub struct Connection {
    id: u64,
    env: Arc<Env>,
    out: Outbound,
    state: Mutex<ConnectionState>,
}

#[derive(Default)]
struct ConnectionState {
    authenticated: bool,
    session: Option<Arc<WebSession>>,
    /// Every window the session reported, to release their players.
    windows: BTreeSet<String>,
    executor: bool,
    queries: BTreeMap<String, Cancellation>,
    subscription: Option<(bern_command::Subscription, bern_media::MediaSubscription)>,
}

fn error(request_id: Option<String>, code: &str, message: impl Into<String>) -> ServerMessage {
    ServerMessage::Error {
        request_id,
        error: CommandError::new(code, message),
    }
}

impl Connection {
    pub fn new(env: Arc<Env>, out: Outbound) -> Arc<Self> {
        let id = env.next.fetch_add(1, Ordering::Relaxed);
        Arc::new(Self {
            id,
            env,
            out,
            state: Mutex::new(ConnectionState::default()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ConnectionState> {
        self.state.lock().expect("connection lock")
    }

    fn send(&self, message: &ServerMessage) {
        (self.out)(message);
    }

    /// One message from the page. Returns `false` when the connection
    /// should close, such as after a failed handshake.
    pub fn receive(self: &Arc<Self>, text: &str) -> bool {
        let message: ClientMessage = match serde_json::from_str(text) {
            Ok(message) => message,
            Err(problem) => {
                self.send(&error(None, "invalid_message", problem.to_string()));
                return self.lock().authenticated;
            }
        };
        if !self.lock().authenticated {
            return self.handshake(message);
        }
        self.handle(message);
        true
    }

    fn handshake(&self, message: ClientMessage) -> bool {
        let descriptor = self.env.host.descriptor();
        let ClientMessage::Hello {
            protocol_version,
            catalog_fingerprint,
            token,
            ..
        } = message
        else {
            self.send(&error(
                None,
                "handshake_required",
                "send a hello message first",
            ));
            return false;
        };
        if !token.is_some_and(|token| crate::digest::same(&token, &self.env.token)) {
            self.send(&error(
                None,
                "unauthorized",
                "this page has no valid token for the engine host; open the address it printed",
            ));
            return false;
        }
        if protocol_version != PROTOCOL_VERSION {
            self.send(&error(
                None,
                "protocol_mismatch",
                format!("the engine host speaks protocol {PROTOCOL_VERSION}, the page {protocol_version}"),
            ));
            return false;
        }
        if catalog_fingerprint
            .is_some_and(|fingerprint| fingerprint != descriptor.fingerprint().to_string())
        {
            self.send(&error(
                None,
                "catalog_mismatch",
                "the page was built for a different command catalog; rebuild it",
            ));
            return false;
        }
        self.lock().authenticated = true;
        self.send(&json::welcome(self.env.host.engine_id(), descriptor));
        true
    }

    fn handle(self: &Arc<Self>, message: ClientMessage) {
        let env = &self.env;
        let descriptor = env.host.descriptor();
        match message {
            ClientMessage::Hello { .. } => {
                self.send(&error(None, "invalid_message", "already connected"));
            }
            ClientMessage::Command {
                request_id,
                engine_id,
                command,
                params,
            } => {
                if engine_id != env.host.engine_id() {
                    self.send(&error(
                        Some(request_id),
                        "engine_mismatch",
                        "this request names a different engine instance",
                    ));
                } else if let Some((index, spec)) = descriptor.command(&command) {
                    match params_from_json(spec, &params, &descriptor.types) {
                        Err(problem) => self.send(&ServerMessage::Result {
                            request_id,
                            status: "completed".into(),
                            revision: env.host.snapshot().0.to_string(),
                            ok: false,
                            result: None,
                            error: Some(problem),
                        }),
                        Ok(params) => {
                            let cancellation = Cancellation::new();
                            if spec.is_query() {
                                self.lock()
                                    .queries
                                    .insert(request_id.clone(), cancellation.clone());
                            }
                            let me = Arc::clone(self);
                            let id = request_id.clone();
                            env.host.dispatch_cancellable(
                                Invocation {
                                    command: index,
                                    params,
                                },
                                CallContext {
                                    entry: Entry::Json,
                                    request_id: request_id.clone(),
                                },
                                cancellation,
                                move |outcome| {
                                    me.lock().queries.remove(&id);
                                    let spec = &me.env.host.descriptor().commands[index];
                                    let types = &me.env.host.descriptor().types;
                                    me.send(&ServerMessage::Result {
                                        request_id: id,
                                        status: "completed".into(),
                                        revision: outcome.revision.to_string(),
                                        ok: outcome.result.is_ok(),
                                        result: outcome
                                            .result
                                            .as_ref()
                                            .ok()
                                            .map(|value| to_json(value, &spec.result, types)),
                                        error: outcome.result.err(),
                                    });
                                },
                            );
                        }
                    }
                } else if let Some(media_command) = MediaCommand::from_id(&command) {
                    let (types, specs) = media_catalog();
                    let spec = specs[media_command.index() as usize].clone();
                    match params_from_json(&spec, &params, &types) {
                        Err(problem) => self.send(&ServerMessage::Result {
                            request_id,
                            status: "completed".into(),
                            revision: env.media.snapshot().0.to_string(),
                            ok: false,
                            result: None,
                            error: Some(problem),
                        }),
                        Ok(params) => {
                            let me = Arc::clone(self);
                            env.media.dispatch(media_command, params, move |result| {
                                let (types, _) = media_catalog();
                                me.send(&ServerMessage::Result {
                                    request_id,
                                    status: "completed".into(),
                                    revision: me.env.media.snapshot().0.to_string(),
                                    ok: result.is_ok(),
                                    result: result
                                        .as_ref()
                                        .ok()
                                        .map(|value| to_json(value, &spec.result, &types)),
                                    error: result.err(),
                                });
                            });
                        }
                    }
                } else {
                    self.send(&ServerMessage::Result {
                        request_id,
                        status: "completed".into(),
                        revision: env.host.snapshot().0.to_string(),
                        ok: false,
                        result: None,
                        error: Some(CommandError::unknown_command(&command)),
                    });
                }
            }
            ClientMessage::Cancel { request_id } => {
                if let Some(cancellation) = self.lock().queries.remove(&request_id) {
                    cancellation.cancel();
                }
            }
            ClientMessage::SessionCommand {
                request_id,
                session_id,
                command,
                params,
            } => {
                // Pages may drive sessions too, without blocking this
                // connection's other messages.
                let me = Arc::clone(self);
                std::thread::spawn(move || {
                    let (types, specs) = session_catalog();
                    let result = match (
                        SessionCommand::from_id(&command),
                        specs.iter().find(|spec| spec.id == command),
                    ) {
                        (Some(session_command), Some(spec)) => {
                            params_from_json(spec, &params, &types)
                                .and_then(|params| {
                                    me.env.sessions.handle(
                                        session_id.as_deref(),
                                        session_command,
                                        params,
                                    )
                                })
                                .map(|value| to_json(&value, &spec.result, &types))
                        }
                        _ => Err(CommandError::unknown_command(&command)),
                    };
                    me.send(&ServerMessage::Result {
                        request_id,
                        status: "completed".into(),
                        revision: me.env.host.snapshot().0.to_string(),
                        ok: result.is_ok(),
                        error: result.as_ref().err().cloned(),
                        result: result.ok(),
                    });
                });
            }
            ClientMessage::State { request_id } => {
                let (revision, values) = env.host.snapshot();
                let values: Vec<(usize, _)> = values.into_iter().enumerate().collect();
                self.send(&ServerMessage::State {
                    request_id,
                    revision: revision.to_string(),
                    values: state_json(descriptor, &values),
                });
            }
            ClientMessage::Catalog { request_id } => {
                let (session_types, session_commands) = session_catalog();
                let (media_types, media_commands) = media_catalog();
                self.send(&ServerMessage::Catalog {
                    request_id,
                    protocol: Box::new(descriptor.clone()),
                    session_commands,
                    session_types,
                    media_commands,
                    media_types,
                });
            }
            ClientMessage::Sessions { request_id } => self.send(&ServerMessage::Sessions {
                request_id,
                sessions: env
                    .sessions
                    .list()
                    .iter()
                    .map(|session| SessionInfo {
                        session_id: session.session_id(),
                        windows: session.windows(),
                    })
                    .collect(),
            }),
            ClientMessage::Subscribe { request_id } => {
                let out = Arc::clone(&self.out);
                let engine = env.host.subscribe(move |update| {
                    out(&ServerMessage::Update {
                        revision: update.revision.to_string(),
                        changes: state_json(descriptor, &update.changes),
                    });
                });
                let out = Arc::clone(&self.out);
                let players = env.media.subscribe(move |update| {
                    out(&ServerMessage::Media {
                        revision: update.revision.to_string(),
                        players: media_players_json(&update.players),
                    });
                });
                self.lock().subscription = Some((engine, players));
                let (revision, values) = env.host.snapshot();
                let values: Vec<(usize, _)> = values.into_iter().enumerate().collect();
                self.send(&ServerMessage::State {
                    request_id,
                    revision: revision.to_string(),
                    values: state_json(descriptor, &values),
                });
                let (revision, players) = env.media.snapshot();
                self.send(&ServerMessage::Media {
                    revision: revision.to_string(),
                    players: media_players_json(&players),
                });
            }
            ClientMessage::OpenSession { request_id, resume } => {
                let existing = self
                    .lock()
                    .session
                    .as_ref()
                    .map(|session| session.id.clone());
                let prefix = format!("{}-web", env.host.engine_id());
                let resumable = resume.filter(|id| {
                    id.starts_with(&prefix)
                        && !env
                            .sessions
                            .list()
                            .iter()
                            .any(|session| session.session_id() == *id)
                });
                let session_id = existing.unwrap_or_else(|| {
                    let session = Arc::new(WebSession {
                        id: resumable.unwrap_or_else(|| format!("{prefix}{}", self.id)),
                        out: Arc::clone(&self.out),
                        windows: Mutex::new(Vec::new()),
                        waiting: Mutex::new(BTreeMap::new()),
                        next: AtomicU64::new(1),
                    });
                    env.sessions.register(session.clone());
                    let id = session.id.clone();
                    self.lock().session = Some(session);
                    id
                });
                self.send(&ServerMessage::SessionOpened {
                    request_id,
                    session_id,
                });
                // A trace the host started records the page's side too. The
                // page answers on this connection, so it is asked from
                // another thread.
                if let Some(rules) = bern_command::trace::rules()
                    && let Some(session) = self.lock().session.clone()
                {
                    std::thread::spawn(move || {
                        let _ = session.forward(SessionCommand::TraceStart, vec![Value::Text(rules)]);
                    });
                }
            }
            ClientMessage::SessionWindows { windows } => {
                let mut state = self.lock();
                if let Some(session) = &state.session {
                    *session.windows.lock().expect("windows lock") = windows.clone();
                    let prefix = session.id.clone();
                    state
                        .windows
                        .extend(windows.iter().map(|window| format!("{prefix}/{window}")));
                }
            }
            ClientMessage::SessionResult {
                request_id,
                ok,
                result,
                error: problem,
            } => {
                let session = self.lock().session.clone();
                if let Some(session) = session {
                    let outcome = if ok {
                        Ok(result)
                    } else {
                        Err(problem.unwrap_or_else(|| {
                            CommandError::new("ui_failed", "the page reported a failure")
                        }))
                    };
                    session.answer(&request_id, outcome);
                }
            }
            ClientMessage::MediaAttach {
                backend,
                backend_version,
                background_playback,
                multi_view,
            } => {
                self.lock().executor = true;
                let first = env.router.attach(self.id, Arc::clone(&self.out));
                if first {
                    env.media.attach_executor(
                        Arc::clone(&env.router) as Arc<dyn MediaExecutor>,
                        MediaCapabilities {
                            backend,
                            backend_version,
                            background_playback,
                            multi_view,
                        },
                    );
                }
            }
            ClientMessage::MediaCreate { player, owner } => {
                match env.media.create(&player, &owner) {
                    Ok(()) => env.router.claim(&player, self.id),
                    Err(problem) => self.send(&ServerMessage::Error {
                        request_id: None,
                        error: problem,
                    }),
                }
            }
            ClientMessage::MediaBind { player, url } => {
                if let Err(problem) = env.media.bind(&player, url) {
                    self.send(&ServerMessage::Error {
                        request_id: None,
                        error: problem,
                    });
                }
            }
            ClientMessage::MediaRelease { player } => {
                env.media.release(&player);
                env.router.forget(&player);
            }
            ClientMessage::MediaReleaseWindow { window } => env.media.release_window(&window),
            ClientMessage::MediaView {
                token,
                player,
                window,
            } => match (player, window) {
                (Some(player), Some(window)) => {
                    env.router.claim(&player, self.id);
                    env.media
                        .attach_view(&format!("{}:{token}", self.id), &player, &window);
                }
                _ => env.media.detach_view(&format!("{}:{token}", self.id)),
            },
            ClientMessage::MediaComplete {
                request,
                ok,
                error: problem,
            } => {
                if let Ok(request) = request.parse() {
                    let result = if ok {
                        Ok(())
                    } else {
                        Err(problem.unwrap_or_else(|| {
                            CommandError::new("media_failed", "the page reported a failure")
                        }))
                    };
                    env.media.complete(request, result);
                }
            }
            ClientMessage::MediaEvent {
                player,
                source_version,
                observation,
            } => {
                if let (Ok(version), Ok(observation)) =
                    (source_version.parse(), observation.to_observation())
                {
                    env.media.observe(&player, version, observation);
                }
            }
        }
    }

    /// The page went away: its session ends, its windows' players are
    /// released, and if it ran players the others take over.
    pub fn closed(&self) {
        let (session, windows, executor) = {
            let mut state = self.lock();
            state.subscription = None;
            for (_, cancellation) in std::mem::take(&mut state.queries) {
                cancellation.cancel();
            }
            (
                state.session.take(),
                std::mem::take(&mut state.windows),
                std::mem::replace(&mut state.executor, false),
            )
        };
        if let Some(session) = session {
            self.env.sessions.unregister(&session.id);
            session.end();
        }
        for window in windows {
            self.env.media.release_window(&window);
        }
        if executor && self.env.router.detach(self.id) {
            self.env.media.detach_executor();
        }
    }
}

#[cfg(test)]
mod tests;
