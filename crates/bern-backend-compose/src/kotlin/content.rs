//! What elements show: text, labels, pictures, video, progress and
//! toasts.

use super::{Ctx, Generator, Out};
use crate::names::{double, float, string};
use bern_ir::{Element, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    /// The text style of a `Text` or `Label`: its style's, with the
    /// weight and size `style!` gives it, and its own `font_size` and
    /// `line_height`.
    fn text_style(&self, element: &Element) -> String {
        let style = string(element.keyword("style").unwrap_or("body"));
        let sp = |name: &str| match element.prop(name)? {
            Prop::Number { value } => Some(float(*value)),
            Prop::Expr { expr } => Some(format!("({}).toFloat()", self.expr(expr))),
            _ => None,
        };
        let mut args = vec![style];
        if let Some(size) = sp("font_size") {
            args.push(format!("size = {size}"));
        }
        if let Some(height) = sp("line_height") {
            args.push(format!("lineHeight = {height}"));
        }
        format!("bernTextStyle({})", args.join(", "))
    }

    pub(super) fn text_view(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let color = match element.keyword("emphasis") {
            Some("secondary") => "MaterialTheme.colorScheme.onSurfaceVariant",
            Some("tertiary") => "MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)",
            _ => "Color.Unspecified",
        };
        let style = self.text_style(element);
        let lines = element
            .number("lines")
            .map(|lines| format!(", lines = {}", lines.max(1.0) as i64))
            .unwrap_or_default();
        match element.keyword("mark") {
            Some(role) => out.line(format!(
                "BernMarkedText({content}, {style}, {color}, BernColor.Role({}){lines})",
                string(role)
            )),
            None => out.line(format!("BernText({content}, {style}, {color}{lines})")),
        }
    }

    pub(super) fn label_view(&self, out: &mut Out, element: &Element) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        let size = element
            .number("font_size")
            .map(|size| format!(", fontSize = {}", float(size)))
            .unwrap_or_default();
        let icon_only = if element.keyword("display") == Some("icon") {
            ", iconOnly = true"
        } else {
            ""
        };
        out.line(format!(
            "BernLabel({title}, {}, {}{size}{icon_only})",
            Self::icon(element.icon("icon")),
            self.text_style(element)
        ));
    }

    pub(super) fn image(&self, out: &mut Out, element: &Element) {
        let length = |name| self.length(element, name).unwrap_or_else(|| "null".into());
        let fit = match element.keyword("fit") {
            Some("cover") => "ContentScale.Crop",
            Some("fill") => "ContentScale.FillBounds",
            _ => "ContentScale.Fit",
        };
        // Rounded corners, the picture's alpha in the text color,
        // and a proportion of its own.
        let mut extra = String::new();
        if let Some(corner) = self.length(element, "corner") {
            extra.push_str(&format!(", corner = {corner}"));
        }
        if element.keyword("rendering") == Some("template") {
            extra.push_str(", template = true");
        }
        if let Some(aspect) = element.number("aspect") {
            extra.push_str(&format!(", aspect = {}", float(aspect)));
        }
        // A picture the engine gives, read at run time.
        if let Some(source) = element.expr("source") {
            let url = if matches!(source.ty, Type::Optional { .. }) {
                format!("({})?.url", self.expr(source))
            } else {
                format!("({}).url", self.expr(source))
            };
            out.line(format!(
                "BernImageURL({url}, {}, {}, {}, {fit}{extra})",
                self.text_prop(element, "label")
                    .unwrap_or_else(|| "null".into()),
                length("width"),
                length("height"),
            ));
            return;
        }
        out.line(format!(
            "BernImage({}, {}, {}, {}, {fit}{extra})",
            string(element.resource("source").unwrap_or_default()),
            self.text_prop(element, "label")
                .unwrap_or_else(|| "null".into()),
            length("width"),
            length("height"),
        ));
    }

    pub(super) fn svg(&self, out: &mut Out, element: &Element) {
        let value = |name, default: &str| {
            element
                .expr(name)
                .map(|e| self.expr(e))
                .unwrap_or_else(|| default.into())
        };
        out.line(format!(
            "BernSvg({}, {}, {}, {}, {}, {}, ({}).toDouble(), ({}).toDouble(), {}, {}, {})",
            string(element.resource("source").unwrap_or_default()),
            self.text_prop(element, "label")
                .unwrap_or_else(|| "null".into()),
            self.length(element, "width")
                .unwrap_or_else(|| "0.dp".into()),
            self.length(element, "height")
                .unwrap_or_else(|| "0.dp".into()),
            string(element.keyword("fit").unwrap_or("contain")),
            value("playing", "true"),
            value("speed", "1"),
            value("restart", "0"),
            element.keyword("interactive").unwrap_or("true"),
            self.text_prop(element, "data")
                .unwrap_or_else(|| "\"\"".into()),
            element.keyword("external_links").unwrap_or("false"),
        ));
    }

    pub(super) fn video(&self, out: &mut Out, element: &Element, ctx: &Ctx) {
        let label = self
            .text_prop(element, "label")
            .unwrap_or_else(|| "null".into());
        let binding = match (element.expr("source"), element.prop("player")) {
            (Some(source), _) => {
                let value = self.expr(source);
                if matches!(source.ty, Type::Optional { .. }) {
                    format!("BernVideoBinding.Source(({value})?.url)")
                } else {
                    format!("BernVideoBinding.Source(({value}).url)")
                }
            }
            (None, Some(Prop::Player { id })) => {
                format!("BernVideoBinding.Player({})", string(id))
            }
            _ => "BernVideoBinding.Source(null)".into(),
        };
        let fit = match element.keyword("fit") {
            Some("cover") => "BernVideoFit.Cover",
            Some("fill") => "BernVideoFit.Fill",
            _ => "BernVideoFit.Contain",
        };
        out.line(format!(
            "BernVideo({}, {label}, {binding}, {}, {fit}{}{})",
            self.target(element, ctx),
            element.keyword("controls") != Some("none"),
            self.length(element, "corner")
                .map(|corner| format!(", corner = {corner}"))
                .unwrap_or_default(),
            if element.keyword("sizing") == Some("fill") {
                ", fillsContainer = true"
            } else {
                ""
            },
        ));
    }

    pub(super) fn progress(&self, out: &mut Out, element: &Element) {
        let label = self
            .text_prop(element, "label")
            .unwrap_or_else(|| "null".into());
        let value = element
            .expr("value")
            .map(|value| format!("({}).toDouble()", self.expr(value)))
            .unwrap_or_else(|| "null".into());
        out.line(format!(
            "BernProgress({value}, {}, {label})",
            double(element.number("total").unwrap_or(1.0))
        ));
    }

    pub(super) fn toast(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let key = element
            .expr("key")
            .map(|key| self.expr(key))
            .unwrap_or_else(|| content.clone());
        out.line(format!(
            "BernToast({content}, {}, {key}, {}, {})",
            Self::icon(element.icon("icon")),
            double(element.number("duration").unwrap_or(3.0)),
            crate::style::motion(self.app, "bouncy")
        ));
    }
}
