//! The WebUI link: the engine and the media hub of a separate engine host,
//! reached through its control channel as JSON messages.
//!
//! The page keeps its own UI session — windows, drafts, navigation, query
//! subscriptions — and asks the engine host only for what the engine owns:
//! commands, queries, state and the merged media state. The host may also
//! run session commands of external entries on the page and send player
//! requests to the page's media executor.
//!
//! A lost connection fails every waiting request with `connection_lost`:
//! the outcome is unknown and nothing is sent again by itself. After a
//! reconnect the page resynchronizes from a full snapshot.

use crate::link::{CommandReply, EngineLink, Guard, MediaLink, MediaReply};
use crate::{AppSpec, BernApp, BernCallbacks, SessionReply, UiBridge, slice, with_app};
use bern_adapter_json::{
    ClientMessage, MediaObservation, ServerMessage, from_json, media_players_from_json,
    params_from_json, params_json, state_from_json, to_json,
};
use bern_command::CallContext;
use bern_media::{
    MediaCapabilities, MediaCommand, MediaExecutor, MediaUpdate, Observation, media_catalog,
};
use bern_protocol::{
    Cancellation, CommandError, CommandOutcome, CommandStatus, Invocation, PROTOCOL_VERSION,
    ProtocolDescriptor, SessionCommand, StateUpdate, Value, session_catalog,
};
use bern_runtime::{SessionHandler, Sessions};
use serde_json::{Map, json};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, Weak};

type StateListener = Arc<dyn Fn(&StateUpdate) + Send + Sync>;
type MediaListener = Arc<dyn Fn(&MediaUpdate) + Send + Sync>;

/// Sends one message on the channel.
pub type Outbox = Box<dyn Fn(&str) + Send + Sync>;

/// Connection changes for the page, as JSON objects with a `state` of
/// `connected`, `disconnected` or `failed`.
pub type Status = Box<dyn Fn(&str) + Send + Sync>;

enum Pending {
    Command {
        command: usize,
        cancellation: Cancellation,
        reply: CommandReply,
    },
    Media {
        command: MediaCommand,
        reply: MediaReply,
    },
    OpenSession,
    Subscribe,
}

#[derive(Default)]
struct State {
    /// Welcome received and the full state applied.
    connected: bool,
    engine_id: String,
    session_id: Option<String>,
    revision: u64,
    values: Vec<Value>,
    media_revision: u64,
    players: Vec<Value>,
    listeners: Vec<(u64, StateListener)>,
    media_listeners: Vec<(u64, MediaListener)>,
    next_listener: u64,
    next_request: u64,
    pending: BTreeMap<String, Pending>,
    executor: Option<(Arc<dyn MediaExecutor>, MediaCapabilities)>,
    bridge: Option<Arc<UiBridge>>,
    /// The session to resume after a reconnect, and the engine it was on.
    previous: Option<(String, String)>,
    /// Players and views this page registered, unqualified, to register
    /// again on a resumed session: player → (owner, source), token →
    /// (player, window).
    players_created: BTreeMap<String, (String, Option<String>)>,
    views: BTreeMap<String, (String, String)>,
}

/// The remote engine and media hub of one page.
pub struct Remote {
    descriptor: &'static ProtocolDescriptor,
    token: Option<String>,
    send: Outbox,
    status: Status,
    state: Mutex<State>,
    me: Weak<Remote>,
}

fn lost() -> CommandError {
    CommandError::new(
        "connection_lost",
        "the connection to the engine closed before it answered; the outcome is unknown",
    )
}

fn unavailable() -> CommandError {
    CommandError::new(
        "engine_unavailable",
        "the page is not connected to its engine",
    )
}

impl Remote {
    pub fn new(
        descriptor: &'static ProtocolDescriptor,
        token: Option<String>,
        send: Outbox,
        status: Status,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            descriptor,
            token,
            send,
            status,
            state: Mutex::new(State {
                values: descriptor
                    .state
                    .iter()
                    .map(|field| {
                        Value::default_for(&field.ty, &descriptor.types).unwrap_or(Value::Unit)
                    })
                    .collect(),
                ..State::default()
            }),
            me: me.clone(),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("remote lock")
    }

    fn post(&self, message: &ClientMessage) {
        if let Ok(text) = serde_json::to_string(message) {
            (self.send)(&text);
        }
    }

    fn request_id(state: &mut State) -> String {
        state.next_request += 1;
        format!("p{}", state.next_request)
    }

    /// The channel opened: introduce the page.
    pub fn open(&self) {
        self.post(&ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            entry: "webui".into(),
            catalog_fingerprint: Some(self.descriptor.fingerprint().to_string()),
            token: self.token.clone(),
        });
    }

    /// The channel closed. Waiting requests fail with an unknown outcome.
    pub fn closed(&self, reason: &str) {
        let pending = {
            let mut state = self.lock();
            state.connected = false;
            if let Some(session) = state.session_id.take() {
                state.previous = Some((state.engine_id.clone(), session));
            }
            std::mem::take(&mut state.pending)
        };
        for (_, pending) in pending {
            match pending {
                Pending::Command { reply, .. } => reply(CommandOutcome {
                    status: CommandStatus::Completed,
                    revision: self.lock().revision,
                    result: Err(lost()),
                }),
                Pending::Media { reply, .. } => reply(Err(lost())),
                Pending::OpenSession | Pending::Subscribe => {}
            }
        }
        (self.status)(&json!({ "state": "disconnected", "reason": reason }).to_string());
    }

    /// Requests whose query was superseded: stop waiting and tell the host.
    fn sweep_cancelled(&self) {
        let cancelled: Vec<(String, CommandReply)> = {
            let mut state = self.lock();
            let ids: Vec<String> = state
                .pending
                .iter()
                .filter(|(_, pending)| {
                    matches!(pending, Pending::Command { cancellation, .. } if cancellation.is_cancelled())
                })
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| match state.pending.remove(&id) {
                    Some(Pending::Command { reply, .. }) => Some((id, reply)),
                    _ => None,
                })
                .collect()
        };
        let revision = self.lock().revision;
        for (request_id, reply) in cancelled {
            self.post(&ClientMessage::Cancel {
                request_id: request_id.clone(),
            });
            reply(CommandOutcome {
                status: CommandStatus::Completed,
                revision,
                result: Err(CommandError::new("cancelled", "the request was cancelled")),
            });
        }
    }

    fn failed(&self, error: &CommandError) {
        (self.status)(
            &json!({ "state": "failed", "code": error.code, "message": error.message }).to_string(),
        );
    }

    /// One message from the engine host.
    pub fn receive(&self, text: &str) {
        let message: ServerMessage = match serde_json::from_str(text) {
            Ok(message) => message,
            Err(problem) => {
                self.failed(&CommandError::new("invalid_message", problem.to_string()));
                return;
            }
        };
        match message {
            ServerMessage::Welcome {
                engine_id,
                catalog_fingerprint,
                ..
            } => {
                if catalog_fingerprint != self.descriptor.fingerprint().to_string() {
                    self.failed(&CommandError::new(
                        "catalog_mismatch",
                        "the engine runs a different command catalog; rebuild the page",
                    ));
                    return;
                }
                let (open, subscribe, executor, resume) = {
                    let mut state = self.lock();
                    // A session survives only on the same engine instance.
                    let resume = state
                        .previous
                        .take()
                        .filter(|(engine, _)| *engine == engine_id)
                        .map(|(_, session)| session);
                    state.engine_id = engine_id;
                    let open = Self::request_id(&mut state);
                    state.pending.insert(open.clone(), Pending::OpenSession);
                    let subscribe = Self::request_id(&mut state);
                    state.pending.insert(subscribe.clone(), Pending::Subscribe);
                    (open, subscribe, state.executor.clone(), resume)
                };
                // The session first, so the page knows its id once connected.
                self.post(&ClientMessage::OpenSession {
                    request_id: open,
                    resume,
                });
                if let Some((_, capabilities)) = executor {
                    self.post(&attach_message(&capabilities));
                }
                self.post(&ClientMessage::Subscribe {
                    request_id: subscribe,
                });
            }
            ServerMessage::SessionOpened {
                request_id,
                session_id,
            } => {
                let (windows, players, views) = {
                    let mut state = self.lock();
                    if !matches!(
                        state.pending.remove(&request_id),
                        Some(Pending::OpenSession)
                    ) {
                        return;
                    }
                    state.session_id = Some(session_id);
                    (
                        state
                            .bridge
                            .as_ref()
                            .map(|bridge| bridge.windows())
                            .unwrap_or_default(),
                        state.players_created.clone(),
                        state.views.clone(),
                    )
                };
                // Tell the engine host what this page already shows: after a
                // reconnect its windows, players and views carry over.
                self.post(&ClientMessage::SessionWindows { windows });
                for (player, (owner, source)) in players {
                    self.post(&ClientMessage::MediaCreate {
                        player: player.clone(),
                        owner: self.qualified(&owner),
                    });
                    self.post(&ClientMessage::MediaBind {
                        player,
                        url: source,
                    });
                }
                for (token, (player, window)) in views {
                    self.post(&ClientMessage::MediaView {
                        token,
                        player: Some(player),
                        window: Some(self.qualified(&window)),
                    });
                }
            }
            ServerMessage::State {
                request_id,
                revision,
                values,
            } => {
                let Ok(changes) = state_from_json(self.descriptor, &values) else {
                    self.failed(&CommandError::new(
                        "invalid_state",
                        "the engine sent an unreadable state",
                    ));
                    return;
                };
                let revision = revision.parse().unwrap_or(0);
                let (listeners, engine_id, session_id) = {
                    let mut state = self.lock();
                    if !matches!(state.pending.remove(&request_id), Some(Pending::Subscribe)) {
                        return;
                    }
                    for (index, value) in &changes {
                        state.values[*index] = value.clone();
                    }
                    state.revision = revision;
                    state.connected = true;
                    (
                        state
                            .listeners
                            .iter()
                            .map(|(_, listener)| Arc::clone(listener))
                            .collect::<Vec<_>>(),
                        state.engine_id.clone(),
                        state.session_id.clone(),
                    )
                };
                // A full update: after a reconnect the engine may have moved on.
                let update = StateUpdate { revision, changes };
                for listener in listeners {
                    listener(&update);
                }
                (self.status)(
                    &json!({ "state": "connected", "engine_id": engine_id, "session_id": session_id })
                        .to_string(),
                );
            }
            ServerMessage::Update { revision, changes } => {
                let Ok(changes) = state_from_json(self.descriptor, &changes) else {
                    return;
                };
                let revision: u64 = revision.parse().unwrap_or(0);
                let listeners = {
                    let mut state = self.lock();
                    if revision <= state.revision {
                        return;
                    }
                    for (index, value) in &changes {
                        state.values[*index] = value.clone();
                    }
                    state.revision = revision;
                    state
                        .listeners
                        .iter()
                        .map(|(_, listener)| Arc::clone(listener))
                        .collect::<Vec<_>>()
                };
                let update = StateUpdate { revision, changes };
                for listener in listeners {
                    listener(&update);
                }
            }
            ServerMessage::Media { revision, players } => {
                let Ok(players) = media_players_from_json(&players) else {
                    return;
                };
                let revision: u64 = revision.parse().unwrap_or(0);
                let listeners = {
                    let mut state = self.lock();
                    if revision < state.media_revision {
                        return;
                    }
                    state.media_revision = revision;
                    state.players = players.clone();
                    state
                        .media_listeners
                        .iter()
                        .map(|(_, listener)| Arc::clone(listener))
                        .collect::<Vec<_>>()
                };
                let update = MediaUpdate { revision, players };
                for listener in listeners {
                    listener(&update);
                }
            }
            ServerMessage::Result {
                request_id,
                revision,
                result,
                error,
                ..
            } => {
                let pending = self.lock().pending.remove(&request_id);
                let revision = revision.parse().unwrap_or(0);
                match pending {
                    Some(Pending::Command { command, reply, .. }) => {
                        let spec = &self.descriptor.commands[command];
                        let result = match (result, error) {
                            (_, Some(error)) => Err(error),
                            (Some(json), None) => {
                                from_json(&json, &spec.result, &self.descriptor.types).map_err(
                                    |problem| CommandError::new("invalid_result", problem.message),
                                )
                            }
                            (None, None) => from_json(
                                &serde_json::Value::Null,
                                &spec.result,
                                &self.descriptor.types,
                            )
                            .map_err(|problem| {
                                CommandError::new("invalid_result", problem.message)
                            }),
                        };
                        reply(CommandOutcome {
                            status: CommandStatus::Completed,
                            revision,
                            result,
                        });
                    }
                    Some(Pending::Media { command, reply }) => {
                        let (types, specs) = media_catalog();
                        let spec = &specs[command.index() as usize];
                        let result = match (result, error) {
                            (_, Some(error)) => Err(error),
                            (json, None) => {
                                from_json(&json.unwrap_or_default(), &spec.result, &types).map_err(
                                    |problem| CommandError::new("invalid_result", problem.message),
                                )
                            }
                        };
                        reply(result);
                    }
                    _ => {}
                }
            }
            ServerMessage::Error { request_id, error } => {
                let pending = request_id.and_then(|id| self.lock().pending.remove(&id));
                match pending {
                    Some(Pending::Command { reply, .. }) => reply(CommandOutcome {
                        status: CommandStatus::Completed,
                        revision: self.lock().revision,
                        result: Err(error),
                    }),
                    Some(Pending::Media { reply, .. }) => reply(Err(error)),
                    _ => self.failed(&error),
                }
            }
            ServerMessage::SessionRequest {
                request_id,
                command,
                params,
            } => self.session_request(request_id, &command, &params),
            ServerMessage::MediaRequest(message) => {
                let executor = self
                    .lock()
                    .executor
                    .as_ref()
                    .map(|(executor, _)| Arc::clone(executor));
                match (message.to_request(), executor) {
                    (Ok(request), Some(executor)) => executor.execute(request),
                    (Ok(request), None) => self.post(&ClientMessage::MediaComplete {
                        request: request.request.to_string(),
                        ok: false,
                        error: Some(CommandError::new(
                            "executor_unavailable",
                            "the page runs no players",
                        )),
                    }),
                    (Err(error), _) => self.post(&ClientMessage::MediaComplete {
                        request: message.request.clone(),
                        ok: false,
                        error: Some(error),
                    }),
                }
            }
            ServerMessage::Catalog { .. } | ServerMessage::Sessions { .. } => {}
        }
        self.sweep_cancelled();
    }

    /// An external entry runs a session command on this page.
    fn session_request(
        &self,
        request_id: String,
        command: &str,
        params: &Map<String, serde_json::Value>,
    ) {
        let (types, specs) = session_catalog();
        let me = self.me.clone();
        let answer = move |result: Result<Value, CommandError>,
                           spec: Option<&bern_protocol::CommandSpec>| {
            let Some(remote) = me.upgrade() else { return };
            let (types, _) = session_catalog();
            let message = match result {
                Ok(value) => ClientMessage::SessionResult {
                    request_id: request_id.clone(),
                    ok: true,
                    result: spec.map(|spec| to_json(&value, &spec.result, &types)),
                    error: None,
                },
                Err(error) => ClientMessage::SessionResult {
                    request_id: request_id.clone(),
                    ok: false,
                    result: None,
                    error: Some(error),
                },
            };
            remote.post(&message);
        };
        let Some((session_command, spec)) =
            SessionCommand::from_id(command).and_then(|session_command| {
                specs
                    .iter()
                    .find(|spec| spec.id == command)
                    .map(|spec| (session_command, spec.clone()))
            })
        else {
            answer(Err(CommandError::unknown_command(command)), None);
            return;
        };
        let params = match params_from_json(&spec, params, &types) {
            Ok(params) => params,
            Err(error) => {
                answer(Err(error), Some(&spec));
                return;
            }
        };
        let Some(bridge) = self.lock().bridge.clone() else {
            answer(
                Err(CommandError::new(
                    "requires_ui_session",
                    "the page has no UI session",
                )),
                Some(&spec),
            );
            return;
        };
        let reply: SessionReply = Box::new(move |result| answer(result, Some(&spec)));
        bridge.run(session_command, params, reply);
    }

    /// The page's open windows changed; external entries list them.
    pub fn windows_changed(&self, windows: Vec<String>) {
        if self.lock().connected {
            self.post(&ClientMessage::SessionWindows { windows });
        }
    }

    /// `window` qualified by this page's session: window ids are only
    /// unique within one session, and the engine host serves several.
    fn qualified(&self, window: &str) -> String {
        match &self.lock().session_id {
            Some(session) if !window.starts_with(&format!("{session}/")) => {
                format!("{session}/{window}")
            }
            _ => window.to_string(),
        }
    }
}

fn attach_message(capabilities: &MediaCapabilities) -> ClientMessage {
    ClientMessage::MediaAttach {
        backend: capabilities.backend.clone(),
        backend_version: capabilities.backend_version.clone(),
        background_playback: capabilities.background_playback,
        multi_view: capabilities.multi_view,
    }
}

struct Listening {
    remote: Weak<Remote>,
    id: u64,
    media: bool,
}

impl Drop for Listening {
    fn drop(&mut self) {
        if let Some(remote) = self.remote.upgrade() {
            let mut state = remote.lock();
            if self.media {
                state.media_listeners.retain(|(id, _)| *id != self.id);
            } else {
                state.listeners.retain(|(id, _)| *id != self.id);
            }
        }
    }
}

/// The engine side of a [`Remote`].
pub struct RemoteEngine(pub Arc<Remote>);

impl EngineLink for RemoteEngine {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        self.0.descriptor
    }

    /// The engine host traces its jobs; the page only the UI's side.
    fn local(&self) -> bool {
        false
    }

    fn engine_id(&self) -> String {
        self.0.lock().engine_id.clone()
    }

    fn snapshot(&self) -> (u64, Vec<Value>) {
        let state = self.0.lock();
        (state.revision, state.values.clone())
    }

    fn dispatch(
        &self,
        invocation: Invocation,
        _context: CallContext,
        cancellation: Cancellation,
        reply: CommandReply,
    ) {
        let remote = &self.0;
        remote.sweep_cancelled();
        let spec = &remote.descriptor.commands[invocation.command];
        let message = {
            let mut state = remote.lock();
            if !state.connected {
                let revision = state.revision;
                drop(state);
                reply(CommandOutcome {
                    status: CommandStatus::Completed,
                    revision,
                    result: Err(unavailable()),
                });
                return;
            }
            let request_id = Remote::request_id(&mut state);
            state.pending.insert(
                request_id.clone(),
                Pending::Command {
                    command: invocation.command,
                    cancellation,
                    reply,
                },
            );
            ClientMessage::Command {
                request_id,
                engine_id: state.engine_id.clone(),
                command: spec.id.clone(),
                params: params_json(spec, &invocation.params, &remote.descriptor.types),
            }
        };
        remote.post(&message);
    }

    fn subscribe(&self, listener: Box<dyn Fn(&StateUpdate) + Send + Sync>) -> Guard {
        let mut state = self.0.lock();
        state.next_listener += 1;
        let id = state.next_listener;
        state.listeners.push((id, Arc::from(listener)));
        Box::new(Listening {
            remote: Arc::downgrade(&self.0),
            id,
            media: false,
        })
    }

    fn stop(&self) {}

    fn windows_changed(&self, windows: Vec<String>) {
        self.0.windows_changed(windows);
    }
}

/// The media side of a [`Remote`].
pub struct RemoteMedia(pub Arc<Remote>);

impl MediaLink for RemoteMedia {
    fn snapshot(&self) -> (u64, Vec<Value>) {
        let state = self.0.lock();
        (state.media_revision, state.players.clone())
    }

    fn subscribe(&self, listener: Box<dyn Fn(&MediaUpdate) + Send + Sync>) -> Guard {
        let mut state = self.0.lock();
        state.next_listener += 1;
        let id = state.next_listener;
        state.media_listeners.push((id, Arc::from(listener)));
        Box::new(Listening {
            remote: Arc::downgrade(&self.0),
            id,
            media: true,
        })
    }

    fn dispatch(&self, command: MediaCommand, params: Vec<Value>, reply: MediaReply) {
        let remote = &self.0;
        let (types, specs) = media_catalog();
        let spec = &specs[command.index() as usize];
        let message = {
            let mut state = remote.lock();
            if !state.connected {
                drop(state);
                reply(Err(unavailable()));
                return;
            }
            let request_id = Remote::request_id(&mut state);
            state
                .pending
                .insert(request_id.clone(), Pending::Media { command, reply });
            ClientMessage::Command {
                request_id,
                engine_id: state.engine_id.clone(),
                command: command.id().into(),
                params: params_json(spec, &params, &types),
            }
        };
        remote.post(&message);
    }

    fn create(&self, player: &str, owner: &str) -> Result<(), CommandError> {
        self.0
            .lock()
            .players_created
            .insert(player.into(), (owner.into(), None));
        self.0.post(&ClientMessage::MediaCreate {
            player: player.into(),
            owner: self.0.qualified(owner),
        });
        Ok(())
    }

    /// The engine host decides the source version; failures arrive as
    /// errors on the channel.
    fn bind(&self, player: &str, url: Option<String>) -> Result<u64, CommandError> {
        if let Some(entry) = self.0.lock().players_created.get_mut(player) {
            entry.1 = url.clone();
        }
        self.0.post(&ClientMessage::MediaBind {
            player: player.into(),
            url,
        });
        Ok(0)
    }

    fn release(&self, player: &str) {
        self.0.lock().players_created.remove(player);
        self.0.post(&ClientMessage::MediaRelease {
            player: player.into(),
        });
    }

    fn release_window(&self, window: &str) {
        self.0
            .lock()
            .players_created
            .retain(|_, (owner, _)| owner != window);
        self.0.post(&ClientMessage::MediaReleaseWindow {
            window: self.0.qualified(window),
        });
    }

    fn attach_view(&self, token: &str, player: &str, window: &str) {
        self.0
            .lock()
            .views
            .insert(token.into(), (player.into(), window.into()));
        self.0.post(&ClientMessage::MediaView {
            token: token.into(),
            player: Some(player.into()),
            window: Some(self.0.qualified(window)),
        });
    }

    fn detach_view(&self, token: &str) {
        self.0.lock().views.remove(token);
        self.0.post(&ClientMessage::MediaView {
            token: token.into(),
            player: None,
            window: None,
        });
    }

    fn attach_executor(&self, executor: Arc<dyn MediaExecutor>, capabilities: MediaCapabilities) {
        let connected = {
            let mut state = self.0.lock();
            state.executor = Some((executor, capabilities.clone()));
            state.connected
        };
        if connected {
            self.0.post(&attach_message(&capabilities));
        }
    }

    fn detach_executor(&self) {
        self.0.lock().executor = None;
    }

    fn complete(&self, request: u64, result: Result<(), CommandError>) {
        self.0.post(&ClientMessage::MediaComplete {
            request: request.to_string(),
            ok: result.is_ok(),
            error: result.err(),
        });
    }

    fn observe(&self, player: &str, version: u64, observation: Observation) {
        self.0.post(&ClientMessage::MediaEvent {
            player: player.into(),
            source_version: version.to_string(),
            observation: MediaObservation::from_observation(&observation),
        });
    }
}

/// Start a page whose engine runs in a separate engine host. Messages go
/// out through `send`; the page passes incoming ones to
/// [`bern_remote_receive`].
pub fn start(
    spec: AppSpec,
    descriptor: &'static ProtocolDescriptor,
    token: Option<String>,
    callbacks: BernCallbacks,
    send: Outbox,
    status: Status,
) -> *mut BernApp {
    let remote = Remote::new(descriptor, token, send, status);
    let app = crate::assemble(
        Arc::new(RemoteEngine(Arc::clone(&remote))),
        Arc::new(RemoteMedia(Arc::clone(&remote))),
        Sessions::new(),
        "webui".into(),
        spec.session,
        callbacks,
        None,
    );
    remote.lock().bridge = Some(Arc::clone(&app.bridge));
    let mut app = app;
    app.remote = Some(remote);
    Box::into_raw(Box::new(app))
}

fn with_remote(app: *mut BernApp, body: impl FnOnce(&Remote)) {
    with_app(app, (), |app| {
        if let Some(remote) = &app.remote {
            let _ = catch_unwind(AssertUnwindSafe(|| body(remote)));
        }
    });
}

/// The page's channel to the engine host opened.
///
/// # Safety
/// `app` must be null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_remote_open(app: *mut BernApp) {
    with_remote(app, Remote::open);
}

/// One message from the engine host, as UTF-8 JSON.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_remote_receive(app: *mut BernApp, bytes: *const u8, len: usize) {
    // SAFETY: guaranteed by the caller.
    let bytes = unsafe { slice(bytes, len) };
    if let Ok(text) = std::str::from_utf8(bytes) {
        with_remote(app, |remote| remote.receive(text));
    }
}

/// The channel closed; `reason` is UTF-8 text.
///
/// # Safety
/// `bytes` must point to `len` readable bytes or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_remote_closed(app: *mut BernApp, bytes: *const u8, len: usize) {
    // SAFETY: guaranteed by the caller.
    let reason = String::from_utf8_lossy(unsafe { slice(bytes, len) }).into_owned();
    with_remote(app, |remote| remote.closed(&reason));
}
