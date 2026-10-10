//! Drive the C ABI the way a frontend does, with Rust callbacks.

use super::*;
use bern_command::QueryAnswer;
use bern_protocol::{
    CommandPolicy, CommandSpec, DESCRIPTOR_VERSION, ExternalNames, OperationKind, ParamSpec,
    ProtocolDescriptor, Scope, StateField, TypeTable,
};
use bern_protocol::{Concurrency, Retry};
use bern_runtime::query::ManualClock;
use query::{bern_query_open, bern_query_request};
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
        commands: vec![
            CommandSpec {
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
            },
            CommandSpec {
                id: "counter.peek".into(),
                name: "peek".into(),
                doc: String::new(),
                kind: OperationKind::Query,
                scope: Scope::Engine,
                params: vec![ParamSpec {
                    name: "delay_ms".into(),
                    ty: Type::Int,
                    doc: String::new(),
                }],
                result: Type::Int,
                errors: vec![],
                policy: CommandPolicy {
                    concurrency: Concurrency::Overlapping,
                    retry: Retry::Idempotent,
                    cancellable: true,
                },
                external: ExternalNames::derive("counter", "peek"),
                reads: vec!["value".into()],
            },
        ],
    })
}

struct Counter {
    value: i64,
    clock: ManualClock,
}

impl EngineProtocol for Counter {
    fn descriptor(&self) -> &'static ProtocolDescriptor {
        descriptor()
    }
    fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
        let Value::Int(amount) = invocation.params[0] else {
            unreachable!()
        };
        self.value += amount;
        Ok(Value::Int(self.value))
    }
    /// `peek(delay_ms)` answers the value after `delay_ms` on the clock.
    fn query(&self, invocation: &Invocation, answer: QueryAnswer) {
        let Value::Int(delay) = invocation.params[0] else {
            unreachable!()
        };
        if delay < 0 {
            return answer.send(Err(CommandError::new("bad_delay", "negative delay")));
        }
        let value = self.value;
        self.clock
            .after(delay as u64, move || answer.send(Ok(Value::Int(value))));
    }
    fn snapshot(&self) -> Vec<Value> {
        vec![Value::Int(self.value)]
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

extern "C" fn on_media_request(context: *mut c_void, bytes: *const u8, len: usize) {
    record(context, "media_request", 0, bytes, len);
}

extern "C" fn on_media_state(context: *mut c_void, bytes: *const u8, len: usize) {
    record(context, "media_state", 0, bytes, len);
}

extern "C" fn on_query(context: *mut c_void, subscription: u64, bytes: *const u8, len: usize) {
    record(context, "query", subscription, bytes, len);
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
        players: vec!["preview".into()],
    }
}

fn start(frontend: &Frontend, fingerprint: u64) -> Result<*mut BernApp, String> {
    start_with(frontend, fingerprint, ManualClock::new())
}

fn start_with(
    frontend: &Frontend,
    fingerprint: u64,
    clock: ManualClock,
) -> Result<*mut BernApp, String> {
    let callbacks = BernCallbacks {
        context: frontend as *const Frontend as *mut c_void,
        on_state,
        on_reply,
        on_session_request: on_session,
        on_media_request,
        on_media_state,
        on_query,
    };
    let mut error = BernBuffer::empty();
    let engine = || Counter { value: 0, clock };
    // SAFETY: `error` is writable.
    let app = unsafe { start_app(engine, spec(), fingerprint, callbacks, &mut error) };
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

fn call(
    function: unsafe extern "C" fn(*mut BernApp, *const u8, usize) -> BernBuffer,
    app: *mut BernApp,
    bytes: &[u8],
) -> Vec<u8> {
    // SAFETY: valid buffer and live handle.
    take(unsafe { function(app, bytes.as_ptr(), bytes.len()) })
}

#[test]
fn media_players_cross_the_abi() {
    use bern_media::{ExecOp, ExecRequest, Observation, Phase};
    let frontend = Box::new(Frontend::default());
    let app = start(&frontend, descriptor().fingerprint()).unwrap();

    // Without an executor, players are unavailable and commands refused.
    let mut params = Writer::new();
    params.text("preview");
    // SAFETY: valid buffer.
    unsafe { media::bern_media_dispatch(app, 3, params.bytes.as_ptr(), params.bytes.len(), 9) };
    let (_, reply) = frontend.wait_for("reply");
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.u8().unwrap(), 1);
    reader.u64().unwrap();
    assert_eq!(reader.text().unwrap(), "executor_unavailable");

    // The frontend attaches its executor.
    let mut capabilities = Writer::new();
    capabilities.text("test");
    capabilities.text("1");
    capabilities.u8(1);
    capabilities.u8(1);
    assert_eq!(
        call(
            media::bern_media_executor_attached,
            app,
            &capabilities.bytes
        ),
        vec![0]
    );

    // A declaration creates and binds its own player; the load reaches
    // the executor once even when the binding is reported again.
    let mut create = Writer::new();
    create.text("main#1/page.video");
    create.text("main#1");
    assert_eq!(call(media::bern_media_create, app, &create.bytes), vec![0]);
    let mut bind = Writer::new();
    bind.text("main#1/page.video");
    bind.u8(1);
    bind.text("file:///clip.mp4");
    for _ in 0..2 {
        let bound = call(media::bern_media_bind, app, &bind.bytes);
        let mut reader = Reader::new(&bound);
        assert_eq!(reader.u8().unwrap(), 0);
        assert_eq!(reader.u64().unwrap(), 1);
    }
    let (_, request) = frontend.wait_for("media_request");
    let load = ExecRequest::decode(&request).unwrap();
    assert_eq!(
        load.op,
        ExecOp::Load {
            url: "file:///clip.mp4".into()
        }
    );
    let mut done = Writer::new();
    done.u64(load.request);
    done.u8(0);
    // SAFETY: valid buffer.
    unsafe { media::bern_media_complete(app, done.bytes.as_ptr(), done.bytes.len()) };
    let ready = Observation {
        phase: Phase::Ready,
        duration_ms: Some(4_000),
        seekable: true,
        ..Observation::idle()
    };
    let event = ready.encode("main#1/page.video", 1);
    // SAFETY: valid buffer.
    unsafe { media::bern_media_event(app, event.as_ptr(), event.len()) };
    let state = take(unsafe { media::bern_media_snapshot(app) });
    let mut reader = Reader::new(&state);
    reader.u64().unwrap();
    assert_eq!(reader.u32().unwrap(), 2);
    let (types, _) = bern_media::media_catalog();
    let player = Type::Record {
        name: "MediaPlayer".into(),
    };
    let first = decode(&player, &types, &mut reader).unwrap();
    let Value::Record(fields) = first else {
        panic!()
    };
    assert_eq!(fields[0], Value::Text("main#1/page.video".into()));
    assert_eq!(fields[5], Value::unit_variant(Phase::Ready.index()));
    assert!(decode(&player, &types, &mut reader).is_ok());
    assert!(reader.is_finished());

    // Play from the UI goes through the same command and executor.
    let mut params = Writer::new();
    params.text("main#1/page.video");
    // SAFETY: valid buffer.
    unsafe { media::bern_media_dispatch(app, 3, params.bytes.as_ptr(), params.bytes.len(), 10) };
    let (_, request) = frontend.wait_for("media_request");
    let play = ExecRequest::decode(&request).unwrap();
    assert_eq!(play.op, ExecOp::Play);
    let mut done = Writer::new();
    done.u64(play.request);
    done.u8(0);
    // SAFETY: valid buffer.
    unsafe { media::bern_media_complete(app, done.bytes.as_ptr(), done.bytes.len()) };
    let (token, reply) = frontend.wait_for("reply");
    assert_eq!(token, 10);
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.u8().unwrap(), 0);
    reader.u64().unwrap();
    assert_eq!(reader.i64().unwrap(), 1);

    // Releasing the owner's player asks the executor to release it.
    let mut release = Writer::new();
    release.text("main#1/page.video");
    // SAFETY: valid buffer.
    unsafe { media::bern_media_release(app, release.bytes.as_ptr(), release.bytes.len()) };
    let (_, request) = frontend.wait_for("media_request");
    assert_eq!(ExecRequest::decode(&request).unwrap().op, ExecOp::Release);

    unsafe { bern_app_stop(app) };
}

#[test]
fn close_confirmations_cross_the_abi() {
    let frontend = Box::new(Frontend::default());
    let app = start(&frontend, descriptor().fingerprint()).unwrap();
    let opened = take(unsafe { bern_window_opened(app, 0) });
    let mut reader = Reader::new(&opened);
    assert_eq!(reader.u8().unwrap(), 0);
    let generation = reader.u64().unwrap();
    let request = take(unsafe { bern_window_request_close(app, 0, generation, 1, 1) });
    let mut reader = Reader::new(&request);
    assert_eq!((reader.u8().unwrap(), reader.u8().unwrap()), (0, 1));
    let confirmation = reader.text().unwrap();
    assert_eq!(reader.u64().unwrap(), 0);
    let decide = |decision: u32| {
        let mut bytes = Writer::new();
        bytes.text(&confirmation);
        bytes.u32(decision);
        // SAFETY: valid buffer and live handle.
        take(unsafe {
            bern_window_confirm_close(app, 0, generation, bytes.bytes.as_ptr(), bytes.bytes.len())
        })
    };
    // Save is chosen: the frontend runs the save command next.
    assert_eq!(decide(0), vec![0, 1]);
    // The confirmation was used; deciding again is stale.
    let stale = decide(1);
    let mut reader = Reader::new(&stale);
    assert_eq!(reader.u8().unwrap(), 1);
    assert_eq!(reader.text().unwrap(), "stale_confirmation");
    // Without a failing check, closing proceeds at once.
    assert_eq!(
        take(unsafe { bern_window_request_close(app, 0, generation, 0, 1) }),
        vec![0, 0]
    );
    unsafe { bern_app_stop(app) };
}

/// A decoded query state message.
#[derive(Debug, PartialEq)]
enum Shown {
    Loading,
    Ready(i64),
    Refreshing(i64),
    Failed(Option<i64>, String),
}

fn shown(bytes: &[u8]) -> (u64, Shown) {
    let mut reader = Reader::new(bytes);
    let version = reader.u64().unwrap();
    let shown = match reader.u8().unwrap() {
        1 => Shown::Loading,
        2 => Shown::Ready(reader.i64().unwrap()),
        3 => Shown::Refreshing(reader.i64().unwrap()),
        4 => {
            let previous = (reader.u8().unwrap() == 1).then(|| reader.i64().unwrap());
            let code = reader.text().unwrap();
            assert!(!reader.text().unwrap().is_empty());
            Shown::Failed(previous, code)
        }
        other => panic!("phase {other}"),
    };
    assert!(reader.is_finished());
    (version, shown)
}

fn request(app: *mut BernApp, subscription: u64, delay: i64) -> (u64, Shown) {
    let mut params = Writer::new();
    params.i64(delay);
    // SAFETY: valid buffer.
    let reply = take(unsafe {
        bern_query_request(app, subscription, params.bytes.as_ptr(), params.bytes.len())
    });
    assert_eq!(reply[0], 0, "{reply:?}");
    shown(&reply[1..])
}

fn wait_timers(clock: &ManualClock, count: usize) {
    for _ in 0..500 {
        if clock.pending() >= count {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("the provider did not schedule its answer");
}

#[test]
fn query_subscriptions_show_only_current_answers() {
    let frontend = Box::new(Frontend::default());
    let clock = ManualClock::new();
    let app = start_with(&frontend, descriptor().fingerprint(), clock.clone()).unwrap();
    let opened = take(unsafe { bern_window_opened(app, 0) });
    let mut reader = Reader::new(&opened);
    reader.u8().unwrap();
    let generation = reader.u64().unwrap();

    // Commands are not queries.
    let refused = take(unsafe { bern_query_open(app, 0, generation, 0) });
    assert_eq!(refused[0], 1);
    let opened = take(unsafe { bern_query_open(app, 0, generation, 1) });
    let mut reader = Reader::new(&opened);
    assert_eq!(reader.u8().unwrap(), 0);
    let subscription = reader.u64().unwrap();

    // A slow request, then a fast one while the first provider is still
    // working: the answers arrive out of order.
    assert_eq!(request(app, subscription, 300), (1, Shown::Loading));
    wait_timers(&clock, 1);
    assert_eq!(request(app, subscription, 100), (2, Shown::Loading));
    wait_timers(&clock, 2);
    clock.advance(100);
    let (id, bytes) = frontend.wait_for("query");
    assert_eq!(id, subscription);
    assert_eq!(shown(&bytes), (2, Shown::Ready(0)));
    // The older answer is late: it is dropped and never reaches the view.
    clock.advance(200);
    assert!(
        !frontend
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|(kind, _, _)| kind == "query")
    );

    // The query reads `value`: a command that changes it refreshes the
    // subscription with the same parameters, keeping the data visible.
    let mut params = Writer::new();
    params.i64(5);
    unsafe { bern_app_dispatch(app, 0, params.bytes.as_ptr(), params.bytes.len(), 1) };
    let (_, bytes) = frontend.wait_for("query");
    assert_eq!(shown(&bytes), (3, Shown::Refreshing(0)));
    wait_timers(&clock, 1);
    clock.advance(100);
    let (_, bytes) = frontend.wait_for("query");
    assert_eq!(shown(&bytes), (3, Shown::Ready(5)));

    // A failure keeps the previous data next to the error.
    assert_eq!(request(app, subscription, -1).0, 4);
    let (_, bytes) = frontend.wait_for("query");
    assert_eq!(
        shown(&bytes),
        (4, Shown::Failed(Some(5), "bad_delay".into()))
    );

    // Closing the window ends its subscriptions; the old instance is stale.
    request(app, subscription, 100);
    wait_timers(&clock, 1);
    unsafe { bern_window_closed(app, 0, generation) };
    clock.advance(100);
    let mut params = Writer::new();
    params.i64(1);
    let ended = take(unsafe {
        bern_query_request(app, subscription, params.bytes.as_ptr(), params.bytes.len())
    });
    let mut reader = Reader::new(&ended);
    assert_eq!(reader.u8().unwrap(), 1);
    assert_eq!(reader.text().unwrap(), "target_not_found");
    let stale = take(unsafe { bern_query_open(app, 0, generation, 1) });
    let mut reader = Reader::new(&stale);
    assert_eq!(reader.u8().unwrap(), 1);
    assert_eq!(reader.text().unwrap(), "stale_window");
    assert!(
        !frontend
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|(kind, _, _)| kind == "query")
    );
    unsafe { bern_app_stop(app) };
}
