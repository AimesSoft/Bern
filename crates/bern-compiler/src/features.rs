//! Feature contracts: lowering `#[bern::features]` enums and proving which
//! features each contract scope reaches, per layout size.
//!
//! The proof is structural. An entry inside `Layout(size: s)` covers only
//! `s`; an entry anywhere else covers every size. An entry under `if`,
//! `if let` or `match` is conditional, unless every branch of that
//! condition reaches the same feature, the one case the framework proves.

use crate::check::location;
use bern_ir::{self as ir, Coverage, LayoutSize, MediaOp, Node, SizeCoverage};
use bern_protocol::ProtocolDescriptor;
use bern_syntax as ast;
use bern_target_api::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};

/// A `#[bern::features]` enum and the file it was declared in.
pub struct Declared<'a> {
    pub file: &'a str,
    pub item: &'a ast::FeaturesItem,
}

/// Lower a features enum: command names resolve to catalog ids.
pub fn lower(
    declared: &Declared<'_>,
    component: String,
    protocol: &ProtocolDescriptor,
    diagnostics: &mut Vec<Diagnostic>,
) -> ir::FeatureContract {
    let item = declared.item;
    let mut features = Vec::new();
    for feature in &item.features {
        let mut commands = Vec::new();
        for command in &feature.commands {
            let id = match command.name.split_once('.') {
                Some(("media", op)) => MediaOp::ALL
                    .into_iter()
                    .find(|candidate| candidate.method() == op)
                    .map(|op| op.command().to_string()),
                Some(_) => None,
                None => protocol
                    .commands
                    .iter()
                    .find(|spec| spec.name == command.name)
                    .map(|spec| spec.id.clone()),
            };
            match id {
                Some(id) => commands.push(id),
                None => diagnostics.push(
                    Diagnostic::error(format!(
                        "`{}` is neither a command of the protocol nor a standard media command (`media.play`, …)",
                        command.name
                    ))
                    .at(location(declared.file, command.span)),
                ),
            }
        }
        let mut layouts = Vec::new();
        for size in &feature.layouts {
            match LayoutSize::parse(&size.to_string())
                .filter(|size| LayoutSize::REQUIRED.contains(size))
            {
                Some(size) => layouts.push(size),
                None => diagnostics.push(
                    Diagnostic::error(format!(
                        "unknown layout size `{size}`; sizes are compact and expanded"
                    ))
                    .at(location(declared.file, size.span())),
                ),
            }
        }
        if layouts.is_empty() {
            layouts = LayoutSize::REQUIRED.to_vec();
        }
        features.push(ir::FeatureDef {
            name: feature.name.to_string(),
            doc: feature.doc.clone(),
            commands,
            layouts,
            guaranteed: feature.guaranteed.as_ref().map(|reason| reason.value()),
            exemptions: feature
                .exemptions
                .iter()
                .map(|(platform, reason)| ir::Exemption {
                    platform: platform.to_string(),
                    reason: reason.value(),
                })
                .collect(),
            coverage: Vec::new(),
            source: location(declared.file, feature.span),
        });
    }
    ir::FeatureContract {
        name: item.name.to_string(),
        component,
        doc: item.doc.clone(),
        features,
        source: location(declared.file, item.span),
    }
}

/// How far a feature is reached in one size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    None,
    Conditional,
    Unconditional,
}

/// Entries found while proving one feature in one size.
struct Search<'a> {
    components: &'a BTreeMap<&'a str, &'a ir::ComponentDef>,
    /// Components that scope a contract, with the contract.
    scoped: &'a BTreeMap<String, String>,
    contract: &'a str,
    feature: &'a str,
    size: LayoutSize,
    entries: BTreeSet<String>,
}

impl Search<'_> {
    fn nodes(&mut self, component: &str, nodes: &[Node]) -> Level {
        nodes
            .iter()
            .map(|node| self.node(component, node))
            .max()
            .unwrap_or(Level::None)
    }

    /// All branches of a condition: proven only if each reaches it.
    fn branches(&mut self, component: &str, branches: &[&[Node]], exhaustive: bool) -> Level {
        let levels: Vec<Level> = branches
            .iter()
            .map(|body| self.nodes(component, body))
            .collect();
        if exhaustive && levels.iter().all(|level| *level == Level::Unconditional) {
            Level::Unconditional
        } else if levels.iter().any(|level| *level != Level::None) {
            Level::Conditional
        } else {
            Level::None
        }
    }

    fn node(&mut self, component: &str, node: &Node) -> Level {
        match node {
            Node::Element(element) => {
                if element.kind == "Layout" && element.keyword("size") != Some(self.size.name()) {
                    return Level::None;
                }
                let own = match element.prop("feature") {
                    Some(ir::Prop::Feature { contract, feature })
                        if contract == self.contract && feature == self.feature =>
                    {
                        self.entries.insert(target(component, element));
                        Level::Unconditional
                    }
                    _ => Level::None,
                };
                own.max(self.nodes(component, &element.children))
            }
            Node::Component(call) => {
                let name = call.component.as_str();
                if self
                    .scoped
                    .get(name)
                    .is_some_and(|scope| scope != self.contract)
                {
                    return Level::None;
                }
                match self.components.get(name) {
                    Some(def) => self.node(name, &def.body),
                    None => Level::None,
                }
            }
            Node::If(node) => {
                let mut bodies: Vec<&[Node]> = node
                    .branches
                    .iter()
                    .map(|(_, body)| body.as_slice())
                    .collect();
                let exhaustive = !node.otherwise.is_empty();
                bodies.push(&node.otherwise);
                self.branches(component, &bodies, exhaustive)
            }
            Node::IfLet(node) => self.branches(
                component,
                &[&node.then, &node.otherwise],
                !node.otherwise.is_empty(),
            ),
            // The checker only accepts exhaustive matches.
            Node::Match(node) => {
                let bodies: Vec<&[Node]> =
                    node.arms.iter().map(|arm| arm.body.as_slice()).collect();
                self.branches(component, &bodies, true)
            }
        }
    }
}

fn target(component: &str, element: &ir::Element) -> String {
    format!(
        "{component}.{}",
        element.id.as_deref().unwrap_or(element.path.as_str())
    )
}

/// The command an element's action runs, as a catalog id.
fn business_command(element: &ir::Element, protocol: &ProtocolDescriptor) -> Option<String> {
    match &element.action {
        Some(ir::Action::Engine { command, .. }) => {
            protocol.commands.get(*command).map(|spec| spec.id.clone())
        }
        Some(ir::Action::Media { op, .. }) => Some(op.command().to_string()),
        Some(ir::Action::Window { .. } | ir::Action::Session { .. }) | None => None,
    }
}

/// Prove every contract's coverage and check the entries in and around
/// the contract scopes: B202 for business entries without a feature, B204
/// for features of another contract or commands outside the feature.
pub fn prove(
    contracts: &mut [ir::FeatureContract],
    components: &[ir::ComponentDef],
    windows: Option<&ir::WindowCatalog>,
    protocol: &ProtocolDescriptor,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let by_name: BTreeMap<&str, &ir::ComponentDef> = components
        .iter()
        .map(|component| (component.name.as_str(), component))
        .collect();
    let scoped: BTreeMap<String, String> = contracts
        .iter()
        .map(|contract| (contract.component.clone(), contract.name.clone()))
        .collect();
    let mut reported = BTreeSet::new();
    let mut in_scope = BTreeSet::new();
    for contract in contracts.iter() {
        let mut walk = Walk {
            components: &by_name,
            scoped: &scoped,
            protocol,
            contract,
            visited: BTreeSet::new(),
            reported: &mut reported,
            diagnostics: &mut *diagnostics,
        };
        if let Some(root) = by_name.get(contract.component.as_str()) {
            walk.component(root);
        }
        in_scope.extend(walk.visited);
    }
    // Feature references outside every contract scope.
    let mut stray = |component: &str, node: &Node| {
        node.walk(&mut |node| {
            if let Node::Element(element) = node
                && let Some(ir::Prop::Feature { contract, feature }) = element.prop("feature")
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "`{contract}::{feature}` is used outside the scope of contract `{contract}`; mark the component with `#[component(features = {contract})]` or render it inside one"
                    ))
                    .code("B204")
                    .at(element.source.clone()),
                );
                let _ = component;
            }
        });
    };
    for component in components {
        if !in_scope.contains(component.name.as_str()) {
            stray(&component.name, &component.body);
        }
    }
    if let Some(windows) = windows {
        for window in &windows.windows {
            stray(&window.id, &window.content);
        }
    }

    for contract in contracts.iter_mut() {
        let name = contract.name.clone();
        let root = contract.component.clone();
        for feature in &mut contract.features {
            feature.coverage = feature
                .layouts
                .iter()
                .map(|size| {
                    let mut search = Search {
                        components: &by_name,
                        scoped: &scoped,
                        contract: &name,
                        feature: &feature.name,
                        size: *size,
                        entries: BTreeSet::new(),
                    };
                    let level = match by_name.get(root.as_str()) {
                        Some(component) => search.node(&root, &component.body),
                        None => Level::None,
                    };
                    let entries: Vec<String> = search.entries.into_iter().collect();
                    let coverage = match level {
                        Level::Unconditional => Coverage::Covered { entries },
                        Level::Conditional => Coverage::Conditional { entries },
                        Level::None => Coverage::Missing,
                    };
                    SizeCoverage {
                        size: *size,
                        coverage,
                    }
                })
                .collect();
        }
    }
}

/// One traversal of a contract scope, checking its business entries.
struct Walk<'a, 'd> {
    components: &'a BTreeMap<&'a str, &'a ir::ComponentDef>,
    scoped: &'a BTreeMap<String, String>,
    protocol: &'a ProtocolDescriptor,
    contract: &'a ir::FeatureContract,
    visited: BTreeSet<String>,
    reported: &'d mut BTreeSet<(String, String)>,
    diagnostics: &'d mut Vec<Diagnostic>,
}

impl Walk<'_, '_> {
    fn component(&mut self, component: &ir::ComponentDef) {
        if !self.visited.insert(component.name.clone()) {
            return;
        }
        let name = component.name.clone();
        let mut elements = Vec::new();
        let mut calls = Vec::new();
        collect(&component.body, &mut elements, &mut calls);
        for element in elements {
            self.element(&name, element);
        }
        for call in calls {
            if self
                .scoped
                .get(call.as_str())
                .is_some_and(|scope| *scope != self.contract.name)
            {
                continue;
            }
            if let Some(child) = self.components.get(call.as_str()) {
                self.component(child);
            }
        }
    }

    fn element(&mut self, component: &str, element: &ir::Element) {
        let feature = match element.prop("feature") {
            Some(ir::Prop::Feature { contract, feature }) => Some((contract, feature)),
            _ => None,
        };
        let command = business_command(element, self.protocol);
        let key = (component.to_string(), element.path.clone());
        let report = |walk: &mut Self, diagnostic: Diagnostic| {
            if walk.reported.insert(key.clone()) {
                walk.diagnostics.push(diagnostic);
            }
        };
        match (feature, command) {
            (None, Some(command)) => report(
                self,
                Diagnostic::error(format!(
                    "`{}` runs `{command}` inside the scope of contract `{}` without a feature; add `feature: {}::…`, or add the feature to the contract first",
                    target(component, element),
                    self.contract.name,
                    self.contract.name
                ))
                .code("B202")
                .at(element.source.clone()),
            ),
            (Some((contract, feature)), command) if *contract != self.contract.name => {
                let _ = command;
                report(
                    self,
                    Diagnostic::error(format!(
                        "`{contract}::{feature}` belongs to contract `{contract}`, but `{}` is in the scope of `{}`",
                        target(component, element),
                        self.contract.name
                    ))
                    .code("B204")
                    .at(element.source.clone()),
                );
            }
            (Some((_, feature)), command) => {
                let declared = self
                    .contract
                    .features
                    .iter()
                    .find(|candidate| candidate.name == *feature);
                let allowed = declared.map(|def| def.commands.clone()).unwrap_or_default();
                match command {
                    Some(command) if allowed.contains(&command) => {}
                    Some(command) => report(
                        self,
                        Diagnostic::error(format!(
                            "`{}` reaches `{}::{feature}` but runs `{command}`; the feature is implemented by {}",
                            target(component, element),
                            self.contract.name,
                            allowed.join(", ")
                        ))
                        .code("B204")
                        .at(element.source.clone()),
                    ),
                    None => report(
                        self,
                        Diagnostic::error(format!(
                            "`{}` names `{}::{feature}` but runs no business command",
                            target(component, element),
                            self.contract.name
                        ))
                        .code("B204")
                        .at(element.source.clone()),
                    ),
                }
            }
            (None, None) => {}
        }
    }
}

/// Every element and component call below `node`.
fn collect<'a>(node: &'a Node, elements: &mut Vec<&'a ir::Element>, calls: &mut Vec<String>) {
    node.walk(&mut |node| match node {
        Node::Element(element) => elements.push(element),
        Node::Component(call) => calls.push(call.component.clone()),
        _ => {}
    });
}
