//! Local development host for Bern's shared command engine.

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bern_core::{
    CommandError, CommandRequest, CommandSpec, Engine, PROTOCOL_VERSION, ServerMessage, Snapshot,
};
use std::{
    collections::BTreeSet,
    error::Error,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, broadcast};
use tower_http::services::ServeDir;

pub type HostError = Box<dyn Error + Send + Sync>;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_ID_BYTES: usize = 128;
const SNAPSHOT_QUEUE_SIZE: usize = 32;
static NEXT_HOST: AtomicU64 = AtomicU64::new(0);

struct EngineState {
    engine: Box<dyn Engine>,
    revision: u64,
}

#[derive(Clone)]
struct PublishedSnapshot {
    revision: u64,
    snapshot: Snapshot,
}

/// Owns one engine instance shared by every HTTP and WebSocket client.
pub struct Host {
    engine_id: String,
    commands: Vec<CommandSpec>,
    command_ids: BTreeSet<String>,
    state: Mutex<EngineState>,
    snapshots: broadcast::Sender<PublishedSnapshot>,
}

impl Host {
    pub fn new(engine: impl Engine) -> Result<Self, CommandError> {
        let commands = engine.commands();
        let mut command_ids = BTreeSet::new();
        for command in &commands {
            if command.id.is_empty() || command.label.trim().is_empty() {
                return Err(CommandError::new(
                    "invalid_catalog",
                    "Commands require an id and label",
                ));
            }
            if !command_ids.insert(command.id.clone()) {
                return Err(CommandError::new(
                    "invalid_catalog",
                    "Command ids must be unique",
                ));
            }
        }
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = NEXT_HOST.fetch_add(1, Ordering::Relaxed);
        let engine_id = format!("bern-{}-{epoch:x}-{sequence:x}", std::process::id());
        let (snapshots, _) = broadcast::channel(SNAPSHOT_QUEUE_SIZE);
        Ok(Self {
            engine_id,
            commands,
            command_ids,
            state: Mutex::new(EngineState {
                engine: Box::new(engine),
                revision: 0,
            }),
            snapshots,
        })
    }

    pub fn engine_id(&self) -> &str {
        &self.engine_id
    }
    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }

    fn published_snapshot(&self, state: &EngineState) -> PublishedSnapshot {
        PublishedSnapshot {
            revision: state.revision,
            snapshot: Snapshot {
                engine_id: self.engine_id.clone(),
                revision: state.revision.to_string(),
                values: state.engine.snapshot(),
            },
        }
    }

    pub async fn snapshot(&self) -> Snapshot {
        let state = self.state.lock().await;
        self.published_snapshot(&state).snapshot
    }

    async fn subscribe(&self) -> (PublishedSnapshot, broadcast::Receiver<PublishedSnapshot>) {
        // The subscription and initial snapshot share the dispatch lock, so a
        // connection cannot miss a change between its welcome and subscription.
        let state = self.state.lock().await;
        let receiver = self.snapshots.subscribe();
        (self.published_snapshot(&state), receiver)
    }

    fn validate(&self, request: &CommandRequest) -> Result<(), CommandError> {
        if request.request_id.trim().is_empty() || request.request_id.len() > MAX_REQUEST_ID_BYTES {
            return Err(CommandError::new(
                "invalid_request_id",
                "request_id must contain 1..=128 bytes",
            ));
        }
        if request.protocol_version != PROTOCOL_VERSION {
            return Err(CommandError::new(
                "unsupported_protocol",
                "This host supports protocol version 1",
            ));
        }
        if request.engine_id != self.engine_id {
            return Err(CommandError::new(
                "engine_mismatch",
                "The requested engine is not this host instance",
            ));
        }
        if !request.params.is_empty() {
            return Err(CommandError::new(
                "invalid_params",
                "This version only supports empty command parameters",
            ));
        }
        if !self.command_ids.contains(&request.command) {
            return Err(CommandError::new(
                "unknown_command",
                "The command is not registered",
            ));
        }
        Ok(())
    }

    fn result(
        &self,
        request_id: String,
        revision: u64,
        error: Option<CommandError>,
    ) -> ServerMessage {
        ServerMessage::Result {
            request_id,
            engine_id: self.engine_id.clone(),
            revision: revision.to_string(),
            ok: error.is_none(),
            error,
        }
    }

    /// Dispatch once. A request id correlates responses; it is not an idempotency key.
    pub async fn dispatch(&self, request: CommandRequest) -> ServerMessage {
        let mut state = self.state.lock().await;
        let error = match self.validate(&request) {
            Err(error) => Some(error),
            Ok(()) => match state.revision.checked_add(1) {
                None => Some(CommandError::new(
                    "revision_exhausted",
                    "Host revision limit reached",
                )),
                Some(next) => match state.engine.dispatch(&request.command) {
                    Err(error) => Some(error),
                    Ok(()) => {
                        state.revision = next;
                        // Publish under the same lock to preserve revision order.
                        // A host without subscribers still accepts commands.
                        let _ = self.snapshots.send(self.published_snapshot(&state));
                        None
                    }
                },
            },
        };
        self.result(request.request_id, state.revision, error)
    }

    async fn malformed_result(&self) -> ServerMessage {
        let state = self.state.lock().await;
        self.result(
            String::new(),
            state.revision,
            Some(CommandError::new(
                "invalid_request",
                "Expected a JSON CommandRequest with no unknown fields",
            )),
        )
    }
}

#[derive(Clone)]
struct LocalEndpoint {
    port: u16,
}

impl LocalEndpoint {
    fn new(address: SocketAddr) -> Result<Self, CommandError> {
        if address.ip() != Ipv4Addr::LOCALHOST {
            return Err(CommandError::new(
                "invalid_address",
                "The development host only binds to 127.0.0.1",
            ));
        }
        Ok(Self {
            port: address.port(),
        })
    }

    fn allowed_authority(&self, authority: &str) -> bool {
        authority == format!("127.0.0.1:{}", self.port)
            || authority == format!("localhost:{}", self.port)
            || (self.port == 80 && matches!(authority, "127.0.0.1" | "localhost"))
    }

    fn check(&self, headers: &HeaderMap, mutation: bool) -> bool {
        let Some(authority) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
            return false;
        };
        // Checking Host also prevents DNS rebinding to the loopback listener.
        if !self.allowed_authority(authority) {
            return false;
        }
        if !mutation {
            return true;
        }
        if let Some(origin) = headers.get(header::ORIGIN)
            && origin.to_str().ok() != Some(format!("http://{authority}").as_str())
        {
            return false;
        }
        if let Some(site) = headers.get("sec-fetch-site")
            && !matches!(site.to_str(), Ok("same-origin" | "none"))
        {
            return false;
        }
        // Local scripts do not send browser Origin/Fetch Metadata headers.
        true
    }
}

async fn local_request_guard(
    State(endpoint): State<LocalEndpoint>,
    request: Request,
    next: Next,
) -> Response {
    let mutation = request.uri().path() == "/api/ws" || request.method() != axum::http::Method::GET;
    if !endpoint.check(request.headers(), mutation) {
        return (
            StatusCode::FORBIDDEN,
            "This local host only accepts same-origin browser requests",
        )
            .into_response();
    }
    next.run(request).await
}

/// Build a router for a known local listening address. `serve` resolves port 0 first.
pub fn router(
    engine: impl Engine,
    directory: PathBuf,
    address: SocketAddr,
) -> Result<Router, HostError> {
    let endpoint = LocalEndpoint::new(address)?;
    let host = Arc::new(Host::new(engine)?);
    Ok(Router::new()
        .route("/api/state", get(get_state))
        .route("/api/commands", get(get_commands))
        .route("/api/command", post(post_command))
        .route("/api/ws", get(upgrade_websocket))
        .fallback_service(ServeDir::new(directory))
        .with_state(host)
        .layer(DefaultBodyLimit::max(MAX_MESSAGE_BYTES))
        .layer(middleware::from_fn_with_state(
            endpoint,
            local_request_guard,
        )))
}

/// Serve one engine and its generated WebUI on a loopback-only development endpoint.
pub async fn serve(
    engine: impl Engine,
    directory: PathBuf,
    address: SocketAddr,
) -> Result<(), HostError> {
    LocalEndpoint::new(address)?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    let address = listener.local_addr()?;
    let app = router(engine, directory, address)?;
    println!("Bern development host: http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn get_state(State(host): State<Arc<Host>>) -> Json<Snapshot> {
    Json(host.snapshot().await)
}

async fn get_commands(State(host): State<Arc<Host>>) -> Json<Vec<CommandSpec>> {
    Json(host.commands.clone())
}

async fn post_command(
    State(host): State<Arc<Host>>,
    Json(request): Json<CommandRequest>,
) -> Json<ServerMessage> {
    Json(host.dispatch(request).await)
}

async fn upgrade_websocket(State(host): State<Arc<Host>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| websocket_session(socket, host))
}

async fn send(socket: &mut WebSocket, message: ServerMessage) -> bool {
    match serde_json::to_string(&message) {
        Ok(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        Err(_) => false,
    }
}

async fn websocket_session(mut socket: WebSocket, host: Arc<Host>) {
    let (initial, mut updates) = host.subscribe().await;
    let mut last_revision = initial.revision;
    if !send(
        &mut socket,
        ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            engine_id: host.engine_id.clone(),
            commands: host.commands.clone(),
            snapshot: initial.snapshot,
        },
    )
    .await
    {
        return;
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(incoming)) = incoming else { break; };
                let result = match incoming {
                    Message::Text(text) => match serde_json::from_str::<CommandRequest>(&text) {
                        Ok(request) => host.dispatch(request).await,
                        Err(_) => host.malformed_result().await,
                    },
                    Message::Binary(_) => host.malformed_result().await,
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) => continue,
                };
                if !send(&mut socket, result).await { break; }
            }
            update = updates.recv() => {
                let published = match update {
                    Ok(snapshot) => snapshot,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let state = host.state.lock().await;
                        host.published_snapshot(&state)
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                // Lag recovery can jump ahead of snapshots still in the queue.
                if published.revision <= last_revision { continue; }
                last_revision = published.revision;
                if !send(&mut socket, ServerMessage::Snapshot { snapshot: published.snapshot }).await { break; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request as HttpRequest,
    };
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use tower::ServiceExt;

    #[derive(Default)]
    struct Counter {
        count: i32,
    }
    impl Engine for Counter {
        fn commands(&self) -> Vec<CommandSpec> {
            vec![CommandSpec {
                id: "increment".into(),
                label: "Increment".into(),
            }]
        }
        fn snapshot(&self) -> BTreeMap<String, Value> {
            BTreeMap::from([("count".into(), json!(self.count))])
        }
        fn dispatch(&mut self, _: &str) -> Result<(), CommandError> {
            self.count += 1;
            Ok(())
        }
    }
    fn request(host: &Host) -> CommandRequest {
        CommandRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "request-1".into(),
            engine_id: host.engine_id().into(),
            command: "increment".into(),
            params: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn clients_observe_one_serial_engine_and_request_ids_are_not_deduplication_keys() {
        let host = Arc::new(Host::new(Counter::default()).unwrap());
        let (a_initial, mut a) = host.subscribe().await;
        let (b_initial, mut b) = host.subscribe().await;
        assert_eq!(a_initial.snapshot, b_initial.snapshot);
        let (first, second) =
            tokio::join!(host.dispatch(request(&host)), host.dispatch(request(&host)));
        assert!(matches!(first, ServerMessage::Result { ok: true, .. }));
        assert!(matches!(second, ServerMessage::Result { ok: true, .. }));
        for revision in 1..=2 {
            let from_a = a.recv().await.unwrap();
            let from_b = b.recv().await.unwrap();
            assert_eq!(from_a.snapshot, from_b.snapshot);
            assert_eq!(from_a.revision, revision);
            assert_eq!(from_a.snapshot.values["count"], json!(revision));
        }
        assert_eq!(host.snapshot().await.revision, "2");
    }

    #[tokio::test]
    async fn invalid_requests_never_reach_engine_or_broadcast() {
        let host = Host::new(Counter::default()).unwrap();
        let (initial, mut subscriber) = host.subscribe().await;
        let mut cases = Vec::new();
        let mut bad = request(&host);
        bad.command = "missing".into();
        cases.push((bad, "unknown_command"));
        let mut bad = request(&host);
        bad.protocol_version += 1;
        cases.push((bad, "unsupported_protocol"));
        let mut bad = request(&host);
        bad.engine_id = "old-host".into();
        cases.push((bad, "engine_mismatch"));
        let mut bad = request(&host);
        bad.params.insert("count".into(), json!(20));
        cases.push((bad, "invalid_params"));
        let mut bad = request(&host);
        bad.request_id = "x".repeat(129);
        cases.push((bad, "invalid_request_id"));
        let mut bad = request(&host);
        bad.request_id.clear();
        cases.push((bad, "invalid_request_id"));
        for (request, code) in cases {
            let result = host.dispatch(request).await;
            assert!(
                matches!(result, ServerMessage::Result { ok: false, error: Some(error), .. } if error.code == code)
            );
            assert_eq!(host.snapshot().await, initial.snapshot);
        }
        assert!(matches!(
            subscriber.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn http_and_subscribers_share_one_engine() {
        let app = router(
            Counter::default(),
            PathBuf::from("."),
            "127.0.0.1:3000".parse().unwrap(),
        )
        .unwrap();
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/state")
                    .header("host", "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let initial: Snapshot =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        let command = json!({"protocol_version":1,"request_id":"http-1","engine_id":initial.engine_id,"command":"increment","params":{}});
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/command")
                    .header("host", "127.0.0.1:3000")
                    .header("content-type", "application/json")
                    .body(Body::from(command.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let result: ServerMessage =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert!(
            matches!(result, ServerMessage::Result { ok:true, revision, .. } if revision == "1")
        );
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/state")
                    .header("host", "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let snapshot: Snapshot =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(snapshot.values["count"], json!(1));
        assert_eq!(snapshot.engine_id, initial.engine_id);
    }

    #[tokio::test]
    async fn browser_origin_and_dns_rebinding_requests_are_rejected() {
        let app = router(
            Counter::default(),
            PathBuf::from("."),
            "127.0.0.1:3000".parse().unwrap(),
        )
        .unwrap();
        for (path, method, host, origin, site) in [
            (
                "/api/command",
                "POST",
                "127.0.0.1:3000",
                "https://untrusted.example",
                "cross-site",
            ),
            (
                "/api/ws",
                "GET",
                "127.0.0.1:3000",
                "http://127.0.0.1:4000",
                "same-site",
            ),
            ("/api/state", "GET", "untrusted.example:3000", "", ""),
            ("/api/command", "POST", "127.0.0.1:3000", "", "cross-site"),
        ] {
            let mut request = HttpRequest::builder()
                .method(method)
                .uri(path)
                .header("host", host);
            if !origin.is_empty() {
                request = request.header("origin", origin);
            }
            if !site.is_empty() {
                request = request.header("sec-fetch-site", site);
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let endpoint = LocalEndpoint { port: 3000 };
        let headers = HeaderMap::from_iter([
            (header::HOST, "127.0.0.1:3000".parse().unwrap()),
            (header::ORIGIN, "http://127.0.0.1:3000".parse().unwrap()),
        ]);
        assert!(endpoint.check(&headers, true));
        assert!(LocalEndpoint::new("0.0.0.0:3000".parse().unwrap()).is_err());
    }
}
