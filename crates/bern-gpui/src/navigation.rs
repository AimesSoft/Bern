//! Navigation: a stack of pushed routes with a way back, the links that
//! push them, and a sidebar beside a detail column whose items choose the
//! page.

use crate::controls::States;
use crate::ctx::{Ctx, Field, Model};
use crate::layout::{Style, color};
use crate::look::{Colors, ControlStyle, Look};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::surface::{GpuiRenderer, apply_box, as_box};
use crate::units::px;
use crate::{El, icons};
use bern_surface::ColorValue;
use bern_surface::controls::{
    DetailSpec, NavLinkSpec, NavigationBarSpec, SidebarItemSpec, SidebarSpec, detail as ir_detail,
    nav_link as ir_nav_link, navigation_bar as ir_navigation_bar, sidebar as ir_sidebar,
    sidebar_item as ir_sidebar_item,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div,
};
use std::rc::Rc;

/// Reset before building either page, with state owned by this mounted
/// element rather than by the session path or the reset key itself.
pub fn navigation_reset(
    c: &Ctx,
    id: &str,
    path: Field,
    value: Value,
    build: impl FnOnce(&Ctx) -> El + 'static,
) -> El {
    ResetNavigation {
        key: ElementId::Name(format!("{}.navigation-reset", c.key(id)).into()),
        c: c.clone(),
        path,
        value,
        build: Box::new(build),
    }
    .into_any_element()
}

#[derive(IntoElement)]
struct ResetNavigation {
    key: ElementId,
    c: Ctx,
    path: Field,
    value: Value,
    build: Box<dyn FnOnce(&Ctx) -> El>,
}

fn reset_path(last: &mut Option<Value>, value: Value, clear: impl FnOnce()) {
    if crate::behaviors::watch_changed(last, value) {
        clear();
    }
}

impl RenderOnce for ResetNavigation {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let last = window.use_keyed_state(self.key, cx, |_, _| None::<Value>);
        last.update(cx, |last, _| {
            reset_path(last, self.value, || {
                self.c.write(self.path, Value::List(Vec::new()));
            });
        });
        (self.build)(&self.c)
    }
}

/// `Navigation`: the root content, or the destination of the last route
/// pushed onto `path` under a bar with the way back.
pub fn navigation(
    c: &Ctx,
    id: Option<&'static str>,
    path: Field,
    depth: usize,
    title: Option<String>,
    content: Vec<El>,
) -> El {
    let nav_id = id.unwrap_or("navigation");
    let back_fn: Rc<dyn Fn() -> bool> = {
        let c = c.clone();
        Rc::new(move || {
            let mut routes = crate::value::as_list(c.read(path));
            if routes.pop().is_some() {
                c.write(path, Value::List(routes));
                true
            } else {
                false
            }
        })
    };
    c.list(Control {
        target: c.target(nav_id),
        window: c.win().id.clone(),
        role: "navigation",
        label: title.clone().unwrap_or_default(),
        value: Some(depth.to_string()),
        enabled: c.enabled(),
        actions: Actions {
            back: Some(Rc::clone(&back_fn)),
            ..Actions::default()
        },
    });
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
        let back_fn = Rc::clone(&back_fn);
        Rc::new(move |_| {
            back_fn();
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
            back: Some(back_fn),
            ..Actions::default()
        },
    });
    let hover = c.color("hover.overlay");
    let back_btn = div()
        .id(c.element_id("back"))
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded(px(8.0))
        .hover(move |style| style.bg(hover))
        .child(icons::icon("chevron.left", 18.0, c.color("accent.text")))
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            back(cx)
        })
        .into_any_element();
    let bar = GpuiRenderer::new(c)
        .slot_one("back", back_btn)
        .render(&ir_navigation_bar(NavigationBarSpec { title }));
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
    // Outside a grouped `Section`, `NavLink` provides its own row insets
    // (16pt horizontal, 12pt vertical), matching SwiftUI, WinUI 3, Compose and Web.
    let in_section = c.in_section_row() && c.sidebar().is_none();
    let ir = ir_nav_link(NavLinkSpec {
        in_section,
        enabled,
    });
    let element = as_box(&ir)
        .map(|b| apply_box(c, div(), b))
        .unwrap_or_else(div)
        .id(c.element_id(id))
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
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            push(cx)
        })
        .into_any_element()
}

/// `SplitView`: the sidebar (hidden while `collapsed` holds), then the detail taking the rest.
pub fn split_view(c: &Ctx, collapsed: Option<Field>, sidebar: El, detail: El) -> El {
    let folded = collapsed.is_some_and(|field| c.read(field) == Value::Bool(true));
    let element = div()
        .flex()
        .flex_row()
        .when(!folded, |row| row.child(sidebar))
        .child(detail);
    c.grow().apply(element.size_full()).into_any_element()
}

/// Default translucent tint for `Sidebar` from `bern-surface::SurfacePalette`
/// (`sidebar.surface`): subtle contrast over plain light/dark canvases while
/// letting window backdrops (`Stack(under_bars: true)`) shine through.
#[cfg(test)]
fn sidebar_default_bg(theme: &crate::theme::Theme) -> gpui::Rgba {
    theme.color("sidebar.surface")
}

/// `Sidebar`: its header, its items scrolling, and its footer, on a
/// translucent panel surface at the leading edge that reaches under the
/// unified title bar so window backdrops flow seamlessly behind it.
pub fn sidebar(
    c: &Ctx,
    width: Option<f32>,
    header: Vec<El>,
    items: Vec<El>,
    footer: Vec<El>,
    style: Option<&'static Style>,
) -> El {
    let top = c.win().title_bar.get();
    let under_bar = top > 0.0 && !c.in_modal_content();
    let ir = ir_sidebar(SidebarSpec {
        width,
        background: style.and_then(|s| s.background).map(|bg| {
            let resolved = color(c, bg);
            ColorValue::Rgba(bern_surface::Rgba8::new(
                (resolved.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                (resolved.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                (resolved.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                (resolved.a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ))
        }),
        has_header: !header.is_empty(),
        has_footer: !footer.is_empty(),
    });
    let base = as_box(&ir)
        .map(|b| apply_box(c, div(), b))
        .unwrap_or_else(div);
    base.when(under_bar, |el| {
        el.mt(px(-top))
            .pt(px(top))
            .h(px(c.win().height.get() + top))
    })
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
    let ir = ir_sidebar_item(SidebarItemSpec {
        label: label.clone(),
        icon,
        badge,
        selected,
        enabled: c.enabled(),
        corner: style.and_then(|s| s.corner),
        padding_x: style.and_then(|s| s.padding),
    });
    let ir_box = as_box(&ir);
    let base = Colors::recipe(c, "nav_item", "rest", false).with(c, style.map(|s| &s.base));
    let rest = if selected {
        let chosen = style
            .and_then(|style| style.selected)
            .filter(|look| !look.none && (look.fill.is_some() || look.background.is_some()));
        match chosen {
            Some(look) => base.with(c, Some(&look)),
            None => Colors::recipe(c, "nav_item", "selected", false),
        }
    } else {
        base
    };
    let state = |rules: Option<&Look>, default: Option<Colors>| match rules {
        Some(rules) if rules.none => None,
        Some(rules) => Some(default.unwrap_or(rest).with(c, Some(rules))),
        None => default,
    };
    let hover_default = (!selected).then(|| Colors::recipe(c, "nav_item", "hover", false));
    let hover = state(style.and_then(|style| style.hover.as_ref()), hover_default);
    let pressed = state(style.and_then(|style| style.pressed.as_ref()), hover);
    let corner = style
        .and_then(|style| style.corner)
        .unwrap_or(bern_surface::controls::navigation::SIDEBAR_ITEM_CORNER);
    let padding = style
        .and_then(|style| style.padding)
        .unwrap_or(bern_surface::controls::navigation::SIDEBAR_ITEM_PADDING_X);
    let item_height = bern_surface::controls::navigation::SIDEBAR_ITEM_HEIGHT;
    let gap = ir_box.map_or(10.0, |b| b.layout.gap);
    let font_size = ir_box
        .and_then(|b| b.style.typography)
        .map_or(13.5, |t| t.size);
    let font_weight = ir_box
        .and_then(|b| b.style.typography)
        .map_or(FontWeight::MEDIUM, |t| FontWeight(f32::from(t.weight)));
    let foreground = rest.foreground;
    let build = move || {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(gap))
            .h(px(item_height))
            .px(px(padding))
            .rounded(px(corner))
            .text_size(px(font_size))
            .font_weight(font_weight)
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

/// Reconcile the sidebar's `selection` against its currently mounted items;
/// when the chosen item is absent, switch to `fallback` (if present) or the
/// first mounted item and persist it back to `field`.
pub fn sidebar_selection(
    c: &Ctx,
    items: &[(Value, String)],
    chosen: Value,
    fallback: Option<Value>,
    field: Field,
) -> Value {
    let present = |value: &Value| items.iter().any(|(item, _)| item == value);
    if items.is_empty() || present(&chosen) {
        return chosen;
    }
    let Some(replacement) = fallback
        .filter(|value| present(value))
        .or_else(|| items.first().map(|(value, _)| value.clone()))
    else {
        return chosen;
    };
    c.write(field, replacement.clone());
    replacement
}

/// The label of the sidebar item the selection names, as the detail
/// column's title.
pub fn sidebar_title(selection: &Value, items: Vec<(Value, String)>) -> Option<String> {
    items
        .into_iter()
        .find(|(value, _)| value == selection)
        .map(|(_, label)| label)
}

/// `Detail`: the page beside the sidebar, under its title (and a sidebar toggle
/// button when `collapsed` is bound).
pub fn detail(
    c: &Ctx,
    title: Option<String>,
    collapsed: Option<Field>,
    content: Vec<El>,
    footer: Vec<El>,
    style: Option<&'static Style>,
) -> El {
    let toggle = collapsed.map(|field| {
        let folded = c.read(field) == Value::Bool(true);
        let label = crate::text::tr(if folded {
            "bern.navigation.show_sidebar"
        } else {
            "bern.navigation.hide_sidebar"
        });
        let flip: crate::ctx::Press = {
            let c = c.clone();
            Rc::new(move |_| {
                let now = c.read(field) == Value::Bool(true);
                c.write(field, Value::Bool(!now));
            })
        };
        c.list(Control {
            target: c.target("sidebar_toggle"),
            window: c.win().id.clone(),
            role: "button",
            label,
            value: Some(if folded { "collapsed" } else { "expanded" }.to_string()),
            enabled: c.enabled(),
            actions: Actions {
                activate: Some(Rc::clone(&flip)),
                ..Actions::default()
            },
        });
        let hover = c.color("hover.overlay");
        div()
            .id(c.element_id("sidebar_toggle"))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(32.0))
            .rounded(px(8.0))
            .hover(move |style| style.bg(hover))
            .child(icons::icon("sidebar.left", 18.0, c.color("icon.primary")))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                flip(cx);
            })
            .into_any_element()
    });
    let ir = ir_detail(DetailSpec {
        title,
        has_toggle: toggle.is_some(),
        has_footer: !footer.is_empty(),
        background: style.and_then(|s| s.background).map(|bg| {
            let resolved = color(c, bg);
            ColorValue::Rgba(bern_surface::Rgba8::new(
                (resolved.r.clamp(0.0, 1.0) * 255.0).round() as u8,
                (resolved.g.clamp(0.0, 1.0) * 255.0).round() as u8,
                (resolved.b.clamp(0.0, 1.0) * 255.0).round() as u8,
                (resolved.a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ))
        }),
    });
    let mut renderer = GpuiRenderer::new(c)
        .slot("content", content)
        .slot("footer", footer);
    if let Some(toggle_el) = toggle {
        renderer = renderer.slot_one("toggle", toggle_el);
    }
    renderer.render(&ir)
}

#[cfg(test)]
mod reset_tests {
    use super::*;

    #[test]
    fn sidebar_default_background_is_translucent_in_both_modes() {
        for dark in [false, true] {
            let theme = crate::theme::of(dark, None);
            let bg = sidebar_default_bg(&theme);
            assert!(
                bg.a > 0.0 && bg.a < 0.1,
                "dark={dark} sidebar tint should be translucent, got a={}",
                bg.a
            );
        }
    }

    #[test]
    fn navigation_reset_only_clears_on_mount_or_key_change() {
        for first in [
            Value::Int(0),
            Value::Text("root".into()),
            Value::unit_variant(0),
        ] {
            let mut last = None;
            let mut path = vec![Value::unit_variant(1)];
            reset_path(&mut last, first.clone(), || path.clear());
            assert!(path.is_empty());
            path.push(Value::unit_variant(2));
            reset_path(&mut last, first.clone(), || path.clear());
            assert_eq!(path.len(), 1);
            reset_path(&mut last, Value::Int(99), || path.clear());
            assert!(path.is_empty());
            path.push(Value::unit_variant(2));
            // Returning to an earlier key is another change, not a cache hit.
            reset_path(&mut last, first.clone(), || path.clear());
            assert!(path.is_empty());
            path.push(Value::unit_variant(2));
            // A new mount starts with no remembered key, even for the same value.
            reset_path(&mut None, first, || path.clear());
            assert!(path.is_empty());
        }
    }
}
