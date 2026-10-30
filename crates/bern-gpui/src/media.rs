//! Video. The application's media kernel plays every player and draws each
//! `Video` into a surface of the view's own: on macOS a Metal layer in a
//! view under the window's GPUI view, placed where the `Video` is laid
//! out. The window paints no background there, so the layer shows through
//! with what the declaration puts over it still drawn on top. A frame
//! clock thread tells the kernel when the display wants each frame.
//!
//! Without a media kernel nothing plays: the target has no media executor
//! of its own yet, and a `Video` shows its black backdrop.

use crate::El;
use crate::ctx::Ctx;
use crate::host::{host, status, take};
use crate::protocol::{Value, Writer, encode};
use bern_ffi::media_contract::kernel::SurfaceKind;
use bern_ffi::media_contract::{MediaCommand, media_catalog};
use gpui::{Bounds, IntoElement, ParentElement, Pixels, Styled, div};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// The standard media commands declarations run on players.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaOp {
    Open,
    Play,
    Pause,
    Seek,
    SetVolume,
    SetMuted,
}

impl MediaOp {
    fn command(self) -> MediaCommand {
        match self {
            MediaOp::Open => MediaCommand::Open,
            MediaOp::Play => MediaCommand::Play,
            MediaOp::Pause => MediaCommand::Pause,
            MediaOp::Seek => MediaCommand::Seek,
            MediaOp::SetVolume => MediaCommand::SetVolume,
            MediaOp::SetMuted => MediaCommand::SetMuted,
        }
    }
}

/// Run a standard media command on `player`.
pub fn media(_c: &Ctx, op: MediaOp, player: &str, args: Vec<Value>) {
    let (types, specs) = media_catalog();
    let command = op.command();
    let spec = &specs[command.index() as usize];
    let mut values = vec![Value::Text(player.to_string())];
    values.extend(args);
    let mut writer = Writer::new();
    for (value, param) in values.iter().zip(&spec.params) {
        encode(value, &param.ty, &types, &mut writer);
    }
    // SAFETY: the handle is live while the application runs; the bytes are
    // valid for this call.
    unsafe {
        bern_ffi::media::bern_media_dispatch(
            host().app(),
            command.index(),
            writer.bytes.as_ptr(),
            writer.bytes.len(),
            0,
        )
    }
}

/// How a `Video` shows.
#[derive(Clone, Default)]
pub struct VideoProps {
    /// An application player (`player: players.x`).
    pub player: Option<&'static str>,
    /// The source of the view's own player (`source:`).
    pub source: Option<Value>,
    pub label: String,
    /// `VideoFit`'s code: contain, cover, fill.
    pub fit: u32,
    pub corner: f32,
}

/// One video view.
struct View {
    window: String,
    player: String,
    /// The view created its player (`source:`).
    owns_player: bool,
    /// The source last bound to its own player.
    bound: Option<Option<String>>,
    /// The size the kernel was last given: physical pixels and scale.
    size: Option<(u32, u32, f64)>,
    seen: bool,
    #[cfg(target_os = "macos")]
    surface: Option<mac::Surface>,
}

/// Where videos are and their corners.
type Holes = Vec<(Bounds<Pixels>, f32)>;

thread_local! {
    /// The kind of surface the kernel draws into; none without a kernel.
    static KIND: Cell<Option<SurfaceKind>> = const { Cell::new(None) };
    static VIEWS: RefCell<HashMap<String, View>> = RefCell::new(HashMap::new());
    /// Where each window's videos are, as laid out this frame, with their
    /// corners: no background is painted there.
    static HOLES: RefCell<HashMap<String, Holes>> = RefCell::new(HashMap::new());
}

/// Let the application's media kernel play the players, when it draws into
/// a surface this target can give it.
pub(crate) fn attach_kernel() {
    let offers: &[SurfaceKind] = if cfg!(target_os = "macos") {
        &[SurfaceKind::MetalLayer]
    } else {
        &[]
    };
    let mut writer = Writer::new();
    writer.u32(offers.len() as u32);
    for kind in offers {
        writer.u32(kind.code());
    }
    // SAFETY: the handle is live; the bytes are valid for this call.
    let reply = take(unsafe {
        bern_ffi::media::bern_media_kernel_attach(
            host().app(),
            writer.bytes.as_ptr(),
            writer.bytes.len(),
        )
    });
    let mut reader = crate::protocol::Reader::new(&reply);
    if let (Ok(1), Ok(kind)) = (reader.u8(), reader.u32()) {
        KIND.with(|current| current.set(SurfaceKind::from_code(kind)));
    }
}

fn post(call: unsafe extern "C" fn(*mut bern_ffi::BernApp, *const u8, usize), bytes: &[u8]) {
    // SAFETY: the handle is live; the bytes are valid for this call.
    unsafe { call(host().app(), bytes.as_ptr(), bytes.len()) }
}

/// The windows with a video this frame paint no background under it.
pub(crate) fn holes(window: &str) -> Holes {
    HOLES.with(|holes| holes.borrow().get(window).cloned().unwrap_or_default())
}

/// Whether `window` showed a video when it last drew.
pub(crate) fn has_video(window: &str) -> bool {
    VIEWS.with(|views| views.borrow().values().any(|view| view.window == window))
}

/// Mark the videos of `window` as not yet drawn in this frame.
pub(crate) fn begin(window: &str) {
    VIEWS.with(|views| {
        for view in views.borrow_mut().values_mut() {
            if view.window == window {
                view.seen = false;
            }
        }
    });
    HOLES.with(|holes| holes.borrow_mut().remove(window));
}

/// Take back the surfaces of `window` its frame did not draw; all of them
/// with `closed`.
pub(crate) fn end(window: &str, closed: bool) {
    let gone: Vec<(String, View)> = VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        let tokens: Vec<String> = views
            .iter()
            .filter(|(_, view)| view.window == window && (closed || !view.seen))
            .map(|(token, _)| token.clone())
            .collect();
        tokens
            .into_iter()
            .filter_map(|token| views.remove(&token).map(|view| (token, view)))
            .collect()
    });
    for (token, view) in gone {
        detach(&token, view);
    }
}

fn detach(token: &str, view: View) {
    if view.size.is_some() {
        clock::stop(token);
        let mut writer = Writer::new();
        writer.text(token);
        post(bern_ffi::media::bern_media_surface_detach, &writer.bytes);
        let mut writer = Writer::new();
        writer.text(token);
        writer.u8(0);
        post(bern_ffi::media::bern_media_view, &writer.bytes);
    }
    #[cfg(target_os = "macos")]
    if let Some(surface) = view.surface {
        surface.remove();
    }
    if view.owns_player {
        let mut writer = Writer::new();
        writer.text(&view.player);
        post(bern_ffi::media::bern_media_release, &writer.bytes);
    }
}

/// `Video`: the picture of a player, letterboxed or filling the view as
/// `fit` says, on black.
pub fn video(c: &Ctx, id: &'static str, props: VideoProps) -> El {
    let window = c.win().id.to_string();
    let token = format!("{window}/{}", c.target(id));
    let player = props
        .player
        .map(str::to_string)
        .unwrap_or_else(|| token.clone());
    if !c.listing() {
        let fresh = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            match views.get_mut(&token) {
                Some(view) => {
                    view.seen = true;
                    false
                }
                None => {
                    views.insert(
                        token.clone(),
                        View {
                            window: window.clone(),
                            player: player.clone(),
                            owns_player: props.player.is_none(),
                            bound: None,
                            size: None,
                            seen: true,
                            #[cfg(target_os = "macos")]
                            surface: None,
                        },
                    );
                    true
                }
            }
        });
        if fresh && props.player.is_none() {
            let mut writer = Writer::new();
            writer.text(&player);
            writer.text(&window);
            // SAFETY: the handle is live; the bytes are valid for this call.
            let reply = take(unsafe {
                bern_ffi::media::bern_media_create(
                    host().app(),
                    writer.bytes.as_ptr(),
                    writer.bytes.len(),
                )
            });
            if let Err(failure) = status(&reply) {
                eprintln!("bern: {}: {}", failure.code, failure.message);
            }
        }
        // The view's own player follows its source.
        if let Some(source) = &props.source {
            let url = crate::value::url(source);
            let url = (!url.is_empty()).then_some(url);
            let changed = VIEWS.with(|views| {
                let mut views = views.borrow_mut();
                let view = views.get_mut(&token)?;
                (view.bound.as_ref() != Some(&url)).then(|| view.bound = Some(url.clone()))
            });
            if changed.is_some() {
                let mut writer = Writer::new();
                writer.text(&player);
                match &url {
                    Some(url) => {
                        writer.u8(1);
                        writer.text(url);
                    }
                    None => writer.u8(0),
                }
                // SAFETY: the handle is live; the bytes are valid for this call.
                let _ = take(unsafe {
                    bern_ffi::media::bern_media_bind(
                        host().app(),
                        writer.bytes.as_ptr(),
                        writer.bytes.len(),
                    )
                });
            }
        }
    }
    let corner = props.corner * crate::units::scale();
    let fit = props.fit;
    let probe = {
        let token = token.clone();
        let window_id = window.clone();
        let player = player.clone();
        gpui::canvas(
            move |bounds, window, _| {
                place(&token, &window_id, &player, bounds, corner, fit, window)
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    };
    let element = div()
        .relative()
        .flex()
        .min_w(gpui::px(0.0))
        .min_h(gpui::px(0.0))
        .rounded(crate::units::px(props.corner))
        .child(probe);
    c.grow().apply(element.size_full()).into_any_element()
}

/// The view `token` is laid out at `bounds`: its surface follows, and the
/// kernel is given it or its new size.
#[allow(unused_variables)]
fn place(
    token: &str,
    window_id: &str,
    player: &str,
    bounds: Bounds<Pixels>,
    corner: f32,
    fit: u32,
    window: &mut gpui::Window,
) {
    HOLES.with(|holes| {
        holes
            .borrow_mut()
            .entry(window_id.to_string())
            .or_default()
            .push((bounds, corner))
    });
    let Some(kind) = KIND.with(Cell::get) else {
        return;
    };
    let scale = f64::from(window.scale_factor());
    let size = (
        ((f64::from(f32::from(bounds.size.width)) * scale).round() as u32).max(1),
        ((f64::from(f32::from(bounds.size.height)) * scale).round() as u32).max(1),
        scale,
    );
    #[cfg(target_os = "macos")]
    {
        if kind != SurfaceKind::MetalLayer {
            return;
        }
        let Some((handle, previous)) = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            let view = views.get_mut(token)?;
            if view.surface.is_none() {
                view.surface = mac::Surface::new(window);
                // The layer shows through where the window paints nothing.
                window.set_background_appearance(gpui::WindowBackgroundAppearance::Transparent);
            }
            let surface = view.surface.as_ref()?;
            surface.place(bounds, corner);
            let previous = view.size.replace(size);
            Some((surface.layer(), previous))
        }) else {
            return;
        };
        let mut writer = Writer::new();
        writer.text(token);
        match previous {
            None => {
                let mut view = Writer::new();
                view.text(token);
                view.u8(1);
                view.text(player);
                view.text(window_id);
                post(bern_ffi::media::bern_media_view, &view.bytes);
                writer.text(player);
                writer.u32(kind.code());
                writer.u64(handle);
                writer.u32(size.0);
                writer.u32(size.1);
                writer.f64(size.2);
                writer.u32(fit);
                post(bern_ffi::media::bern_media_surface_attach, &writer.bytes);
                clock::start(token);
            }
            Some(previous) if previous != size => {
                writer.u32(size.0);
                writer.u32(size.1);
                writer.f64(size.2);
                post(bern_ffi::media::bern_media_surface_resize, &writer.bytes);
            }
            Some(_) => {}
        }
    }
}

/// Paint `color` over `bounds` with `corner`, leaving out the videos of
/// `window`.
pub(crate) fn backdrop(window_id: &str, color: gpui::Rgba, corner: f32) -> impl IntoElement {
    let window_id = window_id.to_string();
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut builder = gpui::PathBuilder::fill();
            crate::canvas::outline(
                crate::canvas::ShapeKind::Rounded,
                bounds,
                corner,
                &mut builder,
            );
            for (hole, radius) in holes(&window_id) {
                if let Some(hole) = clip(hole, bounds) {
                    crate::canvas::outline(
                        crate::canvas::ShapeKind::Rounded,
                        hole,
                        radius,
                        &mut builder,
                    );
                }
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn clip(hole: Bounds<Pixels>, within: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
    let clipped = hole.intersect(&within);
    (clipped.size.width > gpui::px(0.0) && clipped.size.height > gpui::px(0.0)).then_some(clipped)
}

/// The frame clock: a thread that asks the kernel for each attached
/// surface's frame at the display's pace.
mod clock {
    use std::sync::Mutex;

    /// The surfaces' tokens; held while a frame reaches the kernel, so none
    /// arrives once a surface is taken back.
    static TOKENS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static APP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    pub(super) fn start(token: &str) {
        APP.store(
            super::host().app() as usize,
            std::sync::atomic::Ordering::SeqCst,
        );
        if let Ok(mut tokens) = TOKENS.lock() {
            tokens.push(token.to_string());
        }
        if !RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let _ = std::thread::Builder::new()
                .name("Bern media frames".into())
                .spawn(run);
        }
    }

    pub(super) fn stop(token: &str) {
        if let Ok(mut tokens) = TOKENS.lock() {
            tokens.retain(|existing| existing != token);
        }
    }

    fn now() -> f64 {
        #[cfg(target_os = "macos")]
        {
            objc2_quartz_core::CACurrentMediaTime()
        }
        #[cfg(not(target_os = "macos"))]
        {
            0.0
        }
    }

    fn run() {
        let interval = 1.0 / 60.0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs_f64(interval));
            let app = APP.load(std::sync::atomic::Ordering::SeqCst) as *mut bern_ffi::BernApp;
            let Ok(tokens) = TOKENS.lock() else { return };
            let now = now();
            for token in tokens.iter() {
                // SAFETY: the handle is live while the application runs; the
                // token's bytes are valid for this call.
                unsafe {
                    bern_ffi::media::bern_media_surface_frame(
                        app,
                        token.as_ptr(),
                        token.len(),
                        now,
                        now + interval,
                    )
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use gpui::{Bounds, Pixels};
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSView, NSWindowOrderingMode};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use objc2_quartz_core::CAMetalLayer;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// A view under the window's GPUI view, hosting the Metal layer the
    /// kernel draws into.
    pub(super) struct Surface {
        view: Retained<NSView>,
        layer: Retained<CAMetalLayer>,
    }

    impl Surface {
        pub(super) fn new(window: &gpui::Window) -> Option<Self> {
            let main = MainThreadMarker::new()?;
            let RawWindowHandle::AppKit(handle) =
                HasWindowHandle::window_handle(window).ok()?.as_raw()
            else {
                return None;
            };
            // SAFETY: GPUI's view, alive while its window is open.
            let gpui_view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
            // SAFETY: the view is in a window.
            let content = unsafe { gpui_view.superview() }?;
            let layer = CAMetalLayer::new();
            let view = NSView::initWithFrame(
                NSView::alloc(main),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
            );
            view.setLayer(Some(&layer));
            view.setWantsLayer(true);
            let black = objc2_core_graphics::CGColor::new_generic_gray(0.0, 1.0);
            layer.setBackgroundColor(Some(&black));
            layer.setMasksToBounds(true);
            content.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, None);
            Some(Surface { view, layer })
        }

        /// The layer's address, for the kernel.
        pub(super) fn layer(&self) -> u64 {
            Retained::as_ptr(&self.layer) as u64
        }

        /// Stand at `bounds` of the window, with rounded corners.
        pub(super) fn place(&self, bounds: Bounds<Pixels>, corner: f32) {
            // SAFETY: the view is in the window's content view.
            let Some(content) = (unsafe { self.view.superview() }) else {
                return;
            };
            let height = content.bounds().size.height;
            let (x, y) = (
                f64::from(f32::from(bounds.origin.x)),
                f64::from(f32::from(bounds.origin.y)),
            );
            let (w, h) = (
                f64::from(f32::from(bounds.size.width)),
                f64::from(f32::from(bounds.size.height)),
            );
            let flipped = content.isFlipped();
            let frame = NSRect::new(
                NSPoint::new(x, if flipped { y } else { height - y - h }),
                NSSize::new(w, h),
            );
            if self.view.frame() != frame {
                self.view.setFrame(frame);
            }
            self.layer.setCornerRadius(f64::from(corner));
        }

        pub(super) fn remove(self) {
            self.view.removeFromSuperview();
        }
    }
}
