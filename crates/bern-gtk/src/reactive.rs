//! Reactive cells, effects and scopes on the GTK main thread.
//!
//! The same model as the Web runtime: an effect runs now and again
//! whenever a cell it read changes; scopes own effects, cleanups and child
//! scopes, and disposing a scope ends them all. Effects created while an
//! effect runs belong to that run and end before the next one.

use std::cell::{Cell as StdCell, RefCell};
use std::rc::{Rc, Weak};

thread_local! {
    static OBSERVER: RefCell<Option<Rc<EffectInner>>> = const { RefCell::new(None) };
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
    static DEPTH: StdCell<usize> = const { StdCell::new(0) };
    static QUEUED: RefCell<Vec<Rc<EffectInner>>> = const { RefCell::new(Vec::new()) };
}

struct CellInner<T> {
    value: RefCell<T>,
    readers: RefCell<Vec<Weak<EffectInner>>>,
}

/// A cell an effect read; it forgets the effect when the effect re-runs.
trait Source {
    fn forget(&self, effect: *const EffectInner);
}

impl<T> Source for CellInner<T> {
    fn forget(&self, effect: *const EffectInner) {
        self.readers
            .borrow_mut()
            .retain(|reader| reader.as_ptr() != effect && reader.strong_count() > 0);
    }
}

/// A value whose readers re-run when it changes.
pub struct Cell<T>(Rc<CellInner<T>>);

impl<T> Clone for Cell<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Clone + PartialEq + 'static> Cell<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(CellInner {
            value: RefCell::new(value),
            readers: RefCell::new(Vec::new()),
        }))
    }

    /// The value, recording a dependency of the running effect.
    pub fn get(&self) -> T {
        self.track();
        self.0.value.borrow().clone()
    }

    /// Read without recording a dependency.
    pub fn peek(&self) -> T {
        self.0.value.borrow().clone()
    }

    /// Read a part of the value, recording a dependency.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.track();
        read(&self.0.value.borrow())
    }

    fn track(&self) {
        let Some(effect) = OBSERVER.with(|observer| observer.borrow().clone()) else {
            return;
        };
        let me = Rc::as_ptr(&effect);
        let mut readers = self.0.readers.borrow_mut();
        if readers.iter().any(|reader| reader.as_ptr() == me) {
            return;
        }
        readers.push(Rc::downgrade(&effect));
        let source: Rc<dyn Source> = self.0.clone();
        effect.sources.borrow_mut().push(Rc::downgrade(&source));
    }

    /// Change the value; readers re-run when it differs.
    pub fn set(&self, value: T) {
        if *self.0.value.borrow() == value {
            return;
        }
        *self.0.value.borrow_mut() = value;
        self.notify();
    }

    /// Change the value in place and re-run readers.
    pub fn update(&self, change: impl FnOnce(&mut T)) {
        change(&mut self.0.value.borrow_mut());
        self.notify();
    }

    fn notify(&self) {
        let readers: Vec<_> = self
            .0
            .readers
            .borrow_mut()
            .drain(..)
            .filter_map(|reader| reader.upgrade())
            .collect();
        for reader in readers {
            reader.schedule();
        }
    }
}

struct ScopeInner {
    cleanups: RefCell<Vec<Box<dyn FnOnce()>>>,
    disposed: StdCell<bool>,
}

/// Owns effects, cleanups and child scopes.
#[derive(Clone)]
pub struct Scope(Rc<ScopeInner>);

impl Scope {
    /// A scope owned by the current one: it ends when the current one does.
    pub fn new() -> Self {
        let scope = Self::detached();
        if let Some(parent) = current_scope() {
            let child = scope.clone();
            parent.on_dispose(move || child.dispose());
        }
        scope
    }

    /// A scope with no owner; the caller disposes it.
    pub fn detached() -> Self {
        Self(Rc::new(ScopeInner {
            cleanups: RefCell::new(Vec::new()),
            disposed: StdCell::new(false),
        }))
    }

    pub fn on_dispose(&self, cleanup: impl FnOnce() + 'static) {
        if self.0.disposed.get() {
            cleanup();
        } else {
            self.0.cleanups.borrow_mut().push(Box::new(cleanup));
        }
    }

    pub fn dispose(&self) {
        if self.0.disposed.replace(true) {
            return;
        }
        let cleanups: Vec<_> = self.0.cleanups.borrow_mut().drain(..).collect();
        for cleanup in cleanups.into_iter().rev() {
            cleanup();
        }
    }

    pub fn is_disposed(&self) -> bool {
        self.0.disposed.get()
    }

    /// Run `body` with this scope as the current one.
    pub fn run<R>(&self, body: impl FnOnce() -> R) -> R {
        let saved = SCOPE.with(|scope| scope.replace(Some(self.clone())));
        let result = body();
        SCOPE.with(|scope| *scope.borrow_mut() = saved);
        result
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

fn current_scope() -> Option<Scope> {
    SCOPE.with(|scope| scope.borrow().clone())
}

/// Register a cleanup with the current scope.
pub fn on_cleanup(cleanup: impl FnOnce() + 'static) {
    if let Some(scope) = current_scope() {
        scope.on_dispose(cleanup);
    }
}

struct EffectInner {
    body: Box<dyn Fn()>,
    sources: RefCell<Vec<Weak<dyn Source>>>,
    inner: RefCell<Option<Scope>>,
    disposed: StdCell<bool>,
    queued: StdCell<bool>,
}

impl EffectInner {
    fn schedule(self: Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        if DEPTH.with(StdCell::get) > 0 {
            if !self.queued.replace(true) {
                QUEUED.with(|queued| queued.borrow_mut().push(self));
            }
        } else {
            self.run();
        }
    }

    fn run(self: &Rc<Self>) {
        self.queued.set(false);
        if self.disposed.get() {
            return;
        }
        self.clear_sources();
        // Scopes created by the previous run end before the next one.
        let previous = self.inner.borrow_mut().take();
        if let Some(previous) = previous {
            previous.dispose();
        }
        let inner = Scope::detached();
        *self.inner.borrow_mut() = Some(inner.clone());
        let saved = OBSERVER.with(|observer| observer.replace(Some(Rc::clone(self))));
        inner.run(|| (self.body)());
        OBSERVER.with(|observer| *observer.borrow_mut() = saved);
    }

    fn clear_sources(&self) {
        let me = self as *const EffectInner;
        let sources: Vec<_> = self.sources.borrow_mut().drain(..).collect();
        for source in sources {
            if let Some(source) = source.upgrade() {
                source.forget(me);
            }
        }
    }

    fn dispose(&self) {
        self.disposed.set(true);
        self.clear_sources();
        let inner = self.inner.borrow_mut().take();
        if let Some(inner) = inner {
            inner.dispose();
        }
    }
}

/// Run `body` now and again whenever a cell it read changes, until the
/// current scope ends.
pub fn effect(body: impl Fn() + 'static) {
    let effect = Rc::new(EffectInner {
        body: Box::new(body),
        sources: RefCell::new(Vec::new()),
        inner: RefCell::new(None),
        disposed: StdCell::new(false),
        queued: StdCell::new(false),
    });
    let owned = Rc::clone(&effect);
    on_cleanup(move || owned.dispose());
    effect.run();
}

/// Apply several changes and re-run each affected effect once afterwards.
pub fn batch<R>(body: impl FnOnce() -> R) -> R {
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    let result = body();
    let depth = DEPTH.with(|depth| {
        depth.set(depth.get() - 1);
        depth.get()
    });
    if depth == 0 {
        loop {
            let next: Vec<_> = QUEUED.with(|queued| queued.borrow_mut().drain(..).collect());
            if next.is_empty() {
                break;
            }
            for effect in next {
                effect.run();
            }
        }
    }
    result
}

/// Read cells without recording dependencies.
pub fn untrack<R>(body: impl FnOnce() -> R) -> R {
    let saved = OBSERVER.with(|observer| observer.replace(None));
    let result = body();
    OBSERVER.with(|observer| *observer.borrow_mut() = saved);
    result
}

/// A counter cell for collections whose items cannot be compared.
#[derive(Clone)]
pub struct Version(Cell<u64>);

impl Version {
    pub fn new() -> Self {
        Self(Cell::new(0))
    }

    pub fn track(&self) {
        self.0.get();
    }

    pub fn bump(&self) {
        self.0.update(|value| *value += 1);
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the effects of a test saw, in order.
    type Log = Rc<RefCell<Vec<String>>>;

    fn log() -> Log {
        Rc::new(RefCell::new(Vec::new()))
    }

    fn write(log: &Log, entry: impl Into<String>) {
        log.borrow_mut().push(entry.into());
    }

    #[test]
    fn effects_rerun_when_a_cell_they_read_changes() {
        let scope = Scope::detached();
        let count = Cell::new(1);
        let seen = log();
        let (read, written) = (count.clone(), Rc::clone(&seen));
        scope.run(|| effect(move || write(&written, read.get().to_string())));
        count.set(2);
        // The same value again changes nothing.
        count.set(2);
        count.set(3);
        assert_eq!(*seen.borrow(), ["1", "2", "3"]);
        scope.dispose();
    }

    #[test]
    fn peek_and_untrack_record_no_dependency() {
        let scope = Scope::detached();
        let (a, b) = (Cell::new(0), Cell::new(0));
        let runs = Rc::new(StdCell::new(0));
        let (ra, rb, counted) = (a.clone(), b.clone(), Rc::clone(&runs));
        scope.run(|| {
            effect(move || {
                ra.peek();
                untrack(|| rb.get());
                counted.set(counted.get() + 1);
            })
        });
        a.set(1);
        b.set(1);
        assert_eq!(runs.get(), 1);
        scope.dispose();
    }

    #[test]
    fn effects_follow_only_the_cells_their_last_run_read() {
        let scope = Scope::detached();
        let (which, left, right) = (Cell::new(true), Cell::new("l1"), Cell::new("r1"));
        let seen = log();
        let (w, l, r, written) = (which.clone(), left.clone(), right.clone(), seen.clone());
        scope.run(|| {
            effect(move || write(&written, if w.get() { l.get() } else { r.get() }));
        });
        right.set("r2");
        which.set(false);
        left.set("l2");
        right.set("r3");
        assert_eq!(*seen.borrow(), ["l1", "r2", "r3"]);
        scope.dispose();
    }

    #[test]
    fn batches_run_each_effect_once_after_every_change() {
        let scope = Scope::detached();
        let (a, b) = (Cell::new(0), Cell::new(0));
        let seen = log();
        let (ra, rb, written) = (a.clone(), b.clone(), seen.clone());
        scope.run(|| effect(move || write(&written, format!("{} {}", ra.get(), rb.get()))));
        let result = batch(|| {
            a.set(1);
            b.set(2);
            // A nested batch waits for the outer one.
            batch(|| a.set(3));
            assert_eq!(seen.borrow().len(), 1);
            "done"
        });
        assert_eq!(result, "done");
        assert_eq!(*seen.borrow(), ["0 0", "3 2"]);
        scope.dispose();
    }

    #[test]
    fn update_reruns_readers_without_comparing() {
        let scope = Scope::detached();
        let items = Cell::new(vec![1]);
        let seen = log();
        let (read, written) = (items.clone(), seen.clone());
        scope.run(|| effect(move || write(&written, read.with(Vec::len).to_string())));
        items.update(|items| items.push(2));
        items.update(|_| {});
        assert_eq!(*seen.borrow(), ["1", "2", "2"]);
        scope.dispose();
    }

    #[test]
    fn disposing_a_scope_ends_its_effects_and_runs_cleanups_last_first() {
        let scope = Scope::detached();
        let cell = Cell::new(0);
        let seen = log();
        let (read, written) = (cell.clone(), seen.clone());
        scope.run(|| {
            let (first, second) = (seen.clone(), seen.clone());
            effect(move || write(&written, format!("effect {}", read.get())));
            on_cleanup(move || write(&first, "first"));
            on_cleanup(move || write(&second, "second"));
        });
        scope.dispose();
        cell.set(1);
        // Disposing again does nothing; a cleanup added afterwards runs at
        // once.
        scope.dispose();
        let late = seen.clone();
        scope.on_dispose(move || write(&late, "late"));
        assert!(scope.is_disposed());
        assert_eq!(*seen.borrow(), ["effect 0", "second", "first", "late"]);
    }

    #[test]
    fn effects_end_what_their_previous_run_made() {
        let scope = Scope::detached();
        let (outer, inner) = (Cell::new(0), Cell::new(0));
        let seen = log();
        let (round_of, inner_of, written) = (outer.clone(), inner.clone(), seen.clone());
        scope.run(|| {
            effect(move || {
                let round = round_of.get();
                let (inner, shown, ended) = (inner_of.clone(), written.clone(), written.clone());
                effect(move || write(&shown, format!("inner {round}: {}", inner.get())));
                on_cleanup(move || write(&ended, format!("cleanup {round}")));
            })
        });
        inner.set(1);
        outer.set(1);
        inner.set(2);
        assert_eq!(
            *seen.borrow(),
            [
                "inner 0: 0",
                "inner 0: 1",
                "cleanup 0",
                "inner 1: 1",
                "inner 1: 2"
            ]
        );
        scope.dispose();
    }

    #[test]
    fn child_scopes_end_with_their_owner() {
        let parent = Scope::detached();
        let child = parent.run(Scope::new);
        // Outside any scope, a new scope has no owner.
        let free = Scope::new();
        parent.dispose();
        assert!(child.is_disposed());
        assert!(!free.is_disposed());
    }

    #[test]
    fn versions_rerun_their_trackers_on_every_bump() {
        let scope = Scope::detached();
        let version = Version::new();
        let runs = Rc::new(StdCell::new(0));
        let (tracked, counted) = (version.clone(), Rc::clone(&runs));
        scope.run(|| {
            effect(move || {
                tracked.track();
                counted.set(counted.get() + 1);
            })
        });
        version.bump();
        version.bump();
        batch(|| {
            version.bump();
            version.bump();
        });
        assert_eq!(runs.get(), 4);
        scope.dispose();
    }
}
