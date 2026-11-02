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
    Anchor, AnchoredPositionMode, Bounds, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, anchored, deferred, div, point,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

thread_local! {
    /// Targets of the open popovers and menus.
    static OPEN: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    /// Where each popover's and menu's button was laid out, in its window.
    static OPENERS: RefCell<HashMap<String, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

/// Notes where the button of the popover or menu `target` is, for its
/// panel to stand by it; laid over the button, as large as it.
pub(crate) fn opener_place(target: &str) -> impl IntoElement {
    let target = target.to_string();
    gpui::canvas(
        move |bounds, _, _| {
            OPENERS.with(|openers| openers.borrow_mut().insert(target, bounds));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// Open the menu `target` at `position` in the window, as a secondary
/// click does; its panel stands there instead of under a button.
pub(crate) fn open_at(target: &str, position: gpui::Point<gpui::Pixels>) {
    OPENERS.with(|openers| {
        openers.borrow_mut().insert(
            target.to_string(),
            gpui::Bounds::new(position, gpui::size(gpui::px(0.0), gpui::px(0.0))),
        )
    });
    close_all();
    set_open(target, true);
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
    // Just under its button: from the button's trailing edge when the
    // button is in the window's trailing half, else from its leading edge,
    // so the panel stays by it instead of being pushed back from the
    // window's edge.
    let window_width = c.win().width.get();
    let place = OPENERS.with(|openers| openers.borrow().get(target).copied());
    let placed = match place {
        Some(button) => {
            let gap = gpui::px(6.0);
            let middle = c.win().inset.get() + window_width / 2.0;
            let (anchor, x) = if f32::from(button.center().x) > middle {
                (Anchor::TopRight, button.right())
            } else {
                (Anchor::TopLeft, button.left())
            };
            anchored()
                .position_mode(AnchoredPositionMode::Window)
                .anchor(anchor)
                .position(point(x, button.bottom() + gap))
        }
        None => anchored()
            .anchor(Anchor::TopLeft)
            .offset(point(px(0.0), px(6.0))),
    };
    deferred(
        placed.snap_to_window_with_margin(px(8.0)).child(
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

/// A button of only an icon `size` points large, the label being its
/// accessible name: square on the standard button's surface, or with
/// `plain`, without one, as an `IconButton(prominence: plain)`.
fn icon_opener(
    c: &Ctx,
    id: &str,
    icon: &'static str,
    plain: bool,
    size: Option<f32>,
    open: bool,
) -> gpui::Stateful<gpui::Div> {
    let recipe = if plain {
        "icon_button"
    } else {
        "button.standard"
    };
    let state = if open { "pressed" } else { "rest" };
    let foreground = c.slot(recipe, state, "foreground");
    let hover_foreground = c.slot(recipe, "hover", "foreground");
    let hover = c.slot(recipe, "hover", "background");
    let side = 36.0;
    let button = div()
        .id(c.element_id(id))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(side))
        .rounded(px(8.0))
        .bg(c.slot(recipe, state, "background"))
        .text_color(foreground)
        .hover(move |style| style.bg(hover).text_color(hover_foreground))
        .child(icons::icon(icon, size.unwrap_or(16.0), foreground));
    if plain {
        button
    } else {
        button
            .border_1()
            .border_color(c.slot(recipe, "rest", "border"))
    }
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
    let target = c.key(id);
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
        target: c.target(id),
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
    // With an icon the label is only its accessible name.
    let button = match icon {
        Some(icon) => icon_opener(c, id, icon, props.plain, props.size, open),
        None => opener(c, id, label, None, false, open),
    };
    let pressed = Rc::clone(&toggle);
    let button = if enabled {
        button.on_click(move |_, _, cx| {
            cx.stop_propagation();
            pressed(cx)
        })
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
        .child(opener_place(&target))
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
    /// No surface under the button (`prominence: plain`).
    pub plain: bool,
    /// The size of its icon in points (`font_size`).
    pub size: Option<f32>,
}

/// An item of a menu: its label, whether it is checked and what choosing
/// it does.
pub(crate) type Entry = (String, bool, Rc<dyn Fn()>);

/// A menu button: the chosen item's label, which opens the list of items,
/// each with its label, whether it is checked and what choosing it does.
pub(crate) fn menu_button(c: &Ctx, id: &'static str, label: String, items: Vec<Entry>) -> El {
    let target = c.key(id);
    let open = is_open(&target);
    let enabled = c.enabled();
    let button = opener(c, id, label, None, true, open);
    let button = if enabled {
        let target = target.clone();
        button.on_click(move |_, _, cx| {
            cx.stop_propagation();
            set_open(&target, !is_open(&target))
        })
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
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            set_open(&close, false);
                            choose();
                        })
                }),
        );
    div()
        .relative()
        .flex_none()
        .child(button)
        .child(opener_place(&target))
        .when(open, |element| element.child(panel(c, &target, list)))
        .into_any_element()
}

/// The radius of a `Modal`'s card.
pub(crate) const MODAL_CORNER: f32 = 12.0;

/// Size of a `Modal`'s card, and whether it covers a compact window.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModalProps {
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Fill the window when it is compact, as a phone's full-screen flow.
    pub cover_compact: bool,
    /// Where its motion has it now: the scrim fades with the card, which
    /// moves and scales.
    pub pose: crate::Pose,
}

/// `Modal`: its content on a card over a scrim that covers the window,
/// blocking what is under it.
pub fn modal(c: &Ctx, props: ModalProps, content: Vec<El>) -> El {
    let compact = crate::value::truthy(&c.window("compact"));
    let cover = props.cover_compact && compact;
    // In motion the modal is built at its scale (`motion::moving`): the
    // card's lengths scale, the window's do not.
    let pose = props.pose;
    let scale = pose.scale.max(0.001);
    let window_width = c.win().width.get() / scale;
    let window_height = c.win().height.get() / scale;
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
            .rounded(px(MODAL_CORNER))
            .border_1()
            .border_color(c.slot("dialog", "open", "border"))
            .shadow_lg();
        match props.height {
            Some(height) => card.h(px(height.min(window_height - 32.0))),
            None => card,
        }
    };
    let card = card
        .relative()
        .left(px(pose.x / scale))
        .top(px(pose.y / scale));
    // Over the whole window, wherever it is declared.
    let top = c.win().title_bar.get() / scale;
    let inset = c.win().inset.get() / scale;
    deferred(
        anchored()
            .position_mode(gpui::AnchoredPositionMode::Window)
            .position(point(px(inset), px(inset)))
            .child(
                div()
                    .w(px(window_width))
                    .h(px(window_height + top))
                    .pt(px(top))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(c.slot("dialog", "open", "scrim"))
                    .opacity(pose.opacity)
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
    let target = c.key(&more);
    let open = is_open(&target);
    let flip: crate::ctx::Press = {
        let target = target.clone();
        Rc::new(move |_| set_open(&target, !is_open(&target)))
    };
    c.list(Control {
        target: c.target(&more),
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
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            flip(cx)
        });
    div()
        .relative()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(2.0))
        .child(toggle)
        .child(chevron)
        .child(opener_place(&target))
        .when(open, |element| {
            let body = div().flex().flex_col().children(content());
            element.child(panel(c, &target, body))
        })
        .into_any_element()
}
