//! Decorations of a native symbolic icon, drawn without changing its measurement.

use crate::theme::{self, Color};
use gtk::{glib, graphene, prelude::*, subclass::prelude::*};
use std::cell::{Cell, OnceCell};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct IconGlyph {
        pub base: OnceCell<gtk::Image>,
        pub badge: OnceCell<gtk::Image>,
        pub progress: Cell<f64>,
        pub badge_visible: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IconGlyph {
        const NAME: &'static str = "BernIconGlyph";
        type Type = super::IconGlyph;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for IconGlyph {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for IconGlyph {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            self.base
                .get()
                .map_or((0, 0, -1, -1), |base| base.measure(orientation, for_size))
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(base) = self.base.get() {
                base.allocate(width, height, baseline, None);
            }
            if let Some(badge) = self.badge.get() {
                let size = width.min(height) as f32;
                let side = (size * 0.4).round().max(1.0) as i32;
                badge.set_pixel_size(side);
                let offset =
                    graphene::Point::new(width as f32 - size * 0.4, height as f32 - size * 0.4);
                badge.allocate(
                    side,
                    side,
                    -1,
                    Some(gtk::gsk::Transform::new().translate(&offset)),
                );
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let Some(base) = self.base.get() else { return };
            obj.snapshot_child(base, snapshot);
            let width = obj.width() as f32;
            let height = obj.height() as f32;
            let accent = theme::rgba(&Color::Role("accent"), obj.upcast_ref());
            if self.progress.get() > 0.0 {
                snapshot.push_clip(&graphene::Rect::new(
                    0.0,
                    0.0,
                    width,
                    height * self.progress.get() as f32,
                ));
                tint(snapshot, &accent);
                obj.snapshot_child(base, snapshot);
                snapshot.pop();
                snapshot.pop();
            }
            if self.badge_visible.get()
                && let Some(badge) = self.badge.get()
            {
                let size = width.min(height);
                let bounds = graphene::Rect::new(
                    width - size * 0.46,
                    height - size * 0.46,
                    size * 0.52,
                    size * 0.52,
                );
                snapshot.push_rounded_clip(&gtk::gsk::RoundedRect::from_rect(bounds, size));
                snapshot.append_color(
                    &theme::rgba(&Color::Role("surface.canvas"), obj.upcast_ref()),
                    &bounds,
                );
                snapshot.pop();
                tint(snapshot, &accent);
                obj.snapshot_child(badge, snapshot);
                snapshot.pop();
            }
        }
    }

    /// Replace RGB while preserving the theme icon's alpha, including antialiasing.
    fn tint(snapshot: &gtk::Snapshot, color: &gtk::gdk::RGBA) {
        snapshot.push_color_matrix(
            &graphene::Matrix::from_float([
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]),
            &graphene::Vec4::new(color.red(), color.green(), color.blue(), 0.0),
        );
    }
}

glib::wrapper! {
    pub struct IconGlyph(ObjectSubclass<imp::IconGlyph>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl IconGlyph {
    pub(crate) fn new(name: &str, badge: Option<&str>, size: Option<f64>) -> Self {
        let glyph: Self = glib::Object::new();
        glyph.set_can_target(false);
        glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
        let base = gtk::Image::from_icon_name(crate::icons::name(name));
        if let Some(size) = size {
            base.set_pixel_size(size.round().max(1.0) as i32);
        }
        base.set_parent(&glyph);
        glyph.imp().base.set(base).expect("one base image");
        if let Some(badge) = badge {
            let badge = gtk::Image::from_icon_name(crate::icons::name(badge));
            badge.set_parent(&glyph);
            glyph.imp().badge.set(badge).expect("one badge image");
        }
        glyph.imp().badge_visible.set(true);
        glyph
    }

    pub(crate) fn set_progress(&self, value: f64) {
        self.imp().progress.set(if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        });
        self.queue_draw();
    }

    pub(crate) fn set_badge_visible(&self, visible: bool) {
        self.imp().badge_visible.set(visible);
        self.queue_draw();
    }
}
