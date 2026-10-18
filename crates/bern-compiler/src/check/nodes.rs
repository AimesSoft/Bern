//! The tree of a component body: lists of nodes, conditions, matches
//! and the components it places.

use super::{Checker, Ctx, Root, Scope};
use bern_ir as ir;
use bern_protocol::Type;
use bern_syntax as ast;
use std::collections::BTreeSet;

impl Checker<'_> {
    pub(super) fn nodes(
        &mut self,
        nodes: &[ast::Node],
        prefix: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Vec<ir::Node> {
        let mut lowered = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let path = format!("{prefix}.{index}");
            match node {
                ast::Node::Element(element)
                    if matches!(
                        element.name.to_string().as_str(),
                        "Slot" | "Pages" | "PageContent"
                    ) =>
                {
                    lowered.extend(self.expand(element, &path, ctx, scope));
                }
                node => lowered.extend(self.node(node, &path, ctx, scope)),
            }
        }
        lowered
    }

    pub(super) fn node(
        &mut self,
        node: &ast::Node,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Option<ir::Node> {
        let inner = Ctx {
            is_root: false,
            ..ctx.clone()
        };
        match node {
            ast::Node::Element(element) => self
                .element(element, path, ctx, scope)
                .map(ir::Node::Element),
            ast::Node::Component(call) => self.component_call(call, scope).map(ir::Node::Component),
            ast::Node::If(node) => {
                let mut branches = Vec::new();
                for (index, (condition, body)) in node.branches.iter().enumerate() {
                    let condition = self.expect(condition, &Type::Bool, scope, "a condition");
                    let body = self.nodes(body, &format!("{path}.if{index}"), &inner, scope);
                    if let Some(condition) = condition {
                        branches.push((condition, body));
                    }
                }
                let otherwise = node
                    .otherwise
                    .as_ref()
                    .map(|body| self.nodes(body, &format!("{path}.else"), &inner, scope))
                    .unwrap_or_default();
                Some(ir::Node::If(ir::IfNode {
                    branches,
                    otherwise,
                }))
            }
            ast::Node::IfLet(node) => {
                let value = self.expr(&node.value, scope)?;
                let Type::Optional { inner: inner_ty } = value.ty.clone() else {
                    self.error(
                        node.value.span(),
                        format!(
                            "`if let Some(…)` needs an optional value, found {}",
                            value.ty.display()
                        ),
                    );
                    return None;
                };
                scope.push(node.binding.to_string(), Root::Local((*inner_ty).clone()));
                let then = self.nodes(&node.then, &format!("{path}.then"), &inner, scope);
                scope.pop();
                let otherwise = node
                    .otherwise
                    .as_ref()
                    .map(|body| self.nodes(body, &format!("{path}.else"), &inner, scope))
                    .unwrap_or_default();
                Some(ir::Node::IfLet(ir::IfLetNode {
                    var: node.binding.to_string(),
                    value,
                    inner: *inner_ty,
                    then,
                    otherwise,
                }))
            }
            ast::Node::Match(node) => self
                .match_node(node, path, &inner, scope)
                .map(ir::Node::Match),
        }
    }

    fn match_node(
        &mut self,
        node: &ast::MatchNode,
        path: &str,
        ctx: &Ctx,
        scope: &mut Scope,
    ) -> Option<ir::MatchNode> {
        let value = self.expr(&node.value, scope)?;
        let Type::Enum { name } = value.ty.clone() else {
            self.error(
                node.value.span(),
                format!("`match` needs an enum value, found {}", value.ty.display()),
            );
            return None;
        };
        let def = self.types.enum_def(&name)?.clone();
        let mut covered = BTreeSet::new();
        let mut wildcard = false;
        let mut arms = Vec::new();
        for (index, arm) in node.arms.iter().enumerate() {
            let arm_path = format!("{path}.arm{index}");
            match &arm.pattern {
                ast::Pattern::Wildcard(span) => {
                    if wildcard {
                        self.error(*span, "`_` appears twice");
                    }
                    wildcard = true;
                    let body = self.nodes(&arm.body, &arm_path, ctx, scope);
                    arms.push(ir::MatchArm {
                        variant: None,
                        bindings: vec![],
                        body,
                    });
                }
                ast::Pattern::Variant {
                    ty,
                    variant,
                    bindings,
                } => {
                    if *ty != name {
                        self.error(
                            ty.span(),
                            format!("pattern type `{ty}` does not match the matched `{name}`"),
                        );
                        continue;
                    }
                    let Some(index) = def.variant_index(&variant.to_string()) else {
                        self.error(
                            variant.span(),
                            format!("`{name}` has no variant `{variant}`"),
                        );
                        continue;
                    };
                    if !covered.insert(index) {
                        self.error(
                            variant.span(),
                            format!("`{name}::{variant}` is matched twice"),
                        );
                    }
                    let fields = &def.variants[index].fields;
                    let mut lowered = Vec::new();
                    for binding in bindings {
                        match fields.iter().position(|field| *binding == field.name) {
                            Some(field) => lowered.push(ir::MatchBinding {
                                name: binding.to_string(),
                                field,
                                ty: fields[field].ty.clone(),
                            }),
                            None => self.error(
                                binding.span(),
                                format!("`{name}::{variant}` has no field `{binding}`"),
                            ),
                        }
                    }
                    for binding in &lowered {
                        scope.push(binding.name.clone(), Root::Local(binding.ty.clone()));
                    }
                    let body = self.nodes(&arm.body, &arm_path, ctx, scope);
                    for _ in &lowered {
                        scope.pop();
                    }
                    arms.push(ir::MatchArm {
                        variant: Some(index as u32),
                        bindings: lowered,
                        body,
                    });
                }
            }
        }
        if !wildcard && covered.len() < def.variants.len() {
            let missing: Vec<_> = def
                .variants
                .iter()
                .enumerate()
                .filter(|(index, _)| !covered.contains(index))
                .map(|(_, variant)| format!("{name}::{}", variant.name))
                .collect();
            self.error(
                node.span,
                format!("non-exhaustive match; missing {}", missing.join(", ")),
            );
        }
        Some(ir::MatchNode {
            value,
            enum_name: name,
            arms,
        })
    }

    fn component_call(
        &mut self,
        call: &ast::ComponentCall,
        scope: &Scope,
    ) -> Option<ir::ComponentRef> {
        let name = call.name().to_string();
        let Some(signature) = self.signatures.get(&name).cloned() else {
            self.error(
                call.span,
                format!(
                    "unknown component `{name}`; declare it with #[component] in a listed source"
                ),
            );
            return None;
        };
        if call.path.len() > 1 {
            let written: Vec<String> = call.path[..call.path.len() - 1]
                .iter()
                .map(|ident| ident.to_string())
                .filter(|segment| segment != "crate" && segment != "self")
                .collect();
            if !signature.module.ends_with(&written) {
                self.error(
                    call.span,
                    format!(
                        "`{}` resolves to module `{}`; aliases and re-exports are not supported",
                        call.path_string(),
                        signature.module.join("::")
                    ),
                );
            }
        }
        let mut args = Vec::new();
        for arg in &call.args {
            let arg_name = arg.name.to_string();
            let Some((_, kind)) = signature.params.iter().find(|(param, kind)| {
                *param == arg_name && !matches!(kind, ir::ParamKind::Session { .. })
            }) else {
                self.error(
                    arg.name.span(),
                    format!("`{name}` has no parameter `{arg_name}`"),
                );
                continue;
            };
            let lowered = match kind {
                ir::ParamKind::Client { namespace } => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Client(given)) if given == namespace
                            ) =>
                    {
                        Some(ir::ComponentArg::Client)
                    }
                    other => {
                        self.error(
                            other.span(),
                            format!("pass the typed `{namespace}` client, e.g. `{namespace}: {namespace}`"),
                        );
                        None
                    }
                },
                ir::ParamKind::Windows => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Windows)
                            ) =>
                    {
                        Some(ir::ComponentArg::Windows)
                    }
                    other => {
                        self.error(
                            other.span(),
                            "pass the window handle, e.g. `windows: windows`",
                        );
                        None
                    }
                },
                ir::ParamKind::Players => match &arg.value {
                    ast::Expr::Path(path)
                        if path.segments.len() == 1
                            && matches!(
                                scope.get(&path.segments[0].to_string()),
                                Some(Root::Players)
                            ) =>
                    {
                        Some(ir::ComponentArg::Players)
                    }
                    other => {
                        self.error(
                            other.span(),
                            "pass the player catalog, e.g. `players: players`",
                        );
                        None
                    }
                },
                ir::ParamKind::Value { ty } => self
                    .expect(&arg.value, ty, scope, &format!("argument `{arg_name}`"))
                    .map(|expr| ir::ComponentArg::Value { expr }),
                ir::ParamKind::Session { .. } => None,
            };
            if let Some(lowered) = lowered {
                args.push((arg_name, lowered));
            }
        }
        for (param, kind) in &signature.params {
            if !matches!(kind, ir::ParamKind::Session { .. })
                && !call.args.iter().any(|arg| arg.name == param)
            {
                self.error(
                    call.span,
                    format!("missing argument `{param}` for `{name}`"),
                );
            }
        }
        Some(ir::ComponentRef {
            component: name,
            args,
            source: self.loc(call.span),
        })
    }
}
