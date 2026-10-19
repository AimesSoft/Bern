//! What elements show: text, labels, pictures, video, progress and
//! toasts.

use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, string};
use bern_ir::{Element, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    pub(super) fn text_view(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        out.line(format!("SwiftUI.Text(verbatim: {content})"));
        self.font(out, element);
        if let Some(height) = self.length(element, "line_height") {
            out.line(format!(
                ".bernLineHeight({height}, size: {})",
                self.text_size(element)
            ));
        }
        if let Some(lines) = element.number("lines") {
            out.line(format!(".lineLimit({})", lines.max(1.0) as i64));
            out.line(".truncationMode(.tail)");
            if lines > 1.0 {
                out.line(".fixedSize(horizontal: false, vertical: true)");
            }
        }
        match element.keyword("emphasis") {
            Some("secondary") => out.line(".foregroundStyle(.secondary)"),
            Some("tertiary") => out.line(".foregroundStyle(.tertiary)"),
            _ => {}
        }
        if let Some(role) = element.keyword("mark") {
            out.line(format!(".bernMark(.role({}))", string(role)));
        }
    }

    /// The point size a text is set in: its own, its style's, or the
    /// platform's body size.
    fn text_size(&self, element: &Element) -> String {
        let style = element.keyword("style").unwrap_or("body");
        self.length(element, "font_size")
            .or_else(|| {
                self.app
                    .style
                    .text_weights
                    .iter()
                    .find(|entry| entry.style == style)
                    .and_then(|entry| entry.size.map(double))
            })
            .unwrap_or_else(|| match self.platform {
                Platform::Macos => "13.0".into(),
                Platform::Ios => "17.0".into(),
            })
    }

    fn font(&self, out: &mut Out, element: &Element) {
        // A text's style is the body unless it says; the application's own
        // body, when it declares one, then applies to it too.
        let body_declared = self
            .app
            .style
            .text_weights
            .iter()
            .any(|entry| entry.style == "body");
        if let Some(style) = element
            .keyword("style")
            .or_else(|| element.prop("font_size").map(|_| "body"))
            .or_else(|| (element.kind == "Text" && body_declared).then_some("body"))
        {
            let font = match style {
                "large_title" => "largeTitle",
                "title" => "title",
                "title2" => "title2",
                "title3" => "title3",
                "headline" => "headline",
                "subheadline" => "subheadline",
                "callout" => "callout",
                "footnote" => "footnote",
                "caption" => "caption",
                "caption2" => "caption2",
                _ => "body",
            };
            // The weight the application gives the style, if any.
            match self
                .app
                .style
                .text_weights
                .iter()
                .find(|entry| entry.style == style)
            {
                Some(entry) => match self
                    .length(element, "font_size")
                    .or_else(|| entry.size.map(double))
                {
                    Some(size) => out.line(format!(
                        ".font(.system(size: {}, weight: .{}))",
                        size, entry.weight
                    )),
                    None => out.line(format!(".font(.{font}.weight(.{}))", entry.weight)),
                },
                None => match self.length(element, "font_size") {
                    Some(size) => out.line(format!(".font(.system(size: {size}))")),
                    None => out.line(format!(".font(.{font})")),
                },
            }
        }
    }

    pub(super) fn label_view(&self, out: &mut Out, element: &Element) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        let icon = element.icon("icon").unwrap_or("circle");
        out.line(format!(
            "SwiftUI.Label({title}, systemImage: {})",
            string(icon)
        ));
        if element.keyword("display") == Some("icon") {
            out.line(".labelStyle(.iconOnly)");
        }
        self.font(out, element);
    }

    pub(super) fn image(&self, out: &mut Out, element: &Element) {
        let fit = match element.keyword("fit") {
            Some("cover") => ".cover",
            Some("fill") => ".fill",
            _ => ".contain",
        };
        let length = |name| self.length(element, name).unwrap_or_else(|| "nil".into());
        // A declared resource by name, or a `MediaSource`'s URL.
        let source = match element.expr("source") {
            Some(expr) if matches!(expr.ty, Type::Optional { .. }) => {
                format!("url: ({})?.url", self.expr(expr))
            }
            Some(expr) => format!("url: ({}).url", self.expr(expr)),
            None => format!(
                "name: {}",
                string(element.resource("source").unwrap_or_default())
            ),
        };
        let hero = element
            .keyword("hero")
            .map(|name| {
                let key = match element.expr("hero_key") {
                    Some(key) => format!("\"{name}-\\({})\"", self.expr(key)),
                    None => string(name),
                };
                format!(
                    ", hero: {key}, heroMotion: {}, heroCorner: {}",
                    crate::style::motion(self.app, "hero"),
                    self.length(element, "corner").unwrap_or_else(|| "0".into())
                )
            })
            .unwrap_or_default();
        out.line(format!(
            "BernImage({source}, label: {}, width: {}, height: {}, fit: {fit}{}, aspect: {}{hero})",
            self.text_prop(element, "label")
                .unwrap_or_else(|| "nil".into()),
            length("width"),
            length("height"),
            if element.keyword("rendering") == Some("template") {
                ", rendering: .template"
            } else {
                ""
            },
            element
                .number("aspect")
                .map(double)
                .unwrap_or_else(|| "nil".into()),
        ));
        if let Some(corner) = self.length(element, "corner") {
            out.line(format!(
                ".clipShape(SwiftUI.RoundedRectangle(cornerRadius: {corner}, style: .continuous))"
            ));
        }
    }

    pub(super) fn svg(&self, out: &mut Out, element: &Element) {
        let value = |name, default: &str| {
            element
                .expr(name)
                .map(|e| self.expr(e))
                .unwrap_or_else(|| default.into())
        };
        out.line(format!(
            "BernSvg(name: {}, label: {}, width: {}, height: {}, fit: {}, playing: {}, speed: Double({}), restart: Double({}), interactive: {})",
            string(element.resource("source").unwrap_or_default()),
            self.text_prop(element, "label").unwrap_or_else(|| "nil".into()),
            self.length(element, "width").unwrap_or_else(|| "0".into()),
            self.length(element, "height").unwrap_or_else(|| "0".into()),
            string(element.keyword("fit").unwrap_or("contain")),
            value("playing", "true"), value("speed", "1"), value("restart", "0"),
            element.keyword("interactive").unwrap_or("true"),
        ));
    }

    /// A `Video`: the shared runtime view, bound to a declaration's own
    /// player or to an application player.
    pub(super) fn video(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self
            .text_prop(element, "label")
            .unwrap_or_else(|| "nil".into());
        let binding = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => {
                let value = self.expr(source);
                if matches!(source.ty, Type::Optional { .. }) {
                    format!(".source({value}?.url)")
                } else {
                    format!(".source({value}.url)")
                }
            }
            (None, Some(Prop::Player { id })) => format!(".player({})", string(id)),
            _ => ".source(nil)".into(),
        };
        let controls = element.keyword("controls") != Some("none");
        let fit = match element.keyword("fit") {
            Some("cover") => ".cover",
            Some("fill") => ".fill",
            _ => ".contain",
        };
        // The picture's own layer is rounded; a change of corner moves with
        // the element's `animate`.
        let corner = self
            .length(element, "corner")
            .map(|corner| {
                let motion = element
                    .keyword("animate")
                    .map(|token| crate::style::motion(self.app, token))
                    .unwrap_or_else(|| "nil".into());
                format!(", corner: {corner}, cornerMotion: {motion}")
            })
            .unwrap_or_default();
        out.line(format!(
            "BernVideo(target: {}, label: {label}, binding: {binding}, controls: {controls}, fit: {fit}{corner})",
            self.target(element, ctx)
        ));
    }

    pub(super) fn progress(&self, out: &mut Out, element: &Element) {
        let label = self.text_prop(element, "label");
        let head = match element.expr("value") {
            Some(value) => {
                let total = double(element.number("total").unwrap_or(1.0));
                format!(
                    "SwiftUI.ProgressView(value: min(max(Double({}), 0), {total}), total: {total})",
                    self.expr(value)
                )
            }
            None => "SwiftUI.ProgressView()".into(),
        };
        match label {
            Some(label) => {
                out.open(format!("{} {{", head.trim_end_matches("()")));
                out.line(format!("SwiftUI.Text(verbatim: {label})"));
                out.close("}");
            }
            None => out.line(head),
        }
        out.line(".bernAccentTint()");
    }

    pub(super) fn toast(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let icon = element
            .icon("icon")
            .map(string)
            .unwrap_or_else(|| "nil".into());
        let key = element
            .expr("key")
            .map(|key| format!("AnyHashable({})", self.expr(key)))
            .unwrap_or_else(|| format!("AnyHashable({content})"));
        out.line(format!(
            "BernToast(text: {content}, systemImage: {icon}, key: {key}, duration: {}, motion: {})",
            double(element.number("duration").unwrap_or(3.0)),
            crate::style::motion(self.app, "bouncy")
        ));
    }
}
