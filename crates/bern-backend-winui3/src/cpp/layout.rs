//! Layout: stacks, grids, layers and rails, scroll views, forms and
//! sections, lists, repetitions and queries.

use super::{BUILDER, Generator, Out, designated, motion_ref, null, string};
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
                ])
            ),
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
                ])
            ),
            &element.children,
        );
    }

    pub(super) fn form(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!("B::Form(p, c, {}, ", null(self.text_prop(element, "title"))),
            &element.children,
        );
    }

    pub(super) fn section(&self, out: &mut Out, element: &Element) {
        self.call(
            out,
            &format!(
                "B::Section(p, c, {}, {}, ",
                null(self.text_prop(element, "title")),
                null(self.text_prop(element, "footer"))
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
