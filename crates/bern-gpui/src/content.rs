//! What content shows: text in the type scale, labels with icons, pictures
//! of declared resources and progress.

use crate::El;
use crate::ctx::Ctx;
use crate::units::px;
use gpui::{
    FontWeight, IntoElement, ObjectFit, ParentElement, Styled, StyledImage, div, img, relative,
};

/// `Text.style`, after the Web specification's type scale.
#[derive(Clone, Copy, Debug, Default)]
pub enum TextStyle {
    LargeTitle,
    Title,
    Title2,
    Title3,
    Headline,
    Subheadline,
    #[default]
    Body,
    Callout,
    Footnote,
    Caption,
    Caption2,
    /// A button title's font.
    Control,
    /// Monospaced, a little smaller than body.
    Code,
}

impl TextStyle {
    fn name(self) -> &'static str {
        match self {
            TextStyle::LargeTitle => "large_title",
            TextStyle::Title => "title",
            TextStyle::Title2 => "title2",
            TextStyle::Title3 => "title3",
            TextStyle::Headline => "headline",
            TextStyle::Subheadline => "subheadline",
            TextStyle::Body => "body",
            TextStyle::Callout => "callout",
            TextStyle::Footnote => "footnote",
            TextStyle::Caption => "caption",
            TextStyle::Caption2 => "caption2",
            TextStyle::Control => "control",
            TextStyle::Code => "code",
        }
    }

    /// Size and line height in points, and weight: the type scale's, or
    /// the application's own for the style.
    pub(crate) fn metrics(self) -> (f32, f32, FontWeight) {
        let (size, line, weight) = self.scale();
        let own = crate::host::host()
            .def
            .text_weights
            .iter()
            .find(|(style, _, _)| *style == self.name())
            .map(|(_, weight, size)| (*weight, *size));
        match own {
            Some((own, own_size)) => {
                let size_now = own_size.unwrap_or(size);
                (
                    size_now,
                    (line * size_now / size).round(),
                    FontWeight(f32::from(own)),
                )
            }
            None => (size, line, weight),
        }
    }

    fn scale(self) -> (f32, f32, FontWeight) {
        match self {
            TextStyle::LargeTitle => (28.0, 34.0, FontWeight::BOLD),
            TextStyle::Title => (24.0, 32.0, FontWeight::SEMIBOLD),
            TextStyle::Title2 => (20.0, 28.0, FontWeight::SEMIBOLD),
            TextStyle::Title3 => (17.0, 24.0, FontWeight::SEMIBOLD),
            TextStyle::Headline => (14.0, 20.0, FontWeight::SEMIBOLD),
            TextStyle::Subheadline => (13.0, 18.0, FontWeight::NORMAL),
            TextStyle::Body => (14.0, 20.0, FontWeight::NORMAL),
            TextStyle::Callout => (13.0, 20.0, FontWeight::NORMAL),
            TextStyle::Footnote => (12.0, 16.0, FontWeight::NORMAL),
            TextStyle::Caption => (12.0, 16.0, FontWeight::MEDIUM),
            TextStyle::Caption2 => (11.0, 16.0, FontWeight::MEDIUM),
            TextStyle::Control => (13.5, 20.0, FontWeight::MEDIUM),
            TextStyle::Code => (13.0, 20.0, FontWeight::NORMAL),
        }
    }
}

/// The platform's monospaced font, for `style: code`.
const MONOSPACE: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Cascadia Mono"
} else {
    "monospace"
};

/// `Text.emphasis`.
#[derive(Clone, Copy, Debug, Default)]
pub enum Emphasis {
    #[default]
    Primary,
    Secondary,
    Tertiary,
}

impl Emphasis {
    fn role(self) -> &'static str {
        match self {
            Emphasis::Primary => "text.primary",
            Emphasis::Secondary => "text.secondary",
            Emphasis::Tertiary => "text.placeholder",
        }
    }
}

/// How a `Text` shows.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextProps {
    pub style: TextStyle,
    pub emphasis: Emphasis,
    /// At most this many lines, the last ending in an ellipsis.
    pub lines: Option<usize>,
    /// A size in points in place of the style's.
    pub size: Option<f32>,
    /// A theme role in place of the emphasis, as `Text(color:)` names it.
    pub color: Option<&'static str>,
    /// The height of each line in points.
    pub line_height: Option<f32>,
    /// The content is inline Markdown.
    pub markdown: bool,
}

/// `Text`.
pub fn text(c: &Ctx, content: String, props: TextProps) -> El {
    let (size, line, weight) = props.style.metrics();
    let (size, line) = match props.size {
        Some(custom) => (custom, (custom * line / size).round()),
        None => (size, line),
    };
    let line = props.line_height.unwrap_or(line);
    let element = match props.style {
        TextStyle::Code => div().font_family(MONOSPACE),
        _ => div(),
    };
    let element = element
        .text_size(px(size))
        .line_height(px(line))
        .font_weight(weight)
        .text_color(match (props.color, props.emphasis) {
            (Some(role), _) => c.color(role),
            (None, Emphasis::Primary) => c.foreground(),
            (None, emphasis) => c.color(emphasis.role()),
        });
    let element = match props.lines {
        Some(1) => element.min_w(px(0.0)).truncate(),
        Some(lines) => element.min_w(px(0.0)).line_clamp(lines),
        // Never wider than its container: narrower than its single line,
        // it wraps.
        None => element.min_w(px(0.0)).max_w(gpui::relative(1.0)),
    };
    let element = c.grow().apply(element);
    if !props.markdown {
        return element.child(content).into_any_element();
    }
    element.child(markdown(c, &content)).into_any_element()
}

/// Inline Markdown as styled text; links open in the browser.
fn markdown(c: &Ctx, source: &str) -> gpui::AnyElement {
    use crate::markdown::{Mark, parse};
    use gpui::{
        FontStyle, HighlightStyle, InteractiveText, StrikethroughStyle, StyledText, UnderlineStyle,
    };
    let inline = parse(source);
    let accent: gpui::Hsla = c.color("accent.text").into();
    let highlights: Vec<_> = inline
        .marks
        .iter()
        .map(|(range, mark)| {
            let style = match mark {
                Mark::Strong => HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
                Mark::Emphasis => HighlightStyle {
                    font_style: Some(FontStyle::Italic),
                    ..Default::default()
                },
                Mark::Code => HighlightStyle {
                    background_color: Some(c.color("surface.panel").into()),
                    ..Default::default()
                },
                Mark::Struck => HighlightStyle {
                    strikethrough: Some(StrikethroughStyle {
                        thickness: px(1.0),
                        color: None,
                    }),
                    ..Default::default()
                },
                Mark::Link => HighlightStyle {
                    color: Some(accent),
                    underline: Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(accent),
                        wavy: false,
                    }),
                    ..Default::default()
                },
            };
            (range.clone(), style)
        })
        .collect();
    let text = StyledText::new(inline.text).with_highlights(highlights);
    if inline.links.is_empty() {
        return text.into_any_element();
    }
    let ranges: Vec<_> = inline
        .links
        .iter()
        .map(|(range, _)| range.clone())
        .collect();
    let addresses: Vec<String> = inline
        .links
        .into_iter()
        .map(|(_, address)| address)
        .collect();
    InteractiveText::new(
        gpui::ElementId::Name(format!("md-{source:.32}").into()),
        text,
    )
    .on_click(ranges, move |index, _, cx| {
        if let Some(address) = addresses.get(index) {
            cx.open_url(address);
        }
    })
    .into_any_element()
}

/// `Label`: an icon before a title, in a style of the type scale; with
/// `icon_only`, the icon alone, the title its accessible name.
pub fn label(
    c: &Ctx,
    title: String,
    icon: Option<&'static str>,
    style: TextStyle,
    font_size: Option<f32>,
    icon_only: bool,
) -> El {
    let (size, line, weight) = style.metrics();
    let (size, line) = match font_size {
        Some(custom) => (custom, (custom * line / size).round()),
        None => (size, line),
    };
    let color = c.foreground();
    if icon_only {
        return match icon {
            Some(icon) => crate::icons::icon(icon, size + 2.0, color),
            None => div().into_any_element(),
        };
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(px(size))
        .line_height(px(line))
        .font_weight(weight)
        .text_color(color)
        .children(icon.map(|icon| crate::icons::icon(icon, size + 2.0, color)))
        .child(title)
        .into_any_element()
}

/// `Image` of a declared resource; `dark` is the picture for the dark
/// appearance, where the build drew one.
#[allow(clippy::too_many_arguments)]
pub fn image(
    c: &Ctx,
    resource: &'static str,
    dark: Option<&'static str>,
    width: Option<f32>,
    height: Option<f32>,
    corner: Option<f32>,
    fit: Fit,
    template: bool,
    aspect: Option<f32>,
    spin: Option<(&str, crate::spin::Spin)>,
) -> El {
    let name = match dark {
        Some(dark) if c.theme().dark => dark,
        _ => resource,
    };
    let path = std::path::PathBuf::from(crate::host::resources_dir()).join(name);
    // A turning picture draws itself, turned.
    if let Some((element, spin)) = spin {
        let template = template.then(|| c.color("text.primary"));
        return crate::spin::picture(
            c, element, &path, template, width, height, corner, fit, aspect, spin,
        );
    }
    // A template's shape in the text color.
    let source: gpui::ImageSource = match template
        .then(|| tinted(&path, c.color("text.primary")))
        .flatten()
    {
        Some(rendered) => rendered.into(),
        None => path.into(),
    };
    let element = img(source).object_fit(match fit {
        Fit::Contain => ObjectFit::Contain,
        Fit::Cover => ObjectFit::Cover,
        Fit::Fill => ObjectFit::Fill,
    });
    let element = match width {
        Some(width) => element.w(px(width)),
        None => element,
    };
    let element = match height {
        Some(height) => element.h(px(height)),
        None => element,
    };
    let element = match corner {
        Some(corner) => element.rounded(px(corner)),
        None => element,
    };
    // With one side given, the other follows the shape.
    let element = match aspect {
        Some(aspect) => element.aspect_ratio(aspect),
        None => element,
    };
    element.flex_none().into_any_element()
}

thread_local! {
    /// Template pictures recolored, by path and color.
    static TINTED: std::cell::RefCell<std::collections::HashMap<(std::path::PathBuf, u32), std::sync::Arc<gpui::RenderImage>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// A picture's alpha in one color, as a template image draws.
fn tinted(path: &std::path::Path, color: gpui::Rgba) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let key = (
        path.to_path_buf(),
        u32::from_be_bytes([
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            (color.a * 255.0) as u8,
        ]),
    );
    if let Some(found) = TINTED.with(|tinted| tinted.borrow().get(&key).cloned()) {
        return Some(found);
    }
    let mut picture = image::open(path).ok()?.into_rgba8();
    let [r, g, b] = [color.r, color.g, color.b].map(|channel| (channel * 255.0) as u8);
    for pixel in picture.pixels_mut() {
        // BGRA, as GPUI keeps pictures.
        let alpha = (f32::from(pixel[3]) * color.a) as u8;
        *pixel = image::Rgba([b, g, r, alpha]);
    }
    let frame = image::Frame::new(picture);
    let rendered = std::sync::Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]));
    TINTED.with(|tinted| tinted.borrow_mut().insert(key, rendered.clone()));
    Some(rendered)
}

/// `Image` of a picture from state: a file the user chose or an http(s)
/// address; nothing while there is none.
#[allow(clippy::too_many_arguments)]
pub fn image_url(
    c: &Ctx,
    url: String,
    width: Option<f32>,
    height: Option<f32>,
    corner: Option<f32>,
    fit: Fit,
    template: bool,
    aspect: Option<f32>,
) -> El {
    let sized = |element: gpui::Div| {
        let element = match width {
            Some(width) => element.w(px(width)),
            None => element,
        };
        match height {
            Some(height) => element.h(px(height)),
            None => element,
        }
    };
    if url.is_empty() {
        return sized(div()).flex_none().into_any_element();
    }
    let file = url::Url::parse(&url)
        .ok()
        .filter(|parsed| parsed.scheme() == "file")
        .and_then(|parsed| parsed.to_file_path().ok());
    let tinted = file
        .as_ref()
        .filter(|_| template)
        .and_then(|path| tinted(path, c.color("text.primary")));
    let source: gpui::ImageSource = match (tinted, file) {
        (Some(rendered), _) => rendered.into(),
        (None, Some(path)) => path.into(),
        (None, None) => url.into(),
    };
    let element = img(source).object_fit(match fit {
        Fit::Contain => ObjectFit::Contain,
        Fit::Cover => ObjectFit::Cover,
        Fit::Fill => ObjectFit::Fill,
    });
    let element = match width {
        Some(width) => element.w(px(width)),
        None => element,
    };
    let element = match height {
        Some(height) => element.h(px(height)),
        None => element,
    };
    let element = match corner {
        Some(corner) => element.rounded(px(corner)),
        None => element,
    };
    // With one side given, the other follows the shape.
    let element = match aspect {
        Some(aspect) => element.aspect_ratio(aspect),
        None => element,
    };
    element.flex_none().into_any_element()
}

/// How a picture fills its frame (`Image.fit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    /// All of it, letterboxed.
    #[default]
    Contain,
    /// The frame, cropped.
    Cover,
    /// The frame, stretched.
    Fill,
}

/// `Progress`: a bar filled by `value` of `total`, or an indeterminate one
/// without; the label above it.
pub fn progress(c: &Ctx, value: Option<(f64, f64)>, label: Option<String>) -> El {
    let fraction = value
        .map(|(value, total)| {
            if total > 0.0 {
                (value / total).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .unwrap_or(0.3);
    let track = div()
        .w_full()
        .h(px(4.0))
        .rounded(px(2.0))
        .bg(c.color("separator"))
        .child(
            div()
                .h_full()
                .w(relative(fraction as f32))
                .rounded(px(2.0))
                .bg(c.color("accent")),
        );
    let (size, line, _) = TextStyle::Caption.metrics();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .w_full()
        .children(label.map(|label| {
            div()
                .text_size(px(size))
                .line_height(px(line))
                .text_color(c.color("text.secondary"))
                .child(label)
        }))
        .child(track)
        .into_any_element()
}
