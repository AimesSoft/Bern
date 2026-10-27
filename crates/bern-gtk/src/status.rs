//! The application's `StatusItem` in the status area of Linux desktops. GTK
//! has no widget for it: the desktop shows a StatusNotifierItem that the
//! application puts on the session bus, with a `com.canonical.dbusmenu`
//! menu. KDE Plasma shows these itself, GNOME through the AppIndicator
//! extension that Ubuntu ships. The icon is the application icon; a click
//! opens the primary window, and the menu opens the primary window, has the
//! application's items and quits.

use crate::ctx::Ctx;
use crate::host::host;
use crate::protocol::Value;
use crate::reactive::{Scope, effect, untrack};
use crate::{Act, Str, windows};
use glib::prelude::*;
use glib::{Variant, VariantDict, VariantTy};
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use std::cell::RefCell;

/// The `StatusItem` of the declaration: its tooltip, and the application's
/// items of its menu, which the generated code adds each time the menu is
/// built.
pub struct StatusItemDef {
    pub label: Str,
    pub menu: fn(&Ctx, &mut StatusMenu),
}

/// An item of the menu: a command, a divider or a submenu.
struct Entry {
    label: String,
    enabled: bool,
    checked: Option<bool>,
    separator: bool,
    children: Vec<Entry>,
    /// What choosing it runs, in the context it was added in (a `ForEach`
    /// row's, say).
    run: Option<(Act, Ctx)>,
}

impl Entry {
    fn same(&self, other: &Entry) -> bool {
        self.label == other.label
            && self.enabled == other.enabled
            && self.checked == other.checked
            && self.separator == other.separator
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(a, b)| a.same(b))
    }
}

/// The menu while it is built. Dividers come between items only: never
/// first, last or twice in a row.
#[derive(Default)]
pub struct StatusMenu {
    entries: Vec<Entry>,
    divider: bool,
}

impl StatusMenu {
    pub fn item(
        &mut self,
        c: &Ctx,
        label: String,
        enabled: bool,
        checked: Option<bool>,
        act: Option<Act>,
    ) {
        self.separate();
        self.entries.push(Entry {
            label,
            enabled,
            checked,
            separator: false,
            children: Vec::new(),
            run: act.map(|act| (act, c.clone())),
        });
    }

    pub fn divider(&mut self) {
        self.divider = true;
    }

    pub fn sub(&mut self, label: String, fill: impl FnOnce(&mut StatusMenu)) {
        let mut sub = StatusMenu::default();
        fill(&mut sub);
        if sub.entries.is_empty() {
            return;
        }
        self.separate();
        self.entries.push(Entry {
            label,
            enabled: true,
            checked: None,
            separator: false,
            children: sub.entries,
            run: None,
        });
    }

    fn separate(&mut self) {
        if std::mem::take(&mut self.divider) && !self.entries.is_empty() {
            self.entries.push(Entry {
                label: String::new(),
                enabled: true,
                checked: None,
                separator: true,
                children: Vec::new(),
                run: None,
            });
        }
    }
}

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";

const INTERFACES: &str = r#"<node>
  <interface name="org.kde.StatusNotifierItem">
    <method name="ContextMenu"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="Activate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="SecondaryActivate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
    <method name="Scroll"><arg name="delta" type="i" direction="in"/><arg name="orientation" type="s" direction="in"/></method>
    <method name="ProvideXdgActivationToken"><arg name="token" type="s" direction="in"/></method>
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="WindowId" type="i" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconPixmap" type="a(iiay)" access="read"/>
    <property name="OverlayIconName" type="s" access="read"/>
    <property name="OverlayIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionIconName" type="s" access="read"/>
    <property name="AttentionIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionMovieName" type="s" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <signal name="NewTitle"/>
    <signal name="NewIcon"/>
    <signal name="NewAttentionIcon"/>
    <signal name="NewOverlayIcon"/>
    <signal name="NewToolTip"/>
    <signal name="NewStatus"><arg name="status" type="s"/></signal>
  </interface>
  <interface name="com.canonical.dbusmenu">
    <method name="GetLayout">
      <arg name="parentId" type="i" direction="in"/><arg name="recursionDepth" type="i" direction="in"/><arg name="propertyNames" type="as" direction="in"/>
      <arg name="revision" type="u" direction="out"/><arg name="layout" type="(ia{sv}av)" direction="out"/>
    </method>
    <method name="GetGroupProperties">
      <arg name="ids" type="ai" direction="in"/><arg name="propertyNames" type="as" direction="in"/>
      <arg name="properties" type="a(ia{sv})" direction="out"/>
    </method>
    <method name="GetProperty">
      <arg name="id" type="i" direction="in"/><arg name="name" type="s" direction="in"/><arg name="value" type="v" direction="out"/>
    </method>
    <method name="Event">
      <arg name="id" type="i" direction="in"/><arg name="eventId" type="s" direction="in"/><arg name="data" type="v" direction="in"/><arg name="timestamp" type="u" direction="in"/>
    </method>
    <method name="EventGroup">
      <arg name="events" type="a(isvu)" direction="in"/><arg name="idErrors" type="ai" direction="out"/>
    </method>
    <method name="AboutToShow">
      <arg name="id" type="i" direction="in"/><arg name="needUpdate" type="b" direction="out"/>
    </method>
    <method name="AboutToShowGroup">
      <arg name="ids" type="ai" direction="in"/><arg name="updatesNeeded" type="ai" direction="out"/><arg name="idErrors" type="ai" direction="out"/>
    </method>
    <property name="Version" type="u" access="read"/>
    <property name="TextDirection" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="IconThemePath" type="as" access="read"/>
    <signal name="ItemsPropertiesUpdated"><arg type="a(ia{sv})"/><arg type="a(ias)"/></signal>
    <signal name="LayoutUpdated"><arg name="revision" type="u"/><arg name="parent" type="i"/></signal>
    <signal name="ItemActivationRequested"><arg name="id" type="i"/><arg name="timestamp" type="u"/></signal>
  </interface>
</node>"#;

/// The item as the bus sees it.
struct Item {
    def: &'static StatusItemDef,
    connection: Option<gio::DBusConnection>,
    name: String,
    title: String,
    /// The application icon at the sizes status areas use, as ARGB32 in
    /// network byte order.
    pixmaps: Vec<(i32, i32, Vec<u8>)>,
    /// The menu with its top-level entries; ids are positions in a
    /// depth-first walk, from 1 (0 is the root).
    menu: Vec<Entry>,
    revision: u32,
    /// The token the status area gave for raising the window a click
    /// opens.
    activation: Option<String>,
    /// Whether the bus name is ours, the watcher's unique name while there
    /// is one, and the watcher the item registered with: it registers once
    /// with each watcher, once both are there.
    owned: bool,
    watcher: Option<String>,
    registered: Option<String>,
}

thread_local! {
    static ITEM: RefCell<Option<Item>> = const { RefCell::new(None) };
    /// Owns the effects that follow the tooltip and the menu.
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

fn with_item<R>(body: impl FnOnce(&mut Item) -> R) -> Option<R> {
    ITEM.with(|item| item.borrow_mut().as_mut().map(body))
}

/// What the menu reads: engine state, with the primary window, while it is
/// open, for showing failures.
fn context() -> Ctx {
    let primary = host().def.primary;
    let win = windows::all()
        .into_iter()
        .find(|win| win.kind_id() == primary);
    Ctx::detached("status_item", win)
}

/// Put the status item on the session bus; the desktop's status area
/// shows it once its watcher knows it, now or when one starts.
pub(crate) fn show(def: &'static StatusItemDef) {
    let host = host();
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    ITEM.with(|item| {
        *item.borrow_mut() = Some(Item {
            def,
            connection: None,
            name: name.clone(),
            title: String::new(),
            pixmaps: pixmaps(host.def.bundle_id),
            menu: Vec::new(),
            revision: 0,
            activation: None,
            owned: false,
            watcher: None,
            registered: None,
        })
    });
    let scope = Scope::detached();
    scope.run(|| {
        // The title and tooltip follow the label: the state it reads, the
        // language.
        effect(move || {
            let title = (def.label)(&context());
            let changed = with_item(|item| {
                let changed = item.title != title;
                item.title = title;
                changed.then(|| item.connection.clone()).flatten()
            })
            .flatten();
            if let Some(connection) = changed {
                for signal in ["NewTitle", "NewToolTip"] {
                    emit(
                        &connection,
                        ITEM_PATH,
                        "org.kde.StatusNotifierItem",
                        signal,
                        None,
                    );
                }
            }
        });
        // The menu follows the state its items read.
        effect(|| {
            rebuild();
        });
    });
    SCOPE.with(|owner| *owner.borrow_mut() = Some(scope));
    let _ = gio::bus_own_name(
        gio::BusType::Session,
        &name,
        gio::BusNameOwnerFlags::NONE,
        |connection, _| register_objects(&connection),
        |connection, _| {
            with_item(|item| item.owned = true);
            register_with_watcher(&connection);
        },
        |_, name| eprintln!("bern: the status item could not take the bus name {name}"),
    );
    // A status area that starts later, or restarts, learns of the item
    // again.
    let _ = gio::bus_watch_name(
        gio::BusType::Session,
        WATCHER,
        gio::BusNameWatcherFlags::NONE,
        |connection, _, owner| {
            let owner = owner.to_string();
            with_item(|item| item.watcher = Some(owner));
            register_with_watcher(&connection);
        },
        |_, _| {
            with_item(|item| {
                item.watcher = None;
                item.registered = None;
            });
        },
    );
}

fn register_objects(connection: &gio::DBusConnection) {
    with_item(|item| item.connection = Some(connection.clone()));
    let node = match gio::DBusNodeInfo::for_xml(INTERFACES) {
        Ok(node) => node,
        Err(error) => {
            eprintln!("bern: the status item's interfaces: {error}");
            return;
        }
    };
    for (path, interface) in [
        (ITEM_PATH, "org.kde.StatusNotifierItem"),
        (MENU_PATH, "com.canonical.dbusmenu"),
    ] {
        let Some(info) = node.lookup_interface(interface) else {
            continue;
        };
        let registered = connection
            .register_object(path, &info)
            .method_call(|_, _, _, interface, method, params, invocation| {
                let reply = match interface {
                    Some("com.canonical.dbusmenu") => menu_call(method, &params),
                    _ => item_call(method, &params),
                };
                invocation.return_value(reply.as_ref());
            })
            .property(|_, _, _, interface, property| match interface {
                "com.canonical.dbusmenu" => menu_property(property),
                _ => item_property(property),
            })
            .build();
        if let Err(error) = registered {
            eprintln!("bern: cannot put the status item on the bus: {error}");
        }
    }
}

/// Register with the watcher once the bus name is ours and a watcher is
/// there, once per watcher.
fn register_with_watcher(connection: &gio::DBusConnection) {
    let Some(name) = with_item(|item| {
        let watcher = item.watcher.clone().filter(|_| item.owned)?;
        if item.registered.as_ref() == Some(&watcher) {
            return None;
        }
        item.registered = Some(watcher);
        Some(item.name.clone())
    })
    .flatten() else {
        return;
    };
    connection.call(
        Some(WATCHER),
        "/StatusNotifierWatcher",
        WATCHER,
        "RegisterStatusNotifierItem",
        Some(&(name,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
        // No watcher yet: the item registers when one appears.
        |_| {},
    );
}

fn emit(
    connection: &gio::DBusConnection,
    path: &str,
    interface: &str,
    signal: &str,
    args: Option<&Variant>,
) {
    let _ = connection.emit_signal(None, path, interface, signal, args);
}

fn item_call(method: &str, params: &Variant) -> Option<Variant> {
    match method {
        "Activate" | "SecondaryActivate" => {
            let token = with_item(|item| item.activation.take()).flatten();
            open_primary(token);
        }
        "ProvideXdgActivationToken" => {
            if let Some((token,)) = params.get::<(String,)>() {
                with_item(|item| item.activation = Some(token));
            }
        }
        // The menu is the item's menu property; a status area without
        // menus asks here, where there is nothing to show by ourselves.
        _ => {}
    }
    None
}

/// Show the primary window, raising it with the status area's token on
/// Wayland, where a window may only take the focus it is given.
fn open_primary(token: Option<String>) {
    if let Some(token) = token {
        let primary = host().def.primary;
        if let Some(win) = windows::all()
            .into_iter()
            .find(|win| win.kind_id() == primary)
        {
            win.window.set_startup_id(&token);
        }
    }
    windows::reopen_primary();
}

fn item_property(property: &str) -> Variant {
    let (id, title, pixmaps) = with_item(|item| {
        (
            host().def.id.to_string(),
            item.title.clone(),
            item.pixmaps.clone(),
        )
    })
    .unwrap_or_default();
    let none: Vec<(i32, i32, Vec<u8>)> = Vec::new();
    match property {
        "Category" => "ApplicationStatus".to_variant(),
        "Id" => id.to_variant(),
        "Title" => title.to_variant(),
        "Status" => "Active".to_variant(),
        "WindowId" => 0i32.to_variant(),
        "IconName" if pixmaps.is_empty() => "application-x-executable".to_variant(),
        "IconPixmap" => pixmaps.to_variant(),
        "OverlayIconPixmap" | "AttentionIconPixmap" => none.to_variant(),
        "ToolTip" => (String::new(), none, title, String::new()).to_variant(),
        "ItemIsMenu" => false.to_variant(),
        "Menu" => glib::variant::ObjectPath::try_from(MENU_PATH.to_string())
            .expect("an object path")
            .to_variant(),
        _ => String::new().to_variant(),
    }
}

fn menu_property(property: &str) -> Variant {
    match property {
        "Version" => 3u32.to_variant(),
        "TextDirection" => (if gtk::Widget::default_direction() == gtk::TextDirection::Rtl {
            "rtl"
        } else {
            "ltr"
        })
        .to_variant(),
        "IconThemePath" => Vec::<String>::new().to_variant(),
        _ => "normal".to_variant(),
    }
}

/// Build the menu as the state is now; when it differs from what the
/// status area has, tell it. Whether it changed.
fn rebuild() -> bool {
    let Some(def) = with_item(|item| item.def) else {
        return false;
    };
    let c = context();
    let primary = host().def.primary;
    let title = host()
        .def
        .windows
        .iter()
        .find(|window| window.id == primary)
        .map(|window| crate::text::tr(window.title))
        .unwrap_or_default();
    let mut menu = StatusMenu::default();
    menu.item(
        &c,
        crate::text::fmt(
            "bern.window.reopen",
            &[("title", crate::text::arg(&Value::Text(title), None))],
        ),
        true,
        None,
        Some(crate::act(|_| windows::reopen_primary())),
    );
    menu.divider();
    (def.menu)(&c, &mut menu);
    menu.divider();
    menu.item(
        &c,
        crate::text::tr("bern.app.quit"),
        true,
        None,
        Some(crate::act(|_| quit())),
    );
    let changed = with_item(|item| {
        let same = item.menu.len() == menu.entries.len()
            && item.menu.iter().zip(&menu.entries).all(|(a, b)| a.same(b));
        // The new entries carry the current contexts either way.
        item.menu = menu.entries;
        if same {
            return None;
        }
        item.revision += 1;
        Some((item.revision, item.connection.clone()))
    })
    .flatten();
    let Some((revision, connection)) = changed else {
        return false;
    };
    if let Some(connection) = connection {
        emit(
            &connection,
            MENU_PATH,
            "com.canonical.dbusmenu",
            "LayoutUpdated",
            Some(&(revision, 0i32).to_variant()),
        );
    }
    true
}

fn quit() {
    if let Some(application) = host().application.get() {
        application.quit();
    }
}

/// The entry with `id`, counting entries depth first from 1.
fn find(entries: &[Entry], id: i32) -> Option<&Entry> {
    fn walk<'a>(entries: &'a [Entry], id: i32, next: &mut i32) -> Option<&'a Entry> {
        for entry in entries {
            let own = *next;
            *next += 1;
            if own == id {
                return Some(entry);
            }
            if let Some(found) = walk(&entry.children, id, next) {
                return Some(found);
            }
        }
        None
    }
    walk(entries, id, &mut 1)
}

/// The id of each entry and of each child in turn.
fn numbered(entries: &[Entry], first: i32) -> Vec<(i32, &Entry)> {
    let mut next = first;
    entries
        .iter()
        .map(|entry| {
            let id = next;
            next += 1 + descendants(&entry.children);
            (id, entry)
        })
        .collect()
}

fn descendants(entries: &[Entry]) -> i32 {
    entries
        .iter()
        .map(|entry| 1 + descendants(&entry.children))
        .sum()
}

/// The dbusmenu properties of an entry; the root's are its children's
/// display.
fn properties(entry: Option<&Entry>) -> Variant {
    let dict = VariantDict::new(None);
    match entry {
        None => dict.insert_value("children-display", &"submenu".to_variant()),
        Some(entry) if entry.separator => dict.insert_value("type", &"separator".to_variant()),
        Some(entry) => {
            // Underscores mark mnemonics in dbusmenu labels.
            dict.insert_value("label", &entry.label.replace('_', "__").to_variant());
            dict.insert_value("enabled", &entry.enabled.to_variant());
            if let Some(checked) = entry.checked {
                dict.insert_value("toggle-type", &"checkmark".to_variant());
                dict.insert_value("toggle-state", &i32::from(checked).to_variant());
            }
            if !entry.children.is_empty() {
                dict.insert_value("children-display", &"submenu".to_variant());
            }
        }
    }
    dict.end()
}

/// `(ia{sv}av)`: an entry with its children down to `depth` levels (all
/// of them when negative).
fn layout(id: i32, entry: Option<&Entry>, children: &[Entry], depth: i32) -> Variant {
    let nested: Vec<Variant> = if depth == 0 {
        Vec::new()
    } else {
        numbered(children, id + 1)
            .into_iter()
            .map(|(child_id, child)| {
                Variant::from_variant(&layout(child_id, Some(child), &child.children, depth - 1))
            })
            .collect()
    };
    Variant::tuple_from_iter([
        id.to_variant(),
        properties(entry),
        Variant::array_from_iter_with_type(VariantTy::VARIANT, nested),
    ])
}

fn menu_call(method: &str, params: &Variant) -> Option<Variant> {
    match method {
        "GetLayout" => {
            let (parent, depth, _) = params.get::<(i32, i32, Vec<String>)>()?;
            with_item(|item| {
                let tree = if parent == 0 {
                    layout(0, None, &item.menu, depth)
                } else {
                    let entry = find(&item.menu, parent)?;
                    layout(parent, Some(entry), &entry.children, depth)
                };
                Some(Variant::tuple_from_iter([item.revision.to_variant(), tree]))
            })
            .flatten()
            .or_else(|| {
                Some(Variant::tuple_from_iter([
                    0u32.to_variant(),
                    layout(0, None, &[], 0),
                ]))
            })
        }
        "GetGroupProperties" => {
            let (ids, _) = params.get::<(Vec<i32>, Vec<String>)>()?;
            let found = with_item(|item| {
                ids.iter()
                    .filter_map(|&id| {
                        let entry = if id == 0 {
                            None
                        } else {
                            Some(find(&item.menu, id)?)
                        };
                        Some(Variant::tuple_from_iter([
                            id.to_variant(),
                            properties(entry),
                        ]))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
            let ty = VariantTy::new("(ia{sv})").expect("a type");
            Some(Variant::tuple_from_iter([
                Variant::array_from_iter_with_type(ty, found),
            ]))
        }
        "GetProperty" => {
            let (id, name) = params.get::<(i32, String)>()?;
            let value = with_item(|item| {
                let entry = if id == 0 { None } else { find(&item.menu, id) };
                VariantDict::new(Some(&properties(entry))).lookup_value(&name, None)
            })
            .flatten()
            .unwrap_or_else(|| String::new().to_variant());
            Some((Variant::from_variant(&value),).to_variant())
        }
        "Event" => {
            let id = params.child_value(0).get::<i32>()?;
            let event = params.child_value(1).get::<String>()?;
            if event == "clicked" {
                choose(id);
            }
            None
        }
        "EventGroup" => {
            let events = params.child_value(0);
            for index in 0..events.n_children() {
                let event = events.child_value(index);
                if let (Some(id), Some(kind)) = (
                    event.child_value(0).get::<i32>(),
                    event.child_value(1).get::<String>(),
                ) && kind == "clicked"
                {
                    choose(id);
                }
            }
            Some((Vec::<i32>::new(),).to_variant())
        }
        "AboutToShow" => Some((untrack(rebuild),).to_variant()),
        "AboutToShowGroup" => {
            let updated = if untrack(rebuild) {
                vec![0]
            } else {
                Vec::new()
            };
            Some((updated, Vec::<i32>::new()).to_variant())
        }
        _ => None,
    }
}

/// Run what the entry with `id` runs, after the status area handled the
/// click.
fn choose(id: i32) {
    let run = with_item(|item| {
        find(&item.menu, id).and_then(|entry| {
            entry
                .run
                .as_ref()
                .filter(|_| entry.enabled)
                .map(|(act, c)| (act.clone(), c.clone()))
        })
    })
    .flatten();
    if let Some((act, c)) = run {
        glib::idle_add_local_once(move || act(&c));
    }
}

/// The application icon beside the executable, at the sizes status areas
/// draw it.
fn pixmaps(bundle_id: &str) -> Vec<(i32, i32, Vec<u8>)> {
    let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join("icons").join(format!("{bundle_id}.png")))
        })
        .filter(|path| path.is_file())
    else {
        return Vec::new();
    };
    [16, 22, 24, 32, 48, 64]
        .into_iter()
        .filter_map(|side| {
            let pixbuf = Pixbuf::from_file_at_scale(&path, side, side, true).ok()?;
            Some((pixbuf.width(), pixbuf.height(), argb(&pixbuf)))
        })
        .collect()
}

/// A picture's pixels as ARGB32 in network byte order, not premultiplied.
fn argb(pixbuf: &Pixbuf) -> Vec<u8> {
    let (width, height) = (pixbuf.width() as usize, pixbuf.height() as usize);
    let (stride, channels) = (pixbuf.rowstride() as usize, pixbuf.n_channels() as usize);
    let bytes = pixbuf.read_pixel_bytes();
    let mut out = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        for column in 0..width {
            let pixel = &bytes[row * stride + column * channels..][..channels];
            let alpha = if channels == 4 { pixel[3] } else { 255 };
            out.extend_from_slice(&[alpha, pixel[0], pixel[1], pixel[2]]);
        }
    }
    out
}
