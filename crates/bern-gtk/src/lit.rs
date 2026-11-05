//! Text lit part of the way: drawn through a mask that is opaque as far as
//! it is lit and faded after, the edge inside the character it stops in.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};
use std::cell::{Cell, RefCell};

/// How much of its alpha what is not lit keeps.
const FADED: f32 = 0.4;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Lit {
        pub child: RefCell<Option<gtk::Widget>>,
        pub label: RefCell<Option<gtk::Label>>,
        pub lit: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Lit {
        const NAME: &'static str = "BernLit";
        type Type = super::Lit;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Lit {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Lit {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (child, label) = (self.child.borrow(), self.label.borrow());
            let (Some(child), Some(label)) = (child.as_ref(), label.as_ref()) else {
                return;
            };
            let lit = self.lit.get();
            if lit < 0.0 {
                widget.snapshot_child(child, snapshot);
                return;
            }
            // The mask is recorded first, then what shows through it.
            snapshot.push_mask(gsk::MaskMode::Alpha);
            super::mask(snapshot, &widget, label, lit);
            snapshot.pop();
            widget.snapshot_child(child, snapshot);
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    /// `child`, which holds `label`, lit as far as `set` says.
    pub struct Lit(ObjectSubclass<imp::Lit>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Lit {
    pub fn new(child: &impl IsA<gtk::Widget>, label: &gtk::Label, lit: f64) -> Self {
        let widget: Self = glib::Object::new();
        child.as_ref().set_parent(&widget);
        widget.imp().child.replace(Some(child.as_ref().clone()));
        widget.imp().label.replace(Some(label.clone()));
        widget.imp().lit.set(lit);
        widget
    }

    /// As far through its characters as `lit`; below zero, all of it.
    pub fn set(&self, lit: f64) {
        self.imp().lit.set(lit);
        self.queue_draw();
    }
}

fn mask(snapshot: &gtk::Snapshot, widget: &Lit, label: &gtk::Label, lit: f64) {
    let (width, height) = (widget.width() as f32, widget.height() as f32);
    let opaque = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
    let faded = gdk::RGBA::new(0.0, 0.0, 0.0, FADED);
    let text = label.text();
    let Some((at, _)) = text.char_indices().nth(lit as usize) else {
        snapshot.append_color(&opaque, &graphene::Rect::new(0.0, 0.0, width, height));
        return;
    };
    let origin = label
        .compute_point(widget, &graphene::Point::zero())
        .unwrap_or_else(graphene::Point::zero);
    let (left, top) = label.layout_offsets();
    let scale = gtk::pango::SCALE as f32;
    let place = label.layout().index_to_pos(at as i32);
    let x = origin.x() + left as f32 + place.x() as f32 / scale;
    let y = origin.y() + top as f32 + place.y() as f32 / scale;
    let (wide, high) = (place.width() as f32 / scale, place.height() as f32 / scale);
    let end = x + wide * lit.fract() as f32;
    snapshot.append_color(&opaque, &graphene::Rect::new(0.0, 0.0, width, y));
    snapshot.append_color(&opaque, &graphene::Rect::new(0.0, y, end, high));
    snapshot.append_color(&faded, &graphene::Rect::new(end, y, width - end, high));
    let below = y + high;
    snapshot.append_color(
        &faded,
        &graphene::Rect::new(0.0, below, width, (height - below).max(0.0)),
    );
}
