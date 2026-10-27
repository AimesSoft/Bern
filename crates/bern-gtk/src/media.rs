//! Video: the media executor and the `Video` view with its standard,
//! command-driven control bar.
//!
//! The executor plays each player with one `GtkMediaFile` (GTK's media
//! backend, GStreamer on Linux) and reports what the stream does as
//! observations, never as commands. Views present a player's stream with
//! a `GtkPicture`; the control bar sends the same media commands every
//! other entry uses, and shows the hub's merged state.
//!
//! When the application brings a media kernel that draws into frame sinks,
//! the kernel executes the players instead and views show its pictures
//! (`kernel`).

use crate::controls::{self, Actions};
use crate::host::{host, status, take, with_bytes};
use crate::protocol::{Type, Value, Writer, encode};
use crate::reactive::{Cell, effect, on_cleanup, untrack};
use crate::text::tr;
use crate::value::{as_f64, number};
use crate::widgets::plain_row;
use crate::{Ctx, Get, Parent, Str};
use adw::prelude::*;
use bern_ffi::media_contract::kernel::{SurfaceKind, VideoFit};
use bern_ffi::media_contract::{
    ExecOp, ExecRequest, MediaCommand, Observation, Phase, media_catalog,
};
use std::cell::{Cell as StdCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// The standard media commands declarations use on players.
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

/// One player of the executor. Each source gets a new media stream: a
/// stream reused for another file keeps the first file's duration.
struct Stream {
    id: String,
    file: RefCell<gtk::MediaFile>,
    /// Pictures presenting this player, which follow its current stream.
    pictures: RefCell<Vec<glib::WeakRef<gtk::Picture>>>,
    version: StdCell<u64>,
    loaded: StdCell<bool>,
    started: StdCell<bool>,
    last_report: StdCell<Option<Instant>>,
    /// The position last reported, in microseconds.
    last_position: StdCell<i64>,
}

impl Stream {
    fn new(id: &str) -> Rc<Self> {
        let stream = Rc::new(Self {
            id: id.to_string(),
            file: RefCell::new(gtk::MediaFile::new()),
            pictures: RefCell::new(Vec::new()),
            version: StdCell::new(0),
            loaded: StdCell::new(false),
            started: StdCell::new(false),
            last_report: StdCell::new(None),
            last_position: StdCell::new(0),
        });
        stream.watch(&stream.file());
        stream
    }

    fn file(&self) -> gtk::MediaFile {
        self.file.borrow().clone()
    }

    /// Report what `file` does while it is this player's stream.
    fn watch(self: &Rc<Self>, file: &gtk::MediaFile) {
        for property in [
            "prepared", "playing", "ended", "error", "duration", "volume", "muted", "seekable",
            "seeking",
        ] {
            let watched = Rc::downgrade(self);
            file.connect_notify_local(Some(property), move |file, _| {
                if let Some(stream) = watched.upgrade()
                    && stream.file() == *file
                {
                    stream.report();
                }
            });
        }
        let ticking = Rc::downgrade(self);
        file.connect_notify_local(Some("timestamp"), move |file, _| {
            let Some(stream) = ticking.upgrade() else {
                return;
            };
            if stream.file() != *file {
                return;
            }
            // Playback reports twice a second; a jump, such as a finished
            // seek, reports at once.
            let jumped = (file.timestamp() - stream.last_position.get()).abs() > 750_000;
            let due = jumped
                || stream
                    .last_report
                    .get()
                    .is_none_or(|last| last.elapsed() >= Duration::from_millis(500));
            if due {
                stream.report();
            }
        });
    }

    /// Show this player in `picture`, now and after every new source.
    fn present(&self, picture: &gtk::Picture) {
        picture.set_paintable(Some(&self.file()));
        let mut pictures = self.pictures.borrow_mut();
        pictures.retain(|picture| picture.upgrade().is_some());
        pictures.push(picture.downgrade());
    }

    /// Replace the stream, keeping volume and mute, and move the pictures
    /// to the new one.
    fn replace(self: &Rc<Self>, file: gtk::MediaFile) {
        let old = self.file();
        file.set_volume(old.volume());
        file.set_muted(old.is_muted());
        old.pause();
        old.clear();
        self.watch(&file);
        *self.file.borrow_mut() = file.clone();
        for picture in self
            .pictures
            .borrow()
            .iter()
            .filter_map(|picture| picture.upgrade())
        {
            if picture.paintable().is_some() {
                picture.set_paintable(Some(&file));
            }
        }
    }

    fn phase(&self) -> (Phase, Option<String>) {
        let file = self.file();
        if !self.loaded.get() {
            (Phase::Idle, None)
        } else if let Some(error) = file.error() {
            (Phase::Failed, Some(summary(error.message())))
        } else if !file.is_prepared() {
            if file.is_playing() {
                (Phase::Buffering, None)
            } else {
                (Phase::Loading, None)
            }
        } else if file.is_ended() {
            (Phase::Ended, None)
        } else if file.is_playing() {
            (Phase::Playing, None)
        } else if self.started.get() {
            (Phase::Paused, None)
        } else {
            (Phase::Ready, None)
        }
    }

    fn report(&self) {
        let file = self.file();
        self.last_report.set(Some(Instant::now()));
        self.last_position.set(file.timestamp());
        let (phase, error) = self.phase();
        let duration = file.duration();
        let known = file.is_prepared() && duration > 0;
        let observation = Observation {
            phase,
            position_ms: (file.timestamp().max(0) / 1000),
            duration_ms: known.then_some(duration / 1000),
            seekable: known && file.is_seekable(),
            volume: (file.volume() * 1000.0).round() / 1000.0,
            muted: file.is_muted(),
            error,
        };
        let bytes = observation.encode(&self.id, self.version.get());
        with_bytes(&bytes, |ptr, len| {
            // SAFETY: the handle is live while the application runs.
            unsafe { bern_ffi::media::bern_media_event(host().app(), ptr, len) }
        });
    }

    fn load(self: &Rc<Self>, url: &str, version: u64) {
        self.version.set(version);
        self.started.set(false);
        self.replace(gtk::MediaFile::for_file(&gio::File::for_uri(url)));
        self.loaded.set(true);
        self.report();
    }

    fn unload(self: &Rc<Self>, version: u64) {
        self.version.set(version);
        self.started.set(false);
        self.replace(gtk::MediaFile::new());
        self.loaded.set(false);
        self.report();
    }
}

/// The first line of a media error, without GStreamer's element path; the
/// rest is debugging detail for the log.
fn summary(message: &str) -> String {
    let first = message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let text = match first.strip_prefix("Error from element ") {
        Some(rest) => rest.split_once(": ").map(|(_, text)| text).unwrap_or(rest),
        None => first,
    };
    text.trim().to_string()
}

/// The merged state of one player, from the hub.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlayerState {
    source: Option<String>,
    phase: u32,
    position_ms: i64,
    duration_ms: Option<i64>,
    seekable: bool,
    volume: f64,
    muted: bool,
    error: Option<String>,
}

impl PlayerState {
    fn from_value(value: &Value) -> Option<(String, Self)> {
        let Value::Record(fields) = value else {
            return None;
        };
        let text = |index: usize| match fields.get(index) {
            Some(Value::Text(text)) => text.clone(),
            _ => String::new(),
        };
        let int = |index: usize| match fields.get(index) {
            Some(Value::Int(value)) => *value,
            _ => 0,
        };
        let flag = |index: usize| matches!(fields.get(index), Some(Value::Bool(true)));
        let source = match fields.get(3) {
            Some(Value::Optional(Some(source))) => match source.as_ref() {
                Value::Record(source) => match source.first() {
                    Some(Value::Text(url)) => Some(url.clone()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let phase = match fields.get(5) {
            Some(Value::Enum { variant, .. }) => *variant,
            _ => Phase::Unavailable.index(),
        };
        let duration_ms = match fields.get(7) {
            Some(Value::Optional(Some(value))) => match value.as_ref() {
                Value::Int(value) => Some(*value),
                _ => None,
            },
            _ => None,
        };
        let error = match fields.get(11) {
            Some(Value::Optional(Some(value))) => match value.as_ref() {
                Value::Text(text) => Some(text.clone()),
                _ => None,
            },
            _ => None,
        };
        Some((
            text(0),
            Self {
                source,
                phase,
                position_ms: int(6),
                duration_ms,
                seekable: flag(8),
                volume: fields.get(9).map(as_f64).unwrap_or(1.0),
                muted: flag(10),
                error,
            },
        ))
    }

    fn is(&self, phase: Phase) -> bool {
        self.phase == phase.index()
    }

    fn playing(&self) -> bool {
        self.is(Phase::Playing) || self.is(Phase::Buffering)
    }

    fn playable(&self) -> bool {
        self.source.is_some()
            && !self.is(Phase::Idle)
            && !self.is(Phase::Failed)
            && !self.is(Phase::Unavailable)
    }

    fn seekable(&self) -> bool {
        self.playable() && self.seekable && self.duration_ms.is_some()
    }

    fn controllable(&self) -> bool {
        !self.is(Phase::Unavailable)
    }

    fn shows_frames(&self) -> bool {
        [
            Phase::Ready,
            Phase::Playing,
            Phase::Paused,
            Phase::Buffering,
            Phase::Ended,
        ]
        .iter()
        .any(|phase| self.is(*phase))
    }

    fn describe(&self) -> String {
        let name = Phase::from_index(self.phase)
            .map(Phase::name)
            .unwrap_or("Unavailable");
        let duration = self.duration_ms.map(time).unwrap_or_else(|| "--:--".into());
        format!("{name} {} / {duration}", time(self.position_ms))
    }
}

struct Media {
    streams: RefCell<HashMap<String, Rc<Stream>>>,
    players: Cell<Rc<HashMap<String, PlayerState>>>,
    revision: StdCell<u64>,
    views: StdCell<u64>,
    /// The application's media kernel executes the players.
    kernel: StdCell<bool>,
}

thread_local! {
    static MEDIA: Media = Media {
        streams: RefCell::new(HashMap::new()),
        players: Cell::new(Rc::new(HashMap::new())),
        revision: StdCell::new(0),
        views: StdCell::new(0),
        kernel: StdCell::new(false),
    };
}

fn stream(id: &str) -> Rc<Stream> {
    MEDIA.with(|media| {
        Rc::clone(
            media
                .streams
                .borrow_mut()
                .entry(id.to_string())
                .or_insert_with(|| Stream::new(id)),
        )
    })
}

/// Attach this frontend's executor to the hub and take the current state,
/// unless the application's media kernel executes the players.
pub(crate) fn attach_executor() {
    if attach_kernel() {
        return;
    }
    let mut writer = Writer::new();
    writer.text("gtk-mediafile");
    writer.text("1");
    writer.u8(1); // keeps playing without a view
    writer.u8(1); // several views may show one player
    let reply = with_bytes(&writer.bytes, |ptr, len| {
        // SAFETY: the handle is live while the application runs.
        take(unsafe { bern_ffi::media::bern_media_executor_attached(host().app(), ptr, len) })
    });
    if let Err(failure) = status(&reply) {
        eprintln!("bern: media executor: {}", failure.message);
    }
    // SAFETY: as above.
    receive_state(&take(unsafe {
        bern_ffi::media::bern_media_snapshot(host().app())
    }));
}

/// Offer the kernel frame sinks; when it takes them it executes every
/// player and video views show its pictures.
fn attach_kernel() -> bool {
    let sink = SurfaceKind::FrameSink.code();
    let mut offer = Writer::new();
    offer.u32(1);
    offer.u32(sink);
    let answer = with_bytes(&offer.bytes, |ptr, len| {
        // SAFETY: the handle is live while the application runs.
        take(unsafe { bern_ffi::media::bern_media_kernel_attach(host().app(), ptr, len) })
    });
    let mut reader = crate::protocol::Reader::new(&answer);
    if !matches!(reader.u8(), Ok(1)) || !matches!(reader.u32(), Ok(kind) if kind == sink) {
        return false;
    }
    MEDIA.with(|media| media.kernel.set(true));
    // SAFETY: as above.
    receive_state(&take(unsafe {
        bern_ffi::media::bern_media_snapshot(host().app())
    }));
    true
}

pub(crate) fn detach_executor() {
    // The kernel stops with the application; its views stop their frame
    // clocks when they go away.
    if MEDIA.with(|media| media.kernel.get()) {
        return;
    }
    let app = host().app();
    if !app.is_null() {
        // SAFETY: the handle is live until the application stops.
        unsafe { bern_ffi::media::bern_media_executor_detached(app) };
    }
}

/// Merged player states: revision, count, then `MediaPlayer` records.
pub(crate) fn receive_state(bytes: &[u8]) {
    let (types, _) = media_catalog();
    let player = Type::Record {
        name: "MediaPlayer".into(),
    };
    let mut reader = crate::protocol::Reader::new(bytes);
    let (Ok(revision), Ok(count)) = (reader.u64(), reader.u32()) else {
        return;
    };
    MEDIA.with(|media| {
        if revision < media.revision.get() {
            return;
        }
        media.revision.set(revision);
        let mut players = HashMap::new();
        for _ in 0..count {
            let Ok(value) = crate::protocol::decode(&player, &types, &mut reader) else {
                return;
            };
            if let Some((id, state)) = PlayerState::from_value(&value) {
                players.insert(id, state);
            }
        }
        media.players.set(Rc::new(players));
    });
    crate::snapshot::schedule();
}

fn complete(request: u64, result: Result<(), crate::Failure>) {
    let mut writer = Writer::new();
    writer.u64(request);
    match result {
        Ok(()) => writer.u8(0),
        Err(failure) => {
            writer.u8(1);
            writer.text(&failure.code);
            writer.text(&failure.message);
        }
    }
    with_bytes(&writer.bytes, |ptr, len| {
        // SAFETY: the handle is live while the application runs.
        unsafe { bern_ffi::media::bern_media_complete(host().app(), ptr, len) }
    });
}

/// Perform one hub request.
pub(crate) fn request(bytes: &[u8]) {
    let Ok(request) = ExecRequest::decode(bytes) else {
        return;
    };
    let id = request.request;
    match &request.op {
        ExecOp::Load { url } => {
            stream(&request.player).load(url, request.source_version);
            return complete(id, Ok(()));
        }
        ExecOp::Unload => {
            stream(&request.player).unload(request.source_version);
            return complete(id, Ok(()));
        }
        ExecOp::Release => {
            let removed = MEDIA.with(|media| media.streams.borrow_mut().remove(&request.player));
            if let Some(stream) = removed {
                stream.file().pause();
                stream.file().clear();
            }
            return complete(id, Ok(()));
        }
        _ => {}
    }
    let stream = stream(&request.player);
    if stream.version.get() != request.source_version {
        return complete(
            id,
            Err(crate::Failure::new(
                "stale_source",
                "a newer source is loaded",
            )),
        );
    }
    match request.op {
        ExecOp::Play => {
            if stream.file().is_ended() {
                stream.file().seek(0);
            }
            stream.started.set(true);
            stream.file().play();
            stream.report();
            if let Some(error) = stream.file().error() {
                eprintln!("bern: media {}: {}", stream.id, error.message());
                complete(
                    id,
                    Err(crate::Failure::new(
                        "media_failed",
                        summary(error.message()),
                    )),
                );
            } else {
                complete(id, Ok(()));
            }
        }
        ExecOp::Pause => {
            stream.file().pause();
            stream.report();
            complete(id, Ok(()));
        }
        ExecOp::Seek { position_ms } => {
            stream.file().seek(position_ms.max(0) * 1000);
            if !stream.file().is_seeking() {
                stream.report();
                return complete(id, Ok(()));
            }
            // Finish when the seek settles.
            let pending = Rc::new(RefCell::new(None::<glib::SignalHandlerId>));
            let handler = Rc::clone(&pending);
            let settled = stream
                .file()
                .connect_notify_local(Some("seeking"), move |file, _| {
                    if file.is_seeking() {
                        return;
                    }
                    if let Some(handler) = handler.borrow_mut().take() {
                        file.disconnect(handler);
                    }
                    complete(id, Ok(()));
                });
            *pending.borrow_mut() = Some(settled);
        }
        ExecOp::SetVolume { volume } => {
            stream.file().set_volume(volume.clamp(0.0, 1.0));
            complete(id, Ok(()));
        }
        ExecOp::SetMuted { muted } => {
            stream.file().set_muted(muted);
            complete(id, Ok(()));
        }
        _ => complete(
            id,
            Err(crate::Failure::new(
                "unknown_command",
                "unknown media operation",
            )),
        ),
    }
}

/// Dispatch a standard media command on `player` with its arguments after
/// the player id.
pub fn media(ctx: &Ctx, op: MediaOp, player: &str, args: Vec<Value>) {
    let (types, specs) = media_catalog();
    let command = op.command();
    let spec = &specs[command.index() as usize];
    let mut writer = Writer::new();
    let mut values = vec![Value::Text(player.to_string())];
    values.extend(args);
    for (value, param) in values.iter().zip(&spec.params) {
        encode(value, &param.ty, &types, &mut writer);
    }
    let host = host();
    let token = host.token(ctx.window_of(), None);
    with_bytes(&writer.bytes, |ptr, len| {
        // SAFETY: the handle is live while the application runs.
        unsafe {
            bern_ffi::media::bern_media_dispatch(host.app(), command.index(), ptr, len, token)
        }
    });
}

/// `players.<id>.open(source)`: `source` is a `MediaSource` value.
pub fn media_open(ctx: &Ctx, player: &str, source: Value) {
    media(ctx, MediaOp::Open, player, vec![source]);
}

fn time(milliseconds: i64) -> String {
    let seconds = milliseconds.max(0) / 1000;
    let (hours, minutes, rest) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{rest:02}")
    } else {
        format!("{minutes}:{rest:02}")
    }
}

#[derive(Default)]
pub struct VideoProps {
    pub id: &'static str,
    /// A `MediaSource` or `Option<MediaSource>` binding: the declaration
    /// owns its player.
    pub source: Option<Get>,
    /// An application player.
    pub player: Option<&'static str>,
    pub label: Option<Str>,
    pub controls: bool,
    pub fit: Option<&'static str>,
    pub fills_container: bool,
    /// Rounded corners of the picture, bound or fixed.
    pub corner: Option<crate::motion::Num>,
    pub animate: Option<crate::motion::Curve>,
}

pub(crate) fn url_of(value: &Value) -> Option<String> {
    let value = match value {
        Value::Optional(Some(inner)) => inner.as_ref(),
        Value::Optional(None) => return None,
        other => other,
    };
    match value {
        Value::Record(fields) => match fields.first() {
            Some(Value::Text(url)) => Some(url.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn media_call(
    name: fn(*mut bern_ffi::BernApp, *const u8, usize) -> bern_ffi::BernBuffer,
    bytes: &[u8],
) -> Vec<u8> {
    with_bytes(bytes, |ptr, len| take(name(host().app(), ptr, len)))
}

fn create(player: &str, owner: &str) -> Result<(), crate::Failure> {
    let mut writer = Writer::new();
    writer.text(player);
    writer.text(owner);
    // SAFETY: the handle is live; the bytes are valid for this call.
    let reply = media_call(
        |app, ptr, len| unsafe { bern_ffi::media::bern_media_create(app, ptr, len) },
        &writer.bytes,
    );
    status(&reply).map(|_| ())
}

fn bind(player: &str, url: Option<&str>) -> Result<(), crate::Failure> {
    let mut writer = Writer::new();
    writer.text(player);
    match url {
        Some(url) => {
            writer.u8(1);
            writer.text(url);
        }
        None => writer.u8(0),
    }
    // SAFETY: the handle is live; the bytes are valid for this call.
    let reply = media_call(
        |app, ptr, len| unsafe { bern_ffi::media::bern_media_bind(app, ptr, len) },
        &writer.bytes,
    );
    status(&reply).map(|_| ())
}

fn post(name: unsafe extern "C" fn(*mut bern_ffi::BernApp, *const u8, usize), bytes: &[u8]) {
    with_bytes(bytes, |ptr, len| {
        // SAFETY: the handle is live; the bytes are valid for this call.
        unsafe { name(host().app(), ptr, len) }
    });
}

fn view(token: &str, attached: Option<(&str, &str)>) {
    let mut writer = Writer::new();
    writer.text(token);
    match attached {
        Some((player, window)) => {
            writer.u8(1);
            writer.text(player);
            writer.text(window);
        }
        None => writer.u8(0),
    }
    post(bern_ffi::media::bern_media_view, &writer.bytes);
}

/// The hub's state of `player`, recording a dependency.
fn state_of(player: &str) -> Option<PlayerState> {
    MEDIA.with(|media| media.players.with(|players| players.get(player).cloned()))
}

fn peek_state(player: &str) -> Option<PlayerState> {
    MEDIA.with(|media| untrack(|| media.players.with(|players| players.get(player).cloned())))
}

/// A scale that previews while dragged and commits on release; keyboard
/// and scroll changes commit at once.
fn scrubber(
    commit: Rc<dyn Fn(f64)>,
    preview: Option<Rc<dyn Fn(f64)>>,
) -> (gtk::Scale, Rc<StdCell<bool>>, Rc<StdCell<bool>>) {
    let scale = gtk::Scale::new(gtk::Orientation::Horizontal, None::<&gtk::Adjustment>);
    scale.set_draw_value(false);
    let updating = Rc::new(StdCell::new(false));
    let dragging = Rc::new(StdCell::new(false));
    let pressed = Rc::clone(&dragging);
    let released = Rc::clone(&commit);
    let read = scale.clone();
    let legacy = gtk::EventControllerLegacy::new();
    legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
    legacy.connect_event(move |_, event| {
        match event.event_type() {
            gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => pressed.set(true),
            gtk::gdk::EventType::ButtonRelease | gtk::gdk::EventType::TouchEnd if pressed.get() => {
                let pressed = Rc::clone(&pressed);
                let released = Rc::clone(&released);
                let read = read.clone();
                glib::idle_add_local_once(move || {
                    pressed.set(false);
                    released(read.value());
                });
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    scale.add_controller(legacy);
    let guard = Rc::clone(&updating);
    let moving = Rc::clone(&dragging);
    scale.connect_value_changed(move |scale| {
        if guard.get() {
            return;
        }
        if moving.get() {
            if let Some(preview) = &preview {
                preview(scale.value());
            }
        } else {
            commit(scale.value());
        }
    });
    (scale, updating, dragging)
}

/// A video of a declaration's own player (`source`) or of an application
/// player, with the standard command-driven control bar.
#[allow(non_snake_case)]
pub fn Video(p: &Parent, ctx: &Ctx, props: VideoProps) {
    let win = Rc::clone(ctx.win());
    let label: Rc<dyn Fn() -> String> = match &props.label {
        Some(label) => {
            let at = ctx.clone();
            let label = Rc::clone(label);
            Rc::new(move || label(&at))
        }
        None => Rc::new(|| tr("bern.media.video")),
    };
    let player: String = match props.player {
        Some(player) => player.to_string(),
        None => format!("{}/{}", win.id, ctx.target(props.id)),
    };
    if let Some(source) = props.source.clone() {
        match create(&player, &win.id) {
            Err(failure) => win.fail(&failure),
            Ok(()) => {
                let released = player.clone();
                on_cleanup(move || {
                    let mut writer = Writer::new();
                    writer.text(&released);
                    post(bern_ffi::media::bern_media_release, &writer.bytes);
                });
                let bound: Rc<RefCell<Option<Option<String>>>> = Rc::new(RefCell::new(None));
                let at = ctx.clone();
                let binding = player.clone();
                let failed = Rc::clone(&win);
                effect(move || {
                    let url = url_of(&source(&at));
                    if bound.borrow().as_ref() == Some(&url) {
                        return;
                    }
                    *bound.borrow_mut() = Some(url.clone());
                    if let Err(failure) = untrack(|| bind(&binding, url.as_deref())) {
                        failed.fail(&failure);
                    }
                });
            }
        }
    }

    let token = MEDIA.with(|media| {
        let next = media.views.get() + 1;
        media.views.set(next);
        format!("view-{next}")
    });
    let picture: gtk::Widget = if MEDIA.with(|media| media.kernel.get()) {
        let fit = match props.fit {
            Some("cover") => VideoFit::Cover,
            Some("fill") => VideoFit::Fill,
            _ => VideoFit::Contain,
        };
        crate::kernel::Screen::new(&token, &player, fit).upcast()
    } else {
        let picture = gtk::Picture::new();
        stream(&player).present(&picture);
        picture.set_content_fit(match props.fit {
            Some("cover") => gtk::ContentFit::Cover,
            Some("fill") => gtk::ContentFit::Fill,
            _ => gtk::ContentFit::Contain,
        });
        picture.set_can_shrink(true);
        picture.upcast()
    };
    let picture_widget: gtk::Widget = match &props.corner {
        Some(corner) => crate::features::rounded(ctx, &picture, corner, props.animate).upcast(),
        None => picture.clone().upcast(),
    };
    let overlay = gtk::Overlay::new();
    if props.fills_container {
        picture_widget.set_hexpand(true);
        picture_widget.set_vexpand(true);
        overlay.set_child(Some(&picture_widget));
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
    } else {
        let frame = gtk::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
        frame.set_child(Some(&picture_widget));
        frame.set_hexpand(true);
        overlay.set_child(Some(&frame));
        // Room for the status while there is no picture to size the frame.
        overlay.set_size_request(-1, 220);
    }
    let status_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    status_box.set_halign(gtk::Align::Center);
    status_box.set_valign(gtk::Align::Center);
    overlay.add_overlay(&status_box);
    view(&token, Some((&player, &win.id)));
    let detached = token.clone();
    let shown_picture = picture.clone();
    on_cleanup(move || {
        view(&detached, None);
        if let Some(picture) = shown_picture.downcast_ref::<gtk::Picture>() {
            picture.set_paintable(None::<&gtk::gdk::Paintable>);
        }
    });
    {
        let player = player.clone();
        let label = Rc::clone(&label);
        let picture = picture.clone();
        effect(move || {
            let state = state_of(&player);
            picture.set_opacity(if state.as_ref().is_some_and(PlayerState::shows_frames) {
                1.0
            } else {
                0.0
            });
            while let Some(child) = status_box.first_child() {
                status_box.remove(&child);
            }
            let add = |icon: &str, title: String, detail: Option<String>| {
                let image = gtk::Image::from_icon_name(icon);
                image.set_pixel_size(32);
                image.add_css_class("dim-label");
                status_box.append(&image);
                let text = gtk::Label::new(Some(&title));
                text.add_css_class("heading");
                status_box.append(&text);
                if let Some(detail) = detail {
                    let note = gtk::Label::new(Some(&detail));
                    note.add_css_class("dim-label");
                    note.set_wrap(true);
                    note.set_justify(gtk::Justification::Center);
                    note.set_max_width_chars(48);
                    note.set_lines(3);
                    note.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    status_box.append(&note);
                }
            };
            match &state {
                None => add("camera-video-symbolic", tr("bern.media.unavailable"), None),
                Some(state) if state.is(Phase::Unavailable) => {
                    add("camera-video-symbolic", tr("bern.media.unavailable"), None)
                }
                Some(state) if state.is(Phase::Idle) => {
                    add("camera-video-symbolic", tr("bern.media.empty"), None)
                }
                Some(state) if state.is(Phase::Loading) => {
                    let spinner = gtk::Spinner::new();
                    spinner.start();
                    spinner.set_size_request(32, 32);
                    status_box.append(&spinner);
                }
                Some(state) if state.is(Phase::Failed) => add(
                    "dialog-warning-symbolic",
                    tr("bern.media.failed"),
                    state.error.clone(),
                ),
                _ => {}
            }
            let description = state
                .as_ref()
                .map(PlayerState::describe)
                .unwrap_or_default();
            picture.update_property(&[
                gtk::accessible::Property::Label(&label()),
                gtk::accessible::Property::Description(&description),
            ]);
        });
    }
    let read = player.clone();
    controls::register(
        ctx,
        props.id,
        "video",
        Some(Rc::clone(&label)),
        None,
        Actions {
            value: Some(Rc::new(move || {
                peek_state(&read).map(|state| state.describe())
            })),
            ..Actions::default()
        },
    );
    let container = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if props.fills_container {
        container.set_hexpand(true);
        container.set_vexpand(true);
    } else {
        // The standard picture measures 16:9 of the available width.
        let probe = gtk::DrawingArea::new();
        probe.set_content_height(0);
        probe.set_hexpand(true);
        let sized = overlay.clone();
        probe.connect_resize(move |_, width, _| {
            if width > 0 {
                sized.set_size_request(-1, (width * 9 / 16).max(120));
            }
        });
        container.append(&probe);
    }
    container.append(&overlay);
    if props.controls {
        container.append(&control_bar(ctx, props.id, &player));
    }
    if p.is_list() {
        p.add(&plain_row(&container));
    } else {
        p.add(&container);
    }
}

fn control_bar(ctx: &Ctx, id: &str, player: &str) -> gtk::Box {
    let player = player.to_string();
    let at = ctx.clone();
    let send = {
        let player = player.clone();
        let at = at.clone();
        Rc::new(move |op: MediaOp, args: Vec<Value>| media(&at, op, &player, args))
    };
    let toggle: Rc<dyn Fn()> = {
        let send = Rc::clone(&send);
        let player = player.clone();
        Rc::new(move || {
            let playing = peek_state(&player).is_some_and(|state| state.playing());
            send(
                if playing {
                    MediaOp::Pause
                } else {
                    MediaOp::Play
                },
                vec![],
            );
        })
    };
    let seek: Rc<dyn Fn(f64)> = {
        let send = Rc::clone(&send);
        Rc::new(move |ms| send(MediaOp::Seek, vec![Value::Int(ms.round() as i64)]))
    };
    let set_muted: Rc<dyn Fn(bool)> = {
        let send = Rc::clone(&send);
        Rc::new(move |muted| send(MediaOp::SetMuted, vec![Value::Bool(muted)]))
    };
    let set_volume: Rc<dyn Fn(f64)> = {
        let send = Rc::clone(&send);
        Rc::new(move |volume| {
            send(
                MediaOp::SetVolume,
                vec![Value::Float(volume.clamp(0.0, 1.0))],
            )
        })
    };

    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play.add_css_class("flat");
    let run = Rc::clone(&toggle);
    play.connect_clicked(move |_| run());
    let position_text = gtk::Label::new(Some("0:00"));
    position_text.add_css_class("numeric");
    position_text.add_css_class("caption");
    let duration_text = gtk::Label::new(Some("--:--"));
    duration_text.add_css_class("numeric");
    duration_text.add_css_class("caption");
    let preview_text = position_text.clone();
    let (position, position_updating, position_dragging) = scrubber(
        Rc::clone(&seek),
        Some(Rc::new(move |ms| preview_text.set_label(&time(ms as i64)))),
    );
    position.set_hexpand(true);
    let mute = gtk::Button::from_icon_name("audio-volume-high-symbolic");
    mute.add_css_class("flat");
    let toggle_mute = {
        let set_muted = Rc::clone(&set_muted);
        let player = player.clone();
        Rc::new(move || set_muted(!peek_state(&player).is_some_and(|state| state.muted)))
    };
    let run = Rc::clone(&toggle_mute);
    mute.connect_clicked(move |_| run());
    let (volume, volume_updating, volume_dragging) = scrubber(Rc::clone(&set_volume), None);
    volume.set_range(0.0, 1.0);
    volume.set_increments(0.05, 0.1);
    volume.set_size_request(96, -1);

    {
        let player = player.clone();
        let play = play.clone();
        let position = position.clone();
        let mute = mute.clone();
        let volume = volume.clone();
        let position_text = position_text.clone();
        let duration_text = duration_text.clone();
        effect(move || {
            let state = state_of(&player);
            let playing = state.as_ref().is_some_and(PlayerState::playing);
            let play_label = tr(if playing {
                "bern.media.pause"
            } else {
                "bern.media.play"
            });
            play.set_icon_name(if playing {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            });
            play.set_tooltip_text(Some(&play_label));
            play.update_property(&[gtk::accessible::Property::Label(&play_label)]);
            play.set_sensitive(state.as_ref().is_some_and(PlayerState::playable));
            let duration = state
                .as_ref()
                .and_then(|state| state.duration_ms)
                .unwrap_or(0);
            let at = state.as_ref().map(|state| state.position_ms).unwrap_or(0);
            if !position_dragging.get() {
                position_updating.set(true);
                position.set_range(0.0, duration.max(1) as f64);
                position.set_increments(5000.0, (duration as f64 / 10.0).max(5000.0));
                position.set_value(at.min(duration) as f64);
                position_updating.set(false);
                position_text.set_label(&time(at));
            }
            position.set_sensitive(state.as_ref().is_some_and(PlayerState::seekable));
            position
                .update_property(&[gtk::accessible::Property::Label(&tr("bern.media.position"))]);
            duration_text.set_label(&match state.as_ref() {
                Some(state) if state.duration_ms.is_some() => time(duration),
                Some(state) if state.playable() => tr("bern.media.live"),
                _ => "--:--".to_string(),
            });
            let muted = state.as_ref().is_some_and(|state| state.muted);
            let mute_label = tr(if muted {
                "bern.media.unmute"
            } else {
                "bern.media.mute"
            });
            mute.set_icon_name(if muted {
                "audio-volume-muted-symbolic"
            } else {
                "audio-volume-high-symbolic"
            });
            mute.set_tooltip_text(Some(&mute_label));
            mute.update_property(&[gtk::accessible::Property::Label(&mute_label)]);
            let controllable = state.as_ref().is_some_and(PlayerState::controllable);
            mute.set_sensitive(controllable);
            volume.set_sensitive(controllable);
            if !volume_dragging.get() {
                volume_updating.set(true);
                volume.set_value(state.as_ref().map(|state| state.volume).unwrap_or(1.0));
                volume_updating.set(false);
            }
            volume.update_property(&[gtk::accessible::Property::Label(&tr("bern.media.volume"))]);
        });
    }

    let target = |name: &str| format!("{id}.{name}");
    {
        let player = player.clone();
        let enabled_player = player.clone();
        controls::register(
            ctx,
            &target("play_pause"),
            "button",
            Some(Rc::new(move || {
                let playing = state_of(&player).is_some_and(|state| state.playing());
                tr(if playing {
                    "bern.media.pause"
                } else {
                    "bern.media.play"
                })
            })),
            Some(Rc::new(move || {
                state_of(&enabled_player).is_some_and(|state| state.playable())
            })),
            Actions {
                activate: Some(Rc::clone(&toggle)),
                ..Actions::default()
            },
        );
    }
    {
        let enabled_player = player.clone();
        let value_player = player.clone();
        controls::register(
            ctx,
            &target("position"),
            "slider",
            Some(Rc::new(|| tr("bern.media.position"))),
            Some(Rc::new(move || {
                state_of(&enabled_player).is_some_and(|state| state.seekable())
            })),
            Actions {
                set_number: Some(Rc::clone(&seek)),
                value: Some(Rc::new(move || {
                    Some(number(
                        peek_state(&value_player)
                            .map(|state| state.position_ms)
                            .unwrap_or(0) as f64,
                    ))
                })),
                ..Actions::default()
            },
        );
    }
    {
        let enabled_player = player.clone();
        let value_player = player.clone();
        controls::register(
            ctx,
            &target("mute"),
            "switch",
            Some(Rc::new(|| tr("bern.media.mute"))),
            Some(Rc::new(move || {
                state_of(&enabled_player).is_some_and(|state| state.controllable())
            })),
            Actions {
                activate: Some(toggle_mute),
                set_bool: Some(Rc::clone(&set_muted)),
                value: Some(Rc::new(move || {
                    Some(
                        peek_state(&value_player)
                            .is_some_and(|state| state.muted)
                            .to_string(),
                    )
                })),
                ..Actions::default()
            },
        );
    }
    {
        let enabled_player = player.clone();
        let value_player = player.clone();
        controls::register(
            ctx,
            &target("volume"),
            "slider",
            Some(Rc::new(|| tr("bern.media.volume"))),
            Some(Rc::new(move || {
                state_of(&enabled_player).is_some_and(|state| state.controllable())
            })),
            Actions {
                set_number: Some(Rc::clone(&set_volume)),
                value: Some(Rc::new(move || {
                    Some(number(
                        peek_state(&value_player)
                            .map(|state| state.volume)
                            .unwrap_or(1.0),
                    ))
                })),
                ..Actions::default()
            },
        );
    }

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.append(&play);
    bar.append(&position_text);
    bar.append(&position);
    bar.append(&duration_text);
    bar.append(&mute);
    bar.append(&volume);
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `MediaPlayer` record as the hub sends it, 1:05 into its source.
    fn player(id: &str, phase: Phase, source: Option<&str>, duration_ms: Option<i64>) -> Value {
        let source = source.map_or(Value::none(), |url| {
            Value::some(Value::Record(vec![Value::Text(url.into())]))
        });
        let duration = duration_ms.map_or(Value::none(), |ms| Value::some(Value::Int(ms)));
        Value::Record(vec![
            Value::Text(id.into()),
            Value::Text("application".into()),
            Value::Bool(false),
            source,
            Value::Int(1),
            Value::unit_variant(phase.index()),
            Value::Int(65_000),
            duration,
            Value::Bool(true),
            Value::Float(0.5),
            Value::Bool(false),
            Value::none(),
            Value::Int(1),
            Value::none(),
        ])
    }

    fn state(value: &Value) -> PlayerState {
        PlayerState::from_value(value).expect("a player").1
    }

    #[test]
    fn times_show_minutes_and_hours() {
        assert_eq!(time(0), "0:00");
        assert_eq!(time(59_999), "0:59");
        assert_eq!(time(61_000), "1:01");
        assert_eq!(time(3_599_000), "59:59");
        assert_eq!(time(3_661_000), "1:01:01");
        assert_eq!(time(-5_000), "0:00");
    }

    #[test]
    fn errors_keep_their_first_line_without_the_element() {
        assert_eq!(
            summary(
                "Error from element /GstPlayBin:playbin0/GstURIDecodeBin:uridecodebin0: Resource not found.\n../gst/playback/gsturidecodebin.c(1431): debug"
            ),
            "Resource not found."
        );
        assert_eq!(
            summary("\n  \n Could not open file \nmore detail"),
            "Could not open file"
        );
        assert_eq!(summary(""), "");
    }

    #[test]
    fn sources_give_their_url() {
        let source = Value::Record(vec![Value::Text("file:///a.mp4".into())]);
        assert_eq!(url_of(&source).as_deref(), Some("file:///a.mp4"));
        assert_eq!(
            url_of(&Value::some(source)).as_deref(),
            Some("file:///a.mp4")
        );
        assert_eq!(url_of(&Value::none()), None);
        assert_eq!(url_of(&Value::Text("file:///a.mp4".into())), None);
        assert_eq!(url_of(&Value::Record(vec![Value::Int(1)])), None);
    }

    #[test]
    fn player_records_decode_by_field() {
        let record = player("main", Phase::Playing, Some("file:///a.mp4"), Some(180_000));
        let (id, decoded) = PlayerState::from_value(&record).expect("a player");
        assert_eq!(id, "main");
        assert_eq!(
            decoded,
            PlayerState {
                source: Some("file:///a.mp4".into()),
                phase: Phase::Playing.index(),
                position_ms: 65_000,
                duration_ms: Some(180_000),
                seekable: true,
                volume: 0.5,
                muted: false,
                error: None,
            }
        );
        // Missing fields read as neutral values, and no phase as
        // unavailable.
        let short = state(&Value::Record(vec![Value::Text("p".into())]));
        assert!(short.is(Phase::Unavailable));
        assert_eq!((short.source, short.volume), (None, 1.0));
        assert_eq!(PlayerState::from_value(&Value::Text("p".into())), None);
    }

    #[test]
    fn phases_decide_what_the_controls_offer() {
        let source = Some("file:///a.mp4");
        let playing = state(&player("p", Phase::Playing, source, Some(1000)));
        assert!(playing.playing() && playing.playable() && playing.seekable());
        assert!(playing.shows_frames());
        assert!(state(&player("p", Phase::Buffering, source, Some(1000))).playing());
        let paused = state(&player("p", Phase::Paused, source, Some(1000)));
        assert!(!paused.playing() && paused.seekable());
        // Without a duration, as for live media, there is nowhere to seek.
        assert!(!state(&player("p", Phase::Playing, source, None)).seekable());
        assert!(!state(&player("p", Phase::Ready, None, Some(1000))).playable());
        for phase in [Phase::Idle, Phase::Failed, Phase::Unavailable] {
            let shown = state(&player("p", phase, source, Some(1000)));
            assert!(
                !shown.playable() && !shown.seekable() && !shown.shows_frames(),
                "{phase:?}"
            );
        }
        assert!(state(&player("p", Phase::Loading, source, None)).controllable());
        assert!(!state(&player("p", Phase::Unavailable, source, None)).controllable());
    }

    #[test]
    fn players_describe_their_phase_and_time() {
        assert_eq!(
            state(&player("p", Phase::Playing, Some("x"), Some(10_800_000))).describe(),
            "Playing 1:05 / 3:00:00"
        );
        assert_eq!(
            state(&player("p", Phase::Paused, Some("x"), None)).describe(),
            "Paused 1:05 / --:--"
        );
        let mut unknown = state(&player("p", Phase::Ready, None, None));
        unknown.phase = 42;
        assert_eq!(unknown.describe(), "Unavailable 1:05 / --:--");
    }

    #[test]
    fn hub_states_replace_the_players_by_revision() {
        let (types, _) = media_catalog();
        let ty = Type::Record {
            name: "MediaPlayer".into(),
        };
        let snapshot = |revision: u64, players: &[Value]| {
            let mut writer = Writer::new();
            writer.u64(revision);
            writer.u32(players.len() as u32);
            for player in players {
                encode(player, &ty, &types, &mut writer);
            }
            writer.bytes
        };
        receive_state(&snapshot(
            2,
            &[
                player("a", Phase::Playing, Some("x"), None),
                player("b", Phase::Paused, Some("y"), None),
            ],
        ));
        assert!(peek_state("a").is_some_and(|state| state.is(Phase::Playing)));
        assert!(peek_state("b").is_some());
        // An older revision is ignored; a newer one is the whole set.
        receive_state(&snapshot(1, &[player("a", Phase::Ended, Some("x"), None)]));
        assert!(peek_state("a").is_some_and(|state| state.is(Phase::Playing)));
        receive_state(&snapshot(3, &[player("a", Phase::Ended, Some("x"), None)]));
        assert!(peek_state("a").is_some_and(|state| state.is(Phase::Ended)));
        assert_eq!(peek_state("b"), None);
    }
}
