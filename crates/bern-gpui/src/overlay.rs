//! What shows over the content: the panel of a popover button, the list of
//! a menu button and modals.
//!
//! Whether a popover is open is interface state of its window, kept here
//! by the control's target, so a press, a click outside, `Escape` and an
//! external entry all open and close the same panel, and its content is
//! built (and its controls listed) only while it is open.

use crate::ctx::Ctx;
use crate::layout::{Style, color};
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::prelude::FluentBuilder;
use gpui::{
    Anchor, FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, anchored, deferred, div, point,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

thread_local! {
    /// Targets of the open popovers and menus.
    static OPEN: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

pub(crate) fn is_open(target: &str) -> bool {
    OPEN.with(|open| open.borrow().contains(target))
}

pub(crate) fn set_open(target: &str, open: bool) {
    let changed = OPEN.with(|set| {
        let mut set = set.borrow_mut();
        if open {
            set.insert(target.to_string())
        } else {
            set.remove(target)
        }
    });
    if changed {
        crate::host::redraw();
    }
}

/// Close every popover and menu, as a press outside them does.
pub(crate) fn close_all() {
    let any = OPEN.with(|open| {
        let mut open = open.borrow_mut();
        let any = !open.is_empty();
        open.clear();
        any
    });
    if any {
        crate::host::redraw();
    }
}

/// A panel below its button, over everything else, on the window's
/// raised surface; a press outside it closes it.
pub(crate) fn panel(c: &Ctx, target: &str, content: impl IntoElement) -> impl IntoElement {
    panel_with(c, target, content, None, None)
}

/// A panel that a press outside closes: by `dismiss` when its open state
/// is a model's, else by forgetting it here.
fn panel_with(
    c: &Ctx,
    target: &str,
    content: impl IntoElement,
    dismiss: Option<crate::ctx::Press>,
    style: Option<&'static Style>,
) -> impl IntoElement {
    let close = target.to_string();
    let background = match style.and_then(|style| style.background) {
        Some(background) => color(c, background),
        None => c.slot("menu", "popup", "background"),
    };
    let foreground = match style.and_then(|style| style.foreground) {
        Some(foreground) => color(c, foreground),
        None => c.color("text.primary"),
    };
    deferred(
        anchored()
            .anchor(Anchor::TopLeft)
            .offset(point(px(0.0), px(40.0)))
            .snap_to_window_with_margin(px(8.0))
            .child(
                div()
                    .id(gpui::ElementId::Name(format!("{target}.panel").into()))
                    .occlude()
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(c.slot("menu", "popup", "border"))
                    .bg(background)
                    .text_color(foreground)
                    .shadow_lg()
                    .on_mouse_down_out(move |_, _, cx| match &dismiss {
                        Some(dismiss) => dismiss(cx),
                        None => set_open(&close, false),
                    })
                    .child(content),
            ),
    )
    .with_priority(1)
}

/// The button a popover or menu opens from: a label, an icon or both.
fn opener(
    c: &Ctx,
    id: &str,
    label: String,
    icon: Option<&'static str>,
    chevron: bool,
    open: bool,
) -> gpui::Stateful<gpui::Div> {
    let recipe = "button.standard";
    let foreground = c.slot(recipe, "rest", "foreground");
    let hover = c.slot(recipe, "hover", "background");
    div()
        .id(c.element_id(id))
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(36.0))
        .px(px(12.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(c.slot(recipe, "rest", "border"))
        .bg(if open {
            c.slot(recipe, "pressed", "background")
        } else {
            c.slot(recipe, "rest", "background")
        })
        .text_color(foreground)
        .text_size(px(13.5))
        .font_weight(FontWeight::MEDIUM)
        .hover(move |style| style.bg(hover))
        .children(icon.map(|icon| icons::icon(icon, 16.0, foreground)))
        .child(label)
        .when(chevron, |button| {
            button.child(icons::icon("chevron.down", 12.0, c.color("icon.secondary")))
        })
}

/// `PopoverButton`: a button that opens a panel of content below it.
pub fn popover_button(
    c: &Ctx,
    id: &'static str,
    label: String,
    icon: Option<&'static str>,
    props: PopoverProps,
    content: impl FnOnce() -> Vec<El>,
) -> El {
    let target = c.target(id);
    // Open while its draft or the engine says so, else while it was opened
    // here.
    let open = match &props.presented {
        Some(model) => model.value(c) == crate::protocol::Value::Bool(true),
        None => is_open(&target),
    };
    let enabled = c.enabled();
    let toggle: crate::ctx::Press = match &props.presented {
        Some(model) => {
            let c = c.clone();
            let model = model.clone();
            Rc::new(move |_| model.set(&c, crate::protocol::Value::Bool(!open)))
        }
        None => {
            let target = target.clone();
            Rc::new(move |_| set_open(&target, !is_open(&target)))
        }
    };
    c.list(Control {
        target: target.clone(),
        window: c.win().id.clone(),
        role: "popover_button",
        label: label.clone(),
        value: Some(open.to_string()),
        enabled,
        actions: Actions {
            activate: Some(toggle.clone()),
            ..Actions::default()
        },
    });
    let button = opener(c, id, label, icon, false, open);
    let pressed = Rc::clone(&toggle);
    let button = if enabled {
        button.on_click(move |_, _, cx| pressed(cx))
    } else {
        button.opacity(0.5)
    };
    // The panel in its own appearance, as a dark panel over a light page.
    let panel_ctx = match props.dark {
        Some(dark) => c.with_appearance(dark),
        None => c.clone(),
    };
    let close: Option<crate::ctx::Press> = props.presented.as_ref().map(|_| Rc::clone(&toggle));
    div()
        .relative()
        .flex_none()
        .child(button)
        .when(open, |element| {
            let body = div().flex().flex_col().children(content());
            element.child(panel_with(&panel_ctx, &target, body, close, props.style))
        })
        .into_any_element()
}

/// What opens a popover, and the appearance of its panel.
#[derive(Clone, Default)]
pub struct PopoverProps {
    /// A session field (`presented`) or engine state (`open`) that says
    /// whether it is open, which opening and dismissal change.
    pub presented: Option<crate::Model>,
    /// The panel in the dark (or light) appearance whatever the page's.
    pub dark: Option<bool>,
    /// `style!`'s background and text color of the panel.
    pub style: Option<&'static Style>,
}

/// An item of a menu: its label, whether it is checked and what choosing
/// it does.
pub(crate) type Entry = (String, bool, Rc<dyn Fn()>);

/// A menu button: the chosen item's label, which opens the list of items,
/// each with its label, whether it is checked and what choosing it does.
pub(crate) fn menu_button(c: &Ctx, id: &'static str, label: String, items: Vec<Entry>) -> El {
    let target = c.target(id);
    let open = is_open(&target);
    let enabled = c.enabled();
    let button = opener(c, id, label, None, true, open);
    let button = if enabled {
        let target = target.clone();
        button.on_click(move |_, _, _| set_open(&target, !is_open(&target)))
    } else {
        button.opacity(0.5)
    };
    let hover = c.slot("menu", "hover", "background");
    let check = c.slot("menu", "selected", "check");
    let list = div()
        .flex()
        .flex_col()
        .min_w(px(180.0))
        .p(px(4.0))
        .children(
            items
                .into_iter()
                .enumerate()
                .map(|(index, (text, checked, choose))| {
                    let close = target.clone();
                    div()
                        .id(gpui::ElementId::Name(format!("{target}.{index}").into()))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .h(px(32.0))
                        .px(px(8.0))
                        .rounded(px(6.0))
                        .text_size(px(14.0))
                        .hover(move |style| style.bg(hover))
                        .child(div().size(px(14.0)).flex_none().when(checked, |mark| {
                            mark.child(icons::icon("checkmark", 14.0, check))
                        }))
                        .child(text)
                        .on_click(move |_, _, _| {
                            set_open(&close, false);
                            choose();
                        })
                }),
        );
    div()
        .relative()
        .flex_none()
        .child(button)
        .when(open, |element| element.child(panel(c, &target, list)))
        .into_any_element()
}

/// Size of a `Modal`'s card, and whether it covers a compact window.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModalProps {
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Fill the window when it is compact, as a phone's full-screen flow.
    pub cover_compact: bool,
}

/// `Modal`: its content on a card over a scrim that covers the window,
/// blocking what is under it.
pub fn modal(c: &Ctx, props: ModalProps, content: Vec<El>) -> El {
    let compact = crate::value::truthy(&c.window("compact"));
    let cover = props.cover_compact && compact;
    let window_width = c.win().width.get();
    let window_height = c.win().height.get();
    let card = div()
        .id("bern-modal")
        .occlude()
        .flex()
        .flex_col()
        .flex_none()
        .overflow_hidden()
        .bg(c.slot("dialog", "open", "background"))
        .text_color(c.slot("dialog", "open", "foreground"));
    let card = if cover {
        card.size_full()
    } else {
        let width = props.width.unwrap_or(560.0).min(window_width - 32.0);
        let card = card
            .w(px(width))
            .max_h(px(window_height - 32.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(c.slot("dialog", "open", "border"))
            .shadow_lg();
        match props.height {
            Some(height) => card.h(px(height.min(window_height - 32.0))),
            None => card,
        }
    };
    // Over the whole window, wherever it is declared.
    let top = c.win().title_bar;
    deferred(
        anchored()
            .position_mode(gpui::AnchoredPositionMode::Window)
            .position(point(px(0.0), px(0.0)))
            .child(
                div()
                    .w(px(window_width))
                    .h(px(window_height + top))
                    .pt(px(top))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(c.slot("dialog", "open", "scrim"))
                    .child(card.children(content)),
            ),
    )
    .with_priority(2)
    .into_any_element()
}

/// How a `SplitButton` shows.
#[derive(Clone, Default)]
pub struct SplitProps {
    pub icon: Option<&'static str>,
    pub shape: crate::look::Shape,
    pub height: Option<f32>,
    /// The accessible name of the part that opens the popover.
    pub more_label: String,
    pub shortcut: Option<String>,
}

/// `SplitButton`: a toggle, and beside it a part that shows `content` in a
/// popover.
pub fn split_button(
    c: &Ctx,
    id: &'static str,
    label: String,
    model: crate::Model,
    props: SplitProps,
    content: impl FnOnce() -> Vec<El>,
) -> El {
    let toggle = crate::controls::switch(
        c,
        id,
        label,
        model.clone(),
        crate::controls::SwitchProps {
            style: crate::controls::SwitchStyle::Button,
            shape: props.shape,
            height: props.height,
            icon: props.icon,
            ..Default::default()
        },
    );
    if let Some(shortcut) = props.shortcut.as_deref() {
        let c2 = c.clone();
        let flip: crate::ctx::Press = Rc::new(move |_| {
            let on = model.value(&c2) == crate::protocol::Value::Bool(true);
            model.set(&c2, crate::protocol::Value::Bool(!on));
        });
        crate::controls::register_shortcut(c, Some(shortcut), &flip);
    }
    let more = format!("{id}.more");
    let target = c.target(&more);
    let open = is_open(&target);
    let flip: crate::ctx::Press = {
        let target = target.clone();
        Rc::new(move |_| set_open(&target, !is_open(&target)))
    };
    c.list(Control {
        target: target.clone(),
        window: c.win().id.clone(),
        role: "popover_button",
        label: props.more_label.clone(),
        value: Some(open.to_string()),
        enabled: c.enabled(),
        actions: Actions {
            activate: Some(Rc::clone(&flip)),
            ..Actions::default()
        },
    });
    let (height, _, _) = crate::look::metrics(None, props.shape);
    let height = props.height.unwrap_or(height);
    let hover = c.color("hover.overlay");
    let chevron = div()
        .id(c.element_id(&more))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(height))
        .w(px(height * 0.8))
        .rounded(px(height / 2.0))
        .border_1()
        .border_color(c.slot("button.standard", "rest", "border"))
        .bg(if open {
            c.slot("button.standard", "pressed", "background")
        } else {
            c.slot("button.standard", "rest", "background")
        })
        .hover(move |style| style.bg(hover))
        .child(icons::icon(
            "chevron.down",
            12.0,
            c.slot("button.standard", "rest", "foreground"),
        ))
        .on_click(move |_, _, cx| flip(cx));
    div()
        .relative()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(2.0))
        .child(toggle)
        .child(chevron)
        .when(open, |element| {
            let body = div().flex().flex_col().children(content());
            element.child(panel(c, &target, body))
        })
        .into_any_element()
}
