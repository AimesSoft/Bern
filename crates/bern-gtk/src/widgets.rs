//! Standard controls and containers as GTK 4 and libadwaita widgets.
//!
//! Inside a form section, controls are libadwaita rows (`AdwSwitchRow`,
//! `AdwEntryRow`, `AdwComboRow` and action rows) in a boxed list; elsewhere
//! they are the plain GTK controls. Colors and type come from the system
//! style: widgets only use libadwaita's style classes.

use crate::controls::{self, Actions};
use crate::ctx::Field;
use crate::motion::{Curve, Num, bind_number};
use crate::parent::keeps_width;
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
/// libadwaita row itself. Its padding is the row's, so a form's layout
/// can change it.
pub(crate) fn plain_row(child: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.add_css_class("bern-plain-row");
    row.set_child(Some(child.as_ref()));
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
        Some("code") => "monospace",
        _ => "body",
    }
}

#[derive(Default)]
pub struct TextProps {
    pub content: Option<Str>,
    pub style: Option<&'static str>,
    pub emphasis: Option<&'static str>,
    /// The accent or a status role the text is drawn in.
    pub color: Option<&'static str>,
    /// A capsule of this color under each line.
    pub mark: Option<crate::theme::Color>,
    /// At most this many lines, the last ending in an ellipsis.
    pub lines: Option<f64>,
    /// The size of the type in points, in place of the style's.
    pub font_size: Option<Num>,
    /// The height of each line, the extra split above and below it.
    pub line_height: Option<Num>,
    /// The content is inline Markdown.
    pub markdown: bool,
}

/// The size and line height of a label's text, as Pango attributes.
fn text_attributes(label: &gtk::Label, size: Option<f64>, line_height: Option<f64>) {
    let attributes = gtk::pango::AttrList::new();
    let scale = f64::from(gtk::pango::SCALE);
    if let Some(size) = size {
        attributes.insert(gtk::pango::AttrSize::new_size_absolute(
            (size.max(1.0) * scale).round() as i32,
        ));
    }
    if let Some(height) = line_height {
        attributes.insert(gtk::pango::AttrInt::new_line_height_absolute(
            (height.max(0.0) * scale).round() as i32,
        ));
    }
    label.set_attributes(Some(&attributes));
}

/// Keep a label's size and line height in step with bound lengths.
fn bind_text_size(ctx: &Ctx, label: &gtk::Label, size: Option<Num>, line_height: Option<Num>) {
    if size.is_none() && line_height.is_none() {
        return;
    }
    let values = Rc::new(Cell::new((None::<f64>, None::<f64>)));
    if let Some(size) = &size {
        let (values, label_) = (Rc::clone(&values), label.clone());
        bind_number(ctx, size, None, label, move |value| {
            values.set((Some(value), values.get().1));
            let (size, height) = values.get();
            text_attributes(&label_, size, height);
        });
    }
    if let Some(height) = &line_height {
        let (values, label_) = (Rc::clone(&values), label.clone());
        bind_number(ctx, height, None, label, move |value| {
            values.set((values.get().0, Some(value)));
            let (size, height) = values.get();
            text_attributes(&label_, size, height);
        });
    }
}

/// The classes of a text style: libadwaita's for its size, and Bern's for
/// the weights and sizes `text { … }` gives.
fn text_style(widget: &impl IsA<gtk::Widget>, style: Option<&str>) {
    // `control`: the font of the control it stands in, which the label
    // inherits, as a button's title does.
    if style == Some("control") {
        return;
    }
    widget.add_css_class(style_class(style));
    widget.add_css_class(&crate::style::text_class(style.unwrap_or("body")));
}

#[allow(non_snake_case)]
pub fn Text(p: &Parent, ctx: &Ctx, props: TextProps) {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    text_style(&label, props.style);
    if matches!(props.emphasis, Some("secondary") | Some("tertiary")) {
        label.add_css_class("dim-label");
    }
    // libadwaita's text colors of the accent and the status colors.
    match props.color {
        Some("accent") => label.add_css_class("accent"),
        Some("status.success") => label.add_css_class("success"),
        Some("status.warning") => label.add_css_class("warning"),
        Some("status.error") => label.add_css_class("error"),
        _ => {}
    }
    if let Some(lines) = props.lines {
        label.set_lines(lines.max(1.0) as i32);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    }
    bind_text_size(ctx, &label, props.font_size, props.line_height);
    if let Some(content) = &props.content {
        if props.markdown {
            let text = bound_text(ctx, content);
            let shown = label.clone();
            shown.set_use_markup(true);
            effect(move || shown.set_markup(&crate::markdown::markup(&text())));
        } else {
            bind_label(&label, bound_text(ctx, content));
        }
    }
    match props.mark {
        Some(mark) => place(p, &crate::canvas::Marked::new(&label, mark)),
        None => place(p, &crate::layout::Words::wrap(&label)),
    }
}

#[derive(Default)]
pub struct LabelProps {
    pub title: Option<Str>,
    pub icon: Option<&'static str>,
    pub style: Option<&'static str>,
    /// The size of the title and icon in points.
    pub font_size: Option<f64>,
    /// Only the icon shows; the title names it for assistive technology.
    pub icon_only: bool,
}

#[allow(non_snake_case)]
pub fn Label(p: &Parent, ctx: &Ctx, props: LabelProps) {
    let title = props.title.as_ref().map(|title| bound_text(ctx, title));
    let icon = |name: &str| {
        let image = icons::image(name);
        if let Some(size) = props.font_size {
            image.set_pixel_size(size.round().max(1.0) as i32);
        }
        image
    };
    if props.icon_only {
        // An image's role is already `img`.
        let image = icon(props.icon.unwrap_or("circle"));
        if let Some(title) = title {
            let named = image.clone();
            effect(move || {
                let text = title();
                named.set_tooltip_text(Some(&text));
                named.update_property(&[gtk::accessible::Property::Label(&text)]);
            });
        }
        place(p, &image);
        return;
    }
    if p.is_list() {
        let row = adw::ActionRow::new();
        if let Some(name) = props.icon {
            row.add_prefix(&icon(name));
        }
        if let Some(title) = title {
            let shown = row.clone();
            effect(move || shown.set_title(&glib::markup_escape_text(&title())));
        }
        p.add(&row);
        return;
    }
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if let Some(name) = props.icon {
        container.append(&icon(name));
    }
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    text_style(&label, props.style);
    if let Some(size) = props.font_size {
        text_attributes(&label, Some(size), None);
    }
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
    /// `paste:`: the clipboard's text goes into this draft first.
    pub paste: Option<crate::ctx::Field>,
    pub shape: Shape,
    /// `width: fill`: as wide as its container.
    pub fill: bool,
}

fn register_button(
    ctx: &Ctx,
    button: &gtk::Button,
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
            focus: Some(crate::features::focus_of(button.upcast_ref())),
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
    let run = crate::features::pasting(ctx, props.paste, run);
    if let Some(run) = run.clone() {
        button.connect_clicked(move |_| run());
    }
    register_button(
        ctx,
        &button,
        props.id,
        label,
        enabled,
        run,
        set_file.zip(props.pick.map(|pick| pick.kinds)),
    );
    shape(ctx, button.upcast_ref(), &props.shape);
    if props.fill {
        button.set_hexpand(true);
        button.set_halign(gtk::Align::Fill);
    }
    if p.is_list() && !ctx.in_toolbar() {
        if !props.fill {
            button.set_halign(gtk::Align::Start);
        }
        let row = plain_row(&styled(&button, props.style));
        row.add_css_class("bern-button-row");
        p.add(&row);
    } else {
        let widget = styled(&button, props.style);
        if !props.fill {
            keeps_width(&widget);
        }
        p.add(&widget);
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
    /// `paste:`: the clipboard's text goes into this draft first.
    pub paste: Option<crate::ctx::Field>,
    /// The size of the icon in points.
    pub font_size: Option<f64>,
    /// A round button of this diameter.
    pub diameter: Option<Num>,
    /// `prominent`: filled with the accent color.
    pub prominence: Option<&'static str>,
}

#[allow(non_snake_case)]
pub fn IconButton(p: &Parent, ctx: &Ctx, props: IconButtonProps, action: Option<Act>) {
    let button = gtk::Button::from_icon_name(icons::name(props.icon));
    if props.prominence == Some("prominent") {
        button.add_css_class("suggested-action");
    } else {
        button.add_css_class("flat");
    }
    if let Some(size) = props.font_size
        && let Some(image) = button.child().and_downcast::<gtk::Image>()
    {
        image.set_pixel_size(size.round().max(1.0) as i32);
    }
    if let Some(diameter) = &props.diameter {
        button.add_css_class("circular");
        button.add_css_class("bern-sized-control");
        let sized = button.clone();
        bind_number(ctx, diameter, None, &button, move |value| {
            let side = value.round().max(0.0) as i32;
            sized.set_size_request(side, side);
        });
    }
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
    let run = crate::features::pasting(ctx, props.paste, run);
    if let Some(run) = run.clone() {
        button.connect_clicked(move |_| run());
    }
    register_button(
        ctx,
        &button,
        props.id,
        label,
        enabled,
        run,
        set_file.zip(props.pick.map(|pick| pick.kinds)),
    );
    button.set_valign(gtk::Align::Center);
    let widget = styled(&button, props.style);
    keeps_width(&widget);
    place(p, &widget);
}

/// What a control shows and changes.
#[derive(Clone)]
pub enum Model {
    /// A draft field of the window; the action, if any, runs after each
    /// change.
    Field(Field, Option<Act>),
    /// A value read from state; a change runs this with the new value, as
    /// a command that sets it.
    Controlled(Get, Commit),
}

/// Runs the command that changes a controlled value to the one given.
pub type Commit = Rc<dyn Fn(&Ctx, Value)>;

/// Turn a control bound to a `bool` on or off, as its shortcut does.
pub fn toggle(ctx: &Ctx, model: &Model) {
    let bound = Bound::model(ctx, model.clone());
    bound.set(Value::Bool(bound.peek() != Value::Bool(true)));
}

/// The value a control is bound to.
#[derive(Clone)]
pub(crate) struct Bound {
    ctx: Ctx,
    model: Model,
}

impl Bound {
    pub(crate) fn new(ctx: &Ctx, field: Field) -> Self {
        Self::model(ctx, Model::Field(field, None))
    }

    pub(crate) fn model(ctx: &Ctx, model: Model) -> Self {
        Self {
            ctx: ctx.clone(),
            model,
        }
    }

    /// Whether the engine controls the value: the control previews its
    /// own changes until they arrive.
    pub(crate) fn controlled(&self) -> bool {
        matches!(self.model, Model::Controlled(..))
    }

    pub(crate) fn get(&self) -> Value {
        match &self.model {
            Model::Field(field, _) => self.ctx.read(*field),
            Model::Controlled(get, _) => get(&self.ctx),
        }
    }

    pub(crate) fn peek(&self) -> Value {
        match &self.model {
            Model::Field(field, _) => self.ctx.peek(*field),
            Model::Controlled(get, _) => untrack(|| get(&self.ctx)),
        }
    }

    pub(crate) fn set(&self, value: Value) {
        if self.peek() == value {
            return;
        }
        match &self.model {
            Model::Field(field, then) => {
                if self.ctx.write(*field, value)
                    && let Some(then) = then
                {
                    then(&self.ctx);
                }
            }
            Model::Controlled(_, commit) => commit(&self.ctx, value),
        }
    }
}

/// `shape: capsule` and `height` of a system control.
#[derive(Clone, Default)]
pub struct Shape {
    pub capsule: bool,
    pub height: Option<Num>,
}

/// Round a control's ends and give it its height: the theme's own
/// minimum height and vertical padding give way to the declared height.
pub(crate) fn shape(ctx: &Ctx, widget: &gtk::Widget, shape: &Shape) {
    if shape.capsule {
        widget.add_css_class("bern-capsule");
    }
    if let Some(height) = &shape.height {
        widget.add_css_class("bern-sized-control");
        let sized = widget.clone();
        bind_number(ctx, height, None, widget, move |value| {
            sized.set_size_request(sized.width_request(), value.round().max(0.0) as i32);
        });
        widget.set_valign(gtk::Align::Center);
    }
}

/// `padding`, `padding_x` and `padding_y` of a container.
#[derive(Clone, Default)]
pub struct Padding {
    pub all: Option<Num>,
    pub x: Option<Num>,
    pub y: Option<Num>,
}

/// Padding as the widget's margins: `all` on every side, then `x` across
/// and `y` along the vertical, which win where given.
pub(crate) fn pad(ctx: &Ctx, widget: &gtk::Widget, padding: &Padding, animate: Option<Curve>) {
    let side = |value: f64| value.round().max(0.0) as i32;
    if let Some(all) = &padding.all {
        let padded = widget.clone();
        let (x, y) = (padding.x.is_none(), padding.y.is_none());
        bind_number(ctx, all, animate, widget, move |value| {
            if x {
                padded.set_margin_start(side(value));
                padded.set_margin_end(side(value));
            }
            if y {
                padded.set_margin_top(side(value));
                padded.set_margin_bottom(side(value));
            }
        });
    }
    if let Some(across) = &padding.x {
        let padded = widget.clone();
        bind_number(ctx, across, animate, widget, move |value| {
            padded.set_margin_start(side(value));
            padded.set_margin_end(side(value));
        });
    }
    if let Some(down) = &padding.y {
        let padded = widget.clone();
        bind_number(ctx, down, animate, widget, move |value| {
            padded.set_margin_top(side(value));
            padded.set_margin_bottom(side(value));
        });
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

/// Names `widget` for assistive technologies where no label shows.
fn accessible_name(widget: &impl IsA<gtk::Widget>, label: Option<Rc<dyn Fn() -> String>>) {
    if let Some(label) = label {
        let named = widget.clone().upcast::<gtk::Widget>();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
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
    /// `label_hidden`: the label names the field without showing.
    pub label_hidden: bool,
    /// The draft it edits (`value`), or the state it shows and the command
    /// each edit runs (`current`).
    pub model: Option<Model>,
    pub prompt: Option<Str>,
    pub kind: Option<&'static str>,
}

/// Text editing: changes update the draft; draft changes from elsewhere
/// update the text without echoing back.
pub(crate) fn editable(
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: Bound,
) -> Rc<Cell<bool>> {
    editable_unless(text, set, bound, None)
}

/// Like [`editable`]; while `editing` holds, state from elsewhere waits:
/// a field showing engine state keeps what the user types until the
/// engine has caught up with it.
fn editable_unless(
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: Bound,
    editing: Option<Rc<dyn Fn() -> bool>>,
) -> Rc<Cell<bool>> {
    let updating = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    effect(move || {
        let value = as_text(&bound.get());
        if editing.as_ref().is_some_and(|editing| editing()) {
            return;
        }
        if untrack(|| text()) != value {
            guard.set(true);
            set(&value);
            guard.set(false);
        }
    });
    updating
}

/// For a field showing engine state: whether `widget` is being edited, and
/// when it stops, the engine's text again.
fn held_while_focused(
    widget: &impl IsA<gtk::Widget>,
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: &Bound,
    updating: Rc<Cell<bool>>,
) {
    let focus = gtk::EventControllerFocus::new();
    let bound = bound.clone();
    focus.connect_leave(move |_| {
        let value = as_text(&bound.peek());
        if text() != value {
            updating.set(true);
            set(&value);
            updating.set(false);
        }
    });
    widget.as_ref().add_controller(focus);
}

/// Whether the keyboard focus is in `widget`, as in the text of an entry.
fn contains_focus(widget: &gtk::Widget) -> bool {
    widget
        .root()
        .and_then(|root| root.focus())
        .is_some_and(|focus| focus == *widget || focus.is_ancestor(widget))
}

/// The text of a field bound to `bound`; a field showing engine state is
/// held while it is being edited.
fn text_binding(
    widget: &impl IsA<gtk::Widget>,
    text: Rc<dyn Fn() -> String>,
    set: Rc<dyn Fn(&str)>,
    bound: &Bound,
) -> Rc<Cell<bool>> {
    if !bound.controlled() {
        return editable(text, set, bound.clone());
    }
    let watched = widget.as_ref().downgrade();
    let editing: Rc<dyn Fn() -> bool> = Rc::new(move || {
        watched
            .upgrade()
            .is_some_and(|widget| contains_focus(&widget))
    });
    let updating = editable_unless(
        Rc::clone(&text),
        Rc::clone(&set),
        bound.clone(),
        Some(editing),
    );
    held_while_focused(widget, text, set, bound, Rc::clone(&updating));
    updating
}

#[allow(non_snake_case)]
pub fn TextInput(p: &Parent, ctx: &Ctx, props: TextInputProps, submit: Option<Act>) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    // The label shown above or in the field; a hidden one only names it.
    let shown = label.clone().filter(|_| !props.label_hidden);
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
        let updating = text_binding(
            &view,
            Rc::new(move || {
                read.text(&read.start_iter(), &read.end_iter(), false)
                    .to_string()
            }),
            Rc::new(move |value| write.set_text(value)),
            &bound,
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
        accessible_name(&view, label.clone());
        if p.is_list() {
            p.add(&plain_row(&stacked(shown, &frame)));
        } else {
            p.add(&stacked(shown, &frame));
        }
    } else if p.is_list() {
        let row: adw::EntryRow = if secure {
            adw::PasswordEntryRow::new().upcast()
        } else {
            adw::EntryRow::new()
        };
        if let Some(title) = shown {
            let titled = row.clone();
            effect(move || titled.set_title(&title()));
        } else {
            accessible_name(&row, label.clone());
        }
        let read = row.clone();
        let write = row.clone();
        let updating = text_binding(
            &row,
            Rc::new(move || read.text().to_string()),
            Rc::new(move |value| write.set_text(value)),
            &bound,
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
        let updating = text_binding(
            &entry,
            Rc::new(move || read.text().to_string()),
            Rc::new(move |value| write.set_text(value)),
            &bound,
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
        // A field takes the free width of its row, as a text field does in
        // a SwiftUI stack.
        entry.set_hexpand(true);
        if shown.is_none() {
            accessible_name(&entry, label.clone());
            p.add(&entry);
        } else {
            p.add(&stacked(shown, &entry));
        }
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
    /// The prompt of a palette's own field.
    pub panel_prompt: Option<Str>,
    /// The field's size in the page, or a palette's.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// The size of the field's text in points.
    pub font_size: Option<f64>,
    /// The results show on focus, before anything is typed.
    pub suggest: bool,
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
        // In a page: as wide as given, else as wide as its container.
        entry.set_hexpand(props.width.is_none());
        if props.width.is_some() || props.height.is_some() {
            entry.add_css_class("bern-sized-control");
            entry.set_size_request(
                props.width.map(|width| width.round() as i32).unwrap_or(-1),
                props
                    .height
                    .map(|height| height.round() as i32)
                    .unwrap_or(-1),
            );
        }
        if let Some(size) = props.font_size {
            entry.add_css_class(&crate::style::class_for(&format!("font-size: {size}px;")));
        }
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
        search_results(ctx, &entry, &bound, results, props.suggest);
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

fn search_results(
    ctx: &Ctx,
    entry: &gtk::SearchEntry,
    bound: &Bound,
    results: Build,
    suggest: bool,
) {
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
        let open = focused.get() && (suggest || !as_text(&read.get()).is_empty());
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
    pub model: Option<Model>,
    pub style: Option<&'static str>,
    pub icon: Option<&'static str>,
    /// The label names the switch for assistive technology only.
    pub label_hidden: bool,
    pub shape: Shape,
}

#[allow(non_snake_case)]
pub fn Switch(p: &Parent, ctx: &Ctx, props: SwitchProps) {
    let Some(model) = props.model else { return };
    if props.style == Some("button") || ctx.in_toolbar() {
        crate::features::toggle_button(
            p,
            ctx,
            crate::features::ToggleProps {
                id: props.id,
                label: props.label,
                model,
                icon: props.icon,
                shape: props.shape,
            },
        );
        return;
    }
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let shown_label = label.clone().filter(|_| !props.label_hidden);
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
            if let Some(label) = shown_label.clone() {
                let shown = check.clone();
                effect(move || shown.set_label(Some(&label())));
            }
            control = check.clone().upcast();
        }
        active = check.upcast();
    } else {
        let switch = gtk::Switch::new();
        shape(ctx, switch.upcast_ref(), &props.shape);
        control = beside(shown_label.clone(), &switch).upcast();
        active = switch.upcast();
    }
    if props.label_hidden
        && let Some(label) = label.clone()
        && let Ok(widget) = active.clone().downcast::<gtk::Widget>()
    {
        effect(move || widget.update_property(&[gtk::accessible::Property::Label(&label())]));
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
    pub model: Option<Model>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub min_icon: Option<&'static str>,
    pub max_icon: Option<&'static str>,
    /// The label names the slider for assistive technology only.
    pub label_hidden: bool,
    pub continuous: bool,
}

/// A slider. Bound to a draft, dragging only updates this window's draft
/// as a preview and releasing commits once; keyboard and scroll changes
/// commit at once. Bound to a value the engine controls, dragging moves
/// only the slider, and releasing sets the value.
#[allow(non_snake_case)]
pub fn Slider(p: &Parent, ctx: &Ctx, props: SliderProps, commit: Option<Act>) {
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let controlled = bound.controlled();
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
    let dragging = Rc::new(Cell::new(false));
    let guard = Rc::clone(&updating);
    let shown = scale.clone();
    let read = bound.clone();
    let held = Rc::clone(&dragging);
    effect(move || {
        let value = as_f64(&read.get());
        // A drag in progress keeps the slider where the pointer has it.
        if held.get() && controlled {
            return;
        }
        if (shown.value() - value).abs() > f64::EPSILON {
            guard.set(true);
            shown.set_value(value);
            guard.set(false);
        }
    });
    let pressed = Rc::clone(&dragging);
    let released_commit = commit.clone();
    let released = bound.clone();
    let released_scale = scale.clone();
    let legacy = gtk::EventControllerLegacy::new();
    legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
    legacy.connect_event(move |_, event| {
        match event.event_type() {
            gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => pressed.set(true),
            gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::TouchEnd
                if pressed.replace(false) =>
            {
                let commit = released_commit.clone();
                let bound = released.clone();
                let scale = released_scale.clone();
                // After the scale handled the release.
                glib::idle_add_local_once(move || {
                    if controlled {
                        bound.set(Value::Float(snap(scale.value())));
                    }
                    if let Some(commit) = commit {
                        commit();
                    }
                });
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
        if dragging.get() {
            // A preview: the draft follows, a controlled value waits.
            if !controlled || props.continuous {
                changed.set(Value::Float(value));
            }
            return;
        }
        changed.set(Value::Float(value));
        if let Some(commit) = &keyed_commit {
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
    let content = stacked(label.clone().filter(|_| !props.label_hidden), &track);
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

/// One option of a picker: its value, the name of its variant and its
/// label.
pub struct PickOption {
    pub value: Value,
    pub name: &'static str,
    pub label: Str,
}

pub fn option(value: Value, name: &'static str, label: Str) -> PickOption {
    PickOption { value, name, label }
}

#[derive(Default)]
pub struct PickerProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub model: Option<Model>,
    pub options: Vec<PickOption>,
    pub style: Option<&'static str>,
    /// The label names the picker for assistive technology only.
    pub label_hidden: bool,
    pub shape: Shape,
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
    let Some(model) = props.model else { return };
    let bound = Bound::model(ctx, model);
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    let values: Rc<Vec<Value>> = Rc::new(
        props
            .options
            .iter()
            .map(|option| option.value.clone())
            .collect(),
    );
    let names: Rc<Vec<&'static str>> =
        Rc::new(props.options.iter().map(|option| option.name).collect());
    let labels: Vec<Str> = props
        .options
        .iter()
        .map(|option| Rc::clone(&option.label))
        .collect();
    let index_of = {
        let values = Rc::clone(&values);
        move |value: &Value| values.iter().position(|candidate| candidate == value)
    };
    let select_index: Rc<dyn Fn(usize)> = {
        let values = Rc::clone(&values);
        let bound = bound.clone();
        Rc::new(move |index| {
            if let Some(value) = values.get(index) {
                bound.set(value.clone());
            }
        })
    };
    let segmented = matches!(
        props.style,
        Some("segmented") | Some("radio") | Some("inline")
    );
    // Chips without a label of their own in a flow are items of the flow;
    // the picker then has no widget of its own.
    let control: Option<gtk::Widget> = if props.style == Some("chips") {
        // A toggle button per option, one of a group, the chosen one
        // active; elsewhere than in a flow they wrap in a flow of their own.
        let flat = p.is_flow() && props.label_hidden && !ctx.in_toolbar();
        let holder = (!flat).then(|| crate::scene::flow_of(8.0));
        let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
        for index in 0..props.options.len() {
            let button = gtk::ToggleButton::new();
            button.set_valign(gtk::Align::Center);
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            let select = Rc::clone(&select_index);
            button.connect_toggled(move |button| {
                if button.is_active() {
                    select(index);
                }
            });
            match &holder {
                Some(flow) => button.set_parent(flow),
                None => p.add(&button),
            }
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
        holder.map(|flow| {
            if let Some(label) = label.clone()
                && !props.label_hidden
            {
                let shown = flow.clone();
                effect(move || {
                    shown.update_property(&[gtk::accessible::Property::Label(&label())])
                });
            }
            flow.upcast()
        })
    } else if segmented {
        let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        group.add_css_class("linked");
        group.set_valign(gtk::Align::Center);
        shape(ctx, group.upcast_ref(), &props.shape);
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
        Some(group.upcast())
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
                shape(ctx, dropdown.upcast_ref(), &props.shape);
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
        Some(widget)
    };
    if let Some(control) = control {
        if props.label_hidden
            && let Some(label) = label.clone()
        {
            let named = control.clone();
            effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
        }
        if ctx.in_toolbar() || props.label_hidden {
            if p.is_list() {
                p.add(&plain_row(&control));
            } else {
                keeps_width(&control);
                p.add(&control);
            }
        } else if (segmented || props.style == Some("chips")) && p.is_list() {
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
    pub padding: Padding,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub align: Option<&'static str>,
    /// The spring bound lengths change with.
    pub animate: Option<Curve>,
    /// Children slide to new places with this spring.
    pub reflow: Option<Curve>,
}

/// `width` and `height` fix a side of `widget`: it neither shrinks below
/// nor stretches past it.
pub(crate) fn bind_size(
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

/// How a column or row aligns its children across its axis. A column
/// without `align` is a leading one: what fills across, such as a field, a
/// divider or a grid, fills it, and the rest keeps its own width.
fn stack_cross(align: Option<&str>, column: bool) -> gtk::Align {
    match (align, column) {
        (Some("center"), _) => gtk::Align::Center,
        (Some("trailing") | Some("bottom"), _) => gtk::Align::End,
        (Some("leading") | Some("top"), _) => gtk::Align::Start,
        // A row's items share the baseline of their first lines.
        (Some("baseline"), false) => gtk::Align::BaselineCenter,
        (Some("fill"), _) => gtk::Align::Fill,
        (None, true) => gtk::Align::Start,
        (None, false) => gtk::Align::Center,
        _ => gtk::Align::Fill,
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
    let cross = stack_cross(props.align, column);
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
    pad(ctx, &container, &props.padding, props.animate);
    // A declared side is the size the column or row asks for, as wide as
    // its content would like or not.
    let outer: gtk::Widget = if props.width.is_some() || props.height.is_some() {
        crate::layout::Sized::wrap(&container).upcast()
    } else {
        container
    };
    bind_size(ctx, &outer, &props.width, &props.height, props.animate);
    children(&inner, ctx);
    place(p, &outer);
}

#[derive(Default)]
pub struct GridProps {
    pub min_width: Option<Num>,
    pub gap: Option<Num>,
    /// The space between rows, where it differs from `gap`.
    pub row_gap: Option<Num>,
    pub padding: Padding,
    pub animate: Option<Curve>,
    /// Rows as tall as the column width over the ratio, plus the extra.
    pub cells: Option<(f64, Num)>,
    /// Cells slide to new places with this spring.
    pub reflow: Option<Curve>,
}

/// Equal columns, as many as fit at `min_width` or wider: a flow box whose
/// cells share each line. A bound `min_width` springs with `animate`, and
/// the cells move to their new places as the columns change. With
/// `cells`, every row is as tall as the columns' width gives, its items
/// from the top of their cells.
#[allow(non_snake_case)]
pub fn Grid(p: &Parent, ctx: &Ctx, props: GridProps, children: Build) {
    if let Some((ratio, extra)) = props.cells.clone() {
        crate::scene::cell_grid(p, ctx, props, ratio, extra, children);
        return;
    }
    let flow = gtk::FlowBox::new();
    flow.set_homogeneous(true);
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_activate_on_single_click(false);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(64);
    // The columns share the width it is given, as many as fit.
    flow.set_hexpand(true);
    flow.set_valign(gtk::Align::Start);
    flow.set_row_spacing(8);
    flow.set_column_spacing(8);
    if let Some(gap) = &props.gap {
        let spaced = flow.clone();
        let rows = props.row_gap.is_none();
        bind_number(ctx, gap, props.animate, &flow, move |value| {
            let gap = value.round().max(0.0) as u32;
            if rows {
                spaced.set_row_spacing(gap);
            }
            spaced.set_column_spacing(gap);
        });
    }
    if let Some(gap) = &props.row_gap {
        let spaced = flow.clone();
        bind_number(ctx, gap, props.animate, &flow, move |value| {
            spaced.set_row_spacing(value.round().max(0.0) as u32);
        });
    }
    pad(ctx, flow.upcast_ref(), &props.padding, props.animate);
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
    /// The surface a style gives the stack.
    pub style: Option<&'static crate::style::StyleDef>,
    /// While false, nothing in the stack takes input.
    pub enabled: Option<Get>,
    /// Layers slide to new places with this spring.
    pub reflow: Option<Curve>,
    /// `under_bars`: while shown, the stack fills the window up to its top
    /// edge, under the header bar.
    pub under_bars: bool,
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
    layers.set_reflow(props.reflow);
    layers.set_block(!p.is_horizontal() && props.width.is_none());
    bind_size(
        ctx,
        layers.upcast_ref(),
        &props.width,
        &props.height,
        props.animate,
    );
    if let Some(style) = props.style {
        crate::style::surface(style, &layers);
    }
    if let Some(enabled) = bound_enabled(ctx, &props.enabled) {
        // Disabled content keeps its look: it only stops taking input and
        // leaves the accessibility tree.
        let shown = layers.clone();
        effect(move || {
            let on = enabled();
            shown.set_can_target(on);
            shown.set_can_focus(on);
            shown.update_state(&[gtk::accessible::State::Hidden(!on)]);
        });
    }
    if props.under_bars {
        under_bars(layers.upcast_ref());
    } else if p.is_window() && props.width.is_none() && props.height.is_none() {
        // Written directly in a window, a stack without a size fills it and
        // places its smaller layers there by its alignment.
        layers.set_hexpand(true);
        layers.set_vexpand(true);
    }
    children(&Parent::of_layers(&layers, halign, valign), ctx);
    place(p, &layers);
}

/// While `widget` is shown it fills the window, and the window's toolbar
/// view lays the content up to its top edge, under the header bar.
fn under_bars(widget: &gtk::Widget) {
    widget.set_hexpand(true);
    widget.set_vexpand(true);
    let extend = |widget: &gtk::Widget, on: bool| {
        if let Some(view) = widget
            .ancestor(adw::ToolbarView::static_type())
            .and_then(|view| view.downcast::<adw::ToolbarView>().ok())
        {
            view.set_extend_content_to_top_edge(on);
        }
    };
    widget.connect_map(move |widget| extend(widget, true));
    widget.connect_unmap(move |widget| extend(widget, false));
}

#[derive(Default)]
pub struct ImageProps {
    /// Name of the declared resource.
    pub source: &'static str,
    /// The declared resource shown in place of `source` where the picture
    /// is dark: in a dark part, or a dark window.
    pub dark: Option<&'static str>,
    /// Its sides never change: it is double-sampled.
    pub one_size: bool,
    pub label: Option<Str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub fit: Option<&'static str>,
    pub animate: Option<Curve>,
    /// Rounded corners of this radius.
    pub corner: Option<Num>,
    /// Drawn in the foreground color, as a symbol: only the picture's
    /// shape shows.
    pub template: bool,
    /// Width over height, in place of the picture's own.
    pub aspect: Option<f64>,
}

/// The content fit of `fit`.
pub(crate) fn content_fit(fit: Option<&str>) -> gtk::ContentFit {
    match fit {
        Some("cover") => gtk::ContentFit::Cover,
        Some("fill") => gtk::ContentFit::Fill,
        _ => gtk::ContentFit::Contain,
    }
}

/// How a picture is sized and shaped.
pub(crate) struct Sizing<'a> {
    pub(crate) width: &'a Option<Num>,
    pub(crate) height: &'a Option<Num>,
    pub(crate) animate: Option<Curve>,
    pub(crate) corner: &'a Option<Num>,
    pub(crate) aspect: Option<f64>,
}

/// A picture of a declared size and shape, holding `picture`: with one
/// side given, the other follows `ratio`; with only `aspect`, as wide as
/// its container and as tall as the aspect makes it. Returns the widget
/// to place and what lays it out again when the ratio changes.
pub(crate) fn framed(
    ctx: &Ctx,
    picture: &gtk::Widget,
    sizing: Sizing<'_>,
    ratio: Rc<Cell<f64>>,
) -> (gtk::Widget, crate::picture::Frame, Rc<dyn Fn()>) {
    let Sizing {
        width,
        height,
        animate,
        corner,
        aspect,
    } = sizing;
    let frame = crate::picture::Frame::new(picture, 0, 0);
    let sides: Rc<Cell<(Option<f64>, Option<f64>)>> = Rc::new(Cell::new((None, None)));
    let resize: Rc<dyn Fn()> = {
        let frame = frame.clone();
        let sides = Rc::clone(&sides);
        let ratio = Rc::clone(&ratio);
        Rc::new(move || {
            let ratio = ratio.get();
            let (width, height) = sides.get();
            let w = width.or(height.map(|height| height * ratio)).unwrap_or(0.0);
            let h = height.or(width.map(|width| width / ratio)).unwrap_or(0.0);
            frame.set_size(w.round() as i32, h.round() as i32);
        })
    };
    match (width, height, aspect) {
        (None, None, Some(aspect)) => frame.fill_width(aspect),
        _ => {
            if let Some(width) = width {
                let sides = Rc::clone(&sides);
                let resize = Rc::clone(&resize);
                bind_number(ctx, width, animate, &frame, move |value| {
                    sides.set((Some(value), sides.get().1));
                    resize();
                });
            }
            if let Some(height) = height {
                let sides = Rc::clone(&sides);
                let resize = Rc::clone(&resize);
                bind_number(ctx, height, animate, &frame, move |value| {
                    sides.set((sides.get().0, Some(value)));
                    resize();
                });
            }
        }
    }
    let outer: gtk::Widget = match corner {
        Some(corner) => crate::features::rounded(ctx, &frame, corner, animate).upcast(),
        None => frame.clone().upcast(),
    };
    (outer, frame, resize)
}

/// A picture from a declared resource. With one side given, the other
/// follows the picture's aspect ratio. Without a label it is decorative.
#[allow(non_snake_case)]
pub fn Image(p: &Parent, ctx: &Ctx, props: ImageProps) {
    let resources = std::path::PathBuf::from(crate::host::resources_dir());
    let light = resources.join(props.source);
    let dark = props.dark.map(|dark| resources.join(dark));
    // The window's appearance until the picture is placed in its part.
    let path = match &dark {
        Some(dark) if crate::theme::is_dark() => dark.clone(),
        _ => light.clone(),
    };
    let texture = read_picture(&path, None);
    let (natural_width, natural_height) = texture
        .as_ref()
        .map(|paintable| (paintable.intrinsic_width(), paintable.intrinsic_height()))
        .unwrap_or((0, 0));
    let ratio = props
        .aspect
        .unwrap_or(if natural_width > 0 && natural_height > 0 {
            f64::from(natural_width) / f64::from(natural_height)
        } else {
            1.0
        });
    let fit = content_fit(props.fit);
    let picture: gtk::Widget = if props.template {
        let template = crate::picture::Template::new(fit);
        template.set_paintable(texture.as_ref());
        template.upcast()
    } else {
        let picture = match &texture {
            Some(texture) => gtk::Picture::for_paintable(texture),
            None => gtk::Picture::for_filename(&path),
        };
        picture.set_can_shrink(true);
        picture.set_content_fit(fit);
        picture.upcast()
    };
    match &props.label {
        Some(label) => {
            let text = bound_text(ctx, label);
            let described = picture.clone();
            effect(move || match described.downcast_ref::<gtk::Picture>() {
                Some(picture) => picture.set_alternative_text(Some(&text())),
                None => described.update_property(&[gtk::accessible::Property::Label(&text())]),
            });
        }
        None => picture.update_state(&[gtk::accessible::State::Hidden(true)]),
    }
    if dark.is_some() || props.one_size {
        let sides = props.one_size.then(|| {
            let side = |num: &Option<Num>| num.as_ref().map(|num| num.peek(ctx));
            (side(&props.width), side(&props.height))
        });
        follow_picture(&picture, path, light, dark, sides, fit);
    }
    let (outer, _, _) = framed(
        ctx,
        &picture,
        Sizing {
            width: &props.width,
            height: &props.height,
            animate: props.animate,
            corner: &props.corner,
            aspect: props.aspect,
        },
        Rc::new(Cell::new(ratio)),
    );
    place(p, &outer);
}

/// The declared picture at `path`: in HDR where GTK presents it, upright,
/// and with `pixels`, its longer side that many pixels, drawn down by
/// gdk-pixbuf, which averages the pixels each one covers.
fn read_picture(path: &std::path::Path, pixels: Option<i32>) -> Option<gtk::gdk::Paintable> {
    if let Some(texture) = crate::hdr::texture(path) {
        return Some(texture.upcast());
    }
    if let Some(pixels) = pixels
        && let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, pixels, pixels, true)
    {
        let upright = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
        return crate::features::premultiplied_texture(&upright).map(|texture| texture.upcast());
    }
    oriented(path).map(|texture| texture.upcast()).or_else(|| {
        gtk::gdk::Texture::from_filename(path)
            .ok()
            .map(|texture| texture.upcast())
    })
}

/// Keep in `picture` (a `gtk::Picture` or a template) the declared picture
/// for where it is, starting from `shown`: `dark` in a dark part or window,
/// else `light`; and with `sides` that never change (points, `None` for a
/// side that follows the picture) double-sampled once it is placed:
/// decoded at twice the pixels it covers, which GTK halves when it draws
/// it. Squeezed down from a much larger picture by GTK alone, its edges
/// would break up.
fn follow_picture(
    picture: &gtk::Widget,
    shown: std::path::PathBuf,
    light: std::path::PathBuf,
    dark: Option<std::path::PathBuf>,
    sides: Option<(Option<f64>, Option<f64>)>,
    fit: gtk::ContentFit,
) {
    // Placed in a window, or on a display of another scale.
    let placed = crate::reactive::Version::new();
    let bump = placed.clone();
    picture.connect_map(move |_| bump.bump());
    let bump = placed.clone();
    picture.connect_scale_factor_notify(move |_| bump.bump());
    let current = Rc::new(std::cell::RefCell::new((shown, None::<i32>)));
    let weak = picture.downgrade();
    effect(move || {
        crate::theme::track();
        placed.track();
        let Some(widget) = weak.upgrade() else {
            return;
        };
        let path = match &dark {
            Some(dark) if crate::theme::part_of(&widget).unwrap_or_else(crate::theme::is_dark) => {
                dark
            }
            _ => &light,
        };
        let scale = widget
            .native()
            .and_then(|native| native.surface())
            .map(|surface| surface.scale())
            .unwrap_or_else(|| f64::from(widget.scale_factor()));
        let pixels = sides
            .filter(|_| widget.is_mapped())
            .and_then(|(width, height)| twice_pixels(path, width, height, fit, scale));
        let next = (path.clone(), pixels);
        if *current.borrow() == next {
            return;
        }
        let texture = read_picture(path, pixels);
        current.replace(next);
        if let Some(template) = widget.downcast_ref::<crate::picture::Template>() {
            template.set_paintable(texture.as_ref());
        } else if let Some(picture) = widget.downcast_ref::<gtk::Picture>() {
            picture.set_paintable(texture.as_ref());
        }
    });
}

/// The longer side, in pixels, of the picture at `path` decoded at twice
/// the pixels it covers shown `width` × `height` points (`None` for a side
/// that follows the picture) as `fit` places it, at `scale` pixels per
/// point; `None` when it has no more pixels than that.
fn twice_pixels(
    path: &std::path::Path,
    width: Option<f64>,
    height: Option<f64>,
    fit: gtk::ContentFit,
    scale: f64,
) -> Option<i32> {
    let (_, w, h) = gtk::gdk_pixbuf::Pixbuf::file_info(path)?;
    let (w, h) = (f64::from(w), f64::from(h));
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (shown_width, shown_height) = match (width, height) {
        (Some(width), Some(height)) => match fit {
            gtk::ContentFit::Fill => (width, height),
            _ => {
                let (fx, fy) = (width / w, height / h);
                let s = if fit == gtk::ContentFit::Cover {
                    fx.max(fy)
                } else {
                    fx.min(fy)
                };
                (w * s, h * s)
            }
        },
        (Some(width), None) => (width, width * h / w),
        (None, Some(height)) => (height * w / h, height),
        (None, None) => return None,
    };
    let factor = (2.0 * shown_width * scale / w).max(2.0 * shown_height * scale / h);
    (factor < 1.0).then(|| (w.max(h) * factor).ceil().max(1.0) as i32)
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
    crate::features::premultiplied_texture(&pixbuf.apply_embedded_orientation().unwrap_or(pixbuf))
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
    spacer.add_css_class(crate::scene::SPACER);
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
    let line = gtk::Separator::new(orientation);
    // A line across what holds it, in a column or a stack alike.
    if !p.is_horizontal() {
        line.set_hexpand(true);
    }
    place(p, &line);
}

#[derive(Default)]
pub struct FormProps {
    pub title: Option<Str>,
    /// The space between sections.
    pub gap: Option<f64>,
    /// The least height of a row.
    pub row_height: Option<f64>,
    /// The padding of every row across and down, unless its section says.
    pub padding_x: Option<f64>,
    pub padding_y: Option<f64>,
}

/// The layout a form gives the rows of its sections.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowLayout {
    pub row_height: Option<f64>,
    pub padding_x: Option<f64>,
    pub padding_y: Option<f64>,
}

impl RowLayout {
    /// The rules that lay out the rows of a list carrying their class: a
    /// plain row's padding, a libadwaita row's header, the least height.
    fn class(&self) -> Option<String> {
        if *self == RowLayout::default() {
            return None;
        }
        let mut rules = String::new();
        let padding = match (self.padding_y, self.padding_x) {
            (None, None) => None,
            (y, x) => Some(format!(
                "padding: {}px {}px;",
                y.unwrap_or(10.0),
                x.unwrap_or(12.0)
            )),
        };
        if let Some(padding) = padding {
            rules.push_str(&format!(
                "& > row.bern-plain-row {{ {padding} }} & > row > box.header {{ {padding} }} "
            ));
        }
        if let Some(height) = self.row_height {
            rules.push_str(&format!(
                "& > row {{ min-height: {height}px; }} & > row > box.header {{ min-height: {height}px; }} "
            ));
        }
        Some(crate::style::class_with(&rules))
    }
}

/// A grouped form: sections of boxed lists in a clamped, scrolling page,
/// as libadwaita preference pages lay them out. `gap` spaces the sections;
/// a form that opens with a plain section starts at the top of the page.
#[allow(non_snake_case)]
pub fn Form(p: &Parent, ctx: &Ctx, props: FormProps, children: Build) {
    if let (Some(title), Some(page)) = (&props.title, ctx.page()) {
        page.push_title(bound_text(ctx, title));
    }
    let content = gtk::Box::new(
        gtk::Orientation::Vertical,
        props
            .gap
            .map(|gap| gap.round().max(0.0) as i32)
            .unwrap_or(24),
    );
    // The room above the first section, unless that is a plain one: a
    // rule on the first child, so it follows a heading a condition gives
    // or takes away later.
    content.add_css_class("bern-form-rows");
    content.set_margin_bottom(24);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(680);
    clamp.set_tightening_threshold(480);
    clamp.set_child(Some(&content));
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    // A form takes the room it is given both ways, as on the other
    // targets; the clamp keeps its rows readable.
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_propagate_natural_height(true);
    scrolled.set_child(Some(&clamp));
    crate::features::natural_scrolling(&scrolled, false);
    let layout = RowLayout {
        row_height: props.row_height,
        padding_x: props.padding_x,
        padding_y: props.padding_y,
    };
    children(&Parent::of_box(&content), &ctx.with_form(Rc::new(layout)));
    p.add(&scrolled);
}

#[derive(Default)]
pub struct SectionProps {
    pub title: Option<Str>,
    pub footer: Option<Str>,
    /// Rows without a background, padding or separators, as a heading
    /// that scrolls with the form.
    pub plain: bool,
    /// The padding of the section's rows, in place of the form's.
    pub padding_x: Option<f64>,
    pub padding_y: Option<f64>,
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
    // A plain section's rows keep the page's layout, whatever the form
    // gives its groups: only its own insets apply.
    let form = ctx
        .form()
        .filter(|_| !props.plain)
        .map(|form| **form)
        .unwrap_or_default();
    let layout = RowLayout {
        row_height: form.row_height,
        padding_x: props.padding_x.or(form.padding_x),
        padding_y: props.padding_y.or(form.padding_y),
    };
    if props.plain {
        list.remove_css_class("boxed-list");
        list.add_css_class("bern-plain-list");
        group.add_css_class("bern-plain-section");
    }
    if let Some(class) = layout.class() {
        list.add_css_class(&class);
    }
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
pub fn Toolbar(p: &Parent, ctx: &Ctx, children: Build) {
    tools(p, ctx, children, false);
}

/// Toolbar items at the leading end of the page's header, after the way
/// back: those before a toolbar's `Spacer`.
#[allow(non_snake_case)]
pub fn LeadingToolbar(p: &Parent, ctx: &Ctx, children: Build) {
    tools(p, ctx, children, true);
}

fn tools(_p: &Parent, ctx: &Ctx, children: Build, leading: bool) {
    let Some(page) = ctx.page().cloned() else {
        return;
    };
    let holder = page.add_tools(leading);
    holder.set_valign(gtk::Align::Center);
    let shown = holder.clone();
    let at = ctx.clone();
    effect(move || shown.set_visible(at.shown()));
    children(
        &Parent::of_aligned_box(&holder, gtk::Align::Center),
        &ctx.toolbar(),
    );
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
    /// `paste:`: the clipboard's text goes into this draft before the
    /// action runs.
    pub paste: Option<crate::ctx::Field>,
}

#[derive(Default)]
pub struct MenuProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    /// `style: icon`: a round button showing only the icon, the label as
    /// its tooltip and accessible name.
    pub icon_only: bool,
    pub hide_disabled: bool,
    pub shape: Shape,
}

/// A menu button. Each item is registered on the button, so external
/// entries reach the items while the menu is closed.
#[allow(non_snake_case)]
pub fn Menu(p: &Parent, ctx: &Ctx, props: MenuProps, items: Vec<Option<MenuItem>>) {
    let button = gtk::MenuButton::new();
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if props.icon_only {
        button.set_icon_name(icons::name(props.icon.unwrap_or("ellipsis")));
        button.add_css_class("flat");
        button.add_css_class("circular");
        if let Some(label) = label.clone() {
            let named = button.clone();
            effect(move || {
                let text = label();
                named.set_tooltip_text(Some(&text));
                named.update_property(&[gtk::accessible::Property::Label(&text)]);
            });
        }
    } else {
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
    }
    shape(ctx, button.upcast_ref(), &props.shape);
    button.set_valign(gtk::Align::Center);
    let group = gio::SimpleActionGroup::new();
    let mut entries = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Some(item) = item else {
            entries.push(None);
            continue;
        };
        let name = format!("i{index}");
        let action = gio::SimpleAction::new(&name, None);
        let (run, set_file) =
            crate::features::picking(ctx, item.pick, bound_action(ctx, &item.action));
        let run = crate::features::pasting(ctx, item.paste, run);
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
            enabled.clone(),
            Actions {
                activate: run,
                set_file,
                file_kinds: item.pick.map(|pick| pick.kinds),
                ..Actions::default()
            },
        );
        let _ = (item.icon, item.role);
        entries.push(Some((format!("menu.{name}"), item_label, enabled)));
    }
    button.insert_action_group("menu", Some(&group));
    let shown = button.clone();
    effect(move || {
        let menu = gio::Menu::new();
        let mut section = gio::Menu::new();
        for entry in &entries {
            match entry {
                Some((action, label, enabled)) => {
                    if !props.hide_disabled || enabled.as_ref().is_none_or(|enabled| enabled()) {
                        section.append(Some(&label()), Some(action));
                    }
                }
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
    keeps_width(&button);
    place(p, &button);
}

/// Text of a value for `Text(content: …)` with runtime data: integers,
/// floats and texts as Bern formats them, enums by their label.
pub fn display(value: &Value, enum_name: Option<&str>) -> String {
    match (value, enum_name) {
        (Value::Enum { variant, fields }, Some(name)) => label_of(name, *variant, fields),
        (value, _) => crate::value::plain(value),
    }
}

/// The localized label of a variant without fields.
pub fn label(enum_name: &str, variant: u32) -> String {
    label_of(enum_name, variant, &[])
}

/// The localized label of an enum value. A label that shows the
/// variant's fields writes them as its text says.
pub fn label_of(enum_name: &str, variant: u32, fields: &[Value]) -> String {
    let host = crate::host::host();
    let Some(def) = host.def.types.enum_def(enum_name) else {
        return String::new();
    };
    let Some(variant) = def.variants.get(variant as usize) else {
        return String::new();
    };
    let key = crate::protocol::label_key(enum_name, &variant.name);
    let Some(label) = variant
        .label
        .as_ref()
        .filter(|label| !label.params.is_empty())
    else {
        return tr(&key);
    };
    let args: Vec<(&str, crate::protocol::format::Arg)> = label
        .params
        .iter()
        .filter_map(|param| {
            let at = variant
                .fields
                .iter()
                .position(|field| field.name == *param)?;
            let value = fields.get(at)?;
            let arg = match &variant.fields[at].ty {
                crate::protocol::Type::Record { name }
                    if name == crate::protocol::format::CALENDAR_DATE =>
                {
                    text::date(value)
                }
                crate::protocol::Type::Enum { name } => text::arg(value, Some(name)),
                _ => text::arg(value, None),
            };
            Some((param.as_str(), arg))
        })
        .collect();
    text::fmt(&key, &args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::tests::{app, start, unit_enum};

    #[test]
    fn a_column_without_align_is_a_leading_one() {
        assert_eq!(stack_cross(None, true), gtk::Align::Start);
        assert_eq!(stack_cross(Some("leading"), true), stack_cross(None, true));
        assert_eq!(stack_cross(Some("center"), true), gtk::Align::Center);
        assert_eq!(stack_cross(Some("trailing"), true), gtk::Align::End);
        // A row without `align` centers its items vertically.
        assert_eq!(stack_cross(None, false), gtk::Align::Center);
        assert_eq!(
            stack_cross(Some("baseline"), false),
            gtk::Align::BaselineCenter
        );
    }

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
            (Some("code"), "monospace"),
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
