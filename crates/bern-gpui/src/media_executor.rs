//! Built-in native media executor for GPUI when the application does not supply
//! its own `app-kernel`:
//! - macOS: `AVFoundation` (`AVPlayer` + `AVPlayerLayer` placed below GPUI's view).
//! - Windows: `Media Foundation` (`IMFMediaEngine` + D3D11 frame server -> GPUI `RenderImage`).
//! - Linux: `GStreamer` (`playbin` + BGRA `appsink` loaded via `dlopen` -> GPUI `RenderImage`).
//!
//! When an application installs its own media kernel, `media::attach_kernel()` claims
//! execution first and this module stays dormant.

use crate::host::{host, take};
use crate::protocol::Writer;
use bern_ffi::media_contract::{ExecOp, ExecRequest, Observation, Phase};
use gpui::{Bounds, Pixels, RenderImage};
use std::cell::Cell;
use std::sync::Arc;

thread_local! {
    static ATTACHED: Cell<bool> = const { Cell::new(false) };
}

/// Whether the built-in system media executor is active.
pub(crate) fn active() -> bool {
    ATTACHED.get()
}

/// Whether the built-in executor renders via an underlay surface below GPUI's view
/// (requiring transparent hole-punching in the window backdrop).
pub(crate) fn uses_underlay() -> bool {
    cfg!(target_os = "macos") && ATTACHED.get()
}

/// Attach the platform's built-in media executor to the media hub.
pub(crate) fn attach() {
    let backend = if cfg!(target_os = "macos") {
        "avfoundation"
    } else if cfg!(target_os = "windows") {
        "media-foundation"
    } else {
        "gstreamer"
    };
    let mut writer = Writer::new();
    writer.text(backend);
    writer.text("1");
    writer.u8(1); // background_playback
    writer.u8(1); // multi_view
    // SAFETY: the app handle is live during startup.
    let _ = take(unsafe {
        bern_ffi::media::bern_media_executor_attached(
            host().app(),
            writer.bytes.as_ptr(),
            writer.bytes.len(),
        )
    });
    ATTACHED.set(true);
}

/// Detach the built-in media executor and release all players.
pub(crate) fn detach() {
    if !ATTACHED.replace(false) {
        return;
    }
    platform::shutdown();
    let app = host().app();
    if !app.is_null() {
        // SAFETY: the handle is still live until `bern_app_stop`.
        unsafe { bern_ffi::media::bern_media_executor_detached(app) };
    }
}

fn complete_request(request: u64, result: Result<(), (&str, String)>) {
    let mut writer = Writer::new();
    writer.u64(request);
    match result {
        Ok(()) => writer.u8(0),
        Err((code, message)) => {
            writer.u8(1);
            writer.text(code);
            writer.text(&message);
        }
    }
    let app = host().app();
    if !app.is_null() {
        // SAFETY: `app` is live and bytes are valid for this call.
        unsafe {
            bern_ffi::media::bern_media_complete(app, writer.bytes.as_ptr(), writer.bytes.len())
        };
    }
}

fn report_observation(player: &str, version: u64, obs: &Observation) {
    let bytes = obs.encode(player, version);
    let app = host().app();
    if !app.is_null() {
        // SAFETY: `app` is live and bytes are valid for this call.
        unsafe { bern_ffi::media::bern_media_event(app, bytes.as_ptr(), bytes.len()) };
    }
}

/// Execute a request delivered via `on_media_request`.
pub(crate) fn perform(bytes: &[u8]) {
    if !ATTACHED.get() {
        return;
    }
    let Ok(req) = ExecRequest::decode(bytes) else {
        return;
    };
    let outcome = platform::perform(&req);
    complete_request(req.request, outcome);
}

/// Poll active players for state observations and (on frame-server platforms) new frames.
/// Returns `true` if any player is currently active (playing/loading/buffering).
pub(crate) fn poll() -> bool {
    if !ATTACHED.get() {
        return false;
    }
    platform::poll()
}

/// Attach or update a video view for `player` in `window` at `bounds`.
pub(crate) fn place_view(
    token: &str,
    player: &str,
    window: &mut gpui::Window,
    bounds: Bounds<Pixels>,
    corner: f32,
    fit: u32,
) {
    if !ATTACHED.get() {
        return;
    }
    platform::place_view(token, player, window, bounds, corner, fit);
}

/// Remove an attached video view.
pub(crate) fn remove_view(token: &str) {
    platform::remove_view(token);
}

/// Retrieve the latest painted frame for `player` (used on Windows and Linux frame-server paths).
#[allow(unused_variables)]
pub(crate) fn take_frame(player: &str, window: &mut gpui::Window) -> Option<Arc<RenderImage>> {
    platform::take_frame(player, window)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{ExecOp, ExecRequest, Observation, Phase, report_observation};
    use gpui::{Bounds, Pixels, RenderImage};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
    use objc2_app_kit::{NSView, NSWindowOrderingMode};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "CoreMedia", kind = "framework")]
    unsafe extern "C" {
        fn CMTimeMake(value: i64, timescale: i32) -> CMTime;
        fn CMTimeGetSeconds(time: CMTime) -> f64;
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct CMTime {
        value: i64,
        timescale: i32,
        flags: u32,
        epoch: i64,
    }

    // SAFETY: `CMTime` matches CoreMedia's C struct layout `{qiIq}` on 64-bit macOS.
    unsafe impl objc2::Encode for CMTime {
        const ENCODING: objc2::Encoding = objc2::Encoding::Struct(
            "?",
            &[i64::ENCODING, i32::ENCODING, u32::ENCODING, i64::ENCODING],
        );
    }

    struct AvPlayerState {
        player: *mut AnyObject,
        version: u64,
        has_source: bool,
        volume: f64,
        muted: bool,
        last_obs: Option<Observation>,
    }

    struct AvView {
        player_id: String,
        view: Retained<NSView>,
        layer: *mut AnyObject,
    }

    thread_local! {
        static PLAYERS: RefCell<HashMap<String, AvPlayerState>> = RefCell::new(HashMap::new());
        static VIEWS: RefCell<HashMap<String, AvView>> = RefCell::new(HashMap::new());
    }

    fn ensure_player<'a>(
        players: &'a mut HashMap<String, AvPlayerState>,
        id: &str,
    ) -> Option<&'a mut AvPlayerState> {
        if !players.contains_key(id) {
            let cls = AnyClass::get(c"AVPlayer")?;
            let player: *mut AnyObject = unsafe {
                let raw: *mut AnyObject = msg_send![cls, alloc];
                msg_send![raw, init]
            };
            if player.is_null() {
                return None;
            }
            players.insert(
                id.to_string(),
                AvPlayerState {
                    player,
                    version: 0,
                    has_source: false,
                    volume: 1.0,
                    muted: false,
                    last_obs: None,
                },
            );
        }
        players.get_mut(id)
    }

    fn parse_nsurl(raw: &str) -> Option<Retained<NSURL>> {
        if raw.contains("://") {
            let s = NSString::from_str(raw);
            NSURL::URLWithString(&s)
        } else {
            let s = NSString::from_str(raw);
            Some(NSURL::fileURLWithPath(&s))
        }
    }

    fn gravity_name(fit: u32) -> &'static str {
        match fit {
            1 => "AVLayerVideoGravityResizeAspectFill",
            2 => "AVLayerVideoGravityResize",
            _ => "AVLayerVideoGravityResizeAspect",
        }
    }

    fn sync_views_for_player(player_id: &str, player_ptr: *mut AnyObject) {
        VIEWS.with(|views| {
            for view in views.borrow().values() {
                if view.player_id == player_id && !view.layer.is_null() {
                    unsafe {
                        let _: () = msg_send![view.layer, setPlayer: player_ptr];
                    }
                }
            }
        });
    }

    pub(super) fn perform(req: &ExecRequest) -> Result<(), (&'static str, String)> {
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            if matches!(req.op, ExecOp::Release) {
                if let Some(st) = players.remove(&req.player) {
                    sync_views_for_player(&req.player, std::ptr::null_mut());
                    unsafe {
                        let nil: *mut AnyObject = std::ptr::null_mut();
                        let _: () = msg_send![st.player, pause];
                        let _: () = msg_send![st.player, replaceCurrentItemWithPlayerItem: nil];
                        let _: () = msg_send![st.player, release];
                    }
                }
                return Ok(());
            }
            let Some(st) = ensure_player(&mut players, &req.player) else {
                return Err(("executor_unavailable", "AVPlayer is unavailable".into()));
            };
            if req.source_version < st.version {
                return Err(("stale_source", "stale source version".into()));
            }
            match &req.op {
                ExecOp::Load { url } => {
                    let Some(nsurl) = parse_nsurl(url) else {
                        return Err(("invalid_source", format!("`{url}` is not a valid URL")));
                    };
                    let Some(item_cls) = AnyClass::get(c"AVPlayerItem") else {
                        return Err(("executor_unavailable", "AVPlayerItem is unavailable".into()));
                    };
                    let item: *mut AnyObject =
                        unsafe { msg_send![item_cls, playerItemWithURL: &*nsurl] };
                    st.version = req.source_version;
                    st.has_source = true;
                    unsafe {
                        let _: () = msg_send![st.player, replaceCurrentItemWithPlayerItem: item];
                        let _: () = msg_send![st.player, setVolume: st.volume as f32];
                        let _: () = msg_send![st.player, setMuted: Bool::new(st.muted)];
                    }
                    sync_views_for_player(&req.player, st.player);
                    let obs = Observation {
                        phase: Phase::Loading,
                        position_ms: 0,
                        duration_ms: None,
                        seekable: false,
                        volume: st.volume,
                        muted: st.muted,
                        error: None,
                    };
                    st.last_obs = Some(obs.clone());
                    report_observation(&req.player, st.version, &obs);
                }
                ExecOp::Unload => {
                    st.version = req.source_version;
                    st.has_source = false;
                    let nil: *mut AnyObject = std::ptr::null_mut();
                    unsafe {
                        let _: () = msg_send![st.player, pause];
                        let _: () = msg_send![st.player, replaceCurrentItemWithPlayerItem: nil];
                    }
                    let obs = Observation {
                        volume: st.volume,
                        muted: st.muted,
                        ..Observation::idle()
                    };
                    st.last_obs = Some(obs.clone());
                    report_observation(&req.player, st.version, &obs);
                }
                ExecOp::Play => {
                    if !st.has_source {
                        return Err(("no_source", "no media loaded".into()));
                    }
                    // If at end, restart from 0.
                    if st
                        .last_obs
                        .as_ref()
                        .is_some_and(|o| o.phase == Phase::Ended)
                    {
                        unsafe {
                            let zero = CMTimeMake(0, 1000);
                            let _: () = msg_send![st.player, seekToTime: zero];
                        }
                    }
                    unsafe {
                        let _: () = msg_send![st.player, play];
                    }
                }
                ExecOp::Pause => unsafe {
                    let _: () = msg_send![st.player, pause];
                },
                ExecOp::Seek { position_ms } => {
                    if !st.has_source {
                        return Err(("no_source", "no media loaded".into()));
                    }
                    let ms = (*position_ms).max(0);
                    unsafe {
                        let target = CMTimeMake(ms, 1000);
                        let _: () = msg_send![st.player, seekToTime: target];
                    }
                }
                ExecOp::SetVolume { volume } => {
                    st.volume = volume.clamp(0.0, 1.0);
                    unsafe {
                        let _: () = msg_send![st.player, setVolume: st.volume as f32];
                    }
                }
                ExecOp::SetMuted { muted } => {
                    st.muted = *muted;
                    unsafe {
                        let _: () = msg_send![st.player, setMuted: Bool::new(*muted)];
                    }
                }
                ExecOp::Release => unreachable!(),
            }
            Ok(())
        })
    }

    pub(super) fn poll() -> bool {
        let mut any_active = false;
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            for (id, st) in players.iter_mut() {
                if !st.has_source {
                    continue;
                }
                let item: *mut AnyObject = unsafe { msg_send![st.player, currentItem] };
                if item.is_null() {
                    continue;
                }
                let item_status: isize = unsafe { msg_send![item, status] };
                let time_ctrl: isize = unsafe { msg_send![st.player, timeControlStatus] };
                let cur_time: CMTime = unsafe { msg_send![st.player, currentTime] };
                let dur_time: CMTime = unsafe { msg_send![item, duration] };
                let cur_sec = unsafe { CMTimeGetSeconds(cur_time) };
                let dur_sec = unsafe { CMTimeGetSeconds(dur_time) };
                let position_ms = if cur_sec.is_finite() && cur_sec >= 0.0 {
                    (cur_sec * 1000.0).round() as i64
                } else {
                    0
                };
                let duration_ms = if dur_sec.is_finite() && dur_sec > 0.0 {
                    Some((dur_sec * 1000.0).round() as i64)
                } else {
                    None
                };
                let mut error = None;
                let phase = match item_status {
                    0 => Phase::Loading,
                    2 => {
                        let err_obj: *mut AnyObject = unsafe { msg_send![item, error] };
                        if !err_obj.is_null() {
                            let desc: Option<Retained<NSString>> =
                                unsafe { msg_send![err_obj, localizedDescription] };
                            error = desc.map(|s| s.to_string());
                        }
                        if error.is_none() {
                            error = Some("AVPlayerItem failed to load".into());
                        }
                        Phase::Failed
                    }
                    _ => match time_ctrl {
                        2 => Phase::Playing,
                        1 => Phase::Buffering,
                        _ => {
                            if let Some(d) = duration_ms
                                && d > 0
                                && position_ms >= d.saturating_sub(80)
                            {
                                Phase::Ended
                            } else {
                                Phase::Paused
                            }
                        }
                    },
                };
                if matches!(phase, Phase::Playing | Phase::Loading | Phase::Buffering) {
                    any_active = true;
                }
                let obs = Observation {
                    phase,
                    position_ms,
                    duration_ms,
                    seekable: duration_ms.is_some(),
                    volume: st.volume,
                    muted: st.muted,
                    error,
                };
                if st.last_obs.as_ref() != Some(&obs) {
                    st.last_obs = Some(obs.clone());
                    report_observation(id, st.version, &obs);
                }
            }
        });
        any_active
    }

    pub(super) fn place_view(
        token: &str,
        player_id: &str,
        window: &mut gpui::Window,
        bounds: Bounds<Pixels>,
        corner: f32,
        fit: u32,
    ) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let player_ptr = PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            ensure_player(&mut players, player_id).map(|p| p.player)
        });
        let Some(player_ptr) = player_ptr else {
            return;
        };
        VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            if !views.contains_key(token) {
                let Ok(handle) = HasWindowHandle::window_handle(window) else {
                    return;
                };
                let RawWindowHandle::AppKit(h) = handle.as_raw() else {
                    return;
                };
                let gpui_view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
                let Some(content) = (unsafe { gpui_view.superview() }) else {
                    return;
                };
                let Some(layer_cls) = AnyClass::get(c"AVPlayerLayer") else {
                    return;
                };
                let layer: *mut AnyObject =
                    unsafe { msg_send![layer_cls, playerLayerWithPlayer: player_ptr] };
                if layer.is_null() {
                    return;
                }
                unsafe {
                    let _: *mut AnyObject = msg_send![layer, retain];
                }
                let view = NSView::initWithFrame(
                    NSView::alloc(mtm),
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
                );
                unsafe {
                    let _: () = msg_send![&view, setLayer: layer];
                }
                view.setWantsLayer(true);
                let black = objc2_core_graphics::CGColor::new_generic_gray(0.0, 1.0);
                let black_ptr: *const objc2_core_graphics::CGColor = &*black;
                unsafe {
                    let _: () = msg_send![layer, setBackgroundColor: black_ptr];
                    let _: () = msg_send![layer, setMasksToBounds: Bool::YES];
                }
                content.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, None);
                window.set_background_appearance(gpui::WindowBackgroundAppearance::Transparent);
                views.insert(
                    token.to_string(),
                    AvView {
                        player_id: player_id.to_string(),
                        view,
                        layer,
                    },
                );
            }
            if let Some(av) = views.get_mut(token) {
                if av.player_id != player_id {
                    av.player_id = player_id.to_string();
                    unsafe {
                        let _: () = msg_send![av.layer, setPlayer: player_ptr];
                    }
                }
                let gravity = NSString::from_str(gravity_name(fit));
                unsafe {
                    let _: () = msg_send![av.layer, setVideoGravity: &*gravity];
                    let _: () = msg_send![av.layer, setCornerRadius: f64::from(corner)];
                }
                if let Some(content) = unsafe { av.view.superview() } {
                    let height = content.bounds().size.height;
                    let (x, y) = (
                        f64::from(f32::from(bounds.origin.x)),
                        f64::from(f32::from(bounds.origin.y)),
                    );
                    let (w, h) = (
                        f64::from(f32::from(bounds.size.width)),
                        f64::from(f32::from(bounds.size.height)),
                    );
                    let frame = NSRect::new(
                        NSPoint::new(
                            x,
                            if content.isFlipped() {
                                y
                            } else {
                                height - y - h
                            },
                        ),
                        NSSize::new(w, h),
                    );
                    if av.view.frame() != frame {
                        av.view.setFrame(frame);
                    }
                }
            }
        });
    }

    pub(super) fn remove_view(token: &str) {
        VIEWS.with(|views| {
            if let Some(av) = views.borrow_mut().remove(token) {
                let nil: *mut AnyObject = std::ptr::null_mut();
                unsafe {
                    let _: () = msg_send![av.layer, setPlayer: nil];
                    let _: () = msg_send![av.layer, release];
                }
                av.view.removeFromSuperview();
            }
        });
    }

    pub(super) fn take_frame(
        _player: &str,
        _window: &mut gpui::Window,
    ) -> Option<Arc<RenderImage>> {
        None
    }

    pub(super) fn shutdown() {
        VIEWS.with(|views| {
            for (_, av) in views.borrow_mut().drain() {
                let nil: *mut AnyObject = std::ptr::null_mut();
                unsafe {
                    let _: () = msg_send![av.layer, setPlayer: nil];
                    let _: () = msg_send![av.layer, release];
                }
                av.view.removeFromSuperview();
            }
        });
        PLAYERS.with(|players| {
            for (_, st) in players.borrow_mut().drain() {
                let nil: *mut AnyObject = std::ptr::null_mut();
                unsafe {
                    let _: () = msg_send![st.player, pause];
                    let _: () = msg_send![st.player, replaceCurrentItemWithPlayerItem: nil];
                    let _: () = msg_send![st.player, release];
                }
            }
        });
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{ExecOp, ExecRequest, Observation, Phase, report_observation};
    use gpui::{Bounds, Pixels, RenderImage};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use win32::Win32::Foundation::{E_NOINTERFACE, RECT, S_OK};
    use win32::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use win32::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use win32::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    use win32::Win32::Media::MediaFoundation::{
        CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEngine,
        IMFMediaEngineClassFactory, IMFMediaEngineNotify, MF_MEDIA_ENGINE_CALLBACK,
        MF_MEDIA_ENGINE_DXGI_MANAGER, MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, MF_VERSION, MFARGB,
        MFCreateAttributes, MFCreateDXGIDeviceManager, MFSTARTUP_NOSOCKET, MFStartup,
    };
    use win32::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use win32::core::{BSTR, GUID, HRESULT, IUnknown, Interface};

    // Minimal COM `IMFMediaEngineNotify` sink that wakes redraw on media engine events.
    #[repr(C)]
    struct NotifyVtbl {
        qi: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        event_notify: unsafe extern "system" fn(*mut c_void, u32, usize, u32) -> HRESULT,
    }

    #[repr(C)]
    struct NotifyObj {
        vtbl: &'static NotifyVtbl,
        refs: AtomicU32,
    }

    unsafe extern "system" fn notify_qi(
        this: *mut c_void,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        unsafe {
            if *riid == IUnknown::IID || *riid == IMFMediaEngineNotify::IID {
                let obj = &*(this as *const NotifyObj);
                obj.refs.fetch_add(1, Ordering::Relaxed);
                *ppv = this;
                S_OK
            } else {
                *ppv = std::ptr::null_mut();
                E_NOINTERFACE
            }
        }
    }

    unsafe extern "system" fn notify_add_ref(this: *mut c_void) -> u32 {
        unsafe {
            let obj = &*(this as *const NotifyObj);
            obj.refs.fetch_add(1, Ordering::Relaxed) + 1
        }
    }

    unsafe extern "system" fn notify_release(this: *mut c_void) -> u32 {
        unsafe {
            let obj = &*(this as *const NotifyObj);
            let prev = obj.refs.fetch_sub(1, Ordering::Release);
            if prev == 1 {
                std::sync::atomic::fence(Ordering::Acquire);
                drop(Box::from_raw(this as *mut NotifyObj));
                0
            } else {
                prev - 1
            }
        }
    }

    unsafe extern "system" fn notify_event(
        _this: *mut c_void,
        _event: u32,
        _param1: usize,
        _param2: u32,
    ) -> HRESULT {
        crate::host::redraw();
        S_OK
    }

    static NOTIFY_VTBL: NotifyVtbl = NotifyVtbl {
        qi: notify_qi,
        add_ref: notify_add_ref,
        release: notify_release,
        event_notify: notify_event,
    };

    fn make_notify() -> IUnknown {
        let raw = Box::into_raw(Box::new(NotifyObj {
            vtbl: &NOTIFY_VTBL,
            refs: AtomicU32::new(1),
        })) as *mut c_void;
        unsafe { IUnknown::from_raw(raw) }
    }

    struct D3dContext {
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        manager: IMFDXGIDeviceManager,
    }

    struct MfPlayer {
        engine: IMFMediaEngine,
        version: u64,
        has_source: bool,
        volume: f64,
        muted: bool,
        last_obs: Option<Observation>,
        rt_tex: Option<(u32, u32, ID3D11Texture2D, ID3D11Texture2D)>,
        pending_frame: Option<(u32, u32, Vec<u8>)>,
        shown: Option<Arc<RenderImage>>,
    }

    thread_local! {
        static D3D: RefCell<Option<D3dContext>> = const { RefCell::new(None) };
        static PLAYERS: RefCell<HashMap<String, MfPlayer>> = RefCell::new(HashMap::new());
    }

    fn ensure_d3d() -> Option<IMFDXGIDeviceManager> {
        D3D.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                unsafe {
                    let _ = MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET);
                    let mut device: Option<ID3D11Device> = None;
                    let mut context: Option<ID3D11DeviceContext> = None;
                    D3D11CreateDevice(
                        None,
                        D3D_DRIVER_TYPE_HARDWARE,
                        Default::default(),
                        D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                        None,
                        D3D11_SDK_VERSION,
                        Some(&mut device),
                        None,
                        Some(&mut context),
                    )
                    .ok()?;
                    let device = device?;
                    let context = context?;
                    let mut token = 0u32;
                    let mut manager: Option<IMFDXGIDeviceManager> = None;
                    MFCreateDXGIDeviceManager(&mut token, &mut manager).ok()?;
                    let manager = manager?;
                    manager.ResetDevice(&device, token).ok()?;
                    *slot = Some(D3dContext {
                        device,
                        context,
                        manager,
                    });
                }
            }
            slot.as_ref().map(|d| d.manager.clone())
        })
    }

    fn create_engine() -> Option<IMFMediaEngine> {
        let manager = ensure_d3d()?;
        unsafe {
            let factory: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)
                    .ok()?;
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 3).ok()?;
            let attrs = attrs?;
            let notify = make_notify();
            attrs.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify).ok()?;
            attrs
                .SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, &manager)
                .ok()?;
            attrs
                .SetUINT32(
                    &MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT,
                    DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32,
                )
                .ok()?;
            factory.CreateInstance(0, &attrs).ok()
        }
    }

    pub(super) fn perform(req: &ExecRequest) -> Result<(), (&'static str, String)> {
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            if matches!(req.op, ExecOp::Release) {
                if let Some(st) = players.remove(&req.player) {
                    unsafe {
                        let _ = st.engine.Shutdown();
                    }
                }
                return Ok(());
            }
            if !players.contains_key(&req.player) {
                let Some(engine) = create_engine() else {
                    return Err((
                        "executor_unavailable",
                        "Media Foundation IMFMediaEngine unavailable".into(),
                    ));
                };
                players.insert(
                    req.player.clone(),
                    MfPlayer {
                        engine,
                        version: 0,
                        has_source: false,
                        volume: 1.0,
                        muted: false,
                        last_obs: None,
                        rt_tex: None,
                        pending_frame: None,
                        shown: None,
                    },
                );
            }
            let st = players.get_mut(&req.player).unwrap();
            if req.source_version < st.version {
                return Err(("stale_source", "stale source version".into()));
            }
            match &req.op {
                ExecOp::Load { url } => {
                    st.version = req.source_version;
                    st.has_source = true;
                    let bstr = BSTR::from(url.as_str());
                    unsafe {
                        let _ = st.engine.SetSource(&bstr);
                        let _ = st.engine.Load();
                        let _ = st.engine.SetVolume(st.volume);
                        let _ = st.engine.SetMuted(st.muted);
                    }
                    let obs = Observation {
                        phase: Phase::Loading,
                        position_ms: 0,
                        duration_ms: None,
                        seekable: false,
                        volume: st.volume,
                        muted: st.muted,
                        error: None,
                    };
                    st.last_obs = Some(obs.clone());
                    report_observation(&req.player, st.version, &obs);
                }
                ExecOp::Unload => {
                    st.version = req.source_version;
                    st.has_source = false;
                    let empty = BSTR::from("");
                    unsafe {
                        let _ = st.engine.Pause();
                        let _ = st.engine.SetSource(&empty);
                    }
                    let obs = Observation {
                        volume: st.volume,
                        muted: st.muted,
                        ..Observation::idle()
                    };
                    st.last_obs = Some(obs.clone());
                    report_observation(&req.player, st.version, &obs);
                }
                ExecOp::Play => {
                    if !st.has_source {
                        return Err(("no_source", "no media loaded".into()));
                    }
                    unsafe {
                        if st.engine.IsEnded().as_bool() {
                            let _ = st.engine.SetCurrentTime(0.0);
                        }
                        let _ = st.engine.Play();
                    }
                }
                ExecOp::Pause => unsafe {
                    let _ = st.engine.Pause();
                },
                ExecOp::Seek { position_ms } => {
                    if !st.has_source {
                        return Err(("no_source", "no media loaded".into()));
                    }
                    unsafe {
                        let _ = st
                            .engine
                            .SetCurrentTime((*position_ms).max(0) as f64 / 1000.0);
                    }
                }
                ExecOp::SetVolume { volume } => {
                    st.volume = volume.clamp(0.0, 1.0);
                    unsafe {
                        let _ = st.engine.SetVolume(st.volume);
                    }
                }
                ExecOp::SetMuted { muted } => {
                    st.muted = *muted;
                    unsafe {
                        let _ = st.engine.SetMuted(*muted);
                    }
                }
                ExecOp::Release => unreachable!(),
            }
            Ok(())
        })
    }

    fn pull_frame(st: &mut MfPlayer) -> bool {
        unsafe {
            if !st.engine.HasVideo().as_bool() || st.engine.OnVideoStreamTick().is_err() {
                return false;
            }
            let (mut vw, mut vh) = (0u32, 0u32);
            if st
                .engine
                .GetNativeVideoSize(Some(&mut vw), Some(&mut vh))
                .is_err()
                || vw == 0
                || vh == 0
            {
                return false;
            }
            D3D.with(|slot| {
                let slot = slot.borrow();
                let Some(d3d) = slot.as_ref() else {
                    return false;
                };
                let needs_alloc = st
                    .rt_tex
                    .as_ref()
                    .is_none_or(|(w, h, _, _)| *w != vw || *h != vh);
                if needs_alloc {
                    let rt_desc = D3D11_TEXTURE2D_DESC {
                        Width: vw,
                        Height: vh,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                        CPUAccessFlags: 0,
                        MiscFlags: 0,
                    };
                    let st_desc = D3D11_TEXTURE2D_DESC {
                        Usage: D3D11_USAGE_STAGING,
                        BindFlags: 0,
                        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                        ..rt_desc
                    };
                    let mut rt: Option<ID3D11Texture2D> = None;
                    let mut staging: Option<ID3D11Texture2D> = None;
                    if d3d
                        .device
                        .CreateTexture2D(&rt_desc, None, Some(&mut rt))
                        .is_err()
                        || d3d
                            .device
                            .CreateTexture2D(&st_desc, None, Some(&mut staging))
                            .is_err()
                    {
                        return false;
                    }
                    if let (Some(rt), Some(staging)) = (rt, staging) {
                        st.rt_tex = Some((vw, vh, rt, staging));
                    } else {
                        return false;
                    }
                }
                let Some((w, h, rt, staging)) = st.rt_tex.as_ref() else {
                    return false;
                };
                let dst = RECT {
                    left: 0,
                    top: 0,
                    right: *w as i32,
                    bottom: *h as i32,
                };
                let border = MFARGB {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbAlpha: 255,
                };
                let Ok(unk) = rt.cast::<IUnknown>() else {
                    return false;
                };
                if st
                    .engine
                    .TransferVideoFrame(&unk, None, &dst, Some(&border))
                    .is_err()
                {
                    return false;
                }
                d3d.context.CopyResource(staging, rt);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                if d3d
                    .context
                    .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .is_err()
                {
                    return false;
                }
                let row_bytes = (*w * 4) as usize;
                let mut pixels = vec![0u8; (*w * *h * 4) as usize];
                let src_base = mapped.pData as *const u8;
                let pitch = mapped.RowPitch as usize;
                for y in 0..(*h as usize) {
                    std::ptr::copy_nonoverlapping(
                        src_base.add(y * pitch),
                        pixels.as_mut_ptr().add(y * row_bytes),
                        row_bytes,
                    );
                }
                d3d.context.Unmap(staging, 0);
                st.pending_frame = Some((*w, *h, pixels));
                true
            })
        }
    }

    pub(super) fn poll() -> bool {
        let mut any_active = false;
        let mut drew = false;
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            for (id, st) in players.iter_mut() {
                if !st.has_source {
                    continue;
                }
                if pull_frame(st) {
                    drew = true;
                }
                let ready = unsafe { st.engine.GetReadyState() };
                let paused = unsafe { st.engine.IsPaused().as_bool() };
                let ended = unsafe { st.engine.IsEnded().as_bool() };
                let pos = unsafe { st.engine.GetCurrentTime() };
                let dur = unsafe { st.engine.GetDuration() };
                let has_err = unsafe { st.engine.GetError().is_ok() };
                let position_ms = if pos.is_finite() && pos >= 0.0 {
                    (pos * 1000.0).round() as i64
                } else {
                    0
                };
                let duration_ms = if dur.is_finite() && dur > 0.0 {
                    Some((dur * 1000.0).round() as i64)
                } else {
                    None
                };
                let phase = if has_err {
                    Phase::Failed
                } else if ended {
                    Phase::Ended
                } else if ready < 2 {
                    Phase::Loading
                } else if !paused && ready < 3 {
                    Phase::Buffering
                } else if !paused {
                    Phase::Playing
                } else {
                    Phase::Paused
                };
                if matches!(phase, Phase::Playing | Phase::Loading | Phase::Buffering) {
                    any_active = true;
                }
                let obs = Observation {
                    phase,
                    position_ms,
                    duration_ms,
                    seekable: duration_ms.is_some(),
                    volume: st.volume,
                    muted: st.muted,
                    error: has_err.then(|| "Media Foundation playback error".into()),
                };
                if st.last_obs.as_ref() != Some(&obs) {
                    st.last_obs = Some(obs.clone());
                    report_observation(id, st.version, &obs);
                }
            }
        });
        if drew {
            crate::host::redraw();
        }
        any_active
    }

    pub(super) fn place_view(
        _token: &str,
        _player: &str,
        _window: &mut gpui::Window,
        _bounds: Bounds<Pixels>,
        _corner: f32,
        _fit: u32,
    ) {
    }

    pub(super) fn remove_view(_token: &str) {}

    pub(super) fn take_frame(player: &str, window: &mut gpui::Window) -> Option<Arc<RenderImage>> {
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            let st = players.get_mut(player)?;
            if let Some((w, h, pixels)) = st.pending_frame.take()
                && let Some(buffer) = image::RgbaImage::from_raw(w, h, pixels)
            {
                let img = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                    buffer
                )]));
                if let Some(old) = st.shown.replace(img) {
                    let _ = window.drop_image(old);
                }
            }
            st.shown.clone()
        })
    }

    pub(super) fn shutdown() {
        PLAYERS.with(|players| {
            for (_, st) in players.borrow_mut().drain() {
                unsafe {
                    let _ = st.engine.Shutdown();
                }
            }
        });
        D3D.with(|slot| *slot.borrow_mut() = None);
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod platform {
    use super::{ExecOp, ExecRequest, Observation, Phase, report_observation};
    use gpui::{Bounds, Pixels, RenderImage};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::{CStr, CString, c_char, c_double, c_int, c_uint, c_void};
    use std::sync::{Arc, OnceLock};

    const GST_STATE_NULL: c_int = 1;
    const GST_STATE_PAUSED: c_int = 3;
    const GST_STATE_PLAYING: c_int = 4;
    const GST_FORMAT_TIME: c_int = 3;
    const GST_SEEK_FLAG_FLUSH: c_int = 1;
    const GST_SEEK_FLAG_KEY_UNIT: c_int = 4;
    const GST_MESSAGE_EOS: c_int = 1 << 0;
    const GST_MESSAGE_ERROR: c_int = 1 << 1;
    const GST_MAP_READ: c_int = 1;

    #[repr(C)]
    struct GstMapInfo {
        memory: *mut c_void,
        flags: c_int,
        data: *mut u8,
        size: usize,
        maxsize: usize,
        user_data: [*mut c_void; 4],
        _gst_reserved: [*mut c_void; 4],
    }

    struct GstApi {
        gst_init_check:
            unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char, *mut *mut c_void) -> c_int,
        gst_element_factory_make: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void,
        gst_element_set_state: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
        gst_element_get_state:
            unsafe extern "C" fn(*mut c_void, *mut c_int, *mut c_int, u64) -> c_int,
        gst_element_seek_simple: unsafe extern "C" fn(*mut c_void, c_int, c_int, i64) -> c_int,
        gst_element_query_position: unsafe extern "C" fn(*mut c_void, c_int, *mut i64) -> c_int,
        gst_element_query_duration: unsafe extern "C" fn(*mut c_void, c_int, *mut i64) -> c_int,
        gst_element_get_bus: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        gst_bus_pop_filtered: unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void,
        gst_mini_object_unref: unsafe extern "C" fn(*mut c_void),
        gst_object_unref: unsafe extern "C" fn(*mut c_void),
        gst_caps_from_string: unsafe extern "C" fn(*const c_char) -> *mut c_void,
        gst_caps_get_structure: unsafe extern "C" fn(*mut c_void, c_uint) -> *mut c_void,
        gst_structure_get_int:
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_int) -> c_int,
        gst_sample_get_buffer: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        gst_sample_get_caps: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        gst_buffer_map: unsafe extern "C" fn(*mut c_void, *mut GstMapInfo, c_int) -> c_int,
        gst_buffer_unmap: unsafe extern "C" fn(*mut c_void, *mut GstMapInfo),
        gst_app_sink_try_pull_sample: unsafe extern "C" fn(*mut c_void, u64) -> *mut c_void,
        g_object_set_str:
            unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_void),
        g_object_set_ptr:
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void, *const c_void),
        g_object_set_f64: unsafe extern "C" fn(*mut c_void, *const c_char, c_double, *const c_void),
        g_object_set_int: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const c_void),
    }

    static API: OnceLock<Option<GstApi>> = OnceLock::new();

    unsafe fn sym<T: Copy>(lib: *mut c_void, name: &CStr) -> Option<T> {
        unsafe {
            let p = libc::dlsym(lib, name.as_ptr());
            if p.is_null() {
                None
            } else {
                Some(std::mem::transmute_copy(&p))
            }
        }
    }

    fn gst() -> Option<&'static GstApi> {
        API.get_or_init(|| unsafe {
            let flags = libc::RTLD_LAZY | libc::RTLD_LOCAL;
            let gst = libc::dlopen(c"libgstreamer-1.0.so.0".as_ptr(), flags);
            let app = libc::dlopen(c"libgstapp-1.0.so.0".as_ptr(), flags);
            let gobj = libc::dlopen(c"libgobject-2.0.so.0".as_ptr(), flags);
            if gst.is_null() || app.is_null() || gobj.is_null() {
                return None;
            }
            let init: unsafe extern "C" fn(
                *mut c_int,
                *mut *mut *mut c_char,
                *mut *mut c_void,
            ) -> c_int = sym(gst, c"gst_init_check")?;
            let mut argc = 0;
            let mut argv: *mut *mut c_char = std::ptr::null_mut();
            let mut err: *mut c_void = std::ptr::null_mut();
            if init(&mut argc, &mut argv, &mut err) == 0 {
                return None;
            }
            let set_raw = libc::dlsym(gobj, c"g_object_set".as_ptr());
            if set_raw.is_null() {
                return None;
            }
            Some(GstApi {
                gst_init_check: init,
                gst_element_factory_make: sym(gst, c"gst_element_factory_make")?,
                gst_element_set_state: sym(gst, c"gst_element_set_state")?,
                gst_element_get_state: sym(gst, c"gst_element_get_state")?,
                gst_element_seek_simple: sym(gst, c"gst_element_seek_simple")?,
                gst_element_query_position: sym(gst, c"gst_element_query_position")?,
                gst_element_query_duration: sym(gst, c"gst_element_query_duration")?,
                gst_element_get_bus: sym(gst, c"gst_element_get_bus")?,
                gst_bus_pop_filtered: sym(gst, c"gst_bus_pop_filtered")?,
                gst_mini_object_unref: sym(gst, c"gst_mini_object_unref")?,
                gst_object_unref: sym(gst, c"gst_object_unref")?,
                gst_caps_from_string: sym(gst, c"gst_caps_from_string")?,
                gst_caps_get_structure: sym(gst, c"gst_caps_get_structure")?,
                gst_structure_get_int: sym(gst, c"gst_structure_get_int")?,
                gst_sample_get_buffer: sym(gst, c"gst_sample_get_buffer")?,
                gst_sample_get_caps: sym(gst, c"gst_sample_get_caps")?,
                gst_buffer_map: sym(gst, c"gst_buffer_map")?,
                gst_buffer_unmap: sym(gst, c"gst_buffer_unmap")?,
                gst_app_sink_try_pull_sample: sym(app, c"gst_app_sink_try_pull_sample")?,
                g_object_set_str: std::mem::transmute_copy(&set_raw),
                g_object_set_ptr: std::mem::transmute_copy(&set_raw),
                g_object_set_f64: std::mem::transmute_copy(&set_raw),
                g_object_set_int: std::mem::transmute_copy(&set_raw),
            })
        })
        .as_ref()
    }

    struct GstPlayer {
        playbin: *mut c_void,
        sink: *mut c_void,
        bus: *mut c_void,
        version: u64,
        has_source: bool,
        ended: bool,
        failed: Option<String>,
        buffering: bool,
        volume: f64,
        muted: bool,
        last_obs: Option<Observation>,
        pending_frame: Option<(u32, u32, Vec<u8>)>,
        shown: Option<Arc<RenderImage>>,
    }

    thread_local! {
        static PLAYERS: RefCell<HashMap<String, GstPlayer>> = RefCell::new(HashMap::new());
    }

    fn create_player(api: &GstApi) -> Option<GstPlayer> {
        unsafe {
            let playbin = (api.gst_element_factory_make)(c"playbin".as_ptr(), std::ptr::null());
            let sink = (api.gst_element_factory_make)(c"appsink".as_ptr(), std::ptr::null());
            if playbin.is_null() || sink.is_null() {
                if !playbin.is_null() {
                    (api.gst_object_unref)(playbin);
                }
                if !sink.is_null() {
                    (api.gst_object_unref)(sink);
                }
                return None;
            }
            let caps = (api.gst_caps_from_string)(c"video/x-raw,format=BGRA".as_ptr());
            if !caps.is_null() {
                (api.g_object_set_ptr)(sink, c"caps".as_ptr(), caps, std::ptr::null());
                (api.gst_mini_object_unref)(caps);
            }
            (api.g_object_set_int)(sink, c"max-buffers".as_ptr(), 1, std::ptr::null());
            (api.g_object_set_int)(sink, c"drop".as_ptr(), 1, std::ptr::null());
            (api.g_object_set_ptr)(playbin, c"video-sink".as_ptr(), sink, std::ptr::null());
            let bus = (api.gst_element_get_bus)(playbin);
            Some(GstPlayer {
                playbin,
                sink,
                bus,
                version: 0,
                has_source: false,
                ended: false,
                failed: None,
                buffering: false,
                volume: 1.0,
                muted: false,
                last_obs: None,
                pending_frame: None,
                shown: None,
            })
        }
    }

    fn destroy_player(api: &GstApi, p: GstPlayer) {
        unsafe {
            (api.gst_element_set_state)(p.playbin, GST_STATE_NULL);
            if !p.bus.is_null() {
                (api.gst_object_unref)(p.bus);
            }
            (api.gst_object_unref)(p.playbin);
        }
    }

    pub(super) fn perform(req: &ExecRequest) -> Result<(), (&'static str, String)> {
        let Some(api) = gst() else {
            return Err(("executor_unavailable", "GStreamer is not installed".into()));
        };
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            if matches!(req.op, ExecOp::Release) {
                if let Some(p) = players.remove(&req.player) {
                    destroy_player(api, p);
                }
                return Ok(());
            }
            if !players.contains_key(&req.player) {
                let Some(p) = create_player(api) else {
                    return Err((
                        "executor_unavailable",
                        "cannot create GStreamer playbin".into(),
                    ));
                };
                players.insert(req.player.clone(), p);
            }
            let st = players.get_mut(&req.player).unwrap();
            if req.source_version < st.version {
                return Err(("stale_source", "stale source version".into()));
            }
            match &req.op {
                ExecOp::Load { url } => {
                    let uri = if url.contains("://") {
                        url.clone()
                    } else {
                        crate::files::path_to_file_url(std::path::Path::new(url))
                    };
                    let Ok(c_uri) = CString::new(uri) else {
                        return Err(("invalid_source", "invalid URL".into()));
                    };
                    st.version = req.source_version;
                    st.has_source = true;
                    st.ended = false;
                    st.failed = None;
                    st.buffering = false;
                    unsafe {
                        (api.gst_element_set_state)(st.playbin, GST_STATE_NULL);
                        (api.g_object_set_str)(
                            st.playbin,
                            c"uri".as_ptr(),
                            c_uri.as_ptr(),
                            std::ptr::null(),
                        );
                        (api.g_object_set_f64)(
                            st.playbin,
                            c"volume".as_ptr(),
                            st.volume,
                            std::ptr::null(),
                        );
                        (api.g_object_set_int)(
                            st.playbin,
                            c"mute".as_ptr(),
                            c_int::from(st.muted),
                            std::ptr::null(),
                        );
                        (api.gst_element_set_state)(st.playbin, GST_STATE_PAUSED);
                    }
                    let obs = Observation {
                        phase: Phase::Loading,
                        position_ms: 0,
                        duration_ms: None,
                        seekable: false,
                        volume: st.volume,
                        muted: st.muted,
                        error: None,
                    };
                    st.last_obs = Some(obs.clone());
                    report_observation(&req.player, st.version, &obs);
                }
                ExecOp::Unload => {
                    st.version = req.source_version;
                    st.has_source = false;
                    st.ended = false;
                    st.failed = None;
                    unsafe {
                        (api.gst_element_set_state)(st.playbin, GST_STATE_NULL);
                    }
                    let obs = Observation {
                        volume: st.volume,
                        muted: st.muted,
                        ..Observation::idle()
                    };
                    st.last_obs = Some(obs.clone());
                    report_observation(&req.player, st.version, &obs);
                }
                ExecOp::Play => {
                    if !st.has_source {
                        return Err(("no_source", "no media loaded".into()));
                    }
                    unsafe {
                        if st.ended {
                            (api.gst_element_seek_simple)(
                                st.playbin,
                                GST_FORMAT_TIME,
                                GST_SEEK_FLAG_FLUSH | GST_SEEK_FLAG_KEY_UNIT,
                                0,
                            );
                            st.ended = false;
                        }
                        (api.gst_element_set_state)(st.playbin, GST_STATE_PLAYING);
                    }
                }
                ExecOp::Pause => unsafe {
                    (api.gst_element_set_state)(st.playbin, GST_STATE_PAUSED);
                },
                ExecOp::Seek { position_ms } => {
                    if !st.has_source {
                        return Err(("no_source", "no media loaded".into()));
                    }
                    let ns = (*position_ms).max(0).saturating_mul(1_000_000);
                    unsafe {
                        (api.gst_element_seek_simple)(
                            st.playbin,
                            GST_FORMAT_TIME,
                            GST_SEEK_FLAG_FLUSH | GST_SEEK_FLAG_KEY_UNIT,
                            ns,
                        );
                    }
                    st.ended = false;
                }
                ExecOp::SetVolume { volume } => {
                    st.volume = volume.clamp(0.0, 1.0);
                    unsafe {
                        (api.g_object_set_f64)(
                            st.playbin,
                            c"volume".as_ptr(),
                            st.volume,
                            std::ptr::null(),
                        );
                    }
                }
                ExecOp::SetMuted { muted } => {
                    st.muted = *muted;
                    unsafe {
                        (api.g_object_set_int)(
                            st.playbin,
                            c"mute".as_ptr(),
                            c_int::from(*muted),
                            std::ptr::null(),
                        );
                    }
                }
                ExecOp::Release => unreachable!(),
            }
            Ok(())
        })
    }

    pub(super) fn poll() -> bool {
        let Some(api) = gst() else {
            return false;
        };
        let mut any_active = false;
        let mut drew = false;
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            for (id, st) in players.iter_mut() {
                if !st.has_source {
                    continue;
                }
                // Drain EOS and ERROR messages cleanly via filtered pops.
                if !st.bus.is_null() {
                    loop {
                        let msg = unsafe { (api.gst_bus_pop_filtered)(st.bus, GST_MESSAGE_EOS) };
                        if msg.is_null() {
                            break;
                        }
                        st.ended = true;
                        unsafe { (api.gst_mini_object_unref)(msg) };
                    }
                    loop {
                        let msg = unsafe { (api.gst_bus_pop_filtered)(st.bus, GST_MESSAGE_ERROR) };
                        if msg.is_null() {
                            break;
                        }
                        st.failed = Some("GStreamer playback error".into());
                        unsafe { (api.gst_mini_object_unref)(msg) };
                    }
                }
                // Try pulling one BGRA video frame non-blockingly.
                let sample = unsafe { (api.gst_app_sink_try_pull_sample)(st.sink, 0) };
                if !sample.is_null() {
                    unsafe {
                        let caps = (api.gst_sample_get_caps)(sample);
                        let buf = (api.gst_sample_get_buffer)(sample);
                        if !caps.is_null() && !buf.is_null() {
                            let structure = (api.gst_caps_get_structure)(caps, 0);
                            let (mut w, mut h) = (0 as c_int, 0 as c_int);
                            if !structure.is_null()
                                && (api.gst_structure_get_int)(structure, c"width".as_ptr(), &mut w)
                                    != 0
                                && (api.gst_structure_get_int)(
                                    structure,
                                    c"height".as_ptr(),
                                    &mut h,
                                ) != 0
                                && w > 0
                                && h > 0
                            {
                                let mut map: GstMapInfo = std::mem::zeroed();
                                if (api.gst_buffer_map)(buf, &mut map, GST_MAP_READ) != 0 {
                                    let need = (w as usize) * (h as usize) * 4;
                                    if !map.data.is_null() && map.size >= need {
                                        let pixels =
                                            std::slice::from_raw_parts(map.data, need).to_vec();
                                        st.pending_frame = Some((w as u32, h as u32, pixels));
                                        drew = true;
                                    }
                                    (api.gst_buffer_unmap)(buf, &mut map);
                                }
                            }
                        }
                        (api.gst_mini_object_unref)(sample);
                    }
                }
                let (mut cur, mut pend) = (0 as c_int, 0 as c_int);
                unsafe { (api.gst_element_get_state)(st.playbin, &mut cur, &mut pend, 0) };
                let (mut pos_ns, mut dur_ns) = (0i64, 0i64);
                let has_pos = unsafe {
                    (api.gst_element_query_position)(st.playbin, GST_FORMAT_TIME, &mut pos_ns) != 0
                };
                let has_dur = unsafe {
                    (api.gst_element_query_duration)(st.playbin, GST_FORMAT_TIME, &mut dur_ns) != 0
                };
                let position_ms = if has_pos && pos_ns >= 0 {
                    pos_ns / 1_000_000
                } else {
                    0
                };
                let duration_ms = (has_dur && dur_ns > 0).then_some(dur_ns / 1_000_000);
                if let Some(d) = duration_ms
                    && d > 0
                    && position_ms >= d.saturating_sub(80)
                    && cur == GST_STATE_PLAYING
                {
                    st.ended = true;
                }
                let phase = if st.failed.is_some() {
                    Phase::Failed
                } else if st.ended {
                    Phase::Ended
                } else if st.buffering {
                    Phase::Buffering
                } else if cur == GST_STATE_PLAYING {
                    Phase::Playing
                } else if cur == GST_STATE_PAUSED {
                    Phase::Paused
                } else {
                    Phase::Loading
                };
                if matches!(phase, Phase::Playing | Phase::Loading | Phase::Buffering) {
                    any_active = true;
                }
                let obs = Observation {
                    phase,
                    position_ms,
                    duration_ms,
                    seekable: duration_ms.is_some(),
                    volume: st.volume,
                    muted: st.muted,
                    error: st.failed.clone(),
                };
                if st.last_obs.as_ref() != Some(&obs) {
                    st.last_obs = Some(obs.clone());
                    report_observation(id, st.version, &obs);
                }
            }
        });
        if drew {
            crate::host::redraw();
        }
        any_active
    }

    pub(super) fn place_view(
        _token: &str,
        _player: &str,
        _window: &mut gpui::Window,
        _bounds: Bounds<Pixels>,
        _corner: f32,
        _fit: u32,
    ) {
    }

    pub(super) fn remove_view(_token: &str) {}

    pub(super) fn take_frame(player: &str, window: &mut gpui::Window) -> Option<Arc<RenderImage>> {
        PLAYERS.with(|players| {
            let mut players = players.borrow_mut();
            let st = players.get_mut(player)?;
            if let Some((w, h, pixels)) = st.pending_frame.take()
                && let Some(buffer) = image::RgbaImage::from_raw(w, h, pixels)
            {
                let img = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                    buffer
                )]));
                if let Some(old) = st.shown.replace(img) {
                    let _ = window.drop_image(old);
                }
            }
            st.shown.clone()
        })
    }

    pub(super) fn shutdown() {
        let Some(api) = gst() else {
            return;
        };
        PLAYERS.with(|players| {
            for (_, p) in players.borrow_mut().drain() {
                destroy_player(api, p);
            }
        });
    }
}
