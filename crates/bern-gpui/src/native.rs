//! What GPUI leaves to the platform: the application's icon, hiding the
//! pointer, whether motion is reduced, the about panel and the window
//! commands of the menu bar. Windows and Linux have no about panel or
//! menu bar, and show the icon of the executable (Windows) or of the
//! desktop entry (Linux).

use std::cell::RefCell;

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
            && at.elapsed() < Duration::from_secs(1) { return value; }
        let found = touchscreen_query();
        if found.is_none() {
            static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("bern: direct touchscreen inventory is unavailable; window.touch defaults to false (not a device capability determination)");
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
    #[cfg(not(target_os = "macos"))]
    let _ = changed;
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
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{
        NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion, NSApplication,
        NSCursor, NSImage, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSString};

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
        use objc2_app_kit::{NSView, NSWindowButton};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
        else {
            return None;
        };
        // SAFETY: GPUI's view, alive while its window is open.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let window = view.window()?;
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

/// Linux: GNOME's (and most desktops') animation setting, read in the
/// background every few seconds, as reading it means running `gsettings`.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux {
    use std::sync::atomic::{AtomicBool, Ordering};

    static REDUCED: AtomicBool = AtomicBool::new(false);
    static WATCHED: std::sync::Once = std::sync::Once::new();

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
