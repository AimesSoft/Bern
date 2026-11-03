//! Motion: the springs of `style!`'s tokens and transitions, and the motion
//! props that move an element.

use super::{Axis, Generator, float, string};
use bern_ir::style::TransitionKind;
use bern_ir::{ComponentDef, Element, IfLetNode, IfNode, MatchNode, Node};

/// The name of a constant for a `style!` name: `M_SMOOTH`, `X_PAGE_IN`.
pub(super) fn constant(prefix: &str, name: &str) -> String {
    let mut out = format!("{prefix}_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

/// Elements that draw themselves where their motion has them, given its
/// pose; the runtime draws any other in a box of its own while it moves.
pub(super) fn poses_itself(kind: &str) -> bool {
    matches!(kind, "Positioned" | "Modal")
}

/// Containers whose children can take turns to appear (`stagger`).
pub(super) fn staggers(kind: &str) -> bool {
    bern_ir::spec::STAGGERING.contains(&kind)
}

impl Generator<'_> {
    /// The motion tokens and transitions of `style!`.
    pub(super) fn motion_tables(&self) -> String {
        let sheet = &self.app.style;
        let mut out = String::from("// Motion tokens: period and damping ratio of each spring.\n");
        for token in &sheet.motion {
            out.push_str(&format!(
                "#[allow(dead_code)] const {}: b::Curve = ({}, {});\n",
                constant("M", &token.name),
                float(token.period),
                float(token.damping)
            ));
        }
        for (name, period, damping) in bern_ir::style::BUILT_IN_MOTION {
            if sheet.token(name).is_none() {
                out.push_str(&format!(
                    "#[allow(dead_code)] const {}: b::Curve = ({}, {});\n",
                    constant("M", name),
                    float(period),
                    float(damping)
                ));
            }
        }
        out.push_str("// Transitions of `appear` and `disappear`.\n");
        for transition in &sheet.transitions {
            let kind = match &transition.kind {
                TransitionKind::Fade => "b::TransitionKind::Fade".to_string(),
                TransitionKind::Grow { edge } => {
                    format!("b::TransitionKind::Grow(b::Edge::{edge:?})")
                }
                TransitionKind::Pop { from } => format!("b::TransitionKind::Pop({})", float(*from)),
                TransitionKind::Slide { edge, distance } => format!(
                    "b::TransitionKind::Slide(b::Edge::{edge:?}, {})",
                    float(*distance)
                ),
                TransitionKind::Move { edge } => {
                    format!("b::TransitionKind::Move(b::Edge::{edge:?})")
                }
            };
            out.push_str(&format!(
                "#[allow(dead_code)] static {}: b::Transition = b::Transition {{ kind: {kind}, motion: {} }};\n",
                constant("X", &transition.name),
                self.curve(&transition.motion)
            ));
        }
        out
    }

    /// A motion token's spring.
    pub(super) fn curve(&self, token: &str) -> String {
        constant("M", token)
    }

    /// The motion props that move `element`, as the runtime takes them, if
    /// it has any.
    pub(super) fn motion(&self, element: &Element) -> Option<String> {
        if matches!(
            element.kind.as_str(),
            "ForEach" | "List" | "Toolbar" | "Toast" | "Inspector" | "Query"
        ) {
            return None;
        }
        let at = Some(&element.source);
        let mut entries = Vec::new();
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("Some(&{})", constant("X", transition)))
        };
        if let Some(appear) = transition("appear") {
            entries.push(format!("appear: {appear}"));
        }
        if let Some(delay) = element.number("appear_delay") {
            entries.push(format!("appear_delay: Some({})", float(delay)));
        }
        if let Some(disappear) = transition("disappear") {
            entries.push(format!("disappear: {disappear}"));
        }
        if let Some(when) = element.expr("nudge") {
            entries.push(format!("nudge: Some({})", self.condition(when)));
        }
        if let Some(key) = element.expr("nudge_key") {
            entries.push(format!("nudge_key: Some({})", self.expr(key, at)));
        }
        if let Some(scale) = element.number("nudge_scale") {
            entries.push(format!("nudge_scale: Some({})", float(scale)));
        }
        if let Some(reduce) = element.expr("reduce_motion") {
            entries.push(format!("reduce_motion: Some({})", self.condition(reduce)));
        }
        let scale = element.number("scroll_scale");
        let opacity = element.number("scroll_opacity");
        if scale.is_some() || opacity.is_some() {
            entries.push(format!(
                "scroll: Some(({}, {}, ({}, {})))",
                float(scale.unwrap_or(1.0)),
                float(opacity.unwrap_or(1.0)),
                float(element.number("scroll_origin_x").unwrap_or(0.5)),
                float(element.number("scroll_origin_y").unwrap_or(0.5))
            ));
        }
        if let Some(hero) = element.keyword("hero") {
            let key = match element.expr("hero_key") {
                Some(key) => format!("Some({})", self.expr(key, at)),
                None => "None".into(),
            };
            entries.push(format!(
                "hero: Some((b::hero_key({}, {key}), {}))",
                string(hero),
                self.curve("hero")
            ));
        }
        (!entries.is_empty())
            .then(|| format!("b::Motion {{ {}, ..b::Motion::NONE }}", entries.join(", ")))
    }

    /// The pose an element that draws itself moved takes: its motion's
    /// when it has one.
    pub(super) fn pose(&self, element: &Element) -> &'static str {
        if self.motion(element).is_some() {
            "pose"
        } else {
            "b::Pose::REST"
        }
    }

    /// `code` that builds `element`, moved by its motion props.
    pub(super) fn moved(&self, element: &Element, code: String) -> String {
        match self.motion(element) {
            Some(motion) => {
                let own = poses_itself(&element.kind);
                format!(
                    "b::moving(c, {}, {motion}, {own}, |c, {}| {code})",
                    string(element.id.as_deref().unwrap_or(&element.path)),
                    if own { "pose" } else { "_pose" }
                )
            }
            None => code,
        }
    }

    /// `children`, the code of a container's children, taking turns to
    /// appear when the container staggers.
    pub(super) fn staggered(&self, element: &Element, children: String) -> String {
        match element.number("stagger") {
            Some(step) if staggers(&element.kind) => format!(
                "b::staggered(c, {}, {}, || {children})",
                string(&format!("{}#stagger", element.path)),
                float(step)
            ),
            _ => children,
        }
    }

    /// A bound length of `element`, springing to each new value when it
    /// declares `animate`.
    pub(super) fn animated(&self, element: &Element, name: &str, value: String) -> String {
        match element.keyword("animate") {
            Some(token) => format!(
                "b::eased(c, {}, {}, {value})",
                string(&format!("{}/{name}", element.path)),
                self.curve(token)
            ),
            None => value,
        }
    }
}

// MARK: - Leaving

impl Generator<'_> {
    /// Whether `node`, built as it leaves, plays `disappear`: an element
    /// that declares it, or a component whose body does.
    pub(super) fn node_leaves(&self, node: &Node) -> bool {
        fn leaves(generator: &Generator<'_>, node: &Node, depth: usize) -> bool {
            match node {
                Node::Element(element) => {
                    element.keyword("disappear").is_some() && generator.motion(element).is_some()
                }
                Node::Component(reference) => {
                    depth < 8
                        && generator
                            .app
                            .component(&reference.component)
                            .is_some_and(|component| leaves(generator, &component.body, depth + 1))
                }
                // What a condition shows as it leaves.
                Node::If(node) => node
                    .branches
                    .iter()
                    .flat_map(|(_, body)| body)
                    .chain(&node.otherwise)
                    .any(|node| leaves(generator, node, depth)),
                Node::IfLet(node) => node
                    .then
                    .iter()
                    .chain(&node.otherwise)
                    .any(|node| leaves(generator, node, depth)),
                Node::Match(node) => node
                    .arms
                    .iter()
                    .flat_map(|arm| &arm.body)
                    .any(|node| leaves(generator, node, depth)),
            }
        }
        leaves(self, node, 0)
    }

    pub(super) fn leaves(&self, nodes: &[Node]) -> bool {
        nodes.iter().any(|node| self.node_leaves(node))
    }

    /// A name for a condition, apart from every other in the application.
    fn condition_id(&self) -> String {
        let number = self.conditions.get();
        self.conditions.set(number + 1);
        string(&format!("cond{number}"))
    }

    /// Push what of `nodes` plays `disappear`, as it leaves.
    pub(super) fn push_leaving(&self, out: &mut String, nodes: &[Node], axis: Axis) {
        for node in nodes.iter().filter(|node| self.node_leaves(node)) {
            match node {
                // A condition in what leaves shows, as it leaves, the branch
                // it shows now.
                Node::If(node) => {
                    for (condition, body) in &node.branches {
                        out.push_str(&format!("if {} {{ ", self.condition(condition)));
                        self.push_leaving(out, body, axis);
                        out.push_str("} else ");
                    }
                    out.push_str("{ ");
                    self.push_leaving(out, &node.otherwise, axis);
                    out.push_str("} ");
                }
                Node::IfLet(node) => {
                    out.push_str(&format!(
                        "if let Some(x) = rt::some({}) {{ let c = &c.with_local({}, x); ",
                        self.expr(&node.value, None),
                        string(&node.var)
                    ));
                    self.push_leaving(out, &node.then, axis);
                    out.push_str("} else { ");
                    self.push_leaving(out, &node.otherwise, axis);
                    out.push_str("} ");
                }
                Node::Match(node) => {
                    out.push_str(&format!(
                        "{{ let m = {}; match rt::tag(&m) {{ ",
                        self.expr(&node.value, None)
                    ));
                    for arm in &node.arms {
                        match arm.variant {
                            Some(variant) => out.push_str(&format!("Some({variant}) => {{ ")),
                            None => out.push_str("_ => { "),
                        }
                        for binding in &arm.bindings {
                            out.push_str(&format!(
                                "let c = &c.with_local({}, rt::payload(m.clone(), {})); ",
                                string(&binding.name),
                                binding.field
                            ));
                        }
                        self.push_leaving(out, &arm.body, axis);
                        out.push_str("} ");
                    }
                    if node.arms.iter().all(|arm| arm.variant.is_some()) {
                        out.push_str("_ => {} ");
                    }
                    out.push_str("} } ");
                }
                _ => {
                    self.leaving.set(true);
                    self.push(out, node, axis);
                    self.leaving.set(false);
                }
            }
        }
    }

    /// `code` that builds `element` as it leaves with its `disappear`.
    pub(super) fn left(&self, element: &Element, code: String) -> String {
        let own = poses_itself(&element.kind);
        format!(
            "b::leave(c, {}, &{}, {own}, |c, {}| {code})",
            string(element.id.as_deref().unwrap_or(&element.path)),
            constant("X", element.keyword("disappear").unwrap_or("fade")),
            if own { "pose" } else { "_pose" }
        )
    }

    /// A component's body as it leaves, if it plays `disappear`.
    pub(super) fn leaving_component(&self, component: &ComponentDef) -> Option<String> {
        if !self.node_leaves(&component.body) {
            return None;
        }
        let body = match &component.body {
            Node::Element(element) => {
                self.leaving.set(true);
                let code = self.element(element, "c");
                self.leaving.set(false);
                code?
            }
            node => {
                let mut out = String::new();
                self.push_leaving(&mut out, std::slice::from_ref(node), Axis::Column);
                format!(
                    "b::column(c, b::Flex::default(), {{ let mut v: Vec<b::El> = Vec::new(); {out} v }})"
                )
            }
        };
        Some(format!(
            "pub fn l_{}(c: &b::Ctx) -> b::El {{\n    {body}\n}}\n",
            component.name
        ))
    }

    /// `if`, whose branch keeps showing what plays `disappear` after it
    /// changes, until that has left.
    pub(super) fn push_if_leaving(&self, out: &mut String, node: &IfNode, axis: Axis) {
        let id = self.condition_id();
        let mut bodies: Vec<&[Node]> = node
            .branches
            .iter()
            .map(|(_, body)| body.as_slice())
            .collect();
        bodies.push(&node.otherwise);
        out.push_str("{ let shown: usize = ");
        for (index, (condition, _)) in node.branches.iter().enumerate() {
            out.push_str(&format!(
                "if {} {{ {index} }} else ",
                self.condition(condition)
            ));
        }
        out.push_str(&format!("{{ {} }}; ", node.branches.len()));
        self.push_left(
            out,
            &id,
            "bern_ffi::protocol::Value::Unit",
            &bodies,
            axis,
            &|_, _| {},
        );
        out.push_str("match shown { ");
        for (index, body) in bodies.iter().enumerate() {
            out.push_str(&format!("{index} => {{ "));
            for node in *body {
                self.push(out, node, axis);
            }
            out.push_str("} ");
        }
        out.push_str("_ => {} } } ");
    }

    /// `if let`, keeping what plays `disappear` as it leaves, with the
    /// value it had.
    pub(super) fn push_if_let_leaving(&self, out: &mut String, node: &IfLetNode, axis: Axis) {
        let id = self.condition_id();
        let var = string(&node.var);
        out.push_str(&format!(
            "{{ let found = rt::some({}); let shown: usize = if found.is_some() {{ 0 }} else {{ 1 }}; ",
            self.expr(&node.value, None)
        ));
        let bodies = [node.then.as_slice(), node.otherwise.as_slice()];
        self.push_left(
            out,
            &id,
            "found.clone().unwrap_or(bern_ffi::protocol::Value::Unit)",
            &bodies,
            axis,
            &|out, index| {
                if index == 0 {
                    out.push_str(&format!("let c = &c.with_local({var}, kept.clone()); "));
                }
            },
        );
        out.push_str(&format!(
            "if let Some(x) = found {{ let c = &c.with_local({var}, x); "
        ));
        for node in &node.then {
            self.push(out, node, axis);
        }
        out.push_str("} else { ");
        for node in &node.otherwise {
            self.push(out, node, axis);
        }
        out.push_str("} } ");
    }

    /// `match`, keeping the arm that plays `disappear` as it leaves, with
    /// the value it matched.
    pub(super) fn push_match_leaving(&self, out: &mut String, node: &MatchNode, axis: Axis) {
        let id = self.condition_id();
        out.push_str(&format!(
            "{{ let m = {}; let shown: usize = match rt::tag(&m) {{ ",
            self.expr(&node.value, None)
        ));
        let mut wildcard = node.arms.len();
        for (index, arm) in node.arms.iter().enumerate() {
            match arm.variant {
                Some(variant) => out.push_str(&format!("Some({variant}) => {index}, ")),
                None => wildcard = wildcard.min(index),
            }
        }
        out.push_str(&format!("_ => {wildcard} }}; "));
        let bodies: Vec<&[Node]> = node.arms.iter().map(|arm| arm.body.as_slice()).collect();
        let bind = |out: &mut String, index: usize| {
            out.push_str("let m = kept.clone(); ");
            for binding in &node.arms[index].bindings {
                out.push_str(&format!(
                    "let c = &c.with_local({}, rt::payload(m.clone(), {})); ",
                    string(&binding.name),
                    binding.field
                ));
            }
        };
        self.push_left(out, &id, "m.clone()", &bodies, axis, &bind);
        out.push_str("match shown { ");
        for (index, arm) in node.arms.iter().enumerate() {
            out.push_str(&format!("{index} => {{ "));
            for binding in &arm.bindings {
                out.push_str(&format!(
                    "let c = &c.with_local({}, rt::payload(m.clone(), {})); ",
                    string(&binding.name),
                    binding.field
                ));
            }
            for node in &arm.body {
                self.push(out, node, axis);
            }
            out.push_str("} ");
        }
        out.push_str("_ => {} } } ");
    }

    /// The branch that leaves, among `bodies`, built with the value it had
    /// (`kept`) as `bind` binds it.
    fn push_left(
        &self,
        out: &mut String,
        id: &str,
        value: &str,
        bodies: &[&[Node]],
        axis: Axis,
        bind: &dyn Fn(&mut String, usize),
    ) {
        let leaves: Vec<String> = bodies
            .iter()
            .map(|body| self.leaves(body).to_string())
            .collect();
        out.push_str(&format!(
            "if let Some((old, kept)) = b::leaving(c, {id}, shown, {value}, &[{}]) {{ b::left(c, {id}, || {{ let _ = &kept; match old {{ ",
            leaves.join(", ")
        ));
        for (index, body) in bodies.iter().enumerate() {
            if !self.leaves(body) {
                continue;
            }
            out.push_str(&format!("{index} => {{ "));
            bind(out, index);
            self.push_leaving(out, body, axis);
            out.push_str("} ");
        }
        out.push_str("_ => {} } }); } ");
    }
}

// MARK: - Reflowing

impl Generator<'_> {
    /// The spring the children of a container slide to their places with:
    /// its own `reflow`, or that of a `ForEach` whose rows are its
    /// children.
    pub(super) fn reflow_of(&self, element: &Element) -> Option<String> {
        fn rows(nodes: &[Node]) -> Option<&str> {
            nodes.iter().find_map(|node| match node {
                Node::Element(child) if child.kind == "ForEach" => child.keyword("reflow"),
                Node::If(node) => node
                    .branches
                    .iter()
                    .find_map(|(_, body)| rows(body))
                    .or_else(|| rows(&node.otherwise)),
                Node::IfLet(node) => rows(&node.then).or_else(|| rows(&node.otherwise)),
                Node::Match(node) => node.arms.iter().find_map(|arm| rows(&arm.body)),
                _ => None,
            })
        }
        if !matches!(
            element.kind.as_str(),
            "Column" | "Row" | "Flow" | "Stack" | "Grid"
        ) {
            return None;
        }
        element
            .keyword("reflow")
            .or_else(|| rows(&element.children))
            .map(|token| self.curve(token))
    }
}
