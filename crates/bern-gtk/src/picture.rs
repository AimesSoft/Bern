//! A picture of a declared size.
//!
//! `GtkPicture` asks for the size of its image, which a box then gives it.
//! `Frame` holds one picture at exactly the declared width and height,
//! whatever the image's own size; the picture's content fit draws the image
//! inside. Given more room than that, as across a column, the picture stays
//! at the leading edge. With only an aspect ratio, the frame is as wide as
//! its container and as tall as the ratio makes that.
//!
//! `Template` draws a picture as a symbol: its shape in the foreground
//! color.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Frame {
        pub width: Cell<i32>,
        pub height: Cell<i32>,
        /// Width over height, when the frame takes its container's width.
        pub fill: Cell<Option<f64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Frame {
        const NAME: &'static str = "BernPictureFrame";
        type Type = super::Frame;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Frame {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Frame {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            if self.fill.get().is_some() {
                gtk::SizeRequestMode::HeightForWidth
            } else {
                gtk::SizeRequestMode::ConstantSize
            }
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            if let Some(ratio) = self.fill.get() {
                return match orientation {
                    gtk::Orientation::Horizontal => (0, 0, -1, -1),
                    _ => {
                        // It may be shorter where its container is: its
                        // height grows with the width it gets.
                        let height = if for_size > 0 {
                            (f64::from(for_size) / ratio.max(f64::EPSILON)).round() as i32
                        } else {
                            0
                        };
                        (0, height, -1, -1)
                    }
                };
            }
            let size = match orientation {
                gtk::Orientation::Horizontal => self.width.get(),
                _ => self.height.get(),
            };
            (size, size, -1, -1)
        }

        /// A declared size never asks for more room, whatever the picture
        /// inside; with only a ratio, it takes the container's width.
        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = self.fill.get().is_some();
            *vexpand = false;
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                // GTK expects a measure before every allocation.
                child.measure(gtk::Orientation::Horizontal, -1);
                let (w, h) = if self.fill.get().is_some() {
                    (width, height)
                } else {
                    (width.min(self.width.get()), height.min(self.height.get()))
                };
                child.allocate(w, h, baseline, None);
            }
        }
    }

    #[derive(Default)]
    pub struct Template {
        pub paintable: RefCell<Option<gtk::gdk::Paintable>>,
        pub fit: Cell<Option<gtk::ContentFit>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Template {
        const NAME: &'static str = "BernTemplatePicture";
        type Type = super::Template;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for Template {}

    impl WidgetImpl for Template {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let natural = self
                .paintable
                .borrow()
                .as_ref()
                .map(|paintable| match orientation {
                    gtk::Orientation::Horizontal => paintable.intrinsic_width(),
                    _ => paintable.intrinsic_height(),
                })
                .unwrap_or(0)
                .max(0);
            (0, natural, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some(paintable) = self.paintable.borrow().clone() else {
                return;
            };
            let (width, height) = (widget.width() as f64, widget.height() as f64);
            let (iw, ih) = (
                f64::from(paintable.intrinsic_width().max(1)),
                f64::from(paintable.intrinsic_height().max(1)),
            );
            let (w, h) = match self.fit.get().unwrap_or(gtk::ContentFit::Contain) {
                gtk::ContentFit::Fill => (width, height),
                gtk::ContentFit::Cover => {
                    let scale = (width / iw).max(height / ih);
                    (iw * scale, ih * scale)
                }
                _ => {
                    let scale = (width / iw).min(height / ih);
                    (iw * scale, ih * scale)
                }
            };
            let (x, y) = ((width - w) / 2.0, (height - h) / 2.0);
            let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
            snapshot.push_clip(&bounds);
            // The picture's alpha masks a fill of the foreground color.
            snapshot.push_mask(gtk::gsk::MaskMode::Alpha);
            snapshot.save();
            snapshot.translate(&gtk::graphene::Point::new(x as f32, y as f32));
            paintable.snapshot(snapshot, w, h);
            snapshot.restore();
            snapshot.pop();
            snapshot.append_color(&widget.color(), &bounds);
            snapshot.pop();
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    pub struct Frame(ObjectSubclass<imp::Frame>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Frame {
    /// A frame of `width` by `height` holding `picture`.
    pub fn new(picture: &impl IsA<gtk::Widget>, width: i32, height: i32) -> Self {
        let frame: Self = glib::Object::new();
        frame.imp().width.set(width.max(0));
        frame.imp().height.set(height.max(0));
        frame.set_overflow(gtk::Overflow::Hidden);
        picture.set_parent(&frame);
        frame
    }

    /// Change the declared size, as a bound length does.
    pub fn set_size(&self, width: i32, height: i32) {
        let (width, height) = (width.max(0), height.max(0));
        if self.imp().width.get() != width || self.imp().height.get() != height {
            self.imp().width.set(width);
            self.imp().height.set(height);
            self.queue_resize();
        }
    }

    /// As wide as the container, as tall as `ratio` (width over height)
    /// makes that.
    pub fn fill_width(&self, ratio: f64) {
        self.imp().fill.set(Some(ratio));
        self.set_hexpand(true);
        self.queue_resize();
    }
}

glib::wrapper! {
    /// A picture drawn as a symbol: its shape in the foreground color.
    pub struct Template(ObjectSubclass<imp::Template>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Template {
    pub fn new(fit: gtk::ContentFit) -> Self {
        let template: Self = glib::Object::new();
        template.imp().fit.set(Some(fit));
        template
    }

    pub fn set_paintable(&self, paintable: Option<&gtk::gdk::Paintable>) {
        *self.imp().paintable.borrow_mut() = paintable.cloned();
        self.queue_resize();
    }
}
