//! Bern Theme Algorithm v5.
//!
//! The theme kernel generates semantic colors and control recipes for
//! targets whose UI framework has no complete light/dark system of its own
//! (Web, GPUI) and for anything Bern draws itself. Native targets use their
//! framework's colors and never call it.
//!
//! ```text
//! Snapshot = generate(seed, mode, contrast policy, ALGORITHM_VERSION)
//! ```
//!
//! Generation is deterministic and pure: it reads no platform state. Each
//! role starts from a target lightness in OKLCH (seed hue for brand roles,
//! a fixed neutral hue for surfaces and text, fixed hues for status), is
//! mapped into sRGB by lowering chroma, and is then moved to the nearest
//! lightness at which every pairing declared for it meets its contrast
//! level, measured on the final 8-bit colors. A role no lightness can
//! satisfy falls back to a neutral and is reported, never emitted silently.

mod color;
mod recipes;

pub use color::{Oklch, Srgb, contrast};
pub use recipes::{RECIPES, Recipe, RecipeState};

/// Version of the generation rules; a change may alter emitted colors.
pub const ALGORITHM_VERSION: u32 = 7;

/// Version of the snapshot layout.
pub const SCHEMA_VERSION: u32 = 1;

/// Bern's built-in brand seed, used when an application declares none.
pub const DEFAULT_SEED: Srgb = Srgb::new(0x2f, 0x6f, 0xe4);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }
}

/// Contrast policy, from the platform's contrast preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContrastPolicy {
    Standard,
    /// The user asked for more contrast.
    More,
}

impl ContrastPolicy {
    pub fn name(self) -> &'static str {
        match self {
            ContrastPolicy::Standard => "standard",
            ContrastPolicy::More => "more",
        }
    }

    /// Minimum ratios for text, for graphics needed to identify controls
    /// and states, and for disabled controls, which stay recognizable but
    /// are not operable.
    fn levels(self) -> Levels {
        match self {
            ContrastPolicy::Standard => Levels {
                text: 4.5,
                graphic: 3.0,
                disabled: 3.0,
            },
            ContrastPolicy::More => Levels {
                text: 7.0,
                graphic: 4.5,
                disabled: 4.5,
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Levels {
    text: f64,
    graphic: f64,
    disabled: f64,
}

/// One emitted color. `alpha` is 255 except for overlays such as `scrim`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub name: &'static str,
    pub color: Srgb,
    pub alpha: u8,
}

/// A declared foreground and background pairing and its measured ratio.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub foreground: &'static str,
    pub background: &'static str,
    pub ratio: f64,
    pub required: f64,
}

impl Check {
    pub fn passed(&self) -> bool {
        self.ratio + 1e-9 >= self.required
    }
}

/// The resolved theme for one seed, mode and contrast policy.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub schema_version: u32,
    pub algorithm_version: u32,
    pub seed: Srgb,
    pub mode: Mode,
    pub contrast: ContrastPolicy,
    /// Sorted by name.
    pub tokens: Vec<Token>,
    pub checks: Vec<Check>,
    /// Roles that fell back to a neutral pairing, with the reason.
    pub fallbacks: Vec<String>,
}

impl Snapshot {
    pub fn token(&self, name: &str) -> Option<Srgb> {
        self.tokens
            .iter()
            .find(|token| token.name == name)
            .map(|token| token.color)
    }

    /// Whether every declared pairing meets its level.
    pub fn all_checks_pass(&self) -> bool {
        self.checks.iter().all(Check::passed)
    }
}

/// Hue and chroma of neutral surfaces and text, independent of the brand.
/// The cool grey of neutral web interfaces (zinc): #09090B, #18181B and
/// #27272A in the dark, #F4F4F5 and #E4E4E7 in the light.
const NEUTRAL_HUE: f64 = 286.0;
const SURFACE_CHROMA: f64 = 0.005;
const TEXT_CHROMA: f64 = 0.013;
/// Brand roles never exceed this chroma, so a vivid seed stays legible.
const MAX_BRAND_CHROMA: f64 = 0.20;
/// How far (in OKLCH lightness) an accent fill may darken to carry white
/// text, as the platforms put white on their accent fills, before a dark
/// foreground is used instead; a light accent such as a bright green keeps
/// its color with dark text.
const WHITE_ON_SHIFT: f64 = 0.12;

/// Status hues and chroma; the brand seed never changes them.
const SUCCESS: (f64, f64) = (145.0, 0.15);
const WARNING: (f64, f64) = (70.0, 0.16);
const ERROR: (f64, f64) = (25.0, 0.20);

struct Builder {
    mode: Mode,
    levels: Levels,
    tokens: Vec<Token>,
    checks: Vec<Check>,
    fallbacks: Vec<String>,
}

impl Builder {
    fn get(&self, name: &str) -> Srgb {
        self.tokens
            .iter()
            .find(|token| token.name == name)
            .unwrap_or_else(|| panic!("token `{name}` is solved before use"))
            .color
    }

    fn put(&mut self, name: &'static str, color: Srgb) -> Srgb {
        self.tokens.push(Token {
            name,
            color,
            alpha: 255,
        });
        color
    }

    /// Lightness `light` or `dark` by mode.
    fn pick(&self, light: f64, dark: f64) -> f64 {
        match self.mode {
            Mode::Light => light,
            Mode::Dark => dark,
        }
    }

    /// A color with no pairing, such as a surface.
    fn fixed(&mut self, name: &'static str, color: Oklch) -> Srgb {
        self.put(name, color.to_srgb())
    }

    /// Solve `name` near `target` so that it meets `level` against every
    /// background, record the checks, and fall back to the neutral extreme
    /// with the best worst-case ratio when no lightness works.
    fn solve(
        &mut self,
        name: &'static str,
        target: Oklch,
        level: f64,
        backgrounds: &[&'static str],
    ) -> Srgb {
        let against: Vec<Srgb> = backgrounds.iter().map(|bg| self.get(bg)).collect();
        let color = match nearest(target, |color| {
            against.iter().all(|bg| contrast(color, *bg) >= level)
        }) {
            Some(color) => color,
            None => {
                let fallback = neutral_extreme(&against);
                self.fallbacks.push(format!(
                    "`{name}` cannot reach {level}:1 against {}; using {fallback}",
                    backgrounds.join(", ")
                ));
                fallback
            }
        };
        self.put(name, color);
        for background in backgrounds {
            self.check(name, background, level);
        }
        color
    }

    /// Solve a background `name` together with the foreground `on` that
    /// sits on it: white, as the platforms put on their accent fills, when
    /// the background reaches it within `WHITE_ON_SHIFT` of `target`'s
    /// lightness; otherwise white or a dark neutral, whichever lets the
    /// background stay closest to `target`. `level` applies to the pair,
    /// `graphic` to the background against `surfaces`.
    fn solve_pair(
        &mut self,
        name: &'static str,
        on: &'static str,
        target: Oklch,
        surfaces: &[&'static str],
        level: f64,
    ) {
        let levels = Levels {
            text: level,
            ..self.levels
        };
        let surface_colors: Vec<Srgb> = surfaces.iter().map(|s| self.get(s)).collect();
        let dark = Oklch::new(0.18, TEXT_CHROMA, NEUTRAL_HUE).to_srgb();
        let mut best: Option<(f64, Srgb, Srgb)> = None;
        for foreground in [Srgb::WHITE, dark] {
            let found = nearest(target, |color| {
                contrast(color, foreground) >= levels.text
                    && surface_colors
                        .iter()
                        .all(|surface| contrast(color, *surface) >= levels.graphic)
            });
            if let Some(color) = found {
                let distance = (color.oklch().l - target.l).abs();
                if foreground == Srgb::WHITE && distance <= WHITE_ON_SHIFT {
                    best = Some((distance, color, foreground));
                    break;
                }
                if best.is_none_or(|(d, _, _)| distance < d - 1e-9) {
                    best = Some((distance, color, foreground));
                }
            }
        }
        let (background, foreground) = match best {
            Some((_, background, foreground)) => (background, foreground),
            None => {
                // Surfaces cannot all be met: keep the pair readable.
                let foreground = Srgb::WHITE;
                let background =
                    nearest(target, |color| contrast(color, foreground) >= levels.text)
                        .unwrap_or(Srgb::BLACK);
                self.fallbacks.push(format!(
                    "`{name}` cannot reach {}:1 against {} while keeping `{on}` readable",
                    levels.graphic,
                    surfaces.join(", ")
                ));
                (background, foreground)
            }
        };
        self.put(name, background);
        self.put(on, foreground);
        self.check(on, name, levels.text);
        for surface in surfaces {
            self.check(name, surface, levels.graphic);
        }
    }

    /// A state variant of `base` moved `step` in lightness away from the
    /// foreground `on`, so the pair only gains contrast; it keeps the
    /// pair's `level`.
    fn variant(
        &mut self,
        name: &'static str,
        base: &str,
        on: &'static str,
        step: f64,
        level: f64,
    ) -> Srgb {
        let base_color = self.get(base);
        let foreground = self.get(on);
        let lch = base_color.oklch();
        let direction = if foreground.luminance() > base_color.luminance() {
            -1.0
        } else {
            1.0
        };
        let color = Oklch::new(lch.l + direction * step, lch.c, lch.h).to_srgb();
        self.put(name, color);
        self.check(on, name, level);
        color
    }

    fn check(&mut self, foreground: &'static str, background: &'static str, required: f64) {
        let ratio = contrast(self.get(foreground), self.get(background));
        self.checks.push(Check {
            foreground,
            background,
            ratio,
            required,
        });
    }
}

/// The color nearest to `target` in lightness, after gamut mapping, that
/// satisfies `accept`. Lightness moves in steps of 0.0025, upwards first at
/// equal distance; chroma is lowered in quarters when no lightness works.
fn nearest(target: Oklch, accept: impl Fn(Srgb) -> bool) -> Option<Srgb> {
    const STEPS: i32 = 400;
    for factor in [1.0, 0.75, 0.5, 0.25, 0.0] {
        let chroma = target.c * factor;
        for step in 0..=STEPS {
            for sign in [1.0, -1.0] {
                if step == 0 && sign < 0.0 {
                    continue;
                }
                let l = target.l + sign * f64::from(step) / f64::from(STEPS);
                if !(0.0..=1.0).contains(&l) {
                    continue;
                }
                let color = Oklch::new(l, chroma, target.h).to_srgb();
                if accept(color) {
                    return Some(color);
                }
            }
        }
    }
    None
}

fn neutral_extreme(against: &[Srgb]) -> Srgb {
    let worst = |color: Srgb| {
        against
            .iter()
            .map(|bg| contrast(color, *bg))
            .fold(f64::INFINITY, f64::min)
    };
    if worst(Srgb::BLACK) >= worst(Srgb::WHITE) {
        Srgb::BLACK
    } else {
        Srgb::WHITE
    }
}

/// Luminance above which the native targets put near-black instead of
/// white on an accent fill (SwiftUI, GTK, Compose and WinUI 3 alike).
pub const DARK_ON_ACCENT_ABOVE: f64 = 0.4;

/// An accent of `style!` as the native targets show it in `mode`, whose
/// controls take it as their tint and put the foreground of
/// `DARK_ON_ACCENT_ABOVE` on its fills. The chosen color is kept when it
/// stands out from every surface of the mode at the graphic level and
/// that foreground reads on it at the same level; otherwise it moves to
/// the nearest lightness of its hue that does. A bright green or a pale
/// pink darkens in light mode and stays as chosen in dark mode, carrying
/// dark text; a red or a blue that already reads is never changed.
pub fn native_accent(color: Srgb, mode: Mode) -> Srgb {
    let level = ContrastPolicy::Standard.levels().graphic;
    let neutral = generate(None, mode, ContrastPolicy::Standard);
    let surfaces: Vec<Srgb> = ["surface.canvas", "surface.panel", "surface.raised"]
        .into_iter()
        .filter_map(|name| neutral.token(name))
        .collect();
    let near_black = Srgb::new(0x11, 0x11, 0x14);
    let reads = |fill: Srgb| {
        let on = if fill.luminance() > DARK_ON_ACCENT_ABOVE {
            near_black
        } else {
            Srgb::WHITE
        };
        contrast(on, fill) >= level
            && surfaces
                .iter()
                .all(|surface| contrast(fill, *surface) >= level)
    };
    if reads(color) {
        return color;
    }
    nearest(color.oklch(), reads).unwrap_or(color)
}

/// Generate the snapshot for a brand seed (Bern's default when `None`), a
/// resolved mode and a contrast policy.
pub fn generate(seed: Option<Srgb>, mode: Mode, contrast_policy: ContrastPolicy) -> Snapshot {
    let seed = seed.unwrap_or(DEFAULT_SEED);
    let mut b = Builder {
        mode,
        levels: contrast_policy.levels(),
        tokens: Vec::new(),
        checks: Vec::new(),
        fallbacks: Vec::new(),
    };
    let levels = b.levels;
    let neutral = |l: f64, c: f64| Oklch::new(l, c, NEUTRAL_HUE);
    let brand = seed.oklch();
    let brand_chroma = brand.c.min(MAX_BRAND_CHROMA);
    let brand_at = |l: f64, c: f64| Oklch::new(l, c, brand.h);

    // Surfaces. Light: a white page and cards, a faint grey for the
    // sidebar, and white raised layers set apart by border and shadow.
    // Dark: a near-black page, the sidebar a step above it, and raised
    // layers on the page colour set apart by border and shadow too.
    b.fixed("surface.host", neutral(b.pick(0.967, 0.11), SURFACE_CHROMA));
    let canvas = b.pick(1.0, 0.141);
    b.fixed("surface.canvas", neutral(canvas, SURFACE_CHROMA));
    b.fixed(
        "surface.panel",
        neutral(b.pick(0.985, 0.169), SURFACE_CHROMA),
    );
    b.fixed("surface.raised", neutral(canvas, SURFACE_CHROMA));
    // The lightest fill under the pointer, and the next one while pressed.
    let fill = b.pick(0.967, 0.21);
    let fill_strong = b.pick(0.92, 0.274);
    b.fixed("hover.overlay", neutral(fill, SURFACE_CHROMA));
    b.fixed("pressed.overlay", neutral(fill_strong, SURFACE_CHROMA));
    b.fixed("disabled.surface", neutral(fill, SURFACE_CHROMA));
    // Decorative: card borders, the outline of standard buttons and fields,
    // and dividers between content that is already identified by its text.
    b.fixed("separator", neutral(fill_strong, SURFACE_CHROMA));
    // Standard buttons sit on the page colour and take the fills.
    b.fixed("button.standard.hover", neutral(fill, SURFACE_CHROMA));
    b.fixed(
        "button.standard.pressed",
        neutral(fill_strong, SURFACE_CHROMA),
    );

    // Text and icons must read on every surface and row state.
    const READS_ON: &[&str] = &[
        "surface.canvas",
        "surface.panel",
        "surface.raised",
        "hover.overlay",
        "pressed.overlay",
        "button.standard.hover",
        "button.standard.pressed",
    ];
    b.solve(
        "text.primary",
        neutral(b.pick(0.141, 0.985), TEXT_CHROMA),
        levels.text,
        READS_ON,
    );
    b.solve(
        "text.secondary",
        neutral(b.pick(0.552, 0.712), TEXT_CHROMA),
        levels.text,
        READS_ON,
    );
    b.solve(
        "text.placeholder",
        neutral(b.pick(0.56, 0.68), TEXT_CHROMA),
        levels.text,
        READS_ON,
    );
    b.solve(
        "icon.primary",
        neutral(b.pick(0.141, 0.985), TEXT_CHROMA),
        levels.graphic,
        READS_ON,
    );
    b.solve(
        "icon.secondary",
        neutral(b.pick(0.552, 0.712), TEXT_CHROMA),
        levels.graphic,
        READS_ON,
    );

    // Brand roles. The accent surface and its foreground are solved as a
    // pair; text in the brand color must also read on every surface.
    const SURFACES: &[&str] = &["surface.canvas", "surface.panel", "surface.raised"];
    let accent_l = brand.l.clamp(b.pick(0.45, 0.55), b.pick(0.62, 0.72));
    // The accent fill is the chosen color itself, as the platforms fill
    // their buttons with the system accent: it starts from the seed and
    // moves only as far as its title needs to read as a control's does
    // (the graphic level, 3:1 by default, as white on a system red), and
    // as far as it needs to stand out from every surface, where it also
    // colors icons as hover feedback and fills selected navigation items.
    b.solve_pair("accent", "on_accent", brand, SURFACES, levels.graphic);
    // Text in the brand colour starts from the accent itself and moves
    // only as far as reading on every surface requires, so plain buttons,
    // links and selected tabs stay as vivid as the accent allows.
    b.solve(
        "accent.text",
        brand_at(accent_l, brand_chroma),
        levels.text,
        READS_ON,
    );
    b.solve(
        "focus.indicator",
        brand_at(b.pick(0.52, 0.74), brand_chroma),
        levels.graphic,
        SURFACES,
    );
    b.variant(
        "button.prominent.hover",
        "accent",
        "on_accent",
        b.pick(0.04, 0.04),
        levels.graphic,
    );
    b.variant(
        "button.prominent.pressed",
        "accent",
        "on_accent",
        b.pick(0.08, 0.08),
        levels.graphic,
    );

    // Control graphics.
    b.solve(
        "control.border",
        neutral(b.pick(0.62, 0.52), SURFACE_CHROMA),
        levels.graphic,
        SURFACES,
    );
    b.solve(
        "control.thumb",
        neutral(b.pick(1.0, 0.98), SURFACE_CHROMA),
        levels.graphic,
        &["accent"],
    );
    b.solve(
        "control.track",
        neutral(b.pick(0.68, 0.58), SURFACE_CHROMA),
        levels.graphic,
        &[
            "surface.canvas",
            "surface.panel",
            "surface.raised",
            "control.thumb",
        ],
    );

    // Media progress: the played part is neutral, never the accent, so it
    // does not compete with the picture; the rest of the track is a faint
    // groove. The played part must stand out from both.
    b.fixed("media.track", neutral(b.pick(0.9, 0.36), SURFACE_CHROMA));
    b.solve(
        "media.played",
        neutral(b.pick(0.3, 0.9), TEXT_CHROMA),
        levels.graphic,
        &[
            "media.track",
            "surface.canvas",
            "surface.panel",
            "surface.raised",
        ],
    );

    // Status colors use framework hues, never the brand seed.
    for (name, (hue, chroma), light, dark) in [
        ("status.success", SUCCESS, 0.5, 0.78),
        ("status.warning", WARNING, 0.5, 0.8),
        ("status.error", ERROR, 0.5, 0.74),
    ] {
        b.solve(
            name,
            Oklch::new(b.pick(light, dark), chroma, hue),
            levels.text,
            READS_ON,
        );
    }
    b.solve_pair(
        "status.error.surface",
        "on_error",
        Oklch::new(b.pick(0.55, 0.62), ERROR.1, ERROR.0),
        &["surface.canvas", "surface.panel"],
        levels.text,
    );
    b.variant(
        "button.destructive.hover",
        "status.error.surface",
        "on_error",
        0.04,
        levels.text,
    );
    b.variant(
        "button.destructive.pressed",
        "status.error.surface",
        "on_error",
        0.08,
        levels.text,
    );

    // Disabled controls stay recognizable on their own surface and on the
    // surfaces disabled text appears on directly.
    b.solve(
        "disabled.foreground",
        neutral(b.pick(0.66, 0.56), TEXT_CHROMA),
        levels.disabled,
        &[
            "disabled.surface",
            "surface.canvas",
            "surface.panel",
            "surface.raised",
        ],
    );

    b.tokens.push(Token {
        name: "scrim",
        color: Srgb::BLACK,
        alpha: match mode {
            Mode::Light => 0x59,
            Mode::Dark => 0x80,
        },
    });
    b.tokens.sort_by(|x, y| x.name.cmp(y.name));
    Snapshot {
        schema_version: SCHEMA_VERSION,
        algorithm_version: ALGORITHM_VERSION,
        seed,
        mode,
        contrast: contrast_policy,
        tokens: b.tokens,
        checks: b.checks,
        fallbacks: b.fallbacks,
    }
}

#[cfg(test)]
mod tests;
