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
use crate::{Act, Branch, Build, Ctx, Get, Parent, Str, icons, windows};
use adw::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Content that a builder replaces: the current scope and region.
struct Slot {
    region: Parent,
    owner: RefCell<Option<Scope>>,
}

impl Slot {
    fn new(region: Parent) -> Rc<Self> {
        let slot = Rc::new(Self {
            region,
            owner: RefCell::new(None),
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
    fn fill(&self, build: impl FnOnce(&Parent)) {
        untrack(|| {
            self.end();
            self.region.clear();
            let owner = Scope::detached();
            owner.run(|| build(&self.region));
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
                slot.fill(|region| arm(region, &ctx));
            }
            None => slot.clear(),
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
/// stays; its value is a cell the row's bindings read.
#[allow(non_snake_case)]
pub fn Each(p: &Parent, ctx: &Ctx, items: Get, key: usize, var: &'static str, row: Build) {
    let region = p.region();
    let rows: Rc<RefCell<Vec<Row>>> = Rc::new(RefCell::new(Vec::new()));
    let ended = Rc::clone(&rows);
    on_cleanup(move || {
        for row in ended.borrow_mut().drain(..) {
            row.owner.dispose();
        }
    });
    let ctx = ctx.clone();
    effect(move || {
        let list = as_list(items(&ctx));
        untrack(|| {
            let previous: Vec<Row> = rows.borrow_mut().drain(..).collect();
            let order_before: Vec<String> = previous.iter().map(|row| row.key.clone()).collect();
            let mut old: HashMap<String, Row> = previous
                .into_iter()
                .map(|row| (row.key.clone(), row))
                .collect();
            let mut next = Vec::new();
            for item in list {
                let id = key_text(&at(item.clone(), &[key]));
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
                owner.run(|| build(&child, &row_ctx));
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

/// A destination in the sidebar.
pub struct SidebarItem {
    pub value: Get,
    /// The variant name, for `ui.select`.
    pub name: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
}

pub struct SidebarSection {
    pub title: Option<Str>,
    pub items: Vec<SidebarItem>,
}

pub struct SplitSpec {
    pub id: Option<&'static str>,
    pub selection: Field,
    pub search: Option<(SearchProps, Option<Act>)>,
    pub sections: Vec<SidebarSection>,
}

/// The sidebar and detail split of GNOME applications: a navigation
/// sidebar with optional search, and a detail page whose header shows the
/// selected destination, the detail's tools and back.
#[allow(non_snake_case)]
pub fn SplitView(p: &Parent, ctx: &Ctx, spec: SplitSpec, detail: Build) {
    if let Some(page) = ctx.page() {
        page.hide_header();
    }
    let split = adw::NavigationSplitView::new();
    split.set_vexpand(true);
    split.set_hexpand(true);

    // Sidebar
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_selection_mode(gtk::SelectionMode::Single);
    struct Entry {
        value: Value,
        name: &'static str,
        label: Rc<dyn Fn() -> String>,
        row: gtk::ListBoxRow,
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut headings: Vec<(gtk::ListBoxRow, Rc<dyn Fn() -> String>)> = Vec::new();
    for section in spec.sections {
        let mut first = true;
        for item in section.items {
            let value = untrack(|| (item.value)(ctx));
            let label: Rc<dyn Fn() -> String> = match item.label {
                Some(label) => {
                    let at = ctx.clone();
                    Rc::new(move || label(&at))
                }
                None => Rc::new(String::new),
            };
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            if let Some(icon) = item.icon {
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
            row.set_child(Some(&content));
            list.append(&row);
            if first && let Some(title) = &section.title {
                let at = ctx.clone();
                let title = Rc::clone(title);
                headings.push((row.clone(), Rc::new(move || title(&at))));
            }
            first = false;
            entries.push(Entry {
                value,
                name: item.name,
                label,
                row,
            });
        }
    }
    let headings = Rc::new(headings);
    let header_titles = Rc::clone(&headings);
    list.set_header_func(move |row, _| {
        let title = header_titles
            .iter()
            .find(|(candidate, _)| candidate == row)
            .map(|(_, title)| title());
        match title {
            Some(title) => {
                let heading = gtk::Label::new(Some(&title));
                heading.set_xalign(0.0);
                heading.add_css_class("caption-heading");
                heading.add_css_class("dim-label");
                heading.set_margin_top(12);
                heading.set_margin_bottom(6);
                heading.set_margin_start(12);
                row.set_header(Some(&heading));
            }
            None => row.set_header(None::<&gtk::Widget>),
        }
    });
    let invalidated = list.clone();
    let titles = Rc::clone(&headings);
    effect(move || {
        // Section titles follow the language.
        for (_, title) in titles.iter() {
            title();
        }
        invalidated.invalidate_headers();
    });
    let entries = Rc::new(entries);
    let selection = spec.selection;
    let updating = Rc::new(std::cell::Cell::new(false));
    let selected = Rc::clone(&entries);
    let at = ctx.clone();
    let guard = Rc::clone(&updating);
    let shown = list.clone();
    effect(move || {
        let value = at.read(selection);
        let row = selected
            .iter()
            .find(|entry| entry.value == value)
            .map(|entry| entry.row.clone());
        if shown.selected_row() != row {
            guard.set(true);
            shown.select_row(row.as_ref());
            guard.set(false);
        }
    });
    let chosen = Rc::clone(&entries);
    let at = ctx.clone();
    let revealed = split.clone();
    list.connect_row_selected(move |_, row| {
        if updating.get() {
            return;
        }
        let Some(row) = row else { return };
        if let Some(entry) = chosen.iter().find(|entry| &entry.row == row) {
            at.write(selection, entry.value.clone());
            revealed.set_show_content(true);
        }
    });
    let activated = split.clone();
    list.connect_row_activated(move |_, _| activated.set_show_content(true));

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if let Some((search, submit)) = spec.search {
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        holder.set_margin_top(6);
        holder.set_margin_bottom(6);
        holder.set_margin_start(12);
        holder.set_margin_end(12);
        crate::widgets::Search(&Parent::of_box(&holder), ctx, search, submit);
        sidebar_box.append(&holder);
    }
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&list));
    sidebar_box.append(&scrolled);
    let sidebar_view = adw::ToolbarView::new();
    let sidebar_header = adw::HeaderBar::new();
    let app_title = adw::WindowTitle::new("", "");
    sidebar_header.set_title_widget(Some(&app_title));
    sidebar_view.add_top_bar(&sidebar_header);
    sidebar_view.set_content(Some(&sidebar_box));
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
        let entries = Rc::clone(&entries);
        let at = ctx.clone();
        Rc::new(move || {
            let value = at.read(selection);
            entries
                .iter()
                .find(|entry| entry.value == value)
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
        let names = Rc::clone(&entries);
        let at = ctx.clone();
        let revealed = split.clone();
        let current = Rc::clone(&entries);
        let read = ctx.clone();
        controls::register(
            ctx,
            id,
            "sidebar",
            Some(selected_label),
            None,
            Actions {
                select: Some(Rc::new(move |name: &str| {
                    match names.iter().find(|entry| entry.name == name) {
                        Some(entry) => {
                            at.write(selection, entry.value.clone());
                            revealed.set_show_content(true);
                            true
                        }
                        None => false,
                    }
                })),
                value: Some(Rc::new(move || {
                    let value = read.peek(selection);
                    current
                        .iter()
                        .find(|entry| entry.value == value)
                        .map(|entry| entry.name.to_string())
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
        slot.fill(|region| {
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

/// A tab of `Tabs`.
pub struct TabSpec {
    pub value: Get,
    pub name: &'static str,
    pub label: Str,
    pub icon: Option<&'static str>,
    pub build: Build,
}

/// Tabs as a view stack with a view switcher in the page header, as
/// GNOME preference windows show them. Only the selected tab is built.
#[allow(non_snake_case)]
pub fn Tabs(p: &Parent, ctx: &Ctx, id: Option<&'static str>, selection: Field, tabs: Vec<TabSpec>) {
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    let values: Vec<Value> = tabs
        .iter()
        .map(|tab| untrack(|| (tab.value)(ctx)))
        .collect();
    let mut slots = Vec::new();
    for (index, tab) in tabs.iter().enumerate() {
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let page = stack.add_titled(&holder, Some(&format!("t{index}")), "");
        if let Some(icon) = tab.icon {
            page.set_icon_name(Some(icons::name(icon)));
        }
        let label = Rc::clone(&tab.label);
        let at = ctx.clone();
        effect(move || page.set_title(Some(&label(&at))));
        slots.push(Slot::new(Parent::of_box(&holder)));
    }
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
    let values = Rc::new(values);
    let builders: Vec<Build> = tabs.iter().map(|tab| Rc::clone(&tab.build)).collect();
    let shown: Rc<std::cell::Cell<Option<usize>>> = Rc::new(std::cell::Cell::new(None));
    let updating = Rc::new(std::cell::Cell::new(false));
    let at = ctx.clone();
    let guard = Rc::clone(&updating);
    let visible = stack.clone();
    let read = Rc::clone(&values);
    effect(move || {
        let value = at.read(selection);
        let Some(index) = read.iter().position(|candidate| *candidate == value) else {
            return;
        };
        if shown.get() == Some(index) {
            return;
        }
        if let Some(previous) = shown.get() {
            slots[previous].clear();
        }
        shown.set(Some(index));
        let build = Rc::clone(&builders[index]);
        let at = at.clone();
        slots[index].fill(|region| build(region, &at));
        guard.set(true);
        visible.set_visible_child_name(&format!("t{index}"));
        guard.set(false);
    });
    let at = ctx.clone();
    let chosen = Rc::clone(&values);
    stack.connect_visible_child_name_notify(move |stack| {
        if updating.get() {
            return;
        }
        let index = stack.visible_child_name().and_then(|name| {
            name.strip_prefix('t')
                .and_then(|index| index.parse::<usize>().ok())
        });
        if let Some(value) = index.and_then(|index| chosen.get(index)) {
            at.write(selection, value.clone());
        }
    });
    if let Some(id) = id {
        let names: Rc<Vec<&'static str>> = Rc::new(tabs.iter().map(|tab| tab.name).collect());
        let labels: Rc<Vec<Str>> = Rc::new(tabs.iter().map(|tab| Rc::clone(&tab.label)).collect());
        let select_names = Rc::clone(&names);
        let select_values = Rc::clone(&values);
        let at = ctx.clone();
        let value_names = Rc::clone(&names);
        let value_values = Rc::clone(&values);
        let read = ctx.clone();
        let label_values = Rc::clone(&values);
        let label_at = ctx.clone();
        controls::register(
            ctx,
            id,
            "tabs",
            Some(Rc::new(move || {
                let value = label_at.read(selection);
                label_values
                    .iter()
                    .position(|candidate| *candidate == value)
                    .map(|index| labels[index](&label_at))
                    .unwrap_or_default()
            })),
            None,
            Actions {
                select: Some(Rc::new(move |name: &str| {
                    match select_names.iter().position(|candidate| *candidate == name) {
                        Some(index) => {
                            at.write(selection, select_values[index].clone());
                            true
                        }
                        None => false,
                    }
                })),
                value: Some(Rc::new(move || {
                    let value = read.peek(selection);
                    value_values
                        .iter()
                        .position(|candidate| *candidate == value)
                        .map(|index| value_names[index].to_string())
                })),
                ..Actions::default()
            },
        );
    }
    p.add(&stack);
}

/// Chooses one layout by the width available to it and the breakpoint
/// from `[layouts]`. Engine state and drafts live outside the widget tree,
/// so switching keeps them; controls with the same id in both layouts are
/// the same control.
#[allow(non_snake_case)]
pub fn Adaptive(
    p: &Parent,
    ctx: &Ctx,
    id: Option<&'static str>,
    compact: Option<Build>,
    expanded: Option<Build>,
) {
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
    let below = host().def.compact_below.unwrap_or(0.0);
    let measured = size.clone();
    probe.connect_resize(move |_, width, _| {
        // A hidden layout has no width to judge by; it keeps its size.
        if width > 0 {
            measured.set(if (width as f64) < below {
                "compact"
            } else {
                "expanded"
            });
        }
    });
    let slot = Slot::new(Parent::of_box(&content));
    let at = ctx.clone();
    let read = size.clone();
    effect(move || {
        let current = read.get();
        let build = if current == "compact" {
            compact.clone().or_else(|| expanded.clone())
        } else {
            expanded.clone().or_else(|| compact.clone())
        };
        match build {
            Some(build) => slot.fill(|region| build(region, &at)),
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
