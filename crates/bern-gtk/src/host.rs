//! The engine behind the in-process interface, and the application's
//! lifetime.
//!
//! Callbacks arrive on engine and channel threads; their bytes are copied
//! and handled on the GTK main thread in arrival order. Engine state is
//! mirrored into one cell per field, updated per revision in one batch.

use crate::protocol::{Reader, Value, Writer, decode, encode};
use crate::reactive::{Cell, batch};
use crate::windows::Win;
use crate::{
    AppDef, BytesFn, Ctx, Done, Failure, StartEngine, media, session, snapshot, text, windows,
};
use adw::prelude::*;
use bern_ffi::{BernApp, BernBuffer, BernCallbacks};
use std::cell::{Cell as StdCell, OnceCell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::{Rc, Weak};
use std::sync::Mutex;

struct Reply {
    window: Weak<Win>,
    done: Option<Done>,
}

pub(crate) struct Host {
    app: StdCell<*mut BernApp>,
    pub(crate) def: &'static AppDef,
    engine: Vec<Cell<Value>>,
    revision: StdCell<u64>,
    replies: RefCell<HashMap<u64, Reply>>,
    next: StdCell<u64>,
    queries: RefCell<HashMap<u64, BytesFn>>,
    pub(crate) engine_id: RefCell<String>,
    pub(crate) application: OnceCell<adw::Application>,
}

thread_local! {
    static HOST: OnceCell<Rc<Host>> = const { OnceCell::new() };
}

pub(crate) fn host() -> Rc<Host> {
    HOST.with(|host| Rc::clone(host.get().expect("the Bern host is running")))
}

pub(crate) fn engine_value(field: usize) -> Value {
    HOST.with(|host| {
        host.get()
            .and_then(|host| host.engine.get(field).map(Cell::get))
            .unwrap_or(Value::Unit)
    })
}

/// Bytes the frontend passes in: a pointer and a length for one call.
pub(crate) fn with_bytes<R>(bytes: &[u8], body: impl FnOnce(*const u8, usize) -> R) -> R {
    body(bytes.as_ptr(), bytes.len())
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

pub(crate) fn read_failure(reader: &mut Reader<'_>) -> Failure {
    Failure::new(
        reader.text().unwrap_or_else(|_| "ui_failed".into()),
        reader.text().unwrap_or_default(),
    )
}

/// Run `work` on the GTK main thread after the current call returns.
fn deliver(work: impl FnOnce() + Send + 'static) {
    let work = Mutex::new(Some(work));
    glib::idle_add_full(glib::Priority::DEFAULT, move || {
        if let Some(work) = work.lock().ok().and_then(|mut work| work.take()) {
            // A failing callback must not take the application down with it.
            if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|text| text.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                eprintln!("bern: a runtime callback failed: {message}");
            }
        }
        glib::ControlFlow::Break
    });
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
    let bytes = copy(bytes, len);
    deliver(move || host().receive_state(&bytes));
}

extern "C" fn on_reply(_: *mut c_void, token: u64, bytes: *const u8, len: usize) {
    let bytes = copy(bytes, len);
    deliver(move || host().receive_reply(token, &bytes));
}

extern "C" fn on_session_request(_: *mut c_void, request: u64, bytes: *const u8, len: usize) {
    let bytes = copy(bytes, len);
    deliver(move || session::handle(request, &bytes));
}

extern "C" fn on_media_request(_: *mut c_void, bytes: *const u8, len: usize) {
    let bytes = copy(bytes, len);
    deliver(move || media::request(&bytes));
}

extern "C" fn on_media_state(_: *mut c_void, bytes: *const u8, len: usize) {
    let bytes = copy(bytes, len);
    deliver(move || media::receive_state(&bytes));
}

extern "C" fn on_query(_: *mut c_void, subscription: u64, bytes: *const u8, len: usize) {
    let bytes = copy(bytes, len);
    deliver(move || {
        let receive = host().queries.borrow().get(&subscription).cloned();
        if let Some(receive) = receive {
            receive(&bytes);
        }
    });
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
        batch(|| {
            for (field, value) in changes {
                self.engine[field].set(value);
            }
        });
        snapshot::schedule();
    }

    fn receive_reply(&self, token: u64, bytes: &[u8]) {
        let Some(reply) = self.replies.borrow_mut().remove(&token) else {
            return;
        };
        let mut reader = Reader::new(bytes);
        let result = match (reader.u8(), reader.u64()) {
            (Ok(0), Ok(_)) => Ok(()),
            (Ok(_), Ok(_)) => Err(read_failure(&mut reader)),
            _ => Err(Failure::new("invalid_reply", "malformed reply")),
        };
        if let Err(failure) = &result
            && let Some(window) = reply.window.upgrade()
        {
            window.fail(failure);
        }
        if let Some(done) = reply.done {
            done(result);
        }
        snapshot::schedule();
    }

    /// A reply token for a command whose failure shows in `window`.
    pub(crate) fn token(&self, window: Option<&Rc<Win>>, done: Option<Done>) -> u64 {
        let token = self.next.get();
        self.next.set(token + 1);
        self.replies.borrow_mut().insert(
            token,
            Reply {
                window: window.map(Rc::downgrade).unwrap_or_default(),
                done,
            },
        );
        token
    }

    pub(crate) fn open_query(&self, subscription: u64, receive: BytesFn) {
        self.queries.borrow_mut().insert(subscription, receive);
    }

    pub(crate) fn close_query(&self, subscription: u64) {
        self.queries.borrow_mut().remove(&subscription);
    }
}

/// Dispatch engine command `command` with its arguments. The outcome goes
/// to `done`; a failure also shows in the context's window.
pub fn dispatch(ctx: &Ctx, command: usize, args: Vec<Value>, done: Option<Done>) {
    let host = host();
    let Some(spec) = host.def.descriptor.commands.get(command) else {
        return;
    };
    let mut writer = Writer::new();
    for (value, param) in args.iter().zip(&spec.params) {
        encode(value, &param.ty, &host.def.descriptor.types, &mut writer);
    }
    let token = host.token(ctx.window_of(), done);
    with_bytes(&writer.bytes, |ptr, len| {
        // SAFETY: the handle is live while the application runs; the
        // parameter bytes are valid for this call.
        unsafe { bern_ffi::bern_app_dispatch(host.app(), command as u32, ptr, len, token) }
    });
}

/// A drop area that takes files the system hands the application: its
/// token, the kinds it takes and what it does with a file.
type Opener = (u64, &'static str, crate::features::Give);

thread_local! {
    /// Drop areas that take the files the system hands the application:
    /// the kinds each takes and what it does with a file.
    static OPENERS: RefCell<Vec<Opener>> = const { RefCell::new(Vec::new()) };
    /// Files that arrived while no drop area took them.
    static WAITING: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static NEXT_OPENER: StdCell<u64> = const { StdCell::new(1) };
}

/// A drop area with `opens: true` takes the files the system hands the
/// application while the current scope lasts; files that waited for one
/// arrive now.
pub(crate) fn add_opener(kinds: &'static str, give: Rc<dyn Fn(String)>) {
    let token = NEXT_OPENER.with(|next| {
        let token = next.get();
        next.set(token + 1);
        token
    });
    OPENERS.with(|openers| openers.borrow_mut().push((token, kinds, Rc::clone(&give))));
    crate::reactive::on_cleanup(move || {
        OPENERS.with(|openers| openers.borrow_mut().retain(|(id, _, _)| *id != token));
    });
    let waiting: Vec<String> = WAITING.with(|waiting| {
        let mut waiting = waiting.borrow_mut();
        let (taken, kept) = waiting
            .drain(..)
            .partition(|url| crate::features::accepts(url, kinds));
        *waiting = kept;
        taken
    });
    if !waiting.is_empty() {
        glib::idle_add_local_once(move || {
            for url in waiting {
                give(url);
            }
        });
    }
}

/// Files the system hands the application, as from a file manager's “Open
/// With”: the newest drop area that takes the kind gets each; the primary
/// window comes forward, and files wait for a drop area to appear.
fn open_files(files: &[gio::File]) {
    windows::reopen_primary();
    for file in files {
        let url = file.uri().to_string();
        let taker = OPENERS.with(|openers| {
            openers
                .borrow()
                .iter()
                .rev()
                .find(|(_, kinds, _)| crate::features::accepts(&url, kinds))
                .map(|(_, _, give)| Rc::clone(give))
        });
        match taker {
            Some(give) => give(url),
            None => WAITING.with(|waiting| waiting.borrow_mut().push(url)),
        }
    }
}

/// The directory of the declared resources, next to the executable.
pub(crate) fn resources_dir() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("resources")))
        .filter(|dir| dir.is_dir())
        .map(|dir| dir.display().to_string())
        .unwrap_or_default()
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
    #[cfg(feature = "script")]
    crate::script::start();
    let app = start(callbacks, &resources_dir(), &mut error);
    if app.is_null() {
        #[cfg(feature = "script")]
        crate::script::stop();
        return Err(String::from_utf8_lossy(&take(error)).into_owned());
    }
    host.app.set(app);
    // SAFETY: the handle was just created.
    *host.engine_id.borrow_mut() =
        String::from_utf8_lossy(&take(unsafe { bern_ffi::bern_app_engine_id(app) })).into_owned();
    // SAFETY: as above.
    host.receive_state(&take(unsafe { bern_ffi::bern_app_snapshot(app) }));
    Ok(())
}

/// Bern's icon names are Adwaita's. Other themes, such as Breeze on KDE,
/// lack many of them or draw them in colors of their own, so wherever
/// Adwaita is installed the application keeps it, as libadwaita's widgets
/// expect.
fn keep_adwaita_icons() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let installed = gtk::IconTheme::for_display(&display)
        .search_path()
        .iter()
        .any(|dir| dir.join("Adwaita").join("index.theme").is_file());
    // The display's own icon theme follows this setting; it takes no name
    // of its own.
    if installed {
        gtk::Settings::for_display(&display).set_gtk_icon_theme_name(Some("Adwaita"));
    }
}

/// The application icon: `icons/<bundle id>.png` beside the executable joins
/// the icon theme's search path, and windows take it as their icon.
fn install_icon(bundle_id: &str) {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("icons")))
        .filter(|dir| dir.is_dir())
    else {
        return;
    };
    if let Some(display) = gtk::gdk::Display::default() {
        let theme = gtk::IconTheme::for_display(&display);
        theme.add_search_path(&dir);
        if !theme.has_icon(bundle_id) {
            eprintln!(
                "bern: the application icon {bundle_id}.png is missing from {}",
                dir.display()
            );
        }
    }
    gtk::Window::set_default_icon_name(bundle_id);
}

/// Accent colors from the brand color, for libadwaita's named colors.
fn brand_css(red: u8, green: u8, blue: u8) -> String {
    format!(
        "@define-color accent_bg_color rgb({red},{green},{blue});\n@define-color accent_color rgb({red},{green},{blue});\n"
    )
}

/// Install the host of `def` on this thread: the interface language, and
/// one cell per engine state field at its type's default.
fn install(def: &'static AppDef) -> Rc<Host> {
    text::install(def.strings, def.languages, def.default_language);
    let engine = def
        .descriptor
        .state
        .iter()
        .map(|field| {
            Cell::new(Value::default_for(&field.ty, &def.descriptor.types).unwrap_or(Value::Unit))
        })
        .collect();
    let shared = Rc::new(Host {
        app: StdCell::new(std::ptr::null_mut()),
        engine,
        revision: StdCell::new(0),
        replies: RefCell::new(HashMap::new()),
        next: StdCell::new(1),
        queries: RefCell::new(HashMap::new()),
        engine_id: RefCell::new(String::new()),
        application: OnceCell::new(),
        def,
    });
    HOST.with(|cell| {
        let _ = cell.set(Rc::clone(&shared));
    });
    shared
}

/// Run the application: start the engine, open the initial windows and
/// keep running until the application quits.
/// What the engine asks of the desktop: the clipboard, on the main
/// thread, where GDK lives.
struct Services;

impl bern_ffi::platform::Services for Services {
    fn request(
        &self,
        operation: &str,
        first: &str,
        _second: &str,
    ) -> Result<String, bern_ffi::protocol::CommandError> {
        match operation {
            "copy_text" => {
                let text = first.to_string();
                glib::MainContext::default().invoke(move || {
                    if let Some(display) = gtk::gdk::Display::default() {
                        display.clipboard().set_text(&text);
                    }
                });
                Ok(String::new())
            }
            _ => Err(bern_ffi::protocol::CommandError::new(
                "platform_unavailable",
                format!("Linux has no `{operation}`"),
            )),
        }
    }
}

pub fn run(def: AppDef, start: StartEngine) -> glib::ExitCode {
    // The definition lives as long as the process.
    let def: &'static AppDef = Box::leak(Box::new(def));
    let shared = install(def);
    let application = adw::Application::builder()
        .application_id(shared.def.bundle_id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let _ = shared.application.set(application.clone());
    // Started at login (“Open at Login” of the status item): no window, and
    // a running instance stays as it is.
    let at_login =
        def.status_item.is_some() && std::env::args().skip(1).any(|arg| arg == "--at-login");
    let start = RefCell::new(Some(start));
    let hold = RefCell::new(None);
    let failed = Rc::new(StdCell::new(false));
    let failed_startup = Rc::clone(&failed);
    application.connect_startup(move |application| {
        snapshot::apply_appearance();
        keep_adwaita_icons();
        install_icon(host().def.bundle_id);
        crate::style::install(host().def.styles, host().def.text_styles);
        if let Some((red, green, blue)) = host().def.brand_seed {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&brand_css(red, green, blue));
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        }
        let Some(start) = start.borrow_mut().take() else {
            return;
        };
        bern_ffi::platform::install(Services);
        if let Err(message) = start_engine(&host(), start) {
            eprintln!("bern: {message}");
            failed_startup.set(true);
            windows::show_startup_error(application, &message);
            return;
        }
        media::attach_executor();
        windows::install_actions(application);
        let def = host().def;
        if let Some(item) = &def.status_item {
            crate::status::show(item);
        }
        // The engine keeps running without windows, as the session's
        // entries may still use it; activating the application again
        // opens the primary window.
        *hold.borrow_mut() = Some(application.hold());
    });
    let opened = StdCell::new(false);
    let failed_activate = Rc::clone(&failed);
    application.connect_activate(move |_| {
        if failed_activate.get() {
            return;
        }
        if opened.replace(true) {
            windows::reopen_primary();
        } else if !at_login {
            windows::open_initial();
        }
    });
    let opened_files = Rc::clone(&failed);
    application.connect_open(move |_, files, _| {
        if !opened_files.get() {
            open_files(files);
        }
    });
    application.connect_shutdown(|_| {
        let host = host();
        media::detach_executor();
        #[cfg(feature = "script")]
        crate::script::stop();
        let app = host.app.replace(std::ptr::null_mut());
        if !app.is_null() {
            // SAFETY: the handle is live and not used afterwards.
            unsafe { bern_ffi::bern_app_stop(app) };
        }
    });
    // Files given on the command line, as a desktop entry's `%U` passes
    // them, open in the running instance; GTK's own options are not taken.
    let args: Vec<String> = std::env::args()
        .take(1)
        .chain(std::env::args().skip(1).filter(|arg| !arg.starts_with('-')))
        .collect();
    // Registering runs `startup`, so it comes after the handlers above. A
    // second instance leaves before the application is finalized, which
    // would warn that `run` did not end its registration.
    if at_login
        && application.register(None::<&gio::Cancellable>).is_ok()
        && application.is_remote()
    {
        std::process::exit(0);
    }
    application.run_with_args(&args)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::protocol::{EnumDef, ProtocolDescriptor, StateField, Type, TypeTable, VariantDef};
    use crate::reactive::{Scope, effect};

    /// An application that declares nothing, for a test to fill in.
    pub(crate) fn app() -> AppDef {
        AppDef {
            id: "test",
            bundle_id: "org.bern.Test",
            name: "app.name",
            version: "1.0",
            fingerprint: 0,
            languages: &["en"],
            default_language: "en",
            strings: &[],
            descriptor: ProtocolDescriptor {
                descriptor_version: 1,
                namespace: "test".into(),
                version: 1,
                types: TypeTable::default(),
                state: Vec::new(),
                commands: Vec::new(),
                events: Vec::new(),
                modules: Vec::new(),
            },
            types: TypeTable::default(),
            sessions: Vec::new(),
            windows: Vec::new(),
            primary: "main",
            players: Vec::new(),
            compact_below: None,
            medium_below: None,
            expanded_below: None,
            brand_seed: None,
            styles: &[],
            text_styles: &[],
            theme: None,
            file_kinds: &[],
            status_item: None,
        }
    }

    /// The host of `def` on this test's thread, without an engine or a
    /// display.
    pub(crate) fn start(def: AppDef) -> Rc<Host> {
        install(Box::leak(Box::new(def)))
    }

    /// An enum whose variants have no payload.
    pub(crate) fn unit_enum(name: &str, variants: &[&str]) -> EnumDef {
        EnumDef {
            name: name.into(),
            variants: variants
                .iter()
                .map(|variant| VariantDef {
                    name: variant.to_string(),
                    fields: Vec::new(),
                    label: None,
                })
                .collect(),
        }
    }

    /// A host whose engine state is a count and a title.
    fn counter() -> Rc<Host> {
        let mut def = app();
        def.descriptor.state = ["count", "title"]
            .into_iter()
            .zip([Type::Int, Type::Text])
            .map(|(name, ty)| StateField {
                name: name.into(),
                ty,
                doc: String::new(),
                content: false,
            })
            .collect();
        start(def)
    }

    /// A state update of `revision` changing `changes`, encoded with the
    /// declared field types.
    fn update(host: &Host, revision: u64, changes: &[(u32, Value)]) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.u64(revision);
        writer.u32(changes.len() as u32);
        for (field, value) in changes {
            writer.u32(*field);
            let ty = &host.def.descriptor.state[*field as usize].ty;
            encode(value, ty, &host.def.descriptor.types, &mut writer);
        }
        writer.bytes
    }

    #[test]
    fn engine_state_starts_at_the_defaults() {
        counter();
        assert_eq!(engine_value(0), Value::Int(0));
        assert_eq!(engine_value(1), Value::Text(String::new()));
        assert_eq!(engine_value(2), Value::Unit);
    }

    #[test]
    fn state_updates_apply_in_one_batch() {
        let host = counter();
        let runs = Rc::new(StdCell::new(0));
        let scope = Scope::detached();
        let counted = Rc::clone(&runs);
        scope.run(|| {
            effect(move || {
                engine_value(0);
                engine_value(1);
                counted.set(counted.get() + 1);
            })
        });
        host.receive_state(&update(
            &host,
            1,
            &[(0, Value::Int(3)), (1, Value::Text("Inbox".into()))],
        ));
        assert_eq!(engine_value(0), Value::Int(3));
        assert_eq!(engine_value(1), Value::Text("Inbox".into()));
        assert_eq!(runs.get(), 2);
        scope.dispose();
    }

    #[test]
    fn older_revisions_are_ignored() {
        let host = counter();
        host.receive_state(&update(&host, 5, &[(0, Value::Int(5))]));
        host.receive_state(&update(&host, 4, &[(0, Value::Int(4))]));
        assert_eq!(engine_value(0), Value::Int(5));
        host.receive_state(&update(&host, 6, &[(1, Value::Text("Later".into()))]));
        assert_eq!(engine_value(0), Value::Int(5));
        assert_eq!(engine_value(1), Value::Text("Later".into()));
    }

    #[test]
    fn malformed_updates_change_nothing() {
        let host = counter();
        host.receive_state(&update(&host, 1, &[(0, Value::Int(1))]));
        // A valid change followed by a field the engine does not have.
        let mut unknown = Writer::new();
        unknown.u64(2);
        unknown.u32(2);
        unknown.u32(0);
        unknown.i64(2);
        unknown.u32(9);
        unknown.i64(9);
        host.receive_state(&unknown.bytes);
        // A change without its value.
        let mut truncated = Writer::new();
        truncated.u64(3);
        truncated.u32(1);
        truncated.u32(0);
        host.receive_state(&truncated.bytes);
        host.receive_state(&[]);
        assert_eq!(engine_value(0), Value::Int(1));
    }

    #[test]
    fn status_reads_success_or_failure() {
        let mut ok = Writer::new();
        ok.u8(0);
        ok.u32(42);
        let mut reader = status(&ok.bytes).expect("success");
        assert_eq!(reader.u32().ok(), Some(42));
        let mut failed = Writer::new();
        failed.u8(1);
        failed.text("not_found");
        failed.text("No such window");
        assert_eq!(
            status(&failed.bytes).err(),
            Some(Failure::new("not_found", "No such window"))
        );
        // A failure without its code and message, and no status at all.
        assert_eq!(status(&[1]).err(), Some(Failure::new("ui_failed", "")));
        assert_eq!(
            status(&[]).err().map(|failure| failure.code),
            Some("invalid_reply".to_string())
        );
    }

    #[test]
    fn replies_complete_their_command_once() {
        let host = start(app());
        let results = Rc::new(RefCell::new(Vec::new()));
        let token = || {
            let results = Rc::clone(&results);
            host.token(
                None,
                Some(Box::new(move |result| results.borrow_mut().push(result))),
            )
        };
        let (first, second, third) = (token(), token(), token());
        assert!(first != second && second != third);
        let mut ok = Writer::new();
        ok.u8(0);
        ok.u64(7);
        let mut failed = Writer::new();
        failed.u8(1);
        failed.u64(7);
        failed.text("busy");
        failed.text("Try again later");
        host.receive_reply(first, &ok.bytes);
        host.receive_reply(first, &failed.bytes);
        host.receive_reply(second, &failed.bytes);
        host.receive_reply(third, &[0]);
        host.receive_reply(third + 1, &ok.bytes);
        assert_eq!(
            *results.borrow(),
            vec![
                Ok(()),
                Err(Failure::new("busy", "Try again later")),
                Err(Failure::new("invalid_reply", "malformed reply")),
            ]
        );
    }

    #[test]
    fn brand_color_becomes_the_accent() {
        assert_eq!(
            brand_css(53, 132, 228),
            "@define-color accent_bg_color rgb(53,132,228);\n@define-color accent_color rgb(53,132,228);\n"
        );
    }
}
