//! Swift source generation from the IR.
//!
//! Every component becomes a SwiftUI `View` built from standard containers
//! and controls with system styles. Nothing here sets a color, a material or
//! a version-gated API: appearance comes from the SDK and the running macOS.

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

use crate::names::{case, double, member, pascal};
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

/// What a view body can refer to while generating.
#[derive(Clone, Default)]
struct Ctx {
    component: String,
    /// Swift expression of the enclosing navigation path session field.
    path: Option<String>,
    /// Generating a toolbar group, whose buttons the toolbar draws.
    toolbar: bool,
    /// The toolbar group has other items besides this one: they share one
    /// background, which a menu only joins without its indicator.
    shared: bool,
    /// Inside a scroll view that rests items at its center: 2 for its
    /// content, 1 for the items of that content, whose sizes it measures.
    snap_level: u8,
    /// Inside a scroll view whose `position` is bound: the first
    /// repetition's items report where they are.
    scroll_items: bool,
    /// Each direct accordion child fills and clips to its allocated width.
    accordion_corner: Option<String>,
    /// An item of a row, directly or through a condition or repetition: a
    /// spacer there grows only along the row, a divider stands upright.
    row: bool,
}

/// The system the Swift is generated for. SwiftUI is shared; windows,
/// scenes and a few control styles are not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Ios,
    Tvos,
}

impl Platform {
    /// Whether the app is a UIKit app: iOS and tvOS share the scene, the
    /// sheets and the host of UIKit.
    pub fn uikit(self) -> bool {
        matches!(self, Platform::Ios | Platform::Tvos)
    }

    /// The system's name in messages.
    pub fn system(self) -> &'static str {
        match self {
            Platform::Macos => "macOS",
            Platform::Ios => "iOS",
            Platform::Tvos => "tvOS",
        }
    }
}

pub struct Generator<'a> {
    app: &'a Application,
    platform: Platform,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application, platform: Platform) -> Self {
        Self { app, platform }
    }

    fn types(&self) -> &TypeTable {
        &self.app.types
    }

    /// The platform's name in `style!`.
    fn style_platform(&self) -> &'static str {
        match self.platform {
            Platform::Macos => "macos",
            Platform::Ios => "ios",
            Platform::Tvos => "tvos",
        }
    }

    /// The style of a control from `style!`, if any.
    fn style_of(&self, element: &Element) -> Option<crate::style::Resolved> {
        crate::style::resolve(
            self.app,
            self.style_platform(),
            &element.kind,
            element.keyword("variant"),
        )
    }

    fn nodes(&self, out: &mut Out, nodes: &[Node], ctx: &Ctx) {
        let mut emitted = false;
        let inner = (ctx.snap_level > 0).then(|| Ctx {
            snap_level: ctx.snap_level - 1,
            ..ctx.clone()
        });
        for node in nodes {
            // Toolbars and side panels attach to their container.
            if matches!(node, Node::Element(element) if element.kind == "Toolbar" || element.kind == "Inspector")
            {
                continue;
            }
            match (&inner, ctx.snap_level) {
                // A repetition's items are measured one by one: a modifier
                // around the whole repetition would stack them as one view.
                (Some(_), 1) if matches!(node, Node::Element(element) if element.kind == "ForEach") => {
                    self.node(out, node, ctx)
                }
                // Items of a scroll view that rests them at its center.
                (Some(inner), 1) => {
                    out.open("SwiftUI.Group {");
                    self.node(out, node, inner);
                    out.close("}");
                    out.line(".bernSnapItem()");
                }
                (Some(inner), _) => self.node(out, node, inner),
                (None, _) => self.node(out, node, ctx),
            }
            emitted = true;
        }
        if !emitted {
            out.line("SwiftUI.EmptyView()");
        }
    }

    fn node(&self, out: &mut Out, node: &Node, ctx: &Ctx) {
        if let Some(corner) = &ctx.accordion_corner
            && (matches!(node, Node::Component(_))
                || matches!(node, Node::Element(element) if element.kind != "ForEach"))
        {
            out.open(format!("BernAccordionCell(corner: {corner}) {{"));
            self.node(
                out,
                node,
                &Ctx {
                    accordion_corner: None,
                    ..ctx.clone()
                },
            );
            out.close("}");
            return;
        }
        match node {
            Node::Element(element) => {
                let inside;
                let ctx = if ctx.row
                    && element.kind != "ForEach"
                    && element.kind != "Spacer"
                    && element.kind != "Divider"
                {
                    inside = Ctx {
                        row: false,
                        ..ctx.clone()
                    };
                    &inside
                } else {
                    ctx
                };
                // A staggering container's children take turns appearing.
                let stagger = element.number("stagger");
                if let Some(step) = stagger {
                    out.open(format!("BernStaggerScope(step: {}) {{", double(step)));
                }
                self.element(out, element, ctx);
                if element.kind != "Modal" {
                    self.motion_modifiers(out, element);
                }
                if element.kind != "SplitView" {
                    self.inspector(out, &element.children, ctx);
                }
                if stagger.is_some() {
                    out.close("}");
                }
                self.placement(out, element);
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
                                    Some(format!("{}: {}", member(&param.name), self.expr(expr)))
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
            Node::If(node) => self.if_node(out, node, ctx),
            Node::IfLet(node) => self.if_let(out, node, ctx),
            Node::Match(node) => self.match_node(out, node, ctx),
        }
    }

    fn if_node(&self, out: &mut Out, node: &IfNode, ctx: &Ctx) {
        // Branches whose elements move animate the change of condition.
        let motion = node
            .branches
            .iter()
            .map(|(_, body)| body)
            .chain(std::iter::once(&node.otherwise))
            .find_map(|body| self.children_motion(body));
        if motion.is_some() {
            out.open("SwiftUI.Group {");
        }
        for (index, (condition, body)) in node.branches.iter().enumerate() {
            let keyword = if index == 0 { "if" } else { "} else if" };
            if index == 0 {
                out.open(format!("{keyword} {} {{", self.expr(condition)));
            } else {
                out.indent -= 1;
                out.open(format!("{keyword} {} {{", self.expr(condition)));
            }
            self.nodes(out, body, ctx);
        }
        if !node.otherwise.is_empty() {
            out.indent -= 1;
            out.open("} else {");
            self.nodes(out, &node.otherwise, ctx);
        }
        out.close("}");
        if let Some(motion) = motion {
            out.close("}");
            let conditions: Vec<String> = node
                .branches
                .iter()
                .map(|(condition, _)| self.expr(condition))
                .collect();
            out.line(format!(
                ".animation({motion}.animation, value: [{}])",
                conditions.join(", ")
            ));
        }
    }

    fn if_let(&self, out: &mut Out, node: &IfLetNode, ctx: &Ctx) {
        out.open(format!(
            "if let {} = {} {{",
            member(&node.var),
            self.expr(&node.value)
        ));
        self.nodes(out, &node.then, ctx);
        if !node.otherwise.is_empty() {
            out.indent -= 1;
            out.open("} else {");
            self.nodes(out, &node.otherwise, ctx);
        }
        out.close("}");
    }

    fn match_node(&self, out: &mut Out, node: &MatchNode, ctx: &Ctx) {
        let def = self.types().enum_def(&node.enum_name).expect("enum");
        // Like an if branch, a matched page needs an animated transaction
        // when its discriminator changes; a transition alone is inert.
        let motion = node
            .arms
            .iter()
            .find_map(|arm| self.children_motion(&arm.body));
        if motion.is_some() {
            out.open("SwiftUI.Group {");
        }
        out.open(format!("switch {} {{", self.expr(&node.value)));
        for arm in &node.arms {
            out.open(format!("{}:", case_pattern(def, arm)));
            self.nodes(out, &arm.body, ctx);
            out.indent -= 1;
        }
        out.close("}");
        if let Some(motion) = motion {
            out.close("}");
            out.line(format!(
                ".animation({motion}.animation, value: {})",
                self.expr(&node.value)
            ));
        }
    }

    fn element(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        match element.kind.as_str() {
            "Text" => self.text_view(out, element),
            "Canvas" => self.canvas(out, element),
            "Rail" => self.rail(out, element, ctx),
            "PopoverButton" => self.popover_button(out, element, ctx),
            "ControlGroup" => self.control_group(out, element, ctx),
            "SplitButton" => self.split_button(out, element, ctx),
            "Toast" => self.toast(out, element),
            "Label" => self.label_view(out, element),
            "Search" if element.keyword("style") == Some("palette") => {
                self.search_palette(out, element, ctx)
            }
            "Search" => self.page_search(out, element, ctx),
            "SearchItem" => self.search_item(out, element, ctx),
            "Button" | "IconButton" => self.button(out, element, ctx),
            "Drop" => self.drop_zone(out, element, ctx),
            "TextInput" => self.text_input(out, element, ctx),
            "Switch" => self.switch(out, element, ctx),
            "Slider" => self.slider(out, element, ctx),
            "Video" => self.video(out, element, ctx),
            "Query" => self.query(out, element, ctx),
            "Picker" => self.picker(out, element, ctx),
            "Form" => self.form(out, element, ctx),
            "Section" => self.section(out, element, ctx),
            "Column" | "Row" => self.column_or_row(out, element, ctx),
            "Accordion" => self.accordion(out, element, ctx),
            "Grid" => self.grid(out, element, ctx),
            "Marquee" => self.marquee(out, element, ctx),
            "ScaleBox" => self.scale_box(out, element, ctx),
            "Positioned" => self.positioned(out, element, ctx),
            "Modal" => self.modal(out, element, ctx),
            "Flow" => self.flow(out, element, ctx),
            "Timer" => self.timer(out, element),
            "Shortcut" => self.key_shortcut(out, element),
            "Stack" => self.stack(out, element, ctx),
            "Svg" => self.svg(out, element),
            "Image" => self.image(out, element),
            "Progress" => self.progress(out, element),
            "Spacer" if ctx.row => {
                out.line("SwiftUI.Spacer()");
                out.line(".layoutValue(key: BernRowSpacer.self, value: true)");
            }
            "Spacer" => out.line("SwiftUI.Spacer()"),
            // Only a stack along the row sets a divider upright.
            "Divider" if ctx.row => {
                out.open("SwiftUI.HStack(spacing: 0) {");
                out.line("SwiftUI.Divider()");
                out.close("}");
            }
            "Divider" => out.line("SwiftUI.Divider()"),
            "ScrollView" => self.scroll_view(out, element, ctx),
            "Pinch" | "Swipe" => self.gesture(out, element, ctx),
            "SplitView" => self.split_view(out, element, ctx),
            "Navigation" => self.navigation(out, element, ctx),
            "NavLink" => self.nav_link(out, element, ctx),
            "List" | "ForEach" => self.list(out, element, ctx),
            "Tabs" => self.tabs(out, element, ctx),
            "Adaptive" => self.adaptive(out, element, ctx),
            "Menu" => self.menu(out, element, ctx),
            "LanguagePicker" => self.language_picker(out, element, ctx),
            "Detail" | "Destination" | "Tab" | "Sidebar" | "SidebarItem" | "Toolbar"
            | "WindowSet" | "Window" | "Layout" | "MenuItem" => {
                // Realized by their parents.
            }
            other => out.line(format!("SwiftUI.EmptyView() // unsupported {other}")),
        }
    }
}

/// `case .variant(let a, _)` for one arm of a `match`, or `default`.
fn case_pattern(def: &EnumDef, arm: &MatchArm) -> String {
    let Some(index) = arm.variant else {
        return "default".into();
    };
    let variant = &def.variants[index as usize];
    if variant.fields.is_empty() {
        return format!("case .{}", case(&variant.name));
    }
    let patterns: Vec<_> = (0..variant.fields.len())
        .map(|field| {
            arm.bindings
                .iter()
                .find(|binding| binding.field == field)
                .map(|binding| format!("let {}", member(&binding.name)))
                .unwrap_or_else(|| "_".into())
        })
        .collect();
    format!("case .{}({})", case(&variant.name), patterns.join(", "))
}
