//! Kotlin generation: schema types, engine state, session drafts, one
//! composable per component and per window, and the activity.

mod app;
mod content;
mod controls;
mod drawing;
mod expr;
mod layout;
mod motion;
mod navigation;
mod toolbar;
mod values;
mod windows;

use crate::names::{float, member, pascal, string};
use bern_ir::{
    Application, ComponentArg, Element, IfLetNode, IfNode, MatchArm, MatchNode, Node, ParamKind,
};
use bern_protocol::{EnumDef, TypeTable};

/// Indented line writer.
#[derive(Default)]
pub struct Out {
    lines: Vec<String>,
    indent: usize,
}

impl Out {
    pub fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.lines.push(String::new());
        } else {
            self.lines
                .push(format!("{}{}", "    ".repeat(self.indent), text));
        }
    }

    pub fn open(&mut self, text: impl AsRef<str>) {
        self.line(text);
        self.indent += 1;
    }

    pub fn close(&mut self, text: impl AsRef<str>) {
        self.indent = self.indent.saturating_sub(1);
        self.line(text);
    }

    pub fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

/// The layout a generated statement sits in, which decides whether
/// `Spacer` can take the free space.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Scope {
    #[default]
    Other,
    Column,
    Row,
}

/// What a composable body can refer to while generating.
#[derive(Clone, Default)]
struct Ctx {
    component: String,
    /// Kotlin expression appended to control targets inside rows.
    key: Option<String>,
    /// The enclosing navigation path session field.
    path: Option<String>,
    scope: Scope,
    /// Inside a branch whose condition animates its appearing and
    /// disappearing.
    animated: bool,
    /// Inside a scroll view that rests on its items or follows a
    /// position: the first repetition's items report where they are.
    scroll_items: bool,
    /// The nodes are direct children of a container that declares
    /// `stagger`, or the root of a component, which may be one: those that
    /// appear take turns on the clock in `LocalBernStagger`.
    stagger: bool,
    /// The nodes are layers of a `Stack`.
    layer: bool,
}

impl Ctx {
    fn within(&self, scope: Scope) -> Ctx {
        Ctx {
            scope,
            ..self.clone()
        }
    }
}

pub struct Generator<'a> {
    app: &'a Application,
    /// Whether a container anywhere declares `stagger`. Only then do the
    /// children of containers and the roots of components take part in
    /// the turns.
    staggers: bool,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application) -> Self {
        let mut staggers = false;
        let mut find = |node: &Node| {
            staggers |= matches!(node, Node::Element(element) if element.prop("stagger").is_some())
        };
        for component in &app.components {
            component.body.walk(&mut find);
        }
        for window in &app.windows.windows {
            window.content.walk(&mut find);
        }
        Self { app, staggers }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    /// The style of a control from `style!`, if any.
    fn style_of(&self, element: &Element) -> Option<crate::style::Resolved> {
        crate::style::resolve(
            self.app,
            PLATFORM,
            &element.kind,
            element.keyword("variant"),
        )
    }

    /// `, style = BernStyles.x` for a styled control.
    fn style_arg(&self, element: &Element) -> String {
        self.style_of(element)
            .map(|style| format!(", style = BernStyles.{}", style.name))
            .unwrap_or_default()
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node], ctx: &Ctx) {
        for node in nodes {
            self.node(out, node, ctx);
        }
    }

    fn node(&self, out: &mut Out, node: &Node, ctx: &Ctx) {
        match node {
            Node::Element(element) => {
                // What the place the element stands in says about it ends
                // here; its own children start afresh. A repetition adds no
                // layout: its rows stand where it does.
                let repeats = element.kind == "ForEach";
                let own = if repeats {
                    ctx.clone()
                } else {
                    Ctx {
                        stagger: false,
                        layer: false,
                        ..ctx.clone()
                    }
                };
                // On a rail, the element sits at its fraction or between
                // two.
                let placement = self.placement(element);
                if let Some(place) = &placement {
                    out.open(format!("BernPlace({place}) {{"));
                }
                // Added later, as an item of a list, the element plays its
                // appearing transition; a direct child of a staggering
                // container waits for its turn.
                let appear = element.keyword("appear").filter(|_| !ctx.animated);
                let turns = ctx.stagger && self.staggers;
                let unstaggered =
                    turns && !repeats && appear.is_none() && Self::appears_below(element);
                if unstaggered {
                    out.open("BernUnstaggered {");
                }
                if let Some(name) = appear {
                    let staggered = if turns { ", staggered = true" } else { "" };
                    out.open(format!(
                        "BernAppear({}{staggered}) {{",
                        self.transition_args(Some(name))
                    ));
                }
                // Effects drawn on the element as a whole: a nudge and the
                // look that follows scrolling.
                let mut effects = String::new();
                if let Some(when) = element.expr("nudge") {
                    effects.push_str(&format!(
                        ".bernNudge({}, {})",
                        self.expr(when),
                        crate::style::motion(self.app, "jelly")
                    ));
                }
                let (scale, opacity) = (
                    element.number("scroll_scale"),
                    element.number("scroll_opacity"),
                );
                if scale.is_some() || opacity.is_some() {
                    effects.push_str(&format!(
                        ".bernScrollEffect({}, {})",
                        float(scale.unwrap_or(1.0)),
                        float(opacity.unwrap_or(1.0))
                    ));
                }
                if !effects.is_empty() {
                    out.open(format!("Box(Modifier{effects}) {{"));
                }
                // A part fixed to one appearance, with what opens from it.
                let appearance = element.keyword("appearance");
                if let Some(appearance) = appearance {
                    out.open(format!(
                        "BernAppearance(dark = {}) {{",
                        appearance == "dark"
                    ));
                }
                // A rail that is a layer of a stack, directly, and holds
                // only spans without a thickness of their own takes the
                // stack's size.
                let direct = appear.is_none()
                    && effects.is_empty()
                    && appearance.is_none()
                    && element.keyword("hero").is_none();
                let own = Ctx {
                    layer: if repeats {
                        own.layer
                    } else {
                        ctx.layer && direct && element.kind == "Rail" && Self::rail_fills(element)
                    },
                    ..own
                };
                match element.keyword("hero") {
                    Some(hero) => {
                        let key = match element.expr("hero_key") {
                            Some(key) => format!("\"{hero}-${{{}}}\"", self.expr(key)),
                            None => string(hero),
                        };
                        out.open(format!(
                            "BernHero({key}, {}) {{",
                            crate::style::motion(self.app, "hero")
                        ));
                        self.element(out, element, &own);
                        out.close("}");
                    }
                    None => self.element(out, element, &own),
                }
                if appearance.is_some() {
                    out.close("}");
                }
                if !effects.is_empty() {
                    out.close("}");
                }
                if appear.is_some() {
                    out.close("}");
                }
                if unstaggered {
                    out.close("}");
                }
                if placement.is_some() {
                    out.close("}");
                }
            }
            Node::Component(reference) => {
                let component = self.app.component(&reference.component).expect("component");
                let args: Vec<_> = component
                    .params
                    .iter()
                    .filter_map(|param| match &param.kind {
                        ParamKind::Value { .. } => reference
                            .args
                            .iter()
                            .find(|(name, _)| *name == param.name)
                            .and_then(|(_, arg)| match arg {
                                ComponentArg::Value { expr } => {
                                    Some(format!("{} = {}", member(&param.name), self.expr(expr)))
                                }
                                _ => None,
                            }),
                        _ => None,
                    })
                    .collect();
                out.line(format!(
                    "{}View({})",
                    pascal(&component.name),
                    args.join(", ")
                ));
            }
            Node::If(node) if Self::branches_move(node) => self.animated_if(out, node, ctx),
            Node::If(node) => self.if_node(out, node, ctx, &|out, body| self.nodes(out, body, ctx)),
            Node::IfLet(node) => self.if_let(out, node, &|out, body| self.nodes(out, body, ctx)),
            Node::Match(node) => {
                self.match_node(out, node, &|out, body| self.nodes(out, body, ctx))
            }
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode, _ctx: &Ctx, body: &dyn Fn(&mut Out, &[Node])) {
        for (index, (condition, nodes)) in node.branches.iter().enumerate() {
            if index == 0 {
                out.open(format!("if ({}) {{", self.expr(condition)));
            } else {
                out.close(format!("}} else if ({}) {{", self.expr(condition)));
                out.indent += 1;
            }
            body(out, nodes);
        }
        if !node.otherwise.is_empty() {
            out.close("} else {");
            out.indent += 1;
            body(out, &node.otherwise);
        }
        out.close("}");
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode, body: &dyn Fn(&mut Out, &[Node])) {
        let var = member(&node.var);
        out.open(format!("({}).let {{ {var} ->", self.expr(&node.value)));
        out.open(format!("if ({var} != null) {{"));
        body(out, &node.then);
        if !node.otherwise.is_empty() {
            out.close("} else {");
            out.indent += 1;
            body(out, &node.otherwise);
        }
        out.close("}");
        out.close("}");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode, body: &dyn Fn(&mut Out, &[Node])) {
        let def = self.types().enum_def(&node.enum_name).expect("enum");
        out.open(format!(
            "when (val bernValue = {}) {{",
            self.expr(&node.value)
        ));
        for arm in &node.arms {
            out.open(format!("{} -> {{", case_pattern(def, arm)));
            for line in arm_bindings(def, arm) {
                out.line(line);
            }
            body(out, &arm.body);
            out.close("}");
        }
        out.close("}");
    }

    fn element(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        match element.kind.as_str() {
            "Text" => self.text_view(out, element),
            "Canvas" => self.canvas(out, element),
            "Label" => self.label_view(out, element),
            "Button" | "IconButton" => self.button(out, element, ctx),
            "TextInput" => self.text_input(out, element, ctx),
            "Search" => self.search(out, element, ctx),
            "Toast" => self.toast(out, element),
            "Inspector" => self.inspector(out, element, ctx),
            "SplitButton" => self.split_button(out, element, ctx),
            "PopoverButton" => self.popover_button(out, element, ctx, false),
            // Shown by its sidebar.
            "SidebarFooter" => {}
            "Drop" => self.drop_zone(out, element, ctx),
            "Switch" => self.switch(out, element, ctx),
            "Slider" => self.slider(out, element, ctx),
            "Picker" => self.picker(out, element, ctx),
            "Progress" => self.progress(out, element),
            "Form" => self.form(out, element, ctx),
            "Section" => self.section(out, element, ctx),
            "Column" | "Row" => self.column_or_row(out, element, ctx),
            "Grid" => self.grid(out, element, ctx),
            "Stack" => self.stack(out, element, ctx),
            "Rail" => self.rail(out, element, ctx),
            "Svg" => self.svg(out, element),
            "Image" => self.image(out, element),
            "Spacer" => match ctx.scope {
                Scope::Column | Scope::Row => out.line("Spacer(Modifier.weight(1f))"),
                Scope::Other => out.line("Spacer(Modifier.size(8.dp))"),
            },
            "Divider" => match ctx.scope {
                Scope::Row => out.line("VerticalDivider()"),
                _ => out.line("HorizontalDivider()"),
            },
            "ScrollView"
                if element.prop("snap").is_some()
                    || element.prop("position").is_some()
                    || self.scroll_effects(&element.children) =>
            {
                self.tracked_scroll_view(out, element, ctx)
            }
            "Pinch" | "Swipe" => self.gesture(out, element, ctx),
            "ScrollView" => self.scroll_view(out, element, ctx),
            "List" | "ForEach" => self.list(out, element, ctx),
            "Query" => self.query(out, element, ctx),
            "Video" => self.video(out, element, ctx),
            "SplitView" => self.split_view(out, element, ctx),
            "Navigation" => self.navigation(out, element, ctx),
            "NavLink" => self.nav_link(out, element, ctx),
            "Tabs" => self.tabs(out, element, ctx),
            "Toolbar" => self.toolbar(out, element, ctx),
            "Menu" => self.menu(out, element, ctx),
            "Adaptive" => self.adaptive(out, element, ctx),
            "LanguagePicker" => self.language_picker(out, element, ctx),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "WindowSet"
            | "Window" | "Layout" | "MenuItem" => {
                // Realized by their parents.
            }
            other => out.line(format!("// unsupported {other}")),
        }
    }
}

/// The platform's name in `style!`.
const PLATFORM: &str = "android";

/// The pattern of one `match` arm: a variant, or `else` for `_`.
fn case_pattern(def: &EnumDef, arm: &MatchArm) -> String {
    let Some(index) = arm.variant else {
        return "else".into();
    };
    let variant = &def.variants[index as usize];
    // Unit variants are values; variants with fields are classes.
    if variant.fields.is_empty() {
        format!("{}.{}", def.name, variant.name)
    } else {
        format!("is {}.{}", def.name, variant.name)
    }
}

/// `val name = bernValue.field` for the bindings of one arm.
fn arm_bindings(def: &EnumDef, arm: &MatchArm) -> Vec<String> {
    let Some(index) = arm.variant else {
        return Vec::new();
    };
    let variant = &def.variants[index as usize];
    arm.bindings
        .iter()
        .map(|binding| {
            format!(
                "val {} = bernValue.{}",
                member(&binding.name),
                member(&variant.fields[binding.field].name)
            )
        })
        .collect()
}
