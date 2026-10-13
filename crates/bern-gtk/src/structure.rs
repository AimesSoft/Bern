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

struct Row {
    key: String,
    value: Cell<Value>,
    owner: Scope,
    region: Parent,
}

/// Keyed rows: a row keeps its widgets, controls and state while its key
/// stays; its value is a cell the row's bindings read. Rows added after the
/// first ones appear with their transitions; with `stagger` they take
/// turns.
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
    on_cleanup(move || {
        for row in ended.borrow_mut().drain(..) {
            row.owner.dispose();
        }
    });
    let ctx = ctx.clone();
    let outer = motion::clock();
    let started = std::cell::Cell::new(false);
    let first_entering = motion::entering();
    effect(move || {
        let list = as_list(items(&ctx));
        let entering = started.replace(true) || first_entering;
        let clock = match stagger {
            Some(step) if !motion::reduced() => {
                Some(motion::with_entering(entering, || motion::new_clock(step)))
            }
            _ => outer.clone(),
        };
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
                owner.run(|| {
                    motion::with_entering(entering, || {
                        motion::with_clock(clock.clone(), || build(&child, &row_ctx))
                    });
                    // Rows are the items a scroll view comes to rest on.
                    if let (Some(scroll), Some(first)) = (scroll, child.widgets().first()) {
                        scroll.add_item(key_value, first);
                    }
                });
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
            *rows.borrow_mut() = next;
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
    text.set_xalign(0.0);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let shown = text.clone();
    let read = Rc::clone(&label);
    effect(move || shown.set_label(&read()));
    content.append(&text);
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

pub struct SplitSpec {
    pub id: Option<&'static str>,
    pub selection: Field,
    /// The item chosen when the selected one leaves.
    pub fallback: Option<Get>,
    pub search: Option<(SearchProps, Option<Act>)>,
    /// Content at the foot of the sidebar, below the list.
    pub footer: Option<Build>,
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
                let current = at.peek(selection);
                let entries = registry.ordered();
                if entries.is_empty() || entries.iter().any(|entry| entry.value == current) {
                    return;
                }
                let back = fallback
                    .map(|fallback| untrack(|| fallback(&at)))
                    .filter(|fallback| entries.iter().any(|entry| entry.value == *fallback));
                let next = back.unwrap_or_else(|| entries[0].value.clone());
                at.write(selection, next);
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
    let header = windows::header(&page, Some(Rc::clone(&selected_label)));
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
    slot: Rc<Slot>,
    build: Build,
    ctx: Ctx,
    page: String,
}

/// The tabs a tab view shows now, as they mount and leave.
pub struct TabRegistry {
    stack: adw::ViewStack,
    entries: RefCell<Vec<Rc<TabEntry>>>,
    version: crate::reactive::Version,
    next: std::cell::Cell<u32>,
}

impl TabRegistry {
    fn selected(&self, value: &Value) -> Option<Rc<TabEntry>> {
        self.entries
            .borrow()
            .iter()
            .find(|entry| entry.value == *value)
            .cloned()
    }
}

pub struct TabProps {
    pub value: Get,
    /// The enum of the selection, for the names `ui.select` takes.
    pub enum_name: &'static str,
    pub label: Str,
    pub icon: Option<&'static str>,
}

/// One tab; it is part of the tab view while mounted. Its content is built
/// while it is selected.
#[allow(non_snake_case)]
pub fn Tab(_p: &Parent, ctx: &Ctx, props: TabProps, build: Build) {
    let Some(registry) = ctx.tabs().cloned() else {
        return;
    };
    let value = untrack(|| (props.value)(ctx));
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let index = registry.next.get();
    registry.next.set(index + 1);
    let name = format!("t{index}");
    let page = registry.stack.add_titled(&holder, Some(&name), "");
    if let Some(icon) = props.icon {
        page.set_icon_name(Some(icons::name(icon)));
    }
    let label: Rc<dyn Fn() -> String> = {
        let at = ctx.clone();
        let label = Rc::clone(&props.label);
        Rc::new(move || label(&at))
    };
    let titled = Rc::clone(&label);
    effect(move || page.set_title(Some(&titled())));
    let entry = Rc::new(TabEntry {
        name: variant_name(props.enum_name, &value),
        value,
        label,
        slot: Slot::new(Parent::of_box(&holder)),
        build,
        ctx: ctx.clone(),
        page: name,
    });
    registry.entries.borrow_mut().push(Rc::clone(&entry));
    registry.version.bump();
    let weak = Rc::downgrade(&registry);
    on_cleanup(move || {
        if let Some(registry) = weak.upgrade() {
            registry
                .entries
                .borrow_mut()
                .retain(|kept| !Rc::ptr_eq(kept, &entry));
            registry.stack.remove(&holder);
            registry.version.bump();
        }
    });
}

/// Tabs as a view stack with a view switcher in the page header, as
/// GNOME preference windows show them. Tabs may come and go; only the
/// selected tab is built.
#[allow(non_snake_case)]
pub fn Tabs(p: &Parent, ctx: &Ctx, id: Option<&'static str>, selection: Field, tabs: Build) {
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    let registry = Rc::new(TabRegistry {
        stack: stack.clone(),
        entries: RefCell::new(Vec::new()),
        version: crate::reactive::Version::new(),
        next: std::cell::Cell::new(0),
    });
    let (declared, _) = Parent::capture(p);
    tabs(&declared, &ctx.with_tabs(Rc::clone(&registry)));
    let switcher = adw::ViewSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
    match ctx.page() {
        Some(page) => page.push_title_widget(switcher.upcast()),
        None => {
            let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
            switcher.set_halign(gtk::Align::Center);
            bar.append(&switcher);
            p.add(&bar);
        }
    }
    let shown: Rc<RefCell<Option<Rc<TabEntry>>>> = Rc::new(RefCell::new(None));
    let updating = Rc::new(std::cell::Cell::new(false));
    {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        let guard = Rc::clone(&updating);
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
            guard.set(true);
            registry.stack.set_visible_child_name(&entry.page);
            guard.set(false);
        });
    }
    {
        let registry = Rc::clone(&registry);
        let at = ctx.clone();
        stack.connect_visible_child_name_notify(move |stack| {
            if updating.get() {
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
    // A probe with no height measures the width the layout gets.
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_hexpand(true);
    let probe = gtk::DrawingArea::new();
    probe.set_content_height(0);
    probe.set_hexpand(true);
    holder.append(&probe);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.append(&content);
    let size = Cell::new("expanded");
    let measured = size.clone();
    probe.connect_resize(move |_, width, _| {
        // A hidden layout has no width to judge by; it keeps its size.
        if width > 0 {
            measured.set(size_of(f64::from(width)));
        }
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
