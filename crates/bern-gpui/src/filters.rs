//! `blur` and `glow`, which GPUI does not draw itself.
//!
//! - A picture is blurred on the CPU: placed in its box as `fit` has it,
//!   then a Gaussian (three box passes over premultiplied colors) with
//!   transparency beyond the box, so its edges fade as on the web. Large
//!   radii are worked at a lower resolution, which a blur loses nothing to;
//!   the result is painted within the box and its corners.
//! - Text and icons in a blurred element are drawn as copies spread over a
//!   Gaussian (a spiral of offsets), each faint enough that a stroke comes
//!   out as strong as a real blur of it would.
//! - The light of `glow` is a radial picture centered on the pointer,
//!   painted within the element's box and corners.

use crate::El;
use crate::ctx::Ctx;
use gpui::{
    Bounds, Corners, IntoElement, ParentElement, Pixels, Point, RenderImage, Styled, div, point,
    px, size,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

// MARK: - Gaussian

/// The widths of three box blurs that together approximate a Gaussian of
/// standard deviation `sigma`.
fn boxes(sigma: f32) -> [usize; 3] {
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut lower = ideal.floor() as i64;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let upper = lower + 2;
    let lower_f = lower as f32;
    let m = ((12.0 * sigma * sigma - n * lower_f * lower_f - 4.0 * n * lower_f - 3.0 * n)
        / (-4.0 * lower_f - 4.0))
        .round() as i64;
    std::array::from_fn(|i| {
        if (i as i64) < m {
            lower as usize
        } else {
            upper as usize
        }
    })
}

/// One box pass along rows (`step` 1) or columns, of `width` (the box's
/// full width, odd); what lies beyond the edges is transparent.
fn box_pass(
    from: &[[f32; 4]],
    to: &mut [[f32; 4]],
    lines: usize,
    length: usize,
    along: usize,
    across: usize,
    width: usize,
) {
    let radius = width / 2;
    let scale = 1.0 / width as f32;
    for line in 0..lines {
        let at = |i: usize| line * across + i * along;
        let mut sum = [0.0f32; 4];
        // The window around the first pixel.
        for i in 0..=radius.min(length - 1) {
            let pixel = from[at(i)];
            for k in 0..4 {
                sum[k] += pixel[k];
            }
        }
        for i in 0..length {
            to[at(i)] = sum.map(|channel| channel * scale);
            let enter = i + radius + 1;
            if enter < length {
                let pixel = from[at(enter)];
                for k in 0..4 {
                    sum[k] += pixel[k];
                }
            }
            if i >= radius {
                let pixel = from[at(i - radius)];
                for k in 0..4 {
                    sum[k] -= pixel[k];
                }
            }
        }
    }
}

/// `pixels` (four channels, straight alpha, `width` by `height`) blurred
/// with standard deviation `sigma` pixels.
pub(crate) fn gaussian(pixels: &mut [u8], width: usize, height: usize, sigma: f32) {
    if sigma < 0.2 || width == 0 || height == 0 {
        return;
    }
    // Premultiplied, so transparent pixels lend no color.
    let mut a: Vec<[f32; 4]> = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            let alpha = f32::from(p[3]) / 255.0;
            [
                f32::from(p[0]) * alpha,
                f32::from(p[1]) * alpha,
                f32::from(p[2]) * alpha,
                f32::from(p[3]),
            ]
        })
        .collect();
    let mut b = vec![[0.0f32; 4]; a.len()];
    for width_of_box in boxes(sigma) {
        box_pass(&a, &mut b, height, width, 1, width, width_of_box);
        box_pass(&b, &mut a, width, height, width, 1, width_of_box);
    }
    for (pixel, out) in a.iter().zip(pixels.as_chunks_mut::<4>().0.iter_mut()) {
        let alpha = pixel[3];
        let unmultiply = if alpha > 0.0 { 255.0 / alpha } else { 0.0 };
        out[0] = (pixel[0] * unmultiply).round().clamp(0.0, 255.0) as u8;
        out[1] = (pixel[1] * unmultiply).round().clamp(0.0, 255.0) as u8;
        out[2] = (pixel[2] * unmultiply).round().clamp(0.0, 255.0) as u8;
        out[3] = alpha.round().clamp(0.0, 255.0) as u8;
    }
}

// MARK: - Pictures

/// A picture to blur: from a file or an address, or already decoded.
#[derive(Clone)]
pub(crate) enum Picture {
    Source(gpui::Resource),
    Rendered(Arc<RenderImage>),
}

/// What a blurred picture last painted, by element.
struct Painted {
    key: (usize, u32, u32, u32, u8),
    image: Arc<RenderImage>,
}

thread_local! {
    static PICTURES: RefCell<HashMap<String, Painted>> = RefCell::new(HashMap::new());
}

fn fit_code(fit: crate::content::Fit) -> u8 {
    match fit {
        crate::content::Fit::Contain => 0,
        crate::content::Fit::Cover => 1,
        crate::content::Fit::Fill => 2,
    }
}

/// `picture` placed in a box of `width` by `height` pixels by `fit`
/// (four channels, as the picture keeps them), transparent where it does
/// not reach.
fn placed(
    picture: &RenderImage,
    width: u32,
    height: u32,
    fit: crate::content::Fit,
) -> Option<Vec<u8>> {
    use image::imageops::{FilterType, resize};
    let bytes = picture.as_bytes(0)?;
    let full = picture.size(0);
    let (iw, ih) = (full.width.0.max(1) as u32, full.height.0.max(1) as u32);
    let source = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(iw, ih, bytes)?;
    let (sx, sy) = (width as f32 / iw as f32, height as f32 / ih as f32);
    let mut canvas = image::RgbaImage::new(width, height);
    match fit {
        crate::content::Fit::Fill => {
            canvas = resize(&source, width, height, FilterType::Triangle);
        }
        crate::content::Fit::Cover => {
            // The part of the picture the box shows.
            let scale = sx.max(sy);
            let (cw, ch) = (
                ((width as f32 / scale).round() as u32).clamp(1, iw),
                ((height as f32 / scale).round() as u32).clamp(1, ih),
            );
            let view = image::imageops::crop_imm(&source, (iw - cw) / 2, (ih - ch) / 2, cw, ch);
            canvas = resize(&*view, width, height, FilterType::Triangle);
        }
        crate::content::Fit::Contain => {
            let scale = sx.min(sy);
            let (dw, dh) = (
                ((iw as f32 * scale).round() as u32).clamp(1, width),
                ((ih as f32 * scale).round() as u32).clamp(1, height),
            );
            let shown = resize(&source, dw, dh, FilterType::Triangle);
            image::imageops::replace(
                &mut canvas,
                &shown,
                i64::from((width - dw) / 2),
                i64::from((height - dh) / 2),
            );
        }
    }
    Some(canvas.into_raw())
}

/// A picture blurred with `radius` points, painting the box it is laid
/// out in, cut to `corners`. `key` tells it apart from the other blurred
/// pictures of the window.
pub(crate) fn blurred_picture(
    key: String,
    picture: Picture,
    fit: crate::content::Fit,
    radius: f32,
    corners: [f32; 4],
) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let original = match &picture {
                Picture::Rendered(image) => image.clone(),
                Picture::Source(resource) => {
                    match window.use_asset::<gpui::ImgResourceLoader>(resource, cx) {
                        Some(Ok(image)) => image,
                        // A canvas is not drawn again when it arrives:
                        // look again next frame while it loads.
                        None => {
                            window.request_animation_frame();
                            return;
                        }
                        Some(Err(_)) => return,
                    }
                }
            };
            let scale = window.scale_factor();
            let (bw, bh) = (
                f32::from(bounds.size.width) * scale,
                f32::from(bounds.size.height) * scale,
            );
            if bw < 1.0 || bh < 1.0 {
                return;
            }
            // A wide blur keeps no detail: worked smaller, by as much.
            let sigma = radius * scale;
            let reduce = (sigma / 2.5).floor().max(1.0);
            let largest = 1024.0;
            let reduce = reduce.max((bw.max(bh) / largest).ceil());
            let (w, h) = ((bw / reduce).ceil() as u32, (bh / reduce).ceil() as u32);
            let sigma = sigma / reduce;
            let wanted = (
                original.id.0,
                w,
                h,
                (sigma * 20.0).round() as u32,
                fit_code(fit),
            );
            let held = PICTURES.with(|pictures| {
                pictures
                    .borrow()
                    .get(&key)
                    .filter(|painted| painted.key == wanted)
                    .map(|painted| painted.image.clone())
            });
            let image = match held {
                Some(image) => image,
                None => {
                    let Some(mut pixels) = placed(&original, w, h, fit) else {
                        return;
                    };
                    gaussian(&mut pixels, w as usize, h as usize, sigma);
                    let Some(buffer) = image::RgbaImage::from_raw(w, h, pixels) else {
                        return;
                    };
                    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                        buffer
                    )]));
                    let old = PICTURES.with(|pictures| {
                        pictures.borrow_mut().insert(
                            key.clone(),
                            Painted {
                                key: wanted,
                                image: image.clone(),
                            },
                        )
                    });
                    if let Some(old) = old {
                        let _ = window.drop_image(old.image);
                    }
                    image
                }
            };
            let _ = window.paint_image(bounds, bounds, corners_of(corners), image, 0, false);
        },
    )
    .absolute()
    .inset_0()
}

fn corners_of(corners: [f32; 4]) -> Corners<Pixels> {
    Corners {
        top_left: px(corners[0]),
        top_right: px(corners[1]),
        bottom_right: px(corners[2]),
        bottom_left: px(corners[3]),
    }
}

// MARK: - Text and icons

/// Offsets, in points, spread as a Gaussian of `sigma` points: a spiral
/// with each turn a golden angle on from the last.
fn taps(sigma: f32, scale: f32) -> Vec<Point<Pixels>> {
    let count = (8.0 + sigma * scale * 3.0).round().clamp(8.0, 40.0) as usize;
    let golden = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
    (0..count)
        .map(|i| {
            let u = (i as f32 + 0.5) / count as f32;
            let distance = sigma * (-2.0 * (1.0 - u).ln()).sqrt();
            let angle = i as f32 * golden;
            point(px(distance * angle.cos()), px(distance * angle.sin()))
        })
        .collect()
}

/// The error function (Abramowitz and Stegun 7.1.26).
fn erf(x: f32) -> f32 {
    let sign = x.signum();
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_74) * t
            + 0.254_829_6)
            * t
            * (-x * x).exp();
    sign * y
}

/// How opaque each of `count` copies is, so that the middle of a stroke
/// `stroke` points wide comes out as strong as a Gaussian of `sigma` makes
/// it: copies overlap by the share of them the stroke covers there.
fn tap_alpha(sigma: f32, stroke: f32, count: usize) -> f32 {
    let peak = erf(stroke / (2.0 * std::f32::consts::SQRT_2 * sigma.max(0.01))).clamp(0.01, 0.999);
    let together = 1.0 - (1.0 - peak).powf(1.0 / peak);
    1.0 - (1.0 - together).powf(1.0 / count as f32)
}

/// `content` drawn blurred with `radius` points in `color`, laid out as the
/// text around it is (its font, size and lines), in the box of that text.
pub(crate) fn blurred_text(
    content: String,
    color: gpui::Rgba,
    radius: f32,
    lines: Option<usize>,
) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let line_height = style.line_height_in_pixels(window.rem_size());
            let offsets = taps(radius, window.scale_factor());
            // Strokes about a tenth of the size, a little more when bold.
            let stroke = f32::from(size) * (0.06 + 0.06 * (style.font_weight.0 / 700.0));
            let alpha = tap_alpha(radius, stroke, offsets.len()) * color.a;
            let mut run = style.to_run(content.len());
            run.color = gpui::Rgba { a: alpha, ..color }.into();
            let wrap = (lines != Some(1)).then_some(bounds.size.width);
            let Ok(shaped) =
                window
                    .text_system()
                    .shape_text(content.clone().into(), size, &[run], wrap, lines)
            else {
                return;
            };
            for offset in offsets {
                let mut origin = bounds.origin + offset;
                for line in shaped.iter() {
                    let _ = line.paint(
                        origin,
                        line_height,
                        gpui::TextAlign::Left,
                        Some(bounds),
                        window,
                        cx,
                    );
                    origin.y += line.size(line_height).height;
                }
            }
        },
    )
    .absolute()
    .inset_0()
}

thread_local! {
    /// The blur of the element being built, for icons, which are built
    /// without a context.
    static BUILDING: Cell<f32> = const { Cell::new(0.0) };
}

/// Build `body` blurred with `radius` points: in `c.blurred(radius)`, and
/// with the icons built meanwhile blurred as much.
pub fn blur_scope(c: &Ctx, radius: f32, body: impl FnOnce(&Ctx) -> El) -> El {
    let c = c.blurred(radius);
    let before = BUILDING.with(|building| building.replace(c.blur()));
    let element = body(&c);
    BUILDING.with(|building| building.set(before));
    element
}

/// The blur icons built now take.
pub(crate) fn building_blur() -> f32 {
    BUILDING.with(Cell::get)
}

/// An icon blurred with `radius` points: copies of it spread as text is.
pub(crate) fn blurred_icon(name: &str, side: f32, color: gpui::Rgba, radius: f32) -> El {
    let offsets = taps(radius, 2.0);
    let alpha = tap_alpha(radius, side * 0.09, offsets.len()) * color.a;
    let path = gpui::SharedString::from(format!("icons/{name}.svg"));
    div()
        .relative()
        .flex_none()
        .size(px(side))
        .children(offsets.into_iter().map(|offset| {
            gpui::svg()
                .path(path.clone())
                .absolute()
                .left(offset.x)
                .top(offset.y)
                .size(px(side))
                .text_color(gpui::Rgba { a: alpha, ..color })
        }))
        .into_any_element()
}

// MARK: - Glow

/// How lit an element is, fading in and out over 0.18 s, and where the
/// pointer last was over it.
#[derive(Default)]
struct Light {
    level: f32,
    at: Option<Point<Pixels>>,
    inside: bool,
    time: Option<std::time::Instant>,
}

thread_local! {
    static LIGHTS: RefCell<HashMap<String, Light>> = RefCell::new(HashMap::new());
    /// The light's picture, white, by how opaque its middle is (64ths).
    static SPOTS: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

const FADE: f32 = 0.18;
const SPOT: u32 = 128;

/// A white spot, `peak` opaque in the middle, a fifth of that 55 % of the
/// way out and nothing at the edge.
fn spot(peak: u32) -> Arc<RenderImage> {
    if let Some(found) = SPOTS.with(|spots| spots.borrow().get(&peak).cloned()) {
        return found;
    }
    let middle = peak as f32 / 64.0;
    let mut buffer = image::RgbaImage::new(SPOT, SPOT);
    let half = SPOT as f32 / 2.0;
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        let dx = (x as f32 + 0.5 - half) / half;
        let dy = (y as f32 + 0.5 - half) / half;
        let r = (dx * dx + dy * dy).sqrt();
        let share = if r <= 0.55 {
            1.0 - 0.8 * (r / 0.55)
        } else if r < 1.0 {
            0.2 * (1.0 - (r - 0.55) / 0.45)
        } else {
            0.0
        };
        let alpha = (share * middle * 255.0).round().clamp(0.0, 255.0) as u8;
        *pixel = image::Rgba([255, 255, 255, alpha]);
    }
    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        buffer
    )]));
    SPOTS.with(|spots| spots.borrow_mut().insert(peak, image.clone()));
    image
}

/// `glow`: the element `build` makes, lit by a white light that follows
/// the pointer while it is over the element (over what lies on it too),
/// `factor` times the element's shorter side in radius and `opacity` at
/// its middle, within the element's box and `corner`s.
pub fn glow(
    c: &Ctx,
    id: &str,
    build: &dyn Fn(&Ctx) -> El,
    factor: f32,
    opacity: f32,
    corner: Option<f32>,
) -> El {
    let key = format!("{}/{}#glow", c.win().id, c.key(id));
    let grow = c.grow();
    let element = build(&c.grown(crate::Grow {
        flex: false,
        width: grow.width || grow.flex,
        height: grow.height || grow.flex,
        keep: false,
    }));
    let corners = match corner {
        Some(corner) => [corner; 4],
        None => c.round(),
    };
    let light = gpui::canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let pointer = window.mouse_position();
            let inside = bounds.contains(&pointer);
            let now = std::time::Instant::now();
            let (level, at) = LIGHTS.with(|lights| {
                let mut lights = lights.borrow_mut();
                let light = lights.entry(key.clone()).or_default();
                let elapsed = light
                    .time
                    .map_or(0.0, |time| now.duration_since(time).as_secs_f32());
                light.time = Some(now);
                let step = elapsed / FADE;
                light.level = if inside {
                    (light.level + step).min(1.0)
                } else {
                    (light.level - step).max(0.0)
                };
                if inside {
                    light.at = Some(pointer);
                }
                light.inside = inside;
                (light.level, light.at)
            });
            if (inside && level < 1.0) || (!inside && level > 0.0) {
                window.request_animation_frame();
            }
            // Moving over it moves the light; leaving it fades it out.
            let watched = key.clone();
            window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, window, _| {
                if phase != gpui::DispatchPhase::Bubble {
                    return;
                }
                let was = LIGHTS.with(|lights| {
                    lights
                        .borrow()
                        .get(&watched)
                        .is_some_and(|light| light.inside)
                });
                if was || bounds.contains(&event.position) {
                    window.refresh();
                }
            });
            let Some(at) = at.filter(|_| level > 0.0) else {
                return;
            };
            let radius = f32::from(bounds.size.width.min(bounds.size.height)) * factor;
            if radius <= 0.0 {
                return;
            }
            let peak = (opacity * level * 64.0).round().clamp(0.0, 64.0) as u32;
            if peak == 0 {
                return;
            }
            let area = Bounds::new(
                point(at.x - px(radius), at.y - px(radius)),
                size(px(radius * 2.0), px(radius * 2.0)),
            );
            let _ = window.paint_image(bounds, area, corners_of(corners), spot(peak), 0, false);
        },
    )
    .absolute()
    .inset_0();
    let wrapper = div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .child(element)
        .child(light);
    c.grow().apply(wrapper).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_boxes_approach_the_deviation() {
        for sigma in [1.0f32, 2.5, 6.0, 20.0] {
            let widths = boxes(sigma);
            // Each box of width w has variance (w² - 1) / 12.
            let variance: f32 = widths.iter().map(|&w| ((w * w) as f32 - 1.0) / 12.0).sum();
            assert!((variance.sqrt() - sigma).abs() < 0.6, "{sigma}: {widths:?}");
        }
    }

    #[test]
    fn a_blur_keeps_color_and_fades_the_edges() {
        let (w, h) = (40usize, 40usize);
        let mut pixels = vec![0u8; w * h * 4];
        for pixel in pixels.as_chunks_mut::<4>().0.iter_mut() {
            pixel.copy_from_slice(&[200, 100, 50, 255]);
        }
        gaussian(&mut pixels, w, h, 3.0);
        let at = |x: usize, y: usize| &pixels[(y * w + x) * 4..(y * w + x) * 4 + 4];
        // The middle stays as it was; the corner fades, keeping its color.
        assert_eq!(at(20, 20), &[200, 100, 50, 255]);
        assert!(at(0, 0)[3] < 120, "{:?}", at(0, 0));
        assert_eq!(&at(0, 0)[..3], &[200, 100, 50]);
    }

    #[test]
    fn copies_spread_as_far_as_the_radius_says() {
        let offsets = taps(4.0, 2.0);
        let mean_square: f32 = offsets
            .iter()
            .map(|offset| f32::from(offset.x).powi(2) + f32::from(offset.y).powi(2))
            .sum::<f32>()
            / offsets.len() as f32;
        // Two dimensions: twice the variance.
        assert!((mean_square / 2.0).sqrt() > 3.0 && (mean_square / 2.0).sqrt() < 5.0);
        // A thin stroke under a wide blur: faint copies; under none, solid.
        assert!(tap_alpha(6.0, 1.5, 24) < 0.05);
        assert!(tap_alpha(0.1, 2.0, 24) > 0.15);
    }
}
