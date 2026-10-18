//! Layout containers: forms and sections, stacks and grids, scrolling,
//! rails, lists, queries and adaptive layouts.

use super::{Generator, Out, some, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    pub(super) fn form(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Form(p, c, b::FormProps {{ title: {} }}, ",
                some(self.text_prop(element, "title"))
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Section(p, c, b::SectionProps {{ title: {}, footer: {} }}, ",
                some(self.text_prop(element, "title")),
                some(self.text_prop(element, "footer"))
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn column_or_row(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::Stack(p, c, b::StackProps {{ axis: {}, gap: {}, padding: {}, width: {}, height: {}, align: {}, animate: {}, reflow: {} }}, ",
                string(if element.kind == "Column" { "column" } else { "row" }),
                some(self.num(element, "gap")),
                some(self.num(element, "padding")),
                some(self.num(element, "width")),
                some(self.num(element, "height")),
                Self::keyword(element, "align"),
                self.animate(element),
                some(Self::reflow(element).map(|token| self.motion(token))),
            ),
            element,
            ");",
        );
    }

    pub(super) fn grid(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::Grid(p, c, b::GridProps {{ min_width: {}, gap: {}, padding: {}, animate: {} }}, ",
                some(self.num(element, "min_width")),
                some(self.num(element, "gap")),
                some(self.num(element, "padding")),
                self.animate(element),
            ),
            element,
            ");",
        );
    }

    pub(super) fn stack(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::Layers(p, c, b::LayersProps {{ align: {}, width: {}, height: {}, animate: {} }}, ",
                Self::keyword(element, "align"),
                some(self.num(element, "width")),
                some(self.num(element, "height")),
                self.animate(element),
            ),
            element,
            ");",
        );
    }

    pub(super) fn scroll_view(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::ScrollView(p, c, b::ScrollProps {{ axis: {}, snap: {}, position: {}, arrival_nudge: {}, hide_indicators: {}, nudge_motion: {}, smooth: {} }}, ",
                Self::keyword(element, "axis"),
                Self::keyword(element, "snap"),
                some(self.field(element, "position")),
                element.keyword("arrival") == Some("nudge"),
                element.keyword("indicators") == Some("hidden"),
                self.motion("jelly"),
                self.motion("smooth"),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn rail(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Rail(p, c, b::RailProps {{ vertical: {}, anchor: {} }}, ",
                element.keyword("axis") == Some("vertical"),
                Self::keyword(element, "anchor"),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn for_each(&self, out: &mut Out, element: &Element) {
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { field, .. })) => self.children(
                out,
                &format!(
                    "b::Each(p, c, {}, {field}, {}, {}, ",
                    self.getter(items),
                    string(&template.var),
                    Self::number(element, "stagger"),
                ),
                &element.children,
                ");",
            ),
            _ => self.nodes(out, &element.children),
        }
    }

    pub(super) fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let params: Vec<_> = binding.args.iter().map(|arg| self.getter(arg)).collect();
        self.children(
            out,
            &format!(
                "b::Query(p, c, b::QueryProps {{ id: {}, query: {}, params: vec![{}], loading: {}, failed: {}, empty: {}, icon: {}, var: {} }}, ",
                some(element.id.as_ref().map(|_| self.id(element))),
                binding.query,
                params.join(", "),
                some(self.text_prop(element, "loading")),
                some(self.text_prop(element, "failed")),
                some(self.text_prop(element, "empty")),
                Self::icon(element, "icon"),
                string(&template.var),
            ),
            &element.children,
            ");",
        );
    }

    /// One builder per layout size; the runtime picks one by the width
    /// available to it and the breakpoints from `[layouts]`.
    pub(super) fn adaptive(&self, out: &mut Out, element: &Element) {
        let layout = |size: &str| {
            element.children.iter().find_map(|node| match node {
                Node::Element(layout) if layout.keyword("size") == Some(size) => Some(layout),
                _ => None,
            })
        };
        out.open(format!(
            "b::Adaptive(p, c, {}, [",
            some(element.id.as_ref().map(|_| self.id(element)))
        ));
        for size in ["compact", "medium", "expanded", "large"] {
            match layout(size) {
                Some(layout) => self.children(out, "Some(", &layout.children, "),"),
                None => out.line("None,"),
            }
        }
        out.close("]);");
    }
}
