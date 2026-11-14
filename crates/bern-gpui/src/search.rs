//! Search: a field whose results show below it while it is in use, or a
//! palette that a field in the toolbar opens over the window; the
//! palette's items are chosen with the arrow keys and Return.

use crate::controls::{self, InputKind, InputProps};
use crate::ctx::{Act, Ctx, Field};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnchoredPositionMode, App, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, anchored, deferred, div, point,
};
use std::cell::RefCell;
use std::rc::Rc;

/// How a `Search` shows.
#[derive(Clone, Default)]
pub struct SearchProps {
    pub prompt: String,
    /// The palette field's prompt; the field's otherwise.
    pub panel_prompt: Option<String>,
    /// `style: palette`.
    pub palette: bool,
    /// `display: icon` on a palette trigger.
    pub icon_only: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub font_size: Option<f32>,
    /// Results before anything is typed.
    pub suggest: bool,
    pub shortcut: Option<String>,
}

/// The open palette: its search and the item the keyboard stands on.
struct Palette {
    key: String,
    highlight: usize,
    /// The navigable items of its last frame, in order.
    items: Vec<crate::ctx::Press>,
    /// Going back into its field.
    closing: bool,
}

/// The spring the palette grows out of its field and goes back with
/// (`hero`).
const GROWTH: crate::Curve = (0.48, 0.86);

thread_local! {
    static PALETTE: RefCell<Option<Palette>> = const { RefCell::new(None) };
    /// Where each palette's field was last laid out, in the window.
    static FIELDS: RefCell<std::collections::HashMap<String, gpui::Bounds<gpui::Pixels>>> =
        RefCell::new(std::collections::HashMap::new());
    /// While a palette's results build: the item the keyboard stands on,
    /// and the navigable items so far.
    static COLLECTING: RefCell<Option<(usize, Vec<crate::ctx::Press>)>> = const { RefCell::new(None) };
}

fn open_key() -> Option<String> {
    PALETTE.with(|palette| palette.borrow().as_ref().map(|palette| palette.key.clone()))
}

/// The palette goes back into its field, and is gone once there.
fn close() {
    let closing = PALETTE.with(|palette| match palette.borrow_mut().as_mut() {
        Some(palette) if !palette.closing => {
            palette.closing = true;
            true
        }
        _ => false,
    });
    if closing {
        crate::host::redraw();
    }
}

/// Arrow keys, Return and Escape while a palette is open; true when the
/// key was the palette's.
pub(crate) fn keystroke(key: &str, cx: &mut App) -> bool {
    enum Then {
        Nothing,
        Redraw,
        Close,
        Choose(crate::ctx::Press),
    }
    let then = PALETTE.with(|palette| {
        let mut palette = palette.borrow_mut();
        let Some(palette) = palette.as_mut().filter(|palette| !palette.closing) else {
            return Then::Nothing;
        };
        let last = palette.items.len().saturating_sub(1);
        match key {
            "down" => {
                palette.highlight = (palette.highlight + 1).min(last);
                Then::Redraw
            }
            "up" => {
                palette.highlight = palette.highlight.saturating_sub(1);
                Then::Redraw
            }
            "enter" => match palette.items.get(palette.highlight) {
                Some(item) => Then::Choose(Rc::clone(item)),
                None => Then::Nothing,
            },
            "escape" => Then::Close,
            _ => Then::Nothing,
        }
    });
    match then {
        Then::Nothing => false,
        Then::Redraw => {
            crate::host::redraw();
            true
        }
        Then::Close => {
            close();
            true
        }
        Then::Choose(choose) => {
            choose(cx);
            true
        }
    }
}

/// `cmd+k` as the platform writes it: `⌘K` on a Mac, `Ctrl+K` elsewhere.
fn shortcut_text(shortcut: &str) -> String {
    let mac = cfg!(target_os = "macos");
    let parts: Vec<String> = shortcut
        .split('+')
        .map(|part| match (part, mac) {
            ("cmd", true) => "⌘".into(),
            ("cmd", false) => "Ctrl".into(),
            ("shift", true) => "⇧".into(),
            ("alt" | "option", true) => "⌥".into(),
            ("ctrl", true) => "⌃".into(),
            (key, _) if key.chars().count() == 1 => key.to_uppercase(),
            (key, _) => {
                let mut chars = key.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            }
        })
        .collect();
    parts.join(if mac { "" } else { "+" })
}

/// `Search`.
pub fn search(
    c: &Ctx,
    id: &'static str,
    field: Field,
    props: SearchProps,
    submit: Option<Act>,
    results: Option<crate::ctx::Content<'_>>,
) -> El {
    if props.palette {
        return palette(c, id, field, props, submit, results);
    }
    let key = c.key(id);
    let value = crate::value::as_text(&c.read(field));
    let input = controls::text_input(
        c,
        id,
        InputProps {
            label: props.prompt.clone(),
            label_hidden: true,
            prompt: props.prompt.clone(),
            kind: InputKind::Search,
            height: props.height,
            font_size: props.font_size,
            ..InputProps::default()
        },
        field,
        submit,
    );
    if let Some(shortcut) = props.shortcut.as_deref() {
        let key = key.clone();
        let focus: crate::ctx::Press = Rc::new(move |_| controls::request_focus(&key));
        controls::register_shortcut(c, Some(shortcut), &focus);
    }
    let width = props.width.or(c.toolbar().then_some(220.0));
    let in_use = controls::focused(&key) && (props.suggest || !value.is_empty());
    let shown = results.filter(|_| in_use || c.listing());
    let height = props.height.unwrap_or(32.0);
    let element = div()
        .relative()
        .flex()
        .flex_col()
        .map(|element| match width {
            Some(width) => element.w(px(width)).flex_none(),
            None => element.w_full(),
        })
        .child(input)
        .children(shown.map(|results| {
            let panel = div()
                .id(gpui::ElementId::Name(format!("{key}.results").into()))
                .occlude()
                .flex()
                .flex_col()
                .min_w(px(280.0))
                .max_h(px(420.0))
                .overflow_y_scroll()
                .p(px(6.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(c.slot("menu", "popup", "border"))
                .bg(c.slot("menu", "popup", "background"))
                .text_color(c.color("text.primary"))
                .shadow_lg()
                .children(results(&c.in_toolbar(false)));
            deferred(
                anchored()
                    .offset(point(px(0.0), px(height + 4.0)))
                    .snap_to_window_with_margin(px(8.0))
                    .child(panel),
            )
            .with_priority(1)
        }));
    match width {
        Some(_) => element.into_any_element(),
        None => c.grow().apply(element).into_any_element(),
    }
}

/// `Search(style: palette)`: a field in the toolbar that opens the
/// palette, which searches as it is typed into and shows the results
/// under it.
fn palette(
    c: &Ctx,
    id: &'static str,
    field: Field,
    props: SearchProps,
    submit: Option<Act>,
    results: Option<crate::ctx::Content<'_>>,
) -> El {
    let key = format!("{}/{}", c.win().id, c.key(id));
    let field_key = c.key(&format!("{id}.palette"));
    let open: crate::ctx::Press = {
        let c = c.clone();
        let key = key.clone();
        let field_key = field_key.clone();
        Rc::new(move |_| {
            c.write(field, Value::Text(String::new()));
            PALETTE.with(|palette| {
                *palette.borrow_mut() = Some(Palette {
                    key: key.clone(),
                    highlight: 0,
                    items: Vec::new(),
                    closing: false,
                })
            });
            controls::request_focus(&field_key);
        })
    };
    let is_open = open_key().as_deref() == Some(&key);
    let value = crate::value::as_text(&c.read(field));
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "search",
        label: props.prompt.clone(),
        value: Some(value),
        enabled: c.enabled(),
        actions: Actions {
            activate: Some(Rc::clone(&open)),
            edit: Some({
                let c = c.clone();
                Rc::new(move |text| {
                    c.write(field, Value::Text(text));
                })
            }),
            submit: submit.clone().map(|submit| {
                let c = c.clone();
                Rc::new(move || submit(&c)) as Rc<dyn Fn()>
            }),
            ..Actions::default()
        },
    });
    let enabled = c.enabled();
    if enabled {
        controls::register_shortcut(c, props.shortcut.as_deref(), &open);
    }
    let secondary = c.color("text.secondary");
    let primary = c.color("text.primary");
    let trigger = {
        let open = Rc::clone(&open);
        let base = div()
            .id(c.element_id(id))
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .when(is_open, |trigger| trigger.invisible());
        let styled = if props.icon_only {
            base.justify_center()
                .w(px(32.0))
                .h(px(32.0))
                .rounded(px(16.0))
                .child(icons::icon("magnifyingglass", 16.0, primary))
        } else {
            base.gap(px(6.0))
                .w(px(260.0))
                .h(px(32.0))
                .px(px(10.0))
                .rounded(px(16.0))
                .border_1()
                .border_color(c.slot("field", "rest", "border"))
                .bg(c.slot("field", "rest", "background"))
                .text_size(px(13.0))
                .text_color(secondary)
                .child(icons::icon("magnifyingglass", 14.0, secondary))
                .child(div().flex_1().overflow_hidden().child(props.prompt.clone()))
                .children(props.shortcut.as_deref().map(|shortcut| {
                    div()
                        .flex_none()
                        .px(px(5.0))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(c.color("separator"))
                        .text_size(px(11.0))
                        .child(shortcut_text(shortcut))
                }))
        };
        styled
            .when(!enabled, |trigger| trigger.opacity(0.5))
            .when(enabled, |trigger| {
                trigger.on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    open(cx)
                })
            })
            .relative()
            .child({
                // Where the field is, for the palette to grow out of.
                let key = key.clone();
                gpui::canvas(
                    move |bounds, _, _| {
                        FIELDS.with(|fields| fields.borrow_mut().insert(key, bounds));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full()
            })
    };
    if !is_open && !c.listing() {
        return trigger.into_any_element();
    }
    // The results, with the item the keyboard stands on.
    let highlight = PALETTE.with(|palette| {
        palette
            .borrow()
            .as_ref()
            .map_or(0, |palette| palette.highlight)
    });
    COLLECTING.with(|collecting| *collecting.borrow_mut() = Some((highlight, Vec::new())));
    let list = results
        .map(|results| results(&c.in_toolbar(false)))
        .unwrap_or_default();
    let items = COLLECTING
        .with(|collecting| collecting.borrow_mut().take())
        .map(|(_, items)| items)
        .unwrap_or_default();
    if !c.listing() {
        PALETTE.with(|palette| {
            if let Some(palette) = palette.borrow_mut().as_mut() {
                palette.highlight = palette.highlight.min(items.len().saturating_sub(1));
                palette.items = items;
            }
        });
    }
    if !is_open {
        return trigger.into_any_element();
    }
    // Return with no item to choose runs the search's action.
    let submit: Option<Act> = submit.map(|submit| {
        Rc::new(move |c: &Ctx| {
            let items = PALETTE.with(|palette| {
                palette
                    .borrow()
                    .as_ref()
                    .map_or(0, |palette| palette.items.len())
            });
            if items == 0 {
                submit(c);
            }
        }) as Act
    });
    let input = controls::text_input(
        c,
        &format!("{id}.palette"),
        InputProps {
            label: props.panel_prompt.clone().unwrap_or(props.prompt.clone()),
            label_hidden: true,
            prompt: props.panel_prompt.clone().unwrap_or(props.prompt.clone()),
            kind: InputKind::Search,
            height: Some(44.0),
            font_size: Some(16.0),
            bare: true,
            ..InputProps::default()
        },
        field,
        submit,
    );
    let window_width = c.win().width.get();
    let window_height = c.win().height.get();
    let width = props.width.unwrap_or(640.0).min(window_width - 32.0);
    let height = props.height.unwrap_or(480.0).min(window_height - 48.0);
    // It grows out of its field on the hero spring and goes back into it;
    // the window dims under it as it grows.
    let closing = PALETTE.with(|palette| {
        palette
            .borrow()
            .as_ref()
            .is_some_and(|palette| palette.closing)
    });
    let grown = crate::motion::spring_from(
        c,
        &format!("{key}#grown"),
        Some(0.0),
        GROWTH,
        if closing { 0.0 } else { 1.0 },
        1.0,
    );
    if closing && grown <= 0.001 {
        PALETTE.with(|palette| palette.borrow_mut().take());
        crate::host::redraw();
    }
    let panel = div()
        .id(gpui::ElementId::Name(format!("{key}.palette").into()))
        .occlude()
        .flex()
        .flex_col()
        .w(px(width))
        .h(px(height))
        .rounded(px(14.0))
        .border_1()
        .border_color(c.slot("menu", "popup", "border"))
        .bg(c.slot("menu", "popup", "background"))
        .text_color(c.color("text.primary"))
        .shadow_lg()
        .overflow_hidden()
        .on_mouse_down_out(|_, _, _| close())
        .child(
            div()
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(4.0))
                .px(px(6.0))
                .border_b_1()
                .border_color(c.color("separator"))
                .child(div().flex_1().child(input))
                .child(
                    div()
                        .id("close")
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.0))
                        .rounded(px(14.0))
                        .child(icons::icon("xmark", 14.0, secondary))
                        .on_click(|_, _, _| close()),
                ),
        )
        .child(
            div()
                .id("results")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .p(px(6.0))
                .children(list),
        );
    let top = 56.0;
    let field = FIELDS.with(|fields| fields.borrow().get(&key).copied());
    let shown = match field {
        // On its way: a box from the field's place and size to the
        // panel's, showing the panel from its leading top corner.
        Some(field) if grown < 0.999 => {
            let mix = |from: f32, to: f32| from + (to - from) * grown;
            let (left, panel_top) = ((window_width - width) / 2.0, top);
            div()
                .absolute()
                .left(gpui::px(mix(f32::from(field.origin.x), left)))
                .top(gpui::px(mix(f32::from(field.origin.y), panel_top)))
                .w(gpui::px(mix(f32::from(field.size.width), width).max(0.0)))
                .h(gpui::px(mix(f32::from(field.size.height), height).max(0.0)))
                .rounded(gpui::px(mix(16.0, 14.0)))
                .overflow_hidden()
                .child(panel)
                .into_any_element()
        }
        _ => div()
            .absolute()
            .top(px(top))
            .w(px(window_width))
            .flex()
            .flex_col()
            .items_center()
            .child(panel)
            .into_any_element(),
    };
    div()
        .relative()
        .flex_none()
        .child(trigger)
        .child(
            deferred(
                anchored()
                    .position_mode(AnchoredPositionMode::Window)
                    .position(point(px(c.win().inset.get()), px(c.win().inset.get())))
                    .child(
                        div()
                            .relative()
                            .w(px(window_width))
                            .h(px(window_height + 100.0))
                            .child(
                                div()
                                    .absolute()
                                    .size_full()
                                    .bg(c.slot("dialog", "open", "scrim"))
                                    .opacity(grown.clamp(0.0, 1.0)),
                            )
                            .child(shown),
                    ),
            )
            .with_priority(2),
        )
        .into_any_element()
}

/// How a `SearchItem` shows.
#[derive(Clone, Default)]
pub struct SearchItemProps {
    pub label: String,
    pub icon: Option<&'static str>,
    pub detail: Option<String>,
    pub hint: Option<String>,
    /// Reached with the arrow keys.
    pub navigation: bool,
    pub pick: Option<(Field, &'static str)>,
}

/// `SearchItem`: a result of the palette; choosing it closes the palette,
/// then runs its action. Its content, when it has any, replaces the row.
pub fn search_item(
    c: &Ctx,
    id: &'static str,
    props: SearchItemProps,
    action: Option<Act>,
    content: Option<Vec<El>>,
) -> El {
    let run = controls::press(c, props.pick, action);
    let choose: crate::ctx::Press = Rc::new(move |cx| {
        close();
        run(cx);
    });
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "button",
        label: props.label.clone(),
        value: None,
        enabled: c.enabled(),
        actions: Actions {
            activate: Some(Rc::clone(&choose)),
            ..Actions::default()
        },
    });
    // Its place among the items the arrow keys reach.
    let place = props
        .navigation
        .then(|| {
            COLLECTING.with(|collecting| {
                let mut collecting = collecting.borrow_mut();
                let (highlight, items) = collecting.as_mut()?;
                items.push(Rc::clone(&choose));
                Some((items.len() - 1, items.len() - 1 == *highlight))
            })
        })
        .flatten();
    let highlighted = place.is_some_and(|(_, highlighted)| highlighted);
    let hover = c.color("hover.overlay");
    let element = div()
        .id(c.element_id(id))
        .flex()
        .w_full()
        .rounded(px(8.0))
        .when(highlighted, |element| {
            element.bg(c.color("pressed.overlay"))
        })
        .when(!highlighted, |element| {
            element.hover(move |style| style.bg(hover))
        })
        .when_some(place, |element, (index, _)| {
            element.on_hover(move |hovered, _, _| {
                if *hovered {
                    PALETTE.with(|palette| {
                        if let Some(palette) = palette.borrow_mut().as_mut()
                            && palette.highlight != index
                        {
                            palette.highlight = index;
                            crate::host::redraw();
                        }
                    });
                }
            })
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            choose(cx)
        });
    let element = match content {
        Some(content) => element.flex_col().p(px(4.0)).children(content),
        None => {
            let secondary = c.color("text.secondary");
            element
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .h(px(36.0))
                .px(px(10.0))
                .text_size(px(14.0))
                .children(props.icon.map(|icon| icons::icon(icon, 16.0, secondary)))
                .child(div().flex_1().overflow_hidden().child(props.label))
                .children(props.detail.map(|detail| {
                    div()
                        .text_size(px(12.0))
                        .text_color(secondary)
                        .child(detail)
                }))
                .children(props.hint.map(|hint| {
                    div()
                        .px(px(5.0))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(c.color("separator"))
                        .text_size(px(11.0))
                        .text_color(secondary)
                        .child(hint)
                }))
        }
    };
    element.into_any_element()
}

/// `Toolbar`: a bar of the window's or page's actions, those before its
/// `Spacer` at the leading end and the rest at the trailing end.
pub fn toolbar(c: &Ctx, leading: Vec<El>, trailing: Vec<El>) -> El {
    // An immersive window hides it.
    if c.win().immersive.get() {
        return div().into_any_element();
    }
    let gap = 8.0;
    let row = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(gap))
        .w_full();
    if !(c.is_under_bars() && c.win().title_bar.get() > 0.0) {
        return row
            .h(px(52.0))
            .px(px(12.0))
            .children(leading)
            .child(div().flex_1())
            .children(trailing)
            .into_any_element();
    }
    // Under the title bar it stands in the bar's row, as tall as the bar
    // and on the window buttons' middle line, its ends clear of them. Its
    // free room moves the window, and its items take their clicks.
    c.win().bar_toolbar_built.set(true);
    let (start, end) = c.win().caption.get();
    // The row's gap stands between the room at an end and the item there.
    // Where the window buttons stand the room does not drag: Windows takes
    // the first area it finds under the pointer, and the toolbar's come
    // before the buttons'.
    let end_room = |caption: f32| {
        let room = div().flex_none().h_full();
        if caption > 0.0 {
            room.w(px(caption - gap))
        } else {
            crate::chrome::drag(room.w(px(12.0 - gap)))
        }
    };
    row.h(px(crate::chrome::TOOLBAR_ROW))
        .child(end_room(start))
        .children(leading)
        .child(crate::chrome::drag(div().flex_1().h_full()))
        .children(trailing)
        .child(end_room(end))
        .into_any_element()
}
