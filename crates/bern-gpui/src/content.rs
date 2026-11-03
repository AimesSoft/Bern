//! What content shows: text in the type scale, labels with icons, pictures
//! of declared resources and progress.

use crate::El;
use crate::ctx::Ctx;
use crate::units::px;
use gpui::{
    FontWeight, IntoElement, ObjectFit, ParentElement, PathBuilder, Styled, StyledImage, canvas,
    div, img, point, relative,
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
    /// The color role of a mark under each line (`mark`).
    pub mark: Option<&'static str>,
    /// Bold while its condition holds (`bold`).
    pub bold: bool,
}

/// `Text`.
pub fn text(c: &Ctx, content: String, props: TextProps) -> El {
    let (size, line, weight) = props.style.metrics();
    let weight = if props.bold {
        gpui::FontWeight::BOLD
    } else {
        weight
    };
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
    // Never wider than its container: narrower than its single line, it
    // wraps, or with a number of lines ends in an ellipsis.
    let element = element.min_w(px(0.0)).max_w(gpui::relative(1.0));
    let element = match props.lines {
        Some(1) => element.truncate(),
        Some(lines) => element.line_clamp(lines),
        None => element,
    };
    let element = c.grow().apply(element);
    // Blurred, the text keeps its place but is drawn as spread copies.
    let blur = c.blur();
    if blur > 0.05 {
        let color = match (props.color, props.emphasis) {
            (Some(role), _) => c.color(role),
            (None, Emphasis::Primary) => c.foreground(),
            (None, emphasis) => c.color(emphasis.role()),
        };
        return element
            .relative()
            .text_color(gpui::transparent_black())
            .child(content.clone())
            .child(crate::filters::blurred_text(
                content,
                color,
                blur,
                props.lines,
            ))
            .into_any_element();
    }
    // A mark is drawn behind the text, taking no room.
    let element = match props.mark {
        Some(role) => {
            element
                .relative()
                .child(mark(content.clone(), c.color(role), px(line), props.lines))
        }
        None => element,
    };
    if !props.markdown {
        return element.child(content).into_any_element();
    }
    element.child(markdown(c, &content)).into_any_element()
}

/// `Text(mark:)`: a capsule of `color` under each line of `content`, from
/// 74 % to 98 % of the line's height and 0.4 em longer than the line, laid
/// out as the text is, in its font, at the width it is given.
fn mark(content: String, color: gpui::Rgba, line: gpui::Pixels, lines: Option<usize>) -> El {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let run = style.to_run(content.len());
            let wrap = (lines != Some(1)).then_some(bounds.size.width);
            let Ok(shaped) =
                window
                    .text_system()
                    .shape_text(content.clone().into(), size, &[run], wrap, lines)
            else {
                return;
            };
            let _ = cx;
            let mut row = 0usize;
            for paragraph in shaped.iter() {
                let layout = &paragraph.unwrapped_layout;
                let mut start = 0usize;
                let mut ends: Vec<usize> = paragraph
                    .wrap_boundaries
                    .iter()
                    .map(|boundary| layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index)
                    .collect();
                ends.push(paragraph.text.len());
                for end in ends {
                    let left = layout.x_for_index(start);
                    let width = (layout.x_for_index(end) - left).min(bounds.size.width);
                    start = end;
                    if lines.is_some_and(|lines| row >= lines) {
                        return;
                    }
                    if width > gpui::px(0.0) {
                        let band = line * 0.24;
                        let origin = gpui::point(
                            bounds.origin.x + left - layout.x_for_index(0),
                            bounds.origin.y + line * row as f32 + line * 0.74,
                        );
                        window.paint_quad(
                            gpui::fill(
                                gpui::Bounds::new(origin, gpui::size(width + size * 0.4, band)),
                                color,
                            )
                            .corner_radii(band / 2.0),
                        );
                    }
                    row += 1;
                }
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
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
    covers: bool,
) -> El {
    let name = match dark {
        Some(dark) if c.theme().dark => dark,
        _ => resource,
    };
    let path = std::path::PathBuf::from(crate::host::resources_dir()).join(name);
    // A turning picture draws itself, turned.
    if let Some((element, spin)) = spin {
        let template = template.then(|| c.color("text.primary"));
        let picture = crate::spin::picture(
            c, element, &path, template, width, height, corner, fit, aspect, spin,
        );
        return if covers {
            div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .child(picture)
                .into_any_element()
        } else {
            picture
        };
    }
    // A template's shape in the text color.
    let (source, picture): (gpui::ImageSource, crate::filters::Picture) = match template
        .then(|| tinted(&path, c.color("text.primary")))
        .flatten()
    {
        Some(rendered) => (
            rendered.clone().into(),
            crate::filters::Picture::Rendered(rendered),
        ),
        None => (
            path.clone().into(),
            crate::filters::Picture::Source(gpui::Resource::Path(path.clone().into())),
        ),
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
        None => crate::look::round_corners(element, c.round()),
    };
    // With one side given, the other follows the shape; with neither, it
    // is as wide as its container.
    let element = match aspect {
        Some(aspect) if width.is_none() && height.is_none() => {
            element.aspect_ratio(aspect).w_full().min_w(px(0.0))
        }
        Some(aspect) => element.aspect_ratio(aspect),
        None => element,
    };
    framed(
        c,
        element,
        covers,
        Blurred {
            key: resource,
            picture,
            fit,
            corner,
        },
    )
}

/// What a picture is drawn from when it is blurred.
struct Blurred<'a> {
    key: &'a str,
    picture: crate::filters::Picture,
    fit: Fit,
    corner: Option<f32>,
}

/// A picture placed: at its own size, or covering the room its container
/// gives without asking for any, as a layer of a `Stack` under the rest.
/// Blurred, it keeps that place and draws itself blurred there.
fn framed(c: &Ctx, element: gpui::Img, covers: bool, blurred: Blurred<'_>) -> El {
    let blur = c.blur();
    if blur > 0.05 {
        let corners = match blurred.corner {
            Some(corner) => [corner; 4],
            None => c.round(),
        };
        let key = format!("{}/{}", c.key(blurred.key), blurred.key);
        let painted =
            crate::filters::blurred_picture(key, blurred.picture, blurred.fit, blur, corners);
        let element = element.opacity(0.0);
        return if covers {
            div()
                .absolute()
                .inset_0()
                .child(element.size_full())
                .child(painted)
                .into_any_element()
        } else {
            let filled = c.grow();
            let element = crate::Grow {
                flex: false,
                width: filled.width || filled.flex,
                height: filled.height || filled.flex,
            }
            .apply(element.flex_none());
            filled
                .apply(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .child(element)
                        .child(painted),
                )
                .into_any_element()
        };
    }
    if covers {
        element.absolute().inset_0().size_full().into_any_element()
    } else {
        c.grow().apply(element.flex_none()).into_any_element()
    }
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
    covers: bool,
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
    let key = url.clone();
    let (source, picture): (gpui::ImageSource, crate::filters::Picture) = match (tinted, file) {
        (Some(rendered), _) => (
            rendered.clone().into(),
            crate::filters::Picture::Rendered(rendered),
        ),
        (None, Some(path)) => (
            path.clone().into(),
            crate::filters::Picture::Source(gpui::Resource::Path(path.into())),
        ),
        (None, None) => (
            url.clone().into(),
            crate::filters::Picture::Source(gpui::Resource::Uri(url.clone().into())),
        ),
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
        None => crate::look::round_corners(element, c.round()),
    };
    // With one side given, the other follows the shape; with neither, it
    // is as wide as its container.
    let element = match aspect {
        Some(aspect) if width.is_none() && height.is_none() => {
            element.aspect_ratio(aspect).w_full().min_w(px(0.0))
        }
        Some(aspect) => element.aspect_ratio(aspect),
        None => element,
    };
    framed(
        c,
        element,
        covers,
        Blurred {
            key: &key,
            picture,
            fit,
            corner,
        },
    )
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

/// `Progress(style: ring)`: `value` of `total` as an arc in the accent from
/// the top, clockwise, with round ends, `width` across and `stroke` thick;
/// at nothing yet, a dot at the top. It lies over what it rings and takes
/// no input. The round ends are discs: the path's stroke has flat ends, and
/// an arc cannot close on itself, so a whole ring stops just short.
pub fn progress_ring(c: &Ctx, value: f64, total: f64, width: f32, stroke: f32) -> El {
    let fraction = if total > 0.0 { (value / total).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let color = c.color("accent");
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let shown = fraction.clamp(0.0001, 0.9999);
            let center = bounds.center();
            let radius = (bounds.size.width.min(bounds.size.height) - px(stroke)) / 2.0;
            let angle = shown * std::f32::consts::TAU;
            let start = point(center.x, center.y - radius);
            let end = point(center.x + radius * angle.sin(), center.y - radius * angle.cos());
            let mut arc = PathBuilder::stroke(px(stroke));
            arc.move_to(start);
            arc.arc_to(point(radius, radius), px(0.0), shown > 0.5, true, end);
            if let Ok(path) = arc.build() {
                window.paint_path(path, color);
            }
            let cap = px(stroke / 2.0);
            for at in [start, end] {
                let mut disc = PathBuilder::fill();
                disc.move_to(point(at.x - cap, at.y));
                disc.arc_to(point(cap, cap), px(0.0), false, true, point(at.x + cap, at.y));
                disc.arc_to(point(cap, cap), px(0.0), false, true, point(at.x - cap, at.y));
                disc.close();
                if let Ok(path) = disc.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .flex_none()
    .size(px(width))
    .into_any_element()
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
