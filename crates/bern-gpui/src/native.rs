//! What GPUI leaves to the platform: the application's icon, hiding the
//! pointer, whether motion is reduced, the about panel and the window
//! commands of the menu bar. Elsewhere than macOS these do nothing yet.

use std::cell::RefCell;

thread_local! {
    /// The picture the application shows as its icon.
    static ICON: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

/// Show the picture `name` among the resources as the application's icon;
/// none keeps the system's. In an app bundle the application's own icon is
/// the bundle's, which the system draws in its shape.
pub(crate) fn show_icon(name: Option<&'static str>) {
    let name = name.filter(|name| {
        !(crate::host::bundled() && Some(*name) == crate::host::host().def.app_icon)
    });
    let changed = ICON.with(|icon| icon.replace(name) != name);
    if !changed {
        return;
    }
    #[cfg(target_os = "macos")]
    mac::set_icon(name.map(|name| std::path::Path::new(&crate::host::resources_dir()).join(name)));
}

/// Hide the pointer until it moves.
pub(crate) fn hide_pointer() {
    #[cfg(target_os = "macos")]
    mac::hide_pointer();
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
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
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

    pub(super) fn set_icon(path: Option<std::path::PathBuf>) {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(main);
        let image = path.and_then(|path| {
            let path = NSString::from_str(&path.to_string_lossy());
            NSImage::initWithContentsOfFile(NSImage::alloc(), &path)
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
}
