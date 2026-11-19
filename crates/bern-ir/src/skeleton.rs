//! Compile-time skeleton projection.
//!
//! Projects a real UI subtree (from a `Query` body or any element carrying
//! `skeleton: <bool>`) into an equivalent non-interactive layout tree built
//! from `Skeleton` placeholders and the original layout containers. The
//! projection runs purely on the static IR: it never synthesizes fake business
//! data or evaluates unloaded bindings, keeps responsive `window.*` and
//! surrounding state conditions alive, expands user components with constant
//! argument folding, and removes all interactive actions, IDs and events.

use crate::{
    Application, BinaryOp, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind, IfNode,
    MatchArm, MatchNode, Node, Prop, SourceLocation, spec,
};
use bern_protocol::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Stagger delay (seconds) applied to expanded repeated skeleton containers so
/// rows/cards rise softly into view one after another.
const DEFAULT_STAGGER: f64 = 0.04;

/// Default sample counts when no explicit `skeleton_count` is provided.
const DEFAULT_LIST_COUNT: usize = 6;
const DEFAULT_GRID_COUNT: usize = 8;
const DEFAULT_PREVIEW_COUNT: usize = 3;

/// Staggered relative width fractions for primary and secondary text bars so
/// repeated skeleton rows read naturally without hard-coded pixel widths.
const PRIMARY_FRACTIONS: [f64; 6] = [0.78, 0.56, 0.88, 0.66, 0.82, 0.60];
const SECONDARY_FRACTIONS: [f64; 6] = [0.48, 0.36, 0.56, 0.42, 0.34, 0.50];

/// Natural width caps (in logical units) for flexible text skeleton bars so
/// they stay proportional on wide desktops while still shrinking cleanly on
/// phones and watches.
const PRIMARY_MAX_WIDTH: f64 = 260.0;
const SECONDARY_MAX_WIDTH: f64 = 160.0;

/// Populate `Element::skeleton` for every `Query` that defaults to (or
/// requests) skeleton loading and every element carrying `skeleton: <bool>`.
pub fn lower_skeletons(app: &mut Application) {
    let components: BTreeMap<String, ComponentDef> = app
        .components
        .iter()
        .cloned()
        .map(|component| (component.name.clone(), component))
        .collect();

    for window in &mut app.windows.windows {
        lower_node(&mut window.content, &components);
    }
    for component in &mut app.components {
        lower_node(&mut component.body, &components);
    }
}

fn lower_node(node: &mut Node, components: &BTreeMap<String, ComponentDef>) {
    match node {
        Node::Element(element) => {
            for child in &mut element.children {
                lower_node(child, components);
            }
            if element.kind == "Query" {
                let uses_skeleton = match element.keyword("loading_style") {
                    Some("spinner") => false,
                    Some("skeleton") => true,
                    _ => element.prop("loading").is_none(),
                };
                if uses_skeleton {
                    let count = skeleton_count(element);
                    let mut unloaded = BTreeSet::new();
                    if let Some(template) = &element.template {
                        unloaded.insert(template.var.clone());
                    }
                    let (_, body) = element.query_children();
                    let mut ctx = ProjectCtx::root(components, count, unloaded);
                    let mut projected = project_nodes(body, &mut ctx);
                    if projected.is_empty() {
                        projected.push(default_fallback_skeleton(&element.path, &element.source));
                    }
                    element.skeleton = Some(projected);
                } else {
                    element.skeleton = None;
                }
            } else if element.expr("skeleton").is_some() {
                let count = skeleton_count(element);
                let mut ctx = ProjectCtx::root(components, count, BTreeSet::new());
                let mut stripped = element.clone();
                stripped.props.remove("skeleton");
                stripped.props.remove("skeleton_count");
                stripped.skeleton = None;
                let mut projected = project_element(&stripped, &mut ctx);
                if projected.is_empty() {
                    projected.push(default_fallback_skeleton(&element.path, &element.source));
                }
                element.skeleton = Some(projected);
            }
        }
        Node::Component(_) => {}
        Node::If(node) => {
            for (_, body) in &mut node.branches {
                for child in body {
                    lower_node(child, components);
                }
            }
            for child in &mut node.otherwise {
                lower_node(child, components);
            }
        }
        Node::IfLet(node) => {
            for child in node.then.iter_mut().chain(&mut node.otherwise) {
                lower_node(child, components);
            }
        }
        Node::Match(node) => {
            for arm in &mut node.arms {
                for child in &mut arm.body {
                    lower_node(child, components);
                }
            }
        }
    }
}

fn skeleton_count(element: &Element) -> Option<usize> {
    element
        .number("skeleton_count")
        .map(|value| value.round().clamp(1.0, 30.0) as usize)
}

#[derive(Clone)]
struct ProjectCtx<'a> {
    components: &'a BTreeMap<String, ComponentDef>,
    /// Depth guard against recursive component definitions.
    depth: usize,
    /// Optional explicit sample count from `skeleton_count`.
    repeat_count: Option<usize>,
    /// Sample row index (0..N) used to stagger placeholder bar widths.
    row_index: usize,
    /// Running text-bar counter inside one row so sibling bars vary subtly.
    text_slot: usize,
    /// Fixed width of the narrowest enclosing column, if any (for table/rank columns).
    column_width: Option<f64>,
    /// Whether the enclosing layout is a horizontal `Row`.
    in_row: bool,
    /// Whether we are directly inside a `Grid`, `Flow`, or `PreviewList`.
    parent_kind: Option<String>,
    /// Local variables whose values come from unloaded data (`Query`, `ForEach`,
    /// `IfLet`, or component params bound to them) and cannot remain in runtime exprs.
    unloaded_locals: BTreeSet<String>,
    /// Parameter substitutions active while expanding an inlined `ComponentRef`.
    subst: BTreeMap<String, Expr>,
    /// Local variables declared in the enclosing caller scope that ARE valid to
    /// reference in runtime expressions (anything outside `inlined` scope).
    inlined: bool,
    /// Monotonic counter for unique synthetic IR paths.
    next_id: usize,
}

impl<'a> ProjectCtx<'a> {
    fn root(
        components: &'a BTreeMap<String, ComponentDef>,
        repeat_count: Option<usize>,
        unloaded_locals: BTreeSet<String>,
    ) -> Self {
        Self {
            components,
            depth: 0,
            repeat_count,
            row_index: 0,
            text_slot: 0,
            column_width: None,
            in_row: false,
            parent_kind: None,
            unloaded_locals,
            subst: BTreeMap::new(),
            inlined: false,
            next_id: 0,
        }
    }

    fn fresh_path(&mut self, base: &str, suffix: &str) -> String {
        let n = self.next_id;
        self.next_id += 1;
        format!("{base}/skel_{suffix}_{n}")
    }

    fn substitute_expr(&self, expr: &Expr) -> Option<Expr> {
        let substituted = subst_expr_inner(expr, &self.subst, &self.unloaded_locals, self.inlined)?;
        Some(fold_expr(substituted))
    }
}

/// Substitute inlined component parameters and reject any expression that
/// still depends on an unloaded or out-of-scope local binding.
fn subst_expr_inner(
    expr: &Expr,
    subst: &BTreeMap<String, Expr>,
    unloaded: &BTreeSet<String>,
    inlined: bool,
) -> Option<Expr> {
    let kind = match &expr.kind {
        ExprKind::Literal { value } => ExprKind::Literal {
            value: value.clone(),
        },
        ExprKind::Corner { name, radius } => ExprKind::Corner {
            name: name.clone(),
            radius: *radius,
        },
        ExprKind::Binding { binding } => match binding {
            Binding::Local { name, path, .. } => {
                if unloaded.contains(name) {
                    return None;
                }
                if let Some(replacement) = subst.get(name) {
                    if path.is_empty() {
                        return Some(replacement.clone());
                    }
                    // Field access on a substituted local record cannot be
                    // lowered into a scalar expression unless it was a literal.
                    return None;
                }
                if inlined {
                    // Unbound local from an inlined component body is not in
                    // scope at the call site.
                    return None;
                }
                ExprKind::Binding {
                    binding: binding.clone(),
                }
            }
            _ => ExprKind::Binding {
                binding: binding.clone(),
            },
        },
        ExprKind::Variant { variant, fields } => {
            let mut next = Vec::with_capacity(fields.len());
            for field in fields {
                next.push(subst_expr_inner(field, subst, unloaded, inlined)?);
            }
            ExprKind::Variant {
                variant: *variant,
                fields: next,
            }
        }
        ExprKind::Not { expr: inner } => ExprKind::Not {
            expr: Box::new(subst_expr_inner(inner, subst, unloaded, inlined)?),
        },
        ExprKind::Neg { expr: inner } => ExprKind::Neg {
            expr: Box::new(subst_expr_inner(inner, subst, unloaded, inlined)?),
        },
        ExprKind::Binary { op, lhs, rhs } => {
            let l = subst_expr_inner(lhs, subst, unloaded, inlined);
            let r = subst_expr_inner(rhs, subst, unloaded, inlined);
            match (op, l, r) {
                // Short-circuit boolean folding even when the other side is unloaded.
                (BinaryOp::And, Some(e), _) | (BinaryOp::And, _, Some(e))
                    if matches!(
                        fold_expr(e.clone()).kind,
                        ExprKind::Literal {
                            value: Value::Bool(false)
                        }
                    ) =>
                {
                    ExprKind::Literal {
                        value: Value::Bool(false),
                    }
                }
                (BinaryOp::Or, Some(e), _) | (BinaryOp::Or, _, Some(e))
                    if matches!(
                        fold_expr(e.clone()).kind,
                        ExprKind::Literal {
                            value: Value::Bool(true)
                        }
                    ) =>
                {
                    ExprKind::Literal {
                        value: Value::Bool(true),
                    }
                }
                (_, Some(l), Some(r)) => ExprKind::Binary {
                    op: *op,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
                _ => return None,
            }
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            let c = fold_expr(subst_expr_inner(cond, subst, unloaded, inlined)?);
            if let ExprKind::Literal {
                value: Value::Bool(b),
            } = c.kind
            {
                return subst_expr_inner(
                    if b { then } else { otherwise },
                    subst,
                    unloaded,
                    inlined,
                );
            }
            let t = subst_expr_inner(then, subst, unloaded, inlined)?;
            let o = subst_expr_inner(otherwise, subst, unloaded, inlined)?;
            ExprKind::If {
                cond: Box::new(c),
                then: Box::new(t),
                otherwise: Box::new(o),
            }
        }
    };
    Some(Expr {
        kind,
        ty: expr.ty.clone(),
    })
}

fn fold_expr(expr: Expr) -> Expr {
    let ty = expr.ty.clone();
    match expr.kind {
        ExprKind::Not { expr: inner } => {
            let inner = fold_expr(*inner);
            match inner.kind {
                ExprKind::Literal {
                    value: Value::Bool(value),
                } => Expr {
                    kind: ExprKind::Literal {
                        value: Value::Bool(!value),
                    },
                    ty,
                },
                _ => Expr {
                    kind: ExprKind::Not {
                        expr: Box::new(inner),
                    },
                    ty,
                },
            }
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let lhs = fold_expr(*lhs);
            let rhs = fold_expr(*rhs);
            match (op, &lhs.kind, &rhs.kind) {
                (
                    BinaryOp::And,
                    ExprKind::Literal {
                        value: Value::Bool(false),
                    },
                    _,
                )
                | (
                    BinaryOp::And,
                    _,
                    ExprKind::Literal {
                        value: Value::Bool(false),
                    },
                ) => Expr {
                    kind: ExprKind::Literal {
                        value: Value::Bool(false),
                    },
                    ty,
                },
                (
                    BinaryOp::And,
                    ExprKind::Literal {
                        value: Value::Bool(true),
                    },
                    _,
                ) => rhs,
                (
                    BinaryOp::And,
                    _,
                    ExprKind::Literal {
                        value: Value::Bool(true),
                    },
                ) => lhs,
                (
                    BinaryOp::Or,
                    ExprKind::Literal {
                        value: Value::Bool(true),
                    },
                    _,
                )
                | (
                    BinaryOp::Or,
                    _,
                    ExprKind::Literal {
                        value: Value::Bool(true),
                    },
                ) => Expr {
                    kind: ExprKind::Literal {
                        value: Value::Bool(true),
                    },
                    ty,
                },
                (
                    BinaryOp::Or,
                    ExprKind::Literal {
                        value: Value::Bool(false),
                    },
                    _,
                ) => rhs,
                (
                    BinaryOp::Or,
                    _,
                    ExprKind::Literal {
                        value: Value::Bool(false),
                    },
                ) => lhs,
                (BinaryOp::Eq, ExprKind::Literal { value: a }, ExprKind::Literal { value: b }) => {
                    Expr {
                        kind: ExprKind::Literal {
                            value: Value::Bool(a == b),
                        },
                        ty,
                    }
                }
                (BinaryOp::Ne, ExprKind::Literal { value: a }, ExprKind::Literal { value: b }) => {
                    Expr {
                        kind: ExprKind::Literal {
                            value: Value::Bool(a != b),
                        },
                        ty,
                    }
                }
                _ => Expr {
                    kind: ExprKind::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty,
                },
            }
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            let cond = fold_expr(*cond);
            match cond.kind {
                ExprKind::Literal {
                    value: Value::Bool(true),
                } => fold_expr(*then),
                ExprKind::Literal {
                    value: Value::Bool(false),
                } => fold_expr(*otherwise),
                _ => Expr {
                    kind: ExprKind::If {
                        cond: Box::new(cond),
                        then: Box::new(fold_expr(*then)),
                        otherwise: Box::new(fold_expr(*otherwise)),
                    },
                    ty,
                },
            }
        }
        other => Expr { kind: other, ty },
    }
}

fn project_nodes(nodes: &[Node], ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    let mut out = Vec::new();
    for node in nodes {
        out.extend(project_node(node, ctx));
    }
    out
}

fn project_node(node: &Node, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    match node {
        Node::Element(element) => project_element(element, ctx),
        Node::Component(call) => {
            if ctx.depth >= 12 {
                return Vec::new();
            }
            let Some(def) = ctx.components.get(&call.component) else {
                return Vec::new();
            };
            let mut child_subst = BTreeMap::new();
            let mut child_unloaded = BTreeSet::new();
            for (name, arg) in &call.args {
                match arg {
                    ComponentArg::Value { expr } => match ctx.substitute_expr(expr) {
                        Some(resolved) => {
                            child_subst.insert(name.clone(), resolved);
                        }
                        None => {
                            child_unloaded.insert(name.clone());
                        }
                    },
                    ComponentArg::Client | ComponentArg::Windows | ComponentArg::Players => {}
                }
            }
            let mut child_ctx = ProjectCtx {
                components: ctx.components,
                depth: ctx.depth + 1,
                repeat_count: ctx.repeat_count,
                row_index: ctx.row_index,
                text_slot: ctx.text_slot,
                column_width: ctx.column_width,
                in_row: ctx.in_row,
                parent_kind: ctx.parent_kind.clone(),
                unloaded_locals: child_unloaded,
                subst: child_subst,
                inlined: true,
                next_id: ctx.next_id,
            };
            let result = project_node(&def.body, &mut child_ctx);
            ctx.next_id = child_ctx.next_id;
            ctx.text_slot = child_ctx.text_slot;
            result
        }
        Node::If(if_node) => project_if(if_node, ctx),
        Node::IfLet(if_let) => {
            // In skeleton mode, an `if let Some(x) = ...` represents data that
            // will arrive once loading finishes: project the `then` branch with
            // `var` marked unloaded.
            let mut child_ctx = ctx.clone();
            child_ctx.unloaded_locals.insert(if_let.var.clone());
            let result = project_nodes(&if_let.then, &mut child_ctx);
            ctx.next_id = child_ctx.next_id;
            ctx.text_slot = child_ctx.text_slot;
            result
        }
        Node::Match(match_node) => project_match(match_node, ctx),
    }
}

fn project_if(if_node: &IfNode, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    let mut runtime_branches: Vec<(Expr, Vec<Node>)> = Vec::new();
    let mut static_fallback: Option<&[Node]> = None;
    let base_text_slot = ctx.text_slot;
    let mut max_text_slot = base_text_slot;

    for (cond, body) in &if_node.branches {
        match ctx.substitute_expr(cond) {
            Some(Expr {
                kind:
                    ExprKind::Literal {
                        value: Value::Bool(true),
                    },
                ..
            }) => {
                if runtime_branches.is_empty() {
                    return project_nodes(body, ctx);
                }
                let mut branch_ctx = ctx.clone();
                branch_ctx.text_slot = base_text_slot;
                let projected = project_nodes(body, &mut branch_ctx);
                ctx.next_id = branch_ctx.next_id;
                ctx.text_slot = max_text_slot.max(branch_ctx.text_slot);
                return vec![Node::If(IfNode {
                    branches: runtime_branches,
                    otherwise: projected,
                })];
            }
            Some(Expr {
                kind:
                    ExprKind::Literal {
                        value: Value::Bool(false),
                    },
                ..
            }) => continue,
            Some(expr) => {
                let mut branch_ctx = ctx.clone();
                branch_ctx.text_slot = base_text_slot;
                let projected = project_nodes(body, &mut branch_ctx);
                ctx.next_id = branch_ctx.next_id;
                max_text_slot = max_text_slot.max(branch_ctx.text_slot);
                runtime_branches.push((expr, projected));
            }
            None => {
                // Condition depends on unloaded data (e.g. `song.id == current_id`,
                // `song.downloaded`, `word.note != ""`). Defer selection below.
                if static_fallback.is_none() && !is_minor_badge_branch(body) {
                    static_fallback = Some(body);
                }
            }
        }
    }

    if !runtime_branches.is_empty() {
        let mut otherwise_ctx = ctx.clone();
        otherwise_ctx.text_slot = base_text_slot;
        let otherwise = project_nodes(&if_node.otherwise, &mut otherwise_ctx);
        ctx.next_id = otherwise_ctx.next_id;
        ctx.text_slot = max_text_slot.max(otherwise_ctx.text_slot);
        // When every runtime branch and the fallback project to the exact same
        // skeleton geometry (e.g. different context menus per item kind), drop
        // the branch and emit the shared skeleton once.
        if !if_node.otherwise.is_empty()
            && runtime_branches
                .iter()
                .all(|(_, body)| nodes_equivalent(body, &otherwise))
        {
            return otherwise;
        }
        return vec![Node::If(IfNode {
            branches: runtime_branches,
            otherwise,
        })];
    }

    // All branches depended on unloaded data: prefer the `else` branch when it
    // has real content (e.g. normal non-highlighted song row vs active row),
    // otherwise use the first substantive `then` branch.
    if !if_node.otherwise.is_empty() && !is_minor_badge_branch(&if_node.otherwise) {
        return project_nodes(&if_node.otherwise, ctx);
    }
    if let Some(body) = static_fallback {
        return project_nodes(body, ctx);
    }
    Vec::new()
}

fn project_match(match_node: &MatchNode, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    if let Some(value) = ctx.substitute_expr(&match_node.value) {
        // Scrutinee is available during loading (e.g. `match search.kind`):
        // preserve the match and project each arm independently!
        let arms = match_node
            .arms
            .iter()
            .map(|arm| {
                let mut arm_ctx = ctx.clone();
                for binding in &arm.bindings {
                    arm_ctx.unloaded_locals.insert(binding.name.clone());
                }
                let body = project_nodes(&arm.body, &mut arm_ctx);
                ctx.next_id = arm_ctx.next_id;
                MatchArm {
                    variant: arm.variant,
                    bindings: Vec::new(),
                    body,
                }
            })
            .collect();
        return vec![Node::Match(MatchNode {
            value,
            enum_name: match_node.enum_name.clone(),
            arms,
        })];
    }

    // Scrutinee depends on unloaded data: pick the richest non-empty arm.
    if let Some(best) = match_node
        .arms
        .iter()
        .max_by_key(|arm| node_richness(&arm.body))
        && !best.body.is_empty()
    {
        let mut arm_ctx = ctx.clone();
        for binding in &best.bindings {
            arm_ctx.unloaded_locals.insert(binding.name.clone());
        }
        let result = project_nodes(&best.body, &mut arm_ctx);
        ctx.next_id = arm_ctx.next_id;
        return result;
    }
    Vec::new()
}

/// Heuristic: a single tiny icon badge (such as a downloaded/local indicator)
/// should not clutter every skeleton row.
fn is_minor_badge_branch(nodes: &[Node]) -> bool {
    if nodes.is_empty() {
        return true;
    }
    nodes.iter().all(|node| match node {
        Node::Element(e) if e.kind == "Label" && e.keyword("display") == Some("icon") => true,
        Node::Element(e) if e.kind == "Stack" => is_minor_badge_branch(&e.children),
        _ => false,
    })
}

fn node_richness(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element(e) => {
                let bonus = if matches!(
                    e.kind.as_str(),
                    "ForEach" | "List" | "Grid" | "Table" | "Image" | "Video"
                ) {
                    10
                } else {
                    1
                };
                bonus + node_richness(&e.children)
            }
            Node::Component(_) => 5,
            Node::If(n) => {
                n.branches
                    .iter()
                    .map(|(_, b)| node_richness(b))
                    .sum::<usize>()
                    + node_richness(&n.otherwise)
            }
            Node::IfLet(n) => node_richness(&n.then) + node_richness(&n.otherwise),
            Node::Match(n) => n.arms.iter().map(|a| node_richness(&a.body)).sum(),
        })
        .sum()
}

fn project_element(element: &Element, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    match element.kind.as_str() {
        // Non-visual, event, gesture, overlay, or side-effect elements vanish.
        "ContextMenu" | "TableMenu" | "DoubleTap" | "WheelUp" | "WheelDown" | "Pinch" | "Swipe"
        | "OnCommit" | "Drop" | "Shortcut" | "Watch" | "Timer" | "ScreenBrightness" | "Toast"
        | "Modal" | "MenuItem" | "PickerOption" | "SearchItem" | "CommandMenu" | "CommandItem"
        | "QueryStatusHeader" => Vec::new(),

        // Already a manual Skeleton: keep as-is (with safe prop substitution).
        "Skeleton" => vec![Node::Element(clone_skeleton_props(element, ctx))],

        // Structural spacers / dividers stay verbatim.
        "Spacer" | "Divider" | "WindowButtons" => vec![Node::Element(Element {
            kind: element.kind.clone(),
            id: None,
            path: ctx.fresh_path(&element.path, "keep"),
            props: BTreeMap::new(),
            template: None,
            children: Vec::new(),
            action: None,
            source: element.source.clone(),
            skeleton: None,
        })],

        // Wrapper buttons/links/regions: drop interactivity, project inner content!
        "Button" | "NavLink" | "FocusRegion" => {
            if element.has_content() {
                let visual_children: Vec<&Node> = element
                    .children
                    .iter()
                    .filter(|child| {
                        !matches!(
                            child,
                            Node::Element(e)
                                if e.kind == "ContextMenu"
                                    || e.kind == "OnCommit"
                                    || spec::POINTER_EVENTS.contains(&e.kind.as_str())
                        )
                    })
                    .collect();
                let owned: Vec<Node> = visual_children.into_iter().cloned().collect();
                project_nodes(&owned, ctx)
            } else {
                vec![project_plain_button(element, ctx)]
            }
        }

        // Repetition containers: expand into N staggered sample rows/cards.
        "ForEach" => project_for_each(element, ctx),
        "List" => project_list(element, ctx, DEFAULT_LIST_COUNT),
        "PreviewList" => project_list(element, ctx, DEFAULT_PREVIEW_COUNT),
        "Table" => project_table(element, ctx),

        // Layout containers: keep geometry/spacing/variants, strip interactivity,
        // and add a gentle stagger when directly wrapping a `ForEach`.
        "Column" | "Row" | "Stack" | "Grid" | "Flow" | "ScrollView" | "Section" | "Form"
        | "ScaleBox" | "Positioned" | "Rail" | "Adaptive" | "Layout" | "Accordion" | "Marquee" => {
            project_container(element, ctx)
        }

        // Leaf content controls.
        "Text" => vec![project_text(element, ctx)],
        "Label" => vec![project_label(element, ctx)],
        "Image" | "Video" | "Svg" | "Canvas" => vec![project_media(element, ctx)],
        "IconButton" => vec![project_icon_button(element, ctx)],
        "Switch" => vec![make_skeleton_node(
            ctx,
            &element.path,
            &element.source,
            Some(Prop::Number { value: 44.0 }),
            Some(Prop::Number { value: 24.0 }),
            None,
            None,
            None,
            None,
            "capsule",
            "media",
        )],
        "Slider" => vec![make_skeleton_node(
            ctx,
            &element.path,
            &element.source,
            None,
            Some(Prop::Number { value: 8.0 }),
            Some(1.0),
            None,
            None,
            None,
            "capsule",
            "media",
        )],
        "Progress" => {
            let is_ring = element.keyword("style") == Some("ring");
            let height = if is_ring {
                element
                    .prop("width")
                    .and_then(|p| substitute_prop(p, ctx))
                    .unwrap_or(Prop::Number { value: 24.0 })
            } else {
                Prop::Number { value: 6.0 }
            };
            vec![make_skeleton_node(
                ctx,
                &element.path,
                &element.source,
                is_ring.then(|| height.clone()),
                Some(height),
                (!is_ring).then_some(1.0),
                None,
                None,
                None,
                if is_ring { "circle" } else { "capsule" },
                "media",
            )]
        }
        "TextInput" | "Search" | "Picker" | "LanguagePicker" | "ColorPicker" | "ShortcutInput"
        | "Menu" | "SplitButton" | "PopoverButton" => {
            let height = element
                .prop("height")
                .and_then(|p| substitute_prop(p, ctx))
                .unwrap_or(Prop::Number { value: 32.0 });
            let shape = if element.keyword("shape") == Some("capsule") {
                "capsule"
            } else {
                "rect"
            };
            let frac = if element.kind == "TextInput" || element.kind == "Search" {
                1.0
            } else {
                0.42
            };
            vec![make_skeleton_node(
                ctx,
                &element.path,
                &element.source,
                None,
                Some(height),
                Some(frac),
                Some(Prop::Number { value: 220.0 }),
                None,
                Some(Prop::Number { value: 8.0 }),
                shape,
                "media",
            )]
        }

        // Nested Query or other composite container: project its visual children.
        "Query" => {
            let (_, body) = element.query_children();
            let mut child_ctx = ctx.clone();
            if let Some(template) = &element.template {
                child_ctx.unloaded_locals.insert(template.var.clone());
            }
            let result = project_nodes(body, &mut child_ctx);
            ctx.next_id = child_ctx.next_id;
            result
        }
        _ => {
            if element.children.is_empty() {
                vec![make_skeleton_node(
                    ctx,
                    &element.path,
                    &element.source,
                    None,
                    Some(Prop::Number { value: 24.0 }),
                    Some(0.6),
                    Some(Prop::Number { value: 200.0 }),
                    None,
                    Some(Prop::Number { value: 8.0 }),
                    "rect",
                    "media",
                )]
            } else {
                project_nodes(&element.children, ctx)
            }
        }
    }
}

fn project_container(element: &Element, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    // Collapse cover placeholder + Image stacks into a single clean media skeleton
    // so two placeholder layers do not double-stack on top of each other.
    if element.kind == "Stack"
        && stack_is_image_overlay(element)
        && let Some(img) = find_overlay_media(&element.children)
    {
        return vec![project_media_with_fallback(img, element, ctx)];
    }

    let mut props = BTreeMap::new();
    const KEEP: &[&str] = &[
        "gap",
        "row_gap",
        "padding",
        "padding_x",
        "padding_y",
        "width",
        "height",
        "align",
        "min_width",
        "cell_ratio",
        "cell_extra",
        "masonry",
        "corner",
        "variant",
        "axis",
        "size",
        "at",
        "from",
        "to",
        "under_bars",
        "appearance",
        "style",
        "stagger",
        "appear",
    ];
    for key in KEEP {
        if let Some(prop) = element.props.get(*key)
            && let Some(clean) = substitute_prop(prop, ctx)
        {
            props.insert((*key).to_string(), clean);
        }
    }

    let has_direct_for_each = element
        .children
        .iter()
        .any(|child| matches!(child, Node::Element(e) if e.kind == "ForEach"));
    if has_direct_for_each
        && spec::STAGGERING.contains(&element.kind.as_str())
        && !props.contains_key("stagger")
    {
        props.insert(
            "stagger".into(),
            Prop::Number {
                value: DEFAULT_STAGGER,
            },
        );
    }

    let mut child_ctx = ctx.clone();
    child_ctx.parent_kind = Some(element.kind.clone());
    if element.kind == "Row" {
        child_ctx.in_row = true;
    } else if matches!(element.kind.as_str(), "Column" | "Grid" | "Stack") {
        child_ctx.in_row = false;
    }
    if element.kind == "Column" {
        if let Some(w) = element.props.get("width").and_then(estimated_length) {
            child_ctx.column_width = Some(w);
        }
    } else if matches!(element.kind.as_str(), "Grid" | "ScrollView" | "Form") {
        child_ctx.column_width = None;
    }

    let children = project_nodes(&element.children, &mut child_ctx);
    ctx.next_id = child_ctx.next_id;
    ctx.text_slot = child_ctx.text_slot;

    if children.is_empty() && !props.contains_key("width") && !props.contains_key("height") {
        return Vec::new();
    }

    // Drop a plain wrapper `Row` whose optional icon badges were pruned down
    // to a single placeholder child, so the child sits directly in its parent
    // `Column` without an extra shrink-to-fit row wrapper.
    if element.kind == "Row"
        && element.children.len() > 1
        && children.len() == 1
        && !props.contains_key("width")
        && !props.contains_key("height")
        && !props.contains_key("padding")
        && !props.contains_key("padding_x")
        && !props.contains_key("padding_y")
        && !props.contains_key("corner")
        && !props.contains_key("variant")
        && !props.contains_key("appear")
    {
        return children;
    }

    vec![Node::Element(Element {
        kind: element.kind.clone(),
        id: None,
        path: ctx.fresh_path(&element.path, "box"),
        props,
        template: None,
        children,
        action: None,
        source: element.source.clone(),
        skeleton: None,
    })]
}

fn find_overlay_media(nodes: &[Node]) -> Option<&Element> {
    for node in nodes {
        match node {
            Node::Element(e) if matches!(e.kind.as_str(), "Image" | "Video") => return Some(e),
            Node::If(n) => {
                for (_, body) in &n.branches {
                    if let Some(found) = find_overlay_media(body) {
                        return Some(found);
                    }
                }
                if let Some(found) = find_overlay_media(&n.otherwise) {
                    return Some(found);
                }
            }
            Node::IfLet(n) => {
                if let Some(found) = find_overlay_media(&n.then) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_overlay_layer(node: &Node) -> bool {
    match node {
        Node::Element(e) => matches!(
            e.kind.as_str(),
            "Image" | "Video" | "Skeleton" | "Canvas" | "ContextMenu" | "DoubleTap"
        ),
        Node::Component(call) => call.component.contains("placeholder"),
        Node::If(n) => {
            n.branches
                .iter()
                .all(|(_, b)| b.iter().all(is_overlay_layer))
                && n.otherwise.iter().all(is_overlay_layer)
        }
        Node::IfLet(n) => {
            n.then.iter().all(is_overlay_layer) && n.otherwise.iter().all(is_overlay_layer)
        }
        _ => false,
    }
}

fn stack_is_image_overlay(stack: &Element) -> bool {
    find_overlay_media(&stack.children).is_some() && stack.children.iter().all(is_overlay_layer)
}

/// Extract a representative numeric length from a literal or conditional
/// length prop (picking the smallest branch so narrow columns stay compact).
fn estimated_length(prop: &Prop) -> Option<f64> {
    match prop {
        Prop::Number { value } => Some(*value),
        Prop::Expr { expr } => estimated_expr_number(expr),
        _ => None,
    }
}

fn estimated_expr_number(expr: &Expr) -> Option<f64> {
    match &expr.kind {
        ExprKind::Literal {
            value: Value::Float(v),
        } => Some(*v),
        ExprKind::Literal {
            value: Value::Int(v),
        } => Some(*v as f64),
        ExprKind::If {
            then, otherwise, ..
        } => match (
            estimated_expr_number(then),
            estimated_expr_number(otherwise),
        ) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        },
        _ => None,
    }
}

fn nodes_equivalent(a: &[Node], b: &[Node]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| node_equivalent(x, y))
}

fn node_equivalent(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Element(x), Node::Element(y)) => {
            x.kind == y.kind
                && x.id == y.id
                && x.props == y.props
                && x.template == y.template
                && x.action == y.action
                && nodes_equivalent(&x.children, &y.children)
        }
        (Node::If(x), Node::If(y)) => {
            x.branches.len() == y.branches.len()
                && x.branches
                    .iter()
                    .zip(&y.branches)
                    .all(|((cx, bx), (cy, by))| cx == cy && nodes_equivalent(bx, by))
                && nodes_equivalent(&x.otherwise, &y.otherwise)
        }
        (Node::IfLet(x), Node::IfLet(y)) => {
            x.var == y.var
                && x.value == y.value
                && nodes_equivalent(&x.then, &y.then)
                && nodes_equivalent(&x.otherwise, &y.otherwise)
        }
        (Node::Match(x), Node::Match(y)) => {
            x.value == y.value
                && x.enum_name == y.enum_name
                && x.arms.len() == y.arms.len()
                && x.arms.iter().zip(&y.arms).all(|(ax, ay)| {
                    ax.variant == ay.variant && nodes_equivalent(&ax.body, &ay.body)
                })
        }
        (Node::Component(x), Node::Component(y)) => x.component == y.component && x.args == y.args,
        _ => false,
    }
}

fn project_for_each(element: &Element, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    let default_count = match ctx.parent_kind.as_deref() {
        Some("Grid") => DEFAULT_GRID_COUNT,
        Some("PreviewList") => DEFAULT_PREVIEW_COUNT,
        _ => DEFAULT_LIST_COUNT,
    };
    let count = ctx.repeat_count.unwrap_or(default_count);
    let mut rows = Vec::with_capacity(count);
    for i in 0..count {
        let mut item_ctx = ctx.clone();
        item_ctx.row_index = i;
        item_ctx.text_slot = 0;
        if let Some(template) = &element.template {
            item_ctx.unloaded_locals.insert(template.var.clone());
        }
        let mut projected = project_nodes(&element.children, &mut item_ctx);
        ctx.next_id = item_ctx.next_id;
        attach_rise_appear(&mut projected);
        rows.extend(projected);
    }
    rows
}

fn project_list(element: &Element, ctx: &mut ProjectCtx<'_>, default_count: usize) -> Vec<Node> {
    let count = ctx.repeat_count.unwrap_or(default_count);
    let mut rows = Vec::with_capacity(count);
    for i in 0..count {
        let mut item_ctx = ctx.clone();
        item_ctx.row_index = i;
        item_ctx.text_slot = 0;
        item_ctx.parent_kind = Some("Column".into());
        if let Some(template) = &element.template {
            item_ctx.unloaded_locals.insert(template.var.clone());
        }
        let mut projected = project_nodes(&element.children, &mut item_ctx);
        ctx.next_id = item_ctx.next_id;
        attach_rise_appear(&mut projected);
        rows.extend(projected);
    }
    let mut props = BTreeMap::new();
    props.insert("gap".into(), Prop::Number { value: 4.0 });
    props.insert(
        "stagger".into(),
        Prop::Number {
            value: DEFAULT_STAGGER,
        },
    );
    vec![Node::Element(Element {
        kind: "Column".into(),
        id: None,
        path: ctx.fresh_path(&element.path, "list"),
        props,
        template: None,
        children: rows,
        action: None,
        source: element.source.clone(),
        skeleton: None,
    })]
}

fn project_table(element: &Element, ctx: &mut ProjectCtx<'_>) -> Vec<Node> {
    let count = ctx.repeat_count.unwrap_or(DEFAULT_LIST_COUNT);
    let columns: Vec<&Element> = element
        .children
        .iter()
        .filter_map(|child| match child {
            Node::Element(col) if col.kind == "TableColumn" => Some(col),
            _ => None,
        })
        .collect();
    let col_count = columns.len().max(1);
    let mut rows = Vec::with_capacity(count);
    for i in 0..count {
        let mut cells = Vec::with_capacity(col_count);
        for c in 0..col_count {
            let tone = if c == 0 { "primary" } else { "secondary" };
            let fracs = if c == 0 {
                &PRIMARY_FRACTIONS
            } else {
                &SECONDARY_FRACTIONS
            };
            let frac = fracs[(i + c) % fracs.len()];
            cells.push(make_skeleton_node(
                ctx,
                &element.path,
                &element.source,
                None,
                Some(Prop::Number {
                    value: if c == 0 { 13.0 } else { 11.0 },
                }),
                Some(frac),
                Some(Prop::Number {
                    value: if c == 0 { 220.0 } else { 140.0 },
                }),
                None,
                None,
                "capsule",
                tone,
            ));
            if c + 1 < col_count {
                cells.push(Node::Element(Element {
                    kind: "Spacer".into(),
                    id: None,
                    path: ctx.fresh_path(&element.path, "sp"),
                    props: BTreeMap::new(),
                    template: None,
                    children: Vec::new(),
                    action: None,
                    source: element.source.clone(),
                    skeleton: None,
                }));
            }
        }
        let mut row_props = BTreeMap::new();
        row_props.insert("gap".into(), Prop::Number { value: 12.0 });
        row_props.insert("padding_x".into(), Prop::Number { value: 12.0 });
        row_props.insert("padding_y".into(), Prop::Number { value: 9.0 });
        row_props.insert(
            "align".into(),
            Prop::Keyword {
                value: "center".into(),
            },
        );
        row_props.insert(
            "appear".into(),
            Prop::Keyword {
                value: "slide".into(),
            },
        );
        rows.push(Node::Element(Element {
            kind: "Row".into(),
            id: None,
            path: ctx.fresh_path(&element.path, "trow"),
            props: row_props,
            template: None,
            children: cells,
            action: None,
            source: element.source.clone(),
            skeleton: None,
        }));
    }
    let mut col_props = BTreeMap::new();
    col_props.insert("gap".into(), Prop::Number { value: 2.0 });
    col_props.insert(
        "stagger".into(),
        Prop::Number {
            value: DEFAULT_STAGGER,
        },
    );
    vec![Node::Element(Element {
        kind: "Column".into(),
        id: None,
        path: ctx.fresh_path(&element.path, "table"),
        props: col_props,
        template: None,
        children: rows,
        action: None,
        source: element.source.clone(),
        skeleton: None,
    })]
}

fn attach_rise_appear(nodes: &mut [Node]) {
    for node in nodes {
        if let Node::Element(element) = node
            && spec::component(&element.kind).is_some_and(|s| s.moves())
            && !element.props.contains_key("appear")
        {
            element.props.insert(
                "appear".into(),
                Prop::Keyword {
                    value: "slide".into(),
                },
            );
        }
    }
}

fn project_text(element: &Element, ctx: &mut ProjectCtx<'_>) -> Node {
    let style = element.keyword("style").unwrap_or("body");
    let emphasis = element.keyword("emphasis");
    let is_secondary = matches!(emphasis, Some("secondary" | "tertiary"))
        || matches!(style, "caption" | "caption2" | "footnote");
    let tone = if is_secondary { "secondary" } else { "primary" };

    let bar_height = if let Some(fs) = element.number("font_size") {
        (fs * 0.74).clamp(8.0, 32.0)
    } else {
        match style {
            "large_title" => 26.0,
            "title" => 20.0,
            "title2" => 17.0,
            "title3" => 15.0,
            "headline" => 13.0,
            "body" => 12.0,
            "callout" | "subheadline" => 11.0,
            "footnote" => 10.0,
            "caption" | "caption2" => 10.0,
            _ => 12.0,
        }
    };

    let slot = ctx.text_slot;
    ctx.text_slot += 1;
    let idx = (ctx.row_index + slot) % PRIMARY_FRACTIONS.len();

    let (fraction, max_width) = if let Some(col_w) = ctx.column_width {
        if col_w <= 80.0 {
            (0.72, col_w)
        } else {
            let f = if is_secondary {
                SECONDARY_FRACTIONS[idx]
            } else {
                PRIMARY_FRACTIONS[idx]
            };
            (f, col_w)
        }
    } else if is_secondary {
        (SECONDARY_FRACTIONS[idx], SECONDARY_MAX_WIDTH)
    } else {
        (PRIMARY_FRACTIONS[idx], PRIMARY_MAX_WIDTH)
    };

    let lines = element.number("lines").map(|v| v.round() as usize);
    let multiline = matches!(lines, Some(n) if n >= 2)
        && !matches!(
            style,
            "large_title" | "title" | "title2" | "title3" | "headline"
        )
        && ctx.column_width.is_none_or(|w| w > 120.0);

    if multiline {
        let mut col_props = BTreeMap::new();
        col_props.insert("gap".into(), Prop::Number { value: 6.0 });
        col_props.insert(
            "align".into(),
            Prop::Keyword {
                value: "leading".into(),
            },
        );
        let first = make_skeleton_node(
            ctx,
            &element.path,
            &element.source,
            None,
            Some(Prop::Number { value: bar_height }),
            Some(0.92),
            Some(Prop::Number {
                value: max_width.max(240.0),
            }),
            None,
            None,
            "capsule",
            tone,
        );
        let second = make_skeleton_node(
            ctx,
            &element.path,
            &element.source,
            None,
            Some(Prop::Number { value: bar_height }),
            Some(0.62),
            Some(Prop::Number {
                value: max_width.max(240.0),
            }),
            None,
            None,
            "capsule",
            tone,
        );
        return Node::Element(Element {
            kind: "Column".into(),
            id: None,
            path: ctx.fresh_path(&element.path, "lines"),
            props: col_props,
            template: None,
            children: vec![first, second],
            action: None,
            source: element.source.clone(),
            skeleton: None,
        });
    }

    make_skeleton_node(
        ctx,
        &element.path,
        &element.source,
        None,
        Some(Prop::Number { value: bar_height }),
        Some(fraction),
        Some(Prop::Number { value: max_width }),
        None,
        None,
        "capsule",
        tone,
    )
}

fn project_label(element: &Element, ctx: &mut ProjectCtx<'_>) -> Node {
    let icon_side = element
        .number("font_size")
        .unwrap_or(14.0)
        .clamp(10.0, 48.0);
    if element.keyword("display") == Some("icon") {
        return make_skeleton_node(
            ctx,
            &element.path,
            &element.source,
            Some(Prop::Number { value: icon_side }),
            Some(Prop::Number { value: icon_side }),
            None,
            None,
            None,
            None,
            "circle",
            "secondary",
        );
    }
    let mut row_props = BTreeMap::new();
    row_props.insert("gap".into(), Prop::Number { value: 6.0 });
    row_props.insert(
        "align".into(),
        Prop::Keyword {
            value: "center".into(),
        },
    );
    let dot = make_skeleton_node(
        ctx,
        &element.path,
        &element.source,
        Some(Prop::Number { value: icon_side }),
        Some(Prop::Number { value: icon_side }),
        None,
        None,
        None,
        None,
        "circle",
        "secondary",
    );
    let text = project_text(element, ctx);
    Node::Element(Element {
        kind: "Row".into(),
        id: None,
        path: ctx.fresh_path(&element.path, "label"),
        props: row_props,
        template: None,
        children: vec![dot, text],
        action: None,
        source: element.source.clone(),
        skeleton: None,
    })
}

fn project_media(element: &Element, ctx: &mut ProjectCtx<'_>) -> Node {
    project_media_with_fallback(element, element, ctx)
}

fn project_media_with_fallback(
    element: &Element,
    fallback_size: &Element,
    ctx: &mut ProjectCtx<'_>,
) -> Node {
    let width = element
        .prop("width")
        .or_else(|| fallback_size.prop("width"))
        .and_then(|p| substitute_prop(p, ctx));
    let height = element
        .prop("height")
        .or_else(|| fallback_size.prop("height"))
        .and_then(|p| substitute_prop(p, ctx));
    let aspect = element.number("aspect");
    let corner = element
        .prop("corner")
        .or_else(|| fallback_size.prop("corner"))
        .and_then(|p| substitute_prop(p, ctx))
        .unwrap_or(Prop::Number { value: 6.0 });

    // Detect circular avatars (`corner >= width / 2`, or a large corner on a
    // square aspect-ratio image whose width comes from its container).
    let is_circle = match (&width, &corner) {
        (Some(Prop::Number { value: w }), Prop::Number { value: c })
            if *w > 0.0 && *c >= *w * 0.48 =>
        {
            true
        }
        (None, Prop::Number { value: c })
            if aspect.is_some_and(|a| (a - 1.0).abs() < 0.01) && *c >= 200.0 =>
        {
            true
        }
        _ => false,
    };

    make_skeleton_node(
        ctx,
        &element.path,
        &element.source,
        width,
        height,
        None,
        None,
        aspect,
        if is_circle { None } else { Some(corner) },
        if is_circle { "circle" } else { "rect" },
        "media",
    )
}

fn project_plain_button(element: &Element, ctx: &mut ProjectCtx<'_>) -> Node {
    let height = element
        .prop("height")
        .and_then(|p| substitute_prop(p, ctx))
        .unwrap_or(Prop::Number { value: 32.0 });
    let fill = element.keyword("width") == Some("fill");
    let shape = if matches!(element.keyword("shape"), Some("capsule" | "chip")) {
        "capsule"
    } else {
        "rect"
    };
    make_skeleton_node(
        ctx,
        &element.path,
        &element.source,
        if fill {
            None
        } else {
            Some(Prop::Number { value: 92.0 })
        },
        Some(height),
        fill.then_some(1.0),
        None,
        None,
        Some(Prop::Number { value: 8.0 }),
        shape,
        "media",
    )
}

fn project_icon_button(element: &Element, ctx: &mut ProjectCtx<'_>) -> Node {
    let side = element
        .prop("height")
        .and_then(|p| substitute_prop(p, ctx))
        .unwrap_or(Prop::Number { value: 32.0 });
    make_skeleton_node(
        ctx,
        &element.path,
        &element.source,
        Some(side.clone()),
        Some(side),
        None,
        None,
        None,
        None,
        "circle",
        "media",
    )
}

#[allow(clippy::too_many_arguments)]
fn make_skeleton_node(
    ctx: &mut ProjectCtx<'_>,
    base_path: &str,
    source: &SourceLocation,
    width: Option<Prop>,
    height: Option<Prop>,
    fraction: Option<f64>,
    max_width: Option<Prop>,
    aspect: Option<f64>,
    corner: Option<Prop>,
    shape: &str,
    tone: &str,
) -> Node {
    let mut props = BTreeMap::new();
    if let Some(w) = width {
        props.insert("width".into(), w);
    } else {
        if let Some(f) = fraction {
            props.insert("fraction".into(), Prop::Number { value: f });
        }
        if let Some(mw) = max_width {
            props.insert("max_width".into(), mw);
        }
    }
    if let Some(h) = height {
        props.insert("height".into(), h);
    }
    if let Some(a) = aspect {
        props.insert("aspect".into(), Prop::Number { value: a });
    }
    if let Some(c) = corner {
        props.insert("corner".into(), c);
    }
    if shape != "rect" {
        props.insert(
            "shape".into(),
            Prop::Keyword {
                value: shape.into(),
            },
        );
    }
    props.insert("tone".into(), Prop::Keyword { value: tone.into() });
    Node::Element(Element {
        kind: "Skeleton".into(),
        id: None,
        path: ctx.fresh_path(base_path, "s"),
        props,
        template: None,
        children: Vec::new(),
        action: None,
        source: source.clone(),
        skeleton: None,
    })
}

fn clone_skeleton_props(element: &Element, ctx: &mut ProjectCtx<'_>) -> Element {
    let mut props = BTreeMap::new();
    for (k, v) in &element.props {
        if matches!(k.as_str(), "skeleton" | "skeleton_count") {
            continue;
        }
        if let Some(clean) = substitute_prop(v, ctx) {
            props.insert(k.clone(), clean);
        }
    }
    Element {
        kind: "Skeleton".into(),
        id: None,
        path: ctx.fresh_path(&element.path, "manual"),
        props,
        template: None,
        children: Vec::new(),
        action: None,
        source: element.source.clone(),
        skeleton: None,
    }
}

fn substitute_prop(prop: &Prop, ctx: &ProjectCtx<'_>) -> Option<Prop> {
    match prop {
        Prop::Expr { expr } => ctx.substitute_expr(expr).map(|expr| Prop::Expr { expr }),
        Prop::Keyword { .. } | Prop::Number { .. } => Some(prop.clone()),
        _ => None,
    }
}

fn default_fallback_skeleton(base_path: &str, source: &SourceLocation) -> Node {
    let mut col_props = BTreeMap::new();
    col_props.insert("gap".into(), Prop::Number { value: 10.0 });
    col_props.insert("padding".into(), Prop::Number { value: 16.0 });
    col_props.insert(
        "stagger".into(),
        Prop::Number {
            value: DEFAULT_STAGGER,
        },
    );
    let mut children = Vec::with_capacity(4);
    for (i, frac) in [0.76, 0.92, 0.64, 0.84].into_iter().enumerate() {
        let mut p = BTreeMap::new();
        p.insert("fraction".into(), Prop::Number { value: frac });
        p.insert("max_width".into(), Prop::Number { value: 280.0 });
        p.insert("height".into(), Prop::Number { value: 13.0 });
        p.insert(
            "shape".into(),
            Prop::Keyword {
                value: "capsule".into(),
            },
        );
        p.insert(
            "tone".into(),
            Prop::Keyword {
                value: if i == 0 { "primary" } else { "secondary" }.into(),
            },
        );
        p.insert(
            "appear".into(),
            Prop::Keyword {
                value: "slide".into(),
            },
        );
        children.push(Node::Element(Element {
            kind: "Skeleton".into(),
            id: None,
            path: format!("{base_path}/skel_fb_{i}"),
            props: p,
            template: None,
            children: Vec::new(),
            action: None,
            source: source.clone(),
            skeleton: None,
        }));
    }
    Node::Element(Element {
        kind: "Column".into(),
        id: None,
        path: format!("{base_path}/skel_fb"),
        props: col_props,
        template: None,
        children,
        action: None,
        source: source.clone(),
        skeleton: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, IfLetNode, LocalizedText, ParamDef, ParamKind, QueryBinding, Template};
    use bern_protocol::Type;

    fn src() -> SourceLocation {
        SourceLocation {
            file: "test.rs".into(),
            line: 1,
            column: 1,
        }
    }

    fn el(kind: &str, props: Vec<(&str, Prop)>, children: Vec<Node>) -> Element {
        Element {
            kind: kind.into(),
            id: None,
            path: format!("p.{kind}"),
            props: props.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            template: None,
            children,
            action: None,
            source: src(),
            skeleton: None,
        }
    }

    fn localized(s: &str) -> Prop {
        Prop::Text {
            text: crate::Text::Localized {
                text: LocalizedText {
                    key: s.into(),
                    translations: [("en".into(), s.into())].into(),
                    params: Vec::new(),
                },
                args: Vec::new(),
            },
        }
    }

    fn lower(mut root: Node, components: Vec<ComponentDef>) -> Element {
        let map: BTreeMap<String, ComponentDef> = components
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        lower_node(&mut root, &map);
        let Node::Element(e) = root else {
            panic!("expected Element root");
        };
        e
    }

    fn bool_engine(field: usize) -> Expr {
        Expr {
            kind: ExprKind::Binding {
                binding: Binding::Engine {
                    field,
                    path: Vec::new(),
                },
            },
            ty: Type::Bool,
        }
    }

    fn window_flag(field: &str) -> Expr {
        Expr {
            kind: ExprKind::Binding {
                binding: Binding::Window {
                    field: field.into(),
                },
            },
            ty: Type::Bool,
        }
    }

    fn collect_kinds<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
        for node in nodes {
            match node {
                Node::Element(e) => {
                    out.push(e.kind.as_str());
                    collect_kinds(&e.children, out);
                }
                Node::If(n) => {
                    for (_, b) in &n.branches {
                        collect_kinds(b, out);
                    }
                    collect_kinds(&n.otherwise, out);
                }
                Node::IfLet(n) => {
                    collect_kinds(&n.then, out);
                    collect_kinds(&n.otherwise, out);
                }
                Node::Match(n) => {
                    for a in &n.arms {
                        collect_kinds(&a.body, out);
                    }
                }
                Node::Component(_) => {}
            }
        }
    }

    fn assert_no_actions_or_ids(nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element(e) => {
                    assert!(e.id.is_none(), "skeleton leaked id on {}", e.kind);
                    assert!(e.action.is_none(), "skeleton leaked action on {}", e.kind);
                    assert_no_actions_or_ids(&e.children);
                }
                Node::If(n) => {
                    for (_, b) in &n.branches {
                        assert_no_actions_or_ids(b);
                    }
                    assert_no_actions_or_ids(&n.otherwise);
                }
                Node::IfLet(n) => {
                    assert_no_actions_or_ids(&n.then);
                    assert_no_actions_or_ids(&n.otherwise);
                }
                Node::Match(n) => {
                    for a in &n.arms {
                        assert_no_actions_or_ids(&a.body);
                    }
                }
                Node::Component(_) => panic!("skeleton left un-inlined ComponentRef"),
            }
        }
    }

    #[test]
    fn query_defaults_to_skeleton_unless_loading_or_spinner_set() {
        let make_query = |props: Vec<(&str, Prop)>| {
            let mut q = el(
                "Query",
                props,
                vec![Node::Element(el(
                    "Text",
                    vec![("content", localized("k"))],
                    vec![],
                ))],
            );
            q.props.insert(
                "query".into(),
                Prop::Query {
                    query: QueryBinding {
                        query: 0,
                        args: vec![],
                        result: Type::Text,
                    },
                },
            );
            q.template = Some(Template {
                var: "data".into(),
                ty: Type::Text,
            });
            q
        };

        let q = lower(Node::Element(make_query(vec![])), vec![]);
        assert!(q.skeleton.is_some());

        let q = lower(
            Node::Element(make_query(vec![("loading", localized("Loading..."))])),
            vec![],
        );
        assert!(q.skeleton.is_none());

        let q = lower(
            Node::Element(make_query(vec![
                ("loading", localized("Loading...")),
                (
                    "loading_style",
                    Prop::Keyword {
                        value: "skeleton".into(),
                    },
                ),
            ])),
            vec![],
        );
        assert!(q.skeleton.is_some());

        let q = lower(
            Node::Element(make_query(vec![(
                "loading_style",
                Prop::Keyword {
                    value: "spinner".into(),
                },
            )])),
            vec![],
        );
        assert!(q.skeleton.is_none());
    }

    #[test]
    fn grid_for_each_projects_staggered_cards_and_strips_interactivity() {
        let mut button = el(
            "Button",
            vec![],
            vec![Node::Element(el(
                "Column",
                vec![("gap", Prop::Number { value: 6.0 })],
                vec![
                    Node::Element(el(
                        "Image",
                        vec![
                            ("aspect", Prop::Number { value: 1.0 }),
                            ("corner", Prop::Number { value: 12.0 }),
                        ],
                        vec![],
                    )),
                    Node::Element(el("Text", vec![], vec![])),
                ],
            ))],
        );
        button.id = Some("open_card".into());
        button.action = Some(Action::Engine {
            command: 0,
            args: vec![],
        });

        let mut each = el("ForEach", vec![], vec![Node::Element(button)]);
        each.template = Some(Template {
            var: "item".into(),
            ty: Type::Text,
        });

        let grid = el(
            "Grid",
            vec![
                ("min_width", Prop::Number { value: 160.0 }),
                ("gap", Prop::Number { value: 16.0 }),
                (
                    "skeleton",
                    Prop::Expr {
                        expr: bool_engine(0),
                    },
                ),
                ("skeleton_count", Prop::Number { value: 4.0 }),
            ],
            vec![Node::Element(each)],
        );

        let grid = lower(Node::Element(grid), vec![]);
        let skel = grid.skeleton.as_ref().expect("grid skeleton");
        assert_no_actions_or_ids(skel);
        assert_eq!(skel.len(), 1);
        let Node::Element(skel_grid) = &skel[0] else {
            panic!("expected projected Grid")
        };
        assert_eq!(skel_grid.kind, "Grid");
        assert_eq!(skel_grid.number("stagger"), Some(DEFAULT_STAGGER));
        assert_eq!(skel_grid.children.len(), 4);
        for child in &skel_grid.children {
            let Node::Element(card) = child else {
                panic!("expected card Column")
            };
            assert_eq!(card.kind, "Column");
            assert_eq!(card.keyword("appear"), Some("slide"));
        }
    }

    #[test]
    fn keeps_responsive_window_conditions_and_collapses_identical_runtime_branches() {
        let ident_branch = Node::If(IfNode {
            branches: vec![(
                bool_engine(1),
                vec![Node::Element(el("Text", vec![], vec![]))],
            )],
            otherwise: vec![Node::Element(el("Text", vec![], vec![]))],
        });
        let responsive = Node::If(IfNode {
            branches: vec![(
                window_flag("compact"),
                vec![Node::Element(el(
                    "Row",
                    vec![("gap", Prop::Number { value: 8.0 })],
                    vec![ident_branch],
                ))],
            )],
            otherwise: vec![Node::Element(el(
                "Column",
                vec![("gap", Prop::Number { value: 12.0 })],
                vec![Node::Element(el("Text", vec![], vec![]))],
            ))],
        });
        let root = el(
            "Column",
            vec![(
                "skeleton",
                Prop::Expr {
                    expr: bool_engine(0),
                },
            )],
            vec![responsive],
        );
        let root = lower(Node::Element(root), vec![]);
        let skel = root.skeleton.as_ref().unwrap();
        let Node::Element(col) = &skel[0] else {
            panic!("expected Column")
        };
        let Node::If(outer_if) = &col.children[0] else {
            panic!("expected responsive If to be preserved")
        };
        let Node::Element(row) = &outer_if.branches[0].1[0] else {
            panic!("expected Row")
        };
        assert_eq!(row.children.len(), 1);
        assert!(matches!(&row.children[0], Node::Element(e) if e.kind == "Skeleton"));
    }

    #[test]
    fn collapses_placeholder_and_if_let_image_stack() {
        let placeholder_comp = ComponentDef {
            name: "cover_placeholder".into(),
            module: Vec::new(),
            params: vec![ParamDef {
                name: "side".into(),
                kind: ParamKind::Value { ty: Type::Float },
                content: false,
            }],
            body: Node::Element(el("Canvas", vec![], vec![])),
            source: src(),
        };
        let stack = el(
            "Stack",
            vec![
                ("width", Prop::Number { value: 44.0 }),
                ("height", Prop::Number { value: 44.0 }),
                (
                    "skeleton",
                    Prop::Expr {
                        expr: bool_engine(0),
                    },
                ),
            ],
            vec![
                Node::Component(crate::ComponentRef {
                    component: "cover_placeholder".into(),
                    args: vec![],
                    source: src(),
                }),
                Node::IfLet(IfLetNode {
                    var: "cover".into(),
                    value: bool_engine(2),
                    inner: Type::Text,
                    then: vec![Node::Element(el(
                        "Image",
                        vec![("corner", Prop::Number { value: 6.0 })],
                        vec![],
                    ))],
                    otherwise: vec![],
                }),
            ],
        );
        let stack = lower(Node::Element(stack), vec![placeholder_comp]);
        let skel = stack.skeleton.as_ref().unwrap();
        let mut kinds = Vec::new();
        collect_kinds(skel, &mut kinds);
        assert_eq!(kinds, vec!["Skeleton"]);
    }
}
