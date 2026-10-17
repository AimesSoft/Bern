//! The controls media applications need: buttons showing content,
//! toggle, split and popover buttons, notices, the inspector panel, drop
//! areas, pinch and swipe, rails, scroll views that come to rest on an
//! item, pictures from engine state, file panels and keyboard shortcuts.

use crate::controls::{self, Actions};
use crate::ctx::Field;
use crate::layout::{Clip, Placement, Rail as RailBox};
use crate::motion::{self, Curve, MotionBox, Num, Spring, bind_number};
use crate::protocol::Value;
use crate::reactive::{Scope, effect, on_cleanup, untrack};
use crate::style::StyleDef;
use crate::widgets::{Bound, bound_action, bound_enabled, bound_text, place};
use crate::{Act, Build, Ctx, Get, Parent, Str, icons};
use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

// MARK: - Files

/// Takes a file by its URL, as a drop or the open panel gives it.
pub(crate) type Give = Rc<dyn Fn(String)>;

/// A `MediaSource` value for a URL.
pub(crate) fn media_source(url: String) -> Value {
    Value::Record(vec![Value::Text(url)])
}

fn extension_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default();
    match name.rsplit_once('.') {
        Some((_, extension)) if !extension.is_empty() => extension.to_lowercase(),
        _ => String::new(),
    }
}

/// Whether a file at `url` is one of `kinds`, by its extension.
pub(crate) fn accepts(url: &str, kinds: &str) -> bool {
    let extension = extension_of(url);
    let table = crate::host::host().def.file_kinds;
    kinds
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .any(|word| match word {
            "any" => true,
            "folder" => false,
            word if word.starts_with('.') => extension == word[1..],
            kind => table
                .iter()
                .find(|(name, _)| *name == kind)
                .is_some_and(|(_, extensions)| extensions.contains(&extension.as_str())),
        })
}

/// Filters of the open panel for `kinds`: one for all of them.
fn filters(kinds: &str) -> Option<gio::ListStore> {
    let table = crate::host::host().def.file_kinds;
    let filter = gtk::FileFilter::new();
    let mut any = false;
    for word in kinds
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
    {
        match word {
            "any" | "folder" => return None,
            word if word.starts_with('.') => {
                filter.add_suffix(&word[1..]);
                any = true;
            }
            kind => {
                let mime = match kind {
                    "video" => Some("video/*"),
                    "audio" => Some("audio/*"),
                    "picture" => Some("image/*"),
                    _ => None,
                };
                if let Some(mime) = mime {
                    filter.add_mime_type(mime);
                }
                if let Some((_, extensions)) = table.iter().find(|(name, _)| *name == kind) {
                    for extension in *extensions {
                        filter.add_suffix(extension);
                    }
                }
                any = true;
            }
        }
    }
    if !any {
        return None;
    }
    filter.set_name(Some(kinds));
    let store = gio::ListStore::new::<gtk::FileFilter>();
    store.append(&filter);
    Some(store)
}

/// The system's open panel for `kinds`; `give` runs with the chosen file's
/// URL, nothing on cancel.
pub(crate) fn pick(ctx: &Ctx, kinds: &'static str, give: Rc<dyn Fn(String)>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_modal(true);
    if let Some(filters) = filters(kinds) {
        dialog.set_filters(Some(&filters));
    }
    let window = ctx.win().window.clone();
    let chosen = move |result: Result<gio::File, glib::Error>| {
        if let Ok(file) = result {
            give(file.uri().to_string());
        }
    };
    if kinds.split_whitespace().any(|word| word == "folder") {
        dialog.select_folder(Some(&window), None::<&gio::Cancellable>, chosen);
    } else {
        dialog.open(Some(&window), None::<&gio::Cancellable>, chosen);
    }
}

/// `pick:` of a button or menu item: the field the file goes into, and the
/// kinds it takes.
#[derive(Clone, Copy)]
pub struct Pick {
    pub field: Field,
    pub kinds: &'static str,
}

/// For a control with `pick`: what pressing it does (ask for a file, then
/// act) and what `ui.set_file` does (take the file, then act).
pub(crate) fn picking(
    ctx: &Ctx,
    pick: Option<Pick>,
    action: Option<Rc<dyn Fn()>>,
) -> (Option<crate::Thunk>, Option<Give>) {
    let Some(pick) = pick else {
        return (action, None);
    };
    let give: Rc<dyn Fn(String)> = {
        let at = ctx.clone();
        Rc::new(move |url| {
            at.write(pick.field, media_source(url));
            if let Some(action) = &action {
                action();
            }
        })
    };
    let press: Rc<dyn Fn()> = {
        let at = ctx.clone();
        let give = Rc::clone(&give);
        Rc::new(move || pick_file(&at, pick.kinds, Rc::clone(&give)))
    };
    (Some(press), Some(give))
}

fn pick_file(ctx: &Ctx, kinds: &'static str, give: Rc<dyn Fn(String)>) {
    pick(ctx, kinds, give);
}

// MARK: - Shortcuts

/// Run `run` on `keys` (such as `cmd+o`, where `cmd` is Control) while the
/// current scope lasts.
pub fn shortcut(ctx: &Ctx, keys: &'static str, run: Act) {
    let at = ctx.clone();
    ctx.win().add_shortcut(keys, Rc::new(move || run(&at)));
}

/// Give the keyboard focus to the control `id`, as `ui.focus` does.
pub fn focus_target(ctx: &Ctx, id: &str) {
    if let Some(focus) = controls::find(ctx, id).and_then(|entry| entry.actions.focus) {
        focus();
    }
}

/// Run the control `id`'s `activate`, as `ui.activate` does.
pub fn activate_target(ctx: &Ctx, id: &str) {
    if let Some(activate) = controls::find(ctx, id).and_then(|entry| entry.actions.activate) {
        activate();
    }
}

// MARK: - Buttons with content

pub struct ContentButtonProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub enabled: Option<Get>,
    pub style: Option<&'static StyleDef>,
    pub pick: Option<Pick>,
}

/// A button showing its content instead of a label: no frame of its own,
/// the content as it is; a style's corner clips it and its shadow follows
/// the clipped shape. Without a style the content dims while pressed.
#[allow(non_snake_case)]
pub fn ContentButton(
    p: &Parent,
    ctx: &Ctx,
    props: ContentButtonProps,
    action: Option<Act>,
    content: Build,
) {
    let button = gtk::Button::new();
    button.add_css_class("bern-content-button");
    button.set_overflow(gtk::Overflow::Hidden);
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content(&Parent::of_box(&holder), ctx);
    button.set_child(Some(&holder));
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if let Some(label) = label.clone() {
        let named = button.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
    let enabled = bound_enabled(ctx, &props.enabled);
    if let Some(enabled) = enabled.clone() {
        let shown = button.clone();
        effect(move || shown.set_sensitive(enabled()));
    }
    let (run, set_file) = picking(ctx, props.pick, bound_action(ctx, &action));
    if let Some(run) = run.clone() {
        button.connect_clicked(move |_| run());
    }
    controls::register(
        ctx,
        props.id,
        "button",
        label,
        enabled,
        Actions {
            activate: run,
            set_file,
            file_kinds: props.pick.map(|pick| pick.kinds),
            ..Actions::default()
        },
    );
    let motion = MotionBox::wrap(&button);
    match props.style {
        Some(style) => {
            crate::style::apply(style, button.upcast_ref(), button.upcast_ref(), &motion)
        }
        None => button.add_css_class("bern-content-plain"),
    }
    place(p, &motion);
}

// MARK: - Toggle buttons

pub struct ToggleProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Field,
    pub icon: Option<&'static str>,
}

/// A button that stays pressed while `field` is on.
fn toggle_widget(
    ctx: &Ctx,
    label: Option<Rc<dyn Fn() -> String>>,
    field: Field,
    icon: Option<&'static str>,
    icon_only: bool,
) -> gtk::ToggleButton {
    let bound = Bound::new(ctx, field);
    let button = gtk::ToggleButton::new();
    match (icon, label.clone()) {
        (Some(icon), _) if icon_only => button.set_icon_name(icons::name(icon)),
        (Some(icon), Some(label)) => {
            let content = adw::ButtonContent::new();
            content.set_icon_name(icons::name(icon));
            let shown = content.clone();
            effect(move || shown.set_label(&label()));
            button.set_child(Some(&content));
        }
        (Some(icon), None) => button.set_icon_name(icons::name(icon)),
        (None, Some(label)) => {
            let shown = button.clone();
            effect(move || shown.set_label(&label()));
        }
        (None, None) => {}
    }
    button.set_valign(gtk::Align::Center);
    if let Some(label) = label {
        let named = button.clone();
        effect(move || {
            let text = label();
            named.set_tooltip_text(Some(&text));
            named.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = button.clone();
    let read = bound.clone();
    effect(move || {
        let on = read.get() == Value::Bool(true);
        if shown.is_active() != on {
            guard.set(true);
            shown.set_active(on);
            guard.set(false);
        }
    });
    button.connect_toggled(move |button| {
        if !updating.get() {
            bound.set(Value::Bool(button.is_active()));
        }
    });
    button
}

/// The actions of a control bound to a `bool` field.
fn switch_actions(ctx: &Ctx, field: Field) -> Actions {
    let bound = Bound::new(ctx, field);
    let set = bound.clone();
    let toggle = bound.clone();
    Actions {
        set_bool: Some(Rc::new(move |on| set.set(Value::Bool(on)))),
        activate: Some(Rc::new(move || {
            toggle.set(Value::Bool(toggle.peek() != Value::Bool(true)))
        })),
        value: Some(Rc::new(move || {
            Some((bound.peek() == Value::Bool(true)).to_string())
        })),
        ..Actions::default()
    }
}

/// `Switch(style: button)`, and a switch in a toolbar: a button that stays
/// pressed while on. In a toolbar one with an icon shows only the icon.
pub(crate) fn toggle_button(p: &Parent, ctx: &Ctx, props: ToggleProps) {
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let icon_only = props.icon.is_some() && ctx.in_toolbar();
    let button = toggle_widget(ctx, label.clone(), props.field, props.icon, icon_only);
    if ctx.in_toolbar() {
        button.add_css_class("flat");
    }
    place(p, &button);
    controls::register(
        ctx,
        props.id,
        "switch",
        label,
        None,
        switch_actions(ctx, props.field),
    );
}

// MARK: - Split and popover buttons

/// The content of a popover: a scrolling column with room around it.
fn popover_content() -> (gtk::Popover, gtk::Box) {
    let popover = gtk::Popover::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    column.set_size_request(240, -1);
    popover.set_child(Some(&column));
    (popover, column)
}

pub struct SplitButtonProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub field: Field,
    pub more: Option<Str>,
}

/// A toggle linked with a button that shows its children in a popover,
/// such as the usual settings of what the toggle turns on.
#[allow(non_snake_case)]
pub fn SplitButton(p: &Parent, ctx: &Ctx, props: SplitButtonProps, content: Build) {
    let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    group.add_css_class("linked");
    group.set_valign(gtk::Align::Center);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let toggle = toggle_widget(
        ctx,
        label.clone(),
        props.field,
        props.icon,
        props.icon.is_some(),
    );
    group.append(&toggle);
    let more = gtk::MenuButton::new();
    more.set_icon_name("pan-down-symbolic");
    let more_label = match &props.more {
        Some(more) => bound_text(ctx, more),
        None => Rc::new(|| crate::text::tr("bern.split.more")),
    };
    let named = more.clone();
    effect(move || {
        let text = more_label();
        named.set_tooltip_text(Some(&text));
        named.update_property(&[gtk::accessible::Property::Label(&text)]);
    });
    let (popover, column) = popover_content();
    content(&Parent::of_box(&column), &ctx.outside_toolbar());
    more.set_popover(Some(&popover));
    group.append(&more);
    place(p, &group);
    let opened = more.clone();
    controls::register(
        ctx,
        props.id,
        "split_button",
        label,
        None,
        Actions {
            activate: Some(Rc::new(move || opened.popup())),
            ..switch_actions(ctx, props.field)
        },
    );
}

pub struct PopoverButtonProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub presented: Option<Field>,
}

/// A button that shows its children in a popover. The children mount
/// while the popover shows; `presented` follows it and opens or closes it.
#[allow(non_snake_case)]
pub fn PopoverButton(p: &Parent, ctx: &Ctx, props: PopoverButtonProps, content: Build) {
    let button = gtk::MenuButton::new();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    match (props.icon, label.clone()) {
        (Some(icon), _) => button.set_icon_name(icons::name(icon)),
        (None, Some(label)) => {
            let shown = button.clone();
            effect(move || shown.set_label(&label()));
        }
        (None, None) => {}
    }
    button.set_valign(gtk::Align::Center);
    if let Some(label) = label.clone() {
        let named = button.clone();
        effect(move || {
            let text = label();
            named.set_tooltip_text(Some(&text));
            named.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let (popover, column) = popover_content();
    button.set_popover(Some(&popover));
    let open = crate::reactive::Cell::new(false);
    let owner: Rc<RefCell<Option<Scope>>> = Rc::new(RefCell::new(None));
    {
        let owner = Rc::clone(&owner);
        let at = ctx.outside_toolbar();
        let column = column.clone();
        let open = open.clone();
        let presented = props.presented;
        let window = ctx.clone();
        popover.connect_show(move |_| {
            if owner.borrow().is_some() {
                return;
            }
            let scope = Scope::detached();
            untrack(|| scope.run(|| content(&Parent::of_box(&column), &at)));
            *owner.borrow_mut() = Some(scope);
            open.set(true);
            if let Some(field) = presented
                && window.peek(field) != Value::Bool(true)
            {
                window.write(field, Value::Bool(true));
            }
        });
    }
    {
        let owner = Rc::clone(&owner);
        let column = column.clone();
        let open = open.clone();
        let presented = props.presented;
        let window = ctx.clone();
        popover.connect_closed(move |_| {
            let scope = owner.borrow_mut().take();
            if let Some(scope) = scope {
                scope.dispose();
            }
            while let Some(child) = column.first_child() {
                column.remove(&child);
            }
            open.set(false);
            if let Some(field) = presented
                && window.peek(field) == Value::Bool(true)
            {
                window.write(field, Value::Bool(false));
            }
        });
    }
    if let Some(field) = props.presented {
        let at = ctx.clone();
        let shown = button.clone();
        effect(move || {
            let want = at.read(field) == Value::Bool(true);
            let shown = shown.clone();
            // Presented from the start: open once the button is on screen.
            glib::idle_add_local_once(move || {
                if want && shown.is_mapped() && !shown.is_active() {
                    shown.popup();
                } else if !want && shown.is_active() {
                    shown.popdown();
                }
            });
        });
    }
    {
        let owner = Rc::clone(&owner);
        let popover = popover.clone();
        on_cleanup(move || {
            popover.popdown();
            let scope = owner.borrow_mut().take();
            if let Some(scope) = scope {
                scope.dispose();
            }
        });
    }
    place(p, &button);
    let opened = button.clone();
    controls::register(
        ctx,
        props.id,
        "popover_button",
        label,
        None,
        Actions {
            activate: Some(Rc::new(move || opened.popup())),
            value: Some(Rc::new(move || {
                Some(if open.peek() { "open" } else { "closed" }.to_string())
            })),
            ..Actions::default()
        },
    );
}

// MARK: - Notices

pub struct ToastProps {
    pub content: Str,
    pub icon: Option<&'static str>,
    pub key: Get,
    pub duration: Option<f64>,
}

/// A short notice, shown when it appears or its key changes and hidden
/// after `duration` seconds: libadwaita's toast at the bottom of the
/// window, read out by screen readers.
#[allow(non_snake_case)]
pub fn Toast(_p: &Parent, ctx: &Ctx, props: ToastProps) {
    let win = Rc::clone(ctx.win());
    let shown: Rc<RefCell<Option<adw::Toast>>> = Rc::new(RefCell::new(None));
    let at = ctx.clone();
    let current = Rc::clone(&shown);
    let seconds = props.duration.unwrap_or(3.0).round().max(1.0) as u32;
    let icon = props.icon;
    effect(move || {
        (props.key)(&at);
        let text = (props.content)(&at);
        untrack(|| {
            if let Some(previous) = current.borrow_mut().take() {
                previous.dismiss();
            }
            let toast = adw::Toast::new(&glib::markup_escape_text(&text));
            // An icon goes before the text in a title of its own.
            if let Some(icon) = icon {
                let title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                title.append(&icons::image(icon));
                let label = gtk::Label::new(Some(&text));
                label.add_css_class("heading");
                title.append(&label);
                toast.set_custom_title(Some(&title));
            }
            toast.set_timeout(seconds);
            win.show_toast(&toast);
            *current.borrow_mut() = Some(toast);
        });
    });
    on_cleanup(move || {
        if let Some(toast) = shown.borrow_mut().take() {
            toast.dismiss();
        }
    });
}

// MARK: - Inspector

pub struct InspectorProps {
    pub presented: Field,
    pub width: Option<f64>,
}

/// The rest of a container, with a side panel at its trailing edge that
/// pushes the content aside while `presented`; on narrow windows the panel
/// lies over the content. libadwaita's overlay split view slides it.
#[allow(non_snake_case)]
pub fn Inspector(p: &Parent, ctx: &Ctx, props: InspectorProps, content: Build, panel: Build) {
    let split = adw::OverlaySplitView::new();
    split.set_sidebar_position(gtk::PackType::End);
    split.set_pin_sidebar(true);
    split.set_vexpand(true);
    split.set_hexpand(true);
    let width = props.width.unwrap_or(280.0);
    split.set_min_sidebar_width(width);
    split.set_max_sidebar_width(width);
    let main = gtk::Box::new(gtk::Orientation::Vertical, 0);
    main.set_vexpand(true);
    content(&Parent::of_box(&main), ctx);
    split.set_content(Some(&main));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    panel(&Parent::of_box(&column), ctx);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&column));
    scrolled.add_css_class("bern-inspector");
    split.set_sidebar(Some(&scrolled));
    let bound = Bound::new(ctx, props.presented);
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = split.clone();
    let read = bound.clone();
    effect(move || {
        let open = read.get() == Value::Bool(true);
        if shown.shows_sidebar() != open {
            guard.set(true);
            shown.set_show_sidebar(open);
            guard.set(false);
        }
    });
    split.connect_show_sidebar_notify(move |split| {
        if !updating.get() {
            bound.set(Value::Bool(split.shows_sidebar()));
        }
    });
    // Narrow windows lay the panel over the content.
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        640.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    ctx.win().window.add_breakpoint(breakpoint);
    p.add(&split);
}

// MARK: - Drop areas

pub struct DropProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Field,
    pub kinds: &'static str,
    pub over: Option<Field>,
    pub opens: bool,
}

/// Its content accepts a file dragged onto it: while an acceptable file is
/// over it `over` is true; dropping stores the file and runs the action.
/// Other files are refused.
#[allow(non_snake_case)]
pub fn Drop(p: &Parent, ctx: &Ctx, props: DropProps, action: Option<Act>, content: Build) {
    let area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content(&Parent::of_box(&area), ctx);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if let Some(label) = label.clone() {
        let named = area.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
    let action = bound_action(ctx, &action);
    let give: Rc<dyn Fn(String)> = {
        let at = ctx.clone();
        let field = props.field;
        Rc::new(move |url| {
            at.write(field, media_source(url));
            if let Some(action) = &action {
                action();
            }
        })
    };
    let over: Rc<dyn Fn(bool)> = {
        let at = ctx.clone();
        let field = props.over;
        Rc::new(move |on| {
            if let Some(field) = field
                && at.peek(field) != Value::Bool(on)
            {
                at.write(field, Value::Bool(on));
            }
        })
    };
    let kinds = props.kinds;
    let target = gtk::DropTarget::new(
        gtk::gdk::FileList::static_type(),
        gtk::gdk::DragAction::COPY,
    );
    target.set_preload(true);
    let acceptable = move |value: &glib::Value| -> Option<String> {
        let files = value.get::<gtk::gdk::FileList>().ok()?;
        files
            .files()
            .into_iter()
            .map(|file| file.uri().to_string())
            .find(|url| accepts(url, kinds))
    };
    {
        let over = Rc::clone(&over);
        target.connect_value_notify(move |target| match target.value() {
            Some(value) if acceptable(&value).is_some() => over(true),
            Some(_) => {
                over(false);
                target.reject();
            }
            None => {}
        });
    }
    {
        let over = Rc::clone(&over);
        target.connect_leave(move |_| over(false));
    }
    {
        let over = Rc::clone(&over);
        let give = Rc::clone(&give);
        target.connect_drop(move |_, value, _, _| {
            over(false);
            match acceptable(value) {
                Some(url) => {
                    give(url);
                    true
                }
                None => false,
            }
        });
    }
    area.add_controller(target);
    if props.opens {
        crate::host::add_opener(kinds, Rc::clone(&give));
    }
    let value = Bound::new(ctx, props.field);
    controls::register(
        ctx,
        props.id,
        "drop",
        label,
        None,
        Actions {
            set_file: Some(give),
            file_kinds: Some(kinds),
            value: Some(Rc::new(move || crate::media::url_of(&value.peek()))),
            ..Actions::default()
        },
    );
    place(p, &area);
}

// MARK: - Gestures bound to values

pub struct GestureProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Field,
    pub min: f64,
    pub max: f64,
    pub vertical: bool,
    pub distance: Option<f64>,
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

fn register_gesture(
    ctx: &Ctx,
    props: &GestureProps,
    role: &'static str,
    commit: Option<Rc<dyn Fn()>>,
) {
    let bound = Bound::new(ctx, props.field);
    let set = bound.clone();
    let (min, max) = (props.min, props.max);
    controls::register(
        ctx,
        props.id,
        role,
        props.label.as_ref().map(|label| bound_text(ctx, label)),
        None,
        Actions {
            set_number: Some(Rc::new(move |value| {
                set.set(Value::Float(clamp(value, min, max)));
                if let Some(commit) = &commit {
                    commit();
                }
            })),
            value: Some(Rc::new(move || {
                Some(crate::value::number(crate::value::as_f64(&bound.peek())))
            })),
            ..Actions::default()
        },
    );
}

/// Pinching the content — two fingers on a touch screen or touchpad, or
/// the wheel with Control — scales the value within its range; the action
/// runs when the gesture ends.
#[allow(non_snake_case)]
pub fn Pinch(p: &Parent, ctx: &Ctx, props: GestureProps, commit: Option<Act>, content: Build) {
    let area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content(&Parent::of_box(&area), ctx);
    let bound = Bound::new(ctx, props.field);
    let commit = bound_action(ctx, &commit);
    let (min, max) = (props.min, props.max);
    let zoom = gtk::GestureZoom::new();
    let start = Rc::new(Cell::new(0.0));
    {
        let start = Rc::clone(&start);
        let bound = bound.clone();
        zoom.connect_begin(move |_, _| start.set(crate::value::as_f64(&bound.peek())));
    }
    {
        let start = Rc::clone(&start);
        let bound = bound.clone();
        zoom.connect_scale_changed(move |_, scale| {
            bound.set(Value::Float(clamp(start.get() * scale, min, max)));
        });
    }
    {
        let commit = commit.clone();
        zoom.connect_end(move |_, _| {
            if let Some(commit) = &commit {
                commit();
            }
        });
    }
    area.add_controller(zoom);
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let settle: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let bound = bound.clone();
        let commit = commit.clone();
        wheel.connect_scroll(move |controller, _, dy| {
            if !controller
                .current_event_state()
                .contains(gtk::gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let value = crate::value::as_f64(&bound.peek());
            bound.set(Value::Float(clamp(value * (-dy * 0.1).exp(), min, max)));
            if let Some(source) = settle.borrow_mut().take() {
                source.remove();
            }
            let commit = commit.clone();
            let done = Rc::clone(&settle);
            *settle.borrow_mut() = Some(glib::timeout_add_local_once(
                Duration::from_millis(200),
                move || {
                    done.borrow_mut().take();
                    if let Some(commit) = &commit {
                        commit();
                    }
                },
            ));
            glib::Propagation::Stop
        });
    }
    area.add_controller(wheel);
    register_gesture(ctx, &props, "pinch", commit);
    place(p, &area);
}

/// Dragging over the content along its axis moves the value through its
/// range over `distance` points (up and right increase it); the action
/// runs when the pointer lifts.
#[allow(non_snake_case)]
pub fn Swipe(p: &Parent, ctx: &Ctx, props: GestureProps, commit: Option<Act>, content: Build) {
    let area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content(&Parent::of_box(&area), ctx);
    let bound = Bound::new(ctx, props.field);
    let commit = bound_action(ctx, &commit);
    let (min, max) = (props.min, props.max);
    let span = max - min;
    let reach = props.distance.unwrap_or(300.0).max(1.0);
    let vertical = props.vertical;
    let drag = gtk::GestureDrag::new();
    let from = Rc::new(Cell::new(0.0));
    let moving = Rc::new(Cell::new(false));
    {
        let from = Rc::clone(&from);
        let moving = Rc::clone(&moving);
        let bound = bound.clone();
        drag.connect_drag_begin(move |_, _, _| {
            from.set(crate::value::as_f64(&bound.peek()));
            moving.set(false);
        });
    }
    {
        let from = Rc::clone(&from);
        let moving = Rc::clone(&moving);
        let bound = bound.clone();
        drag.connect_drag_update(move |gesture, x, y| {
            let moved = if vertical { -y } else { x };
            if !moving.get() {
                if moved.abs() < 10.0 {
                    return;
                }
                moving.set(true);
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
            bound.set(Value::Float(clamp(
                from.get() + moved / reach * span,
                min,
                max,
            )));
        });
    }
    {
        let commit = commit.clone();
        drag.connect_drag_end(move |_, _, _| {
            if moving.replace(false)
                && let Some(commit) = &commit
            {
                commit();
            }
        });
    }
    area.add_controller(drag);
    register_gesture(ctx, &props, "swipe", commit);
    place(p, &area);
}

// MARK: - Rails

pub struct RailProps {
    pub vertical: bool,
    /// `start`, `center` or `end`.
    pub anchor: Option<&'static str>,
}

/// A strip that places each child at a fraction of its length or between
/// two.
#[allow(non_snake_case)]
pub fn Rail(p: &Parent, ctx: &Ctx, props: RailProps, children: Build) {
    let anchor = match props.anchor {
        Some("start") => 0.0,
        Some("end") => 1.0,
        _ => 0.5,
    };
    let rail = RailBox::new(props.vertical, anchor);
    rail.set_can_target(true);
    children(&Parent::of_rail(&rail), ctx);
    place(p, &rail);
}

pub struct PlaceProps {
    pub at: Option<Num>,
    pub from: Option<Num>,
    pub to: Option<Num>,
    pub motion: Option<Curve>,
}

/// Places the element `build` makes on the rail it stands in; with
/// `motion` it springs to a new place.
#[allow(non_snake_case)]
pub fn Place(p: &Parent, ctx: &Ctx, props: PlaceProps, build: Build) {
    let (capture, made) = Parent::capture(p);
    build(&capture, ctx);
    let made: Vec<gtk::Widget> = made.borrow().clone();
    let Some(rail) = p.rail().cloned() else {
        for widget in made {
            p.add(&widget);
        }
        return;
    };
    for widget in made {
        let placement = Rc::new(Placement::default());
        let relayout = {
            let rail = rail.downgrade();
            move || {
                if let Some(rail) = rail.upgrade() {
                    rail.queue_allocate();
                }
            }
        };
        match &props.at {
            Some(at) => {
                let placement = Rc::clone(&placement);
                let relayout = relayout.clone();
                bind_number(ctx, at, props.motion, &widget, move |value| {
                    placement.at.set(Some(value));
                    relayout();
                });
            }
            None => {
                if let Some(from) = &props.from {
                    let placement = Rc::clone(&placement);
                    let relayout = relayout.clone();
                    bind_number(ctx, from, props.motion, &widget, move |value| {
                        placement.from.set(value);
                        relayout();
                    });
                }
                if let Some(to) = &props.to {
                    let placement = Rc::clone(&placement);
                    bind_number(ctx, to, props.motion, &widget, move |value| {
                        placement.to.set(value);
                        relayout();
                    });
                }
            }
        }
        rail.place(&widget, placement);
        p.add(&widget);
    }
}

// MARK: - Scroll views

/// A scroll view and the rows of the lists inside it, by key.
pub struct ScrollScope {
    pub(crate) view: gtk::ScrolledWindow,
    items: RefCell<Vec<(Value, glib::WeakRef<gtk::Widget>)>>,
    changed: crate::reactive::Version,
}

impl ScrollScope {
    /// A row of a list inside the view, until the current scope ends.
    pub(crate) fn add_item(self: &Rc<Self>, key: Value, widget: &gtk::Widget) {
        self.items.borrow_mut().push((key, widget.downgrade()));
        self.changed.bump();
        let scope = Rc::downgrade(self);
        let gone = widget.downgrade();
        on_cleanup(move || {
            if let Some(scope) = scope.upgrade() {
                scope.items.borrow_mut().retain(|(_, widget)| {
                    widget
                        .upgrade()
                        .is_some_and(|kept| Some(kept) != gone.upgrade())
                });
                scope.changed.bump();
            }
        });
    }

    fn items(&self) -> Vec<(Value, gtk::Widget)> {
        self.items
            .borrow()
            .iter()
            .filter_map(|(key, widget)| Some((key.clone(), widget.upgrade()?)))
            .collect()
    }
}

pub struct ScrollProps {
    pub axis: Option<&'static str>,
    /// `start` or `center`.
    pub snap: Option<&'static str>,
    pub position: Option<Field>,
    pub arrival_nudge: bool,
    /// Scroll without showing the scroll bar.
    pub hide_indicators: bool,
    pub nudge_motion: Curve,
    pub smooth: Curve,
}

#[allow(non_snake_case)]
pub fn ScrollView(p: &Parent, ctx: &Ctx, props: ScrollProps, children: Build) {
    let horizontal = props.axis == Some("horizontal");
    let view = gtk::ScrolledWindow::new();
    // `External` still scrolls, without a scroll bar.
    let scrolling = if props.hide_indicators {
        gtk::PolicyType::External
    } else {
        gtk::PolicyType::Automatic
    };
    view.set_policy(
        if horizontal {
            scrolling
        } else {
            gtk::PolicyType::Never
        },
        if horizontal {
            gtk::PolicyType::Never
        } else {
            scrolling
        },
    );
    view.set_vexpand(!horizontal);
    view.set_hexpand(true);
    let content = gtk::Box::new(
        if horizontal {
            gtk::Orientation::Horizontal
        } else {
            gtk::Orientation::Vertical
        },
        8,
    );
    view.set_child(Some(&content));
    let scope = Rc::new(ScrollScope {
        view: view.clone(),
        items: RefCell::new(Vec::new()),
        changed: crate::reactive::Version::new(),
    });
    children(
        &Parent::of_box(&content),
        &ctx.with_scroll(Rc::clone(&scope)),
    );
    place(p, &view);
    if props.snap.is_some() || props.position.is_some() {
        snapping(ctx, &scope, &content, horizontal, &props);
    }
}

/// Scrolling comes to rest with an item at the anchor; `position` follows
/// the item there and scrolls to the item it is set to.
fn snapping(
    ctx: &Ctx,
    scope: &Rc<ScrollScope>,
    content: &gtk::Box,
    horizontal: bool,
    props: &ScrollProps,
) {
    let view = scope.view.clone();
    let center = props.snap == Some("center");
    let snap = props.snap.is_some();
    let adjustment = if horizontal {
        view.hadjustment()
    } else {
        view.vadjustment()
    };
    // The scroll offset that puts `item` at the anchor.
    // Measured in the content, which the scroll offset does not move, and
    // past the content's margins.
    let offset_of = {
        let content = content.clone();
        let adjustment = adjustment.clone();
        move |item: &gtk::Widget| -> Option<f64> {
            let rect = item.compute_bounds(&content)?;
            let (start, size) = if horizontal {
                (
                    (rect.x() + content.margin_start() as f32) as f64,
                    rect.width() as f64,
                )
            } else {
                (
                    (rect.y() + content.margin_top() as f32) as f64,
                    rect.height() as f64,
                )
            };
            Some(if center {
                start + size / 2.0 - adjustment.page_size() / 2.0
            } else {
                start
            })
        }
    };
    let offset_of = Rc::new(offset_of);
    // Room before the first and after the last item, so they can come to
    // the center too.
    let margins: Option<Rc<dyn Fn() -> bool>> = center.then(|| {
        let content = content.clone();
        let held = Rc::clone(scope);
        Rc::new(move || {
            let page = if horizontal {
                held.view.width()
            } else {
                held.view.height()
            };
            if page <= 0 {
                return false;
            }
            let largest = held
                .items()
                .iter()
                .map(|(_, item)| {
                    if horizontal {
                        item.width()
                    } else {
                        item.height()
                    }
                })
                .max()
                .unwrap_or(0);
            let room = ((page - largest) / 2).max(0);
            let current = if horizontal {
                content.margin_start()
            } else {
                content.margin_top()
            };
            if current == room {
                return false;
            }
            if horizontal {
                content.set_margin_start(room);
                content.set_margin_end(room);
            } else {
                content.set_margin_top(room);
                content.set_margin_bottom(room);
            }
            true
        }) as Rc<dyn Fn() -> bool>
    });
    let animation: Rc<RefCell<Option<Spring>>> = Rc::new(RefCell::new(None));
    let moving_by_us = Rc::new(Cell::new(false));
    let scroll_to = {
        let adjustment = adjustment.clone();
        let animation = Rc::clone(&animation);
        let moving_by_us = Rc::clone(&moving_by_us);
        let view = view.clone();
        let smooth = props.smooth;
        Rc::new(
            move |offset: f64, animated: bool, done: Option<Box<dyn FnOnce()>>| {
                let upper = (adjustment.upper() - adjustment.page_size()).max(0.0);
                let target = offset.clamp(adjustment.lower(), upper);
                let spring = animation
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        let adjustment = adjustment.clone();
                        let moving = Rc::clone(&moving_by_us);
                        Spring::new(adjustment.value(), &view, move |value| {
                            moving.set(true);
                            adjustment.set_value(value);
                            moving.set(false);
                        })
                    })
                    .clone();
                if animated && !motion::reduced() {
                    spring.set(adjustment.value());
                    spring.to(target, Some(smooth), done);
                } else {
                    spring.set(target);
                    if let Some(done) = done {
                        done();
                    }
                }
            },
        )
    };
    let position = props.position;
    let at = ctx.clone();
    let rest_offset = Rc::new(Cell::new(adjustment.value()));
    let placed = Rc::new(Cell::new(false));
    let nearest = {
        let scope = Rc::clone(scope);
        let offset_of = Rc::clone(&offset_of);
        let adjustment = adjustment.clone();
        let rest_offset = Rc::clone(&rest_offset);
        Rc::new(
            move |follow_motion: bool| -> Option<(Value, gtk::Widget, f64)> {
                let current = adjustment.value();
                let mut items: Vec<(Value, gtk::Widget, f64)> = scope
                    .items()
                    .into_iter()
                    .filter_map(|(key, item)| Some((key, item.clone(), offset_of(&item)?)))
                    .collect();
                items.sort_by(|a, b| a.2.total_cmp(&b.2));
                let best = items
                    .iter()
                    .min_by(|a, b| (a.2 - current).abs().total_cmp(&(b.2 - current).abs()))?
                    .clone();
                // A short scroll still moves on: past a few points from where it
                // rested, the next item that way wins over the one behind.
                let moved = current - rest_offset.get();
                if follow_motion && moved.abs() > 24.0 && (best.2 - current) * moved < 0.0 {
                    let ahead = if moved > 0.0 {
                        items.iter().find(|item| item.2 > current)
                    } else {
                        items.iter().rev().find(|item| item.2 < current)
                    };
                    if let Some(ahead) = ahead {
                        return Some(ahead.clone());
                    }
                }
                Some(best)
            },
        )
    };
    let resting: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let nearest = Rc::clone(&nearest);
        let scroll_to = Rc::clone(&scroll_to);
        let rest_offset = Rc::clone(&rest_offset);
        let placed = Rc::clone(&placed);
        let moving_by_us = Rc::clone(&moving_by_us);
        let at = at.clone();
        let resting = Rc::clone(&resting);
        adjustment.connect_value_changed(move |adjustment| {
            if moving_by_us.get() || !placed.get() {
                return;
            }
            if let Some(source) = resting.borrow_mut().take() {
                source.remove();
            }
            let nearest = Rc::clone(&nearest);
            let scroll_to = Rc::clone(&scroll_to);
            let rest_offset = Rc::clone(&rest_offset);
            let at = at.clone();
            let adjustment = adjustment.clone();
            let done = Rc::clone(&resting);
            *resting.borrow_mut() = Some(glib::timeout_add_local_once(
                Duration::from_millis(160),
                move || {
                    done.borrow_mut().take();
                    let Some((key, _, offset)) = nearest(true) else {
                        return;
                    };
                    let settle = {
                        let rest_offset = Rc::clone(&rest_offset);
                        let adjustment = adjustment.clone();
                        Box::new(move || rest_offset.set(adjustment.value())) as Box<dyn FnOnce()>
                    };
                    if snap && (offset - adjustment.value()).abs() > 0.5 {
                        scroll_to(offset, true, Some(settle));
                    } else {
                        settle();
                    }
                    if let Some(field) = position
                        && at.peek(field) != key
                    {
                        at.write(field, key);
                    }
                },
            ));
        });
    }
    // Setting the field scrolls to its item, and nudges it on arrival.
    if let Some(field) = position {
        let scope = Rc::clone(scope);
        let offset_of = Rc::clone(&offset_of);
        let scroll_to = Rc::clone(&scroll_to);
        let placed = Rc::clone(&placed);
        let adjustment = adjustment.clone();
        let rest_offset = Rc::clone(&rest_offset);
        let arrival_nudge = props.arrival_nudge;
        let nudge_motion = props.nudge_motion;
        effect(move || {
            let key = at.read(field);
            scope.changed.track();
            untrack(|| {
                if !placed.get() {
                    return;
                }
                let Some((_, item)) = scope
                    .items()
                    .into_iter()
                    .find(|(candidate, _)| *candidate == key)
                else {
                    return;
                };
                let Some(offset) = offset_of(&item) else {
                    return;
                };
                if (offset - adjustment.value()).abs() < 1.0 {
                    return;
                }
                let arrived = item.clone();
                let rest_offset = Rc::clone(&rest_offset);
                let settled = adjustment.clone();
                scroll_to(
                    offset,
                    true,
                    Some(Box::new(move || {
                        rest_offset.set(settled.value());
                        if arrival_nudge {
                            motion::nudge_widget(&arrived, nudge_motion);
                        }
                    })),
                );
            });
        });
    }
    // The first display starts at the field's item, without a nudge, and
    // the item stays at the center when the margins change. Both wait for
    // the layout: each time the scroll range changes, the margins are
    // checked first, and the item is placed once they hold.
    let at_item: Rc<dyn Fn()> = {
        let scope = Rc::clone(scope);
        let offset_of = Rc::clone(&offset_of);
        let scroll_to = Rc::clone(&scroll_to);
        let first = ctx.clone();
        let rest_offset = Rc::clone(&rest_offset);
        let adjustment = adjustment.clone();
        Rc::new(move || {
            let Some(field) = position else { return };
            let key = first.peek(field);
            if let Some(offset) = scope
                .items()
                .into_iter()
                .find(|(candidate, _)| *candidate == key)
                .and_then(|(_, item)| offset_of(&item))
            {
                scroll_to(offset, false, None);
                rest_offset.set(adjustment.value());
            }
        })
    };
    let moved_margins = Rc::new(Cell::new(false));
    let settle: Rc<dyn Fn()> = Rc::new(move || {
        if adjustment.page_size() <= 0.0 {
            return;
        }
        if let Some(margins) = &margins
            && margins()
        {
            moved_margins.set(true);
            return;
        }
        if !placed.get() {
            at_item();
            placed.set(true);
        } else if moved_margins.replace(false) {
            at_item();
        }
    });
    let changed = Rc::clone(&settle);
    view_adjustment(&view, horizontal).connect_changed(move |_| {
        let changed = Rc::clone(&changed);
        glib::idle_add_local_once(move || changed());
    });
    let items = Rc::clone(&settle);
    let watched = Rc::clone(scope);
    effect(move || {
        watched.changed.track();
        let items = Rc::clone(&items);
        glib::idle_add_local_once(move || items());
    });
    let mapped = Rc::clone(&settle);
    view.connect_map(move |_| {
        let mapped = Rc::clone(&mapped);
        glib::idle_add_local_once(move || mapped());
    });
}

fn view_adjustment(view: &gtk::ScrolledWindow, horizontal: bool) -> gtk::Adjustment {
    if horizontal {
        view.hadjustment()
    } else {
        view.vadjustment()
    }
}

// MARK: - Pictures from state

/// A texture read from a `file://` or http(s) URL.
fn load_texture(url: &str, done: impl FnOnce(Option<gtk::gdk::Texture>) + 'static) {
    let file = if url.contains("://") {
        gio::File::for_uri(url)
    } else {
        gio::File::for_path(url)
    };
    file.load_bytes_async(None::<&gio::Cancellable>, move |result| {
        let texture = result.ok().and_then(|(bytes, _)| {
            let stream = gio::MemoryInputStream::from_bytes(&bytes);
            let pixbuf =
                gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, None::<&gio::Cancellable>).ok()?;
            let upright = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
            Some(gtk::gdk::Texture::for_pixbuf(&upright))
        });
        done(texture);
    });
}

pub struct RuntimeImageProps {
    pub url: Get,
    pub label: Option<Str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub fit: Option<&'static str>,
    pub animate: Option<Curve>,
}

/// A picture from a `MediaSource` in state: a file the user chose or an
/// http(s) URL, read in the background; nothing while the state has none.
/// With one side given, the other follows the picture.
#[allow(non_snake_case)]
pub fn RuntimeImage(p: &Parent, ctx: &Ctx, props: RuntimeImageProps) {
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(match props.fit {
        Some("cover") => gtk::ContentFit::Cover,
        Some("fill") => gtk::ContentFit::Fill,
        _ => gtk::ContentFit::Contain,
    });
    let frame = crate::picture::Frame::new(&picture, 0, 0);
    let ratio = Rc::new(Cell::new(1.0));
    let sides: Rc<RefCell<(Option<f64>, Option<f64>)>> = Rc::new(RefCell::new((None, None)));
    let resize = {
        let frame = frame.clone();
        let ratio = Rc::clone(&ratio);
        let sides = Rc::clone(&sides);
        Rc::new(move || {
            let (width, height) = *sides.borrow();
            let ratio = ratio.get();
            let w = width.or(height.map(|height| height * ratio)).unwrap_or(0.0);
            let h = height.or(width.map(|width| width / ratio)).unwrap_or(0.0);
            frame.set_size(w.round() as i32, h.round() as i32);
        })
    };
    if let Some(width) = &props.width {
        let sides = Rc::clone(&sides);
        let resize = Rc::clone(&resize);
        bind_number(ctx, width, props.animate, &frame, move |value| {
            sides.borrow_mut().0 = Some(value);
            resize();
        });
    }
    if let Some(height) = &props.height {
        let sides = Rc::clone(&sides);
        let resize = Rc::clone(&resize);
        bind_number(ctx, height, props.animate, &frame, move |value| {
            sides.borrow_mut().1 = Some(value);
            resize();
        });
    }
    match &props.label {
        Some(label) => {
            let text = bound_text(ctx, label);
            let described = picture.clone();
            effect(move || described.set_alternative_text(Some(&text())));
        }
        None => picture.update_state(&[gtk::accessible::State::Hidden(true)]),
    }
    let at = ctx.clone();
    let shown = picture.clone();
    let hidden = frame.clone();
    let current: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    effect(move || {
        let url = crate::media::url_of(&(props.url)(&at));
        untrack(|| {
            if *current.borrow() == url {
                return;
            }
            *current.borrow_mut() = url.clone();
            hidden.set_visible(url.is_some());
            let Some(url) = url else {
                shown.set_paintable(None::<&gtk::gdk::Paintable>);
                return;
            };
            let shown = shown.clone();
            let ratio = Rc::clone(&ratio);
            let resize = Rc::clone(&resize);
            let wanted = Rc::clone(&current);
            let requested = url.clone();
            load_texture(&url, move |texture| {
                // A later URL replaced this one while it was read.
                if wanted.borrow().as_deref() != Some(requested.as_str()) {
                    return;
                }
                if let Some(texture) = &texture
                    && texture.width() > 0
                    && texture.height() > 0
                {
                    ratio.set(f64::from(texture.width()) / f64::from(texture.height()));
                }
                shown.set_paintable(texture.as_ref());
                resize();
            });
        });
    });
    place(p, &frame);
}

// MARK: - Rounded corners

/// Round the corners of `widget` with a bound radius, springing with
/// `animate`; returns the clip to place.
pub(crate) fn rounded(
    ctx: &Ctx,
    widget: &impl IsA<gtk::Widget>,
    corner: &Num,
    animate: Option<Curve>,
) -> Clip {
    let clip = Clip::new(widget);
    let shown = clip.clone();
    bind_number(ctx, corner, animate, &clip, move |radius| {
        shown.set_radius(radius)
    });
    clip
}
