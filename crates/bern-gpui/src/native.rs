//! What GPUI leaves to the platform: the application's icon, hiding the
//! pointer, whether motion is reduced, the about panel, the window
//! commands of the menu bar, platform window geometry/state facts,
//! window presentation commands and window screenshots.

use std::cell::RefCell;

/// Platform window facts reported to `bern_window_facts`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowFacts {
    /// 0 = Visible, 1 = Minimized, 2 = Hidden.
    pub(crate) visibility: u8,
    /// 0 = Normal, 1 = Maximized, 2 = FullScreen.
    pub(crate) size: u8,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) focused: bool,
    /// Bitmask of `WindowAction::ALL`:
    /// 0 Activate, 1 Close, 2 Minimize, 3 Restore, 4 Maximize, 5 Move, 6 Resize.
    pub(crate) actions: u32,
}

pub(crate) const ACTION_ACTIVATE: u32 = 1 << 0;
pub(crate) const ACTION_CLOSE: u32 = 1 << 1;
pub(crate) const ACTION_MINIMIZE: u32 = 1 << 2;
pub(crate) const ACTION_RESTORE: u32 = 1 << 3;
pub(crate) const ACTION_MAXIMIZE: u32 = 1 << 4;
pub(crate) const ACTION_MOVE: u32 = 1 << 5;
pub(crate) const ACTION_RESIZE: u32 = 1 << 6;

/// Direct touch input, not a trackpad. GPUI has no capability query, so
/// inspect the host's device inventory without opening or seizing devices.
/// Recheck on later renders so a cached answer does not survive hot-plugging.
pub(crate) fn touchscreen() -> bool {
    use std::time::{Duration, Instant};
    thread_local! {
        static TOUCH: RefCell<Option<(Instant, bool)>> = const { RefCell::new(None) };
    }
    TOUCH.with(|cached| {
        if let Some((at, value)) = *cached.borrow()
            && at.elapsed() < Duration::from_secs(1)
        {
            return value;
        }
        let found = touchscreen_query();
        if found.is_none() {
            static REPORTED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!(
                    "bern: direct touchscreen inventory is unavailable; window.touch defaults to false (not a device capability determination)"
                );
            }
        }
        let value = found.unwrap_or(false);
        *cached.borrow_mut() = Some((Instant::now(), value));
        value
    })
}

#[cfg(target_os = "macos")]
fn touchscreen_query() -> Option<bool> {
    mac::touchscreen()
}

#[cfg(target_os = "windows")]
fn touchscreen_query() -> Option<bool> {
    use win32::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_DIGITIZER};
    // NID_READY and NID_INTEGRATED_TOUCH | NID_EXTERNAL_TOUCH. Pen-only
    // digitizers and indirect precision touchpads do not satisfy this mask.
    let capabilities = unsafe { GetSystemMetrics(SM_DIGITIZER) };
    Some(capabilities & 0x80 != 0 && capabilities & 0x03 != 0)
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn touchscreen_query() -> Option<bool> {
    let devices = std::fs::read_dir("/sys/class/input").ok()?;
    let mut unknown = false;
    for device in devices.flatten() {
        if !device.file_name().to_string_lossy().starts_with("event") {
            continue;
        }
        let properties = std::fs::read_to_string(device.path().join("dev"))
            .ok()
            .and_then(|dev| {
                std::fs::read_to_string(format!("/run/udev/data/c{}", dev.trim())).ok()
            });
        match properties {
            Some(properties) if direct_touch_properties(&properties) => return Some(true),
            Some(_) => {}
            None => unknown = true,
        }
    }
    (!unknown).then_some(false)
}

#[cfg(any(target_os = "linux", target_os = "freebsd", test))]
fn direct_touch_properties(properties: &str) -> bool {
    properties
        .lines()
        .any(|line| line == "E:ID_INPUT_TOUCHSCREEN=1")
        && !properties
            .lines()
            .any(|line| line == "E:ID_INPUT_TOUCHPAD=1")
}

thread_local! {
    /// The picture the application shows as its icon.
    static ICON: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

/// Show the picture `name` among the resources (`icons/<name>.png`) as the
/// application's icon; none keeps the system's. In an app bundle the icons
/// are the bundle's own, by asset name (`AppIcon-<name>`), which the
/// system draws in its shape, as the SwiftUI target shows them; the
/// application's own icon is the bundle's.
pub(crate) fn show_icon(name: Option<&'static str>) {
    let name = name.filter(|name| {
        !(crate::host::bundled() && Some(*name) == crate::host::host().def.app_icon)
    });
    let changed = ICON.with(|icon| icon.replace(name) != name);
    #[cfg(target_os = "macos")]
    if changed {
        mac::set_icon(name.map(|name| {
            let asset = std::path::Path::new(name)
                .file_stem()
                .filter(|_| crate::host::bundled())
                .map(|stem| format!("AppIcon-{}", stem.to_string_lossy()));
            (
                asset,
                std::path::Path::new(&crate::host::resources_dir()).join(name),
            )
        }));
    }
    #[cfg(target_os = "windows")]
    if changed {
        windows_os::set_icon(
            name.map(|name| std::path::Path::new(&crate::host::resources_dir()).join(name)),
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = changed;
}

/// Apply the current icon to a newly created Windows window when an alternate
/// icon is active.
#[allow(unused_variables)]
pub(crate) fn apply_window_icon(window: &gpui::Window) {
    #[cfg(target_os = "windows")]
    windows_os::apply_window_icon(window);
}

/// Hide the pointer until it moves.
pub(crate) fn hide_pointer() {
    #[cfg(target_os = "macos")]
    mac::hide_pointer();
    // The next move sets the pointer of what is under it again.
    #[cfg(target_os = "windows")]
    // SAFETY: on the thread of the window under the pointer.
    unsafe {
        win32::Win32::UI::WindowsAndMessaging::SetCursor(None);
    }
}

/// Read plain text from the system clipboard (`paste:` on controls).
pub(crate) fn clipboard_text(cx: &mut gpui::App) -> Option<String> {
    cx.read_from_clipboard()
        .and_then(|item| item.text().map(|text| text.to_string()))
}

/// Move a window just opened outside every display (`BERN_WINDOW_OFFSCREEN`),
/// where Windows does not open it; macOS opens it there, and Linux
/// desktops place windows themselves.
#[allow(unused_variables)]
pub(crate) fn move_offscreen(window: &gpui::Window) {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use win32::Win32::UI::WindowsAndMessaging::{
            SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        };
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = win32::Win32::Foundation::HWND(handle.hwnd.get() as *mut _);
        // SAFETY: the window is open; nothing else is changed.
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                -10000,
                -10000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }
}

/// Inspect the platform facts of `window` for `bern_window_facts`.
pub(crate) fn window_facts(window: &gpui::Window, resizable: bool) -> WindowFacts {
    #[cfg(target_os = "macos")]
    {
        if let Some(facts) = mac::window_facts(window, resizable) {
            return facts;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(facts) = windows_os::window_facts(window, resizable) {
            return facts;
        }
    }
    let bounds = window.bounds();
    let size_state = if window.is_fullscreen() {
        2
    } else if window.is_maximized() {
        1
    } else {
        0
    };
    let mut actions = ACTION_ACTIVATE | ACTION_CLOSE | ACTION_MINIMIZE | ACTION_RESTORE;
    if resizable {
        actions |= ACTION_MAXIMIZE | ACTION_RESIZE;
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if linux::is_x11(window) {
        actions |= ACTION_MOVE;
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        actions |= ACTION_MOVE;
    }
    WindowFacts {
        visibility: 0,
        size: size_state,
        x: f64::from(f32::from(bounds.origin.x)),
        y: f64::from(f32::from(bounds.origin.y)),
        width: f64::from(f32::from(bounds.size.width)),
        height: f64::from(f32::from(bounds.size.height)),
        focused: window.is_window_active(),
        actions,
    }
}

pub(crate) fn minimize_window(window: &mut gpui::Window) -> Result<(), crate::Failure> {
    window.minimize_window();
    Ok(())
}

pub(crate) fn maximize_window(window: &mut gpui::Window) -> Result<(), crate::Failure> {
    #[cfg(target_os = "macos")]
    {
        if mac::maximize(window) {
            return Ok(());
        }
    }
    #[cfg(target_os = "windows")]
    {
        if windows_os::maximize(window) {
            return Ok(());
        }
    }
    if !window.is_maximized() {
        window.zoom_window();
    }
    Ok(())
}

pub(crate) fn restore_window(window: &mut gpui::Window) -> Result<(), crate::Failure> {
    if window.is_fullscreen() {
        window.toggle_fullscreen();
    }
    #[cfg(target_os = "macos")]
    {
        if mac::restore(window) {
            return Ok(());
        }
    }
    #[cfg(target_os = "windows")]
    {
        if windows_os::restore(window) {
            return Ok(());
        }
    }
    if window.is_maximized() {
        window.zoom_window();
    }
    Ok(())
}

#[allow(unused_variables)]
pub(crate) fn move_window(window: &mut gpui::Window, x: f64, y: f64) -> Result<(), crate::Failure> {
    if !x.is_finite() || !y.is_finite() {
        return Err(crate::Failure::new(
            "invalid_params",
            "window coordinates must be finite",
        ));
    }
    #[cfg(target_os = "macos")]
    {
        mac::move_window(window, x, y)
    }
    #[cfg(target_os = "windows")]
    {
        windows_os::move_window(window, x, y)
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        linux::move_window(window, x, y)
    }
}

pub(crate) fn resize_window(
    window: &mut gpui::Window,
    width: f64,
    height: f64,
) -> Result<(), crate::Failure> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(crate::Failure::new(
            "invalid_params",
            "window dimensions must be positive finite numbers",
        ));
    }
    #[cfg(target_os = "macos")]
    {
        mac::resize_window(window, width, height);
    }
    #[cfg(target_os = "windows")]
    {
        windows_os::resize_window(window, width, height);
    }
    window.resize(gpui::size(gpui::px(width as f32), gpui::px(height as f32)));
    Ok(())
}

/// Capture `window` as PNG bytes, returning `(pixel_width, pixel_height, scale, png_bytes)`.
pub(crate) fn capture_window_png(
    window: &gpui::Window,
) -> Result<(i64, i64, f64, Vec<u8>), crate::Failure> {
    #[cfg(target_os = "macos")]
    {
        mac::capture_window_png(window)
    }
    #[cfg(target_os = "windows")]
    {
        windows_os::capture_window_png(window)
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        linux::capture_window_png(window)
    }
}

/// Whether motion is reduced: by the system, or by `BERN_MOTION=reduce`
/// for repeatable screenshots.
pub(crate) fn reduce_motion() -> bool {
    if std::env::var("BERN_MOTION").as_deref() == Ok("reduce") {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        mac::reduce_motion()
    }
    // "Animation effects" off in the accessibility settings.
    #[cfg(target_os = "windows")]
    {
        use win32::Win32::UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        };
        let mut animate = win32::core::BOOL(1);
        // SAFETY: the value is a BOOL, as the parameter asks.
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut animate as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        read.is_ok() && !animate.as_bool()
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        linux::reduce_motion()
    }
}

/// Where AppKit put a window's three buttons, in points from the window's
/// top leading corner: the close button's leading and top inset, the
/// buttons' height and the width the three take.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lights {
    pub(crate) x: f32,
    pub(crate) top: f32,
    pub(crate) height: f32,
    pub(crate) width: f32,
}

/// The window buttons of `window` where they are now.
#[cfg(target_os = "macos")]
pub(crate) fn lights(window: &gpui::Window) -> Option<Lights> {
    mac::lights(window)
}

/// Show the system's about panel for the application `name` at `version`,
/// with the icon the application shows.
pub(crate) fn about(name: &str, version: &str) {
    #[cfg(target_os = "macos")]
    mac::about(name, version);
    #[cfg(not(target_os = "macos"))]
    let _ = (name, version);
}

/// A command of the Window menu, on the window in front.
#[derive(Clone, Copy, Debug)]
pub(crate) enum WindowCommand {
    Close,
    Minimize,
    Zoom,
    BringAllToFront,
}

/// Run `command` as the system's own window command, so closing asks the
/// window as its close button does.
pub(crate) fn window_command(command: WindowCommand) {
    #[cfg(target_os = "macos")]
    mac::window_command(command);
    #[cfg(not(target_os = "macos"))]
    let _ = command;
}

/// Tell the system which menus of the menu bar are the Window and Help
/// menus, by title, so it adds its own items to them. The Window menu's
/// Minimize, Zoom and Bring All to Front (`standard`, by title) become the
/// system's own commands, which it places its items around.
pub(crate) fn designate_menus(windows: &str, standard: [String; 3], help: Option<&str>) {
    #[cfg(target_os = "macos")]
    mac::designate_menus(windows, standard, help);
    #[cfg(not(target_os = "macos"))]
    let _ = (windows, standard, help);
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{
        ACTION_ACTIVATE, ACTION_CLOSE, ACTION_MAXIMIZE, ACTION_MINIMIZE, ACTION_MOVE,
        ACTION_RESIZE, ACTION_RESTORE, WindowFacts,
    };
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{
        NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion, NSApplication,
        NSBitmapImageFileType, NSBitmapImageRep, NSCursor, NSImage, NSScreen, NSView, NSWindow,
        NSWindowButton, NSWindowStyleMask, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    fn ns_window(window: &gpui::Window) -> Option<Retained<NSWindow>> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
        else {
            return None;
        };
        // SAFETY: GPUI's view, alive while its window is open.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        view.window()
    }

    fn primary_screen_height(main: MainThreadMarker) -> f64 {
        let screens = NSScreen::screens(main);
        screens
            .firstObject()
            .map(|screen| screen.frame().size.height)
            .unwrap_or(0.0)
    }

    pub(super) fn window_facts(window: &gpui::Window, resizable: bool) -> Option<WindowFacts> {
        let main = MainThreadMarker::new()?;
        let ns = ns_window(window)?;
        let frame = ns.frame();
        let screen_h = primary_screen_height(main);
        let y = if screen_h > 0.0 {
            screen_h - (frame.origin.y + frame.size.height)
        } else {
            frame.origin.y
        };
        let visibility = if ns.isMiniaturized() {
            1
        } else if ns.isVisible() {
            0
        } else {
            2
        };
        let fullscreen = ns.styleMask().contains(NSWindowStyleMask::FullScreen);
        let size = if fullscreen {
            2
        } else if ns.isZoomed() {
            1
        } else {
            0
        };
        let mut actions =
            ACTION_ACTIVATE | ACTION_CLOSE | ACTION_MINIMIZE | ACTION_RESTORE | ACTION_MOVE;
        if resizable {
            actions |= ACTION_MAXIMIZE | ACTION_RESIZE;
        }
        Some(WindowFacts {
            visibility,
            size,
            x: frame.origin.x,
            y,
            width: frame.size.width,
            height: frame.size.height,
            focused: ns.isKeyWindow(),
            actions,
        })
    }

    pub(super) fn maximize(window: &gpui::Window) -> bool {
        let Some(ns) = ns_window(window) else {
            return false;
        };
        if !ns.isZoomed() {
            ns.zoom(None);
        }
        true
    }

    pub(super) fn restore(window: &gpui::Window) -> bool {
        let Some(ns) = ns_window(window) else {
            return false;
        };
        if ns.isMiniaturized() {
            ns.deminiaturize(None);
        }
        if ns.isZoomed() {
            ns.zoom(None);
        }
        true
    }

    pub(super) fn move_window(window: &gpui::Window, x: f64, y: f64) -> Result<(), crate::Failure> {
        let Some(main) = MainThreadMarker::new() else {
            return Err(crate::Failure::new("ui_failed", "not on the main thread"));
        };
        let Some(ns) = ns_window(window) else {
            return Err(crate::Failure::new(
                "target_unavailable",
                "window has no NSWindow",
            ));
        };
        let screen_h = primary_screen_height(main);
        let top_y = if screen_h > 0.0 { screen_h - y } else { y };
        ns.setFrameTopLeftPoint(NSPoint::new(x, top_y));
        Ok(())
    }

    pub(super) fn resize_window(window: &gpui::Window, width: f64, height: f64) {
        let Some(ns) = ns_window(window) else {
            return;
        };
        let mut frame = ns.frame();
        let top = frame.origin.y + frame.size.height;
        frame.size = NSSize::new(width, height);
        frame.origin.y = top - height;
        ns.setFrame_display(NSRect::new(frame.origin, frame.size), true);
    }

    pub(super) fn capture_window_png(
        window: &gpui::Window,
    ) -> Result<(i64, i64, f64, Vec<u8>), crate::Failure> {
        use objc2_core_graphics::{CGImage, CGWindowImageOption, CGWindowListOption};
        use std::ffi::{c_char, c_void};
        let Some(ns) = ns_window(window) else {
            return Err(crate::Failure::new(
                "capture_failed",
                "window has no NSWindow",
            ));
        };
        let scale = ns.backingScaleFactor().max(1.0);
        let win_num = ns.windowNumber();
        // Prefer compositing our own window directly via CoreGraphics so
        // Metal layers and title bars appear accurately (same approach as SwiftUI).
        if win_num > 0 {
            unsafe extern "C" {
                fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
            }
            #[link(name = "CoreFoundation", kind = "framework")]
            unsafe extern "C" {
                fn CFRelease(value: *const c_void);
            }
            type WindowImageFn = unsafe extern "C" fn(
                NSRect,
                CGWindowListOption,
                u32,
                CGWindowImageOption,
            ) -> *const CGImage;
            // RTLD_DEFAULT on macOS is (-2isize) as *mut c_void.
            let sym = unsafe {
                dlsym(
                    (-2isize) as *mut c_void,
                    c"CGWindowListCreateImage".as_ptr(),
                )
            };
            if !sym.is_null() {
                // SAFETY: matches CoreGraphics's CGWindowListCreateImage signature.
                let capture: WindowImageFn = unsafe { std::mem::transmute(sym) };
                // CGRectNull = { origin: (inf, inf), size: (0, 0) }
                let null_rect = NSRect::new(
                    NSPoint::new(f64::INFINITY, f64::INFINITY),
                    NSSize::new(0.0, 0.0),
                );
                let options =
                    CGWindowImageOption::BoundsIgnoreFraming | CGWindowImageOption::BestResolution;
                let cg_ptr = unsafe {
                    capture(
                        null_rect,
                        CGWindowListOption::OptionIncludingWindow,
                        win_num as u32,
                        options,
                    )
                };
                if !cg_ptr.is_null() {
                    // SAFETY: cg_ptr is a live owned CGImage until CFRelease below.
                    let cg_ref: &CGImage = unsafe { &*cg_ptr };
                    let w = CGImage::width(Some(cg_ref)) as i64;
                    let h = CGImage::height(Some(cg_ref)) as i64;
                    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), cg_ref);
                    unsafe { CFRelease(cg_ptr.cast()) };
                    let empty = NSDictionary::new();
                    // SAFETY: valid empty properties dictionary for PNG representation.
                    let data = unsafe {
                        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &empty)
                    };
                    if let Some(data) = data {
                        return Ok((w, h, scale, data.to_vec()));
                    }
                }
            }
        }
        // Fallback: NSView display caching.
        let view = ns
            .contentView()
            .and_then(|v| unsafe { v.superview() }.or(Some(v)))
            .ok_or_else(|| {
                crate::Failure::new("capture_failed", "window has no view to capture")
            })?;
        view.layoutSubtreeIfNeeded();
        let bounds = view.bounds();
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return Err(crate::Failure::new(
                "capture_failed",
                "window has empty bounds",
            ));
        }
        let rep = view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .ok_or_else(|| crate::Failure::new("capture_failed", "cannot allocate bitmap"))?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let empty = NSDictionary::new();
        // SAFETY: valid empty properties dictionary for PNG representation.
        let data =
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &empty) }
                .ok_or_else(|| crate::Failure::new("capture_failed", "cannot encode PNG"))?;
        let w = rep.pixelsWide() as i64;
        let h = rep.pixelsHigh() as i64;
        Ok((w, h, scale, data.to_vec()))
    }

    pub(super) fn touchscreen() -> Option<bool> {
        use objc2_foundation::NSNumber;
        use std::ffi::c_void;
        #[link(name = "IOKit", kind = "framework")]
        unsafe extern "C" {
            fn IOHIDManagerCreate(allocator: *const c_void, options: u32) -> *const c_void;
            fn IOHIDManagerSetDeviceMatching(manager: *const c_void, matching: *const c_void);
            fn IOHIDManagerCopyDevices(manager: *const c_void) -> *const c_void;
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        unsafe extern "C" {
            fn CFSetGetCount(set: *const c_void) -> isize;
            fn CFRelease(value: *const c_void);
        }
        let page = NSString::from_str("DeviceUsagePage");
        let usage = NSString::from_str("DeviceUsage");
        // HID Digitizer (0x0d), Touch Screen (0x04), NOT Touch Pad (0x05).
        let matching = NSDictionary::from_retained_objects(
            &[&*page, &*usage],
            &[NSNumber::numberWithInt(0x0d), NSNumber::numberWithInt(0x04)],
        );
        // SAFETY: NSDictionary/CFDictionary are toll-free bridged. Matching
        // enumerates devices synchronously; no open, event tap or capture is
        // needed. Both Create/Copy results are released exactly once.
        unsafe {
            let manager = IOHIDManagerCreate(std::ptr::null(), 0);
            if manager.is_null() {
                return None;
            }
            IOHIDManagerSetDeviceMatching(
                manager,
                (&*matching as *const NSDictionary<NSString, NSNumber>).cast(),
            );
            let devices = IOHIDManagerCopyDevices(manager);
            let found = !devices.is_null() && CFSetGetCount(devices) > 0;
            if !devices.is_null() {
                CFRelease(devices);
            }
            CFRelease(manager);
            Some(found)
        }
    }

    /// The icon by its asset name in the bundle, else from its picture.
    pub(super) fn set_icon(icon: Option<(Option<String>, std::path::PathBuf)>) {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(main);
        let image = icon.and_then(|(asset, path)| {
            asset
                .and_then(|asset| NSImage::imageNamed(&NSString::from_str(&asset)))
                .or_else(|| {
                    let path = NSString::from_str(&path.to_string_lossy());
                    NSImage::initWithContentsOfFile(NSImage::alloc(), &path)
                })
        });
        // SAFETY: on the main thread, with an image or none.
        unsafe { app.setApplicationIconImage(image.as_deref()) };
    }

    pub(super) fn hide_pointer() {
        NSCursor::setHiddenUntilMouseMoves(true);
    }

    pub(super) fn reduce_motion() -> bool {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }

    pub(super) fn about(name: &str, version: &str) {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(main);
        let mut keys: Vec<&NSString> = Vec::new();
        let mut values: Vec<Retained<AnyObject>> = Vec::new();
        // SAFETY: the option keys are AppKit's constants.
        unsafe {
            keys.push(NSAboutPanelOptionApplicationName);
            values.push(Retained::into_super(Retained::into_super(
                NSString::from_str(name),
            )));
            keys.push(NSAboutPanelOptionApplicationVersion);
            values.push(Retained::into_super(Retained::into_super(
                NSString::from_str(version),
            )));
        }
        let options = NSDictionary::from_retained_objects(&keys, &values);
        // SAFETY: on the main thread, with option keys AppKit defines.
        unsafe { app.orderFrontStandardAboutPanelWithOptions(&options) };
        app.activate();
    }

    pub(super) fn designate_menus(windows: &str, standard: [String; 3], help: Option<&str>) {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(main);
        let Some(bar) = app.mainMenu() else {
            return;
        };
        let find = |title: &str| {
            bar.itemArray()
                .iter()
                .filter_map(|item| item.submenu())
                .find(|menu| menu.title().to_string() == title)
        };
        if let Some(menu) = find(windows) {
            let commands = [
                objc2::sel!(performMiniaturize:),
                objc2::sel!(performZoom:),
                objc2::sel!(arrangeInFront:),
            ];
            for item in menu.itemArray().iter() {
                let title = item.title().to_string();
                if let Some(command) = standard
                    .iter()
                    .position(|name| *name == title)
                    .map(|index| commands[index])
                {
                    // SAFETY: the system's own selectors, sent to the
                    // responder chain.
                    unsafe {
                        item.setTarget(None);
                        item.setAction(Some(command));
                    }
                }
            }
            app.setWindowsMenu(Some(&menu));
        }
        if let Some(menu) = help.and_then(find) {
            app.setHelpMenu(Some(&menu));
        }
    }

    pub(super) fn window_command(command: super::WindowCommand) {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(main);
        if let super::WindowCommand::BringAllToFront = command {
            app.arrangeInFront(None);
            return;
        }
        let Some(window) = app.keyWindow().or_else(|| app.mainWindow()) else {
            return;
        };
        match command {
            super::WindowCommand::Close => window.performClose(None),
            super::WindowCommand::Minimize => window.performMiniaturize(None),
            super::WindowCommand::Zoom => window.performZoom(None),
            super::WindowCommand::BringAllToFront => {}
        }
    }

    pub(super) fn lights(window: &gpui::Window) -> Option<super::Lights> {
        let window = ns_window(window)?;
        let close = window.standardWindowButton(NSWindowButton::CloseButton)?;
        let zoom = window.standardWindowButton(NSWindowButton::ZoomButton)?;
        // In the window's coordinates, which start at its bottom.
        let close = close.convertRect_toView(close.bounds(), None);
        let zoom = zoom.convertRect_toView(zoom.bounds(), None);
        let height = window.frame().size.height;
        Some(super::Lights {
            x: close.origin.x as f32,
            top: (height - close.origin.y - close.size.height) as f32,
            height: close.size.height as f32,
            width: (zoom.origin.x + zoom.size.width - close.origin.x) as f32,
        })
    }
}

#[cfg(target_os = "windows")]
mod windows_os {
    use super::{
        ACTION_ACTIVATE, ACTION_CLOSE, ACTION_MAXIMIZE, ACTION_MINIMIZE, ACTION_MOVE,
        ACTION_RESIZE, ACTION_RESTORE, WindowFacts,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use win32::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
    use win32::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateCompatibleBitmap,
        CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject,
    };
    use win32::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use win32::Win32::UI::HiDpi::GetDpiForWindow;
    use win32::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, DestroyIcon, GetForegroundWindow, GetWindowRect, HICON, ICON_BIG,
        ICON_SMALL, ICONINFO, IsIconic, IsWindowVisible, IsZoomed, PW_RENDERFULLCONTENT,
        SW_MAXIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SendMessageW, SetWindowPos, ShowWindow, WM_SETICON,
    };

    thread_local! {
        static CURRENT_ICON: RefCell<Option<(HICON, HICON)>> = const { RefCell::new(None) };
        static HWNDS: RefCell<Vec<HWND>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn hwnd(window: &gpui::Window) -> Option<HWND> {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return None;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return None;
        };
        Some(HWND(handle.hwnd.get() as *mut _))
    }

    fn scale_for(hwnd: HWND) -> f64 {
        // SAFETY: valid live HWND.
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 }
    }

    pub(super) fn window_facts(window: &gpui::Window, resizable: bool) -> Option<WindowFacts> {
        let hwnd = hwnd(window)?;
        let mut rect = RECT::default();
        // SAFETY: valid live HWND and RECT pointer.
        unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
        let scale = scale_for(hwnd);
        let visibility = unsafe {
            if IsIconic(hwnd).as_bool() {
                1
            } else if IsWindowVisible(hwnd).as_bool() {
                0
            } else {
                2
            }
        };
        let size = if window.is_fullscreen() {
            2
        } else if unsafe { IsZoomed(hwnd).as_bool() } {
            1
        } else {
            0
        };
        let focused = unsafe { GetForegroundWindow() == hwnd };
        let mut actions =
            ACTION_ACTIVATE | ACTION_CLOSE | ACTION_MINIMIZE | ACTION_RESTORE | ACTION_MOVE;
        if resizable {
            actions |= ACTION_MAXIMIZE | ACTION_RESIZE;
        }
        Some(WindowFacts {
            visibility,
            size,
            x: f64::from(rect.left) / scale,
            y: f64::from(rect.top) / scale,
            width: f64::from(rect.right - rect.left) / scale,
            height: f64::from(rect.bottom - rect.top) / scale,
            focused,
            actions,
        })
    }

    pub(super) fn maximize(window: &gpui::Window) -> bool {
        let Some(hwnd) = hwnd(window) else {
            return false;
        };
        // SAFETY: valid live HWND.
        unsafe {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
        true
    }

    pub(super) fn restore(window: &gpui::Window) -> bool {
        let Some(hwnd) = hwnd(window) else {
            return false;
        };
        // SAFETY: valid live HWND.
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        true
    }

    pub(super) fn move_window(window: &gpui::Window, x: f64, y: f64) -> Result<(), crate::Failure> {
        let Some(hwnd) = hwnd(window) else {
            return Err(crate::Failure::new(
                "target_unavailable",
                "window has no HWND",
            ));
        };
        let scale = scale_for(hwnd);
        let px_x = (x * scale).round() as i32;
        let px_y = (y * scale).round() as i32;
        // SAFETY: valid live HWND.
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                px_x,
                px_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|error| crate::Failure::new("window_failed", error.to_string()))
    }

    pub(super) fn resize_window(window: &gpui::Window, width: f64, height: f64) {
        let Some(hwnd) = hwnd(window) else {
            return;
        };
        let scale = scale_for(hwnd);
        let px_w = (width * scale).round() as i32;
        let px_h = (height * scale).round() as i32;
        // SAFETY: valid live HWND.
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                px_w,
                px_h,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }

    pub(super) fn capture_window_png(
        window: &gpui::Window,
    ) -> Result<(i64, i64, f64, Vec<u8>), crate::Failure> {
        let Some(hwnd) = hwnd(window) else {
            return Err(crate::Failure::new("capture_failed", "window has no HWND"));
        };
        let mut rect = RECT::default();
        // SAFETY: valid live HWND.
        unsafe { GetWindowRect(hwnd, &mut rect) }
            .map_err(|error| crate::Failure::new("capture_failed", error.to_string()))?;
        let w = (rect.right - rect.left).max(0);
        let h = (rect.bottom - rect.top).max(0);
        if w <= 0 || h <= 0 {
            return Err(crate::Failure::new(
                "capture_failed",
                "window has empty bounds",
            ));
        }
        let scale = scale_for(hwnd);
        // SAFETY: GDI objects are created, checked, and released before returning.
        let mut bgra = vec![0u8; (w as usize) * (h as usize) * 4];
        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bmp = CreateCompatibleBitmap(screen_dc, w, h);
            let prev = SelectObject(mem_dc, bmp.into());
            let ok = PrintWindow(hwnd, mem_dc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let lines = if ok {
                GetDIBits(
                    mem_dc,
                    bmp,
                    0,
                    h as u32,
                    Some(bgra.as_mut_ptr().cast()),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            } else {
                0
            };
            SelectObject(mem_dc, prev);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, screen_dc);
            if !ok || lines == 0 {
                return Err(crate::Failure::new("capture_failed", "PrintWindow failed"));
            }
        }
        // Convert BGRA -> RGBA (force opaque alpha where DWM leaves 0).
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            if pixel[3] == 0 {
                pixel[3] = 255;
            }
        }
        use image::ImageEncoder as _;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&bgra, w as u32, h as u32, image::ExtendedColorType::Rgba8)
            .map_err(|error| crate::Failure::new("capture_failed", error.to_string()))?;
        Ok((i64::from(w), i64::from(h), scale, png))
    }

    fn icon_from_image(path: &std::path::Path, size: u32) -> Option<HICON> {
        let img =
            image::open(path)
                .ok()?
                .resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = img.to_rgba8();
        let mut bgra = Vec::with_capacity((size * size * 4) as usize);
        for p in rgba.pixels() {
            bgra.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
        let header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let bmi = BITMAPINFO {
            bmiHeader: header,
            ..Default::default()
        };
        unsafe {
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let color_bmp =
                CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
            }
            let mask_bmp = CreateBitmap(size as i32, size as i32, 1, 1, None);
            let info = ICONINFO {
                fIcon: true.into(),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask_bmp,
                hbmColor: color_bmp,
            };
            let hicon = CreateIconIndirect(&info).ok();
            let _ = DeleteObject(color_bmp.into());
            let _ = DeleteObject(mask_bmp.into());
            hicon
        }
    }

    pub(super) fn set_icon(path: Option<PathBuf>) {
        let next = path.and_then(|path| {
            let big = icon_from_image(&path, 256)?;
            let small = icon_from_image(&path, 32).unwrap_or(big);
            Some((big, small))
        });
        CURRENT_ICON.with(|slot| {
            if let Some((old_big, old_small)) = slot.borrow_mut().replace_if_needed(next) {
                unsafe {
                    let _ = DestroyIcon(old_big);
                    if old_small != old_big {
                        let _ = DestroyIcon(old_small);
                    }
                }
            }
        });
        HWNDS.with(|list| {
            for &hwnd in list.borrow().iter() {
                apply_hwnd_icon(hwnd);
            }
        });
    }

    trait ReplaceHelper<T> {
        fn replace_if_needed(&mut self, value: Option<T>) -> Option<T>;
    }

    impl<T> ReplaceHelper<T> for Option<T> {
        fn replace_if_needed(&mut self, value: Option<T>) -> Option<T> {
            std::mem::replace(self, value)
        }
    }

    fn apply_hwnd_icon(hwnd: HWND) {
        CURRENT_ICON.with(|slot| {
            if let Some((big, small)) = *slot.borrow() {
                unsafe {
                    let _ = SendMessageW(
                        hwnd,
                        WM_SETICON,
                        Some(WPARAM(ICON_BIG as usize)),
                        Some(LPARAM(big.0 as isize)),
                    );
                    let _ = SendMessageW(
                        hwnd,
                        WM_SETICON,
                        Some(WPARAM(ICON_SMALL as usize)),
                        Some(LPARAM(small.0 as isize)),
                    );
                }
            }
        });
    }

    pub(super) fn apply_window_icon(window: &gpui::Window) {
        if let Some(hwnd) = hwnd(window) {
            HWNDS.with(|list| {
                let mut list = list.borrow_mut();
                if !list.contains(&hwnd) {
                    list.push(hwnd);
                }
            });
            apply_hwnd_icon(hwnd);
        }
    }
}

/// Linux: GNOME's animation setting, plus X11 window move and capture via
/// `dlopen("libX11.so.6")` without a hard link-time dependency.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    static REDUCED: AtomicBool = AtomicBool::new(false);
    static WATCHED: std::sync::Once = std::sync::Once::new();

    pub(super) fn is_x11(window: &gpui::Window) -> bool {
        HasWindowHandle::window_handle(window)
            .ok()
            .is_some_and(|h| {
                matches!(
                    h.as_raw(),
                    RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
                )
            })
    }

    pub(super) fn move_window(window: &gpui::Window, x: f64, y: f64) -> Result<(), crate::Failure> {
        let Ok(win_h) = HasWindowHandle::window_handle(window) else {
            return Err(crate::Failure::new(
                "target_unavailable",
                "window handle unavailable",
            ));
        };
        let Ok(disp_h) = HasDisplayHandle::display_handle(window) else {
            return Err(crate::Failure::new(
                "target_unavailable",
                "display handle unavailable",
            ));
        };
        let (RawWindowHandle::Xlib(w), RawDisplayHandle::Xlib(d)) =
            (win_h.as_raw(), disp_h.as_raw())
        else {
            return Err(crate::Failure::new(
                "target_unavailable",
                "Wayland compositors position windows themselves",
            ));
        };
        let Some(display) = d.display else {
            return Err(crate::Failure::new(
                "target_unavailable",
                "X11 display missing",
            ));
        };
        type XMoveWindowFn = unsafe extern "C" fn(*mut c_void, u64, i32, i32) -> i32;
        type XFlushFn = unsafe extern "C" fn(*mut c_void) -> i32;
        unsafe {
            let lib = libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_LAZY);
            if lib.is_null() {
                return Err(crate::Failure::new(
                    "target_unavailable",
                    "libX11.so.6 not available",
                ));
            }
            let move_sym = libc::dlsym(lib, c"XMoveWindow".as_ptr());
            let flush_sym = libc::dlsym(lib, c"XFlush".as_ptr());
            if !move_sym.is_null() {
                let xmove: XMoveWindowFn = std::mem::transmute(move_sym);
                let scale = f64::from(window.scale_factor().max(1.0));
                xmove(
                    display.as_ptr(),
                    w.window,
                    (x * scale).round() as i32,
                    (y * scale).round() as i32,
                );
            }
            if !flush_sym.is_null() {
                let xflush: XFlushFn = std::mem::transmute(flush_sym);
                xflush(display.as_ptr());
            }
        }
        Ok(())
    }

    pub(super) fn capture_window_png(
        window: &gpui::Window,
    ) -> Result<(i64, i64, f64, Vec<u8>), crate::Failure> {
        let scale = f64::from(window.scale_factor().max(1.0));
        // Try X11 XGetImage when running on Xlib.
        if let (Ok(win_h), Ok(disp_h)) = (
            HasWindowHandle::window_handle(window),
            HasDisplayHandle::display_handle(window),
        ) && let (RawWindowHandle::Xlib(w), RawDisplayHandle::Xlib(d)) =
            (win_h.as_raw(), disp_h.as_raw())
            && let Some(display) = d.display
        {
            let bounds = window.viewport_size();
            let pw = ((f32::from(bounds.width) * scale as f32).round() as u32).max(1);
            let ph = ((f32::from(bounds.height) * scale as f32).round() as u32).max(1);
            #[repr(C)]
            struct XImage {
                width: i32,
                height: i32,
                xoffset: i32,
                format: i32,
                data: *mut u8,
                byte_order: i32,
                bitmap_unit: i32,
                bitmap_bit_order: i32,
                bitmap_pad: i32,
                depth: i32,
                bytes_per_line: i32,
                bits_per_pixel: i32,
                red_mask: u64,
                green_mask: u64,
                blue_mask: u64,
            }
            type XGetImageFn =
                unsafe extern "C" fn(*mut c_void, u64, i32, i32, u32, u32, u64, i32) -> *mut XImage;
            type XDestroyImageFn = unsafe extern "C" fn(*mut XImage) -> i32;
            unsafe {
                let lib = libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_LAZY);
                if !lib.is_null() {
                    let get_sym = libc::dlsym(lib, c"XGetImage".as_ptr());
                    let destroy_sym = libc::dlsym(lib, c"XDestroyImage".as_ptr());
                    if !get_sym.is_null() {
                        let xget: XGetImageFn = std::mem::transmute(get_sym);
                        // ZPixmap = 2, AllPlanes = !0u64
                        let img = xget(display.as_ptr(), w.window, 0, 0, pw, ph, !0u64, 2);
                        if !img.is_null() {
                            let xr = &*img;
                            let width = xr.width.max(1) as u32;
                            let height = xr.height.max(1) as u32;
                            let stride = xr.bytes_per_line.max(0) as usize;
                            let bpp = (xr.bits_per_pixel / 8).max(3) as usize;
                            let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
                            for row in 0..height as usize {
                                for col in 0..width as usize {
                                    let src = xr.data.add(row * stride + col * bpp);
                                    let dst = (row * (width as usize) + col) * 4;
                                    rgba[dst] = *src.add(2);
                                    rgba[dst + 1] = *src.add(1);
                                    rgba[dst + 2] = *src;
                                    rgba[dst + 3] = 255;
                                }
                            }
                            if !destroy_sym.is_null() {
                                let xdestroy: XDestroyImageFn = std::mem::transmute(destroy_sym);
                                xdestroy(img);
                            }
                            use image::ImageEncoder as _;
                            let mut png = Vec::new();
                            if image::codecs::png::PngEncoder::new(&mut png)
                                .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
                                .is_ok()
                            {
                                return Ok((i64::from(width), i64::from(height), scale, png));
                            }
                        }
                    }
                }
            }
        }
        // Fallback: GNOME Shell Screenshot D-Bus or grim on Wayland.
        let tmp = std::env::temp_dir().join(format!("bern-gpui-shot-{}.png", std::process::id()));
        let ok = std::process::Command::new("grim")
            .arg(&tmp)
            .status()
            .is_ok_and(|s| s.success())
            || std::process::Command::new("gnome-screenshot")
                .args(["-w", "-f"])
                .arg(&tmp)
                .status()
                .is_ok_and(|s| s.success());
        if ok && let Ok(bytes) = std::fs::read(&tmp) {
            let _ = std::fs::remove_file(&tmp);
            if let Ok(img) = image::load_from_memory(&bytes) {
                return Ok((
                    i64::from(img.width()),
                    i64::from(img.height()),
                    scale,
                    bytes,
                ));
            }
        }
        let _ = std::fs::remove_file(&tmp);
        Err(crate::Failure::new(
            "capture_failed",
            "window screenshot requires X11 or a Wayland screenshot portal",
        ))
    }

    pub(super) fn reduce_motion() -> bool {
        WATCHED.call_once(|| {
            let _ = std::thread::Builder::new()
                .name("Bern motion setting".into())
                .spawn(|| {
                    loop {
                        let output = std::process::Command::new("gsettings")
                            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
                            .output();
                        let Ok(output) = output else { return };
                        let value = String::from_utf8_lossy(&output.stdout);
                        REDUCED.store(value.trim() == "false", Ordering::Relaxed);
                        std::thread::sleep(std::time::Duration::from_secs(5));
                    }
                });
        });
        REDUCED.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod touch_tests {
    #[test]
    fn direct_touch_inventory_excludes_trackpads_and_pens() {
        use super::direct_touch_properties as direct;
        assert!(direct("E:ID_INPUT=1\nE:ID_INPUT_TOUCHSCREEN=1\n"));
        assert!(!direct("E:ID_INPUT_TOUCHPAD=1\n"));
        assert!(!direct("E:ID_INPUT_TABLET=1\n"));
        assert!(!direct("E:ID_INPUT_TOUCHSCREEN=1\nE:ID_INPUT_TOUCHPAD=1\n"));
        assert!(!direct("E:ID_INPUT_TOUCHSCREEN=0\n"));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_can_enumerate_direct_touch_without_opening_a_window() {
        assert!(super::touchscreen_query().is_some());
    }
}
