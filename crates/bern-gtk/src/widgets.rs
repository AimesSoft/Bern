//! Standard controls and containers as GTK 4 and libadwaita widgets.
//!
//! Inside a form section, controls are libadwaita rows (`AdwSwitchRow`,
//! `AdwEntryRow`, `AdwComboRow` and action rows) in a boxed list; elsewhere
//! they are the plain GTK controls. Colors and type come from the system
//! style: widgets only use libadwaita's style classes.

use crate::controls::{self, Actions};
use crate::ctx::Field;
use crate::motion::{Curve, Num, bind_number};
use crate::protocol::Value;
use crate::reactive::{effect, untrack};
use crate::text::{self, tr};
use crate::value::{as_f64, as_text, number};
use crate::{Act, Build, Ctx, Get, Parent, Str, icons};
use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// A text getter bound to a context.
pub(crate) fn bound_text(ctx: &Ctx, text: &Str) -> Rc<dyn Fn() -> String> {
    let ctx = ctx.clone();
    let text = Rc::clone(text);
    Rc::new(move || text(&ctx))
}

pub(crate) fn bound_enabled(ctx: &Ctx, enabled: &Option<Get>) -> Option<Rc<dyn Fn() -> bool>> {
    let enabled = enabled.clone()?;
    let ctx = ctx.clone();
    Some(Rc::new(move || crate::value::truthy(&enabled(&ctx))))
}

pub(crate) fn bound_action(ctx: &Ctx, action: &Option<Act>) -> Option<Rc<dyn Fn()>> {
    let action = action.clone()?;
    let ctx = ctx.clone();
    Some(Rc::new(move || action(&ctx)))
}

/// Keep a label's text in step with a getter.
pub(crate) fn bind_label(label: &gtk::Label, text: Rc<dyn Fn() -> String>) {
    let label = label.clone();
    effect(move || label.set_label(&text()));
}

thread_local! {
    static ACTIVATIONS: std::cell::RefCell<Vec<(glib::WeakRef<gtk::ListBoxRow>, crate::Thunk)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `activate` when the row is activated by a click or the keyboard,
/// until the current scope ends.
pub(crate) fn on_activate(row: &gtk::ListBoxRow, activate: Rc<dyn Fn()>) {
    let weak = row.downgrade();
    ACTIVATIONS.with(|rows| {
        rows.borrow_mut().retain(|(row, _)| row.upgrade().is_some());
        rows.borrow_mut().push((weak, activate));
    });
    let ended = row.downgrade();
    crate::reactive::on_cleanup(move || {
        ACTIVATIONS.with(|rows| {
            rows.borrow_mut()
                .retain(|(row, _)| row.upgrade().is_some() && row.upgrade() != ended.upgrade())
        });
    });
}

/// A boxed list whose activatable rows run their actions.
pub(crate) fn boxed_list() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.connect_row_activated(|_, row| {
        let found = ACTIVATIONS.with(|rows| {
            rows.borrow()
                .iter()
                .find(|(candidate, _)| candidate.upgrade().as_ref() == Some(row))
                .map(|(_, activate)| Rc::clone(activate))
        });
        if let Some(activate) = found {
            activate();
        }
    });
    list
}

/// A row of a boxed list holding `child`, for content that is not a
/// libadwaita row itself.
pub(crate) fn plain_row(child: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let child = child.as_ref();
    child.set_margin_top(10);
    child.set_margin_bottom(10);
    child.set_margin_start(12);
    child.set_margin_end(12);
    row.set_child(Some(child));
    row
}

/// Add a widget as it fits the parent: plainly, or in a row of a list.
pub(crate) fn place(p: &Parent, widget: &impl IsA<gtk::Widget>) {
    if p.is_list() {
        p.add(&plain_row(widget));
    } else {
        p.add(widget);
    }
}

fn style_class(style: Option<&str>) -> &'static str {
    match style {
        Some("large_title") => "title-1",
        Some("title") => "title-2",
        Some("title2") => "title-3",
        Some("title3") => "title-4",
        Some("headline") | Some("subheadline") => "heading",
        Some("footnote") | Some("caption") | Some("caption2") => "caption",
        _ => "body",
    }
}

#[derive(Default)]
pub struct TextProps {
    pub content: Option<Str>,
    pub style: Option<&'static str>,
    pub emphasis: Option<&'static str>,
    /// A capsule of this color under each line.
    pub mark: Option<crate::theme::Color>,
}

#[allow(non_snake_case)]
pub fn Text(p: &Parent, ctx: &Ctx, props: TextProps) {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.add_css_class(style_class(props.style));
    if matches!(props.emphasis, Some("secondary") | Some("tertiary")) {
        label.add_css_class("dim-label");
    }
    if let Some(content) = &props.content {
        bind_label(&label, bound_text(ctx, content));
    }
    match props.mark {
        Some(mark) => place(p, &crate::canvas::Marked::new(&label, mark)),
        None => place(p, &label),
    }
}

#[derive(Default)]
pub struct LabelProps {
    pub title: Option<Str>,
    pub icon: Option<&'static str>,
    pub style: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn Label(p: &Parent, ctx: &Ctx, props: LabelProps) {
    let title = props.title.as_ref().map(|title| bound_text(ctx, title));
    if p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(icon) = props.icon {
            row.add_prefix(&icons::image(icon));
        }
        if let Some(title) = title {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&title())));
        }
        p.add(&row);
        return;
    }
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if let Some(icon) = props.icon {
        container.append(&icons::image(icon));
    }
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.add_css_class(style_class(props.style));
    if let Some(title) = title {
        bind_label(&label, title);
    }
    container.append(&label);
    p.add(&container);
}

#[derive(Default)]
pub struct ButtonProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub role: Option<&'static str>,
    pub prominence: Option<&'static str>,
    pub enabled: Option<Get>,
    pub style: Option<&'static crate::style::StyleDef>,
    /// `pick:`: the open panel asks for a file before the action runs.
    pub pick: Option<crate::features::Pick>,
}

fn register_button(
    ctx: &Ctx,
    id: &str,
    label: Option<Rc<dyn Fn() -> String>>,
    enabled: Option<Rc<dyn Fn() -> bool>>,
    action: Option<Rc<dyn Fn()>>,
    set_file: Option<(crate::features::Give, &'static str)>,
) {
    controls::register(
        ctx,
        id,
        "button",
        label,
        enabled,
        Actions {
            activate: action,
            file_kinds: set_file.as_ref().map(|(_, kinds)| *kinds),
            set_file: set_file.map(|(give, _)| give),
            ..Actions::default()
        },
    );
}

/// The button as it goes into its container: in a motion box when a style
/// moves it.
fn styled(button: &gtk::Button, style: Option<&'static crate::style::StyleDef>) -> gtk::Widget {
    match style {
        Some(style) => {
            let motion = crate::motion::MotionBox::wrap(button);
            crate::style::apply(style, button.upcast_ref(), button.upcast_ref(), &motion);
            motion.upcast()
        }
        None => button.clone().upcast(),
    }
}

#[allow(non_snake_case)]
pub fn Button(p: &Parent, ctx: &Ctx, props: ButtonProps, action: Option<Act>) {
    let button = gtk::Button::new();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    match (props.icon, &label) {
        (Some(icon), Some(label)) => {
            let content = adw::ButtonContent::new();
            content.set_icon_name(icons::name(icon));
            let shown = content.clone();
            let label = Rc::clone(label);
            effect(move || shown.set_label(&label()));
            button.set_child(Some(&content));
        }
        (Some(icon), None) => button.set_icon_name(icons::name(icon)),
        (None, Some(label)) => {
            let shown = button.clone();
            let label = Rc::clone(label);
            effect(move || shown.set_label(&label()));
        }
        (None, None) => {}
    }
    match props.prominence {
        Some("prominent") => button.add_css_class("suggested-action"),
        Some("plain") => button.add_css_class("flat"),
        _ => {}
    }
    if props.role == Some("destructive") {
        button.add_css_class("destructive-action");
    }
    let enabled = bound_enabled(ctx, &props.enabled);
    if let Some(enabled) = enabled.clone() {
        let shown = button.clone();
        effect(move || shown.set_sensitive(enabled()));
    }
    let (run, set_file) = crate::features::picking(ctx, props.pick, bound_action(ctx, &action));
    if let Some(run) = run.clone() {
        button.connect_clicked(move |_| run());
    }
    register_button(
        ctx,
        props.id,
        label,
        enabled,
        run,
        set_file.zip(props.pick.map(|pick| pick.kinds)),
    );
    if p.is_list() && !ctx.in_toolbar() {
        button.set_halign(gtk::Align::Start);
        let row = plain_row(&styled(&button, props.style));
        row.child().inspect(|child| {
            child.set_margin_top(8);
            child.set_margin_bottom(8);
        });
        p.add(&row);
    } else {
        p.add(&styled(&button, props.style));
    }
}

#[derive(Default)]
pub struct IconButtonProps {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: Option<Str>,
    pub enabled: Option<Get>,
    pub style: Option<&'static crate::style::StyleDef>,
    pub pick: Option<crate::features::Pick>,
}

#[allow(non_snake_case)]
pub fn IconButton(p: &Parent, ctx: &Ctx, props: IconButtonProps, action: Option<Act>) {
    let button = gtk::Button::from_icon_name(icons::name(props.icon));
    button.add_css_class("flat");
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if let Some(label) = label.clone() {
        let shown = button.clone();
        effect(move || {
            let text = label();
            shown.set_tooltip_text(Some(&text));
            shown.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let enabled = bound_enabled(ctx, &props.enabled);
    if let Some(enabled) = enabled.clone() {
        let shown = button.clone();
        effect(move || shown.set_sensitive(enabled()));
    }
    let (run, set_file) = crate::features::picking(ctx, props.pick, bound_action(ctx, &action));
    if let Some(run) = run.clone() {
        button.connect_clicked(move |_| run());
    }
    register_button(
        ctx,
        props.id,
        label,
        enabled,
        run,
        set_file.zip(props.pick.map(|pick| pick.kinds)),
    );
    button.set_valign(gtk::Align::Center);
    place(p, &styled(&button, props.style));
}

/// A draft field bound to a control.
#[derive(Clone)]
pub(crate) struct Bound {
    ctx: Ctx,
    field: Field,
}

impl Bound {
    pub(crate) fn new(ctx: &Ctx, field: Field) -> Self {
        Self {
            ctx: ctx.clone(),
            field,
        }
    }

    pub(crate) fn get(&self) -> Value {
        self.ctx.read(self.field)
    }

    pub(crate) fn peek(&self) -> Value {
        self.ctx.peek(self.field)
    }

    pub(crate) fn set(&self, value: Value) {
        if self.peek() != value {
            self.ctx.write(self.field, value);
        }
    }
}

/// A caption above a control outside forms.
pub(crate) fn stacked(
    label: Option<Rc<dyn Fn() -> String>>,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if let Some(label) = label {
        let caption = gtk::Label::new(None);
        caption.set_xalign(0.0);
        caption.add_css_class("caption-heading");
        bind_label(&caption, label);
        container.append(&caption);
    }
    container.append(control);
    container
}

/// A label beside a control outside forms.
pub(crate) fn beside(
    label: Option<Rc<dyn Fn() -> String>>,
    control: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(label) = label {
        let caption = gtk::Label::new(None);
        caption.set_xalign(0.0);
        caption.set_hexpand(true);
        bind_label(&caption, label);
        container.append(&caption);
    }
    control.as_ref().set_valign(gtk::Align::Center);
    container.append(control);
    container
}

#[derive(Default)]
pub struct TextInputProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Option<Field>,
    pub prompt: Option<Str>,
    pub kind: Option<&'static str>,
}

/// Text editing: changes update the draft; draft changes from elsewhere
/// update the text without echoing back.
fn editable(text: Rc<dyn Fn() -> String>, set: Rc<dyn Fn(&str)>, bound: Bound) -> Rc<Cell<bool>> {
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    effect(move || {
        let value = as_text(&bound.get());
        if untrack(|| text()) != value {
            guard.set(true);
            set(&value);
            guard.set(false);
        }
    });
    updating
}

#[allow(non_snake_case)]
pub fn TextInput(p: &Parent, ctx: &Ctx, props: TextInputProps, submit: Option<Act>) {
    let Some(field) = props.field else { return };
    let bound = Bound::new(ctx, field);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let prompt = props.prompt.as_ref().map(|prompt| bound_text(ctx, prompt));
    let submit = bound_action(ctx, &submit);
    let kind = props.kind.unwrap_or("text");
    let secure = kind == "secure";

    if kind == "multiline" {
        let view = gtk::TextView::new();
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_accepts_tab(false);
        view.set_top_margin(8);
        view.set_bottom_margin(8);
        view.set_left_margin(8);
        view.set_right_margin(8);
        view.set_size_request(-1, 72);
        let buffer = view.buffer();
        let read = buffer.clone();
        let write = buffer.clone();
        let updating = editable(
            Rc::new(move || {
                read.text(&read.start_iter(), &read.end_iter(), false)
                    .to_string()
            }),
            Rc::new(move |value| write.set_text(value)),
            bound.clone(),
        );
        let changed = bound.clone();
        buffer.connect_changed(move |buffer| {
            if !updating.get() {
                changed.set(Value::Text(
                    buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), false)
                        .to_string(),
                ));
            }
        });
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&view));
        if p.is_list() {
            p.add(&plain_row(&stacked(label.clone(), &frame)));
        } else {
            p.add(&stacked(label.clone(), &frame));
        }
    } else if p.is_list() {
        let row: adw::EntryRow = if secure {
            adw::PasswordEntryRow::new().upcast()
        } else {
            adw::EntryRow::new()
        };
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&label()));
        }
        let read = row.clone();
        let write = row.clone();
        let updating = editable(
            Rc::new(move || read.text().to_string()),
            Rc::new(move |value| write.set_text(value)),
            bound.clone(),
        );
        let changed = bound.clone();
        row.connect_changed(move |row| {
            if !updating.get() {
                changed.set(Value::Text(row.text().to_string()));
            }
        });
        if let Some(submit) = submit.clone() {
            row.connect_entry_activated(move |_| submit());
        }
        p.add(&row);
    } else {
        let entry = gtk::Entry::new();
        entry.set_visibility(!secure);
        if secure {
            entry.set_input_purpose(gtk::InputPurpose::Password);
        }
        if let Some(prompt) = prompt.clone() {
            let shown = entry.clone();
            effect(move || shown.set_placeholder_text(Some(&prompt())));
        }
        let read = entry.clone();
        let write = entry.clone();
        let updating = editable(
            Rc::new(move || read.text().to_string()),
            Rc::new(move |value| write.set_text(value)),
            bound.clone(),
        );
        let changed = bound.clone();
        entry.connect_changed(move |entry| {
            if !updating.get() {
                changed.set(Value::Text(entry.text().to_string()));
            }
        });
        if let Some(submit) = submit.clone() {
            entry.connect_activate(move |_| submit());
        }
        p.add(&stacked(label.clone(), &entry));
    }
    let set = bound.clone();
    let edit: Rc<dyn Fn(String)> = Rc::new(move |value| set.set(Value::Text(value)));
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "text_input",
        label,
        None,
        Actions {
            edit: Some(edit),
            submit,
            value: (!secure).then(|| {
                Rc::new(move || Some(as_text(&value.peek()))) as Rc<dyn Fn() -> Option<String>>
            }),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct SearchProps {
    pub id: &'static str,
    pub field: Option<Field>,
    pub prompt: Option<Str>,
}

/// A search field. Its children are results shown below it while it has
/// the focus and text, in a popover that keeps the focus in the field;
/// choosing a result closes them.
#[allow(non_snake_case)]
pub fn Search(
    p: &Parent,
    ctx: &Ctx,
    props: SearchProps,
    submit: Option<Act>,
    results: Option<Build>,
) {
    let Some(field) = props.field else { return };
    let bound = Bound::new(ctx, field);
    let prompt = props.prompt.as_ref().map(|prompt| bound_text(ctx, prompt));
    let entry = gtk::SearchEntry::new();
    if ctx.in_toolbar() {
        entry.set_hexpand(false);
        entry.set_width_chars(18);
        entry.set_valign(gtk::Align::Center);
    } else {
        entry.set_hexpand(true);
    }
    if let Some(prompt) = prompt.clone() {
        let shown = entry.clone();
        effect(move || {
            let text = prompt();
            shown.set_placeholder_text(Some(&text));
            shown.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let read = entry.clone();
    let write = entry.clone();
    let updating = editable(
        Rc::new(move || read.text().to_string()),
        Rc::new(move |value| write.set_text(value)),
        bound.clone(),
    );
    let changed = bound.clone();
    entry.connect_changed(move |entry| {
        if !updating.get() {
            changed.set(Value::Text(entry.text().to_string()));
        }
    });
    let submit = bound_action(ctx, &submit);
    if let Some(submit) = submit.clone() {
        entry.connect_activate(move |_| submit());
    }
    let cleared = bound.clone();
    entry.connect_stop_search(move |_| cleared.set(Value::Text(String::new())));
    if let Some(results) = results {
        search_results(ctx, &entry, &bound, results);
    }
    place(p, &entry);
    let set = bound.clone();
    let value = bound.clone();
    let focused = entry.clone();
    controls::register(
        ctx,
        props.id,
        "search",
        prompt.or_else(|| Some(Rc::new(String::new))),
        None,
        Actions {
            edit: Some(Rc::new(move |text| set.set(Value::Text(text)))),
            submit,
            value: Some(Rc::new(move || Some(as_text(&value.peek())))),
            focus: Some(Rc::new(move || {
                focused.grab_focus();
            })),
            ..Actions::default()
        },
    );
}

fn search_results(ctx: &Ctx, entry: &gtk::SearchEntry, bound: &Bound, results: Build) {
    let popover = gtk::Popover::new();
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.set_can_focus(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    column.set_margin_top(6);
    column.set_margin_bottom(6);
    column.set_margin_start(6);
    column.set_margin_end(6);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_max_content_height(360);
    scrolled.set_min_content_width(280);
    scrolled.set_child(Some(&column));
    popover.set_child(Some(&scrolled));
    popover.set_parent(entry);
    results(&Parent::of_box(&column), &ctx.outside_toolbar());
    let focused = crate::reactive::Cell::new(false);
    let focus = gtk::EventControllerFocus::new();
    {
        let focused = focused.clone();
        focus.connect_enter(move |_| focused.set(true));
    }
    {
        let focused = focused.clone();
        let popover = popover.clone();
        focus.connect_leave(move |_| {
            let focused = focused.clone();
            let popover = popover.clone();
            // Focus moving into the results keeps them.
            glib::idle_add_local_once(move || {
                let inside = popover
                    .root()
                    .and_then(|root| gtk::prelude::RootExt::focus(&root))
                    .is_some_and(|focus| focus.is_ancestor(&popover));
                if !inside {
                    focused.set(false);
                }
            });
        });
    }
    entry.add_controller(focus);
    // Choosing a result closes them, until the field takes the focus again.
    // The raw release is seen before the result's own click, which then
    // still runs.
    let release = gtk::EventControllerLegacy::new();
    release.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let focused = focused.clone();
        release.connect_event(move |_, event| {
            if matches!(
                event.event_type(),
                gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::TouchEnd
            ) {
                let focused = focused.clone();
                glib::idle_add_local_once(move || focused.set(false));
            }
            glib::Propagation::Proceed
        });
    }
    column.add_controller(release);
    // Typing again in the field shows them again.
    {
        let focused = focused.clone();
        entry.connect_search_changed(move |entry| {
            let inside = entry
                .root()
                .and_then(|root| gtk::prelude::RootExt::focus(&root))
                .is_some_and(|focus| {
                    &focus == entry.upcast_ref::<gtk::Widget>() || focus.is_ancestor(entry)
                });
            if inside {
                focused.set(true);
            }
        });
    }
    let read = bound.clone();
    let shown = popover.clone();
    effect(move || {
        let open = focused.get() && !as_text(&read.get()).is_empty();
        if open && !shown.is_visible() {
            shown.popup();
        } else if !open && shown.is_visible() {
            shown.popdown();
        }
    });
    crate::reactive::on_cleanup(move || {
        popover.popdown();
        popover.unparent();
    });
}

#[derive(Default)]
pub struct SwitchProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Option<Field>,
    pub style: Option<&'static str>,
    pub icon: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn Switch(p: &Parent, ctx: &Ctx, props: SwitchProps) {
    let Some(field) = props.field else { return };
    if props.style == Some("button") || ctx.in_toolbar() {
        crate::features::toggle_button(
            p,
            ctx,
            crate::features::ToggleProps {
                id: props.id,
                label: props.label,
                field,
                icon: props.icon,
            },
        );
        return;
    }
    let bound = Bound::new(ctx, field);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let checkbox = props.style == Some("checkbox");
    // Every presentation exposes one boolean property, `active`.
    let control: gtk::Widget;
    let active: glib::Object;
    if p.is_list() && !checkbox {
        let row = adw::SwitchRow::new();
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&label()));
        }
        active = row.clone().upcast();
        control = row.upcast();
    } else if checkbox {
        let check = gtk::CheckButton::new();
        if p.is_list() {
            let row = adw::ActionRow::new();
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            if let Some(label) = label.clone() {
                let shown = row.clone();
                effect(move || shown.set_title(&glib::markup_escape_text(&label())));
            }
            control = row.upcast();
        } else {
            if let Some(label) = label.clone() {
                let shown = check.clone();
                effect(move || shown.set_label(Some(&label())));
            }
            control = check.clone().upcast();
        }
        active = check.upcast();
    } else {
        let switch = gtk::Switch::new();
        control = beside(label.clone(), &switch).upcast();
        active = switch.upcast();
    }
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = active.clone();
    let read = bound.clone();
    effect(move || {
        let value = read.get() == Value::Bool(true);
        if shown.property::<bool>("active") != value {
            guard.set(true);
            shown.set_property("active", value);
            guard.set(false);
        }
    });
    let changed = bound.clone();
    active.connect_notify_local(Some("active"), move |object, _| {
        if !updating.get() {
            changed.set(Value::Bool(object.property::<bool>("active")));
        }
    });
    p.add(&control);
    let set = bound.clone();
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "switch",
        label,
        None,
        Actions {
            set_bool: Some(Rc::new(move |on| set.set(Value::Bool(on)))),
            value: Some(Rc::new(move || {
                Some((value.peek() == Value::Bool(true)).to_string())
            })),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct SliderProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Option<Field>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub min_icon: Option<&'static str>,
    pub max_icon: Option<&'static str>,
}

/// A slider: dragging only updates this window's draft as a preview, and
/// releasing commits once; keyboard and scroll changes commit at once.
#[allow(non_snake_case)]
pub fn Slider(p: &Parent, ctx: &Ctx, props: SliderProps, commit: Option<Act>) {
    let Some(field) = props.field else { return };
    let bound = Bound::new(ctx, field);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let (min, max) = (props.min.unwrap_or(0.0), props.max.unwrap_or(1.0));
    let increment = props.step.unwrap_or((max - min) / 100.0).max(f64::EPSILON);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, increment);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    if let Some(step) = props.step {
        scale.set_round_digits(if step.fract() == 0.0 { 0 } else { -1 });
    }
    let commit = bound_action(ctx, &commit);
    let snap = move |value: f64| match props.step {
        Some(step) if step > 0.0 => (min + ((value - min) / step).round() * step).clamp(min, max),
        _ => value.clamp(min, max),
    };
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = scale.clone();
    let read = bound.clone();
    effect(move || {
        let value = as_f64(&read.get());
        if (shown.value() - value).abs() > f64::EPSILON {
            guard.set(true);
            shown.set_value(value);
            guard.set(false);
        }
    });
    let dragging = Rc::new(Cell::new(false));
    let pressed = Rc::clone(&dragging);
    let released_commit = commit.clone();
    let legacy = gtk::EventControllerLegacy::new();
    legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
    legacy.connect_event(move |_, event| {
        match event.event_type() {
            gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => pressed.set(true),
            gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::TouchEnd
                if pressed.replace(false) =>
            {
                if let Some(commit) = &released_commit {
                    let commit = Rc::clone(commit);
                    // After the scale handled the release.
                    glib::idle_add_local_once(move || commit());
                }
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    scale.add_controller(legacy);
    let changed = bound.clone();
    let keyed_commit = commit.clone();
    scale.connect_value_changed(move |scale| {
        if updating.get() {
            return;
        }
        let value = snap(scale.value());
        changed.set(Value::Float(value));
        if !dragging.get()
            && let Some(commit) = &keyed_commit
        {
            commit();
        }
    });
    let track = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if let Some(icon) = props.min_icon {
        let image = icons::image(icon);
        image.add_css_class("dim-label");
        track.append(&image);
    }
    track.append(&scale);
    if let Some(icon) = props.max_icon {
        let image = icons::image(icon);
        image.add_css_class("dim-label");
        track.append(&image);
    }
    if let Some(label) = label.clone() {
        let shown = scale.clone();
        effect(move || {
            shown.update_property(&[gtk::accessible::Property::Label(&label())]);
        });
    }
    let content = stacked(label.clone(), &track);
    if p.is_list() {
        p.add(&plain_row(&content));
    } else {
        p.add(&content);
    }
    let set = bound.clone();
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "slider",
        label,
        None,
        Actions {
            set_number: Some(Rc::new(move |number| {
                set.set(Value::Float(snap(number)));
                if let Some(commit) = &commit {
                    commit();
                }
            })),
            value: Some(Rc::new(move || Some(number(as_f64(&value.peek()))))),
            ..Actions::default()
        },
    );
}

/// One option of a picker: variant index, variant name and label.
pub struct PickOption {
    pub value: u32,
    pub name: &'static str,
    pub label: Str,
}

pub fn option(value: u32, name: &'static str, label: Str) -> PickOption {
    PickOption { value, name, label }
}

#[derive(Default)]
pub struct PickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub field: Option<Field>,
    pub options: Vec<PickOption>,
    pub style: Option<&'static str>,
}

/// A list of strings that follows the options' labels.
fn label_model(ctx: &Ctx, labels: Vec<Str>, apply: impl Fn(&[String]) + 'static) {
    let ctx = ctx.clone();
    effect(move || {
        let texts: Vec<String> = labels.iter().map(|label| label(&ctx)).collect();
        apply(&texts);
    });
}

#[allow(non_snake_case)]
pub fn Picker(p: &Parent, ctx: &Ctx, props: PickerProps) {
    let Some(field) = props.field else { return };
    let bound = Bound::new(ctx, field);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let values: Rc<Vec<u32>> = Rc::new(props.options.iter().map(|option| option.value).collect());
    let names: Rc<Vec<&'static str>> =
        Rc::new(props.options.iter().map(|option| option.name).collect());
    let labels: Vec<Str> = props
        .options
        .iter()
        .map(|option| Rc::clone(&option.label))
        .collect();
    let index_of = {
        let values = Rc::clone(&values);
        move |value: &Value| match value {
            Value::Enum { variant, .. } => values.iter().position(|candidate| candidate == variant),
            _ => None,
        }
    };
    let select_index: Rc<dyn Fn(usize)> = {
        let values = Rc::clone(&values);
        let bound = bound.clone();
        Rc::new(move |index| {
            if let Some(variant) = values.get(index) {
                bound.set(Value::unit_variant(*variant));
            }
        })
    };
    let segmented = matches!(
        props.style,
        Some("segmented") | Some("radio") | Some("inline")
    );
    let control: gtk::Widget = if segmented {
        let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        group.add_css_class("linked");
        group.set_valign(gtk::Align::Center);
        let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
        for (index, _) in props.options.iter().enumerate() {
            let button = gtk::ToggleButton::new();
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            let select = Rc::clone(&select_index);
            button.connect_toggled(move |button| {
                if button.is_active() {
                    select(index);
                }
            });
            group.append(&button);
            buttons.push(button);
        }
        let shown = buttons.clone();
        label_model(ctx, labels, move |texts| {
            for (button, text) in shown.iter().zip(texts) {
                button.set_label(text);
            }
        });
        let read = bound.clone();
        let index_of = index_of.clone();
        effect(move || {
            if let Some(index) = index_of(&read.get())
                && let Some(button) = buttons.get(index)
                && !button.is_active()
            {
                button.set_active(true);
            }
        });
        if let Some(label) = label.clone() {
            let shown = group.clone();
            effect(move || shown.update_property(&[gtk::accessible::Property::Label(&label())]));
        }
        group.upcast()
    } else {
        let model = gtk::StringList::new(&[]);
        let updating = Rc::new(Cell::new(false));
        let (selected_owner, widget): (glib::Object, gtk::Widget) =
            if p.is_list() && !ctx.in_toolbar() {
                let row = adw::ComboRow::new();
                row.set_model(Some(&model));
                if let Some(label) = label.clone() {
                    let shown = row.clone();
                    effect(move || shown.set_title(&label()));
                }
                (row.clone().upcast(), row.upcast())
            } else {
                let dropdown = gtk::DropDown::new(Some(model.clone()), None::<gtk::Expression>);
                dropdown.set_valign(gtk::Align::Center);
                (dropdown.clone().upcast(), dropdown.upcast())
            };
        let reselect = {
            let owner = selected_owner.clone();
            let read = bound.clone();
            let index_of = index_of.clone();
            let guard = Rc::clone(&updating);
            move || {
                if let Some(index) = index_of(&read.peek())
                    && owner.property::<u32>("selected") != index as u32
                {
                    guard.set(true);
                    owner.set_property("selected", index as u32);
                    guard.set(false);
                }
            }
        };
        let relabel = reselect.clone();
        let guard = Rc::clone(&updating);
        label_model(ctx, labels, move |texts| {
            guard.set(true);
            let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
            model.splice(0, model.n_items(), &texts);
            guard.set(false);
            relabel();
        });
        let read = bound.clone();
        effect(move || {
            read.get();
            reselect();
        });
        let select = Rc::clone(&select_index);
        selected_owner.connect_notify_local(Some("selected"), move |owner, _| {
            if !updating.get() {
                select(owner.property::<u32>("selected") as usize);
            }
        });
        widget
    };
    if ctx.in_toolbar() {
        p.add(&control);
    } else if segmented && p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&label())));
        }
        row.add_suffix(&control);
        p.add(&row);
    } else if p.is_list() {
        p.add(&control);
    } else {
        p.add(&beside(label.clone(), &control));
    }
    let select = {
        let names = Rc::clone(&names);
        let select_index = Rc::clone(&select_index);
        Rc::new(
            move |name: &str| match names.iter().position(|candidate| *candidate == name) {
                Some(index) => {
                    select_index(index);
                    true
                }
                None => false,
            },
        )
    };
    let value = bound.clone();
    controls::register(
        ctx,
        props.id,
        "picker",
        label,
        None,
        Actions {
            select: Some(select),
            value: Some(Rc::new(move || {
                index_of(&value.peek()).map(|index| names[index].to_string())
            })),
            ..Actions::default()
        },
    );
}

#[derive(Default)]
pub struct StackProps {
    /// `column` or `row`.
    pub axis: &'static str,
    pub gap: Option<Num>,
    pub padding: Option<Num>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub align: Option<&'static str>,
    /// The spring bound lengths change with.
    pub animate: Option<Curve>,
    /// Children slide to new places with this spring.
    pub reflow: Option<Curve>,
}

/// Padding on every side of `widget`, as its margins.
fn set_padding(widget: &gtk::Widget, padding: f64) {
    let padding = padding.round().max(0.0) as i32;
    widget.set_margin_top(padding);
    widget.set_margin_bottom(padding);
    widget.set_margin_start(padding);
    widget.set_margin_end(padding);
}

/// `width` and `height` fix a side of `widget`: it neither shrinks below
/// nor stretches past it.
fn bind_size(
    ctx: &Ctx,
    widget: &gtk::Widget,
    width: &Option<Num>,
    height: &Option<Num>,
    animate: Option<Curve>,
) {
    let size = Rc::new(Cell::new((-1, -1)));
    if let Some(width) = width {
        widget.set_hexpand(false);
        let sized = widget.clone();
        let size = Rc::clone(&size);
        bind_number(ctx, width, animate, widget, move |value| {
            let (_, height) = size.get();
            let width = value.round().max(0.0) as i32;
            size.set((width, height));
            sized.set_size_request(width, height);
        });
    }
    if let Some(height) = height {
        widget.set_vexpand(false);
        let sized = widget.clone();
        bind_number(ctx, height, animate, widget, move |value| {
            let (width, _) = size.get();
            let height = value.round().max(0.0) as i32;
            size.set((width, height));
            sized.set_size_request(width, height);
        });
    }
}

#[allow(non_snake_case)]
pub fn Stack(p: &Parent, ctx: &Ctx, props: StackProps, children: Build) {
    let column = props.axis == "column";
    let orientation = if column {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    };
    let cross = match (props.align, column) {
        (Some("center"), _) => gtk::Align::Center,
        (Some("trailing") | Some("bottom"), _) => gtk::Align::End,
        (Some("leading") | Some("top"), _) => gtk::Align::Start,
        (Some("fill"), _) => gtk::Align::Fill,
        (None, true) => gtk::Align::Fill,
        (None, false) => gtk::Align::Center,
        _ => gtk::Align::Fill,
    };
    // A column or row whose children slide takes a container of its own;
    // others are GTK boxes.
    let (container, inner): (gtk::Widget, Parent) = match props.reflow {
        Some(curve) => {
            let reflow = crate::layout::ReflowBox::new(orientation, 8, curve);
            if let Some(gap) = &props.gap {
                let spaced = reflow.clone();
                bind_number(ctx, gap, props.animate, &reflow, move |value| {
                    spaced.set_spacing(value.round() as i32);
                });
            }
            let inner = Parent::of_reflow(&reflow, cross);
            (reflow.upcast(), inner)
        }
        None => {
            let container = gtk::Box::new(orientation, 8);
            if let Some(gap) = &props.gap {
                let spaced = container.clone();
                bind_number(ctx, gap, props.animate, &container, move |value| {
                    spaced.set_spacing(value.round().max(0.0) as i32);
                });
            }
            let inner = Parent::of_aligned_box(&container, cross);
            (container.upcast(), inner)
        }
    };
    if let Some(padding) = &props.padding {
        let padded = container.clone();
        bind_number(ctx, padding, props.animate, &container, move |value| {
            set_padding(&padded, value);
        });
    }
    bind_size(ctx, &container, &props.width, &props.height, props.animate);
    children(&inner, ctx);
    place(p, &container);
}

#[derive(Default)]
pub struct GridProps {
    pub min_width: Option<Num>,
    pub gap: Option<Num>,
    pub padding: Option<Num>,
    pub animate: Option<Curve>,
}

/// Equal columns, as many as fit at `min_width` or wider: a flow box whose
/// cells share each line. A bound `min_width` springs with `animate`, and
/// the cells move to their new places as the columns change.
#[allow(non_snake_case)]
pub fn Grid(p: &Parent, ctx: &Ctx, props: GridProps, children: Build) {
    let flow = gtk::FlowBox::new();
    flow.set_homogeneous(true);
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_activate_on_single_click(false);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(64);
    flow.set_valign(gtk::Align::Start);
    flow.set_row_spacing(8);
    flow.set_column_spacing(8);
    if let Some(gap) = &props.gap {
        let spaced = flow.clone();
        bind_number(ctx, gap, props.animate, &flow, move |value| {
            let gap = value.round().max(0.0) as u32;
            spaced.set_row_spacing(gap);
            spaced.set_column_spacing(gap);
        });
    }
    if let Some(padding) = &props.padding {
        let padded = flow.clone().upcast::<gtk::Widget>();
        bind_number(ctx, padding, props.animate, &flow, move |value| {
            set_padding(&padded, value);
        });
    }
    let first = props
        .min_width
        .as_ref()
        .map(|width| width.peek(ctx))
        .unwrap_or(120.0);
    let cells = Parent::of_flow(&flow, Rc::new(Cell::new(first.round() as i32)));
    if let Some(min_width) = &props.min_width {
        let resized = cells.clone();
        bind_number(ctx, min_width, props.animate, &flow, move |value| {
            resized.set_cell_width(value.round().max(1.0) as i32);
        });
    }
    children(&cells, ctx);
    place(p, &flow);
}

#[derive(Default)]
pub struct LayersProps {
    pub align: Option<&'static str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub animate: Option<Curve>,
}

/// `Stack`: layers in one place, later ones above earlier ones, the stack
/// as large as its largest layer.
#[allow(non_snake_case)]
pub fn Layers(p: &Parent, ctx: &Ctx, props: LayersProps, children: Build) {
    use gtk::Align::{Center, End, Start};
    let (valign, halign) = match props.align.unwrap_or("center") {
        "top_leading" => (Start, Start),
        "top" => (Start, Center),
        "top_trailing" => (Start, End),
        "leading" => (Center, Start),
        "trailing" => (Center, End),
        "bottom_leading" => (End, Start),
        "bottom" => (End, Center),
        "bottom_trailing" => (End, End),
        _ => (Center, Center),
    };
    let layers = crate::layout::Layers::new();
    bind_size(
        ctx,
        layers.upcast_ref(),
        &props.width,
        &props.height,
        props.animate,
    );
    children(&Parent::of_layers(&layers, halign, valign), ctx);
    place(p, &layers);
}

#[derive(Default)]
pub struct ImageProps {
    /// Name of the declared resource.
    pub source: &'static str,
    pub label: Option<Str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub fit: Option<&'static str>,
    pub animate: Option<Curve>,
}

/// A picture from a declared resource. With one side given, the other
/// follows the picture's aspect ratio. Without a label it is decorative.
#[allow(non_snake_case)]
pub fn Image(p: &Parent, ctx: &Ctx, props: ImageProps) {
    let path = std::path::Path::new(&crate::host::resources_dir()).join(props.source);
    let picture = match crate::hdr::texture(&path).or_else(|| oriented(&path)) {
        Some(texture) => gtk::Picture::for_paintable(&texture),
        None => gtk::Picture::for_filename(&path),
    };
    picture.set_can_shrink(true);
    picture.set_content_fit(match props.fit {
        Some("cover") => gtk::ContentFit::Cover,
        Some("fill") => gtk::ContentFit::Fill,
        _ => gtk::ContentFit::Contain,
    });
    let (natural_width, natural_height) = picture
        .paintable()
        .map(|paintable| (paintable.intrinsic_width(), paintable.intrinsic_height()))
        .unwrap_or((0, 0));
    let ratio = if natural_width > 0 && natural_height > 0 {
        f64::from(natural_width) / f64::from(natural_height)
    } else {
        1.0
    };
    match &props.label {
        Some(label) => {
            let text = bound_text(ctx, label);
            let described = picture.clone();
            effect(move || described.set_alternative_text(Some(&text())));
        }
        None => picture.update_state(&[gtk::accessible::State::Hidden(true)]),
    }
    let frame = crate::picture::Frame::new(&picture, 0, 0);
    let sides: Rc<Cell<(Option<f64>, Option<f64>)>> = Rc::new(Cell::new((None, None)));
    let resize = {
        let frame = frame.clone();
        let sides = Rc::clone(&sides);
        Rc::new(move || {
            let (width, height) = sides.get();
            let w = width.or(height.map(|height| height * ratio)).unwrap_or(0.0);
            let h = height.or(width.map(|width| width / ratio)).unwrap_or(0.0);
            frame.set_size(w.round() as i32, h.round() as i32);
        })
    };
    if let Some(width) = &props.width {
        let sides = Rc::clone(&sides);
        let resize = Rc::clone(&resize);
        bind_number(ctx, width, props.animate, &frame, move |value| {
            sides.set((Some(value), sides.get().1));
            resize();
        });
    }
    if let Some(height) = &props.height {
        let sides = Rc::clone(&sides);
        let resize = Rc::clone(&resize);
        bind_number(ctx, height, props.animate, &frame, move |value| {
            sides.set((sides.get().0, Some(value)));
            resize();
        });
    }
    place(p, &frame);
}

/// A JPEG turned upright by its EXIF orientation, which GTK's own loader
/// ignores. gdk-pixbuf reads it; for JPEG it decodes the same 8-bit
/// picture GTK would. Other formats are left to GTK's loaders.
fn oriented(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if extension != "jpg" && extension != "jpeg" {
        return None;
    }
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    let upright = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
    Some(gtk::gdk::Texture::for_pixbuf(&upright))
}

#[derive(Default)]
pub struct ProgressProps {
    pub value: Option<Get>,
    pub total: Option<f64>,
    pub label: Option<Str>,
}

/// A progress bar of `value` out of `total` under its label, or without a
/// value a spinner beside it.
#[allow(non_snake_case)]
pub fn Progress(p: &Parent, ctx: &Ctx, props: ProgressProps) {
    let label = props.label.as_ref().map(|text| {
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_wrap(true);
        bind_label(&label, bound_text(ctx, text));
        label
    });
    let container = match props.value {
        Some(value) => {
            let total = props.total.unwrap_or(1.0);
            let bar = gtk::ProgressBar::new();
            let shown = bar.clone();
            let ctx = ctx.clone();
            effect(move || {
                let current = as_f64(&value(&ctx));
                shown.set_fraction((current / total).clamp(0.0, 1.0));
            });
            let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
            column.set_hexpand(true);
            if let Some(label) = &label {
                bar.update_relation(&[gtk::accessible::Relation::LabelledBy(
                    &[label.upcast_ref()],
                )]);
                column.append(label);
            }
            column.append(&bar);
            column
        }
        None => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let spinner = gtk::Spinner::new();
            spinner.start();
            row.append(&spinner);
            if let Some(label) = &label {
                label.add_css_class("dim-label");
                spinner.update_relation(&[gtk::accessible::Relation::LabelledBy(&[
                    label.upcast_ref()
                ])]);
                row.append(label);
            }
            row
        }
    };
    place(p, &container);
}

#[allow(non_snake_case)]
pub fn Spacer(p: &Parent) {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    if p.is_horizontal() {
        spacer.set_hexpand(true);
    } else {
        spacer.set_vexpand(true);
    }
    p.add(&spacer);
}

#[allow(non_snake_case)]
pub fn Divider(p: &Parent) {
    let orientation = if p.is_horizontal() {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    };
    place(p, &gtk::Separator::new(orientation));
}

#[derive(Default)]
pub struct FormProps {
    pub title: Option<Str>,
}

/// A grouped form: sections of boxed lists in a clamped, scrolling page,
/// as libadwaita preference pages lay them out.
#[allow(non_snake_case)]
pub fn Form(p: &Parent, ctx: &Ctx, props: FormProps, children: Build) {
    if let (Some(title), Some(page)) = (&props.title, ctx.page()) {
        page.push_title(bound_text(ctx, title));
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(680);
    clamp.set_tightening_threshold(480);
    clamp.set_child(Some(&content));
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_child(Some(&clamp));
    children(&Parent::of_box(&content), ctx);
    p.add(&scrolled);
}

#[derive(Default)]
pub struct SectionProps {
    pub title: Option<Str>,
    pub footer: Option<Str>,
}

/// A titled group of rows with an optional footnote.
#[allow(non_snake_case)]
pub fn Section(p: &Parent, ctx: &Ctx, props: SectionProps, children: Build) {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if let Some(title) = &props.title {
        let heading = gtk::Label::new(None);
        heading.set_xalign(0.0);
        heading.set_wrap(true);
        heading.add_css_class("heading");
        heading.set_margin_bottom(6);
        bind_label(&heading, bound_text(ctx, title));
        group.append(&heading);
    }
    let list = boxed_list();
    group.append(&list);
    if let Some(footer) = &props.footer {
        let note = gtk::Label::new(None);
        note.set_xalign(0.0);
        note.set_wrap(true);
        note.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        note.add_css_class("dim-label");
        note.add_css_class("caption");
        note.set_margin_top(6);
        bind_label(&note, bound_text(ctx, footer));
        group.append(&note);
    }
    children(&Parent::of_list(&list), ctx);
    p.add(&group);
}

/// Toolbar items for the enclosing page's header.
#[allow(non_snake_case)]
pub fn Toolbar(_p: &Parent, ctx: &Ctx, children: Build) {
    let Some(page) = ctx.page().cloned() else {
        return;
    };
    let holder = page.add_tools();
    let shown = holder.clone();
    let at = ctx.clone();
    effect(move || shown.set_visible(at.shown()));
    children(&Parent::of_box(&holder), &ctx.toolbar());
}

#[derive(Default)]
pub struct LanguagePickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
}

#[allow(non_snake_case)]
pub fn LanguagePicker(p: &Parent, ctx: &Ctx, props: LanguagePickerProps) {
    let supported: Rc<Vec<String>> = Rc::new(text::with(|language| language.supported.clone()));
    let names: Vec<String> = supported.iter().map(|tag| text::autonym(tag)).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&names);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let (owner, widget): (glib::Object, gtk::Widget) = if p.is_list() {
        let row = adw::ComboRow::new();
        row.set_model(Some(&model));
        if let Some(label) = label.clone() {
            let shown = row.clone();
            effect(move || shown.set_title(&label()));
        }
        (row.clone().upcast(), row.upcast())
    } else {
        let dropdown = gtk::DropDown::new(Some(model), None::<gtk::Expression>);
        (dropdown.clone().upcast(), dropdown.upcast())
    };
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = owner.clone();
    let tags = Rc::clone(&supported);
    effect(move || {
        let current = text::language();
        if let Some(index) = tags.iter().position(|tag| *tag == current)
            && shown.property::<u32>("selected") != index as u32
        {
            guard.set(true);
            shown.set_property("selected", index as u32);
            guard.set(false);
        }
    });
    let tags = Rc::clone(&supported);
    owner.connect_notify_local(Some("selected"), move |owner, _| {
        if updating.get() {
            return;
        }
        let index = owner.property::<u32>("selected") as usize;
        if let Some(tag) = tags.get(index) {
            let tag = tag.clone();
            // Text bindings update after the selection change finishes.
            glib::idle_add_local_once(move || {
                text::set_language(&tag);
            });
        }
    });
    if p.is_list() {
        p.add(&widget);
    } else {
        p.add(&beside(label.clone(), &widget));
    }
    controls::register(
        ctx,
        props.id,
        "picker",
        label,
        None,
        Actions {
            select: Some(Rc::new(|tag: &str| text::set_language(tag))),
            value: Some(Rc::new(|| {
                Some(text::with(|language| language.current.peek()))
            })),
            ..Actions::default()
        },
    );
}

/// An item of a menu; `None` entries are dividers.
pub struct MenuItem {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub role: Option<&'static str>,
    pub enabled: Option<Get>,
    pub action: Option<Act>,
    pub pick: Option<crate::features::Pick>,
}

#[derive(Default)]
pub struct MenuProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
}

/// A menu button. Each item is registered on the button, so external
/// entries reach the items while the menu is closed.
#[allow(non_snake_case)]
pub fn Menu(p: &Parent, ctx: &Ctx, props: MenuProps, items: Vec<Option<MenuItem>>) {
    let button = gtk::MenuButton::new();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    match (props.icon, label.clone()) {
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
    let group = gio::SimpleActionGroup::new();
    let mut entries: Vec<Option<(String, crate::TextFn)>> = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Some(item) = item else {
            entries.push(None);
            continue;
        };
        let name = format!("i{index}");
        let action = gio::SimpleAction::new(&name, None);
        let (run, set_file) =
            crate::features::picking(ctx, item.pick, bound_action(ctx, &item.action));
        if let Some(run) = run.clone() {
            action.connect_activate(move |_, _| run());
        }
        let enabled = bound_enabled(ctx, &item.enabled);
        if let Some(enabled) = enabled.clone() {
            let shown = action.clone();
            effect(move || shown.set_enabled(enabled()));
        }
        group.add_action(&action);
        let item_label = item
            .label
            .as_ref()
            .map(|label| bound_text(ctx, label))
            .unwrap_or_else(|| Rc::new(String::new));
        controls::register(
            ctx,
            item.id,
            "button",
            Some(Rc::clone(&item_label)),
            enabled,
            Actions {
                activate: run,
                set_file,
                file_kinds: item.pick.map(|pick| pick.kinds),
                ..Actions::default()
            },
        );
        let _ = (item.icon, item.role);
        entries.push(Some((format!("menu.{name}"), item_label)));
    }
    button.insert_action_group("menu", Some(&group));
    let shown = button.clone();
    effect(move || {
        let menu = gio::Menu::new();
        let mut section = gio::Menu::new();
        for entry in &entries {
            match entry {
                Some((action, label)) => section.append(Some(&label()), Some(action)),
                None => {
                    menu.append_section(None, &section);
                    section = gio::Menu::new();
                }
            }
        }
        menu.append_section(None, &section);
        shown.set_menu_model(Some(&menu));
    });
    let open = button.clone();
    controls::register(
        ctx,
        props.id,
        "menu",
        label,
        None,
        Actions {
            value: Some(Rc::new(move || {
                Some(if open.is_active() { "open" } else { "closed" }.to_string())
            })),
            ..Actions::default()
        },
    );
    place(p, &button);
}

/// Text of a value for `Text(content: …)` with runtime data: integers,
/// floats and texts as Bern formats them, enums by their label.
pub fn display(value: &Value, enum_name: Option<&str>) -> String {
    match (value, enum_name) {
        (Value::Enum { variant, .. }, Some(name)) => label(name, *variant),
        (value, _) => crate::value::plain(value),
    }
}

/// The localized label of an enum variant.
pub fn label(enum_name: &str, variant: u32) -> String {
    let host = crate::host::host();
    let Some(def) = host.def.types.enum_def(enum_name) else {
        return String::new();
    };
    let Some(variant) = def.variants.get(variant as usize) else {
        return String::new();
    };
    tr(&crate::protocol::label_key(enum_name, &variant.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::tests::{app, start, unit_enum};

    #[test]
    fn text_styles_map_to_libadwaita_classes() {
        for (style, class) in [
            (Some("large_title"), "title-1"),
            (Some("title"), "title-2"),
            (Some("title2"), "title-3"),
            (Some("title3"), "title-4"),
            (Some("headline"), "heading"),
            (Some("subheadline"), "heading"),
            (Some("footnote"), "caption"),
            (Some("caption2"), "caption"),
            (Some("body"), "body"),
            (None, "body"),
        ] {
            assert_eq!(style_class(style), class, "{style:?}");
        }
    }

    #[test]
    fn values_show_as_text_or_by_their_enum_label() {
        let mut def = app();
        def.types.enums = vec![unit_enum("Mood", &["Calm", "Busy"])];
        def.strings = &[("en", &[("Mood.Busy", "Busy now")])];
        start(def);
        assert_eq!(display(&Value::unit_variant(1), Some("Mood")), "Busy now");
        // Without a translation, the label is its key.
        assert_eq!(display(&Value::unit_variant(0), Some("Mood")), "Mood.Calm");
        assert_eq!(display(&Value::Float(2.0), Some("Mood")), "2");
        assert_eq!(display(&Value::unit_variant(1), None), "");
        assert_eq!(label("Mood", 7), "");
        assert_eq!(label("Weather", 0), "");
    }
}
