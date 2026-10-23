//! Motion: transitions, shared elements, reflow and the other motion
//! properties.

use super::{Generator, Out};
use crate::names::{double, member, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// Whether an element moves when it appears or disappears: its
    /// container then animates the change. Shared elements (`hero`) run
    /// their own motion and need no animated change. Animating it would
    /// also animate whatever else the change inserts, and a split view
    /// inserted in an animation on macOS can keep its columns under the
    /// title bar.
    fn moves(element: &Element) -> bool {
        ["appear", "disappear"]
            .iter()
            .any(|prop| element.keyword(prop).is_some())
    }

    /// The spring a container animates its moving children with.
    pub(super) fn children_motion(&self, nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            Node::Element(element) if Self::moves(element) => {
                let transition = element
                    .keyword("appear")
                    .or(element.keyword("disappear"))
                    .and_then(|name| crate::style::transition(self.app, name));
                Some(match transition {
                    Some((_, motion)) => motion,
                    None => crate::style::motion(self.app, "hero"),
                })
            }
            _ => None,
        })
    }

    /// What changes when children appear, disappear or reorder: their
    /// conditions and list identities, for `.animation(_:value:)`.
    fn change_key(&self, nodes: &[Node]) -> Option<String> {
        let mut parts = Vec::new();
        for node in nodes {
            match node {
                Node::If(node) => {
                    for (condition, _) in &node.branches {
                        parts.push(format!("AnyHashable({})", self.expr(condition)));
                    }
                }
                Node::Element(element) if element.kind == "ForEach" || element.kind == "List" => {
                    if let (Some(items), Some(Prop::Key { name, .. })) =
                        (element.expr("items"), element.prop("key"))
                    {
                        parts.push(format!(
                            "AnyHashable(({}).map(\\.{}))",
                            self.expr(items),
                            member(name)
                        ));
                    }
                }
                _ => {}
            }
        }
        (!parts.is_empty()).then(|| format!("[{}]", parts.join(", ")))
    }

    /// An element's transitions, shared element and reflow.
    pub(super) fn motion_modifiers(&self, out: &mut Out, element: &Element) {
        let appear = element
            .keyword("appear")
            .and_then(|name| crate::style::transition(self.app, name));
        // A scroll view draws the background of the bar above it (the
        // title bar or navigation bar): moving or fading the scroll view
        // drags that background along, and the bar flickers. A scroll view
        // moves its content instead (in its own case of `element`); a form,
        // whose rows cannot be wrapped, fades in what lies below the bar.
        // Both leave at once.
        if element.kind == "ScrollView" {
            return self.later_motion(out, element);
        }
        if element.kind == "Form"
            && let Some((_, motion)) = &appear
        {
            let delay = element.number("appear_delay").unwrap_or(0.0);
            out.line(format!(
                ".bernScrollEntrance(motion: {motion}, delay: {})",
                double(delay)
            ));
        } else if let Some(line) = self.transition_line(element, false) {
            out.line(line);
        }
        self.later_motion(out, element);
    }

    /// `.bernTransition(…)` for an element's `appear` and `disappear`;
    /// with `on_appear`, the entrance runs when the element appears rather
    /// than as an inserting transition.
    pub(super) fn transition_line(&self, element: &Element, on_appear: bool) -> Option<String> {
        let appear = element
            .keyword("appear")
            .and_then(|name| crate::style::transition(self.app, name));
        let disappear = element
            .keyword("disappear")
            .and_then(|name| crate::style::transition(self.app, name));
        if appear.is_some() || disappear.is_some() {
            let part = |transition: &Option<(String, String)>| match transition {
                Some((swift, motion)) => (format!("AnyTransition{swift}"), motion.clone()),
                None => ("nil".into(), "nil".into()),
            };
            let (insertion, insertion_motion) = part(&appear);
            let (removal, removal_motion) = part(&disappear);
            let manual = element
                .keyword("appear")
                .and_then(|name| crate::style::appear(self.app, name))
                .map(|appear| format!(", appear: {appear}"))
                .unwrap_or_default();
            // A modal's panel mounts with its layer at the window root, not
            // where a transition would see it come: it enters on appear.
            let delay = element
                .number("appear_delay")
                .or((on_appear || element.kind == "Modal").then_some(0.0))
                .map(|delay| format!(", delay: {}", double(delay)))
                .unwrap_or_default();
            return Some(format!(
                ".bernTransition(insertion: {insertion}, insertionMotion: {insertion_motion}, removal: {removal}, removalMotion: {removal_motion}{manual}{delay})"
            ));
        }
        None
    }

    /// The motion properties other than `appear` and `disappear`.
    fn later_motion(&self, out: &mut Out, element: &Element) {
        if let Some(hero) = element.keyword("hero").filter(|_| element.kind != "Image") {
            let motion = crate::style::motion(self.app, "hero");
            match element.expr("hero_key") {
                Some(key) => out.line(format!(
                    ".bernHero(\"{hero}-\\({})\", motion: {motion})",
                    self.expr(key)
                )),
                None => out.line(format!(".bernHero({}, motion: {motion})", string(hero))),
            }
        }
        if let Some(token) = element.keyword("animate") {
            let bound: Vec<String> = element
                .props
                .iter()
                // The grid animates its resolved column count and spacing.
                // Other properties, such as padding, still animate here.
                .filter(|(name, _)| {
                    element.kind != "Accordion"
                        && (element.kind != "Grid"
                            || !matches!(name.as_str(), "min_width" | "gap" | "row_gap"))
                })
                .filter_map(|(_, prop)| match prop {
                    Prop::Expr { expr } if matches!(expr.ty, Type::Int | Type::Float) => {
                        Some(format!("AnyHashable({})", self.expr(expr)))
                    }
                    _ => None,
                })
                .collect();
            if !bound.is_empty() {
                out.line(format!(
                    ".animation({}.animation, value: [{}])",
                    crate::style::motion(self.app, token),
                    bound.join(", ")
                ));
            }
        }
        if let Some(reduce) = element.expr("reduce_motion") {
            out.line(format!(
                ".transformEnvironment(\\.bernReduceMotion) {{ $0 = $0 || ({}) }}",
                self.expr(reduce)
            ));
        }
        if let Some(appearance) = element.keyword("appearance") {
            out.line(format!(".environment(\\.colorScheme, .{appearance})"));
            // Menus opened over the part take its appearance too.
            out.line(format!(
                ".background(BernAppearanceRegion(dark: {}))",
                appearance == "dark"
            ));
        }
        if let Some(tint) = element.keyword("tint") {
            out.line(format!(
                ".environment(\\.bernAccent, BernColor.role({}).color)",
                crate::names::string(tint)
            ));
            out.line(format!(
                ".tint(BernColor.role({}).color)",
                crate::names::string(tint)
            ));
        }
        if let Some(when) = element.expr("nudge") {
            out.line(format!(
                ".bernNudge({}, motion: {})",
                self.expr(when),
                crate::style::motion(self.app, "jelly")
            ));
        }
        if let Some(key) = element.expr("nudge_key") {
            out.line(format!(
                ".bernNudge(key: {}, scale: {}, motion: {})",
                self.expr(key),
                double(element.number("nudge_scale").unwrap_or(1.06)),
                crate::style::motion(self.app, "jelly")
            ));
        }
        let (scale, opacity) = (
            element.number("scroll_scale"),
            element.number("scroll_opacity"),
        );
        if scale.is_some() || opacity.is_some() {
            out.line(format!(
                ".bernScrollEffect(scale: {}, opacity: {}, anchor: UnitPoint(x: {}, y: {}))",
                double(scale.unwrap_or(1.0)),
                double(opacity.unwrap_or(1.0)),
                double(element.number("scroll_origin_x").unwrap_or(0.5)),
                double(element.number("scroll_origin_y").unwrap_or(0.5))
            ));
        }
        if element.keyword("reveal") == Some("hover") {
            out.line(format!(
                ".bernReveal({})",
                crate::style::motion(self.app, "snappy")
            ));
        }
        if let Some(seconds) = element.number("idle_area") {
            out.line(format!(".bernIdleArea({})", double(seconds)));
        }
        if element.kind != "ForEach" && element.kind != "List" && reveals(&element.children) {
            out.line(".bernHoverScope()");
        }

        if let Some(token) = element.keyword("reflow")
            && element.kind != "ForEach"
            && element.kind != "List"
            && let Some(key) = self.change_key(&element.children)
        {
            out.line(format!(
                ".animation({}.animation, value: {key})",
                crate::style::motion(self.app, token)
            ));
        }
    }
}

/// Whether `nodes` place content marked `reveal: hover` directly in their
/// container, through conditions and repetitions.
fn reveals(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) => {
            element.props.contains_key("reveal")
                || (element.kind == "ForEach" && reveals(&element.children))
        }
        Node::If(node) => {
            node.branches.iter().any(|(_, body)| reveals(body)) || reveals(&node.otherwise)
        }
        Node::IfLet(node) => reveals(&node.then) || reveals(&node.otherwise),
        Node::Match(node) => node.arms.iter().any(|arm| reveals(&arm.body)),
        Node::Component(_) => false,
    })
}
