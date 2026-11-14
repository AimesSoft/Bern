//! Bern's portable pure-Rust animated SVG compiler and player.
//!
//! Static SVG documents (`bern_svg::picture`) are rasterized once at build time
//! into PNGs and lowered to `Image`. Animated and interactive SVG documents are
//! compiled at build time into compact binary `BSVG` bytecode (`bern_svg::compile`)
//! and played in-process by `bern_svg::Player` using `tiny-skia`, with zero
//! WebView or JavaScript runtime dependency on any target.

pub mod codec;
#[cfg(feature = "svg")]
pub mod compile;
#[cfg(feature = "svg")]
pub mod css;
pub mod model;
pub mod path;
pub mod player;

pub use model::{MAGIC, ViewBox};
pub use player::{Config, Fit, PixelFormat, Player, PointerEvent, Scene, StepOutcome};

#[cfg(feature = "svg")]
pub use compile::root;

/// Compile an `.svg` XML document into compact binary `BSVG` bytecode.
#[cfg(feature = "svg")]
pub fn compile(source: &str) -> Result<Vec<u8>, String> {
    let scene = compile::compile_scene(source)?;
    Ok(codec::encode(&scene))
}

/// How a document shows as a picture of it, drawn once when the app is
/// built, with no animation player.
#[cfg(feature = "svg")]
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

/// Classify an SVG document that neither moves nor reacts to input as a
/// build-time static picture; returns `None` for animated or interactive SVGs
/// and unreadable documents.
#[cfg(feature = "svg")]
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
    let declarations = |css_text: &str| {
        css_text
            .split([';', '{', '}'])
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
                let sheet = node.text().unwrap_or("");
                if live(sheet) {
                    return None;
                }
                for block in media_blocks(sheet) {
                    scheme(block.condition)?;
                    if sheet[block.body.clone()]
                        .to_ascii_lowercase()
                        .contains("@media")
                    {
                        return None;
                    }
                    themed = true;
                }
                themed |= sheet.to_ascii_lowercase().contains("light-dark(");
                for (property, value) in declarations(sheet) {
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
            let name = attribute.name();
            let value = attribute.value().to_ascii_lowercase();
            if name.starts_with("on")
                || name.starts_with("data-bern-")
                || live(&value)
                || value.contains("@media")
            {
                return None;
            }
            themed |= value.contains("light-dark(");
            if name == "style" {
                for (property, value) in declarations(attribute.value()) {
                    paint(&property, &value);
                }
            } else {
                paint(name, attribute.value());
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

/// Expand an SVG document for either light (`dark = false`) or dark
/// (`dark = true`) appearance, for static build-time rasterizers (`resvg`)
/// that evaluate neither `@media (prefers-color-scheme)` nor `light-dark()`.
#[cfg(feature = "svg")]
pub fn in_appearance(source: &str, dark: bool) -> String {
    let Ok(doc) = roxmltree::Document::parse(source) else {
        return source.to_string();
    };
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
        let range = node.range();
        let whole = &source[range.clone()];
        let (Some(open), Some(close)) = (whole.find('>'), whole.rfind("</")) else {
            continue;
        };
        if open + 1 > close {
            continue;
        }
        let sheet = &whole[open + 1..close];
        let mut out_sheet = String::new();
        let mut at = 0;
        for block in media_blocks(sheet) {
            let Some(only) = scheme(block.condition) else {
                continue;
            };
            out_sheet.push_str(&sheet[at..block.whole.start]);
            if only == dark {
                out_sheet.push_str(&sheet[block.body.clone()]);
            }
            at = block.whole.end;
        }
        out_sheet.push_str(&sheet[at..]);
        let start = range.start + open + 1;
        edits.push((start..range.start + close, chosen(&out_sheet, dark)));
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

#[cfg(feature = "svg")]
struct MediaBlock<'a> {
    whole: std::ops::Range<usize>,
    body: std::ops::Range<usize>,
    condition: &'a str,
}

#[cfg(feature = "svg")]
fn media_blocks(css_text: &str) -> Vec<MediaBlock<'_>> {
    let bytes = css_text.as_bytes();
    let mut blocks = Vec::new();
    let (mut at, mut depth) = (0, 0usize);
    while at < bytes.len() {
        if css_text[at..].starts_with("/*") {
            at = css_text[at + 2..]
                .find("*/")
                .map_or(bytes.len(), |end| at + 2 + end + 2);
            continue;
        }
        match bytes[at] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'@' if depth == 0
                && css_text
                    .get(at..at + 6)
                    .is_some_and(|word| word.eq_ignore_ascii_case("@media")) =>
            {
                let Some(open) = css_text[at..].find('{').map(|open| at + open) else {
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
                    condition: &css_text[at + 6..open],
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

#[cfg(feature = "svg")]
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

#[cfg(feature = "svg")]
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

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::*;
    use std::sync::Arc;

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
        ] {
            assert_eq!(picture(&svg(live)), None, "{live}");
        }
        assert_eq!(
            picture(
                r#"<svg xmlns="http://www.w3.org/2000/svg" data-bern-dblclick="toggle(alt)"><rect/></svg>"#
            ),
            None
        );
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
        assert!(light.contains("rect{fill:black}.b{stroke:#111}@media (min-width: 4px)"));
        assert!(light.contains("circle{fill:rgb(1, 2, 3)}"));
        let dark = in_appearance(svg, true);
        assert!(
            dark.contains("rect{fill:black}rect{fill:white}.b{stroke:#eee}@media (min-width: 4px)")
        );
        assert!(dark.contains("circle{fill:#abc}"));
    }

    #[test]
    fn rejects_non_svg_external_entities_and_unmigrated_scripts() {
        for source in [
            "<html/>",
            "<svg/>",
            "<svg",
            "<!DOCTYPE svg [<!ENTITY x SYSTEM 'file:///tmp/x'>]><svg xmlns=\"http://www.w3.org/2000/svg\">&x;</svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><script>1</script></svg>",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }

    #[test]
    fn bsvg_roundtrip_and_idle_sleep_after_finite_entrance_and_random_actions() {
        let svg = r##"
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="-16 -16 1056 1056"
             data-bern-ready="2.3s"
             data-bern-dblclick="random(do-wink: 1.5s, do-pet: 2.6s, do-smug: 2.8s)">
          <style>
            .hair, .loop { transform-box: fill-box; animation-fill-mode: both; animation-timing-function: cubic-bezier(.3, .7, .4, 1); }
            .hair { transform-origin: 50% 50%; animation-name: pop; animation-duration: .55s; }
            .loop { transform-origin: 0 0; animation-name: swing; animation-duration: .95s; animation-delay: 1.05s; }
            @keyframes pop { 0% { transform: scale(0); } 55% { transform: scale(1.03); } 100% { transform: none; } }
            @keyframes swing { 0% { transform: rotate(38deg); opacity: 0; } 12% { opacity: 1; } 55% { transform: translateY(14px) rotate(9deg) scale(1.08, .9); } 100% { transform: none; opacity: 1; } }
            .rx { transform-box: fill-box; transform-origin: 50% 80.29%; }
            .do-wink .rx { transform-origin: 78.02% 80.29%; animation: wk 1.5s cubic-bezier(.3, .7, .4, 1) both; }
            .do-pet  .rx { animation: wk 2.6s cubic-bezier(.3, .7, .4, 1) both; }
            .do-smug .rx { transform-origin: 78.02% 42.86%; animation: wk 2.8s cubic-bezier(.3, .7, .4, 1) both; }
            @keyframes wk { 0% { transform: rotate(0); } 50% { transform: rotate(-6deg); } 100% { transform: rotate(0); } }
          </style>
          <defs><clipPath id="ec"><circle cx="512" cy="512" r="512"/></clipPath></defs>
          <g class="hair"><path d="M-512 0A512 512 0 1 0 512 0A512 512 0 1 0 -512 0Z" fill="#3E5BD8" transform="translate(512 512)"/></g>
          <g clip-path="url(#ec)"><g class="rx"><rect x="-100" y="-100" width="1224" height="1400" fill="none"/><circle cx="512" cy="600" r="200" fill="#222B4F"/></g></g>
          <g class="loop"><path d="M-64.28 -193.97L63.12 111.39L-64.28 -193.97Z" fill="none" stroke="#D340AD" stroke-width="26" transform="translate(893.28 410.97)"/></g>
          <script><![CDATA[/* ignored when data-bern-* is declared */]]></script>
        </svg>"##;

        let bytes = compile(svg).unwrap();
        assert!(bytes.starts_with(MAGIC));
        let scene = Arc::new(Scene::from_bytes(&bytes).unwrap());

        // Verify exact fill-box origin math for `.loop` -> `(829, 217)`!
        let loop_node = scene
            .data
            .nodes
            .iter()
            .find(|n| n.classes.iter().any(|c| c == "loop"))
            .unwrap();
        assert!((loop_node.fill_box.x - 829.0).abs() < 0.05);
        assert!((loop_node.fill_box.y - 217.0).abs() < 0.05);

        let mut player = Player::new(scene);
        let (step0, px0) = player.render(0.0, 64, 64, PixelFormat::RgbaPremul);
        assert!(step0.changed && step0.active);
        assert_eq!(px0.len(), 64 * 64 * 4);

        // Double-click during entrance (< 2.3s) is ignored.
        assert!(!player.pointer(PointerEvent::DoubleClick(0.5, 0.5)).changed);

        // After entrance finishes (t = 2.5s), player enters 0% CPU idle state (`active == false`)!
        let (step_idle, _) = player.render(2.5, 64, 64, PixelFormat::RgbaPremul);
        assert!(
            !step_idle.active,
            "player must sleep once finite animations end"
        );
        // Subsequent render at same state skips rasterization (`changed == false`).
        let (step_same, _) = player.render(2.6, 64, 64, PixelFormat::RgbaPremul);
        assert!(!step_same.changed && !step_same.active);

        // Double-click after 2.3s wakes player and never repeats two actions in a row.
        let mut prev_cls = String::new();
        let mut t = 3.0;
        for _ in 0..8 {
            let out = player.pointer(PointerEvent::DoubleClick(0.5, 0.5));
            assert!(out.changed && out.active);
            let cur = player.root_classes().join(",");
            assert!(!cur.is_empty() && cur != prev_cls, "repeated {cur}");
            prev_cls = cur;
            t += 0.1;
            let _ = player.render(t, 64, 64, PixelFormat::BgraStraight);
        }

        // After the longest action (2.8s) elapses, class auto-clears and player sleeps again!
        let (after, _) = player.render(t + 3.0, 64, 64, PixelFormat::RgbaStraight);
        assert!(!after.active);
        assert!(player.root_classes().is_empty());
    }

    #[test]
    fn toggle_transition_smil_and_playback_controls() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 280 280" data-bern-dblclick="toggle(alternate)">
          <style>
            .orbit { transform-origin: 140px 140px; animation: turn 4s linear infinite; }
            .ball { fill: #e94476; transition: fill .3s; }
            .alternate .ball { fill: #8065e8; }
            @keyframes turn { to { transform: rotate(360deg); } }
          </style>
          <defs><radialGradient id="g"><stop stop-color="#b1c5ff"/><stop offset="1" stop-color="#596bdc"/></radialGradient></defs>
          <g class="orbit"><circle class="ball" cx="140" cy="45" r="18"/></g>
          <circle cx="140" cy="140" r="38" fill="url(#g)"><animate attributeName="r" values="30;50;30" dur="2s" repeatCount="indefinite"/></circle>
        </svg>"##;
        let bytes = compile(svg).unwrap();
        let scene = Arc::new(Scene::from_bytes(&bytes).unwrap());
        let mut player = Player::new(scene);
        let (s0, _) = player.render(0.0, 64, 64, PixelFormat::RgbaPremul);
        assert!(s0.active);

        // Pause freezes clock and marks `active == false`.
        let paused = player.configure(Config {
            playing: false,
            ..Config::default()
        });
        assert!(!paused.active);

        // Double-click toggles `alternate` and starts 0.3s fill transition.
        let _ = player.configure(Config::default());
        assert!(player.pointer(PointerEvent::DoubleClick(0.5, 0.5)).changed);
        assert_eq!(player.root_classes(), vec!["alternate"]);
        let _ = player.render(1.0, 64, 64, PixelFormat::RgbaPremul);
        let _ = player.render(1.15, 64, 64, PixelFormat::RgbaPremul);
        assert!((player.clock() - 0.15).abs() < 1e-6);

        // Restart resets clock to 0.
        let _ = player.configure(Config {
            restart: 1.0,
            ..Config::default()
        });
        assert_eq!(player.clock(), 0.0);
    }
}
