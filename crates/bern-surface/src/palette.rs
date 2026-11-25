//! Unified self-drawn color palette over [`bern_theme::Snapshot`].
//!
//! Both Web and GPUI (and any future self-drawn canvas renderer) resolve
//! colors through [`SurfacePalette`], eliminating drift between targets:
//!
//! - Translucent neutral overlays, fills, hairlines, and the default sidebar
//!   tint derive from `text.primary` with identical alpha steps so cards,
//!   hover states, and sidebars blend smoothly over blurred backdrops while
//!   preserving contrast on plain light/dark canvases.
//! - Slider thumbs use pure white pills with subtle hairline outlines and
//!   elevation shadows in both light and dark modes.
//! - Runtime palette overrides (`WindowSet(palette:)`) apply identically
//!   before derived fallbacks.

use crate::ir::{ColorValue, Rgba8};
use bern_theme::{ContrastPolicy, Mode, RECIPES, Snapshot, Srgb};
use std::collections::BTreeMap;

/// Translucent alpha channels for neutral tints derived from `text.primary`.
pub const HOVER_OVERLAY_ALPHA: u8 = 15; // ~5.9%
pub const PRESSED_OVERLAY_ALPHA: u8 = 31; // ~12.2%
pub const DISABLED_SURFACE_ALPHA: u8 = 13; // ~5.1%
pub const SEPARATOR_ALPHA: u8 = 31; // ~12.2%
pub const SIDEBAR_LIGHT_ALPHA: u8 = 9; // ~3.5%
pub const SIDEBAR_DARK_ALPHA: u8 = 12; // ~4.7%

/// Frosted glass surface tokens (`glass: true`), shared across self-drawn targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlassTokens {
    pub background: Rgba8,
    pub hover: Rgba8,
    pub active: Rgba8,
    pub border: Rgba8,
    pub shadow: Rgba8,
    pub panel_background: Rgba8,
    pub panel_shadow: Rgba8,
    pub item_hover: Rgba8,
    pub item_active: Rgba8,
}

pub const DEFAULT_GLASS_LIGHT: GlassTokens = GlassTokens {
    background: Rgba8::new(255, 255, 255, 166),
    hover: Rgba8::new(255, 255, 255, 204),
    active: Rgba8::new(238, 238, 242, 219),
    border: Rgba8::new(0, 0, 0, 26),
    shadow: Rgba8::new(0, 0, 0, 20),
    panel_background: Rgba8::new(250, 250, 252, 194),
    panel_shadow: Rgba8::new(0, 0, 0, 46),
    item_hover: Rgba8::new(0, 0, 0, 15),
    item_active: Rgba8::new(0, 0, 0, 28),
};

pub const DEFAULT_GLASS_DARK: GlassTokens = GlassTokens {
    background: Rgba8::new(20, 20, 24, 133),
    hover: Rgba8::new(36, 36, 42, 163),
    active: Rgba8::new(50, 50, 58, 184),
    border: Rgba8::new(255, 255, 255, 41),
    shadow: Rgba8::new(0, 0, 0, 71),
    panel_background: Rgba8::new(22, 22, 26, 184),
    panel_shadow: Rgba8::new(0, 0, 0, 140),
    item_hover: Rgba8::new(255, 255, 255, 31),
    item_active: Rgba8::new(255, 255, 255, 51),
};

/// Resolved self-drawn palette for one mode, contrast policy, and seed.
#[derive(Clone, Debug)]
pub struct SurfacePalette {
    snapshot: Snapshot,
    overrides: BTreeMap<String, (Rgba8, Rgba8)>,
}

impl SurfacePalette {
    pub fn from_snapshot(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            overrides: BTreeMap::new(),
        }
    }

    pub fn generate(seed: Option<Srgb>, mode: Mode, contrast: ContrastPolicy) -> Self {
        Self::from_snapshot(bern_theme::generate(seed, mode, contrast))
    }

    /// Attach runtime role overrides (`WindowSet(palette:)`: role -> `(light, dark)`).
    pub fn with_overrides(mut self, overrides: BTreeMap<String, (Rgba8, Rgba8)>) -> Self {
        self.overrides = overrides;
        self
    }

    pub fn set_overrides(&mut self, overrides: BTreeMap<String, (Rgba8, Rgba8)>) {
        self.overrides = overrides;
    }

    pub fn mode(&self) -> Mode {
        self.snapshot.mode
    }

    pub fn is_dark(&self) -> bool {
        self.snapshot.mode == Mode::Dark
    }

    pub fn contrast(&self) -> ContrastPolicy {
        self.snapshot.contrast
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn glass(&self) -> GlassTokens {
        if self.is_dark() {
            DEFAULT_GLASS_DARK
        } else {
            DEFAULT_GLASS_LIGHT
        }
    }

    /// Look up the canonical role name for a recipe slot.
    pub fn slot_role(recipe: &str, state: &str, slot: &str) -> &'static str {
        RECIPES
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
            .unwrap_or("transparent")
    }

    /// Resolve a semantic role name to an 8-bit RGBA color, returning `None` if unknown.
    pub fn try_role(&self, role: &str) -> Option<Rgba8> {
        if role == "transparent" {
            return Some(Rgba8::TRANSPARENT);
        }
        if let Some((light, dark)) = self.overrides.get(role) {
            return Some(if self.is_dark() { *dark } else { *light });
        }

        // Canonical derived aliases shared by Web and GPUI.
        let mapped = match role {
            "fill.subtle" => "hover.overlay",
            "fill.strong" => "pressed.overlay",
            "selection.background" => "accent",
            "selection.foreground" => "on_accent",
            other => other,
        };
        if let Some((light, dark)) = self.overrides.get(mapped) {
            return Some(if self.is_dark() { *dark } else { *light });
        }

        // Self-drawn slider vertical-pill tokens: solid pure white thumb with
        // subtle dark outline and soft elevation shadow in both light and dark.
        match mapped {
            "slider.thumb" => return Some(Rgba8::WHITE),
            "slider.thumb.border" => {
                return Some(if self.is_dark() {
                    Rgba8::new(0, 0, 0, 56) // ~22%
                } else {
                    Rgba8::new(0, 0, 0, 36) // ~14%
                });
            }
            "slider.thumb.shadow" => {
                return Some(if self.is_dark() {
                    Rgba8::new(0, 0, 0, 102) // ~40%
                } else {
                    Rgba8::new(0, 0, 0, 46) // ~18%
                });
            }
            "sidebar.surface" => {
                let base = self.text_primary_srgb();
                let alpha = if self.is_dark() {
                    SIDEBAR_DARK_ALPHA
                } else {
                    SIDEBAR_LIGHT_ALPHA
                };
                return Some(Rgba8::from_srgb(base, alpha));
            }
            "glass.bg" => return Some(self.glass().background),
            "glass.hover" => return Some(self.glass().hover),
            "glass.active" => return Some(self.glass().active),
            "glass.border" => return Some(self.glass().border),
            "glass.shadow" => return Some(self.glass().shadow),
            "glass.panel.bg" => return Some(self.glass().panel_background),
            "glass.panel.shadow" => return Some(self.glass().panel_shadow),
            "glass.item.hover" => return Some(self.glass().item_hover),
            "glass.item.active" => return Some(self.glass().item_active),
            _ => {}
        }

        // Translucent neutral tints over `text.primary` so cards, hover rows,
        // pressed fills, and hairlines blend over blurred backdrops while
        // keeping neutral steps on plain canvases.
        if let Some(alpha) = match mapped {
            "hover.overlay" => Some(HOVER_OVERLAY_ALPHA),
            "pressed.overlay" => Some(PRESSED_OVERLAY_ALPHA),
            "disabled.surface" => Some(DISABLED_SURFACE_ALPHA),
            "separator" => Some(SEPARATOR_ALPHA),
            _ => None,
        } {
            let base = self.text_primary_srgb();
            return Some(Rgba8::from_srgb(base, alpha));
        }

        self.snapshot
            .tokens
            .iter()
            .find(|token| token.name == mapped)
            .map(|token| Rgba8::from_srgb(token.color, token.alpha))
    }

    /// Resolve a role name, falling back to diagnostic magenta on unknown names.
    pub fn role(&self, role: &str) -> Rgba8 {
        self.try_role(role)
            .unwrap_or(Rgba8::new(0xff, 0x00, 0xff, 0xff))
    }

    /// Resolve a recipe slot directly to RGBA.
    pub fn slot(&self, recipe: &str, state: &str, slot: &str) -> Rgba8 {
        self.role(Self::slot_role(recipe, state, slot))
    }

    /// Resolve any [`ColorValue`] in the Surface IR.
    pub fn resolve(&self, value: ColorValue) -> Rgba8 {
        match value {
            ColorValue::Transparent => Rgba8::TRANSPARENT,
            ColorValue::Role(role) => self.role(role),
            ColorValue::RoleAlpha(role, alpha) => self.role(role).with_alpha_f32(alpha),
            ColorValue::Recipe {
                recipe,
                state,
                slot,
            } => self.slot(recipe, state, slot),
            ColorValue::Fixed { light, dark } => {
                if self.is_dark() {
                    dark
                } else {
                    light
                }
            }
            ColorValue::Rgba(rgba) => rgba,
        }
    }

    /// All resolved CSS-exportable surface tokens (`(name, rgba)`) in deterministic order.
    pub fn resolved_tokens(&self) -> Vec<(&'static str, Rgba8)> {
        let mut out = Vec::with_capacity(self.snapshot.tokens.len() + 18);
        for token in &self.snapshot.tokens {
            out.push((token.name, self.role(token.name)));
        }
        for extra in [
            "fill.subtle",
            "fill.strong",
            "selection.background",
            "selection.foreground",
            "sidebar.surface",
            "slider.thumb",
            "slider.thumb.border",
            "slider.thumb.shadow",
            "glass.bg",
            "glass.hover",
            "glass.active",
            "glass.border",
            "glass.shadow",
            "glass.panel.bg",
            "glass.panel.shadow",
            "glass.item.hover",
            "glass.item.active",
        ] {
            out.push((extra, self.role(extra)));
        }
        out
    }

    fn text_primary_srgb(&self) -> Srgb {
        self.snapshot
            .token("text.primary")
            .unwrap_or(if self.is_dark() {
                Srgb::WHITE
            } else {
                Srgb::BLACK
            })
    }
}
