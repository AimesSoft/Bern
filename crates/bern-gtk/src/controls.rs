//! Mounted controls by target. External entries operate them through the
//! same closures the user's input runs.

use crate::Ctx;
use crate::reactive::on_cleanup;
use crate::windows::Win;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

/// Selects an option by name; false when there is no such option.
pub(crate) type Select = Rc<dyn Fn(&str) -> bool>;

/// What a control can do, each the same closure its widget runs.
#[derive(Default, Clone)]
pub(crate) struct Actions {
    pub(crate) activate: Option<Rc<dyn Fn()>>,
    pub(crate) edit: Option<Rc<dyn Fn(String)>>,
    pub(crate) submit: Option<Rc<dyn Fn()>>,
    pub(crate) set_bool: Option<Rc<dyn Fn(bool)>>,
    pub(crate) set_number: Option<Rc<dyn Fn(f64)>>,
    /// Select an option by its name; false when there is no such option.
    pub(crate) select: Option<Select>,
    /// Go back; false when there is nothing to go back to.
    pub(crate) back: Option<Rc<dyn Fn() -> bool>>,
    pub(crate) value: Option<Rc<dyn Fn() -> Option<String>>>,
    /// Take the keyboard focus.
    pub(crate) focus: Option<Rc<dyn Fn()>>,
    /// Take a file as if the user chose or dropped it, then act.
    pub(crate) set_file: Option<Rc<dyn Fn(String)>>,
    /// The kinds of file `set_file` takes.
    pub(crate) file_kinds: Option<&'static str>,
}

#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) target: String,
    pub(crate) win: Weak<Win>,
    pub(crate) role: &'static str,
    pub(crate) label: Option<Rc<dyn Fn() -> String>>,
    pub(crate) enabled: Option<Rc<dyn Fn() -> bool>>,
    pub(crate) actions: Actions,
}

thread_local! {
    static CONTROLS: RefCell<BTreeMap<u64, Entry>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

/// Register a control of the context's window until the current scope
/// ends.
pub(crate) fn register(
    ctx: &Ctx,
    id: &str,
    role: &'static str,
    label: Option<Rc<dyn Fn() -> String>>,
    enabled: Option<Rc<dyn Fn() -> bool>>,
    actions: Actions,
) {
    let token = NEXT.with(|next| {
        let token = next.get();
        next.set(token + 1);
        token
    });
    let entry = Entry {
        target: ctx.target(id),
        win: Rc::downgrade(ctx.win()),
        role,
        label,
        enabled,
        actions,
    };
    CONTROLS.with(|controls| controls.borrow_mut().insert(token, entry));
    on_cleanup(move || {
        CONTROLS.with(|controls| controls.borrow_mut().remove(&token));
    });
}

pub(crate) fn entries() -> Vec<Entry> {
    CONTROLS.with(|controls| controls.borrow().values().cloned().collect())
}

/// The mounted control of `id` in the context, as a shortcut reaches it.
pub(crate) fn find(ctx: &Ctx, id: &str) -> Option<Entry> {
    let target = ctx.target(id);
    CONTROLS.with(|controls| {
        controls
            .borrow()
            .values()
            .rev()
            .find(|entry| entry.target == target)
            .cloned()
    })
}
