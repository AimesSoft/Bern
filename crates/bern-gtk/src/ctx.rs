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
    key: Option<Rc<dyn Fn() -> String>>,
    locals: Option<Rc<Local>>,
    page: Option<Rc<Page>>,
    toolbar: bool,
    /// The route path of the enclosing navigation.
    nav: Option<Field>,
    /// Whether this content is showing, for its page tools.
    shown: Option<crate::reactive::Cell<bool>>,
}

/// The context generated code evaluates expressions in.
#[derive(Clone)]
pub struct Ctx(Rc<Inner>);

impl Ctx {
    pub(crate) fn root(win: Rc<Win>, page: Option<Rc<Page>>) -> Self {
        Self(Rc::new(Inner {
            win,
            comp: "",
            key: None,
            locals: None,
            page,
            toolbar: false,
            nav: None,
            shown: None,
        }))
    }

    fn with(&self, change: impl FnOnce(&mut Inner)) -> Self {
        let mut inner = Inner {
            win: Rc::clone(&self.0.win),
            comp: self.0.comp,
            key: self.0.key.clone(),
            locals: self.0.locals.clone(),
            page: self.0.page.clone(),
            toolbar: self.0.toolbar,
            nav: self.0.nav,
            shown: self.0.shown.clone(),
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
        self.with(|inner| {
            inner.comp = comp;
            inner.key = None;
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
        self.with(|inner| inner.key = Some(key))
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

    /// The control target of `id` in this context: `component.id`, with the
    /// row key inside lists.
    pub(crate) fn target(&self, id: &str) -> String {
        match &self.0.key {
            Some(key) => format!("{}.{id}[{}]", self.0.comp, key()),
            None => format!("{}.{id}", self.0.comp),
        }
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
    tool_count: StdCell<usize>,
    pub(crate) version: Version,
    next: StdCell<u64>,
}

impl Page {
    pub(crate) fn new() -> Rc<Self> {
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        Rc::new(Self {
            titles: RefCell::new(Vec::new()),
            backs: RefCell::new(Vec::new()),
            title_widgets: RefCell::new(Vec::new()),
            hidden: RefCell::new(Vec::new()),
            tools,
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

    /// A holder for toolbar items, removed when the current scope ends.
    pub(crate) fn add_tools(self: &Rc<Self>) -> gtk::Box {
        let holder = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        self.tools.append(&holder);
        self.tool_count.set(self.tool_count.get() + 1);
        self.version.bump();
        let page = Rc::clone(self);
        let removed = holder.clone();
        on_cleanup(move || {
            page.tools.remove(&removed);
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
