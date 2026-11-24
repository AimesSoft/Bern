//! `Marquee`: a clipped horizontal scrolling line owned by the renderer's
//! clock and pause state.

use crate::El;
use crate::ctx::Ctx;
use crate::units::px;
use gpui::{
    ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, canvas,
    div,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

/// Properties of a `Marquee`.
#[derive(Clone, Copy, Debug)]
pub struct MarqueeProps {
    /// Seconds for one full horizontal pass across the content.
    pub duration: f32,
    /// Starting phase offset in `0.0..1.0`.
    pub phase: f32,
    /// Pause scrolling while true.
    pub paused: bool,
    /// Opacity of the scrolling content (`0.0..=1.0`).
    pub opacity: f32,
}

impl Default for MarqueeProps {
    fn default() -> Self {
        Self {
            duration: 12.0,
            phase: 0.0,
            paused: false,
            opacity: 1.0,
        }
    }
}

struct Clock {
    viewport: f32,
    content: f32,
    offset: f32,
    last: Instant,
}

thread_local! {
    static CLOCKS: RefCell<HashMap<String, Clock>> = RefCell::new(HashMap::new());
}

const GAP: f32 = 36.0;

#[derive(IntoElement)]
struct MarqueeEl {
    id: ElementId,
    key: String,
    props: MarqueeProps,
    children: Vec<El>,
}

impl RenderOnce for MarqueeEl {
    fn render(self, window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let now = Instant::now();
        let reduce = crate::native::reduce_motion();
        let duration = self.props.duration.max(0.5);
        let (offset, scrolling) = CLOCKS.with(|clocks| {
            let mut clocks = clocks.borrow_mut();
            let entry = clocks.entry(self.key.clone()).or_insert_with(|| Clock {
                viewport: 0.0,
                content: 0.0,
                offset: self.props.phase.rem_euclid(1.0),
                last: now,
            });
            let dt = (now.saturating_duration_since(entry.last).as_secs_f32()).min(0.1);
            entry.last = now;
            let overflows = entry.content > entry.viewport + 1.0 && entry.viewport > 1.0;
            let active = overflows && !self.props.paused && !reduce;
            if active {
                entry.offset = (entry.offset + dt / duration).rem_euclid(1.0);
            }
            let span = if overflows { entry.content + GAP } else { 0.0 };
            (entry.offset * span, active)
        });
        if scrolling {
            window.request_animation_frame();
        }
        let key_vp = self.key.clone();
        let key_ct = self.key;
        let vp_probe = canvas(
            move |bounds, _, _| {
                let w = f32::from(bounds.size.width);
                CLOCKS.with(|clocks| {
                    if let Some(entry) = clocks.borrow_mut().get_mut(&key_vp)
                        && (entry.viewport - w).abs() > 0.5
                    {
                        entry.viewport = w;
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let ct_probe = canvas(
            move |bounds, _, _| {
                let w = f32::from(bounds.size.width);
                CLOCKS.with(|clocks| {
                    if let Some(entry) = clocks.borrow_mut().get_mut(&key_ct)
                        && (entry.content - w).abs() > 0.5
                    {
                        entry.content = w;
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        div()
            .id(self.id)
            .relative()
            .w_full()
            .overflow_hidden()
            .opacity(self.props.opacity.clamp(0.0, 1.0))
            .child(vp_probe)
            .child(
                div()
                    .relative()
                    .left(px(-offset))
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_row()
                            .flex_none()
                            .items_center()
                            .child(ct_probe)
                            .children(self.children),
                    ),
            )
    }
}

/// `Marquee`: clipped horizontal scrolling strip.
pub fn marquee(c: &Ctx, id: &'static str, props: MarqueeProps, children: Vec<El>) -> El {
    c.grow()
        .apply(div().w_full().min_w(px(0.0)).child(MarqueeEl {
            id: c.element_id(id),
            key: c.key(id),
            props,
            children,
        }))
        .into_any_element()
}
