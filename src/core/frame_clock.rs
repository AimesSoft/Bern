//! Display-aware frame timing for Bern animations.

use std::time::Duration;

const FALLBACK_REFRESH_RATE_HZ: u32 = 60;
const MIN_REASONABLE_REFRESH_RATE_HZ: u32 = 24;
const MAX_REASONABLE_REFRESH_RATE_HZ: u32 = 480;

/// Returns the interval of the display currently hosting the key window.
///
/// AppKit's `mainScreen` follows the screen containing the key window, so the
/// value is refreshed whenever an animation asks for its next frame instead
/// of being permanently cached at application startup.
pub(crate) fn animation_frame_interval() -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(display_refresh_rate_hz()))
}

fn display_refresh_rate_hz() -> u32 {
    platform_refresh_rate_hz().unwrap_or(FALLBACK_REFRESH_RATE_HZ)
}

#[cfg(target_os = "macos")]
fn platform_refresh_rate_hz() -> Option<u32> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    let mtm = MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let refresh_rate = u32::try_from(screen.maximumFramesPerSecond()).ok()?;
    (MIN_REASONABLE_REFRESH_RATE_HZ..=MAX_REASONABLE_REFRESH_RATE_HZ)
        .contains(&refresh_rate)
        .then_some(refresh_rate)
}

#[cfg(not(target_os = "macos"))]
fn platform_refresh_rate_hz() -> Option<u32> {
    None
}
