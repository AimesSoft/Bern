use super::*;
use bern_command::EngineProtocol;
use bern_protocol::{
    CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, OperationKind, ParamSpec,
    ProtocolDescriptor, Scope, StateField, Type, TypeTable, Value,
};
use bern_runtime::SessionHandler;
use serde_json::json;
use std::sync::OnceLock;

fn descriptor() -> &'static ProtocolDescriptor {
    static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
    DESCRIPTOR.get_or_init(|| ProtocolDescriptor {
        descriptor_version: DESCRIPTOR_VERSION,
        namespace: "counter".into(),
        version: 1,
        types: TypeTable::default(),
        state: vec![StateField {
            name: "value".into(),
            ty: Type::Int,
            doc: String::new(),
        }],
        commands: vec![CommandSpec {
            id: "counter.add".into(),
            name: "add".into(),
            doc: String::new(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params: vec![ParamSpec {
                name: "amount".into(),
                ty: Type::Int,
                doc: String::new(),
            }],
            result: Type::Int,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive("counter", "add"),
        }],
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
    fn snapshot(&self) -> Vec<Value> {
        vec![Value::Int(self.0)]
    }
}

struct FakeUi;

impl SessionHandler for FakeUi {
    fn session_id(&self) -> String {
        "ui-1".into()
    }
    fn windows(&self) -> Vec<String> {
        vec!["main#1".into()]
    }
    fn handle(&self, command: SessionCommand, params: Vec<Value>) -> Result<Value, CommandError> {
        match command {
            SessionCommand::WindowOpen => match &params[0] {
                Value::Text(kind) => Ok(Value::Text(format!("{kind}#1"))),
                _ => unreachable!(),
            },
            _ => Err(CommandError::new("target_not_found", "no such control")),
        }
    }
}

fn start(name: &str) -> (Server, String) {
    let bundle = format!("dev.bern.test.{name}.{}", std::process::id());
    let host = Host::start(Counter(0));
    let sessions = Sessions::new();
    sessions.register(Arc::new(FakeUi));
    (Server::start(host, sessions, &bundle).unwrap(), bundle)
}

#[test]
fn commands_state_and_updates_reach_the_shared_engine() {
    let (server, bundle) = start("commands");
    let mode = std::fs::metadata(server.path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let mut client =
        Client::connect(&bundle, None, "cli", Some(descriptor().fingerprint())).unwrap();
    assert_eq!(client.namespace, "counter");
    let engine = client.engine_id.clone();

    let mut watcher = Client::connect(&bundle, Some(&engine), "json", None).unwrap();
    let initial = watcher
        .request(|request_id| ClientMessage::Subscribe { request_id })
        .unwrap();
    assert!(matches!(initial, ServerMessage::State { .. }));

    let reply = client
        .request(|request_id| ClientMessage::Command {
            request_id,
            engine_id: engine.clone(),
            command: "counter.add".into(),
            params: json!({ "amount": 5 }).as_object().unwrap().clone(),
        })
        .unwrap();
    match reply {
        ServerMessage::Result {
            ok,
            result,
            revision,
            ..
        } => {
            assert!(ok);
            assert_eq!(result, Some(json!(5)));
            assert_eq!(revision, "1");
        }
        other => panic!("{other:?}"),
    }
    match watcher.read().unwrap() {
        ServerMessage::Update { revision, changes } => {
            assert_eq!(revision, "1");
            assert_eq!(changes["value"], json!(5));
        }
        other => panic!("{other:?}"),
    }
    let bad = client
        .request(|request_id| ClientMessage::Command {
            request_id,
            engine_id: engine.clone(),
            command: "counter.add".into(),
            params: json!({ "amount": "five" }).as_object().unwrap().clone(),
        })
        .unwrap();
    assert!(matches!(bad, ServerMessage::Result { ok: false, .. }));
}

#[test]
fn session_commands_route_to_the_live_ui() {
    let (_server, bundle) = start("sessions");
    let mut client = Client::connect(&bundle, None, "mcp", None).unwrap();
    let reply = client
        .request(|request_id| ClientMessage::SessionCommand {
            request_id,
            session_id: None,
            command: "window.open".into(),
            params: json!({ "kind": "settings" }).as_object().unwrap().clone(),
        })
        .unwrap();
    assert!(
        matches!(reply, ServerMessage::Result { ok: true, result: Some(ref r), .. } if *r == json!("settings#1"))
    );
    let reply = client
        .request(|request_id| ClientMessage::SessionCommand {
            request_id,
            session_id: Some("other".into()),
            command: "ui.activate".into(),
            params: json!({ "target": "x" }).as_object().unwrap().clone(),
        })
        .unwrap();
    match reply {
        ServerMessage::Result { ok, error, .. } => {
            assert!(!ok);
            assert_eq!(error.unwrap().code, "session_not_found");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn handshake_rejects_stale_catalogs_and_wrong_engines() {
    let (_server, bundle) = start("handshake");
    match Client::connect(&bundle, None, "cli", Some(1)) {
        Err(ConnectError::Rejected(error)) => assert_eq!(error.code, "catalog_mismatch"),
        Err(other) => panic!("{other}"),
        Ok(_) => panic!("stale catalog accepted"),
    }
    let mut client = Client::connect(&bundle, None, "cli", None).unwrap();
    let reply = client
        .request(|request_id| ClientMessage::Command {
            request_id,
            engine_id: "someone-else".into(),
            command: "counter.add".into(),
            params: Default::default(),
        })
        .unwrap();
    assert!(
        matches!(reply, ServerMessage::Error { ref error, .. } if error.code == "engine_mismatch")
    );
    assert!(matches!(
        Client::connect(&bundle, Some("missing"), "cli", None),
        Err(ConnectError::NotFound(_))
    ));
}

#[test]
fn dropping_the_server_removes_the_socket() {
    let (server, bundle) = start("drop");
    let path = server.path().to_path_buf();
    assert!(path.exists());
    drop(server);
    assert!(!path.exists());
    assert!(matches!(
        Client::connect(&bundle, None, "cli", None),
        Err(ConnectError::NoEngine)
    ));
}
