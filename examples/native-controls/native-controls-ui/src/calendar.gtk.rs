// Co-located GTK 4 native gtk::Calendar control.

use super::{Actions, Props, Theme};
use bern_gtk::{
    glib,
    gtk::{self, prelude::*},
};
use std::cell::Cell;
use std::rc::Rc;

pub fn build(props: Props, actions: Actions, _theme: Theme) -> impl IsA<gtk::Widget> {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.add_css_class("card");

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
    inner.set_margin_top(14);
    inner.set_margin_bottom(14);
    inner.set_margin_start(14);
    inner.set_margin_end(14);
    root.append(&inner);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let heading = gtk::Label::new(None);
    heading.add_css_class("heading");
    heading.set_halign(gtk::Align::Start);
    heading.set_hexpand(true);
    let date_label = gtk::Label::new(None);
    date_label.add_css_class("dim-label");
    date_label.add_css_class("numeric");
    let today_btn = gtk::Button::new();
    {
        let on_today = actions.today.clone();
        today_btn.connect_clicked(move |_| on_today());
    }
    header.append(&heading);
    header.append(&date_label);
    header.append(&today_btn);
    inner.append(&header);

    let calendar = gtk::Calendar::new();
    let syncing = Rc::new(Cell::new(false));
    {
        let syncing = syncing.clone();
        let set_date = actions.set_date.clone();
        calendar.connect_day_selected(move |cal| {
            if syncing.get() {
                return;
            }
            let dt = cal.date();
            let iso = format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day_of_month());
            set_date(iso);
        });
    }
    inner.append(&calendar);

    bern_gtk::reactive::effect(move || {
        syncing.set(true);
        heading.set_text(&(props.heading)());
        let iso = (props.date)();
        date_label.set_text(&iso);
        today_btn.set_label(&(props.today_label)());
        let mut parts = iso.split('-');
        if let (Some(y), Some(m), Some(d)) = (
            parts.next().and_then(|s| s.parse::<i32>().ok()),
            parts.next().and_then(|s| s.parse::<i32>().ok()),
            parts.next().and_then(|s| s.parse::<i32>().ok()),
        ) && let Ok(dt) = glib::DateTime::from_utc(y, m, d, 12, 0, 0.0)
        {
            calendar.select_day(&dt);
        }
        syncing.set(false);
    });

    root
}
