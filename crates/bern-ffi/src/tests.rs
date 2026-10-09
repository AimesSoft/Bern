//! Drive the C ABI the way a frontend does, with Rust callbacks.

use super::*;
use bern_protocol::{
    CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, OperationKind, ParamSpec,
    ProtocolDescriptor, Scope, StateField, TypeTable,
};
use std::sync::{Condvar, OnceLock};

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

#[derive(Default)]
struct Frontend {
    events: Mutex<Vec<(String, u64, Vec<u8>)>>,
    changed: Condvar,
}

impl Frontend {
    fn wait_for(&self, kind: &str) -> (u64, Vec<u8>) {
        let mut events = self.events.lock().unwrap();
        loop {
            if let Some(position) = events.iter().position(|(k, _, _)| k == kind) {
                let (_, token, bytes) = events.remove(position);
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

fn record(context: *mut c_void, kind: &str, token: u64, bytes: *const u8, len: usize) {
    // SAFETY: the tests pass a pointer to a live `Frontend`.
    let frontend = unsafe { &*(context as *const Frontend) };
    // SAFETY: the library passes valid bytes for the call's duration.
    let bytes = unsafe { slice(bytes, len) }.to_vec();
    frontend
        .events
        .lock()
        .unwrap()
        .push((kind.to_string(), token, bytes));
    frontend.changed.notify_all();
}

extern "C" fn on_state(context: *mut c_void, bytes: *const u8, len: usize) {
    record(context, "state", 0, bytes, len);
}

extern "C" fn on_reply(context: *mut c_void, token: u64, bytes: *const u8, len: usize) {
    record(context, "reply", token, bytes, len);
}

extern "C" fn on_session(context: *mut c_void, request: u64, bytes: *const u8, len: usize) {
    record(context, "session", request, bytes, len);
}

fn spec() -> AppSpec {
    AppSpec {
        bundle_id: "dev.bern.ffi-test",
        ipc: false,
        session: SessionSpec {
            types: TypeTable::default(),
            sessions: vec![SessionType {
                name: "Draft".into(),
                fields: vec![("name".into(), Type::Text, Value::Text("x".into()))],
            }],
            windows: vec![WindowKind {
                id: "main".into(),
                sessions: vec![0],
            }],
        },
    }
}

fn start(frontend: &Frontend, fingerprint: u64) -> Result<*mut BernApp, String> {
    let callbacks = BernCallbacks {
        context: frontend as *const Frontend as *mut c_void,
        on_state,
        on_reply,
        on_session_request: on_session,
    };
    let mut error = BernBuffer::empty();
    // SAFETY: `error` is writable.
    let app = unsafe { start_app(Counter(0), spec(), fingerprint, callbacks, &mut error) };
    if app.is_null() {
        // SAFETY: on failure the library wrote an owned UTF-8 buffer.
        let message = unsafe { slice(error.ptr, error.len) };
        let message = String::from_utf8_lossy(message).into_owned();
        unsafe { bern_buffer_free(error) };
        Err(message)
    } else {
        unsafe { bern_buffer_free(error) };
        Ok(app)
    }
}

fn take(buffer: BernBuffer) -> Vec<u8> {
    // SAFETY: buffers from the library hold `len` initialized bytes.
    let bytes = unsafe { slice(buffer.ptr, buffer.len) }.to_vec();
    unsafe { bern_buffer_free(buffer) };
    bytes
}

#[test]
fn a_stale_frontend_cannot_start_the_engine() {
    let frontend = Frontend::default();
    let error = start(&frontend, 7).unwrap_err();
    assert!(error.contains("rebuild"), "{error}");
}

#[test]
fn commands_drafts_and_session_requests_cross_the_abi() {
    let frontend = Box::new(Frontend::default());
    let app = start(&frontend, descriptor().fingerprint()).unwrap();

    // Engine command with an encoded parameter.
    let mut params = Writer::new();
    params.i64(4);
    // SAFETY: valid buffer.
    unsafe { bern_app_dispatch(app, 0, params.bytes.as_ptr(), params.bytes.len(), 42) };
    let (token, reply) = frontend.wait_for("reply");
    assert_eq!(token, 42);
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.u8().unwrap(), 0);
    assert_eq!(reader.u64().unwrap(), 1);
    assert_eq!(reader.i64().unwrap(), 4);
    let (_, update) = frontend.wait_for("state");
    let mut reader = Reader::new(&update);
    assert_eq!(reader.u64().unwrap(), 1);
    assert_eq!(reader.u32().unwrap(), 1);

    // Malformed parameters are rejected at the boundary.
    // SAFETY: valid buffer.
    unsafe { bern_app_dispatch(app, 0, [1u8].as_ptr(), 1, 43) };
    let (_, reply) = frontend.wait_for("reply");
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.u8().unwrap(), 1);
    reader.u64().unwrap();
    assert_eq!(reader.text().unwrap(), "invalid_params");

    // Window drafts.
    let opened = take(unsafe { bern_window_opened(app, 0) });
    let mut reader = Reader::new(&opened);
    assert_eq!(reader.u8().unwrap(), 0);
    let generation = reader.u64().unwrap();
    assert_eq!(reader.text().unwrap(), "main#1");
    let mut value = Writer::new();
    value.text("Ada");
    // SAFETY: valid buffer.
    let set = take(unsafe {
        bern_session_set(
            app,
            0,
            generation,
            0,
            0,
            value.bytes.as_ptr(),
            value.bytes.len(),
        )
    });
    assert_eq!(set[0], 0);
    unsafe { bern_window_closed(app, 0, generation) };
    // SAFETY: valid buffer.
    let stale = take(unsafe {
        bern_session_set(
            app,
            0,
            generation,
            0,
            0,
            value.bytes.as_ptr(),
            value.bytes.len(),
        )
    });
    assert_eq!(stale[0], 1);

    // An external session command reaches the frontend and gets its answer.
    // SAFETY: the handle stays valid until `bern_app_stop` below.
    let sessions = unsafe { &*app }.sessions.clone();
    let worker = std::thread::spawn(move || {
        sessions.handle(
            None,
            SessionCommand::WindowOpen,
            vec![Value::Text("main".into())],
        )
    });
    let (request, bytes) = frontend.wait_for("session");
    let mut reader = Reader::new(&bytes);
    let index = reader.u32().unwrap();
    assert_eq!(
        SessionCommand::ALL[index as usize],
        SessionCommand::WindowOpen
    );
    assert_eq!(reader.text().unwrap(), "main");
    let mut answer = Writer::new();
    answer.u8(0);
    answer.u32(index);
    answer.text("main#2");
    // SAFETY: valid buffer.
    unsafe { bern_session_complete(app, request, answer.bytes.as_ptr(), answer.bytes.len()) };
    assert_eq!(
        worker.join().unwrap().unwrap(),
        Value::Text("main#2".into())
    );

    unsafe { bern_app_stop(app) };
}
