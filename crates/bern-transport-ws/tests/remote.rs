//! A WebUI page's runtime (`bern_ffi::remote`) against the engine host's
//! connection, wired back to back in memory and driven through the same C
//! ABI the page calls.

use bern_command::{EngineProtocol, Host};
use bern_ffi::{AppSpec, BernApp, BernBuffer, BernCallbacks, SessionType, WindowKind};
use bern_media::MediaHub;
use bern_protocol::{
    CommandError, CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, Invocation,
    OperationKind, ParamSpec, ProtocolDescriptor, Reader, Scope, SessionCommand, StateField, Type,
    TypeTable, Value, Writer,
};
use bern_runtime::Sessions;
use bern_transport_ws::{Connection, Env, Outbound};
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

fn descriptor() -> &'static ProtocolDescriptor {
    static DESCRIPTOR: OnceLock<ProtocolDescriptor> = OnceLock::new();
    DESCRIPTOR.get_or_init(|| ProtocolDescriptor {
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
            reads: vec![],
            on: None,
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

/// What the page's runtime told the frontend.
#[derive(Default)]
struct Frontend {
    events: Mutex<Vec<(String, u64, Vec<u8>)>>,
    changed: Condvar,
}

impl Frontend {
    fn wait(&self, kind: &str) -> (u64, Vec<u8>) {
        let mut events = self.events.lock().unwrap();
        loop {
            if let Some(at) = events.iter().position(|(k, _, _)| k == kind) {
                let (_, token, bytes) = events.remove(at);
                return (token, bytes);
            }
            let (next, timeout) = self
                .changed
                .wait_timeout(events, Duration::from_secs(5))
                .unwrap();
            assert!(!timeout.timed_out(), "no `{kind}` event");
            events = next;
        }
    }
}

fn frontend() -> &'static Frontend {
    static FRONTEND: OnceLock<Frontend> = OnceLock::new();
    FRONTEND.get_or_init(Frontend::default)
}

fn record(kind: &str, token: u64, bytes: *const u8, len: usize) {
    // SAFETY: the runtime passes valid bytes for the call's duration.
    let bytes = unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec();
    frontend()
        .events
        .lock()
        .unwrap()
        .push((kind.into(), token, bytes));
    frontend().changed.notify_all();
}

extern "C" fn on_state(_: *mut c_void, bytes: *const u8, len: usize) {
    record("state", 0, bytes, len);
}
extern "C" fn on_reply(_: *mut c_void, token: u64, bytes: *const u8, len: usize) {
    record("reply", token, bytes, len);
}
extern "C" fn on_session(_: *mut c_void, request: u64, bytes: *const u8, len: usize) {
    record("session", request, bytes, len);
}
extern "C" fn on_media_request(_: *mut c_void, bytes: *const u8, len: usize) {
    record("media_request", 0, bytes, len);
}
extern "C" fn on_media_state(_: *mut c_void, bytes: *const u8, len: usize) {
    record("media_state", 0, bytes, len);
}
extern "C" fn on_query(_: *mut c_void, subscription: u64, bytes: *const u8, len: usize) {
    record("query", subscription, bytes, len);
}

fn take(buffer: BernBuffer) -> Vec<u8> {
    // SAFETY: buffers from the runtime hold `len` initialized bytes.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.ptr, buffer.len) }.to_vec();
    unsafe { bern_ffi::bern_buffer_free(buffer) };
    bytes
}

struct AppPtr(*mut BernApp);
// SAFETY: the runtime's entry points may be called from any thread.
unsafe impl Send for AppPtr {}
unsafe impl Sync for AppPtr {}

#[test]
fn a_page_runs_against_a_separate_engine_host() {
    let host = Host::start(Counter(40));
    let env = Env::new(
        host.clone(),
        Sessions::new(),
        MediaHub::new(&[]),
        "secret".into(),
    );
    let page: Arc<OnceLock<AppPtr>> = Arc::new(OnceLock::new());
    let statuses: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();

    // Host → page.
    let to_page = Arc::clone(&page);
    let out: Outbound = Arc::new(move |message| {
        let text = serde_json::to_string(message).unwrap();
        let app = to_page.get().expect("page started").0;
        // SAFETY: live handle, valid bytes.
        unsafe { bern_ffi::remote::bern_remote_receive(app, text.as_ptr(), text.len()) };
    });
    let connection = Connection::new(Arc::clone(&env), out);

    // Page → host.
    let to_host = Arc::clone(&connection);
    let seen = Arc::clone(&statuses);
    let spec = AppSpec {
        bundle_id: "dev.bern.webui-test",
        ipc: false,
        session: bern_runtime::SessionSpec {
            types: TypeTable::default(),
            sessions: vec![SessionType {
                name: "Draft".into(),
                fields: vec![("name".into(), Type::Text, Value::Text(String::new()))],
            }],
            windows: vec![WindowKind {
                id: "main".into(),
                sessions: vec![0],
                presentation: bern_runtime::Presentation::Virtual,
            }],
        },
        players: vec![],
        resources: vec![],
    };
    let callbacks = BernCallbacks {
        context: std::ptr::null_mut(),
        on_state,
        on_reply,
        on_session_request: on_session,
        on_media_request,
        on_media_state,
        on_query,
    };
    let app = bern_ffi::remote::start(
        spec,
        descriptor(),
        Some("secret".into()),
        callbacks,
        Box::new(move |text| {
            to_host.receive(text);
        }),
        Box::new(move |status| {
            seen.lock()
                .unwrap()
                .push(serde_json::from_str(status).unwrap());
        }),
    );
    let _ = page.set(AppPtr(app));

    // Opening the channel runs the handshake, registers the session and
    // applies the engine's full state.
    unsafe { bern_ffi::remote::bern_remote_open(app) };
    let connected = statuses.lock().unwrap().last().cloned().unwrap();
    assert_eq!(connected["state"], "connected");
    assert_eq!(connected["engine_id"], host.engine_id());
    let session_id = connected["session_id"].as_str().unwrap().to_string();
    let (_, state) = frontend().wait("state");
    let mut reader = Reader::new(&state);
    assert_eq!(reader.u64().unwrap(), 0);
    assert_eq!(reader.u32().unwrap(), 1);
    assert_eq!(reader.u32().unwrap(), 0);
    assert_eq!(reader.i64().unwrap(), 40);

    // A command from the page runs on the engine host; the state update
    // comes back through the subscription.
    let mut params = Writer::new();
    params.i64(2);
    unsafe { bern_ffi::bern_app_dispatch(app, 0, params.bytes.as_ptr(), params.bytes.len(), 7) };
    let (token, reply) = frontend().wait("reply");
    assert_eq!(token, 7);
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.u8().unwrap(), 0);
    reader.u64().unwrap();
    assert_eq!(reader.i64().unwrap(), 42);
    let (_, update) = frontend().wait("state");
    let mut reader = Reader::new(&update);
    assert_eq!(reader.u64().unwrap(), 1);
    assert_eq!(host.snapshot().1, vec![Value::Int(42)]);

    // The page's windows are listed for external entries, and their
    // session commands are answered by the page's own session runtime.
    let opened = take(unsafe { bern_ffi::bern_window_opened(app, 0) });
    assert_eq!(opened[0], 0);
    let listed = env.sessions.list();
    assert_eq!(listed[0].session_id(), session_id);
    assert_eq!(listed[0].windows(), vec!["main#1".to_string()]);
    let answer = env
        .sessions
        .handle(Some(&session_id), SessionCommand::WindowList, vec![])
        .unwrap();
    let Value::List(windows) = answer else {
        panic!("a list of windows")
    };
    assert_eq!(windows.len(), 1);

    // A lost connection fails what was waiting with an unknown outcome and
    // ends the session on the engine host.
    let reason = "gone";
    unsafe { bern_ffi::remote::bern_remote_closed(app, reason.as_ptr(), reason.len()) };
    connection.closed();
    assert_eq!(
        statuses.lock().unwrap().last().unwrap()["state"],
        "disconnected"
    );
    assert!(env.sessions.list().is_empty());
    let mut params = Writer::new();
    params.i64(1);
    unsafe { bern_ffi::bern_app_dispatch(app, 0, params.bytes.as_ptr(), params.bytes.len(), 8) };
    let (_, reply) = frontend().wait("reply");
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.u8().unwrap(), 1);
    reader.u64().unwrap();
    assert_eq!(reader.text().unwrap(), "engine_unavailable");
    assert_eq!(
        host.snapshot().1,
        vec![Value::Int(42)],
        "nothing was sent again"
    );
    unsafe { bern_ffi::bern_app_stop(app) };
}
