//! Named corner radii on one platform. `style!` names radii once
//! (`corner { card: 22 }`) and gives other values on some platforms
//! (`#[target(windows)] corner { card: 8 }`); elements and style rules use
//! the names. Before a target generates, the names in elements become the
//! numbers they have on its platform, and `window.menu_bar` what it is
//! there.

use crate::{Application, Expr, ExprKind, Node, Prop};
use bern_protocol::Value;

impl Application {
    /// This application with every named corner of its elements at the
    /// radius it has on `platform` (`macos`, `windows`, `browser`, …).
    /// `window.menu_bar` holds on macOS, whose menu bar shows the
    /// application's `MenuBar`, and nowhere else.
    pub fn on_platform(&self, platform: &str) -> Application {
        let mut app = self.with_menu_bar(platform == "macos");
        if app.style.corners.is_empty() {
            return app;
        }
        let style = std::mem::take(&mut app.style);
        let radius = |name: &str, radius: f64| style.corner(name, platform).unwrap_or(radius);
        for component in &mut app.components {
            resolve_node(&mut component.body, &radius);
        }
        for window in &mut app.windows.windows {
            resolve_node(&mut window.content, &radius);
        }
        app.style = style;
        app
    }
}

fn resolve_node(node: &mut Node, radius: &impl Fn(&str, f64) -> f64) {
    match node {
        Node::Element(element) => {
            for prop in element.props.values_mut() {
                let Prop::Expr { expr } = prop else {
                    continue;
                };
                // A name alone is a number, as the radius written out is.
                if let ExprKind::Corner {
                    name,
                    radius: value,
                } = &expr.kind
                {
                    *prop = Prop::Number {
                        value: radius(name, *value),
                    };
                    continue;
                }
                resolve_expr(expr, radius);
            }
            for child in &mut element.children {
                resolve_node(child, radius);
            }
        }
        Node::Component(_) => {}
        Node::If(node) => {
            for (_, body) in &mut node.branches {
                for child in body {
                    resolve_node(child, radius);
                }
            }
            for child in &mut node.otherwise {
                resolve_node(child, radius);
            }
        }
        Node::IfLet(node) => {
            for child in node.then.iter_mut().chain(&mut node.otherwise) {
                resolve_node(child, radius);
            }
        }
        Node::Match(node) => {
            for arm in &mut node.arms {
                for child in &mut arm.body {
                    resolve_node(child, radius);
                }
            }
        }
    }
}

fn resolve_expr(expr: &mut Expr, radius: &impl Fn(&str, f64) -> f64) {
    match &mut expr.kind {
        ExprKind::Corner {
            name,
            radius: value,
        } => {
            expr.kind = ExprKind::Literal {
                value: Value::Float(radius(name, *value)),
            };
        }
        ExprKind::Literal { .. } | ExprKind::Binding { .. } => {}
        ExprKind::Variant { fields, .. } => {
            for field in fields {
                resolve_expr(field, radius);
            }
        }
        ExprKind::Not { expr } | ExprKind::Neg { expr } => resolve_expr(expr, radius),
        ExprKind::Binary { lhs, rhs, .. } => {
            resolve_expr(lhs, radius);
            resolve_expr(rhs, radius);
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            resolve_expr(cond, radius);
            resolve_expr(then, radius);
            resolve_expr(otherwise, radius);
        }
    }
}
