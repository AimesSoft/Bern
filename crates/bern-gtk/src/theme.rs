//! Colors of semantic roles, and the accent and appearance chosen at run
//! time.
//!
//! Colors come from libadwaita: a role names one of its named colors (with
//! the opacity libadwaita itself uses for dimmed text and subtle fills), so
//! roles follow the system's appearance, contrast and accent. Drawings and
//! style overrides resolve roles the same way. A part with its own
//! `appearance` resolves the neutral roles from libadwaita 1.5's light or
//! dark palette instead of the window's.

use adw::prelude::*;
use std::cell::{Cell, RefCell};

/// A color of a style or drawing: a role, or a fixed pair for light and
/// dark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Role(&'static str),
    Fixed((u8, u8, u8), (u8, u8, u8)),
}

/// A role as a libadwaita named color and an opacity.
fn named(role: &str) -> (&'static str, f64) {
    match role {
        "surface.canvas" => ("window_bg_color", 1.0),
        "surface.panel" => ("card_bg_color", 1.0),
        "surface.raised" => ("popover_bg_color", 1.0),
        "text.primary" | "icon.primary" | "selection.foreground" => ("window_fg_color", 1.0),
        "text.secondary" | "icon.secondary" => ("window_fg_color", 0.55),
        "text.placeholder" | "disabled.foreground" => ("window_fg_color", 0.5),
        "accent" => ("accent_bg_color", 1.0),
        "on_accent" => ("accent_fg_color", 1.0),
        "status.success" => ("success_bg_color", 1.0),
        "status.warning" => ("warning_bg_color", 1.0),
        "status.error" => ("error_bg_color", 1.0),
        "on_error" => ("error_fg_color", 1.0),
        "control.border" | "control.track" | "fill.strong" => ("window_fg_color", 0.15),
        "control.thumb" => ("view_bg_color", 1.0),
        "focus.indicator" => ("accent_color", 0.5),
        "selection.background" => ("accent_bg_color", 0.25),
        "disabled.surface" => ("window_fg_color", 0.05),
        "fill.subtle" => ("window_fg_color", 0.07),
        _ => ("window_fg_color", 1.0),
    }
}

/// libadwaita 1.5's own values of the neutral named colors, for parts
/// with an appearance of their own: `(name, light, dark)` as RGBA.
const PALETTE: &[(&str, [f64; 4], [f64; 4])] = &[
    (
        "window_bg_color",
        [0.980, 0.980, 0.980, 1.0],
        [0.141, 0.141, 0.141, 1.0],
    ),
    (
        "window_fg_color",
        [0.0, 0.0, 0.0, 0.8],
        [1.0, 1.0, 1.0, 1.0],
    ),
    (
        "view_bg_color",
        [1.0, 1.0, 1.0, 1.0],
        [0.118, 0.118, 0.118, 1.0],
    ),
    ("view_fg_color", [0.0, 0.0, 0.0, 0.8], [1.0, 1.0, 1.0, 1.0]),
    ("card_bg_color", [1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.08]),
    ("card_fg_color", [0.0, 0.0, 0.0, 0.8], [1.0, 1.0, 1.0, 1.0]),
    (
        "popover_bg_color",
        [1.0, 1.0, 1.0, 1.0],
        [0.220, 0.220, 0.220, 1.0],
    ),
    (
        "popover_fg_color",
        [0.0, 0.0, 0.0, 0.8],
        [1.0, 1.0, 1.0, 1.0],
    ),
    (
        "headerbar_bg_color",
        [0.922, 0.922, 0.922, 1.0],
        [0.188, 0.188, 0.188, 1.0],
    ),
];

fn palette(name: &str, dark: bool) -> Option<[f64; 4]> {
    PALETTE
        .iter()
        .find(|(candidate, _, _)| *candidate == name)
        .map(|(_, light, night)| if dark { *night } else { *light })
}

fn css_rgba([r, g, b, a]: [f64; 4]) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        (r * 255.0).round(),
        (g * 255.0).round(),
        (b * 255.0).round(),
        a
    )
}

/// A color as a CSS expression for GTK's style sheets; `part` is the
/// appearance of the part it shows in, when it has one.
pub(crate) fn css(color: &Color, part: Option<bool>) -> String {
    match color {
        Color::Role(role) => {
            if let Some((light, dark)) = CUSTOM_PALETTE.with(|palette| palette.borrow().get(*role).copied()) {
                return css(&Color::Fixed(light, dark), part);
            }
            let (name, alpha) = named(role);
            match part.and_then(|dark| palette(name, dark)) {
                Some([r, g, b, a]) => css_rgba([r, g, b, a * alpha]),
                None if alpha >= 1.0 => format!("@{name}"),
                None => format!("alpha(@{name}, {alpha})"),
            }
        }
        Color::Fixed(light, dark) => {
            let (r, g, b) = if part.unwrap_or_else(is_dark) {
                *dark
            } else {
                *light
            };
            format!("rgb({r}, {g}, {b})")
        }
    }
}

/// The color text and icons take on `background`, when the background is a
/// role with a pair.
pub(crate) fn pair(background: &Color) -> Option<Color> {
    match background {
        Color::Role("accent") => Some(Color::Role("on_accent")),
        Color::Role("status.error") => Some(Color::Role("on_error")),
        _ => None,
    }
}

pub(crate) fn is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

/// The appearance of the part `widget` shows in: `Some(true)` inside a
/// dark part, `Some(false)` inside a light one.
pub(crate) fn part_of(widget: &gtk::Widget) -> Option<bool> {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget.has_css_class("bern-appearance-window") {
            return Some(is_dark());
        }
        if widget.has_css_class("bern-appearance-dark") {
            return Some(true);
        }
        if widget.has_css_class("bern-appearance-light") {
            return Some(false);
        }
        current = widget.parent();
    }
    None
}

/// A color as RGBA for drawing in `widget`.
pub(crate) fn rgba(color: &Color, widget: &gtk::Widget) -> gtk::gdk::RGBA {
    let part = part_of(widget);
    match color {
        Color::Role(role) => {
            if let Some((light, dark)) = CUSTOM_PALETTE.with(|palette| palette.borrow().get(*role).copied()) {
                return rgba(&Color::Fixed(light, dark), widget);
            }
            let (name, alpha) = named(role);
            let base = match part.and_then(|dark| palette(name, dark)) {
                Some([r, g, b, a]) => gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32),
                None => lookup(widget, name),
            };
            gtk::gdk::RGBA::new(
                base.red(),
                base.green(),
                base.blue(),
                base.alpha() * alpha as f32,
            )
        }
        Color::Fixed(light, dark) => {
            let (r, g, b) = if part.unwrap_or_else(is_dark) {
                *dark
            } else {
                *light
            };
            gtk::gdk::RGBA::new(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            )
        }
    }
}

/// A libadwaita named color as the style machinery resolves it now.
#[allow(deprecated)]
fn lookup(widget: &gtk::Widget, name: &str) -> gtk::gdk::RGBA {
    widget
        .style_context()
        .lookup_color(name)
        .or_else(|| {
            palette(name, is_dark())
                .map(|[r, g, b, a]| gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32))
        })
        .unwrap_or(gtk::gdk::RGBA::new(0.5, 0.5, 0.5, 1.0))
}

// MARK: - Theme changes

thread_local! {
    static TICK: crate::reactive::Version = crate::reactive::Version::new();
    static WATCHING: Cell<bool> = const { Cell::new(false) };
}

/// Record a dependency on the colors behind the roles: the system's
/// appearance, contrast and accent, and the application's choices.
pub(crate) fn track() {
    watch();
    TICK.with(crate::reactive::Version::track);
}

fn bump() {
    TICK.with(crate::reactive::Version::bump);
}

fn watch() {
    if WATCHING.with(|watching| watching.replace(true)) {
        return;
    }
    let manager = adw::StyleManager::default();
    manager.connect_dark_notify(|_| {
        crate::style::reload();
        reload_accent();
        // After GTK restyled with the new colors.
        glib::idle_add_local_once(bump);
    });
    manager.connect_high_contrast_notify(|_| {
        glib::idle_add_local_once(bump);
    });
}

// MARK: - Accent and appearance chosen at run time

/// A fixed color for light and for dark.
type Pair = ((u8, u8, u8), (u8, u8, u8));

/// The accents of `style!`, by enum variant: `(variant, light, dark)`.
pub type Accents = &'static [(u32, (u8, u8, u8), (u8, u8, u8))];

thread_local! {
    static CUSTOM_PALETTE: RefCell<std::collections::BTreeMap<String, Pair>> = const { RefCell::new(std::collections::BTreeMap::new()) };
    static ACCENT: RefCell<Option<Pair>> = const { RefCell::new(None) };
    static ACCENT_CSS: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// Mix `color` toward `toward` by `amount`.
fn mix((r, g, b): (u8, u8, u8), toward: f64, amount: f64) -> (u8, u8, u8) {
    let channel =
        |value: u8| (f64::from(value) + (toward * 255.0 - f64::from(value)) * amount).round() as u8;
    (channel(r), channel(g), channel(b))
}

fn reload_accent() {
    let accent = ACCENT.with(|accent| *accent.borrow());
    let mut css = match accent {
        None => String::new(),
        Some((light, dark)) => {
            let night = is_dark();
            let background = if night { dark } else { light };
            // Text in the accent color stays readable on the window: a
            // little darker on light windows, lighter on dark ones.
            let text = if night {
                mix(background, 1.0, 0.3)
            } else {
                mix(background, 0.0, 0.2)
            };
            let foreground = if luminance(background) > 0.4 {
                "rgba(0, 0, 0, 0.8)"
            } else {
                "#ffffff"
            };
            let rgb = |(r, g, b): (u8, u8, u8)| format!("rgb({r}, {g}, {b})");
            format!(
                "@define-color accent_bg_color {};\n@define-color accent_color {};\n@define-color accent_fg_color {foreground};\n",
                rgb(background),
                rgb(text)
            )
        }
    };
    CUSTOM_PALETTE.with(|palette| {
        for (role, (light, dark)) in palette.borrow().iter() {
            let (r, g, b) = if is_dark() { *dark } else { *light };
            let token = role.replace('.', "_");
            css.push_str(&format!("@define-color bern_palette_{token} rgb({r}, {g}, {b});\n"));
            let aliases: &[&str] = match role.as_str() {
                "surface.canvas" => &["window_bg_color"],
                "surface.panel" => &["card_bg_color", "view_bg_color"],
                "surface.raised" => &["popover_bg_color"],
                "surface.host" => &["headerbar_bg_color"],
                "accent" => &["accent_bg_color", "accent_color"],
                "on_accent" => &["accent_fg_color"],
                "text.primary" => &["window_fg_color", "card_fg_color", "view_fg_color", "popover_fg_color"],
                _ => &[],
            };
            for alias in aliases { css.push_str(&format!("@define-color {alias} rgb({r}, {g}, {b});\n")); }
        }
    });
    ACCENT_CSS.with(|provider| {
        let mut provider = provider.borrow_mut();
        let provider = provider.get_or_insert_with(|| {
            let provider = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                );
            }
            provider
        });
        provider.load_from_string(&css);
    });
}

/// Show the accent of `variant` from `accents`, or the system's.
pub(crate) fn set_accent(accents: Accents, variant: Option<u32>) {
    watch();
    let chosen = variant.and_then(|variant| {
        accents
            .iter()
            .find(|(candidate, _, _)| *candidate == variant)
            .map(|(_, light, dark)| (*light, *dark))
    });
    let changed = ACCENT.with(|accent| accent.replace(chosen) != chosen);
    if changed {
        reload_accent();
        glib::idle_add_local_once(bump);
    }
}

/// `light`, `dark`, or anything else for the system's appearance.
pub(crate) fn set_appearance(name: &str) {
    watch();
    // The development override stays in charge.
    if std::env::var_os("BERN_APPEARANCE").is_some() {
        return;
    }
    let scheme = match name {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    let manager = adw::StyleManager::default();
    if manager.color_scheme() != scheme {
        manager.set_color_scheme(scheme);
    }
}

// MARK: - Parts with an appearance of their own

/// Names that keep following the window: the accent the application or
/// the system chose.
const LIVE: &[&str] = &["accent_bg_color", "accent_fg_color", "accent_color"];

fn resource(path: &str) -> Option<String> {
    gio::resources_lookup_data(path, gio::ResourceLookupFlags::NONE)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// `@define-color name value;` lines.
fn definitions(css: &str, into: &mut std::collections::HashMap<String, String>) {
    for line in css.lines() {
        if let Some(rest) = line.trim().strip_prefix("@define-color ")
            && let Some((name, value)) = rest.split_once(char::is_whitespace)
        {
            into.insert(
                name.to_string(),
                value.trim().trim_end_matches(';').trim().to_string(),
            );
        }
    }
}

/// `text` with every `@name` it uses replaced by its value, but for the
/// accent, which stays live.
fn resolve(text: &str, names: &std::collections::HashMap<String, String>, depth: usize) -> String {
    let mut out = String::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c != '@' {
            out.push(c);
            continue;
        }
        let start = at + 1;
        let mut end = start;
        while let Some((index, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                end = index + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        let name = &text[start..end];
        match names.get(name) {
            Some(value) if depth < 8 && !LIVE.contains(&name) => {
                out.push_str(&resolve(value, names, depth + 1));
            }
            _ => {
                out.push('@');
                out.push_str(name);
            }
        }
    }
    out
}

/// Split a selector list at the commas outside parentheses.
fn selectors(list: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (index, c) in list.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(list[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(list[start..].trim());
    parts
}

/// libadwaita's own rules for the descendants of `scope`, with the named
/// colors of the other appearance: the part looks as the whole window would
/// in that appearance.
fn part_sheet(dark: bool) -> String {
    scoped_part_sheet(dark, if dark { ".bern-appearance-dark" } else { ".bern-appearance-light" })
}

pub(crate) fn scoped_part_sheet(dark: bool, scope: &str) -> String {
    let (Some(base), Some(defaults)) = (
        resource("/org/gnome/Adwaita/styles/base.css"),
        resource(if dark {
            "/org/gnome/Adwaita/styles/defaults-dark.css"
        } else {
            "/org/gnome/Adwaita/styles/defaults-light.css"
        }),
    ) else {
        return String::new();
    };
    let mut names = std::collections::HashMap::new();
    definitions(&base, &mut names);
    definitions(&defaults, &mut names);
    // Without comments, and without the definitions themselves.
    let mut css = String::new();
    let mut rest = base.as_str();
    while let Some(open) = rest.find("/*") {
        css.push_str(&rest[..open]);
        rest = rest[open..]
            .find("*/")
            .map_or("", |close| &rest[open + close + 2..]);
    }
    css.push_str(rest);
    let mut out = String::new();
    let window_fg = names
        .get("window_fg_color")
        .map(|value| resolve(value, &names, 0))
        .unwrap_or_else(|| "white".into());
    out.push_str(&format!("{scope} {{ color: {window_fg}; }}\n"));
    let mut text = css.as_str();
    loop {
        let text_trimmed = text.trim_start();
        if text_trimmed.is_empty() {
            break;
        }
        text = text_trimmed;
        if text.starts_with("@define-color") {
            text = text.find(';').map_or("", |end| &text[end + 1..]);
            continue;
        }
        let Some(open) = text.find('{') else { break };
        let head = text[..open].trim();
        // The block, braces balanced (keyframes nest blocks).
        let mut depth = 0;
        let mut close = None;
        for (index, c) in text[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { break };
        let body = &text[open + 1..close];
        text = &text[close + 1..];
        // Every rule, not only those naming colors: within the part the
        // rules then override one another as they do in the theme.
        if head.starts_with('@') {
            continue;
        }
        let scoped: Vec<String> = selectors(head)
            .into_iter()
            .filter(|selector| !selector.is_empty())
            .map(|selector| format!("{scope} {selector}"))
            .collect();
        out.push_str(&format!(
            "{} {{{}}}\n",
            scoped.join(", "),
            resolve(body, &names, 0)
        ));
    }
    out
}

thread_local! {
    static PARTS: RefCell<Vec<(bool, gtk::CssProvider)>> = const { RefCell::new(Vec::new()) };
}

/// Make parts with `appearance` look dark or light: once per appearance,
/// libadwaita's rules for that appearance, scoped to such parts.
pub(crate) fn use_part(dark: bool) {
    if PARTS.with(|parts| parts.borrow().iter().any(|(mode, _)| *mode == dark)) {
        return;
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&part_sheet(dark));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
        );
    }
    PARTS.with(|parts| parts.borrow_mut().push((dark, provider)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn roles_name_libadwaita_colors() {
        assert_eq!(
            css(&Color::Role("surface.canvas"), None),
            "@window_bg_color"
        );
        assert_eq!(
            css(&Color::Role("text.secondary"), None),
            "alpha(@window_fg_color, 0.55)"
        );
        assert_eq!(
            css(&Color::Role("selection.background"), None),
            "alpha(@accent_bg_color, 0.25)"
        );
        // A role the runtime does not know is the window's text color.
        assert_eq!(css(&Color::Role("brand.unknown"), None), "@window_fg_color");
    }

    #[test]
    fn parts_take_neutral_colors_from_their_own_palette() {
        let canvas = Color::Role("surface.canvas");
        assert_eq!(css(&canvas, Some(false)), "rgba(250, 250, 250, 1.000)");
        assert_eq!(css(&canvas, Some(true)), "rgba(36, 36, 36, 1.000)");
        let secondary = Color::Role("text.secondary");
        assert_eq!(css(&secondary, Some(false)), "rgba(0, 0, 0, 0.440)");
        assert_eq!(css(&secondary, Some(true)), "rgba(255, 255, 255, 0.550)");
        assert_eq!(
            css(&Color::Role("surface.panel"), Some(true)),
            "rgba(255, 255, 255, 0.080)"
        );
        // The accent keeps following the window.
        assert_eq!(css(&Color::Role("accent"), Some(true)), "@accent_bg_color");
        assert_eq!(
            css(&Color::Role("focus.indicator"), Some(false)),
            "alpha(@accent_color, 0.5)"
        );
    }

    #[test]
    fn fixed_colors_follow_the_part() {
        let color = Color::Fixed((1, 2, 3), (250, 251, 252));
        assert_eq!(css(&color, Some(false)), "rgb(1, 2, 3)");
        assert_eq!(css(&color, Some(true)), "rgb(250, 251, 252)");
    }

    #[test]
    fn accent_and_error_backgrounds_pair_with_their_text() {
        assert_eq!(pair(&Color::Role("accent")), Some(Color::Role("on_accent")));
        assert_eq!(
            pair(&Color::Role("status.error")),
            Some(Color::Role("on_error"))
        );
        assert_eq!(pair(&Color::Role("status.success")), None);
        assert_eq!(pair(&Color::Fixed((0, 0, 0), (0, 0, 0))), None);
    }

    #[test]
    fn luminance_and_mixing() {
        assert_eq!(luminance((0, 0, 0)), 0.0);
        assert!((luminance((255, 255, 255)) - 1.0).abs() < 1e-9);
        assert!((luminance((255, 0, 0)) - 0.2126).abs() < 1e-9);
        // sRGB's linear segment near black.
        assert!((luminance((0, 10, 0)) - 0.7152 * (10.0 / 255.0) / 12.92).abs() < 1e-12);
        assert_eq!(mix((0, 0, 0), 1.0, 0.5), (128, 128, 128));
        assert_eq!(mix((200, 100, 50), 0.0, 0.2), (160, 80, 40));
        assert_eq!(mix((12, 34, 56), 1.0, 0.0), (12, 34, 56));
        assert_eq!(mix((12, 34, 56), 1.0, 1.0), (255, 255, 255));
    }

    #[test]
    fn color_definitions_are_read_line_by_line() {
        let mut names = HashMap::new();
        definitions(
            "@define-color accent_bg_color #3584e4;\n  @define-color window_fg_color   rgba(0, 0, 0, 0.8) ;\nbutton { color: red; }\n@define-color lonely\n",
            &mut names,
        );
        assert_eq!(names.len(), 2);
        assert_eq!(names["accent_bg_color"], "#3584e4");
        assert_eq!(names["window_fg_color"], "rgba(0, 0, 0, 0.8)");
        // Later sheets override earlier ones, as defaults-dark.css does.
        definitions("@define-color accent_bg_color #78aeed;", &mut names);
        assert_eq!(names["accent_bg_color"], "#78aeed");
    }

    #[test]
    fn named_colors_resolve_but_for_the_accent() {
        let names: HashMap<String, String> = [
            ("a", "@b"),
            ("b", "#fff"),
            ("accent_bg_color", "#00f"),
            ("loop", "@loop"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
        assert_eq!(
            resolve(
                "color: @a; background: @accent_bg_color; border-color: @unknown;",
                &names,
                0
            ),
            "color: #fff; background: @accent_bg_color; border-color: @unknown;"
        );
        assert_eq!(resolve("mix(@b,@b,0.5)", &names, 0), "mix(#fff,#fff,0.5)");
        assert_eq!(resolve("a @ b — ü", &names, 0), "a @ b — ü");
        // A definition naming itself stops instead of recursing for ever.
        assert_eq!(resolve("@loop", &names, 0), "@loop");
    }

    #[test]
    fn selector_lists_split_outside_parentheses() {
        assert_eq!(
            selectors("button, :is(.a, .b) > label,row "),
            ["button", ":is(.a, .b) > label", "row"]
        );
        assert_eq!(selectors("window"), ["window"]);
    }
}

/// Palette values are strict hex pairs; role names cannot inject CSS.
pub(crate) fn set_palette(value: &str) {
    let Ok(input) = serde_json::from_str::<std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>>(value) else { return; };
    fn rgb(value: &str) -> Option<(u8,u8,u8)> {
        if value.len() != 7 || !value.starts_with('#') { return None; }
        let n = u32::from_str_radix(&value[1..], 16).ok()?;
        Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
    }
    let colors: std::collections::BTreeMap<_,_> = input.into_iter().filter_map(|(role, pair)| {
        if !role.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_') { return None; }
        Some((role, (rgb(pair.get("light")?)?, rgb(pair.get("dark")?)?)))
    }).collect();
    watch();
    let accent = colors.get("accent").copied();
    CUSTOM_PALETTE.with(|palette| *palette.borrow_mut() = colors);
    ACCENT.with(|current| *current.borrow_mut() = accent);
    reload_accent();
    crate::style::reload();
    glib::idle_add_local_once(bump);
}

thread_local! {
    static TINTS: RefCell<Vec<(String, gtk::CssProvider)>> = const { RefCell::new(Vec::new()) };
}

/// Scoped copies of native CSS replace only the accent roles of this subtree.
pub(crate) fn use_tint(role: &'static str, dark: bool) -> String {
    let class = format!("bern-tint-{}-{}", role.replace('.', "-"), dark);
    if TINTS.with(|tints| tints.borrow().iter().any(|(name, _)| *name == class)) { return class; }
    let provider = gtk::CssProvider::new();
    let (name, _) = named(role);
    let tint = PALETTE.iter().find(|entry| entry.0 == name).map(|entry| if dark { entry.2 } else { entry.1 });
    let color = tint.map(|c| format!("rgba({},{},{},{})", c[0]*255.0,c[1]*255.0,c[2]*255.0,c[3])).unwrap_or_else(|| format!("@{name}"));
    let foreground = if dark { "black" } else { "white" };
    let scope = if dark { ".bern-appearance-dark" } else { ".bern-appearance-light" };
    let css = part_sheet(dark).replace(scope, &format!(".{class}"))
        .replace("@accent_bg_color", &color).replace("@accent_color", &color).replace("@accent_fg_color", foreground);
    provider.load_from_string(&css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    TINTS.with(|tints| tints.borrow_mut().push((class.clone(), provider)));
    class
}
