//! Style overrides of one platform: the rules of `style!` that apply to a
//! control kind and variant, merged for a target to draw.

use crate::names::pascal;
use bern_ir::style::{ColorRef, ControlSize, StyleEffect, StyleRule, StyleState};
use bern_ir::{Application, Element, Prop, Srgb};
use std::borrow::Cow;

/// The accent `[theme].brand_seed` gives the native targets that take
/// their accent from the application rather than from the system's
/// settings: the seed in each mode as far as it reads there, as a single
/// color of `style!`'s `accents` is (`bern_theme::native_accent`). `None`
/// without a seed, or when `WindowSet(accent:)` chooses one at run time.
pub fn brand_accent(app: &Application) -> Option<ColorRef> {
    if app.windows.accent.is_some() {
        return None;
    }
    let seed = app.theme.brand_seed?;
    let readable = |mode| {
        let color = bern_theme::Srgb::new(seed.red, seed.green, seed.blue);
        let adjusted = bern_theme::native_accent(color, mode);
        Srgb {
            red: adjusted.r,
            green: adjusted.g,
            blue: adjusted.b,
        }
    };
    Some(ColorRef::Fixed {
        light: readable(bern_theme::Mode::Light),
        dark: readable(bern_theme::Mode::Dark),
    })
}

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

/// Opaque app-colored panels can follow the window; native material panels
/// inherit their presenting area's contrast (for example, white on video).
pub fn popover_window_theme(app: &Application, platform: &str, element: &Element) -> bool {
    match element.keyword("panel_appearance") {
        Some("window") => true,
        Some("surface") => resolve(app, platform, "PopoverButton", element.keyword("variant"))
            .is_some_and(|style| {
                style
                    .base
                    .iter()
                    .any(|effect| matches!(effect, StyleEffect::Background { .. }))
            }),
        _ => false,
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
            let suffix = variant.map(pascal).unwrap_or_default();
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
/// and each variant instances select, in components and in the content
/// written directly in a window.
pub fn used(app: &Application, platform: &str) -> Vec<Resolved> {
    let mut keys: Vec<(&str, Option<&str>)> = Vec::new();
    app.walk(&mut |node, _| {
        if let bern_ir::Node::Element(element) = node {
            let key = (element.kind.as_str(), element.keyword("variant"));
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    });
    keys.into_iter()
        .filter_map(|(kind, variant)| resolve(app, platform, kind, variant))
        .collect()
}

/// The lengths the styles set on one of `platforms` in place of those the
/// element writes: each length with a value replaces the written one, and
/// a width or height styled `fit` drops it, so the element takes its
/// content's.
fn restyle(app: &Application, platforms: &[&str], element: &mut Element) {
    let Some(style) = platforms
        .iter()
        .find_map(|platform| resolve(app, platform, &element.kind, element.keyword("variant")))
    else {
        return;
    };
    for effect in &style.base {
        let (name, value) = match effect {
            StyleEffect::Metric { metric, value } => (metric.name(), *value),
            // A row's or column's padding is a length it writes too.
            StyleEffect::Padding { length }
                if matches!(element.kind.as_str(), "Row" | "Column") =>
            {
                ("padding", Some(*length))
            }
            _ => continue,
        };
        match value {
            Some(value) => {
                element.props.insert(name.into(), Prop::Number { value });
            }
            None => {
                element.props.remove(name);
            }
        }
    }
}

/// A chip outside the web: the platform's capsule at the chip's 30 points
/// unless it says how tall.
fn chip_as_capsule(element: &mut Element) {
    if element.keyword("shape") == Some("chip") {
        element.props.insert(
            "shape".into(),
            Prop::Keyword {
                value: "capsule".into(),
            },
        );
        element
            .props
            .entry("height".into())
            .or_insert(Prop::Number { value: 30.0 });
    }
}

/// `app` as `platforms` show it: the elements whose styles set lengths
/// there take them, and outside the web a chip is the platform's capsule.
pub fn for_platforms<'a>(app: &'a Application, platforms: &[&str]) -> Cow<'a, Application> {
    let web = platforms.contains(&"browser");
    let mut owned = app.clone();
    let restyled = &mut |element: &mut Element| {
        restyle(app, platforms, element);
        if !web {
            chip_as_capsule(element);
        }
    };
    for component in &mut owned.components {
        component.body.elements_mut(restyled);
    }
    for window in &mut owned.windows.windows {
        window.content.elements_mut(restyled);
    }
    if owned == *app {
        Cow::Borrowed(app)
    } else {
        Cow::Owned(owned)
    }
}

#[cfg(test)]
mod tests {
    use super::brand_accent;
    use bern_ir::style::ColorRef;
    use bern_ir::{Expr, ExprKind, Srgb};
    use bern_protocol::{Type, Value};

    #[test]
    fn the_brand_seed_is_the_accent_each_mode_reads() {
        let mut app = crate::texts::tests::app(Vec::new());
        assert_eq!(brand_accent(&app), None);
        // A medium blue reads on both modes and stays as chosen; a pale
        // yellow darkens on light surfaces.
        app.theme.brand_seed = Some(Srgb {
            red: 0x33,
            green: 0x66,
            blue: 0xCC,
        });
        let Some(ColorRef::Fixed { light, dark }) = brand_accent(&app) else {
            panic!("no accent");
        };
        assert_eq!((light.red, light.green, light.blue), (0x33, 0x66, 0xCC));
        assert_eq!((dark.red, dark.green, dark.blue), (0x33, 0x66, 0xCC));
        app.theme.brand_seed = Some(Srgb {
            red: 0xFF,
            green: 0xE6,
            blue: 0x6D,
        });
        let Some(ColorRef::Fixed { light, .. }) = brand_accent(&app) else {
            panic!("no accent");
        };
        let sum = |c: Srgb| u32::from(c.red) + u32::from(c.green) + u32::from(c.blue);
        assert!(sum(light) < 0xFF + 0xE6 + 0x6D);
        // An accent chosen at run time wins.
        app.windows.accent = Some(Expr {
            kind: ExprKind::Literal {
                value: Value::Text("blue".into()),
            },
            ty: Type::Text,
        });
        assert_eq!(brand_accent(&app), None);
    }
}
