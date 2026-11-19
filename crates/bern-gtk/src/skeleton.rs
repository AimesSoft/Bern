//! Non-interactive theme-aware skeleton placeholders and anti-flicker host.

use crate::motion::{self, Num};
use crate::reactive::{effect, on_cleanup, untrack};
use crate::structure::Slot;
use crate::text::tr;
use crate::{Build, Ctx, Get, Parent};
use adw::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ShapeKind {
    #[default]
    Rect,
    Capsule,
    Circle,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SkeletonWidget {
        pub width: Cell<Option<f64>>,
        pub height: Cell<Option<f64>>,
        pub fraction: Cell<Option<f64>>,
        pub max_width: Cell<Option<f64>>,
        pub aspect: Cell<Option<f64>>,
        pub corner: Cell<f64>,
        pub(super) shape: Cell<ShapeKind>,
        pub base_opacity: Cell<f64>,
        pub pulse: Cell<bool>,
        pub started_us: Cell<Option<i64>>,
        pub factor: Cell<f64>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SkeletonWidget {
        const NAME: &'static str = "BernSkeleton";
        type Type = super::SkeletonWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SkeletonWidget {
        fn constructed(&self) {
            self.parent_constructed();
            self.factor.set(1.0);
            self.base_opacity.set(0.07);
            self.corner.set(8.0);
            self.pulse.set(true);
            let obj = self.obj();
            obj.set_can_target(false);
            obj.set_focusable(false);
            obj.update_state(&[gtk::accessible::State::Hidden(true)]);
        }
    }

    impl WidgetImpl for SkeletonWidget {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            if self.width.get().is_none() && self.aspect.get().is_some() {
                gtk::SizeRequestMode::HeightForWidth
            } else {
                gtk::SizeRequestMode::ConstantSize
            }
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            if let Some(w) = self.width.get() {
                let size = match orientation {
                    gtk::Orientation::Horizontal => w.max(0.0).round() as i32,
                    _ => {
                        let h = self
                            .height
                            .get()
                            .or_else(|| self.aspect.get().filter(|a| *a > 0.0).map(|a| w / a))
                            .or_else(|| (self.shape.get() == ShapeKind::Circle).then_some(w))
                            .unwrap_or(0.0);
                        h.max(0.0).round() as i32
                    }
                };
                return (size, size, -1, -1);
            }
            if self.fraction.get().is_some() {
                return match orientation {
                    gtk::Orientation::Horizontal => {
                        let nat = self.max_width.get().unwrap_or(200.0).max(1.0).round() as i32;
                        (0, nat, -1, -1)
                    }
                    _ => {
                        let h = self.height.get().unwrap_or(12.0).max(0.0).round() as i32;
                        (h, h, -1, -1)
                    }
                };
            }
            if let Some(aspect) = self.aspect.get().filter(|a| *a > 0.0) {
                return match orientation {
                    gtk::Orientation::Horizontal => (0, 0, -1, -1),
                    _ => {
                        let h = if for_size > 0 {
                            (f64::from(for_size) / aspect.max(0.01)).round() as i32
                        } else {
                            0
                        };
                        (0, h, -1, -1)
                    }
                };
            }
            match orientation {
                gtk::Orientation::Horizontal => (0, 0, -1, -1),
                _ => {
                    let h = self.height.get().map_or(0, |v| v.max(0.0).round() as i32);
                    (h, h, -1, -1)
                }
            }
        }

        fn compute_expand(&self, hexpand: &mut bool, vexpand: &mut bool) {
            *hexpand = self.width.get().is_none();
            *vexpand = self.width.get().is_none()
                && self.height.get().is_none()
                && self.fraction.get().is_none()
                && self.aspect.get().is_none();
        }

        fn map(&self) {
            self.parent_map();
            if !self.pulse.get() || motion::reduced() {
                self.factor.set(1.0);
                return;
            }
            if self.tick.borrow().is_some() {
                return;
            }
            self.started_us.set(None);
            let tick = self.obj().add_tick_callback(|widget, clock| {
                let imp = widget.imp();
                if !imp.pulse.get() || motion::reduced() {
                    imp.factor.set(1.0);
                    widget.queue_draw();
                    return glib::ControlFlow::Continue;
                }
                let now = clock.frame_time();
                let start = match imp.started_us.get() {
                    Some(s) => s,
                    None => {
                        imp.started_us.set(Some(now));
                        now
                    }
                };
                let elapsed = ((now - start).max(0) as f64) * 1e-6;
                // Gentle breathing cycle over 1.8 s: 1.0 ↔ 0.55.
                let wave = 0.5 + 0.5 * (elapsed * std::f64::consts::PI / 0.9).cos();
                imp.factor.set(0.55 + 0.45 * wave);
                widget.queue_draw();
                glib::ControlFlow::Continue
            });
            *self.tick.borrow_mut() = Some(tick);
        }

        fn unmap(&self) {
            if let Some(tick) = self.tick.borrow_mut().take() {
                tick.remove();
            }
            self.parent_unmap();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let alloc_w = f64::from(obj.width());
            let alloc_h = f64::from(obj.height());
            if alloc_w <= 0.0 || alloc_h <= 0.0 {
                return;
            }
            let (x, draw_w) = if let Some(frac) = self.fraction.get() {
                let bounded = self.max_width.get().map_or(alloc_w, |m| alloc_w.min(m));
                let w = (bounded * frac).clamp(0.0, alloc_w);
                let x = obj.parent().map_or(0.0, |parent| {
                    if parent.has_css_class("bern-cross-center") {
                        (alloc_w - w) * 0.5
                    } else if parent.has_css_class("bern-cross-end") {
                        alloc_w - w
                    } else {
                        0.0
                    }
                });
                (x, w)
            } else {
                (0.0, alloc_w)
            };
            if draw_w <= 0.0 {
                return;
            }
            let rect = gtk::graphene::Rect::new(x as f32, 0.0, draw_w as f32, alloc_h as f32);
            let max_r = (draw_w.min(alloc_h) * 0.5) as f32;
            let radius = match self.shape.get() {
                ShapeKind::Capsule | ShapeKind::Circle => max_r,
                ShapeKind::Rect => (self.corner.get() as f32).clamp(0.0, max_r),
            };
            let mut color = obj.color();
            let alpha = (f64::from(color.alpha()) * self.base_opacity.get() * self.factor.get())
                .clamp(0.0, 1.0) as f32;
            color.set_alpha(alpha);
            if radius > 0.0 {
                snapshot.push_rounded_clip(&gtk::gsk::RoundedRect::from_rect(rect, radius));
                snapshot.append_color(&color, &rect);
                snapshot.pop();
            } else {
                snapshot.append_color(&color, &rect);
            }
        }
    }
}

glib::wrapper! {
    pub struct SkeletonWidget(ObjectSubclass<imp::SkeletonWidget>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SkeletonWidget {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    pub(crate) fn media_placeholder() -> Self {
        let widget = Self::new();
        let imp = widget.imp();
        imp.base_opacity.set(0.07);
        imp.corner.set(0.0);
        imp.pulse.set(true);
        widget
    }
}

pub struct SkeletonProps {
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub fraction: Option<f64>,
    pub max_width: Option<Num>,
    pub aspect: Option<f64>,
    pub corner: Option<Num>,
    pub shape: Option<&'static str>,
    pub base_opacity: f64,
    pub pulse: bool,
}

#[allow(non_snake_case)]
pub fn Skeleton(p: &Parent, ctx: &Ctx, props: SkeletonProps) {
    let widget = SkeletonWidget::new();
    let imp = widget.imp();
    imp.width
        .set(props.width.as_ref().map(|n| untrack(|| n.now(ctx))));
    imp.height
        .set(props.height.as_ref().map(|n| untrack(|| n.now(ctx))));
    imp.fraction.set(props.fraction);
    imp.max_width
        .set(props.max_width.as_ref().map(|n| untrack(|| n.now(ctx))));
    imp.aspect.set(props.aspect);
    imp.corner.set(
        props
            .corner
            .as_ref()
            .map_or(8.0, |n| untrack(|| n.now(ctx)).max(0.0)),
    );
    imp.shape.set(match props.shape {
        Some("capsule") => ShapeKind::Capsule,
        Some("circle") => ShapeKind::Circle,
        _ => ShapeKind::Rect,
    });
    imp.base_opacity.set(props.base_opacity.clamp(0.0, 1.0));
    imp.pulse.set(props.pulse);
    p.add(&widget);
}

/// Fade newly mounted widgets from 0 to 1 over 150 ms unless motion is reduced.
pub(crate) fn fade_in_widgets(widgets: &[gtk::Widget]) {
    if motion::reduced() {
        return;
    }
    for widget in widgets {
        widget.set_opacity(0.0);
        let started: Cell<Option<i64>> = Cell::new(None);
        widget.add_tick_callback(move |w, clock| {
            let now = clock.frame_time();
            let start = match started.get() {
                Some(s) => s,
                None => {
                    started.set(Some(now));
                    now
                }
            };
            let t = (((now - start).max(0) as f64) * 1e-6 / 0.15).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            w.set_opacity(eased);
            if t >= 1.0 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
}

/// Mark a mounted skeleton region non-interactive and announce it once to
/// assistive technology.
pub(crate) fn mark_skeleton_widgets(widgets: &[gtk::Widget], label: &str) {
    let mut announced = false;
    for widget in widgets {
        widget.set_can_target(false);
        if !announced {
            announced = true;
            widget.update_property(&[gtk::accessible::Property::Label(label)]);
        } else {
            widget.update_state(&[gtk::accessible::State::Hidden(true)]);
        }
    }
}

/// Anti-flicker skeleton host: waits 120 ms before revealing `skeleton` when
/// `active` is truthy so fast loads never flash, and fades `content` in over
/// 150 ms once ready.
#[allow(non_snake_case)]
pub fn SkeletonHost(p: &Parent, ctx: &Ctx, active: Get, skeleton: Build, content: Build) {
    let slot = Slot::new(p.presentation_region());
    struct State {
        started: bool,
        was_active: bool,
        showing_skeleton: bool,
        generation: u64,
        disposed: bool,
    }
    let state = Rc::new(RefCell::new(State {
        started: false,
        was_active: false,
        showing_skeleton: false,
        generation: 0,
        disposed: false,
    }));
    let drop_state = Rc::clone(&state);
    on_cleanup(move || {
        let mut s = drop_state.borrow_mut();
        s.disposed = true;
        s.generation = s.generation.wrapping_add(1);
    });
    let ctx = ctx.clone();
    effect(move || {
        let now = crate::value::truthy(&active(&ctx));
        let (mine, from_skeleton) = {
            let mut s = state.borrow_mut();
            if s.started && s.was_active == now {
                return;
            }
            s.started = true;
            s.was_active = now;
            s.generation = s.generation.wrapping_add(1);
            let from = if !now {
                let prev = s.showing_skeleton;
                s.showing_skeleton = false;
                prev
            } else {
                false
            };
            (s.generation, from)
        };
        if now {
            let slot = Rc::clone(&slot);
            let state = Rc::clone(&state);
            let skeleton = Rc::clone(&skeleton);
            let ctx = ctx.clone();
            glib::timeout_add_local_once(Duration::from_millis(120), move || {
                {
                    let mut s = state.borrow_mut();
                    if s.disposed || s.generation != mine || !s.was_active {
                        return;
                    }
                    s.showing_skeleton = true;
                }
                slot.fill(true, |region| {
                    skeleton(region, &ctx);
                    mark_skeleton_widgets(&region.widgets(), &tr("bern.query.loading"));
                });
            });
        } else {
            let content = Rc::clone(&content);
            let ctx = ctx.clone();
            slot.fill(from_skeleton, |region| {
                content(region, &ctx);
                if from_skeleton {
                    fade_in_widgets(&region.widgets());
                }
            });
        }
    });
}
