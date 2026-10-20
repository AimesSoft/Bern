//! The application as a television shows it. A target whose device is
//! always a television (tvOS) builds and checks only what can appear
//! there: an `Adaptive` with a `tv` layout shows that layout and no other,
//! and conditions on the window are known, as a television's window is
//! always its whole screen: `window.tv` holds, and the window's size class
//! and `window.short` follow from the screen's size in points and the
//! breakpoints. Components that only the parts left out place are left out
//! with them.

use crate::{
    Application, BinaryOp, Binding, ComponentDef, Expr, ExprKind, IfNode, LayoutConfig, Node,
};
use bern_protocol::{Type, Value};
use std::collections::BTreeSet;

/// A television's screen in points, whatever its pixels: 1920 × 1080.
pub const TELEVISION_SCREEN: (f64, f64) = (1920.0, 1080.0);

impl Application {
    /// This application with what a television never shows left out.
    pub fn on_television(&self) -> Application {
        let mut app = self.clone();
        let window = Known {
            layouts: app.layouts,
        };
        for component in &mut app.components {
            component.body = single(window.prune_node(&component.body));
        }
        for content in app.windows.windows.iter_mut().map(|w| &mut w.content) {
            *content = single(window.prune_node(content));
        }
        // Components placed from what is left, starting at the windows.
        let mut reached = BTreeSet::new();
        let mut pending: Vec<String> = Vec::new();
        let collect = |node: &Node, pending: &mut Vec<String>| {
            node.walk(&mut |node| {
                if let Node::Component(call) = node {
                    pending.push(call.component.clone());
                }
            });
        };
        for window in &app.windows.windows {
            collect(&window.content, &mut pending);
        }
        while let Some(name) = pending.pop() {
            if !reached.insert(name.clone()) {
                continue;
            }
            if let Some(component) = app.components.iter().find(|c| c.name == name) {
                collect(&component.body, &mut pending);
            }
        }
        app.components
            .retain(|component: &ComponentDef| reached.contains(&component.name));
        app
    }
}

/// What the window of a television is known to be.
struct Known {
    layouts: Option<LayoutConfig>,
}

impl Known {
    /// What a condition is known to be on a television: the window's
    /// fields, and negations and combinations of them; any other condition
    /// is decided while the app runs.
    fn known(&self, expr: &Expr) -> Option<bool> {
        match &expr.kind {
            ExprKind::Binding {
                binding: Binding::Window { field },
            } => match field.as_str() {
                "tv" => Some(true),
                "idle" => None,
                "short" => self
                    .layouts
                    .map(|layouts| TELEVISION_SCREEN.1 < layouts.compact_below),
                size => self
                    .layouts
                    .map(|layouts| layouts.size(TELEVISION_SCREEN.0).name() == size),
            },
            ExprKind::Literal {
                value: Value::Bool(value),
            } => Some(*value),
            ExprKind::Not { expr } => self.known(expr).map(|value| !value),
            ExprKind::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
            } => match (self.known(lhs), self.known(rhs)) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            },
            ExprKind::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
            } => match (self.known(lhs), self.known(rhs)) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
}

/// One node standing for `nodes`: the node itself, or a condition that is
/// always true around them.
fn single(mut nodes: Vec<Node>) -> Node {
    if nodes.len() == 1 {
        return nodes.pop().expect("one node");
    }
    Node::If(IfNode {
        branches: vec![(
            Expr {
                kind: ExprKind::Literal {
                    value: Value::Bool(true),
                },
                ty: Type::Bool,
            },
            nodes,
        )],
        otherwise: Vec::new(),
    })
}

impl Known {
    fn prune_nodes(&self, nodes: &[Node]) -> Vec<Node> {
        nodes
            .iter()
            .flat_map(|node| self.prune_node(node))
            .collect()
    }

    /// A node as a television shows it: none, itself, or the branch of a
    /// condition that is known there.
    fn prune_node(&self, node: &Node) -> Vec<Node> {
        match node {
            Node::Element(element) => {
                let mut element = element.clone();
                let tv = element.kind == "Adaptive"
                && element.children.iter().any(|child| {
                    matches!(child, Node::Element(layout) if layout.keyword("size") == Some("tv"))
                });
                if tv {
                    element.children.retain(|child| {
                    matches!(child, Node::Element(layout) if layout.keyword("size") == Some("tv"))
                });
                }
                element.children = self.prune_nodes(&element.children);
                vec![Node::Element(element)]
            }
            Node::Component(_) => vec![node.clone()],
            Node::If(condition) => {
                let mut branches = Vec::new();
                for (expr, body) in &condition.branches {
                    match self.known(expr) {
                        Some(true) => {
                            // The first branch known to hold ends the chain: it
                            // is what remains when the ones before it fail.
                            let body = self.prune_nodes(body);
                            if branches.is_empty() {
                                return body;
                            }
                            return vec![Node::If(IfNode {
                                branches,
                                otherwise: body,
                            })];
                        }
                        Some(false) => {}
                        None => branches.push((expr.clone(), self.prune_nodes(body))),
                    }
                }
                let otherwise = self.prune_nodes(&condition.otherwise);
                if branches.is_empty() {
                    return otherwise;
                }
                vec![Node::If(IfNode {
                    branches,
                    otherwise,
                })]
            }
            Node::IfLet(node) => {
                let mut node = node.clone();
                node.then = self.prune_nodes(&node.then);
                node.otherwise = self.prune_nodes(&node.otherwise);
                vec![Node::IfLet(node)]
            }
            Node::Match(node) => {
                let mut node = node.clone();
                for arm in &mut node.arms {
                    arm.body = self.prune_nodes(&arm.body);
                }
                vec![Node::Match(node)]
            }
        }
    }
}
