//! Motion: where an element sits on its `Rail`, the props of
//! `F.Decorate`, and springs.

use super::{Generator, member, number, string};
use bern_ir::{Element, Prop};

impl Generator<'_> {
    /// Where an element sits on its `Rail`, for `F.Place`.
    pub(super) fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| match element.prop(name)? {
            Prop::Expr { expr } => Some(format!("() => Number({})", self.expr(expr))),
            Prop::Number { value } => Some(number(*value)),
            _ => None,
        };
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        (at.is_some() || from.is_some()).then(|| {
            Self::props(vec![
                ("at", at),
                ("from", from),
                ("to", to),
                ("motion", self.animate(element)),
            ])
        })
    }

    /// The motion and appearance props of an element, for `F.Decorate`.
    pub(super) fn decoration(&self, element: &Element) -> Option<String> {
        let mut entries: Vec<(&str, Option<String>)> = Vec::new();
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("X.{}", member(transition)))
        };
        entries.push(("appear", transition("appear")));
        entries.push(("appearDelay", element.number("appear_delay").map(number)));
        entries.push(("disappear", transition("disappear")));
        if let Some(hero) = element.keyword("hero") {
            let key = match element.expr("hero_key") {
                Some(key) => format!(
                    "() => {} + \"-\" + String({})",
                    string(hero),
                    self.expr(key)
                ),
                None => format!("() => {}", string(hero)),
            };
            entries.push(("hero", Some(key)));
            entries.push(("heroMotion", Some(self.motion("hero"))));
        }
        if element.keyword("reveal") == Some("hover") {
            entries.push(("reveal", Some("true".into())));
        }
        if let Some(seconds) = element.number("idle_area") {
            entries.push(("idleArea", Some(seconds.to_string())));
        }
        entries.push(("appearance", Self::keyword(element, "appearance")));
        if let Some(when) = element.expr("nudge") {
            entries.push(("nudge", Some(format!("() => ({})", self.expr(when)))));
            entries.push(("nudgeMotion", Some(self.motion("jelly"))));
        }
        if let Some(key) = self.value_prop(element, "nudge_key") {
            entries.push(("nudgeKey", Some(key)));
            entries.push(("nudgeMotion", Some(self.motion("jelly"))));
        }
        entries.push(("nudgeScale", element.number("nudge_scale").map(number)));
        entries.push(("reduceMotion", self.value_prop(element, "reduce_motion")));
        entries.push(("scrollScale", element.number("scroll_scale").map(number)));
        entries.push((
            "scrollOpacity",
            element.number("scroll_opacity").map(number),
        ));
        entries.push((
            "scrollOriginX",
            element.number("scroll_origin_x").map(number),
        ));
        entries.push((
            "scrollOriginY",
            element.number("scroll_origin_y").map(number),
        ));
        entries
            .iter()
            .any(|(_, value)| value.is_some())
            .then(|| Self::props(entries))
    }

    /// The spring of a motion token, `M.name`.
    pub(super) fn motion(&self, name: &str) -> String {
        format!("M.{}", member(name))
    }

    /// `animate`'s spring, for the bound lengths of an element.
    pub(super) fn animate(&self, element: &Element) -> Option<String> {
        element.keyword("animate").map(|token| self.motion(token))
    }
}
