//! Conditional and repeated content, and the containers that navigate:
//! the sidebar split view, push navigation, tabs and adaptive layouts.

use crate::controls::{self, Actions};
use crate::ctx::{Field, Page};
use crate::host::host;
use crate::protocol::Value;
use crate::reactive::{Cell, Scope, effect, on_cleanup, untrack};
use crate::text::tr;
use crate::value::{as_list, at, key_text, payload, some, tag};
use crate::widgets::SearchProps;
use crate::{Act, Branch, Build, Ctx, Get, Parent, Str, icons, motion, windows};
use adw::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// An editing region consumes the first outside pointer gesture at the window
/// capture phase, before a button or slider can act on it.
#[allow(non_snake_case)]
pub fn FocusRegion(p: &Parent, ctx: &Ctx, children: Build) {
    let region = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    children(&Parent::of_box(&region), ctx);
    p.add(&region);
    let window = ctx.win().window.clone();
    let controller = gtk::EventControllerLegacy::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak_region = region.downgrade();
    let weak_window = window.downgrade();
    let held = std::cell::Cell::new(false);
    controller.connect_event(move |_, event| {
        use gtk::gdk::EventType;
        use gtk::glib::Propagation;
        let kind = event.event_type();
        if held.get() {
            match kind {
                EventType::ButtonRelease | EventType::TouchEnd | EventType::TouchCancel => {
                    held.set(false);
                    return Propagation::Stop;
                }
                EventType::MotionNotify | EventType::TouchUpdate => return Propagation::Stop,
                _ => {}
            }
        }
        if !matches!(kind, EventType::ButtonPress | EventType::TouchBegin) {
            return Propagation::Proceed;
        }
        held.set(false);
        let (Some(region), Some(window)) = (weak_region.upgrade(), weak_window.upgrade()) else {
            return Propagation::Proceed;
        };
        if !region.is_mapped() || event.surface() != window.surface() {
            return Propagation::Proceed;
        }
        let Some(focus) = gtk::prelude::RootExt::focus(&window) else {
            return Propagation::Proceed;
        };
        if !focus.is_ancestor(&region)
            || !(focus.is::<gtk::Editable>() || focus.is::<gtk::TextView>())
        {
            return Propagation::Proceed;
        }
        let (Some((x, y)), Some(bounds)) = (event.position(), region.compute_bounds(&window))
        else {
            return Propagation::Proceed;
        };
        let (dx, dy) = window.surface_transform();
        if bounds.contains_point(&gtk::graphene::Point::new((x + dx) as f32, (y + dy) as f32)) {
            return Propagation::Proceed;
        }
        held.set(true);
        gtk::prelude::RootExt::set_focus(&window, None::<&gtk::Widget>);
        Propagation::Stop
    });
    window.add_controller(controller.clone());
    on_cleanup(move || window.remove_controller(&controller));
}

/// Content that a builder replaces: the current scope and region.
struct Slot {
    region: Parent,
    owner: RefCell<Option<Scope>>,
    /// Whether it showed content before: what comes later appears in an
    /// update.
    filled: std::cell::Cell<bool>,
    /// The stagger clock of the container it stands in.
    clock: Option<Rc<motion::Clock>>,
}

impl Slot {
    fn new(region: Parent) -> Rc<Self> {
        let slot = Rc::new(Self {
            region,
            owner: RefCell::new(None),
            filled: std::cell::Cell::new(false),
            clock: motion::clock(),
        });
        let ended = Rc::clone(&slot);
        on_cleanup(move || ended.end());
        slot
    }

    fn end(&self) {
        let owner = self.owner.borrow_mut().take();
        if let Some(owner) = owner {
            owner.dispose();
        }
    }

    /// Replace the content with what `build` makes, outside any effect.
    /// `structural` content (of `if`, `match` and `ForEach`) appears with
    /// its transitions after the first time.
    fn fill(&self, structural: bool, build: impl FnOnce(&Parent)) {
        let entering = structural && (self.filled.replace(true) || motion::entering());
        untrack(|| {
            self.end();
            self.region.clear();
            let owner = Scope::detached();
            motion::with_entering(entering, || {
                motion::with_clock(self.clock.clone(), || owner.run(|| build(&self.region)))
            });
            *self.owner.borrow_mut() = Some(owner);
        });
    }

    fn clear(&self) {
        untrack(|| {
            self.end();
            self.region.clear();
        });
    }
}

/// Conditional content: `branch` picks an arm, or none. Only a change of
/// arm rebuilds; the arm's own bindings update in place.
#[allow(non_snake_case)]
pub fn When(p: &Parent, ctx: &Ctx, branch: Branch, arms: Vec<Build>) {
    let slot = Slot::new(p.region());
    let current: Rc<std::cell::Cell<Option<Option<usize>>>> = Rc::new(std::cell::Cell::new(None));
    let ctx = ctx.clone();
    effect(move || {
        let next = branch(&ctx);
        if current.get() == Some(next) {
            return;
        }
        current.set(Some(next));
        match next.and_then(|index| arms.get(index)) {
            Some(arm) => {
                let arm = Rc::clone(arm);
                slot.fill(true, |region| arm(region, &ctx));
            }
            None => {
                slot.filled.set(true);
                slot.clear();
            }
        }
    });
}

/// `if let Some(var) = value { then } else { otherwise }`.
#[allow(non_snake_case)]
pub fn IfLet(
    p: &Parent,
    ctx: &Ctx,
    var: &'static str,
    value: Get,
    then: Build,
    otherwise: Option<Build>,
) {
    let read = Rc::clone(&value);
    let at = ctx.clone();
    let inner = ctx.with_local(var, Rc::new(move || some(read(&at)).unwrap_or(Value::Unit)));
    let mut arms = vec![build_in(then, inner)];
    if let Some(otherwise) = otherwise {
        arms.push(otherwise);
    }
    let has_else = arms.len() > 1;
    When(
        p,
        ctx,
        Rc::new(move |ctx| match value(ctx) {
            Value::Optional(Some(_)) => Some(0),
            _ if has_else => Some(1),
            _ => None,
        }),
        arms,
    );
}

/// An arm builder that runs in a fixed context.
fn build_in(build: Build, ctx: Ctx) -> Build {
    Rc::new(move |p, _| build(p, &ctx))
}

/// One arm of a `match`: the variant (`None` for `_`), the names its
/// payload fields bind, and its content.
pub struct MatchArm {
    pub variant: Option<u32>,
    pub bindings: Vec<(&'static str, usize)>,
    pub build: Build,
}

#[allow(non_snake_case)]
pub fn Match(p: &Parent, ctx: &Ctx, value: Get, arms: Vec<MatchArm>) {
    let variants: Vec<Option<u32>> = arms.iter().map(|arm| arm.variant).collect();
    let builders: Vec<Build> = arms
        .into_iter()
        .map(|arm| {
            let mut inner = ctx.clone();
            for (name, field) in arm.bindings {
                let read = Rc::clone(&value);
                let at = ctx.clone();
                inner = inner.with_local(name, Rc::new(move || payload(read(&at), field)));
            }
            build_in(arm.build, inner)
        })
        .collect();
    When(
        p,
        ctx,
        Rc::new(move |ctx| {
            let tag = tag(&value(ctx));
            variants
                .iter()
                .position(|variant| variant.is_some() && *variant == tag)
                .or_else(|| variants.iter().position(Option::is_none))
        }),
        builders,
    );
}

/// A preview list's rows: each one's height, the gap between them, and the
/// height of the line counting the rows left out.
pub struct PreviewListProps {
    pub item_height: f64,
    pub gap: f64,
    pub more_height: f64,
}

/// A height-bounded preview. The overlay's measuring child has no intrinsic
/// height, so the rows cannot enlarge their own viewport.
#[allow(non_snake_case)]
pub fn PreviewList(
    p: &Parent,
    ctx: &Ctx,
    items: Get,
    key: usize,
    var: &'static str,
    props: PreviewListProps,
    row: Build,
) {
    let PreviewListProps {
        item_height,
        gap,
        more_height,
    } = props;
    let gap = gap.max(0.0);
    let more_height = more_height.max(0.0);
    let viewport = gtk::Overlay::new();
    viewport.set_hexpand(true);
    viewport.set_vexpand(true);
    let measure = gtk::DrawingArea::new();
    measure.set_hexpand(true);
    measure.set_vexpand(true);
    viewport.set_child(Some(&measure));
    let column = gtk::Box::new(gtk::Orientation::Vertical, gap.round() as i32);
    column.set_halign(gtk::Align::Center);
    column.set_valign(gtk::Align::Start);
    viewport.add_overlay(&column);
    viewport.set_measure_overlay(&column, false);
    viewport.set_clip_overlay(&column, true);
    let rows = gtk::Box::new(gtk::Orientation::Vertical, gap.round() as i32);
    column.append(&rows);
    let more = gtk::Label::new(None);
    more.add_css_class("caption");
    more.add_css_class("dim-label");
    more.set_height_request(more_height.round() as i32);
    column.append(&more);
    let height = Cell::new(0.0_f64);
    let measured = height.clone();
    measure.connect_resize(move |_, _, h| measured.set(f64::from(h)));
    let badge_items = items.clone();
    let badge_height = height.clone();
    let shown: Get = Rc::new(move |ctx| {
        let mut list = as_list(items(ctx));
        let total = list.len();
        let h = height.get();
        let count = if total == 0 || h <= 0.0 || item_height <= 0.0 {
            0
        } else if total as f64 * item_height + total.saturating_sub(1) as f64 * gap <= h {
            total
        } else {
            (((h - more_height).max(0.0) / (item_height + gap)).floor() as usize).min(total)
        };
        list.truncate(count);
        Value::List(list)
    });
    // The badge uses the same geometry without constructing omitted rows.
    Each(
        &Parent::of_box(&rows),
        ctx,
        shown.clone(),
        key,
        var,
        None,
        row,
    );
    let at = ctx.clone();
    effect(move || {
        let count = as_list(shown(&at)).len();
        let remaining = as_list(badge_items(&at)).len().saturating_sub(count);
        rows.set_visible(count > 0);
        more.set_text(&format!("+{remaining}"));
        more.set_visible(remaining > 0 && badge_height.get() >= more_height);
    });
    p.add(&viewport);
}

struct Row {
    key: String,
    value: Cell<Value>,
    owner: Scope,
    region: Parent,
}

/// Rows that enter in turns are built this long before their turn comes,
/// so their pictures and text are ready when they start to appear.
const BUILT_AHEAD: f64 = 1.0 / 60.0;

/// A row that enters in turns, waiting to be built near its turn.
struct Waiting {
    due: std::time::Instant,
    /// When the turns began, its turn among them, and their step.
    began: std::time::Instant,
    turn: u32,
    step: f64,
    owner: Scope,
    fill: Box<dyn FnOnce(Option<Rc<motion::Clock>>)>,
}

/// The rows a list builds late, one a frame: rows built between two frames
/// would all be laid out in the next one.
#[derive(Default)]
struct Late {
    rows: RefCell<std::collections::VecDeque<Waiting>>,
    /// The row widget whose frame clock builds them, while it does.
    ticking: RefCell<Option<glib::WeakRef<gtk::Widget>>>,
}

impl Late {
    /// Build the waiting rows on `widget`'s frame clock, unless a live one
    /// already does; without one, build them all now.
    fn start(self: &Rc<Self>, widget: Option<gtk::Widget>) {
        if self.rows.borrow().is_empty() {
            return;
        }
        let live = self
            .ticking
            .borrow()
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .is_some();
        if live {
            return;
        }
        let Some(widget) = widget else {
            while let Some(waiting) = self.next(None) {
                Self::build(waiting);
            }
            return;
        };
        *self.ticking.borrow_mut() = Some(widget.downgrade());
        let late = Rc::clone(self);
        widget.add_tick_callback(move |_, _| {
            if let Some(waiting) = late.next(Some(std::time::Instant::now())) {
                Self::build(waiting);
            }
            if late.rows.borrow().is_empty() {
                late.ticking.borrow_mut().take();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    /// The first row still waiting, once it is due at `now` (any time
    /// without one); rows that left are dropped.
    fn next(&self, now: Option<std::time::Instant>) -> Option<Waiting> {
        let mut rows = self.rows.borrow_mut();
        while rows
            .front()
            .is_some_and(|waiting| waiting.owner.is_disposed())
        {
            rows.pop_front();
        }
        match (rows.front(), now) {
            (Some(waiting), Some(now)) if waiting.due > now => None,
            _ => rows.pop_front(),
        }
    }

    fn build(waiting: Waiting) {
        let late = waiting.began.elapsed().as_secs_f64();
        let clock = motion::late_clock(waiting.step, waiting.turn, late);
        let fill = waiting.fill;
        untrack(|| waiting.owner.run(|| fill(Some(clock))));
    }
}

/// Keyed rows: a row keeps its widgets, controls and state while its key
/// stays; its value is a cell the row's bindings read. Rows added after the
/// first ones appear with their transitions; with `stagger` they take
/// turns, and each but the first is built just before its turn, one a
/// frame, instead of with the others: a page of rows that enter one by one
/// would otherwise be built and laid out in one frame, which stalls it.
/// Each row's place is kept from the start, so they arrive in order.
#[allow(non_snake_case)]
pub fn Each(
    p: &Parent,
    ctx: &Ctx,
    items: Get,
    key: usize,
    var: &'static str,
    stagger: Option<f64>,
    row: Build,
) {
    let region = p.region();
    let rows: Rc<RefCell<Vec<Row>>> = Rc::new(RefCell::new(Vec::new()));
    let ended = Rc::clone(&rows);
    let late = Rc::new(Late::default());
    let abandoned = Rc::clone(&late);
    on_cleanup(move || {
        abandoned.rows.borrow_mut().clear();
        for row in ended.borrow_mut().drain(..) {
            row.owner.dispose();
        }
    });
    let ctx = ctx.clone();
    let outer = motion::clock();
    let started = std::cell::Cell::new(false);
    // Staggering rows also appear with what the list stands in.
    let first_entering = motion::entering() || (stagger.is_some() && motion::appearing());
    effect(move || {
        let list = as_list(items(&ctx));
        let entering = started.replace(true) || first_entering;
        let clock = match stagger {
            Some(step) if !motion::reduced() => {
                Some(motion::with_entering(entering, || motion::new_clock(step)))
            }
            _ => outer.clone(),
        };
        // Rows entering in turns, after the first, wait for theirs.
        let turns = match (&clock, stagger) {
            (Some(clock), Some(_)) if entering && !motion::reduced() => Some(Rc::clone(clock)),
            _ => None,
        };
        let began = std::time::Instant::now();
        let mut built_now = false;
        let mut waiting: u32 = 0;
        untrack(|| {
            let previous: Vec<Row> = rows.borrow_mut().drain(..).collect();
            let order_before: Vec<String> = previous.iter().map(|row| row.key.clone()).collect();
            let mut old: HashMap<String, Row> = previous
                .into_iter()
                .map(|row| (row.key.clone(), row))
                .collect();
            let mut next = Vec::new();
            for item in list {
                let key_value = at(item.clone(), &[key]);
                let id = key_text(&key_value);
                if let Some(existing) = old.remove(&id) {
                    existing.value.set(item);
                    next.push(existing);
                    continue;
                }
                let value = Cell::new(item);
                let owner = Scope::detached();
                let child = region.region();
                let read = value.clone();
                let shown_key = id.clone();
                let row_ctx = ctx
                    .with_key(Rc::new(move || shown_key.clone()))
                    .with_local(var, Rc::new(move || read.get()));
                let build = Rc::clone(&row);
                let scroll = ctx.scroll().cloned();
                let fill = {
                    let child = child.clone();
                    move |clock: Option<Rc<motion::Clock>>| {
                        motion::with_entering(entering, || {
                            motion::with_clock(clock, || build(&child, &row_ctx))
                        });
                        // Rows are the items a scroll view comes to rest on.
                        if let (Some(scroll), Some(first)) = (scroll, child.widgets().first()) {
                            scroll.add_item(key_value, first);
                        }
                    }
                };
                match &turns {
                    Some(turns) if built_now => {
                        // Its turn, had it been built with the first row,
                        // and when to build it: just before that.
                        let step = turns.step();
                        let turn = turns.next_turn() + waiting;
                        waiting += 1;
                        let due = (f64::from(turn) * step - BUILT_AHEAD).max(0.0);
                        late.rows.borrow_mut().push_back(Waiting {
                            due: began + std::time::Duration::from_secs_f64(due),
                            began,
                            turn,
                            step,
                            owner: owner.clone(),
                            fill: Box::new(fill),
                        });
                    }
                    _ => {
                        built_now = true;
                        owner.run(|| fill(clock.clone()));
                    }
                }
                next.push(Row {
                    key: id,
                    value,
                    owner,
                    region: child,
                });
            }
            for (_, stale) in old {
                stale.owner.dispose();
                region.remove_region(&stale.region);
            }
            // New rows were added at the end; reorder only when the kept
            // rows changed order.
            let kept: Vec<&String> = order_before
                .iter()
                .filter(|key| next.iter().any(|row| &row.key == *key))
                .collect();
            let in_order = next.iter().take(kept.len()).map(|row| &row.key).eq(kept);
            if !in_order {
                let order: Vec<Parent> = next.iter().map(|row| row.region.clone()).collect();
                region.reorder(&order);
            }
            let shown = next
                .iter()
                .find_map(|row| row.region.widgets().first().cloned());
            *rows.borrow_mut() = next;
            late.start(shown);
        });
    });
}

/// A list of rows; inside a form section the rows join the section.
#[allow(non_snake_case)]
pub fn List(p: &Parent, ctx: &Ctx, children: Build) {
    if p.is_list() {
        children(p, ctx);
        return;
    }
    let list = crate::widgets::boxed_list();
    // Its rows span the width it is given, in a column of any alignment.
    list.set_hexpand(true);
    children(&Parent::of_list(&list), ctx);
    p.add(&list);
}

/// The name of an enum variant value of `enum_name`, for `ui.select`.
fn variant_name(enum_name: &str, value: &Value) -> &'static str {
    let Some(variant) = tag(value) else {
        return "";
    };
    let def: &'static crate::AppDef = host().def;
    def.types
        .enum_def(enum_name)
        .and_then(|def| def.variants.get(variant as usize))
        .map(|variant| variant.name.as_str())
        .unwrap_or("")
}

/// A sidebar item while it is mounted.
pub(crate) struct Entry {
    value: Value,
    name: &'static str,
    label: Rc<dyn Fn() -> String>,
    row: gtk::ListBoxRow,
}

/// The items a sidebar shows now, as they mount and leave with `if`,
/// `match` and `ForEach`.
pub struct Registry {
    entries: RefCell<Vec<Rc<Entry>>>,
    version: crate::reactive::Version,
    selection: Field,
    ctx: Ctx,
    showing_detail: RefCell<Option<adw::NavigationSplitView>>,
}

impl Registry {
    fn add(self: &Rc<Self>, entry: Rc<Entry>) {
        self.entries.borrow_mut().push(Rc::clone(&entry));
        self.version.bump();
        let registry = Rc::downgrade(self);
        on_cleanup(move || {
            if let Some(registry) = registry.upgrade() {
                registry
                    .entries
                    .borrow_mut()
                    .retain(|kept| !Rc::ptr_eq(kept, &entry));
                registry.version.bump();
            }
        });
    }

    fn choose(&self, value: Value) {
        self.ctx.write(self.selection, value);
        if let Some(split) = self.showing_detail.borrow().as_ref() {
            split.set_show_content(true);
        }
    }

    /// Mounted items in the order they show.
    fn ordered(&self) -> Vec<Rc<Entry>> {
        let mut entries = self.entries.borrow().clone();
        entries.sort_by_key(|entry| entry.row.index());
        entries
    }

    fn selected(&self, value: &Value) -> Option<Rc<Entry>> {
        self.entries
            .borrow()
            .iter()
            .find(|entry| entry.value == *value)
            .cloned()
    }
}

pub struct SidebarItemProps {
    pub value: Get,
    /// The enum of the selection, for the names `ui.select` takes.
    pub enum_name: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub style: Option<&'static crate::style::StyleDef>,
    /// A count at the trailing end, hidden while it is zero.
    pub badge: Option<crate::motion::Num>,
}

/// One sidebar destination; it is part of the selection while mounted.
#[allow(non_snake_case)]
pub fn SidebarItem(p: &Parent, ctx: &Ctx, props: SidebarItemProps) {
    let Some(registry) = ctx.sidebar().cloned() else {
        return;
    };
    let value = untrack(|| (props.value)(ctx));
    let label: Rc<dyn Fn() -> String> = match props.label {
        Some(label) => {
            let at = ctx.clone();
            Rc::new(move || label(&at))
        }
        None => Rc::new(String::new),
    };
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(icon) = props.icon {
        content.append(&icons::image(icon));
    }
    let text = gtk::Label::new(None);
    text.add_css_class("bern-sidebar-label");
    text.set_xalign(0.0);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let shown = text.clone();
    let read = Rc::clone(&label);
    effect(move || shown.set_label(&read()));
    text.set_hexpand(true);
    content.append(&text);
    if let Some(badge) = &props.badge {
        let count = gtk::Label::new(None);
        count.add_css_class("bern-badge");
        count.add_css_class("numeric");
        count.set_valign(gtk::Align::Center);
        let shown = count.clone();
        crate::motion::bind_number(ctx, badge, None, &count, move |value| {
            let value = value.round().max(0.0) as i64;
            shown.set_visible(value > 0);
            shown.set_label(&value.to_string());
        });
        content.append(&count);
    }
    let row = gtk::ListBoxRow::new();
    match props.style {
        Some(style) => {
            content.add_css_class("bern-sidebar-content");
            let motion = crate::motion::MotionBox::wrap(&content);
            row.add_css_class("bern-styled-row");
            row.set_child(Some(&motion));
            crate::style::apply(style, row.upcast_ref(), content.upcast_ref(), &motion);
        }
        None => row.set_child(Some(&content)),
    }
    p.add(&row);
    registry.add(Rc::new(Entry {
        name: variant_name(props.enum_name, &value),
        value,
        label,
        row,
    }));
}

/// A titled group of sidebar items.
#[allow(non_snake_case)]
pub fn SidebarSection(p: &Parent, ctx: &Ctx, title: Option<Str>, children: Build) {
    if let Some(title) = title {
        let heading = gtk::Label::new(None);
        heading.set_xalign(0.0);
        heading.add_css_class("caption-heading");
        heading.add_css_class("dim-label");
        heading.set_margin_top(12);
        heading.set_margin_bottom(6);
        heading.set_margin_start(6);
        let at = ctx.clone();
        let shown = heading.clone();
        effect(move || shown.set_label(&title(&at)));
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&heading));
        p.add(&row);
    }
    children(p, ctx);
}

/// Where a selection goes when no mounted item holds it: to the fallback
/// while that is mounted, else to the first item. `None` while an item
/// holds it or none is mounted.
fn settled(
    current: &Value,
    mounted: &[Value],
    fallback: impl FnOnce() -> Option<Value>,
) -> Option<Value> {
    if mounted.is_empty() || mounted.contains(current) {
        return None;
    }
    let back = fallback().filter(|fallback| mounted.contains(fallback));
    Some(back.unwrap_or_else(|| mounted[0].clone()))
}

pub struct SplitSpec {
    pub id: Option<&'static str>,
    pub selection: Field,
    /// The item chosen when the selected one leaves.
    pub fallback: Option<Get>,
    pub search: Option<(SearchProps, Option<Act>)>,
    /// Content pinned above the sidebar's list.
    pub header: Option<Build>,
    /// Content at the foot of the sidebar, below the list.
    pub footer: Option<Build>,
    /// The sidebar's width, fixed.
    pub width: Option<f64>,
    /// While false, the detail's title bar leaves out the destination's
    /// name, for pages that show their own.
    pub titled: Option<Get>,
}

/// The sidebar and detail split of GNOME applications: a navigation
/// sidebar with optional search and footer, and a detail page whose header
/// shows the selected destination, the detail's tools and back. Items may
/// come and go; when the selected one leaves, the selection moves to the
/// fallback or the first item left.
#[allow(non_snake_case)]
pub fn SplitView(p: &Parent, ctx: &Ctx, spec: SplitSpec, sidebar: Build, detail: Build) {
    if let Some(page) = ctx.page() {
        page.hide_header();
    }
    let split = adw::NavigationSplitView::new();
    split.set_vexpand(true);
    split.set_hexpand(true);
    let selection = spec.selection;

    // Sidebar
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_selection_mode(gtk::SelectionMode::Single);
    let registry = Rc::new(Registry {
        entries: RefCell::new(Vec::new()),
        version: crate::reactive::Version::new(),
        selection,
        ctx: ctx.clone(),
        showing_detail: RefCell::new(Some(split.clone())),
    });
    sidebar(
        &Parent::of_list(&list),
        &ctx.with_sidebar(Rc::clone(&registry)),
    );
    let updating = Rc::new(std::cell::Cell::new(false));
    {
        let registry = Rc::clone(&registry);
        let guard = Rc::clone(&updating);
        let shown = list.clone();
        let at = ctx.clone();
        effect(move || {
            registry.version.track();
            let value = at.read(selection);
            let row = registry.selected(&value).map(|entry| entry.row.clone());
            if shown.selected_row() != row {
                guard.set(true);
                shown.select_row(row.as_ref());
                guard.set(false);
            }
        });
    }
    // A selection without a mounted item moves on, once the update that
    // removed it has mounted whatever replaces it.
    {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        let fallback = spec.fallback.clone();
        effect(move || {
            registry.version.track();
            at.read(selection);
            let registry = Rc::clone(&registry);
            let at = at.clone();
            let fallback = fallback.clone();
            glib::idle_add_local_once(move || {
                let mounted: Vec<Value> = registry
                    .ordered()
                    .iter()
                    .map(|entry| entry.value.clone())
                    .collect();
                let next = settled(&at.peek(selection), &mounted, || {
                    fallback.map(|fallback| untrack(|| fallback(&at)))
                });
                if let Some(next) = next {
                    at.write(selection, next);
                }
            });
        });
    }
    {
        let registry = Rc::clone(&registry);
        list.connect_row_selected(move |_, row| {
            if updating.get() {
                return;
            }
            let Some(row) = row else { return };
            let chosen = registry
                .entries
                .borrow()
                .iter()
                .find(|entry| &entry.row == row)
                .map(|entry| entry.value.clone());
            if let Some(value) = chosen {
                registry.choose(value);
            }
        });
    }
    let activated = split.clone();
    list.connect_row_activated(move |_, _| activated.set_show_content(true));

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if let Some((search, submit)) = spec.search {
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        holder.set_margin_top(6);
        holder.set_margin_bottom(6);
        holder.set_margin_start(12);
        holder.set_margin_end(12);
        crate::widgets::Search(&Parent::of_box(&holder), ctx, search, submit, None);
        sidebar_box.append(&holder);
    }
    if let Some(header) = spec.header {
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 8);
        holder.add_css_class("bern-sidebar-header");
        holder.set_margin_top(6);
        holder.set_margin_bottom(6);
        holder.set_margin_start(12);
        holder.set_margin_end(12);
        header(&Parent::of_box(&holder), ctx);
        sidebar_box.append(&holder);
    }
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&list));
    sidebar_box.append(&scrolled);
    if let Some(footer) = spec.footer {
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 8);
        holder.add_css_class("bern-sidebar-footer");
        holder.set_margin_top(6);
        holder.set_margin_bottom(12);
        holder.set_margin_start(12);
        holder.set_margin_end(12);
        footer(&Parent::of_box(&holder), ctx);
        sidebar_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        sidebar_box.append(&holder);
    }
    let sidebar_view = adw::ToolbarView::new();
    let sidebar_header = adw::HeaderBar::new();
    let app_title = adw::WindowTitle::new("", "");
    sidebar_header.set_title_widget(Some(&app_title));
    sidebar_view.add_top_bar(&sidebar_header);
    sidebar_view.set_content(Some(&sidebar_box));
    ctx.win().chrome_view(&sidebar_view);
    let sidebar_page = adw::NavigationPage::new(&sidebar_view, "");
    let name_key = host().def.name;
    let titled = sidebar_page.clone();
    effect(move || {
        let name = tr(name_key);
        app_title.set_title(&name);
        titled.set_title(&name);
    });
    split.set_sidebar(Some(&sidebar_page));
    if let Some(width) = spec.width {
        split.set_sidebar_width_unit(adw::LengthUnit::Px);
        split.set_min_sidebar_width(width);
        split.set_max_sidebar_width(width);
    }

    // Detail
    let selected_label: Rc<dyn Fn() -> String> = {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        Rc::new(move || {
            registry.version.track();
            let value = at.read(selection);
            registry
                .selected(&value)
                .map(|entry| (entry.label)())
                .unwrap_or_default()
        })
    };
    let page = Page::new();
    let titled: Rc<dyn Fn() -> bool> = match spec.titled.clone() {
        Some(titled) => {
            let at = ctx.clone();
            Rc::new(move || crate::value::truthy(&titled(&at)))
        }
        None => Rc::new(|| true),
    };
    let header_label: Rc<dyn Fn() -> String> = {
        let label = Rc::clone(&selected_label);
        let titled = Rc::clone(&titled);
        Rc::new(move || if titled() { label() } else { String::new() })
    };
    let header = windows::header(&page, Some(header_label));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_vexpand(true);
    let detail_view = adw::ToolbarView::new();
    detail_view.add_top_bar(&header);
    detail_view.set_content(Some(&content));
    ctx.win().chrome_view(&detail_view);
    let detail_page = adw::NavigationPage::new(&detail_view, "");
    let titled = detail_page.clone();
    let shown_page = Rc::clone(&page);
    let fallback = Rc::clone(&selected_label);
    effect(move || {
        shown_page.version.track();
        titled.set_title(&shown_page.title().unwrap_or_else(|| fallback()));
    });
    split.set_content(Some(&detail_page));
    detail(&Parent::of_box(&content), &ctx.with_page(page));

    // Narrow windows show one pane at a time.
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        400.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    ctx.win().window.add_breakpoint(breakpoint);

    if let Some(id) = spec.id {
        let names = Rc::clone(&registry);
        let current = Rc::clone(&registry);
        let read = ctx.clone();
        controls::register(
            ctx,
            id,
            "sidebar",
            Some(selected_label),
            None,
            Actions {
                select: Some(Rc::new(move |name: &str| {
                    let found = names
                        .entries
                        .borrow()
                        .iter()
                        .find(|entry| entry.name == name)
                        .map(|entry| entry.value.clone());
                    match found {
                        Some(value) => {
                            names.choose(value);
                            true
                        }
                        None => false,
                    }
                })),
                value: Some(Rc::new(move || {
                    let value = read.peek(selection);
                    current.selected(&value).map(|entry| entry.name.to_string())
                })),
                ..Actions::default()
            },
        );
    }
    p.add(&split);
}

/// Push navigation over a typed route path. The root content stays
/// mounted under pushed destinations, so its subscriptions, players and
/// scroll position survive going back; the top route's destination is
/// built on top, and the page header offers back.
#[allow(non_snake_case)]
pub fn Navigation(
    p: &Parent,
    ctx: &Ctx,
    id: Option<&'static str>,
    path: Field,
    content: Build,
    var: &'static str,
    destination: Build,
) {
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    // The pages take the room they are given both ways, as on the other
    // targets.
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_hhomogeneous(false);
    stack.set_vhomogeneous(false);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let top = gtk::Box::new(gtk::Orientation::Vertical, 0);
    stack.add_named(&root, Some("root"));
    stack.add_named(&top, Some("top"));
    let inner = ctx.with_nav(path);
    // The root's tools show only while the root is.
    let root_shown = Cell::new(true);
    content(
        &Parent::of_box(&root),
        &inner.with_shown(root_shown.clone()),
    );

    let back: Rc<dyn Fn() -> bool> = {
        let at = ctx.clone();
        Rc::new(move || {
            let mut routes = as_list(at.peek(path));
            if routes.pop().is_none() {
                return false;
            }
            at.write(path, Value::List(routes));
            true
        })
    };
    let keys = gtk::ShortcutController::new();
    let escape = Rc::clone(&back);
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("Escape|<Alt>Left"),
        Some(gtk::CallbackAction::new(move |_, _| {
            if escape() {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        })),
    ));
    top.add_controller(keys);

    let slot = Slot::new(Parent::of_box(&top));
    let page = ctx.page().cloned();
    let at = ctx.clone();
    let shown_stack = stack.clone();
    let pop = Rc::clone(&back);
    effect(move || {
        let routes = as_list(at.read(path));
        root_shown.set(routes.is_empty());
        let Some(route) = routes.last().cloned() else {
            shown_stack.set_visible_child_name("root");
            slot.clear();
            return;
        };
        let page = page.clone();
        let pop = Rc::clone(&pop);
        let destination = Rc::clone(&destination);
        let inner = inner.with_local(var, Rc::new(move || route.clone()));
        slot.fill(false, |region| {
            if let Some(page) = &page {
                page.push_back(Rc::new(move || {
                    pop();
                }));
            }
            destination(region, &inner);
        });
        shown_stack.set_visible_child_name("top");
    });
    if let Some(id) = id {
        let read = ctx.clone();
        controls::register(
            ctx,
            id,
            "navigation",
            Some(Rc::new(String::new)),
            None,
            Actions {
                back: Some(back),
                value: Some(Rc::new(move || {
                    Some(as_list(read.peek(path)).len().to_string())
                })),
                ..Actions::default()
            },
        );
    }
    p.add(&stack);
}

/// A row that pushes its route on the enclosing navigation.
#[allow(non_snake_case)]
pub fn NavLink(p: &Parent, ctx: &Ctx, id: &'static str, route: Get, label: Str, children: Build) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_hexpand(true);
    children(&Parent::of_aligned_box(&content, gtk::Align::Center), ctx);
    let activate: Rc<dyn Fn()> = {
        let at = ctx.clone();
        Rc::new(move || {
            let Some(path) = at.nav() else { return };
            let mut routes = as_list(at.peek(path));
            routes.push(route(&at));
            at.write(path, Value::List(routes));
        })
    };
    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    if p.is_list() {
        let inside = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        inside.append(&content);
        inside.append(&chevron);
        inside.set_margin_top(14);
        inside.set_margin_bottom(14);
        inside.set_margin_start(12);
        inside.set_margin_end(12);
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);
        row.set_child(Some(&inside));
        crate::widgets::on_activate(&row, Rc::clone(&activate));
        p.add(&row);
    } else {
        let inside = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        inside.append(&content);
        inside.append(&chevron);
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.set_child(Some(&inside));
        let run = Rc::clone(&activate);
        button.connect_clicked(move |_| run());
        p.add(&button);
    }
    if ctx.nav().is_some() {
        let at = ctx.clone();
        controls::register(
            ctx,
            id,
            "link",
            Some(Rc::new(move || label(&at))),
            None,
            Actions {
                activate: Some(activate),
                ..Actions::default()
            },
        );
    }
}

/// A tab while it is mounted.
pub(crate) struct TabEntry {
    value: Value,
    name: &'static str,
    label: Rc<dyn Fn() -> String>,
    icon: Option<&'static str>,
    slot: Rc<Slot>,
    build: Build,
    ctx: Ctx,
    page: String,
    /// The page's content in the stack.
    holder: gtk::Box,
    /// The stack's page, which carries the title and icon; a new one each
    /// time the page comes back.
    titled: Rc<RefCell<adw::ViewStackPage>>,
    /// The tab's place among those declared.
    marker: gtk::Widget,
}

/// The stack's new page order from `places`, the declared place of each
/// page in the stack's order: the pages in declared order, by their
/// index in the stack, and how many lead it in place. The stack only
/// appends, so the pages after the longest run of the declared order it
/// already shows from the start leave and come back.
fn arranged(places: &[usize]) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..places.len()).collect();
    order.sort_by_key(|&page| places[page]);
    let mut kept = 0;
    for page in 0..places.len() {
        if order.get(kept) == Some(&page) {
            kept += 1;
        }
    }
    (order, kept)
}

/// The tabs a tab view shows now, in the order of the stack's pages,
/// which the switcher shows: the order they are declared in, also when a
/// condition or a list mounts one before others.
pub struct TabRegistry {
    stack: adw::ViewStack,
    /// Where the tabs are declared: each marks its place there.
    declared: Parent,
    entries: RefCell<Vec<Rc<TabEntry>>>,
    version: crate::reactive::Version,
    next: std::cell::Cell<u32>,
    /// The names of the pages whose tabs show only their icon.
    icon_only: RefCell<std::collections::HashSet<String>>,
    /// The view switchers of the pages: the header's and the bottom bar's.
    switchers: RefCell<Vec<adw::ViewSwitcher>>,
    updating: std::cell::Cell<bool>,
    arranging: std::cell::Cell<bool>,
}

impl TabRegistry {
    fn selected(&self, value: &Value) -> Option<Rc<TabEntry>> {
        self.entries
            .borrow()
            .iter()
            .find(|entry| entry.value == *value)
            .cloned()
    }

    /// Change the stack's pages, or the page it shows, without the stack's
    /// own pick counting as a choice: it shows the first page added to it,
    /// and none once the page shown leaves. The selection decides, through
    /// `settled`.
    fn quietly<T>(&self, change: impl FnOnce() -> T) -> T {
        let was = self.updating.replace(true);
        let result = change();
        self.updating.set(was);
        result
    }

    /// Put the stack's pages in the order the tabs are declared, keeping
    /// each page's title and icon and the page shown. Whether the order
    /// changed.
    fn arrange(&self) -> bool {
        let entries = self.entries.borrow().clone();
        let declared = self.declared.widgets();
        let places: Vec<usize> = entries
            .iter()
            .map(|entry| {
                declared
                    .iter()
                    .position(|marker| *marker == entry.marker)
                    .unwrap_or(usize::MAX)
            })
            .collect();
        let (order, kept) = arranged(&places);
        if kept == order.len() {
            return false;
        }
        let order: Vec<Rc<TabEntry>> = order
            .into_iter()
            .map(|page| Rc::clone(&entries[page]))
            .collect();
        let shown = self.stack.visible_child_name();
        self.quietly(|| {
            for entry in &order[kept..] {
                self.stack.remove(&entry.holder);
            }
            for entry in &order[kept..] {
                let title = untrack(|| (entry.label)());
                let page = self
                    .stack
                    .add_titled(&entry.holder, Some(&entry.page), &title);
                if let Some(icon) = entry.icon {
                    page.set_icon_name(Some(icons::name(icon)));
                }
                *entry.titled.borrow_mut() = page;
            }
            if let Some(shown) = shown {
                self.stack.set_visible_child_name(&shown);
            }
        });
        *self.entries.borrow_mut() = order;
        true
    }

    /// Arrange the pages once the update that moved a tab's place, such
    /// as a list reordering its rows, is over: before the selection
    /// settles, which counts from the first tab, and before the next frame.
    fn arrange_later(self: &Rc<Self>) {
        if self.arranging.replace(true) {
            return;
        }
        let registry = Rc::downgrade(self);
        glib::idle_add_local_full(glib::Priority::HIGH_IDLE, move || {
            if let Some(registry) = registry.upgrade() {
                registry.arranging.set(false);
                if registry.arrange() {
                    registry.version.bump();
                }
            }
            glib::ControlFlow::Break
        });
    }

    /// Shows the tabs with `display: icon` as their icon alone in every
    /// switcher: the title is hidden and names the button, as its tooltip
    /// and accessible label. A switcher makes its buttons anew, one per
    /// page in the pages' order, whenever a page comes or goes, and keeps
    /// them when a title changes.
    fn decorate(&self) {
        let icon_only = self.icon_only.borrow();
        if icon_only.is_empty() {
            return;
        }
        let pages = self.stack.pages();
        for switcher in self.switchers.borrow().iter() {
            let mut button = switcher.first_child();
            for position in 0..pages.n_items() {
                let Some(current) = button else {
                    break;
                };
                button = current.next_sibling();
                let Some(page) = pages.item(position).and_downcast::<adw::ViewStackPage>() else {
                    continue;
                };
                if !page
                    .name()
                    .is_some_and(|name| icon_only.contains(name.as_str()))
                {
                    continue;
                }
                let title = page.title().unwrap_or_default();
                hide_labels(&current, &title);
                current.set_tooltip_text(Some(&title));
                current.update_property(&[gtk::accessible::Property::Label(&title)]);
            }
        }
    }
}

/// Hides the labels below `widget` that show `title`: a switcher button's
/// wide and narrow titles, not its badge.
fn hide_labels(widget: &gtk::Widget, title: &str) {
    let mut child = widget.first_child();
    while let Some(current) = child {
        match current.downcast_ref::<gtk::Label>() {
            Some(label) if label.label() == title => label.set_visible(false),
            Some(_) => {}
            None => hide_labels(&current, title),
        }
        child = current.next_sibling();
    }
}

/// The view switcher inside `widget`, as a switcher bar holds one.
fn inner_switcher(widget: &gtk::Widget) -> Option<adw::ViewSwitcher> {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(switcher) = current.downcast_ref::<adw::ViewSwitcher>() {
            return Some(switcher.clone());
        }
        if let Some(found) = inner_switcher(&current) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

pub struct TabProps {
    pub value: Get,
    /// The enum of the selection, for the names `ui.select` takes.
    pub enum_name: &'static str,
    pub label: Str,
    pub icon: Option<&'static str>,
    /// `display: icon`: the tab shows only its icon, the label naming it.
    pub icon_only: bool,
}

/// One tab; it is part of the tab view while mounted, at the place it is
/// declared. Its content is built while it is selected.
#[allow(non_snake_case)]
pub fn Tab(p: &Parent, ctx: &Ctx, props: TabProps, build: Build) {
    let Some(registry) = ctx.tabs().cloned() else {
        return;
    };
    let value = untrack(|| (props.value)(ctx));
    // The tab's place among the others, in the regions of conditions and
    // lists; the page follows it when a list reorders its rows.
    let marker = gtk::Box::new(gtk::Orientation::Vertical, 0);
    marker.set_visible(false);
    p.add(&marker);
    {
        let registry = Rc::downgrade(&registry);
        marker.connect_parent_notify(move |_| {
            if let Some(registry) = registry.upgrade() {
                registry.arrange_later();
            }
        });
    }
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let index = registry.next.get();
    registry.next.set(index + 1);
    let name = format!("t{index}");
    // Known before the page is added, which makes the switchers' buttons.
    let icon_only = props.icon_only && props.icon.is_some();
    if icon_only {
        registry.icon_only.borrow_mut().insert(name.clone());
    }
    let page = registry.quietly(|| registry.stack.add_titled(&holder, Some(&name), ""));
    if let Some(icon) = props.icon {
        page.set_icon_name(Some(icons::name(icon)));
    }
    let titled = Rc::new(RefCell::new(page));
    let label: Rc<dyn Fn() -> String> = {
        let at = ctx.clone();
        let label = Rc::clone(&props.label);
        Rc::new(move || label(&at))
    };
    {
        let label = Rc::clone(&label);
        let titled = Rc::clone(&titled);
        let named = Rc::downgrade(&registry);
        effect(move || {
            titled.borrow().set_title(Some(&label()));
            if icon_only && let Some(registry) = named.upgrade() {
                registry.decorate();
            }
        });
    }
    let entry = Rc::new(TabEntry {
        name: variant_name(props.enum_name, &value),
        value,
        label,
        icon: props.icon,
        slot: Slot::new(Parent::of_box(&holder)),
        build,
        ctx: ctx.clone(),
        page: name,
        holder,
        titled,
        marker: marker.upcast(),
    });
    registry.entries.borrow_mut().push(Rc::clone(&entry));
    registry.arrange();
    registry.version.bump();
    let weak = Rc::downgrade(&registry);
    on_cleanup(move || {
        if let Some(registry) = weak.upgrade() {
            registry
                .entries
                .borrow_mut()
                .retain(|kept| !Rc::ptr_eq(kept, &entry));
            registry.icon_only.borrow_mut().remove(&entry.page);
            registry.quietly(|| registry.stack.remove(&entry.holder));
            registry.version.bump();
        }
    });
}

/// `TabAccessory`: a bar kept under the pages while `shown` holds.
pub struct TabAccessory {
    pub shown: Option<Get>,
    pub build: Build,
}

/// Tabs as a view stack with a view switcher in the page header, as
/// GNOME preference windows show them. Tabs may come and go; when the
/// selected one leaves, the selection moves to `fallback` or the first tab
/// left. Only the selected tab is built.
#[allow(non_snake_case)]
pub fn Tabs(
    p: &Parent,
    ctx: &Ctx,
    id: Option<&'static str>,
    selection: Field,
    fallback: Option<Get>,
    tabs: Build,
    accessory: Option<TabAccessory>,
) {
    let stack = adw::ViewStack::new();
    // The pages take the room they are given both ways, as on the other
    // targets.
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    // The tabs mark their places in a box that is never shown.
    let declared = Parent::of_box(&gtk::Box::new(gtk::Orientation::Vertical, 0));
    let registry = Rc::new(TabRegistry {
        stack: stack.clone(),
        declared: declared.clone(),
        entries: RefCell::new(Vec::new()),
        version: crate::reactive::Version::new(),
        next: std::cell::Cell::new(0),
        icon_only: RefCell::new(std::collections::HashSet::new()),
        switchers: RefCell::new(Vec::new()),
        updating: std::cell::Cell::new(false),
        arranging: std::cell::Cell::new(false),
    });
    tabs(&declared, &ctx.with_tabs(Rc::clone(&registry)));
    let switcher = adw::ViewSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
    match ctx.page() {
        Some(page) => page.push_title_widget(switcher.clone().upcast()),
        None => {
            let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
            bar.set_hexpand(true);
            switcher.set_halign(gtk::Align::Center);
            bar.append(&switcher);
            p.add(&bar);
        }
    }
    // A compact window switches tabs from a bar under the pages, as
    // GNOME's adaptive applications do; wider ones from the header.
    let switcher_bar = adw::ViewSwitcherBar::new();
    switcher_bar.set_hexpand(true);
    switcher_bar.set_stack(Some(&stack));
    // Tabs with `display: icon` show their icon alone in both switchers,
    // also after pages come and go, which makes their buttons anew; this
    // handler runs after theirs.
    registry.switchers.borrow_mut().push(switcher.clone());
    registry
        .switchers
        .borrow_mut()
        .extend(inner_switcher(switcher_bar.upcast_ref()));
    registry.decorate();
    {
        let weak = Rc::downgrade(&registry);
        stack.pages().connect_items_changed(move |_, _, _, _| {
            if let Some(registry) = weak.upgrade() {
                registry.decorate();
            }
        });
    }
    {
        let at = ctx.clone();
        let (header, bottom) = (switcher.clone(), switcher_bar.clone());
        effect(move || {
            let compact = crate::value::truthy(&at.window("compact"));
            header.set_visible(!compact);
            bottom.set_reveal(compact);
        });
    }
    let shown: Rc<RefCell<Option<Rc<TabEntry>>>> = Rc::new(RefCell::new(None));
    {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        effect(move || {
            registry.version.track();
            let value = at.read(selection);
            let Some(entry) = registry.selected(&value) else {
                return;
            };
            let previous = shown.borrow().clone();
            if previous
                .as_ref()
                .is_some_and(|previous| Rc::ptr_eq(previous, &entry))
            {
                return;
            }
            if let Some(previous) = previous {
                previous.slot.clear();
            }
            *shown.borrow_mut() = Some(Rc::clone(&entry));
            let build = Rc::clone(&entry.build);
            let inner = entry.ctx.clone();
            entry.slot.fill(false, |region| build(region, &inner));
            registry.quietly(|| registry.stack.set_visible_child_name(&entry.page));
        });
    }
    // A selection without a mounted tab moves on, once the update that
    // removed it has mounted whatever replaces it.
    {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        effect(move || {
            registry.version.track();
            at.read(selection);
            let registry = Rc::clone(&registry);
            let at = at.clone();
            let fallback = fallback.clone();
            glib::idle_add_local_once(move || {
                let mounted: Vec<Value> = registry
                    .entries
                    .borrow()
                    .iter()
                    .map(|entry| entry.value.clone())
                    .collect();
                let next = settled(&at.peek(selection), &mounted, || {
                    fallback.map(|fallback| untrack(|| fallback(&at)))
                });
                if let Some(next) = next {
                    at.write(selection, next);
                }
            });
        });
    }
    {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        stack.connect_visible_child_name_notify(move |stack| {
            if registry.updating.get() {
                return;
            }
            let Some(name) = stack.visible_child_name() else {
                return;
            };
            let chosen = registry
                .entries
                .borrow()
                .iter()
                .find(|entry| entry.page == name.as_str())
                .map(|entry| entry.value.clone());
            if let Some(value) = chosen {
                at.write(selection, value);
            }
        });
    }
    if let Some(id) = id {
        let names = Rc::clone(&registry);
        let values = Rc::clone(&registry);
        let labels = Rc::clone(&registry);
        let at = ctx.clone();
        let read = ctx.clone();
        let label_at = ctx.clone();
        controls::register(
            ctx,
            id,
            "tabs",
            Some(Rc::new(move || {
                labels.version.track();
                labels
                    .selected(&label_at.read(selection))
                    .map(|entry| (entry.label)())
                    .unwrap_or_default()
            })),
            None,
            Actions {
                select: Some(Rc::new(move |name: &str| {
                    let found = names
                        .entries
                        .borrow()
                        .iter()
                        .find(|entry| entry.name == name)
                        .map(|entry| entry.value.clone());
                    match found {
                        Some(value) => {
                            at.write(selection, value);
                            true
                        }
                        None => false,
                    }
                })),
                value: Some(Rc::new(move || {
                    values
                        .selected(&read.peek(selection))
                        .map(|entry| entry.name.to_string())
                })),
                ..Actions::default()
            },
        );
    }
    p.add(&stack);
    // The accessory slides up from under the pages, on a card as
    // Adwaita sets a raised bar.
    if let Some(accessory) = accessory {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("card");
        bar.set_margin_start(12);
        bar.set_margin_end(12);
        bar.set_margin_top(6);
        bar.set_margin_bottom(6);
        bar.set_size_request(-1, 48);
        let revealer = gtk::Revealer::new();
        revealer.set_hexpand(true);
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        revealer.set_child(Some(&bar));
        (accessory.build)(&Parent::of_box(&bar), ctx);
        match accessory.shown {
            Some(shown) => {
                let at = ctx.clone();
                let shown_in = revealer.clone();
                effect(move || shown_in.set_reveal_child(crate::value::truthy(&shown(&at))));
            }
            None => revealer.set_reveal_child(true),
        }
        p.add(&revealer);
    }
    p.add(&switcher_bar);
}

/// The layout size of an available width, by the breakpoints of
/// `[layouts]`.
pub(crate) fn size_of(width: f64) -> &'static str {
    let def = host().def;
    let compact = def.compact_below.unwrap_or(0.0);
    if width < compact {
        "compact"
    } else if def.medium_below.is_some_and(|below| width < below) {
        "medium"
    } else if def.expanded_below.is_none_or(|below| width < below) {
        "expanded"
    } else {
        "large"
    }
}

const SIZES: [&str; 4] = ["compact", "medium", "expanded", "large"];

/// Chooses one layout by the width available to it and the breakpoints
/// from `[layouts]`; a size without a layout of its own shows the next
/// larger declared one, else the next smaller. Engine state and drafts
/// live outside the widget tree, so switching keeps them; controls with the
/// same id in several layouts are the same control.
#[allow(non_snake_case)]
pub fn Adaptive(p: &Parent, ctx: &Ctx, id: Option<&'static str>, layouts: [Option<Build>; 4]) {
    // The layout is chosen by the width the container offers, which the
    // layout shown does not widen. Until that is measured the window's
    // width stands in: the room is no wider, so a layout built as a narrow
    // window opens, or inserted into it later, is right from its first
    // build.
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let size = Cell::new(size_of(ctx.win().width.get()));
    let measured = size.clone();
    // A hidden layout has no width to judge by; it keeps its size.
    let holder = crate::layout::Offered::new(&content, move |width| {
        measured.set(size_of(f64::from(width)));
    });
    let slot = Slot::new(Parent::of_box(&content));
    let at = ctx.clone();
    let read = size.clone();
    let shown_size = Rc::new(std::cell::Cell::new(None::<usize>));
    effect(move || {
        let current = SIZES
            .iter()
            .position(|size| *size == read.get())
            .unwrap_or(2);
        let larger = (current..SIZES.len()).find(|index| layouts[*index].is_some());
        let smaller = (0..current).rev().find(|index| layouts[*index].is_some());
        let chosen = larger.or(smaller);
        if shown_size.get() == chosen && chosen.is_some() {
            return;
        }
        shown_size.set(chosen);
        match chosen.and_then(|index| layouts[index].clone()) {
            Some(build) => slot.fill(false, |region| build(region, &at)),
            None => slot.clear(),
        }
    });
    if let Some(id) = id {
        let value = size.clone();
        controls::register(
            ctx,
            id,
            "adaptive",
            Some(Rc::new(String::new)),
            None,
            Actions {
                value: Some(Rc::new(move || Some(value.peek().to_string()))),
                ..Actions::default()
            },
        );
    }
    if p.is_list() {
        p.add(&crate::widgets::plain_row(&holder));
    } else {
        p.add(&holder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::tests::{app, start, unit_enum};

    #[test]
    fn widths_choose_a_layout_size_by_the_breakpoints() {
        let mut def = app();
        def.compact_below = Some(600.0);
        def.medium_below = Some(840.0);
        def.expanded_below = Some(1200.0);
        start(def);
        for (width, size) in [
            (0.0, "compact"),
            (599.0, "compact"),
            (600.0, "medium"),
            (839.5, "medium"),
            (840.0, "expanded"),
            (1199.0, "expanded"),
            (1200.0, "large"),
            (4000.0, "large"),
        ] {
            assert_eq!(size_of(width), size, "{width}");
        }
    }

    #[test]
    fn sizes_the_application_does_not_have_are_skipped() {
        let mut def = app();
        def.compact_below = Some(600.0);
        start(def);
        assert_eq!(size_of(599.0), "compact");
        assert_eq!(size_of(600.0), "expanded");
        assert_eq!(size_of(10_000.0), "expanded");
    }

    /// When the selected item leaves, sidebars and tabs go to the fallback
    /// while it is mounted, else to the first item, as on the other
    /// targets.
    #[test]
    fn a_selection_without_its_item_moves_to_the_fallback_or_the_first() {
        let [a, b, c, d] = [0, 1, 2, 3].map(Value::unit_variant);
        let mounted = [a.clone(), c.clone(), d.clone()];
        let unused = || -> Option<Value> { panic!("the fallback is not read") };
        assert_eq!(settled(&c, &mounted, unused), None);
        assert_eq!(settled(&b, &[], unused), None);
        assert_eq!(settled(&b, &mounted, || Some(d.clone())), Some(d.clone()));
        assert_eq!(settled(&b, &mounted, || None), Some(a.clone()));
        // A fallback that is not mounted either is passed over.
        assert_eq!(settled(&d, &mounted[..2], || Some(b.clone())), Some(a));
    }

    /// Tabs show in the order they are declared, as on the other targets,
    /// also when a condition mounts one after the others; only the pages
    /// after it come back.
    #[test]
    fn tab_pages_follow_the_declared_order() {
        // Declared in order: nothing moves.
        assert_eq!(arranged(&[0, 1, 2, 3]), (vec![0, 1, 2, 3], 4));
        assert_eq!(arranged(&[]), (vec![], 0));
        // The second tab came back last: the two after it come back.
        assert_eq!(arranged(&[0, 2, 3, 1]), (vec![0, 3, 1, 2], 2));
        // The first did: all the others.
        assert_eq!(arranged(&[1, 2, 3, 0]), (vec![3, 0, 1, 2], 1));
        // A list moved its last row first: the rest follow it.
        assert_eq!(arranged(&[4, 6, 8, 2]), (vec![3, 0, 1, 2], 1));
        // It swapped the first two rows: the one now second and those
        // after it come back.
        assert_eq!(arranged(&[1, 0, 2]), (vec![1, 0, 2], 1));
        // A tab without a declared place stays last.
        assert_eq!(arranged(&[usize::MAX, 0, 1]), (vec![1, 2, 0], 2));
    }

    /// Rows built late come in order once due; those whose list left are
    /// dropped, and without a time all come at once.
    #[test]
    fn late_rows_come_in_order_once_due() {
        let began = std::time::Instant::now();
        let late = Late::default();
        let owners: Vec<Scope> = (0..3).map(|_| Scope::detached()).collect();
        for (turn, owner) in owners.iter().enumerate() {
            late.rows.borrow_mut().push_back(Waiting {
                due: began + std::time::Duration::from_millis(30 * turn as u64 + 30),
                began,
                turn: turn as u32 + 1,
                step: 0.03,
                owner: owner.clone(),
                fill: Box::new(|_| {}),
            });
        }
        let at = |ms| Some(began + std::time::Duration::from_millis(ms));
        assert!(late.next(at(10)).is_none());
        assert_eq!(late.next(at(31)).map(|row| row.turn), Some(1));
        assert!(late.next(at(31)).is_none());
        owners[1].dispose();
        assert!(late.next(at(70)).is_none());
        assert_eq!(late.next(None).map(|row| row.turn), Some(3));
        assert!(late.rows.borrow().is_empty());
    }

    #[test]
    fn enum_values_are_selected_by_variant_name() {
        let mut def = app();
        def.types.enums = vec![unit_enum("Section", &["Inbox", "Archive"])];
        start(def);
        assert_eq!(variant_name("Section", &Value::unit_variant(1)), "Archive");
        assert_eq!(variant_name("Section", &Value::unit_variant(5)), "");
        assert_eq!(variant_name("Other", &Value::unit_variant(0)), "");
        assert_eq!(variant_name("Section", &Value::Int(1)), "");
    }
}
