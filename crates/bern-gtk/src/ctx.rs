//! What a component body reads: its window, the window's drafts, the
//! component name for control targets, the row key inside lists, local
//! variables, and the page whose header shows titles, back and tools.

use crate::protocol::Value;
use crate::reactive::{Version, on_cleanup};
use crate::windows::Win;
use crate::{Get, host};
use gtk::prelude::*;
use std::cell::{Cell as StdCell, RefCell};
use std::rc::Rc;

/// A writable session field bound to a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// Global index of the session type.
    pub session: usize,
    pub field: usize,
}

struct Local {
    name: &'static str,
    get: Rc<dyn Fn() -> Value>,
    next: Option<Rc<Local>>,
}

struct Inner {
    win: Rc<Win>,
    comp: &'static str,
    /// The key of every repeated row the content is in, outermost first.
    keys: Vec<Rc<dyn Fn() -> String>>,
    locals: Option<Rc<Local>>,
    page: Option<Rc<Page>>,
    toolbar: bool,
    /// The route path of the enclosing navigation.
    nav: Option<Field>,
    /// Whether this content is showing, for its page tools.
    shown: Option<crate::reactive::Cell<bool>>,
    /// The scroll view the content is in.
    scroll: Option<Rc<crate::features::ScrollScope>>,
    /// The sidebar whose items the content lists.
    sidebar: Option<Rc<crate::structure::Registry>>,
    /// The tab view whose tabs the content declares.
    tabs: Option<Rc<crate::structure::TabRegistry>>,
    /// The row layout of the form the content is in.
    form: Option<Rc<crate::widgets::RowLayout>>,
    /// The search palette whose results the content lists.
    palette: Option<Rc<crate::palette::Palette>>,
}

/// The context generated code evaluates expressions in.
#[derive(Clone)]
pub struct Ctx(Rc<Inner>);

impl Ctx {
    /// The context of a window's own content: controls written in the
    /// window itself are named after it.
    pub(crate) fn root(win: Rc<Win>, page: Option<Rc<Page>>) -> Self {
        Self(Rc::new(Inner {
            comp: win.kind_id(),
            win,
            keys: Vec::new(),
            locals: None,
            page,
            toolbar: false,
            nav: None,
            shown: None,
            scroll: None,
            sidebar: None,
            tabs: None,
            form: None,
            palette: None,
        }))
    }

    fn with(&self, change: impl FnOnce(&mut Inner)) -> Self {
        let mut inner = Inner {
            win: Rc::clone(&self.0.win),
            comp: self.0.comp,
            keys: self.0.keys.clone(),
            locals: self.0.locals.clone(),
            page: self.0.page.clone(),
            toolbar: self.0.toolbar,
            nav: self.0.nav,
            shown: self.0.shown.clone(),
            scroll: self.0.scroll.clone(),
            sidebar: self.0.sidebar.clone(),
            tabs: self.0.tabs.clone(),
            form: self.0.form.clone(),
            palette: self.0.palette.clone(),
        };
        change(&mut inner);
        Self(Rc::new(inner))
    }

    pub(crate) fn win(&self) -> &Rc<Win> {
        &self.0.win
    }

    pub(crate) fn page(&self) -> Option<&Rc<Page>> {
        self.0.page.as_ref()
    }

    pub(crate) fn in_toolbar(&self) -> bool {
        self.0.toolbar
    }

    /// Engine state field, recording a dependency.
    pub fn engine(&self, field: usize) -> Value {
        host::engine_value(field)
    }

    /// A field of the window's session drafts, recording a dependency.
    pub fn session(&self, session: usize, field: usize) -> Value {
        self.0.win.cell(Field { session, field }).get()
    }

    /// UI state of the window, such as `window.idle` and `window.compact`,
    /// recording a dependency.
    pub fn window(&self, field: &str) -> Value {
        self.0.win.state(field)
    }

    /// A local variable: a component parameter, row variable or pattern
    /// binding.
    pub fn local(&self, name: &str) -> Value {
        let mut local = self.0.locals.as_deref();
        while let Some(current) = local {
            if current.name == name {
                return (current.get)();
            }
            local = current.next.as_deref();
        }
        Value::Unit
    }

    /// The context of a referenced component: its own name and parameters,
    /// evaluated in this context.
    pub fn child(&self, comp: &'static str, params: Vec<(&'static str, Get)>) -> Self {
        let parent = self.clone();
        let mut locals = None;
        for (name, get) in params {
            let at = parent.clone();
            locals = Some(Rc::new(Local {
                name,
                get: Rc::new(move || get(&at)),
                next: locals,
            }));
        }
        // Row keys stay: a control in a component in a row is that row's.
        self.with(|inner| {
            inner.comp = comp;
            inner.locals = locals;
        })
    }

    pub(crate) fn with_local(&self, name: &'static str, get: Rc<dyn Fn() -> Value>) -> Self {
        let next = self.0.locals.clone();
        self.with(|inner| {
            inner.locals = Some(Rc::new(Local { name, get, next }));
        })
    }

    pub(crate) fn with_key(&self, key: Rc<dyn Fn() -> String>) -> Self {
        self.with(|inner| inner.keys.push(key))
    }

    pub(crate) fn with_scroll(&self, scroll: Rc<crate::features::ScrollScope>) -> Self {
        self.with(|inner| inner.scroll = Some(scroll))
    }

    pub(crate) fn scroll(&self) -> Option<&Rc<crate::features::ScrollScope>> {
        self.0.scroll.as_ref()
    }

    pub(crate) fn with_sidebar(&self, sidebar: Rc<crate::structure::Registry>) -> Self {
        self.with(|inner| inner.sidebar = Some(sidebar))
    }

    pub(crate) fn sidebar(&self) -> Option<&Rc<crate::structure::Registry>> {
        self.0.sidebar.as_ref()
    }

    pub(crate) fn with_tabs(&self, tabs: Rc<crate::structure::TabRegistry>) -> Self {
        self.with(|inner| inner.tabs = Some(tabs))
    }

    pub(crate) fn tabs(&self) -> Option<&Rc<crate::structure::TabRegistry>> {
        self.0.tabs.as_ref()
    }

    pub(crate) fn with_form(&self, form: Rc<crate::widgets::RowLayout>) -> Self {
        self.with(|inner| inner.form = Some(form))
    }

    pub(crate) fn form(&self) -> Option<&Rc<crate::widgets::RowLayout>> {
        self.0.form.as_ref()
    }

    pub(crate) fn with_palette(&self, palette: Rc<crate::palette::Palette>) -> Self {
        self.with(|inner| inner.palette = Some(palette))
    }

    pub(crate) fn palette(&self) -> Option<&Rc<crate::palette::Palette>> {
        self.0.palette.as_ref()
    }

    pub(crate) fn with_page(&self, page: Rc<Page>) -> Self {
        self.with(|inner| inner.page = Some(page))
    }

    pub(crate) fn with_nav(&self, path: Field) -> Self {
        self.with(|inner| inner.nav = Some(path))
    }

    pub(crate) fn with_shown(&self, shown: crate::reactive::Cell<bool>) -> Self {
        self.with(|inner| inner.shown = Some(shown))
    }

    /// Whether this content is showing; content under a pushed
    /// destination is not.
    pub(crate) fn shown(&self) -> bool {
        self.0.shown.as_ref().is_none_or(|shown| shown.get())
    }

    pub(crate) fn nav(&self) -> Option<Field> {
        self.0.nav
    }

    pub(crate) fn toolbar(&self) -> Self {
        self.with(|inner| inner.toolbar = true)
    }

    /// The context of content outside the toolbar, such as search results.
    pub(crate) fn outside_toolbar(&self) -> Self {
        self.with(|inner| inner.toolbar = false)
    }

    /// The control target of `id` in this context: `component.id`, with the
    /// row key inside lists.
    pub(crate) fn target(&self, id: &str) -> String {
        let mut target = format!("{}.{id}", self.0.comp);
        for key in &self.0.keys {
            target.push_str(&format!("[{}]", key()));
        }
        target
    }

    /// Write a session field of this window: the session runtime first,
    /// then the cell. Returns false when the runtime refused the write.
    pub(crate) fn write(&self, field: Field, value: Value) -> bool {
        self.0.win.write(field, value)
    }

    pub(crate) fn read(&self, field: Field) -> Value {
        self.0.win.cell(field).get()
    }

    pub(crate) fn peek(&self, field: Field) -> Value {
        self.0.win.cell(field).peek()
    }
}

/// A page with a header: titles, back actions, title widgets and tools
/// registered by what it shows. The innermost registration wins.
pub struct Page {
    titles: RefCell<Vec<(u64, crate::TextFn)>>,
    backs: RefCell<Vec<(u64, crate::Thunk)>>,
    title_widgets: RefCell<Vec<(u64, gtk::Widget)>>,
    hidden: RefCell<Vec<u64>>,
    pub(crate) tools: gtk::Box,
    /// Toolbar items at the leading end, after the way back.
    pub(crate) leading_tools: gtk::Box,
    tool_count: StdCell<usize>,
    pub(crate) version: Version,
    next: StdCell<u64>,
}

impl Page {
    pub(crate) fn new() -> Rc<Self> {
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let leading_tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        Rc::new(Self {
            titles: RefCell::new(Vec::new()),
            backs: RefCell::new(Vec::new()),
            title_widgets: RefCell::new(Vec::new()),
            hidden: RefCell::new(Vec::new()),
            tools,
            leading_tools,
            tool_count: StdCell::new(0),
            version: Version::new(),
            next: StdCell::new(1),
        })
    }

    fn token(&self) -> u64 {
        let token = self.next.get();
        self.next.set(token + 1);
        token
    }

    fn register<T: 'static>(
        self: &Rc<Self>,
        list: impl Fn(&Page) -> &RefCell<Vec<(u64, T)>> + 'static,
        item: T,
    ) {
        let token = self.token();
        list(self).borrow_mut().push((token, item));
        self.version.bump();
        let page = Rc::clone(self);
        on_cleanup(move || {
            list(&page).borrow_mut().retain(|(id, _)| *id != token);
            page.version.bump();
        });
    }

    /// A title for this page until the current scope ends.
    pub(crate) fn push_title(self: &Rc<Self>, title: Rc<dyn Fn() -> String>) {
        self.register(|page| &page.titles, title);
    }

    pub(crate) fn push_back(self: &Rc<Self>, back: Rc<dyn Fn()>) {
        self.register(|page| &page.backs, back);
    }

    pub(crate) fn push_title_widget(self: &Rc<Self>, widget: gtk::Widget) {
        self.register(|page| &page.title_widgets, widget);
    }

    /// Hide this page's header while the current scope lasts, for content
    /// that brings headers of its own.
    pub(crate) fn hide_header(self: &Rc<Self>) {
        let token = self.token();
        self.hidden.borrow_mut().push(token);
        self.version.bump();
        let page = Rc::clone(self);
        on_cleanup(move || {
            page.hidden.borrow_mut().retain(|id| *id != token);
            page.version.bump();
        });
    }

    /// A holder for toolbar items, removed when the current scope ends;
    /// `leading` ones stand after the way back.
    pub(crate) fn add_tools(self: &Rc<Self>, leading: bool) -> gtk::Box {
        let holder = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let area = if leading {
            self.leading_tools.clone()
        } else {
            self.tools.clone()
        };
        area.append(&holder);
        self.tool_count.set(self.tool_count.get() + 1);
        self.version.bump();
        let page = Rc::clone(self);
        let removed = holder.clone();
        on_cleanup(move || {
            area.remove(&removed);
            page.tool_count.set(page.tool_count.get() - 1);
            page.version.bump();
        });
        holder
    }

    pub(crate) fn title(&self) -> Option<String> {
        let last = self
            .titles
            .borrow()
            .last()
            .map(|(_, title)| Rc::clone(title));
        last.map(|title| title())
    }

    pub(crate) fn back(&self) -> Option<Rc<dyn Fn()>> {
        self.backs.borrow().last().map(|(_, back)| Rc::clone(back))
    }

    pub(crate) fn title_widget(&self) -> Option<gtk::Widget> {
        self.title_widgets
            .borrow()
            .last()
            .map(|(_, widget)| widget.clone())
    }

    pub(crate) fn header_hidden(&self) -> bool {
        !self.hidden.borrow().is_empty()
    }
}

/// `draft.field.set(value)`: an action that changes a session draft. A
/// whole number set on a `f64` field is that number.
pub fn session_set(ctx: &Ctx, field: Field, value: Value) {
    let host = host::host();
    let ty = host
        .def
        .sessions
        .get(field.session)
        .and_then(|session| session.fields.get(field.field))
        .map(|(_, ty, _)| ty);
    let value = match (ty, value) {
        (Some(crate::protocol::Type::Float), Value::Int(number)) => Value::Float(number as f64),
        (_, value) => value,
    };
    ctx.write(field, value);
}

/// `draft.field.toggle()` on a `bool`.
pub fn session_toggle(ctx: &Ctx, field: Field) {
    let on = ctx.peek(field) == Value::Bool(true);
    ctx.write(field, Value::Bool(!on));
}
