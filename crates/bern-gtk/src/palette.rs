//! `Search(style: palette)`: a button that opens, above the window, a
//! search field and its results. The results are built while the palette
//! is open; the arrow keys move among the items that take part in
//! navigation, Return chooses one, Escape or a click outside closes it.

use crate::controls::{self, Actions};
use crate::protocol::Value;
use crate::reactive::{Scope, effect, on_cleanup, untrack};
use crate::scene::ModalFrame;
use crate::value::as_text;
use crate::widgets::{Bound, SearchProps, bound_action, bound_text, editable, place};
use crate::{Act, Build, Ctx, Get, Parent, Str, icons};
use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// An item of a palette, and whether it takes part in keyboard
/// navigation now.
type Item = (gtk::Button, Rc<dyn Fn() -> bool>);

/// The palette while it is open, and what its results live in.
type OpenPalette = Rc<RefCell<Option<(Rc<Palette>, Scope)>>>;

/// An open palette: how it closes, and the items in it.
pub(crate) struct Palette {
    close: Rc<dyn Fn()>,
    items: RefCell<Vec<Item>>,
    active: RefCell<Option<gtk::Button>>,
    /// The column the items are listed in, for their order on screen.
    list: gtk::Box,
    refreshing: Cell<bool>,
}

impl Palette {
    /// The items the keyboard moves among, top to bottom.
    fn navigable(&self) -> Vec<gtk::Button> {
        let mut items: Vec<(f32, gtk::Button)> = self
            .items
            .borrow()
            .iter()
            .filter(|(button, navigable)| navigable() && button.is_drawable())
            .filter_map(|(button, _)| {
                let point = button.compute_point(&self.list, &gtk::graphene::Point::zero())?;
                Some((point.y(), button.clone()))
            })
            .collect();
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        items.into_iter().map(|(_, button)| button).collect()
    }

    fn select(&self, button: Option<gtk::Button>) {
        if let Some(previous) = self.active.borrow_mut().take() {
            previous.remove_css_class("bern-active");
        }
        if let Some(button) = &button {
            button.add_css_class("bern-active");
            // Keep the chosen item in view.
            if let Some(scrolled) = self
                .list
                .ancestor(gtk::ScrolledWindow::static_type())
                .and_downcast::<gtk::ScrolledWindow>()
                && let Some(bounds) = button.compute_bounds(&self.list)
            {
                let adjustment = scrolled.vadjustment();
                let (top, bottom) = (
                    f64::from(bounds.y()),
                    f64::from(bounds.y() + bounds.height()),
                );
                if top < adjustment.value() {
                    adjustment.set_value(top);
                } else if bottom > adjustment.value() + adjustment.page_size() {
                    adjustment.set_value(bottom - adjustment.page_size());
                }
            }
        }
        *self.active.borrow_mut() = button;
    }

    /// The first item is chosen when the chosen one has gone.
    fn refresh(self: &Rc<Self>) {
        if self.refreshing.replace(true) {
            return;
        }
        let palette = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(palette) = palette.upgrade() else {
                return;
            };
            palette.refreshing.set(false);
            let available = palette.navigable();
            let current = palette.active.borrow().clone();
            if !current.is_some_and(|current| available.contains(&current)) {
                palette.select(available.first().cloned());
            }
        });
    }

    fn step(&self, by: i32) {
        let available = self.navigable();
        if available.is_empty() {
            return;
        }
        let at = self
            .active
            .borrow()
            .as_ref()
            .and_then(|active| available.iter().position(|item| item == active));
        let next = match at {
            Some(at) => (at as i32 + by).clamp(0, available.len() as i32 - 1) as usize,
            None => 0,
        };
        self.select(Some(available[next].clone()));
    }

    /// Choose the chosen item; false when there is none.
    fn choose(&self) -> bool {
        let active = self.active.borrow().clone();
        match active {
            Some(active) => {
                active.emit_clicked();
                true
            }
            None => false,
        }
    }
}

/// The trigger of a search palette and, while open, the palette.
#[allow(non_snake_case)]
pub fn SearchPalette(
    p: &Parent,
    ctx: &Ctx,
    props: SearchProps,
    submit: Option<Act>,
    results: Option<Build>,
) {
    let Some(field) = props.field else { return };
    let bound = Bound::new(ctx, field);
    let prompt = props.prompt.as_ref().map(|prompt| bound_text(ctx, prompt));
    let panel_prompt = props
        .panel_prompt
        .as_ref()
        .map(|prompt| bound_text(ctx, prompt))
        .or_else(|| prompt.clone());
    let trigger = gtk::Button::new();
    trigger.add_css_class("bern-palette-trigger");
    if ctx.in_toolbar() {
        trigger.add_css_class("flat");
    }
    trigger.set_valign(gtk::Align::Center);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&icons::image("magnifyingglass"));
    let caption = gtk::Label::new(None);
    caption.add_css_class("dim-label");
    content.append(&caption);
    trigger.set_child(Some(&content));
    if let Some(prompt) = prompt.clone() {
        let (shown, named) = (caption.clone(), trigger.clone());
        effect(move || {
            let text = prompt();
            shown.set_label(&text);
            named.update_property(&[gtk::accessible::Property::Label(&text)]);
        });
    }
    let submit = bound_action(ctx, &submit);
    let open_palette: OpenPalette = Rc::new(RefCell::new(None));
    let size = (props.width.unwrap_or(640.0), props.height.unwrap_or(480.0));
    let open: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let bound = bound.clone();
        let held = Rc::clone(&open_palette);
        let trigger = trigger.clone();
        let submit = submit.clone();
        Rc::new(move || {
            if held.borrow().is_some() {
                return;
            }
            // What the palette builds lives until it closes.
            let owner = Scope::detached();
            owner.run(|| {
                untrack(|| {
                    bound.set(Value::Text(String::new()));
                    let frame = ModalFrame::above(ctx.win(), size.0, Some(size.1));
                    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    card.add_css_class("bern-modal");
                    card.set_overflow(gtk::Overflow::Hidden);
                    card.set_parent(&frame);
                    // The field, and a way to close.
                    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    header.set_margin_top(12);
                    header.set_margin_bottom(12);
                    header.set_margin_start(16);
                    header.set_margin_end(12);
                    header.append(&icons::image("magnifyingglass"));
                    let entry = gtk::SearchEntry::new();
                    entry.set_hexpand(true);
                    if let Some(prompt) = panel_prompt.clone() {
                        let shown = entry.clone();
                        effect(move || {
                            let text = prompt();
                            shown.set_placeholder_text(Some(&text));
                            shown.update_property(&[gtk::accessible::Property::Label(&text)]);
                        });
                    }
                    header.append(&entry);
                    let close_button = gtk::Button::from_icon_name(icons::name("xmark"));
                    close_button.add_css_class("flat");
                    close_button.add_css_class("circular");
                    close_button.set_tooltip_text(Some(&crate::text::tr("bern.window.close")));
                    header.append(&close_button);
                    card.append(&header);
                    card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
                    list.set_margin_top(8);
                    list.set_margin_bottom(8);
                    list.set_margin_start(8);
                    list.set_margin_end(8);
                    let scrolled = gtk::ScrolledWindow::new();
                    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                    scrolled.set_vexpand(true);
                    scrolled.set_child(Some(&list));
                    card.append(&scrolled);

                    let close: Rc<dyn Fn()> = {
                        let held = Rc::clone(&held);
                        let frame = frame.clone();
                        let trigger = trigger.clone();
                        Rc::new(move || {
                            let open = held.borrow_mut().take();
                            if let Some((_, owner)) = open {
                                owner.dispose();
                                frame.remove();
                                trigger.grab_focus();
                            }
                        })
                    };
                    let palette = Rc::new(Palette {
                        close: Rc::clone(&close),
                        items: RefCell::new(Vec::new()),
                        active: RefCell::new(None),
                        list: list.clone(),
                        refreshing: Cell::new(false),
                    });
                    {
                        let read = entry.clone();
                        let write = entry.clone();
                        let updating = editable(
                            Rc::new(move || read.text().to_string()),
                            Rc::new(move |value| write.set_text(value)),
                            bound.clone(),
                        );
                        let changed = bound.clone();
                        let moved = Rc::clone(&palette);
                        entry.connect_changed(move |entry| {
                            if !updating.get() {
                                changed.set(Value::Text(entry.text().to_string()));
                            }
                            moved.refresh();
                        });
                        if let Some(results) = &results {
                            results(
                                &Parent::of_box(&list),
                                &ctx.outside_toolbar().with_palette(Rc::clone(&palette)),
                            );
                        }
                    }
                    // The keys of the field move among the items.
                    let keys = gtk::EventControllerKey::new();
                    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
                    let moving = Rc::clone(&palette);
                    keys.connect_key_pressed(move |_, key, _, _| match key {
                        gtk::gdk::Key::Down => {
                            moving.step(1);
                            glib::Propagation::Stop
                        }
                        gtk::gdk::Key::Up => {
                            moving.step(-1);
                            glib::Propagation::Stop
                        }
                        gtk::gdk::Key::Escape => {
                            (moving.close)();
                            glib::Propagation::Stop
                        }
                        _ => glib::Propagation::Proceed,
                    });
                    entry.add_controller(keys);
                    let choosing = Rc::clone(&palette);
                    let submitted = submit.clone();
                    entry.connect_activate(move |_| {
                        if !choosing.choose()
                            && let Some(submit) = &submitted
                        {
                            submit();
                        }
                    });
                    let closing = Rc::clone(&close);
                    close_button.connect_clicked(move |_| closing());
                    // A click outside the card closes it.
                    let outside = gtk::GestureClick::new();
                    let closing = Rc::clone(&close);
                    let around = frame.clone();
                    outside.connect_released(move |_, _, x, y| {
                        if around.outside(x, y) {
                            closing();
                        }
                    });
                    frame.add_controller(outside);
                    *held.borrow_mut() = Some((Rc::clone(&palette), owner.clone()));
                    palette.refresh();
                    let focused = entry.clone();
                    glib::idle_add_local_once(move || {
                        focused.grab_focus();
                    });
                })
            });
        })
    };
    let pressed = Rc::clone(&open);
    trigger.connect_clicked(move |_| pressed());
    // The palette closes when its trigger leaves.
    let leaving = Rc::clone(&open_palette);
    on_cleanup(move || {
        let close = leaving
            .borrow()
            .as_ref()
            .map(|(palette, _)| Rc::clone(&palette.close));
        if let Some(close) = close {
            close();
        }
    });
    place(p, &trigger);
    let set = bound.clone();
    let value = bound.clone();
    let chosen = Rc::clone(&open_palette);
    controls::register(
        ctx,
        props.id,
        "search",
        prompt.or_else(|| Some(Rc::new(String::new))),
        None,
        Actions {
            activate: Some(Rc::clone(&open)),
            edit: Some(Rc::new(move |text| set.set(Value::Text(text)))),
            submit: Some(Rc::new(move || {
                let palette = chosen
                    .borrow()
                    .as_ref()
                    .map(|(palette, _)| Rc::clone(palette));
                let chose = palette.is_some_and(|palette| palette.choose());
                if !chose && let Some(submit) = &submit {
                    submit();
                }
            })),
            value: Some(Rc::new(move || Some(as_text(&value.peek())))),
            focus: Some(open),
            ..Actions::default()
        },
    );
}

pub struct SearchItemProps {
    pub id: &'static str,
    pub label: Option<Str>,
    pub icon: Option<&'static str>,
    pub detail: Option<Str>,
    pub hint: Option<Str>,
    /// Whether the keyboard moves to it; true when not given.
    pub navigation: Option<Get>,
    pub pick: Option<crate::features::Pick>,
}

/// A result of a search: its icon, title, detail and hint, or its own
/// content. Choosing it closes the palette it is in, then acts.
#[allow(non_snake_case)]
pub fn SearchItem(
    p: &Parent,
    ctx: &Ctx,
    props: SearchItemProps,
    action: Option<Act>,
    content: Option<Build>,
) {
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("bern-search-item");
    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if let Some(label) = label.clone() {
        let named = button.clone();
        effect(move || named.update_property(&[gtk::accessible::Property::Label(&label())]));
    }
    match content {
        Some(content) => {
            let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
            content(&Parent::of_box(&holder), &ctx.outside_toolbar());
            button.set_child(Some(&holder));
        }
        None => {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            if let Some(icon) = props.icon {
                row.append(&icons::image(icon));
            }
            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_hexpand(true);
            let title = gtk::Label::new(None);
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if let Some(label) = label.clone() {
                crate::widgets::bind_label(&title, label);
            }
            text.append(&title);
            if let Some(detail) = &props.detail {
                let caption = gtk::Label::new(None);
                caption.set_xalign(0.0);
                caption.add_css_class("caption");
                caption.add_css_class("dim-label");
                caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
                crate::widgets::bind_label(&caption, bound_text(ctx, detail));
                text.append(&caption);
            }
            row.append(&text);
            if let Some(hint) = &props.hint {
                let shortcut = gtk::Label::new(None);
                shortcut.add_css_class("dim-label");
                shortcut.set_valign(gtk::Align::Center);
                crate::widgets::bind_label(&shortcut, bound_text(ctx, hint));
                row.append(&shortcut);
            }
            button.set_child(Some(&row));
        }
    }
    let navigable: Rc<dyn Fn() -> bool> = match props.navigation {
        Some(navigation) => {
            let at = ctx.clone();
            Rc::new(move || untrack(|| crate::value::truthy(&navigation(&at))))
        }
        None => Rc::new(|| true),
    };
    {
        let shown = button.clone();
        let navigable = Rc::clone(&navigable);
        effect(move || {
            if navigable() {
                shown.remove_css_class("bern-search-item-card");
            } else {
                shown.add_css_class("bern-search-item-card");
            }
        });
    }
    let palette = ctx.palette().cloned();
    let closing = palette.clone();
    let closes = move || {
        if let Some(palette) = &closing {
            (palette.close)();
        }
    };
    let (run, set_file) = crate::features::picking(ctx, props.pick, bound_action(ctx, &action));
    let activate: Rc<dyn Fn()> = {
        let closes = closes.clone();
        Rc::new(move || {
            closes();
            if let Some(run) = &run {
                run();
            }
        })
    };
    let clicked = Rc::clone(&activate);
    button.connect_clicked(move |_| clicked());
    if let Some(palette) = &palette {
        palette
            .items
            .borrow_mut()
            .push((button.clone(), Rc::clone(&navigable)));
        palette.refresh();
        // The pointer chooses what it rests on.
        let hover = gtk::EventControllerMotion::new();
        let (chosen, item) = (Rc::downgrade(palette), button.clone());
        hover.connect_enter(move |_, _, _| {
            if let Some(palette) = chosen.upgrade()
                && navigable()
            {
                palette.select(Some(item.clone()));
            }
        });
        button.add_controller(hover);
        let (registry, gone) = (Rc::downgrade(palette), button.clone());
        on_cleanup(move || {
            if let Some(palette) = registry.upgrade() {
                palette.items.borrow_mut().retain(|(item, _)| item != &gone);
                palette.refresh();
            }
        });
    }
    place(p, &button);
    controls::register(
        ctx,
        props.id,
        "button",
        label,
        None,
        Actions {
            activate: Some(activate),
            set_file: set_file.map(|give| {
                Rc::new(move |url: String| {
                    closes();
                    give(url);
                }) as Rc<dyn Fn(String)>
            }),
            file_kinds: props.pick.map(|pick| pick.kinds),
            ..Actions::default()
        },
    );
}
