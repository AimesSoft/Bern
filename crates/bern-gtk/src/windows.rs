//! Native windows: instances and their drafts, page headers, the close
//! protocol and presentation facts.
//!
//! Every declared window is a libadwaita application window. An instance
//! registers with the session runtime when it opens and gets a generation
//! and an id such as `settings#2`; its drafts live in cells mirrored from
//! the runtime. Closing always runs the declared close check first, from
//! the title bar, Ctrl+W, a declared action or an external entry.

use crate::ctx::Page;
use crate::host::{host, status, take, with_bytes};
use crate::protocol::{Value, Writer, decode, encode};
use crate::reactive::{Cell, Scope, effect, untrack};
use crate::text::tr;
use crate::{CloseCheck, Ctx, Done, Failure, Field, Parent, snapshot};
use adw::prelude::*;
use std::cell::{Cell as StdCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

/// A close confirmation waiting for a decision.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingClose {
    pub(crate) id: String,
    pub(crate) message: String,
    pub(crate) can_save: bool,
    pub(crate) draft_revision: u64,
}

/// Outcome of a close request or decision, as the session catalog's
/// `CloseResult` describes it.
pub(crate) struct CloseResult {
    pub(crate) outcome: u32,
    pub(crate) window: String,
    pub(crate) confirmation: Option<String>,
    pub(crate) draft_revision: u64,
    pub(crate) decisions: Vec<u32>,
    pub(crate) message: Option<String>,
    pub(crate) reason: Option<String>,
}

pub(crate) const CLOSED: u32 = 0;
pub(crate) const REJECTED: u32 = 1;
pub(crate) const CONFIRMATION_REQUIRED: u32 = 2;

impl CloseResult {
    fn closed(window: &str, draft_revision: u64) -> Self {
        Self {
            outcome: CLOSED,
            window: window.to_string(),
            confirmation: None,
            draft_revision,
            decisions: vec![],
            message: None,
            reason: None,
        }
    }

    fn rejected(window: &str, draft_revision: u64, reason: String) -> Self {
        Self {
            outcome: REJECTED,
            reason: Some(reason),
            ..Self::closed(window, draft_revision)
        }
    }

    pub(crate) fn to_value(&self) -> Value {
        let text = |value: &Option<String>| match value {
            Some(value) => Value::some(Value::Text(value.clone())),
            None => Value::none(),
        };
        Value::Record(vec![
            Value::unit_variant(self.outcome),
            Value::Text(self.window.clone()),
            text(&self.confirmation),
            Value::Int(self.draft_revision as i64),
            Value::List(
                self.decisions
                    .iter()
                    .map(|decision| Value::unit_variant(*decision))
                    .collect(),
            ),
            text(&self.message),
            text(&self.reason),
        ])
    }
}

/// What the platform shows for a window, in the session catalog's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Facts {
    /// 0 visible, 1 minimized, 2 hidden.
    pub(crate) visibility: u8,
    /// 0 normal, 1 maximized, 2 full screen.
    pub(crate) size: u8,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) focused: bool,
    pub(crate) actions: u32,
}

/// Window actions, by their index in the session catalog. GTK 4 cannot
/// place windows, so `Move` (5) is never available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Activate = 0,
    Close = 1,
    Minimize = 2,
    Restore = 3,
    Maximize = 4,
    Resize = 6,
}

struct Wait {
    satisfied: Box<dyn Fn(&Facts) -> bool>,
    complete: Option<Done>,
}

/// A window instance.
pub(crate) struct Win {
    pub(crate) kind: usize,
    pub(crate) generation: u64,
    pub(crate) id: String,
    cells: HashMap<usize, Vec<Cell<Value>>>,
    pub(crate) window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    pub(crate) pending: Cell<Option<PendingClose>>,
    allow_close: StdCell<bool>,
    pub(crate) scope: Scope,
    command: StdCell<bool>,
    minimize_requested: StdCell<bool>,
    waits: RefCell<Vec<Wait>>,
    dialog: RefCell<Option<adw::AlertDialog>>,
    report_queued: StdCell<bool>,
    closed: StdCell<bool>,
    /// The layer above the content where shared elements leave pictures of
    /// themselves.
    pub(crate) ghosts: gtk::Fixed,
    /// `window.idle`: no input for `idle_after` seconds.
    idle: Cell<bool>,
    /// Whether rest counts toward `window.idle` (`idle_enabled`).
    idle_enabled: StdCell<bool>,
    /// The layout size of the window's width.
    size: Cell<&'static str>,
    /// `window.short`: the window is less tall than the compact breakpoint.
    short: Cell<bool>,
    /// The layer above the window's content where modals and palettes
    /// stand.
    pub(crate) overlay: gtk::Overlay,
    /// The window's content below that layer, kept from the focus while a
    /// modal stands above it.
    pub(crate) below: gtk::Widget,
    immersive: Cell<bool>,
    /// Toolbar views whose top bars follow `chrome` and `immersive`.
    views: RefCell<Vec<glib::WeakRef<adw::ToolbarView>>>,
    shortcuts: RefCell<Vec<Shortcut>>,
    next_shortcut: StdCell<u64>,
}

/// A mounted shortcut: its token, keys and action.
type Shortcut = (u64, Keys, Rc<dyn Fn()>);

/// A keyboard shortcut: the modifiers and the key, lowercase.
#[derive(Clone, Debug, PartialEq)]
struct Keys {
    modifiers: gtk::gdk::ModifierType,
    key: String,
}

/// `cmd+shift+k` as modifiers and a key; `cmd` is Control here.
fn parse_keys(text: &str) -> Keys {
    let mut modifiers = gtk::gdk::ModifierType::empty();
    let mut key = String::new();
    for part in text.split('+') {
        match part {
            "cmd" | "ctrl" => modifiers |= gtk::gdk::ModifierType::CONTROL_MASK,
            "alt" => modifiers |= gtk::gdk::ModifierType::ALT_MASK,
            "shift" => modifiers |= gtk::gdk::ModifierType::SHIFT_MASK,
            other => key = other.to_string(),
        }
    }
    Keys { modifiers, key }
}

/// The name of a pressed key, as shortcuts write keys.
fn key_name(keyval: gtk::gdk::Key) -> Option<String> {
    use gtk::gdk::Key;
    let named = match keyval {
        Key::space => "space",
        Key::Escape => "escape",
        Key::Return | Key::KP_Enter => "return",
        Key::Tab | Key::ISO_Left_Tab => "tab",
        Key::BackSpace | Key::Delete => "delete",
        Key::Up => "up",
        Key::Down => "down",
        Key::Left => "left",
        Key::Right => "right",
        Key::Home => "home",
        Key::End => "end",
        Key::Page_Up => "pageup",
        Key::Page_Down => "pagedown",
        other => {
            return other
                .to_lower()
                .to_unicode()
                .filter(|c| !c.is_control())
                .map(|c| c.to_string());
        }
    };
    Some(named.to_string())
}

impl Win {
    pub(crate) fn cell(&self, field: Field) -> Cell<Value> {
        self.cells
            .get(&field.session)
            .and_then(|cells| cells.get(field.field))
            .cloned()
            .unwrap_or_else(|| Cell::new(Value::Unit))
    }

    /// Write a draft field: the session runtime first, then the cell.
    pub(crate) fn write(&self, field: Field, value: Value) -> bool {
        let host = host();
        let Some((_, ty, _)) = host
            .def
            .sessions
            .get(field.session)
            .and_then(|session| session.fields.get(field.field))
        else {
            return false;
        };
        let mut writer = Writer::new();
        encode(&value, ty, &host.def.types, &mut writer);
        let bytes = with_bytes(&writer.bytes, |ptr, len| {
            // SAFETY: the handle is live; the value bytes are valid for
            // this call.
            take(unsafe {
                bern_ffi::bern_session_set(
                    host.app(),
                    self.kind as u32,
                    self.generation,
                    field.session as u32,
                    field.field as u32,
                    ptr,
                    len,
                )
            })
        });
        match status(&bytes) {
            Ok(_) => {
                self.cell(field).set(value);
                snapshot::schedule();
                true
            }
            Err(failure) => {
                eprintln!("bern: {}: {}", failure.code, failure.message);
                false
            }
        }
    }

    /// Show a failed command to the user.
    pub(crate) fn fail(&self, failure: &Failure) {
        let toast = adw::Toast::new(&format!(
            "{} — {}",
            tr("bern.alert.failed"),
            failure.message
        ));
        toast.set_timeout(6);
        self.toasts.add_toast(toast);
    }

    pub(crate) fn is_blocked(&self) -> bool {
        self.pending.peek().is_some()
    }

    /// Show a notice at the bottom of the window.
    pub(crate) fn show_toast(&self, toast: &adw::Toast) {
        self.toasts.add_toast(toast.clone());
    }

    /// UI state of the window, recording a dependency: `idle`, and the
    /// layout size `compact`, `medium`, `expanded` or `large`.
    pub(crate) fn state(&self, field: &str) -> Value {
        match field {
            "idle" => Value::Bool(self.idle.get()),
            "short" => Value::Bool(self.short.get()),
            size => Value::Bool(self.size.get() == size),
        }
    }

    /// A toolbar view of the window: its top bars follow `chrome` and hide
    /// while the window is immersive.
    pub(crate) fn chrome_view(self: &Rc<Self>, view: &adw::ToolbarView) {
        if self.def().unified {
            view.set_top_bar_style(adw::ToolbarStyle::Flat);
        }
        self.views.borrow_mut().push(view.downgrade());
        view.set_reveal_top_bars(!self.immersive.peek());
    }

    /// Run `run` on `keys` while the current scope lasts. The latest
    /// shortcut for keys wins.
    pub(crate) fn add_shortcut(self: &Rc<Self>, keys: &str, run: Rc<dyn Fn()>) {
        let token = self.next_shortcut.get();
        self.next_shortcut.set(token + 1);
        self.shortcuts
            .borrow_mut()
            .push((token, parse_keys(keys), run));
        let win = Rc::downgrade(self);
        crate::reactive::on_cleanup(move || {
            if let Some(win) = win.upgrade() {
                win.shortcuts
                    .borrow_mut()
                    .retain(|(candidate, _, _)| *candidate != token);
            }
        });
    }

    fn shortcut_for(
        &self,
        keyval: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) -> Option<Rc<dyn Fn()>> {
        let key = key_name(keyval)?;
        let wanted = gtk::gdk::ModifierType::CONTROL_MASK
            | gtk::gdk::ModifierType::ALT_MASK
            | gtk::gdk::ModifierType::SHIFT_MASK
            | gtk::gdk::ModifierType::SUPER_MASK;
        let pressed = Keys {
            modifiers: state & wanted,
            key,
        };
        let found = self
            .shortcuts
            .borrow()
            .iter()
            .rev()
            .find(|(_, keys, _)| *keys == pressed)
            .map(|(_, keys, run)| (keys.clone(), Rc::clone(run)))?;
        // A key without a modifier does not fire while text is being edited.
        let plain = !found
            .0
            .modifiers
            .intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK);
        let editing = gtk::prelude::GtkWindowExt::focus(&self.window).is_some_and(|focus| {
            focus.is::<gtk::Text>() || focus.is::<gtk::TextView>() || focus.is::<gtk::Editable>()
        });
        if plain && editing {
            return None;
        }
        Some(found.1)
    }

    /// Input arrived: `window.idle` is false until the window rests again.
    fn rest(self: &Rc<Self>, seconds: f64, timer: &Rc<RefCell<Option<glib::SourceId>>>) {
        if self.idle.peek() {
            self.idle.set(false);
            snapshot::schedule();
        }
        if let Some(source) = timer.borrow_mut().take() {
            source.remove();
        }
        // While idling is off, rest counts for nothing.
        if !self.idle_enabled.get() {
            return;
        }
        let win = Rc::downgrade(self);
        let done = Rc::clone(timer);
        *timer.borrow_mut() = Some(glib::timeout_add_local_once(
            Duration::from_secs_f64(seconds.max(0.0)),
            move || {
                done.borrow_mut().take();
                if let Some(win) = win.upgrade() {
                    win.idle.set(true);
                    snapshot::schedule();
                }
            },
        ));
    }

    fn def(&self) -> &'static crate::WindowDef {
        &host().def.windows[self.kind]
    }

    pub(crate) fn kind_id(&self) -> &'static str {
        self.def().id
    }

    fn resizable(&self) -> bool {
        self.window.is_resizable()
    }

    pub(crate) fn facts(&self) -> Facts {
        let window = &self.window;
        let state = window
            .surface()
            .and_then(|surface| surface.downcast::<gtk::gdk::Toplevel>().ok())
            .map(|toplevel| toplevel.state())
            .unwrap_or(gtk::gdk::ToplevelState::empty());
        let minimized = state.contains(gtk::gdk::ToplevelState::MINIMIZED)
            || (self.minimize_requested.get()
                && state.contains(gtk::gdk::ToplevelState::SUSPENDED));
        let visibility = if !window.is_visible() {
            2
        } else if minimized {
            1
        } else {
            0
        };
        let size = if window.is_fullscreen() {
            2
        } else if window.is_maximized() {
            1
        } else {
            0
        };
        // The window's own size, without the shadow its surface includes.
        let (width, height) = (window.width() as f64, window.height() as f64);
        let mut actions = (1 << Action::Activate as u32) | (1 << Action::Close as u32);
        if visibility == 0 {
            actions |= 1 << Action::Minimize as u32;
        }
        if visibility == 1 || size != 0 {
            actions |= 1 << Action::Restore as u32;
        }
        if self.resizable() && size == 0 {
            actions |= (1 << Action::Maximize as u32) | (1 << Action::Resize as u32);
        }
        Facts {
            visibility,
            size,
            width,
            height,
            focused: window.is_active(),
            actions,
        }
    }

    /// Report the current facts to the session runtime, then finish the
    /// presentation commands they satisfy.
    pub(crate) fn report(self: &Rc<Self>) {
        if self.closed.get() {
            return;
        }
        let facts = self.facts();
        if facts.visibility == 0 {
            self.minimize_requested.set(false);
        }
        // SAFETY: the handle is live while the application runs.
        unsafe {
            bern_ffi::bern_window_facts(
                host().app(),
                self.kind as u32,
                self.generation,
                if self.command.get() { 0 } else { 1 },
                facts.visibility,
                facts.size,
                0.0,
                0.0,
                facts.width,
                facts.height,
                facts.focused as u8,
                facts.actions,
            )
        };
        let finished: Vec<Wait> = {
            let mut waits = self.waits.borrow_mut();
            let (done, waiting): (Vec<_>, Vec<_>) =
                waits.drain(..).partition(|wait| (wait.satisfied)(&facts));
            *waits = waiting;
            done
        };
        for mut wait in finished {
            if let Some(complete) = wait.complete.take() {
                complete(Ok(()));
            }
        }
        if self.waits.borrow().is_empty() {
            self.command.set(false);
        }
    }

    /// Report facts once after the current changes settle.
    pub(crate) fn schedule_report(self: &Rc<Self>) {
        if self.report_queued.replace(true) {
            return;
        }
        let win = Rc::clone(self);
        glib::idle_add_local_once(move || {
            win.report_queued.set(false);
            win.report();
        });
    }

    /// Perform a presentation change and finish `complete` once the facts
    /// show it, or with `ui_timeout` after five seconds.
    fn perform(
        self: &Rc<Self>,
        change: impl FnOnce(&adw::ApplicationWindow),
        satisfied: impl Fn(&Facts) -> bool + 'static,
        complete: Done,
    ) {
        self.command.set(true);
        self.waits.borrow_mut().push(Wait {
            satisfied: Box::new(satisfied),
            complete: Some(complete),
        });
        change(&self.window);
        self.schedule_report();
        let win = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_secs(5), move || {
            let Some(win) = win.upgrade() else { return };
            win.report();
            let expired: Vec<Wait> = win.waits.borrow_mut().drain(..).collect();
            for mut wait in expired {
                if let Some(complete) = wait.complete.take() {
                    complete(Err(Failure::new(
                        "ui_timeout",
                        "the window system did not report the change in time; the outcome is unknown",
                    )));
                }
            }
            win.command.set(false);
        });
    }
}

struct Manager {
    windows: RefCell<Vec<Rc<Win>>>,
}

thread_local! {
    static MANAGER: Manager = const { Manager { windows: RefCell::new(Vec::new()) } };
}

pub(crate) fn all() -> Vec<Rc<Win>> {
    MANAGER.with(|manager| manager.windows.borrow().clone())
}

pub(crate) fn named(id: &str) -> Result<Rc<Win>, Failure> {
    all()
        .into_iter()
        .find(|win| win.id == id)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{id}`")))
}

fn instance_of(kind: usize) -> Option<Rc<Win>> {
    all().into_iter().find(|win| win.kind == kind)
}

/// A page header: the innermost title or title widget, a back button
/// while something can go back, and the page's tools.
pub(crate) fn header(page: &Rc<Page>, fallback: Option<Rc<dyn Fn() -> String>>) -> adw::HeaderBar {
    let header = adw::HeaderBar::new();
    let back = gtk::Button::from_icon_name("go-previous-symbolic");
    back.add_css_class("flat");
    header.pack_start(&back);
    header.pack_start(&page.leading_tools);
    let clicked = Rc::clone(page);
    back.connect_clicked(move |_| {
        if let Some(back) = clicked.back() {
            back();
        }
    });
    header.pack_end(&page.tools);
    let title = adw::WindowTitle::new("", "");
    let page = Rc::clone(page);
    let shown = header.clone();
    effect(move || {
        page.version.track();
        back.set_tooltip_text(Some(&tr("bern.navigation.back")));
        back.set_visible(page.back().is_some());
        match page.title_widget() {
            Some(widget) => {
                if shown.title_widget().as_ref() != Some(&widget) {
                    shown.set_title_widget(Some(&widget));
                }
            }
            None => {
                let text = page
                    .title()
                    .or_else(|| fallback.as_ref().map(|fallback| fallback()))
                    .unwrap_or_default();
                title.set_title(&text);
                if shown.title_widget().as_ref() != Some(title.upcast_ref()) {
                    shown.set_title_widget(Some(&title));
                }
            }
        }
    });
    header
}

/// Generation, id and draft cells of a new window instance.
type Instance = (u64, String, HashMap<usize, Vec<Cell<Value>>>);

fn decode_instance(bytes: &[u8]) -> Result<Instance, Failure> {
    let host = host();
    let mut reader = status(bytes)?;
    let malformed = |error: crate::protocol::CodecError| Failure::new("invalid_reply", error.0);
    let generation = reader.u64().map_err(malformed)?;
    let id = reader.text().map_err(malformed)?;
    let slots = reader.u32().map_err(malformed)?;
    let mut cells = HashMap::new();
    for _ in 0..slots {
        let session = reader.u32().map_err(malformed)? as usize;
        let def = host
            .def
            .sessions
            .get(session)
            .ok_or_else(|| Failure::new("invalid_reply", "unknown session type"))?;
        let mut values = Vec::new();
        for (_, ty, _) in &def.fields {
            values.push(Cell::new(
                decode(ty, &host.def.types, &mut reader).map_err(malformed)?,
            ));
        }
        cells.insert(session, values);
    }
    Ok((generation, id, cells))
}

/// Open a window of `kind`, or show the one already open. Returns the
/// instance id.
pub(crate) fn open(kind_id: &str) -> Result<String, Failure> {
    let host = host();
    let kind = host
        .def
        .windows
        .iter()
        .position(|window| window.id == kind_id)
        .ok_or_else(|| Failure::new("target_not_found", format!("no window kind `{kind_id}`")))?;
    if let Some(existing) = instance_of(kind) {
        existing.window.present();
        return Ok(existing.id.clone());
    }
    let application = host.application.get().expect("application").clone();
    // SAFETY: the handle is live while the application runs.
    let bytes = take(unsafe { bern_ffi::bern_window_opened(host.app(), kind as u32) });
    let (generation, id, cells) = decode_instance(&bytes)?;
    let def = &host.def.windows[kind];

    let window = adw::ApplicationWindow::new(&application);
    let width = def.width.map(|width| width as i32).unwrap_or(-1);
    let height = def.height.map(|height| height as i32).unwrap_or(-1);
    window.set_default_size(width, height);
    // libadwaita's breakpoints need a minimum size; without one declared,
    // the smallest size GNOME's own applications take.
    window.set_size_request(
        def.min_width.map(|width| width as i32).unwrap_or(360),
        def.min_height.map(|height| height as i32).unwrap_or(294),
    );
    if def.dialog {
        // A dialog shows over the primary window and blocks it.
        if let Some(primary) = instance_of(
            host.def
                .windows
                .iter()
                .position(|window| window.id == host.def.primary)
                .unwrap_or(usize::MAX),
        ) {
            window.set_transient_for(Some(&primary.window));
            window.set_modal(true);
            window.set_destroy_with_parent(true);
        }
    }
    let toasts = adw::ToastOverlay::new();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&toasts));
    let ghosts = gtk::Fixed::new();
    ghosts.set_can_target(false);
    overlay.add_overlay(&ghosts);
    overlay.set_clip_overlay(&ghosts, true);
    // A probe over the whole content measures its height for
    // `window.short`.
    let tall = gtk::DrawingArea::new();
    tall.set_can_target(false);
    tall.set_can_focus(false);
    overlay.add_overlay(&tall);
    window.set_content(Some(&overlay));

    let win = Rc::new(Win {
        kind,
        generation,
        id: id.clone(),
        cells,
        window: window.clone(),
        toasts: toasts.clone(),
        pending: Cell::new(None),
        allow_close: StdCell::new(false),
        scope: Scope::detached(),
        command: StdCell::new(false),
        minimize_requested: StdCell::new(false),
        waits: RefCell::new(Vec::new()),
        dialog: RefCell::new(None),
        report_queued: StdCell::new(false),
        closed: StdCell::new(false),
        ghosts,
        idle: Cell::new(false),
        idle_enabled: StdCell::new(true),
        size: Cell::new("expanded"),
        short: Cell::new(false),
        overlay: overlay.clone(),
        below: toasts.clone().upcast(),
        immersive: Cell::new(false),
        views: RefCell::new(Vec::new()),
        shortcuts: RefCell::new(Vec::new()),
        next_shortcut: StdCell::new(1),
    });
    MANAGER.with(|manager| manager.windows.borrow_mut().push(Rc::clone(&win)));

    let page = Page::new();
    let title_key = def.title;
    let view = adw::ToolbarView::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_vexpand(true);
    // A probe with no height measures the window's width for its size.
    let probe = gtk::DrawingArea::new();
    probe.set_content_height(0);
    probe.set_hexpand(true);
    let sized = Rc::downgrade(&win);
    probe.connect_resize(move |_, width, _| {
        if width > 0
            && let Some(win) = sized.upgrade()
        {
            win.size.set(crate::structure::size_of(f64::from(width)));
        }
    });
    content.append(&probe);
    let measured = Rc::downgrade(&win);
    tall.connect_resize(move |_, _, height| {
        let compact = crate::host::host().def.compact_below.unwrap_or(0.0);
        if let Some(win) = measured.upgrade() {
            win.short.set(height > 0 && f64::from(height) < compact);
        }
    });
    view.set_content(Some(&content));
    toasts.set_child(Some(&view));
    let render = def.render;
    let settings = def.settings;
    win.scope.run(|| {
        let title_window = window.clone();
        effect(move || title_window.set_title(Some(&tr(title_key))));
        let bar = header(&page, Some(Rc::new(move || tr(title_key))));
        view.add_top_bar(&bar);
        if def.unified {
            view.set_top_bar_style(adw::ToolbarStyle::Flat);
        }
        let shown = Rc::clone(&page);
        let revealed = view.clone();
        let immersed = Rc::clone(&win);
        effect(move || {
            shown.version.track();
            revealed.set_reveal_top_bars(!shown.header_hidden() && !immersed.immersive.get());
        });
        let ctx = Ctx::root(Rc::clone(&win), Some(Rc::clone(&page)));
        let rest = watch_input(&win, def.idle_after);
        if let (Some(enabled), Some(rest)) = (def.idle_enabled, rest) {
            let at = ctx.clone();
            let shown = Rc::clone(&win);
            effect(move || {
                let on = enabled(&at);
                untrack(|| {
                    shown.idle_enabled.set(on);
                    // Turning idling on starts the count again; off, the
                    // window is not idle.
                    rest();
                });
            });
        }
        if let Some(immersive) = def.immersive {
            let at = ctx.clone();
            let shown = Rc::clone(&win);
            effect(move || {
                let on = immersive(&at);
                shown.immersive.set(on);
                for view in shown
                    .views
                    .borrow()
                    .iter()
                    .filter_map(|view| view.upgrade())
                {
                    view.set_reveal_top_bars(!on);
                }
                // The pointer hides while the window rests immersed.
                let hidden = on && shown.idle.get();
                shown
                    .window
                    .set_cursor_from_name(if hidden { Some("none") } else { None });
            });
        }
        let app: &'static crate::AppDef = host.def;
        if let Some(theme) = &app.theme {
            follow_theme(&ctx, theme);
        }
        render(&Parent::of_box(&content), &ctx);
        let blocked = Rc::clone(&win);
        let guarded = content.clone();
        effect(move || guarded.set_sensitive(blocked.pending.get().is_none()));
    });

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let shortcuts = Rc::downgrade(&win);
    keys.connect_key_pressed(move |_, keyval, _, state| {
        let run = shortcuts
            .upgrade()
            .and_then(|win| win.shortcut_for(keyval, state));
        match run {
            Some(run) => {
                run();
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    });
    window.add_controller(keys);

    if settings || def.dialog {
        let escape = gtk::ShortcutController::new();
        escape.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::NamedAction::new("window.close")),
        ));
        window.add_controller(escape);
    }

    let guard = Rc::downgrade(&win);
    window.connect_close_request(move |_| {
        let Some(win) = guard.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let proceed = win.allow_close.get()
            || match request_close_from(&win, false) {
                Ok(result) => result.outcome == CLOSED,
                Err(failure) => {
                    win.fail(&failure);
                    false
                }
            };
        if !proceed {
            return glib::Propagation::Stop;
        }
        // The window goes away now; the instance ends with it.
        closed(&win);
        glib::Propagation::Proceed
    });
    for property in [
        "maximized",
        "fullscreened",
        "is-active",
        "visible",
        "default-width",
        "default-height",
    ] {
        let watched = Rc::downgrade(&win);
        window.connect_notify_local(Some(property), move |_, _| {
            if let Some(win) = watched.upgrade() {
                win.schedule_report();
            }
        });
    }
    let observed = Rc::downgrade(&win);
    window.connect_realize(move |window| {
        let Some(surface) = window.surface() else {
            return;
        };
        let layout = observed.clone();
        surface.connect_layout(move |_, _, _| {
            if let Some(win) = layout.upgrade() {
                win.schedule_report();
            }
        });
        if let Ok(toplevel) = surface.downcast::<gtk::gdk::Toplevel>() {
            let state = observed.clone();
            toplevel.connect_state_notify(move |_| {
                if let Some(win) = state.upgrade() {
                    win.schedule_report();
                }
            });
        }
    });
    window.present();
    win.schedule_report();
    snapshot::watch(&win);
    Ok(id)
}

/// `window.idle`: pointer, touch, wheel and key input in the window start
/// the count of `seconds` again. Returns what starts it again.
fn watch_input(win: &Rc<Win>, seconds: Option<f64>) -> Option<Rc<dyn Fn()>> {
    let seconds = seconds?;
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let rest = {
        let win = Rc::downgrade(win);
        let timer = Rc::clone(&timer);
        Rc::new(move || {
            if let Some(win) = win.upgrade() {
                win.rest(seconds, &timer);
            }
        })
    };
    let pointer = gtk::EventControllerMotion::new();
    pointer.set_propagation_phase(gtk::PropagationPhase::Capture);
    let moved = Rc::clone(&rest);
    // Content moving under a still pointer makes GTK report motion at the
    // same place; only a pointer that moves is input.
    let last = StdCell::new(None::<(f64, f64)>);
    pointer.connect_motion(move |_, x, y| {
        if last.replace(Some((x, y))) != Some((x, y)) {
            moved();
        }
    });
    win.window.add_controller(pointer);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let typed = Rc::clone(&rest);
    keys.connect_key_pressed(move |_, _, _, _| {
        typed();
        glib::Propagation::Proceed
    });
    win.window.add_controller(keys);
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
    let scrolled = Rc::clone(&rest);
    wheel.connect_scroll(move |_, _, _| {
        scrolled();
        glib::Propagation::Proceed
    });
    win.window.add_controller(wheel);
    let press = gtk::GestureClick::new();
    press.set_propagation_phase(gtk::PropagationPhase::Capture);
    press.set_button(0);
    let pressed = Rc::clone(&rest);
    press.connect_pressed(move |_, _, _, _| pressed());
    win.window.add_controller(press);
    let ended = Rc::clone(&timer);
    win.scope.on_dispose(move || {
        if let Some(source) = ended.borrow_mut().take() {
            source.remove();
        }
    });
    rest();
    Some(rest)
}

/// `WindowSet(accent:, appearance:)`: the accent and appearance every
/// window shows, chosen at run time.
fn follow_theme(ctx: &Ctx, theme: &'static crate::ThemeDef) {
    if let Some(accent) = theme.accent {
        let at = ctx.clone();
        effect(move || {
            let variant = crate::value::tag(&accent(&at));
            untrack(|| crate::theme::set_accent(theme.accents, variant));
        });
    }
    if let Some(appearance) = theme.appearance {
        let at = ctx.clone();
        effect(move || {
            let variant = crate::value::tag(&appearance(&at));
            let name = variant
                .and_then(|variant| {
                    theme
                        .appearances
                        .iter()
                        .find(|(candidate, _)| *candidate == variant)
                })
                .map(|(_, name)| *name)
                .unwrap_or("system");
            untrack(|| crate::theme::set_appearance(name));
        });
    }
}

/// The window instance ended: release its drafts, controls and
/// subscriptions; application players stay.
fn closed(win: &Rc<Win>) {
    if win.closed.replace(true) {
        return;
    }
    MANAGER.with(|manager| {
        manager
            .windows
            .borrow_mut()
            .retain(|candidate| !Rc::ptr_eq(candidate, win))
    });
    win.scope.dispose();
    for mut wait in win.waits.borrow_mut().drain(..) {
        if let Some(complete) = wait.complete.take() {
            complete(Err(Failure::new(
                "stale_window",
                format!("`{}` closed while the command ran", win.id),
            )));
        }
    }
    // SAFETY: the handle is live while the application runs.
    unsafe { bern_ffi::bern_window_closed(host().app(), win.kind as u32, win.generation) };
}

fn evaluate(win: &Rc<Win>) -> Option<CloseCheck> {
    let check = win.def().close?;
    let ctx = Ctx::root(Rc::clone(win), None);
    Some(untrack(|| check(&ctx)))
}

/// Run the window's close check and ask the session runtime whether to
/// close now or wait for a decision. A pending confirmation is returned
/// again rather than asked twice. `close` closes the window when the
/// outcome is to close; the window's own close request passes false and
/// lets the close go on.
fn request_close_from(win: &Rc<Win>, close: bool) -> Result<CloseResult, Failure> {
    let check = evaluate(win);
    let needs = check.as_ref().is_some_and(|check| check.needs_confirmation);
    let can_save = check.as_ref().is_some_and(|check| check.save.is_some());
    // SAFETY: the handle is live while the application runs.
    let bytes = take(unsafe {
        bern_ffi::bern_window_request_close(
            host().app(),
            win.kind as u32,
            win.generation,
            needs as u8,
            can_save as u8,
        )
    });
    let mut reader = status(&bytes)?;
    let malformed = |error: crate::protocol::CodecError| Failure::new("invalid_reply", error.0);
    if reader.u8().map_err(malformed)? == 0 {
        if close {
            finish_close(win);
        } else {
            win.allow_close.set(true);
        }
        return Ok(CloseResult::closed(&win.id, 0));
    }
    let pending = PendingClose {
        id: reader.text().map_err(malformed)?,
        message: check.map(|check| check.message).unwrap_or_default(),
        can_save,
        draft_revision: reader.u64().map_err(malformed)?,
    };
    win.pending.set(Some(pending.clone()));
    show_dialog(win, &pending);
    snapshot::schedule();
    Ok(CloseResult {
        outcome: CONFIRMATION_REQUIRED,
        window: win.id.clone(),
        confirmation: Some(pending.id.clone()),
        draft_revision: pending.draft_revision,
        decisions: if pending.can_save {
            vec![0, 1, 2]
        } else {
            vec![1, 2]
        },
        message: Some(pending.message.clone()),
        reason: None,
    })
}

pub(crate) fn request_close(win: &Rc<Win>) -> Result<CloseResult, Failure> {
    request_close_from(win, true)
}

/// Close the window after the close protocol decided to: destroy it
/// directly, as a close request now would only close a dialog that is
/// still on it.
fn finish_close(win: &Rc<Win>) {
    win.allow_close.set(true);
    closed(win);
    win.window.destroy();
}

fn show_dialog(win: &Rc<Win>, pending: &PendingClose) {
    if let Some(dialog) = win.dialog.borrow().as_ref() {
        // The same confirmation is already on screen.
        if dialog.heading().map(|heading| heading.to_string()) == Some(pending.message.clone()) {
            return;
        }
    }
    let detail = if pending.can_save {
        tr("bern.close.save_detail")
    } else {
        tr("bern.close.discard_detail")
    };
    let dialog = adw::AlertDialog::new(Some(&pending.message), Some(&detail));
    dialog.add_response("cancel", &tr("bern.close.cancel"));
    if pending.can_save {
        dialog.add_response("discard", &tr("bern.close.dont_save"));
        dialog.add_response("save", &tr("bern.close.save"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
    } else {
        dialog.add_response("discard", &tr("bern.close.discard"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
    }
    dialog.set_close_response("cancel");
    let decided = Rc::downgrade(win);
    let confirmation = pending.id.clone();
    dialog.connect_response(None, move |dialog, response| {
        let Some(win) = decided.upgrade() else { return };
        // Answered elsewhere, for example by an external entry.
        if win.dialog.borrow().as_ref() != Some(dialog) {
            return;
        }
        win.dialog.borrow_mut().take();
        let decision = match response {
            "save" => 0,
            "discard" => 1,
            _ => 2,
        };
        confirm_close(&win, &confirmation, decision, Box::new(|_| {}));
    });
    *win.dialog.borrow_mut() = Some(dialog.clone());
    dialog.present(Some(&win.window));
}

fn dismiss_dialog(win: &Rc<Win>) {
    let dialog = win.dialog.borrow_mut().take();
    if let Some(dialog) = dialog {
        dialog.force_close();
    }
}

/// Apply a decision from the dialog or from an external entry. A save
/// that fails keeps the window open.
pub(crate) fn confirm_close(
    win: &Rc<Win>,
    confirmation: &str,
    decision: u32,
    completion: Box<dyn FnOnce(Result<CloseResult, Failure>)>,
) {
    let mut writer = Writer::new();
    writer.text(confirmation);
    writer.u32(decision);
    let bytes = with_bytes(&writer.bytes, |ptr, len| {
        // SAFETY: the handle is live; the bytes are valid for this call.
        take(unsafe {
            bern_ffi::bern_window_confirm_close(
                host().app(),
                win.kind as u32,
                win.generation,
                ptr,
                len,
            )
        })
    });
    let revision = win
        .pending
        .peek()
        .map(|pending| pending.draft_revision)
        .unwrap_or(0);
    let mut reader = match status(&bytes) {
        Ok(reader) => reader,
        Err(failure) => {
            // A stale confirmation no longer describes the window.
            if failure.code == "stale_confirmation"
                && win
                    .pending
                    .peek()
                    .is_some_and(|pending| pending.id == confirmation)
            {
                win.pending.set(None);
                dismiss_dialog(win);
            }
            completion(Err(failure));
            return;
        }
    };
    let step = reader.u8().unwrap_or(2);
    win.pending.set(None);
    dismiss_dialog(win);
    snapshot::schedule();
    match step {
        0 => {
            finish_close(win);
            completion(Ok(CloseResult::closed(&win.id, revision)));
        }
        1 => {
            let Some(save) = evaluate(win).and_then(|check| check.save) else {
                completion(Err(Failure::new("invalid_params", "no save command")));
                return;
            };
            let saved = Rc::clone(win);
            let ctx = Ctx::root(Rc::clone(win), None);
            save(
                &ctx,
                Box::new(move |result| match result {
                    Ok(()) => {
                        finish_close(&saved);
                        completion(Ok(CloseResult::closed(&saved.id, revision)));
                    }
                    Err(failure) => completion(Ok(CloseResult::rejected(
                        &saved.id,
                        revision,
                        format!("save_failed: {}: {}", failure.code, failure.message),
                    ))),
                }),
            );
        }
        _ => completion(Ok(CloseResult::rejected(
            &win.id,
            revision,
            "cancelled".into(),
        ))),
    }
}

/// Whether the window system reports minimized windows. Wayland
/// compositors do not: a minimized window only loses focus.
fn observes_minimize(window: &adw::ApplicationWindow) -> bool {
    !WidgetExt::display(window)
        .type_()
        .name()
        .contains("Wayland")
}

pub(crate) fn minimize(win: &Rc<Win>, done: Done) {
    win.minimize_requested.set(true);
    if !observes_minimize(&win.window) {
        win.window.minimize();
        win.schedule_report();
        done(Err(Failure::new(
            "ui_timeout",
            "the request was sent, but this Wayland compositor does not report minimized windows, so the outcome is unknown; window.activate shows the window again",
        )));
        return;
    }
    win.perform(
        |window| window.minimize(),
        |facts| facts.visibility == 1,
        done,
    );
}

pub(crate) fn restore(win: &Rc<Win>, done: Done) {
    let facts = win.facts();
    if facts.visibility == 1 {
        win.perform(
            |window| window.present(),
            |facts| facts.visibility == 0,
            done,
        );
    } else if facts.size == 2 {
        win.perform(
            |window| window.unfullscreen(),
            |facts| facts.size != 2,
            done,
        );
    } else {
        win.perform(|window| window.unmaximize(), |facts| facts.size == 0, done);
    }
}

pub(crate) fn maximize(win: &Rc<Win>, done: Done) {
    win.perform(|window| window.maximize(), |facts| facts.size == 1, done);
}

pub(crate) fn resize(win: &Rc<Win>, width: f64, height: f64, done: Done) {
    let def = win.def();
    let (minimum_width, minimum_height) = (
        def.min_width.unwrap_or(360.0),
        def.min_height.unwrap_or(294.0),
    );
    if width < minimum_width || height < minimum_height {
        done(Err(Failure::new(
            "out_of_range",
            format!(
                "`{}` needs at least {} × {}",
                win.id,
                crate::value::number(minimum_width),
                crate::value::number(minimum_height)
            ),
        )));
        return;
    }
    win.perform(
        move |window| window.set_default_size(width.round() as i32, height.round() as i32),
        move |facts| {
            (facts.width - width.round()).abs() <= 1.0
                && (facts.height - height.round()).abs() <= 1.0
        },
        done,
    );
}

/// Open the windows declared `initial: open`.
pub(crate) fn open_initial() {
    let kinds: Vec<&'static str> = host()
        .def
        .windows
        .iter()
        .filter(|window| window.initial_open)
        .map(|window| window.id)
        .collect();
    for kind in kinds {
        if let Err(failure) = open(kind) {
            eprintln!("bern: cannot open `{kind}`: {}", failure.message);
        }
    }
}

/// The application was activated again, for example from the launcher:
/// show the primary window, opening it when it was closed.
pub(crate) fn reopen_primary() {
    let primary = host().def.primary;
    if let Err(failure) = open(primary) {
        eprintln!("bern: cannot open `{primary}`: {}", failure.message);
    }
}

/// `windows.<kind>.open()` and `activate()` in declarations.
pub fn open_window(ctx: &Ctx, kind: &str) {
    if let Err(failure) = open(kind) {
        ctx.win().fail(&failure);
    }
}

/// `windows.<kind>.close()` in declarations: the close protocol, as from
/// the title bar.
pub fn close_window(ctx: &Ctx, kind: &str) {
    let found = all().into_iter().find(|win| win.kind_id() == kind);
    if let Some(win) = found
        && let Err(failure) = request_close(&win)
    {
        ctx.win().fail(&failure);
    }
}

/// Application actions and their shortcuts.
pub(crate) fn install_actions(application: &adw::Application) {
    let quit = gio::SimpleAction::new("quit", None);
    let quitting = application.clone();
    quit.connect_activate(move |_, _| quitting.quit());
    application.add_action(&quit);
    application.set_accels_for_action("app.quit", &["<Control>q"]);
    application.set_accels_for_action("window.close", &["<Control>w"]);
}

/// A window explaining why the engine could not start.
pub(crate) fn show_startup_error(application: &adw::Application, message: &str) {
    let window = adw::ApplicationWindow::new(application);
    window.set_default_size(480, 320);
    let page = adw::StatusPage::new();
    page.set_icon_name(Some("dialog-error-symbolic"));
    page.set_title(&tr("bern.startup.failed"));
    page.set_description(Some(message));
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&page));
    window.set_content(Some(&view));
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::tests::{app, start};
    use crate::protocol::{Type, session_catalog};
    use gtk::gdk::{Key, ModifierType};

    #[test]
    fn shortcuts_parse_into_modifiers_and_a_key() {
        assert_eq!(
            parse_keys("cmd+shift+k"),
            Keys {
                modifiers: ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK,
                key: "k".into(),
            }
        );
        assert_eq!(
            parse_keys("ctrl+alt+delete"),
            Keys {
                modifiers: ModifierType::CONTROL_MASK | ModifierType::ALT_MASK,
                key: "delete".into(),
            }
        );
        assert_eq!(
            parse_keys("space"),
            Keys {
                modifiers: ModifierType::empty(),
                key: "space".into(),
            }
        );
        // `cmd` is Control here.
        assert_eq!(parse_keys("cmd+s"), parse_keys("ctrl+s"));
    }

    #[test]
    fn pressed_keys_are_named_as_shortcuts_write_them() {
        for (key, name) in [
            (Key::A, "a"),
            (Key::k, "k"),
            (Key::_1, "1"),
            (Key::slash, "/"),
            (Key::space, "space"),
            (Key::Escape, "escape"),
            (Key::Return, "return"),
            (Key::KP_Enter, "return"),
            (Key::ISO_Left_Tab, "tab"),
            (Key::BackSpace, "delete"),
            (Key::Delete, "delete"),
            (Key::Page_Down, "pagedown"),
            (Key::End, "end"),
        ] {
            assert_eq!(key_name(key).as_deref(), Some(name), "{name}");
        }
        // Keys that type nothing have no name.
        assert_eq!(key_name(Key::F1), None);
        assert_eq!(key_name(Key::Shift_L), None);
    }

    #[test]
    fn close_results_match_the_session_catalog() {
        let (types, _) = session_catalog();
        let ty = Type::Record {
            name: "CloseResult".into(),
        };
        let outcomes = &types.enum_def("CloseOutcome").expect("outcomes").variants;
        for (outcome, name) in [
            (CLOSED, "Closed"),
            (REJECTED, "Rejected"),
            (CONFIRMATION_REQUIRED, "ConfirmationRequired"),
        ] {
            assert_eq!(outcomes[outcome as usize].name, name);
        }
        let rejected = CloseResult::rejected("main#1", 4, "busy".into()).to_value();
        rejected.check(&ty, &types).expect("a CloseResult");
        assert_eq!(
            rejected,
            Value::Record(vec![
                Value::unit_variant(REJECTED),
                Value::Text("main#1".into()),
                Value::none(),
                Value::Int(4),
                Value::List(Vec::new()),
                Value::none(),
                Value::some(Value::Text("busy".into())),
            ])
        );
        let waiting = CloseResult {
            outcome: CONFIRMATION_REQUIRED,
            confirmation: Some("close-1".into()),
            decisions: vec![0, 1, 2],
            message: Some("Save the draft?".into()),
            ..CloseResult::closed("main#1", 5)
        };
        waiting
            .to_value()
            .check(&ty, &types)
            .expect("a CloseResult");
    }

    #[test]
    fn window_actions_match_the_session_catalog() {
        let (types, _) = session_catalog();
        let actions = &types.enum_def("WindowAction").expect("actions").variants;
        for (action, name) in [
            (Action::Activate, "Activate"),
            (Action::Close, "Close"),
            (Action::Minimize, "Minimize"),
            (Action::Restore, "Restore"),
            (Action::Maximize, "Maximize"),
            (Action::Resize, "Resize"),
        ] {
            assert_eq!(actions[action as usize].name, name);
        }
    }

    /// Start a host whose windows own one draft: a title and whether it is
    /// done.
    fn drafts() {
        let mut def = app();
        def.sessions = vec![crate::SessionDef {
            name: "draft",
            fields: vec![
                ("title", Type::Text, Value::Text(String::new())),
                ("done", Type::Bool, Value::Bool(false)),
            ],
        }];
        start(def);
    }

    /// A registered instance: success, generation, id, then each session
    /// slot and its values.
    fn instance(session: u32) -> Writer {
        let mut writer = Writer::new();
        writer.u8(0);
        writer.u64(2);
        writer.text("editor#2");
        writer.u32(1);
        writer.u32(session);
        writer.text("Notes");
        writer.u8(1);
        writer
    }

    #[test]
    fn new_instances_decode_their_drafts() {
        drafts();
        let (generation, id, cells) = decode_instance(&instance(0).bytes).expect("an instance");
        assert_eq!((generation, id.as_str()), (2, "editor#2"));
        let values: Vec<Value> = cells[&0].iter().map(Cell::peek).collect();
        assert_eq!(values, [Value::Text("Notes".into()), Value::Bool(true)]);
    }

    #[test]
    fn malformed_instances_fail() {
        drafts();
        assert_eq!(
            decode_instance(&instance(3).bytes).err(),
            Some(Failure::new("invalid_reply", "unknown session type"))
        );
        let mut truncated = instance(0).bytes;
        truncated.pop();
        assert_eq!(
            decode_instance(&truncated)
                .err()
                .map(|failure| failure.code),
            Some("invalid_reply".to_string())
        );
        let mut refused = Writer::new();
        refused.u8(1);
        refused.text("window_limit");
        refused.text("Only one editor can be open");
        assert_eq!(
            decode_instance(&refused.bytes).err(),
            Some(Failure::new("window_limit", "Only one editor can be open"))
        );
    }
}
