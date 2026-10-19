//! Style overrides on GTK: what the target shows, and the Rust that
//! describes each rule to the runtime's style sheet.

use crate::rust::{color, constant, float, string};
use bern_ir::Application;
use bern_ir::style::StyleState::{Disabled, Focus, Hover, Pressed, Selected};
use bern_ir::style::{ControlSize, Edge, StyleEffect};
use bern_target_api::StyleSupport;
pub use bern_target_api::style::{Resolved, resolve};

/// The platform name rules and exemptions use for this target.
pub const PLATFORM: &str = "linux";

const BUTTON_STATES: &[&str] = &[
    "background",
    "foreground",
    "border",
    "shadow",
    "scale",
    "offset",
    "opacity",
    "system",
    "none",
];
const ROW_STATES: &[&str] = &[
    "background",
    "foreground",
    "scale",
    "offset",
    "opacity",
    "system",
    "none",
];

/// What GTK shows: a style sheet over the theme's buttons, over the
/// content of sidebar rows and on the layers of a `Stack`, transforms on
/// springs in a motion box.
pub const SUPPORT: &[StyleSupport] = &[
    // A stack's surface: its background, the color of the text and
    // symbolic icons in it, and corners that clip its layers.
    StyleSupport {
        component: "Stack",
        base: &["background", "foreground", "corner"],
        states: &[],
    },
    StyleSupport {
        component: "Button",
        base: &[
            "background",
            "foreground",
            "border",
            "shadow",
            "corner",
            "padding",
            "size",
        ],
        states: &[
            (Hover, BUTTON_STATES),
            (Pressed, BUTTON_STATES),
            (Focus, &["system", "none"]),
            (Disabled, BUTTON_STATES),
        ],
    },
    StyleSupport {
        component: "IconButton",
        base: &[
            "background",
            "foreground",
            "border",
            "shadow",
            "corner",
            "padding",
            "size",
        ],
        states: &[
            (Hover, BUTTON_STATES),
            (Pressed, BUTTON_STATES),
            (Focus, &["system", "none"]),
            (Disabled, BUTTON_STATES),
        ],
    },
    StyleSupport {
        component: "SidebarItem",
        base: &["background", "foreground", "corner", "padding"],
        states: &[
            (Hover, ROW_STATES),
            (Pressed, ROW_STATES),
            (
                Selected,
                &[
                    "fill",
                    "indicator",
                    "background",
                    "foreground",
                    "scale",
                    "offset",
                    "opacity",
                    "system",
                    "none",
                ],
            ),
        ],
    },
];

/// Every style the application's controls use on Linux.
pub fn used(app: &Application) -> Vec<Resolved> {
    bern_target_api::style::used(app, PLATFORM)
}

/// A `b::Look { … }`: what one state shows. A later effect of the same
/// kind replaces an earlier one.
fn look(app: &Application, effects: &[StyleEffect]) -> String {
    let mut entries: Vec<(&str, String)> = Vec::new();
    let mut set = |name: &'static str, value: String| {
        entries.retain(|(existing, _)| *existing != name);
        entries.push((name, value));
    };
    let motion = |name: &str| {
        if app.style.token(name).is_some()
            || bern_ir::style::BUILT_IN_MOTION
                .iter()
                .any(|(builtin, _, _)| *builtin == name)
        {
            format!("Some({})", constant("M", name))
        } else {
            "None".into()
        }
    };
    for effect in effects {
        match effect {
            StyleEffect::Background { color: c } => {
                set("background", format!("Some({})", color(c)))
            }
            StyleEffect::Foreground { color: c } => {
                set("foreground", format!("Some({})", color(c)))
            }
            StyleEffect::Border { color: c, width } => {
                set("border", format!("Some(({}, {}))", color(c), float(*width)))
            }
            StyleEffect::Fill { color: c } => set("fill", format!("Some({})", color(c))),
            StyleEffect::Indicator {
                color: c,
                edge,
                width,
            } => {
                let edge = match edge {
                    Edge::Leading => "Leading",
                    Edge::Trailing => "Trailing",
                    Edge::Top => "Top",
                    Edge::Bottom => "Bottom",
                };
                set(
                    "indicator",
                    format!("Some(({}, b::Edge::{edge}, {}))", color(c), float(*width)),
                );
            }
            StyleEffect::Scale { x, y, motion: m } => {
                set("scale", format!("Some(({}, {}))", float(*x), float(*y)));
                set("motion", motion(m));
            }
            StyleEffect::Offset { x, y, motion: m } => {
                set("offset", format!("Some(({}, {}))", float(*x), float(*y)));
                set("motion", motion(m));
            }
            StyleEffect::Opacity { value, motion: m } => {
                set("opacity", format!("Some({})", float(*value)));
                set("motion", motion(m));
            }
            StyleEffect::Shadow { radius, y, opacity } => set(
                "shadow",
                format!(
                    "Some(({}, {}, {}))",
                    float(*radius),
                    float(*y),
                    float(*opacity)
                ),
            ),
            StyleEffect::System => set("system", "true".into()),
            StyleEffect::Nothing => set("quiet", "true".into()),
            StyleEffect::Corner { .. } | StyleEffect::Padding { .. } | StyleEffect::Size { .. } => {
            }
        }
    }
    let body: Vec<String> = entries
        .into_iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    if body.is_empty() {
        "b::Look::NONE".into()
    } else {
        format!("b::Look {{ {}, ..b::Look::NONE }}", body.join(", "))
    }
}

/// The `b::StyleDef { … }` of a resolved style.
pub fn declaration(app: &Application, style: &Resolved) -> String {
    let mut corner = None;
    let mut padding = None;
    for effect in &style.base {
        match effect {
            StyleEffect::Corner { radius } => corner = Some(float(*radius)),
            StyleEffect::Padding { length } => padding = Some(float(*length)),
            _ => {}
        }
    }
    let size = style.size().map(|size| match size {
        ControlSize::Small => "small",
        ControlSize::Regular => "regular",
        ControlSize::Large => "large",
        ControlSize::ExtraLarge => "extra_large",
    });
    let state = |wanted| {
        style
            .states
            .iter()
            .find(|(existing, _)| *existing == wanted)
            .map(|(_, effects)| format!("Some({})", look(app, effects)))
            .unwrap_or_else(|| "None".into())
    };
    let settle = style
        .settle
        .as_ref()
        .map(|token| format!("Some({})", constant("M", token)))
        .unwrap_or_else(|| "None".into());
    format!(
        "b::StyleDef {{ name: {}, base: {}, corner: {}, padding: {}, size: {}, draws: {}, hover: {}, pressed: {}, selected: {}, focus: {}, disabled: {}, settle: {settle} }}",
        string(&style.name),
        look(app, &style.base),
        corner
            .map(|value| format!("Some({value})"))
            .unwrap_or_else(|| "None".into()),
        padding
            .map(|value| format!("Some({value})"))
            .unwrap_or_else(|| "None".into()),
        size.map(|size| format!("Some({})", string(size)))
            .unwrap_or_else(|| "None".into()),
        style.draws_look(),
        state(Hover),
        state(Pressed),
        state(Selected),
        state(Focus),
        state(Disabled),
    )
}
