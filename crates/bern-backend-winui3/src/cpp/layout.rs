//! Layout: stacks, grids, layers and rails, scroll views, forms and
//! sections, lists, repetitions and queries.

use super::{BUILDER, Generator, Out, designated, ident, motion_ref, null, string};
use bern_ir::{Element, Prop};

impl Generator<'_> {
    /// `Column` and `Row`: a stack along their axis.
    pub(super) fn stack(&self, out: &mut Out, element: &Element) {
        self.container(
            out,
            element,
            &format!(
                "B::Stack(p, c, {}, ",
                designated(&[
                    (
                        "axis",
                        Some(string(if element.kind == "Column" {
                            "column"
                        } else {
                            "row"
                        }))
                    ),
                    ("gap", self.len(element, "gap")),
                    ("padding", self.len(element, "padding")),
                    ("width", self.len(element, "width")),
                    ("height", self.len(element, "height")),
                    ("align", Self::keyword(element, "align")),
                    ("animate", Self::animate(element)),
                    ("reflow", Self::reflow(element)),
                    ("paddingX", self.len(element, "padding_x")),
                    ("paddingY", self.len(element, "padding_y")),
                ])
            ),
        );
    }

    pub(super) fn grid(&self, out: &mut Out, element: &Element) {
        self.container(
            out,
            element,
            &format!(
                "B::Grid(p, c, {}, ",
                designated(&[
                    ("minWidth", self.len(element, "min_width")),
                    ("gap", self.len(element, "gap")),
                    ("padding", self.len(element, "padding")),
                    ("animate", Self::animate(element)),
                    ("reflow", Self::reflow(element)),
                    ("rowGap", self.len(element, "row_gap")),
                    ("paddingX", self.len(element, "padding_x")),
                    ("paddingY", self.len(element, "padding_y")),
                    ("cellRatio", Self::number(element, "cell_ratio")),
                    ("cellExtra", self.len(element, "cell_extra")),
                ])
            ),
        );
    }

    /// `Stack`: its children layered over one another.
    pub(super) fn layers(&self, out: &mut Out, element: &Element) {
        self.container(
            out,
            element,
            &format!(
                "B::Layers(p, c, {}, ",
                designated(&[
                    ("align", Self::keyword(element, "align")),
                    ("width", self.len(element, "width")),
                    ("height", self.len(element, "height")),
                    ("animate", Self::animate(element)),
                    ("reflow", Self::reflow(element)),
                    ("style", self.style("Stack", element.keyword("variant"))),
                    ("enabled", self.enabled(element)),
                    (
                        "underBars",
                        (element.keyword("under_bars") == Some("true")).then(|| "true".into()),
                    ),
                ])
            ),
        );
    }

    /// `ScaleBox`: its content laid out at a design size and scaled to fit.
    pub(super) fn scale_box(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::ScaleBox(p, c, {}, ",
                designated(&[
                    ("width", self.len(element, "width")),
                    ("height", self.len(element, "height")),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn positioned(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Positioned(p, c, {}, ",
                designated(&[
                    ("x", self.len(element, "x")),
                    ("y", self.len(element, "y")),
                    ("width", self.len(element, "width")),
                    ("height", self.len(element, "height")),
                    ("animate", Self::animate(element)),
                    ("bleed", element.expr("bleed").map(|e| self.getter(e))),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn marquee(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Marquee(p, c, {}, ",
                designated(&[
                    ("duration", self.len(element, "duration")),
                    ("phase", Self::number(element, "phase")),
                    ("paused", element.expr("paused").map(|e| self.getter(e))),
                    ("opacity", self.len(element, "opacity")),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn accordion(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Accordion(p, c, {}, ",
                designated(&[
                    ("count", self.len(element, "count")),
                    ("selected", self.len(element, "selected")),
                    ("collapsedWidth", self.len(element, "collapsed_width")),
                    ("gap", self.len(element, "gap")),
                    ("height", self.len(element, "height")),
                    ("corner", self.len(element, "corner")),
                    ("animate", Self::animate(element)),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn flow(&self, out: &mut Out, element: &Element) {
        self.container(
            out,
            element,
            &format!(
                "B::Flow(p, c, {}, ",
                designated(&[("gap", self.len(element, "gap"))])
            ),
        );
    }

    pub(super) fn modal(&self, out: &mut Out, element: &Element) {
        let transition = |name: &str| {
            element
                .keyword(name)
                .map(|transition| format!("&X_{}", ident(transition)))
        };
        self.call(
            out,
            &format!(
                "B::Modal(p, c, {}, ",
                designated(&[
                    ("width", self.len(element, "width")),
                    ("height", self.len(element, "height")),
                    (
                        "cover",
                        (element.keyword("cover") == Some("compact")).then(|| "true".to_string())
                    ),
                    ("appear", transition("appear")),
                    ("disappear", transition("disappear")),
                    ("appearDelay", Self::number(element, "appear_delay")),
                ])
            ),
            &element.children,
        )
    }

    /// `Shortcut`: its keys run the action while it is mounted and
    /// enabled.
    pub(super) fn key_shortcut(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::KeyShortcut(p, c, {});",
            designated(&[
                ("keys", Self::keyword(element, "key")),
                ("bound", self.shortcut_keys(element, "key")),
                ("enabled", self.enabled(element)),
                ("action", self.action(element)),
            ])
        ));
    }

    pub(super) fn timer(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Timer(p, c, {});",
            designated(&[
                ("interval", Self::number(element, "interval")),
                ("enabled", self.enabled(element)),
                ("action", self.action(element)),
            ])
        ));
    }

    pub(super) fn watch(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Watch(p, c, {});",
            designated(&[
                (
                    "value",
                    element.expr("value").map(|expr| self.getter(expr)),
                ),
                ("action", self.action(element)),
            ])
        ));
    }

    pub(super) fn control_group(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::ControlGroup(p, c, {}, ",
                designated(&[
                    ("label", self.text_prop(element, "label")),
                    ("height", self.len(element, "height")),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn rail(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Rail(p, c, {}, ",
                designated(&[
                    ("axis", Self::keyword(element, "axis")),
                    ("anchor", Self::keyword(element, "anchor")),
                    ("overhang", self.len(element, "overhang")),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn scroll_view(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::ScrollView(p, c, {}, ",
                designated(&[
                    ("axis", Self::keyword(element, "axis")),
                    ("snap", Self::keyword(element, "snap")),
                    ("position", self.field(element, "position")),
                    ("arrival", Self::keyword(element, "arrival")),
                    ("nudgeMotion", Some(motion_ref("jelly"))),
                    ("reflow", Self::reflow(element)),
                    (
                        "hideIndicators",
                        (element.keyword("indicators") == Some("hidden")).then(|| "true".into()),
                    ),
                    (
                        "mouseIndicators",
                        (element.keyword("indicators") == Some("mouse")).then(|| "true".into()),
                    ),
                    ("openAt", Self::number(element, "open_at")),
                    ("openCenter", self.len(element, "open_center")),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn form(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Form(p, c, {}, {}, ",
                null(self.text_prop(element, "title")),
                designated(&[
                    ("gap", Self::number(element, "gap")),
                    ("rowHeight", Self::number(element, "row_height")),
                    ("paddingX", Self::number(element, "padding_x")),
                    ("paddingY", Self::number(element, "padding_y")),
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Section(p, c, {}, {}, {}, ",
                null(self.text_prop(element, "title")),
                null(self.text_prop(element, "footer")),
                designated(&[
                    (
                        "plain",
                        (element.keyword("style") == Some("plain")).then(|| "true".to_string())
                    ),
                    ("paddingX", Self::number(element, "padding_x")),
                    ("paddingY", Self::number(element, "padding_y")),
                ])
            ),
            &element.children,
        );
    }

    /// A `Section` among the items of a sidebar.
    pub(super) fn sidebar_section(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::SidebarSection(p, c, {}, ",
                null(self.text_prop(element, "title"))
            ),
            &element.children,
        );
    }

    /// A `List` of `items:`, or of its children.
    pub(super) fn list(&self, out: &mut Out, element: &Element) {
        if element.expr("items").is_some() {
            out.open(format!("B::List(p, c, {BUILDER}"));
            self.for_each(out, element);
            out.close("});");
        } else {
            self.call(out, "B::List(p, c, ", &element.children);
        }
    }

    pub(super) fn preview_list(&self, out: &mut Out, element: &Element) {
        if let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            self.call(
                out,
                &format!(
                    "B::PreviewList(p, c, {}, {field}, {}, {}, {}, {}, ",
                    self.getter(items),
                    string(&template.var),
                    element.number("item_height").unwrap_or(1.0),
                    element.number("gap").unwrap_or(0.0),
                    element.number("more_height").unwrap_or(22.0),
                ),
                &element.children,
            );
        }
    }

    pub(super) fn for_each(&self, out: &mut Out, element: &Element) {
        match (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) {
            (Some(items), Some(template), Some(Prop::Key { field, .. })) => self.call(
                out,
                &format!(
                    "B::Each(p, c, {}, ",
                    designated(&[
                        ("items", Some(self.getter(items))),
                        ("key", Some(field.to_string())),
                        ("variable", Some(string(&template.var))),
                        ("stagger", Self::number(element, "stagger")),
                        ("reflow", Self::reflow(element)),
                    ])
                ),
                &element.children,
            ),
            _ => self.nodes(out, &element.children),
        }
    }

    pub(super) fn query(&self, out: &mut Out, element: &Element) {
        let (Some(binding), Some(template)) = (element.query("query"), &element.template) else {
            return;
        };
        let params: Vec<_> = binding.args.iter().map(|arg| self.getter(arg)).collect();
        let props = designated(&[
            ("id", element.id.as_ref().map(|_| self.id(element))),
            ("query", Some(binding.query.to_string())),
            (
                "params",
                (!params.is_empty()).then(|| format!("{{{}}}", params.join(", "))),
            ),
            ("loading", self.text_prop(element, "loading")),
            ("failed", self.text_prop(element, "failed")),
            ("empty", self.text_prop(element, "empty")),
            ("icon", Self::icon(element, "icon")),
            ("variable", Some(string(&template.var))),
        ]);
        self.call(out, &format!("B::Query(p, c, {props}, "), &element.children);
    }
}
