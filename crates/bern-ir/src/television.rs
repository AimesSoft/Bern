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
        let window = Known {
            layouts: self.layouts,
            television: true,
            webview: None,
            menu_bar: Some(false),
        };
        self.pruned(&window)
    }

    /// Resolve a target capability before checking and code generation.
    /// Unselected branches, including their component dependencies, vanish.
    pub fn with_webview(&self, enabled: bool) -> Application {
        self.pruned(&Known {
            layouts: self.layouts,
            television: false,
            webview: Some(enabled),
            menu_bar: None,
        })
    }

    /// This application where the `MenuBar` does (`shown`) or does not show
    /// in a system menu bar: `window.menu_bar` is known, and the branches it
    /// leaves out vanish with the components only they place.
    pub fn with_menu_bar(&self, shown: bool) -> Application {
        self.pruned(&Known {
            layouts: self.layouts,
            television: false,
            webview: None,
            menu_bar: Some(shown),
        })
    }

    fn pruned(&self, window: &Known) -> Application {
        let mut app = self.clone();
        for component in &mut app.components {
            component.body = single(window.prune_node(&component.body));
        }
        for content in app.windows.windows.iter_mut().map(|w| &mut w.content) {
            *content = single(window.prune_node(content));
        }
        if let Some(bar) = &mut app.windows.menu_bar
            && let [Node::Element(pruned)] =
                window.prune_node(&Node::Element(bar.clone())).as_slice()
        {
            *bar = pruned.clone();
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
    television: bool,
    webview: Option<bool>,
    /// Whether the application's `MenuBar` shows in a system menu bar.
    menu_bar: Option<bool>,
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
                "webview" => self.webview,
                "menu_bar" => self.menu_bar,
                _ if !self.television => None,
                "tv" => Some(true),
                "landscape" => Some(true),
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
                let tv = self.television && element.kind == "Adaptive"
                && element.children.iter().any(|child| {
                    matches!(child, Node::Element(layout) if layout.keyword("size") == Some("tv"))
                });
                if tv {
                    element.children.retain(|child| {
                    matches!(child, Node::Element(layout) if layout.keyword("size") == Some("tv"))
                });
                }
                element.children = self.prune_nodes(&element.children);
                for prop in element.props.values_mut() {
                    match prop {
                        crate::Prop::Expr { expr } => self.resolve_expr(expr),
                        crate::Prop::Text {
                            text: crate::Text::Dynamic { expr },
                        } => self.resolve_expr(expr),
                        crate::Prop::Text {
                            text: crate::Text::Localized { args, .. },
                        } => {
                            for (_, expr) in args {
                                self.resolve_expr(expr);
                            }
                        }
                        crate::Prop::Query { query } => {
                            for expr in &mut query.args {
                                self.resolve_expr(expr);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(action) = &mut element.action {
                    match action {
                        crate::Action::Engine { args, .. } | crate::Action::Media { args, .. } => {
                            for expr in args {
                                self.resolve_expr(expr);
                            }
                        }
                        crate::Action::Session {
                            op:
                                crate::SessionOp::Set { value } | crate::SessionOp::PushStr { value },
                            ..
                        } => self.resolve_expr(value),
                        _ => {}
                    }
                }
                vec![Node::Element(element)]
            }
            Node::Component(call) => {
                let mut call = call.clone();
                for (_, arg) in &mut call.args {
                    if let crate::ComponentArg::Value { expr } = arg {
                        self.resolve_expr(expr);
                    }
                }
                vec![Node::Component(call)]
            }
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
                        None => {
                            let mut expr = expr.clone();
                            self.resolve_expr(&mut expr);
                            branches.push((expr, self.prune_nodes(body)));
                        }
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
                self.resolve_expr(&mut node.value);
                node.then = self.prune_nodes(&node.then);
                node.otherwise = self.prune_nodes(&node.otherwise);
                vec![Node::IfLet(node)]
            }
            Node::Match(node) => {
                let mut node = node.clone();
                self.resolve_expr(&mut node.value);
                for arm in &mut node.arms {
                    arm.body = self.prune_nodes(&arm.body);
                }
                vec![Node::Match(node)]
            }
        }
    }

    fn resolve_expr(&self, expr: &mut Expr) {
        if let Some(value) = self.known(expr) {
            expr.kind = ExprKind::Literal {
                value: Value::Bool(value),
            };
            return;
        }
        match &mut expr.kind {
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                if let Some(value) = self.known(cond) {
                    *expr = if value {
                        (**then).clone()
                    } else {
                        (**otherwise).clone()
                    };
                    self.resolve_expr(expr);
                } else {
                    self.resolve_expr(cond);
                    self.resolve_expr(then);
                    self.resolve_expr(otherwise);
                }
            }
            ExprKind::Not { expr } | ExprKind::Neg { expr } => self.resolve_expr(expr),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            ExprKind::Variant { fields, .. } => {
                for field in fields {
                    self.resolve_expr(field);
                }
            }
            _ => {}
        }
    }
}
