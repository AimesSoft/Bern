//! Animated SVG documents shared by every Bern presenter.
//!
//! Rendering uses the platform's SVG/DOM engine, preserving CSS, SMIL,
//! filters, masks, text, and document-local JavaScript instead of flattening
//! an animation into bitmaps. Documents are self-contained bundled assets;
//! they have no native command bridge or access to external resources.

pub const RUNTIME: &str = include_str!("runtime.js");
pub const PREFIX: &str = r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; connect-src 'none'; base-uri 'none'; form-action 'none'"><style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}body>svg{display:block;width:100%!important;height:100%!important}html{color-scheme:light dark}</style></head><body>"#;

pub fn suffix() -> String {
    format!("<script>{RUNTIME}</script></body></html>")
}

/// Validate an SVG document, preserving its original source and scripts.
/// Reject DTDs/external entities. XML declarations are valid on input, but
/// only the root SVG is embedded into the HTML document.
pub fn root(source: &str) -> Result<&str, String> {
    let doc = roxmltree::Document::parse(source).map_err(|e| format!("invalid SVG: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err("expected an SVG root in the SVG namespace".into());
    }
    Ok(&source[root.range()])
}

pub fn document(source: &str) -> Result<String, String> {
    Ok(format!("{PREFIX}{}{}", root(source)?, suffix()))
}

/// How a document shows as a picture of it, drawn once when the app is
/// built, with no document engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    /// In its own colors.
    Still,
    /// In the color of the text around it (`currentColor`, and no other
    /// color): a template whose shape the platform paints.
    Tinted,
    /// In colors of its own for the light and for the dark appearance
    /// (`@media (prefers-color-scheme: …)`, `light-dark()`): one picture of
    /// each, `in_appearance`.
    Themed,
}

/// A document engine is for what moves or runs: SMIL or CSS animation,
/// transitions, pointer states, scripts, links, embedded HTML or media, and
/// what a picture cannot follow yet: media queries other than the
/// appearance, CSS variables, and `currentColor` beside other colors or
/// beside colors per appearance. Everything else is a picture of the
/// document; `None` for the rest and anything unreadable.
pub fn picture(source: &str) -> Option<Picture> {
    const LIVE: &[&str] = &[
        "animate",
        "animateMotion",
        "animateTransform",
        "animateColor",
        "set",
        "discard",
        "script",
        "a",
        "foreignObject",
        "video",
        "audio",
        "iframe",
        "canvas",
    ];
    const LIVE_CSS: &[&str] = &[
        "@keyframes",
        "animation",
        "transition",
        ":hover",
        ":active",
        ":focus",
        "var(",
    ];
    const COLORS: &[&str] = &[
        "fill",
        "stroke",
        "stop-color",
        "flood-color",
        "lighting-color",
        "color",
    ];
    let live = |text: &str| {
        let text = text.to_ascii_lowercase();
        LIVE_CSS.iter().any(|word| text.contains(word))
    };
    let doc = roxmltree::Document::parse(source).ok()?;
    let (mut tinted, mut colored, mut themed) = (false, false, false);
    let mut paint = |property: &str, value: &str| {
        if !COLORS.contains(&property.trim()) {
            return;
        }
        let value = value.trim().trim_end_matches("!important").trim();
        let value = value.to_ascii_lowercase();
        if value.contains("currentcolor") {
            tinted = true;
        }
        match value.as_str() {
            "currentcolor" | "" | "none" | "transparent" | "inherit" => {}
            _ => colored = true,
        }
    };
    // `property: value` pairs of a style sheet or a `style` attribute.
    let declarations = |css: &str| {
        css.split([';', '{', '}'])
            .filter_map(|part| part.split_once(':'))
            .map(|(property, value)| (property.trim().to_string(), value.to_string()))
            .collect::<Vec<_>>()
    };
    for node in doc.descendants() {
        if node.is_text() {
            if node
                .parent_element()
                .is_some_and(|parent| parent.tag_name().name() == "style")
            {
                let css = node.text().unwrap_or("");
                if live(css) {
                    return None;
                }
                // Rules for one appearance, and no other media queries.
                for block in media_blocks(css) {
                    scheme(block.condition)?;
                    if css[block.body.clone()]
                        .to_ascii_lowercase()
                        .contains("@media")
                    {
                        return None;
                    }
                    themed = true;
                }
                themed |= css.to_ascii_lowercase().contains("light-dark(");
                for (property, value) in declarations(css) {
                    paint(&property, &value);
                }
            }
            continue;
        }
        if !node.is_element() {
            continue;
        }
        if LIVE.contains(&node.tag_name().name()) {
            return None;
        }
        for attribute in node.attributes() {
            let value = attribute.value().to_ascii_lowercase();
            if attribute.name().starts_with("on") || live(&value) || value.contains("@media") {
                return None;
            }
            themed |= value.contains("light-dark(");
            if attribute.name() == "style" {
                for (property, value) in declarations(attribute.value()) {
                    paint(&property, &value);
                }
            } else {
                paint(attribute.name(), attribute.value());
            }
        }
    }
    match (tinted, colored, themed) {
        (false, _, false) => Some(Picture::Still),
        (false, _, true) => Some(Picture::Themed),
        (true, false, false) => Some(Picture::Tinted),
        (true, _, _) => None,
    }
}

/// The document as it shows in the light or the dark appearance, for a
/// renderer that follows neither media queries nor `light-dark()`: the
/// style rules for this appearance apply everywhere, those for the other
/// are gone, and each `light-dark(light, dark)` is its value here. Other
/// media queries stay as they are; an unreadable document is unchanged.
pub fn in_appearance(source: &str, dark: bool) -> String {
    let Ok(doc) = roxmltree::Document::parse(source) else {
        return source.to_string();
    };
    // Byte ranges of the source and what they become, apart and in order.
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for node in doc.descendants().filter(|node| node.is_element()) {
        for attribute in node.attributes() {
            let range = attribute.range_value();
            let value = &source[range.clone()];
            if value.to_ascii_lowercase().contains("light-dark(") {
                edits.push((range, chosen(value, dark)));
            }
        }
        if node.tag_name().name() != "style" {
            continue;
        }
        // The style sheet between `<style …>` and `</style>`.
        let range = node.range();
        let whole = &source[range.clone()];
        let (Some(open), Some(close)) = (whole.find('>'), whole.rfind("</")) else {
            continue;
        };
        if open + 1 > close {
            continue;
        }
        let css = &whole[open + 1..close];
        let mut sheet = String::new();
        let mut at = 0;
        for block in media_blocks(css) {
            let Some(only) = scheme(block.condition) else {
                continue;
            };
            sheet.push_str(&css[at..block.whole.start]);
            if only == dark {
                sheet.push_str(&css[block.body.clone()]);
            }
            at = block.whole.end;
        }
        sheet.push_str(&css[at..]);
        let start = range.start + open + 1;
        edits.push((start..range.start + close, chosen(&sheet, dark)));
    }
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for (range, text) in edits {
        out.push_str(&source[at..range.start]);
        out.push_str(&text);
        at = range.end;
    }
    out.push_str(&source[at..]);
    out
}

/// An `@media` rule of a style sheet, in bytes of the sheet.
struct MediaBlock<'a> {
    /// From `@media` to its closing brace.
    whole: std::ops::Range<usize>,
    /// Between its braces.
    body: std::ops::Range<usize>,
    condition: &'a str,
}

/// The `@media` rules at the top level of `css`, in order, skipping
/// comments.
fn media_blocks(css: &str) -> Vec<MediaBlock<'_>> {
    let bytes = css.as_bytes();
    let mut blocks = Vec::new();
    let (mut at, mut depth) = (0, 0usize);
    while at < bytes.len() {
        if css[at..].starts_with("/*") {
            at = css[at + 2..]
                .find("*/")
                .map_or(bytes.len(), |end| at + 2 + end + 2);
            continue;
        }
        match bytes[at] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'@' if depth == 0
                && css
                    .get(at..at + 6)
                    .is_some_and(|word| word.eq_ignore_ascii_case("@media")) =>
            {
                let Some(open) = css[at..].find('{').map(|open| at + open) else {
                    break;
                };
                let mut inner = 0usize;
                let mut end = None;
                for (offset, byte) in bytes[open..].iter().enumerate() {
                    match byte {
                        b'{' => inner += 1,
                        b'}' => {
                            inner -= 1;
                            if inner == 0 {
                                end = Some(open + offset);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Some(close) = end else {
                    break;
                };
                blocks.push(MediaBlock {
                    whole: at..close + 1,
                    body: open + 1..close,
                    condition: &css[at + 6..open],
                });
                at = close + 1;
                continue;
            }
            _ => {}
        }
        at += 1;
    }
    blocks
}

/// The appearance a media condition asks for: `Some(true)` for
/// `(prefers-color-scheme: dark)`, `Some(false)` for light, with or without
/// `screen and`; `None` for any other condition.
fn scheme(condition: &str) -> Option<bool> {
    let condition = condition.to_ascii_lowercase();
    let mut words = condition.split_whitespace().collect::<Vec<_>>().join(" ");
    for prefix in ["only ", "screen and ", "all and "] {
        if let Some(rest) = words.strip_prefix(prefix) {
            words = rest.to_string();
        }
    }
    match words.replace(' ', "").as_str() {
        "(prefers-color-scheme:dark)" => Some(true),
        "(prefers-color-scheme:light)" => Some(false),
        _ => None,
    }
}

/// `text` with each `light-dark(light, dark)` replaced by its value in the
/// appearance, innermost calls included.
fn chosen(text: &str, dark: bool) -> String {
    let mut text = text.to_string();
    while let Some(start) = text.to_ascii_lowercase().find("light-dark(") {
        let open = start + "light-dark(".len();
        let (mut depth, mut comma, mut close) = (1usize, None, None);
        for (offset, byte) in text.as_bytes()[open..].iter().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
                b',' if depth == 1 && comma.is_none() => comma = Some(open + offset),
                _ => {}
            }
        }
        let Some(close) = close else {
            break;
        };
        let value = match comma {
            Some(comma) if dark => text[comma + 1..close].trim().to_string(),
            Some(comma) => text[open..comma].trim().to_string(),
            None => text[open..close].trim().to_string(),
        };
        text.replace_range(start..close + 1, &value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_css_smil_and_script() {
        let svg = r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"><style>@keyframes a{to{opacity:0}}</style><rect><animate attributeName="x" to="20" dur="1s"/></rect><script><![CDATA[document.documentElement.dataset.test = 'ok';]]></script></svg>"#;
        let html = document(svg).unwrap();
        assert!(!html.contains("<?xml"));
        assert!(html.contains("@keyframes a"));
        assert!(html.contains("attributeName=\"x\""));
        assert!(html.contains("dataset.test"));
        assert!(html.contains("default-src 'none'"));
    }

    #[test]
    fn documents_that_neither_move_nor_run_are_pictures() {
        let svg = |body: &str| {
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">{body}</svg>"#)
        };
        for still in [
            r#"<defs><mask id="m"><rect width="10" height="10" fill="white"/></mask></defs><circle r="4" fill="white" mask="url(#m)"/>"#,
            r#"<style>.a{fill:red}</style><rect class="a" width="2" height="2"/>"#,
            r#"<text>Bern</text>"#,
        ] {
            assert_eq!(picture(&svg(still)), Some(Picture::Still), "{still}");
        }
        for tinted in [
            r#"<path d="M0 0h4v4z" fill="currentColor"/>"#,
            r#"<path d="M0 0h4v4z" style="stroke: currentColor; fill: none"/>"#,
            r#"<style>path{fill:currentcolor}</style><path d="M0 0h4v4z"/>"#,
        ] {
            assert_eq!(picture(&svg(tinted)), Some(Picture::Tinted), "{tinted}");
        }
        for themed in [
            r#"<style>rect{fill:black}@media (prefers-color-scheme: dark){rect{fill:white}}</style><rect/>"#,
            r#"<style><![CDATA[@media screen and (prefers-color-scheme:light){rect{fill:#123}}]]></style><rect/>"#,
            r#"<style>rect{fill:light-dark(#000, #fff)}</style><rect/>"#,
            r#"<rect style="fill: light-dark(rgb(0, 0, 0), white)"/>"#,
        ] {
            assert_eq!(picture(&svg(themed)), Some(Picture::Themed), "{themed}");
        }
        for live in [
            r#"<rect><animate attributeName="x" to="2" dur="1s"/></rect>"#,
            r#"<style>@keyframes s{to{opacity:0}}</style>"#,
            r#"<rect style="animation: s 1s infinite"/>"#,
            r#"<style>rect:hover{fill:red}</style><rect/>"#,
            r#"<style>@media (min-width: 20px){rect{fill:white}}</style><rect/>"#,
            r#"<style>@media not (prefers-color-scheme: dark){rect{fill:white}}</style><rect/>"#,
            r#"<style>@media (prefers-color-scheme: dark){rect{fill:currentColor}}</style><rect/>"#,
            r#"<rect style="fill: light-dark(currentColor, white)"/>"#,
            r#"<rect fill="currentColor"/><rect fill="red"/>"#,
            r#"<rect onclick="alert(1)"/>"#,
            r#"<script>1</script>"#,
            r#"<a href="https://example.com"><rect/></a>"#,
        ] {
            assert_eq!(picture(&svg(live)), None, "{live}");
        }
        assert_eq!(picture("<svg"), None);
    }

    #[test]
    fn a_document_in_one_appearance_keeps_only_its_rules() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">"#,
            r#"<style type="text/css">rect{fill:black}"#,
            r#"@media (prefers-color-scheme: dark){rect{fill:white}.b{stroke:#eee}}"#,
            r#"@media (prefers-color-scheme: light){.b{stroke:#111}}"#,
            r#"@media (min-width: 4px){.c{fill:red}}"#,
            r#"circle{fill:light-dark(rgb(1, 2, 3), #abc)}</style>"#,
            r#"<rect class="b" width="4" height="4"/>"#,
            r#"<circle r="2" style="stroke: light-dark(red, light-dark(green, blue))"/></svg>"#,
        );
        let light = in_appearance(svg, false);
        assert!(
            light.contains("rect{fill:black}.b{stroke:#111}@media (min-width: 4px)"),
            "{light}"
        );
        assert!(light.contains("circle{fill:rgb(1, 2, 3)}"), "{light}");
        assert!(light.contains(r#"style="stroke: red""#), "{light}");
        assert!(
            !light.contains("prefers-color-scheme") && !light.contains("#eee"),
            "{light}"
        );
        let dark = in_appearance(svg, true);
        assert!(
            dark.contains("rect{fill:black}rect{fill:white}.b{stroke:#eee}@media (min-width: 4px)"),
            "{dark}"
        );
        assert!(dark.contains("circle{fill:#abc}"), "{dark}");
        assert!(dark.contains(r#"style="stroke: blue""#), "{dark}");
        assert!(!dark.contains("#111"), "{dark}");
        // A document without rules per appearance is the same in both.
        let plain = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="red"/></svg>"#;
        assert_eq!(in_appearance(plain, true), plain);
        assert_eq!(in_appearance("<svg", true), "<svg");
    }

    #[test]
    fn rejects_non_svg_and_external_entities() {
        for source in [
            "<html/>",
            "<svg/>",
            "<svg",
            "<!DOCTYPE svg [<!ENTITY x SYSTEM 'file:///tmp/x'>]><svg xmlns=\"http://www.w3.org/2000/svg\">&x;</svg>",
        ] {
            assert!(document(source).is_err());
        }
    }
}
