//! The system's now playing: what the application plays, shown where the
//! system shows it, and the commands the system sends for it.
//!
//! An engine that plays audio itself calls [`update`] whenever the song,
//! whether it plays, or where it is changes by more than playing on, and
//! [`update`] with `None` when nothing plays. The system then shows it in
//! Control Center, the menu bar's Now Playing and on the lock screen
//! (macOS and iOS), in the media flyout of the volume keys (Windows), to
//! MPRIS players and desktop shells (Linux), and in the media notification
//! (Android, through the host). Media keys, headphone buttons and those
//! controls come back as [`Command`]s: [`take_commands`] hands over the
//! ones not taken yet, and each one wakes the engine.
//!
//! Positions are in seconds. The systems move the position on by
//! themselves while it plays, so playing on needs no updates.

use std::sync::{Arc, Mutex};

/// What plays now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The cover, as the bytes of a PNG or JPEG.
    pub artwork: Option<Arc<Vec<u8>>>,
    /// Its length, 0 when not known.
    pub duration: f64,
    /// Where it is.
    pub position: f64,
    pub playing: bool,
}

/// What the system asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Play,
    Pause,
    /// Play or pause, whichever it is not doing.
    Toggle,
    Next,
    Previous,
    /// Go to this many seconds into the song.
    Seek(f64),
}

static COMMANDS: Mutex<Vec<Command>> = Mutex::new(Vec::new());

/// Queue a command the system sent and wake the engine for it.
fn push(command: Command) {
    COMMANDS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(command);
    if let Some(waker) = crate::wake::waker() {
        waker.wake();
    }
}

/// The commands the system sent since the last call, the earliest first.
pub fn take_commands() -> Vec<Command> {
    std::mem::take(&mut *COMMANDS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Show `now` as what plays, or that nothing plays.
pub fn update(now: Option<&NowPlaying>) {
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "watchos"))]
    apple::update(now.cloned());
    #[cfg(target_os = "windows")]
    windows_smtc::update(now.cloned());
    #[cfg(target_os = "linux")]
    mpris::update(now.cloned());
    #[cfg(target_os = "android")]
    android::update(now);
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "watchos",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    )))]
    let _ = now;
}

/// A command the host received, by name: `play`, `pause`, `toggle`,
/// `next`, `previous`, or `seek:<seconds>`. Hosts that hear the system's
/// commands themselves (Android's media session) hand them over here.
pub fn command_from_host(name: &str) {
    let command = match name {
        "play" => Command::Play,
        "pause" => Command::Pause,
        "toggle" => Command::Toggle,
        "next" => Command::Next,
        "previous" => Command::Previous,
        other => match other
            .strip_prefix("seek:")
            .and_then(|seconds| seconds.parse().ok())
        {
            Some(seconds) => Command::Seek(seconds),
            None => return,
        },
    };
    push(command);
}

/// What the host shows, as JSON: the fields of [`NowPlaying`], the cover
/// as base64.
#[cfg(any(target_os = "android", test))]
fn as_json(now: &NowPlaying) -> String {
    fn text(value: &str) -> String {
        let mut out = String::from("\"");
        for c in value.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    format!(
        "{{\"title\":{},\"artist\":{},\"album\":{},\"artwork\":{},\"duration\":{},\"position\":{},\"playing\":{}}}",
        text(&now.title),
        text(&now.artist),
        text(&now.album),
        text(
            &now.artwork
                .as_deref()
                .map(|bytes| base64(bytes))
                .unwrap_or_default()
        ),
        now.duration.max(0.0),
        now.position.max(0.0),
        now.playing
    )
}

#[cfg(any(target_os = "android", test))]
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Android: the host keeps a media session and its notification.
#[cfg(target_os = "android")]
mod android {
    use super::NowPlaying;

    pub fn update(now: Option<&NowPlaying>) {
        let json = now.map(super::as_json).unwrap_or_default();
        let _ = crate::platform::request("now_playing", &json, "");
    }
}

/// macOS, iOS and watchOS: `MPNowPlayingInfoCenter` and
/// `MPRemoteCommandCenter`, used on the main thread.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "watchos"))]
mod apple {
    use super::{Command, NowPlaying, push};
    use block2::RcBlock;
    use dispatch2::DispatchQueue;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2::{AnyThread, ClassType};
    use objc2_core_foundation::CGSize;
    use objc2_foundation::{NSData, NSMutableDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPChangePlaybackPositionCommandEvent, MPMediaItemArtwork, MPMediaItemPropertyAlbumTitle,
        MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork, MPMediaItemPropertyPlaybackDuration,
        MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
        MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyPlaybackRate,
        MPRemoteCommand, MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
    };
    use std::ptr::NonNull;
    use std::sync::Once;

    static COMMANDS: Once = Once::new();

    pub fn update(now: Option<NowPlaying>) {
        DispatchQueue::main().exec_async(move || {
            // SAFETY: on the main thread, with the framework's own types.
            unsafe { show(now) }
        });
    }

    unsafe fn show(now: Option<NowPlaying>) {
        COMMANDS.call_once(|| unsafe { listen() });
        let center = unsafe { MPNowPlayingInfoCenter::defaultCenter() };
        let Some(now) = now else {
            unsafe { center.setNowPlayingInfo(None) };
            #[cfg(target_os = "macos")]
            unsafe {
                center.setPlaybackState(objc2_media_player::MPNowPlayingPlaybackState::Stopped)
            };
            return;
        };
        #[cfg(any(target_os = "ios", target_os = "watchos"))]
        unsafe {
            session()
        };
        let info = NSMutableDictionary::<NSString, AnyObject>::new();
        let put = |key: &NSString, value: &AnyObject| unsafe {
            info.setObject_forKey(value, ProtocolObject::from_ref(key))
        };
        put(
            unsafe { MPMediaItemPropertyTitle },
            &NSString::from_str(&now.title),
        );
        put(
            unsafe { MPMediaItemPropertyArtist },
            &NSString::from_str(&now.artist),
        );
        put(
            unsafe { MPMediaItemPropertyAlbumTitle },
            &NSString::from_str(&now.album),
        );
        if now.duration > 0.0 {
            put(
                unsafe { MPMediaItemPropertyPlaybackDuration },
                &NSNumber::new_f64(now.duration),
            );
        }
        put(
            unsafe { MPNowPlayingInfoPropertyElapsedPlaybackTime },
            &NSNumber::new_f64(now.position.max(0.0)),
        );
        put(
            unsafe { MPNowPlayingInfoPropertyPlaybackRate },
            &NSNumber::new_f64(if now.playing { 1.0 } else { 0.0 }),
        );
        if let Some(artwork) = now
            .artwork
            .as_deref()
            .and_then(|bytes| unsafe { artwork(bytes) })
        {
            put(unsafe { MPMediaItemPropertyArtwork }, &artwork);
        }
        unsafe { center.setNowPlayingInfo(Some(&info)) };
        // macOS sends the media keys to the application whose state says
        // it plays.
        #[cfg(target_os = "macos")]
        unsafe {
            center.setPlaybackState(if now.playing {
                objc2_media_player::MPNowPlayingPlaybackState::Playing
            } else {
                objc2_media_player::MPNowPlayingPlaybackState::Paused
            })
        };
    }

    /// The cover as the system draws it: an `NSImage` on macOS, a
    /// `UIImage` on iOS.
    unsafe fn artwork(bytes: &[u8]) -> Option<Retained<AnyObject>> {
        let data = NSData::with_bytes(bytes);
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::NSImage;
            let image = NSImage::initWithData(NSImage::alloc(), &data)?;
            let size = image.size();
            let handler = RcBlock::new(move |_: CGSize| NonNull::from(&*image));
            let artwork = unsafe {
                MPMediaItemArtwork::initWithBoundsSize_requestHandler(
                    MPMediaItemArtwork::alloc(),
                    size,
                    &handler,
                )
            };
            Some(Retained::into_super(Retained::into_super(artwork)))
        }
        #[cfg(any(target_os = "ios", target_os = "watchos"))]
        {
            use objc2::msg_send;
            use objc2::runtime::AnyClass;
            let class = AnyClass::get(c"UIImage")?;
            let image: Option<Retained<AnyObject>> =
                unsafe { msg_send![class, imageWithData: &*data] };
            let image = image?;
            let size: CGSize = unsafe { msg_send![&*image, size] };
            let handler = RcBlock::new(move |_: CGSize| NonNull::from(&*image));
            let artwork: Option<Retained<MPMediaItemArtwork>> = unsafe {
                msg_send![MPMediaItemArtwork::alloc(), initWithBoundsSize: size, requestHandler: &*handler]
            };
            artwork.map(|artwork| Retained::into_super(Retained::into_super(artwork)))
        }
    }

    /// The commands of Control Center, the lock screen, headphones and the
    /// media keys, queued for the engine.
    unsafe fn listen() {
        let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };
        let simple: [(Retained<MPRemoteCommand>, Command); 5] = unsafe {
            [
                (center.playCommand(), Command::Play),
                (center.pauseCommand(), Command::Pause),
                (center.togglePlayPauseCommand(), Command::Toggle),
                (center.nextTrackCommand(), Command::Next),
                (center.previousTrackCommand(), Command::Previous),
            ]
        };
        for (command, sent) in simple {
            unsafe { command.setEnabled(true) };
            let handler = RcBlock::new(move |_: NonNull<MPRemoteCommandEvent>| {
                push(sent);
                MPRemoteCommandHandlerStatus::Success
            });
            let _ = unsafe { command.addTargetWithHandler(&handler) };
        }
        let seek = unsafe { center.changePlaybackPositionCommand() };
        unsafe { seek.setEnabled(true) };
        let handler = RcBlock::new(|event: NonNull<MPRemoteCommandEvent>| {
            // The command center sends this command only its own events.
            let event = unsafe {
                event
                    .cast::<MPChangePlaybackPositionCommandEvent>()
                    .as_ref()
            };
            push(Command::Seek(unsafe { event.positionTime() }));
            MPRemoteCommandHandlerStatus::Success
        });
        let _ = unsafe { seek.addTargetWithHandler(&handler) };
        let _ = MPRemoteCommand::class();
    }

    /// iOS and watchOS play on behind the lock screen and show their
    /// controls only for a playback session.
    #[cfg(any(target_os = "ios", target_os = "watchos"))]
    unsafe fn session() {
        use objc2_avf_audio::{AVAudioSession, AVAudioSessionCategoryPlayback};
        static ACTIVE: Once = Once::new();
        ACTIVE.call_once(|| unsafe {
            let session = AVAudioSession::sharedInstance();
            if let Some(category) = AVAudioSessionCategoryPlayback {
                let _ = session.setCategory_error(category);
            }
            let _ = session.setActive_error(true);
        });
    }
}

/// Windows: the system media transport controls of a `MediaPlayer` whose
/// own commands are off, kept by a thread of their own.
#[cfg(target_os = "windows")]
mod windows_smtc {
    use super::{Command, NowPlaying, push};
    use std::sync::{Mutex, mpsc};
    use windows::Foundation::{TimeSpan, TypedEventHandler};
    use windows::Media::Playback::MediaPlayer;
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
        SystemMediaTransportControls, SystemMediaTransportControlsButton,
        SystemMediaTransportControlsButtonPressedEventArgs,
        SystemMediaTransportControlsTimelineProperties,
    };
    use windows::Storage::Streams::{
        DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference,
    };

    static SENDER: Mutex<Option<mpsc::Sender<Option<NowPlaying>>>> = Mutex::new(None);

    pub fn update(now: Option<NowPlaying>) {
        let mut sender = SENDER.lock().unwrap_or_else(|e| e.into_inner());
        if sender.is_none() {
            let (send, receive) = mpsc::channel();
            let started = std::thread::Builder::new()
                .name("bern-now-playing".into())
                .spawn(move || run(receive));
            if started.is_err() {
                return;
            }
            *sender = Some(send);
        }
        if let Some(sender) = sender.as_ref() {
            let _ = sender.send(now);
        }
    }

    fn ticks(seconds: f64) -> TimeSpan {
        TimeSpan {
            Duration: (seconds.max(0.0) * 10_000_000.0) as i64,
        }
    }

    fn run(updates: mpsc::Receiver<Option<NowPlaying>>) {
        // SAFETY: this thread's own apartment, for the WinRT objects below.
        let _ = unsafe {
            windows::Win32::System::WinRT::RoInitialize(
                windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
            )
        };
        let Ok(player) = MediaPlayer::new() else {
            return;
        };
        let _ = player
            .CommandManager()
            .and_then(|manager| manager.SetIsEnabled(false));
        let Ok(controls) = player.SystemMediaTransportControls() else {
            return;
        };
        let _ = controls.SetIsPlayEnabled(true);
        let _ = controls.SetIsPauseEnabled(true);
        let _ = controls.SetIsNextEnabled(true);
        let _ = controls.SetIsPreviousEnabled(true);
        let _ = controls.ButtonPressed(&TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(|_, args| {
            if let Some(args) = args.as_ref() {
                match args.Button()? {
                    SystemMediaTransportControlsButton::Play => push(Command::Play),
                    SystemMediaTransportControlsButton::Pause => push(Command::Pause),
                    SystemMediaTransportControlsButton::Next => push(Command::Next),
                    SystemMediaTransportControlsButton::Previous => push(Command::Previous),
                    _ => {}
                }
            }
            Ok(())
        }));
        let _ = controls.PlaybackPositionChangeRequested(&TypedEventHandler::<
            SystemMediaTransportControls,
            PlaybackPositionChangeRequestedEventArgs,
        >::new(|_, args| {
            if let Some(args) = args.as_ref() {
                push(Command::Seek(
                    args.RequestedPlaybackPosition()?.Duration as f64 / 10_000_000.0,
                ));
            }
            Ok(())
        }));
        while let Ok(now) = updates.recv() {
            let _ = show(&controls, now);
        }
    }

    fn show(
        controls: &SystemMediaTransportControls,
        now: Option<NowPlaying>,
    ) -> windows::core::Result<()> {
        let Some(now) = now else {
            controls.SetPlaybackStatus(MediaPlaybackStatus::Stopped)?;
            controls.SetIsEnabled(false)?;
            return Ok(());
        };
        controls.SetIsEnabled(true)?;
        let display = controls.DisplayUpdater()?;
        display.SetType(MediaPlaybackType::Music)?;
        let music = display.MusicProperties()?;
        music.SetTitle(&now.title.as_str().into())?;
        music.SetArtist(&now.artist.as_str().into())?;
        music.SetAlbumTitle(&now.album.as_str().into())?;
        if let Some(bytes) = now.artwork.as_deref() {
            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream)?;
            writer.WriteBytes(bytes)?;
            writer.StoreAsync()?.join()?;
            writer.DetachStream()?;
            stream.Seek(0)?;
            display.SetThumbnail(&RandomAccessStreamReference::CreateFromStream(&stream)?)?;
        }
        display.Update()?;
        controls.SetPlaybackStatus(if now.playing {
            MediaPlaybackStatus::Playing
        } else {
            MediaPlaybackStatus::Paused
        })?;
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetStartTime(ticks(0.0))?;
        timeline.SetMinSeekTime(ticks(0.0))?;
        timeline.SetEndTime(ticks(now.duration))?;
        timeline.SetMaxSeekTime(ticks(now.duration))?;
        timeline.SetPosition(ticks(now.position))?;
        controls.UpdateTimelineProperties(&timeline)?;
        Ok(())
    }
}

/// Linux: an MPRIS player on the session bus, which desktop shells and
/// players such as `playerctl` show and command.
#[cfg(target_os = "linux")]
mod mpris {
    use super::{Command, NowPlaying, push};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::Instant;
    use zbus::blocking::Connection;
    use zbus::object_server::SignalEmitter;
    use zbus::zvariant::{ObjectPath, OwnedValue, Value};

    const PATH: &str = "/org/mpris/MediaPlayer2";

    /// What plays, and when it was at its position.
    #[derive(Default)]
    struct Playing {
        now: Option<NowPlaying>,
        at: Option<Instant>,
        art: String,
    }

    static STATE: Mutex<Playing> = Mutex::new(Playing {
        now: None,
        at: None,
        art: String::new(),
    });
    static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

    fn name() -> String {
        crate::storage::application_id()
            .and_then(|id| id.rsplit('.').next())
            .filter(|last| !last.is_empty())
            .map(|last| {
                last.chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect()
            })
            .unwrap_or_else(|| "bern".into())
    }

    /// Where it is now: its position moved on by the time since, while it
    /// plays.
    fn position(state: &Playing) -> f64 {
        let Some(now) = state.now.as_ref() else {
            return 0.0;
        };
        let moved = match (now.playing, state.at) {
            (true, Some(at)) => at.elapsed().as_secs_f64(),
            _ => 0.0,
        };
        (now.position + moved).min(if now.duration > 0.0 {
            now.duration
        } else {
            f64::MAX
        })
    }

    struct Root;

    #[zbus::interface(name = "org.mpris.MediaPlayer2")]
    impl Root {
        fn raise(&self) {}
        fn quit(&self) {}
        #[zbus(property)]
        fn can_quit(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn can_raise(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn has_track_list(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn identity(&self) -> String {
            let name = name();
            let mut chars = name.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
        #[zbus(property)]
        fn desktop_entry(&self) -> String {
            crate::storage::application_id()
                .unwrap_or_default()
                .to_string()
        }
        #[zbus(property)]
        fn supported_uri_schemes(&self) -> Vec<String> {
            Vec::new()
        }
        #[zbus(property)]
        fn supported_mime_types(&self) -> Vec<String> {
            Vec::new()
        }
    }

    struct Player;

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Player {
        fn play(&self) {
            push(Command::Play);
        }
        fn pause(&self) {
            push(Command::Pause);
        }
        fn play_pause(&self) {
            push(Command::Toggle);
        }
        fn stop(&self) {
            push(Command::Pause);
        }
        fn next(&self) {
            push(Command::Next);
        }
        fn previous(&self) {
            push(Command::Previous);
        }
        /// Moves by `offset` microseconds.
        fn seek(&self, offset: i64) {
            let at = position(&STATE.lock().unwrap_or_else(|e| e.into_inner()));
            push(Command::Seek((at + offset as f64 / 1e6).max(0.0)));
        }
        fn set_position(&self, _track: ObjectPath<'_>, position: i64) {
            push(Command::Seek(position as f64 / 1e6));
        }
        fn open_uri(&self, _uri: &str) {}

        #[zbus(property)]
        fn playback_status(&self) -> String {
            let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            match state.now.as_ref() {
                Some(now) if now.playing => "Playing",
                Some(_) => "Paused",
                None => "Stopped",
            }
            .into()
        }
        #[zbus(property)]
        fn metadata(&self) -> HashMap<String, OwnedValue> {
            let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            let mut metadata = HashMap::new();
            let Some(now) = state.now.as_ref() else {
                return metadata;
            };
            let mut put = |key: &str, value: Value<'_>| {
                if let Ok(value) = value.try_into() {
                    metadata.insert(key.to_string(), value);
                }
            };
            if let Ok(track) = ObjectPath::try_from("/org/bern/track") {
                put("mpris:trackid", Value::ObjectPath(track));
            }
            put("xesam:title", Value::from(now.title.clone()));
            put("xesam:artist", Value::from(vec![now.artist.clone()]));
            put("xesam:album", Value::from(now.album.clone()));
            if now.duration > 0.0 {
                put("mpris:length", Value::from((now.duration * 1e6) as i64));
            }
            if !state.art.is_empty() {
                put("mpris:artUrl", Value::from(state.art.clone()));
            }
            metadata
        }
        #[zbus(property)]
        fn position(&self) -> i64 {
            (position(&STATE.lock().unwrap_or_else(|e| e.into_inner())) * 1e6) as i64
        }
        #[zbus(property)]
        fn rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn minimum_rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn maximum_rate(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn volume(&self) -> f64 {
            1.0
        }
        #[zbus(property)]
        fn can_go_next(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_go_previous(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_play(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_pause(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_seek(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn can_control(&self) -> bool {
            true
        }
    }

    /// The session bus, with the player on it; `None` without a session
    /// bus.
    fn connection() -> Option<Connection> {
        let mut connection = CONNECTION.lock().unwrap_or_else(|e| e.into_inner());
        if connection.is_none() {
            let built = zbus::blocking::connection::Builder::session()
                .and_then(|builder| builder.name(format!("org.mpris.MediaPlayer2.{}", name())))
                .and_then(|builder| builder.serve_at(PATH, Root))
                .and_then(|builder| builder.serve_at(PATH, Player))
                .and_then(|builder| builder.build());
            *connection = built.ok();
        }
        connection.clone()
    }

    pub fn update(now: Option<NowPlaying>) {
        {
            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            // The cover as a file the shell can show.
            if let Some(bytes) = now.as_ref().and_then(|now| now.artwork.as_deref()) {
                if let Some(folder) = crate::storage::cache_dir() {
                    let path = folder.join("now-playing-artwork");
                    if std::fs::write(&path, bytes).is_ok() {
                        state.art = format!("file://{}", path.display());
                    }
                }
            } else {
                state.art.clear();
            }
            state.at = Some(Instant::now());
            state.now = now;
        }
        let Some(connection) = connection() else {
            return;
        };
        let Ok(player) = connection.object_server().interface::<_, Player>(PATH) else {
            return;
        };
        let emitter: &SignalEmitter<'_> = player.signal_emitter();
        let player = player.get();
        let _ = zbus::block_on(player.playback_status_changed(emitter));
        let _ = zbus::block_on(player.metadata_changed(emitter));
        // Position is not announced as it changes; a seek is.
        let _ = zbus::block_on(emitter.emit(
            "org.mpris.MediaPlayer2.Player",
            "Seeked",
            &player.position(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_from_the_host_queue_in_order() {
        take_commands();
        command_from_host("toggle");
        command_from_host("seek:12.5");
        command_from_host("unknown");
        command_from_host("next");
        assert_eq!(
            take_commands(),
            vec![Command::Toggle, Command::Seek(12.5), Command::Next]
        );
        assert!(take_commands().is_empty());
    }

    #[test]
    fn the_host_reads_what_plays_as_json() {
        let now = NowPlaying {
            title: "A \"B\"".into(),
            artist: "C".into(),
            album: String::new(),
            artwork: Some(Arc::new(vec![1, 2, 3, 4])),
            duration: 200.0,
            position: 12.5,
            playing: true,
        };
        assert_eq!(
            as_json(&now),
            r#"{"title":"A \"B\"","artist":"C","album":"","artwork":"AQIDBA==","duration":200,"position":12.5,"playing":true}"#
        );
    }
}
