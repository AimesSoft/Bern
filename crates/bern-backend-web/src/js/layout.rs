//! Layout containers: forms and sections, stacks and grids, scrolling,
//! rails, lists, queries and adaptive layouts.

use super::codecs::{decode, encode};
use super::{Generator, Out, local, number, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// Lines that act on a container `p`: its `reflow`, and the lengths
    /// bound to expressions, or given in sizes the runtime does not take
    /// as props, moving with `animate`.
    fn layout_lines(&self, element: &Element, lengths: &[(&str, &str)]) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(token) = element.keyword("reflow") {
            lines.push(format!("F.reflow(p, {});", self.motion(token)));
        }
        let animate = self.animate(element).unwrap_or_else(|| "null".into());
        for (name, property) in lengths {
            if let Some(value) = self.length(element, name) {
                lines.push(format!(
                    "F.bindLength(p, {}, {value}, {animate});",
                    string(property)
                ));
            }
        }
        lines
    }

    pub(super) fn form(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("title", self.text_prop(element, "title")),
            ("gap", element.number("gap").map(number)),
            ("rowHeight", element.number("row_height").map(number)),
            ("paddingX", element.number("padding_x").map(number)),
            ("paddingY", element.number("padding_y").map(number)),
        ]);
        // A form style's background is its rows'.
        let first: Vec<String> =
            crate::style::reference(self.app, "Form", element.keyword("variant"))
                .map(|style| format!("F.formRows(p, {style});"))
                .into_iter()
                .collect();
        self.children_with(
            out,
            &format!("B.Form(p, ctx, {props}, "),
            &first,
            &element.children,
            ");",
        );
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("title", self.text_prop(element, "title")),
            ("footer", self.text_prop(element, "footer")),
            (
                "plain",
                (element.keyword("style") == Some("plain")).then(|| "true".into()),
            ),
            ("paddingX", element.number("padding_x").map(number)),
            ("paddingY", element.number("padding_y").map(number)),
        ]);
        self.children(
            out,
            &format!("B.Section(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }

    pub(super) fn column_or_row(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            (
                "axis",
                Some(string(if element.kind == "Column" {
                    "column"
                } else {
                    "row"
                })),
            ),
            ("align", Self::keyword(element, "align")),
        ]);
        let first = self.layout_lines(
            element,
            &[
                ("gap", "gap"),
                ("padding", "padding"),
                ("padding_x", "paddingInline"),
                ("padding_y", "paddingBlock"),
                ("width", "width"),
                ("height", "height"),
            ],
        );
        self.container_children(
            out,
            element,
            &format!("B.Stack(p, ctx, {props}, "),
            &first,
            ");",
        );
    }

    pub(super) fn grid(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![(
            "minWidth",
            Some(
                element
                    .number("min_width")
                    .map(number)
                    .unwrap_or("0".into()),
            ),
        )]);
        let mut first = self.layout_lines(
            element,
            &[
                ("gap", "gap"),
                ("row_gap", "rowGap"),
                ("padding", "padding"),
                ("padding_x", "paddingInline"),
                ("padding_y", "paddingBlock"),
            ],
        );
        if let Some(Prop::Expr { .. }) = element.prop("min_width") {
            first.push(format!(
                "F.bindGridMinWidth(p, {}, {});",
                self.length(element, "min_width").expect("min width"),
                self.animate(element).unwrap_or_else(|| "null".into())
            ));
        }
        if element.keyword("masonry") == Some("true") {
            first.push(format!(
                "F.bindMasonry(p, {});",
                self.length(element, "row_gap")
                    .or_else(|| self.length(element, "gap"))
                    .unwrap_or_else(|| "8".into())
            ));
        }
        if let Some(ratio) = element.number("cell_ratio") {
            first.push(format!(
                "F.bindGridCells(p, {}, {}, {});",
                number(ratio),
                self.length(element, "cell_extra")
                    .unwrap_or_else(|| "0".into()),
                self.animate(element).unwrap_or_else(|| "null".into())
            ));
        }
        self.container_children(
            out,
            element,
            &format!("B.Grid(p, ctx, {props}, "),
            &first,
            ");",
        );
    }

    pub(super) fn stack(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("align", Self::keyword(element, "align")),
            ("enabled", self.value_prop(element, "enabled")),
            (
                "style",
                crate::style::reference(self.app, "Stack", element.keyword("variant")),
            ),
            (
                "underBars",
                (element.keyword("under_bars") == Some("true")).then(|| "true".into()),
            ),
        ]);
        let first = self.layout_lines(element, &[("width", "width"), ("height", "height")]);
        self.container_children(
            out,
            element,
            &format!("B.Layers(p, ctx, {props}, "),
            &first,
            ");",
        );
    }

    pub(super) fn marquee(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("duration", self.length(element, "duration")),
            ("phase", element.number("phase").map(number)),
            ("paused", self.value_prop(element, "paused")),
            ("opacity", self.value_prop(element, "opacity")),
        ]);
        self.children(
            out,
            &format!("B.Marquee(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }

    /// A `ScaleBox`, or a `Positioned` placed at its `x` and `y`.
    pub(super) fn scale_box(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("width", self.length(element, "width")),
            ("height", self.length(element, "height")),
        ]);
        let first = if element.kind == "Positioned" {
            let mut lines = self.layout_lines(element, &[]);
            let place = Self::props(vec![
                ("x", self.length(element, "x")),
                ("y", self.length(element, "y")),
                ("width", self.length(element, "width")),
                ("height", self.length(element, "height")),
                ("bleed", self.value_prop(element, "bleed")),
            ]);
            lines.push(format!(
                "F.bindLayer(p, ctx, {place}, {});",
                self.animate(element).unwrap_or_else(|| "null".into())
            ));
            lines
        } else {
            vec![]
        };
        self.container_children(
            out,
            element,
            &format!("B.{}(p, ctx, {props}, ", element.kind),
            &first,
            ");",
        );
    }

    /// A `Modal`, `Flow` or `Accordion`, with its lengths as props.
    pub(super) fn sized_container(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("width", self.length(element, "width")),
            ("height", self.length(element, "height")),
            ("gap", self.length(element, "gap")),
            ("count", self.length(element, "count")),
            ("selected", self.length(element, "selected")),
            ("collapsedWidth", self.length(element, "collapsed_width")),
            ("corner", self.length(element, "corner")),
            ("animate", self.animate(element)),
            ("cover", Self::keyword(element, "cover")),
        ]);
        self.children(
            out,
            &format!("B.{}(p, ctx, {props}, ", element.kind),
            &element.children,
            ");",
        );
    }

    pub(super) fn scroll_view(&self, out: &mut Out, element: &Element) {
        let position = self.field(element, "position");
        let snapping = element.keyword("snap").is_some() || position.is_some();
        let props = Self::props(vec![
            ("axis", Self::keyword(element, "axis")),
            ("snap", Self::keyword(element, "snap")),
            ("position", position),
            ("arrival", Self::keyword(element, "arrival")),
            ("nudgeMotion", snapping.then(|| self.motion("jelly"))),
            (
                "hideIndicators",
                (element.keyword("indicators") == Some("hidden")).then(|| "true".into()),
            ),
            ("openAt", element.number("open_at").map(number)),
            ("openCenter", self.length(element, "open_center")),
        ]);
        let first = self.layout_lines(element, &[]);
        self.children_with(
            out,
            &format!(
                "{}(p, ctx, {props}, ",
                if snapping {
                    "F.ScrollView"
                } else {
                    "B.ScrollView"
                }
            ),
            &first,
            &element.children,
            ");",
        );
    }

    pub(super) fn rail(&self, out: &mut Out, element: &Element) {
        let props = Self::props(vec![
            ("axis", Self::keyword(element, "axis")),
            ("anchor", Self::keyword(element, "anchor")),
            ("overhang", self.length(element, "overhang")),
        ]);
        self.children(
            out,
            &format!("F.Rail(p, ctx, {props}, "),
            &element.children,
            ");",
        );
    }

    pub(super) fn preview_list(&self, out: &mut Out, element: &Element) {
        if let (Some(items), Some(template), Some(Prop::Key { name, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            out.open(format!(
                "B.PreviewList(p, ctx, {{ items: () => {}, key: (v) => v.{name}, itemHeight: {}, gap: {}, moreHeight: {} }}, (p, ctx, {}) => {{",
                self.expr(items),
                number(element.number("item_height").unwrap_or(1.0)),
                number(element.number("gap").unwrap_or(0.0)),
                number(element.number("more_height").unwrap_or(22.0)),
                local(&template.var),
            ));
            self.nodes(out, &element.children);
            out.close("});");
        }
    }

    pub(super) fn list(&self, out: &mut Out, element: &Element) {
        let wrap = element.kind == "List";
        if wrap {
            out.open("B.List(p, ctx, (p, ctx) => {");
        }
        if let Some(token) = element.keyword("reflow") {
            out.line(format!("F.reflow(p, {});", self.motion(token)));
        }
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { name, .. })) => {
                let stagger = element
                    .number("stagger")
                    .map(|step| format!(", stagger: {}", number(step)))
                    .unwrap_or_default();
                out.open(format!(
                    "B.Each(p, ctx, {{ items: () => {}, key: (v) => v.{name}{stagger} }}, (p, ctx, {}) => {{",
                    self.expr(items),
                    local(&template.var)
                ));
                self.nodes(out, &element.children);
                out.close("});");
            }
            _ => self.nodes(out, &element.children),
        }
        if wrap {
            out.close("});");
        }
    }

    pub(super) fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let spec = &self.app.protocol.commands[binding.query];
        let encoders: Vec<_> = binding
            .args
            .iter()
            .zip(&spec.params)
            .map(|(arg, param)| format!("{};", encode(&param.ty, &self.expr(arg))))
            .collect();
        let params = format!(
            "() => {{ const w = new B.Writer(); {} return w.finish(); }}",
            encoders.join(" ")
        );
        let framework = |key: &str| format!("() => B.tr({})", string(key));
        let empty = self.text_prop(element, "empty");
        let (shown, count) = match &binding.result {
            Type::List { .. } if empty.is_some() => {
                ("(d) => (d.length ? d : null)", "(d) => d.length")
            }
            Type::List { .. } => ("(d) => d", "(d) => d.length"),
            Type::Optional { .. } => ("(d) => d", "(d) => (d === null ? 0 : 1)"),
            _ => ("(d) => d", "() => null"),
        };
        let props = Self::props(vec![
            ("id", Some(self.id(element))),
            ("query", Some(binding.query.to_string())),
            ("params", Some(params)),
            (
                "decode",
                Some(format!("(r) => {}", decode(&binding.result))),
            ),
            (
                "loading",
                self.text_prop(element, "loading")
                    .or_else(|| Some(framework("bern.query.loading"))),
            ),
            (
                "failed",
                self.text_prop(element, "failed")
                    .or_else(|| Some(framework("bern.query.failed"))),
            ),
            ("empty", empty),
            ("emptyIcon", element.icon("icon").map(string)),
            ("count", Some(count.into())),
            ("shown", Some(shown.into())),
        ]);
        out.open(format!(
            "B.Query(p, ctx, {props}, (p, ctx, {}) => {{",
            local(&template.var)
        ));
        self.nodes(out, &element.children);
        out.close("});");
    }

    /// One builder per layout size; the runtime picks one by the available
    /// width and the breakpoint from `[layouts]`.
    pub(super) fn adaptive(&self, out: &mut Out, element: &Element) {
        out.open("B.Adaptive(p, ctx, {");
        if element.id.is_some() {
            out.line(format!("id: {},", self.id(element)));
        }
        out.line(format!("breakpoints: {},", self.breakpoints()));
        out.open("layouts: {");
        for node in &element.children {
            let Node::Element(layout) = node else {
                continue;
            };
            let Some(size) = layout.keyword("size") else {
                continue;
            };
            out.open(format!("{size}: (p, ctx) => {{"));
            self.nodes(out, &layout.children);
            out.close("},");
        }
        out.close("},");
        out.close("});");
    }

    /// The breakpoints of `[layouts]` for the runtime, `{ compact, medium,
    /// expanded }`; absent ones are `null`.
    pub(super) fn breakpoints(&self) -> String {
        match self.app.layouts {
            Some(layouts) => format!(
                "{{ compact: {}, medium: {}, expanded: {} }}",
                number(layouts.compact_below),
                layouts.medium_below.map(number).unwrap_or("null".into()),
                layouts.expanded_below.map(number).unwrap_or("null".into())
            ),
            None => "{ compact: 0, medium: null, expanded: null }".into(),
        }
    }
}
