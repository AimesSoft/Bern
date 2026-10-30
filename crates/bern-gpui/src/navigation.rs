//! Navigation: a stack of pushed routes with a way back, the links that
//! push them, and a sidebar beside a detail column whose items choose the
//! page.

use crate::controls::States;
use crate::ctx::{Ctx, Field, Model};
use crate::layout::{Style, color};
use crate::look::{Colors, ControlStyle, Look};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::prelude::FluentBuilder;
use gpui::{
    FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div,
};
use std::rc::Rc;

/// `Navigation`: the root content, or the destination of the last route
/// pushed onto `path` under a bar with the way back.
pub fn navigation(
    c: &Ctx,
    path: Field,
    depth: usize,
    title: Option<String>,
    content: Vec<El>,
) -> El {
    let body = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .children(content);
    let element = div().flex().flex_col().size_full();
    if depth == 0 {
        return c.grow().apply(element.child(body)).into_any_element();
    }
    let back: crate::ctx::Press = {
        let c = c.clone();
        Rc::new(move |_| {
            let mut routes = crate::value::as_list(c.read(path));
            routes.pop();
            c.write(path, Value::List(routes));
        })
    };
    c.list(Control {
        target: c.target("back"),
        window: c.win().id.clone(),
        role: "button",
        label: crate::text::tr("bern.window.done"),
        value: None,
        enabled: c.enabled(),
        actions: Actions {
            activate: Some(Rc::clone(&back)),
            ..Actions::default()
        },
    });
    let hover = c.color("hover.overlay");
    let bar = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .h(px(44.0))
        .px(px(8.0))
        .border_b_1()
        .border_color(c.color("separator"))
        .child(
            div()
                .id(c.element_id("back"))
                .flex()
                .items_center()
                .justify_center()
                .size(px(32.0))
                .rounded(px(8.0))
                .hover(move |style| style.bg(hover))
                .child(icons::icon("chevron.left", 18.0, c.color("accent.text")))
                .on_click(move |_, _, cx| back(cx)),
        )
        .children(title.map(|title| {
            div()
                .text_size(px(17.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title)
        }));
    c.grow()
        .apply(element.child(bar).child(body))
        .into_any_element()
}

/// `NavLink`: a row that pushes `route` onto the navigation's path.
pub fn nav_link(c: &Ctx, id: &'static str, label: String, route: Value, children: Vec<El>) -> El {
    let Some(path) = c.nav() else {
        return div()
            .flex()
            .flex_col()
            .children(children)
            .into_any_element();
    };
    let push: crate::ctx::Press = {
        let c = c.clone();
        Rc::new(move |_| {
            let mut routes = crate::value::as_list(c.read(path));
            routes.push(route.clone());
            c.write(path, Value::List(routes));
        })
    };
    let enabled = c.enabled();
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "link",
        label,
        value: None,
        enabled,
        actions: Actions {
            activate: Some(Rc::clone(&push)),
            ..Actions::default()
        },
    });
    let hover = c.slot("row", "hover", "background");
    let element = div()
        .id(c.element_id(id))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .w_full()
        .rounded(px(6.0))
        .child(div().flex_1().min_w(px(0.0)).children(children))
        .child(icons::icon(
            "chevron.right",
            14.0,
            c.color("icon.secondary"),
        ));
    if !enabled {
        return element.into_any_element();
    }
    element
        .hover(move |style| style.bg(hover))
        .on_click(move |_, _, cx| push(cx))
        .into_any_element()
}

/// `SplitView`: the sidebar, then the detail taking the rest.
pub fn split_view(c: &Ctx, sidebar: El, detail: El) -> El {
    let element = div().flex().flex_row().child(sidebar).child(detail);
    c.grow().apply(element.size_full()).into_any_element()
}

/// `Sidebar`: its header, its items scrolling, and its footer, on the
/// panel surface at the leading edge.
pub fn sidebar(
    c: &Ctx,
    width: Option<f32>,
    header: Vec<El>,
    items: Vec<El>,
    footer: Vec<El>,
    style: Option<&'static Style>,
) -> El {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .h_full()
        .w(px(width.unwrap_or(240.0)))
        .bg(match style.and_then(|style| style.background) {
            Some(background) => color(c, background),
            None => c.color("surface.panel"),
        })
        .border_r_1()
        .border_color(c.color("separator"))
        .when(!header.is_empty(), |element| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .px(px(12.0))
                    .pt(px(12.0))
                    .children(header),
            )
        })
        .child(
            div()
                .id(c.element_id("sidebar"))
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .gap(px(2.0))
                .p(px(8.0))
                .overflow_y_scroll()
                .children(items),
        )
        .when(!footer.is_empty(), |element| {
            element.child(div().flex().flex_col().p(px(8.0)).children(footer))
        })
        .into_any_element()
}

/// `SidebarItem`: a destination of the sidebar, filled with the accent
/// while it is the one chosen, with its count at the trailing edge;
/// `style!` rules change its corners, room, and colors at rest, under the
/// pointer, pressed and chosen.
pub fn sidebar_item(
    c: &Ctx,
    value: Value,
    label: String,
    icon: Option<&'static str>,
    badge: Option<i64>,
    style: Option<&'static ControlStyle>,
) -> El {
    let model = match c.sidebar() {
        Some(selection) => Model::Field(selection, None),
        None => Model::Controlled(Value::Unit, Rc::new(|_, _| {})),
    };
    let selected = model.value(c) == value;
    let target = format!("item.{}", crate::value::key_text(&value));
    let choose: crate::ctx::Press = {
        let c = c.clone();
        let value = value.clone();
        Rc::new(move |_| model.set(&c, value.clone()))
    };
    c.list(Control {
        target: c.target(&target),
        window: c.win().id.clone(),
        role: "sidebar_item",
        label: label.clone(),
        value: Some(selected.to_string()),
        enabled: c.enabled(),
        actions: Actions {
            activate: Some(Rc::clone(&choose)),
            ..Actions::default()
        },
    });
    let base = Colors {
        background: c.color("transparent"),
        foreground: c.color("text.primary"),
        border: None,
        shadow: None,
        opacity: 1.0,
    }
    .with(c, style.map(|style| &style.base));
    let rest = if selected {
        let chosen = style
            .and_then(|style| style.selected)
            .filter(|look| !look.none && (look.fill.is_some() || look.background.is_some()));
        match chosen {
            Some(look) => base.with(c, Some(&look)),
            None => Colors {
                background: c.color("accent"),
                foreground: c.color("on_accent"),
                ..base
            },
        }
    } else {
        base
    };
    let state = |rules: Option<&Look>, default: Option<Colors>| match rules {
        Some(rules) if rules.none => None,
        Some(rules) => Some(default.unwrap_or(rest).with(c, Some(rules))),
        None => default,
    };
    let hover_default = (!selected).then(|| Colors {
        background: c.color("hover.overlay"),
        ..rest
    });
    let hover = state(style.and_then(|style| style.hover.as_ref()), hover_default);
    let pressed = state(style.and_then(|style| style.pressed.as_ref()), hover);
    let corner = style.and_then(|style| style.corner).unwrap_or(8.0);
    let padding = style.and_then(|style| style.padding).unwrap_or(10.0);
    let foreground = rest.foreground;
    let build = move || {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .h(px(34.0))
            .px(px(padding))
            .rounded(px(corner))
            .text_size(px(14.0))
            .children(icon.map(|icon| icons::icon(icon, 16.0, foreground)))
            .child(div().flex_1().child(label.clone()))
            .children(badge.filter(|count| *count > 0).map(|count| {
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(count.to_string())
            }))
    };
    if !c.enabled() {
        return crate::look::paint(build().id(c.element_id(&target)), rest)
            .opacity(0.5)
            .flex_shrink_0()
            .into_any_element();
    }
    let states = States {
        rest,
        hover,
        pressed,
    };
    crate::controls::painted(c, &target, build, states, style, choose)
        .flex_shrink_0()
        .into_any_element()
}

/// The label of the sidebar item the selection names, as the detail
/// column's title.
pub fn sidebar_title(selection: &Value, items: Vec<(Value, String)>) -> Option<String> {
    items
        .into_iter()
        .find(|(value, _)| value == selection)
        .map(|(_, label)| label)
}

/// `Detail`: the page beside the sidebar, under its title.
pub fn detail(
    c: &Ctx,
    title: Option<String>,
    content: Vec<El>,
    style: Option<&'static Style>,
) -> El {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .when_some(
            style.and_then(|style| style.background),
            |element, background| element.bg(color(c, background)),
        )
        .children(title.map(|title| {
            div()
                .flex_none()
                .px(px(24.0))
                .pt(px(16.0))
                .pb(px(8.0))
                .text_size(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c.color("text.primary"))
                .child(title)
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .children(content),
        )
        .into_any_element()
}
