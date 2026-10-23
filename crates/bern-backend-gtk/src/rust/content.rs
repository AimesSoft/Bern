//! Content: text and labels, images, video, progress, toasts and
//! drawings.

use super::{Generator, Out, constant, some, string};
use bern_ir::{Element, Node, Prop};

impl Generator<'_> {
    /// A `Text`, with its `mark` as a role color.
    pub(super) fn text_element(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Text(p, c, b::TextProps {{ content: {}, style: {}, emphasis: {}, mark: {}, lines: {}, font_size: {}, line_height: {} }});",
            some(self.text_prop(element, "content")),
            Self::keyword(element, "style"),
            Self::keyword(element, "emphasis"),
            some(
                element
                    .keyword("mark")
                    .map(|role| format!("b::Color::Role({})", string(role)))
            ),
            Self::number(element, "lines"),
            some(self.num(element, "font_size")),
            some(self.num(element, "line_height")),
        ));
    }

    pub(super) fn label(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Label(p, c, b::LabelProps {{ title: {}, icon: {}, style: {}, font_size: {}, icon_only: {} }});",
            some(self.text_prop(element, "title")),
            Self::icon(element, "icon"),
            Self::keyword(element, "style"),
            Self::number(element, "font_size"),
            element.keyword("display") == Some("icon"),
        ));
    }

    pub(super) fn svg(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Svg(p, c, b::SvgProps {{ source: {}, label: {}, width: {}, height: {}, fit: {}, playing: {}, speed: {}, restart: {}, interactive: {}, data: {}, external_links: {} }});",
            string(element.resource("source").unwrap_or_default()),
            some(self.text_prop(element, "label")),
            some(self.num(element, "width")), some(self.num(element, "height")),
            Self::keyword(element, "fit"),
            some(element.expr("playing").map(|e| self.getter(e))),
            some(element.expr("speed").map(|e| self.getter(e))),
            some(element.expr("restart").map(|e| self.getter(e))),
            element.keyword("interactive").unwrap_or("true"),
            some(self.text_prop(element, "data")),
            element.keyword("external_links").unwrap_or("false"),
        ));
    }

    pub(super) fn image(&self, out: &mut Out, element: &Element) {
        let shared = format!(
            "label: {}, width: {}, height: {}, fit: {}, animate: {}, corner: {}, template: {}, aspect: {}",
            some(self.text_prop(element, "label")),
            some(self.num(element, "width")),
            some(self.num(element, "height")),
            Self::keyword(element, "fit"),
            self.animate(element),
            some(self.num(element, "corner")),
            element.keyword("rendering") == Some("template"),
            Self::number(element, "aspect"),
        );
        match element.expr("source") {
            Some(source) => out.line(format!(
                "b::RuntimeImage(p, c, b::RuntimeImageProps {{ url: {}, {shared} }});",
                self.getter(source),
            )),
            None => out.line(format!(
                "b::Image(p, c, b::ImageProps {{ source: {}, {shared} }});",
                string(element.resource("source").unwrap_or_default()),
            )),
        }
    }

    pub(super) fn video(&self, out: &mut Out, element: &Element) {
        let (source, player) = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => (Some(self.getter(source)), None),
            (None, Some(Prop::Player { id })) => (None, Some(string(id))),
            _ => (None, None),
        };
        out.line(format!(
            "b::Video(p, c, b::VideoProps {{ id: {}, source: {}, player: {}, label: {}, controls: {}, fit: {}, fills_container: {}, corner: {}, animate: {} }});",
            self.id(element),
            some(source),
            some(player),
            some(self.text_prop(element, "label")),
            element.keyword("controls") != Some("none"),
            Self::keyword(element, "fit"),
            element.keyword("sizing") == Some("fill"),
            some(self.num(element, "corner")),
            self.animate(element),
        ));
    }

    pub(super) fn progress(&self, out: &mut Out, element: &Element) {
        out.line(format!(
            "b::Progress(p, c, b::ProgressProps {{ value: {}, total: {}, label: {} }});",
            some(element.expr("value").map(|value| self.getter(value))),
            Self::number(element, "total"),
            some(self.text_prop(element, "label")),
        ));
    }

    pub(super) fn toast(&self, out: &mut Out, element: &Element) {
        let content = self
            .text_prop(element, "content")
            .unwrap_or_else(|| "b::str(|_| String::new())".into());
        let key = match (element.expr("key"), element.text("content")) {
            (Some(key), _) => self.getter(key),
            (None, Some(text)) => format!("b::get(|c| V::Text({}))", self.text(text)),
            (None, None) => "b::get(|_| V::Unit)".into(),
        };
        out.line(format!(
            "b::Toast(p, c, b::ToastProps {{ content: {content}, icon: {}, key: {key}, duration: {} }});",
            Self::icon(element, "icon"),
            Self::number(element, "duration"),
        ));
    }

    /// A `Canvas`: its drawing elements in order, read in one getter.
    pub(super) fn canvas(&self, out: &mut Out, element: &Element) {
        let role = |child: &Element, name: &str| {
            some(
                child
                    .keyword(name)
                    .map(|role| format!("b::Color::Role({})", string(role))),
            )
        };
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else { continue };
            match child.kind.as_str() {
                "Shape" => {
                    let fill_from = match (child.expr("fill_from"), child.resource("fill_from")) {
                        (Some(expr), _) => {
                            Some(format!("b::Source::Url(b::url(&{}))", self.expr(expr)))
                        }
                        (None, Some(name)) => {
                            Some(format!("b::Source::Resource({})", string(name)))
                        }
                        (None, None) => None,
                    };
                    ops.push(format!(
                        "b::Op::Shape(b::ShapeOp {{ kind: {}, x: {}, y: {}, width: {}, height: {}, corner: {}, fill: {}, fill_from: {}, fade: {}, stroke: {}, stroke_width: {}, dash: {}, dash_gap: {}, opacity: {} }})",
                        string(child.keyword("kind").unwrap_or("rect")),
                        Self::number(child, "x"),
                        Self::number(child, "y"),
                        Self::number(child, "width"),
                        Self::number(child, "height"),
                        Self::number(child, "corner"),
                        role(child, "fill"),
                        some(fill_from),
                        Self::keyword(child, "fade"),
                        role(child, "stroke"),
                        Self::number(child, "stroke_width"),
                        Self::number(child, "dash"),
                        Self::number(child, "dash_gap"),
                        Self::number(child, "opacity"),
                    ));
                }
                "Series" => {
                    let values = child
                        .expr("values")
                        .map(|list| format!("b::numbers({})", self.expr(list)))
                        .unwrap_or_else(|| "Vec::new()".into());
                    let max = child
                        .expr("max")
                        .map(|max| format!("Some(rt::as_f64(&{}))", self.expr(max)))
                        .unwrap_or_else(|| Self::number(child, "max"));
                    ops.push(format!(
                        "b::Op::Series(b::SeriesOp {{ values: {values}, max: {max}, area: {}, fill: {}, stroke: {}, stroke_width: {}, opacity: {} }})",
                        child.keyword("kind") == Some("area"),
                        role(child, "fill"),
                        role(child, "stroke"),
                        Self::number(child, "stroke_width"),
                        Self::number(child, "opacity"),
                    ));
                }
                "Pattern" => {
                    if let Some(name) = child.keyword("name") {
                        ops.push(format!(
                            "b::Op::Pattern(&{}, {})",
                            constant("P", name),
                            Self::number(child, "opacity")
                        ));
                    }
                }
                _ => {}
            }
        }
        out.line(format!(
            "b::Canvas(p, c, b::CanvasProps {{ width: {}, height: {}, label: {}, animate: {} }}, Rc::new(|c: &b::Ctx| vec![{}]));",
            some(self.num(element, "width")),
            some(self.num(element, "height")),
            some(self.text_prop(element, "label")),
            self.animate(element),
            ops.join(", ")
        ));
    }
}
