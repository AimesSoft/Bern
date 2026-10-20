//! The Web rendering of the Bern icon set (SF Symbols naming), drawn with
//! Lucide (`lucide-static` v1.54.0, ISC; icons derived from Feather are
//! MIT).
//!
//! The SVG files under `icons/lucide` are Lucide's, unchanged. This table
//! maps each Bern name to one of them and says how a `.fill` name fills
//! it: Lucide has no filled icons, so Bern adds fills to the outline and
//! keeps its 2 px stroke, which keeps the filled icon's silhouette. The
//! runtime's `icon()` puts Lucide's stroke attributes on the root `<svg>`.

/// How a name fills its Lucide outline, by the position of its shapes.
enum Fill {
    /// An outline icon.
    None,
    /// Every shape filled: icons made of closed shapes only.
    All,
    /// Only these shapes filled; the rest stay strokes.
    Shapes(&'static [usize]),
    /// `solid` shapes filled with `cut` shapes knocked out of them.
    Knockout {
        solid: &'static [usize],
        cut: &'static [usize],
    },
}

macro_rules! lucide {
    ($file:literal) => {
        include_str!(concat!("../icons/lucide/", $file, ".svg"))
    };
}

/// `(Bern name, Lucide SVG, fill)`.
const ICONS: &[(&str, &str, Fill)] = &[
    ("plus", lucide!("plus"), Fill::None),
    ("minus", lucide!("minus"), Fill::None),
    ("xmark", lucide!("x"), Fill::None),
    ("checkmark", lucide!("check"), Fill::None),
    ("chevron.left", lucide!("chevron-left"), Fill::None),
    ("chevron.right", lucide!("chevron-right"), Fill::None),
    ("chevron.up", lucide!("chevron-up"), Fill::None),
    ("chevron.down", lucide!("chevron-down"), Fill::None),
    (
        "chevron.up.chevron.down",
        lucide!("chevrons-up-down"),
        Fill::None,
    ),
    ("circle", lucide!("circle"), Fill::None),
    ("ellipsis", lucide!("ellipsis"), Fill::None),
    ("ellipsis.circle", lucide!("circle-ellipsis"), Fill::None),
    ("square", lucide!("square"), Fill::None),
    ("square.on.square", lucide!("copy"), Fill::None),
    ("magnifyingglass", lucide!("search"), Fill::None),
    (
        "xmark.circle.fill",
        lucide!("circle-x"),
        Fill::Knockout {
            solid: &[0],
            cut: &[1, 2],
        },
    ),
    ("doc", lucide!("file"), Fill::None),
    ("note.text", lucide!("notebook-text"), Fill::None),
    ("archivebox", lucide!("archive"), Fill::None),
    ("photo", lucide!("image"), Fill::None),
    ("film", lucide!("film"), Fill::None),
    ("film.stack", lucide!("gallery-vertical-end"), Fill::None),
    ("house", lucide!("house"), Fill::None),
    ("calendar", lucide!("calendar"), Fill::None),
    ("text.bubble", lucide!("message-square-text"), Fill::None),
    ("paperplane", lucide!("send"), Fill::None),
    ("sparkles", lucide!("sparkles"), Fill::None),
    (
        "arrow.up.left.and.arrow.down.right",
        lucide!("maximize-2"),
        Fill::None,
    ),
    ("speaker.wave.2", lucide!("volume-2"), Fill::None),
    ("link", lucide!("link"), Fill::None),
    ("camera", lucide!("camera"), Fill::None),
    (
        "backward.end.fill",
        lucide!("skip-back"),
        Fill::Shapes(&[0]),
    ),
    (
        "forward.end.fill",
        lucide!("skip-forward"),
        Fill::Shapes(&[0]),
    ),
    ("pip.enter", lucide!("picture-in-picture-2"), Fill::None),
    (
        "slider.horizontal.3",
        lucide!("sliders-horizontal"),
        Fill::None,
    ),
    ("gobackward.10", lucide!("rotate-ccw"), Fill::None),
    ("goforward.10", lucide!("rotate-cw"), Fill::None),
    ("circle.fill", lucide!("circle"), Fill::All),
    ("play", lucide!("play"), Fill::None),
    ("pause", lucide!("pause"), Fill::None),
    ("puzzlepiece.extension", lucide!("puzzle"), Fill::None),
    ("keyboard", lucide!("keyboard"), Fill::None),
    ("moon", lucide!("moon"), Fill::None),
    (
        "arrow.triangle.2.circlepath",
        lucide!("refresh-cw"),
        Fill::None,
    ),
    ("server.rack", lucide!("server"), Fill::None),
    ("wifi", lucide!("wifi"), Fill::None),
    ("eye.slash", lucide!("eye-off"), Fill::None),
    (
        "line.3.horizontal.decrease",
        lucide!("list-filter"),
        Fill::None,
    ),
    ("shield", lucide!("shield"), Fill::None),
    ("tv", lucide!("tv"), Fill::None),
    ("scissors", lucide!("scissors"), Fill::None),
    (
        "arrow.up.right.square",
        lucide!("square-arrow-out-up-right"),
        Fill::None,
    ),
    ("heart", lucide!("heart"), Fill::None),
    ("arrow.up.arrow.down", lucide!("arrow-down-up"), Fill::None),
    ("checkmark.circle", lucide!("circle-check"), Fill::None),
    ("arrow.down", lucide!("arrow-down"), Fill::None),
    ("arrow.left", lucide!("arrow-left"), Fill::None),
    ("music.note", lucide!("music"), Fill::None),
    (
        "arrow.down.circle",
        lucide!("circle-arrow-down"),
        Fill::None,
    ),
    ("gearshape", lucide!("settings"), Fill::None),
    ("gearshape.2", lucide!("cog"), Fill::None),
    ("paintbrush", lucide!("paintbrush"), Fill::None),
    ("hand.tap", lucide!("pointer"), Fill::None),
    ("list.bullet.rectangle", lucide!("list"), Fill::None),
    ("play.rectangle", lucide!("square-play"), Fill::None),
    ("play.fill", lucide!("play"), Fill::All),
    ("pause.fill", lucide!("pause"), Fill::All),
    ("speaker.fill", lucide!("volume"), Fill::All),
    // The speaker is filled; the sound waves and the cross stay lines.
    (
        "speaker.wave.3.fill",
        lucide!("volume-2"),
        Fill::Shapes(&[0]),
    ),
    (
        "speaker.slash.fill",
        lucide!("volume-x"),
        Fill::Shapes(&[0]),
    ),
    ("square.and.arrow.up", lucide!("share"), Fill::None),
    ("star", lucide!("star"), Fill::None),
    ("star.fill", lucide!("star"), Fill::All),
    ("star.slash", lucide!("star-off"), Fill::None),
    ("folder", lucide!("folder"), Fill::None),
    ("paintpalette", lucide!("palette"), Fill::None),
    ("info.circle", lucide!("info"), Fill::None),
    ("sidebar.right", lucide!("panel-right"), Fill::None),
    ("square.grid.2x2", lucide!("layout-grid"), Fill::None),
    ("square.grid.3x3", lucide!("grid-3x3"), Fill::None),
    ("person.crop.circle", lucide!("circle-user"), Fill::None),
    (
        "captions.bubble",
        lucide!("message-square-text"),
        Fill::None,
    ),
    ("textformat.size", lucide!("a-large-small"), Fill::None),
    // The disc is filled; the rays stay lines.
    ("sun.max.fill", lucide!("sun"), Fill::Shapes(&[0])),
    ("sun.min", lucide!("sun-dim"), Fill::None),
    ("switch.2", lucide!("toggle-right"), Fill::None),
    ("textformat", lucide!("type"), Fill::None),
    ("textformat.size.larger", lucide!("a-arrow-up"), Fill::None),
    (
        "textformat.size.smaller",
        lucide!("a-arrow-down"),
        Fill::None,
    ),
    (
        "character.cursor.ibeam",
        lucide!("text-cursor-input"),
        Fill::None,
    ),
    ("trash", lucide!("trash-2"), Fill::None),
    ("tray", lucide!("inbox"), Fill::None),
    (
        "exclamationmark.triangle",
        lucide!("triangle-alert"),
        Fill::None,
    ),
];

/// The license notices that must ship with the icons, written next to the
/// generated page.
pub const NOTICES: &str = concat!(
    "Bern's Web icons are drawn with Lucide (lucide-static 1.54.0), https://lucide.dev.\n",
    "Icons whose Bern names end in `.fill` add fills to Lucide's outlines; they are\n",
    "modified versions based on Lucide, not official Lucide icons.\n\n",
    include_str!("../icons/lucide/LICENSE"),
);

/// Placeholder the runtime replaces with an id unique to each icon it
/// draws, for the masks of knocked-out icons.
pub const ID_PLACEHOLDER: &str = "__ID__";

/// The shape elements of a Lucide SVG, each a self-closing tag.
fn shapes(source: &str) -> Vec<String> {
    let body = source
        .find("<svg")
        .and_then(|at| source[at..].find('>').map(|end| at + end + 1))
        .map(|start| &source[start..source.rfind("</svg>").unwrap_or(source.len())])
        .unwrap_or("");
    body.split('<')
        .filter_map(|part| part.split_once("/>").map(|(tag, _)| tag))
        .map(|tag| {
            let tag: Vec<&str> = tag.split_whitespace().collect();
            format!("<{}/>", tag.join(" "))
        })
        .collect()
}

fn with(shape: &str, attributes: &str) -> String {
    format!("{} {attributes}/>", shape.trim_end_matches("/>"))
}

/// The inner SVG markup of `name`, or `None` when the Web icon set does
/// not have it.
pub fn svg(name: &str) -> Option<String> {
    let (_, source, fill) = ICONS.iter().find(|(icon, _, _)| *icon == name)?;
    let shapes = shapes(source);
    let filled = |index: usize| with(&shapes[index], r#"fill="currentColor""#);
    Some(match fill {
        Fill::None => shapes.concat(),
        Fill::All => (0..shapes.len()).map(filled).collect(),
        Fill::Shapes(solid) => (0..shapes.len())
            .map(|index| {
                if solid.contains(&index) {
                    filled(index)
                } else {
                    shapes[index].clone()
                }
            })
            .collect(),
        Fill::Knockout { solid, cut } => {
            // White keeps, black removes: the cut shapes' strokes become
            // gaps in the filled shapes.
            let mut markup = format!(
                r##"<defs><mask id="{ID_PLACEHOLDER}" maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24"><rect width="24" height="24" fill="#fff" stroke="none"/>"##
            );
            for &index in *cut {
                markup.push_str(&with(&shapes[index], r##"stroke="#000""##));
            }
            markup.push_str("</mask></defs>");
            for &index in *solid {
                markup.push_str(&with(
                    &shapes[index],
                    &format!(r#"fill="currentColor" mask="url(#{ID_PLACEHOLDER})""#),
                ));
            }
            markup
        }
    })
}

/// Icons the runtime itself draws, whatever the application declares.
pub const RUNTIME_ICONS: &[&str] = &[
    "xmark",
    "minus",
    "square",
    "square.on.square",
    "chevron.left",
    "chevron.right",
    "chevron.up.chevron.down",
    "chevron.down",
    "checkmark",
    "magnifyingglass",
    "xmark.circle.fill",
    "gearshape",
    "tray",
    "exclamationmark.triangle",
    "play.fill",
    "pause.fill",
    "speaker.wave.3.fill",
    "speaker.slash.fill",
    "circle",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_comes_from_lucide_and_its_fills_name_real_shapes() {
        for (name, source, fill) in ICONS {
            assert!(
                source.contains("@license lucide-static v1.54.0"),
                "{name}: not a Lucide file"
            );
            let count = shapes(source).len();
            assert!(count > 0, "{name}: no shapes");
            let indices: Vec<usize> = match fill {
                Fill::None | Fill::All => vec![],
                Fill::Shapes(solid) => solid.to_vec(),
                Fill::Knockout { solid, cut } => solid.iter().chain(cut.iter()).copied().collect(),
            };
            assert!(indices.iter().all(|&at| at < count), "{name}: {indices:?}");
            // Only `.fill` names are filled.
            assert_eq!(
                name.ends_with(".fill"),
                !matches!(fill, Fill::None),
                "{name}"
            );
        }
    }

    #[test]
    fn runtime_icons_exist_and_markup_is_well_formed() {
        for name in RUNTIME_ICONS {
            assert!(svg(name).is_some(), "missing runtime icon {name}");
        }
        for (name, _, _) in ICONS {
            let markup = svg(name).unwrap();
            // Every element either closes itself or is a pair.
            assert_eq!(
                markup.matches('<').count(),
                markup.matches("/>").count() + 2 * markup.matches("</").count(),
                "{name}: every element closes"
            );
            assert!(!markup.contains("class="), "{name}");
        }
        assert!(svg("not.an.icon").is_none());
    }

    #[test]
    fn fills_follow_the_table() {
        let count = |markup: &str| markup.matches(r#"fill="currentColor""#).count();
        assert_eq!(count(&svg("play.fill").unwrap()), 1);
        // The speaker is filled and both waves stay lines.
        let speaker = svg("speaker.wave.3.fill").unwrap();
        assert_eq!(count(&speaker), 1);
        assert_eq!(speaker.matches("<path").count(), 3);
        // The cross is cut out of the disc through a per-icon mask.
        let clear = svg("xmark.circle.fill").unwrap();
        assert!(clear.contains(&format!(r#"<mask id="{ID_PLACEHOLDER}""#)));
        assert!(clear.contains(&format!(r#"mask="url(#{ID_PLACEHOLDER})""#)));
        assert!(!svg("star").unwrap().contains("fill="));
    }

    #[test]
    fn every_name_of_the_bern_icon_set_is_drawn() {
        for name in bern_target_api::icon_set::NAMES {
            assert!(svg(name).is_some(), "the Web icon set lacks {name}");
        }
    }

    #[test]
    fn notices_carry_both_licenses() {
        assert!(NOTICES.contains("ISC License"));
        assert!(NOTICES.contains("Cole Bemis"));
    }
}
