//! Bern icon names (SF Symbols naming) as glyphs of the system icon fonts:
//! Segoe Fluent Icons on Windows 11 and Segoe MDL2 Assets on Windows 10,
//! which share these code points. The table is the one source: the catalog
//! carries it to the runtime, and the check rejects names it lacks.

/// `(name, code point)`, sorted by name.
pub const GLYPHS: &[(&str, u32)] = &[
    ("archivebox", 0xE7B8),
    ("arrow.down", 0xE74B),
    ("arrow.down.circle", 0xE896),
    ("arrow.left", 0xE72B),
    ("arrow.triangle.2.circlepath", 0xE895),
    ("arrow.up.arrow.down", 0xE8CB),
    ("arrow.up.left.and.arrow.down.right", 0xE740),
    ("arrow.up.right.square", 0xE8A7),
    ("backward.end.fill", 0xE892),
    ("calendar", 0xE787),
    ("camera", 0xE722),
    ("captions.bubble", 0xED1E),
    ("character.cursor.ibeam", 0xE765),
    ("checkmark", 0xE73E),
    ("checkmark.circle", 0xE930),
    ("chevron.down", 0xE70D),
    ("chevron.left", 0xE76B),
    ("chevron.right", 0xE76C),
    ("chevron.up", 0xE70E),
    ("chevron.up.chevron.down", 0xE70D),
    ("circle", 0xEA3A),
    ("circle.fill", 0xEA3B),
    ("doc", 0xE8A5),
    ("ellipsis", 0xE712),
    ("ellipsis.circle", 0xE712),
    ("exclamationmark.triangle", 0xE7BA),
    ("eye.slash", 0xED1A),
    ("film", 0xE8B2),
    ("film.stack", 0xE8B2),
    ("folder", 0xE8B7),
    ("forward.end.fill", 0xE893),
    ("gearshape", 0xE713),
    ("gearshape.2", 0xE912),
    ("gobackward.10", 0xED3C),
    ("goforward.10", 0xEB9D),
    ("hand.tap", 0xE7C9),
    ("heart", 0xEB51),
    ("house", 0xE80F),
    ("info.circle", 0xE946),
    ("keyboard", 0xE92E),
    ("line.3.horizontal.decrease", 0xE71C),
    ("link", 0xE71B),
    ("list.bullet.rectangle", 0xE8FD),
    ("magnifyingglass", 0xE721),
    ("minus", 0xE738),
    ("moon", 0xE708),
    ("music.note", 0xEC4F),
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
    ("puzzlepiece.extension", 0xEA86),
    ("scissors", 0xE8C6),
    ("server.rack", 0xE965),
    ("shield", 0xEA18),
    ("sidebar.right", 0xE90D),
    ("slider.horizontal.3", 0xE9E9),
    ("sparkles", 0xED37),
    ("speaker.fill", 0xE993),
    ("speaker.slash.fill", 0xE74F),
    ("speaker.wave.2", 0xE994),
    ("speaker.wave.3.fill", 0xE995),
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
    ("textformat", 0xE8D2),
    ("textformat.size", 0xE8E9),
    ("textformat.size.larger", 0xE8E8),
    ("textformat.size.smaller", 0xE8E7),
    ("trash", 0xE74D),
    ("tray", 0xE8F1),
    ("tv", 0xE7F4),
    ("wifi", 0xE701),
    ("xmark", 0xE711),
    ("xmark.circle.fill", 0xE894),
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
    fn every_name_of_the_bern_icon_set_has_a_glyph() {
        for name in bern_target_api::icon_set::NAMES {
            assert!(glyph(name).is_some(), "the Windows icon fonts lack {name}");
        }
    }
}
