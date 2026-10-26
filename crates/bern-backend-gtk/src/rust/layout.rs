//! Layout containers: forms and sections, stacks and grids, scrolling,
//! rails, lists, queries and adaptive layouts.

use super::{Generator, Out, float, some, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    pub(super) fn form(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Form(p, c, b::FormProps {{ title: {}, gap: {}, row_height: {}, padding_x: {}, padding_y: {} }}, ",
                some(self.text_prop(element, "title")),
                Self::number(element, "gap"),
                Self::number(element, "row_height"),
                Self::number(element, "padding_x"),
                Self::number(element, "padding_y"),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Section(p, c, b::SectionProps {{ title: {}, footer: {}, plain: {}, padding_x: {}, padding_y: {} }}, ",
                some(self.text_prop(element, "title")),
                some(self.text_prop(element, "footer")),
                element.keyword("style") == Some("plain"),
                Self::number(element, "padding_x"),
                Self::number(element, "padding_y"),
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
                self.padding(element),
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
                "b::Grid(p, c, b::GridProps {{ min_width: {}, gap: {}, row_gap: {}, padding: {}, animate: {}, cells: {}, reflow: {} }}, ",
                some(self.num(element, "min_width")),
                some(self.num(element, "gap")),
                some(self.num(element, "row_gap")),
                self.padding(element),
                self.animate(element),
                some(element.number("cell_ratio").map(|ratio| format!(
                    "({}, {})",
                    float(ratio),
                    self.num(element, "cell_extra").unwrap_or_else(|| "b::Num::Fixed(0.0)".into())
                ))),
                some(Self::reflow(element).map(|token| self.motion(token))),
            ),
            element,
            ");",
        );
    }

    pub(super) fn stack(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::Layers(p, c, b::LayersProps {{ align: {}, width: {}, height: {}, animate: {}, style: {}, enabled: {}, reflow: {}, under_bars: {} }}, ",
                Self::keyword(element, "align"),
                some(self.num(element, "width")),
                some(self.num(element, "height")),
                self.animate(element),
                self.style("Stack", element),
                self.enabled(element),
                some(Self::reflow(element).map(|token| self.motion(token))),
                element.keyword("under_bars") == Some("true"),
            ),
            element,
            ");",
        );
    }

    pub(super) fn scale_box(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::ScaleBox(p, c, b::ScaleBoxProps {{ width: {}, height: {} }}, ",
                some(self.num(element, "width")),
                some(self.num(element, "height")),
            ),
            element,
            ");",
        );
    }

    pub(super) fn positioned(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::Positioned(p, c, b::PositionedProps {{ x: {}, y: {}, width: {}, height: {}, animate: {}, bleed: {} }}, ",
                some(self.num(element, "x")),
                some(self.num(element, "y")),
                some(self.num(element, "width")),
                some(self.num(element, "height")),
                self.animate(element),
                self.value(element, "bleed"),
            ),
            element,
            ");",
        );
    }

    pub(super) fn marquee(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Marquee(p, c, b::MarqueeProps {{ duration: {}, phase: {}, paused: {}, opacity: {} }}, ",
                some(self.num(element, "duration")),
                float(element.number("phase").unwrap_or(0.0)),
                self.value(element, "paused"),
                some(self.num(element, "opacity")),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn accordion(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Accordion(p, c, b::AccordionProps {{ count: {}, selected: {}, collapsed_width: {}, gap: {}, height: {}, corner: {}, animate: {} }}, ",
                some(self.num(element, "count")),
                some(self.num(element, "selected")),
                some(self.num(element, "collapsed_width")),
                some(self.num(element, "gap")),
                some(self.num(element, "height")),
                some(self.num(element, "corner")),
                self.animate(element),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn flow(&self, out: &mut Out, element: &Element) {
        self.container_children(
            out,
            &format!(
                "b::Flow(p, c, b::FlowProps {{ gap: {} }}, ",
                some(self.num(element, "gap")),
            ),
            element,
            ");",
        );
    }

    pub(super) fn modal(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Modal(p, c, b::ModalProps {{ width: {}, height: {}, cover: {}, decor: {} }}, ",
                some(self.num(element, "width")),
                some(self.num(element, "height")),
                element.keyword("cover") == Some("compact"),
                some(self.decoration(element)),
            ),
            &element.children,
            ");",
        );
    }

    /// `Shortcut`: its keys run the action while it is mounted and
    /// enabled.
    pub(super) fn key_shortcut(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::KeyShortcut(p, c, b::KeyShortcutProps {{ keys: {}, bound: {}, enabled: {} }}, {});",
            string(element.keyword("key").unwrap_or_default()),
            some(self.bound_keys(element, "key")),
            self.enabled(element),
            self.action(element),
        ));
    }

    /// `ShortcutInput`: a button that takes the next key pressed as the
    /// shortcut, which the action keeps.
    pub(super) fn shortcut_input(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::ShortcutInput(p, c, b::ShortcutInputProps {{ id: {}, label: {}, model: {}, prompt: {}, empty: {}, enabled: {}, label_hidden: {} }});",
            self.id(element),
            some(self.text_prop(element, "label")),
            some(self.model(element, "current", false)),
            some(self.text_prop(element, "prompt")),
            some(self.text_prop(element, "empty")),
            self.enabled(element),
            element.keyword("label_hidden") == Some("true"),
        ));
    }

    pub(super) fn timer(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Timer(p, c, b::TimerProps {{ interval: {}, enabled: {} }}, {});",
            float(element.number("interval").unwrap_or(1.0)),
            self.enabled(element),
            self.action(element),
        ));
    }

    pub(super) fn control_group(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::ControlGroup(p, c, b::ControlGroupProps {{ label: {}, height: {} }}, ",
                some(self.text_prop(element, "label")),
                some(self.num(element, "height")),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn scroll_view(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::ScrollView(p, c, b::ScrollProps {{ axis: {}, snap: {}, position: {}, arrival_nudge: {}, hide_indicators: {}, nudge_motion: {}, smooth: {}, open_at: {}, open_center: {} }}, ",
                Self::keyword(element, "axis"),
                Self::keyword(element, "snap"),
                some(self.field(element, "position")),
                element.keyword("arrival") == Some("nudge"),
                element.keyword("indicators") == Some("hidden"),
                self.motion("jelly"),
                self.motion("smooth"),
                some(element.number("open_at").map(float)),
                some(self.num(element, "open_center")),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn rail(&self, out: &mut Out, element: &Element) {
        self.children(
            out,
            &format!(
                "b::Rail(p, c, b::RailProps {{ vertical: {}, anchor: {}, overhang: {} }}, ",
                element.keyword("axis") == Some("vertical"),
                Self::keyword(element, "anchor"),
                some(self.num(element, "overhang")),
            ),
            &element.children,
            ");",
        );
    }

    pub(super) fn preview_list(&self, out: &mut Out, element: &Element) {
        if let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            self.children(
                out,
                &format!(
                    "b::PreviewList(p, c, {}, {field}, {}, b::PreviewListProps {{ item_height: {}f64, gap: {}f64, more_height: {}f64 }}, ",
                    self.getter(items),
                    string(&template.var),
                    element.number("item_height").unwrap_or(1.0),
                    element.number("gap").unwrap_or(0.0),
                    element.number("more_height").unwrap_or(22.0),
                ),
                &element.children,
                ");",
            );
        }
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
        // A desktop is never a television: a `tv` layout is not built.
        for size in ["compact", "medium", "expanded", "large"] {
            match layout(size) {
                Some(layout) => self.children(out, "Some(", &layout.children, "),"),
                None => out.line("None,"),
            }
        }
        out.close("]);");
    }
}
