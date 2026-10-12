//! A picture of a declared size.
//!
//! `GtkPicture` asks for the size of its image, which a box then gives it.
//! `Frame` holds one picture at exactly the declared width and height,
//! whatever the image's own size; the picture's content fit draws the image
//! inside. Given more room than that, as across a column, the picture stays
//! at the leading edge.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    pub struct Frame {
        pub width: Cell<i32>,
        pub height: Cell<i32>,
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
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let size = match orientation {
                gtk::Orientation::Horizontal => self.width.get(),
                _ => self.height.get(),
            };
            (size, size, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                // GTK expects a measure before every allocation.
                child.measure(gtk::Orientation::Horizontal, -1);
                child.allocate(
                    width.min(self.width.get()),
                    height.min(self.height.get()),
                    baseline,
                    None,
                );
            }
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
    pub fn new(picture: &gtk::Picture, width: i32, height: i32) -> Self {
        let frame: Self = glib::Object::new();
        frame.imp().width.set(width.max(0));
        frame.imp().height.set(height.max(0));
        frame.set_overflow(gtk::Overflow::Hidden);
        picture.set_parent(&frame);
        frame
    }
}
