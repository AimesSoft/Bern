//! Layout containers: stacks, layers, rails, grids, scroll views,
//! forms, repetitions, queries and adaptive layouts.

use super::values::{decode, encode, kotlin_type};
use super::{Ctx, Generator, Out, Scope};
use crate::names::{double, float, member, string};
use bern_ir::{Element, LayoutSize, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// The direct children of a container: with `stagger`, those that
    /// appear take turns on the container's clock.
    fn children(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        match element.number("stagger") {
            Some(step) => {
                out.open(format!(
                    "BernStaggered(rememberBernStagger({})) {{",
                    double(step)
                ));
                let turns = Ctx {
                    stagger: true,
                    ..ctx.clone()
                };
                self.nodes(out, &element.children, &turns);
                out.close("}");
            }
            None => self.nodes(out, &element.children, ctx),
        }
    }

    /// `.padding(…)` of a container: `padding` on every edge, `padding_x`
    /// and `padding_y` in its place across and along.
    fn padding(&self, element: &Element) -> String {
        let all = self.length(element, "padding");
        let x = self.length(element, "padding_x");
        let y = self.length(element, "padding_y");
        match (all, x, y) {
            (None, None, None) => String::new(),
            (Some(all), None, None) => format!(".padding({all})"),
            (all, x, y) => {
                let zero = || "0.dp".to_string();
                format!(
                    ".padding(horizontal = {}, vertical = {})",
                    x.or_else(|| all.clone()).unwrap_or_else(zero),
                    y.or(all).unwrap_or_else(zero)
                )
            }
        }
    }

    /// `.width(…)` and `.height(…)` of a container that declares them.
    fn size_modifiers(&self, element: &Element) -> String {
        let mut modifiers = String::new();
        if let Some(width) = self.length(element, "width") {
            modifiers.push_str(&format!(".width({width})"));
        }
        if let Some(height) = self.length(element, "height") {
            modifiers.push_str(&format!(".height({height})"));
        }
        modifiers
    }

    pub(super) fn column_or_row(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let column = element.kind == "Column";
        let gap = self
            .length(element, "gap")
            .map(|gap| format!("Arrangement.spacedBy({gap})"))
            .unwrap_or_else(|| "Arrangement.spacedBy(8.dp)".into());
        // A column in a column (a row in a row) holding a spacer takes the
        // room its siblings leave, as on the other targets, instead of all
        // of it: a foot after it stays on screen.
        let along = element
            .prop(if column { "height" } else { "width" })
            .is_none()
            && !ctx.scroll_content
            && match (column, ctx.scope) {
                (true, Scope::Column) | (false, Scope::Row) => element
                    .children
                    .iter()
                    .any(|node| matches!(node, Node::Element(child) if child.kind == "Spacer")),
                _ => false,
            };
        let modifier = format!(
            "Modifier{}{}{}{}",
            if along { ".weight(1f)" } else { "" },
            self.reflow(element),
            self.size_modifiers(element),
            self.padding(element)
        );
        if column {
            let alignment = match element.keyword("align") {
                Some("center") => "Alignment.CenterHorizontally",
                Some("trailing") => "Alignment.End",
                _ => "Alignment.Start",
            };
            out.open(format!(
                "Column({modifier}, verticalArrangement = {gap}, horizontalAlignment = {alignment}) {{"
            ));
            self.children(out, element, &ctx.within(Scope::Column));
        } else {
            let alignment = match element.keyword("align") {
                Some("top") | Some("baseline") => "Alignment.Top",
                Some("bottom") => "Alignment.Bottom",
                _ => "Alignment.CenterVertically",
            };
            out.open(format!(
                "Row({modifier}, horizontalArrangement = {gap}, verticalAlignment = {alignment}) {{"
            ));
            let row = Ctx {
                baseline: element.keyword("align") == Some("baseline"),
                overflow: element.prop("height").map(|_| alignment),
                ..ctx.within(Scope::Row)
            };
            self.children(out, element, &row);
        }
        out.close("}");
    }

    /// The window room of a component's body, through the components and
    /// conditions it is made of.
    pub(super) fn window_body(&self, body: &Node) -> Option<String> {
        match body {
            Node::Element(element) => window_box(element),
            Node::Component(reference) => {
                self.window_body(&self.app.component(&reference.component)?.body)
            }
            // A condition: each branch is written in the window as well.
            Node::If(node) => self.window_branches(
                node.branches
                    .iter()
                    .map(|(_, body)| body.as_slice())
                    .chain([node.otherwise.as_slice()]),
            ),
            Node::IfLet(node) => {
                self.window_branches([node.then.as_slice(), node.otherwise.as_slice()].into_iter())
            }
            Node::Match(node) => {
                self.window_branches(node.arms.iter().map(|arm| arm.body.as_slice()))
            }
        }
    }

    /// The room of branches written in a window: theirs when they agree,
    /// else the whole window, which each branch then fills.
    fn window_branches<'a>(&self, branches: impl Iterator<Item = &'a [Node]>) -> Option<String> {
        let rooms: Vec<Option<String>> = branches
            .map(|body| match body {
                [node] => self.window_body(node),
                _ => None,
            })
            .collect();
        match rooms.first() {
            Some(first) if rooms.iter().all(|room| room == first) => first.clone(),
            _ if rooms.iter().any(Option::is_some) => Some(
                "Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopStart, propagateMinConstraints = true) {"
                    .into(),
            ),
            _ => None,
        }
    }

    pub(super) fn stack(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let alignment = stack_alignment(element.keyword("align"));
        // A styled stack draws its surface; a disabled one takes no
        // input.
        let style = self
            .style_of(element)
            .map(|style| format!(", style = BernStyles.{}", style.name));
        let enabled = element
            .expr("enabled")
            .map(|enabled| format!(", enabled = {}", self.expr(enabled)));
        // To the window's edges, under the app bar and the system bars.
        // With a backdrop (a canvas without a size) and no width of its
        // own, a stack in a column fills across it, as a block does, and
        // keeps the height its other layers give.
        let across = ctx.scope == Scope::Column
            && element.prop("width").is_none()
            && element.children.iter().any(|node| {
                matches!(node, Node::Element(child) if child.kind == "Canvas"
                    && child.prop("width").is_none()
                    && child.prop("height").is_none())
            });
        let under_bars = match (element.keyword("under_bars") == Some("true"), across) {
            (true, _) => ".bernUnderBars()",
            (false, true) => ".fillMaxWidth()",
            (false, false) => "",
        };
        if style.is_some() || enabled.is_some() {
            out.open(format!(
                "BernStack(Modifier{}{}{under_bars}, {alignment}{}{}) {{",
                self.reflow(element),
                self.size_modifiers(element),
                style.unwrap_or_default(),
                enabled.unwrap_or_default()
            ));
        } else {
            out.open(format!(
                "Box(Modifier{}{}{under_bars}, contentAlignment = {alignment}) {{",
                self.reflow(element),
                self.size_modifiers(element)
            ));
        }
        let backdrop = |node: &Node| match node {
            Node::Element(child) => {
                child.kind == "Rail"
                    || (child.kind == "Canvas"
                        && child.prop("width").is_none()
                        && child.prop("height").is_none())
            }
            _ => false,
        };
        let layers = Ctx {
            layer: true,
            layer_align: alignment,
            sized_layers: element.children.iter().any(|node| !backdrop(node)),
            ..ctx.within(Scope::Other)
        };
        self.children(out, element, &layers);
        out.close("}");
    }

    pub(super) fn scale_box(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernScaleBox({}, {}) {{",
            self.length(element, "width")
                .unwrap_or_else(|| "0.dp".into()),
            self.length(element, "height")
                .unwrap_or_else(|| "0.dp".into())
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn positioned(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let length = |name| self.length(element, name).unwrap_or_else(|| "0.dp".into());
        // A layer that bleeds learns where it is going before any motion,
        // to know which edges of its scene it reaches.
        let bleed = element
            .expr("bleed")
            .map(|on| {
                let place = |name| match element.prop(name) {
                    Some(Prop::Number { value }) => format!("{}.dp", float(*value)),
                    Some(Prop::Expr { expr }) => format!("({}).toFloat().dp", self.expr(expr)),
                    _ => "0.dp".into(),
                };
                format!(
                    ", bleed = BernBleed({}, {}, {}, {}, {}, {})",
                    self.expr(on),
                    place("x"),
                    place("y"),
                    place("width"),
                    place("height"),
                    element
                        .keyword("animate")
                        .map(|token| crate::style::motion(self.app, token))
                        .unwrap_or_else(|| "null".into())
                )
            })
            .unwrap_or_default();
        out.open(format!(
            "BernPositioned({}, {}, {}, {}{bleed}) {{",
            length("x"),
            length("y"),
            length("width"),
            length("height")
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn marquee(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let value = |name, default: &str| {
            element
                .expr(name)
                .map(|e| self.expr(e))
                .unwrap_or_else(|| default.into())
        };
        let duration = match element.prop("duration") {
            Some(Prop::Number { value }) => double(*value),
            Some(Prop::Expr { expr }) => format!("({}).toDouble()", self.expr(expr)),
            _ => "8.0".into(),
        };
        out.open(format!(
            "BernMarquee({duration}, {}, {}, ({}).toFloat()) {{",
            double(element.number("phase").unwrap_or(0.0)),
            value("paused", "false"),
            value("opacity", "1"),
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn accordion(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let modifier = if element.prop("height").is_none()
            && ctx.scope == Scope::Column
            && !ctx.scroll_content
        {
            ", modifier = Modifier.weight(1f)"
        } else {
            ""
        };
        let number = |name| match element.prop(name) {
            Some(Prop::Expr { expr }) => format!("({}).toInt()", self.expr(expr)),
            Some(Prop::Number { value }) => format!("{}", *value as i64),
            _ => "0".into(),
        };
        let motion = element
            .keyword("animate")
            .map(|token| crate::style::motion(self.app, token))
            .unwrap_or_else(|| "null".into());
        out.open(format!(
            "BernAccordion({}, {}, {}, {}, {}, {}, {motion}{modifier}) {{",
            number("count"),
            number("selected"),
            self.length(element, "collapsed_width")
                .unwrap_or_else(|| "86.dp".into()),
            self.length(element, "gap").unwrap_or_else(|| "8.dp".into()),
            self.length(element, "height")
                .unwrap_or_else(|| "null".into()),
            self.length(element, "corner")
                .unwrap_or_else(|| "0.dp".into()),
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn flow(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernFlow({}) {{",
            self.length(element, "gap").unwrap_or_else(|| "8.dp".into())
        ));
        self.children(out, element, &ctx.within(Scope::Flow));
        out.close("}");
    }

    pub(super) fn modal(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        // The modal's own transitions play on its card.
        let enter = match element.keyword("appear") {
            Some(name) => self.enter(Some(name), element.number("appear_delay")),
            None => "null".into(),
        };
        let exit = match element.keyword("disappear") {
            Some(name) => self.exit(Some(name)),
            None => "null".into(),
        };
        out.open(format!(
            "BernModal({}, {}, {}, {enter}, {exit}) {{",
            self.length(element, "width")
                .unwrap_or_else(|| "960.dp".into()),
            // Without a height, as tall as its content.
            self.length(element, "height")
                .unwrap_or_else(|| "androidx.compose.ui.unit.Dp.Unspecified".into()),
            element.keyword("cover") == Some("compact"),
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    /// `Shortcut`: its key runs the action while it is composed and
    /// enabled, without a control of its own.
    pub(super) fn key_shortcut(&self, out: &mut Out, element: &Element) {
        let enabled = self.enabled(element);
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_else(|| "Unit".into());
        out.line(format!(
            "BernShortcut({}, {{ {enabled} }}) {{ {action} }}",
            self.shortcut_keys(element, "key")
                .unwrap_or_else(|| string(""))
        ));
    }

    pub(super) fn timer(&self, out: &mut Out, element: &Element) {
        let enabled = self.enabled(element);
        let action = element
            .action
            .as_ref()
            .map(|a| self.action(a))
            .unwrap_or_else(|| "Unit".into());
        out.line(format!(
            "BernTimer({}, {enabled}) {{ {action} }}",
            double(element.number("interval").unwrap_or(1.0))
        ));
    }

    pub(super) fn control_group(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernControlGroup({}, {}) {{",
            self.text_prop(element, "label")
                .unwrap_or_else(|| "null".into()),
            self.length(element, "height")
                .unwrap_or_else(|| "null".into())
        ));
        self.nodes(out, &element.children, &ctx.within(Scope::Row));
        out.close("}");
    }

    pub(super) fn search_item(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self.text_prop(element, "label").unwrap_or_default();
        let (action, file) = self.pressed(element);
        let target = self.target(element, ctx);
        let or_null = |name| {
            self.text_prop(element, name)
                .unwrap_or_else(|| "null".into())
        };
        let navigation = element
            .expr("navigation")
            .map(|e| self.expr(e))
            .unwrap_or_else(|| "true".into());
        // Choosing a result closes the palette first, also from an
        // external entry.
        let head = format!(
            "BernSearchItem({target}, {label}, {}, {}, {}, {navigation}, BernControlActions(activate = {{ {action} }}{file}))",
            Self::icon(element.icon("icon")),
            or_null("detail"),
            or_null("hint"),
        );
        if element.children.is_empty() {
            out.line(head);
        } else {
            out.open(format!("{head} {{"));
            self.nodes(out, &element.children, &ctx.within(Scope::Other));
            out.close("}");
        }
    }

    pub(super) fn rail(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let anchor = match element.keyword("anchor") {
            Some("start") => "BernRailAnchor.Start",
            Some("end") => "BernRailAnchor.End",
            _ => "BernRailAnchor.Center",
        };
        // A rail among the layers of a stack never makes the stack larger.
        // Holding only spans without a thickness of their own it is as
        // large as the stack; otherwise, beside layers that give the stack
        // its size, it takes the stack's length along its axis, its own
        // thickness across, and stands where the stack aligns its layers,
        // as a progress bar at the foot of a picture. A stack of rails
        // alone takes their size.
        let vertical = element.keyword("axis") == Some("vertical");
        let fills = Self::rail_fills(element);
        let aligned = ctx.layer && !fills && ctx.sized_layers;
        // In a row (a column, for a vertical rail) it takes the room the
        // others leave, after them, as a flexible view does.
        let modifier = if ctx.layer && fills {
            ", Modifier.matchParentSize()"
        } else if aligned && vertical {
            ", Modifier.fillMaxHeight()"
        } else if aligned {
            ", Modifier.fillMaxWidth()"
        } else if matches!(
            (ctx.scope, vertical),
            (Scope::Row, false) | (Scope::Column, true)
        ) {
            ", Modifier.weight(1f)"
        } else {
            ""
        };
        if aligned {
            out.open(format!(
                "Box(Modifier.matchParentSize(), contentAlignment = {}) {{",
                ctx.layer_align
            ));
        }
        let overhang = self
            .length(element, "overhang")
            .map(|overhang| format!(", overhang = {overhang}"))
            .unwrap_or_default();
        out.open(format!(
            "BernRail({}, {anchor}{modifier}{overhang}) {{",
            element.keyword("axis") == Some("vertical")
        ));
        let placed = Ctx {
            layer: false,
            ..ctx.within(Scope::Other)
        };
        self.nodes(out, &element.children, &placed);
        out.close("}");
        if aligned {
            out.close("}");
        }
    }

    /// The arguments of `BernPlace` for an element on a rail: its fraction
    /// `at`, or `from` and `to`, and the spring it moves with.
    pub(super) fn placement(&self, element: &Element) -> Option<String> {
        let fraction = |name: &str| match element.prop(name)? {
            Prop::Expr { expr } => Some(format!("({}).toDouble()", self.expr(expr))),
            Prop::Number { value } => Some(double(*value)),
            _ => None,
        };
        let (at, from, to) = (fraction("at"), fraction("from"), fraction("to"));
        if at.is_none() && from.is_none() && to.is_none() {
            return None;
        }
        let motion = element
            .keyword("animate")
            .map(|token| crate::style::motion(self.app, token))
            .unwrap_or_else(|| "null".into());
        let or_null = |value: Option<String>| value.unwrap_or_else(|| "null".into());
        Some(format!(
            "{}, {}, {}, {motion}",
            or_null(at),
            or_null(from),
            or_null(to)
        ))
    }

    /// Whether every element a rail places spans between two fractions
    /// without a size of its own across the rail, so that the rail has no
    /// thickness but what its parent gives it.
    pub(super) fn rail_fills(rail: &Element) -> bool {
        let across = if rail.keyword("axis") == Some("vertical") {
            "width"
        } else {
            "height"
        };
        let mut placed = Vec::new();
        Self::placed(&rail.children, &mut placed)
            && !placed.is_empty()
            && placed.iter().all(|element| {
                element.prop("from").is_some()
                    && element.prop("at").is_none()
                    && element.prop(across).is_none()
            })
    }

    /// The elements a rail places among `nodes`, through conditions and
    /// repetitions, which add no layout of their own; false when one is a
    /// component, whose size is not known here.
    fn placed<'n>(nodes: &'n [Node], found: &mut Vec<&'n Element>) -> bool {
        nodes.iter().all(|node| match node {
            Node::Element(element) if element.kind == "ForEach" => {
                Self::placed(&element.children, found)
            }
            Node::Element(element) => {
                found.push(element);
                true
            }
            Node::Component(_) => false,
            Node::If(node) => {
                node.branches
                    .iter()
                    .all(|(_, body)| Self::placed(body, found))
                    && Self::placed(&node.otherwise, found)
            }
            Node::IfLet(node) => {
                Self::placed(&node.then, found) && Self::placed(&node.otherwise, found)
            }
            Node::Match(node) => node.arms.iter().all(|arm| Self::placed(&arm.body, found)),
        })
    }

    pub(super) fn grid(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let gap = self.length(element, "gap").unwrap_or_else(|| "8.dp".into());
        let mut extra = String::new();
        if let Some(row_gap) = self.length(element, "row_gap") {
            extra.push_str(&format!(", rowGap = {row_gap}"));
        }
        // With `cell_ratio`, every row is as tall as the column
        // width gives.
        if let Some(ratio) = element.number("cell_ratio") {
            extra.push_str(&format!(
                ", cellRatio = {}, cellExtra = {}",
                float(ratio),
                self.length(element, "cell_extra")
                    .unwrap_or_else(|| "0.dp".into())
            ));
        }
        if element.keyword("masonry") == Some("true") {
            extra.push_str(", masonry = true");
        }
        let padding = self.padding(element);
        if !padding.is_empty() {
            extra.push_str(&format!(", modifier = Modifier{padding}"));
        }
        out.open(format!(
            "BernGrid(minWidth = {}, gap = {gap}{extra}) {{",
            self.length(element, "min_width")
                .unwrap_or_else(|| "120.dp".into()),
        ));
        self.children(out, element, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn form(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        if let Some(title) = self.text_prop(element, "title") {
            out.line(format!("BernChrome(title = {title})"));
        }
        // A form opening with a plain section starts at the top of its
        // page: the form sees that as it lays out, so it follows a heading
        // that a slot, a component or a condition gives.
        let or_null = |name| self.length(element, name).unwrap_or_else(|| "null".into());
        out.open(format!(
            "BernForm(gap = {}, rows = BernRows({}, {}, {})) {{",
            or_null("gap"),
            or_null("row_height"),
            or_null("padding_x"),
            or_null("padding_y"),
        ));
        // The form scrolls: its rows cannot share its height by weight.
        let rows = Ctx {
            scroll_content: true,
            ..ctx.within(Scope::Column)
        };
        self.nodes(out, &element.children, &rows);
        out.close("}");
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let title = self
            .text_prop(element, "title")
            .unwrap_or_else(|| "null".into());
        let footer = self
            .text_prop(element, "footer")
            .unwrap_or_else(|| "null".into());
        let mut extra = String::new();
        if element.keyword("style") == Some("plain") {
            extra.push_str(", plain = true");
        }
        if let Some(x) = self.length(element, "padding_x") {
            extra.push_str(&format!(", paddingX = {x}"));
        }
        if let Some(y) = self.length(element, "padding_y") {
            extra.push_str(&format!(", paddingY = {y}"));
        }
        out.open(format!("BernSection({title}, {footer}{extra}) {{"));
        self.nodes(out, &element.children, &ctx.within(Scope::Other));
        out.close("}");
    }

    pub(super) fn tracked_scroll_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let horizontal = element.keyword("axis") == Some("horizontal");
        let snap = element
            .keyword("snap")
            .map(string)
            .unwrap_or_else(|| "null".into());
        let (position, on_position) =
            match (self.field(element, "position"), element.field("position")) {
                (Some(field), Some(field_ref)) => (
                    field.clone(),
                    format!("{{ {field} = it as {} }}", kotlin_type(&field_ref.ty)),
                ),
                _ => ("null".into(), "null".into()),
            };
        let arrival = if element.keyword("arrival").is_some() {
            crate::style::motion(self.app, "jelly")
        } else {
            "null".into()
        };
        // It takes the free room of the column or row it is in, as
        // a plain scroll view does.
        let flexible = match (ctx.scope, horizontal) {
            (Scope::Row, _) | (Scope::Column, false) => "Modifier.weight(1f)",
            (Scope::Column, true) => "Modifier",
            (Scope::Other | Scope::Flow, true) => "Modifier.bernFlexible(column = false)",
            (Scope::Other | Scope::Flow, false) => "Modifier.bernFlexible().fillMaxHeight()",
        };
        out.open(format!(
            "BernScroll({horizontal}, {snap}, {position}, {on_position}, {}, {arrival}, 0.dp, {flexible}) {{",
            crate::style::motion(self.app, "smooth")
        ));
        // Its content lambda has no receiver: the column or row it
        // lays out in is passed down through `LocalBernFlex`.
        let inner = Ctx {
            scroll_items: true,
            ..ctx.within(Scope::Other)
        };
        self.nodes(out, &element.children, &inner);
        out.close("}");
    }

    pub(super) fn scroll_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let center = match element.prop("open_center") {
            Some(Prop::Number { value }) => Some(float(*value)),
            Some(Prop::Expr { expr }) => Some(format!("({}).toFloat()", self.expr(expr))),
            _ => None,
        };
        let state = element
            .number("open_at")
            .map(|at| format!("rememberBernScroll({})", float(at)))
            .or_else(|| center.map(|at| format!("rememberBernCenterScroll({at})")))
            .unwrap_or_else(|| "rememberScrollState()".into());
        // A scroll view takes the free room of the column or row it
        // is in (a horizontal one only along a row), as on the
        // other targets.
        if element.keyword("axis") == Some("horizontal") {
            let flexible = match ctx.scope {
                Scope::Row => "Modifier.weight(1f)",
                Scope::Column => "Modifier",
                Scope::Other | Scope::Flow => "Modifier.bernFlexible(column = false)",
            };
            out.open(format!("Row({flexible}.horizontalScroll({state}), horizontalArrangement = Arrangement.spacedBy(8.dp)) {{"));
            self.nodes(out, &element.children, &ctx.within(Scope::Row));
        } else {
            // In a stack it takes the stack's height, as a scroll view
            // does in a layered stack on the other targets, so that a
            // short page does not shrink to its content.
            let flexible = match ctx.scope {
                Scope::Column | Scope::Row => "Modifier.weight(1f)",
                Scope::Other | Scope::Flow => "Modifier.bernFlexible().fillMaxHeight()",
            };
            out.open(format!(
                "Column({flexible}.fillMaxWidth().verticalScroll({state})) {{",
            ));
            let content = Ctx {
                scroll_content: true,
                ..ctx.within(Scope::Column)
            };
            self.nodes(out, &element.children, &content);
        }
        out.close("}");
    }

    pub(super) fn preview_list(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        if let (Some(items), Some(template), Some(Prop::Key { name, .. })) =
            (element.expr("items"), &element.template, element.prop("key"))
        {
            let item = element.number("item_height").unwrap_or(1.0);
            let gap = element.number("gap").unwrap_or(0.0).max(0.0);
            let more = element.number("more_height").unwrap_or(22.0).max(0.0);
            let modifier = if ctx.scope == Scope::Column && !ctx.scroll_content {
                "Modifier.fillMaxWidth().weight(1f)"
            } else { "Modifier.fillMaxSize()" };
            let var = member(&template.var);
            out.open(format!("BoxWithConstraints({modifier}.clipToBounds()) {{"));
            out.line(format!("val previewItems = {}", self.expr(items)));
            out.line("val previewHeight = maxHeight.value");
            out.line(format!("val previewCount = if (!previewHeight.isFinite() || previewHeight <= 0f || {item} <= 0.0) 0 else if (previewItems.size * {item} + (previewItems.size - 1) * {gap} <= previewHeight) previewItems.size else ((previewHeight.toDouble() - {more}).coerceAtLeast(0.0) / {}).toInt().coerceIn(0, previewItems.size)", item + gap));
            out.open(format!("Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy({gap}.dp), horizontalAlignment = Alignment.CenterHorizontally) {{"));
            out.open(format!("for ({var} in previewItems.take(previewCount)) {{"));
            out.open(format!("key(BernKey.of({var}.{})) {{", member(name)));
            out.open(format!("Box(Modifier.height({item}.dp), contentAlignment = Alignment.TopCenter) {{"));
            self.nodes(out, &element.children, &ctx.within(Scope::Other));
            out.close("}"); out.close("}"); out.close("}");
            out.open(format!("if (previewCount < previewItems.size && previewHeight >= {more}) {{"));
            out.open(format!("Box(Modifier.height({more}.dp).background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(50)).padding(horizontal = 6.dp), contentAlignment = Alignment.Center) {{"));
            out.line("Text(\"+${previewItems.size - previewCount}\", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)");
            out.close("}"); out.close("}"); out.close("}"); out.close("}");
        }
    }

    pub(super) fn list(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let lazy = element.kind == "List";
        // With `stagger`, the rows take turns appearing; without, rows of a
        // repetition are direct children of the container it stands in.
        let stagger = element.number("stagger");
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                let var = member(&template.var);
                let key = format!("{var}.{}", member(name));
                let mut inner = ctx.clone();
                inner.key = Some(key.clone());
                if stagger.is_some() {
                    inner.stagger = true;
                }
                // The clock reaches the rows of a lazy list too: they are
                // composed with the list's locals.
                if let Some(step) = stagger {
                    out.open(format!(
                        "BernStaggered(rememberBernStagger({})) {{",
                        double(step)
                    ));
                }
                if lazy {
                    out.open("LazyColumn(Modifier.fillMaxSize()) {");
                    out.open(format!(
                        "items({}, key = {{ {var} -> BernKey.of({key}) }}) {{ {var} ->",
                        self.expr(items)
                    ));
                    out.open(format!(
                        "CompositionLocalProvider(LocalBernItemKeys provides LocalBernItemKeys.current + \"[${{{key}}}]\") {{"
                    ));
                    out.open("Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {");
                    self.nodes(out, &element.children, &inner.within(Scope::Column));
                    out.close("}");
                    out.close("}");
                    out.line("HorizontalDivider()");
                    out.close("}");
                    out.close("}");
                } else {
                    out.open(format!("for ({var} in {}) {{", self.expr(items)));
                    out.open(format!("key(BernKey.of({key})) {{"));
                    out.open(format!(
                        "CompositionLocalProvider(LocalBernItemKeys provides LocalBernItemKeys.current + \"[${{{key}}}]\") {{"
                    ));
                    if ctx.scroll_items {
                        inner.scroll_items = false;
                        out.open(format!("Box(Modifier.bernScrollItem({key})) {{"));
                        self.nodes(out, &element.children, &inner.within(Scope::Other));
                        out.close("}");
                    } else {
                        self.nodes(out, &element.children, &inner);
                    }
                    out.close("}");
                    out.close("}");
                    out.close("}");
                }
                if stagger.is_some() {
                    out.close("}");
                }
            }
            _ => {
                if lazy {
                    out.open(
                        "Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {",
                    );
                    let content = Ctx {
                        scroll_content: true,
                        ..ctx.within(Scope::Column)
                    };
                    self.children(out, element, &content);
                    out.close("}");
                } else {
                    self.children(out, element, ctx);
                }
            }
        }
    }

    /// A `Query`: the runtime view owns the subscription and shows the
    /// loading, refreshing, failed and empty states; the template shows the
    /// data. Its parameters are encoded on every composition; the runtime
    /// requests them only when they change.
    pub(super) fn query(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let data = kotlin_type(&binding.result);
        let encode: Vec<_> = binding
            .args
            .iter()
            .map(|arg| encode(&arg.ty, &format!("({})", self.expr(arg)), "writer"))
            .collect();
        let params = format!(
            "run {{ val writer = BernWriter(); {}; writer.bytes() }}",
            encode.join("; ")
        );
        let framework = |key: &str| format!("BernText.string({})", string(key));
        let loading = self
            .text_prop(element, "loading")
            .unwrap_or_else(|| framework("bern.query.loading"));
        let failed = self
            .text_prop(element, "failed")
            .unwrap_or_else(|| framework("bern.query.failed"));
        let empty = self.text_prop(element, "empty");
        let icon = element.icon("icon").unwrap_or("tray");
        // The declared empty rule: an empty list or a missing optional.
        let (shown, count) = match &binding.result {
            Type::List { .. } if empty.is_some() => (
                "if (data.isEmpty()) null else data",
                format!("{{ data: {data} -> data.size }}"),
            ),
            Type::List { .. } => ("data", format!("{{ data: {data} -> data.size }}")),
            Type::Optional { .. } => (
                "data",
                format!("{{ data: {data} -> if (data == null) 0 else 1 }}"),
            ),
            _ => ("data", "null".to_string()),
        };
        out.open("BernQueryView(");
        out.line(format!("{}, {},", binding.query, self.target(element, ctx)));
        out.line(format!("{params},"));
        out.line(format!(
            "{loading}, {}, {}, {failed},",
            empty.unwrap_or_else(|| "null".into()),
            string(icon)
        ));
        out.line(format!(
            "{{ reader -> {} }},",
            decode(&binding.result, "reader")
        ));
        out.line(format!("{count},"));
        out.line(format!("{{ data: {data} -> {shown} }},"));
        out.close(format!(") {{ {} ->", member(&template.var)));
        out.indent += 1;
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn adaptive(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let target = if element.id.is_some() {
            self.target(element, ctx)
        } else {
            "null".into()
        };
        // The breakpoints are the application's, in `BernHost.layouts`; a
        // size without its own layout shows another one.
        out.open(format!("BernAdaptive({target},"));
        for size in LayoutSize::ALL {
            let layout = element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size.name()) => {
                    Some(layout)
                }
                _ => None,
            });
            match layout {
                Some(layout) => {
                    out.open(format!("{} = {{", size.name()));
                    self.nodes(out, &layout.children, &ctx.within(Scope::Other));
                    out.close("},");
                }
                None => out.line(format!("{} = null,", size.name())),
            }
        }
        // The layout a television shows, chosen by the device.
        if let Some(layout) = element.children.iter().find_map(|node| match node {
            Node::Element(layout) if layout.keyword("size") == Some("tv") => Some(layout),
            _ => None,
        }) {
            out.open("tv = {");
            self.nodes(out, &layout.children, &ctx.within(Scope::Other));
            out.close("},");
        }
        out.close(")");
    }
}

/// Where a stack aligns its layers.
fn stack_alignment(align: Option<&str>) -> &'static str {
    match align {
        Some("top_leading") => "Alignment.TopStart",
        Some("top") => "Alignment.TopCenter",
        Some("top_trailing") => "Alignment.TopEnd",
        Some("leading") => "Alignment.CenterStart",
        Some("trailing") => "Alignment.CenterEnd",
        Some("bottom_leading") => "Alignment.BottomStart",
        Some("bottom") => "Alignment.BottomCenter",
        Some("bottom_trailing") => "Alignment.BottomEnd",
        _ => "Alignment.Center",
    }
}

/// The box around an element written directly in a window, which starts
/// at the window's top leading corner: a column without a width spans the
/// window's width, its alignment placing its items across it, and a stack
/// without a size fills the window, its alignment placing its layers there.
pub(super) fn window_box(element: &Element) -> Option<String> {
    match element.kind.as_str() {
        "Column" if element.prop("width").is_none() => Some(format!(
            "Box(Modifier.fillMaxWidth(), contentAlignment = {}) {{",
            match element.keyword("align") {
                Some("center") => "Alignment.TopCenter",
                Some("trailing") => "Alignment.TopEnd",
                _ => "Alignment.TopStart",
            }
        )),
        // `under_bars` fills the window already.
        "Stack"
            if element.prop("width").is_none()
                && element.prop("height").is_none()
                && element.keyword("under_bars") != Some("true") =>
        {
            // The stack itself fills it, its surface too, not only the room
            // its layers are placed in.
            Some(format!(
                "Box(Modifier.fillMaxSize(), contentAlignment = {}, propagateMinConstraints = true) {{",
                stack_alignment(element.keyword("align"))
            ))
        }
        _ => None,
    }
}
