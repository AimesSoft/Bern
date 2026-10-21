//! Where elements may stand, checked on the lowered trees of every
//! component and of the root's window set: controls inside buttons,
//! searches and dividers in toolbars, items on rails and side panels.
//! These run after lowering because a button's content is checked
//! through the components it places.

use bern_ir as ir;
use bern_target_api::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};

/// Check the lowered `components` and the `root` window set. Each check
/// visits every tree in turn, the components by name and the root last.
pub fn check(
    components: &[ir::ComponentDef],
    root: Option<&ir::Element>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let bodies: BTreeMap<&str, &ir::Node> = components
        .iter()
        .map(|component| (component.name.as_str(), &component.body))
        .collect();
    let root = root.cloned().map(ir::Node::Element);
    let trees: Vec<&ir::Node> = bodies.values().copied().chain(root.as_ref()).collect();
    check_buttons(&trees, &bodies, diagnostics);
    check_toolbar_searches(&trees, diagnostics);
    check_toolbar_dividers(&trees, diagnostics);
    check_rails(&trees, diagnostics);
    check_inspectors(&trees, diagnostics);
}

/// A button's content is what it shows: no other control inside it,
/// also through the components it places.
fn check_buttons(
    trees: &[&ir::Node],
    bodies: &BTreeMap<&str, &ir::Node>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for body in trees {
        body.walk(&mut |node| {
            let ir::Node::Element(button) = node else {
                return;
            };
            if button.kind != "Button" || button.children.is_empty() {
                return;
            }
            if let Some(control) = control_inside(&button.children, bodies) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "a `Button` cannot hold `{}`: one press would mean two things; put the `{}` beside the button, for example in a `Stack`",
                        control.kind, control.kind
                    ))
                    .at(control.source.clone()),
                );
            }
        });
    }
}

/// A toolbar's search field stands in the toolbar itself, not in its
/// conditions or repetitions: a window shows one or none.
fn check_toolbar_searches(trees: &[&ir::Node], diagnostics: &mut Vec<Diagnostic>) {
    for body in trees {
        body.walk(&mut |node| {
            let ir::Node::Element(toolbar) = node else {
                return;
            };
            if toolbar.kind != "Toolbar" {
                return;
            }
            for child in &toolbar.children {
                if matches!(child, ir::Node::Element(_)) {
                    continue;
                }
                child.walk(&mut |node| {
                    if let ir::Node::Element(search) = node
                        && search.kind == "Search"
                    {
                        diagnostics.push(
                            Diagnostic::error(
                                "a `Search` stands directly in its `Toolbar`, not inside `if`, `match` or `ForEach`",
                            )
                            .at(search.source.clone()),
                        );
                    }
                });
            }
        });
    }
}

/// A divider in a toolbar parts two groups of items: it stands directly
/// in the toolbar, with an item before and after it.
fn check_toolbar_dividers(trees: &[&ir::Node], diagnostics: &mut Vec<Diagnostic>) {
    for body in trees {
        body.walk(&mut |node| {
            let ir::Node::Element(toolbar) = node else {
                return;
            };
            if toolbar.kind != "Toolbar" {
                return;
            }
            let mut nested = Vec::new();
            for child in &toolbar.children {
                if !matches!(child, ir::Node::Element(_)) {
                    toolbar_dividers(std::slice::from_ref(child), &mut nested);
                }
            }
            for divider in nested {
                diagnostics.push(
                    Diagnostic::error(
                        "a `Divider` parts the groups of a `Toolbar` and stands directly in it, not inside `if`, `match` or `ForEach`",
                    )
                    .at(divider.source.clone()),
                );
            }
            // Search fields have their own place; they part nothing.
            let items: Vec<&ir::Node> = toolbar
                .children
                .iter()
                .filter(|child| !matches!(child, ir::Node::Element(e) if e.kind == "Search"))
                .collect();
            for (index, child) in items.iter().enumerate() {
                let ir::Node::Element(divider) = child else {
                    continue;
                };
                if divider.kind != "Divider" {
                    continue;
                }
                let is_divider = |node: Option<&&ir::Node>| {
                    matches!(node, Some(ir::Node::Element(e)) if e.kind == "Divider")
                };
                if index == 0 || index + 1 == items.len() || is_divider(items.get(index + 1)) {
                    diagnostics.push(
                        Diagnostic::error(
                            "a `Divider` in a `Toolbar` parts two groups of items; put it between items",
                        )
                        .at(divider.source.clone()),
                    );
                }
            }
        });
    }
}

/// An element sits on a `Rail` at a fraction or between two; only
/// there. Containers stagger their children.
fn check_rails(trees: &[&ir::Node], diagnostics: &mut Vec<Diagnostic>) {
    for body in trees {
        let mut items = Vec::new();
        let mut components = Vec::new();
        body.walk(&mut |node| {
            if let ir::Node::Element(rail) = node
                && rail.kind == "Rail"
            {
                rail_items(&rail.children, &mut items, &mut components);
            }
        });
        for component in components {
            diagnostics.push(
                Diagnostic::error(format!(
                    "a `Rail` places elements by their `at` or `from` and `to`; put `{}` inside one, such as a `Column` with `at:`",
                    component.component
                ))
                .at(component.source.clone()),
            );
        }
        body.walk(&mut |node| {
            let ir::Node::Element(element) = node else {
                return;
            };
            let on_rail = items.iter().any(|item| std::ptr::eq(*item, element));
            let at = element.prop("at").is_some();
            let from = element.prop("from").is_some();
            let to = element.prop("to").is_some();
            let message = if on_rail {
                match (at, from, to) {
                    (true, false, false) | (false, true, true) => None,
                    (false, false, false) => Some(
                        "an element on a `Rail` needs `at:` (a fraction of the rail) or `from:` and `to:`",
                    ),
                    (true, _, _) => Some("write either `at:` or `from:` and `to:`, not both"),
                    _ => Some("`from:` and `to:` go together"),
                }
            } else if at || from || to {
                Some("`at`, `from` and `to` place an element on the `Rail` it stands in")
            } else {
                None
            };
            if let Some(message) = message {
                diagnostics.push(Diagnostic::error(message).at(element.source.clone()));
            }
            if element.prop("stagger").is_some()
                && !bern_ir::spec::STAGGERING.contains(&element.kind.as_str())
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "`stagger` spaces out the children of a container (`{}`); `{}` has none to space",
                        bern_ir::spec::STAGGERING.join("`, `"),
                        element.kind
                    ))
                    .at(element.source.clone()),
                );
            }
        });
    }
}

/// A side panel attaches to the container it stands in: directly there,
/// one per container, not inside a condition or repetition.
fn check_inspectors(trees: &[&ir::Node], diagnostics: &mut Vec<Diagnostic>) {
    for body in trees {
        let mut misplaced = Vec::new();
        inspectors(std::slice::from_ref(*body), true, &mut misplaced);
        for (inspector, message) in misplaced {
            diagnostics.push(Diagnostic::error(message).at(inspector.source.clone()));
        }
    }
}

/// Inspectors that do not stand directly in their container, and
/// containers with more than one.
fn inspectors<'a>(
    nodes: &'a [ir::Node],
    direct: bool,
    out: &mut Vec<(&'a ir::Element, &'static str)>,
) {
    for node in nodes {
        match node {
            ir::Node::Element(element) => {
                if element.kind == "Inspector" && !direct {
                    out.push((
                        element,
                        "an `Inspector` stands directly in the container it attaches to, not inside `if`, `match` or `ForEach`; `presented` shows and hides it",
                    ));
                }
                let panels = element
                    .children
                    .iter()
                    .filter(|child| matches!(child, ir::Node::Element(e) if e.kind == "Inspector"))
                    .collect::<Vec<_>>();
                if let Some(ir::Node::Element(extra)) = panels.get(1) {
                    out.push((extra, "a container has at most one `Inspector`"));
                }
                inspectors(&element.children, element.kind != "ForEach", out);
            }
            ir::Node::If(node) => {
                for (_, body) in &node.branches {
                    inspectors(body, false, out);
                }
                inspectors(&node.otherwise, false, out);
            }
            ir::Node::IfLet(node) => {
                inspectors(&node.then, false, out);
                inspectors(&node.otherwise, false, out);
            }
            ir::Node::Match(node) => {
                for arm in &node.arms {
                    inspectors(&arm.body, false, out);
                }
            }
            ir::Node::Component(_) => {}
        }
    }
}

/// The items of a `Rail`: the elements standing in it, also inside `if`,
/// `match` and the rows of `ForEach`; components placed there directly.
fn rail_items<'a>(
    nodes: &'a [ir::Node],
    items: &mut Vec<&'a ir::Element>,
    components: &mut Vec<&'a ir::ComponentRef>,
) {
    for node in nodes {
        match node {
            ir::Node::Element(element) if element.kind == "ForEach" => {
                rail_items(&element.children, items, components);
            }
            ir::Node::Element(element) => items.push(element),
            ir::Node::If(node) => {
                for (_, body) in &node.branches {
                    rail_items(body, items, components);
                }
                rail_items(&node.otherwise, items, components);
            }
            ir::Node::IfLet(node) => {
                rail_items(&node.then, items, components);
                rail_items(&node.otherwise, items, components);
            }
            ir::Node::Match(node) => {
                for arm in &node.arms {
                    rail_items(&arm.body, items, components);
                }
            }
            ir::Node::Component(component) => components.push(component),
        }
    }
}

/// The first control in `nodes`, looking into the components they place;
/// component cycles are reported elsewhere, so each is visited once.
/// A `Video` without controls only shows its picture and takes no input,
/// as an `Image`: a button may show it, such as a player's frame that
/// pauses when pressed.
fn passive_video(element: &ir::Element) -> bool {
    element.kind == "Video" && element.keyword("controls") == Some("none")
}

fn control_inside<'a>(
    nodes: &'a [ir::Node],
    bodies: &BTreeMap<&str, &'a ir::Node>,
) -> Option<&'a ir::Element> {
    let mut pending: Vec<&'a ir::Node> = nodes.iter().collect();
    let mut seen = BTreeSet::new();
    while let Some(node) = pending.pop() {
        let mut found = None;
        node.walk(&mut |node| match node {
            ir::Node::Element(element)
                if found.is_none()
                    && ir::spec::component(&element.kind).is_some_and(|spec| spec.interactive)
                    && !passive_video(element) =>
            {
                found = Some(element);
            }
            ir::Node::Component(reference) if seen.insert(reference.component.clone()) => {
                if let Some(body) = bodies.get(reference.component.as_str()) {
                    pending.push(body);
                }
            }
            _ => {}
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Dividers among the items a toolbar shows through its conditions and
/// repetitions, not inside the items themselves.
fn toolbar_dividers<'a>(nodes: &'a [ir::Node], found: &mut Vec<&'a ir::Element>) {
    for node in nodes {
        match node {
            ir::Node::Element(element) if element.kind == "Divider" => found.push(element),
            ir::Node::Element(element) if element.kind == "ForEach" => {
                toolbar_dividers(&element.children, found);
            }
            ir::Node::Element(_) | ir::Node::Component(_) => {}
            ir::Node::If(node) => {
                for (_, body) in &node.branches {
                    toolbar_dividers(body, found);
                }
                toolbar_dividers(&node.otherwise, found);
            }
            ir::Node::IfLet(node) => {
                toolbar_dividers(&node.then, found);
                toolbar_dividers(&node.otherwise, found);
            }
            ir::Node::Match(node) => {
                for arm in &node.arms {
                    toolbar_dividers(&arm.body, found);
                }
            }
        }
    }
}
