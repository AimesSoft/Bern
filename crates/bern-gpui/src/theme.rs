//! Colors from the Bern theme algorithm: a snapshot per mode and seed,
//! generated once. The seed is the brand color, or the accent the
//! application chooses at run time (`WindowSet(accent:)`).

use bern_theme::{ContrastPolicy, Mode, Snapshot, Srgb};
use gpui::Rgba;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The resolved colors of one mode.
pub struct Theme {
    snapshot: Snapshot,
    pub(crate) dark: bool,
}

const CLEAR: Rgba = Rgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub(crate) fn rgba(color: Srgb, alpha: u8) -> Rgba {
    Rgba {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: f32::from(alpha) / 255.0,
    }
}

impl Theme {
    /// The color of a theme role, such as `surface.canvas` or `accent`.
    /// A role the snapshot lacks is drawn in magenta, so a misspelt role
    /// shows at once.
    pub fn color(&self, role: &str) -> Rgba {
        // Roles the theme derives from others, as the Web target does.
        let role = match role {
            "transparent" => return CLEAR,
            "fill.subtle" => "hover.overlay",
            "fill.strong" => "pressed.overlay",
            "selection.background" => "accent",
            "selection.foreground" => "on_accent",
            role => role,
        };
        self.snapshot
            .tokens
            .iter()
            .find(|token| token.name == role)
            .map(|token| rgba(token.color, token.alpha))
            .unwrap_or(rgba(Srgb::new(0xff, 0x00, 0xff), 0xff))
    }

    /// The color of a recipe slot, such as the `background` of
    /// `button.prominent` at `hover`; the `rest` state's when the state
    /// does not name the slot, clear when neither does.
    pub fn slot(&self, recipe: &str, state: &str, slot: &str) -> Rgba {
        let role = bern_theme::RECIPES
            .iter()
            .find(|candidate| candidate.name == recipe)
            .and_then(|recipe| {
                let find = |state: &str| {
                    recipe.state(state).and_then(|state| {
                        state
                            .slots
                            .iter()
                            .find(|(name, _)| *name == slot)
                            .map(|(_, role)| *role)
                    })
                };
                find(state).or_else(|| find("rest"))
            })
            .unwrap_or("transparent");
        self.color(role)
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
                snapshot: bern_theme::generate(seed, mode, ContrastPolicy::Standard),
                dark,
            })
        }))
    })
}
