//! Content: text, labels, pictures, progress, video, toasts and the
//! drawings of a canvas.

use super::{Generator, Out, designated, float, ident, motion_ref, string};
use bern_ir::{Element, Node, Prop};

/// A role keyword of a drawing element as a `B::Paint`.
fn role(element: &Element, name: &str) -> Option<String> {
    element
        .keyword(name)
        .map(|role| format!("B::Paint{{{}}}", string(role)))
}

impl Generator<'_> {
    pub(super) fn text_element(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Text(p, c, {});",
            designated(&[
                ("content", self.text_prop(element, "content")),
                ("style", Self::keyword(element, "style")),
                ("emphasis", Self::keyword(element, "emphasis")),
                ("mark", Self::keyword(element, "mark")),
                ("lines", Self::number(element, "lines")),
                ("fontSize", self.len(element, "font_size")),
                ("lineHeight", self.len(element, "line_height")),
                (
                    "markdown",
                    (element.keyword("markdown") == Some("true")).then(|| "true".to_string()),
                ),
            ])
        ));
    }

    pub(super) fn label(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Label(p, c, {});",
            designated(&[
                ("title", self.text_prop(element, "title")),
                ("icon", Self::icon(element, "icon")),
                ("style", Self::keyword(element, "style")),
                ("fontSize", Self::number(element, "font_size")),
                (
                    "iconOnly",
                    (element.keyword("display") == Some("icon")).then(|| "true".to_string()),
                ),
            ])
        ));
    }

    pub(super) fn svg(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Svg(p, c, {});",
            designated(&[
                ("source", element.resource("source").map(string)),
                ("label", self.text_prop(element, "label")),
                ("width", self.len(element, "width")),
                ("height", self.len(element, "height")),
                ("fit", Self::keyword(element, "fit")),
                ("playing", element.expr("playing").map(|e| self.getter(e))),
                ("speed", element.expr("speed").map(|e| self.getter(e))),
                ("restart", element.expr("restart").map(|e| self.getter(e))),
                ("data", self.text_prop(element, "data")),
                (
                    "externalLinks",
                    element.keyword("external_links").map(str::to_owned)
                ),
                (
                    "interactive",
                    element.keyword("interactive").map(str::to_owned)
                ),
            ])
        ));
    }

    pub(super) fn image(&self, out: &mut Out, element: &Element) {
        let url = element.expr("source").map(|expr| self.getter(expr));
        out.line(format!(
            "B::Image(p, c, {});",
            designated(&[
                ("source", element.resource("source").map(string)),
                ("darkSource", element.resource("dark_source").map(string)),
                ("url", url),
                ("label", self.text_prop(element, "label")),
                ("width", self.len(element, "width")),
                ("height", self.len(element, "height")),
                ("fit", Self::keyword(element, "fit")),
                ("animate", Self::animate(element)),
                ("corner", self.len(element, "corner")),
                (
                    "templated",
                    (element.keyword("rendering") == Some("template")).then(|| "true".to_string())
                ),
                ("aspect", Self::number(element, "aspect")),
                (
                    "flies",
                    element
                        .keyword("hero")
                        .is_some()
                        .then(|| "true".to_string())
                ),
            ])
        ));
    }

    pub(super) fn progress(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B::Progress(p, c, {});",
            designated(&[
                (
                    "value",
                    element.expr("value").map(|value| self.getter(value))
                ),
                ("total", Self::number(element, "total")),
                ("label", self.text_prop(element, "label")),
            ])
        ));
    }

    pub(super) fn video(&self, out: &mut Out, element: &Element) {
        let (source, player) = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => (Some(self.getter(source)), None),
            (None, Some(Prop::Player { id })) => (None, Some(string(id))),
            _ => (None, None),
        };
        let controls = element.keyword("controls") != Some("none");
        out.line(format!(
            "B::Video(p, c, {});",
            designated(&[
                ("id", Some(self.id(element))),
                ("source", source),
                ("player", player),
                ("label", self.text_prop(element, "label")),
                ("controls", (!controls).then(|| "false".to_string())),
                ("fit", Self::keyword(element, "fit")),
                (
                    "fillsContainer",
                    (element.keyword("sizing") == Some("fill")).then(|| "true".to_string()),
                ),
                ("corner", self.len(element, "corner")),
                ("animate", Self::animate(element)),
            ])
        ));
    }

    /// A `Toast`, shown again when its `key` (else its content) changes.
    pub(super) fn toast(&self, out: &mut Out, element: &Element) {
        let key = element.expr("key").map(|key| self.getter(key)).or_else(|| {
            element.text("content").map(|text| {
                format!(
                    "[](Ctx const& c) {{ return Value::Text(Utf8({})); }}",
                    self.text(text)
                )
            })
        });
        out.line(format!(
            "B::Toast(p, c, {});",
            designated(&[
                ("content", self.text_prop(element, "content")),
                ("icon", Self::icon(element, "icon")),
                ("key", key),
                ("duration", Self::number(element, "duration")),
                ("motion", Some(motion_ref("bouncy"))),
            ])
        ));
    }

    /// A `Canvas`: its drawing elements in order, read in one getter so the
    /// canvas redraws when the state they show changes.
    pub(super) fn canvas(&self, out: &mut Out, element: &Element) {
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else { continue };
            match child.kind.as_str() {
                "Shape" => ops.push(self.shape_op(child)),
                "Series" => ops.push(self.series_op(child)),
                "Pattern" => ops.push(Self::pattern_op(child)),
                _ => {}
            }
        }
        out.open(format!(
            "B::Canvas(p, c, {}, [](Ctx const& c) {{",
            designated(&[
                ("width", self.len(element, "width")),
                ("height", self.len(element, "height")),
                ("label", self.text_prop(element, "label")),
                ("animate", Self::animate(element)),
            ])
        ));
        out.line("std::vector<B::DrawOp> ops;");
        for op in ops {
            out.line(format!("ops.push_back({op});"));
        }
        out.line("return ops;");
        out.close("});");
    }

    /// A `Shape` of a canvas, as a `B::ShapeOp`.
    fn shape_op(&self, child: &Element) -> String {
        let number = |name: &str| Self::number(child, name);
        let fill_from = match (child.expr("fill_from"), child.resource("fill_from")) {
            (Some(expr), _) => Some(format!("R::Url({})", self.expr(expr))),
            (None, Some(name)) => Some(format!("B::ResourceUrl({})", string(name))),
            (None, None) => None,
        };
        format!(
            "B::ShapeOp{}",
            designated(&[
                (
                    "kind",
                    Some(string(child.keyword("kind").unwrap_or("rect")))
                ),
                ("x", number("x")),
                ("y", number("y")),
                ("width", number("width")),
                ("height", number("height")),
                ("corner", number("corner")),
                ("fill", role(child, "fill")),
                ("fillFrom", fill_from),
                ("fade", Self::keyword(child, "fade")),
                ("stroke", role(child, "stroke")),
                ("strokeWidth", number("stroke_width")),
                ("dash", number("dash")),
                ("dashGap", number("dash_gap")),
                ("opacity", number("opacity")),
            ])
        )
    }

    /// A `Series` of a canvas, as a `B::SeriesOp`.
    fn series_op(&self, child: &Element) -> String {
        let number = |name: &str| Self::number(child, name);
        let values = child
            .expr("values")
            .map(|list| format!("R::Numbers({})", self.expr(list)));
        let max = match child.prop("max") {
            Some(Prop::Expr { expr }) => Some(format!("R::AsDouble({})", self.expr(expr))),
            Some(Prop::Number { value }) => Some(float(*value)),
            _ => None,
        };
        format!(
            "B::SeriesOp{}",
            designated(&[
                ("values", values),
                ("max", max),
                (
                    "area",
                    (child.keyword("kind") == Some("area")).then(|| "true".to_string())
                ),
                ("fill", role(child, "fill")),
                ("stroke", role(child, "stroke")),
                ("strokeWidth", number("stroke_width")),
                ("opacity", number("opacity")),
            ])
        )
    }

    /// A `Pattern` of a canvas, as a `B::PatternOp`.
    fn pattern_op(child: &Element) -> String {
        let number = |name: &str| Self::number(child, name);
        format!(
            "B::PatternOp{}",
            designated(&[
                (
                    "pattern",
                    child
                        .keyword("name")
                        .map(|name| format!("&P_{}", ident(name)))
                ),
                ("opacity", number("opacity")),
            ])
        )
    }
}
