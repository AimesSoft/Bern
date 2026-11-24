//! The engine behind the in-process interface, and the application's
//! lifetime.
//!
//! Callbacks arrive on engine and channel threads; their bytes are copied
//! into a channel that a task on GPUI's main thread drains in arrival
//! order. Engine state is mirrored per field and every window renders
//! again after a change, as after a draft write or a switch of language:
//! GPUI rebuilds a window's elements on each render, so no binding has to
//! follow single fields.

use crate::protocol::{Reader, Value, Writer, decode_state_field, encode};
use crate::{AppDef, Ctx, Failure, StartEngine, text, windows};
use bern_ffi::{BernApp, BernBuffer, BernCallbacks};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{App, KeyBinding, Menu, MenuItem, actions};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

/// Completion callback for an asynchronous engine command (`dispatch_with`).
pub type Done = Box<dyn FnOnce(Result<(), Failure>)>;

actions!(
    bern,
    [
        Quit,
        About,
        OpenSettings,
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
    /// The media hub's state: every player.
    Media(Vec<u8>),
    /// A media request routed to the built-in system media executor.
    MediaRequest(Vec<u8>),
    /// Interface state changed: every window renders again.
    Redraw,
    /// The network changed: what it refused may come now.
    PicturesChanged,
    /// A window command of an action.
    Window(WindowOp, &'static str),
    /// Force-close a window instance after an async save completed.
    ForceClose(String),
    /// Files or URLs handed to the application by the system ("Open With").
    OpenUrls(Vec<String>),
    /// Invoke a menu entry from the native `StatusItem`.
    StatusAction(usize),
    /// Reopen and activate the primary window from the native `StatusItem`.
    StatusReopen,
    /// Close callback triggered by `WebPage(close_on:)` click interception.
    WebClose(String),
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

pub(crate) fn window_command_close_id(window_id: String) {
    send(Event::ForceClose(window_id));
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

/// Ask again for the pictures the network refused.
pub(crate) fn pictures_changed() {
    send(Event::PicturesChanged);
}

/// Queue a `StatusItem` menu action to run on GPUI's main thread.
pub(crate) fn send_status_action(id: usize) {
    send(Event::StatusAction(id));
}

/// Queue reopening the primary window from the native `StatusItem`.
#[allow(dead_code)]
pub(crate) fn send_status_reopen() {
    send(Event::StatusReopen);
}

/// Queue firing the `WebPage(close_on:)` action on GPUI's main thread.
pub(crate) fn send_web_close(key: String) {
    send(Event::WebClose(key));
}

pub(crate) struct Host {
    app: Cell<*mut BernApp>,
    pub(crate) def: &'static AppDef,
    engine: RefCell<Vec<Value>>,
    revision: Cell<u64>,
    next: Cell<u64>,
    pending: RefCell<HashMap<u64, Done>>,
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

// Routed to the built-in system media executor when no application media kernel attached.
extern "C" fn on_media_request(_: *mut c_void, bytes: *const u8, len: usize) {
    send(Event::MediaRequest(copy(bytes, len)));
}
extern "C" fn on_media_state(_: *mut c_void, bytes: *const u8, len: usize) {
    send(Event::Media(copy(bytes, len)));
}
extern "C" fn on_query(_: *mut c_void, subscription: u64, bytes: *const u8, len: usize) {
    send(Event::Query(subscription, copy(bytes, len)));
}

impl Host {
    pub(crate) fn app(&self) -> *mut BernApp {
        self.app.get()
    }

    fn receive_state(&self, bytes: &[u8]) -> Vec<usize> {
        let t0 = crate::trace::enabled().then(crate::trace::now);
        let mut reader = Reader::new(bytes);
        let (Ok(revision), Ok(count)) = (reader.u64(), reader.u32()) else {
            return Vec::new();
        };
        if revision < self.revision.get() {
            return Vec::new();
        }
        self.revision.set(revision);
        let state = &self.def.descriptor.state;
        let types = &self.def.descriptor.types;
        let current = self.engine.borrow();
        let mut changes = Vec::new();
        for _ in 0..count {
            let Ok(field) = reader.u32() else {
                return Vec::new();
            };
            let index = field as usize;
            let Some(spec) = state.get(index) else {
                return Vec::new();
            };
            match decode_state_field(current.get(index), spec, types, &mut reader) {
                Ok(value) => changes.push((index, value)),
                Err(_) => return Vec::new(),
            }
        }
        drop(current);
        let mut engine = self.engine.borrow_mut();
        let mut touched = Vec::with_capacity(changes.len());
        for (field, value) in changes {
            engine[field] = value;
            touched.push(field);
        }
        if let Some(t0) = t0 {
            crate::trace::receive_state(revision, bytes.len(), t0, crate::trace::now());
        }
        touched
    }

    fn receive_reply(&self, token: u64, bytes: &[u8]) {
        let t0 = crate::trace::enabled().then(crate::trace::now);
        let mut reader = Reader::new(bytes);
        let outcome = match (reader.u8(), reader.u64()) {
            (Ok(0), Ok(_)) => Ok(()),
            (Ok(1), Ok(_)) => Err(read_failure(&mut reader)),
            _ => Ok(()),
        };
        if let Some(done) = self.pending.borrow_mut().remove(&token) {
            done(outcome);
        } else if let Err(failure) = outcome {
            eprintln!("bern: {}: {}", failure.code, failure.message);
        }
        if let Some(t0) = t0 {
            crate::trace::receive_reply(token, t0, crate::trace::now());
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
pub fn dispatch(ctx: &Ctx, command: usize, args: Vec<Value>) {
    dispatch_with(ctx, command, args, None);
}

/// Dispatch engine command `command` with an optional completion callback.
pub fn dispatch_with(_ctx: &Ctx, command: usize, args: Vec<Value>, done: Option<Done>) {
    let t0 = crate::trace::enabled().then(crate::trace::now);
    let host = host();
    let Some(spec) = host.def.descriptor.commands.get(command) else {
        if let Some(done) = done {
            done(Err(Failure::new("unknown_command", "unknown command")));
        }
        return;
    };
    let mut writer = Writer::new();
    for (value, param) in args.iter().zip(&spec.params) {
        encode(value, &param.ty, &host.def.descriptor.types, &mut writer);
    }
    let token = host.token();
    if let Some(done) = done {
        host.pending.borrow_mut().insert(token, done);
    }
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
    if let Some(t0) = t0 {
        crate::trace::dispatch(token, command, t0, crate::trace::now());
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
    let _ = host.receive_state(&take(unsafe { bern_ffi::bern_app_snapshot(app) }));
    crate::media::attach_kernel();
    // SAFETY: the handle is live.
    crate::media::receive_state(&take(unsafe { bern_ffi::media::bern_media_snapshot(app) }));
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
        pending: RefCell::new(HashMap::new()),
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
    let settings_window = def.windows.iter().find(|w| w.settings);
    if settings_window.is_some() {
        cx.on_action(|_: &OpenSettings, cx| windows::open_settings(cx));
    }
    if !cfg!(target_os = "macos") {
        let name = text::tr(def.name);
        let mut bindings = vec![KeyBinding::new("ctrl-q", Quit, None)];
        let mut items = Vec::new();
        if let Some(settings) = settings_window {
            bindings.push(KeyBinding::new("ctrl-,", OpenSettings, None));
            items.push(MenuItem::action(text::tr(settings.title), OpenSettings));
            items.push(MenuItem::separator());
        }
        items.push(MenuItem::action(text::tr("bern.app.quit"), Quit));
        cx.bind_keys(bindings);
        cx.set_menus(vec![Menu {
            name: name.clone().into(),
            items,
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
    crate::bar::install(cx);
    let mut bindings = vec![
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ];
    if settings_window.is_some() {
        bindings.push(KeyBinding::new("cmd-,", OpenSettings, None));
    }
    cx.bind_keys(bindings);
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
pub(crate) fn set_menus(def: &AppDef, cx: &mut App) {
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
    // The application's `MenuBar` adds to these menus by role, where
    // SwiftUI puts its command groups.
    let group = |role: &str| {
        let items = crate::bar::items(role);
        let empty = items.is_empty();
        (items, empty)
    };
    let mut app = match group("about") {
        (_, true) => vec![MenuItem::action(named("bern.menu.about"), About)],
        (about, false) => about,
    };
    app.push(MenuItem::separator());
    if let Some(settings) = def.windows.iter().find(|w| w.settings) {
        app.push(MenuItem::action(text::tr(settings.title), OpenSettings));
        app.push(MenuItem::separator());
    }
    if let (items, false) = group("app") {
        app.extend(items);
        app.push(MenuItem::separator());
    }
    app.extend([
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
    ]);
    let mut file = Vec::new();
    if let (items, false) = group("file") {
        file.extend(items);
        file.push(MenuItem::separator());
    }
    file.push(MenuItem::action(text::tr("bern.window.close"), CloseWindow));
    // The edit commands are the text fields': enabled while one has the
    // focus, with the shortcuts they bind.
    use gpui::OsAction;
    use gpui_base::input;
    let mut edit = vec![
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
    ];
    if let (items, false) = group("edit") {
        edit.push(MenuItem::separator());
        edit.extend(items);
    }
    // The system adds Enter Full Screen to the View menu itself, after the
    // application's items.
    let mut view = group("view").0;
    if !view.is_empty() {
        view.push(MenuItem::separator());
    }
    let mut windows = vec![
        MenuItem::action(text::tr("bern.window.minimize"), Minimize),
        MenuItem::action(text::tr("bern.menu.zoom"), Zoom),
        MenuItem::separator(),
        MenuItem::action(text::tr("bern.menu.front"), BringAllToFront),
    ];
    if let (items, false) = group("window") {
        windows.push(MenuItem::separator());
        windows.extend(items);
    }
    let mut menus = vec![
        Menu::new(name.clone()).items(app),
        Menu::new(text::tr("bern.menu.file")).items(file),
        Menu::new(text::tr("bern.menu.edit")).items(edit),
        Menu::new(text::tr("bern.menu.view")).items(view),
    ];
    menus.extend(crate::bar::own_menus());
    menus.push(Menu::new(text::tr("bern.window.switcher")).items(windows));
    let help = group("help").0;
    let helps = !help.is_empty();
    if helps {
        menus.push(Menu::new(text::tr("bern.menu.help")).items(help));
    }
    cx.set_menus(menus);
    // The system's own items: the window list and tiling in the Window
    // menu, the search field in the Help menu.
    crate::native::designate_menus(
        &text::tr("bern.window.switcher"),
        [
            text::tr("bern.window.minimize"),
            text::tr("bern.menu.zoom"),
            text::tr("bern.menu.front"),
        ],
        helps.then(|| text::tr("bern.menu.help")).as_deref(),
    );
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

type OpenerEntry = (String, &'static str, Rc<dyn Fn(String)>);

thread_local! {
    /// Active `Drop(opens: true)` receivers, keyed by window id.
    static OPENERS: RefCell<Vec<OpenerEntry>> = const { RefCell::new(Vec::new()) };
    /// Files handed to the application before a matching `Drop(opens: true)` mounted.
    static WAITING_URLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn begin_openers(window_id: &str) {
    OPENERS.with(|openers| openers.borrow_mut().retain(|(w, _, _)| w != window_id));
}

pub(crate) fn register_opener(window_id: &str, kinds: &'static str, deliver: Rc<dyn Fn(String)>) {
    OPENERS.with(|openers| {
        openers
            .borrow_mut()
            .push((window_id.to_string(), kinds, deliver));
    });
}

type OpenUrlHandler = Rc<dyn Fn(String)>;

pub(crate) fn end_openers() {
    let pending: Vec<(String, OpenUrlHandler)> = WAITING_URLS.with(|waiting| {
        let mut waiting = waiting.borrow_mut();
        if waiting.is_empty() {
            return Vec::new();
        }
        let mut matched = Vec::new();
        let mut kept = Vec::new();
        for url in waiting.drain(..) {
            let taker = OPENERS.with(|openers| {
                openers
                    .borrow()
                    .iter()
                    .rev()
                    .find(|(_, kinds, _)| crate::files::accepts(&url, kinds))
                    .map(|(_, _, deliver)| Rc::clone(deliver))
            });
            match taker {
                Some(deliver) => matched.push((url, deliver)),
                None => kept.push(url),
            }
        }
        *waiting = kept;
        matched
    });
    for (url, deliver) in pending {
        deliver(url);
    }
}

fn normalize_open_url(raw: &str) -> String {
    if raw.contains("://") {
        return raw.to_string();
    }
    let path = std::path::Path::new(raw);
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    crate::files::path_to_file_url(&abs)
}

fn open_urls(urls: Vec<String>, cx: &mut App) {
    if urls.is_empty() {
        return;
    }
    windows::reopen_primary(cx);
    let opens = host().def.opens;
    for raw in urls {
        let url = normalize_open_url(&raw);
        let taker = OPENERS.with(|openers| {
            openers
                .borrow()
                .iter()
                .rev()
                .find(|(_, kinds, _)| crate::files::accepts(&url, kinds))
                .map(|(_, _, deliver)| Rc::clone(deliver))
        });
        match taker {
            Some(deliver) => deliver(url),
            None if !opens.is_empty() && crate::files::accepts(&url, opens) => {
                bern_ffi::files::opened(&url);
            }
            None => {
                WAITING_URLS.with(|waiting| waiting.borrow_mut().push(url));
            }
        }
    }
}

pub(crate) fn run(def: &'static AppDef, start: StartEngine) {
    let shared = install(def);
    let (sender, mut events) = unbounded();
    let _ = EVENTS.set(Mutex::new(sender));
    let start = RefCell::new(Some(start));
    let app = gpui::Application::with_platform(platform())
        .with_assets(crate::icons::Assets(def.icons))
        .with_http_client(std::sync::Arc::new(crate::pictures::Patient));
    app.on_open_urls(|urls| send(Event::OpenUrls(urls)));
    app.run(move |cx: &mut App| {
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
                        let touched = host().receive_state(&bytes);
                        windows::changed_fields(cx, &touched);
                        crate::status::sync(cx);
                    }
                    Event::Reply(token, bytes) => host().receive_reply(token, &bytes),
                    Event::SessionRequest(request, bytes) => {
                        crate::session::handle(request, &bytes, cx)
                    }
                    Event::Query(subscription, bytes) => {
                        let t0 = crate::trace::enabled().then(crate::trace::now);
                        if crate::query::receive(subscription, &bytes) {
                            windows::changed(cx);
                        }
                        if let Some(t0) = t0 {
                            crate::trace::receive_query(subscription, t0, crate::trace::now());
                        }
                    }
                    Event::Media(bytes) => {
                        if crate::media::receive_state(&bytes) {
                            windows::changed(cx);
                        }
                    }
                    Event::MediaRequest(bytes) => crate::media_executor::perform(&bytes),
                    Event::Redraw => {
                        windows::changed(cx);
                        crate::status::sync(cx);
                    }
                    Event::PicturesChanged => {
                        crate::pictures::forget_refused(cx);
                        windows::changed(cx);
                    }
                    Event::Window(op, window) => windows::command(cx, op, window),
                    Event::ForceClose(id) => windows::force_close_id(&id, cx),
                    Event::OpenUrls(urls) => open_urls(urls, cx),
                    Event::StatusAction(id) => crate::status::invoke_action(id, cx),
                    Event::StatusReopen => {
                        cx.activate(true);
                        windows::reopen_primary(cx);
                    }
                    Event::WebClose(key) => crate::webpage::fire_close(&key),
                });
            }
        })
        .detach();
        // The application ends with its last window unless a visible `StatusItem`
        // keeps it running in the status area.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() && !crate::status::keeps_running() {
                cx.quit();
            }
        })
        .detach();
        cx.on_app_quit(|_| {
            crate::status::remove();
            stop_engine();
            async {}
        })
        .detach();
        crate::status::init(def, cx);
        if crate::media_executor::active() {
            cx.spawn(async move |cx| {
                loop {
                    let active = cx.update(|_| crate::media_executor::poll());
                    let delay = if active {
                        if cfg!(target_os = "macos") {
                            std::time::Duration::from_millis(100)
                        } else {
                            std::time::Duration::from_millis(16)
                        }
                    } else {
                        std::time::Duration::from_millis(200)
                    };
                    futures_timer::Delay::new(delay).await;
                }
            })
            .detach();
        }
        let at_login =
            def.status_item.is_some() && std::env::args().skip(1).any(|arg| arg == "--at-login");
        if !at_login {
            windows::open_initial(cx);
        }
        let cli_urls: Vec<String> = std::env::args()
            .skip(1)
            .filter(|arg| !arg.starts_with('-'))
            .collect();
        if !cli_urls.is_empty() {
            send(Event::OpenUrls(cli_urls));
        }
    });
    stop_engine();
}

/// Stop the engine once, whichever way the application ends.
fn stop_engine() {
    crate::media_executor::detach();
    let host = host();
    let app = host.app.replace(std::ptr::null_mut());
    if !app.is_null() {
        // SAFETY: the handle was live and is stopped once.
        unsafe { bern_ffi::bern_app_stop(app) };
    }
}
