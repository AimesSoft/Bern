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
mod table;
mod toolbar;
mod values;
mod windows;

use crate::names::{self, double, float, member, string, value_type};
use bern_ir::{
    Application, ComponentArg, Element, IfLetNode, IfNode, MatchArm, MatchNode, Node, ParamKind,
};
use bern_protocol::{EnumDef, TypeTable};
use layout::window_box;

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
    /// The items of a `Flow`: a `FlowRow`, where a spacer takes the rest
    /// of its line and a chip picker's chips are items.
    Flow,
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
    /// The nodes are layers of a `Stack`, which aligns them this way.
    layer: bool,
    layer_align: &'static str,
    /// Among the layers, some besides unsized canvases and rails give the
    /// stack its size, so those backdrops can take it.
    sized_layers: bool,
    /// The nodes are direct children of `Row(align: baseline)`: each
    /// lines up on its first baseline.
    baseline: bool,
    /// The nodes are direct children of a row of a given height, aligned
    /// this way: text taller than the row keeps its height and overflows
    /// by that alignment, as on the other targets.
    overflow: Option<&'static str>,
    /// The nodes are the content of a vertical scroll view, whose height
    /// has no end: none of them can share it by weight.
    scroll_content: bool,
    /// The nodes are written directly in a window, or through a condition
    /// there: a column spans the window's width and a stack fills it.
    window: bool,
}

impl Ctx {
    fn within(&self, scope: Scope) -> Ctx {
        Ctx {
            scope,
            scroll_content: false,
            ..self.clone()
        }
    }
}

pub struct Generator<'a> {
    app: &'a Application,
    multicast: bool,
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
        Self {
            app,
            staggers,
            multicast: false,
        }
    }

    /// Hold Android's Wi-Fi multicast filter open while the activity is visible.
    pub fn with_multicast(mut self, enabled: bool) -> Self {
        self.multicast = enabled;
        self
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
        let mut spacer_taken = false;
        for (index, node) in nodes.iter().enumerate() {
            if std::mem::take(&mut spacer_taken) {
                continue;
            }
            // The words that lead a row take the spacer's room: the row
            // measures the controls after them first, and the words wrap
            // in what is left instead of pushing them out.
            if ctx.scope == Scope::Row
                && !ctx.baseline
                && bern_ir::leads_row(node, nodes.get(index + 1))
            {
                out.open("Box(Modifier.weight(1f)) {");
                self.node(out, node, &ctx.within(Scope::Other));
                out.close("}");
                spacer_taken = true;
                continue;
            }
            self.node(out, node, ctx);
        }
    }

    fn node(&self, out: &mut Out, node: &Node, ctx: &Ctx) {
        match node {
            Node::Element(element) => {
                // Written in the window itself, the element takes the
                // window's room; what it holds does not.
                let window = ctx.window.then(|| window_box(element)).flatten();
                if let Some(open) = &window {
                    out.open(open);
                }
                // What the place the element stands in says about it ends
                // here; its own children start afresh. A repetition adds no
                // layout: its rows stand where it does.
                let repeats = element.kind == "ForEach";
                let own = if repeats {
                    Ctx {
                        window: false,
                        ..ctx.clone()
                    }
                } else {
                    Ctx {
                        stagger: false,
                        layer: false,
                        baseline: false,
                        overflow: None,
                        window: false,
                        ..ctx.clone()
                    }
                };
                // In a row lined up on its baseline, each child is; a
                // spacer takes the free space instead.
                let baseline = ctx.baseline && !repeats && element.kind != "Spacer";
                if baseline {
                    out.open("Box(Modifier.alignByBaseline()) {");
                }
                // Text cannot be shorter than its line: in a row of a given
                // height it keeps its own and stands out of the row by the
                // row's alignment instead of being cut.
                let overflow = ctx
                    .overflow
                    .filter(|_| matches!(element.kind.as_str(), "Text" | "Label"));
                if let Some(align) = overflow {
                    out.open(format!(
                        "Box(Modifier.wrapContentHeight({align}, unbounded = true)) {{"
                    ));
                }
                // The whole application runs with reduced motion while
                // this holds.
                if let Some(reduce) = element.expr("reduce_motion") {
                    out.line(format!("BernReduceMotion({})", self.expr(reduce)));
                }
                // On a rail, the element sits at its fraction or between
                // two.
                let placement = self.placement(element);
                if let Some(place) = &placement {
                    out.open(format!("BernPlace({place}) {{"));
                }
                // Added later, as an item of a list, the element plays its
                // appearing transition; a direct child of a staggering
                // container waits for its turn.
                // A modal plays its transitions on its card, above the
                // window.
                let appear = element
                    .keyword("appear")
                    .filter(|_| !ctx.animated && element.kind != "Modal");
                let turns = ctx.stagger && self.staggers;
                let unstaggered =
                    turns && !repeats && appear.is_none() && Self::appears_below(element);
                if unstaggered {
                    out.open("BernUnstaggered {");
                }
                if let Some(name) = appear {
                    let staggered = if turns { ", staggered = true" } else { "" };
                    let delay = element
                        .number("appear_delay")
                        .map(|delay| format!(", delay = {}", double(delay)))
                        .unwrap_or_default();
                    out.open(format!(
                        "BernAppear({}{staggered}{delay}) {{",
                        self.transition_args(Some(name))
                    ));
                }
                // Effects on the element as a whole: a nudge, the look that
                // follows scrolling, and an area a resting mouse lets the
                // window rest over.
                let mut effects = String::new();
                let nudge_scale = float(element.number("nudge_scale").unwrap_or(1.06));
                if let Some(when) = element.expr("nudge") {
                    effects.push_str(&format!(
                        ".bernNudge({}, {}, {nudge_scale})",
                        self.expr(when),
                        crate::style::motion(self.app, "jelly")
                    ));
                }
                if let Some(key) = element.expr("nudge_key") {
                    effects.push_str(&format!(
                        ".bernNudgeKey({}, {}, {nudge_scale})",
                        self.expr(key),
                        crate::style::motion(self.app, "jelly")
                    ));
                }
                let (scale, opacity) = (
                    element.number("scroll_scale"),
                    element.number("scroll_opacity"),
                );
                if scale.is_some() || opacity.is_some() {
                    effects.push_str(&format!(
                        ".bernScrollEffect({}, {}, {}, {})",
                        float(scale.unwrap_or(1.0)),
                        float(opacity.unwrap_or(1.0)),
                        float(element.number("scroll_origin_x").unwrap_or(0.5)),
                        float(element.number("scroll_origin_y").unwrap_or(0.5))
                    ));
                }
                if let Some(seconds) = element.number("idle_area") {
                    effects.push_str(&format!(".bernIdleArea({})", double(seconds)));
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
                // A rail or a canvas that is a layer of a stack, directly,
                // takes the size the other layers give the stack: a rail
                // holding only spans without a thickness of their own, a
                // canvas without a size, its backdrop.
                let tint = element.keyword("tint");
                if let Some(tint) = tint {
                    out.open(format!("BernTint({}) {{", string(tint)));
                }
                let direct = tint.is_none()
                    && appear.is_none()
                    && effects.is_empty()
                    && appearance.is_none()
                    && element.keyword("hero").is_none();
                let own = Ctx {
                    layer: if repeats {
                        own.layer
                    } else {
                        ctx.layer && direct && matches!(element.kind.as_str(), "Rail" | "Canvas")
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
                if tint.is_some() {
                    out.close("}");
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
                if overflow.is_some() {
                    out.close("}");
                }
                if baseline {
                    out.close("}");
                }
                if window.is_some() {
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
                let call = format!("{}({})", names::component(&component.name), args.join(", "));
                // In a column or row the component's layout can take the
                // free room there, as it would written in place.
                let call = match ctx.scope {
                    Scope::Column | Scope::Row => {
                        format!(
                            "CompositionLocalProvider(LocalBernFlex provides this) {{ {call} }}"
                        )
                    }
                    Scope::Other | Scope::Flow => call,
                };
                // A component written in the window takes the window's room
                // as its body would.
                let window = ctx
                    .window
                    .then(|| self.window_body(&component.body))
                    .flatten();
                if ctx.baseline {
                    out.line(format!("Box(Modifier.alignByBaseline()) {{ {call} }}"));
                } else if let Some(open) = window {
                    out.line(format!("{open} {call} }}"));
                } else {
                    out.line(call);
                }
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
            "Canvas" => self.canvas(out, element, ctx),
            "Label" => self.label_view(out, element),
            "Button" | "IconButton" => self.button(out, element, ctx),
            "TextInput" => self.text_input(out, element, ctx),
            "Search" if element.keyword("style") == Some("palette") => {
                self.search_palette(out, element, ctx)
            }
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
            "FocusRegion" => {
                out.open("BernFocusRegion {");
                self.nodes(out, &element.children, &ctx.within(Scope::Other));
                out.close("}");
            }
            "ScaleBox" => self.scale_box(out, element, ctx),
            "Positioned" => self.positioned(out, element, ctx),
            "Marquee" => self.marquee(out, element, ctx),
            "Accordion" => self.accordion(out, element, ctx),
            "Flow" => self.flow(out, element, ctx),
            "Modal" => self.modal(out, element, ctx),
            "Timer" => self.timer(out, element),
            "Watch" => self.watch(out, element),
            "Shortcut" => self.key_shortcut(out, element),
            "ShortcutInput" => self.shortcut_input(out, element, ctx),
            "ControlGroup" => self.control_group(out, element, ctx),
            "SearchItem" => self.search_item(out, element, ctx),
            "Rail" => self.rail(out, element, ctx),
            "Svg" => self.svg(out, element),
            "Image" => self.image(out, element),
            "Spacer" => match ctx.scope {
                Scope::Column | Scope::Row | Scope::Flow => out.line("Spacer(Modifier.weight(1f))"),
                Scope::Other => out.line("Spacer(Modifier.size(8.dp))"),
            },
            // Android windows have no buttons of their own.
            "WindowButtons" => {}
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
            "PreviewList" => self.preview_list(out, element, ctx),
            "Query" => self.query(out, element, ctx),
            "Video" => self.video(out, element, ctx),
            "SplitView" => self.split_view(out, element, ctx),
            "Navigation" => self.navigation(out, element, ctx),
            "NavLink" => self.nav_link(out, element, ctx),
            "Tabs" => self.tabs(out, element, ctx),
            "Toolbar" => self.toolbar(out, element, ctx),
            "Menu" => self.menu(out, element, ctx),
            "Table" => self.table(out, element, ctx),
            "Adaptive" => self.adaptive(out, element, ctx),
            "LanguagePicker" => self.language_picker(out, element, ctx),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "SidebarHeader"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" | "TabAccessory" | "PickerOption"
            | "TableColumn" | "TableMenu"
            // Android has no menu bar for applications.
            | "MenuBar" | "CommandMenu" | "CommandItem"
            // Nor a status area.
            | "StatusItem" => {
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
        format!("{}.{}", value_type(&def.name), variant.name)
    } else {
        format!("is {}.{}", value_type(&def.name), variant.name)
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
