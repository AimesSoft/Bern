// Co-located GTK 4 native LevelBar + SpinButton + ToggleButton control.

use super::{Actions, Props, Theme};
use bern_gtk::gtk::{self, prelude::*};
use std::cell::Cell;
use std::rc::Rc;

pub fn build(props: Props, actions: Actions, _theme: Theme) -> impl IsA<gtk::Widget> {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.add_css_class("card");
    root.set_margin_top(4);
    root.set_margin_bottom(4);

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
    inner.set_margin_top(14);
    inner.set_margin_bottom(14);
    inner.set_margin_start(14);
    inner.set_margin_end(14);
    root.append(&inner);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let heading = gtk::Label::new(None);
    heading.add_css_class("heading");
    heading.set_halign(gtk::Align::Start);
    heading.set_hexpand(true);
    let readout = gtk::Label::new(None);
    readout.add_css_class("dim-label");
    readout.add_css_class("numeric");
    header.append(&heading);
    header.append(&readout);
    inner.append(&header);

    // Native GTK 4 discrete LevelBar + SpinButton.
    let meter_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let level_bar = gtk::LevelBar::for_interval(props.level_min, props.level_max);
    level_bar.set_mode(gtk::LevelBarMode::Discrete);
    level_bar.set_hexpand(true);
    level_bar.set_valign(gtk::Align::Center);
    level_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 3.0);
    level_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 7.0);
    level_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_FULL, 9.0);

    // Track hover position along the LevelBar and write back to `draft.hover_level`.
    let motion = gtk::EventControllerMotion::new();
    {
        let bar = level_bar.clone();
        let set_hover = actions.set_hover.clone();
        let max_val = props.level_max;
        motion.connect_motion(move |_, x, _| {
            let w = bar.width() as f64;
            if w > 1.0 {
                let frac = (x / w).clamp(0.0, 1.0);
                set_hover(Some((frac * max_val * 10.0).round() / 10.0));
            }
        });
    }
    {
        let set_hover = actions.set_hover.clone();
        motion.connect_leave(move |_| set_hover(None));
    }
    level_bar.add_controller(motion);

    let adj = gtk::Adjustment::new(
        (props.level)(),
        props.level_min,
        props.level_max,
        props.level_step.unwrap_or(1.0),
        1.0,
        0.0,
    );
    let spin = gtk::SpinButton::new(Some(&adj), 1.0, 1);
    let syncing = Rc::new(Cell::new(false));
    {
        let syncing = syncing.clone();
        let set_level = actions.set_level.clone();
        spin.connect_value_changed(move |s| {
            if !syncing.get() {
                set_level(s.value());
            }
        });
    }
    meter_row.append(&level_bar);
    meter_row.append(&spin);
    inner.append(&meter_row);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let boost = gtk::ToggleButton::new();
    {
        let syncing = syncing.clone();
        let set_boosted = actions.set_boosted.clone();
        boost.connect_toggled(move |b| {
            if !syncing.get() {
                set_boosted(b.is_active());
            }
        });
    }
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let reset = gtk::Button::new();
    {
        let on_reset = actions.reset.clone();
        reset.connect_clicked(move |_| on_reset());
    }
    footer.append(&boost);
    footer.append(&spacer);
    footer.append(&reset);
    inner.append(&footer);

    bern_gtk::reactive::effect(move || {
        syncing.set(true);
        heading.set_text(&(props.heading)());
        readout.set_text(&(props.readout)());
        let val = (props.level)();
        level_bar.set_value(val);
        if (spin.value() - val).abs() > 1e-6 {
            spin.set_value(val);
        }
        boost.set_label(&(props.boosted_label)());
        if boost.is_active() != (props.boosted)() {
            boost.set_active((props.boosted)());
        }
        reset.set_label(&(props.reset_label)());
        syncing.set(false);
    });

    root
}
