//! High-frequency UI controls for the GPUI runtime:
//! `Tooltip`, `Banner`, `Disclosure`, `NumberInput`, and `Alert`.

use crate::controls::ButtonProps;
use crate::ctx::{Act, Ctx, Field, Model, Press};
use crate::icons;
use crate::session::{Actions, Control};
use crate::{El, Prominence, Role, protocol::Value};
use gpui::prelude::*;
use gpui::{
    Anchor, AnchoredPositionMode, App, Bounds, Context, FontWeight, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Render, Window, anchored, canvas, deferred, div, point,
    px, relative,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

thread_local! {
    static DISCLOSURE_OPEN: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    static TREE_SELECTION: RefCell<HashMap<String, Value>> = RefCell::new(HashMap::new());
    static TREE_EXPANDED: RefCell<HashMap<String, Vec<Value>>> = RefCell::new(HashMap::new());
    static SPLIT_RATIO: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
    static TOOLTIP_HOVERED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    static TOOLTIP_BOUNDS: RefCell<HashMap<String, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    static REORDER_DRAG: RefCell<Option<(String, usize, usize)>> = const { RefCell::new(None) };
}

fn disclosure_open(key: &str) -> bool {
    DISCLOSURE_OPEN.with(|map| map.borrow().get(key).copied().unwrap_or(false))
}

fn set_disclosure_open(key: &str, open: bool) {
    DISCLOSURE_OPEN.with(|map| {
        map.borrow_mut().insert(key.to_string(), open);
    });
}

/// Properties of a `Tooltip`.
#[derive(Clone, Debug, Default)]
pub struct TooltipProps {
    pub content: String,
    pub keys: Option<String>,
    pub placement: Option<&'static str>,
}

fn format_shortcut_keys(spec: &str) -> String {
    let mut parts: Vec<&str> = spec
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let Some(raw_key) = parts.pop() else {
        return String::new();
    };
    let key_lower = raw_key.to_ascii_lowercase();
    let apple = cfg!(target_os = "macos");
    let key_label = match key_lower.as_str() {
        "space" => crate::text::tr("bern.shortcut.space"),
        "escape" => if apple { "⎋" } else { "Esc" }.to_string(),
        "return" | "enter" => if apple { "↩" } else { "Enter" }.to_string(),
        "tab" => if apple { "⇥" } else { "Tab" }.to_string(),
        "delete" | "backspace" => if apple { "⌫" } else { "Backspace" }.to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "home" => if apple { "↖" } else { "Home" }.to_string(),
        "end" => if apple { "↘" } else { "End" }.to_string(),
        "pageup" => if apple { "⇞" } else { "PgUp" }.to_string(),
        "pagedown" => if apple { "⇟" } else { "PgDn" }.to_string(),
        _ => raw_key.to_uppercase(),
    };
    let lower_mods: Vec<String> = parts.iter().map(|m| m.to_ascii_lowercase()).collect();
    if apple {
        let mut out = String::new();
        for (name, glyph) in [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")] {
            if lower_mods.iter().any(|m| m == name) {
                out.push_str(glyph);
            }
        }
        out.push_str(&key_label);
        out
    } else {
        let mut out = Vec::new();
        if lower_mods.iter().any(|m| m == "cmd" || m == "ctrl") {
            out.push("Ctrl".to_string());
        }
        if lower_mods.iter().any(|m| m == "alt") {
            out.push("Alt".to_string());
        }
        if lower_mods.iter().any(|m| m == "shift") {
            out.push("Shift".to_string());
        }
        out.push(key_label);
        out.join("+")
    }
}

struct TooltipBubble {
    text: String,
    keys: Option<String>,
    bg: gpui::Rgba,
    border: gpui::Rgba,
    fg: gpui::Rgba,
    key_bg: gpui::Rgba,
    key_fg: gpui::Rgba,
}

impl TooltipBubble {
    fn view(&self) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(6.0))
            .bg(self.bg)
            .border_1()
            .border_color(self.border)
            .text_color(self.fg)
            .text_size(px(12.0))
            .shadow_md()
            .child(self.text.clone())
            .when_some(self.keys.as_ref().filter(|k| !k.is_empty()), |el, keys| {
                el.child(
                    div()
                        .px(px(5.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .bg(self.key_bg)
                        .border_1()
                        .border_color(self.border)
                        .text_color(self.key_fg)
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(keys.clone()),
                )
            })
    }
}

impl Render for TooltipBubble {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.view()
    }
}

/// `Tooltip`: attaches a hover tooltip (with optional formatted shortcut keys
/// and `above` / `below` placement) to its single child.
pub fn tooltip(c: &Ctx, key: &'static str, props: TooltipProps, child: El) -> El {
    let bg = c.color("surface.raised");
    let border = c.color("control.border");
    let fg = c.color("text.primary");
    let key_bg = c.color("surface.panel");
    let key_fg = c.color("text.secondary");
    let formatted_keys = props
        .keys
        .as_deref()
        .map(format_shortcut_keys)
        .filter(|s| !s.is_empty());
    let bubble = TooltipBubble {
        text: props.content,
        keys: formatted_keys,
        bg,
        border,
        fg,
        key_bg,
        key_fg,
    };
    let Some(placement) = props.placement else {
        let text = bubble.text;
        let keys = bubble.keys;
        return div()
            .id(c.element_id(key))
            .tooltip(move |_window, cx: &mut App| {
                let text = text.clone();
                let keys = keys.clone();
                cx.new(move |_| TooltipBubble {
                    text,
                    keys,
                    bg,
                    border,
                    fg,
                    key_bg,
                    key_fg,
                })
                .into()
            })
            .child(child)
            .into_any_element();
    };
    let target = c.key(key);
    let hovered = TOOLTIP_HOVERED.with(|set| set.borrow().contains(&target));
    let target_hover = target.clone();
    let target_bounds = target.clone();
    let above = placement != "below";
    let bounds = TOOLTIP_BOUNDS.with(|map| map.borrow().get(&target).copied());
    div()
        .id(c.element_id(key))
        .relative()
        .on_hover(move |inside, _, _| {
            let changed = TOOLTIP_HOVERED.with(|set| {
                let mut set = set.borrow_mut();
                if *inside {
                    set.insert(target_hover.clone())
                } else {
                    set.remove(&target_hover)
                }
            });
            if changed {
                crate::host::redraw();
            }
        })
        .child(
            canvas(
                move |b, _, _| {
                    TOOLTIP_BOUNDS.with(|map| {
                        map.borrow_mut().insert(target_bounds.clone(), b);
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(child)
        .when(hovered, |el| {
            let placed = match bounds {
                Some(rect) => {
                    let gap = px(6.0);
                    let anchor = if above {
                        Anchor::BottomLeft
                    } else {
                        Anchor::TopLeft
                    };
                    let y = if above {
                        rect.top() - gap
                    } else {
                        rect.bottom() + gap
                    };
                    anchored()
                        .position_mode(AnchoredPositionMode::Window)
                        .anchor(anchor)
                        .position(point(rect.left(), y))
                }
                None => anchored()
                    .anchor(if above {
                        Anchor::BottomLeft
                    } else {
                        Anchor::TopLeft
                    })
                    .offset(point(px(0.0), px(if above { -6.0 } else { 6.0 }))),
            };
            el.child(
                deferred(
                    placed
                        .snap_to_window_with_margin(px(8.0))
                        .child(bubble.view()),
                )
                .with_priority(2),
            )
        })
        .into_any_element()
}

pub struct BannerProps {
    pub id: Option<&'static str>,
    pub severity: &'static str,
    pub title: Option<String>,
    pub content: String,
    pub icon: Option<&'static str>,
    pub dismiss_label: Option<String>,
}

/// `Banner`: inline semantic status banner with optional title, actions and dismiss.
pub fn banner(c: &Ctx, props: BannerProps, dismiss: Option<Act>, actions: Vec<El>) -> El {
    let role = match props.severity {
        "success" => "status.success",
        "warning" => "status.warning",
        "error" => "status.error",
        _ => "accent",
    };
    let default_icon = match props.severity {
        "success" => "checkmark.circle",
        "warning" => "exclamationmark.triangle",
        "error" => "xmark.circle.fill",
        _ => "info.circle",
    };
    let tint = c.color(role);
    let mut bg = tint;
    bg.a = 0.12;
    let mut border = tint;
    border.a = 0.28;
    let icon_name = props.icon.unwrap_or(default_icon);
    let mut text_col = div().flex().flex_col().flex_1().min_w_0().gap(px(2.0));
    if let Some(title) = props.title.filter(|t| !t.is_empty()) {
        text_col = text_col.child(
            div()
                .text_size(px(13.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c.color("text.primary"))
                .child(title),
        );
    }
    text_col = text_col.child(
        div()
            .text_size(px(13.0))
            .text_color(c.color("text.secondary"))
            .child(props.content.clone()),
    );
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .gap(px(12.0))
        .px(px(14.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .bg(bg)
        .border_1()
        .border_color(border)
        .child(icons::icon(icon_name, 16.0, tint))
        .child(text_col);
    if !actions.is_empty() {
        row = row.child(
            div()
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(8.0))
                .children(actions),
        );
    }
    if let Some(on_dismiss) = dismiss {
        let close_label = props.dismiss_label.unwrap_or_else(|| props.content.clone());
        let c_run = c.clone();
        let run: Press = Rc::new(move |_| on_dismiss(&c_run));
        if let Some(id) = props.id {
            c.list(Control {
                target: c.target(id),
                window: c.win().id.clone(),
                role: "button",
                label: close_label,
                value: None,
                enabled: c.enabled(),
                actions: Actions {
                    activate: Some(Rc::clone(&run)),
                    ..Actions::default()
                },
            });
        }
        let hover = c.color("hover.overlay");
        let close_id = props.id.unwrap_or("banner.close");
        let close_btn = div()
            .id(c.element_id(close_id))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.0))
            .rounded(px(6.0))
            .hover(move |s| s.bg(hover))
            .child(icons::icon("xmark", 12.0, c.color("icon.secondary")))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                run(cx);
            });
        row = row.child(close_btn);
    }
    row.into_any_element()
}

/// `Disclosure`: collapsible header + content body.
pub fn disclosure(
    c: &Ctx,
    id: &'static str,
    label: String,
    icon: Option<&'static str>,
    model: Option<Model>,
    then: Option<Act>,
    children: impl FnOnce() -> Vec<El>,
) -> El {
    let key = c.key(id);
    let expanded = match &model {
        Some(m) => crate::value::truthy(&m.value(c)),
        None => disclosure_open(&key),
    };
    let set_open: Rc<dyn Fn(bool)> = {
        let c = c.clone();
        let key = key.clone();
        let model = model.clone();
        let then = then.clone();
        Rc::new(move |next: bool| {
            if let Some(m) = &model {
                if crate::value::truthy(&m.value(&c)) != next {
                    m.set(&c, Value::Bool(next));
                }
            } else if disclosure_open(&key) != next {
                set_disclosure_open(&key, next);
                if let Some(act) = &then {
                    act(&c);
                }
            }
        })
    };
    let toggle: Press = {
        let set_open = Rc::clone(&set_open);
        Rc::new(move |_| set_open(!expanded))
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "disclosure",
        label: label.clone(),
        value: Some(if expanded { "true" } else { "false" }.to_string()),
        enabled: c.enabled(),
        actions: Actions {
            activate: Some(Rc::clone(&toggle)),
            set_bool: Some(Rc::clone(&set_open)),
            ..Actions::default()
        },
    });
    let hover = c.color("hover.overlay");
    let chevron = if expanded {
        "chevron.down"
    } else {
        "chevron.right"
    };
    let header = div()
        .id(c.element_id(id))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .gap(px(8.0))
        .px(px(6.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .hover(move |s| s.bg(hover))
        .child(icons::icon(chevron, 12.0, c.color("icon.secondary")))
        .children(icon.map(|ic| icons::icon(ic, 15.0, c.color("icon.primary"))))
        .child(
            div()
                .flex_1()
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c.color("text.primary"))
                .child(label),
        )
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            toggle(cx);
        });
    let mut col = div().flex().flex_col().w_full().gap(px(4.0)).child(header);
    if expanded {
        col = col.child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .pl(px(20.0))
                .pt(px(4.0))
                .gap(px(8.0))
                .children(children()),
        );
    }
    col.into_any_element()
}

pub struct NumberInputProps {
    pub label: String,
    pub label_hidden: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub precision: u32,
    pub enabled: bool,
}

/// `NumberInput`: numeric field with `-` / `+` stepper buttons.
pub fn number_input(c: &Ctx, id: &'static str, props: NumberInputProps, model: Model) -> El {
    let enabled = props.enabled && c.enabled();
    let prec = props.precision.min(6);
    let factor = 10_f64.powi(prec as i32);
    let min = props.min;
    let max = props.max;
    let step = props.step;
    let clamp_round = move |raw: f64| -> f64 {
        let mut v = raw;
        if let Some(lo) = min {
            v = v.max(lo);
        }
        if let Some(hi) = max {
            v = v.min(hi);
        }
        (v * factor).round() / factor
    };
    let current = clamp_round(crate::value::as_f64(&model.value(c)));
    let formatted = if prec == 0 {
        format!("{}", current.round() as i64)
    } else {
        format!("{current:.*}", prec as usize)
    };
    let commit: Rc<dyn Fn(f64)> = {
        let c = c.clone();
        let model = model.clone();
        Rc::new(move |raw: f64| {
            if !raw.is_finite() {
                return;
            }
            let next = clamp_round(raw);
            if (crate::value::as_f64(&model.value(&c)) - next).abs() > 1e-12 {
                model.set(&c, Value::Float(next));
            }
        })
    };
    let commit_edit = Rc::clone(&commit);
    let commit_num = Rc::clone(&commit);
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "number_input",
        label: props.label.clone(),
        value: Some(crate::value::number(current)),
        enabled,
        actions: Actions {
            edit: Some(Rc::new(move |text| {
                if let Ok(v) = text.trim().parse::<f64>() {
                    commit_edit(v);
                }
            })),
            set_number: Some(Rc::new(move |v| commit_num(v))),
            ..Actions::default()
        },
    });
    let hover = c.color("hover.overlay");
    let can_dec = enabled && min.is_none_or(|lo| current > lo);
    let can_inc = enabled && max.is_none_or(|hi| current < hi);
    let dec_commit = Rc::clone(&commit);
    let inc_commit = Rc::clone(&commit);
    let dec_btn = div()
        .id(c.element_id(&format!("{id}.dec")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.panel"))
        .when(can_dec, |el| {
            el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                cx.stop_propagation();
                dec_commit(current - step);
            })
        })
        .when(!can_dec, |el| el.opacity(0.45))
        .child(icons::icon("minus", 12.0, c.color("icon.primary")));
    let inc_btn = div()
        .id(c.element_id(&format!("{id}.inc")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.panel"))
        .when(can_inc, |el| {
            el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                cx.stop_propagation();
                inc_commit(current + step);
            })
        })
        .when(!can_inc, |el| el.opacity(0.45))
        .child(icons::icon("plus", 12.0, c.color("icon.primary")));
    let readout = div()
        .flex()
        .items_center()
        .justify_end()
        .min_w(px(72.0))
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.raised"))
        .text_size(px(13.0))
        .text_color(if enabled {
            c.color("text.primary")
        } else {
            c.color("disabled.foreground")
        })
        .child(formatted);
    let controls_row = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .child(dec_btn)
        .child(readout)
        .child(inc_btn);
    if props.label_hidden {
        return controls_row.into_any_element();
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(c.color("text.primary"))
                .child(props.label),
        )
        .child(controls_row)
        .into_any_element()
}

pub struct AlertActionSpec {
    pub id: &'static str,
    pub label: String,
    pub role: &'static str,
    pub enabled: bool,
    pub action: Option<Act>,
}

/// `Alert`: modal confirmation dialog over a dimmed scrim.
pub fn alert(
    c: &Ctx,
    title: String,
    message: Option<String>,
    actions: Vec<AlertActionSpec>,
    dismiss: Option<Act>,
) -> El {
    let window_width = c.win().width.get();
    let window_height = c.win().height.get();
    let top = c.win().title_bar.get();
    let inset = c.win().inset.get();
    let buttons: Vec<El> = actions
        .into_iter()
        .map(|spec| {
            let (role, prominence) = match spec.role {
                "destructive" => (Role::Destructive, Prominence::Standard),
                "cancel" => (Role::Cancel, Prominence::Standard),
                _ => (Role::Normal, Prominence::Prominent),
            };
            crate::controls::button(
                c,
                spec.id,
                ButtonProps {
                    label: spec.label,
                    role,
                    prominence,
                    enabled: spec.enabled,
                    ..ButtonProps::default()
                },
                spec.action,
            )
        })
        .collect();
    let mut card = div()
        .id("bern-alert")
        .occlude()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .w(px(380.0_f32.min((window_width - 32.0).max(220.0))))
        .p(px(20.0))
        .rounded(px(14.0))
        .border_1()
        .border_color(c.slot("dialog", "open", "border"))
        .bg(c.slot("dialog", "open", "background"))
        .text_color(c.slot("dialog", "open", "foreground"))
        .shadow_lg()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        );
    if let Some(msg) = message.filter(|m| !m.is_empty()) {
        card = card.child(
            div()
                .text_size(px(13.0))
                .text_color(c.color("text.secondary"))
                .child(msg),
        );
    }
    card = card.child(
        div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(8.0))
            .pt(px(6.0))
            .children(buttons),
    );
    let c_dismiss = c.clone();
    let mut scrim = div()
        .id("bern-alert-scrim")
        .w(px(window_width))
        .h(px(window_height + top))
        .pt(px(top))
        .flex()
        .items_center()
        .justify_center()
        .bg(c.slot("dialog", "open", "scrim"))
        .child(card);
    if let Some(on_dismiss) = dismiss {
        scrim = scrim.on_mouse_down(MouseButton::Left, move |_, _, _| on_dismiss(&c_dismiss));
    }
    deferred(
        anchored()
            .position_mode(gpui::AnchoredPositionMode::Window)
            .position(point(px(inset), px(inset)))
            .child(scrim),
    )
    .with_priority(3)
    .into_any_element()
}

fn date_ymd(value: &Value) -> (i64, i64, i64) {
    if let Value::Record(fields) = value
        && fields.len() >= 3
        && let (Value::Int(y), Value::Int(m), Value::Int(d)) = (&fields[0], &fields[1], &fields[2])
    {
        let y = if (1..=9999).contains(y) { *y } else { 2026 };
        let m = (*m).clamp(1, 12);
        let d = (*d).clamp(1, 31);
        return (y, m, d);
    }
    (2026, 1, 1)
}

fn date_value((y, m, d): (i64, i64, i64)) -> Value {
    Value::Record(vec![Value::Int(y), Value::Int(m), Value::Int(d)])
}

fn format_iso_date((y, m, d): (i64, i64, i64)) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 30,
    }
}

fn shift_date((mut y, mut m, mut d): (i64, i64, i64), delta: i64) -> (i64, i64, i64) {
    d += delta;
    while d > days_in_month(y, m) {
        d -= days_in_month(y, m);
        m += 1;
        if m > 12 {
            m = 1;
            y = (y + 1).min(9999);
        }
    }
    while d < 1 {
        m -= 1;
        if m < 1 {
            m = 12;
            y = (y - 1).max(1);
        }
        d += days_in_month(y, m);
    }
    (y, m, d)
}

fn parse_iso_date(raw: &str) -> Option<(i64, i64, i64)> {
    let mut parts = raw.trim().split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=9999).contains(&y) || !(1..=12).contains(&m) {
        return None;
    }
    let max_d = days_in_month(y, m);
    if !(1..=max_d).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

pub struct DatePickerProps {
    pub label: String,
    pub label_hidden: bool,
    pub min: Option<Value>,
    pub max: Option<Value>,
    pub enabled: bool,
}

/// `DatePicker`: calendar date stepper and ISO date editor.
pub fn date_picker(c: &Ctx, id: &'static str, props: DatePickerProps, model: Model) -> El {
    let enabled = props.enabled && c.enabled();
    let min_ymd = props.min.as_ref().map(date_ymd);
    let max_ymd = props.max.as_ref().map(date_ymd);
    let clamp = move |mut ymd: (i64, i64, i64)| -> (i64, i64, i64) {
        if let Some(lo) = min_ymd
            && ymd < lo
        {
            ymd = lo;
        }
        if let Some(hi) = max_ymd
            && ymd > hi
        {
            ymd = hi;
        }
        ymd
    };
    let current = clamp(date_ymd(&model.value(c)));
    let c_edit = c.clone();
    let m_edit = model.clone();
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "date_picker",
        label: props.label.clone(),
        value: Some(format_iso_date(current)),
        enabled,
        actions: Actions {
            edit: Some(Rc::new(move |text| {
                if let Some(parsed) = parse_iso_date(&text) {
                    let next = clamp(parsed);
                    m_edit.set(&c_edit, date_value(next));
                }
            })),
            ..Actions::default()
        },
    });
    let hover = c.color("hover.overlay");
    let c_dec = c.clone();
    let m_dec = model.clone();
    let dec = div()
        .id(c.element_id(&format!("{id}.prev")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.panel"))
        .child(icons::icon("chevron.left", 12.0, c.color("icon.primary")))
        .when(enabled, |el| {
            el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                cx.stop_propagation();
                let next = clamp(shift_date(current, -1));
                m_dec.set(&c_dec, date_value(next));
            })
        })
        .when(!enabled, |el| el.opacity(0.45));
    let c_inc = c.clone();
    let m_inc = model.clone();
    let inc = div()
        .id(c.element_id(&format!("{id}.next")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.panel"))
        .child(icons::icon("chevron.right", 12.0, c.color("icon.primary")))
        .when(enabled, |el| {
            el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                cx.stop_propagation();
                let next = clamp(shift_date(current, 1));
                m_inc.set(&c_inc, date_value(next));
            })
        })
        .when(!enabled, |el| el.opacity(0.45));
    let readout = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.raised"))
        .text_size(px(13.0))
        .text_color(c.color("text.primary"))
        .child(icons::icon("calendar", 13.0, c.color("icon.secondary")))
        .child(format_iso_date(current));
    let controls_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(dec)
        .child(readout)
        .child(inc);
    if props.label_hidden {
        return controls_row.into_any_element();
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(c.color("text.primary"))
                .child(props.label),
        )
        .child(controls_row)
        .into_any_element()
}

fn clamp_time_minutes(raw: f64, lo: f64, hi: f64, step: f64) -> f64 {
    if !raw.is_finite() {
        return lo;
    }
    let s = step.round().max(1.0);
    let clamped = raw.clamp(lo, hi);
    let snapped = lo + ((clamped - lo) / s).round() * s;
    snapped.round().clamp(lo, hi)
}

fn format_time_minutes(mins: f64) -> String {
    let total = if mins.is_finite() {
        mins.round().clamp(0.0, 1439.0) as i32
    } else {
        0
    };
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn parse_time_minutes(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if let Some((h, m)) = trimmed.split_once(':') {
        let hh: i32 = h.trim().parse().ok()?;
        let mm: i32 = m.trim().parse().ok()?;
        if (0..=23).contains(&hh) && (0..=59).contains(&mm) {
            return Some(f64::from(hh * 60 + mm));
        }
    }
    trimmed.parse::<f64>().ok().filter(|v| v.is_finite())
}

pub struct TimePickerProps {
    pub label: String,
    pub label_hidden: bool,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub enabled: bool,
}

/// `TimePicker`: time-of-day stepper storing minutes since midnight (0..=1439).
pub fn time_picker(c: &Ctx, id: &'static str, props: TimePickerProps, model: Model) -> El {
    let enabled = props.enabled && c.enabled();
    let lo = props.min.clamp(0.0, 1439.0);
    let hi = props.max.clamp(lo, 1439.0);
    let step = props.step.round().clamp(1.0, 60.0);
    let current = clamp_time_minutes(crate::value::as_f64(&model.value(c)), lo, hi, step);
    let c_edit = c.clone();
    let m_edit = model.clone();
    let c_num = c.clone();
    let m_num = model.clone();
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "time_picker",
        label: props.label.clone(),
        value: Some(format_time_minutes(current)),
        enabled,
        actions: Actions {
            edit: Some(Rc::new(move |text| {
                if let Some(mins) = parse_time_minutes(&text) {
                    m_edit.set(
                        &c_edit,
                        Value::Float(clamp_time_minutes(mins, lo, hi, step)),
                    );
                }
            })),
            set_number: Some(Rc::new(move |v| {
                if v.is_finite() {
                    m_num.set(&c_num, Value::Float(clamp_time_minutes(v, lo, hi, step)));
                }
            })),
            ..Actions::default()
        },
    });
    let hover = c.color("hover.overlay");
    let c_dec = c.clone();
    let m_dec = model.clone();
    let dec = div()
        .id(c.element_id(&format!("{id}.dec")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.panel"))
        .child(icons::icon("minus", 12.0, c.color("icon.primary")))
        .when(enabled, |el| {
            el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                cx.stop_propagation();
                m_dec.set(
                    &c_dec,
                    Value::Float(clamp_time_minutes(current - step, lo, hi, step)),
                );
            })
        })
        .when(!enabled, |el| el.opacity(0.45));
    let c_inc = c.clone();
    let m_inc = model.clone();
    let inc = div()
        .id(c.element_id(&format!("{id}.inc")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.panel"))
        .child(icons::icon("plus", 12.0, c.color("icon.primary")))
        .when(enabled, |el| {
            el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                cx.stop_propagation();
                m_inc.set(
                    &c_inc,
                    Value::Float(clamp_time_minutes(current + step, lo, hi, step)),
                );
            })
        })
        .when(!enabled, |el| el.opacity(0.45));
    let readout = div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(68.0))
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(c.color("control.border"))
        .bg(c.color("surface.raised"))
        .text_size(px(13.0))
        .text_color(c.color("text.primary"))
        .child(format_time_minutes(current));
    let controls_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(dec)
        .child(readout)
        .child(inc);
    if props.label_hidden {
        return controls_row.into_any_element();
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(c.color("text.primary"))
                .child(props.label),
        )
        .child(controls_row)
        .into_any_element()
}

/// `EmptyState`: centered placeholder view with icon, title, message, and actions.
pub fn empty_state(
    c: &Ctx,
    title: String,
    message: Option<String>,
    icon: Option<&'static str>,
    actions: Vec<El>,
) -> El {
    let mut col = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .w_full()
        .p(px(24.0))
        .gap(px(10.0));
    if let Some(name) = icon {
        col = col.child(icons::icon(name, 36.0, c.color("icon.secondary")));
    }
    col = col.child(
        div()
            .text_size(px(16.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c.color("text.primary"))
            .child(title),
    );
    if let Some(msg) = message.filter(|m| !m.is_empty()) {
        col = col.child(
            div()
                .max_w(px(360.0))
                .text_size(px(13.0))
                .text_color(c.color("text.secondary"))
                .child(msg),
        );
    }
    if !actions.is_empty() {
        col = col.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .pt(px(4.0))
                .children(actions),
        );
    }
    col.into_any_element()
}

/// `Badge`: compact semantic status capsule.
pub fn badge(c: &Ctx, content: String, tone: &'static str, icon: Option<&'static str>) -> El {
    let (fg, base, neutral) = match tone {
        "accent" => (c.color("accent"), c.color("accent"), false),
        "success" => (c.color("status.success"), c.color("status.success"), false),
        "warning" => (c.color("status.warning"), c.color("status.warning"), false),
        "error" => (c.color("status.error"), c.color("status.error"), false),
        _ => (c.color("text.secondary"), c.color("text.primary"), true),
    };
    let mut bg = base;
    bg.a = if neutral { 0.08 } else { 0.14 };
    let mut border = base;
    border.a = if neutral { 0.16 } else { 0.28 };
    let mut pill = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg);
    if let Some(name) = icon {
        pill = pill.child(icons::icon(name, 11.0, fg));
    }
    pill.child(content).into_any_element()
}

fn avatar_initials(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "?".into();
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() >= 2
        && let (Some(a), Some(b)) = (words[0].chars().next(), words[1].chars().next())
    {
        return format!("{a}{b}").to_uppercase();
    }
    let chars: Vec<char> = trimmed.chars().take(2).collect();
    if chars.len() == 2 && chars[0].is_ascii() && chars[1].is_ascii() {
        return format!("{}{}", chars[0], chars[1]).to_uppercase();
    }
    chars[0].to_uppercase().to_string()
}

pub struct AvatarProps {
    pub label: String,
    pub resource: Option<&'static str>,
    pub url: Option<String>,
    pub size: f32,
    pub rounded: bool,
}

/// `Avatar`: circular or rounded avatar with optional image and initials fallback.
pub fn avatar(c: &Ctx, props: AvatarProps) -> El {
    let side = props.size.max(8.0);
    let radius = if props.rounded {
        (side * 0.22).max(4.0)
    } else {
        side * 0.5
    };
    const PALETTE: [(f32, f32, f32); 6] = [
        (0.23, 0.51, 0.96),
        (0.06, 0.73, 0.51),
        (0.96, 0.62, 0.04),
        (0.55, 0.36, 0.96),
        (0.93, 0.28, 0.60),
        (0.08, 0.72, 0.65),
    ];
    let mut hash: u64 = 14695981039346656037;
    for b in props.label.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(1099511628211);
    }
    let (r, g, b) = PALETTE[(hash as usize) % PALETTE.len()];
    let fg = gpui::Rgba { r, g, b, a: 1.0 };
    let bg = gpui::Rgba { r, g, b, a: 0.22 };
    let mut frame = div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .w(px(side))
        .h(px(side))
        .rounded(px(radius))
        .overflow_hidden()
        .bg(bg)
        .text_size(px((side * 0.38).max(10.0)))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(avatar_initials(&props.label));
    if let Some(res) = props.resource {
        frame = frame.child(div().absolute().inset_0().child(crate::content::image(
            c,
            res,
            None,
            Some(side),
            Some(side),
            Some(radius),
            crate::content::Fit::Cover,
            false,
            None,
            None,
            None,
            false,
        )));
    } else if let Some(url) = props.url.filter(|u| !u.is_empty()) {
        frame = frame.child(div().absolute().inset_0().child(crate::content::image_url(
            c,
            url,
            Some(side),
            Some(side),
            Some(radius),
            crate::content::Fit::Cover,
            false,
            None,
            None,
            false,
        )));
    }
    frame.into_any_element()
}

pub struct TreeViewProps {
    pub label: String,
    pub key_field: usize,
    pub parent_field: usize,
    pub selection: Option<Field>,
    pub expanded: Option<Field>,
}

/// `TreeView`: hierarchical expandable tree with selection and keyboard/CLI support.
pub fn tree_view(
    c: &Ctx,
    id: &'static str,
    props: TreeViewProps,
    items: Vec<Value>,
    row: &dyn Fn(&Ctx, Value) -> El,
    action: Option<Act>,
) -> El {
    let store_key = c.key(id);
    let key_of = |item: &Value| crate::value::at(item.clone(), &[props.key_field]);
    let parent_of =
        |item: &Value| crate::value::some(crate::value::at(item.clone(), &[props.parent_field]));

    let selected_val: Option<Value> = match props.selection {
        Some(f) => {
            let v = c.read(f);
            if matches!(v, Value::Unit) {
                None
            } else {
                Some(v)
            }
        }
        None => TREE_SELECTION.with(|m| m.borrow().get(&store_key).cloned()),
    };
    let expanded_list: Vec<Value> = match props.expanded {
        Some(f) => crate::value::as_list(c.read(f)),
        None => TREE_EXPANDED.with(|m| m.borrow().get(&store_key).cloned().unwrap_or_default()),
    };
    let expanded_keys: HashSet<String> = expanded_list.iter().map(crate::value::key_text).collect();

    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        by_key.insert(crate::value::key_text(&key_of(item)), idx);
    }
    let mut children_of: HashMap<String, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match parent_of(item) {
            Some(pid) => {
                let pk = crate::value::key_text(&pid);
                if by_key.contains_key(&pk) {
                    children_of.entry(pk).or_default().push(idx);
                } else {
                    roots.push(idx);
                }
            }
            None => roots.push(idx),
        }
    }

    struct FlatRow {
        idx: usize,
        key: Value,
        key_text: String,
        depth: usize,
        has_children: bool,
        open: bool,
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        idx: usize,
        depth: usize,
        items: &[Value],
        key_field: usize,
        children_of: &HashMap<String, Vec<usize>>,
        expanded_keys: &HashSet<String>,
        visited: &mut HashSet<String>,
        out: &mut Vec<FlatRow>,
    ) {
        let key = crate::value::at(items[idx].clone(), &[key_field]);
        let kt = crate::value::key_text(&key);
        if !visited.insert(kt.clone()) {
            return;
        }
        let kids = children_of.get(&kt);
        let has_children = kids.is_some_and(|k| !k.is_empty());
        let open = has_children && expanded_keys.contains(&kt);
        out.push(FlatRow {
            idx,
            key,
            key_text: kt.clone(),
            depth,
            has_children,
            open,
        });
        if open && let Some(kids) = kids {
            for &kid in kids {
                walk(
                    kid,
                    depth + 1,
                    items,
                    key_field,
                    children_of,
                    expanded_keys,
                    visited,
                    out,
                );
            }
        }
    }
    let mut flat = Vec::new();
    let mut visited = HashSet::new();
    for root_idx in roots {
        walk(
            root_idx,
            0,
            &items,
            props.key_field,
            &children_of,
            &expanded_keys,
            &mut visited,
            &mut flat,
        );
    }

    let select_key: Rc<dyn Fn(Value)> = {
        let c = c.clone();
        let store_key = store_key.clone();
        let sel_field = props.selection;
        Rc::new(move |k: Value| {
            if let Some(f) = sel_field {
                c.write(f, k);
            } else {
                TREE_SELECTION.with(|m| {
                    m.borrow_mut().insert(store_key.clone(), k);
                });
                crate::host::redraw();
            }
        })
    };
    let toggle_expand: Rc<dyn Fn(Value)> = {
        let c = c.clone();
        let store_key = store_key.clone();
        let exp_field = props.expanded;
        Rc::new(move |k: Value| {
            let kt = crate::value::key_text(&k);
            let mut cur = match exp_field {
                Some(f) => crate::value::as_list(c.read(f)),
                None => {
                    TREE_EXPANDED.with(|m| m.borrow().get(&store_key).cloned().unwrap_or_default())
                }
            };
            if let Some(pos) = cur.iter().position(|v| crate::value::key_text(v) == kt) {
                cur.remove(pos);
            } else {
                cur.push(k);
            }
            if let Some(f) = exp_field {
                c.write(f, Value::List(cur));
            } else {
                TREE_EXPANDED.with(|m| {
                    m.borrow_mut().insert(store_key.clone(), cur);
                });
                crate::host::redraw();
            }
        })
    };

    let key_lookup: Vec<(String, Value)> = items
        .iter()
        .map(|item| {
            let k = key_of(item);
            (crate::value::key_text(&k), k)
        })
        .collect();
    let cli_select = {
        let lookup = key_lookup.clone();
        let select_key = Rc::clone(&select_key);
        Rc::new(move |text: &str| {
            let wanted = text.trim();
            if let Some((_, k)) = lookup.iter().find(|(kt, _)| kt == wanted) {
                select_key(k.clone());
                true
            } else {
                false
            }
        })
    };
    let cli_edit = {
        let lookup = key_lookup;
        let toggle_expand = Rc::clone(&toggle_expand);
        Rc::new(move |text: String| {
            let wanted = text.trim();
            if let Some((_, k)) = lookup.iter().find(|(kt, _)| kt == wanted) {
                toggle_expand(k.clone());
            }
        })
    };
    let cli_activate = action.as_ref().map(|act| {
        let act = Rc::clone(act);
        let c = c.clone();
        Rc::new(move |_: &mut App| act(&c)) as Press
    });

    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "tree_view",
        label: props.label,
        value: selected_val.as_ref().map(crate::value::key_text),
        enabled: c.enabled(),
        actions: Actions {
            activate: cli_activate,
            select: Some(cli_select),
            edit: Some(cli_edit),
            ..Actions::default()
        },
    });

    let hover = c.color("hover.overlay");
    let selected_bg = c.color("selection.background");
    let selected_text = selected_val.as_ref().map(crate::value::key_text);
    let mut rows_els: Vec<El> = Vec::with_capacity(flat.len());
    for entry in flat {
        let is_selected = selected_text.as_deref() == Some(entry.key_text.as_str());
        let row_ctx = c.with_key(entry.key_text.clone());
        let content = row(&row_ctx, items[entry.idx].clone());
        let chevron: El = if entry.has_children {
            let k_toggle = entry.key.clone();
            let toggle = Rc::clone(&toggle_expand);
            div()
                .id(c.element_id(&format!("{id}.toggle.{}", entry.key_text)))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(20.0))
                .rounded(px(4.0))
                .hover(move |s| s.bg(hover))
                .child(icons::icon(
                    if entry.open {
                        "chevron.down"
                    } else {
                        "chevron.right"
                    },
                    12.0,
                    c.color("icon.secondary"),
                ))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    toggle(k_toggle.clone());
                })
                .into_any_element()
        } else {
            div().flex_none().size(px(20.0)).into_any_element()
        };
        let k_sel = entry.key.clone();
        let select = Rc::clone(&select_key);
        let row_act = action.clone();
        let c_act = c.clone();
        let mut row_div = div()
            .id(c.element_id(&format!("{id}.row.{}", entry.key_text)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .min_h(px(30.0))
            .pl(px(8.0 + entry.depth as f32 * 18.0))
            .pr(px(8.0))
            .py(px(4.0))
            .gap(px(6.0))
            .rounded(px(6.0))
            .child(chevron)
            .child(div().flex_1().min_w(px(0.0)).child(content))
            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                select(k_sel.clone());
                if event.click_count >= 2
                    && let Some(act) = &row_act
                {
                    act(&c_act);
                }
            });
        row_div = if is_selected {
            row_div.bg(selected_bg)
        } else {
            row_div.hover(move |s| s.bg(hover))
        };
        rows_els.push(row_div.into_any_element());
    }
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(2.0))
        .children(rows_els)
        .into_any_element()
}

pub struct SplitPaneProps {
    pub id: Option<&'static str>,
    pub vertical: bool,
    pub ratio: Option<Field>,
    pub min_first: f32,
    pub min_second: f32,
}

/// `SplitPane`: resizable two-pane split container with draggable divider.
pub fn split_pane(c: &Ctx, key: &'static str, props: SplitPaneProps, first: El, second: El) -> El {
    let store_key = c.key(props.id.unwrap_or(key));
    let clamp_ratio = |raw: f64, total: f64| -> f64 {
        let mut r = if raw.is_finite() { raw } else { 0.5 };
        let mut lo = 0.05_f64;
        let mut hi = 0.95_f64;
        let min_a = f64::from(props.min_first.max(0.0));
        let min_b = f64::from(props.min_second.max(0.0));
        if total > min_a + min_b && total > 0.0 {
            lo = lo.max(min_a / total);
            hi = hi.min(1.0 - min_b / total);
        }
        r = r.clamp(lo, hi);
        r
    };
    let current = match props.ratio {
        Some(f) => clamp_ratio(crate::value::as_f64(&c.read(f)), 0.0),
        None => clamp_ratio(
            SPLIT_RATIO.with(|m| m.borrow().get(&store_key).copied().unwrap_or(0.5)),
            0.0,
        ),
    };
    let write_ratio: Rc<dyn Fn(f64)> = {
        let c = c.clone();
        let store_key = store_key.clone();
        let field = props.ratio;
        Rc::new(move |next: f64| {
            if let Some(f) = field {
                c.write(f, Value::Float(next));
            } else {
                SPLIT_RATIO.with(|m| {
                    m.borrow_mut().insert(store_key.clone(), next);
                });
                crate::host::redraw();
            }
        })
    };
    if let Some(id) = props.id {
        let write_cli = Rc::clone(&write_ratio);
        let min_first = props.min_first;
        let min_second = props.min_second;
        c.list(Control {
            target: c.target(id),
            window: c.win().id.clone(),
            role: "split_pane",
            label: id.to_string(),
            value: Some(format!("{current:.4}")),
            enabled: c.enabled(),
            actions: Actions {
                set_number: Some(Rc::new(move |v| {
                    if v.is_finite() {
                        let _ = (min_first, min_second);
                        write_cli(v.clamp(0.05, 0.95));
                    }
                })),
                ..Actions::default()
            },
        });
    }

    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let dragging = Rc::new(Cell::new(false));
    let measured = Rc::clone(&bounds);
    let drag_down = Rc::clone(&dragging);
    let drag_move = Rc::clone(&dragging);
    let drag_up = Rc::clone(&dragging);
    let drag_up_out = Rc::clone(&dragging);
    let bounds_move = Rc::clone(&bounds);
    let write_move = Rc::clone(&write_ratio);
    let vertical = props.vertical;
    let min_first = props.min_first;
    let min_second = props.min_second;

    let ratio_at = move |pos: gpui::Point<Pixels>, b: Bounds<Pixels>| -> f64 {
        let total = if vertical {
            f64::from(b.size.height)
        } else {
            f64::from(b.size.width)
        };
        if total <= 1.0 {
            return current;
        }
        let offset = if vertical {
            f64::from(pos.y - b.origin.y)
        } else {
            f64::from(pos.x - b.origin.x)
        };
        let min_a = f64::from(min_first.max(0.0));
        let min_b = f64::from(min_second.max(0.0));
        let (lo, hi) = if total > min_a + min_b {
            ((min_a / total).max(0.05), (1.0 - min_b / total).min(0.95))
        } else {
            (0.05, 0.95)
        };
        (offset / total).clamp(lo, hi)
    };

    let sep = c.color("separator");
    let accent = c.color("accent");
    let handle = div()
        .id(c.element_id(&format!("{key}.handle")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .when(vertical, |el| el.w_full().h(px(8.0)))
        .when(!vertical, |el| el.h_full().w(px(8.0)))
        .child(
            div()
                .bg(sep)
                .when(vertical, |el| el.w_full().h(px(1.0)))
                .when(!vertical, |el| el.h_full().w(px(1.0))),
        )
        .hover(move |s| s.bg(gpui::Rgba { a: 0.14, ..accent }))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            drag_down.set(true);
        });

    let pane_a = div()
        .flex()
        .flex_col()
        .flex_none()
        .overflow_hidden()
        .when(vertical, |el| {
            el.w_full()
                .h(relative(current as f32))
                .min_h(px(props.min_first.max(0.0)))
        })
        .when(!vertical, |el| {
            el.h_full()
                .w(relative(current as f32))
                .min_w(px(props.min_first.max(0.0)))
        })
        .child(first);

    let pane_b = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(if vertical {
            0.0
        } else {
            props.min_second.max(0.0)
        }))
        .min_h(px(if vertical {
            props.min_second.max(0.0)
        } else {
            0.0
        }))
        .overflow_hidden()
        .child(second);

    div()
        .id(c.element_id(key))
        .relative()
        .flex()
        .when(vertical, |el| el.flex_col())
        .when(!vertical, |el| el.flex_row())
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .child(
            canvas(move |b, _, _| measured.set(b), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(pane_a)
        .child(handle)
        .child(pane_b)
        .on_mouse_move(move |event: &MouseMoveEvent, _, _| {
            if drag_move.get() && event.pressed_button == Some(MouseButton::Left) {
                write_move(ratio_at(event.position, bounds_move.get()));
            }
        })
        .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, _, _| {
            drag_up.set(false);
        })
        .on_mouse_up_out(MouseButton::Left, move |_: &MouseUpEvent, _, _| {
            drag_up_out.set(false);
        })
        .into_any_element()
}

pub struct ReorderableProps {
    pub label: String,
    pub key_field: usize,
    pub from: Field,
    pub to: Field,
}

/// `Reorderable`: reorderable vertical list with pointer drag-to-reorder,
/// move up/down buttons, horizontal row content (`Spacer`-aware), and CLI `edit("from:to")`.
pub fn reorderable(
    c: &Ctx,
    id: &'static str,
    props: ReorderableProps,
    items: Vec<Value>,
    row: &dyn Fn(&Ctx, Value) -> Vec<El>,
    on_move: Option<Act>,
) -> El {
    let list_key = c.key(id);
    let count = items.len();
    let commit_move: Rc<dyn Fn(usize, usize)> = {
        let c = c.clone();
        let from = props.from;
        let to = props.to;
        let on_move = on_move.clone();
        Rc::new(move |src: usize, dst: usize| {
            if src >= count || dst >= count || src == dst {
                return;
            }
            c.write(from, Value::Int(src as i64));
            c.write(to, Value::Int(dst as i64));
            if let Some(act) = &on_move {
                act(&c);
            }
        })
    };
    let cli_edit = {
        let commit = Rc::clone(&commit_move);
        Rc::new(move |text: String| {
            let trimmed = text.trim();
            let parts: Vec<&str> = trimmed
                .split([':', ',', '-', '>'])
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() == 2
                && let (Ok(src), Ok(dst)) = (parts[0].trim().parse(), parts[1].trim().parse())
            {
                commit(src, dst);
            }
        })
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "reorderable",
        label: props.label,
        value: Some(count.to_string()),
        enabled: c.enabled(),
        actions: Actions {
            edit: Some(cli_edit),
            ..Actions::default()
        },
    });

    let active_drag = REORDER_DRAG.with(|drag| {
        drag.borrow()
            .as_ref()
            .filter(|(k, _, _)| *k == list_key)
            .map(|(_, from, over)| (*from, *over))
    });
    let hover = c.color("hover.overlay");
    let accent = c.color("accent");
    let mut rows_els: Vec<El> = Vec::with_capacity(count);
    for (idx, item) in items.into_iter().enumerate() {
        let kt = crate::value::key_text(&crate::value::at(item.clone(), &[props.key_field]));
        let row_ctx = c.with_key(kt.clone());
        let content = row(&row_ctx, item);
        let can_up = idx > 0;
        let can_down = idx + 1 < count;
        let commit_up = Rc::clone(&commit_move);
        let commit_down = Rc::clone(&commit_move);
        let up_btn = div()
            .id(c.element_id(&format!("{id}.up.{kt}")))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.0))
            .rounded(px(5.0))
            .child(icons::icon("chevron.up", 12.0, c.color("icon.secondary")))
            .when(can_up, |el| {
                el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    commit_up(idx, idx - 1);
                })
            })
            .when(!can_up, |el| el.opacity(0.35));
        let down_btn = div()
            .id(c.element_id(&format!("{id}.down.{kt}")))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.0))
            .rounded(px(5.0))
            .child(icons::icon("chevron.down", 12.0, c.color("icon.secondary")))
            .when(can_down, |el| {
                el.hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    commit_down(idx, idx + 1);
                })
            })
            .when(!can_down, |el| el.opacity(0.35));
        let is_source = active_drag.is_some_and(|(from, _)| from == idx);
        let is_target = active_drag.is_some_and(|(from, over)| from != over && over == idx);
        let drag_down_key = list_key.clone();
        let drag_move_key = list_key.clone();
        let drag_up_key = list_key.clone();
        let commit_drop = Rc::clone(&commit_move);
        rows_els.push(
            div()
                .id(c.element_id(&format!("{id}.row.{kt}")))
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .px(px(10.0))
                .py(px(6.0))
                .gap(px(8.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if is_target {
                    accent
                } else {
                    c.color("control.border")
                })
                .bg(c.color("surface.panel"))
                .when(is_source, |el| el.opacity(0.6))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    REORDER_DRAG.with(|drag| {
                        *drag.borrow_mut() = Some((drag_down_key.clone(), idx, idx));
                    });
                    crate::host::redraw();
                })
                .on_mouse_move(move |event: &MouseMoveEvent, _, _| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        return;
                    }
                    let changed = REORDER_DRAG.with(|drag| {
                        let mut drag = drag.borrow_mut();
                        if let Some((key, _, over)) = drag.as_mut()
                            && *key == drag_move_key
                            && *over != idx
                        {
                            *over = idx;
                            return true;
                        }
                        false
                    });
                    if changed {
                        crate::host::redraw();
                    }
                })
                .on_mouse_up(MouseButton::Left, move |_, _, _| {
                    let finished = REORDER_DRAG.with(|drag| {
                        let mut drag = drag.borrow_mut();
                        if drag.as_ref().is_some_and(|(key, _, _)| *key == drag_up_key) {
                            drag.take()
                        } else {
                            None
                        }
                    });
                    if let Some((_, from, to)) = finished {
                        if from != to {
                            commit_drop(from, to);
                        } else {
                            crate::host::redraw();
                        }
                    }
                })
                .child(icons::icon(
                    "chevron.up.chevron.down",
                    13.0,
                    c.color("icon.secondary"),
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .flex_1()
                        .min_w(px(0.0))
                        .children(content),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(2.0))
                        .child(up_btn)
                        .child(down_btn),
                )
                .into_any_element(),
        );
    }
    let cancel_key = list_key.clone();
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(4.0))
        .on_mouse_up_out(MouseButton::Left, move |_, _, _| {
            let cleared = REORDER_DRAG.with(|drag| {
                let mut drag = drag.borrow_mut();
                if drag.as_ref().is_some_and(|(key, _, _)| *key == cancel_key) {
                    drag.take().is_some()
                } else {
                    false
                }
            });
            if cleared {
                crate::host::redraw();
            }
        })
        .children(rows_els)
        .into_any_element()
}

pub struct BottomSheetProps {
    pub detents: &'static str,
    pub drag_indicator: bool,
}

/// `BottomSheet`: bottom-anchored overlay sheet over a dimmed scrim.
pub fn bottom_sheet(
    c: &Ctx,
    props: BottomSheetProps,
    children: Vec<El>,
    dismiss: Option<Act>,
) -> El {
    let window_width = c.win().width.get();
    let window_height = c.win().height.get();
    let top = c.win().title_bar.get();
    let inset = c.win().inset.get();
    let max_h = match props.detents {
        "medium" => (window_height * 0.55).max(180.0),
        _ => (window_height * 0.88).max(240.0),
    };
    let mut sheet = div()
        .id("bern-bottom-sheet")
        .occlude()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .w(px(640.0_f32.min(window_width.max(240.0))))
        .max_h(px(max_h))
        .px(px(18.0))
        .pt(px(10.0))
        .pb(px(20.0))
        .rounded_t(px(18.0))
        .border_t_1()
        .border_l_1()
        .border_r_1()
        .border_color(c.slot("dialog", "open", "border"))
        .bg(c.slot("dialog", "open", "background"))
        .text_color(c.slot("dialog", "open", "foreground"))
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if props.drag_indicator {
        sheet = sheet.child(
            div().flex().justify_center().w_full().pb(px(4.0)).child(
                div()
                    .w(px(36.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(c.color("control.border")),
            ),
        );
    }
    sheet = sheet.children(children);

    let c_dismiss = c.clone();
    let mut scrim = div()
        .id("bern-bottom-sheet-scrim")
        .w(px(window_width))
        .h(px(window_height + top))
        .pt(px(top))
        .flex()
        .items_end()
        .justify_center()
        .bg(c.slot("dialog", "open", "scrim"))
        .child(sheet);
    if let Some(on_dismiss) = dismiss {
        scrim = scrim.on_mouse_down(MouseButton::Left, move |_, _, _| on_dismiss(&c_dismiss));
    }
    deferred(
        anchored()
            .position_mode(gpui::AnchoredPositionMode::Window)
            .position(point(px(inset), px(inset)))
            .child(scrim),
    )
    .with_priority(3)
    .into_any_element()
}

/// `Refreshable`: wraps scrollable content with a refresh indicator and CLI activate trigger.
pub fn refreshable(
    c: &Ctx,
    id: &'static str,
    refreshing: bool,
    child: El,
    on_refresh: Option<Act>,
) -> El {
    let cli_activate = on_refresh.map(|act| {
        let c = c.clone();
        Rc::new(move |_: &mut App| act(&c)) as Press
    });
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "refreshable",
        label: id.to_string(),
        value: Some(if refreshing { "true" } else { "false" }.to_string()),
        enabled: c.enabled(),
        actions: Actions {
            activate: cli_activate,
            ..Actions::default()
        },
    });
    let mut col = div().flex().flex_col().w_full().min_w(px(0.0));
    if refreshing {
        col = col.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .w_full()
                .py(px(6.0))
                .gap(px(6.0))
                .child(icons::icon("arrow.clockwise", 14.0, c.color("accent"))),
        );
    }
    col.child(child).into_any_element()
}
