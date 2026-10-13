//! Style overrides on Compose: what the target shows, and the Kotlin that
//! describes each rule to the runtime's `BernStyle`.

use crate::names::{double, float, string};
use bern_ir::Application;
use bern_ir::style::StyleState::{Disabled, Focus, Hover, Pressed, Selected};
use bern_ir::style::{ColorRef, ControlSize, Edge, StyleEffect, StyleState};
use bern_target_api::StyleSupport;
pub use bern_target_api::style::{Resolved, resolve, used};

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

/// What Compose shows: buttons drawn by `BernStyledButton`, sidebar items
/// by `BernStyledSidebarItem`.
pub const SUPPORT: &[StyleSupport] = &[
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

pub fn color(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!("BernColor.Role({})", string(role)),
        ColorRef::Fixed { light, dark } => format!(
            "BernColor.Fixed(0x{:02X}{:02X}{:02X}, 0x{:02X}{:02X}{:02X})",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

pub fn motion(app: &Application, name: &str) -> String {
    let token = app.style.token(name);
    format!(
        "BernMotion({}, {})",
        double(token.map_or(0.45, |token| token.period)),
        double(token.map_or(1.0, |token| token.damping))
    )
}

/// A `BernLook(…)` from `effects`.
fn look(app: &Application, effects: &[StyleEffect]) -> String {
    let mut args: Vec<String> = Vec::new();
    let mut motion_of = None;
    for effect in effects {
        match effect {
            StyleEffect::Background { color: c } => args.push(format!("background = {}", color(c))),
            StyleEffect::Foreground { color: c } => args.push(format!("foreground = {}", color(c))),
            StyleEffect::Border { color: c, width } => {
                args.push(format!("border = {}", color(c)));
                args.push(format!("borderWidth = {}", double(*width)));
            }
            StyleEffect::Fill { color: c } => args.push(format!("fill = {}", color(c))),
            StyleEffect::Indicator {
                color: c,
                edge,
                width,
            } => {
                args.push(format!("indicator = {}", color(c)));
                let edge = match edge {
                    Edge::Leading => "BernEdge.Leading",
                    Edge::Trailing => "BernEdge.Trailing",
                    Edge::Top => "BernEdge.Top",
                    Edge::Bottom => "BernEdge.Bottom",
                };
                args.push(format!("indicatorEdge = {edge}"));
                args.push(format!("indicatorWidth = {}", double(*width)));
            }
            StyleEffect::Scale { x, y, motion } => {
                args.push(format!("scaleX = {}", float(*x)));
                args.push(format!("scaleY = {}", float(*y)));
                motion_of = Some(motion.clone());
            }
            StyleEffect::Offset { x, y, motion } => {
                args.push(format!("offsetX = {}", float(*x)));
                args.push(format!("offsetY = {}", float(*y)));
                motion_of = Some(motion.clone());
            }
            StyleEffect::Opacity { value, motion } => {
                args.push(format!("opacity = {}", float(*value)));
                motion_of = Some(motion.clone());
            }
            StyleEffect::Shadow { radius, y, opacity } => {
                args.push(format!("shadowRadius = {}", double(*radius)));
                args.push(format!("shadowY = {}", double(*y)));
                args.push(format!("shadowOpacity = {}", double(*opacity)));
            }
            // `system` keeps what lies below; `none` shows nothing of its own.
            StyleEffect::System | StyleEffect::Nothing => args.push("nothing = true".into()),
            StyleEffect::Corner { .. } | StyleEffect::Padding { .. } | StyleEffect::Size { .. } => {
            }
        }
    }
    if let Some(name) = motion_of {
        args.push(format!("motion = {}", motion(app, &name)));
    }
    // A later effect of the same kind replaces an earlier one.
    let mut seen = std::collections::BTreeSet::new();
    let mut unique: Vec<String> = Vec::new();
    for arg in args.into_iter().rev() {
        let name = arg.split(" =").next().unwrap_or_default().to_string();
        if seen.insert(name) {
            unique.push(arg);
        }
    }
    unique.reverse();
    format!("BernLook({})", unique.join(", "))
}

/// The Kotlin declaration `val <name> = BernStyle(…)`.
pub fn declaration(app: &Application, style: &Resolved) -> String {
    let mut args = vec![format!("base = {}", look(app, &style.base))];
    for effect in &style.base {
        match effect {
            StyleEffect::Corner { radius } => args.push(format!("corner = {}", double(*radius))),
            StyleEffect::Padding { length } => args.push(format!("padding = {}", double(*length))),
            _ => {}
        }
    }
    if let Some(size) = style.size() {
        args.push(format!("size = {}", control_size(size)));
    }
    for state in [
        StyleState::Hover,
        StyleState::Pressed,
        StyleState::Selected,
        StyleState::Disabled,
    ] {
        if let Some((_, effects)) = style.states.iter().find(|(existing, _)| *existing == state) {
            args.push(format!("{} = {}", state.name(), look(app, effects)));
        }
    }
    if let Some(settle) = &style.settle {
        args.push(format!("settle = {}", motion(app, settle)));
    }
    format!("val {} = BernStyle({})", style.name, args.join(", "))
}

pub fn control_size(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => "BernControlSize.Small",
        ControlSize::Regular => "BernControlSize.Regular",
        ControlSize::Large => "BernControlSize.Large",
        ControlSize::ExtraLarge => "BernControlSize.ExtraLarge",
    }
}

/// A transition as a `BernTransition` and the `BernMotion` moving it.
pub fn transition(app: &Application, name: &str) -> Option<(String, String)> {
    use bern_ir::style::TransitionKind;
    let def = app.style.transition(name)?;
    let kotlin = match &def.kind {
        TransitionKind::Fade => "BernTransition.Fade".to_string(),
        TransitionKind::Pop { from } => format!("BernTransition.Pop({})", float(*from)),
        TransitionKind::Slide { edge, distance } => {
            let edge = match edge {
                Edge::Leading => "BernEdge.Leading",
                Edge::Trailing => "BernEdge.Trailing",
                Edge::Top => "BernEdge.Top",
                Edge::Bottom => "BernEdge.Bottom",
            };
            format!("BernTransition.Slide({edge}, {})", float(*distance))
        }
    };
    Some((kotlin, motion(app, &def.motion)))
}
