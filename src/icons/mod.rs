//! Material Icons — the default icon package for rern controls.
//!
//! Ported from Flutter's Material Icons (Apache-2.0):
//!
//! - The `MaterialIcons-Regular.otf` font is embedded in this crate
//!   (`assets/fonts/`), so applications do not need to ship it themselves.
//! - The name → codepoint table is generated from the Flutter SDK's
//!   `packages/flutter/lib/src/material/icons.dart`, so every icon uses the
//!   same name as `Icons.xxx` in Flutter (e.g. `"add"`, `"dark_mode"`,
//!   `"favorite_rounded"`).
//!
//! Controls such as `icon_button` and `icon` resolve their `icon` prop
//! through this package by default; unknown names fall back to being treated
//! as raw text glyphs.

pub mod material_icons;

/// The font family name of the embedded Material Icons font.
pub const MATERIAL_FONT: &str = "Material Icons";

/// Returns the [`iced::Font`] used to render Material icon glyphs.
pub fn font() -> iced::Font {
    iced::Font::with_name(MATERIAL_FONT)
}

/// Looks up a Material icon glyph by its Flutter name, e.g. `"add"`.
pub fn glyph(name: &str) -> Option<char> {
    material_icons::MATERIAL_ICONS
        .binary_search_by_key(&name, |(n, _)| n)
        .ok()
        .map(|index| material_icons::MATERIAL_ICONS[index].1)
}

/// Same as [`glyph`], as a `String`.
pub fn glyph_string(name: &str) -> Option<String> {
    glyph(name).map(|c| c.to_string())
}

/// Loads the embedded Material Icons font into the iced runtime.
///
/// Call this once at application startup, from your boot/init task:
///
/// ```no_run
/// # fn main() {}
/// let task = rern::icons::load();
/// # drop(task);
/// ```
pub fn load() -> iced::Task<Result<(), iced::font::Error>> {
    iced::font::load(
        include_bytes!("../../assets/fonts/MaterialIcons-Regular.otf").as_slice(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_icons_by_flutter_name() {
        assert_eq!(glyph("add"), Some('\u{e047}'));
        assert_eq!(glyph("dark_mode"), Some('\u{e1b0}'));
        assert!(glyph("dark_mode_rounded").is_some());
        assert!(glyph("ten_k").is_some());
        assert_eq!(glyph("no_such_icon_xyz"), None);
    }

    #[test]
    fn font_uses_material_icons_family() {
        assert_eq!(font().family, iced::font::Family::Name("Material Icons"));
    }
}
