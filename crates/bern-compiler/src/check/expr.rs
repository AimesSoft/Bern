//! Expressions, displayed text and queries.

use super::{Checker, Root, Scope, is_displayable, not_content};
use bern_ir as ir;
use bern_protocol::format::Use;
use bern_protocol::{LocalizedText, Type, Value};
use bern_syntax::{self as ast, BinaryOp, Literal, UnaryOp};
use std::collections::{BTreeMap, BTreeSet};

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

    /// The corner of `style!` that `expr` names. In a `corner` the names
    /// `style!` declares stand for the corners, before any variable of the
    /// same name.
    pub(super) fn corner_name(&self, expr: &ast::Expr) -> Option<ir::Expr> {
        let ast::Expr::Path(path) = expr else {
            return None;
        };
        let [name] = path.segments.as_slice() else {
            return None;
        };
        let name = name.to_string();
        let (_, radius) = self.corners.iter().find(|(corner, _)| *corner == name)?;
        Some(ir::Expr {
            kind: ir::ExprKind::Corner {
                name,
                radius: *radius,
            },
            ty: Type::Float,
        })
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
            ast::Expr::Path(path) => {
                if self.corner_names {
                    if let Some(corner) = self.corner_name(expr) {
                        return Some(corner);
                    }
                    if let [name] = path.segments.as_slice()
                        && scope.get(&name.to_string()).is_none()
                    {
                        let message = crate::style::unknown_corner(
                            &name.to_string(),
                            self.corners.iter().map(|(name, _)| name.as_str()),
                        );
                        self.error(path.span(), message);
                        return None;
                    }
                }
                self.binding(path, scope)
            }
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
                let mut uses: BTreeMap<String, BTreeSet<Use>> = BTreeMap::new();
                for entry in &tr.entries {
                    if let Ok(pieces) = bern_protocol::format::parse(&entry.text) {
                        for (name, how) in bern_protocol::format::uses(&pieces) {
                            uses.entry(name).or_default().insert(how);
                        }
                    }
                }
                let mut args = Vec::new();
                let mut wrong = false;
                for arg in &tr.args {
                    let lowered = self.expr(&arg.value, scope)?;
                    let name = arg.name.to_string();
                    let span = arg.value.span();
                    for how in uses.get(&name).into_iter().flatten() {
                        if let Some(problem) = self.argument_kind(&name, *how, &lowered.ty) {
                            self.error(span, problem);
                            wrong = true;
                        }
                    }
                    if lowered.ty == Type::Text && !self.is_content(&lowered, scope) {
                        self.error(
                            span,
                            match &arg.value {
                                ast::Expr::Path(path) => not_content(&path.dotted()),
                                _ => "error[B105]: fixed text belongs in the translations; an argument is a value, such as a number, a date or text marked `#[content]`".to_string(),
                            },
                        );
                        wrong = true;
                    }
                    args.push((name, lowered));
                }
                if failed || wrong {
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
                if bern_syntax::lower::is_date(&lowered.ty) {
                    self.error(
                        path.span(),
                        format!(
                            "a date is written in the reader's language with `tr!`, such as `tr! {{ \"en\" => \"{{day, date, EEE, MMM d}}\" ; day: {} }}`",
                            path.dotted()
                        ),
                    );
                    return None;
                }
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
                if lowered.ty == Type::Text && !self.is_content(&lowered, scope) {
                    self.error(path.span(), not_content(&path.dotted()));
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

    /// Whether a text value is content: data shown as it is, such as a
    /// title or a file name, declared with `#[content]`. What the user
    /// typed into session state is content too.
    pub(super) fn is_content(&self, expr: &ir::Expr, scope: &Scope) -> bool {
        match &expr.kind {
            ir::ExprKind::Binding { binding } => match binding {
                ir::Binding::Engine { field, path } => {
                    let state = &self.protocol.state[*field];
                    self.path_content(&state.ty, path, state.content)
                }
                ir::Binding::Session { .. } => true,
                ir::Binding::Local { name, path, root } => {
                    let whole = matches!(scope.get(name), Some(Root::Local(_, true)));
                    self.path_content(root, path, whole)
                }
                ir::Binding::Window { .. } => false,
            },
            ir::ExprKind::If {
                then, otherwise, ..
            } => self.is_content(then, scope) && self.is_content(otherwise, scope),
            _ => false,
        }
    }

    /// Whether the field at the end of `path` inside `ty` is content;
    /// `whole` for an empty path.
    fn path_content(&self, ty: &Type, path: &[usize], whole: bool) -> bool {
        let mut content = whole;
        let mut ty = ty.clone();
        for index in path {
            let Type::Record { name } = &ty else {
                return false;
            };
            let Some(field) = self
                .types
                .record_def(name)
                .and_then(|def| def.fields.get(*index))
            else {
                return false;
            };
            content = field.content;
            ty = field.ty.clone();
        }
        content
    }

    /// What is wrong with interpolating a `ty` value as `how`, if anything.
    fn argument_kind(&self, name: &str, how: Use, ty: &Type) -> Option<String> {
        let number = matches!(ty, Type::Int | Type::Float);
        let date = bern_syntax::lower::is_date(ty);
        let fits = match how {
            Use::Value => is_displayable(ty, self.types),
            Use::Plural | Use::Duration => number,
            Use::Date => date,
        };
        if fits {
            return None;
        }
        Some(match how {
            Use::Value if date => format!(
                "error[B104]: `{{{name}}}` is a date; write it with a pattern, such as `{{{name}, date, MMM d}}`"
            ),
            Use::Value => format!(
                "error[B104]: {} has no text presentation; interpolate text, numbers, dates or labelled enums",
                ty.display()
            ),
            Use::Plural | Use::Duration => format!(
                "error[B104]: `{{{name}, {}, …}}` needs a number, found {}",
                how.name(),
                ty.display()
            ),
            Use::Date => format!(
                "error[B104]: `{{{name}, date, …}}` needs a `CalendarDate`, found {}",
                ty.display()
            ),
        })
    }
}
