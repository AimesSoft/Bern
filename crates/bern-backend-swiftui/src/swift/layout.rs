//! Layout containers: stacks, layers, rails, grids, scroll views,
//! forms, repetitions, queries and adaptive layouts.

use super::values::swift_type;
use super::{Ctx, Generator, Out};
use crate::names::{double, member, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// A container's declared `width` and `height`.
    fn frame(&self, out: &mut Out, element: &Element) {
        let width = self.length(element, "width");
        let height = self.length(element, "height");
        if width.is_none() && height.is_none() {
            return;
        }
        out.line(format!(
            ".frame(width: {}, height: {})",
            width.unwrap_or_else(|| "nil".into()),
            height.unwrap_or_else(|| "nil".into())
        ));
    }

    /// `padding` on every edge; `padding_x` and `padding_y` on the
    /// horizontal and vertical edges, in place of `padding` there.
    fn padding(&self, out: &mut Out, element: &Element) {
        let all = self.length(element, "padding");
        let x = self.length(element, "padding_x");
        let y = self.length(element, "padding_y");
        match (all, x.as_ref(), y.as_ref()) {
            (Some(all), None, None) => out.line(format!(".padding({all})")),
            (all, x, y) => {
                if let Some(x) = x.cloned().or(all.clone()) {
                    out.line(format!(".padding(.horizontal, {x})"));
                }
                if let Some(y) = y.cloned().or(all) {
                    out.line(format!(".padding(.vertical, {y})"));
                }
            }
        }
    }

    pub(super) fn column_or_row(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (stack, alignment) = if element.kind == "Column" {
            (
                "VStack",
                match element.keyword("align") {
                    Some("center") => ".center",
                    Some("trailing") => ".trailing",
                    _ => ".leading",
                },
            )
        } else {
            (
                "HStack",
                match element.keyword("align") {
                    Some("top") => ".top",
                    Some("bottom") => ".bottom",
                    Some("baseline") => ".firstTextBaseline",
                    _ => ".center",
                },
            )
        };
        let spacing = self
            .length(element, "gap")
            .map(|gap| format!(", spacing: {gap}"))
            .unwrap_or_default();
        out.open(format!(
            "SwiftUI.{stack}(alignment: {alignment}{spacing}) {{"
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        self.padding(out, element);
        self.frame(out, element);
        self.toolbar(out, &element.children, ctx);
    }

    pub(super) fn stack(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let alignment = match element.keyword("align") {
            Some("top_leading") => ".topLeading",
            Some("top") => ".top",
            Some("top_trailing") => ".topTrailing",
            Some("leading") => ".leading",
            Some("trailing") => ".trailing",
            Some("bottom_leading") => ".bottomLeading",
            Some("bottom") => ".bottom",
            Some("bottom_trailing") => ".bottomTrailing",
            _ => ".center",
        };
        // A rail among the layers takes the size of the others.
        let rails = has_rail_layer(&element.children);
        out.open(if rails {
            format!("BernLayers(alignment: {alignment}) {{")
        } else {
            format!("SwiftUI.ZStack(alignment: {alignment}) {{")
        });
        self.nodes(out, &element.children, ctx);
        out.close("}");
        self.frame(out, element);
        if let Some(enabled) = element.expr("enabled") {
            let enabled = self.expr(enabled);
            // `disabled` already stops its controls and gestures.
            // Turning hit testing off as well takes a split view's
            // toolbar safe area away on macOS: its columns then
            // slide under the title bar.
            out.line(format!(
                ".disabled(!({enabled})).accessibilityHidden(!({enabled}))"
            ));
        }
        if let Some(style) = self.style_of(element) {
            out.line(format!(
                ".modifier(BernSurface(style: BernStyles.{}))",
                style.name
            ));
        }
    }

    pub(super) fn rail(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let vertical = element.keyword("axis") == Some("vertical");
        let anchor = match element.keyword("anchor") {
            Some("start") => "0",
            Some("end") => "1",
            _ => "0.5",
        };
        out.open(format!(
            "BernRail(vertical: {vertical}, anchor: {anchor}) {{"
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        // The rail takes the length it is given.
        out.line(if vertical {
            ".frame(maxHeight: .infinity)"
        } else {
            ".frame(maxWidth: .infinity)"
        });
        out.line(".bernFillsLayers()");
    }

    /// Where an element sits on the `Rail` it stands in.
    pub(super) fn placement(&self, out: &mut Out, element: &Element) {
        let fraction = |name: &str| {
            element
                .expr(name)
                .map(|expr| format!("Double({})", self.expr(expr)))
        };
        let motion = element
            .keyword("animate")
            .map(|token| crate::style::motion(self.app, token))
            .unwrap_or_else(|| "nil".into());
        if let Some(at) = fraction("at") {
            out.line(format!(".bernRailAt({at}, motion: {motion})"));
        } else if let (Some(from), Some(to)) = (fraction("from"), fraction("to")) {
            out.line(format!(
                ".bernRailSpan(from: {from}, to: {to}, motion: {motion})"
            ));
        }
    }

    pub(super) fn grid(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let gap = self.length(element, "gap").unwrap_or_else(|| double(8.0));
        let row_gap = self
            .length(element, "row_gap")
            .unwrap_or_else(|| gap.clone());
        let motion = element
            .keyword("animate")
            .map(|token| crate::style::motion(self.app, token))
            .unwrap_or_else(|| "nil".into());
        out.open(format!(
            "BernAdaptiveGrid(minimum: {}, gap: {gap}, rowGap: {row_gap}, motion: {motion}) {{",
            self.length(element, "min_width")
                .unwrap_or_else(|| double(120.0))
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
        out.line(".frame(maxWidth: .infinity, alignment: .leading)");
        self.padding(out, element);
    }

    pub(super) fn flow(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernFlow(gap: {}) {{",
            self.length(element, "gap").unwrap_or_else(|| "8.0".into())
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn accordion(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let motion = element
            .keyword("animate")
            .map(|token| crate::style::motion(self.app, token))
            .unwrap_or_else(|| "nil".into());
        let number = |name| {
            element
                .expr(name)
                .map(|expr| self.expr(expr))
                .unwrap_or_else(|| "0".into())
        };
        out.open(format!(
            "BernAccordion(count: Int({}), selected: Int({}), collapsedWidth: {}, gap: {}, height: {}, motion: {motion}) {{",
            number("count"), number("selected"),
            self.length(element, "collapsed_width").unwrap_or_else(|| "86.0".into()),
            self.length(element, "gap").unwrap_or_else(|| "8.0".into()),
            self.length(element, "height").unwrap_or_else(|| "560.0".into()),
        ));
        self.nodes(
            out,
            &element.children,
            &Ctx {
                accordion_corner: Some(
                    self.length(element, "corner")
                        .unwrap_or_else(|| "0.0".into()),
                ),
                ..ctx.clone()
            },
        );
        out.close("}");
    }

    pub(super) fn positioned(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open("SwiftUI.ZStack(alignment: .topLeading) {");
        self.nodes(out, &element.children, ctx);
        out.close("}");
        self.frame(out, element);
        out.line(format!(
            ".offset(x: {}, y: {})",
            self.length(element, "x").unwrap(),
            self.length(element, "y").unwrap()
        ));
        out.line(".frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)");
    }

    pub(super) fn scale_box(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernScaleBox(width: {}, height: {}) {{",
            self.length(element, "width").unwrap(),
            self.length(element, "height").unwrap()
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn marquee(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernMarquee(duration: {}, phase: {}, paused: {}, opacity: {}) {{",
            self.length(element, "duration").unwrap(),
            double(element.number("phase").unwrap_or(0.0)),
            element
                .expr("paused")
                .map(|e| self.expr(e))
                .unwrap_or_else(|| "false".into()),
            element
                .expr("opacity")
                .map(|e| format!("Double({})", self.expr(e)))
                .unwrap_or_else(|| "1.0".into())
        ));
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    pub(super) fn modal(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open(format!(
            "BernModal(width: {}, height: {}) {{",
            self.length(element, "width")
                .unwrap_or_else(|| "960.0".into()),
            self.length(element, "height")
                .unwrap_or_else(|| "600.0".into())
        ));
        self.nodes(out, &element.children, ctx);
        out.close("} presentation: { modal in");
        out.indent += 1;
        out.open("AnyView(modal");
        self.motion_modifiers(out, element);
        out.close(")");
        out.close("}");
    }

    pub(super) fn form(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        out.open("SwiftUI.Form {");
        self.nodes(out, &element.children, ctx);
        out.close("}");
        out.line(".formStyle(.grouped)");
        if let Some(title) = self.text_prop(element, "title") {
            out.line(format!(".navigationTitle({title})"));
        }
        self.toolbar(out, &element.children, ctx);
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let title = self.text_prop(element, "title");
        let footer = self.text_prop(element, "footer");
        out.open("SwiftUI.Section {");
        self.nodes(out, &element.children, ctx);
        match (&title, &footer) {
            (None, None) => out.close("}"),
            _ => {
                if let Some(title) = &title {
                    out.close("} header: {");
                    out.indent += 1;
                    out.line(format!("SwiftUI.Text(verbatim: {title})"));
                }
                if let Some(footer) = &footer {
                    out.close("} footer: {");
                    out.indent += 1;
                    out.line(format!("SwiftUI.Text(verbatim: {footer})"));
                }
                out.close("}");
            }
        }
    }

    pub(super) fn scroll_view(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let horizontal = element.keyword("axis") == Some("horizontal");
        let axis = if horizontal {
            ".horizontal"
        } else {
            ".vertical"
        };
        let snap = element.keyword("snap");
        let position = self.field(element, "position");
        // The scroll view's entrance moves what it scrolls, which
        // mounts with it: it runs on appear.
        let entrance = element
            .keyword("appear")
            .and_then(|_| self.transition_line(element, true));
        out.open(format!("SwiftUI.ScrollView({axis}) {{"));
        if snap.is_some() || position.is_some() {
            // The items to rest on and to scroll to are those of
            // the layout inside.
            let content_ctx = Ctx {
                snap_level: if snap.is_some() { 2 } else { 0 },
                scroll_items: position.is_some(),
                ..ctx.clone()
            };
            out.open("SwiftUI.Group {");
            self.nodes(out, &element.children, &content_ctx);
            out.close("}");
            out.line(".scrollTargetLayout()");
            if snap.is_some() {
                out.line(".bernSnapContent()");
            }
            if let Some(entrance) = &entrance {
                out.line(entrance);
            }
        } else if let Some(entrance) = &entrance {
            out.open("SwiftUI.Group {");
            self.nodes(out, &element.children, ctx);
            out.close("}");
            out.line(entrance);
        } else {
            self.nodes(out, &element.children, ctx);
        }
        out.close("}");
        if let Some(snap) = snap {
            out.line(format!(
                ".bernSnap(center: {}, axis: {axis})",
                snap == "center"
            ));
        }
        if let Some(position) = position {
            let anchor = match (snap, horizontal) {
                (Some("center"), _) => ".center",
                (_, true) => ".leading",
                (_, false) => ".top",
            };
            let arrival = match element.keyword("arrival") {
                Some(_) => {
                    format!(", arrival: {}", crate::style::motion(self.app, "jelly"))
                }
                None => String::new(),
            };
            out.line(format!(
                ".bernScrollPosition({}, anchor: {anchor}, motion: {}{arrival})",
                self.binding_of(&position),
                crate::style::motion(self.app, "smooth")
            ));
        }
        // `.hidden` still shows the bar when macOS is set to show
        // scroll bars always; `.never` does not.
        if element.keyword("indicators") == Some("hidden") {
            out.line(".scrollIndicators(.never)");
        }
        self.toolbar(out, &element.children, ctx);
    }

    pub(super) fn list(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let view = if element.kind == "List" {
            "List"
        } else {
            "ForEach"
        };
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                let var = member(&template.var);
                out.open(format!(
                    "SwiftUI.{view}({}, id: \\.{}) {{ {var} in",
                    self.expr(items),
                    member(name)
                ));
                out.open("SwiftUI.Group {");
                let item_ctx = Ctx {
                    scroll_items: false,
                    ..ctx.clone()
                };
                self.nodes(out, &element.children, &item_ctx);
                out.close("}");
                out.line(format!(".bernItemKey({var}.{})", member(name)));
                if ctx.scroll_items {
                    out.line(format!(".bernScrollItem({var}.{})", member(name)));
                }
                out.close("}");
                // Items that move, or a list that reflows, animate
                // when the list changes.
                let motion = self.children_motion(&element.children).or_else(|| {
                    element
                        .keyword("reflow")
                        .map(|token| crate::style::motion(self.app, token))
                });
                if let Some(motion) = motion {
                    out.line(format!(
                        ".animation({motion}.animation, value: ({}).map(\\.{}))",
                        self.expr(items),
                        member(name)
                    ));
                }
            }
            _ => {
                out.open(format!("SwiftUI.{view} {{"));
                self.nodes(out, &element.children, ctx);
                out.close("}");
            }
        }
        self.toolbar(out, &element.children, ctx);
    }

    /// A `Query`: the runtime view owns the subscription and shows the
    /// loading, refreshing, failed and empty states; the template shows
    /// the data. Its parameters are encoded on every render; the runtime
    /// requests them only when they change.
    pub(super) fn query(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let data = swift_type(&binding.result);
        let shown = swift_type(&template.ty);
        let params = if binding.args.is_empty() {
            "[]".to_string()
        } else {
            let encode: Vec<_> = binding
                .args
                .iter()
                .map(|arg| format!("({}).encode(bern: &writer)", self.expr(arg)))
                .collect();
            format!(
                "{{ var writer = BernWriter(); {}; return writer.bytes }}()",
                encode.join("; ")
            )
        };
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
        let (shown_body, count) = match &binding.result {
            Type::List { .. } if empty.is_some() => (
                "data.isEmpty ? nil : data",
                format!("{{ (data: {data}) in data.count }}"),
            ),
            Type::List { .. } => ("data", format!("{{ (data: {data}) in data.count }}")),
            Type::Optional { .. } => (
                "data",
                format!("{{ (data: {data}) in data == nil ? 0 : 1 }}"),
            ),
            _ => ("data", "nil".to_string()),
        };
        out.open("BernQueryView(");
        out.line(format!(
            "query: {}, target: {},",
            binding.query,
            self.target(element, ctx)
        ));
        out.line(format!("params: {params},"));
        out.line(format!(
            "loading: {loading}, empty: {}, emptyIcon: {}, failed: {failed},",
            empty.unwrap_or_else(|| "nil".into()),
            string(icon)
        ));
        out.line(format!(
            "count: {count}, shown: {{ (data: {data}) -> {shown}? in {shown_body} }}"
        ));
        out.close(format!(") {{ {} in", member(&template.var)));
        out.indent += 1;
        self.nodes(out, &element.children, ctx);
        out.close("}");
    }

    /// One closure per layout size; `BernAdaptive` picks one by the
    /// available width and the breakpoint from `[layouts]`.
    pub(super) fn adaptive(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let target = if element.id.is_some() {
            self.target(element, ctx)
        } else {
            "nil".into()
        };
        let layouts: Vec<(&str, &Element)> = element
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(layout) => layout.keyword("size").map(|size| (size, layout)),
                _ => None,
            })
            .collect();
        let declared: Vec<String> = layouts.iter().map(|(size, _)| string(size)).collect();
        out.open(format!(
            "BernAdaptive(breakpoints: bernAppBreakpoints, declared: [{}], target: {target}) {{ bernSize in",
            declared.join(", ")
        ));
        out.open("switch bernSize {");
        for (size, layout) in &layouts {
            if *size == "expanded" {
                continue;
            }
            out.open(format!("case {}:", string(size)));
            self.nodes(out, &layout.children, ctx);
            out.indent -= 1;
        }
        // The expanded layout is always declared.
        out.open("default:");
        if let Some((_, expanded)) = layouts.iter().find(|(size, _)| *size == "expanded") {
            self.nodes(out, &expanded.children, ctx);
        }
        out.indent -= 1;
        out.close("}");
        out.close("}");
    }
}

/// Conditions and repetitions do not introduce a layout container: their
/// rails still overlay the other layers of the surrounding Stack.
fn has_rail_layer(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) => {
            element.kind == "Rail"
                || (element.kind == "ForEach" && has_rail_layer(&element.children))
        }
        Node::If(node) => {
            node.branches.iter().any(|(_, body)| has_rail_layer(body))
                || has_rail_layer(&node.otherwise)
        }
        Node::IfLet(node) => has_rail_layer(&node.then) || has_rail_layer(&node.otherwise),
        Node::Match(node) => node.arms.iter().any(|arm| has_rail_layer(&arm.body)),
        Node::Component(_) => false,
    })
}
