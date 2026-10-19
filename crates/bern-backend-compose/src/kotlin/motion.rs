//! Motion: transitions, conditions whose branches animate, reflow and
//! the effects that follow scrolling.

use super::{Ctx, Generator, Out, Scope};
use crate::names::double;
use bern_ir::{Element, IfNode, Node};

impl Generator<'_> {
    /// Whether a branch of the condition holds an element that appears or
    /// disappears with a transition.
    pub(super) fn branches_move(node: &IfNode) -> bool {
        node.branches
            .iter()
            .map(|(_, body)| body)
            .chain(std::iter::once(&node.otherwise))
            .flatten()
            .any(|node| {
                matches!(node, Node::Element(element)
                    if element.keyword("appear").is_some()
                        || element.keyword("disappear").is_some()
                        || element.keyword("hero").is_some())
            })
    }

    /// `bernEnter(…)` for a transition name, after `delay` seconds.
    pub(super) fn enter(&self, name: Option<&str>, delay: Option<f64>) -> String {
        match delay {
            Some(delay) => format!(
                "bernEnter({}, {})",
                self.transition_args(name),
                double(delay)
            ),
            None => format!("bernEnter({})", self.transition_args(name)),
        }
    }

    /// The transition and its spring for a transition name, as arguments.
    pub(super) fn transition_args(&self, name: Option<&str>) -> String {
        match name.and_then(|name| crate::style::transition(self.app, name)) {
            Some((transition, motion)) => format!("{transition}, {motion}"),
            None => "null, null".into(),
        }
    }

    /// Whether an element below this one, or a component, may appear and
    /// so take a turn it should not: only direct children of a staggering
    /// container take turns.
    pub(super) fn appears_below(element: &Element) -> bool {
        let mut found = false;
        for child in &element.children {
            child.walk(&mut |node| {
                found |= match node {
                    Node::Component(_) => true,
                    Node::Element(element) => element.keyword("appear").is_some(),
                    _ => false,
                }
            });
        }
        found
    }

    /// `bernExit(…)` for a transition name.
    pub(super) fn exit(&self, name: Option<&str>) -> String {
        match name.and_then(|name| crate::style::transition(self.app, name)) {
            Some((transition, motion)) => format!("bernExit({transition}, {motion})"),
            None => "bernExit(null, null)".into(),
        }
    }

    /// A condition whose branches appear and disappear with their
    /// elements' transitions: the branch shown replaces the previous one in
    /// the same place, and shared elements pair up across the two.
    pub(super) fn animated_if(&self, out: &mut Out, node: &IfNode, ctx: &Ctx) {
        out.open("run {");
        let mut branch = String::new();
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            branch.push_str(&format!("if ({}) {index} else ", self.expr(condition)));
        }
        branch.push_str(&node.branches.len().to_string());
        out.line(format!("val bernBranch = {branch}"));
        let bodies: Vec<&Vec<Node>> = node
            .branches
            .iter()
            .map(|(_, body)| body)
            .chain(std::iter::once(&node.otherwise))
            .collect();
        // Each branch enters with its first element's appearing transition
        // and leaves with its disappearing one; shared elements alone
        // cross-fade.
        let mut enters = Vec::new();
        let mut exits = Vec::new();
        for body in &bodies {
            let first = body.iter().find_map(|node| match node {
                Node::Element(element) => Some(element),
                _ => None,
            });
            let hero = body.iter().any(
                |node| matches!(node, Node::Element(element) if element.keyword("hero").is_some()),
            );
            let appear = first.and_then(|element| element.keyword("appear"));
            let delay = first.and_then(|element| element.number("appear_delay"));
            let disappear = first.and_then(|element| element.keyword("disappear"));
            let fade = |name: Option<&str>, enter: bool| match (name, hero) {
                (None, true) => format!(
                    "{}(BernTransition.Fade, {})",
                    if enter { "bernEnter" } else { "bernExit" },
                    crate::style::motion(self.app, "hero")
                ),
                (name, _) if enter => self.enter(name, delay),
                (name, _) => self.exit(name),
            };
            enters.push(fade(appear, true));
            exits.push(fade(disappear, false));
        }
        out.line(format!("val bernEnters = listOf({})", enters.join(", ")));
        out.line(format!("val bernExits = listOf({})", exits.join(", ")));
        out.open("AnimatedContent(");
        out.line("targetState = bernBranch,");
        out.line("transitionSpec = { (bernEnters[targetState] togetherWith bernExits[initialState]).using(SizeTransform(clip = false)) },");
        out.line("label = \"branch\",");
        out.close(") { bernShown ->");
        out.indent += 1;
        let inner = Ctx {
            animated: true,
            ..ctx.clone()
        };
        out.open("CompositionLocalProvider(LocalBernVisibility provides this) {");
        out.open("when (bernShown) {");
        for (index, body) in bodies.iter().enumerate() {
            if body.is_empty() {
                continue;
            }
            out.open(format!("{index} -> {{"));
            if body.len() > 1 {
                out.open("Column {");
                self.nodes(out, body, &inner.within(Scope::Column));
                out.close("}");
            } else {
                self.nodes(out, body, &inner);
            }
            out.close("}");
        }
        out.line("else -> {}");
        out.close("}");
        out.close("}");
        out.close("}");
        out.close("}");
    }

    /// `.bernReflow(…)` for a container that declares `reflow`.
    pub(super) fn reflow(&self, element: &Element) -> String {
        element
            .keyword("reflow")
            .map(|token| format!(".bernReflow({})", crate::style::motion(self.app, token)))
            .unwrap_or_default()
    }

    /// Whether an element below follows scrolling, also inside the
    /// components it places.
    pub(super) fn scroll_effects(&self, nodes: &[Node]) -> bool {
        let mut found = false;
        let mut seen = std::collections::BTreeSet::new();
        let mut pending: Vec<&Node> = nodes.iter().collect();
        while let Some(node) = pending.pop() {
            node.walk(&mut |node| match node {
                Node::Element(element)
                    if element.prop("scroll_scale").is_some()
                        || element.prop("scroll_opacity").is_some() =>
                {
                    found = true;
                }
                Node::Component(reference) if seen.insert(reference.component.clone()) => {
                    if let Some(component) = self.app.component(&reference.component) {
                        pending.push(&component.body);
                    }
                }
                _ => {}
            });
        }
        found
    }
}
