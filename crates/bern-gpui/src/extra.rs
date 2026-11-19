//! High-frequency UI controls for the GPUI runtime:
//! `Tooltip`, `Banner`, `Disclosure`, `NumberInput`, and `Alert`.

use crate::controls::ButtonProps;
use crate::ctx::{Act, Ctx, Model, Press};
use crate::icons;
use crate::session::{Actions, Control};
use crate::{El, Prominence, Role, protocol::Value};
use gpui::prelude::*;
use gpui::{
    App, Context, FontWeight, MouseButton, Render, Window, anchored, deferred, div, point, px,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    static DISCLOSURE_OPEN: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
}

fn disclosure_open(key: &str) -> bool {
    DISCLOSURE_OPEN.with(|map| map.borrow().get(key).copied().unwrap_or(false))
}

fn set_disclosure_open(key: &str, open: bool) {
    DISCLOSURE_OPEN.with(|map| {
        map.borrow_mut().insert(key.to_string(), open);
    });
}

struct TooltipBubble {
    text: String,
    bg: gpui::Rgba,
    border: gpui::Rgba,
    fg: gpui::Rgba,
}

impl Render for TooltipBubble {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
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
    }
}

/// `Tooltip`: attaches native GPUI hover tooltip text to its single child.
pub fn tooltip(c: &Ctx, key: &'static str, text: String, child: El) -> El {
    let bg = c.color("surface.raised");
    let border = c.color("control.border");
    let fg = c.color("text.primary");
    div()
        .id(c.element_id(key))
        .tooltip(move |_window, cx: &mut App| {
            let text = text.clone();
            cx.new(move |_| TooltipBubble {
                text,
                bg,
                border,
                fg,
            })
            .into()
        })
        .child(child)
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
