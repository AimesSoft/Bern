//! What generated content reads while it builds: engine state, the drafts
//! of its window, local variables, the window's size, and where its
//! controls are declared; and the actions and draft writes it runs.

use crate::host::{host, status, take};
use crate::layout::Grow;
use crate::protocol::{Type, Value, Writer, encode};
use crate::session::Control;
use crate::theme::Theme;
use gpui::Rgba;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A draft field of the window's sessions: the session's index in the
/// application's catalog and the field's in the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub session: usize,
    pub field: usize,
}

/// An action a control runs, given the context it runs in.
pub type Act = Rc<dyn Fn(&Ctx)>;

/// An action from a closure.
pub fn act(run: impl Fn(&Ctx) + 'static) -> Act {
    Rc::new(run)
}

/// What a control does when pressed, with the application at hand.
pub type Press = Rc<dyn Fn(&mut gpui::App)>;

/// Content built in a context, each time it shows.
pub type Content<'a> = &'a dyn Fn(&Ctx) -> Vec<crate::El>;

/// How a controlled value reports a change.
pub type Commit = Rc<dyn Fn(&Ctx, Value)>;

/// What a control shows and changes: a draft field, after which its action
/// runs; or a value the engine controls, which a change commits by running
/// the action's command with the new value.
#[derive(Clone)]
pub enum Model {
    Field(Field, Option<Act>),
    Controlled(Value, Commit),
}

impl Model {
    pub(crate) fn value(&self, c: &Ctx) -> Value {
        match self {
            Model::Field(field, _) => c.read(*field),
            Model::Controlled(value, _) => value.clone(),
        }
    }

    pub(crate) fn set(&self, c: &Ctx, value: Value) {
        match self {
            Model::Field(field, then) => {
                if c.write(*field, value)
                    && let Some(then) = then
                {
                    then(c);
                }
            }
            Model::Controlled(_, commit) => commit(c, value),
        }
    }
}

/// A window instance as content sees it: its drafts and its size.
pub(crate) struct Win {
    pub(crate) kind: usize,
    pub(crate) generation: u64,
    /// `main#1`.
    pub(crate) id: Rc<str>,
    /// Draft values by session index; none for sessions the window does
    /// not host.
    drafts: RefCell<Vec<Option<Vec<Value>>>>,
    /// The content's size in points, as the window last drew.
    pub(crate) width: Cell<f32>,
    pub(crate) height: Cell<f32>,
    /// Content runs under a transparent title bar this tall, as the window
    /// last drew.
    pub(crate) title_bar: Cell<f32>,
    /// The frame's shadow around the content (Linux): where the content
    /// starts in the window.
    pub(crate) inset: Cell<f32>,
    /// The room the title bar's own buttons take at the leading and the
    /// trailing end, which a toolbar under the title bar leaves free.
    pub(crate) caption: Cell<(f32, f32)>,
    /// A toolbar stands in the title bar's row: as the window last drew,
    /// which sizes the title bar, and in the frame being built.
    pub(crate) bar_toolbar: Cell<bool>,
    pub(crate) bar_toolbar_built: Cell<bool>,
    /// macOS's window buttons where AppKit put them, before they were
    /// moved, and where they were put last.
    #[cfg(target_os = "macos")]
    pub(crate) lights: Cell<Option<crate::native::Lights>>,
    #[cfg(target_os = "macos")]
    pub(crate) lights_at: Cell<Option<(f32, f32)>>,
    /// Seconds without input after which the window is idle.
    pub(crate) idle_after: Option<f64>,
    pub(crate) last_input: Cell<std::time::Instant>,
    pub(crate) fullscreen: Cell<bool>,
    /// `idle_enabled` is false: the window does not go idle.
    pub(crate) paused: Cell<bool>,
    /// `immersive` holds.
    pub(crate) immersive: Cell<bool>,
}

impl Win {
    pub(crate) fn new(
        kind: usize,
        generation: u64,
        id: Rc<str>,
        drafts: Vec<Option<Vec<Value>>>,
        size: (f32, f32),
        idle_after: Option<f64>,
    ) -> Rc<Self> {
        Rc::new(Win {
            kind,
            generation,
            id,
            drafts: RefCell::new(drafts),
            width: Cell::new(size.0),
            height: Cell::new(size.1),
            title_bar: Cell::new(0.0),
            inset: Cell::new(0.0),
            caption: Cell::new((0.0, 0.0)),
            bar_toolbar: Cell::new(false),
            bar_toolbar_built: Cell::new(false),
            #[cfg(target_os = "macos")]
            lights: Cell::new(None),
            #[cfg(target_os = "macos")]
            lights_at: Cell::new(None),
            idle_after,
            last_input: Cell::new(std::time::Instant::now()),
            fullscreen: Cell::new(false),
            paused: Cell::new(false),
            immersive: Cell::new(false),
        })
    }

    /// Whether the window has gone without input for its `idle_after`.
    pub(crate) fn idle(&self) -> bool {
        !self.paused.get()
            && self
                .idle_after
                .is_some_and(|after| self.last_input.get().elapsed().as_secs_f64() >= after)
    }

    pub(crate) fn read(&self, field: Field) -> Value {
        self.drafts
            .borrow()
            .get(field.session)
            .and_then(|fields| fields.as_ref())
            .and_then(|fields| fields.get(field.field).cloned())
            .unwrap_or(Value::Unit)
    }

    /// Write a draft: the session runtime first, then the window's copy,
    /// which every window then shows. False when the runtime refused it.
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
        // A whole number written to a float field, as a slider's step.
        let value = match (ty, value) {
            (Type::Float, Value::Int(number)) => Value::Float(number as f64),
            (_, value) => value,
        };
        let mut writer = Writer::new();
        encode(&value, ty, &host.def.types, &mut writer);
        // SAFETY: the handle is live while windows are open; the value
        // bytes are valid for this call.
        let bytes = take(unsafe {
            bern_ffi::bern_session_set(
                host.app(),
                self.kind as u32,
                self.generation,
                field.session as u32,
                field.field as u32,
                writer.bytes.as_ptr(),
                writer.bytes.len(),
            )
        });
        match status(&bytes) {
            Ok(_) => {
                self.set(field, value);
                crate::host::redraw();
                true
            }
            Err(failure) => {
                eprintln!("bern: {}: {}", failure.code, failure.message);
                false
            }
        }
    }

    /// Change the window's copy of a draft, as the session runtime did.
    pub(crate) fn set(&self, field: Field, value: Value) {
        if let Some(Some(fields)) = self.drafts.borrow_mut().get_mut(field.session)
            && let Some(slot) = fields.get_mut(field.field)
        {
            *slot = value;
        }
    }
}

struct Local {
    name: &'static str,
    value: Value,
    next: Option<Rc<Local>>,
}

/// The theme's seed in the light and the dark appearance.
pub(crate) type Seeds = (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>);

#[derive(Clone)]
struct Inner {
    theme: Rc<Theme>,
    seeds: Seeds,
    win: Rc<Win>,
    /// The component the controls are written in, or the window kind.
    scope: Rc<str>,
    /// Where each component on the way here is called, outermost first: the
    /// same component called twice gives its controls the same targets, and
    /// this keeps what each draws apart.
    instance: Rc<str>,
    locals: Option<Rc<Local>>,
    /// Row keys, outermost first: a control inside a list is its row's.
    keys: Vec<Rc<str>>,
    /// False inside a disabled `Stack`.
    enabled: bool,
    /// The form the content is in, whose row metrics its sections take.
    form: crate::form::FormProps,
    /// The path of the navigation the content is in, which its links push
    /// onto.
    nav: Option<Field>,
    /// The selection of the sidebar the content is in, which its items set.
    sidebar: Option<Field>,
    /// Inside a `Toolbar`, whose fields keep a width of their own.
    toolbar: bool,
    /// A container with content that shows only while the pointer is over
    /// it (`reveal: hover`).
    reveal: bool,
    /// The color text and icons take where nothing names one: a style's
    /// foreground around them; the theme's primary text color without one.
    foreground: Option<gpui::Rgba>,
    /// The scroll view whose items the rows of the next list are.
    scroll: Option<Rc<str>>,
    /// The scroll view this row is an item of.
    item_of: Option<Rc<str>>,
    /// The scroll view the content is in, and whether it scrolls across.
    scroller: Option<(Rc<str>, bool)>,
    /// Content of a `Stack(under_bars:)`, which reaches under the title bar.
    under_bars: bool,
    /// How the element built with this context takes its container's
    /// space.
    grow: Grow,
    /// The radius of each corner of its box (top-left, top-right,
    /// bottom-right, bottom-left) where it lies on a rounded corner of a
    /// container: what it paints there is rounded too, as GPUI clips only to
    /// rectangles.
    round: Corners,
    /// The same for what it contains: its own corner, or `round` without the
    /// sides its padding keeps clear.
    inner: Corners,
    /// Inside a `Modal`, where there is no page to take a form's title.
    modal: bool,
    /// Where controls list themselves while an external entry looks for
    /// them; none while the window draws.
    controls: Option<Rc<RefCell<Vec<Control>>>>,
    /// The opacity of an element in motion, which the container it builds
    /// takes on its outermost box; not passed on to what it contains.
    fade: Option<Rc<Fade>>,
    /// How blurred what is built here is, in points: the `blur` of the
    /// element and of the containers around it, together.
    blur: f32,
    /// The element built with this context has a `blur` of its own: the
    /// container it builds keeps the blur within its box. Not passed on to
    /// what it contains.
    blur_own: bool,
}

/// An opacity for an element's outermost box, and whether it took it.
pub(crate) struct Fade {
    opacity: f32,
    taken: Cell<bool>,
}

impl Fade {
    pub(crate) fn taken(&self) -> bool {
        self.taken.get()
    }
}

/// Radii of a box's corners: top-left, top-right, bottom-right,
/// bottom-left.
pub type Corners = [f32; 4];

/// The context content builds in.
#[derive(Clone)]
pub struct Ctx(Rc<Inner>);

impl Ctx {
    pub(crate) fn root(
        theme: Rc<Theme>,
        seeds: Seeds,
        win: Rc<Win>,
        scope: &str,
        controls: Option<Rc<RefCell<Vec<Control>>>>,
    ) -> Self {
        Ctx(Rc::new(Inner {
            theme,
            seeds,
            win,
            scope: scope.into(),
            instance: "".into(),
            locals: None,
            keys: Vec::new(),
            enabled: true,
            form: crate::form::FormProps::default(),
            nav: None,
            sidebar: None,
            toolbar: false,
            reveal: false,
            foreground: None,
            scroll: None,
            item_of: None,
            scroller: None,
            under_bars: false,
            grow: Grow::NONE,
            round: [0.0; 4],
            inner: [0.0; 4],
            modal: false,
            controls,
            fade: None,
            blur: 0.0,
            blur_own: false,
        }))
    }

    fn with(&self, change: impl FnOnce(&mut Inner)) -> Self {
        let mut inner = (*self.0).clone();
        inner.fade = None;
        change(&mut inner);
        Ctx(Rc::new(inner))
    }

    /// The context of an element shown at `opacity` while it moves; whether
    /// its container took the opacity tells after it is built.
    pub(crate) fn with_fade(&self, opacity: f32) -> (Self, Rc<Fade>) {
        let fade = Rc::new(Fade {
            opacity,
            taken: Cell::new(false),
        });
        let held = Rc::clone(&fade);
        (self.with(|inner| inner.fade = Some(held)), fade)
    }

    /// `element`, the outermost box of a container, at the opacity its
    /// motion has it.
    pub(crate) fn faded<E: gpui::Styled>(&self, element: E) -> E {
        // What a blurred container draws stays within its box.
        let element = if self.0.blur_own {
            element.overflow_hidden()
        } else {
            element
        };
        match &self.0.fade {
            Some(fade) => {
                fade.taken.set(true);
                element.opacity(fade.opacity)
            }
            None => element,
        }
    }

    /// Engine state field `field`.
    pub fn engine(&self, field: usize) -> Value {
        crate::host::state(field)
    }

    /// A field of the window's drafts.
    pub fn session(&self, session: usize, field: usize) -> Value {
        self.read(Field { session, field })
    }

    /// A local variable: a component parameter, row variable or pattern
    /// binding.
    pub fn local(&self, name: &str) -> Value {
        let mut local = self.0.locals.as_deref();
        while let Some(current) = local {
            if current.name == name {
                return current.value.clone();
            }
            local = current.next.as_deref();
        }
        Value::Unit
    }

    /// UI state of the window: its layout size (`compact`, `medium`,
    /// `expanded`, `large`), `short`; `idle` and `tv` never hold here.
    pub fn window(&self, field: &str) -> Value {
        let def = host().def;
        let width = f64::from(self.0.win.width.get());
        let height = f64::from(self.0.win.height.get());
        let compact = def.compact_below.unwrap_or(600.0);
        let below = |limit: Option<f64>| limit.is_some_and(|limit| width < limit);
        let size = if width < compact {
            "compact"
        } else if def.medium_below.is_some() && below(def.medium_below) {
            "medium"
        } else if def.expanded_below.is_none() || below(def.expanded_below) {
            "expanded"
        } else {
            "large"
        };
        Value::Bool(match field {
            "compact" | "medium" | "expanded" | "large" => field == size,
            "short" => height < compact,
            "keyboard" => true,
            "touch" => crate::native::touchscreen(),
            "idle" => self.0.win.idle(),
            "fullscreen" => self.0.win.fullscreen.get(),
            _ => false,
        })
    }

    /// The context of a referenced component: its own name and parameters.
    pub fn child(&self, component: &str, site: &str, params: Vec<(&'static str, Value)>) -> Self {
        let mut locals = None;
        for (name, value) in params {
            locals = Some(Rc::new(Local {
                name,
                value,
                next: locals,
            }));
        }
        self.with(|inner| {
            inner.scope = component.into();
            inner.instance = format!("{}/{site}", inner.instance).into();
            inner.locals = locals;
        })
    }

    /// The context with a local variable: a row variable or a binding of a
    /// pattern.
    pub fn with_local(&self, name: &'static str, value: Value) -> Self {
        let next = self.0.locals.clone();
        self.with(|inner| {
            inner.locals = Some(Rc::new(Local { name, value, next }));
        })
    }

    /// The context of a list row, by its key.
    pub fn with_key(&self, key: String) -> Self {
        self.with(|inner| inner.keys.push(key.into()))
    }

    /// The context of a list row by its key value; inside a scroll view
    /// that rests on items, the row is one of them.
    pub fn with_item(&self, key: &Value) -> Self {
        self.with(|inner| {
            inner.keys.push(crate::value::key_text(key).into());
            inner.item_of = inner.scroll.take();
        })
    }

    /// The context of a scroll view's content, whose first lists' rows are
    /// its items.
    pub fn in_scroll(&self, scroll: Option<Rc<str>>) -> Self {
        self.with(|inner| inner.scroll = scroll)
    }

    /// The context of the content of the scroll view `key`, across when
    /// `horizontal`, which what follows scrolling there measures itself
    /// against.
    pub fn within_scroll(&self, key: Rc<str>, horizontal: bool) -> Self {
        self.with(|inner| inner.scroller = Some((key, horizontal)))
    }

    pub(crate) fn scroller(&self) -> Option<(Rc<str>, bool)> {
        self.0.scroller.clone()
    }

    /// The context of the layers of a `Stack(under_bars:)`.
    pub fn under_bars(&self) -> Self {
        self.with(|inner| inner.under_bars = true)
    }

    /// Whether this content reaches under the title bar.
    pub(crate) fn is_under_bars(&self) -> bool {
        self.0.under_bars
    }

    pub(crate) fn item_of(&self) -> Option<Rc<str>> {
        self.0.item_of.clone()
    }

    /// The scroll view whose items the rows of a list here are.
    pub(crate) fn scroll_of(&self) -> Option<Rc<str>> {
        self.0.scroll.clone()
    }

    /// The context of content shown in the light or dark appearance
    /// whatever the window's (`appearance: dark`).
    pub fn with_appearance(&self, dark: bool) -> Self {
        let seed = if dark { self.0.seeds.1 } else { self.0.seeds.0 };
        self.with(|inner| {
            inner.theme = crate::theme::of(dark, seed);
            inner.foreground = None;
        })
    }

    /// The context inside something whose foreground is `color`.
    pub(crate) fn with_foreground(&self, color: gpui::Rgba) -> Self {
        self.with(|inner| inner.foreground = Some(color))
    }

    /// The context inside a surface with `style`, whose foreground its
    /// text and icons take.
    pub fn with_surface(&self, style: Option<&'static crate::Style>) -> Self {
        match style.and_then(|style| style.foreground) {
            Some(foreground) => self.with_foreground(crate::layout::color(self, foreground)),
            None => self.clone(),
        }
    }

    /// The color of text and icons that name none.
    pub(crate) fn foreground(&self) -> gpui::Rgba {
        self.0
            .foreground
            .unwrap_or_else(|| self.color("text.primary"))
    }

    /// The context inside a `Stack(enabled:)`.
    pub fn with_enabled(&self, enabled: bool) -> Self {
        self.with(|inner| inner.enabled = inner.enabled && enabled)
    }

    pub(crate) fn enabled(&self) -> bool {
        self.0.enabled
    }

    /// The context of a form's sections.
    pub fn in_form(&self, form: crate::form::FormProps) -> Self {
        self.with(|inner| inner.form = form)
    }

    pub(crate) fn form(&self) -> crate::form::FormProps {
        self.0.form
    }

    /// The context inside a `Navigation` whose path is `path`.
    pub fn in_navigation(&self, path: Field) -> Self {
        self.with(|inner| inner.nav = Some(path))
    }

    pub(crate) fn nav(&self) -> Option<Field> {
        self.0.nav
    }

    /// The context of a sidebar's items, which set `selection`.
    pub fn in_sidebar(&self, selection: Field) -> Self {
        self.with(|inner| inner.sidebar = Some(selection))
    }

    pub(crate) fn sidebar(&self) -> Option<Field> {
        self.0.sidebar
    }

    pub fn in_toolbar(&self, inside: bool) -> Self {
        self.with(|inner| inner.toolbar = inside)
    }

    pub(crate) fn toolbar(&self) -> bool {
        self.0.toolbar
    }

    /// The context of a container with content shown only while the
    /// pointer is over it.
    pub fn revealing(&self) -> Self {
        self.with(|inner| inner.reveal = true)
    }

    pub(crate) fn reveals(&self) -> bool {
        self.0.reveal
    }

    /// The context of one element, with how it takes its container's
    /// space.
    /// The context of a child that lies on the corners in `mask` (bits for
    /// top-left, top-right, bottom-right and bottom-left) of its container.
    pub fn corners(&self, mask: u8) -> Self {
        let mut round = [0.0; 4];
        for (corner, radius) in round.iter_mut().enumerate() {
            if mask & (1 << corner) != 0 {
                *radius = self.0.inner[corner];
            }
        }
        if round == self.0.round && round == self.0.inner && self.0.fade.is_none() {
            return self.clone();
        }
        self.with(|inner| {
            inner.round = round;
            inner.inner = round;
        })
    }

    /// The context of a container's content: its own corner radius, or the
    /// corners it lies on; none on the sides in `padded` (bits for top,
    /// right, bottom and left), which its padding keeps clear.
    pub fn rounding(&self, own: Option<f32>, padded: u8) -> Self {
        let mut corners = own.map_or(self.0.round, |radius| [radius; 4]);
        // Each corner lies between two sides: top-left between left and
        // top, and so on clockwise.
        for (corner, sides) in [(0, 0b1001), (1, 0b0011), (2, 0b0110), (3, 0b1100)] {
            if padded & sides != 0 {
                corners[corner] = 0.0;
            }
        }
        if corners == self.0.inner && self.0.fade.is_none() {
            return self.clone();
        }
        self.with(|inner| inner.inner = corners)
    }

    /// The context of a `Modal`'s content, which lies on its card's corners
    /// unless the card covers a compact window.
    pub fn in_modal(&self, cover_compact: bool) -> Self {
        let covered = cover_compact && crate::value::truthy(&self.window("compact"));
        self.with(|inner| {
            inner.modal = true;
            inner.inner = if covered {
                [0.0; 4]
            } else {
                [crate::overlay::MODAL_CORNER; 4]
            };
        })
    }

    /// Whether the content is in a `Modal`.
    pub(crate) fn in_modal_content(&self) -> bool {
        self.0.modal
    }

    /// The corners of the element's box that lie on rounded corners.
    pub(crate) fn round(&self) -> Corners {
        self.0.round
    }

    pub fn grown(&self, grow: Grow) -> Self {
        if self.0.grow == grow && self.0.fade.is_none() && !self.0.blur_own {
            return self.clone();
        }
        self.with(|inner| {
            inner.grow = grow;
            inner.blur_own = false;
        })
    }

    /// The context of an element blurred with `radius` points (`blur`),
    /// blurred as much as it already was too.
    pub fn blurred(&self, radius: f32) -> Self {
        let radius = radius.max(0.0);
        let fade = self.0.fade.clone();
        self.with(|inner| {
            inner.blur = (inner.blur * inner.blur + radius * radius).sqrt();
            inner.blur_own = true;
            inner.fade = fade;
        })
    }

    /// How blurred what is built here is, in points.
    pub(crate) fn blur(&self) -> f32 {
        self.0.blur
    }

    pub(crate) fn grow(&self) -> Grow {
        self.0.grow
    }

    pub(crate) fn theme(&self) -> &Rc<Theme> {
        &self.0.theme
    }

    pub(crate) fn win(&self) -> &Rc<Win> {
        &self.0.win
    }

    /// The control target of `id` here: `component.id`, with the row keys
    /// inside lists.
    pub(crate) fn target(&self, id: &str) -> String {
        let mut target = format!("{}.{id}", self.0.scope);
        for key in &self.0.keys {
            target.push_str(&format!("[{key}]"));
        }
        target
    }

    /// The key of what the control `id` keeps while it shows (its element
    /// id, whether it is open, under the pointer, scrolled): its target,
    /// apart for each place its component is called.
    pub(crate) fn key(&self, id: &str) -> String {
        format!("{}|{}", self.0.instance, self.target(id))
    }

    /// An element id unique in the window for the control `id`.
    pub(crate) fn element_id(&self, id: &str) -> gpui::ElementId {
        gpui::ElementId::Name(self.key(id).into())
    }

    pub(crate) fn read(&self, field: Field) -> Value {
        self.0.win.read(field)
    }

    pub(crate) fn write(&self, field: Field, value: Value) -> bool {
        self.0.win.write(field, value)
    }

    pub(crate) fn color(&self, role: &str) -> Rgba {
        self.0.theme.color(role)
    }

    /// The color of a recipe slot, such as the `background` of
    /// `button.prominent` at `hover`; the `rest` state's when the state
    /// does not name the slot.
    pub(crate) fn slot(&self, recipe: &str, state: &str, slot: &str) -> Rgba {
        self.0.theme.slot(recipe, state, slot)
    }

    /// List a control for external entries, while they look.
    pub(crate) fn list(&self, control: Control) {
        if let Some(controls) = &self.0.controls {
            controls.borrow_mut().push(control);
        }
    }

    pub(crate) fn listing(&self) -> bool {
        self.0.controls.is_some()
    }
}

/// `draft.field = value`.
pub fn session_set(c: &Ctx, field: Field, value: Value) {
    c.write(field, value);
}

/// `draft.field.toggle()` on a `bool`.
pub fn session_toggle(c: &Ctx, field: Field) {
    let on = c.read(field) == Value::Bool(true);
    c.write(field, Value::Bool(!on));
}

/// `draft.field.push_str(text)`: the text after the field's.
pub fn session_push_str(c: &Ctx, field: Field, text: Value) {
    let mut value = match c.read(field) {
        Value::Text(value) => value,
        _ => String::new(),
    };
    if let Value::Text(text) = text {
        value.push_str(&text);
    }
    c.write(field, Value::Text(value));
}

/// `draft.field.pop()`: the field's text without its last character.
pub fn session_pop(c: &Ctx, field: Field) {
    let mut value = match c.read(field) {
        Value::Text(value) => value,
        _ => String::new(),
    };
    value.pop();
    c.write(field, Value::Text(value));
}
