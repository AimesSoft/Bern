//! Non-interactive theme-aware skeleton placeholders and anti-flicker host.

use crate::El;
use crate::ctx::Ctx;
use crate::units::px;
use gpui::{IntoElement, ParentElement, Rgba, Styled, canvas, div};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SkeletonShape {
    #[default]
    Rect,
    Capsule,
    Circle,
}

#[derive(Clone, Copy, Default)]
pub struct SkeletonProps {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fraction: Option<f32>,
    pub max_width: Option<f32>,
    pub aspect: Option<f32>,
    pub corner: f32,
    pub shape: SkeletonShape,
    pub base_opacity: f32,
    pub pulse: bool,
}

fn pulse_epoch() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

fn paint_skeleton_quad(
    fg: Rgba,
    base_opacity: f32,
    pulse: bool,
    reduced: bool,
    corner: f32,
    shape: SkeletonShape,
) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _cx| {
            let factor = if pulse && !reduced {
                let elapsed = Instant::now().duration_since(pulse_epoch()).as_secs_f64();
                let wave = 0.5 + 0.5 * (elapsed * std::f64::consts::PI / 0.9).cos();
                window.request_animation_frame();
                (0.55 + 0.45 * wave) as f32
            } else {
                1.0
            };
            let mut color = fg;
            color.a = (color.a * base_opacity.clamp(0.0, 1.0) * factor).clamp(0.0, 1.0);
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            let max_r = (w.min(h) * 0.5).max(0.0);
            let radius = match shape {
                SkeletonShape::Capsule | SkeletonShape::Circle => max_r,
                SkeletonShape::Rect => corner.clamp(0.0, max_r),
            };
            window.paint_quad(gpui::fill(bounds, color).corner_radii(gpui::px(radius)));
        },
    )
}

/// Non-interactive theme-aware skeleton placeholder shape, aligned with
/// `bern-surface::controls::skeleton`.
pub fn skeleton(c: &Ctx, props: SkeletonProps) -> El {
    let surface_shape = match props.shape {
        SkeletonShape::Rect => bern_surface::controls::SkeletonShape::Rect,
        SkeletonShape::Capsule => bern_surface::controls::SkeletonShape::Capsule,
        SkeletonShape::Circle => bern_surface::controls::SkeletonShape::Circle,
    };
    let ir = bern_surface::controls::skeleton(bern_surface::controls::SkeletonSpec {
        width: props.width,
        height: props.height,
        corner: props.corner,
        shape: surface_shape,
        base_opacity: props.base_opacity,
    });
    let fg = c.color("text.primary");
    let base_opacity = crate::surface::as_box(&ir)
        .and_then(|b| b.style.background)
        .map(|bg| crate::surface::resolve_color(c, bg).a)
        .unwrap_or(props.base_opacity);
    let reduced = crate::motion::reduced(c);
    let paint = paint_skeleton_quad(
        fg,
        base_opacity,
        props.pulse,
        reduced,
        props.corner,
        props.shape,
    )
    .size_full();

    let mut frame = div();
    if let Some(w) = props.width {
        frame = frame.flex_none().w(px(w.max(0.0)));
        if let Some(h) = props.height {
            frame = frame.h(px(h.max(0.0)));
        } else if let Some(aspect) = props.aspect.filter(|a| *a > 0.0) {
            frame = frame.h(px((w / aspect).max(0.0)));
        } else if props.shape == SkeletonShape::Circle {
            frame = frame.h(px(w.max(0.0)));
        }
    } else if let Some(fraction) = props.fraction {
        let clamped = fraction.clamp(0.01, 1.0);
        let h = props.height.unwrap_or(12.0).max(0.0);
        if let Some(max_w) = props.max_width {
            let target = (max_w * clamped).max(0.0);
            frame = frame
                .w(px(target))
                .max_w(gpui::relative(clamped))
                .min_w(px(0.0))
                .h(px(h));
        } else {
            frame = frame.w(gpui::relative(clamped)).min_w(px(0.0)).h(px(h));
        }
    } else if let Some(aspect) = props.aspect.filter(|a| *a > 0.0) {
        frame = frame.w_full().min_w(px(0.0)).aspect_ratio(aspect);
    } else if let Some(h) = props.height {
        frame = frame.w_full().h(px(h.max(0.0)));
    } else {
        frame = frame.size_full();
    }
    frame.child(paint).into_any_element()
}

/// Pulsing media placeholder for `Image` URLs while they load.
pub(crate) fn media_placeholder(fg: Rgba, reduced: bool, corner: f32) -> El {
    div()
        .size_full()
        .child(
            paint_skeleton_quad(fg, 0.07, true, reduced, corner, SkeletonShape::Rect).size_full(),
        )
        .into_any_element()
}

struct HostState {
    window: String,
    active: bool,
    active_since: Option<Instant>,
    showing_skeleton: bool,
    had_content: bool,
    fade_since: Option<Instant>,
    seen: bool,
}

thread_local! {
    static HOSTS: RefCell<HashMap<String, HostState>> = RefCell::new(HashMap::new());
}

const DELAY: Duration = Duration::from_millis(120);
const FADE: Duration = Duration::from_millis(150);

/// What a skeleton host should draw on the current frame.
#[derive(Clone, Copy)]
pub(crate) enum HostPhase {
    Empty,
    KeepContent,
    Skeleton,
    Content { fade: Option<f32> },
}

pub(crate) fn begin(window: &str) {
    HOSTS.with(|hosts| {
        for state in hosts.borrow_mut().values_mut() {
            if state.window == window {
                state.seen = false;
            }
        }
    });
}

/// Sweep unseen hosts for `window` and return whether another frame is needed
/// for a pending anti-flicker delay or content fade-in.
pub(crate) fn end(window: &str, closed: bool) -> bool {
    let mut animate = false;
    HOSTS.with(|hosts| {
        let mut hosts = hosts.borrow_mut();
        hosts.retain(|_, state| state.window != window || (state.seen && !closed));
        if !closed {
            for state in hosts.values() {
                if state.window == window
                    && ((state.active && !state.showing_skeleton) || state.fade_since.is_some())
                {
                    animate = true;
                    break;
                }
            }
        }
    });
    animate
}

pub(crate) fn host_phase(
    window: &str,
    key: &str,
    active: bool,
    reduced: bool,
    listing: bool,
) -> HostPhase {
    if listing {
        return if active {
            HostPhase::Skeleton
        } else {
            HostPhase::Content { fade: None }
        };
    }
    let now = Instant::now();
    HOSTS.with(|hosts| {
        let mut hosts = hosts.borrow_mut();
        let state = hosts.entry(key.to_string()).or_insert_with(|| HostState {
            window: window.to_string(),
            active,
            active_since: active.then_some(now),
            showing_skeleton: false,
            had_content: !active,
            fade_since: None,
            seen: true,
        });
        state.seen = true;
        if active {
            if !state.active {
                state.active = true;
                state.active_since = Some(now);
                state.showing_skeleton = false;
                state.fade_since = None;
            }
            let elapsed = state
                .active_since
                .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
            if elapsed >= DELAY {
                state.showing_skeleton = true;
                HostPhase::Skeleton
            } else if state.had_content {
                HostPhase::KeepContent
            } else {
                HostPhase::Empty
            }
        } else {
            if state.showing_skeleton && !reduced {
                state.fade_since = Some(now);
            }
            state.active = false;
            state.active_since = None;
            state.showing_skeleton = false;
            state.had_content = true;
            let fade = if let Some(since) = state.fade_since {
                let t = now.saturating_duration_since(since).as_secs_f32() / FADE.as_secs_f32();
                if t >= 1.0 || reduced {
                    state.fade_since = None;
                    None
                } else {
                    let eased = 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
                    Some(eased)
                }
            } else {
                None
            };
            HostPhase::Content { fade }
        }
    })
}

pub(crate) fn apply_fade(elements: Vec<El>, fade: Option<f32>) -> Vec<El> {
    let Some(alpha) = fade else {
        return elements;
    };
    elements
        .into_iter()
        .map(|el| div().opacity(alpha).child(el).into_any_element())
        .collect()
}

/// Anti-flicker host returning zero, one, or multiple sibling elements.
pub fn skeleton_host(
    c: &Ctx,
    id: &str,
    active: bool,
    skeleton: impl FnOnce(&Ctx) -> Vec<El>,
    content: impl FnOnce(&Ctx) -> Vec<El>,
) -> Vec<El> {
    let key = format!("{}/skel:{}", c.win().id, c.key(id));
    match host_phase(
        &c.win().id,
        &key,
        active,
        crate::motion::reduced(c),
        c.listing(),
    ) {
        HostPhase::Empty => Vec::new(),
        HostPhase::KeepContent => content(c),
        HostPhase::Skeleton => skeleton(c),
        HostPhase::Content { fade } => apply_fade(content(c), fade),
    }
}

/// Anti-flicker host for a single root element (window or component root).
pub fn skeleton_host_one(
    c: &Ctx,
    id: &str,
    active: bool,
    skeleton: impl FnOnce(&Ctx) -> Vec<El>,
    content: impl FnOnce(&Ctx) -> El,
) -> El {
    let key = format!("{}/skel:{}", c.win().id, c.key(id));
    match host_phase(
        &c.win().id,
        &key,
        active,
        crate::motion::reduced(c),
        c.listing(),
    ) {
        HostPhase::Empty => div().into_any_element(),
        HostPhase::KeepContent => content(c),
        HostPhase::Skeleton => {
            let items = skeleton(c);
            if items.len() == 1 {
                items.into_iter().next().unwrap()
            } else {
                c.grow()
                    .apply(div().flex().flex_col().children(items))
                    .into_any_element()
            }
        }
        HostPhase::Content { fade } => {
            let el = content(c);
            match fade {
                Some(alpha) => c
                    .grow()
                    .apply(div().opacity(alpha).child(el))
                    .into_any_element(),
                None => el,
            }
        }
    }
}
