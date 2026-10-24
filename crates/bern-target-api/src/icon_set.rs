//! The Bern icon set: icon names in SF Symbols naming. The Apple targets
//! draw every SF Symbol; the other targets map each name to one of their
//! platform's own icons, and every one of them draws at least the names
//! listed here. A name a target cannot draw fails its check at the
//! element, so no target shows a placeholder in its place.

use crate::Diagnostic;
use bern_ir::{Application, Node, Prop};

/// The names every target draws, sorted.
pub const NAMES: &[&str] = &[
    "archivebox",
    "arrow.down",
    "arrow.down.circle",
    "arrow.down.right.and.arrow.up.left",
    "arrow.left",
    "arrow.triangle.2.circlepath",
    "arrow.up.arrow.down",
    "arrow.up.left.and.arrow.down.right",
    "arrow.up.right.square",
    "backward.end.fill",
    "calendar",
    "camera",
    "captions.bubble",
    "character.cursor.ibeam",
    "checkmark",
    "checkmark.circle",
    "chevron.down",
    "chevron.left",
    "chevron.right",
    "chevron.up",
    "chevron.up.chevron.down",
    "circle",
    "circle.fill",
    "doc",
    "ellipsis",
    "ellipsis.circle",
    "exclamationmark.triangle",
    "eye.slash",
    "film",
    "film.stack",
    "folder",
    "forward.end.fill",
    "gearshape",
    "gearshape.2",
    "gobackward.10",
    "goforward.10",
    "hand.tap",
    "heart",
    "house",
    "info.circle",
    "keyboard",
    "line.3.horizontal.decrease",
    "link",
    "list.bullet.rectangle",
    "magnifyingglass",
    "minus",
    "moon",
    "music.note",
    "note.text",
    "paintbrush",
    "paintpalette",
    "paperplane",
    "pause",
    "pause.fill",
    "person.crop.circle",
    "photo",
    "pip.enter",
    "play",
    "play.fill",
    "play.rectangle",
    "plus",
    "puzzlepiece.extension",
    "scissors",
    "server.rack",
    "shield",
    "sidebar.right",
    "slider.horizontal.3",
    "sparkles",
    "speaker.fill",
    "speaker.slash.fill",
    "speaker.wave.2",
    "speaker.wave.3.fill",
    "speedometer",
    "square",
    "square.and.arrow.up",
    "square.grid.2x2",
    "square.grid.3x3",
    "square.on.square",
    "star",
    "star.fill",
    "star.slash",
    "sun.max.fill",
    "sun.min",
    "switch.2",
    "text.bubble",
    "text.rectangle",
    "textformat",
    "textformat.size",
    "textformat.size.larger",
    "textformat.size.smaller",
    "trash",
    "tray",
    "tv",
    "viewfinder",
    "wifi",
    "xmark",
    "xmark.circle.fill",
];

/// An error at each element whose icon `has` rejects; `set` names the
/// target's icons in the message, e.g. "the Web icon set".
pub fn check(app: &Application, set: &str, has: impl Fn(&str) -> bool) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    app.walk(&mut |node: &Node, _| {
        if let Node::Element(element) = node {
            for (name, prop) in &element.props {
                if let Prop::Icon { name: icon } = prop
                    && !has(icon)
                {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "{set} has no `{icon}` (property `{name}` of `{}`)",
                            element.kind
                        ))
                        .at(element.source.clone()),
                    );
                }
            }
        }
    });
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_sorted_and_in_sf_symbols_naming() {
        assert!(NAMES.windows(2).all(|pair| pair[0] < pair[1]));
        for name in NAMES {
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.'),
                "{name}"
            );
        }
    }
}
