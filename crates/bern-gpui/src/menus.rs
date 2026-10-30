//! `Menu`: a button that opens a list of commands, and the option labels of
//! pickers; `ControlGroup`: controls on one capsule surface.

use crate::ctx::Ctx;
use crate::look::{self, Colors, Shape};
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div,
};
use std::rc::Rc;

/// One entry of a menu: a command, or a separator.
#[derive(Clone)]
pub enum MenuEntry {
    Item {
        id: &'static str,
        label: String,
        icon: Option<&'static str>,
        destructive: bool,
        enabled: bool,
        press: crate::ctx::Press,
    },
    Divider,
}

/// A menu item: its action, with the open panel first when it picks.
#[allow(clippy::too_many_arguments)]
pub fn menu_item(
    c: &Ctx,
    id: &'static str,
    label: String,
    icon: Option<&'static str>,
    destructive: bool,
    enabled: bool,
    pick: Option<(crate::Field, &'static str)>,
    shortcut: Option<String>,
    action: Option<crate::Act>,
) -> MenuEntry {
    let enabled = enabled && c.enabled();
    let press = crate::controls::press(c, pick, action);
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "menu_item",
        label: label.clone(),
        value: None,
        enabled,
        actions: Actions {
            activate: Some(Rc::clone(&press)),
            ..Actions::default()
        },
    });
    if enabled {
        crate::controls::register_shortcut(c, shortcut.as_deref(), &press);
    }
    MenuEntry::Item {
        id,
        label,
        icon,
        destructive,
        enabled,
        press,
    }
}

/// How a `Menu`'s button shows.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuProps {
    pub icon: Option<&'static str>,
    /// The icon alone, as an icon button.
    pub icon_only: bool,
    pub shape: Shape,
    pub height: Option<f32>,
    /// Leave disabled items out instead of dimming them.
    pub hide_disabled: bool,
}

/// `Menu`: a button that opens its commands below it.
pub fn menu(
    c: &Ctx,
    id: &'static str,
    label: String,
    props: MenuProps,
    entries: Vec<MenuEntry>,
) -> El {
    let target = c.target(id);
    let open = crate::overlay::is_open(&target);
    let enabled = c.enabled();
    let toggle = {
        let target = target.clone();
        Rc::new(move |_: &mut App| {
            crate::overlay::set_open(&target, !crate::overlay::is_open(&target))
        })
    };
    c.list(Control {
        target: target.clone(),
        window: c.win().id.clone(),
        role: "menu",
        label: label.clone(),
        value: Some(open.to_string()),
        enabled,
        actions: Actions {
            activate: Some(toggle.clone()),
            ..Actions::default()
        },
    });
    let button = if props.icon_only {
        let side = props.height.unwrap_or(36.0);
        let recipe = if props.height.is_some() {
            "button.standard"
        } else {
            "icon_button"
        };
        let colors = Colors::recipe(
            c,
            recipe,
            if enabled { "rest" } else { "disabled" },
            props.height.is_some(),
        );
        let hover = c.color("hover.overlay");
        look::paint(
            div()
                .id(c.element_id(id))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(side))
                .rounded(px(if props.height.is_some() {
                    side / 2.0
                } else {
                    8.0
                })),
            colors,
        )
        .hover(move |style| style.bg(hover))
        .children(
            props
                .icon
                .map(|icon| icons::icon(icon, 18.0, colors.foreground)),
        )
    } else {
        let (height, padding, text) = look::metrics(None, props.shape);
        let height = props.height.unwrap_or(height);
        let colors = Colors::recipe(
            c,
            "button.standard",
            if enabled { "rest" } else { "disabled" },
            true,
        );
        let hover = c.slot("button.standard", "hover", "background");
        look::paint(
            div()
                .id(c.element_id(id))
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(6.0))
                .h(px(height))
                .px(px(padding))
                .rounded(px(if props.shape == Shape::Default {
                    8.0
                } else {
                    height / 2.0
                }))
                .text_size(px(text))
                .font_weight(FontWeight::MEDIUM),
            colors,
        )
        .hover(move |style| style.bg(hover))
        .children(
            props
                .icon
                .map(|icon| icons::icon(icon, text + 2.5, colors.foreground)),
        )
        .child(label)
        .child(icons::icon("chevron.down", 12.0, c.color("icon.secondary")))
    };
    let button = if enabled {
        button.on_click(move |_, _, cx| toggle(cx))
    } else {
        button
    };
    let hover = c.slot("menu", "hover", "background");
    let error = c.color("status.error");
    let disabled = c.color("disabled.foreground");
    let list = div()
        .flex()
        .flex_col()
        .min_w(px(200.0))
        .p(px(4.0))
        .children(
            entries
                .into_iter()
                .filter(|entry| {
                    !props.hide_disabled || !matches!(entry, MenuEntry::Item { enabled: false, .. })
                })
                .map(|entry| match entry {
                    MenuEntry::Divider => div()
                        .my(px(4.0))
                        .h(px(1.0))
                        .bg(c.color("separator"))
                        .into_any_element(),
                    MenuEntry::Item {
                        id,
                        label,
                        icon,
                        destructive,
                        enabled,
                        press,
                    } => {
                        let color = match (enabled, destructive) {
                            (false, _) => disabled,
                            (true, true) => error,
                            (true, false) => c.color("text.primary"),
                        };
                        let close = target.clone();
                        div()
                            .id(c.element_id(id))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .h(px(32.0))
                            .px(px(10.0))
                            .rounded(px(6.0))
                            .text_size(px(14.0))
                            .text_color(color)
                            .children(icon.map(|icon| icons::icon(icon, 16.0, color)))
                            .child(label)
                            .when(enabled, |row| {
                                row.hover(move |style| style.bg(hover))
                                    .on_click(move |_, _, cx| {
                                        crate::overlay::set_open(&close, false);
                                        press(cx);
                                    })
                            })
                            .into_any_element()
                    }
                }),
        );
    div()
        .relative()
        .flex_none()
        .child(button)
        .when(open, |element| {
            element.child(crate::overlay::panel(c, &target, list))
        })
        .into_any_element()
}

/// `ControlGroup`: related controls side by side on one capsule surface.
pub fn control_group(c: &Ctx, height: Option<f32>, children: Vec<El>) -> El {
    let element = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(c.slot("segmented", "rest", "border"))
        .bg(c.slot("segmented", "rest", "track"))
        .children(children);
    match height {
        Some(height) => element.h(px(height)).into_any_element(),
        None => element.into_any_element(),
    }
}
