//! Layout containers: stacks, layers, rails, grids, scroll views,
//! forms, repetitions, queries and adaptive layouts.

use super::values::swift_type;
use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, member, string};
use bern_ir::{Application, Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// A container's declared `width` and `height`.
    fn frame(&self, out: &mut Out, element: &Element) {
        let width = self.length(element, "width");
        let height = self.length(element, "height");
        if width.is_none() && height.is_none() {
            return;
        }
        // A stack of a given size places its layers by its alignment in
        // that size too, not only against each other. A column or a row
        // larger than its items starts them at its top and its leading
        // edge, as on the other targets, its alignment placing them across.
        let alignment = match (element.kind.as_str(), element.keyword("align")) {
            ("Stack", Some(align)) if align != "center" => {
                format!(", alignment: {}", stack_alignment(Some(align)))
            }
            ("Column", align) => format!(
                ", alignment: {}",
                match align {
                    Some("center") => ".top",
                    Some("trailing") => ".topTrailing",
                    _ => ".topLeading",
                }
            ),
            ("Row", align) => format!(
                ", alignment: {}",
                match align {
                    Some("top") => ".topLeading",
                    Some("bottom") => ".bottomLeading",
                    _ => ".leading",
                }
            ),
            _ => String::new(),
        };
        out.line(format!(
            ".frame(width: {}, height: {}{alignment})",
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
        if element.kind == "Row" {
            out.open(format!("BernRow(alignment: {alignment}{spacing}) {{"));
            self.nodes(
                out,
                &element.children,
                &Ctx {
                    row: true,
                    ..ctx.clone()
                },
            );
        } else {
            out.open(format!(
                "SwiftUI.{stack}(alignment: {alignment}{spacing}) {{"
            ));
            self.nodes(out, &element.children, ctx);
        }
        out.close("}");
        self.padding(out, element);
        self.frame(out, element);
        self.toolbar(out, &element.children, ctx);
    }

    pub(super) fn stack(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let alignment = stack_alignment(element.keyword("align"));
        // A rail or a canvas without a size among the layers takes the size
        // of the others.
        let rails = has_rail_layer(&element.children);
        out.open(if rails {
            // A stack of a given size gives that size to its backdrops.
            let size = ["width", "height"]
                .into_iter()
                .filter_map(|name| {
                    self.length(element, name)
                        .map(|length| format!(", {name}: {length}"))
                })
                .collect::<String>();
            // With a backdrop it fills across what it stands in, as a block
            // does, and keeps the length its other layers give it; in a row
            // it keeps its size.
            let across = if !ctx.row
                && element.prop("width").is_none()
                && has_backdrop_layer(&element.children)
            {
                ", fillsWidth: true"
            } else {
                ""
            };
            format!("BernLayers(alignment: {alignment}{size}{across}) {{")
        } else {
            format!("SwiftUI.ZStack(alignment: {alignment}) {{")
        });
        self.nodes(out, &element.children, ctx);
        out.close("}");
        self.frame(out, element);
        // The whole window to its edges, under its bars: the title bar and
        // toolbar of a Mac window, a phone's status bar and home indicator.
        if element.keyword("under_bars") == Some("true") {
            out.line(format!(
                ".frame(maxWidth: .infinity, maxHeight: .infinity, alignment: {alignment})"
            ));
            out.line(match self.platform {
                Platform::Macos => ".ignoresSafeArea(.container, edges: .top)",
                Platform::Ios | Platform::Tvos => ".ignoresSafeArea(.container)",
            });
            // Over the layer the title bar keeps its controls, not the
            // window's title, also when the content before had a split
            // view that titled it.
            if self.platform == Platform::Macos {
                out.line(".bernToolbarTitleHidden()");
            }
        }
        // A toolbar among the layers belongs to the window, as a column's
        // does.
        self.toolbar(out, &element.children, ctx);
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
        let overhang = self
            .length(element, "overhang")
            .map(|overhang| format!(", overhang: {overhang}"))
            .unwrap_or_default();
        out.open(format!(
            "BernRail(vertical: {vertical}, anchor: {anchor}{overhang}) {{"
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
        let cells = element
            .number("cell_ratio")
            .map(|ratio| {
                format!(
                    ", cellRatio: {}, cellExtra: {}",
                    double(ratio),
                    self.length(element, "cell_extra")
                        .unwrap_or_else(|| double(0.0))
                )
            })
            .unwrap_or_default();
        out.open(format!(
            "BernAdaptiveGrid(minimum: {}, gap: {gap}, rowGap: {row_gap}, motion: {motion}{cells}) {{",
            self.length(element, "min_width")
                .unwrap_or_else(|| double(120.0))
        ));
        if cells.is_empty() {
            self.nodes(out, &element.children, ctx);
        } else {
            // A group's modifier applies to each of its views: every
            // item takes the cell's height.
            out.open("SwiftUI.Group {");
            self.nodes(out, &element.children, ctx);
            out.close("}");
            out.line(".bernGridCell()");
        }
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
            "BernModal(width: {}, height: {}{}) {{",
            self.length(element, "width")
                .unwrap_or_else(|| "960.0".into()),
            self.length(element, "height")
                .unwrap_or_else(|| "600.0".into()),
            if element.keyword("cover") == Some("compact") {
                ", cover: true"
            } else {
                ""
            }
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
        if let Some(style) = self.style_of(element) {
            out.line(format!(".bernFormStyle(BernStyles.{})", style.name));
        }
        // The room between sections, and none above a plain one
        // that opens the form, which is then the page's heading.
        // The heading may come from a slot: a component, inside a
        // condition.
        fn opens_plain(app: &Application, nodes: &[Node]) -> bool {
            match nodes.first() {
                Some(Node::Element(child)) => {
                    child.kind == "Section" && child.keyword("style") == Some("plain")
                }
                Some(Node::If(node)) => node
                    .branches
                    .first()
                    .is_some_and(|(_, nodes)| opens_plain(app, nodes)),
                Some(Node::Component(reference)) => app
                    .component(&reference.component)
                    .is_some_and(|component| {
                        opens_plain(app, std::slice::from_ref(&component.body))
                    }),
                _ => false,
            }
        }
        let opens_plain = opens_plain(self.app, &element.children);
        let (x, y) = (
            self.length(element, "padding_x"),
            self.length(element, "padding_y"),
        );
        if x.is_some() || y.is_some() {
            out.line(format!(
                ".environment(\\.bernFormInsets, BernFormInsets(x: {}, y: {}))",
                x.unwrap_or_else(|| "nil".into()),
                y.unwrap_or_else(|| "nil".into())
            ));
        }
        if element.prop("gap").is_some() || element.prop("row_height").is_some() || opens_plain {
            out.line(format!(
                ".bernFormLayout(gap: {}, rowHeight: {}, heading: {opens_plain})",
                self.length(element, "gap").unwrap_or_else(|| "nil".into()),
                self.length(element, "row_height")
                    .unwrap_or_else(|| "nil".into())
            ));
        }
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
                    out.line(format!(
                        "SwiftUI.Text(verbatim: {title}).bernSectionTitle()"
                    ));
                }
                if let Some(footer) = &footer {
                    out.close("} footer: {");
                    out.indent += 1;
                    out.line(format!(
                        "SwiftUI.Text(verbatim: {footer}).bernSectionFooter()"
                    ));
                }
                out.close("}");
            }
        }
        out.line(if element.keyword("style") == Some("plain") {
            ".bernPlainRows()"
        } else {
            ".modifier(BernFormRows())"
        });
        let (x, y) = (
            self.length(element, "padding_x"),
            self.length(element, "padding_y"),
        );
        if x.is_some() || y.is_some() {
            out.line(format!(
                ".bernRowInsets(x: {}, y: {})",
                x.unwrap_or_else(|| "nil".into()),
                y.unwrap_or_else(|| "nil".into())
            ));
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
        // Where it opens: that fraction of the content at the same
        // fraction of the view.
        if let Some(at) = element.number("open_at") {
            out.line(if horizontal {
                format!(".defaultScrollAnchor(UnitPoint(x: {}, y: 0))", double(at))
            } else {
                format!(".defaultScrollAnchor(UnitPoint(x: 0, y: {}))", double(at))
            });
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
        // The expanded layout is always declared, but on a television only
        // the `tv` layout is built.
        out.open("default:");
        if let Some((_, expanded)) = layouts.iter().find(|(size, _)| *size == "expanded") {
            self.nodes(out, &expanded.children, ctx);
        } else {
            out.line("SwiftUI.EmptyView()");
        }
        out.indent -= 1;
        out.close("}");
        out.close("}");
    }
}

/// Whether a layer is a canvas without a size, the stack's backdrop,
/// directly or through a condition or repetition.
fn has_backdrop_layer(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) => {
            (element.kind == "Canvas"
                && element.prop("width").is_none()
                && element.prop("height").is_none())
                || (element.kind == "ForEach" && has_backdrop_layer(&element.children))
        }
        Node::If(node) => {
            node.branches
                .iter()
                .any(|(_, body)| has_backdrop_layer(body))
                || has_backdrop_layer(&node.otherwise)
        }
        Node::IfLet(node) => has_backdrop_layer(&node.then) || has_backdrop_layer(&node.otherwise),
        Node::Match(node) => node.arms.iter().any(|arm| has_backdrop_layer(&arm.body)),
        Node::Component(_) => false,
    })
}

/// Whether a layer is a rail or a canvas without a size, which take the
/// size of the other layers. Conditions and repetitions do not introduce a
/// layout container: such layers in them still overlay the other layers of
/// the surrounding Stack.
fn has_rail_layer(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) => {
            element.kind == "Rail"
                || (element.kind == "Canvas"
                    && element.prop("width").is_none()
                    && element.prop("height").is_none())
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

/// SwiftUI's alignment for a `Stack`'s `align`.
fn stack_alignment(align: Option<&str>) -> &'static str {
    match align {
        Some("top_leading") => ".topLeading",
        Some("top") => ".top",
        Some("top_trailing") => ".topTrailing",
        Some("leading") => ".leading",
        Some("trailing") => ".trailing",
        Some("bottom_leading") => ".bottomLeading",
        Some("bottom") => ".bottom",
        Some("bottom_trailing") => ".bottomTrailing",
        _ => ".center",
    }
}
