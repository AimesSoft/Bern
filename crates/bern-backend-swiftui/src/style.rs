//! Style overrides on SwiftUI: what the target shows, and the Swift that
//! describes each rule to the runtime's `BernStyle`.

use bern_ir::style::StyleState::{Disabled, Focus, Hover, Pressed, Selected};
use bern_target_api::StyleSupport;

const BUTTON_STATES: &[&str] = &[
    "background",
    "foreground",
    "border",
    "scale",
    "offset",
    "opacity",
    "shadow",
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

/// What SwiftUI shows: buttons drawn by `BernStyledButton` (or around
/// their content by `BernContentButtonStyle`), sidebar rows drawn by
/// `BernStyledRow`.
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
            "corner",
            "padding",
            "size",
            "shadow",
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
            "corner",
            "padding",
            "size",
            "shadow",
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

use bern_ir::Application;
use bern_ir::style::{ColorRef, ControlSize, Edge, StyleEffect, StyleState};
pub use bern_target_api::style::{Resolved, resolve, used};

pub(crate) fn color(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!(".role(\"{role}\")"),
        ColorRef::Fixed { light, dark } => format!(
            ".fixed(light: 0x{:02X}{:02X}{:02X}, dark: 0x{:02X}{:02X}{:02X})",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

pub fn number(value: f64) -> String {
    let text = value.to_string();
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}

pub fn motion(app: &Application, name: &str) -> String {
    let token = app.style.token(name);
    format!(
        "BernMotion(period: {}, damping: {})",
        number(token.map_or(0.45, |token| token.period)),
        number(token.map_or(1.0, |token| token.damping))
    )
}

/// A `BernLook(…)` from `effects`, with arguments in declaration order.
fn look(app: &Application, effects: &[StyleEffect]) -> String {
    let mut args: Vec<(usize, String)> = Vec::new();
    let mut motion_of = None;
    for effect in effects {
        match effect {
            StyleEffect::Background { color: c } => {
                args.push((0, format!("background: {}", color(c))))
            }
            StyleEffect::Foreground { color: c } => {
                args.push((1, format!("foreground: {}", color(c))))
            }
            StyleEffect::Border { color: c, width } => {
                args.push((2, format!("border: {}", color(c))));
                args.push((3, format!("borderWidth: {}", number(*width))));
            }
            StyleEffect::Fill { color: c } => args.push((4, format!("fill: {}", color(c)))),
            StyleEffect::Indicator {
                color: c,
                edge,
                width,
            } => {
                args.push((5, format!("indicator: {}", color(c))));
                let edge = match edge {
                    Edge::Leading => ".leading",
                    Edge::Trailing => ".trailing",
                    Edge::Top => ".top",
                    Edge::Bottom => ".bottom",
                };
                args.push((6, format!("indicatorEdge: {edge}")));
                args.push((7, format!("indicatorWidth: {}", number(*width))));
            }
            StyleEffect::Scale { x, y, motion } => {
                args.push((
                    8,
                    format!(
                        "scale: CGSize(width: {}, height: {})",
                        number(*x),
                        number(*y)
                    ),
                ));
                motion_of = Some(motion.clone());
            }
            StyleEffect::Offset { x, y, motion } => {
                args.push((
                    9,
                    format!(
                        "offset: CGSize(width: {}, height: {})",
                        number(*x),
                        number(*y)
                    ),
                ));
                motion_of = Some(motion.clone());
            }
            StyleEffect::Opacity { value, motion } => {
                args.push((10, format!("opacity: {}", number(*value))));
                motion_of = Some(motion.clone());
            }
            // `system` keeps what lies below; `none` shows nothing of its own.
            StyleEffect::Shadow { radius, y, opacity } => args.push((
                11,
                format!(
                    "shadow: BernShadow(radius: {}, y: {}, opacity: {})",
                    number(*radius),
                    number(*y),
                    number(*opacity)
                ),
            )),
            StyleEffect::System | StyleEffect::Nothing => args.push((13, "nothing: true".into())),
            StyleEffect::Corner { .. } | StyleEffect::Padding { .. } | StyleEffect::Size { .. } => {
            }
        }
    }
    if let Some(name) = motion_of {
        args.push((12, format!("motion: {}", motion(app, &name))));
    }
    args.sort_by_key(|(order, _)| *order);
    args.dedup_by_key(|(order, _)| *order);
    format!(
        "BernLook({})",
        args.into_iter()
            .map(|(_, arg)| arg)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The Swift declaration `static let <name> = BernStyle(…)`.
pub fn declaration(app: &Application, style: &Resolved) -> String {
    let mut args = vec![format!("base: {}", look(app, &style.base))];
    for effect in &style.base {
        match effect {
            StyleEffect::Corner { radius } => args.push(format!("corner: {}", number(*radius))),
            StyleEffect::Padding { length } => args.push(format!("padding: {}", number(*length))),
            _ => {}
        }
    }
    if let Some(size) = style.size() {
        args.push(format!("size: {}", control_size(size)));
    }
    for state in [
        StyleState::Hover,
        StyleState::Pressed,
        StyleState::Selected,
        StyleState::Disabled,
    ] {
        if let Some((_, effects)) = style.states.iter().find(|(existing, _)| *existing == state) {
            args.push(format!("{}: {}", state.name(), look(app, effects)));
        }
    }
    if let Some(settle) = &style.settle {
        args.push(format!("settle: {}", motion(app, settle)));
    }
    format!("static let {} = BernStyle({})", style.name, args.join(", "))
}

pub fn control_size(size: ControlSize) -> &'static str {
    match size {
        ControlSize::Small => ".small",
        ControlSize::Regular => ".regular",
        ControlSize::Large => ".large",
        ControlSize::ExtraLarge => ".extraLarge",
    }
}

/// A transition as a SwiftUI `AnyTransition` and the `BernMotion` moving it.
/// A transition as a `BernAppear`, for a staggered entrance run by hand.
fn transition_edge(edge: &Edge) -> &'static str {
    match edge {
        Edge::Leading => ".leading",
        Edge::Trailing => ".trailing",
        Edge::Top => ".top",
        Edge::Bottom => ".bottom",
    }
}

pub fn appear(app: &Application, name: &str) -> Option<String> {
    use bern_ir::style::TransitionKind;
    let def = app.style.transition(name)?;
    Some(match &def.kind {
        TransitionKind::Fade => "BernAppear.fade".to_string(),
        TransitionKind::Pop { from } => format!("BernAppear.pop({})", number(*from)),
        TransitionKind::Grow { edge } => format!("BernAppear.grow({})", transition_edge(edge)),
        TransitionKind::Slide { edge, distance } => {
            let (x, y) = match edge {
                Edge::Leading => (-distance, 0.0),
                Edge::Trailing => (*distance, 0.0),
                Edge::Top => (0.0, -distance),
                Edge::Bottom => (0.0, *distance),
            };
            format!("BernAppear.slide(x: {}, y: {})", number(x), number(y))
        }
    })
}

pub fn transition(app: &Application, name: &str) -> Option<(String, String)> {
    use bern_ir::style::TransitionKind;
    let def = app.style.transition(name)?;
    let swift = match &def.kind {
        TransitionKind::Fade => ".opacity".to_string(),
        TransitionKind::Pop { from } => {
            format!(".scale(scale: {}).combined(with: .opacity)", number(*from))
        }
        TransitionKind::Grow { edge } => format!(".bernGrow(edge: {})", transition_edge(edge)),
        TransitionKind::Slide { edge, distance } => {
            let (x, y) = match edge {
                Edge::Leading => (-distance, 0.0),
                Edge::Trailing => (*distance, 0.0),
                Edge::Top => (0.0, -distance),
                Edge::Bottom => (0.0, *distance),
            };
            format!(
                ".offset(x: {}, y: {}).combined(with: .opacity)",
                number(x),
                number(y)
            )
        }
    };
    Some((swift, motion(app, &def.motion)))
}
