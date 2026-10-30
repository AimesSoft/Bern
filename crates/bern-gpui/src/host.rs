//! The engine behind the in-process interface, and the application's
//! lifetime.
//!
//! Callbacks arrive on engine and channel threads; their bytes are copied
//! into a channel that a task on GPUI's main thread drains in arrival
//! order. Engine state is mirrored per field and every window renders
//! again after a change, as after a draft write or a switch of language:
//! GPUI rebuilds a window's elements on each render, so no binding has to
//! follow single fields.

use crate::protocol::{Reader, Value, Writer, decode, encode};
use crate::{AppDef, Ctx, Failure, StartEngine, text, windows};
use bern_ffi::{BernApp, BernBuffer, BernCallbacks};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{App, KeyBinding, Menu, MenuItem, actions};
use std::cell::{Cell, OnceCell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

actions!(
    bern,
    [
        Quit,
        About,
        Hide,
        HideOthers,
        ShowAll,
        CloseWindow,
        Minimize,
        Zoom,
        BringAllToFront
    ]
);

/// What the runtime tells the frontend, copied off its thread.
enum Event {
    State(Vec<u8>),
    Reply(u64, Vec<u8>),
    SessionRequest(u64, Vec<u8>),
    /// A query subscription's new state.
    Query(u64, Vec<u8>),
    /// Interface state changed: every window renders again.
    Redraw,
    /// A window command of an action.
    Window(WindowOp, &'static str),
}

/// A window command an action runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowOp {
    /// Open the window, or bring it forward when it is open.
    Open,
    Close,
    ToggleFullscreen,
}

/// Run a window command once the current event is handled.
pub fn window_command(_c: &Ctx, op: WindowOp, window: &'static str) {
    send(Event::Window(op, window));
}

static EVENTS: OnceLock<Mutex<UnboundedSender<Event>>> = OnceLock::new();

fn send(event: Event) {
    if let Some(sender) = EVENTS.get()
        && let Ok(sender) = sender.lock()
    {
        let _ = sender.unbounded_send(event);
    }
}

/// Have every window render again once the current event is handled.
pub(crate) fn redraw() {
    send(Event::Redraw);
}

pub(crate) struct Host {
    app: Cell<*mut BernApp>,
    pub(crate) def: &'static AppDef,
    engine: RefCell<Vec<Value>>,
    revision: Cell<u64>,
    next: Cell<u64>,
}

thread_local! {
    static HOST: OnceCell<Rc<Host>> = const { OnceCell::new() };
}

pub(crate) fn host() -> Rc<Host> {
    HOST.with(|host| Rc::clone(host.get().expect("the Bern host is running")))
}

/// The engine state field `field` as the frontend last received it.
pub fn state(field: usize) -> Value {
    HOST.with(|host| {
        host.get()
            .and_then(|host| host.engine.borrow().get(field).cloned())
            .unwrap_or(Value::Unit)
    })
}

/// Copy and release a buffer returned by the runtime.
pub(crate) fn take(buffer: BernBuffer) -> Vec<u8> {
    let bytes = if buffer.ptr.is_null() || buffer.len == 0 {
        Vec::new()
    } else {
        // SAFETY: the runtime returned `len` readable bytes.
        unsafe { std::slice::from_raw_parts(buffer.ptr, buffer.len) }.to_vec()
    };
    // SAFETY: the buffer came from the runtime and is released once.
    unsafe { bern_ffi::bern_buffer_free(buffer) };
    bytes
}

/// `0` or `1, code, message`.
pub(crate) fn status(bytes: &[u8]) -> Result<Reader<'_>, Failure> {
    let mut reader = Reader::new(bytes);
    match reader.u8() {
        Ok(0) => Ok(reader),
        Ok(_) => Err(read_failure(&mut reader)),
        Err(error) => Err(Failure::new("invalid_reply", error.0)),
    }
}

fn read_failure(reader: &mut Reader<'_>) -> Failure {
    Failure::new(
        reader.text().unwrap_or_else(|_| "ui_failed".into()),
        reader.text().unwrap_or_default(),
    )
}

fn copy(bytes: *const u8, len: usize) -> Vec<u8> {
    if bytes.is_null() || len == 0 {
        Vec::new()
    } else {
        // SAFETY: the runtime passes `len` readable bytes for this call.
        unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec()
    }
}

extern "C" fn on_state(_: *mut c_void, bytes: *const u8, len: usize) {
    send(Event::State(copy(bytes, len)));
}

extern "C" fn on_reply(_: *mut c_void, token: u64, bytes: *const u8, len: usize) {
    send(Event::Reply(token, copy(bytes, len)));
}

extern "C" fn on_session_request(_: *mut c_void, request: u64, bytes: *const u8, len: usize) {
    send(Event::SessionRequest(request, copy(bytes, len)));
}

// Media and queries are not part of this target yet; its capabilities do
// not list them, so the compiler rejects applications that use them.
extern "C" fn on_media_request(_: *mut c_void, _: *const u8, _: usize) {}
extern "C" fn on_media_state(_: *mut c_void, _: *const u8, _: usize) {}
extern "C" fn on_query(_: *mut c_void, subscription: u64, bytes: *const u8, len: usize) {
    send(Event::Query(subscription, copy(bytes, len)));
}

impl Host {
    pub(crate) fn app(&self) -> *mut BernApp {
        self.app.get()
    }

    fn receive_state(&self, bytes: &[u8]) {
        let mut reader = Reader::new(bytes);
        let (Ok(revision), Ok(count)) = (reader.u64(), reader.u32()) else {
            return;
        };
        if revision < self.revision.get() {
            return;
        }
        self.revision.set(revision);
        let state = &self.def.descriptor.state;
        let types = &self.def.descriptor.types;
        let mut changes = Vec::new();
        for _ in 0..count {
            let Ok(field) = reader.u32() else { return };
            let Some(spec) = state.get(field as usize) else {
                return;
            };
            match decode(&spec.ty, types, &mut reader) {
                Ok(value) => changes.push((field as usize, value)),
                Err(_) => return,
            }
        }
        let mut engine = self.engine.borrow_mut();
        for (field, value) in changes {
            engine[field] = value;
        }
    }

    fn receive_reply(&self, _token: u64, bytes: &[u8]) {
        let mut reader = Reader::new(bytes);
        if let (Ok(1), Ok(_)) = (reader.u8(), reader.u64()) {
            let failure = read_failure(&mut reader);
            eprintln!("bern: {}: {}", failure.code, failure.message);
        }
    }

    fn token(&self) -> u64 {
        let token = self.next.get();
        self.next.set(token + 1);
        token
    }
}

/// Dispatch engine command `command` with its arguments; a failure is
/// reported on standard error.
pub fn dispatch(_ctx: &Ctx, command: usize, args: Vec<Value>) {
    let host = host();
    let Some(spec) = host.def.descriptor.commands.get(command) else {
        return;
    };
    let mut writer = Writer::new();
    for (value, param) in args.iter().zip(&spec.params) {
        encode(value, &param.ty, &host.def.descriptor.types, &mut writer);
    }
    let token = host.token();
    // SAFETY: the handle is live while the application runs; the parameter
    // bytes are valid for this call.
    unsafe {
        bern_ffi::bern_app_dispatch(
            host.app(),
            command as u32,
            writer.bytes.as_ptr(),
            writer.bytes.len(),
            token,
        )
    }
}

/// The directory of the declared resources, next to the executable.
pub(crate) fn resources_dir() -> String {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
    else {
        return String::new();
    };
    // Beside the program, or in a macOS app bundle's `Contents/Resources`.
    [dir.join("resources"), dir.join("../Resources/resources")]
        .into_iter()
        .find(|dir| dir.is_dir())
        .map(|dir| dir.display().to_string())
        .unwrap_or_default()
}

/// Whether the program runs from a macOS app bundle, whose icon the
/// system shows.
pub(crate) fn bundled() -> bool {
    std::env::current_exe().is_ok_and(|exe| {
        exe.parent()
            .is_some_and(|dir| dir.ends_with("Contents/MacOS"))
    })
}

fn start_engine(host: &Host, start: StartEngine) -> Result<(), String> {
    let callbacks = BernCallbacks {
        context: std::ptr::null_mut(),
        on_state,
        on_reply,
        on_session_request,
        on_media_request,
        on_media_state,
        on_query,
    };
    let mut error = BernBuffer {
        ptr: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    let app = start(callbacks, &resources_dir(), &mut error);
    if app.is_null() {
        return Err(String::from_utf8_lossy(&take(error)).into_owned());
    }
    host.app.set(app);
    // SAFETY: the handle was just created.
    host.receive_state(&take(unsafe { bern_ffi::bern_app_snapshot(app) }));
    crate::media::attach_kernel();
    Ok(())
}

/// Install the host of `def` on this thread: the interface language, and
/// each engine state field at its type's default.
fn install(def: &'static AppDef) -> Rc<Host> {
    text::install(def.strings, def.languages, def.default_language);
    let engine = def
        .descriptor
        .state
        .iter()
        .map(|field| Value::default_for(&field.ty, &def.descriptor.types).unwrap_or(Value::Unit))
        .collect();
    let shared = Rc::new(Host {
        app: Cell::new(std::ptr::null_mut()),
        def,
        engine: RefCell::new(engine),
        revision: Cell::new(0),
        next: Cell::new(1),
    });
    HOST.with(|cell| {
        let _ = cell.set(Rc::clone(&shared));
    });
    shared
}

/// The menu bar: on macOS the system's standard menus, with their
/// shortcuts; elsewhere the application menu with Quit.
fn install_menus(def: &'static AppDef, cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    if !cfg!(target_os = "macos") {
        let name = text::tr(def.name);
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
        cx.set_menus(vec![Menu {
            name: name.clone().into(),
            items: vec![MenuItem::action(text::tr("bern.app.quit"), Quit)],
            disabled: false,
        }]);
        return;
    }
    let (name, version) = (def.name, def.version);
    cx.on_action(move |_: &About, _| crate::native::about(&text::tr(name), version));
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &CloseWindow, _| {
        crate::native::window_command(crate::native::WindowCommand::Close)
    });
    cx.on_action(|_: &Minimize, _| {
        crate::native::window_command(crate::native::WindowCommand::Minimize)
    });
    cx.on_action(|_: &Zoom, _| crate::native::window_command(crate::native::WindowCommand::Zoom));
    cx.on_action(|_: &BringAllToFront, _| {
        crate::native::window_command(crate::native::WindowCommand::BringAllToFront)
    });
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
    set_menus(def, cx);
}

thread_local! {
    /// The language the menus are in.
    static MENU_LANGUAGE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Set the menus again once the interface language changed.
pub(crate) fn menus_follow_language(cx: &mut App) {
    let language = text::language();
    if MENU_LANGUAGE.with(|current| *current.borrow() != language) {
        set_menus(host().def, cx);
    }
}

/// Set the macOS menus in the current language.
fn set_menus(def: &AppDef, cx: &mut App) {
    MENU_LANGUAGE.with(|current| *current.borrow_mut() = text::language());
    if !cfg!(target_os = "macos") {
        return;
    }
    let name = text::tr(def.name);
    let named = |key: &str| {
        text::fmt(
            key,
            &[("name", crate::protocol::format::Arg::Text(name.clone()))],
        )
    };
    // The edit commands are the text fields': enabled while one has the
    // focus, with the shortcuts they bind.
    use gpui::OsAction;
    use gpui_base::input;
    cx.set_menus(vec![
        Menu::new(name.clone()).items([
            MenuItem::action(named("bern.menu.about"), About),
            MenuItem::separator(),
            MenuItem::os_submenu(
                text::tr("bern.menu.services"),
                gpui::SystemMenuType::Services,
            ),
            MenuItem::separator(),
            MenuItem::action(named("bern.menu.hide"), Hide),
            MenuItem::action(text::tr("bern.menu.hide_others"), HideOthers),
            MenuItem::action(text::tr("bern.menu.show_all"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(named("bern.menu.quit"), Quit),
        ]),
        Menu::new(text::tr("bern.menu.file"))
            .items([MenuItem::action(text::tr("bern.window.close"), CloseWindow)]),
        Menu::new(text::tr("bern.menu.edit")).items([
            MenuItem::os_action(text::tr("bern.menu.undo"), input::Undo, OsAction::Undo),
            MenuItem::os_action(text::tr("bern.menu.redo"), input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(text::tr("bern.menu.cut"), input::Cut, OsAction::Cut),
            MenuItem::os_action(text::tr("bern.menu.copy"), input::Copy, OsAction::Copy),
            MenuItem::os_action(text::tr("bern.menu.paste"), input::Paste, OsAction::Paste),
            MenuItem::os_action(
                text::tr("bern.menu.select_all"),
                input::SelectAll,
                OsAction::SelectAll,
            ),
        ]),
        Menu::new(text::tr("bern.window.switcher")).items([
            MenuItem::action(text::tr("bern.window.minimize"), Minimize),
            MenuItem::action(text::tr("bern.menu.zoom"), Zoom),
            MenuItem::separator(),
            MenuItem::action(text::tr("bern.menu.front"), BringAllToFront),
        ]),
    ]);
}

/// The system GPUI draws on, from the platform crate the application
/// enabled.
fn platform() -> Rc<dyn gpui::Platform> {
    #[cfg(all(target_os = "macos", feature = "macos"))]
    {
        Rc::new(gpui_macos::MacPlatform::new(false))
    }
    #[cfg(all(any(target_os = "linux", target_os = "freebsd"), feature = "linux"))]
    {
        gpui_linux::current_platform(false)
    }
    #[cfg(all(target_os = "windows", feature = "windows"))]
    {
        Rc::new(
            gpui_windows::WindowsPlatform::new(false)
                .expect("failed to initialize Windows platform"),
        )
    }
}

pub(crate) fn run(def: &'static AppDef, start: StartEngine) {
    let shared = install(def);
    let (sender, mut events) = unbounded();
    let _ = EVENTS.set(Mutex::new(sender));
    let start = RefCell::new(Some(start));
    gpui::Application::with_platform(platform())
        .with_assets(crate::icons::Assets(def.icons))
        .with_http_client(std::sync::Arc::new(reqwest_client::ReqwestClient::new()))
        .run(move |cx: &mut App| {
            gpui_base::init(cx);
            install_menus(def, cx);
            crate::native::show_icon(def.app_icon);
            // Buttons' shortcuts, wherever the keyboard is.
            cx.observe_keystrokes(|event, _, cx| {
                crate::controls::keystroke_pressed(&event.keystroke, cx);
            })
            .detach();
            let Some(start) = start.borrow_mut().take() else {
                return;
            };
            if let Err(message) = start_engine(&shared, start) {
                eprintln!("bern: {message}");
                cx.quit();
                return;
            }
            cx.spawn(async move |cx| {
                while let Some(event) = events.next().await {
                    cx.update(|cx| match event {
                        Event::State(bytes) => {
                            host().receive_state(&bytes);
                            windows::changed(cx);
                        }
                        Event::Reply(token, bytes) => host().receive_reply(token, &bytes),
                        Event::SessionRequest(request, bytes) => {
                            crate::session::handle(request, &bytes, cx)
                        }
                        Event::Query(subscription, bytes) => {
                            if crate::query::receive(subscription, &bytes) {
                                windows::changed(cx);
                            }
                        }
                        Event::Redraw => windows::changed(cx),
                        Event::Window(op, window) => windows::command(cx, op, window),
                    });
                }
            })
            .detach();
            // The application ends with its last window, and the engine stops
            // before the process exits.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.on_app_quit(|_| {
                stop_engine();
                async {}
            })
            .detach();
            windows::open_initial(cx);
        });
    stop_engine();
}

/// Stop the engine once, whichever way the application ends.
fn stop_engine() {
    let host = host();
    let app = host.app.replace(std::ptr::null_mut());
    if !app.is_null() {
        // SAFETY: the handle was live and is stopped once.
        unsafe { bern_ffi::bern_app_stop(app) };
    }
}
