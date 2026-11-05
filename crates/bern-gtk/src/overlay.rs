//! `bern::overlay` on Linux: an undecorated, transparent window per overlay
//! with a label per line, made on the GTK thread. While the pointer is over
//! it the bar of buttons shows and, unlocked, a plate; unlocked, dragging
//! it but by a button moves it. Locked, its input region holds only the
//! buttons that unlock it, so the pointer goes through to what lies under
//! the rest. GTK 4 asks the compositor for no place and no staying on top,
//! so where it stands and whether it stays above other windows is the
//! desktop's choice.

use bern_ffi::overlay::{Host, Line, Overlay, Place};
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

pub(crate) struct Overlays;

impl Host for Overlays {
    fn show(&self, id: &str, overlay: &Overlay) {
        let (id, overlay) = (id.to_string(), overlay.clone());
        glib::MainContext::default().invoke(move || show(&id, &overlay));
    }

    fn hide(&self, id: &str) {
        let id = id.to_string();
        glib::MainContext::default().invoke(move || {
            WINDOWS.with(|windows| {
                if let Some(parts) = windows.borrow().get(&id) {
                    parts.window.set_visible(false);
                }
            });
        });
    }
}

/// An overlay's window, its bar and its lines.
#[derive(Clone)]
struct Parts {
    window: gtk::Window,
    bar: gtk::Box,
    lines: gtk::Box,
}

thread_local! {
    static WINDOWS: RefCell<HashMap<String, Parts>> = RefCell::new(HashMap::new());
}

const CSS: &str = "window.bern-overlay { background: transparent; }
window.bern-overlay box.bern-overlay-plate { padding: 12px; border-radius: 12px; }
window.bern-overlay.unlocked.over box.bern-overlay-plate { background: rgba(0, 0, 0, 0.3); }
window.bern-overlay label { text-shadow: 0 1px 3px rgba(0, 0, 0, 0.85); }
window.bern-overlay box.bern-overlay-bar { opacity: 0; }
window.bern-overlay.over box.bern-overlay-bar { opacity: 1; }
window.bern-overlay button.bern-overlay-button { min-width: 28px; min-height: 28px; padding: 0; border-radius: 14px;
  background: rgba(0, 0, 0, 0.3); color: white; box-shadow: none; }
window.bern-overlay button.bern-overlay-button:hover { background: rgba(0, 0, 0, 0.55); }";

fn parts(id: &str) -> Parts {
    WINDOWS.with(|windows| {
        windows
            .borrow_mut()
            .entry(id.to_string())
            .or_insert_with(|| {
                if let Some(display) = gtk::gdk::Display::default() {
                    let provider = gtk::CssProvider::new();
                    provider.load_from_string(CSS);
                    gtk::style_context_add_provider_for_display(
                        &display,
                        &provider,
                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                    );
                }
                let window = gtk::Window::builder()
                    .decorated(false)
                    .resizable(false)
                    .focusable(false)
                    .title("")
                    .build();
                window.add_css_class("bern-overlay");
                let plate = gtk::Box::new(gtk::Orientation::Vertical, 8);
                plate.add_css_class("bern-overlay-plate");
                let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                bar.add_css_class("bern-overlay-bar");
                bar.set_halign(gtk::Align::Center);
                let lines = gtk::Box::new(gtk::Orientation::Vertical, 2);
                plate.append(&bar);
                plate.append(&lines);
                window.set_child(Some(&plate));
                // The bar and the plate show while the pointer is over it.
                let motion = gtk::EventControllerMotion::new();
                let entered = window.clone();
                motion.connect_enter(move |_, _, _| entered.add_css_class("over"));
                let left = window.clone();
                motion.connect_leave(move |_| left.remove_css_class("over"));
                window.add_controller(motion);
                // Unlocked, a press but on a button starts the compositor's
                // move.
                let press = gtk::GestureClick::new();
                let moved = window.clone();
                press.connect_pressed(move |gesture, _, x, y| {
                    if !moved.has_css_class("unlocked") {
                        return;
                    }
                    let picked = moved.pick(x, y, gtk::PickFlags::DEFAULT);
                    if picked.is_some_and(|widget| {
                        widget.is::<gtk::Button>()
                            || widget.ancestor(gtk::Button::static_type()).is_some()
                    }) {
                        return;
                    }
                    let Some(surface) = moved.surface() else {
                        return;
                    };
                    let Ok(toplevel) = surface.downcast::<gtk::gdk::Toplevel>() else {
                        return;
                    };
                    if let Some(device) = gesture.device() {
                        toplevel.begin_move(
                            &device,
                            gesture.current_button() as i32,
                            x,
                            y,
                            gesture.current_event_time(),
                        );
                    }
                });
                window.add_controller(press);
                // After each frame the input region follows the lock and
                // where the buttons are.
                let painted = window.downgrade();
                let barred = bar.downgrade();
                window.connect_realize(move |window| {
                    let Some(clock) = window.frame_clock() else {
                        return;
                    };
                    let (painted, barred) = (painted.clone(), barred.clone());
                    clock.connect_after_paint(move |_| {
                        if let (Some(window), Some(bar)) = (painted.upgrade(), barred.upgrade()) {
                            input_region(&window, &bar);
                        }
                    });
                });
                Parts { window, bar, lines }
            })
            .clone()
    })
}

/// Unlocked, the whole window takes the pointer; locked, only the buttons
/// that unlock it.
fn input_region(window: &gtk::Window, bar: &gtk::Box) {
    let Some(surface) = window.surface() else {
        return;
    };
    let region = if window.has_css_class("unlocked") {
        gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
            0,
            0,
            surface.width().max(1),
            surface.height().max(1),
        ))
    } else {
        let region = gtk::cairo::Region::create();
        let mut child = bar.first_child();
        while let Some(button) = child {
            if button.can_target()
                && let Some(bounds) = button.compute_bounds(window)
            {
                let _ = region.union_rectangle(&gtk::cairo::RectangleInt::new(
                    bounds.x().floor() as i32,
                    bounds.y().floor() as i32,
                    bounds.width().ceil() as i32,
                    bounds.height().ceil() as i32,
                ));
            }
            child = button.next_sibling();
        }
        region
    };
    surface.set_input_region(Some(&region));
}

fn label(line: &Line) -> gtk::Label {
    let label = gtk::Label::new(Some(&line.text));
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrSize::new_size_absolute(
        (line.size * gtk::pango::SCALE as f64) as i32,
    ));
    if line.bold {
        attributes.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
    }
    let [r, g, b, a] = line.color;
    let wide = |c: u8| c as u16 * 257;
    attributes.insert(gtk::pango::AttrColor::new_foreground(
        wide(r),
        wide(g),
        wide(b),
    ));
    attributes.insert(gtk::pango::AttrInt::new_foreground_alpha(wide(a)));
    label.set_attributes(Some(&attributes));
    label
}

fn show(id: &str, overlay: &Overlay) {
    let Parts { window, bar, lines } = parts(id);
    while let Some(child) = lines.first_child() {
        lines.remove(&child);
    }
    for line in overlay.lines.iter().filter(|line| !line.text.is_empty()) {
        let text = label(line);
        match line.lit.filter(|lit| *lit >= 0.0) {
            Some(lit) => lines.append(&crate::lit::Lit::new(&text, &text, lit)),
            None => lines.append(&text),
        }
    }
    // Every button keeps its slot; locked, only those that unlock it show
    // and take the pointer.
    while let Some(child) = bar.first_child() {
        bar.remove(&child);
    }
    if overlay.place == Place::Free {
        for described in &overlay.buttons {
            let button = gtk::Button::from_icon_name(crate::icons::name(&described.symbol));
            button.add_css_class("bern-overlay-button");
            button.set_focusable(false);
            if overlay.locked && !described.while_locked {
                button.set_opacity(0.0);
                button.set_can_target(false);
            }
            let (overlay_id, button_id) = (id.to_string(), described.id.clone());
            button.connect_clicked(move |_| bern_ffi::overlay::press(&overlay_id, &button_id));
            bar.append(&button);
        }
    }
    bar.set_visible(bar.first_child().is_some());
    if overlay.locked {
        window.remove_css_class("unlocked");
    } else {
        window.add_css_class("unlocked");
    }
    if lines.first_child().is_none() {
        window.set_visible(false);
        return;
    }
    window.present();
    input_region(&window, &bar);
}
