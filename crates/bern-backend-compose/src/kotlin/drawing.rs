//! `Canvas` drawings: shapes, series and patterns.

use super::{Ctx, Generator, Out};
use crate::names::{double, member, string};
use bern_ir::{Element, Node};
use bern_protocol::Type;

impl Generator<'_> {
    /// A `Canvas`: shapes, series and patterns in order, in canvas dp.
    pub(super) fn canvas(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let color = |element: &Element, name: &str| {
            element
                .keyword(name)
                .map(|role| format!("BernColor.Role({})", string(role)))
                .unwrap_or_else(|| "null".into())
        };
        let number = |element: &Element, name: &str, default: f64| {
            double(element.number(name).unwrap_or(default))
        };
        let length = |name| self.length(element, name).unwrap_or_else(|| "null".into());
        let semantics = match self.text_prop(element, "label") {
            Some(label) => {
                format!(".semantics {{ contentDescription = {label}; role = Role.Image }}")
            }
            None => ".clearAndSetSemantics {}".into(),
        };
        out.open("run {");
        out.line("val bernPalette = bernPalette()");
        // The average colors of the pictures shapes are filled from, read
        // while composing so the drawing follows them once known.
        for (index, node) in element.children.iter().enumerate() {
            let Node::Element(child) = node else { continue };
            let url = match (child.expr("fill_from"), child.resource("fill_from")) {
                (Some(expr), _) if matches!(expr.ty, Type::Optional { .. }) => {
                    format!("({})?.url", self.expr(expr))
                }
                (Some(expr), _) => format!("({}).url", self.expr(expr)),
                (None, Some(name)) => format!("BernPictures.resource({})", string(name)),
                (None, None) => continue,
            };
            out.line(format!(
                "val bernFill{index} = BernPictures.average(LocalContext.current, {url})"
            ));
        }
        let mut values = Vec::new();
        for node in &element.children {
            if let Node::Element(child) = node
                && child.kind == "Series"
                && let Some(list) = child.expr("values")
            {
                let index = values.len();
                let read = match &list.ty {
                    Type::List { inner } if matches!(**inner, Type::Int) => {
                        format!("({}).map {{ it.toDouble() }}", self.expr(list))
                    }
                    _ => self.expr(list),
                };
                out.line(format!("val bernValues{index} = {read}"));
                let max = child
                    .expr("max")
                    .map(|max| format!("({}).toDouble()", self.expr(max)))
                    .unwrap_or_else(|| "null".into());
                out.line(format!("val bernMax{index}: Double? = {max}"));
                values.push(index);
            }
        }
        // Without a size, a layer of a stack is its backdrop: as large as
        // the other layers, without making the stack larger.
        let frame = if ctx.layer
            && ctx.sized_layers
            && element.prop("width").is_none()
            && element.prop("height").is_none()
        {
            "Modifier.matchParentSize()".to_string()
        } else {
            format!(
                "Modifier.bernFrame({}, {})",
                length("width"),
                length("height")
            )
        };
        out.open(format!("Canvas({frame}{semantics}) {{"));
        let mut series = 0;
        for (index, node) in element.children.iter().enumerate() {
            // A canvas holds its drawing elements directly.
            let Node::Element(child) = node else { continue };
            match child.kind.as_str() {
                "Shape" => {
                    let filled = child.prop("fill_from").is_some();
                    let fill = match (filled, color(child, "fill")) {
                        (false, fill) => fill,
                        (true, fallback) if fallback == "null" => format!("bernFill{index}"),
                        (true, fallback) => format!("(bernFill{index} ?: {fallback})"),
                    };
                    let fade = child
                        .keyword("fade")
                        .map(|edge| format!(", fade = {}", string(edge)))
                        .unwrap_or_default();
                    // Dashes and gaps of the stroke; none draws it whole.
                    let dash = child
                        .number("dash")
                        .map(|dash| {
                            format!(
                                ", dash = {}, dashGap = {}",
                                double(dash),
                                double(child.number("dash_gap").unwrap_or(dash))
                            )
                        })
                        .unwrap_or_default();
                    let optional = |name: &str| {
                        child
                            .number(name)
                            .map(double)
                            .unwrap_or_else(|| "null".into())
                    };
                    out.line(format!(
                        "BernDraw.shape(this, bernPalette, {}, {}, {}, {}, {}, {}, {fill}, {}, {}, {}{fade}{dash})",
                        string(child.keyword("kind").unwrap_or("rect")),
                        number(child, "x", 0.0),
                        number(child, "y", 0.0),
                        optional("width"),
                        optional("height"),
                        number(child, "corner", 8.0),
                        color(child, "stroke"),
                        number(child, "stroke_width", 1.0),
                        number(child, "opacity", 1.0),
                    ));
                }
                "Series" => {
                    out.line(format!(
                        "BernDraw.series(this, bernPalette, bernValues{series}, bernMax{series}, {}, {}, {}, {}, {})",
                        child.keyword("kind") == Some("area"),
                        color(child, "fill"),
                        color(child, "stroke"),
                        number(child, "stroke_width", 2.0),
                        number(child, "opacity", 1.0),
                    ));
                    series += 1;
                }
                "Pattern" => out.line(format!(
                    "BernDraw.pattern(this, bernPalette, BernPatterns.{}, {})",
                    member(child.keyword("name").expect("pattern name")),
                    number(child, "opacity", 1.0),
                )),
                _ => {}
            }
        }
        out.close("}");
        out.close("}");
    }
}
