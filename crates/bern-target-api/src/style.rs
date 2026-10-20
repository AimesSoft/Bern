//! Style overrides of one platform: the rules of `style!` that apply to a
//! control kind and variant, merged for a target to draw.

use crate::names::pascal;
use bern_ir::Application;
use bern_ir::style::{ControlSize, StyleEffect, StyleRule, StyleState};

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
