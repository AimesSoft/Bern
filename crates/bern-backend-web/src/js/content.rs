//! Content: text and labels, images, video, progress, toasts and
//! drawings.

use super::{Generator, Out, member, number, string};
use bern_ir::{Element, Node, Prop};
use bern_protocol::Type;

/// A role color for the runtime, from a keyword prop.
fn role(element: &Element, name: &str) -> Option<String> {
    element
        .keyword(name)
        .map(|role| format!("{{ role: {} }}", string(role)))
}

impl Generator<'_> {
    /// A `Text`; one with `mark` is the feature runtime's `MarkedText`.
    pub(super) fn text_element(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "{}(p, ctx, {});",
            if element.prop("mark").is_some() {
                "F.MarkedText"
            } else {
                "B.Text"
            },
            Self::props(vec![
                ("content", self.text_prop(element, "content")),
                ("style", Self::keyword(element, "style")),
                ("emphasis", Self::keyword(element, "emphasis")),
                ("mark", Self::keyword(element, "mark")),
                ("lines", element.number("lines").map(number)),
                ("fontSize", self.length(element, "font_size")),
                ("lineHeight", self.length(element, "line_height")),
            ])
        ));
    }

    pub(super) fn label(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.Label(p, ctx, {});",
            Self::props(vec![
                ("title", self.text_prop(element, "title")),
                ("icon", element.icon("icon").map(string)),
                ("style", Self::keyword(element, "style")),
                ("fontSize", element.number("font_size").map(number)),
                ("display", Self::keyword(element, "display")),
            ])
        ));
    }

    pub(super) fn svg(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "F.Svg(p, ctx, {});",
            Self::props(vec![
                ("source", element.resource("source").map(string)),
                ("label", self.text_prop(element, "label")),
                ("width", self.length(element, "width")),
                ("height", self.length(element, "height")),
                ("fit", Self::keyword(element, "fit")),
                ("playing", self.value_prop(element, "playing")),
                ("speed", self.value_prop(element, "speed")),
                ("restart", self.value_prop(element, "restart")),
                (
                    "interactive",
                    element.keyword("interactive").map(str::to_owned)
                ),
            ])
        ));
    }

    pub(super) fn image(&self, out: &mut Out, element: &Element) {
        let url = element.expr("source").map(|expr| {
            if matches!(expr.ty, Type::Optional { .. }) {
                format!("() => ({})?.url ?? null", self.expr(expr))
            } else {
                format!("() => ({}).url", self.expr(expr))
            }
        });
        out.line(format!(
            "F.Image(p, ctx, {});",
            Self::props(vec![
                ("source", element.resource("source").map(string)),
                ("url", url),
                ("label", self.text_prop(element, "label")),
                ("width", self.length(element, "width")),
                ("height", self.length(element, "height")),
                ("fit", Self::keyword(element, "fit")),
                ("animate", self.animate(element)),
                ("corner", self.length(element, "corner")),
                ("rendering", Self::keyword(element, "rendering")),
                ("aspect", element.number("aspect").map(number)),
            ])
        ));
    }

    pub(super) fn video(&self, out: &mut Out, element: &Element) {
        let binding = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => {
                let value = self.expr(source);
                if matches!(source.ty, Type::Optional { .. }) {
                    format!("source: () => ({value})?.url ?? null")
                } else {
                    format!("source: () => ({value}).url")
                }
            }
            (None, Some(Prop::Player { id })) => format!("player: {}", string(id)),
            _ => "source: () => null".into(),
        };
        out.line(format!(
            "B.Video(p, ctx, {});",
            Self::props(vec![
                ("id", Some(self.id(element))),
                ("label", self.text_prop(element, "label")),
                (
                    "controls",
                    Some((element.keyword("controls") != Some("none")).to_string())
                ),
                ("fit", Self::keyword(element, "fit")),
                ("corner", self.length(element, "corner")),
                ("animate", self.animate(element)),
            ])
            .replacen("{ ", &format!("{{ {binding}, "), 1)
        ));
    }

    pub(super) fn progress(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "B.Progress(p, ctx, {});",
            Self::props(vec![
                (
                    "value",
                    element
                        .expr("value")
                        .map(|value| format!("() => {}", self.expr(value)))
                ),
                ("total", element.number("total").map(number)),
                ("label", self.text_prop(element, "label")),
            ])
        ));
    }

    pub(super) fn toast(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let key = element
            .expr("key")
            .map(|key| format!("() => {}", self.expr(key)))
            .unwrap_or_else(|| content.clone());
        out.line(format!(
            "F.Toast(p, ctx, {});",
            Self::props(vec![
                ("content", Some(content)),
                ("icon", element.icon("icon").map(string)),
                ("key", Some(key)),
                ("duration", element.number("duration").map(number)),
                ("motion", Some(self.motion("bouncy"))),
            ])
        ));
    }

    /// A `Canvas`: its drawing elements in order, read in one getter.
    pub(super) fn canvas(&self, out: &mut Out, element: &Element) {
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else { continue };
            let number_of = |name: &str| child.number(name).map(number);
            match child.kind.as_str() {
                "Shape" => {
                    let fill_from = match (child.expr("fill_from"), child.resource("fill_from")) {
                        (Some(expr), _) if matches!(expr.ty, Type::Optional { .. }) => Some(
                            format!("F.averageColor(({})?.url ?? null)", self.expr(expr)),
                        ),
                        (Some(expr), _) => {
                            Some(format!("F.averageColor(({}).url)", self.expr(expr)))
                        }
                        (None, Some(name)) => {
                            Some(format!("F.averageColor(F.resource({}))", string(name)))
                        }
                        (None, None) => None,
                    };
                    ops.push(Self::props(vec![
                        (
                            "shape",
                            Some(string(child.keyword("kind").unwrap_or("rect"))),
                        ),
                        ("x", number_of("x")),
                        ("y", number_of("y")),
                        ("width", number_of("width")),
                        ("height", number_of("height")),
                        ("corner", number_of("corner")),
                        ("fill", role(child, "fill")),
                        ("fillFrom", fill_from),
                        ("fade", Self::keyword(child, "fade")),
                        ("stroke", role(child, "stroke")),
                        ("strokeWidth", number_of("stroke_width")),
                        ("dash", number_of("dash")),
                        ("dashGap", number_of("dash_gap")),
                        ("opacity", number_of("opacity")),
                    ]));
                }
                "Series" => {
                    let values = child
                        .expr("values")
                        .map(|list| format!("({}).map(Number)", self.expr(list)));
                    let max = child
                        .expr("max")
                        .map(|max| format!("Number({})", self.expr(max)))
                        .or_else(|| number_of("max"));
                    ops.push(Self::props(vec![
                        ("series", values),
                        ("max", max),
                        (
                            "area",
                            Some((child.keyword("kind") == Some("area")).to_string()),
                        ),
                        ("fill", role(child, "fill")),
                        ("stroke", role(child, "stroke")),
                        ("strokeWidth", number_of("stroke_width")),
                        ("opacity", number_of("opacity")),
                    ]));
                }
                "Pattern" => ops.push(Self::props(vec![
                    (
                        "pattern",
                        child
                            .keyword("name")
                            .map(|name| format!("P.{}", member(name))),
                    ),
                    ("opacity", number_of("opacity")),
                ])),
                _ => {}
            }
        }
        out.line(format!(
            "F.Canvas(p, ctx, {}, () => [{}]);",
            Self::props(vec![
                ("width", self.length(element, "width")),
                ("height", self.length(element, "height")),
                ("label", self.text_prop(element, "label")),
                ("animate", self.animate(element)),
            ]),
            ops.join(", ")
        ));
    }
}
