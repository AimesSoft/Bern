//! `spin` on pictures. GPUI draws pictures without a transform but shapes
//! in one color with one, so a picture whose visible pixels share a color,
//! or a template, turns on the GPU as that shape. Any other picture is
//! turned on a worker thread a frame at a time and drawn as a picture,
//! the latest frame showing meanwhile.

use crate::El;
use crate::content::Fit;
use crate::ctx::Ctx;
use crate::units::px;
use gpui::{
    App, Bounds, IntoElement, ParentElement, Pixels, RenderImage, Styled, TransformationMatrix,
    Window, canvas, div, point, radians, size,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Instant;

/// `spin` and `spin_period`: whether the picture turns, and the seconds
/// of a turn, counterclockwise below zero.
#[derive(Clone, Copy, Debug)]
pub struct Spin {
    pub on: bool,
    pub period: f32,
}

/// A picture read for turning.
struct Picture {
    width: u32,
    height: u32,
    /// The file, for the shape's SVG.
    bytes: Vec<u8>,
    /// The color its visible pixels share, if they share one.
    color: Option<gpui::Rgba>,
    pixels: Arc<image::RgbaImage>,
}

/// Where one turning picture stands.
#[derive(Default)]
struct Spinner {
    /// How far it is turned, in turns.
    turn: f64,
    /// When the window last drew it turning.
    last: Option<Instant>,
    /// The shape's SVG, and the key GPUI keeps its mask by.
    shape: Option<(String, Arc<[u8]>)>,
    /// The latest frame turned on the CPU, its turn and the size of the
    /// frame it was fitted into.
    frame: Option<(f64, (u32, u32), Arc<RenderImage>)>,
    /// A frame is being turned.
    pending: bool,
}

thread_local! {
    static PICTURES: RefCell<HashMap<PathBuf, Option<Rc<Picture>>>> = RefCell::new(HashMap::new());
    static SPINNERS: RefCell<HashMap<String, Spinner>> = RefCell::new(HashMap::new());
}

/// An `Image` of the picture at `path` that turns about its center while
/// `spin.on` holds; `template` paints its alpha in that color.
#[allow(clippy::too_many_arguments)]
pub(crate) fn picture(
    c: &Ctx,
    element: &str,
    path: &Path,
    template: Option<gpui::Rgba>,
    width: Option<f32>,
    height: Option<f32>,
    corner: Option<f32>,
    fit: Fit,
    aspect: Option<f32>,
    spin: Spin,
) -> El {
    let Some(picture) = read(path) else {
        return div().into_any_element();
    };
    let ratio = aspect.unwrap_or(picture.width as f32 / picture.height.max(1) as f32);
    let (width, height) = match (width, height) {
        (Some(width), None) => (Some(width), Some(width / ratio)),
        (None, Some(height)) => (Some(height * ratio), Some(height)),
        sides => sides,
    };
    let key = format!("{:p}{}", Rc::as_ptr(c.win()), c.target(element));
    let color = template.or(picture.color);
    let corner = corner.unwrap_or(0.0);
    let path = path.to_path_buf();
    let mut frame = div().flex_none().relative().child(
        canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| {
                let turning = spin.on && !crate::native::reduce_motion();
                let turn = advance(&key, turning, f64::from(spin.period));
                if turning {
                    window.request_animation_frame();
                }
                match color {
                    Some(color) => shape(
                        &key, &path, &picture, color, bounds, fit, corner, turn, window, cx,
                    ),
                    None => turned(&key, &picture, bounds, fit, corner, turn, turning, window),
                }
            },
        )
        .size_full(),
    );
    frame = match width {
        Some(width) => frame.w(px(width)),
        None => frame.w_full(),
    };
    frame = match height {
        Some(height) => frame.h(px(height)),
        None => frame.aspect_ratio(ratio),
    };
    frame.into_any_element()
}

fn read(path: &Path) -> Option<Rc<Picture>> {
    PICTURES.with(|pictures| {
        pictures
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let bytes = std::fs::read(path).ok()?;
                let pixels = image::load_from_memory(&bytes).ok()?.into_rgba8();
                Some(Rc::new(Picture {
                    width: pixels.width(),
                    height: pixels.height(),
                    color: one_color(&pixels),
                    bytes,
                    pixels: Arc::new(pixels),
                }))
            })
            .clone()
    })
}

/// The color the visible pixels of `pixels` share, edges aside, if they
/// share one.
fn one_color(pixels: &image::RgbaImage) -> Option<gpui::Rgba> {
    let mut shared: Option<[u8; 3]> = None;
    for pixel in pixels.pixels().filter(|pixel| pixel[3] >= 96) {
        let rgb = [pixel[0], pixel[1], pixel[2]];
        match shared {
            None => shared = Some(rgb),
            Some(first) if first.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 12) => {}
            Some(_) => return None,
        }
    }
    let [r, g, b] = shared
        .unwrap_or_default()
        .map(|channel| f32::from(channel) / 255.0);
    Some(gpui::Rgba { r, g, b, a: 1.0 })
}

/// The turn of the picture `key` now, moved on by the time since it was
/// last drawn while it turns.
fn advance(key: &str, turning: bool, period: f64) -> f64 {
    SPINNERS.with(|spinners| {
        let mut spinners = spinners.borrow_mut();
        let spinner = spinners.entry(key.to_string()).or_default();
        if turning {
            let now = Instant::now();
            if let Some(last) = spinner.last {
                spinner.turn = (spinner.turn + now.duration_since(last).as_secs_f64() / period)
                    .rem_euclid(1.0);
            }
            spinner.last = Some(now);
        } else {
            spinner.last = None;
        }
        spinner.turn
    })
}

/// Draw the picture as a shape in `color`, turned on the GPU.
#[allow(clippy::too_many_arguments)]
fn shape(
    key: &str,
    path: &Path,
    picture: &Picture,
    color: gpui::Rgba,
    bounds: Bounds<Pixels>,
    fit: Fit,
    corner: f32,
    turn: f64,
    window: &mut Window,
    cx: &mut App,
) {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let name = format!(
        "bern-spin:{}:{width}x{height}:{fit:?}:{corner}",
        path.display()
    );
    let svg = SPINNERS.with(|spinners| {
        let mut spinners = spinners.borrow_mut();
        let spinner = spinners.entry(key.to_string()).or_default();
        match &spinner.shape {
            Some((made, svg)) if *made == name => svg.clone(),
            _ => {
                let svg: Arc<[u8]> = shape_svg(picture, width, height, fit, corner).into();
                spinner.shape = Some((name.clone(), svg.clone()));
                svg
            }
        }
    });
    let scale = window.scale_factor();
    let center = bounds.center();
    let matrix = TransformationMatrix::unit()
        .translate(center.scale(scale))
        .rotate(radians(turn as f32 * std::f32::consts::TAU))
        .translate(center.scale(-scale));
    let _ = window.paint_svg(bounds, name.into(), Some(&svg), matrix, color.into(), cx);
}

/// An SVG of the picture fitted into a `width` by `height` frame, for
/// GPUI to draw its alpha as a shape.
fn shape_svg(picture: &Picture, width: f32, height: f32, fit: Fit, corner: f32) -> Vec<u8> {
    let aspect = match fit {
        Fit::Contain => "xMidYMid meet",
        Fit::Cover => "xMidYMid slice",
        Fit::Fill => "none",
    };
    let (clip, clipped) = if corner > 0.0 {
        (
            format!(
                r#"<clipPath id="c"><rect width="{width}" height="{height}" rx="{corner}"/></clipPath>"#
            ),
            r#" clip-path="url(#c)""#,
        )
    } else {
        (String::new(), "")
    };
    let (mime, data) = match image::guess_format(&picture.bytes) {
        Ok(image::ImageFormat::Png) => ("image/png", picture.bytes.clone()),
        Ok(image::ImageFormat::Jpeg) => ("image/jpeg", picture.bytes.clone()),
        Ok(image::ImageFormat::Gif) => ("image/gif", picture.bytes.clone()),
        Ok(image::ImageFormat::WebP) => ("image/webp", picture.bytes.clone()),
        _ => {
            let mut png = std::io::Cursor::new(Vec::new());
            let _ = picture.pixels.write_to(&mut png, image::ImageFormat::Png);
            ("image/png", png.into_inner())
        }
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><defs>{clip}</defs><image width="{width}" height="{height}" preserveAspectRatio="{aspect}"{clipped} xlink:href="data:{mime};base64,{}"/></svg>"#,
        base64(&data)
    )
    .into_bytes()
}

fn base64(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk.iter().enumerate().fold(0u32, |group, (i, byte)| {
            group | u32::from(*byte) << (16 - 8 * i)
        });
        for i in 0..4 {
            text.push(if i <= chunk.len() {
                char::from(DIGITS[(group >> (18 - 6 * i) & 63) as usize])
            } else {
                '='
            });
        }
    }
    text
}

// MARK: - Turning on the CPU

/// A frame to turn: the picture fitted into a frame of `width` by `height`
/// pixels, turned by `turn`.
struct Job {
    key: String,
    pixels: Arc<image::RgbaImage>,
    width: u32,
    height: u32,
    fit: Fit,
    corner: f32,
    turn: f64,
}

/// A turned frame: a square as wide as the frame's diagonal, BGRA.
struct Done {
    turn: f64,
    fitted: (u32, u32),
    side: u32,
    bgra: Vec<u8>,
}

/// Frames turned, by picture, until the window takes them.
static DONE: Mutex<Option<HashMap<String, Done>>> = Mutex::new(None);
static JOBS: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();

fn send(job: Job) {
    let jobs = JOBS.get_or_init(|| {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let _ = std::thread::Builder::new()
            .name("Bern turning pictures".into())
            .spawn(move || work(inbox));
        Mutex::new(jobs)
    });
    if let Ok(jobs) = jobs.lock() {
        let _ = jobs.send(job);
    }
}

fn work(inbox: mpsc::Receiver<Job>) {
    // Pictures fitted into their frames, premultiplied.
    let mut fitted: HashMap<(String, u32, u32), Vec<u8>> = HashMap::new();
    while let Ok(job) = inbox.recv() {
        if fitted.len() > 16 {
            fitted.clear();
        }
        let base = fitted
            .entry((job.key.clone(), job.width, job.height))
            .or_insert_with(|| fit_into(&job));
        let side = (f64::from(job.width).hypot(f64::from(job.height))).ceil() as u32;
        let mut bgra = vec![0; (side * side * 4) as usize];
        rotate(base, job.width, job.height, &mut bgra, side, job.turn);
        if let Ok(mut done) = DONE.lock() {
            done.get_or_insert_with(HashMap::new).insert(
                job.key,
                Done {
                    turn: job.turn,
                    fitted: (job.width, job.height),
                    side,
                    bgra,
                },
            );
        }
    }
}

/// The picture fitted into the job's frame, premultiplied RGBA, its
/// corners rounded.
fn fit_into(job: &Job) -> Vec<u8> {
    let (width, height) = (job.width, job.height);
    let (pw, ph) = (job.pixels.width() as f32, job.pixels.height() as f32);
    let (fw, fh) = match job.fit {
        Fit::Fill => (width as f32, height as f32),
        Fit::Contain => {
            let scale = (width as f32 / pw).min(height as f32 / ph);
            (pw * scale, ph * scale)
        }
        Fit::Cover => {
            let scale = (width as f32 / pw).max(height as f32 / ph);
            (pw * scale, ph * scale)
        }
    };
    let (fw, fh) = ((fw.round() as u32).max(1), (fh.round() as u32).max(1));
    let mut premultiplied = (*job.pixels).clone();
    for pixel in premultiplied.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in 0..3 {
            pixel[channel] = ((u16::from(pixel[channel]) * alpha + 127) / 255) as u8;
        }
    }
    let scaled = image::imageops::resize(
        &premultiplied,
        fw,
        fh,
        image::imageops::FilterType::Triangle,
    );
    let (ox, oy) = (
        (i64::from(width) - i64::from(fw)) / 2,
        (i64::from(height) - i64::from(fh)) / 2,
    );
    let r = job.corner;
    let mut base = vec![0; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let (sx, sy) = (i64::from(x) - ox, i64::from(y) - oy);
            if sx < 0 || sy < 0 || sx >= i64::from(fw) || sy >= i64::from(fh) {
                continue;
            }
            if r > 0.0 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (cx, cy) = (
                    px.clamp(r, width as f32 - r),
                    py.clamp(r, height as f32 - r),
                );
                if (px - cx).powi(2) + (py - cy).powi(2) > r * r {
                    continue;
                }
            }
            let at = ((y * width + x) * 4) as usize;
            base[at..at + 4].copy_from_slice(&scaled.get_pixel(sx as u32, sy as u32).0);
        }
    }
    base
}

/// `base`, premultiplied RGBA of `width` by `height`, turned by `turn`
/// about its center into the middle of `out`, straight BGRA `side` wide,
/// sampled bilinearly.
fn rotate(base: &[u8], width: u32, height: u32, out: &mut [u8], side: u32, turn: f64) {
    let (sin, cos) = ((turn * std::f64::consts::TAU) as f32).sin_cos();
    let (w, h, side) = (width as usize, height as usize, side as usize);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let half = side as f32 / 2.0;
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - half, y as f32 + 0.5 - half);
            // Where this pixel was before the turn.
            let sx = cos * dx + sin * dy + cx - 0.5;
            let sy = -sin * dx + cos * dy + cy - 0.5;
            if sx < -1.0 || sy < -1.0 || sx >= w as f32 || sy >= h as f32 {
                continue;
            }
            let (x0, y0) = (sx.floor(), sy.floor());
            let (fx, fy) = (sx - x0, sy - y0);
            let mut sum = [0f32; 4];
            for (ox, oy, weight) in [
                (0, 0, (1.0 - fx) * (1.0 - fy)),
                (1, 0, fx * (1.0 - fy)),
                (0, 1, (1.0 - fx) * fy),
                (1, 1, fx * fy),
            ] {
                let (px, py) = (x0 as i64 + ox, y0 as i64 + oy);
                if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                    continue;
                }
                let at = (py as usize * w + px as usize) * 4;
                for (total, value) in sum.iter_mut().zip(&base[at..at + 4]) {
                    *total += f32::from(*value) * weight;
                }
            }
            let alpha = sum[3];
            if alpha < 0.5 {
                continue;
            }
            let at = (y * side + x) * 4;
            let unmultiply = 255.0 / alpha;
            out[at] = (sum[2] * unmultiply).min(255.0) as u8;
            out[at + 1] = (sum[1] * unmultiply).min(255.0) as u8;
            out[at + 2] = (sum[0] * unmultiply).min(255.0) as u8;
            out[at + 3] = alpha.round().min(255.0) as u8;
        }
    }
}

/// Draw the latest frame turned on the CPU, asking for the next one.
#[allow(clippy::too_many_arguments)]
fn turned(
    key: &str,
    picture: &Picture,
    bounds: Bounds<Pixels>,
    fit: Fit,
    corner: f32,
    turn: f64,
    turning: bool,
    window: &mut Window,
) {
    let scale = window.scale_factor();
    let finished = DONE
        .lock()
        .ok()
        .and_then(|mut done| done.as_mut()?.remove(key));
    let frame = SPINNERS.with(|spinners| {
        let mut spinners = spinners.borrow_mut();
        let spinner = spinners.entry(key.to_string()).or_default();
        if let Some(done) = finished {
            spinner.pending = false;
            if let Some(frame) = image::RgbaImage::from_raw(done.side, done.side, done.bgra) {
                let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                    frame
                )]));
                if let Some((_, _, old)) = spinner.frame.replace((done.turn, done.fitted, image)) {
                    let _ = window.drop_image(old);
                }
            }
        }
        let fitted = (
            ((f32::from(bounds.size.width) * scale).round() as u32).max(1),
            ((f32::from(bounds.size.height) * scale).round() as u32).max(1),
        );
        let current = spinner
            .frame
            .as_ref()
            .is_some_and(|(at, made, _)| (at - turn).abs() < 1e-9 && *made == fitted);
        if !spinner.pending && (turning || !current) {
            spinner.pending = true;
            send(Job {
                key: key.to_string(),
                pixels: picture.pixels.clone(),
                width: fitted.0,
                height: fitted.1,
                fit,
                corner: corner * scale,
                turn,
            });
        }
        if spinner.pending {
            window.request_animation_frame();
        }
        spinner.frame.as_ref().map(|(_, _, frame)| frame.clone())
    });
    let Some(frame) = frame else {
        return;
    };
    let side = frame.size(0).width.0 as f32 / scale;
    let center = bounds.center();
    let square = Bounds::new(
        point(
            center.x - gpui::px(side / 2.0),
            center.y - gpui::px(side / 2.0),
        ),
        size(gpui::px(side), gpui::px(side)),
    );
    let _ = window.paint_image(square, square, Default::default(), frame, 0, false);
}
