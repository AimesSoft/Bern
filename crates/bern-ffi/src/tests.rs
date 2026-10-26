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
use std::collections::BTreeMap;
use std::sync::{Condvar, OnceLock};
use trace::{bern_trace_mask, bern_trace_now, bern_trace_ui};

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
            content: false,
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
                on: None,
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
                on: None,
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
                presentation: bern_runtime::Presentation::Native,
            }],
        },
        players: vec!["preview".into()],
        resources: vec!["media/clip.mp4".into()],
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
    launch(frontend, fingerprint, None, || Counter { value: 0, clock })
}

/// Start with an engine factory and the host's resource directory.
fn launch<E: EngineProtocol>(
    frontend: &Frontend,
    fingerprint: u64,
    resources: Option<&str>,
    engine: impl FnOnce() -> E,
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
    let (resources, resources_len) =
        resources.map_or((std::ptr::null(), 0), |root| (root.as_ptr(), root.len()));
    // SAFETY: `resources` is null or a live string, `error` is writable.
    let app = unsafe {
        start_app(
            engine,
            spec(),
            fingerprint,
            resources,
            resources_len,
            callbacks,
            &mut error,
        )
    };
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
fn the_host_installs_declared_resources_before_the_engine_starts() {
    use bern_command::resources::Resources;
    let dir = std::env::temp_dir().join(format!("bern-ffi-resources-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(dir.join("media/clip.mp4"), b"clip").unwrap();
    std::fs::write(dir.join("media/extra.mp4"), b"extra").unwrap();
    let root = dir.to_str().unwrap();
    let frontend = Frontend::default();
    let fingerprint = descriptor().fingerprint();
    let counter = || Counter {
        value: 0,
        clock: ManualClock::new(),
    };

    // A host without resources leaves every lookup unavailable.
    let app = launch(&frontend, fingerprint, None, || {
        let error = Resources::host().path("media/clip.mp4").unwrap_err();
        assert_eq!(error.code, "resources_unavailable");
        counter()
    })
    .unwrap();
    // SAFETY: `app` is live and not used afterwards.
    unsafe { bern_app_stop(app) };

    // The engine sees the declared names in the host's directory only.
    let seen = Mutex::new(None);
    let app = launch(&frontend, fingerprint, Some(root), || {
        let resources = Resources::host();
        *seen.lock().unwrap() = Some((
            resources.path("media/clip.mp4"),
            resources.path("media/extra.mp4").unwrap_err().code,
        ));
        counter()
    })
    .unwrap();
    unsafe { bern_app_stop(app) };
    let (clip, extra) = seen.lock().unwrap().take().unwrap();
    assert_eq!(clip.unwrap(), dir.join("media/clip.mp4"));
    assert_eq!(extra, "undeclared_resource");

    // The process keeps one resource directory; the same one may start again.
    let error = launch(&frontend, fingerprint, Some("/elsewhere"), counter).unwrap_err();
    assert!(error.contains("already uses the resources"), "{error}");
    let app = launch(&frontend, fingerprint, Some(root), counter).unwrap();
    unsafe { bern_app_stop(app) };

    // A failing engine constructor reports why.
    let error = launch(&frontend, fingerprint, None, || -> Counter {
        panic!("clip `media/clip.mp4` is missing")
    })
    .unwrap_err();
    assert!(
        error.contains("panicked while starting: clip `media/clip.mp4` is missing"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(&dir);
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

/// Report window facts as the frontend does: `actions` lists the indices
/// of the available `WindowAction`s.
fn report(app: *mut BernApp, generation: u64, source: u8, visibility: u8, actions: &[u32]) -> u64 {
    let bits = actions.iter().fold(0, |bits, action| bits | 1 << action);
    // SAFETY: live handle.
    unsafe {
        bern_window_facts(
            app, 0, generation, source, visibility, 0, 10.0, 40.0, 920.0, 640.0, 1, bits,
        )
    }
}

#[test]
fn window_presentation_comes_from_reported_facts() {
    let frontend = Frontend::default();
    let app = start(&frontend, descriptor().fingerprint()).unwrap();
    let opened = take(unsafe { bern_window_opened(app, 0) });
    let mut reader = Reader::new(&opened);
    assert_eq!(reader.u8().unwrap(), 0);
    let generation = reader.u64().unwrap();
    // SAFETY: the handle stays valid until `bern_app_stop` below.
    let sessions = unsafe { &*app }.sessions.clone();
    let field = |info: &Value, index: usize| match info {
        Value::Record(fields) => fields[index].clone(),
        other => panic!("not a record: {other:?}"),
    };
    let list = |sessions: &Sessions| match sessions.handle(None, SessionCommand::WindowList, vec![])
    {
        Ok(Value::List(infos)) => infos,
        other => panic!("unexpected {other:?}"),
    };

    // Before the frontend reports the window, it is opening and offers
    // nothing; window.list is answered without asking the frontend.
    let infos = list(&sessions);
    assert_eq!(field(&infos[0], 0), Value::Text("main#1".into()));
    assert_eq!(field(&infos[0], 3), Value::unit_variant(0));
    let minimize = |sessions: &Sessions| {
        sessions.handle(
            None,
            SessionCommand::WindowMinimize,
            vec![Value::Text("main#1".into())],
        )
    };
    assert_eq!(minimize(&sessions).unwrap_err().code, "target_unavailable");

    // Facts from the user or system: visible, can be minimized.
    let minimize_bit = WindowAction::Minimize.index();
    let restore_bit = WindowAction::Restore.index();
    assert_eq!(report(app, generation, 1, 0, &[minimize_bit]), 1);
    let infos = list(&sessions);
    assert_eq!(field(&infos[0], 3), Value::unit_variant(1));
    assert_eq!(field(&infos[0], 10), Value::some(Value::unit_variant(1)));

    // The command reaches the frontend, which minimizes the window,
    // reports the new facts and answers with no value; the result is the
    // window as the runtime now records it.
    let worker = {
        let sessions = sessions.clone();
        std::thread::spawn(move || minimize(&sessions))
    };
    let (request, bytes) = frontend.wait_for("session");
    let mut reader = Reader::new(&bytes);
    let index = reader.u32().unwrap();
    assert_eq!(
        SessionCommand::ALL[index as usize],
        SessionCommand::WindowMinimize
    );
    assert_eq!(reader.text().unwrap(), "main#1");
    assert_eq!(report(app, generation, 0, 1, &[restore_bit]), 2);
    let mut answer = Writer::new();
    answer.u8(0);
    answer.u32(index);
    // SAFETY: valid buffer.
    unsafe { bern_session_complete(app, request, answer.bytes.as_ptr(), answer.bytes.len()) };
    let info = worker.join().unwrap().unwrap();
    assert_eq!(field(&info, 4), Value::unit_variant(1));
    assert_eq!(
        field(&info, 8),
        Value::List(vec![Value::unit_variant(restore_bit)])
    );
    assert_eq!(field(&info, 9), Value::Int(2));
    assert_eq!(field(&info, 10), Value::some(Value::unit_variant(0)));

    // A minimized window cannot be minimized again, and facts of a closed
    // instance are dropped.
    assert_eq!(
        minimize(&sessions).unwrap_err().message,
        "`main#1` cannot minimize now; available: restore"
    );
    unsafe { bern_window_closed(app, 0, generation) };
    assert_eq!(report(app, generation, 1, 0, &[minimize_bit]), 0);
    assert!(list(&sessions).is_empty());

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

/// A media kernel that records what reaches it and completes every request.
#[derive(Default)]
struct RecordingKernel {
    seen: Mutex<Vec<String>>,
    hub: Mutex<Option<Arc<dyn bern_media::kernel::KernelHub>>>,
}

impl RecordingKernel {
    fn note(&self, what: String) {
        self.seen.lock().unwrap().push(what);
    }
    fn seen(&self) -> Vec<String> {
        std::mem::take(&mut *self.seen.lock().unwrap())
    }
}

impl bern_media::MediaExecutor for RecordingKernel {
    fn execute(&self, request: bern_media::ExecRequest) {
        self.note(format!("{:?}", request.op));
        let hub = self
            .hub
            .lock()
            .unwrap()
            .clone()
            .expect("attached before requests");
        hub.complete(request.request, Ok(()));
    }
}

impl bern_media::kernel::MediaKernel for RecordingKernel {
    fn capabilities(&self) -> bern_media::MediaCapabilities {
        bern_media::MediaCapabilities {
            backend: "recording".into(),
            backend_version: "1".into(),
            background_playback: true,
            multi_view: false,
        }
    }
    fn surfaces(&self) -> Vec<bern_media::kernel::SurfaceKind> {
        vec![bern_media::kernel::SurfaceKind::MetalLayer]
    }
    fn attached(&self, hub: Arc<dyn bern_media::kernel::KernelHub>) {
        *self.hub.lock().unwrap() = Some(hub);
        self.note("attached".into());
    }
    fn detached(&self) {
        self.note("detached".into());
    }
    fn attach_surface(&self, view: &str, player: &str, surface: bern_media::kernel::VideoSurface) {
        self.note(format!("attach {view} {player} {surface:?}"));
    }
    fn resize_surface(&self, view: &str, size: bern_media::kernel::SurfaceSize) {
        self.note(format!("resize {view} {size:?}"));
    }
    fn detach_surface(&self, view: &str) {
        self.note(format!("detach {view}"));
    }
    fn frame(&self, view: &str, timing: bern_media::kernel::FrameTiming) {
        self.note(format!("frame {view} {timing:?}"));
    }
}

#[test]
fn an_installed_kernel_plays_and_draws_into_the_views_surfaces() {
    use bern_media::kernel::{self, SurfaceKind};
    let frontend = Box::new(Frontend::default());
    let app = start(&frontend, descriptor().fingerprint()).unwrap();
    let offer = |kinds: &[SurfaceKind]| {
        let mut offer = Writer::new();
        offer.u32(kinds.len() as u32);
        for kind in kinds {
            offer.u32(kind.code());
        }
        call(media::bern_media_kernel_attach, app, &offer.bytes)
    };
    // No kernel: the frontend keeps its own media facilities.
    assert_eq!(offer(&[SurfaceKind::MetalLayer]), vec![0]);

    let recording = Arc::new(RecordingKernel::default());
    kernel::install(recording.clone());
    // A frontend whose views give no surface the kernel draws into.
    assert_eq!(offer(&[]), vec![0]);
    assert_eq!(offer(&[SurfaceKind::SwapChainPanel]), vec![0]);
    let answer = offer(&[SurfaceKind::MetalLayer]);
    let mut reader = Reader::new(&answer);
    assert_eq!(reader.u8().unwrap(), 1);
    assert_eq!(reader.u32().unwrap(), SurfaceKind::MetalLayer.code());
    assert_eq!(recording.seen(), ["attached"]);

    // Every entry's commands reach the kernel through the hub.
    let (types, _) = bern_media::media_catalog();
    let mut params = Writer::new();
    params.text("preview");
    encode(
        &Value::Record(vec![Value::Text("file:///clip.mkv".into())]),
        &Type::Record {
            name: bern_media::MEDIA_SOURCE.into(),
        },
        &types,
        &mut params,
    );
    // SAFETY: valid buffer.
    unsafe { media::bern_media_dispatch(app, 2, params.bytes.as_ptr(), params.bytes.len(), 11) };
    let (token, reply) = frontend.wait_for("reply");
    assert_eq!((token, reply[0]), (11, 0));
    assert_eq!(recording.seen(), [r#"Load { url: "file:///clip.mkv" }"#]);

    // A view's surface, its size, its frames and its end.
    let mut surface = Writer::new();
    surface.text("view-1");
    surface.text("preview");
    surface.u32(SurfaceKind::MetalLayer.code());
    surface.u64(0x1234);
    surface.u32(1280);
    surface.u32(720);
    surface.f64(2.0);
    surface.u32(1);
    // SAFETY: valid buffers and a live handle.
    unsafe {
        media::bern_media_surface_attach(app, surface.bytes.as_ptr(), surface.bytes.len());
        let mut resize = Writer::new();
        resize.text("view-1");
        resize.u32(0);
        resize.u32(360);
        resize.f64(f64::NAN);
        media::bern_media_surface_resize(app, resize.bytes.as_ptr(), resize.bytes.len());
        let view = b"view-1";
        media::bern_media_surface_frame(app, view.as_ptr(), view.len(), 1.0, 1.008);
        let mut detach = Writer::new();
        detach.text("view-1");
        media::bern_media_surface_detach(app, detach.bytes.as_ptr(), detach.bytes.len());
        // A Windows panel.
        let mut panel = Writer::new();
        panel.text("view-2");
        panel.text("preview");
        panel.u32(SurfaceKind::SwapChainPanel.code());
        panel.u64(0x5678);
        panel.u32(1920);
        panel.u32(1080);
        panel.f64(1.5);
        panel.u32(0);
        media::bern_media_surface_attach(app, panel.bytes.as_ptr(), panel.bytes.len());
        // A GTK view's frame sink, by its registration.
        let mut sink = Writer::new();
        sink.text("view-3");
        sink.text("preview");
        sink.u32(SurfaceKind::FrameSink.code());
        sink.u64(7);
        sink.u32(1280);
        sink.u32(720);
        sink.f64(1.0);
        sink.u32(2);
        media::bern_media_surface_attach(app, sink.bytes.as_ptr(), sink.bytes.len());
    }
    assert_eq!(
        recording.seen(),
        [
            "attach view-1 preview VideoSurface { handle: MetalLayer(4660), size: SurfaceSize { width: 1280, height: 720, scale: 2.0 }, fit: Cover }",
            // Sizes are at least a pixel, and an unknown scale is 1.
            "resize view-1 SurfaceSize { width: 1, height: 360, scale: 1.0 }",
            "frame view-1 FrameTiming { now: 1.0, target: 1.008 }",
            "detach view-1",
            "attach view-2 preview VideoSurface { handle: SwapChainPanel(22136), size: SurfaceSize { width: 1920, height: 1080, scale: 1.5 }, fit: Contain }",
            "attach view-3 preview VideoSurface { handle: FrameSink(7), size: SurfaceSize { width: 1280, height: 720, scale: 1.0 }, fit: Fill }",
        ]
    );
    kernel::clear();
    unsafe { bern_app_stop(app) };
    assert_eq!(recording.seen(), ["detached"]);
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

/// The `TraceSegment`s of a `TraceInteraction` value, by name.
fn segments(interaction: &Value) -> BTreeMap<String, i64> {
    let Value::Record(fields) = interaction else {
        panic!("not a record")
    };
    let Value::List(segments) = &fields[6] else {
        panic!("no segments")
    };
    segments
        .iter()
        .map(|segment| match segment {
            Value::Record(parts) => match (&parts[0], &parts[1]) {
                (Value::Text(name), Value::Int(us)) => (name.clone(), *us),
                _ => panic!("bad segment"),
            },
            _ => panic!("bad segment"),
        })
        .collect()
}

#[test]
fn trace_commands_answer_from_the_runtime_and_join_the_ui_side() {
    let frontend = Box::new(Frontend::default());
    let app = start(&frontend, descriptor().fingerprint()).unwrap();
    // Revisions other tests' engines will not reach, as the trace matches
    // the UI's records by revision.
    for token in 0..40 {
        let mut params = Writer::new();
        params.i64(1);
        // SAFETY: valid buffer.
        unsafe { bern_app_dispatch(app, 0, params.bytes.as_ptr(), params.bytes.len(), token) };
        frontend.wait_for("reply");
        frontend.wait_for("state");
    }
    // SAFETY: the handle stays valid until `bern_app_stop` below.
    let sessions = unsafe { &*app }.sessions.clone();
    let started = sessions
        .handle(
            None,
            SessionCommand::TraceStart,
            vec![Value::Text(
                "all,control:Counter.plus,quiet,file:none".into(),
            )],
        )
        .unwrap();
    assert_eq!(
        started,
        Value::Record(vec![
            Value::Text("all,control:Counter.plus,file:none,quiet".into()),
            Value::Optional(None),
        ])
    );
    let invalid = sessions
        .handle(
            None,
            SessionCommand::TraceStart,
            vec![Value::Text("fast".into())],
        )
        .unwrap_err();
    assert_eq!(invalid.code, "invalid_rules");
    assert_ne!(
        bern_trace_mask() & trace::UI,
        0,
        "a failed start keeps the trace"
    );

    // The frontend's side of one press of `Counter.plus`.
    let name = "Counter.plus";
    let pressed = bern_trace_now();
    let mut params = Writer::new();
    params.i64(2);
    // SAFETY: valid buffer.
    unsafe { bern_app_dispatch(app, 0, params.bytes.as_ptr(), params.bytes.len(), 9001) };
    let released = bern_trace_now();
    let ui = |kind: u16, key: u64, a: u64, b: u64, t0: u64, t1: u64, name: &str| {
        // SAFETY: `name` is a live string.
        unsafe { bern_trace_ui(kind as u32, key, a, b, t0, t1, name.as_ptr(), name.len()) }
    };
    ui(trace::kind::ACT, 5, 0, 0, pressed, released, name);
    ui(trace::kind::DISPATCH, 9001, 0, 5, pressed, released, "");
    let (_, state) = frontend.wait_for("state");
    let revision = Reader::new(&state).u64().unwrap();
    assert!(revision > 40);
    let applying = bern_trace_now();
    ui(
        trace::kind::APPLY_FIELD,
        revision,
        0,
        8,
        applying,
        bern_trace_now(),
        "",
    );
    ui(
        trace::kind::RECEIVE_STATE,
        revision,
        state.len() as u64,
        0,
        applying,
        bern_trace_now(),
        "",
    );
    frontend.wait_for("reply");
    let replied = bern_trace_now();
    ui(
        trace::kind::RECEIVE_REPLY,
        9001,
        0,
        0,
        replied,
        bern_trace_now(),
        "",
    );
    let tick = bern_trace_now() + 1;
    ui(
        trace::kind::FRAME,
        1,
        16_666_667,
        0,
        tick,
        tick + 1_000_000,
        "",
    );

    let report = sessions
        .handle(
            None,
            SessionCommand::TraceReport,
            vec![
                Value::Optional(Some(Box::new(Value::Int(5)))),
                Value::Optional(None),
            ],
        )
        .unwrap();
    let Value::Record(fields) = &report else {
        panic!("not a report")
    };
    assert_eq!(fields[0], Value::Bool(true), "still running");
    let Value::List(interactions) = &fields[7] else {
        panic!("no interactions")
    };
    let pressed = interactions
        .iter()
        .find(|interaction| match interaction {
            Value::Record(fields) => {
                fields[0] == Value::Optional(Some(Box::new(Value::Text(name.into()))))
            }
            _ => false,
        })
        .expect("the press is reported");
    let segments = segments(pressed);
    for segment in [
        "act", "dispatch", "queue", "exec", "publish", "to_ui", "apply", "to_frame",
    ] {
        assert!(segments.contains_key(segment), "{segment} in {segments:?}");
    }
    let Value::List(fields_sent) = &fields[6] else {
        panic!("no fields")
    };
    assert!(
        fields_sent.iter().any(|field| matches!(field, Value::Record(parts) if parts[0] == Value::Text("value".into()) && parts[2] == Value::Int(8))),
        "the encoded size of `value`: {fields_sent:?}"
    );

    let stopped = sessions
        .handle(
            None,
            SessionCommand::TraceStop,
            vec![Value::Optional(Some(Box::new(Value::Bool(true))))],
        )
        .unwrap();
    assert_eq!(bern_trace_mask(), 0);
    let Value::Record(stopped) = stopped else {
        panic!("not a stop result")
    };
    assert_eq!(stopped[0], Value::List(vec![]), "file:none writes no file");
    // No frontend saw any of the trace commands.
    assert!(
        frontend
            .events
            .lock()
            .unwrap()
            .iter()
            .all(|(kind, _, _)| kind != "session")
    );
    unsafe { bern_app_stop(app) };
}
