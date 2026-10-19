//! `Canvas` drawings: shapes, series and patterns.

use super::{Generator, Out};
use crate::names::{double, string};
use bern_ir::{Element, Node};
use bern_protocol::Type;

impl Generator<'_> {
    /// A `Canvas`: its drawing runs on values captured when the view is
    /// built, so state the drawing reads still updates the view.
    pub(super) fn canvas(&self, out: &mut Out, element: &Element) {
        let mut captures = Vec::new();
        let mut body = Out {
            indent: out.indent + 1,
            ..Out::default()
        };
        self.drawing(&mut body, &element.children, &mut captures);
        let list = if captures.is_empty() {
            String::new()
        } else {
            format!(
                "[{}] ",
                captures
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("v{index} = {value}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        out.line(format!("SwiftUI.Canvas {{ {list}context, size in"));
        out.lines.extend(body.lines);
        out.line("}");
        let length = |name| self.length(element, name).unwrap_or_else(|| "nil".into());
        let (width, height) = (length("width"), length("height"));
        out.line(format!(
            ".frame(width: {width}, height: {height}){}",
            match (element.prop("width"), element.prop("height")) {
                (None, None) => ".frame(maxWidth: .infinity, maxHeight: .infinity)",
                (None, Some(_)) => ".frame(maxWidth: .infinity)",
                (Some(_), None) => ".frame(maxHeight: .infinity)",
                _ => "",
            }
        ));
        match self.text_prop(element, "label") {
            Some(label) => {
                out.line(".accessibilityElement(children: .ignore)");
                out.line(".accessibilityAddTraits(.isImage)");
                out.line(format!(
                    ".accessibilityLabel(SwiftUI.Text(verbatim: {label}))"
                ));
            }
            None => out.line(".accessibilityHidden(true)"),
        }
    }

    fn drawing(&self, out: &mut Out, nodes: &[Node], captures: &mut Vec<String>) {
        fn capture(captures: &mut Vec<String>, value: String) -> String {
            captures.push(value);
            format!("v{}", captures.len() - 1)
        }
        let color = |element: &Element, name: &str| {
            element
                .keyword(name)
                .map(|role| format!(".role({})", string(role)))
                .unwrap_or_else(|| "nil".into())
        };
        let number = |element: &Element, name: &str, default: f64| {
            double(element.number(name).unwrap_or(default))
        };
        for node in nodes {
            // A canvas holds its drawing elements directly.
            let Node::Element(element) = node else {
                continue;
            };
            match element.kind.as_str() {
                "Shape" => {
                    let at = |name: &str, extent: &str, origin: &str| match element.number(name) {
                        Some(value) => double(value),
                        None => format!("size.{extent} - {origin}"),
                    };
                    let x = number(element, "x", 0.0);
                    let y = number(element, "y", 0.0);
                    // A picture's average color, read while the view is
                    // built so the drawing follows it once it is known.
                    let fallback = color(element, "fill");
                    let picture = match (element.expr("fill_from"), element.resource("fill_from")) {
                        (Some(expr), _) => {
                            let url = if matches!(expr.ty, Type::Optional { .. }) {
                                format!("({})?.url", self.expr(expr))
                            } else {
                                format!("({}).url", self.expr(expr))
                            };
                            capture(captures, format!("BernPictures.shared.average({url})"))
                        }
                        (None, Some(name)) => capture(
                            captures,
                            format!(
                                "BernPictures.shared.average(Bundle.main.resourceURL?.appendingPathComponent({}).absoluteString)",
                                string(name)
                            ),
                        ),
                        (None, None) => "nil".into(),
                    };
                    // Until the picture is read, its `fill` stands in.
                    let fill = match (picture.as_str(), fallback.as_str()) {
                        ("nil", _) => fallback,
                        (_, "nil") => picture,
                        _ => format!("({picture} ?? {fallback})"),
                    };
                    let fade = element
                        .keyword("fade")
                        .map(string)
                        .unwrap_or_else(|| "nil".into());
                    // Dashes and gaps of the stroke; none draws it whole.
                    let dash = match element.number("dash") {
                        Some(dash) => format!(
                            ", dash: [{}, {}]",
                            double(dash),
                            double(element.number("dash_gap").unwrap_or(dash))
                        ),
                        None => String::new(),
                    };
                    out.line(format!(
                            "BernDraw.shape(&context, {}, CGRect(x: {x}, y: {y}, width: {}, height: {}), corner: {}, fill: {fill}, fade: {fade}, stroke: {}, strokeWidth: {}{dash}, opacity: {})",
                            string(element.keyword("kind").unwrap_or("rect")),
                            at("width", "width", &x),
                            at("height", "height", &y),
                            number(element, "corner", 8.0),
                            color(element, "stroke"),
                            number(element, "stroke_width", 1.0),
                            number(element, "opacity", 1.0),
                        ));
                }
                "Series" => {
                    let values = element.expr("values").expect("series values");
                    let list = match &values.ty {
                        Type::List { inner } if matches!(**inner, Type::Int) => {
                            format!("({}).map {{ Double($0) }}", self.expr(values))
                        }
                        _ => self.expr(values),
                    };
                    let values = capture(captures, list);
                    let max = match element.expr("max") {
                        Some(max) => capture(captures, format!("Double({})", self.expr(max))),
                        None => "nil".into(),
                    };
                    out.line(format!(
                            "BernDraw.series(&context, size, values: {values}, max: {max}, area: {}, fill: {}, stroke: {}, strokeWidth: {}, opacity: {})",
                            element.keyword("kind") == Some("area"),
                            color(element, "fill"),
                            color(element, "stroke"),
                            number(element, "stroke_width", 2.0),
                            number(element, "opacity", 1.0),
                        ));
                }
                "Pattern" => out.line(format!(
                    "BernDraw.pattern(&context, size, BernPatterns.`{}`, opacity: {})",
                    element.keyword("name").expect("pattern name"),
                    number(element, "opacity", 1.0),
                )),
                _ => {}
            }
        }
    }
}
