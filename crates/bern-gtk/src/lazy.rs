//! A grid whose items are one `ForEach` (`LazyGrid`): in a vertical scroll
//! view it builds only the rows near what is in sight, one view's height
//! before and after it; the rows out of sight take the height they had
//! when last built, or that of the first item built. Outside a scroll view
//! it builds every row. Rows are built and dropped outside layout, after
//! the view scrolls or the items change. Laid out as `Grid`: as many equal
//! columns as fit at `min_width`, `gap` apart, rows `row_gap` apart, and
//! with `cells` every row as tall as the columns' width gives.

use crate::features::ScrollScope;
use crate::motion;
use crate::protocol::Value;
use crate::reactive::{Cell, Scope, effect, on_cleanup, untrack};
use crate::value::{as_list, at, key_text};
use crate::widgets::{GridProps, pad, place};
use crate::{Build, Ctx, Get, Parent};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell as Plain, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// A row built: its value, the scope its bindings live in, and the box it
/// is built into.
pub struct Built {
    value: Cell<Value>,
    owner: Scope,
    holder: gtk::Box,
    /// The row's region in its box: clearing it plays the `disappear` of
    /// what is in it.
    parent: Parent,
}

/// A row whose item is gone, playing its `disappear` where it was, with
/// that place.
pub struct Leaving {
    holder: gtk::Box,
    place: (i32, i32, i32, i32),
}

type Make = Box<dyn Fn(&super::LazyGrid, usize) -> Option<Built>>;

mod imp {
    use super::*;

    pub struct LazyGrid {
        pub items: RefCell<Vec<Value>>,
        pub keys: RefCell<Vec<String>>,
        pub built: RefCell<HashMap<String, Built>>,
        /// Heights of built items by key, at the width `laid`; the guess
        /// for the rest.
        pub heights: RefCell<HashMap<String, i32>>,
        pub guess: Plain<i32>,
        pub laid: Plain<i32>,
        pub min_width: Plain<f64>,
        pub gap: Plain<f64>,
        pub row_gap: Plain<Option<f64>>,
        /// `cells`: the ratio and the extra height of every row.
        pub cells: Plain<Option<(f64, f64)>>,
        /// The last layout: columns, their width, where each row starts.
        pub columns: Plain<usize>,
        pub cell: Plain<i32>,
        pub starts: RefCell<Vec<i32>>,
        /// The vertical scroll view it stands in.
        pub view: RefCell<Option<gtk::ScrolledWindow>>,
        pub make: RefCell<Option<Make>>,
        pub posted: Plain<bool>,
        /// Building rows now: a row that registers with its scroll view
        /// can bring a call back here, which waits for later.
        pub updating: Plain<bool>,
        /// Where each built row was last allocated, by key.
        pub places: RefCell<HashMap<String, (i32, i32, i32, i32)>>,
        pub leaving: RefCell<Vec<Leaving>>,
        /// A list from a column: as wide as its widest built row would like.
        pub list: Plain<bool>,
    }

    impl Default for LazyGrid {
        fn default() -> Self {
            Self {
                items: RefCell::default(),
                keys: RefCell::default(),
                built: RefCell::default(),
                heights: RefCell::default(),
                guess: Plain::new(0),
                laid: Plain::new(-1),
                min_width: Plain::new(120.0),
                gap: Plain::new(8.0),
                row_gap: Plain::new(None),
                cells: Plain::new(None),
                columns: Plain::new(1),
                cell: Plain::new(0),
                starts: RefCell::new(vec![0]),
                view: RefCell::new(None),
                make: RefCell::new(None),
                posted: Plain::new(false),
                updating: Plain::new(false),
                places: RefCell::default(),
                leaving: RefCell::default(),
                list: Plain::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LazyGrid {
        const NAME: &'static str = "BernLazyGrid";
        type Type = super::LazyGrid;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for LazyGrid {
        fn dispose(&self) {
            for (_, row) in self.built.borrow_mut().drain() {
                row.owner.dispose();
                row.holder.unparent();
            }
            for gone in self.leaving.borrow_mut().drain(..) {
                gone.holder.unparent();
            }
            self.make.replace(None);
        }
    }

    impl WidgetImpl for LazyGrid {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                // A list wants the width of its widest row built, as the
                // column it came from would: a scroll view that takes its
                // content's width does not shrink it to nothing.
                if self.list.get() {
                    let natural = self
                        .built
                        .borrow()
                        .values()
                        .map(|row| row.holder.measure(gtk::Orientation::Horizontal, -1).1)
                        .max()
                        .unwrap_or(0);
                    return (0, natural, -1, -1);
                }
                return (0, 0, -1, -1);
            }
            let width = if for_size > 0 {
                for_size
            } else {
                self.laid.get().max(0)
            };
            let total = self.obj().lay_out(width);
            (total, total, -1, -1)
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = true;
            *vexpand = false;
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            let grid = self.obj();
            grid.lay_out(width);
            let columns = self.columns.get().max(1);
            let cell = self.cell.get();
            let gap = self.gap.get().round() as i32;
            let row_gap = grid.row_gap();
            let fixed = grid.cell_height(cell);
            let keys = self.keys.borrow();
            let built = self.built.borrow();
            let starts = self.starts.borrow();
            for (i, key) in keys.iter().enumerate() {
                let Some(row) = built.get(key) else { continue };
                let line = i / columns;
                if line + 1 >= starts.len() {
                    continue;
                }
                // In a row of cells, taller content runs into the gap below.
                let (_, natural, _, _) = row.holder.measure(gtk::Orientation::Vertical, cell);
                let tall = starts[line + 1] - row_gap - starts[line];
                let height = fixed.map_or(tall, |fixed| fixed.max(natural));
                let x = (i % columns) as i32 * (cell + gap);
                let at = gtk::gsk::Transform::new()
                    .translate(&gtk::graphene::Point::new(x as f32, starts[line] as f32));
                row.holder.allocate(cell, height.max(0), -1, Some(at));
                self.places
                    .borrow_mut()
                    .insert(key.clone(), (x, starts[line], cell, height.max(0)));
            }
            drop((keys, built, starts));
            // Rows leaving stay where they were until what of them plays
            // `disappear` is gone.
            self.leaving.borrow_mut().retain(|gone| {
                if gone.holder.first_child().is_none() {
                    gone.holder.unparent();
                    return false;
                }
                let (x, y, width, height) = gone.place;
                gone.holder.measure(gtk::Orientation::Vertical, width);
                let at = gtk::gsk::Transform::new()
                    .translate(&gtk::graphene::Point::new(x as f32, y as f32));
                gone.holder.allocate(width, height, -1, Some(at));
                true
            });
            if !grid.current() {
                grid.later();
            }
        }
    }
}

glib::wrapper! {
    pub struct LazyGrid(ObjectSubclass<imp::LazyGrid>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LazyGrid {
    fn row_gap(&self) -> i32 {
        let imp = self.imp();
        imp.row_gap.get().unwrap_or(imp.gap.get()).round() as i32
    }

    fn cell_height(&self, cell: i32) -> Option<i32> {
        let (ratio, extra) = self.imp().cells.get()?;
        (ratio > 0.0).then(|| (f64::from(cell) / ratio + extra).round() as i32)
    }

    /// The columns at `width`, the built items' heights, where each row
    /// starts; the whole height.
    fn lay_out(&self, width: i32) -> i32 {
        let imp = self.imp();
        // Another width wraps text otherwise: heights are measured again.
        if width != imp.laid.get() {
            imp.laid.set(width);
            imp.heights.borrow_mut().clear();
            imp.guess.set(0);
        }
        let gap = imp.gap.get().round() as i32;
        let least = imp.min_width.get().max(1.0).round() as i32;
        let columns = ((width + gap) / (least + gap)).max(1) as usize;
        let cell = ((width - gap * (columns as i32 - 1)) / columns as i32).max(0);
        imp.columns.set(columns);
        imp.cell.set(cell);
        // No width yet, as for a list not laid out: nothing to measure at.
        if cell > 0 {
            for (key, row) in imp.built.borrow().iter() {
                let (_, natural, _, _) = row.holder.measure(gtk::Orientation::Vertical, cell);
                imp.heights.borrow_mut().insert(key.clone(), natural);
                if imp.guess.get() <= 0 && natural > 0 {
                    imp.guess.set(natural);
                }
            }
        }
        let fixed = self.cell_height(cell);
        let row_gap = self.row_gap();
        let keys = imp.keys.borrow();
        let heights = imp.heights.borrow();
        let rows = keys.len().div_ceil(columns);
        let mut starts = vec![0; rows + 1];
        let mut at = 0;
        for (line, start) in starts.iter_mut().enumerate().take(rows) {
            *start = at;
            let tallest = fixed.unwrap_or_else(|| {
                keys[line * columns..((line + 1) * columns).min(keys.len())]
                    .iter()
                    .map(|key| heights.get(key).copied().unwrap_or(imp.guess.get()))
                    .max()
                    .unwrap_or(0)
            });
            at += tallest + row_gap;
        }
        starts[rows] = at;
        drop((keys, heights));
        imp.starts.replace(starts);
        if rows > 0 { at - row_gap } else { 0 }
    }

    /// The first row whose bottom is below `y`.
    fn row_at(&self, y: f64) -> usize {
        let starts = self.imp().starts.borrow();
        let gap = self.row_gap();
        let (mut low, mut high) = (0, starts.len() - 1);
        while low < high {
            let mid = (low + high) / 2;
            if f64::from(starts[mid + 1] - gap) <= y {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low
    }

    /// The rows to build, from the first to before the last.
    fn wanted(&self) -> (usize, usize) {
        let imp = self.imp();
        let rows = imp.starts.borrow().len() - 1;
        let Some(view) = imp.view.borrow().clone() else {
            return (0, rows);
        };
        let first = rows.min(24usize.div_ceil(imp.columns.get().max(1)));
        let known = imp.guess.get() > 0 || imp.cells.get().is_some();
        let content = view
            .child()
            .and_then(|viewport| viewport.first_child().or(Some(viewport)));
        let origin = content
            .and_then(|content| self.compute_point(&content, &gtk::graphene::Point::new(0.0, 0.0)));
        let (Some(origin), true, true) = (origin, known, imp.laid.get() >= 0) else {
            return (0, first);
        };
        let adjustment = view.vadjustment();
        let top = adjustment.value() - f64::from(origin.y());
        let page = adjustment.page_size();
        let from = self.row_at((top - page).max(0.0)).min(rows);
        let to = (self.row_at((top + page * 2.0).max(0.0)) + 1).min(rows);
        (from, to.max(from))
    }

    /// Whether the rows built are the rows wanted.
    fn current(&self) -> bool {
        let imp = self.imp();
        let (from, to) = self.wanted();
        let columns = imp.columns.get().max(1);
        let keys = imp.keys.borrow();
        let end = keys.len().min(to * columns);
        let start = end.min(from * columns);
        let built = imp.built.borrow();
        built.len() == end - start && keys[start..end].iter().all(|key| built.contains_key(key))
    }

    /// Build the rows wanted and drop the rest.
    fn update(&self) {
        let imp = self.imp();
        if imp.updating.get() {
            self.later();
            return;
        }
        imp.posted.set(false);
        if imp.make.borrow().is_none() {
            return;
        }
        imp.updating.set(true);
        let (from, to) = self.wanted();
        let columns = imp.columns.get().max(1);
        let wanted: Vec<(usize, String)> = {
            let keys = imp.keys.borrow();
            let end = keys.len().min(to * columns);
            (end.min(from * columns)..end)
                .map(|i| (i, keys[i].clone()))
                .collect()
        };
        let mut previous = imp.built.take();
        let mut next = HashMap::new();
        for (i, key) in wanted {
            if let Some(row) = previous.remove(&key) {
                next.insert(key, row);
            } else if let Some(row) = imp.make.borrow().as_ref().and_then(|make| make(self, i)) {
                next.insert(key, row);
            }
        }
        // A row whose item is gone plays its `disappear` where it was; a row
        // only scrolled out of range goes at once.
        let present: std::collections::HashSet<String> =
            imp.keys.borrow().iter().cloned().collect();
        for (key, row) in previous {
            row.owner.dispose();
            match imp.places.borrow_mut().remove(&key) {
                Some(place) if !present.contains(&key) => {
                    row.parent.clear();
                    imp.leaving.borrow_mut().push(Leaving {
                        holder: row.holder,
                        place,
                    });
                }
                _ => row.holder.unparent(),
            }
        }
        imp.built.replace(next);
        imp.updating.set(false);
        self.queue_resize();
    }

    /// `update` once layout is done: rows are not added while it runs.
    fn later(&self) {
        let imp = self.imp();
        if imp.posted.replace(true) {
            return;
        }
        let grid = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(grid) = grid.upgrade() {
                grid.update();
            }
        });
    }

    /// Brings the row of `key` near the view, built, and gives its first
    /// widget, for a scroll view's `position`.
    fn reveal(&self, key: &str) -> Option<gtk::Widget> {
        let imp = self.imp();
        let view = imp.view.borrow().clone()?;
        let at = imp.keys.borrow().iter().position(|one| one == key)?;
        let line = at / imp.columns.get().max(1);
        let start = *imp.starts.borrow().get(line)?;
        let content = view
            .child()
            .and_then(|viewport| viewport.first_child().or(Some(viewport)))?;
        let origin = self.compute_point(&content, &gtk::graphene::Point::new(0.0, 0.0))?;
        view.vadjustment()
            .set_value(f64::from(origin.y()) + f64::from(start));
        self.update();
        imp.built
            .borrow()
            .get(key)
            .and_then(|row| row.holder.first_child())
    }
}

/// `Grid` whose items are one `ForEach`: in a vertical scroll view it
/// builds only the rows near what is in sight; elsewhere it builds them
/// all. A built row keeps its widgets, controls and state while its key
/// stays and it is near the view; its value is a cell the row's bindings
/// read. Rows added in an update play their transitions, in turn with
/// `stagger`, as `Each`'s do; rows built as the view scrolls do not. A list
/// from a column (`align`) places its rows' content across as the column
/// would, and takes no place, nor a gap, while it has no rows.
#[allow(non_snake_case, clippy::too_many_arguments)]
pub fn LazyGrid(
    p: &Parent,
    ctx: &Ctx,
    props: GridProps,
    items: Get,
    key: usize,
    var: &'static str,
    stagger: Option<f64>,
    align: Option<&'static str>,
    row: Build,
) {
    let grid: LazyGrid = glib::Object::new();
    let imp = grid.imp();
    imp.list.set(align.is_some());
    imp.min_width.set(
        props
            .min_width
            .as_ref()
            .map_or(120.0, |width| width.peek(ctx)),
    );
    imp.cells.set(
        props
            .cells
            .as_ref()
            .map(|(ratio, extra)| (*ratio, extra.peek(ctx))),
    );
    let relayout = {
        let grid = grid.downgrade();
        move || {
            if let Some(grid) = grid.upgrade() {
                grid.queue_resize();
            }
        }
    };
    if let Some(min_width) = &props.min_width {
        let (weak, relayout) = (grid.downgrade(), relayout.clone());
        motion::bind_number(ctx, min_width, props.animate, &grid, move |value| {
            if let Some(grid) = weak.upgrade() {
                grid.imp().min_width.set(value.max(1.0));
            }
            relayout();
        });
    }
    if let Some(gap) = &props.gap {
        let (weak, relayout) = (grid.downgrade(), relayout.clone());
        motion::bind_number(ctx, gap, props.animate, &grid, move |value| {
            if let Some(grid) = weak.upgrade() {
                grid.imp().gap.set(value.max(0.0));
            }
            relayout();
        });
    }
    if let Some(row_gap) = &props.row_gap {
        let (weak, relayout) = (grid.downgrade(), relayout.clone());
        motion::bind_number(ctx, row_gap, props.animate, &grid, move |value| {
            if let Some(grid) = weak.upgrade() {
                grid.imp().row_gap.set(Some(value.max(0.0)));
            }
            relayout();
        });
    }
    if let Some((ratio, extra)) = props.cells.clone() {
        let (weak, relayout) = (grid.downgrade(), relayout.clone());
        motion::bind_number(ctx, &extra, props.animate, &grid, move |value| {
            if let Some(grid) = weak.upgrade() {
                grid.imp().cells.set(Some((ratio, value)));
            }
            relayout();
        });
    }
    pad(ctx, grid.upcast_ref(), &props.padding, props.animate);
    // The scroll view it builds rows for, when it scrolls vertically.
    let scroll: Option<Rc<ScrollScope>> = ctx
        .scroll()
        .filter(|scope| scope.view.vscrollbar_policy() != gtk::PolicyType::Never)
        .cloned();
    if let Some(scope) = &scroll {
        imp.view.replace(Some(scope.view.clone()));
        let adjustment = scope.view.vadjustment();
        let watched = grid.downgrade();
        let moved = adjustment.connect_value_changed(move |_| {
            if let Some(grid) = watched.upgrade()
                && !grid.current()
            {
                grid.later();
            }
        });
        let watched = grid.downgrade();
        let paged = adjustment.connect_page_size_notify(move |_| {
            if let Some(grid) = watched.upgrade() {
                grid.later();
            }
        });
        let kept = adjustment.clone();
        on_cleanup(move || {
            kept.disconnect(moved);
            kept.disconnect(paged);
        });
        let revealing = grid.downgrade();
        let moving = Rc::downgrade(scope);
        scope.add_reveal(Rc::new(move |wanted: &Value| {
            let scope = moving.upgrade()?;
            scope.by_code.set(true);
            let found = revealing
                .upgrade()
                .and_then(|grid| grid.reveal(&key_text(wanted)));
            scope.by_code.set(false);
            found
        }));
    }
    let entering = Rc::new(Plain::new(false));
    let made = Rc::clone(&entering);
    let rows_ctx = ctx.clone();
    imp.make
        .replace(Some(Box::new(move |grid: &LazyGrid, i: usize| {
            let imp = grid.imp();
            let item = imp.items.borrow().get(i)?.clone();
            let id = imp.keys.borrow().get(i)?.clone();
            let key_value = at(item.clone(), &[key]);
            let value = Cell::new(item);
            let owner = Scope::detached();
            let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
            holder.set_parent(grid);
            let read = value.clone();
            let shown_key = id.clone();
            let row_ctx = rows_ctx
                .with_key(Rc::new(move || shown_key.clone()))
                .with_local(var, Rc::new(move || read.get()));
            let parent = match align {
                Some(align) => {
                    Parent::of_aligned_box(&holder, crate::widgets::stack_cross(Some(align), true))
                }
                None => Parent::of_box(&holder),
            }
            .region();
            let build = Rc::clone(&row);
            let scroll = scroll.clone();
            let entering = made.get();
            untrack(|| {
                owner.run(|| {
                    motion::with_entering(entering, || build(&parent, &row_ctx));
                    // Rows are the items a scroll view comes to rest on.
                    if let (Some(scroll), Some(first)) = (scroll, holder.first_child()) {
                        scroll.add_item(key_value, &first);
                    }
                })
            });
            Some(Built {
                value,
                owner,
                holder,
                parent,
            })
        })));
    let watched = grid.downgrade();
    let started = Plain::new(false);
    // Staggering rows also appear with what the grid stands in.
    let first_entering = motion::entering() || (stagger.is_some() && motion::appearing());
    let outer = motion::clock();
    let ctx = ctx.clone();
    effect(move || {
        let list = as_list(items(&ctx));
        let Some(grid) = watched.upgrade() else {
            return;
        };
        untrack(|| {
            let imp = grid.imp();
            let keys: Vec<String> = list
                .iter()
                .map(|item| key_text(&at(item.clone(), &[key])))
                .collect();
            // Rows still near the view follow their items' new values.
            {
                let built = imp.built.borrow();
                for (item, id) in list.iter().zip(&keys) {
                    if let Some(row) = built.get(id) {
                        row.value.set(item.clone());
                    }
                }
            }
            if align.is_some() {
                grid.set_visible(!list.is_empty());
            }
            imp.items.replace(list);
            imp.keys.replace(keys);
            let now = started.replace(true) || first_entering;
            let clock = match stagger {
                Some(step) if now && !motion::reduced() => {
                    Some(motion::with_entering(now, || motion::new_clock(step)))
                }
                _ => outer.clone(),
            };
            entering.set(now);
            motion::with_clock(clock, || {
                grid.lay_out(imp.laid.get().max(0));
                grid.update();
            });
            entering.set(false);
        });
    });
    let ended = grid.downgrade();
    on_cleanup(move || {
        if let Some(grid) = ended.upgrade() {
            let imp = grid.imp();
            for (_, row) in imp.built.borrow_mut().drain() {
                row.owner.dispose();
                row.holder.unparent();
            }
            imp.make.replace(None);
        }
    });
    place(p, &grid);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::features::{ScrollProps, ScrollView};
    use std::time::Duration;

    fn wait(seconds: f64) {
        let until = std::time::Instant::now() + Duration::from_secs_f64(seconds);
        let context = glib::MainContext::default();
        while std::time::Instant::now() < until {
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The places in the items of the rows built, first to last.
    fn built(grid: &LazyGrid) -> Vec<usize> {
        let imp = grid.imp();
        let keys = imp.keys.borrow();
        let built = imp.built.borrow();
        keys.iter()
            .enumerate()
            .filter(|(_, key)| built.contains_key(*key))
            .map(|(i, _)| i)
            .collect()
    }

    /// Where there is a display (Broadway on the Linux test machine): a
    /// grid of thousands of rows in a scroll view builds only the rows near
    /// the view, builds the rows where the view scrolls to, and follows its
    /// items. Without a display nothing lays out, and there is nothing to
    /// check.
    pub(crate) fn a_long_grid_builds_only_the_rows_near_the_view() {
        let window = gtk::Window::new();
        window.set_default_size(400, 300);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.set_child(Some(&content));
        let scope = Scope::detached();
        let entry = |i: i64| Value::Record(vec![Value::Int(i)]);
        let items = Cell::new(Value::List((0..5000).map(entry).collect()));
        let read = items.clone();
        let get: Get = Rc::new(move |_| read.get());
        let row: Build = Rc::new(|p: &Parent, _: &Ctx| {
            let label = gtk::Label::new(Some("row"));
            label.set_size_request(-1, 30);
            place(p, &label);
        });
        let ctx = Ctx::detached("lazy", None);
        let props = ScrollProps {
            axis: None,
            snap: None,
            position: None,
            arrival_nudge: false,
            hide_indicators: false,
            mouse_indicators: false,
            nudge_motion: (0.3, 1.0),
            smooth: (0.3, 1.0),
            open_at: None,
            open_center: None,
            anchor: None,
            browsing: None,
        };
        let grid_slot: Rc<RefCell<Option<LazyGrid>>> = Rc::default();
        let found = Rc::clone(&grid_slot);
        let children: Build = Rc::new(move |p: &Parent, c: &Ctx| {
            let grid_props = GridProps {
                min_width: Some(motion::Num::Fixed(10000.0)),
                ..GridProps::default()
            };
            LazyGrid(
                p,
                c,
                grid_props,
                Rc::clone(&get),
                0,
                "entry",
                None,
                None,
                Rc::clone(&row),
            );
            *found.borrow_mut() = p
                .widgets()
                .last()
                .and_then(|widget| widget.clone().downcast::<LazyGrid>().ok());
        });
        scope.run(|| ScrollView(&Parent::of_box(&content), &ctx, props, children));
        window.present();
        wait(1.0);
        let grid = grid_slot.borrow().clone().expect("the grid");
        let first = built(&grid);
        assert!(
            !first.is_empty() && first.len() < 60,
            "rows near the view only: {}",
            first.len()
        );
        assert_eq!(first[0], 0);
        // Far down: the rows there, not the first ones.
        let view = grid.imp().view.borrow().clone().expect("in a scroll view");
        view.vadjustment().set_value(60000.0);
        wait(0.6);
        let there = built(&grid);
        assert!(
            there.len() < 60,
            "still only the rows near the view: {}",
            there.len()
        );
        assert!(
            there[0] > 1000,
            "the rows where it scrolled to, from {}",
            there[0]
        );
        // The first ten rows leave: the rows built follow the items.
        items.set(Value::List((10..5000).map(entry).collect()));
        wait(0.4);
        assert_eq!(grid.imp().keys.borrow().len(), 4990);
        assert!(built(&grid).len() < 60);
        scope.dispose();
        window.destroy();
    }
}
