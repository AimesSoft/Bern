//! Style overrides on the Web: what the target shows, and the JavaScript
//! that describes each rule to the runtime's `applyStyle`.

use crate::js::{color_ref, string};
use bern_ir::Application;
use bern_ir::style::StyleState::{Disabled, Focus, Hover, Pressed, Selected};
use bern_ir::style::{ControlSize, Edge, StyleEffect, StyleState};
use bern_target_api::StyleSupport;
pub use bern_target_api::style::{Resolved, resolve, used};

/// The platform name rules and exemptions use for the Web.
pub const PLATFORM: &str = "browser";

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

/// What the Web shows: Bern's own buttons and sidebar rows, drawn with
/// CSS, their transforms moved by springs.
pub const SUPPORT: &[StyleSupport] = &[
    StyleSupport {
        component: "Stack",
        base: &["background", "foreground", "corner"],
        states: &[],
    },
    // A form's background is its rows'.
    StyleSupport {
        component: "Form",
        base: &["background"],
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

fn motion(name: &str) -> String {
    format!("M.{}", crate::js::member(name))
}

/// A look `{ … }`: the colors and transforms one state shows. A later
/// effect of the same kind replaces an earlier one.
fn look(effects: &[StyleEffect]) -> String {
    let mut entries: Vec<(&str, String)> = Vec::new();
    let mut set = |name: &'static str, value: String| {
        entries.retain(|(existing, _)| *existing != name);
        entries.push((name, value));
    };
    for effect in effects {
        match effect {
            StyleEffect::Background { color } => set("background", color_ref(color)),
            StyleEffect::Foreground { color } => set("foreground", color_ref(color)),
            StyleEffect::Border { color, width } => {
                set("border", color_ref(color));
                set("borderWidth", crate::js::number(*width));
            }
            StyleEffect::Fill { color } => set("fill", color_ref(color)),
            StyleEffect::Indicator { color, edge, width } => {
                set("indicator", color_ref(color));
                let edge = match edge {
                    Edge::Leading => "leading",
                    Edge::Trailing => "trailing",
                    Edge::Top => "top",
                    Edge::Bottom => "bottom",
                };
                set("indicatorEdge", string(edge));
                set("indicatorWidth", crate::js::number(*width));
            }
            StyleEffect::Scale { x, y, motion: m } => {
                set("scaleX", crate::js::number(*x));
                set("scaleY", crate::js::number(*y));
                set("motion", motion(m));
            }
            StyleEffect::Offset { x, y, motion: m } => {
                set("offsetX", crate::js::number(*x));
                set("offsetY", crate::js::number(*y));
                set("motion", motion(m));
            }
            StyleEffect::Opacity { value, motion: m } => {
                set("opacity", crate::js::number(*value));
                set("motion", motion(m));
            }
            StyleEffect::Shadow { radius, y, opacity } => set(
                "shadow",
                format!(
                    "[{}, {}, {}]",
                    crate::js::number(*radius),
                    crate::js::number(*y),
                    crate::js::number(*opacity)
                ),
            ),
            // `system` keeps the browser look below; `none` hides it.
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
    format!("{{ {} }}", body.join(", "))
}

fn control_size(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => "small",
        ControlSize::Regular => "regular",
        ControlSize::Large => "large",
        ControlSize::ExtraLarge => "extra_large",
    }
}

/// The JavaScript entry `name: { … }` of a resolved style.
pub fn declaration(style: &Resolved) -> String {
    let mut parts = vec![format!("base: {}", look(&style.base))];
    for effect in &style.base {
        match effect {
            StyleEffect::Corner { radius } => {
                parts.push(format!("corner: {}", crate::js::number(*radius)))
            }
            StyleEffect::Padding { length } => {
                parts.push(format!("padding: {}", crate::js::number(*length)))
            }
            _ => {}
        }
    }
    if let Some(size) = style.size() {
        parts.push(format!("size: {}", string(control_size(size))));
    }
    if style.draws_look() {
        parts.push("draws: true".into());
    }
    for state in [Hover, Pressed, Selected, Disabled, Focus] {
        if let Some((_, effects)) = style.states.iter().find(|(existing, _)| *existing == state) {
            parts.push(format!("{}: {}", state_name(state), look(effects)));
        }
    }
    if let Some(settle) = &style.settle {
        parts.push(format!("settle: {}", motion(settle)));
    }
    format!("{}: {{ {} }}", style.name, parts.join(", "))
}

fn state_name(state: StyleState) -> &'static str {
    match state {
        Hover => "hover",
        Pressed => "pressed",
        Selected => "selected",
        Disabled => "disabled",
        Focus => "focus",
    }
}

/// `S.name` for the style an element of `kind` (and `variant`) uses.
pub fn reference(app: &Application, kind: &str, variant: Option<&str>) -> Option<String> {
    resolve(app, PLATFORM, kind, variant).map(|style| format!("S.{}", style.name))
}
