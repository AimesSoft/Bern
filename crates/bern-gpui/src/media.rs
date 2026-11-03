//! Video. The application's media kernel plays every player and draws each
//! `Video` into a surface of the view's own. Native surfaces stand under
//! the window's GPUI content, placed where the `Video` is laid out: on
//! macOS a Metal layer in a view under the GPUI view, on Windows a swap
//! chain in a DirectComposition tree behind the one GPUI draws into. The
//! window paints no background there, so the video shows through with what
//! the declaration puts over it still drawn on top. A kernel that draws
//! no native surface (Linux, or any kernel that renders pictures itself)
//! hands the view pictures of its size instead, which the window paints
//! as an image. A frame clock thread tells the kernel when the display
//! wants each frame.
//!
//! Without a media kernel nothing plays: the target has no media executor
//! of its own yet, and a `Video` shows its black backdrop.

use crate::El;
use crate::ctx::Ctx;
use crate::host::{host, status, take};
use crate::protocol::{Value, Writer, encode};
use bern_ffi::media_contract::kernel::{self, FrameSink, SurfaceHandle, SurfaceKind, VideoFrame};
use bern_ffi::media_contract::{MediaCommand, media_catalog};
use gpui::{Bounds, IntoElement, ParentElement, Pixels, RenderImage, Styled, div};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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
    surface: Option<Surface>,
}

/// What a view gives the kernel to draw into.
enum Surface {
    #[cfg(target_os = "macos")]
    Layer(mac::Surface),
    #[cfg(target_os = "windows")]
    Panel(win::Surface),
    /// Pictures the window paints.
    Pictures(Pictures),
}

impl Surface {
    /// A surface of `kind` for a view of `window`.
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        allow(unused_variables)
    )]
    fn new(kind: SurfaceKind, window: &mut gpui::Window) -> Option<Self> {
        // A native surface shows through where the window paints nothing.
        let _under = |window: &mut gpui::Window| {
            window.set_background_appearance(gpui::WindowBackgroundAppearance::Transparent)
        };
        match kind {
            #[cfg(target_os = "macos")]
            SurfaceKind::MetalLayer => {
                let surface = mac::Surface::new(window)?;
                _under(window);
                Some(Surface::Layer(surface))
            }
            #[cfg(target_os = "windows")]
            SurfaceKind::SwapChainPanel => {
                let surface = win::Surface::new(window)?;
                _under(window);
                Some(Surface::Panel(surface))
            }
            SurfaceKind::FrameSink => {
                let latest = Arc::new(Latest::default());
                let handle = kernel::register_frame_sink(Arc::new(Sink(Arc::clone(&latest))));
                Some(Surface::Pictures(Pictures {
                    handle,
                    latest,
                    shown: None,
                }))
            }
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// The surface's address, for the kernel.
    fn handle(&self) -> u64 {
        match self {
            #[cfg(target_os = "macos")]
            Surface::Layer(surface) => surface.layer(),
            #[cfg(target_os = "windows")]
            Surface::Panel(surface) => surface.handle(),
            Surface::Pictures(pictures) => match pictures.handle {
                SurfaceHandle::FrameSink(id) => id,
                _ => 0,
            },
        }
    }

    /// Stand at `bounds` of the window, with rounded corners.
    #[allow(unused_variables)]
    fn place(&self, bounds: Bounds<Pixels>, corner: f32, scale: f32) {
        match self {
            #[cfg(target_os = "macos")]
            Surface::Layer(surface) => surface.place(bounds, corner),
            #[cfg(target_os = "windows")]
            Surface::Panel(surface) => surface.place(bounds, corner, scale),
            Surface::Pictures(_) => {}
        }
    }

    /// The view is gone and the kernel no longer draws into the surface.
    fn remove(self) {
        match self {
            #[cfg(target_os = "macos")]
            Surface::Layer(surface) => surface.remove(),
            #[cfg(target_os = "windows")]
            Surface::Panel(surface) => surface.remove(),
            Surface::Pictures(pictures) => {
                kernel::unregister_frame_sink(pictures.handle);
                if let Some(shown) = pictures.shown {
                    DROPPED.with(|dropped| dropped.borrow_mut().push(shown));
                }
            }
        }
    }
}

/// The kinds of surface that stand under the window's content.
fn under(kind: SurfaceKind) -> bool {
    matches!(kind, SurfaceKind::MetalLayer | SurfaceKind::SwapChainPanel)
}

/// The newest picture of a view, until the window paints it.
#[derive(Default)]
struct Latest {
    frame: Mutex<Option<VideoFrame>>,
    /// The window was asked to paint it.
    asked: AtomicBool,
}

/// The sink a kernel hands a view's pictures to, from its own threads.
struct Sink(Arc<Latest>);

impl FrameSink for Sink {
    fn present(&self, frame: VideoFrame) {
        if let Ok(mut latest) = self.0.frame.lock() {
            *latest = Some(frame);
        }
        if !self.0.asked.swap(true, Ordering::AcqRel) {
            crate::host::redraw();
        }
    }
}

/// A view's pictures: the sink's handle, the newest picture and the one
/// the window paints.
struct Pictures {
    handle: SurfaceHandle,
    latest: Arc<Latest>,
    shown: Option<Arc<RenderImage>>,
}

impl Pictures {
    /// The picture to paint: the newest, once the window has it.
    fn shown(&mut self, window: &mut gpui::Window) -> Option<Arc<RenderImage>> {
        self.latest.asked.store(false, Ordering::Release);
        let frame = self
            .latest
            .frame
            .lock()
            .ok()
            .and_then(|mut frame| frame.take());
        if let Some(frame) = frame {
            let VideoFrame {
                width,
                height,
                mut pixels,
            } = frame;
            // GPUI's pictures are BGRA.
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            if let Some(buffer) = image::RgbaImage::from_raw(width, height, pixels) {
                let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                    buffer
                )]));
                if let Some(old) = self.shown.replace(image) {
                    let _ = window.drop_image(old);
                }
            }
        }
        self.shown.clone()
    }
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
    /// Pictures of views that are gone, which a window frees.
    static DROPPED: RefCell<Vec<Arc<RenderImage>>> = const { RefCell::new(Vec::new()) };
}

/// Let the application's media kernel play the players, when it draws into
/// a surface this target can give it.
pub(crate) fn attach_kernel() {
    // Native surfaces first; pictures wherever a kernel renders them.
    let offers: &[SurfaceKind] = if cfg!(target_os = "macos") {
        &[SurfaceKind::MetalLayer, SurfaceKind::FrameSink]
    } else if cfg!(target_os = "windows") {
        &[SurfaceKind::SwapChainPanel, SurfaceKind::FrameSink]
    } else {
        &[SurfaceKind::FrameSink]
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
    log(|| format!("kernel attached: {:?}", KIND.with(Cell::get)));
}

/// Development aid: `BERN_MEDIA_LOG` prints how videos reach the kernel.
fn log(what: impl FnOnce() -> String) {
    if std::env::var_os("BERN_MEDIA_LOG").is_some() {
        eprintln!("bern-media {}", what());
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

/// Whether `window` showed a video under its content when it last drew.
pub(crate) fn has_video(window: &str) -> bool {
    KIND.with(Cell::get).is_some_and(under)
        && VIEWS.with(|views| views.borrow().values().any(|view| view.window == window))
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
    let token = format!("{window}/{}", c.key(id));
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
        let painted = token.clone();
        gpui::canvas(
            move |bounds, window, _| {
                place(&token, &window_id, &player, bounds, corner, fit, window)
            },
            move |bounds, _, window, _| paint(&painted, bounds, corner, window),
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
fn place(
    token: &str,
    window_id: &str,
    player: &str,
    bounds: Bounds<Pixels>,
    corner: f32,
    fit: u32,
    window: &mut gpui::Window,
) {
    let Some(kind) = KIND.with(Cell::get) else {
        log(|| format!("{token}: no kernel surface"));
        return;
    };
    if under(kind) {
        HOLES.with(|holes| {
            holes
                .borrow_mut()
                .entry(window_id.to_string())
                .or_default()
                .push((bounds, corner))
        });
    }
    let scale = window.scale_factor();
    let size = (
        ((f64::from(f32::from(bounds.size.width)) * f64::from(scale)).round() as u32).max(1),
        ((f64::from(f32::from(bounds.size.height)) * f64::from(scale)).round() as u32).max(1),
        f64::from(scale),
    );
    let Some((handle, previous)) = VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        let view = views.get_mut(token)?;
        if view.surface.is_none() {
            view.surface = Surface::new(kind, window);
            log(|| format!("{token}: {kind:?} surface {}", view.surface.is_some()));
        }
        let surface = view.surface.as_ref()?;
        surface.place(bounds, corner, scale);
        let previous = view.size.replace(size);
        Some((surface.handle(), previous))
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
            log(|| format!("surface {token} for {player}: {size:?}"));
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

/// Paint the view `token`: nothing over a native surface, else black with
/// the kernel's newest picture over it.
fn paint(token: &str, bounds: Bounds<Pixels>, corner: f32, window: &mut gpui::Window) {
    for gone in DROPPED.with(|dropped| std::mem::take(&mut *dropped.borrow_mut())) {
        let _ = window.drop_image(gone);
    }
    if KIND.with(Cell::get).is_some_and(under) {
        return;
    }
    let corners = gpui::Corners::all(gpui::px(corner));
    window.paint_quad(gpui::fill(bounds, gpui::black()).corner_radii(corners));
    let shown = VIEWS.with(
        |views| match views.borrow_mut().get_mut(token)?.surface.as_mut()? {
            Surface::Pictures(pictures) => pictures.shown(window),
            #[allow(unreachable_patterns)]
            _ => None,
        },
    );
    if let Some(image) = shown {
        let _ = window.paint_image(bounds, bounds, corners, image, 0, false);
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

    /// Seconds of the clock the kernels time frames by: Core Animation's
    /// on macOS, the performance counter on Windows, the monotonic clock
    /// on Linux (GLib's).
    fn now() -> f64 {
        #[cfg(target_os = "macos")]
        {
            objc2_quartz_core::CACurrentMediaTime()
        }
        #[cfg(target_os = "windows")]
        {
            use win32::Win32::System::Performance::{
                QueryPerformanceCounter, QueryPerformanceFrequency,
            };
            let (mut count, mut frequency) = (0i64, 0i64);
            // SAFETY: both write one integer.
            unsafe {
                let _ = QueryPerformanceCounter(&mut count);
                let _ = QueryPerformanceFrequency(&mut frequency);
            }
            count as f64 / frequency.max(1) as f64
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let mut time = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // SAFETY: the call writes the time it reads.
            unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) };
            time.tv_sec as f64 + time.tv_nsec as f64 / 1e9
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

/// Windows: each window's videos are visuals of a DirectComposition tree
/// behind the one GPUI draws into, each showing the swap chain the kernel
/// gives its view's panel.
#[cfg(target_os = "windows")]
mod win {
    use gpui::{Bounds, Pixels};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::Mutex;
    use win32::Win32::Foundation::{E_NOINTERFACE, E_POINTER, HWND, S_OK};
    use win32::Win32::Graphics::DirectComposition::{
        DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionRectangleClip,
        IDCompositionTarget, IDCompositionVisual2,
    };
    use win32::Win32::Graphics::Dxgi::IDXGISwapChain;
    use win32::core::{GUID, HRESULT, IUnknown, Interface};

    /// A window's tree of videos.
    struct Tree {
        device: IDCompositionDesktopDevice,
        _target: IDCompositionTarget,
        root: IDCompositionVisual2,
    }

    thread_local! {
        static TREES: RefCell<HashMap<isize, Tree>> = RefCell::new(HashMap::new());
    }

    fn tree(hwnd: HWND) -> win32::core::Result<Tree> {
        // SAFETY: a device of its own for a window that is open.
        unsafe {
            let device: IDCompositionDesktopDevice = DCompositionCreateDevice2(None::<&IUnknown>)?;
            // Not topmost: behind GPUI's own tree, which is.
            let target = device.CreateTargetForHwnd(hwnd, false)?;
            let root = device.CreateVisual()?;
            target.SetRoot(&root)?;
            device.Commit()?;
            Ok(Tree {
                device,
                _target: target,
                root,
            })
        }
    }

    /// The panel the kernel sees, laid out as `ISwapChainPanelNative`:
    /// `IUnknown`'s three methods, then `SetSwapChain`. The view owns it
    /// and keeps it until the kernel has let go of the surface.
    #[repr(C)]
    struct Panel {
        methods: &'static Methods,
        /// The swap chain the kernel gave last, until the view shows it.
        next: Mutex<Option<Option<IDXGISwapChain>>>,
    }

    #[repr(C)]
    struct Methods {
        query_interface:
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        set_swap_chain: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    }

    static METHODS: Methods = Methods {
        query_interface,
        add_ref,
        release,
        set_swap_chain,
    };

    unsafe extern "system" fn query_interface(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        // SAFETY: the caller passes an interface id and a place for the
        // answer.
        unsafe {
            if !iid.is_null() && *iid == IUnknown::IID {
                *out = this;
                return S_OK;
            }
            *out = std::ptr::null_mut();
        }
        E_NOINTERFACE
    }

    unsafe extern "system" fn add_ref(_: *mut c_void) -> u32 {
        1
    }

    unsafe extern "system" fn release(_: *mut c_void) -> u32 {
        1
    }

    unsafe extern "system" fn set_swap_chain(this: *mut c_void, chain: *mut c_void) -> HRESULT {
        // SAFETY: `this` is a panel of a view that is attached; the chain
        // is a swap chain or null, which the panel takes a reference to.
        let (panel, chain) = unsafe {
            (
                &*(this as *const Panel),
                IDXGISwapChain::from_raw_borrowed(&chain).cloned(),
            )
        };
        if let Ok(mut next) = panel.next.lock() {
            *next = Some(chain);
        }
        crate::host::redraw();
        S_OK
    }

    /// A view's visual and its panel.
    pub(super) struct Surface {
        hwnd: isize,
        visual: IDCompositionVisual2,
        clip: IDCompositionRectangleClip,
        panel: Box<Panel>,
    }

    impl Surface {
        pub(super) fn new(window: &gpui::Window) -> Option<Self> {
            let RawWindowHandle::Win32(handle) =
                HasWindowHandle::window_handle(window).ok()?.as_raw()
            else {
                return None;
            };
            let hwnd = handle.hwnd.get();
            TREES.with(|trees| {
                let mut trees = trees.borrow_mut();
                let tree = match trees.entry(hwnd) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        match tree(HWND(hwnd as *mut c_void)) {
                            Ok(tree) => entry.insert(tree),
                            Err(error) => {
                                eprintln!("bern: the window's videos cannot be composed: {error}");
                                return None;
                            }
                        }
                    }
                };
                // SAFETY: objects of the tree's device, on its thread.
                unsafe {
                    let visual = tree.device.CreateVisual().ok()?;
                    let clip = tree.device.CreateRectangleClip().ok()?;
                    visual.SetClip(&clip).ok()?;
                    tree.root.AddVisual(&visual, true, None).ok()?;
                    tree.device.Commit().ok()?;
                    Some(Surface {
                        hwnd,
                        visual,
                        clip,
                        panel: Box::new(Panel {
                            methods: &METHODS,
                            next: Mutex::new(None),
                        }),
                    })
                }
            })
        }

        /// The panel's address, for the kernel.
        pub(super) fn handle(&self) -> u64 {
            &*self.panel as *const Panel as u64
        }

        /// Stand at `bounds` (points) of the window, with rounded corners,
        /// showing the kernel's newest swap chain.
        pub(super) fn place(&self, bounds: Bounds<Pixels>, corner: f32, scale: f32) {
            TREES.with(|trees| {
                let trees = trees.borrow();
                let Some(tree) = trees.get(&self.hwnd) else {
                    return;
                };
                let next = self.panel.next.lock().ok().and_then(|mut next| next.take());
                let pixels = |value: Pixels| (f32::from(value) * scale).round();
                let (x, y) = (pixels(bounds.origin.x), pixels(bounds.origin.y));
                let (width, height) = (pixels(bounds.size.width), pixels(bounds.size.height));
                let radius = corner * scale;
                // SAFETY: objects of the tree's device, on its thread.
                unsafe {
                    if let Some(chain) = next {
                        let _ = match chain {
                            Some(chain) => self.visual.SetContent(&chain),
                            None => self.visual.SetContent(None::<&IUnknown>),
                        };
                    }
                    let _ = self.visual.SetOffsetX2(x);
                    let _ = self.visual.SetOffsetY2(y);
                    let _ = self.clip.SetLeft2(0.0);
                    let _ = self.clip.SetTop2(0.0);
                    let _ = self.clip.SetRight2(width);
                    let _ = self.clip.SetBottom2(height);
                    let _ = self.clip.SetTopLeftRadiusX2(radius);
                    let _ = self.clip.SetTopLeftRadiusY2(radius);
                    let _ = self.clip.SetTopRightRadiusX2(radius);
                    let _ = self.clip.SetTopRightRadiusY2(radius);
                    let _ = self.clip.SetBottomLeftRadiusX2(radius);
                    let _ = self.clip.SetBottomLeftRadiusY2(radius);
                    let _ = self.clip.SetBottomRightRadiusX2(radius);
                    let _ = self.clip.SetBottomRightRadiusY2(radius);
                    let _ = tree.device.Commit();
                }
            });
        }

        pub(super) fn remove(self) {
            TREES.with(|trees| {
                if let Some(tree) = trees.borrow().get(&self.hwnd) {
                    // SAFETY: the visual is the root's child.
                    unsafe {
                        let _ = tree.root.RemoveVisual(&self.visual);
                        let _ = tree.device.Commit();
                    }
                }
            });
        }
    }
}
