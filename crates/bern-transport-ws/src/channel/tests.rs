//! Drive a page connection with JSON messages, as a page does.

use super::*;
use bern_command::{EngineProtocol, QueryAnswer};
use bern_protocol::{
    CommandPolicy, CommandSpec, Concurrency, DESCRIPTOR_VERSION, ExternalNames, OperationKind,
    ParamSpec, ProtocolDescriptor, Retry, Scope, StateField, Type, TypeTable,
};
use serde_json::{Value as Json, json};
use std::sync::OnceLock;

fn descriptor() -> &'static ProtocolDescriptor {
    static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
    DESCRIPTOR.get_or_init(|| {
        let spec = |name: &str, kind: OperationKind, params: Vec<ParamSpec>, policy| CommandSpec {
            id: format!("counter.{name}"),
            name: name.into(),
            doc: String::new(),
            kind,
            scope: Scope::Engine,
            params,
            result: Type::Int,
            errors: vec![],
            policy,
            external: ExternalNames::derive("counter", name),
            reads: vec![],
            on: None,
        };
        let delay = || {
            vec![ParamSpec {
                name: "amount".into(),
                ty: Type::Int,
                doc: String::new(),
            }]
        };
        ProtocolDescriptor {
            modules: Vec::new(),
            events: Vec::new(),
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "counter".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![StateField {
                name: "value".into(),
                ty: Type::Int,
                doc: String::new(),
                content: false,
            }],
            commands: vec![
                spec(
                    "add",
                    OperationKind::Command,
                    delay(),
                    CommandPolicy::default(),
                ),
                spec(
                    "slow",
                    OperationKind::Query,
                    delay(),
                    CommandPolicy {
                        concurrency: Concurrency::Overlapping,
                        retry: Retry::Idempotent,
                        cancellable: true,
                    },
                ),
            ],
        }
    })
}

struct Counter(i64);

impl EngineProtocol for Counter {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        descriptor()
    }
    fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
        let Value::Int(amount) = invocation.params[0] else {
            unreachable!()
        };
        self.0 += amount;
        Ok(Value::Int(self.0))
    }
    /// `slow(ms)` answers the value after `ms` milliseconds.
    fn query(&self, invocation: &Invocation, answer: QueryAnswer) {
        let Value::Int(delay) = invocation.params[0] else {
            unreachable!()
        };
        let value = self.0;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(delay as u64));
            answer.send(Ok(Value::Int(value)));
        });
    }
    fn snapshot(&self) -> Vec<Value> {
        vec![Value::Int(self.0)]
    }
}

/// A page: what the connection sent it, as JSON.
#[derive(Default)]
struct Page {
    received: Mutex<Vec<Json>>,
}

impl Page {
    fn wait(&self, matches: impl Fn(&Json) -> bool) -> Json {
        for _ in 0..200 {
            if let Some(found) = self
                .received
                .lock()
                .unwrap()
                .iter()
                .find(|m| matches(m))
                .cloned()
            {
                return found;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no matching message in {:?}", self.received.lock().unwrap());
    }

    fn position(&self, matches: impl Fn(&Json) -> bool) -> Option<usize> {
        self.received.lock().unwrap().iter().position(matches)
    }
}

fn connect() -> (Arc<Env>, Arc<Connection>, Arc<Page>) {
    let host = Host::start(Counter(0));
    let env = Env::new(
        host,
        Sessions::new(),
        MediaHub::new(&["preview".into()]),
        "secret".into(),
    );
    let page = Arc::new(Page::default());
    let sink = Arc::clone(&page);
    let out: Outbound = Arc::new(move |message| {
        sink.received
            .lock()
            .unwrap()
            .push(serde_json::to_value(message).unwrap());
    });
    let connection = Connection::new(Arc::clone(&env), out);
    (env, connection, page)
}

fn send(connection: &Arc<Connection>, message: Json) -> bool {
    connection.receive(&message.to_string())
}

fn hello(token: &str) -> Json {
    json!({
        "type": "hello",
        "protocol_version": PROTOCOL_VERSION,
        "entry": "webui",
        "catalog_fingerprint": descriptor().fingerprint().to_string(),
        "token": token,
    })
}

#[test]
fn the_handshake_needs_the_token_version_and_catalog() {
    let (_, connection, page) = connect();
    assert!(!send(
        &connection,
        json!({ "type": "state", "request_id": "1" })
    ));
    assert_eq!(
        page.received.lock().unwrap()[0]["error"]["code"],
        "handshake_required"
    );

    let (_, connection, page) = connect();
    assert!(!send(&connection, hello("wrong")));
    assert_eq!(
        page.received.lock().unwrap()[0]["error"]["code"],
        "unauthorized"
    );

    let (_, connection, page) = connect();
    let mut stale = hello("secret");
    stale["catalog_fingerprint"] = json!("1");
    assert!(!send(&connection, stale));
    assert_eq!(
        page.received.lock().unwrap()[0]["error"]["code"],
        "catalog_mismatch"
    );

    let (env, connection, page) = connect();
    assert!(send(&connection, hello("secret")));
    let welcome = page.wait(|m| m["type"] == "welcome");
    assert_eq!(welcome["engine_id"], env.host.engine_id());
}

#[test]
fn answers_arrive_without_waiting_for_slower_queries() {
    let (env, connection, page) = connect();
    send(&connection, hello("secret"));
    let engine = env.host.engine_id().to_string();
    send(
        &connection,
        json!({
            "type": "command", "request_id": "slow", "engine_id": engine,
            "command": "counter.slow", "params": { "amount": 300 }
        }),
    );
    send(
        &connection,
        json!({
            "type": "command", "request_id": "fast", "engine_id": engine,
            "command": "counter.add", "params": { "amount": 5 }
        }),
    );
    let fast = page.wait(|m| m["request_id"] == "fast" && m["type"] == "result");
    assert_eq!(fast["result"], 5);
    let slow = page.wait(|m| m["request_id"] == "slow" && m["type"] == "result");
    assert_eq!(slow["result"], 0);
    let at = |id: &str| {
        page.position(|m| m["request_id"] == id && m["type"] == "result")
            .unwrap()
    };
    assert!(
        at("fast") < at("slow"),
        "the command answered before the slow query"
    );

    // A cancelled query reports `cancelled` instead of a result.
    send(
        &connection,
        json!({
            "type": "command", "request_id": "gone", "engine_id": engine,
            "command": "counter.slow", "params": { "amount": 200 }
        }),
    );
    send(
        &connection,
        json!({ "type": "cancel", "request_id": "gone" }),
    );
    let gone = page.wait(|m| m["request_id"] == "gone" && m["type"] == "result");
    assert_eq!(gone["error"]["code"], "cancelled");
}

#[test]
fn a_page_is_a_ui_session_external_entries_reach() {
    let (env, connection, page) = connect();
    send(&connection, hello("secret"));
    send(
        &connection,
        json!({ "type": "open_session", "request_id": "o" }),
    );
    let opened = page.wait(|m| m["type"] == "session_opened");
    let session_id = opened["session_id"].as_str().unwrap().to_string();
    send(
        &connection,
        json!({ "type": "session_windows", "windows": ["main#1"] }),
    );
    let listed = env.sessions.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].windows(), vec!["main#1".to_string()]);

    // An external entry asks; the page answers through its controls.
    let sessions = env.sessions.clone();
    let id = session_id.clone();
    let worker =
        std::thread::spawn(move || sessions.handle(Some(&id), SessionCommand::Inspect, vec![]));
    let request = page.wait(|m| m["type"] == "session_request");
    assert_eq!(request["command"], "ui.inspect");
    send(
        &connection,
        json!({
            "type": "session_result",
            "request_id": request["request_id"],
            "ok": true,
            "result": [{ "target": "page.ok", "window": "main#1", "role": "button",
                         "label": "OK", "enabled": true, "value": null, "reason": null }]
        }),
    );
    let Value::List(controls) = worker.join().unwrap().unwrap() else {
        panic!("a list of controls")
    };
    assert_eq!(controls.len(), 1);

    // When the page goes away its session ends.
    connection.closed();
    assert!(env.sessions.list().is_empty());
}

#[test]
fn players_run_on_the_page_that_holds_them() {
    let (env, connection, page) = connect();
    send(&connection, hello("secret"));
    send(
        &connection,
        json!({ "type": "open_session", "request_id": "o" }),
    );
    send(
        &connection,
        json!({
            "type": "media_attach", "backend": "html-media-element", "backend_version": "1",
            "background_playback": true, "multi_view": false
        }),
    );
    send(
        &connection,
        json!({ "type": "media_create", "player": "s/main#1/page.video", "owner": "s/main#1" }),
    );
    send(
        &connection,
        json!({ "type": "media_bind", "player": "s/main#1/page.video", "url": "resources/clip.mp4" }),
    );
    let load = page.wait(|m| m["type"] == "media_request" && m["op"] == "load");
    assert_eq!(load["player"], "s/main#1/page.video");
    assert_eq!(load["url"], "resources/clip.mp4");
    send(
        &connection,
        json!({ "type": "media_complete", "request": load["request"], "ok": true }),
    );
    send(
        &connection,
        json!({
            "type": "media_event", "player": "s/main#1/page.video", "source_version": load["source_version"],
            "observation": { "phase": "Ready", "position_ms": 0, "duration_ms": 8000,
                             "seekable": true, "volume": 1.0, "muted": false, "error": null }
        }),
    );
    let merged = env.media.player("s/main#1/page.video").unwrap();
    let Value::Record(fields) = merged else {
        panic!("a record")
    };
    assert_eq!(
        fields[5],
        Value::unit_variant(bern_media::Phase::Ready.index())
    );

    // A play command from any entry is routed to this page.
    let engine = env.host.engine_id().to_string();
    send(
        &connection,
        json!({
            "type": "command", "request_id": "play", "engine_id": engine,
            "command": "media.play", "params": { "player_id": "s/main#1/page.video" }
        }),
    );
    let play = page.wait(|m| m["type"] == "media_request" && m["op"] == "play");
    send(
        &connection,
        json!({ "type": "media_complete", "request": play["request"], "ok": true }),
    );
    let result = page.wait(|m| m["request_id"] == "play" && m["type"] == "result");
    assert_eq!(result["ok"], true);

    // Closing the window releases its players.
    send(
        &connection,
        json!({ "type": "media_release_window", "window": "s/main#1" }),
    );
    assert!(env.media.player("s/main#1/page.video").is_none());
}
