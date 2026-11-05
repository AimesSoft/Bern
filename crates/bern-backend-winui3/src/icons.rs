//! Bern icon names (SF Symbols naming) as glyphs of the system icon fonts:
//! Segoe Fluent Icons on Windows 11 and Segoe MDL2 Assets on Windows 10,
//! which share these code points except for `WINDOWS_10_GLYPHS`. The tables
//! are the one source: the catalog carries them to the runtime, and the
//! check rejects names they lack.
//! The fonts have no quote bubble, so `quote.bubble`, the lyrics button of
//! Apple's music apps, is the captions box with a speaker: words that go
//! with the sound. (A microphone, which it was, reads as recording.)

/// `(name, code point)`, sorted by name.
pub const GLYPHS: &[(&str, u32)] = &[
    ("archivebox", 0xE7B8),
    ("arrow.clockwise", 0xE72C),
    ("arrow.down", 0xE74B),
    ("arrow.down.circle", 0xE896),
    ("arrow.down.right.and.arrow.up.left", 0xE73F),
    ("arrow.left", 0xE72B),
    ("arrow.triangle.2.circlepath", 0xE895),
    ("arrow.triangle.branch", 0xE8F0),
    ("arrow.up.arrow.down", 0xE8CB),
    ("arrow.up.left.and.arrow.down.right", 0xE740),
    ("arrow.up.right.square", 0xE8A7),
    ("backward.end.fill", 0xE892),
    ("backward.fill", 0xEB9E),
    ("calendar", 0xE787),
    ("camera", 0xE722),
    ("captions.bubble", 0xED1E),
    ("captions.bubble.fill", 0xED1E),
    ("character.cursor.ibeam", 0xE765),
    ("chart.bar", 0xEAFC),
    ("checkmark", 0xE73E),
    ("checkmark.circle", 0xE930),
    ("checkmark.circle.fill", 0xEC61),
    ("chevron.down", 0xE70D),
    ("chevron.left", 0xE76B),
    ("chevron.right", 0xE76C),
    ("chevron.up", 0xE70E),
    ("chevron.up.chevron.down", 0xE70D),
    ("circle", 0xEA3A),
    ("circle.fill", 0xEA3B),
    ("clock.arrow.circlepath", 0xE81C),
    ("doc", 0xE8A5),
    ("dot.radiowaves.left.and.right", 0xEC05),
    ("ellipsis", 0xE712),
    ("ellipsis.circle", 0xE712),
    ("exclamationmark.triangle", 0xE7BA),
    ("eye.slash", 0xED1A),
    ("film", 0xE8B2),
    ("film.stack", 0xE8B2),
    ("folder", 0xE8B7),
    ("forward.end.fill", 0xE893),
    ("forward.fill", 0xEB9D),
    ("gearshape", 0xE713),
    ("gearshape.2", 0xE912),
    ("globe", 0xE774),
    ("gobackward.10", 0xED3C),
    ("goforward.10", 0xEB9D),
    ("hand.tap", 0xE7C9),
    ("headphones", 0xE7F6),
    ("heart", 0xEB51),
    ("heart.fill", 0xEB52),
    ("house", 0xE80F),
    ("info.circle", 0xE946),
    ("info.circle.fill", 0xF167),
    ("iphone", 0xE8EA),
    ("keyboard", 0xE92E),
    ("line.3.horizontal.decrease", 0xE71C),
    ("link", 0xE71B),
    ("list.bullet", 0xE8FD),
    ("list.bullet.rectangle", 0xE8FD),
    ("magnifyingglass", 0xE721),
    ("minus", 0xE738),
    ("moon", 0xE708),
    ("music.note", 0xEC4F),
    ("music.note.list", 0xE90B),
    ("note.text", 0xE70B),
    ("paintbrush", 0xE790),
    ("paintpalette", 0xE790),
    ("paperplane", 0xE724),
    ("pause", 0xE769),
    ("pause.fill", 0xF8AE),
    ("person.crop.circle", 0xE77B),
    ("photo", 0xEB9F),
    ("pip.enter", 0xE93A),
    ("play", 0xE768),
    ("play.fill", 0xF5B0),
    ("play.rectangle", 0xE714),
    ("plus", 0xE710),
    ("power", 0xE7E8),
    ("puzzlepiece.extension", 0xEA86),
    ("qrcode", 0xED14),
    ("qrcode.viewfinder", 0xEE6F),
    ("quote.bubble", 0xED1F),
    ("quote.bubble.fill", 0xED1F),
    ("rectangle.portrait.and.arrow.right", 0xF3B1),
    ("repeat", 0xE8EE),
    ("repeat.1", 0xE8ED),
    ("scissors", 0xE8C6),
    ("server.rack", 0xE965),
    ("shield", 0xEA18),
    ("shuffle", 0xE8B1),
    ("sidebar.right", 0xE90D),
    ("slider.horizontal.3", 0xE9E9),
    ("sparkles", 0xED37),
    ("speaker.fill", 0xE993),
    ("speaker.slash.fill", 0xE74F),
    ("speaker.wave.2", 0xE994),
    ("speaker.wave.3.fill", 0xE995),
    ("speedometer", 0xEC4A),
    ("square", 0xE739),
    ("square.and.arrow.up", 0xE72D),
    ("square.grid.2x2", 0xF0E2),
    ("square.grid.3x3", 0xE80A),
    ("square.on.square", 0xE8C8),
    ("star", 0xE734),
    ("star.fill", 0xE735),
    ("star.slash", 0xE8D9),
    ("sun.max.fill", 0xE706),
    ("sun.min", 0xE706),
    ("switch.2", 0xE73A),
    ("text.bubble", 0xE8BD),
    ("text.bubble.fill", 0xE7E7),
    ("text.rectangle", 0xED1E),
    ("textformat", 0xE8D2),
    ("textformat.size", 0xE8E9),
    ("textformat.size.larger", 0xE8E8),
    ("textformat.size.smaller", 0xE8E7),
    ("trash", 0xE74D),
    ("tray", 0xE8F1),
    ("tv", 0xE7F4),
    ("viewfinder", 0xE740),
    ("wifi", 0xE701),
    ("xmark", 0xE711),
    ("xmark.circle.fill", 0xE894),
];

/// Where Windows 10 needs another code point, sorted by name. Its Segoe MDL2
/// Assets lacks U+ED1A, U+E93A and U+ED37 (compared with the font of build
/// 19045), and its U+E8B7 is a tall page, not a folder. The font has no eye
/// with a slash, so `eye.slash` is its half-closed eye; `sparkles` is the
/// star, as on GTK. All of these are in Segoe Fluent Icons too, so a Windows
/// 10 with that font installed draws them as well.
pub const WINDOWS_10_GLYPHS: &[(&str, u32)] = &[
    ("eye.slash", 0xE890),
    ("folder", 0xF12B),
    ("pip.enter", 0xE944),
    ("sparkles", 0xE734),
];

/// The glyph of `name`, if the fonts have one.
pub fn glyph(name: &str) -> Option<u32> {
    GLYPHS
        .binary_search_by(|(known, _)| known.cmp(&name))
        .ok()
        .map(|index| GLYPHS[index].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_in_the_private_use_area() {
        assert!(GLYPHS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(
            GLYPHS
                .iter()
                .all(|(_, code)| (0xE000..=0xF8FF).contains(code))
        );
        assert_eq!(glyph("folder"), Some(0xE8B7));
        assert_eq!(glyph("no.such.symbol"), None);
    }

    #[test]
    fn windows_10_only_replaces_names_of_the_table() {
        assert!(
            WINDOWS_10_GLYPHS
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
        for (name, code) in WINDOWS_10_GLYPHS {
            assert!((0xE000..=0xF8FF).contains(code), "{name}");
            assert_ne!(glyph(name), Some(*code), "{name} is the same in both");
            assert!(glyph(name).is_some(), "{name} is not in the table");
        }
    }

    #[test]
    fn every_name_of_the_bern_icon_set_has_a_glyph() {
        for name in bern_target_api::icon_set::NAMES {
            assert!(glyph(name).is_some(), "the Windows icon fonts lack {name}");
        }
    }
}
