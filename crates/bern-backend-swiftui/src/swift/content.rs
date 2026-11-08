//! What elements show: text, labels, pictures, video, progress and
//! toasts.

use super::{Ctx, Generator, Out, Platform};
use crate::names::{double, string};
use bern_ir::{Element, Prop};
use bern_protocol::Type;

impl Generator<'_> {
    pub(super) fn text_view(&self, out: &mut Out, element: &Element) {
        let content = self.text_prop(element, "content").unwrap_or_default();
        let ruby = element.expr("ruby").map(|value| self.expr(value));
        if let Some(ruby) = &ruby {
            let style = element.keyword("style").unwrap_or("body");
            let entry = self
                .app
                .style
                .text_weights
                .iter()
                .find(|entry| entry.style == style);
            let size = self
                .length(element, "font_size")
                .or_else(|| entry.and_then(|entry| self.style_size(entry).map(double)))
                .unwrap_or_else(|| "nil".into());
            let weight = entry.map(|entry| entry.weight.as_str()).unwrap_or("regular");
            let bold = element
                .expr("bold")
                .map(|value| self.expr(value))
                .unwrap_or_else(|| "false".into());
            let color = element
                .keyword("color")
                .map(|role| format!("BernColor.role({}).color", string(role)))
                .unwrap_or_else(|| match element.keyword("emphasis") {
                    Some("secondary") => "Color.secondary".into(),
                    Some("tertiary") => "Color.secondary.opacity(0.5)".into(),
                    _ => "Color.primary".into(),
                });
            let lit = self.length(element, "lit").unwrap_or_else(|| "-1".into());
            let path = self.text_prop(element, "lit_path").unwrap_or_else(|| "\"\"".into());
            let clock = self.length(element, "lit_clock").unwrap_or_else(|| "0".into());
            let since = self.length(element, "lit_since").unwrap_or_else(|| "0".into());
            let height = self.length(element, "line_height").unwrap_or_else(|| "nil".into());
            let lines = element.number("lines").map(|lines| (lines.max(1.0) as i64).to_string()).unwrap_or_else(|| "nil".into());
            let mark = element.keyword("mark").map(|role| format!("BernColor.role({}).color", string(role))).unwrap_or_else(|| "nil".into());
            out.open(format!(
                "BernRubyText(text: {content}, ruby: {ruby}, size: {size}, style: {}, weight: {}, bold: {bold}, color: {color}, lineHeight: {height}, lines: {lines}, mark: {mark}, lit: {{ {lit} }}, path: {{ {path} }}, clock: {{ {clock} }}, since: {{ {since} }}) {{",
                string(style), string(weight)
            ));
        }
        if element.keyword("markdown") == Some("true") {
            out.line(format!("SwiftUI.Text(bernMarkdown({content}))"));
        } else if let Some(lit) = self.length(element, "lit") {
            if let Some(path) = self.text_prop(element, "lit_path") {
                let clock = self
                    .length(element, "lit_clock")
                    .unwrap_or_else(|| "0".into());
                let since = self
                    .length(element, "lit_since")
                    .unwrap_or_else(|| "0".into());
                out.line(format!(
                    "BernLitText(text: {content}, lit: {{ {lit} }}, path: {{ {path} }}, clock: {{ {clock} }}, since: {{ {since} }})"
                ));
            } else {
                out.line(format!("bernLit({content}, {lit})"));
                out.line(format!(".bernLitting {{ {lit} }}"));
            }
        } else {
            out.line(format!("SwiftUI.Text(verbatim: {content})"));
        }
        self.font(out, element);
        if let Some(bold) = element.expr("bold") {
            out.line(format!(".bold({})", self.expr(bold)));
        }
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
        } else if element.keyword("markdown") == Some("true") {
            // Writing runs as long as it is: a stack would otherwise cut
            // a paragraph short with an ellipsis.
            out.line(".fixedSize(horizontal: false, vertical: true)");
        }
        match element.keyword("emphasis") {
            Some("secondary") => out.line(".foregroundStyle(.secondary)"),
            Some("tertiary") => out.line(".foregroundStyle(.tertiary)"),
            _ => {}
        }
        if let Some(role) = element.keyword("color") {
            out.line(format!(
                ".foregroundStyle(BernColor.role({}).color)",
                string(role)
            ));
        }
        if let Some(role) = element.keyword("mark") {
            out.line(format!(".bernMark(.role({}))", string(role)));
        }
        if ruby.is_some() {
            out.close("}");
        }
    }

    /// The size the application gives a text style here: on tvOS, a
    /// television, its television size where it gives one.
    fn style_size(&self, entry: &bern_ir::style::TextWeight) -> Option<f64> {
        match self.platform {
            Platform::Tvos => entry.tv_size.or(entry.size),
            _ => entry.size,
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
                    .and_then(|entry| self.style_size(entry).map(double))
            })
            .unwrap_or_else(|| match self.platform {
                Platform::Macos => "13.0".into(),
                Platform::Ios => "17.0".into(),
                // tvOS reads its body at ten feet.
                Platform::Tvos => "29.0".into(),
            })
    }

    fn font(&self, out: &mut Out, element: &Element) {
        // `control`: the font of the control it stands in, as a button's
        // title has it; only a size of its own changes it.
        if element.keyword("style") == Some("control") {
            if let Some(size) = self.length(element, "font_size") {
                out.line(format!(".font(.system(size: {size}))"));
            }
            return;
        }
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
                "code" => "system(.callout, design: .monospaced)",
                _ => "body",
            };
            let design = if style == "code" {
                ", design: .monospaced"
            } else {
                ""
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
                    .or_else(|| self.style_size(entry).map(double))
                {
                    Some(size) => out.line(format!(
                        ".font(.system(size: {}, weight: .{}{design}))",
                        size, entry.weight
                    )),
                    None => out.line(format!(".font(.{font}.weight(.{}))", entry.weight)),
                },
                None => match self.length(element, "font_size") {
                    Some(size) => out.line(format!(".font(.system(size: {size}{design}))")),
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
                "name: {}{}",
                string(element.resource("source").unwrap_or_default()),
                element
                    .resource("dark_source")
                    .map(|dark| format!(", darkName: {}", string(dark)))
                    .unwrap_or_default()
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
        let one_size = if bern_target_api::svg::one_size(element) {
            ", oneSize: true"
        } else {
            ""
        };
        // Without a size it covers the room its container gives.
        let covers = if ["width", "height", "aspect"]
            .iter()
            .all(|name| element.prop(name).is_none())
        {
            ", covers: true"
        } else {
            ""
        };
        out.line(format!(
            "BernImage({source}, label: {}, width: {}, height: {}, fit: {fit}{}, aspect: {}{hero}{one_size}{covers})",
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
            "BernSvg(name: {}, label: {}, width: {}, height: {}, fit: {}, playing: {}, speed: Double({}), restart: Double({}), interactive: {}, data: {}, externalLinks: {})",
            string(element.resource("source").unwrap_or_default()),
            self.text_prop(element, "label").unwrap_or_else(|| "nil".into()),
            self.length(element, "width").unwrap_or_else(|| "0".into()),
            self.length(element, "height").unwrap_or_else(|| "0".into()),
            string(element.keyword("fit").unwrap_or("contain")),
            value("playing", "true"), value("speed", "1"), value("restart", "0"),
            element.keyword("interactive").unwrap_or("true"),
            self.text_prop(element, "data").unwrap_or_else(|| "\"\"".into()),
            element.keyword("external_links").unwrap_or("false"),
        ));
    }

    pub(super) fn web_page(&self, out: &mut Out, element: &Element) {
        let text = |name| {
            self.text_prop(element, name)
                .unwrap_or_else(|| "\"\"".into())
        };
        let close = element
            .action
            .as_ref()
            .map(|action| self.action(action))
            .unwrap_or_default();
        out.line(format!(
            "BernWebPage(url: {}, cookiesFile: {}, cookieDomain: {}, userAgent: {}, closeOn: {}, label: {}) {{ {close} }}",
            text("url"),
            text("cookies_file"),
            text("cookie_domain"),
            text("user_agent"),
            text("close_on"),
            self.text_prop(element, "label")
                .unwrap_or_else(|| "nil".into()),
        ));
    }

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
        let sizing = if element.keyword("sizing") == Some("fill") {
            ", fillsContainer: true"
        } else {
            ""
        };
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
            "BernVideo(target: {}, label: {label}, binding: {binding}, controls: {controls}, fit: {fit}{sizing}{corner})",
            self.target(element, ctx)
        ));
    }

    pub(super) fn progress(&self, out: &mut Out, element: &Element) {
        let label = self.text_prop(element, "label");
        if element.keyword("style") == Some("ring")
            && let Some(value) = element.expr("value")
        {
            out.line(format!(
                "BernProgressRing(value: Double({}), total: {}, width: {}, lineWidth: {}, label: {})",
                self.expr(value),
                double(element.number("total").unwrap_or(1.0)),
                self.length(element, "width").unwrap_or_else(|| "24.0".into()),
                double(element.number("stroke_width").unwrap_or(3.0)),
                label.unwrap_or_else(|| "nil".into())
            ));
            return;
        }
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
