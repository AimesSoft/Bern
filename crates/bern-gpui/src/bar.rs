//! `MenuBar`: the application's menus in the macOS menu bar, beside the
//! system's standard menus. A `CommandMenu` with a `role` adds its items to
//! the system's menu of that name where SwiftUI puts them (`host::
//! set_menus`); one without is a menu of its own between the View and
//! Window menus. The items are built with the primary window's context each
//! time it draws, and the menus are set again when what they show changes.

use crate::ctx::{Ctx, Press};
use gpui::{App, KeyBinding, Menu, MenuItem, SharedString};
use std::cell::RefCell;
use std::collections::HashSet;

/// A menu of the menu bar: its title, and the system's menu it adds to.
pub struct BarMenu {
    label: String,
    role: Option<&'static str>,
    entries: Vec<BarEntry>,
}

/// An item of a menu bar menu.
pub enum BarEntry {
    Item {
        target: String,
        label: String,
        /// GPUI keystrokes, such as `cmd-shift-o`.
        keys: Option<String>,
        enabled: bool,
        checked: bool,
        press: Press,
    },
    Menu {
        label: String,
        entries: Vec<BarEntry>,
    },
    Divider,
}

/// A `CommandMenu` directly in the `MenuBar`.
pub fn bar_menu(label: String, role: Option<&'static str>, entries: Vec<BarEntry>) -> BarMenu {
    BarMenu {
        label,
        role,
        entries,
    }
}

/// A `CommandMenu` inside another: a submenu.
pub fn bar_submenu(label: String, entries: Vec<BarEntry>) -> BarEntry {
    BarEntry::Menu { label, entries }
}

/// A `CommandItem`: its action, with the open panel first when it picks.
#[allow(clippy::too_many_arguments)]
pub fn bar_item(
    c: &Ctx,
    id: &'static str,
    label: String,
    shortcut: Option<String>,
    enabled: bool,
    checked: bool,
    pick: Option<(crate::Field, &'static str)>,
    action: Option<crate::Act>,
) -> BarEntry {
    BarEntry::Item {
        target: c.target(id),
        label,
        keys: shortcut
            .as_deref()
            .map(keystroke)
            .filter(|keys| !keys.is_empty()),
        enabled: enabled && c.enabled(),
        checked,
        press: crate::controls::press(c, pick, action),
    }
}

/// A Bern shortcut (`cmd+shift+o`, `return`) as GPUI keystrokes.
fn keystroke(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| match part.trim().to_lowercase().as_str() {
            "return" => "enter".to_string(),
            "delete" => "backspace".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// A command of the menu bar: the item, and the keys it has now, so keys
/// it no longer has pass on to the window.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = bern, no_json)]
pub(crate) struct BarCommand {
    target: SharedString,
    keys: SharedString,
}

/// The command of an item that is greyed out. Nothing handles it, so the
/// system shows the item disabled, still with its keys, and the keys pass
/// on.
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = bern, no_json)]
pub(crate) struct DisabledCommand {
    target: SharedString,
    keys: SharedString,
}

thread_local! {
    /// The menus as the primary window last built them.
    static MENUS: RefCell<Vec<BarMenu>> = const { RefCell::new(Vec::new()) };
    /// What the menu bar shows of them, to set it again only on a change.
    static SHOWN: RefCell<String> = const { RefCell::new(String::new()) };
    /// The keys bound so far, with their item.
    static BOUND: RefCell<HashSet<(String, String)>> = RefCell::new(HashSet::new());
}

/// Run the commands of the menu bar's items.
pub(crate) fn install(cx: &mut App) {
    cx.on_action(
        |command: &BarCommand, cx| match press(&command.target, &command.keys) {
            Some(press) => press(cx),
            None => cx.propagate(),
        },
    );
}

/// The action of the enabled item `target` while its keys are `keys`.
fn press(target: &str, keys: &str) -> Option<Press> {
    fn find(entries: &[BarEntry], target: &str, keys: &str) -> Option<Press> {
        entries.iter().find_map(|entry| match entry {
            BarEntry::Item {
                target: own,
                keys: own_keys,
                enabled: true,
                press,
                ..
            } if own == target && own_keys.as_deref().unwrap_or_default() == keys => {
                Some(press.clone())
            }
            BarEntry::Menu { entries, .. } => find(entries, target, keys),
            _ => None,
        })
    }
    MENUS.with(|menus| {
        menus
            .borrow()
            .iter()
            .find_map(|menu| find(&menu.entries, target, keys))
    })
}

/// Show `menus`, which the primary window built, setting the menu bar
/// again when what it shows differs.
pub(crate) fn show(menus: Vec<BarMenu>, cx: &mut App) {
    let mut shown = crate::text::language();
    let mut bindings = Vec::new();
    fn walk(entries: &[BarEntry], shown: &mut String, bindings: &mut Vec<KeyBinding>) {
        for entry in entries {
            match entry {
                BarEntry::Item {
                    target,
                    label,
                    keys,
                    enabled,
                    checked,
                    ..
                } => {
                    let keys = keys.clone().unwrap_or_default();
                    shown.push_str(&format!("|{target}:{label}:{keys}:{enabled}:{checked}"));
                    if !keys.is_empty()
                        && BOUND
                            .with(|bound| bound.borrow_mut().insert((keys.clone(), target.clone())))
                    {
                        // Keys without a command key leave text fields
                        // their typing.
                        let context = (!["cmd-", "ctrl-", "alt-"]
                            .iter()
                            .any(|modifier| keys.contains(modifier)))
                        .then_some("!Input");
                        let (target, keys_text) = (
                            SharedString::from(target.clone()),
                            SharedString::from(keys.clone()),
                        );
                        // Bound once for good: each pair of keys and item
                        // is bound only the first time it shows.
                        let keys: &'static str = Box::leak(keys.into_boxed_str());
                        bindings.push(KeyBinding::new(
                            keys,
                            BarCommand {
                                target: target.clone(),
                                keys: keys_text.clone(),
                            },
                            context,
                        ));
                        bindings.push(KeyBinding::new(
                            keys,
                            DisabledCommand {
                                target,
                                keys: keys_text,
                            },
                            context,
                        ));
                    }
                }
                BarEntry::Menu { label, entries } => {
                    shown.push_str(&format!("|{label}["));
                    walk(entries, shown, bindings);
                    shown.push(']');
                }
                BarEntry::Divider => shown.push_str("|-"),
            }
        }
    }
    for menu in &menus {
        shown.push_str(&format!("#{}:{:?}", menu.label, menu.role));
        walk(&menu.entries, &mut shown, &mut bindings);
    }
    MENUS.with(|current| *current.borrow_mut() = menus);
    let changed = SHOWN.with(|current| {
        let changed = *current.borrow() != shown;
        *current.borrow_mut() = shown;
        changed
    });
    if !bindings.is_empty() {
        cx.bind_keys(bindings);
    }
    if changed {
        cx.defer(|cx| crate::host::set_menus(crate::host::host().def, cx));
    }
}

/// The items of the menus with `role`, each menu's after a separator.
pub(crate) fn items(role: &str) -> Vec<MenuItem> {
    MENUS.with(|menus| {
        let mut items = Vec::new();
        for menu in menus.borrow().iter().filter(|menu| menu.role == Some(role)) {
            if !items.is_empty() {
                items.push(MenuItem::separator());
            }
            items.extend(menu.entries.iter().map(item));
        }
        items
    })
}

/// The menus of the application's own, without a role.
pub(crate) fn own_menus() -> Vec<Menu> {
    MENUS.with(|menus| {
        menus
            .borrow()
            .iter()
            .filter(|menu| menu.role.is_none())
            .map(|menu| Menu::new(menu.label.clone()).items(menu.entries.iter().map(item)))
            .collect()
    })
}

fn item(entry: &BarEntry) -> MenuItem {
    match entry {
        BarEntry::Item {
            target,
            label,
            keys,
            enabled,
            checked,
            ..
        } => {
            let (target, keys) = (
                SharedString::from(target.clone()),
                SharedString::from(keys.clone().unwrap_or_default()),
            );
            let item = if *enabled {
                MenuItem::action(label.clone(), BarCommand { target, keys })
            } else {
                MenuItem::action(label.clone(), DisabledCommand { target, keys }).disabled(true)
            };
            item.checked(*checked)
        }
        BarEntry::Menu { label, entries } => {
            MenuItem::submenu(Menu::new(label.clone()).items(entries.iter().map(item)))
        }
        BarEntry::Divider => MenuItem::separator(),
    }
}
