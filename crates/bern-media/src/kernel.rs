//! A media kernel the application brings.
//!
//! An application whose engine plays media with a kernel of its own — a
//! decoder and renderer written in Rust, for example — installs it before
//! the app starts. The kernel then executes the hub's players in place of
//! the platform's media facilities, and each `Video` view of a target that
//! can host it gives the kernel a native surface to draw into, the
//! surface's size, and the display's frame clock. The application never
//! creates or holds a surface: the view owns it, and the kernel draws into
//! it from `attach_surface` until `detach_surface` returns.
//!
//! The standard media commands reach the kernel through the hub like any
//! executor, so the UI, the CLI, JSON clients and the MCP server keep
//! addressing one player. What the kernel offers beyond them (subtitle
//! tracks, overlays, filters) is the application's own business: its
//! engine holds the kernel and calls it directly.

use crate::{MediaCapabilities, MediaCommand, MediaExecutor, MediaHub, Observation};
use bern_protocol::{CommandError, Value};
use std::sync::{Arc, Mutex};

/// The hub a kernel executes for: where it completes requests, reports
/// what it observes, and sends the commands its own application asks of
/// it, so they are validated and versioned like every other entry's.
pub trait KernelHub: Send + Sync {
    /// Run a media command, as [`MediaHub::dispatch`].
    fn dispatch(
        &self,
        command: MediaCommand,
        params: Vec<Value>,
        reply: Box<dyn FnOnce(Result<Value, CommandError>) + Send>,
    );
    /// The kernel finished request `request`, as [`MediaHub::complete`].
    fn complete(&self, request: u64, result: Result<(), CommandError>);
    /// What the kernel observed for `player` while it ran source version
    /// `source_version`, as [`MediaHub::observe`].
    fn observe(&self, player: &str, source_version: u64, observation: Observation);
}

impl KernelHub for MediaHub {
    fn dispatch(
        &self,
        command: MediaCommand,
        params: Vec<Value>,
        reply: Box<dyn FnOnce(Result<Value, CommandError>) + Send>,
    ) {
        MediaHub::dispatch(self, command, params, reply);
    }

    fn complete(&self, request: u64, result: Result<(), CommandError>) {
        MediaHub::complete(self, request, result);
    }

    fn observe(&self, player: &str, source_version: u64, observation: Observation) {
        MediaHub::observe(self, player, source_version, observation);
    }
}

/// A kind of native surface a video view can give a kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    /// A `CAMetalLayer`: macOS, iOS and tvOS.
    MetalLayer,
}

impl SurfaceKind {
    pub const ALL: &[SurfaceKind] = &[SurfaceKind::MetalLayer];

    /// Code of the kind in the C ABI.
    pub fn code(self) -> u32 {
        match self {
            SurfaceKind::MetalLayer => 0,
        }
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.code() == code)
    }
}

/// The native surface itself, as an address the view keeps alive until
/// [`MediaKernel::detach_surface`] returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceHandle {
    /// The address of a `CAMetalLayer`. The view sets nothing on it after
    /// attaching it but its frame and corner; the kernel sets its drawable
    /// size, pixel format and colour space.
    MetalLayer(u64),
}

impl SurfaceHandle {
    pub fn kind(self) -> SurfaceKind {
        match self {
            SurfaceHandle::MetalLayer(_) => SurfaceKind::MetalLayer,
        }
    }
}

/// How large a surface is on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceSize {
    /// Physical pixels, at least 1 each.
    pub width: u32,
    pub height: u32,
    /// Pixels per point of the display, times any scale the layout applies
    /// around the view.
    pub scale: f64,
}

/// How the declaration asks the picture to fill its surface (`Video`'s
/// `fit`). A kernel that draws only one way draws `Contain`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoFit {
    /// The whole picture, letterboxed.
    #[default]
    Contain,
    /// The whole surface, cropping the picture.
    Cover,
    /// The whole surface, stretching the picture.
    Fill,
}

impl VideoFit {
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => VideoFit::Cover,
            2 => VideoFit::Fill,
            _ => VideoFit::Contain,
        }
    }
}

/// A surface a video view gives the kernel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoSurface {
    pub handle: SurfaceHandle,
    pub size: SurfaceSize,
    pub fit: VideoFit,
}

/// One frame of a surface's display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameTiming {
    /// When the frame clock fired, in seconds of the host's monotonic clock
    /// (`CACurrentMediaTime` on Apple platforms).
    pub now: f64,
    /// When the frame being prepared will be on screen, same clock.
    pub target: f64,
}

/// A media kernel: it plays the players the hub routes to it, as any
/// [`MediaExecutor`], and draws their video into the surfaces video views
/// give it.
///
/// Every method returns quickly; decoding and drawing happen on the
/// kernel's own threads. Calls arrive on different threads: requests on
/// the thread that dispatched them, surfaces on the UI thread, frames on
/// the frame clock's thread.
pub trait MediaKernel: MediaExecutor {
    /// Identity and abilities, reported to the hub when the kernel attaches.
    fn capabilities(&self) -> MediaCapabilities;
    /// The kinds of surface the kernel can draw into on this platform. A
    /// target that offers none of them keeps its own media facilities.
    fn surfaces(&self) -> Vec<SurfaceKind>;
    /// The kernel now executes for `hub`. Players are idle until a source
    /// is loaded.
    fn attached(&self, hub: Arc<dyn KernelHub>);
    /// The kernel no longer executes for the hub; it stops every player.
    fn detached(&self) {}
    /// A video view shows `player` through `surface`. `view` names the
    /// attachment until it is detached. A player shows its video in the
    /// view attached last when the kernel cannot draw it twice.
    fn attach_surface(&self, view: &str, player: &str, surface: VideoSurface);
    /// The surface of `view` changed size.
    fn resize_surface(&self, view: &str, size: SurfaceSize);
    /// The view is going away. When this returns the kernel no longer uses
    /// the surface.
    fn detach_surface(&self, view: &str);
    /// The display of `view` prepares a new frame.
    fn frame(&self, view: &str, timing: FrameTiming);
}

static KERNEL: Mutex<Option<Arc<dyn MediaKernel>>> = Mutex::new(None);

/// Make `kernel` the application's media kernel. Called before the app
/// starts, typically while the engine is created; a later call replaces it
/// for apps started afterwards.
pub fn install(kernel: Arc<dyn MediaKernel>) {
    *KERNEL.lock().expect("media kernel lock") = Some(kernel);
}

/// Forget the installed kernel; apps started afterwards use the platform's
/// media facilities.
pub fn clear() {
    *KERNEL.lock().expect("media kernel lock") = None;
}

/// The kernel the application installed, if any.
pub fn installed() -> Option<Arc<dyn MediaKernel>> {
    KERNEL.lock().expect("media kernel lock").clone()
}

/// The installed kernel when it can draw into one of the surface kinds a
/// target `offers`, with the first kind both support.
pub fn for_target(offers: &[SurfaceKind]) -> Option<(Arc<dyn MediaKernel>, SurfaceKind)> {
    let kernel = installed()?;
    let supported = kernel.surfaces();
    let kind = offers
        .iter()
        .copied()
        .find(|kind| supported.contains(kind))?;
    Some((kernel, kind))
}
