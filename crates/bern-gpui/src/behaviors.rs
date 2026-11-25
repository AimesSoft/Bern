//! Elements with a life of their own while they are mounted: sliders being
//! dragged, shortcuts and their recorder, timers, watched values and
//! toasts. Each is a GPUI element that keeps its state with GPUI between
//! renders, so building content only to list its controls (`ui.inspect`)
//! starts nothing.

use crate::ctx::{Act, Ctx, Model};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::surface::{GpuiRenderer, ResolvedTrack, apply_box, as_box};
use crate::units::px;
use crate::{El, icons};
use bern_surface::controls::{
    ColorPickerSpec, ShortcutInputSpec, SliderSpec, ToastSpec, color_picker as ir_color_picker,
    shortcut_input as ir_shortcut_input, slider as ir_slider, toast as ir_toast,
};
use bern_surface::{Axis, SurfaceNode};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement, Styled, Task, Window, canvas, div,
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
    /// Where the pointer is along the track while it is over it, from 0 at
    /// `min` to 1 at `max`; `None` otherwise.
    pub hover: Option<crate::Field>,
    /// Vertical track (`min` at bottom, `max` at top).
    pub vertical: bool,
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

/// Find the lowered [`bern_surface::TrackNode`] inside a `controls::slider` IR tree.
fn find_track(node: &SurfaceNode) -> Option<&bern_surface::TrackNode> {
    match node {
        SurfaceNode::Track(track) => Some(track),
        SurfaceNode::Box(b) => b.children.iter().find_map(find_track),
        _ => None,
    }
}

#[derive(IntoElement)]
struct Track {
    key: ElementId,
    props: SliderProps,
    enabled: bool,
    /// Show a value while dragging, and commit one when the drag ends.
    preview: Rc<dyn Fn(f64)>,
    commit: Rc<dyn Fn(f64)>,
    /// Report where the pointer is along the track, or that it left.
    hover: Option<Rc<dyn Fn(Option<f64>)>>,
    /// Lowered `bern-surface` track & white perpendicular pill thumb.
    track: ResolvedTrack,
}

impl RenderOnce for Track {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Where the track was last laid out, whether a drag is on, and whether
        // the pointer is hovering the track (for active pill scaling).
        let state = window.use_keyed_state(self.key.clone(), cx, |_, _| {
            (
                Rc::new(Cell::new(Bounds::<Pixels>::default())),
                Rc::new(Cell::new(false)),
                Rc::new(Cell::new(false)),
            )
        });
        let (bounds, dragging, hovered) = state.read(cx).clone();
        let props = self.props;
        let vertical = self.track.axis == Axis::Vertical;
        let at = move |pos: gpui::Point<Pixels>, bounds: Bounds<Pixels>| {
            let fraction = if vertical {
                let height = f64::from(bounds.size.height).max(1.0);
                1.0 - f64::from(pos.y - bounds.origin.y) / height
            } else {
                let width = f64::from(bounds.size.width).max(1.0);
                f64::from(pos.x - bounds.origin.x) / width
            };
            snapped(&props, fraction)
        };
        let active = dragging.get() || hovered.get();
        let (rail, thumb) = self.track.build_visual(active);
        let measured = Rc::clone(&bounds);
        let mut element = if vertical {
            let mut d = div()
                .id(self.key)
                .relative()
                .flex_1()
                .w(px(self.track.hit_extent))
                .flex()
                .justify_center();
            if let Some(min_h) = self.track.min_main_extent {
                d = d.min_h(px(min_h));
            }
            d
        } else {
            div()
                .id(self.key)
                .relative()
                .flex_1()
                .h(px(self.track.hit_extent))
                .flex()
                .items_center()
        };
        element = element
            .child(
                canvas(move |bounds, _, _| measured.set(bounds), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(rail)
            .children(thumb);
        if self.enabled {
            element = element.cursor_pointer();
        }
        let hovered_state = Rc::clone(&hovered);
        let hover_cb = self.hover;
        let element = match hover_cb {
            Some(hover) => {
                let (moving, left) = (Rc::clone(&hover), hover);
                let measured = Rc::clone(&bounds);
                element
                    .on_mouse_move(move |event: &MouseMoveEvent, _, _| {
                        let bounds = measured.get();
                        let fraction = if vertical {
                            let height = f64::from(bounds.size.height).max(1.0);
                            1.0 - f64::from(event.position.y - bounds.origin.y) / height
                        } else {
                            let width = f64::from(bounds.size.width).max(1.0);
                            f64::from(event.position.x - bounds.origin.x) / width
                        };
                        moving(Some(fraction.clamp(0.0, 1.0)));
                    })
                    .on_hover(move |inside, window, _| {
                        if hovered_state.replace(*inside) != *inside {
                            window.refresh();
                        }
                        if !*inside {
                            left(None);
                        }
                    })
            }
            None => element.on_hover(move |inside, window, _| {
                if hovered_state.replace(*inside) != *inside {
                    window.refresh();
                }
            }),
        };
        if !self.enabled {
            return element.opacity(0.5);
        }
        let (down, moving, up) = (Rc::clone(&bounds), Rc::clone(&bounds), bounds);
        let (dragging_down, dragging_move, dragging_up) =
            (Rc::clone(&dragging), Rc::clone(&dragging), dragging);
        let (preview, preview_move, commit) = (Rc::clone(&self.preview), self.preview, self.commit);
        element
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    // The slider takes the press: what lies under it does not.
                    cx.stop_propagation();
                    dragging_down.set(true);
                    window.refresh();
                    preview(at(event.position, down.get()));
                },
            )
            .on_mouse_move(move |event: &MouseMoveEvent, _, _| {
                if dragging_move.get() && event.pressed_button == Some(MouseButton::Left) {
                    preview_move(at(event.position, moving.get()));
                }
            })
            .on_mouse_up_out(MouseButton::Left, {
                let dragging = Rc::clone(&dragging_up);
                let commit = Rc::clone(&commit);
                let bounds = Rc::clone(&up);
                move |event: &MouseUpEvent, window, _| {
                    if dragging.replace(false) {
                        window.refresh();
                        commit(at(event.position, bounds.get()));
                    }
                }
            })
            .on_mouse_up(MouseButton::Left, move |event: &MouseUpEvent, window, _| {
                if dragging_up.replace(false) {
                    window.refresh();
                    commit(at(event.position, up.get()));
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
    let span = props.max - props.min;
    let fraction = if span > 0.0 {
        ((value - props.min) / span).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
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
            set_number: Some(Rc::clone(&set)),
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
    let lowered = ir_slider(SliderSpec {
        label: (!props.label_hidden).then(|| label.clone()),
        label_hidden: props.label_hidden,
        axis: if props.vertical {
            Axis::Vertical
        } else {
            Axis::Horizontal
        },
        fraction,
        enabled,
        tint: props.tint,
        min_icon: props.min_icon,
        max_icon: props.max_icon,
    });
    let resolved_track = find_track(&lowered)
        .map(|node| ResolvedTrack::from_node(c, node))
        .expect("bern-surface slider always contains a TrackNode");
    let icon_color = c.color("icon.secondary");
    // A draft of the window, written only when the place changes.
    let hover = props.hover.map(|field| {
        let c = c.clone();
        Rc::new(move |fraction: Option<f64>| {
            let value = match fraction {
                Some(fraction) => Value::some(Value::Float(fraction)),
                None => Value::none(),
            };
            if c.read(field) != value {
                c.write(field, value);
            }
        }) as Rc<dyn Fn(Option<f64>)>
    });
    let track = Track {
        key: c.element_id(id),
        props,
        enabled,
        preview,
        commit: set,
        hover,
        track: resolved_track,
    };
    if props.vertical {
        let col = c
            .grow()
            .apply(div().flex().flex_col().items_center().gap(px(8.0)).h_full())
            .children((!props.label_hidden).then(|| {
                div()
                    .text_size(px(13.0))
                    .text_color(c.color("text.primary"))
                    .child(label)
            }))
            .children(
                props
                    .max_icon
                    .map(|icon| icons::icon(icon, 16.0, icon_color)),
            )
            .child(track)
            .children(
                props
                    .min_icon
                    .map(|icon| icons::icon(icon, 16.0, icon_color)),
            );
        return col.into_any_element();
    }
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
    // It takes the room left beside the texts of its row, which keep theirs.
    c.grow().apply(element.min_w(px(0.0))).into_any_element()
}

// MARK: - ColorPicker

/// How a `ColorPicker` shows.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColorPickerProps {
    pub label_hidden: bool,
}

/// `ColorPicker`: a 24-bit sRGB color (`0xRRGGBB`) swatch button whose
/// popover holds preset swatches and R/G/B channel sliders.
pub fn color_picker(
    c: &Ctx,
    id: &'static str,
    label: String,
    model: Model,
    props: ColorPickerProps,
) -> El {
    let enabled = c.enabled();
    let current = match model.value(c) {
        Value::Int(n) => n.clamp(0, 0xff_ffff) as u32,
        other => (crate::value::as_f64(&other).round() as i64).clamp(0, 0xff_ffff) as u32,
    };
    let int = match &model {
        Model::Field(field, _) => matches!(c.read(*field), Value::Int(_)),
        Model::Controlled(Value::Int(_), _) => true,
        _ => false,
    };
    let set_rgb: Rc<dyn Fn(u32)> = {
        let c = c.clone();
        let model = model.clone();
        Rc::new(move |rgb: u32| {
            let rgb = rgb & 0xff_ffff;
            let value = if int {
                Value::Int(i64::from(rgb))
            } else {
                Value::Float(f64::from(rgb))
            };
            model.set(&c, value);
        })
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "color_picker",
        label: label.clone(),
        value: Some(current.to_string()),
        enabled,
        actions: Actions {
            set_number: Some({
                let set_rgb = Rc::clone(&set_rgb);
                Rc::new(move |number: f64| {
                    if number.is_finite() {
                        set_rgb((number.round() as i64).clamp(0, 0xff_ffff) as u32);
                    }
                })
            }),
            edit: Some({
                let set_rgb = Rc::clone(&set_rgb);
                Rc::new(move |text: String| {
                    let trimmed = text.trim();
                    let parsed = if let Some(hex) = trimmed.strip_prefix('#') {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        trimmed
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .map(|v| (v.round() as i64).clamp(0, 0xff_ffff) as u32)
                    };
                    if let Some(rgb) = parsed {
                        set_rgb(rgb);
                    }
                })
            }),
            ..Actions::default()
        },
    });
    let target = c.key(id);
    let open = crate::overlay::is_open(&target);
    let swatch_ir = ir_color_picker(ColorPickerSpec {
        label: label.clone(),
        label_hidden: true,
        rgb: current,
        enabled,
    });
    let swatch = as_box(&swatch_ir)
        .map(|b| apply_box(c, div(), b))
        .unwrap_or_else(div)
        .id(c.element_id(id));
    let swatch = if enabled {
        let toggle = target.clone();
        swatch.on_click(move |_, _, cx| {
            cx.stop_propagation();
            crate::overlay::set_open(&toggle, !crate::overlay::is_open(&toggle));
        })
    } else {
        swatch
    };
    let button_wrap = div()
        .relative()
        .flex_none()
        .child(swatch)
        .child(crate::overlay::opener_place(&target))
        .when(open, |element| {
            let swatches: [u32; 8] = [
                0xFFFFFF, 0xFFE066, 0xFF6B8B, 0x7ED6DF, 0xA3CB38, 0xD980FA, 0xFFA502, 0x70A1FF,
            ];
            let r = f64::from((current >> 16) & 0xff);
            let g = f64::from((current >> 8) & 0xff);
            let b = f64::from(current & 0xff);
            let channel = |name: &'static str, value: f64, shift: u32| {
                let set_rgb = Rc::clone(&set_rgb);
                let update: Rc<dyn Fn(f64)> = Rc::new(move |next: f64| {
                    let byte = (next.round() as i64).clamp(0, 255) as u32;
                    let mask = !(0xffu32 << shift);
                    set_rgb((current & mask) | (byte << shift));
                });
                let ch_ir = ir_slider(SliderSpec {
                    label: None,
                    label_hidden: true,
                    axis: Axis::Horizontal,
                    fraction: (value / 255.0).clamp(0.0, 1.0) as f32,
                    enabled,
                    tint: None,
                    min_icon: None,
                    max_icon: None,
                });
                let ch_track = find_track(&ch_ir)
                    .map(|node| ResolvedTrack::from_node(c, node))
                    .expect("bern-surface slider always contains a TrackNode");
                let track = Track {
                    key: ElementId::Name(format!("{target}.{name}").into()),
                    props: SliderProps {
                        tint: None,
                        min: 0.0,
                        max: 255.0,
                        step: Some(1.0),
                        continuous: true,
                        label_hidden: true,
                        min_icon: None,
                        max_icon: None,
                        hover: None,
                        vertical: false,
                    },
                    enabled,
                    preview: Rc::clone(&update),
                    commit: update,
                    hover: None,
                    track: ch_track,
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(36.0))
                            .text_size(px(12.0))
                            .text_color(c.color("text.secondary"))
                            .child(format!("{name} {}", value.round() as u32)),
                    )
                    .child(track)
            };
            let body = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .w(px(236.0))
                .p(px(12.0))
                .child(div().flex().flex_row().gap(px(6.0)).children(
                    swatches.into_iter().enumerate().map(|(index, swatch_rgb)| {
                        let set_rgb = Rc::clone(&set_rgb);
                        let selected = swatch_rgb == current;
                        div()
                            .id(ElementId::Name(format!("{target}.swatch.{index}").into()))
                            .size(px(22.0))
                            .rounded(px(11.0))
                            .bg(gpui::rgb(swatch_rgb))
                            .border_1()
                            .border_color(if selected {
                                c.color("accent")
                            } else {
                                c.slot("button.standard", "rest", "border")
                            })
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                set_rgb(swatch_rgb);
                            })
                    }),
                ))
                .child(channel("R", r, 16))
                .child(channel("G", g, 8))
                .child(channel("B", b, 0));
            element.child(crate::overlay::panel(c, &target, body))
        });
    if props.label_hidden {
        button_wrap.into_any_element()
    } else {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .w_full()
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(c.color("text.primary"))
                    .child(label),
            )
            .child(button_wrap)
            .into_any_element()
    }
}

// MARK: - Shortcuts

/// Properties of a `Shortcut` declaration.
#[derive(Clone, Debug)]
pub struct ShortcutProps {
    pub key: String,
    pub enabled: bool,
    pub quiet: bool,
    pub hold: Option<crate::Field>,
    pub hold_after: f64,
}

/// `Shortcut`: a key combination that runs the action while mounted and
/// enabled; it shows nothing. With `quiet`, its key is no input for
/// `window.idle`. With `hold`, holding past `hold_after` seconds sets the
/// draft field true until release instead of running the tap action.
pub fn shortcut(c: &Ctx, props: ShortcutProps, action: Option<Act>) -> El {
    if props.enabled && c.enabled() {
        let press = crate::controls::press(c, None, action);
        if let Some(hold) = props.hold {
            crate::controls::register_hold_shortcut(
                c,
                Some(&props.key),
                &press,
                props.quiet,
                hold,
                props.hold_after,
            );
        } else {
            crate::controls::register_shortcut_with_quiet(c, Some(&props.key), &press, props.quiet);
        }
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
    c: Ctx,
    key: ElementId,
    current: String,
    props: ShortcutInputProps,
    choose: Rc<dyn Fn(String)>,
}

impl RenderOnce for Recorder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus: gpui::Entity<FocusHandle> =
            window.use_keyed_state(self.key.clone(), cx, |_, cx| cx.focus_handle());
        let focus = focus.read(cx).clone();
        let recording = focus.is_focused(window);
        let shown = if recording {
            self.props.prompt.clone()
        } else if self.current.is_empty() {
            self.props.empty.clone()
        } else {
            self.current.clone()
        };
        let ir = ir_shortcut_input(ShortcutInputSpec {
            label: self.props.label.clone(),
            label_hidden: true,
            shown: shown.clone(),
            recording,
            enabled: self.props.enabled,
        });
        let choose = self.choose;
        let element = as_box(&ir)
            .map(|b| apply_box(&self.c, div(), b))
            .unwrap_or_else(div)
            .id(self.key)
            .track_focus(&focus)
            .child(shown);
        if !self.props.enabled {
            return element;
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
    let label = props.label.clone();
    let label_hidden = props.label_hidden;
    let recorder = Recorder {
        c: c.clone(),
        key: c.element_id(id),
        current,
        props: ShortcutInputProps { enabled, ..props },
        choose: chosen,
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
        key: ElementId::Name(format!("{}.timer.{interval}", c.key(id)).into()),
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
        if last.update(cx, |last, _| watch_changed(last, self.value)) {
            (self.run)();
        }
        div().absolute()
    }
}

pub(crate) fn watch_changed(last: &mut Option<Value>, value: Value) -> bool {
    if last.as_ref() == Some(&value) {
        return false;
    }
    *last = Some(value);
    true
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
        key: ElementId::Name(format!("{}.watch", c.key(id)).into()),
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
    c: Ctx,
    key: ElementId,
    shown_key: String,
    content: String,
    icon: Option<&'static str>,
    duration: Duration,
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
        let viewport = window.viewport_size();
        // Over the content, centred at the foot of the window.
        div().absolute().when(visible, |element| {
            let toast_node = ir_toast(ToastSpec {
                content: self.content,
                icon: self.icon,
            });
            let toast_el = GpuiRenderer::new(&self.c).render(&toast_node);
            element.child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Anchor::BottomCenter)
                        .position(gpui::point(
                            viewport.width / 2.0,
                            viewport.height - px(24.0),
                        ))
                        .child(toast_el),
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
        c: c.clone(),
        key: ElementId::Name(format!("{}.toast", c.key("toast")).into()),
        shown_key: key.unwrap_or_else(|| content.clone()),
        content,
        icon,
        duration: Duration::from_secs_f64(duration.unwrap_or(3.0).max(0.1)),
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
        key: ElementId::Name(format!("{}.focus", c.key(id)).into()),
        child,
    }
    .into_any_element()
}

/// `FocusRegion`: while a text field inside is editing, the first press
/// outside only ends the editing.
#[derive(IntoElement)]
struct Region {
    key: gpui::SharedString,
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
            key: c.key(id).into(),
            content,
        }))
        .into_any_element()
}

/// `number` as the kind of number `current` is.
pub(crate) fn like(current: &Value, number: f64) -> Value {
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

/// The desktop renderer does not offer a reversible per-window display
/// brightness override. Keep the requested value intact, and publish only
/// this API's availability; do not change global display gamma/brightness.
pub fn screen_brightness(c: &Ctx, id: &str, available: crate::ctx::Field) -> El {
    let c = c.clone();
    Watcher {
        key: c.element_id(id),
        value: c.read(available),
        run: Rc::new(move || {
            if c.read(available) != Value::Bool(false) {
                c.write(available, Value::Bool(false));
            }
        }),
    }
    .into_any_element()
}
