//! Native pure-Rust animated SVG presenter for GPUI (`bern-svg` + `RenderImage`).

use crate::El;
use crate::content::Fit;
use crate::ctx::Ctx;
use crate::units::px;
use bern_ffi::bern_svg::{Config, Fit as SvgFit, PixelFormat, Player, PointerEvent};
use gpui::{
    Bounds, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    RenderImage, Styled, canvas, div,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

struct Slot {
    source: &'static str,
    player: Option<Player>,
    image: Option<Arc<RenderImage>>,
    bounds: Option<Bounds<Pixels>>,
}

thread_local! {
    static START: Instant = Instant::now();
    static SLOTS: RefCell<HashMap<String, Slot>> = RefCell::new(HashMap::new());
}

fn now_secs() -> f64 {
    START.with(|s| s.elapsed().as_secs_f64())
}

pub struct SvgProps {
    pub element: &'static str,
    pub source: &'static str,
    pub label: Option<String>,
    pub width: f32,
    pub height: f32,
    pub fit: Fit,
    pub playing: bool,
    pub speed: f64,
    pub restart: f64,
    pub interactive: bool,
}

pub fn svg(c: &Ctx, props: SvgProps) -> El {
    let _ = SystemTime::UNIX_EPOCH;
    let key = format!("{:p}{}", Rc::as_ptr(c.win()), c.key(props.element));
    let dark = c.theme().dark;
    let fg = c.color("text.primary");
    let text_rgba = [
        (fg.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (fg.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (fg.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (fg.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    ];
    let svg_fit = match props.fit {
        Fit::Contain => SvgFit::Contain,
        Fit::Cover => SvgFit::Cover,
        Fit::Fill => SvgFit::Fill,
    };
    let width = props.width.max(0.0);
    let height = props.height.max(0.0);
    let source = props.source;
    let playing = props.playing;
    let speed = props.speed;
    let restart = props.restart;
    let interactive = props.interactive;

    let key_paint = key.clone();
    let inner = canvas(
        |_, _, _| {},
        move |bounds, (), window, _cx| {
            let scale = window.scale_factor().max(1.0);
            let w_px = ((f32::from(bounds.size.width).max(1.0) * scale).round() as u32).max(1);
            let h_px = ((f32::from(bounds.size.height).max(1.0) * scale).round() as u32).max(1);

            let (image, active) = SLOTS.with(|slots| {
                let mut slots = slots.borrow_mut();
                let slot = slots.entry(key_paint.clone()).or_insert_with(|| Slot {
                    source: "",
                    player: None,
                    image: None,
                    bounds: None,
                });
                slot.bounds = Some(bounds);
                if slot.source != source {
                    slot.source = source;
                    if let Some(old) = slot.image.take() {
                        let _ = window.drop_image(old);
                    }
                    let path = std::path::Path::new(&crate::host::resources_dir()).join(source);
                    slot.player = std::fs::read(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| bern_ffi::svg::open_scene(source, &bytes))
                        .map(Player::new)
                        .ok();
                }

                let Some(ref mut player) = slot.player else {
                    return (None, false);
                };
                let _ = player.configure(Config {
                    playing,
                    speed: if speed.is_finite() && speed > 0.0 {
                        speed
                    } else {
                        1.0
                    },
                    restart: if restart.is_finite() { restart } else { 0.0 },
                    fit: svg_fit,
                    interactive,
                    dark,
                    reduced_motion: crate::native::reduce_motion(),
                    text_rgba,
                });
                let (outcome, pixels) =
                    player.render(now_secs(), w_px, h_px, PixelFormat::BgraStraight);
                if (outcome.changed || slot.image.is_none())
                    && let Some(buf) = image::RgbaImage::from_raw(w_px, h_px, pixels.to_vec())
                {
                    let rendered =
                        Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                            buf
                        )]));
                    if let Some(old) = slot.image.replace(rendered) {
                        let _ = window.drop_image(old);
                    }
                }
                (slot.image.clone(), outcome.active)
            });

            if active {
                window.request_animation_frame();
            }
            if let Some(image) = image {
                let _ = window.paint_image(bounds, bounds, Default::default(), image, 0, false);
            }
        },
    )
    .size_full();

    let mut el = div()
        .id(ElementId::Name(key.clone().into()))
        .flex_none()
        .relative()
        .overflow_hidden()
        .w(px(width))
        .h(px(height))
        .child(inner);

    if interactive {
        let k_down = key.clone();
        el = el.on_mouse_down(MouseButton::Left, move |ev, window, _cx| {
            let needs_refresh = SLOTS.with(|slots| {
                let mut slots = slots.borrow_mut();
                let Some(slot) = slots.get_mut(&k_down) else {
                    return false;
                };
                let (nx, ny) = norm_point(slot.bounds, ev.position);
                let Some(ref mut player) = slot.player else {
                    return false;
                };
                let mut changed = player.pointer(PointerEvent::Down(nx, ny)).changed;
                if ev.click_count >= 2 {
                    let out = player.pointer(PointerEvent::DoubleClick(nx, ny));
                    changed |= out.changed || out.active;
                } else if ev.click_count == 1 {
                    let out = player.pointer(PointerEvent::Click(nx, ny));
                    changed |= out.changed || out.active;
                }
                changed
            });
            if needs_refresh {
                window.refresh();
            }
        });

        let k_up = key.clone();
        el = el.on_mouse_up(MouseButton::Left, move |ev, window, _cx| {
            let needs_refresh = SLOTS.with(|slots| {
                let mut slots = slots.borrow_mut();
                let Some(slot) = slots.get_mut(&k_up) else {
                    return false;
                };
                let (nx, ny) = norm_point(slot.bounds, ev.position);
                slot.player
                    .as_mut()
                    .is_some_and(|p| p.pointer(PointerEvent::Up(nx, ny)).changed)
            });
            if needs_refresh {
                window.refresh();
            }
        });

        let k_move = key;
        el = el.on_mouse_move(move |ev, window, _cx| {
            let needs_refresh = SLOTS.with(|slots| {
                let mut slots = slots.borrow_mut();
                let Some(slot) = slots.get_mut(&k_move) else {
                    return false;
                };
                let Some(b) = slot.bounds else {
                    return false;
                };
                let inside = b.contains(&ev.position);
                let Some(ref mut player) = slot.player else {
                    return false;
                };
                if inside {
                    let (nx, ny) = norm_point(Some(b), ev.position);
                    player.pointer(PointerEvent::Move(nx, ny)).changed
                } else {
                    player.pointer(PointerEvent::Leave).changed
                }
            });
            if needs_refresh {
                window.refresh();
            }
        });
    }

    let _ = props.label;
    el.into_any_element()
}

fn norm_point(bounds: Option<Bounds<Pixels>>, pt: gpui::Point<Pixels>) -> (f32, f32) {
    let Some(b) = bounds else {
        return (0.5, 0.5);
    };
    let w = f32::from(b.size.width).max(1.0);
    let h = f32::from(b.size.height).max(1.0);
    (
        ((f32::from(pt.x) - f32::from(b.origin.x)) / w).clamp(0.0, 1.0),
        ((f32::from(pt.y) - f32::from(b.origin.y)) / h).clamp(0.0, 1.0),
    )
}
