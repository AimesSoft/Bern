//! The Web rendering of the Bern icon set (SF Symbols naming).
//!
//! Each icon is the inner markup of a 24 × 24 SVG drawn with 1.75 px round
//! strokes in `currentColor`, so its color follows the control's recipe.
//! The shapes are Bern's own; the gear and the star are computed.

use std::f64::consts::PI;

const STROKE: &str = r#"fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round""#;

fn stroke(d: &str) -> String {
    format!(r#"<path {STROKE} d="{d}"/>"#)
}

fn fill(d: &str) -> String {
    format!(r#"<path fill="currentColor" d="{d}"/>"#)
}

fn circle(cx: f64, cy: f64, r: f64) -> String {
    format!(r#"<circle {STROKE} cx="{cx}" cy="{cy}" r="{r}"/>"#)
}

fn dot(cx: f64, cy: f64, r: f64) -> String {
    format!(r#"<circle fill="currentColor" cx="{cx}" cy="{cy}" r="{r}"/>"#)
}

fn number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded}");
    if text == "-0" { "0".into() } else { text }
}

/// A gear centred at `(cx, cy)` with `teeth` teeth between radii `inner`
/// and `outer`, and an axle hole of radius `hole`.
fn gear(cx: f64, cy: f64, outer: f64, inner: f64, teeth: usize, hole: f64) -> String {
    let mut d = String::new();
    let step = 2.0 * PI / teeth as f64;
    for tooth in 0..teeth {
        let start = tooth as f64 * step;
        // Tooth top spans 40 % of the pitch, flanks 10 % each.
        let corners = [
            (inner, start),
            (outer, start + step * 0.1),
            (outer, start + step * 0.5),
            (inner, start + step * 0.6),
        ];
        for (index, (radius, angle)) in corners.into_iter().enumerate() {
            let (x, y) = (cx + radius * angle.cos(), cy + radius * angle.sin());
            let command = if tooth == 0 && index == 0 { 'M' } else { 'L' };
            d.push_str(&format!("{command}{} {}", number(x), number(y)));
        }
    }
    d.push('Z');
    format!("{}{}", stroke(&d), circle(cx, cy, hole))
}

/// A five-pointed star centred at `(cx, cy)`.
fn star(cx: f64, cy: f64, outer: f64, inner: f64) -> String {
    let mut d = String::new();
    for point in 0..10 {
        let radius = if point % 2 == 0 { outer } else { inner };
        let angle = -PI / 2.0 + point as f64 * PI / 5.0;
        let command = if point == 0 { 'M' } else { 'L' };
        d.push_str(&format!(
            "{command}{} {}",
            number(cx + radius * angle.cos()),
            number(cy + radius * angle.sin())
        ));
    }
    d.push('Z');
    stroke(&d)
}

/// Eight sun rays between radii `from` and `to`.
fn rays(from: f64, to: f64) -> String {
    let mut d = String::new();
    for ray in 0..8 {
        let angle = ray as f64 * PI / 4.0;
        let (sin, cos) = angle.sin_cos();
        d.push_str(&format!(
            "M{} {}L{} {}",
            number(12.0 + from * cos),
            number(12.0 + from * sin),
            number(12.0 + to * cos),
            number(12.0 + to * sin)
        ));
    }
    stroke(&d)
}

const SPEAKER: &str = "M4 9.5h3.5L12 5.5v13l-4.5-4H4z";

/// The inner SVG markup of `name`, or `None` when the Web icon set does
/// not have it.
pub fn svg(name: &str) -> Option<String> {
    Some(match name {
        "plus" => stroke("M12 5v14M5 12h14"),
        "minus" => stroke("M5 12h14"),
        "xmark" => stroke("M6.5 6.5l11 11M17.5 6.5l-11 11"),
        "checkmark" => stroke("M5 12.5l4.5 4.5L19 7.5"),
        "chevron.left" => stroke("M14.5 5.5L8 12l6.5 6.5"),
        "chevron.right" => stroke("M9.5 5.5L16 12l-6.5 6.5"),
        "chevron.down" => stroke("M5.5 9.5L12 16l6.5-6.5"),
        "chevron.up.chevron.down" => stroke("M8 9.5l4-4 4 4M8 14.5l4 4 4-4"),
        "circle" => circle(12.0, 12.0, 8.0),
        "square" => stroke(
            "M6.5 5h11A1.5 1.5 0 0 1 19 6.5v11a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 17.5v-11A1.5 1.5 0 0 1 6.5 5z",
        ),
        "square.on.square" => stroke(
            "M9 9h9.5A1.5 1.5 0 0 1 20 10.5V19a1.5 1.5 0 0 1-1.5 1.5H10A1.5 1.5 0 0 1 8.5 19v-8.5M15.5 6V5A1.5 1.5 0 0 0 14 3.5H5.5A1.5 1.5 0 0 0 4 5v8.5A1.5 1.5 0 0 0 5.5 15h3",
        ),
        "magnifyingglass" => format!("{}{}", circle(10.5, 10.5, 6.0), stroke("M15 15l5 5")),
        "xmark.circle.fill" => String::from(
            r#"<mask id="bern-xmark-cut"><rect width="24" height="24" fill="white"/><path d="M9 9l6 6M15 9l-6 6" stroke="black" stroke-width="2" stroke-linecap="round"/></mask><circle cx="12" cy="12" r="9" fill="currentColor" mask="url(#bern-xmark-cut)"/>"#,
        ),
        "doc" => stroke(
            "M14 3.5H7.5A1.5 1.5 0 0 0 6 5v14a1.5 1.5 0 0 0 1.5 1.5h9A1.5 1.5 0 0 0 18 19V7.5zM14 3.5v4h4",
        ),
        "photo" => format!(
            "{}{}{}",
            stroke("M5 5h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"),
            circle(8.5, 10.0, 1.5),
            stroke("M21 15.5l-5-5L7 19")
        ),
        "film" => stroke(
            "M6 3.5h12a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-13a2 2 0 0 1 2-2zM8 3.5v17M16 3.5v17M4 8h4M4 12h4M4 16h4M16 8h4M16 12h4M16 16h4",
        ),
        "music.note" => format!(
            "{}{}{}",
            stroke("M9 17.5V5.5l10-2v12"),
            circle(6.5, 17.5, 2.5),
            circle(16.5, 15.5, 2.5)
        ),
        "arrow.down.circle" => format!(
            "{}{}",
            circle(12.0, 12.0, 9.0),
            stroke("M12 7.5v8.5M8.5 12.5L12 16l3.5-3.5")
        ),
        "gearshape" => gear(12.0, 12.0, 9.5, 7.0, 8, 3.0),
        "gearshape.2" => format!(
            "{}{}",
            gear(9.0, 9.5, 7.0, 5.0, 8, 2.0),
            gear(17.0, 17.0, 5.0, 3.6, 7, 1.4)
        ),
        "paintbrush" => stroke(
            "M14.5 3.5l6 6-8 8-6-6zM6.5 11.5c-2 .2-3 1.8-3 4 0 1.4-.5 3-1 4 3 .5 7-.5 7.5-4.5",
        ),
        "hand.tap" => stroke(
            "M9.5 12V5a1.5 1.5 0 0 1 3 0v6M12.5 10a1.5 1.5 0 0 1 3 0v2M15.5 11a1.5 1.5 0 0 1 3 0v3.5c0 3.5-2.5 6.5-6 6.5h-.8c-2 0-3.4-.9-4.5-2.5l-2.6-4a1.4 1.4 0 0 1 2.3-1.6l2.1 2.6",
        ),
        "list.bullet.rectangle" => format!(
            "{}{}{}{}",
            stroke(
                "M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM10.5 9h6M10.5 12h6M10.5 15h6"
            ),
            dot(7.5, 9.0, 1.0),
            dot(7.5, 12.0, 1.0),
            dot(7.5, 15.0, 1.0)
        ),
        "play.rectangle" => format!(
            "{}{}",
            stroke("M5 5h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"),
            fill("M10 9.2v5.6l4.8-2.8z")
        ),
        "play.fill" => {
            fill("M8 5.2v13.6a.8.8 0 0 0 1.2.7l10.6-6.8a.8.8 0 0 0 0-1.4L9.2 4.5A.8.8 0 0 0 8 5.2z")
        }
        "pause.fill" => fill(
            "M7 5h3a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zM14 5h3a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
        ),
        "speaker.fill" => fill(SPEAKER),
        "speaker.wave.3.fill" => format!(
            "{}{}",
            fill(SPEAKER),
            stroke("M15 9.5a3.5 3.5 0 0 1 0 5M17.5 7a7 7 0 0 1 0 10M20 4.5a10.5 10.5 0 0 1 0 15")
        ),
        "speaker.slash.fill" => {
            format!("{}{}", fill(SPEAKER), stroke("M15.5 9.5l5 5M20.5 9.5l-5 5"))
        }
        "square.and.arrow.up" => stroke(
            "M8.5 9H7a1.5 1.5 0 0 0-1.5 1.5v8A1.5 1.5 0 0 0 7 20h10a1.5 1.5 0 0 0 1.5-1.5v-8A1.5 1.5 0 0 0 17 9h-1.5M12 3.5V14M8.5 7L12 3.5 15.5 7",
        ),
        "star" => star(12.0, 12.5, 9.0, 3.8),
        "sun.max.fill" => format!("{}{}", dot(12.0, 12.0, 4.0), rays(6.5, 9.5)),
        "sun.min" => format!("{}{}", circle(12.0, 12.0, 3.5), rays(6.0, 7.5)),
        "switch.2" => format!(
            "{}{}{}",
            stroke("M3.5 8h9M17.5 8h3M3.5 16h3M11.5 16h9"),
            circle(15.0, 8.0, 2.5),
            circle(9.0, 16.0, 2.5)
        ),
        "textformat" => format!(
            "{}{}",
            stroke("M3 18.5L7.5 6l4.5 12.5M4.6 14h5.8M20.5 12v6.5"),
            circle(17.5, 15.5, 3.0)
        ),
        "textformat.size.larger" => {
            stroke("M3.5 19.5L9 5l5.5 14.5M5.6 14.5h6.8M19 10V3.5M16.5 6L19 3.5 21.5 6")
        }
        "textformat.size.smaller" => {
            stroke("M5 19.5l4-10.5 4 10.5M6.5 15.5h5M19 3.5V10M16.5 7.5L19 10l2.5-2.5")
        }
        "character.cursor.ibeam" => {
            stroke("M3 18.5L7.5 6.5l4.5 12M4.7 14.5h5.6M17 5v14M15 5h4M15 19h4")
        }
        "trash" => stroke("M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13M10 11v5.5M14 11v5.5"),
        "tray" => stroke(
            "M3 13.5l2.5-8h13l2.5 8V19a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1zM3 13.5h5l1.5 2.5h5l1.5-2.5h5",
        ),
        "exclamationmark.triangle" => format!(
            "{}{}",
            stroke(
                "M10.3 4.5a2 2 0 0 1 3.4 0l7.6 13a2 2 0 0 1-1.7 3H4.4a2 2 0 0 1-1.7-3zM12 9.5v4.5"
            ),
            dot(12.0, 17.0, 1.0)
        ),
        _ => return None,
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
    fn runtime_icons_exist_and_markup_is_well_formed() {
        for name in RUNTIME_ICONS {
            let markup = svg(name).unwrap_or_else(|| panic!("missing runtime icon {name}"));
            // Every element either closes itself or is a pair.
            assert_eq!(
                markup.matches('<').count(),
                markup.matches("/>").count() + 2 * markup.matches("</").count(),
                "{name}: every element closes"
            );
            assert!(!markup.contains("NaN"), "{name}");
        }
        assert!(svg("not.an.icon").is_none());
    }
}
