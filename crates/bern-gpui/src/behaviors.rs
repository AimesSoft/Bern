//! Elements with a life of their own while they are mounted: sliders being
//! dragged, shortcuts and their recorder, timers, watched values and
//! toasts. Each is a GPUI element that keeps its state with GPUI between
//! renders, so building content only to list its controls (`ui.inspect`)
//! starts nothing.

use crate::ctx::{Act, Ctx, Model};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, ElementId, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, canvas, div, relative,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

// MARK: - Slider

/// How a `Slider` maps its track to values.
#[derive(Clone, Copy, Debug)]
pub struct SliderProps {
    /// The role that fills the chosen part of the track.
    pub tint: Option<&'static str>,
    pub min: f64,
    pub max: f64,
    pub step: Option<f64>,
    /// Run the action while dragging, not only when the drag ends.
    pub continuous: bool,
    pub label_hidden: bool,
    pub min_icon: Option<&'static str>,
    pub max_icon: Option<&'static str>,
}

/// The value at a fraction of the track, on a step.
fn snapped(props: &SliderProps, fraction: f64) -> f64 {
    let value = props.min + (props.max - props.min) * fraction.clamp(0.0, 1.0);
    match props.step {
        Some(step) if step > 0.0 => {
            (props.min + ((value - props.min) / step).round() * step).clamp(props.min, props.max)
        }
        _ => value,
    }
}

#[derive(IntoElement)]
struct Track {
    key: ElementId,
    props: SliderProps,
    value: f64,
    enabled: bool,
    /// Show a value while dragging, and commit one when the drag ends.
    preview: Rc<dyn Fn(f64)>,
    commit: Rc<dyn Fn(f64)>,
    colors: [gpui::Rgba; 3],
}

impl RenderOnce for Track {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Where the track was last laid out, and whether a drag is on.
        let state = window.use_keyed_state(self.key.clone(), cx, |_, _| {
            (
                Rc::new(Cell::new(Bounds::<Pixels>::default())),
                Rc::new(Cell::new(false)),
            )
        });
        let (bounds, dragging) = state.read(cx).clone();
        let props = self.props;
        let span = props.max - props.min;
        let fraction = if span > 0.0 {
            ((self.value - props.min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let [track, fill, thumb] = self.colors;
        let at = move |x: Pixels, bounds: Bounds<Pixels>| {
            let width = f64::from(bounds.size.width).max(1.0);
            snapped(&props, f64::from(x - bounds.origin.x) / width)
        };
        let measured = Rc::clone(&bounds);
        let element = div()
            .id(self.key)
            .relative()
            .flex_1()
            .h(px(24.0))
            .flex()
            .items_center()
            .child(
                canvas(move |bounds, _, _| measured.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div().w_full().h(px(4.0)).rounded(px(2.0)).bg(track).child(
                    div()
                        .h_full()
                        .w(relative(fraction as f32))
                        .rounded(px(2.0))
                        .bg(fill),
                ),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(fraction as f32))
                    .ml(px(-9.0))
                    .size(px(18.0))
                    .rounded_full()
                    .bg(thumb)
                    .border_1()
                    .border_color(fill)
                    .shadow_sm(),
            );
        if !self.enabled {
            return element.opacity(0.5);
        }
        let (down, moving, up) = (Rc::clone(&bounds), Rc::clone(&bounds), bounds);
        let (dragging_down, dragging_move, dragging_up) =
            (Rc::clone(&dragging), Rc::clone(&dragging), dragging);
        let (preview, preview_move, commit) = (Rc::clone(&self.preview), self.preview, self.commit);
        element
            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, _| {
                dragging_down.set(true);
                preview(at(event.position.x, down.get()));
            })
            .on_mouse_move(move |event: &MouseMoveEvent, _, _| {
                if dragging_move.get() && event.pressed_button == Some(MouseButton::Left) {
                    preview_move(at(event.position.x, moving.get()));
                }
            })
            .on_mouse_up_out(MouseButton::Left, {
                let dragging = Rc::clone(&dragging_up);
                let commit = Rc::clone(&commit);
                let bounds = Rc::clone(&up);
                move |event: &MouseUpEvent, _, _| {
                    if dragging.replace(false) {
                        commit(at(event.position.x, bounds.get()));
                    }
                }
            })
            .on_mouse_up(MouseButton::Left, move |event: &MouseUpEvent, _, _| {
                if dragging_up.replace(false) {
                    commit(at(event.position.x, up.get()));
                }
            })
    }
}

/// `Slider`: a value between `min` and `max` dragged along a track. A
/// draft previews while dragging; the action runs when the drag ends (and
/// while dragging with `continuous`).
pub fn slider(c: &Ctx, id: &'static str, label: String, model: Model, props: SliderProps) -> El {
    let enabled = c.enabled();
    let value = crate::value::as_f64(&model.value(c));
    let number = |value: f64| match &model {
        Model::Field(field, _) if matches!(c.read(*field), Value::Int(_)) => {
            Value::Int(value.round() as i64)
        }
        Model::Controlled(Value::Int(_), _) => Value::Int(value.round() as i64),
        _ => Value::Float(value),
    };
    let set: Rc<dyn Fn(f64)> = {
        let c = c.clone();
        let model = model.clone();
        let int = matches!(number(0.0), Value::Int(_));
        Rc::new(move |value| {
            let value = if int {
                Value::Int(value.round() as i64)
            } else {
                Value::Float(value)
            };
            model.set(&c, value)
        })
    };
    // Dragging a draft previews it; a value the engine holds changes on
    // commit only, unless it is continuous.
    let preview: Rc<dyn Fn(f64)> = match &model {
        Model::Field(field, _) => {
            let c = c.clone();
            let field = *field;
            let int = matches!(number(0.0), Value::Int(_));
            let commit_while = props.continuous;
            let set = Rc::clone(&set);
            Rc::new(move |value| {
                if commit_while {
                    set(value);
                } else {
                    let value = if int {
                        Value::Int(value.round() as i64)
                    } else {
                        Value::Float(value)
                    };
                    c.write(field, value);
                }
            })
        }
        Model::Controlled(..) if props.continuous => Rc::clone(&set),
        Model::Controlled(..) => Rc::new(|_| {}),
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "slider",
        label: label.clone(),
        value: Some(crate::value::number(value)),
        enabled,
        actions: Actions {
            edit: Some({
                let set = Rc::clone(&set);
                Rc::new(move |text: String| {
                    if let Ok(value) = text.trim().parse::<f64>() {
                        set(value);
                    }
                })
            }),
            ..Actions::default()
        },
    });
    let colors = [
        c.color("separator"),
        c.color(props.tint.unwrap_or("accent")),
        c.color("surface.raised"),
    ];
    let icon_color = c.color("icon.secondary");
    let track = Track {
        key: c.element_id(id),
        props,
        value,
        enabled,
        preview,
        commit: set,
        colors,
    };
    let row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .w_full()
        .children(
            props
                .min_icon
                .map(|icon| icons::icon(icon, 16.0, icon_color)),
        )
        .child(track)
        .children(
            props
                .max_icon
                .map(|icon| icons::icon(icon, 16.0, icon_color)),
        );
    let element = if props.label_hidden {
        row
    } else {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .w_full()
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(c.color("text.primary"))
                    .child(label),
            )
            .child(row)
    };
    c.grow().apply(element).into_any_element()
}

// MARK: - Shortcuts

/// `Shortcut`: a key combination that runs the action while mounted and
/// enabled; it shows nothing.
pub fn shortcut(c: &Ctx, key: String, enabled: bool, action: Option<Act>) -> El {
    if enabled && c.enabled() {
        let press = crate::controls::press(c, None, action);
        crate::controls::register_shortcut(c, Some(&key), &press);
    }
    // Out of the flow, so its container's spacing does not count it.
    div().absolute().into_any_element()
}

/// The way GPUI names a key combination, written as Bern writes them:
/// `cmd+shift+k`.
fn written(event: &KeyDownEvent) -> Option<String> {
    let keystroke = &event.keystroke;
    let key = keystroke.key.as_str();
    if matches!(
        key,
        "shift" | "control" | "alt" | "platform" | "function" | "cmd"
    ) {
        return None;
    }
    let mut parts = Vec::new();
    let modifiers = keystroke.modifiers;
    if modifiers.platform {
        parts.push("cmd");
    }
    if modifiers.control {
        parts.push("ctrl");
    }
    if modifiers.alt {
        parts.push("alt");
    }
    if modifiers.shift {
        parts.push("shift");
    }
    parts.push(key);
    Some(parts.join("+"))
}

/// How a `ShortcutInput` reads.
#[derive(Clone, Default)]
pub struct ShortcutInputProps {
    pub label: String,
    pub label_hidden: bool,
    pub prompt: String,
    pub empty: String,
    pub enabled: bool,
}

#[derive(IntoElement)]
struct Recorder {
    key: ElementId,
    current: String,
    props: ShortcutInputProps,
    choose: Rc<dyn Fn(String)>,
    colors: [gpui::Rgba; 4],
}

impl RenderOnce for Recorder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus: gpui::Entity<FocusHandle> =
            window.use_keyed_state(self.key.clone(), cx, |_, cx| cx.focus_handle());
        let focus = focus.read(cx).clone();
        let recording = focus.is_focused(window);
        let [background, foreground, border, accent] = self.colors;
        let shown = if recording {
            self.props.prompt.clone()
        } else if self.current.is_empty() {
            self.props.empty.clone()
        } else {
            self.current.clone()
        };
        let choose = self.choose;
        let element = div()
            .id(self.key)
            .track_focus(&focus)
            .flex()
            .items_center()
            .justify_center()
            .h(px(32.0))
            .min_w(px(96.0))
            .px(px(12.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(if recording { accent } else { border })
            .bg(background)
            .text_color(foreground)
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .child(shown);
        if !self.props.enabled {
            return element.opacity(0.5);
        }
        let clicked = focus.clone();
        element
            .on_click(move |_, window, cx| window.focus(&clicked, cx))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                cx.stop_propagation();
                match event.keystroke.key.as_str() {
                    "escape" => window.blur(cx),
                    "backspace" | "delete" if !event.keystroke.modifiers.modified() => {
                        choose(String::new());
                        window.blur(cx);
                    }
                    _ => {
                        if let Some(keys) = written(event) {
                            choose(keys);
                            window.blur(cx);
                        }
                    }
                }
            })
    }
}

/// `ShortcutInput`: a button showing a shortcut; pressed, it takes the
/// next key combination, which the action receives in place of `current`.
pub fn shortcut_input(
    c: &Ctx,
    id: &'static str,
    current: String,
    props: ShortcutInputProps,
    choose: crate::ctx::Commit,
) -> El {
    let enabled = props.enabled && c.enabled();
    let chosen: Rc<dyn Fn(String)> = {
        let c = c.clone();
        Rc::new(move |keys| choose(&c, Value::Text(keys)))
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "shortcut_input",
        label: props.label.clone(),
        value: Some(current.clone()),
        enabled,
        actions: Actions {
            edit: Some(Rc::clone(&chosen)),
            ..Actions::default()
        },
    });
    let colors = [
        c.slot("field", "rest", "background"),
        c.slot("field", "rest", "foreground"),
        c.slot("field", "rest", "border"),
        c.color("accent"),
    ];
    let label = props.label.clone();
    let label_hidden = props.label_hidden;
    let recorder = Recorder {
        key: c.element_id(id),
        current,
        props: ShortcutInputProps { enabled, ..props },
        choose: chosen,
        colors,
    };
    if label_hidden {
        return recorder.into_any_element();
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .w_full()
        .child(
            div()
                .flex_1()
                .text_size(px(14.0))
                .text_color(c.color("text.primary"))
                .child(label),
        )
        .child(recorder)
        .into_any_element()
}

// MARK: - Timers and watched values

struct Ticking {
    run: Pending,
    interval: Duration,
    _task: Task<()>,
}

#[derive(IntoElement)]
struct Ticker {
    key: ElementId,
    interval: Duration,
    run: Option<Rc<dyn Fn()>>,
}

impl RenderOnce for Ticker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let interval = self.interval;
        let state = window.use_keyed_state(self.key, cx, move |_, cx| {
            let run: Pending = Rc::new(RefCell::new(None));
            let tick = Rc::clone(&run);
            let task = cx.spawn(async move |_, cx| {
                loop {
                    cx.background_executor().timer(interval).await;
                    let action = tick.borrow().clone();
                    if let Some(action) = action {
                        cx.update(|_| action());
                    }
                }
            });
            Ticking {
                run,
                interval,
                _task: task,
            }
        });
        let ticking = state.read(cx);
        if ticking.interval == interval {
            *ticking.run.borrow_mut() = self.run;
        }
        div().absolute()
    }
}

/// The action a ticker runs next, replaced as the element draws again.
type Pending = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// `Timer`: the action every `interval` seconds while mounted and enabled.
pub fn timer(c: &Ctx, id: &str, interval: f64, enabled: bool, action: Option<Act>) -> El {
    let run = (enabled && interval > 0.0)
        .then_some(action)
        .flatten()
        .map(|action| {
            let c = c.clone();
            Rc::new(move || action(&c)) as Rc<dyn Fn()>
        });
    Ticker {
        key: ElementId::Name(format!("{}.timer.{interval}", c.target(id)).into()),
        interval: Duration::from_secs_f64(interval.max(0.05)),
        run,
    }
    .into_any_element()
}

#[derive(IntoElement)]
struct Watcher {
    key: ElementId,
    value: Value,
    run: Rc<dyn Fn()>,
}

impl RenderOnce for Watcher {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let last = window.use_keyed_state(self.key, cx, |_, _| None::<Value>);
        let changed = last.read(cx).as_ref() != Some(&self.value);
        if changed {
            last.update(cx, |last, _| *last = Some(self.value));
            (self.run)();
        }
        div().absolute()
    }
}

/// `Watch`: the action when mounted and whenever the value changes.
pub fn watch(c: &Ctx, id: &str, value: Value, action: Option<Act>) -> El {
    let run: Rc<dyn Fn()> = match action {
        Some(action) => {
            let c = c.clone();
            Rc::new(move || action(&c))
        }
        None => Rc::new(|| {}),
    };
    Watcher {
        key: ElementId::Name(format!("{}.watch", c.target(id)).into()),
        value,
        run,
    }
    .into_any_element()
}

// MARK: - Toasts

struct Shown {
    key: String,
    since: std::time::Instant,
    hide: Option<Task<()>>,
}

#[derive(IntoElement)]
struct Notice {
    key: ElementId,
    shown_key: String,
    content: String,
    icon: Option<&'static str>,
    duration: Duration,
    colors: [gpui::Rgba; 3],
}

impl RenderOnce for Notice {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.key, cx, |_, _| Shown {
            key: String::new(),
            since: std::time::Instant::now(),
            hide: None,
        });
        let duration = self.duration;
        if state.read(cx).key != self.shown_key {
            let hide = cx.spawn(async move |cx| {
                cx.background_executor().timer(duration).await;
                cx.update(|_| crate::host::redraw());
            });
            state.update(cx, |shown, _| {
                shown.key = self.shown_key.clone();
                shown.since = std::time::Instant::now();
                shown.hide = Some(hide);
            });
        }
        let visible = state.read(cx).since.elapsed() < duration;
        let [background, foreground, border] = self.colors;
        let viewport = window.viewport_size();
        // Over the content, centred at the foot of the window.
        div().absolute().when(visible, |element| {
            element.child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Anchor::BottomCenter)
                        .position(gpui::point(
                            viewport.width / 2.0,
                            viewport.height - px(24.0),
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.0))
                                .px(px(14.0))
                                .py(px(10.0))
                                .rounded(px(10.0))
                                .border_1()
                                .border_color(border)
                                .bg(background)
                                .text_color(foreground)
                                .text_size(px(13.0))
                                .shadow_lg()
                                .children(self.icon.map(|icon| icons::icon(icon, 16.0, foreground)))
                                .child(SharedString::from(self.content)),
                        ),
                )
                .with_priority(3),
            )
        })
    }
}

/// `Toast`: a short notice that shows when it appears or its key (else its
/// content) changes, and hides itself after `duration` seconds.
pub fn toast(
    c: &Ctx,
    content: String,
    icon: Option<&'static str>,
    key: Option<String>,
    duration: Option<f64>,
) -> El {
    Notice {
        key: ElementId::Name(format!("{}.toast", c.target("toast")).into()),
        shown_key: key.unwrap_or_else(|| content.clone()),
        content,
        icon,
        duration: Duration::from_secs_f64(duration.unwrap_or(3.0).max(0.1)),
        colors: [
            c.color("surface.raised"),
            c.color("text.primary"),
            c.color("separator"),
        ],
    }
    .into_any_element()
}

// MARK: - Focus

#[derive(IntoElement)]
struct Focused {
    key: ElementId,
    child: El,
}

impl RenderOnce for Focused {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.key, cx, |_, cx| (cx.focus_handle(), false));
        let (handle, focused) = state.read(cx).clone();
        if !focused {
            window.focus(&handle, cx);
            state.update(cx, |state, _| state.1 = true);
        }
        div().track_focus(&handle).child(self.child)
    }
}

/// `default_focus: true`: the control takes the keyboard when it appears.
pub fn focus_on_appear(c: &Ctx, id: &str, child: El) -> El {
    Focused {
        key: ElementId::Name(format!("{}.focus", c.target(id)).into()),
        child,
    }
    .into_any_element()
}

/// `FocusRegion`: while a text field inside is editing, the first press
/// outside only ends the editing.
#[derive(IntoElement)]
struct Region {
    key: SharedString,
    content: Vec<El>,
}

impl RenderOnce for Region {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window.use_keyed_state(self.key, cx, |_, cx| cx.focus_handle());
        let handle: FocusHandle = handle.read(cx).clone();
        let inside = handle.clone();
        div()
            .flex()
            .flex_col()
            .track_focus(&handle)
            .on_mouse_down_out(move |_, window, cx| {
                if inside.contains_focused(window, cx) {
                    window.blur(cx);
                    cx.stop_propagation();
                }
            })
            .children(self.content)
    }
}

/// `FocusRegion`.
pub fn focus_region(c: &Ctx, id: &str, content: Vec<El>) -> El {
    c.grow()
        .apply(div().flex().flex_col().child(Region {
            key: c.target(id).into(),
            content,
        }))
        .into_any_element()
}

/// `number` as the kind of number `current` is.
fn like(current: &Value, number: f64) -> Value {
    match current {
        Value::Int(_) => Value::Int(number.round() as i64),
        _ => Value::Float(number),
    }
}

/// `Pinch`: pinching its content scales `value` within `min` and `max`;
/// the action runs when the fingers lift.
#[allow(clippy::too_many_arguments)]
pub fn pinch(
    c: &Ctx,
    id: &'static str,
    label: String,
    field: crate::ctx::Field,
    min: f64,
    max: f64,
    action: Option<Act>,
    content: Vec<El>,
) -> El {
    let value = crate::value::as_f64(&c.read(field));
    let set: Rc<dyn Fn(String)> = {
        let c = c.clone();
        Rc::new(move |text: String| {
            if let Ok(number) = text.trim().parse::<f64>() {
                c.write(field, like(&c.read(field), number.clamp(min, max)));
            }
        })
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "pinch",
        label,
        value: Some(crate::value::number(value)),
        enabled: c.enabled(),
        actions: Actions {
            edit: Some(set),
            ..Actions::default()
        },
    });
    let ctx = c.clone();
    let element = div()
        .id(c.element_id(id))
        .flex()
        .flex_col()
        .on_pinch(move |event, _, _| {
            let current = crate::value::as_f64(&ctx.read(field));
            let scaled = (current * (1.0 + f64::from(event.delta))).clamp(min, max);
            if scaled != current {
                ctx.write(field, like(&ctx.read(field), scaled));
            }
            if event.phase == gpui::TouchPhase::Ended
                && let Some(action) = &action
            {
                action(&ctx);
            }
        })
        .children(content);
    c.grow().apply(element).into_any_element()
}
