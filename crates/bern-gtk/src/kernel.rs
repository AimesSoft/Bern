//! Video views of the application's media kernel.
//!
//! GTK draws a whole window into one surface, so a view has no native
//! surface of its own to give the kernel. The view gives it a frame sink
//! instead: the kernel renders the view's pictures at the view's size in
//! physical pixels, and the view shows the latest one as a texture, which
//! the window composites with everything around and above it (the control
//! bar, rounded corners, transitions). The view's frame clock asks the
//! kernel for a picture once per display frame while the view is mapped.

use crate::host::{host, with_bytes};
use crate::protocol::Writer;
use bern_ffi::media_contract::kernel::{
    self, FrameSink, SurfaceHandle, SurfaceKind, VideoFit, VideoFrame,
};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The picture waiting for the main thread; a newer one replaces it.
#[derive(Default)]
struct Pending {
    frame: Mutex<Option<VideoFrame>>,
    scheduled: AtomicBool,
}

struct Sink {
    token: String,
    pending: Arc<Pending>,
}

impl FrameSink for Sink {
    fn present(&self, frame: VideoFrame) {
        *self.pending.frame.lock().expect("frame lock") = Some(frame);
        if !self.pending.scheduled.swap(true, Ordering::AcqRel) {
            let token = self.token.clone();
            glib::MainContext::default().invoke(move || show(&token));
        }
    }
}

/// A view with an attached surface, and the picture waiting for it.
type Attached = (glib::WeakRef<Screen>, Arc<Pending>);

thread_local! {
    /// Views with an attached surface, by token.
    static VIEWS: RefCell<HashMap<String, Attached>> = RefCell::new(HashMap::new());
}

/// Show the pending picture of `token`'s view.
fn show(token: &str) {
    let Some((screen, pending)) = VIEWS.with(|views| {
        views
            .borrow()
            .get(token)
            .map(|(screen, pending)| (screen.upgrade(), Arc::clone(pending)))
    }) else {
        return;
    };
    pending.scheduled.store(false, Ordering::Release);
    let frame = pending.frame.lock().expect("frame lock").take();
    if let (Some(screen), Some(frame)) = (screen, frame) {
        screen.imp().set_frame(frame);
    }
}

fn post(name: unsafe extern "C" fn(*mut bern_ffi::BernApp, *const u8, usize), bytes: &[u8]) {
    with_bytes(bytes, |ptr, len| {
        // SAFETY: the handle is live; the bytes are valid for this call.
        unsafe { name(host().app(), ptr, len) }
    });
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Screen {
        pub token: RefCell<String>,
        pub player: RefCell<String>,
        pub fit: Cell<u32>,
        pub texture: RefCell<Option<gtk::gdk::Texture>>,
        /// The sink and the size the kernel draws at, while attached.
        pub attached: Cell<Option<(SurfaceHandle, u32, u32, f64)>>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Screen {
        const NAME: &'static str = "BernKernelVideo";
        type Type = super::Screen;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Screen {
        fn dispose(&self) {
            self.stop();
        }
    }

    impl WidgetImpl for Screen {
        fn map(&self) {
            self.parent_map();
            let tick = self.obj().add_tick_callback(|screen, clock| {
                screen.imp().tick(clock);
                glib::ControlFlow::Continue
            });
            *self.tick.borrow_mut() = Some(tick);
        }

        fn unmap(&self) {
            self.stop();
            self.parent_unmap();
        }

        /// The kernel draws at the view's size, which the picture fills.
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(texture) = self.texture.borrow().clone() else {
                return;
            };
            let obj = self.obj();
            let bounds =
                gtk::graphene::Rect::new(0.0, 0.0, obj.width() as f32, obj.height() as f32);
            snapshot.append_scaled_texture(&texture, gtk::gsk::ScalingFilter::Linear, &bounds);
        }
    }

    impl Screen {
        pub fn set_frame(&self, frame: VideoFrame) {
            if self.attached.get().is_none() || frame.width == 0 || frame.height == 0 {
                return;
            }
            let stride = frame.width as usize * 4;
            if frame.pixels.len() < stride * frame.height as usize {
                return;
            }
            let texture = gtk::gdk::MemoryTexture::new(
                frame.width as i32,
                frame.height as i32,
                gtk::gdk::MemoryFormat::R8g8b8a8,
                &glib::Bytes::from_owned(frame.pixels),
                stride,
            );
            *self.texture.borrow_mut() = Some(texture.upcast());
            self.obj().queue_draw();
        }

        /// Once per display frame: follow the view's size, then ask the
        /// kernel for the picture after the next one, which is when what it
        /// renders now reaches the screen.
        fn tick(&self, clock: &gtk::gdk::FrameClock) {
            let obj = self.obj();
            let (width, height) = (obj.width(), obj.height());
            if width <= 0 || height <= 0 {
                return;
            }
            let scale = obj
                .native()
                .and_then(|native| native.surface())
                .map(|surface| surface.scale())
                .filter(|scale| scale.is_finite() && *scale > 0.0)
                .unwrap_or_else(|| f64::from(obj.scale_factor()));
            let pixels = |points: i32| ((f64::from(points) * scale).round() as u32).max(1);
            let size = (pixels(width), pixels(height), scale);
            match self.attached.get() {
                None => self.attach(size),
                Some((_, w, h, s)) if (w, h, s) != size => self.resize(size),
                _ => {}
            }
            let now = clock.frame_time() as f64 / 1e6;
            let timings = clock.current_timings();
            let interval = timings
                .as_ref()
                .map(|timings| timings.refresh_interval())
                .filter(|interval| *interval > 0)
                .unwrap_or(16_667) as f64
                / 1e6;
            let presented = timings
                .as_ref()
                .map(|timings| timings.predicted_presentation_time())
                .filter(|time| *time > 0)
                .map_or(now + interval, |time| time as f64 / 1e6);
            let token = self.token.borrow();
            let host = host();
            // SAFETY: the handle is live while the application runs; the
            // token is valid for this call.
            unsafe {
                bern_ffi::media::bern_media_surface_frame(
                    host.app(),
                    token.as_ptr(),
                    token.len(),
                    now,
                    presented + interval,
                )
            };
        }

        fn attach(&self, (width, height, scale): (u32, u32, f64)) {
            let token = self.token.borrow().clone();
            let pending = Arc::new(Pending::default());
            let handle = kernel::register_frame_sink(Arc::new(Sink {
                token: token.clone(),
                pending: Arc::clone(&pending),
            }));
            let SurfaceHandle::FrameSink(id) = handle else {
                return;
            };
            VIEWS.with(|views| {
                views
                    .borrow_mut()
                    .insert(token.clone(), (self.obj().downgrade(), pending))
            });
            self.attached.set(Some((handle, width, height, scale)));
            let mut writer = Writer::new();
            writer.text(&token);
            writer.text(&self.player.borrow());
            writer.u32(SurfaceKind::FrameSink.code());
            writer.u64(id);
            writer.u32(width);
            writer.u32(height);
            writer.f64(scale);
            writer.u32(self.fit.get());
            post(bern_ffi::media::bern_media_surface_attach, &writer.bytes);
        }

        fn resize(&self, (width, height, scale): (u32, u32, f64)) {
            let Some((handle, ..)) = self.attached.get() else {
                return;
            };
            self.attached.set(Some((handle, width, height, scale)));
            let mut writer = Writer::new();
            writer.text(&self.token.borrow());
            writer.u32(width);
            writer.u32(height);
            writer.f64(scale);
            post(bern_ffi::media::bern_media_surface_resize, &writer.bytes);
        }

        /// Stop the frame clock and take the surface back from the kernel,
        /// which no longer draws for the view once this returns. The last
        /// picture stays until the view shows again.
        fn stop(&self) {
            if let Some(tick) = self.tick.borrow_mut().take() {
                tick.remove();
            }
            let Some((handle, ..)) = self.attached.take() else {
                return;
            };
            let token = self.token.borrow().clone();
            let mut writer = Writer::new();
            writer.text(&token);
            post(bern_ffi::media::bern_media_surface_detach, &writer.bytes);
            kernel::unregister_frame_sink(handle);
            VIEWS.with(|views| views.borrow_mut().remove(&token));
        }
    }
}

glib::wrapper! {
    /// A video view's picture of the application's media kernel.
    pub struct Screen(ObjectSubclass<imp::Screen>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Screen {
    /// The picture of `player` for the view `token`, drawn with `fit`.
    pub fn new(token: &str, player: &str, fit: VideoFit) -> Self {
        let screen: Self = glib::Object::new();
        let imp = screen.imp();
        *imp.token.borrow_mut() = token.to_string();
        *imp.player.borrow_mut() = player.to_string();
        imp.fit.set(match fit {
            VideoFit::Contain => 0,
            VideoFit::Cover => 1,
            VideoFit::Fill => 2,
        });
        screen
    }
}
