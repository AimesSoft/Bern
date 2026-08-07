//! Bold-font support.
//!
//! iced 0.14's font database ships no bold face by default, so
//! `Font { weight: Weight::Bold }` is silently ignored. This module loads a
//! bold system font at startup and exposes [`bold_font`] for controls that
//! need an actually-bold label (e.g. the selected `h_tab` item).
//!
//! The lookup is platform-pragmatic: macOS loads `Hiragino Sans GB` (a CJK
//! face with a W6 bold weight, covering the demo's Chinese labels). If the
//! file is missing, [`bold_font`] still returns a bold *request* — the
//! renderer just falls back to the regular face (graceful degradation).

use iced::Font;
use iced::font::Weight;
use std::borrow::Cow;

/// The bold family used for selected tab labels.
const BOLD_FAMILY: &str = "Hiragino Sans GB";

/// A bold [`Font`] request for the UI text family.
pub fn bold_font() -> Font {
    Font {
        family: iced::font::Family::Name(BOLD_FAMILY),
        weight: Weight::Bold,
        ..Font::default()
    }
}

/// Loads the bold font into iced's font system. Call once at app startup
/// (e.g. in `boot`); on non-macOS systems it is a no-op.
pub fn load_bold() -> iced::Task<Result<(), iced::font::Error>> {
    match read_bold_font() {
        Some(bytes) => iced::font::load(Cow::Owned(bytes)),
        None => iced::Task::none(),
    }
}

/// Loads the bold font synchronously — used by tests that render headlessly
/// without a running application runtime.
pub fn load_bold_now() {
    if let Some(bytes) = read_bold_font() {
        iced::advanced::graphics::text::font_system()
            .write()
            .unwrap()
            .load_font(Cow::Owned(bytes));
    }
}

/// Reads the platform's bold CJK font, if present.
fn read_bold_font() -> Option<Vec<u8>> {
    for path in [
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        // Other platforms could add their bold font paths here.
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(bytes);
        }
    }
    None
}
