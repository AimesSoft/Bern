//! Players, routing and state merging.
//!
//! The hub owns the identity and merged state of every player. Commands from
//! any entry are validated here and routed to the executor with the player's
//! current source version; the executor's reports are merged only when they
//! carry that version, so late reports from an older source never overwrite
//! the newer one. Merging a report never produces a command.

use crate::{
    ExecOp, ExecRequest, MediaCapabilities, MediaCommand, MediaExecutor, Observation, Phase,
    media_catalog,
};
use bern_protocol::{CommandError, CommandSpec, TypeTable, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// Owner name of players declared for the whole application.
pub const APPLICATION_OWNER: &str = "application";

/// Command label of loads started by a declaration's source binding.
const BINDING: &str = "source_binding";

type Reply = Box<dyn FnOnce(Result<Value, CommandError>) + Send>;
/// The reply of a request whose player was released, with the player id.
type Orphan = (String, Reply);
type Listener = Arc<dyn Fn(&MediaUpdate) + Send + Sync>;

/// A complete, versioned view of every player. Updates may reach listeners
/// on different threads out of order; receivers keep the newest revision.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaUpdate {
    pub revision: u64,
    /// `MediaPlayer` records ordered by player id.
    pub players: Vec<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestStatus {
    Accepted,
    Executed,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
struct RequestInfo {
    request: u64,
    command: &'static str,
    status: RequestStatus,
    version: u64,
    error: Option<String>,
}

struct Player {
    owner: String,
    source_bound: bool,
    source: Option<String>,
    version: u64,
    observed: Observation,
    request: Option<RequestInfo>,
}

/// A video view showing a player inside one window instance.
struct View {
    player: String,
    window: String,
}

struct Pending {
    player: String,
    version: u64,
    reply: Option<Reply>,
}

#[derive(Default)]
struct State {
    players: BTreeMap<String, Player>,
    revision: u64,
    executor: Option<Arc<dyn MediaExecutor>>,
    capabilities: Option<MediaCapabilities>,
    pending: BTreeMap<u64, Pending>,
    next_request: u64,
    ignored_reports: u64,
    /// Attached views by token.
    views: BTreeMap<String, View>,
}

struct Shared {
    state: Mutex<State>,
    /// Held while a request is handed to the executor, so requests reach it
    /// in the order the hub accepted them.
    send: Mutex<()>,
    listeners: Mutex<Vec<(u64, Listener)>>,
    next_listener: AtomicU64,
}

/// The players of one engine. Cloning shares the same hub.
#[derive(Clone)]
pub struct MediaHub {
    shared: Arc<Shared>,
}

/// Keeps a media listener registered until dropped.
pub struct MediaSubscription {
    shared: Arc<Shared>,
    id: u64,
}

impl Drop for MediaSubscription {
    fn drop(&mut self) {
        if let Ok(mut listeners) = self.shared.listeners.lock() {
            listeners.retain(|(id, _)| *id != self.id);
        }
    }
}

fn catalog() -> &'static (TypeTable, Vec<CommandSpec>) {
    static CATALOG: OnceLock<(TypeTable, Vec<CommandSpec>)> = OnceLock::new();
    CATALOG.get_or_init(media_catalog)
}

fn error(code: &str, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

fn not_found(player: &str) -> CommandError {
    error(
        "player_not_found",
        format!("no player `{player}`; see media.players"),
    )
}

fn unavailable() -> CommandError {
    error(
        "executor_unavailable",
        "no media executor is attached to run this player",
    )
}

fn loading(previous: &Observation) -> Observation {
    Observation {
        phase: Phase::Loading,
        volume: previous.volume,
        muted: previous.muted,
        ..Observation::idle()
    }
}

fn idle(previous: &Observation) -> Observation {
    Observation {
        volume: previous.volume,
        muted: previous.muted,
        ..Observation::idle()
    }
}

fn optional(value: Option<Value>) -> Value {
    match value {
        Some(value) => Value::some(value),
        None => Value::none(),
    }
}

fn player_value(id: &str, player: &Player, available: bool, views: usize) -> Value {
    let observed = &player.observed;
    let phase = if available {
        observed.phase
    } else {
        Phase::Unavailable
    };
    let request = player.request.as_ref().map(|info| {
        Value::Record(vec![
            Value::Text(info.command.into()),
            Value::unit_variant(match info.status {
                RequestStatus::Accepted => 0,
                RequestStatus::Executed => 1,
                RequestStatus::Failed => 2,
            }),
            Value::Int(info.version as i64),
            optional(info.error.clone().map(Value::Text)),
        ])
    });
    Value::Record(vec![
        Value::Text(id.into()),
        Value::Text(player.owner.clone()),
        Value::Bool(player.source_bound),
        optional(
            player
                .source
                .clone()
                .map(|url| Value::Record(vec![Value::Text(url)])),
        ),
        Value::Int(player.version as i64),
        Value::unit_variant(phase.index()),
        Value::Int(observed.position_ms),
        optional(observed.duration_ms.map(Value::Int)),
        Value::Bool(observed.seekable),
        Value::Float(observed.volume),
        Value::Bool(observed.muted),
        optional(observed.error.clone().map(Value::Text)),
        Value::Int(views as i64),
        optional(request),
    ])
}

impl State {
    fn views_of(&self, player: &str) -> usize {
        self.views
            .values()
            .filter(|view| view.player == player)
            .count()
    }

    fn value(&self, id: &str) -> Option<Value> {
        let available = self.executor.is_some();
        self.players
            .get(id)
            .map(|player| player_value(id, player, available, self.views_of(id)))
    }

    fn values(&self) -> Vec<Value> {
        self.players
            .keys()
            .filter_map(|id| self.value(id))
            .collect()
    }

    fn bump(&mut self) -> MediaUpdate {
        self.revision += 1;
        MediaUpdate {
            revision: self.revision,
            players: self.values(),
        }
    }

    fn request_id(&mut self) -> u64 {
        self.next_request += 1;
        self.next_request
    }

    /// Track a request sent on behalf of a source binding or an owner.
    fn track(&mut self, player: &str, command: &'static str, reply: Option<Reply>) -> u64 {
        let request = self.request_id();
        let version = self.players[player].version;
        self.players.get_mut(player).unwrap().request = Some(RequestInfo {
            request,
            command,
            status: RequestStatus::Accepted,
            version,
            error: None,
        });
        self.pending.insert(
            request,
            Pending {
                player: player.to_string(),
                version,
                reply,
            },
        );
        request
    }
}

fn text_param(params: &[Value], index: usize) -> String {
    match params.get(index) {
        Some(Value::Text(text)) => text.clone(),
        _ => String::new(),
    }
}

impl Default for MediaHub {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl MediaHub {
    /// A hub with the players declared for the whole application. They
    /// live as long as the engine; no executor is attached yet.
    pub fn new(application_players: &[String]) -> Self {
        let mut state = State::default();
        for id in application_players {
            state.players.insert(
                id.clone(),
                Player {
                    owner: APPLICATION_OWNER.into(),
                    source_bound: false,
                    source: None,
                    version: 0,
                    observed: Observation::idle(),
                    request: None,
                },
            );
        }
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(state),
                send: Mutex::new(()),
                listeners: Mutex::new(Vec::new()),
                next_listener: AtomicU64::new(1),
            }),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state.lock().expect("media state lock")
    }

    fn notify(&self, update: MediaUpdate) {
        let listeners: Vec<Listener> = self
            .shared
            .listeners
            .lock()
            .expect("media listener lock")
            .iter()
            .map(|(_, listener)| Arc::clone(listener))
            .collect();
        for listener in listeners {
            listener(&update);
        }
    }

    /// Receive every media update until the subscription is dropped.
    pub fn subscribe(
        &self,
        listener: impl Fn(&MediaUpdate) + Send + Sync + 'static,
    ) -> MediaSubscription {
        let id = self.shared.next_listener.fetch_add(1, Ordering::Relaxed);
        self.shared
            .listeners
            .lock()
            .expect("media listener lock")
            .push((id, Arc::new(listener)));
        MediaSubscription {
            shared: Arc::clone(&self.shared),
            id,
        }
    }

    /// Current revision and every player's merged state.
    pub fn snapshot(&self) -> (u64, Vec<Value>) {
        let state = self.state();
        (state.revision, state.values())
    }

    /// Merged state of one player.
    pub fn player(&self, id: &str) -> Option<Value> {
        self.state().value(id)
    }

    /// How many executor reports were ignored because they described an
    /// older source, a released player or a detached executor.
    pub fn ignored_reports(&self) -> u64 {
        self.state().ignored_reports
    }

    pub fn capabilities(&self) -> Option<MediaCapabilities> {
        self.state().capabilities.clone()
    }

    /// Attach the executor that runs every player. Players that already
    /// have a source load it again under a new source version, because a
    /// new executor holds none of the previous playback.
    pub fn attach_executor(
        &self,
        executor: Arc<dyn MediaExecutor>,
        capabilities: MediaCapabilities,
    ) {
        let _send = self.shared.send.lock().expect("media send lock");
        let mut state = self.state();
        state.executor = Some(Arc::clone(&executor));
        state.capabilities = Some(capabilities);
        let ids: Vec<String> = state.players.keys().cloned().collect();
        let mut requests = Vec::new();
        for id in ids {
            let player = state.players.get_mut(&id).unwrap();
            let Some(url) = player.source.clone() else {
                player.observed = idle(&player.observed);
                continue;
            };
            player.version += 1;
            player.observed = loading(&player.observed);
            let version = player.version;
            let request = state.track(&id, BINDING, None);
            requests.push(ExecRequest {
                request,
                player: id,
                source_version: version,
                op: ExecOp::Load { url },
            });
        }
        let update = state.bump();
        drop(state);
        self.notify(update);
        for request in requests {
            executor.execute(request);
        }
    }

    /// The executor is gone. Players report `Unavailable`, waiting requests
    /// fail and later commands are refused until an executor attaches.
    pub fn detach_executor(&self) {
        let _send = self.shared.send.lock().expect("media send lock");
        let mut state = self.state();
        state.executor = None;
        let pending = std::mem::take(&mut state.pending);
        for player in state.players.values_mut() {
            if let Some(info) = &mut player.request
                && info.status == RequestStatus::Accepted
            {
                info.status = RequestStatus::Failed;
                info.error = Some("executor_unavailable".into());
            }
        }
        let update = state.bump();
        drop(state);
        self.notify(update);
        for (_, pending) in pending {
            if let Some(reply) = pending.reply {
                reply(Err(error(
                    "executor_unavailable",
                    "the media executor disconnected; the outcome is unknown",
                )));
            }
        }
    }

    /// Create a player owned by a component instance. Its source follows
    /// the declaration's binding through [`MediaHub::bind`].
    pub fn create(&self, id: &str, owner: &str) -> Result<(), CommandError> {
        let mut state = self.state();
        if state.players.contains_key(id) {
            return Err(error(
                "player_exists",
                format!("player `{id}` already exists; one declaration is mounted twice"),
            ));
        }
        state.players.insert(
            id.to_string(),
            Player {
                owner: owner.to_string(),
                source_bound: true,
                source: None,
                version: 0,
                observed: Observation::idle(),
                request: None,
            },
        );
        let update = state.bump();
        drop(state);
        self.notify(update);
        Ok(())
    }

    /// The declaration's source binding has `url`. The same value never
    /// reloads; a new value loads under a new source version.
    pub fn bind(&self, id: &str, url: Option<String>) -> Result<u64, CommandError> {
        let _send = self.shared.send.lock().expect("media send lock");
        let mut state = self.state();
        let player = state.players.get_mut(id).ok_or_else(|| not_found(id))?;
        if !player.source_bound {
            return Err(error(
                "not_source_bound",
                format!("player `{id}` is not bound to a declaration; use media.open"),
            ));
        }
        if player.source == url {
            return Ok(player.version);
        }
        player.version += 1;
        player.source = url.clone();
        player.observed = match url {
            Some(_) => loading(&player.observed),
            None => idle(&player.observed),
        };
        let version = player.version;
        let executor = state.executor.clone();
        let request = executor.as_ref().map(|_| state.track(id, BINDING, None));
        let update = state.bump();
        drop(state);
        self.notify(update);
        if let (Some(executor), Some(request)) = (executor, request) {
            executor.execute(ExecRequest {
                request,
                player: id.to_string(),
                source_version: version,
                op: match url {
                    Some(url) => ExecOp::Load { url },
                    None => ExecOp::Unload,
                },
            });
        }
        Ok(version)
    }

    /// The owner of a player ended its scope. Waiting requests fail and
    /// the executor releases the player; application players stay.
    pub fn release(&self, id: &str) {
        let _send = self.shared.send.lock().expect("media send lock");
        let mut state = self.state();
        let Some((replies, request)) = Self::remove(&mut state, id) else {
            return;
        };
        let executor = state.executor.clone();
        let update = state.bump();
        drop(state);
        self.finish_release(update, executor, vec![request], replies);
    }

    /// A window instance closed: its views detach and the players its
    /// components owned are released. Application players keep playing.
    pub fn release_window(&self, window: &str) {
        let _send = self.shared.send.lock().expect("media send lock");
        let mut state = self.state();
        let before = state.views.len();
        state.views.retain(|_, view| view.window != window);
        let owned: Vec<String> = state
            .players
            .iter()
            .filter(|(_, player)| player.owner == window)
            .map(|(id, _)| id.clone())
            .collect();
        if owned.is_empty() && before == state.views.len() {
            return;
        }
        let mut released = Vec::new();
        let mut replies = Vec::new();
        for id in owned {
            if let Some((mut orphaned, request)) = Self::remove(&mut state, &id) {
                replies.append(&mut orphaned);
                released.push(request);
            }
        }
        let executor = state.executor.clone();
        let update = state.bump();
        drop(state);
        self.finish_release(update, executor, released, replies);
    }

    /// Remove a player; returns the replies of its waiting requests and the
    /// release request for the executor.
    fn remove(state: &mut State, id: &str) -> Option<(Vec<Orphan>, ExecRequest)> {
        let player = state.players.remove(id)?;
        let orphaned: Vec<u64> = state
            .pending
            .iter()
            .filter(|(_, pending)| pending.player == id)
            .map(|(request, _)| *request)
            .collect();
        let replies = orphaned
            .into_iter()
            .filter_map(|request| state.pending.remove(&request)?.reply)
            .map(|reply| (id.to_string(), reply))
            .collect();
        state.views.retain(|_, view| view.player != id);
        let release = ExecRequest {
            request: state.request_id(),
            player: id.to_string(),
            source_version: player.version,
            op: ExecOp::Release,
        };
        Some((replies, release))
    }

    fn finish_release(
        &self,
        update: MediaUpdate,
        executor: Option<Arc<dyn MediaExecutor>>,
        released: Vec<ExecRequest>,
        replies: Vec<Orphan>,
    ) {
        self.notify(update);
        for (id, reply) in replies {
            reply(Err(error(
                "player_released",
                format!("player `{id}` was released by its owner"),
            )));
        }
        if let Some(executor) = executor {
            for request in released {
                executor.execute(request);
            }
        }
    }

    /// A video view in `window` shows `player`. `token` identifies the
    /// attachment until [`MediaHub::detach_view`].
    pub fn attach_view(&self, token: &str, player: &str, window: &str) {
        let mut state = self.state();
        if !state.players.contains_key(player) {
            return;
        }
        state.views.insert(
            token.to_string(),
            View {
                player: player.to_string(),
                window: window.to_string(),
            },
        );
        let update = state.bump();
        drop(state);
        self.notify(update);
    }

    /// A video view detached; unknown tokens are ignored.
    pub fn detach_view(&self, token: &str) {
        let mut state = self.state();
        if state.views.remove(token).is_none() {
            return;
        }
        let update = state.bump();
        drop(state);
        self.notify(update);
    }

    /// Run a media command. Queries answer at once; other commands answer
    /// when the executor has performed them. Returns the executor request
    /// id when the command was routed.
    pub fn dispatch(
        &self,
        command: MediaCommand,
        params: Vec<Value>,
        reply: impl FnOnce(Result<Value, CommandError>) + Send + 'static,
    ) -> Option<u64> {
        let (types, specs) = catalog();
        let spec = &specs[command.index() as usize];
        if let Err(problem) = check_params(spec, &params, types) {
            reply(Err(problem));
            return None;
        }
        match command {
            MediaCommand::Players => {
                reply(Ok(Value::List(self.snapshot().1)));
                None
            }
            MediaCommand::State => {
                let id = text_param(&params, 0);
                reply(self.player(&id).ok_or_else(|| not_found(&id)));
                None
            }
            _ => self.route(command, params, Box::new(reply)),
        }
    }

    fn route(&self, command: MediaCommand, params: Vec<Value>, reply: Reply) -> Option<u64> {
        let _send = self.shared.send.lock().expect("media send lock");
        let mut state = self.state();
        let id = text_param(&params, 0);
        let prepared = (|| {
            let executor = state.executor.clone();
            let player = state.players.get_mut(&id).ok_or_else(|| not_found(&id))?;
            let executor = executor.ok_or_else(unavailable)?;
            let playable = |player: &Player| {
                if player.source.is_none() {
                    return Err(error("no_source", format!("player `{id}` has no source")));
                }
                if player.observed.phase == Phase::Failed {
                    return Err(error(
                        "media_failed",
                        player
                            .observed
                            .error
                            .clone()
                            .unwrap_or_else(|| "the media failed to load".into()),
                    ));
                }
                Ok(())
            };
            let op = match command {
                MediaCommand::Open => {
                    if player.source_bound {
                        return Err(error(
                            "source_bound",
                            format!(
                                "player `{id}` takes its source from a declaration; change the bound value through its owner's command"
                            ),
                        ));
                    }
                    let url = match params.get(1) {
                        Some(Value::Record(fields)) => match fields.first() {
                            Some(Value::Text(url)) => url.clone(),
                            _ => String::new(),
                        },
                        _ => String::new(),
                    };
                    if url.trim().is_empty() {
                        return Err(CommandError::invalid_params(
                            "`source.url` must not be empty",
                        ));
                    }
                    player.version += 1;
                    player.source = Some(url.clone());
                    player.observed = loading(&player.observed);
                    ExecOp::Load { url }
                }
                MediaCommand::Play => {
                    playable(player)?;
                    ExecOp::Play
                }
                MediaCommand::Pause => {
                    playable(player)?;
                    ExecOp::Pause
                }
                MediaCommand::Seek => {
                    playable(player)?;
                    let position_ms = match params.get(1) {
                        Some(Value::Int(position)) => *position,
                        _ => 0,
                    };
                    let duration = match (player.observed.seekable, player.observed.duration_ms) {
                        (true, Some(duration)) => duration,
                        _ => {
                            return Err(error(
                                "not_seekable",
                                "the current media cannot seek, for example live media or media whose duration is unknown",
                            ));
                        }
                    };
                    if !(0..=duration).contains(&position_ms) {
                        return Err(error(
                            "out_of_range",
                            format!("position must be between 0 and {duration} ms"),
                        ));
                    }
                    ExecOp::Seek { position_ms }
                }
                MediaCommand::SetVolume => {
                    let volume = match params.get(1) {
                        Some(Value::Float(volume)) => *volume,
                        _ => -1.0,
                    };
                    if !(0.0..=1.0).contains(&volume) {
                        return Err(error("out_of_range", "volume must be between 0 and 1"));
                    }
                    ExecOp::SetVolume { volume }
                }
                MediaCommand::SetMuted => ExecOp::SetMuted {
                    muted: matches!(params.get(1), Some(Value::Bool(true))),
                },
                MediaCommand::Players | MediaCommand::State => unreachable!("answered directly"),
            };
            Ok((executor, op))
        })();
        let (executor, op) = match prepared {
            Ok(prepared) => prepared,
            Err(problem) => {
                drop(state);
                reply(Err(problem));
                return None;
            }
        };
        let version = state.players[&id].version;
        let request = state.track(&id, command.id(), Some(reply));
        let update = state.bump();
        drop(state);
        self.notify(update);
        executor.execute(ExecRequest {
            request,
            player: id,
            source_version: version,
            op,
        });
        Some(request)
    }

    /// Run a media command and wait for its result. When the executor does
    /// not answer in time the result is unknown and reported as such; a
    /// late answer still updates the request status but nothing else.
    pub fn dispatch_blocking(
        &self,
        command: MediaCommand,
        params: Vec<Value>,
        timeout: Duration,
    ) -> Result<Value, CommandError> {
        let (sender, receiver) = channel();
        let request = self.dispatch(command, params, move |result| {
            let _ = sender.send(result);
        });
        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                if let Some(request) = request
                    && let Some(pending) = self.state().pending.get_mut(&request)
                {
                    pending.reply = None;
                }
                Err(error(
                    "media_timeout",
                    "the media executor did not answer in time; the outcome is unknown",
                ))
            }
        }
    }

    /// The executor finished request `request`. Unknown or abandoned
    /// requests are ignored.
    pub fn complete(&self, request: u64, result: Result<(), CommandError>) {
        let mut state = self.state();
        let Some(pending) = state.pending.remove(&request) else {
            return;
        };
        let mut update = None;
        if let Some(player) = state.players.get_mut(&pending.player)
            && let Some(info) = &mut player.request
            && info.request == request
        {
            match &result {
                Ok(()) => info.status = RequestStatus::Executed,
                Err(problem) => {
                    info.status = RequestStatus::Failed;
                    info.error = Some(problem.code.clone());
                }
            }
            update = Some(state.bump());
        }
        drop(state);
        if let Some(update) = update {
            self.notify(update);
        }
        if let Some(reply) = pending.reply {
            reply(result.map(|()| Value::Int(pending.version as i64)));
        }
    }

    /// Merge what the executor observed for `player` while it ran source
    /// version `source_version`. Returns false when the report was ignored.
    pub fn observe(&self, player: &str, source_version: u64, observation: Observation) -> bool {
        let mut state = self.state();
        let current = state.executor.is_some()
            && state
                .players
                .get(player)
                .is_some_and(|existing| existing.version == source_version);
        if !current {
            state.ignored_reports += 1;
            return false;
        }
        let existing = state.players.get_mut(player).unwrap();
        let observation = Observation {
            volume: observation.volume.clamp(0.0, 1.0),
            position_ms: observation.position_ms.max(0),
            ..observation
        };
        if existing.observed == observation {
            return true;
        }
        existing.observed = observation;
        let update = state.bump();
        drop(state);
        self.notify(update);
        true
    }
}

fn check_params(
    spec: &CommandSpec,
    params: &[Value],
    types: &TypeTable,
) -> Result<(), CommandError> {
    if params.len() != spec.params.len() {
        return Err(CommandError::invalid_params(format!(
            "`{}` expects {} parameters, got {}",
            spec.id,
            spec.params.len(),
            params.len()
        )));
    }
    for (value, param) in params.iter().zip(&spec.params) {
        value.check(&param.ty, types).map_err(|problem| {
            CommandError::invalid_params(format!("`{}`: {}", param.name, problem.message))
        })?;
    }
    Ok(())
}
