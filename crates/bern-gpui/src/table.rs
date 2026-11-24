//! `Table`: rows of data in columns, keyed by `key`.
//!
//! `selection` holds the keys of the chosen rows (click selects one,
//! Cmd/Ctrl toggles, Shift selects a range); clicking a column heading runs
//! that column's sort action while `sorted` and `descending` draw the sort
//! chevron; secondary click opens the `TableMenu` (adding an unselected row
//! to the selection first); double click or Enter runs the table's primary
//! action.

use crate::ctx::{Act, Ctx, Field};
use crate::menus::{MenuEntry, entry_list};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::prelude::FluentBuilder;
use gpui::{
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Configuration of a `Table`.
#[derive(Clone, Default)]
pub struct TableProps {
    pub label: String,
    pub selection: Option<Field>,
}

/// One column of a `Table`.
pub struct TableColumnDef {
    pub title: String,
    pub width: Option<f32>,
    pub sorted: bool,
    pub descending: bool,
    pub sort: Option<Act>,
    pub cell: Rc<dyn Fn(&Ctx) -> El>,
}

/// One pre-evaluated row of a `Table`.
pub struct TableRowData {
    pub key_text: String,
    pub key_value: Value,
    pub item: Value,
}

thread_local! {
    /// Anchor row index for Shift+click range selection, keyed by table target.
    static ANCHOR: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

fn read_selection(c: &Ctx, selection: Option<Field>) -> Vec<Value> {
    selection
        .map(|field| crate::value::as_list(c.read(field)))
        .unwrap_or_default()
}

fn key_matches(a: &Value, b: &Value) -> bool {
    a == b || crate::value::key_text(a) == crate::value::key_text(b)
}

#[allow(clippy::too_many_arguments)]
pub type TableMenuFn = Rc<dyn Fn(&Ctx) -> Vec<MenuEntry>>;

#[allow(clippy::too_many_arguments)]
pub fn table(
    c: &Ctx,
    id: &'static str,
    props: TableProps,
    rows: Vec<TableRowData>,
    columns: Vec<TableColumnDef>,
    var_name: &'static str,
    primary: Option<Act>,
    menu: Option<TableMenuFn>,
) -> El {
    let enabled = c.enabled();
    let table_key = c.key(id);
    let selected = read_selection(c, props.selection);
    let selected_summary = selected
        .iter()
        .map(crate::value::key_text)
        .collect::<Vec<_>>()
        .join(",");

    let select_cli: Option<crate::session::Select> = props.selection.map(|field| {
        let c = c.clone();
        let row_keys: Vec<(String, Value)> = rows
            .iter()
            .map(|r| (r.key_text.clone(), r.key_value.clone()))
            .collect();
        Rc::new(move |wanted: &str| {
            let Some((_, key_val)) = row_keys.iter().find(|(k, _)| k == wanted) else {
                return false;
            };
            c.write(field, Value::List(vec![key_val.clone()]));
            true
        }) as crate::session::Select
    });
    let activate_cli: Option<crate::ctx::Press> = primary.as_ref().map(|action| {
        let c = c.clone();
        let action = Rc::clone(action);
        Rc::new(move |_: &mut gpui::App| action(&c)) as crate::ctx::Press
    });

    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "table",
        label: props.label.clone(),
        value: Some(selected_summary),
        enabled,
        actions: Actions {
            activate: activate_cli,
            select: select_cli,
            ..Actions::default()
        },
    });

    let border = c.color("separator");
    let header_bg = c.color("surface.panel");
    let hover_bg = c.slot("row", "hover", "background");
    let sel_bg = c.color("selection.background").opacity(0.18);
    let text_primary = c.color("text.primary");
    let text_secondary = c.color("text.secondary");

    // Header row.
    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .min_h(px(32.0))
        .bg(header_bg)
        .border_b_1()
        .border_color(border);

    for (col_idx, col) in columns.iter().enumerate() {
        let col_cell = div()
            .id(c.element_id(&format!("{id}.col.{col_idx}")))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .px(px(10.0))
            .h(px(32.0))
            .text_size(px(12.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text_secondary)
            .overflow_hidden()
            .map(|el| match col.width {
                Some(w) => el.w(px(w)).flex_none(),
                None => el.flex_1().min_w(px(60.0)),
            })
            .child(div().truncate().child(col.title.clone()))
            .when(col.sorted, |el| {
                let arrow = if col.descending {
                    "chevron.down"
                } else {
                    "chevron.up"
                };
                el.child(icons::icon(arrow, 11.0, text_secondary))
            });
        let col_cell = if enabled && let Some(sort) = &col.sort {
            let sort = Rc::clone(sort);
            let ctx = c.clone();
            col_cell
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    sort(&ctx);
                })
        } else {
            col_cell
        };
        header = header.child(col_cell);
    }

    let menu_target = format!("{table_key}#table-menu");
    let menu_open = crate::overlay::is_open(&menu_target);

    // Body rows.
    let all_keys: Rc<Vec<Value>> = Rc::new(rows.iter().map(|r| r.key_value.clone()).collect());
    let mut body = div().flex().flex_col().w_full();
    for (row_idx, row) in rows.iter().enumerate() {
        let is_selected = selected.iter().any(|k| key_matches(k, &row.key_value));
        let row_ctx = c
            .with_key(row.key_text.clone())
            .with_local(var_name, row.item.clone());

        let mut row_el = div()
            .id(c.element_id(&format!("{id}.row.{}", row.key_text)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .min_h(px(34.0))
            .border_b_1()
            .border_color(border.opacity(0.5))
            .text_color(text_primary)
            .when(is_selected, |el| el.bg(sel_bg))
            .when(!is_selected && enabled, |el| {
                el.hover(move |s| s.bg(hover_bg))
            });

        for col in &columns {
            let cell_el = (col.cell)(&row_ctx);
            let cell_wrap = div()
                .flex()
                .flex_row()
                .items_center()
                .px(px(10.0))
                .py(px(4.0))
                .overflow_hidden()
                .map(|el| match col.width {
                    Some(w) => el.w(px(w)).flex_none(),
                    None => el.flex_1().min_w(px(60.0)),
                })
                .child(cell_el);
            row_el = row_el.child(cell_wrap);
        }

        if enabled {
            let selection = props.selection;
            let key_value = row.key_value.clone();
            let all_keys_click = Rc::clone(&all_keys);
            let table_key_click = table_key.clone();
            let c_click = c.clone();
            let primary_click = primary.clone();
            row_el = row_el.on_mouse_down(MouseButton::Left, move |event, _, cx| {
                cx.stop_propagation();
                if let Some(field) = selection {
                    let toggle_mod = if cfg!(target_os = "macos") {
                        event.modifiers.platform
                    } else {
                        event.modifiers.control
                    };
                    let mut next = read_selection(&c_click, Some(field));
                    if event.modifiers.shift {
                        let anchor = ANCHOR
                            .with(|a| a.borrow().get(&table_key_click).copied())
                            .unwrap_or(row_idx);
                        let (lo, hi) = if anchor <= row_idx {
                            (anchor, row_idx)
                        } else {
                            (row_idx, anchor)
                        };
                        next = all_keys_click[lo..=hi.min(all_keys_click.len().saturating_sub(1))]
                            .to_vec();
                    } else if toggle_mod {
                        if let Some(pos) = next.iter().position(|k| key_matches(k, &key_value)) {
                            next.remove(pos);
                        } else {
                            next.push(key_value.clone());
                        }
                        ANCHOR.with(|a| a.borrow_mut().insert(table_key_click.clone(), row_idx));
                    } else {
                        next = vec![key_value.clone()];
                        ANCHOR.with(|a| a.borrow_mut().insert(table_key_click.clone(), row_idx));
                    }
                    c_click.write(field, Value::List(next));
                }
                if event.click_count >= 2
                    && let Some(action) = &primary_click
                {
                    action(&c_click);
                }
            });

            if menu.is_some() {
                let selection = props.selection;
                let key_value = row.key_value.clone();
                let c_menu = c.clone();
                let menu_target_open = menu_target.clone();
                row_el = row_el.on_mouse_down(MouseButton::Right, move |event, _, cx| {
                    cx.stop_propagation();
                    if let Some(field) = selection {
                        let cur = read_selection(&c_menu, Some(field));
                        if !cur.iter().any(|k| key_matches(k, &key_value)) {
                            c_menu.write(field, Value::List(vec![key_value.clone()]));
                        }
                    }
                    crate::overlay::open_at(&menu_target_open, event.position);
                });
            }
        }
        body = body.child(row_el);
    }

    let menu_panel = if menu_open && let Some(build_menu) = &menu {
        let entries = build_menu(c);
        Some(crate::overlay::panel(
            c,
            &menu_target,
            entry_list(c, &menu_target, entries, false),
        ))
    } else {
        None
    };

    let primary_enter = primary.clone();
    let c_enter = c.clone();
    let container = div()
        .id(c.element_id(id))
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(8.0))
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .on_key_down(move |event: &KeyDownEvent, _, _| {
            if enabled
                && event.keystroke.key == "enter"
                && let Some(action) = &primary_enter
            {
                action(&c_enter);
            }
        })
        .child(header)
        .child(
            div()
                .id(c.element_id(&format!("{id}.scroll")))
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .child(body),
        )
        .children(menu_panel);

    c.grow().apply(container).into_any_element()
}
