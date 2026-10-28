//! `Table`: rows of data in columns, a `GtkColumnView` over the keyed rows
//! of `items`. The selection is a draft of the rows' keys; a heading's
//! click runs its column's action, which sorts the data where it lives,
//! and `sorted` and `descending` set the heading's arrow; a secondary
//! click or a long press opens the row menu over the selection, which the
//! row under the pointer joins first; a double click or Enter runs the
//! table's action.

use crate::controls::{self, Actions};
use crate::ctx::Field;
use crate::protocol::Value;
use crate::reactive::{Cell, Scope, effect, on_cleanup, untrack};
use crate::value::{as_list, at, key_text, truthy};
use crate::widgets::{MenuItem, bound_action, bound_enabled, bound_text, place};
use crate::{Act, Build, Ctx, Get, Parent, Str};
use adw::prelude::*;
use std::cell::{Cell as StdCell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub struct TableProps {
    pub id: &'static str,
    pub label: Option<Str>,
    /// The draft holding the keys of the chosen rows.
    pub selection: Option<Field>,
}

/// One column: its heading, its width, how the rows are sorted by it, the
/// action of a click on the heading, and the cell of each row.
pub struct TableColumn {
    pub title: Option<Str>,
    pub width: Option<f64>,
    pub sorted: Option<Get>,
    pub descending: Option<Get>,
    pub sort: Option<Act>,
    pub cell: Build,
}

/// A row of the table: its key, as text and as the value the selection
/// holds, and its item, which the cells read. Rows are places in the
/// list: when the items move, each place shows the item now at it, so a
/// sort rebinds no cell. Only places at the end come and go.
struct Row {
    key: Cell<String>,
    key_value: Cell<Value>,
    item: Cell<Value>,
}

/// What a column's sorter compares: nothing, the engine orders the rows.
fn keep_order() -> gtk::CustomSorter {
    gtk::CustomSorter::new(|_, _| gtk::Ordering::Equal)
}

#[allow(non_snake_case, clippy::too_many_arguments)]
pub fn Table(
    p: &Parent,
    ctx: &Ctx,
    props: TableProps,
    items: Get,
    key: usize,
    var: &'static str,
    columns: Vec<TableColumn>,
    menu: Vec<Option<MenuItem>>,
    action: Option<Act>,
) {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::MultiSelection::new(Some(store.clone()));
    let view = gtk::ColumnView::new(Some(selection.clone()));
    view.set_show_row_separators(false);
    view.set_show_column_separators(false);
    view.set_reorderable(false);
    view.add_css_class("data-table");
    let rows: Rc<RefCell<Vec<Rc<Row>>>> = Rc::new(RefCell::new(Vec::new()));
    // Set while the table changes its own selection or headings, so their
    // signals do not echo back.
    let applying = Rc::new(StdCell::new(false));

    // Each bound cell's content, owned until GTK unbinds or the table goes.
    let owners: Rc<RefCell<Vec<(gtk::Box, Scope)>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let owners = Rc::clone(&owners);
        on_cleanup(move || {
            for (_, owner) in owners.borrow_mut().drain(..) {
                owner.dispose();
            }
        });
    }

    let mut gtk_columns = Vec::new();
    let mut sorts: Vec<Option<Rc<dyn Fn()>>> = Vec::new();
    let count = columns.len();
    for (index, column) in columns.iter().enumerate() {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let cell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            cell.set_valign(gtk::Align::Center);
            crate::parent::bound_content(&cell);
            item.set_child(Some(&cell));
        });
        let build = Rc::clone(&column.cell);
        let bound_ctx = ctx.clone();
        let bound_owners = Rc::clone(&owners);
        factory.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let (Some(cell), Some(object)) = (
                item.child().and_downcast::<gtk::Box>(),
                item.item().and_downcast::<glib::BoxedAnyObject>(),
            ) else {
                return;
            };
            let row = Rc::clone(&*object.borrow::<Rc<Row>>());
            let owner = Scope::detached();
            let read = row.item.clone();
            let shown_key = row.key.clone();
            let row_ctx = bound_ctx
                .with_key(Rc::new(move || shown_key.get()))
                .with_local(var, Rc::new(move || read.get()));
            let named = cell.clone();
            let name = row.key.clone();
            owner.run(|| {
                build(&Parent::of_box(&cell), &row_ctx);
                effect(move || named.set_widget_name(&name.get()));
            });
            bound_owners.borrow_mut().push((cell, owner));
        });
        let unbound_owners = Rc::clone(&owners);
        factory.connect_unbind(move |_, item| {
            let Some(cell) = item
                .downcast_ref::<gtk::ListItem>()
                .and_then(|item| item.child())
                .and_downcast::<gtk::Box>()
            else {
                return;
            };
            let mut owners = unbound_owners.borrow_mut();
            if let Some(at) = owners.iter().position(|(widget, _)| *widget == cell) {
                owners.remove(at).1.dispose();
            }
            while let Some(child) = cell.first_child() {
                cell.remove(&child);
            }
        });
        let gtk_column = gtk::ColumnViewColumn::new(None, Some(factory));
        if let Some(title) = &column.title {
            let title = bound_text(ctx, title);
            let shown = gtk_column.clone();
            effect(move || shown.set_title(Some(&title())));
        }
        match column.width {
            Some(width) => {
                gtk_column.set_fixed_width(width.round() as i32);
                gtk_column.set_resizable(true);
            }
            // A column without a width takes the room left, the last one
            // most of all.
            None => gtk_column.set_expand(true),
        }
        if index + 1 == count && column.width.is_some() {
            gtk_column.set_expand(true);
        }
        if column.sort.is_some() || column.sorted.is_some() {
            gtk_column.set_sorter(Some(&keep_order()));
        }
        sorts.push(bound_action(ctx, &column.sort));
        view.append_column(&gtk_column);
        gtk_columns.push(gtk_column);
    }

    // The rows follow `items`: kept rows keep their cells, which read the
    // new item; only a change of order or membership rebinds them.
    {
        let ctx = ctx.clone();
        let store = store.clone();
        let rows = Rc::clone(&rows);
        let selection = selection.clone();
        let applying = Rc::clone(&applying);
        let chosen = props.selection;
        effect(move || {
            let list = as_list(items(&ctx));
            untrack(|| {
                let mut places = rows.borrow_mut();
                // Places at the end come and go; the others show their item.
                if list.len() > places.len() {
                    let added: Vec<Rc<Row>> = (places.len()..list.len())
                        .map(|_| {
                            Rc::new(Row {
                                key: Cell::new(String::new()),
                                key_value: Cell::new(Value::Unit),
                                item: Cell::new(Value::Unit),
                            })
                        })
                        .collect();
                    for (row, item) in added.iter().zip(&list[places.len()..]) {
                        let key_value = at(item.clone(), &[key]);
                        row.key.set(key_text(&key_value));
                        row.key_value.set(key_value);
                        row.item.set(item.clone());
                    }
                    let objects: Vec<glib::BoxedAnyObject> = added
                        .iter()
                        .map(|row| glib::BoxedAnyObject::new(Rc::clone(row)))
                        .collect();
                    applying.set(true);
                    store.splice(places.len() as u32, 0, &objects);
                    applying.set(false);
                    places.extend(added);
                } else if list.len() < places.len() {
                    applying.set(true);
                    store.splice(
                        list.len() as u32,
                        (places.len() - list.len()) as u32,
                        &[] as &[glib::BoxedAnyObject],
                    );
                    applying.set(false);
                    places.truncate(list.len());
                }
                let mut moved = false;
                for (row, item) in places.iter().zip(list) {
                    let key_value = at(item.clone(), &[key]);
                    let text = key_text(&key_value);
                    if row.key.peek() != text {
                        moved = true;
                    }
                    row.key.set(text);
                    row.key_value.set(key_value);
                    row.item.set(item);
                }
                drop(places);
                let same = !moved;
                // The chosen rows stay chosen where they moved.
                if let Some(field) = chosen
                    && !same
                {
                    show_selection(&selection, &rows.borrow(), &ctx.peek(field), &applying);
                }
            });
        });
    }

    // The selection is the draft's: what the user chooses goes into it, and
    // what is written to it elsewhere shows.
    if let Some(field) = props.selection {
        let at_ctx = ctx.clone();
        let read_rows = Rc::clone(&rows);
        let guard = Rc::clone(&applying);
        selection.connect_selection_changed(move |selection, _, _| {
            if guard.get() {
                return;
            }
            let rows = read_rows.borrow();
            let chosen = selection.selection();
            let keys: Vec<Value> = rows
                .iter()
                .enumerate()
                .filter(|(i, _)| chosen.contains(*i as u32))
                .map(|(_, row)| row.key_value.peek())
                .collect();
            at_ctx.write(field, Value::List(keys));
        });
        let shown_ctx = ctx.clone();
        let shown_rows = Rc::clone(&rows);
        let shown_selection = selection.clone();
        let guard = Rc::clone(&applying);
        effect(move || {
            let keys = shown_ctx.read(field);
            untrack(|| show_selection(&shown_selection, &shown_rows.borrow(), &keys, &guard));
        });
    }

    // Headings: a click runs the column's action; the arrow follows state.
    if let Some(sorter) = view.sorter().and_downcast::<gtk::ColumnViewSorter>() {
        let guard = Rc::clone(&applying);
        let clicked = gtk_columns.clone();
        let sorts = sorts.clone();
        sorter.connect_changed(move |sorter, _| {
            if guard.get() {
                return;
            }
            let Some(primary) = sorter.primary_sort_column() else {
                return;
            };
            if let Some(index) = clicked.iter().position(|c| *c == primary)
                && let Some(sort) = &sorts[index]
            {
                sort();
            }
        });
        let state_ctx = ctx.clone();
        let guard = Rc::clone(&applying);
        let shown_view = view.clone();
        let heads: Vec<(Option<Get>, Option<Get>)> = columns
            .iter()
            .map(|c| (c.sorted.clone(), c.descending.clone()))
            .collect();
        let shown_columns = gtk_columns.clone();
        effect(move || {
            let mut chosen = None;
            for (index, (sorted, descending)) in heads.iter().enumerate() {
                if sorted.as_ref().is_some_and(|s| truthy(&s(&state_ctx))) {
                    let down = descending.as_ref().is_some_and(|d| truthy(&d(&state_ctx)));
                    chosen = Some((index, down));
                }
            }
            untrack(|| {
                guard.set(true);
                match chosen {
                    Some((index, down)) => shown_view.sort_by_column(
                        Some(&shown_columns[index]),
                        if down {
                            gtk::SortType::Descending
                        } else {
                            gtk::SortType::Ascending
                        },
                    ),
                    None => shown_view.sort_by_column(None, gtk::SortType::Ascending),
                }
                guard.set(false);
            });
        });
    }

    // The primary action: a double click or Enter on the chosen rows.
    let primary = bound_action(ctx, &action);
    if let Some(primary) = primary.clone() {
        view.connect_activate(move |_, _| primary());
    }

    // The row menu.
    if !menu.is_empty() {
        let popover = row_menu(ctx, &view, menu);
        let open_at = {
            let view = view.clone();
            let rows = Rc::clone(&rows);
            let selection = selection.clone();
            let ctx = ctx.clone();
            let chosen = props.selection;
            let applying = Rc::clone(&applying);
            move |x: f64, y: f64| {
                // The row under the pointer joins the selection first.
                let key = {
                    let rows = rows.borrow();
                    row_key_at(&view, x, y, |name| {
                        rows.iter().any(|row| row.key.peek() == name)
                    })
                };
                if let Some(key) = key {
                    let rows = rows.borrow();
                    if let Some(index) = rows.iter().position(|row| row.key.peek() == key)
                        && !selection.is_selected(index as u32)
                    {
                        match chosen {
                            Some(field) => {
                                ctx.write(field, Value::List(vec![rows[index].key_value.peek()]));
                                show_selection(
                                    &selection,
                                    &rows,
                                    &Value::List(vec![rows[index].key_value.peek()]),
                                    &applying,
                                );
                            }
                            None => {
                                selection.select_item(index as u32, true);
                            }
                        }
                    }
                }
                popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                    x.round() as i32,
                    y.round() as i32,
                    1,
                    1,
                )));
                popover.popup();
            }
        };
        let open_at = Rc::new(open_at);
        let click = gtk::GestureClick::new();
        click.set_button(gtk::gdk::BUTTON_SECONDARY);
        let open = Rc::clone(&open_at);
        click.connect_pressed(move |gesture, _, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            open(x, y);
        });
        view.add_controller(click);
        let press = gtk::GestureLongPress::new();
        press.set_touch_only(true);
        let open = Rc::clone(&open_at);
        press.connect_pressed(move |gesture, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            open(x, y);
        });
        view.add_controller(press);
        // The menu key and Shift+F10, as a context menu opens elsewhere.
        let keys = gtk::ShortcutController::new();
        let open = Rc::clone(&open_at);
        let shown_view = view.clone();
        keys.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Menu|<Shift>F10"),
            Some(gtk::CallbackAction::new(move |_, _| {
                let width = f64::from(shown_view.width());
                open(width / 2.0, 24.0);
                glib::Propagation::Stop
            })),
        ));
        view.add_controller(keys);
    }

    let label = props.label.as_ref().map(|label| bound_text(ctx, label));
    if let Some(label) = label.clone() {
        let named = view.clone();
        effect(move || {
            named.update_property(&[gtk::accessible::Property::Label(&label())]);
        });
    }
    let selected_rows = Rc::clone(&rows);
    let selected = selection.clone();
    let choose_rows = Rc::clone(&rows);
    let choose_ctx = ctx.clone();
    let choose_selection = selection.clone();
    let choose_guard = Rc::clone(&applying);
    let chosen_field = props.selection;
    controls::register(
        ctx,
        props.id,
        "table",
        label,
        None,
        Actions {
            activate: primary,
            // `ui.select` with the keys of rows, separated by commas.
            select: Some(Rc::new(move |text| {
                let wanted: Vec<&str> = text
                    .split(',')
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                    .collect();
                let rows = choose_rows.borrow();
                let keys: Vec<Value> = rows
                    .iter()
                    .filter(|row| wanted.contains(&row.key.peek().as_str()))
                    .map(|row| row.key_value.peek())
                    .collect();
                if keys.len() != wanted.len() {
                    return false;
                }
                let keys = Value::List(keys);
                if let Some(field) = chosen_field {
                    choose_ctx.write(field, keys.clone());
                }
                show_selection(&choose_selection, &rows, &keys, &choose_guard);
                true
            })),
            value: Some(Rc::new(move || {
                let rows = selected_rows.borrow();
                let chosen = selected.selection();
                Some(
                    rows.iter()
                        .enumerate()
                        .filter(|(i, _)| chosen.contains(*i as u32))
                        .map(|(_, row)| row.key.peek())
                        .collect::<Vec<_>>()
                        .join(","),
                )
            })),
            ..Actions::default()
        },
    );

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&view));
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.set_has_frame(true);
    place(p, &scroll);
}

/// Choose exactly the rows whose keys `keys` lists.
fn show_selection(
    selection: &gtk::MultiSelection,
    rows: &[Rc<Row>],
    keys: &Value,
    guard: &StdCell<bool>,
) {
    let wanted = match keys {
        Value::List(keys) => keys.clone(),
        _ => Vec::new(),
    };
    let chosen = gtk::Bitset::new_empty();
    for (index, row) in rows.iter().enumerate() {
        if wanted.contains(&row.key_value.peek()) {
            chosen.add(index as u32);
        }
    }
    let all = gtk::Bitset::new_range(0, rows.len() as u32);
    if selection.selection() == chosen {
        return;
    }
    guard.set(true);
    selection.set_selection(&chosen, &all);
    guard.set(false);
}

/// The key of the row at `(x, y)` in the view: each cell is a box named
/// after its row's key.
fn row_key_at(
    view: &gtk::ColumnView,
    x: f64,
    y: f64,
    is_key: impl Fn(&str) -> bool,
) -> Option<String> {
    let mut widget = view.pick(x, y, gtk::PickFlags::DEFAULT);
    while let Some(current) = widget {
        if current == *view.upcast_ref::<gtk::Widget>() {
            return None;
        }
        if current.is::<gtk::Box>() {
            let name = current.widget_name();
            if is_key(&name) {
                return Some(name.to_string());
            }
        }
        widget = current.parent();
    }
    None
}

/// The popover menu of the rows, with an action per item; each item is
/// also a control, so external entries reach it while the menu is closed.
fn row_menu(ctx: &Ctx, view: &gtk::ColumnView, items: Vec<Option<MenuItem>>) -> gtk::PopoverMenu {
    let group = gio::SimpleActionGroup::new();
    let mut entries = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Some(item) = item else {
            entries.push(None);
            continue;
        };
        let name = format!("i{index}");
        let action = gio::SimpleAction::new(&name, None);
        let (run, set_file) =
            crate::features::picking(ctx, item.pick, bound_action(ctx, &item.action));
        let run = crate::features::pasting(ctx, item.paste, run);
        if let Some(run) = run.clone() {
            action.connect_activate(move |_, _| run());
        }
        let enabled = bound_enabled(ctx, &item.enabled);
        if let Some(enabled) = enabled.clone() {
            let shown = action.clone();
            effect(move || shown.set_enabled(enabled()));
        }
        group.add_action(&action);
        let label = item
            .label
            .as_ref()
            .map(|label| bound_text(ctx, label))
            .unwrap_or_else(|| Rc::new(String::new));
        controls::register(
            ctx,
            item.id,
            "button",
            Some(Rc::clone(&label)),
            enabled,
            Actions {
                activate: run,
                set_file,
                file_kinds: item.pick.map(|pick| pick.kinds),
                ..Actions::default()
            },
        );
        entries.push(Some((format!("row.{name}"), label)));
    }
    view.insert_action_group("row", Some(&group));
    let popover = gtk::PopoverMenu::from_model(None::<&gio::MenuModel>);
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.set_parent(view);
    let shown = popover.clone();
    effect(move || {
        let menu = gio::Menu::new();
        let mut section = gio::Menu::new();
        for entry in &entries {
            match entry {
                Some((action, label)) => section.append(Some(&label()), Some(action)),
                None => {
                    menu.append_section(None, &section);
                    section = gio::Menu::new();
                }
            }
        }
        menu.append_section(None, &section);
        shown.set_menu_model(Some(&menu));
    });
    let owned = popover.clone();
    on_cleanup(move || owned.unparent());
    popover
}
