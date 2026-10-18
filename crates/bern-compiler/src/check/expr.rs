//! Expressions, displayed text and queries.

use super::{Checker, Root, Scope, is_displayable};
use bern_ir as ir;
use bern_protocol::{LocalizedText, Type, Value};
use bern_syntax::{self as ast, BinaryOp, Literal, UnaryOp};
use std::collections::BTreeMap;

impl Checker<'_> {
    pub(super) fn expect(
        &mut self,
        expr: &ast::Expr,
        ty: &Type,
        scope: &Scope,
        what: &str,
    ) -> Option<ir::Expr> {
        // A whole number written where a decimal is expected is that
        // decimal: `draft.size.set(12)` for an `f64` field.
        if let (
            ast::Expr::Literal {
                value: Literal::Int(whole),
                ..
            },
            Type::Float,
        ) = (expr, ty)
        {
            return Some(ir::Expr {
                kind: ir::ExprKind::Literal {
                    value: Value::Float(*whole as f64),
                },
                ty: Type::Float,
            });
        }
        let lowered = self.expr(expr, scope)?;
        if lowered.ty != *ty {
            self.error(
                expr.span(),
                format!(
                    "{what} must be {}, found {}",
                    ty.display(),
                    lowered.ty.display()
                ),
            );
            return None;
        }
        Some(lowered)
    }

    pub fn literal(value: &Literal) -> (Value, Type) {
        match value {
            Literal::Text(text) => (Value::Text(text.clone()), Type::Text),
            Literal::Int(value) => (Value::Int(*value), Type::Int),
            Literal::Float(value) => (Value::Float(*value), Type::Float),
            Literal::Bool(value) => (Value::Bool(*value), Type::Bool),
        }
    }

    pub(super) fn expr(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::Expr> {
        match expr {
            ast::Expr::Literal { value, .. } => {
                let (value, ty) = Self::literal(value);
                Some(ir::Expr {
                    kind: ir::ExprKind::Literal { value },
                    ty,
                })
            }
            ast::Expr::Tr(tr) => {
                self.error(
                    tr.span,
                    "translations are display text and cannot be used as values",
                );
                None
            }
            ast::Expr::Path(path) => self.binding(path, scope),
            ast::Expr::Variant(variant) => {
                let ty_name = variant.ty.to_string();
                let Some(def) = self.types.enum_def(&ty_name).cloned() else {
                    self.error(variant.ty.span(), format!("unknown enum `{ty_name}`"));
                    return None;
                };
                let Some(index) = def.variant_index(&variant.variant.to_string()) else {
                    self.error(
                        variant.variant.span(),
                        format!("`{ty_name}` has no variant `{}`", variant.variant),
                    );
                    return None;
                };
                let fields_def = &def.variants[index].fields;
                if fields_def.is_empty() && !variant.fields.is_empty() {
                    self.error(variant.variant.span(), "this variant has no fields");
                    return None;
                }
                let mut fields = Vec::new();
                for field_def in fields_def {
                    let Some((_, value)) = variant
                        .fields
                        .iter()
                        .find(|(name, _)| *name == field_def.name)
                    else {
                        self.error(
                            variant.variant.span(),
                            format!("missing field `{}`", field_def.name),
                        );
                        return None;
                    };
                    fields.push(self.expect(value, &field_def.ty, scope, "the field")?);
                }
                for (name, _) in &variant.fields {
                    if !fields_def.iter().any(|field| *name == field.name) {
                        self.error(
                            name.span(),
                            format!("`{ty_name}::{}` has no field `{name}`", variant.variant),
                        );
                        return None;
                    }
                }
                Some(ir::Expr {
                    kind: ir::ExprKind::Variant {
                        variant: index as u32,
                        fields,
                    },
                    ty: Type::Enum { name: ty_name },
                })
            }
            ast::Expr::Unary {
                op,
                expr: inner,
                span,
            } => {
                let lowered = self.expr(inner, scope)?;
                match op {
                    UnaryOp::Not => {
                        if lowered.ty != Type::Bool {
                            self.error(
                                *span,
                                format!("`!` needs bool, found {}", lowered.ty.display()),
                            );
                            return None;
                        }
                        Some(ir::Expr {
                            kind: ir::ExprKind::Not {
                                expr: Box::new(lowered),
                            },
                            ty: Type::Bool,
                        })
                    }
                    UnaryOp::Neg => match (&lowered.kind, &lowered.ty) {
                        (
                            ir::ExprKind::Literal {
                                value: Value::Int(v),
                            },
                            _,
                        ) => Some(ir::Expr {
                            kind: ir::ExprKind::Literal {
                                value: Value::Int(-v),
                            },
                            ty: Type::Int,
                        }),
                        (
                            ir::ExprKind::Literal {
                                value: Value::Float(v),
                            },
                            _,
                        ) => Some(ir::Expr {
                            kind: ir::ExprKind::Literal {
                                value: Value::Float(-v),
                            },
                            ty: Type::Float,
                        }),
                        (_, Type::Int | Type::Float) => Some(ir::Expr {
                            ty: lowered.ty.clone(),
                            kind: ir::ExprKind::Neg {
                                expr: Box::new(lowered),
                            },
                        }),
                        (_, ty) => {
                            self.error(
                                *span,
                                format!("`-` needs a number, found {}", ty.display()),
                            );
                            None
                        }
                    },
                }
            }
            ast::Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.expr(lhs, scope)?;
                let rhs = self.expr(rhs, scope)?;
                let ir_op = match op {
                    BinaryOp::Eq => ir::BinaryOp::Eq,
                    BinaryOp::Ne => ir::BinaryOp::Ne,
                    BinaryOp::Lt => ir::BinaryOp::Lt,
                    BinaryOp::Le => ir::BinaryOp::Le,
                    BinaryOp::Gt => ir::BinaryOp::Gt,
                    BinaryOp::Ge => ir::BinaryOp::Ge,
                    BinaryOp::And => ir::BinaryOp::And,
                    BinaryOp::Or => ir::BinaryOp::Or,
                };
                let ok = match op {
                    BinaryOp::Eq | BinaryOp::Ne => lhs.ty == rhs.ty,
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        lhs.ty == rhs.ty && matches!(lhs.ty, Type::Int | Type::Float)
                    }
                    BinaryOp::And | BinaryOp::Or => lhs.ty == Type::Bool && rhs.ty == Type::Bool,
                };
                if !ok {
                    self.error(
                        *span,
                        format!(
                            "`{}` cannot combine {} and {}",
                            op.symbol(),
                            lhs.ty.display(),
                            rhs.ty.display()
                        ),
                    );
                    return None;
                }
                Some(ir::Expr {
                    kind: ir::ExprKind::Binary {
                        op: ir_op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: Type::Bool,
                })
            }
            ast::Expr::Query(call) => {
                self.error(
                    call.span,
                    "a query call is only valid as the `query:` property of `Query`",
                );
                None
            }
            ast::Expr::If {
                cond,
                then,
                otherwise,
                span,
            } => {
                let cond = self.expr(cond, scope)?;
                if cond.ty != Type::Bool {
                    self.error(
                        *span,
                        format!("an `if` condition is a `bool`, found {}", cond.ty.display()),
                    );
                    return None;
                }
                let then = self.expr(then, scope)?;
                let otherwise = self.expr(otherwise, scope)?;
                // A whole number beside a float is a float.
                let ty = match (&then.ty, &otherwise.ty) {
                    (a, b) if a == b => a.clone(),
                    (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
                    (a, b) => {
                        self.error(
                            *span,
                            format!(
                                "both branches of an `if` have the same type, found {} and {}",
                                a.display(),
                                b.display()
                            ),
                        );
                        return None;
                    }
                };
                Some(ir::Expr {
                    kind: ir::ExprKind::If {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        otherwise: Box::new(otherwise),
                    },
                    ty,
                })
            }
        }
    }

    pub(super) fn display(
        &mut self,
        expr: &ast::Expr,
        scope: &Scope,
        key: String,
        localized_only: bool,
    ) -> Option<ir::Text> {
        match expr {
            ast::Expr::Tr(tr) => {
                let config = Some((self.languages, self.strict));
                let problems = bern_syntax::lower::check_translations(tr, config);
                let failed = !problems.is_empty();
                for (span, message) in problems {
                    self.error(span, message);
                }
                let mut args = Vec::new();
                for arg in &tr.args {
                    let lowered = self.expr(&arg.value, scope)?;
                    if !is_displayable(&lowered.ty, self.types) {
                        self.error(
                            arg.value.span(),
                            format!(
                                "{} has no text presentation; interpolate text, numbers or labelled enums",
                                lowered.ty.display()
                            ),
                        );
                        return None;
                    }
                    args.push((arg.name.to_string(), lowered));
                }
                if failed {
                    return None;
                }
                let text = LocalizedText {
                    key,
                    translations: tr
                        .entries
                        .iter()
                        .map(|entry| (entry.language.clone(), entry.text.clone()))
                        .collect(),
                    params: tr.args.iter().map(|arg| arg.name.to_string()).collect(),
                };
                self.texts.push(text.clone());
                Some(ir::Text::Localized { text, args })
            }
            ast::Expr::Literal {
                value: Literal::Text(literal),
                span,
            } => {
                if self.strict {
                    self.error(
                        *span,
                        "error[B103]: user-visible text must be declared with its translations; use `tr! { … }`",
                    );
                    return None;
                }
                let text = LocalizedText {
                    key,
                    translations: BTreeMap::from([(
                        self.default_language.to_string(),
                        literal.clone(),
                    )]),
                    params: vec![],
                };
                self.texts.push(text.clone());
                Some(ir::Text::Localized { text, args: vec![] })
            }
            ast::Expr::Path(path) if !localized_only => {
                let lowered = self.binding(path, scope)?;
                if !is_displayable(&lowered.ty, self.types) {
                    self.error(
                        path.span(),
                        format!(
                            "{} has no text presentation; use `if`, `match` or a labelled enum",
                            lowered.ty.display()
                        ),
                    );
                    return None;
                }
                Some(ir::Text::Dynamic { expr: lowered })
            }
            other => {
                self.error(
                    other.span(),
                    if localized_only {
                        "expected `tr! { … }` text"
                    } else {
                        "expected `tr! { … }` text or a displayable binding"
                    },
                );
                None
            }
        }
    }

    /// Lower `client.query(args)` against the protocol's query catalog.
    pub(super) fn query(&mut self, expr: &ast::Expr, scope: &Scope) -> Option<ir::QueryBinding> {
        let ast::Expr::Query(call) = expr else {
            self.error(
                expr.span(),
                "expected a query call such as `app.search(draft.text)`",
            );
            return None;
        };
        let root = call.receiver[0].to_string();
        let namespace = match scope.get(&root) {
            Some(Root::Client(namespace)) if call.receiver.len() == 1 => namespace.clone(),
            _ => {
                self.error(
                    call.receiver[0].span(),
                    format!("`{root}` is not the typed engine client; queries are called on it"),
                );
                return None;
            }
        };
        let method = call.method.to_string();
        let id = format!("{namespace}.{method}");
        let Some(query) = self.protocol.commands.iter().position(|c| c.id == id) else {
            self.error(
                call.method.span(),
                format!(
                    "protocol `{namespace}` has no query `{method}`; declare it in the `#[queries]` trait"
                ),
            );
            return None;
        };
        let spec = self.protocol.commands[query].clone();
        if !spec.is_query() {
            self.error(
                call.method.span(),
                format!(
                    "`{method}` is a command; `query:` binds a read-only query from the \
                     `#[queries]` trait"
                ),
            );
            return None;
        }
        if spec.params.len() != call.args.len() {
            self.error(
                call.method.span(),
                format!(
                    "`{}` takes {} argument(s), found {}",
                    spec.id,
                    spec.params.len(),
                    call.args.len()
                ),
            );
            return None;
        }
        let mut args = Vec::new();
        for (arg, param) in call.args.iter().zip(&spec.params) {
            args.push(self.expect(arg, &param.ty, scope, &format!("argument `{}`", param.name))?);
        }
        Some(ir::QueryBinding {
            query,
            args,
            result: spec.result,
        })
    }
}
