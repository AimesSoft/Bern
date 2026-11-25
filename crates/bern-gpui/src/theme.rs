//! Colors from the Bern theme algorithm via `bern-surface::SurfacePalette`:
//! a snapshot per mode and seed, generated once, with optional runtime role
//! overrides from `WindowSet(palette:)`. The seed is the brand color, or the
//! accent the application chooses at run time (`WindowSet(accent:)` /
//! `palette`).

use bern_surface::{Rgba8, SurfacePalette};
use bern_theme::{ContrastPolicy, Mode, Srgb};
use gpui::Rgba;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

type Pair = ((u8, u8, u8), (u8, u8, u8));

/// The resolved colors of one mode, backed by [`SurfacePalette`] so GPUI and
/// Web resolve identical translucent overlays, separators, sidebar tints, and
/// slider pill tokens.
pub struct Theme {
    palette: SurfacePalette,
    pub(crate) dark: bool,
}

pub(crate) fn rgba(color: Srgb, alpha: u8) -> Rgba {
    rgba8(Rgba8::from_srgb(color, alpha))
}

pub(crate) fn rgba8(color: Rgba8) -> Rgba {
    let [r, g, b, a] = color.to_f32_array();
    Rgba { r, g, b, a }
}

thread_local! {
    /// Active runtime palette overrides from `WindowSet(palette:)`:
    /// JSON `{ "<role>": { "light": "#RRGGBB", "dark": "#RRGGBB" } }`.
    static PALETTE_RAW: RefCell<String> = const { RefCell::new(String::new()) };
    static PALETTE_MAP: RefCell<BTreeMap<String, Pair>> = const { RefCell::new(BTreeMap::new()) };
}

fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// Update runtime palette overrides from `WindowSet(palette:)`.
pub(crate) fn set_palette(raw: &str) {
    let unchanged = PALETTE_RAW.with(|current| *current.borrow() == raw);
    if unchanged {
        return;
    }
    PALETTE_RAW.with(|current| *current.borrow_mut() = raw.to_string());
    let mut map = BTreeMap::new();
    if let Ok(parsed) = serde_json::from_str::<BTreeMap<String, BTreeMap<String, String>>>(raw) {
        for (role, pair) in parsed {
            if !role
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
            {
                continue;
            }
            if let (Some(light), Some(dark)) = (
                pair.get("light").and_then(|s| parse_hex(s)),
                pair.get("dark").and_then(|s| parse_hex(s)),
            ) {
                map.insert(role, (light, dark));
            }
        }
    }
    PALETTE_MAP.with(|slot| *slot.borrow_mut() = map);
}

/// The custom palette's `accent` override, if any, as `(light, dark)`.
pub(crate) fn palette_accent() -> Option<Pair> {
    PALETTE_MAP.with(|slot| slot.borrow().get("accent").copied())
}

impl Theme {
    /// The color of a theme role, such as `surface.canvas`, `accent`,
    /// `sidebar.surface`, or `slider.thumb`. A role the palette lacks is drawn
    /// in diagnostic magenta, so a misspelt role shows at once.
    pub fn color(&self, role: &str) -> Rgba {
        if role == "transparent" {
            return rgba8(Rgba8::TRANSPARENT);
        }
        // Check runtime palette override first (for both exact and mapped names).
        if let Some((light, dark)) = PALETTE_MAP.with(|map| map.borrow().get(role).copied()) {
            let (r, g, b) = if self.dark { dark } else { light };
            return rgba(Srgb::new(r, g, b), 0xff);
        }
        let mapped = match role {
            "fill.subtle" => "hover.overlay",
            "fill.strong" => "pressed.overlay",
            "selection.background" => "accent",
            "selection.foreground" => "on_accent",
            role => role,
        };
        if let Some((light, dark)) = PALETTE_MAP.with(|map| map.borrow().get(mapped).copied()) {
            let (r, g, b) = if self.dark { dark } else { light };
            return rgba(Srgb::new(r, g, b), 0xff);
        }
        rgba8(self.palette.role(role))
    }

    /// Resolve a role to `(r, g, b)` bytes for `WindowSet(accent:)` role accents.
    pub fn role_rgb(&self, role: &str) -> (u8, u8, u8) {
        let c = self.color(role);
        (
            (c.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (c.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (c.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    /// The role name of a recipe slot, such as the `background` of
    /// `button.prominent` at `hover`; the `rest` state's when the state
    /// does not name the slot, `"transparent"` when neither does.
    pub(crate) fn slot_role(recipe: &str, state: &str, slot: &str) -> &'static str {
        SurfacePalette::slot_role(recipe, state, slot)
    }
}

/// A theme by its appearance and seed.
type ThemeKey = (bool, Option<(u8, u8, u8)>);

thread_local! {
    static THEMES: RefCell<HashMap<ThemeKey, Rc<Theme>>> =
        RefCell::new(HashMap::new());
}

/// The theme of a mode for a seed; none means Bern's own.
pub(crate) fn of(dark: bool, seed: Option<(u8, u8, u8)>) -> Rc<Theme> {
    THEMES.with(|themes| {
        Rc::clone(themes.borrow_mut().entry((dark, seed)).or_insert_with(|| {
            let mode = if dark { Mode::Dark } else { Mode::Light };
            let seed = seed.map(|(red, green, blue)| Srgb::new(red, green, blue));
            Rc::new(Theme {
                palette: SurfacePalette::generate(seed, mode, ContrastPolicy::Standard),
                dark,
            })
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtle_fills_and_separators_are_translucent_in_both_modes() {
        for dark in [false, true] {
            let theme = of(dark, None);
            for role in [
                "fill.subtle",
                "fill.strong",
                "hover.overlay",
                "pressed.overlay",
                "disabled.surface",
                "separator",
                "sidebar.surface",
            ] {
                let c = theme.color(role);
                assert!(
                    c.a > 0.0 && c.a < 0.2,
                    "dark={dark} role={role} should be translucent, got a={}",
                    c.a
                );
            }
            assert!(theme.color("fill.subtle").a < theme.color("fill.strong").a);
            let thumb = theme.color("slider.thumb");
            assert_eq!(thumb, rgba8(Rgba8::WHITE));
            assert!(theme.color("slider.thumb.border").a > 0.0);
            assert!(theme.color("slider.thumb.shadow").a > 0.0);
        }
    }
}
