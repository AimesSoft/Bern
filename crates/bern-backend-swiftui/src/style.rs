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

/// What SwiftUI shows: buttons drawn by `BernStyledButton`, sidebar rows
/// drawn by `BernStyledRow`.
pub const SUPPORT: &[StyleSupport] = &[
    StyleSupport {
        component: "Button",
        base: &[
            "background",
            "foreground",
            "border",
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

use bern_ir::Application;
use bern_ir::style::{ColorRef, ControlSize, Edge, StyleEffect, StyleRule, StyleState};

/// A control kind and variant's style on one platform: the rules for the
/// kind, then those for the variant, merged in declaration order.
pub struct Resolved {
    /// Name in the generated `BernStyles`, such as `buttonHero`.
    pub name: String,
    pub base: Vec<StyleEffect>,
    pub states: Vec<(StyleState, Vec<StyleEffect>)>,
    pub settle: Option<String>,
}

impl Resolved {
    /// Whether the style draws the control instead of keeping its look.
    pub fn draws_look(&self) -> bool {
        let look = |effect: &StyleEffect| {
            matches!(
                effect,
                StyleEffect::Background { .. }
                    | StyleEffect::Foreground { .. }
                    | StyleEffect::Border { .. }
                    | StyleEffect::Corner { .. }
                    | StyleEffect::Padding { .. }
            )
        };
        self.base.iter().any(look)
            || self
                .states
                .iter()
                .filter(|(state, _)| *state != StyleState::Selected)
                .any(|(_, effects)| effects.iter().any(look))
    }

    pub fn size(&self) -> Option<ControlSize> {
        self.base.iter().find_map(|effect| match effect {
            StyleEffect::Size { size } => Some(*size),
            _ => None,
        })
    }
}

fn applies(rule: &StyleRule, platform: &str) -> bool {
    !rule.exempt.iter().any(|exempt| exempt == platform)
}

/// The style of `component` (and `variant`) on `platform`, if `style!`
/// declares one.
pub fn resolve(
    app: &Application,
    platform: &str,
    component: &str,
    variant: Option<&str>,
) -> Option<Resolved> {
    let rules: Vec<&StyleRule> = app
        .style
        .rules_for(platform)
        .filter(|rule| rule.component == component && applies(rule, platform))
        .filter(|rule| rule.variant.is_none() || rule.variant.as_deref() == variant)
        .collect();
    if rules.is_empty() {
        return None;
    }
    let mut resolved = Resolved {
        // `SidebarItem` becomes `sidebarItem`, `Button.hero` `buttonHero`.
        name: {
            let mut chars = component.chars();
            let first: String = chars
                .next()
                .map(|c| c.to_lowercase().collect())
                .unwrap_or_default();
            let suffix = variant.map(bern_syntax::pascal_case).unwrap_or_default();
            format!("{first}{}{suffix}", chars.as_str())
        },
        base: Vec::new(),
        states: Vec::new(),
        settle: None,
    };
    // Rules for the kind first, then for the variant.
    let ordered = rules
        .iter()
        .filter(|rule| rule.variant.is_none())
        .chain(rules.iter().filter(|rule| rule.variant.is_some()));
    for rule in ordered {
        for effect in &rule.base {
            resolved
                .base
                .retain(|existing| existing.name() != effect.name());
            resolved.base.push(effect.clone());
        }
        for state in &rule.states {
            match resolved
                .states
                .iter_mut()
                .find(|(existing, _)| *existing == state.state)
            {
                Some((_, effects)) => *effects = state.effects.clone(),
                None => resolved.states.push((state.state, state.effects.clone())),
            }
        }
        if rule.settle.is_some() {
            resolved.settle = rule.settle.clone();
        }
    }
    Some(resolved)
}

/// Every style the application's controls use on `platform`: the kinds,
/// and each variant instances select.
pub fn used(app: &Application, platform: &str) -> Vec<Resolved> {
    let mut keys: Vec<(String, Option<String>)> = Vec::new();
    for component in &app.components {
        component.body.walk(&mut |node| {
            if let bern_ir::Node::Element(element) = node {
                let key = (
                    element.kind.clone(),
                    element.keyword("variant").map(str::to_string),
                );
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        });
    }
    keys.iter()
        .filter_map(|(kind, variant)| resolve(app, platform, kind, variant.as_deref()))
        .collect()
}

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
            StyleEffect::System | StyleEffect::Nothing => args.push((12, "nothing: true".into())),
            StyleEffect::Corner { .. } | StyleEffect::Padding { .. } | StyleEffect::Size { .. } => {
            }
        }
    }
    if let Some(name) = motion_of {
        args.push((11, format!("motion: {}", motion(app, &name))));
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
pub fn transition(app: &Application, name: &str) -> Option<(String, String)> {
    use bern_ir::style::TransitionKind;
    let def = app.style.transition(name)?;
    let swift = match &def.kind {
        TransitionKind::Fade => ".opacity".to_string(),
        TransitionKind::Pop { from } => {
            format!(".scale(scale: {}).combined(with: .opacity)", number(*from))
        }
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
