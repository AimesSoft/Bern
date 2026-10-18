//! What elements show: text, labels, pictures, video, progress and
//! toasts.

use super::{Ctx, Generator, Out};
use crate::names::{double, string};
use bern_ir::{Element, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    fn style_of_text(element: &Element) -> &'static str {
        match element.keyword("style") {
            Some("large_title") => "headlineLarge",
            Some("title") => "headlineSmall",
            Some("title2") => "titleLarge",
            Some("title3") => "titleMedium",
            Some("headline") => "titleSmall",
            Some("subheadline") => "bodyMedium",
            Some("callout") => "bodyMedium",
            Some("footnote") => "bodySmall",
            Some("caption") => "labelMedium",
            Some("caption2") => "labelSmall",
            _ => "bodyLarge",
        }
    }

    pub(super) fn text_view(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let color = match element.keyword("emphasis") {
            Some("secondary") => "MaterialTheme.colorScheme.onSurfaceVariant",
            Some("tertiary") => "MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)",
            _ => "Color.Unspecified",
        };
        let style = format!("MaterialTheme.typography.{}", Self::style_of_text(element));
        match element.keyword("mark") {
            Some(role) => out.line(format!(
                "BernMarkedText({content}, {style}, {color}, BernColor.Role({}))",
                string(role)
            )),
            None => out.line(format!("Text({content}, style = {style}, color = {color})")),
        }
    }

    pub(super) fn label_view(&self, out: &mut Out, element: &Element) {
        let title = self.text_prop(element, "title").unwrap_or_default();
        out.line(format!(
            "BernLabel({title}, {}, MaterialTheme.typography.{})",
            Self::icon(element.icon("icon")),
            Self::style_of_text(element)
        ));
    }

    pub(super) fn image(&self, out: &mut Out, element: &Element) {
        let length = |name| self.length(element, name).unwrap_or_else(|| "null".into());
        let fit = match element.keyword("fit") {
            Some("cover") => "ContentScale.Crop",
            Some("fill") => "ContentScale.FillBounds",
            _ => "ContentScale.Fit",
        };
        // A picture the engine gives, read at run time.
        if let Some(source) = element.expr("source") {
            let url = if matches!(source.ty, Type::Optional { .. }) {
                format!("({})?.url", self.expr(source))
            } else {
                format!("({}).url", self.expr(source))
            };
            out.line(format!(
                "BernImageURL({url}, {}, {}, {}, {fit})",
                self.text_prop(element, "label")
                    .unwrap_or_else(|| "null".into()),
                length("width"),
                length("height"),
            ));
            return;
        }
        out.line(format!(
            "BernImage({}, {}, {}, {}, {fit})",
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
            "BernSvg({}, {}, {}, {}, {}, {}, ({}).toDouble(), ({}).toDouble(), {})",
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
            "BernVideo({}, {label}, {binding}, {}, {fit}{})",
            self.target(element, ctx),
            element.keyword("controls") != Some("none"),
            self.length(element, "corner")
                .map(|corner| format!(", corner = {corner}"))
                .unwrap_or_default()
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
