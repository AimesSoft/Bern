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
use crate::protocol::{Reader, Type, Value, Writer, decode, encode};
use bern_ffi::media_contract::kernel::{self, FrameSink, SurfaceHandle, SurfaceKind, VideoFrame};
use bern_ffi::media_contract::{MediaCommand, Observation, PLAYER_FIELDS, Phase, media_catalog};
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

thread_local! {
    /// Each player's state as the media hub last reported it, by id: the
    /// fields of its `MediaPlayer` record.
    static PLAYERS: RefCell<HashMap<String, Vec<Value>>> = RefCell::new(HashMap::new());
    static PLAYERS_REVISION: Cell<u64> = const { Cell::new(0) };
    /// A declaration read a player's state, so its changes redraw.
    static PLAYERS_READ: Cell<bool> = const { Cell::new(false) };
}

/// Take the media hub's state (`on_media_state`, `bern_media_snapshot`):
/// its revision, then every player. Whether the windows should render
/// again.
pub(crate) fn receive_state(bytes: &[u8]) -> bool {
    let mut reader = Reader::new(bytes);
    let (Ok(revision), Ok(count)) = (reader.u64(), reader.u32()) else {
        return false;
    };
    if revision < PLAYERS_REVISION.with(Cell::get) {
        return false;
    }
    let (types, _) = media_catalog();
    let record = Type::Record {
        name: "MediaPlayer".into(),
    };
    let mut players = HashMap::new();
    for _ in 0..count {
        let Ok(Value::Record(fields)) = decode(&record, &types, &mut reader) else {
            return false;
        };
        if let Some(Value::Text(id)) = fields.first() {
            players.insert(id.clone(), fields);
        }
    }
    PLAYERS_REVISION.with(|current| current.set(revision));
    PLAYERS.with(|current| *current.borrow_mut() = players);
    PLAYERS_READ.with(Cell::get)
}

/// `players.<player>.<field>` in a declaration, from the player's last
/// reported state; before any, the state of a player that has loaded
/// nothing.
pub(crate) fn player_field(player: &str, field: &str) -> Value {
    PLAYERS_READ.with(|read| read.set(true));
    PLAYERS.with(|players| {
        let players = players.borrow();
        let fields = players.get(player);
        let at = |name: &str| {
            let index = PLAYER_FIELDS
                .iter()
                .position(|(field, ..)| *field == name)?;
            fields?.get(index)
        };
        let int = |name: &str| match at(name)? {
            Value::Int(value) => Some(*value),
            Value::Optional(Some(value)) => match **value {
                Value::Int(value) => Some(value),
                _ => None,
            },
            _ => None,
        };
        let flag = |name: &str| Value::Bool(matches!(at(name), Some(Value::Bool(true))));
        let phase = match at("phase") {
            Some(Value::Enum { variant, .. }) => Phase::from_index(*variant),
            _ => None,
        };
        let seconds = |ms: Option<i64>| ms.map_or(0.0, |ms| ms as f64 / 1000.0);
        let position = seconds(int("position_ms"));
        let duration = seconds(int("duration_ms")).max(0.0);
        match field {
            "position" => Value::Float(position),
            "duration" => Value::Float(duration),
            "progress" if duration > 0.0 => Value::Float((position / duration).clamp(0.0, 1.0)),
            "progress" => Value::Float(0.0),
            "position_ms" => Value::Int(int("position_ms").unwrap_or(0)),
            "duration_ms" => match int("duration_ms") {
                Some(duration) => Value::some(Value::Int(duration)),
                None => Value::none(),
            },
            "playing" => Value::Bool(phase == Some(Phase::Playing)),
            "paused" => Value::Bool(phase == Some(Phase::Paused)),
            "buffering" => Value::Bool(matches!(phase, Some(Phase::Loading | Phase::Buffering))),
            "ended" => Value::Bool(phase == Some(Phase::Ended)),
            "failed" => Value::Bool(phase == Some(Phase::Failed)),
            "seekable" | "muted" => flag(field),
            "volume" => match at("volume") {
                Some(Value::Float(volume)) => Value::Float(*volume),
                _ => Value::Float(Observation::idle().volume),
            },
            _ => Value::Unit,
        }
    })
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
    /// Show Bern's standard GPUI playback control bar (`controls: standard`).
    pub controls: bool,
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
    if KIND.with(Cell::get).is_none() {
        crate::media_executor::attach();
    }
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
    (KIND.with(Cell::get).is_some_and(under) || crate::media_executor::uses_underlay())
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
        if KIND.with(Cell::get).is_some() {
            clock::stop(token);
            let mut writer = Writer::new();
            writer.text(token);
            post(bern_ffi::media::bern_media_surface_detach, &writer.bytes);
        } else {
            crate::media_executor::remove_view(token);
        }
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

fn format_media_time(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds.floor() as u64
    } else {
        0
    };
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

thread_local! {
    static SEEK_BOUNDS: RefCell<HashMap<String, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

fn control_bar(c: &Ctx, id: &'static str, player: &str) -> El {
    use gpui::{InteractiveElement, MouseButton, StatefulInteractiveElement};
    use std::rc::Rc;

    let playing = crate::value::truthy(&player_field(player, "playing"));
    let muted = crate::value::truthy(&player_field(player, "muted"));
    let seekable = crate::value::truthy(&player_field(player, "seekable"));
    let position = crate::value::as_f64(&player_field(player, "position"));
    let duration = crate::value::as_f64(&player_field(player, "duration"));
    let progress = if duration > 0.0 {
        (position / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let hidden = playing && c.win().idle();

    let player_toggle = player.to_string();
    let toggle_play: crate::ctx::Press = {
        let c = c.clone();
        Rc::new(move |_| {
            let op = if playing {
                MediaOp::Pause
            } else {
                MediaOp::Play
            };
            media(&c, op, &player_toggle, Vec::new());
        })
    };
    let player_mute = player.to_string();
    let toggle_mute: crate::ctx::Press = {
        let c = c.clone();
        Rc::new(move |_| {
            media(
                &c,
                MediaOp::SetMuted,
                &player_mute,
                vec![Value::Bool(!muted)],
            );
        })
    };
    let player_seek = player.to_string();
    let do_seek: Rc<dyn Fn(f64)> = {
        let c = c.clone();
        Rc::new(move |ms: f64| {
            let target = ms.max(0.0).round() as i64;
            media(&c, MediaOp::Seek, &player_seek, vec![Value::Int(target)]);
        })
    };

    c.list(crate::session::Control {
        target: c.target(&format!("{id}.play_pause")),
        window: c.win().id.clone(),
        role: "button",
        label: if playing { "Pause" } else { "Play" }.to_string(),
        value: Some(if playing { "playing" } else { "paused" }.to_string()),
        enabled: c.enabled(),
        actions: crate::session::Actions {
            activate: Some(Rc::clone(&toggle_play)),
            ..crate::session::Actions::default()
        },
    });
    c.list(crate::session::Control {
        target: c.target(&format!("{id}.seek")),
        window: c.win().id.clone(),
        role: "slider",
        label: "Seek".to_string(),
        value: Some(format!("{}", (position * 1000.0).round() as i64)),
        enabled: c.enabled() && seekable,
        actions: crate::session::Actions {
            set_number: Some(Rc::clone(&do_seek)),
            ..crate::session::Actions::default()
        },
    });
    c.list(crate::session::Control {
        target: c.target(&format!("{id}.mute")),
        window: c.win().id.clone(),
        role: "button",
        label: if muted { "Unmute" } else { "Mute" }.to_string(),
        value: Some(muted.to_string()),
        enabled: c.enabled(),
        actions: crate::session::Actions {
            activate: Some(Rc::clone(&toggle_mute)),
            ..crate::session::Actions::default()
        },
    });

    if hidden {
        return div().into_any_element();
    }

    let fg = gpui::rgba(0xffffffff);
    let muted_fg = gpui::rgba(0xffffffb8);
    let hover_bg = gpui::rgba(0xffffff22);

    let play_icon = if playing { "pause.fill" } else { "play.fill" };
    let mute_icon = if muted {
        "speaker.slash.fill"
    } else {
        "speaker.wave.3.fill"
    };

    let seek_key = format!("{}/{id}.seek", c.win().id);
    let seek_key_probe = seek_key.clone();
    let seek_key_down = seek_key;
    let seek_cb = Rc::clone(&do_seek);
    let duration_ms = (duration * 1000.0).max(0.0);

    let scrubber_ir =
        bern_surface::controls::media_scrubber(bern_surface::controls::MediaScrubberSpec {
            fraction: progress as f32,
            enabled: seekable,
            tint: Some("accent"),
        });
    let resolved_scrubber = match &scrubber_ir {
        bern_surface::SurfaceNode::Track(track) => {
            crate::surface::ResolvedTrack::from_node(c, track)
        }
        _ => unreachable!("media_scrubber always lowers to TrackNode"),
    };
    let (scrubber_rail, scrubber_thumb) = resolved_scrubber.build_visual(false);

    let seek_track = div()
        .id(c.element_id(&format!("{id}.seek_track")))
        .relative()
        .flex()
        .items_center()
        .flex_1()
        .h(crate::units::px(resolved_scrubber.hit_extent))
        .cursor_pointer()
        .child(
            gpui::canvas(
                move |bounds, _, _| {
                    SEEK_BOUNDS.with(|map| {
                        map.borrow_mut().insert(seek_key_probe.clone(), bounds);
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(scrubber_rail)
        .children(scrubber_thumb)
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            cx.stop_propagation();
            if !seekable || duration_ms <= 0.0 {
                return;
            }
            let bounds = SEEK_BOUNDS.with(|map| map.borrow().get(&seek_key_down).copied());
            if let Some(bounds) = bounds {
                let w = f32::from(bounds.size.width).max(1.0);
                let frac = ((f32::from(event.position.x) - f32::from(bounds.origin.x)) / w)
                    .clamp(0.0, 1.0);
                seek_cb(f64::from(frac) * duration_ms);
            }
        });

    let bar_ir = bern_surface::controls::media_bar(bern_surface::controls::MediaBarSpec);
    crate::surface::as_box(&bar_ir)
        .map(|b| crate::surface::apply_box(c, div(), b))
        .unwrap_or_else(div)
        .absolute()
        .bottom(crate::units::px(12.0))
        .left(crate::units::px(12.0))
        .right(crate::units::px(12.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .id(c.element_id(&format!("{id}.play_btn")))
                .flex()
                .items_center()
                .justify_center()
                .size(crate::units::px(28.0))
                .rounded(crate::units::px(6.0))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .child(crate::icons::icon(play_icon, 14.0, fg))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    toggle_play(cx);
                }),
        )
        .child(
            div()
                .text_size(crate::units::px(11.5))
                .text_color(muted_fg)
                .child(format_media_time(position)),
        )
        .child(seek_track)
        .child(
            div()
                .text_size(crate::units::px(11.5))
                .text_color(muted_fg)
                .child(format_media_time(duration)),
        )
        .child(
            div()
                .id(c.element_id(&format!("{id}.mute_btn")))
                .flex()
                .items_center()
                .justify_center()
                .size(crate::units::px(28.0))
                .rounded(crate::units::px(6.0))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .child(crate::icons::icon(mute_icon, 14.0, fg))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    toggle_mute(cx);
                }),
        )
        .into_any_element()
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
        let painted_player = player.clone();
        gpui::canvas(
            move |bounds, window, _| {
                place(&token, &window_id, &player, bounds, corner, fit, window)
            },
            move |bounds, _, window, _| {
                paint(&painted, &painted_player, bounds, corner, fit, window)
            },
        )
        .absolute()
        .size_full()
    };
    let mut element = div()
        .relative()
        .flex()
        .min_w(gpui::px(0.0))
        .min_h(gpui::px(0.0))
        .rounded(crate::units::px(props.corner))
        .child(probe);
    if props.controls {
        let toggle_player = player.clone();
        let toggle_ctx = c.clone();
        let click_layer = {
            use gpui::{InteractiveElement, StatefulInteractiveElement};
            div()
                .id(c.element_id(&format!("{id}.surface_toggle")))
                .absolute()
                .inset_0()
                .on_click(move |_, _, _| {
                    let is_playing = crate::value::truthy(&player_field(&toggle_player, "playing"));
                    let op = if is_playing {
                        MediaOp::Pause
                    } else {
                        MediaOp::Play
                    };
                    media(&toggle_ctx, op, &toggle_player, Vec::new());
                })
        };
        element = element
            .child(click_layer)
            .child(control_bar(c, id, &player));
    }
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
        if !crate::media_executor::active() {
            log(|| format!("{token}: no kernel or built-in executor"));
            return;
        }
        if crate::media_executor::uses_underlay() {
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
        let first = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            let view = views.get_mut(token)?;
            Some(view.size.replace(size).is_none())
        });
        if first == Some(true) {
            let mut view = Writer::new();
            view.text(token);
            view.u8(1);
            view.text(player);
            view.text(window_id);
            post(bern_ffi::media::bern_media_view, &view.bytes);
        }
        crate::media_executor::place_view(token, player, window, bounds, corner, fit);
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

/// Compute destination rect inside `bounds` for a `frame` size under `fit`
/// (`0` = contain, `1` = cover, `2` = fill).
fn fit_bounds(bounds: Bounds<Pixels>, image: &RenderImage, fit: u32) -> Bounds<Pixels> {
    if fit == 2 {
        return bounds;
    }
    let img = image.size(0);
    let iw = img.width.0.max(1) as f32;
    let ih = img.height.0.max(1) as f32;
    let bw = f32::from(bounds.size.width).max(1.0);
    let bh = f32::from(bounds.size.height).max(1.0);
    let scale = if fit == 1 {
        (bw / iw).max(bh / ih)
    } else {
        (bw / iw).min(bh / ih)
    };
    let w = iw * scale;
    let h = ih * scale;
    Bounds::new(
        gpui::point(
            bounds.origin.x + gpui::px((bw - w) * 0.5),
            bounds.origin.y + gpui::px((bh - h) * 0.5),
        ),
        gpui::size(gpui::px(w), gpui::px(h)),
    )
}

/// Paint the view `token`: nothing over a native surface/underlay, else black
/// with the newest picture over it.
fn paint(
    token: &str,
    player: &str,
    bounds: Bounds<Pixels>,
    corner: f32,
    fit: u32,
    window: &mut gpui::Window,
) {
    for gone in DROPPED.with(|dropped| std::mem::take(&mut *dropped.borrow_mut())) {
        let _ = window.drop_image(gone);
    }
    if KIND.with(Cell::get).is_some_and(under) || crate::media_executor::uses_underlay() {
        return;
    }
    let corners = gpui::Corners::all(gpui::px(corner));
    window.paint_quad(gpui::fill(bounds, gpui::black()).corner_radii(corners));
    let shown = if KIND.with(Cell::get).is_some() {
        VIEWS.with(
            |views| match views.borrow_mut().get_mut(token)?.surface.as_mut()? {
                Surface::Pictures(pictures) => pictures.shown(window),
                #[allow(unreachable_patterns)]
                _ => None,
            },
        )
    } else {
        crate::media_executor::take_frame(player, window)
    };
    if let Some(image) = shown {
        let dest = fit_bounds(bounds, &image, fit);
        let _ = window.paint_image(dest, bounds, corners, image, 0, false);
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
    #[cfg(target_os = "macos")]
    static REFRESH_HZ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(120);

    pub(super) fn start(token: &str) {
        #[cfg(target_os = "macos")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let hz = objc2_app_kit::NSScreen::screens(mtm)
                .iter()
                .map(|screen| screen.maximumFramesPerSecond().max(60) as u32)
                .max()
                .unwrap_or(120);
            REFRESH_HZ.store(hz, std::sync::atomic::Ordering::Relaxed);
        }
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

    fn refresh_interval() -> f64 {
        #[cfg(target_os = "macos")]
        {
            1.0 / f64::from(
                REFRESH_HZ
                    .load(std::sync::atomic::Ordering::Relaxed)
                    .max(60),
            )
        }
        #[cfg(target_os = "windows")]
        {
            #[repr(C)]
            struct DevModeW {
                device_name: [u16; 32],
                spec_version: u16,
                driver_version: u16,
                size: u16,
                driver_extra: u16,
                fields: u32,
                position_union: [u8; 16],
                printer_shorts: [i16; 5],
                form_name: [u16; 32],
                log_pixels: u16,
                bits_per_pel: u32,
                pels_width: u32,
                pels_height: u32,
                display_flags: u32,
                display_frequency: u32,
                tail: [u32; 8],
            }
            const _: () = assert!(
                std::mem::size_of::<DevModeW>() == 220
                    && std::mem::offset_of!(DevModeW, display_frequency) == 184
            );
            #[link(name = "user32")]
            unsafe extern "system" {
                fn EnumDisplaySettingsW(
                    device: *const u16,
                    mode_num: u32,
                    dev_mode: *mut DevModeW,
                ) -> i32;
            }
            // SAFETY: zeroed 220-byte `DEVMODEW` buffer with `dmSize` initialized.
            unsafe {
                let mut mode: DevModeW = std::mem::zeroed();
                mode.size = std::mem::size_of::<DevModeW>() as u16;
                if EnumDisplaySettingsW(std::ptr::null(), u32::MAX, &mut mode) != 0
                    && mode.display_frequency > 1
                {
                    return 1.0 / f64::from(mode.display_frequency);
                }
            }
            1.0 / 60.0
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            1.0 / 120.0
        }
    }

    fn run() {
        #[cfg(target_os = "windows")]
        let (dcomp_boost, dcomp_wait, timer) = {
            use std::ffi::c_void;
            type BoostFn = unsafe extern "system" fn(i32) -> i32;
            type WaitFn = unsafe extern "system" fn(u32, *const *mut c_void, u32) -> i32;
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn LoadLibraryA(name: *const u8) -> *mut c_void;
                fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
                fn CreateWaitableTimerExW(
                    attrs: *const c_void,
                    name: *const u16,
                    flags: u32,
                    access: u32,
                ) -> *mut c_void;
            }
            // SAFETY: null-terminated ASCII module/export names and standard Win32 waitable timer creation.
            unsafe {
                let module = LoadLibraryA(c"dcomp.dll".as_ptr().cast::<u8>());
                let boost = if module.is_null() {
                    None
                } else {
                    let p =
                        GetProcAddress(module, c"DCompositionBoostCompositorClock".as_ptr().cast());
                    (!p.is_null()).then(|| std::mem::transmute::<*const c_void, BoostFn>(p))
                };
                let wait = if module.is_null() {
                    None
                } else {
                    let p = GetProcAddress(
                        module,
                        c"DCompositionWaitForCompositorClock".as_ptr().cast(),
                    );
                    (!p.is_null()).then(|| std::mem::transmute::<*const c_void, WaitFn>(p))
                };
                let timer = CreateWaitableTimerExW(
                    std::ptr::null(),
                    std::ptr::null(),
                    0x0000_0002,
                    0x001F_0003,
                );
                (boost, wait, timer)
            }
        };
        #[cfg(target_os = "windows")]
        let mut boosted = false;
        #[cfg(target_os = "windows")]
        let mut short_waits = 0u32;

        loop {
            let active_count = TOKENS.lock().map(|t| t.len()).unwrap_or(0);
            if active_count == 0 {
                #[cfg(target_os = "windows")]
                if boosted {
                    if let Some(boost) = dcomp_boost {
                        // SAFETY: disables this thread's previous boost.
                        unsafe {
                            let _ = boost(0);
                        }
                    }
                    boosted = false;
                }
                std::thread::sleep(std::time::Duration::from_millis(16));
                continue;
            }
            let interval = refresh_interval();
            #[cfg(target_os = "windows")]
            {
                if !boosted {
                    if let Some(boost) = dcomp_boost {
                        // SAFETY: requests high-refresh compositor clock while surfaces are active.
                        unsafe {
                            let _ = boost(1);
                        }
                    }
                    boosted = true;
                }
                let start = now();
                let synced = dcomp_wait
                    .map(|wait| unsafe { wait(0, std::ptr::null(), 80) >= 0 })
                    .unwrap_or(false);
                let elapsed = now() - start;
                if synced && elapsed >= 0.0001 {
                    short_waits = 0;
                } else if !synced || {
                    short_waits += 1;
                    short_waits >= 4
                } {
                    #[link(name = "kernel32")]
                    unsafe extern "system" {
                        fn SetWaitableTimer(
                            timer: *mut std::ffi::c_void,
                            due: *const i64,
                            period: i32,
                            routine: *const std::ffi::c_void,
                            arg: *const std::ffi::c_void,
                            resume: i32,
                        ) -> i32;
                        fn WaitForSingleObject(handle: *mut std::ffi::c_void, ms: u32) -> u32;
                    }
                    if !timer.is_null() {
                        let due = -((interval * 1e7) as i64).max(1);
                        // SAFETY: valid waitable timer handle created above.
                        unsafe {
                            if SetWaitableTimer(
                                timer,
                                &due,
                                0,
                                std::ptr::null(),
                                std::ptr::null(),
                                0,
                            ) != 0
                            {
                                let _ = WaitForSingleObject(timer, 100);
                            } else {
                                std::thread::sleep(std::time::Duration::from_secs_f64(interval));
                            }
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_secs_f64(interval));
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::thread::sleep(std::time::Duration::from_secs_f64(interval));
            }
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
