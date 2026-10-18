//! Motion: where an element sits on its `Rail`, the decoration its
//! motion props make, and springs.

use super::{Generator, constant, float, some, string};
use bern_ir::{Element, Node, Prop};

/// Elements whose motion props wrap what they build: everything shown in
/// a layout except what builds content later or nothing at all.
fn decorates(kind: &str) -> bool {
    !matches!(
        kind,
        "ForEach" | "List" | "Toolbar" | "Toast" | "Inspector" | "Query"
    )
}

impl Generator<'_> {
    /// A motion token's spring.
    pub(super) fn motion(&self, name: &str) -> String {
        constant("M", name)
    }

    /// `animate`'s spring, for the bound lengths of an element.
    pub(super) fn animate(&self, element: &Element) -> String {
        some(element.keyword("animate").map(|token| self.motion(token)))
    }

    /// The spring a column or row's children slide with: its own `reflow`,
    /// or that of a `ForEach` whose rows are its children.
    pub(super) fn reflow(element: &Element) -> Option<&str> {
        fn rows(nodes: &[Node]) -> Option<&str> {
            nodes.iter().find_map(|node| match node {
                Node::Element(child) if child.kind == "ForEach" => child.keyword("reflow"),
                Node::If(node) => node
                    .branches
                    .iter()
                    .find_map(|(_, body)| rows(body))
                    .or_else(|| rows(&node.otherwise)),
                Node::IfLet(node) => rows(&node.then).or_else(|| rows(&node.otherwise)),
                Node::Match(node) => node.arms.iter().find_map(|arm| rows(&arm.body)),
                _ => None,
            })
        }
        element
            .keyword("reflow")
            .or_else(|| rows(&element.children))
    }

    /// The decoration of an element's motion props, if it has any.
    pub(super) fn decoration(&self, element: &Element) -> Option<String> {
        if !decorates(&element.kind) {
            return None;
        }
        let mut entries = Vec::new();
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("&{}", constant("X", transition)))
        };
        if let Some(appear) = transition("appear") {
            entries.push(format!("appear: Some({appear})"));
        }
        if let Some(disappear) = transition("disappear") {
            entries.push(format!("disappear: Some({disappear})"));
        }
        if let Some(hero) = element.keyword("hero") {
            let key = match element.expr("hero_key") {
                Some(key) => format!(
                    "b::get(|c| V::Text(format!(\"{{}}-{{}}\", {}, rt::key_text(&{}))))",
                    string(hero),
                    self.expr(key)
                ),
                None => format!("b::get(|_| V::Text({}.into()))", string(hero)),
            };
            entries.push(format!("hero: Some(({key}, {}))", self.motion("hero")));
        }
        if element.keyword("reveal") == Some("hover") {
            entries.push("reveal: true".into());
        }
        if let Some(appearance) = element.keyword("appearance") {
            entries.push(format!("appearance: Some({})", string(appearance)));
        }
        if let Some(when) = element.expr("nudge") {
            entries.push(format!(
                "nudge: Some(({}, {}))",
                self.getter(when),
                self.motion("jelly")
            ));
        }
        if let Some(scale) = element.number("scroll_scale") {
            entries.push(format!("scroll_scale: Some({})", float(scale)));
        }
        if let Some(opacity) = element.number("scroll_opacity") {
            entries.push(format!("scroll_opacity: Some({})", float(opacity)));
        }
        (!entries.is_empty()).then(|| {
            format!(
                "b::Decor {{ {}, ..Default::default() }}",
                entries.join(", ")
            )
        })
    }

    /// Where an element sits on its `Rail`, if it says.
    pub(super) fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| match element.prop(name)? {
            Prop::Expr { expr } => Some(format!("b::Num::Bound({})", self.getter(expr))),
            Prop::Number { value } => Some(format!("b::Num::Fixed({})", float(*value))),
            _ => None,
        };
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        (at.is_some() || from.is_some() || to.is_some()).then(|| {
            format!(
                "b::PlaceProps {{ at: {}, from: {}, to: {}, motion: {} }}",
                some(at),
                some(from),
                some(to),
                self.animate(element)
            )
        })
    }
}
