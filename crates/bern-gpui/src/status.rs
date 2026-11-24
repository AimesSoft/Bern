//! System status area / tray icon (`StatusItem`) on macOS, Windows and Linux:
//! - macOS: `NSStatusBar::systemStatusBar()` + dynamic `NSMenu`.
//! - Windows: `Shell_NotifyIconW` hidden window + `TrackPopupMenuEx`.
//! - Linux: `org.kde.StatusNotifierItem` + `com.canonical.dbusmenu` over session D-Bus (`zbus`).

use crate::ctx::{Act, Ctx};
use crate::host::host;
use crate::protocol::Value;
use crate::{AppDef, windows};
use gpui::App;
use std::cell::{Cell, RefCell};

/// The `StatusItem` of the declaration: tooltip, menu builder, login-item toggle,
/// visibility predicate, and optional custom monochrome PNG icon sizes.
pub struct StatusItemDef {
    pub label: fn(&Ctx) -> String,
    pub menu: fn(&Ctx, &mut StatusMenu),
    pub login_item: bool,
    pub shown: Option<fn(&Ctx) -> bool>,
    /// `StatusItem(image:)`: black-on-transparent PNGs at `[16, 22, 24, 32, 48, 64]`.
    pub image: &'static [(i32, &'static [u8])],
}

#[derive(Clone)]
pub(crate) enum EntryKind {
    ReopenPrimary,
    LoginToggle(bool),
    Quit,
    User(Act, Ctx),
    None,
}

pub(crate) struct MenuEntry {
    pub(crate) label: String,
    pub(crate) enabled: bool,
    pub(crate) checked: Option<bool>,
    pub(crate) separator: bool,
    pub(crate) children: Vec<MenuEntry>,
    pub(crate) action: EntryKind,
}

/// Builder passed to the generated `status_menu` function.
#[derive(Default)]
pub struct StatusMenu {
    entries: Vec<MenuEntry>,
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
        self.entries.push(MenuEntry {
            label,
            enabled,
            checked,
            separator: false,
            children: Vec::new(),
            action: match act {
                Some(act) => EntryKind::User(act, c.clone()),
                None => EntryKind::None,
            },
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
        self.entries.push(MenuEntry {
            label,
            enabled: true,
            checked: None,
            separator: false,
            children: sub.entries,
            action: EntryKind::None,
        });
    }

    fn separate(&mut self) {
        if std::mem::take(&mut self.divider) && !self.entries.is_empty() {
            self.entries.push(MenuEntry {
                label: String::new(),
                enabled: true,
                checked: None,
                separator: true,
                children: Vec::new(),
                action: EntryKind::None,
            });
        }
    }
}

thread_local! {
    static SHOWN: Cell<bool> = const { Cell::new(false) };
    static ACTIONS: RefCell<Vec<EntryKind>> = const { RefCell::new(Vec::new()) };
}

/// True while a declared `StatusItem` is visible and keeps the app alive with 0 windows.
pub(crate) fn keeps_running() -> bool {
    host().def.status_item.is_some() && SHOWN.get()
}

/// Build the full status menu entries (Open primary, app items, Open at Login, Quit)
/// and store their executable actions indexed by 1-based action IDs.
fn build_full_menu(def: &'static StatusItemDef) -> Vec<MenuEntry> {
    let app = host().def;
    let c = Ctx::detached("status_item");
    let primary_title = app
        .windows
        .iter()
        .find(|w| w.id == app.primary)
        .map(|w| crate::text::tr(w.title))
        .unwrap_or_else(|| crate::text::tr(app.name));
    let mut menu = StatusMenu::default();
    menu.entries.push(MenuEntry {
        label: crate::text::fmt(
            "bern.window.reopen",
            &[("title", crate::text::arg(&Value::Text(primary_title), None))],
        ),
        enabled: true,
        checked: None,
        separator: false,
        children: Vec::new(),
        action: EntryKind::ReopenPrimary,
    });
    menu.divider();
    (def.menu)(&c, &mut menu);
    menu.divider();
    if def.login_item {
        let on = crate::login_item::enabled();
        menu.separate();
        menu.entries.push(MenuEntry {
            label: crate::text::tr("bern.app.open_at_login"),
            enabled: true,
            checked: Some(on),
            separator: false,
            children: Vec::new(),
            action: EntryKind::LoginToggle(!on),
        });
    }
    menu.separate();
    menu.entries.push(MenuEntry {
        label: crate::text::tr("bern.app.quit"),
        enabled: true,
        checked: None,
        separator: false,
        children: Vec::new(),
        action: EntryKind::Quit,
    });
    menu.entries
}

/// Execute a queued status-menu action on GPUI's main thread.
pub(crate) fn invoke_action(id: usize, cx: &mut App) {
    let action = ACTIONS.with(|actions| {
        id.checked_sub(1)
            .and_then(|idx| actions.borrow().get(idx).cloned())
    });
    let Some(action) = action else {
        return;
    };
    match action {
        EntryKind::ReopenPrimary => {
            cx.activate(true);
            windows::reopen_primary(cx);
        }
        EntryKind::LoginToggle(next) => {
            crate::login_item::set(next);
            sync(cx);
        }
        EntryKind::Quit => cx.quit(),
        EntryKind::User(act, ctx) => {
            act(&ctx);
            sync(cx);
        }
        EntryKind::None => {}
    }
}

/// Initialize the native status item if declared on `def`.
pub(crate) fn init(def: &'static AppDef, cx: &mut App) {
    if def.status_item.is_some() {
        sync(cx);
    }
}

/// Refresh visibility, tooltip, and menu state of the native status item.
pub(crate) fn sync(_cx: &mut App) {
    let Some(def) = host().def.status_item.as_ref() else {
        SHOWN.set(false);
        return;
    };
    let c = Ctx::detached("status_item");
    let visible = def.shown.is_none_or(|pred| pred(&c));
    SHOWN.set(visible);
    let tooltip = (def.label)(&c);
    platform::sync(def, visible, &tooltip);
}

/// Tear down the native status item before the process exits.
pub(crate) fn remove() {
    SHOWN.set(false);
    platform::remove();
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{ACTIONS, EntryKind, MenuEntry, StatusItemDef, build_full_menu};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
        NSVariableStatusItemLength,
    };
    use objc2_foundation::{NSData, NSSize, NSString};
    use std::cell::RefCell;

    define_class!(
        // SAFETY: `NSObject` has no subclassing requirements and `MenuTarget` has no Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "BernGpuiStatusMenuTarget"]
        struct MenuTarget;

        impl MenuTarget {
            #[unsafe(method(menuNeedsUpdate:))]
            fn menu_needs_update(&self, menu: &NSMenu) {
                populate_menu(menu, self);
            }

            #[unsafe(method(bernStatusItemAction:))]
            fn action(&self, sender: &NSMenuItem) {
                let tag = sender.tag();
                if tag > 0 {
                    crate::host::send_status_action(tag as usize);
                }
            }
        }
    );

    struct State {
        item: Option<Retained<NSStatusItem>>,
        target: Retained<MenuTarget>,
        last_tooltip: String,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn new_target(mtm: MainThreadMarker) -> Retained<MenuTarget> {
        let this = MenuTarget::alloc(mtm).set_ivars(());
        // SAFETY: standard `NSObject.init`.
        unsafe { msg_send![super(this), init] }
    }

    fn populate_menu(menu: &NSMenu, target: &MenuTarget) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(def) = crate::host::host().def.status_item.as_ref() else {
            return;
        };
        let entries = build_full_menu(def);
        let mut table = Vec::new();
        menu.removeAllItems();
        fill_ns_menu(mtm, menu, &entries, target, &mut table);
        ACTIONS.with(|actions| *actions.borrow_mut() = table);
    }

    fn fill_ns_menu(
        mtm: MainThreadMarker,
        menu: &NSMenu,
        entries: &[MenuEntry],
        target: &MenuTarget,
        table: &mut Vec<EntryKind>,
    ) {
        for entry in entries {
            if entry.separator {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
                continue;
            }
            let title = NSString::from_str(&entry.label);
            let empty = NSString::from_str("");
            if !entry.children.is_empty() {
                let item = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &title,
                        None,
                        &empty,
                    )
                };
                let sub = NSMenu::initWithTitle(NSMenu::alloc(mtm), &title);
                sub.setAutoenablesItems(false);
                fill_ns_menu(mtm, &sub, &entry.children, target, table);
                item.setSubmenu(Some(&sub));
                menu.addItem(&item);
                continue;
            }
            table.push(entry.action.clone());
            let tag = table.len() as isize;
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &title,
                    Some(sel!(bernStatusItemAction:)),
                    &empty,
                )
            };
            unsafe {
                item.setTarget(Some(target));
            }
            item.setTag(tag);
            item.setEnabled(entry.enabled);
            if let Some(checked) = entry.checked {
                item.setState(if checked { 1 } else { 0 });
            }
            menu.addItem(&item);
        }
    }

    fn status_image(def: &'static StatusItemDef) -> Option<Retained<NSImage>> {
        if let Some((_, bytes)) = def
            .image
            .iter()
            .find(|(side, _)| *side == 32)
            .or_else(|| def.image.last())
        {
            let data = NSData::with_bytes(bytes);
            if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
                img.setSize(NSSize::new(18.0, 18.0));
                img.setTemplate(true);
                return Some(img);
            }
        }
        let mtm = MainThreadMarker::new()?;
        let app = NSApplication::sharedApplication(mtm);
        let icon: Option<Retained<NSImage>> = unsafe { msg_send![&app, applicationIconImage] };
        if let Some(icon) = icon {
            let copy: Option<Retained<NSImage>> = unsafe { msg_send![&icon, copy] };
            if let Some(img) = copy {
                img.setSize(NSSize::new(18.0, 18.0));
                return Some(img);
            }
        }
        None
    }

    pub(super) fn sync(def: &'static StatusItemDef, visible: bool, tooltip: &str) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        STATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot.get_or_insert_with(|| State {
                item: None,
                target: new_target(mtm),
                last_tooltip: String::new(),
            });
            if !visible {
                if let Some(item) = state.item.take() {
                    NSStatusBar::systemStatusBar().removeStatusItem(&item);
                }
                return;
            }
            if state.item.is_none() {
                let item =
                    NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
                if let Some(button) = item.button(mtm) {
                    if let Some(img) = status_image(def) {
                        button.setImage(Some(&img));
                    } else {
                        let fallback =
                            NSString::from_str(&crate::text::tr(crate::host::host().def.name));
                        button.setTitle(&fallback);
                    }
                }
                let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(""));
                menu.setAutoenablesItems(false);
                // SAFETY: `MenuTarget` implements `menuNeedsUpdate:`.
                let delegate_ptr: *const AnyObject =
                    (&*state.target as *const MenuTarget).cast::<AnyObject>();
                unsafe {
                    let _: () = msg_send![&menu, setDelegate: delegate_ptr];
                }
                item.setMenu(Some(&menu));
                state.item = Some(item);
                state.last_tooltip.clear();
            }
            if state.last_tooltip != tooltip
                && let Some(item) = &state.item
                && let Some(button) = item.button(mtm)
            {
                let tip = NSString::from_str(tooltip);
                button.setToolTip(Some(&tip));
                state.last_tooltip = tooltip.to_string();
            }
        });
    }

    pub(super) fn remove() {
        STATE.with(|slot| {
            if let Some(mut state) = slot.borrow_mut().take()
                && let Some(item) = state.item.take()
            {
                NSStatusBar::systemStatusBar().removeStatusItem(&item);
            }
        });
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{ACTIONS, EntryKind, MenuEntry, StatusItemDef, build_full_menu};
    use std::cell::RefCell;
    use std::ffi::c_void;
    use win32::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use win32::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject, GetDC, ReleaseDC,
    };
    use win32::Win32::System::LibraryLoader::GetModuleHandleW;
    use win32::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use win32::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
    use win32::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NIM_SETVERSION, NIN_SELECT, NOTIFY_ICON_DATA_FLAGS, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
        Shell_NotifyIconW,
    };
    use win32::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, GetMenuItemCount, GetSystemMetrics, HICON, HMENU,
        ICONINFO, IMAGE_ICON, LR_SHARED, LoadImageW, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR,
        MF_STRING, PostMessageW, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON,
        SM_MENUDROPALIGNMENT, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY,
        TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TrackPopupMenuEx, WM_APP, WM_CONTEXTMENU,
        WM_NULL, WM_SETTINGCHANGE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
    };
    use win32::core::PCWSTR;

    const ICON_ID: u32 = 1;
    const CALLBACK_MSG: u32 = WM_APP + 1;
    // `NIN_KEYSELECT` (`WM_USER + 1`) is `NIN_SELECT + 1` in ShellAPI.h.
    const NIN_KEYSELECT: u32 = NIN_SELECT + 1;

    struct State {
        hwnd: HWND,
        taskbar_created: u32,
        shown: bool,
        tooltip: String,
        custom_icon: Option<HICON>,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn system_uses_light_theme() -> bool {
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let val = wide("SystemUsesLightTheme");
        let mut light: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(val.as_ptr()),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut light as *mut u32 as *mut c_void),
                Some(&mut size),
            )
        };
        status.is_ok() && light != 0
    }

    fn hicon_from_png(png: &[u8], tint: (u8, u8, u8), side: u32) -> Option<HICON> {
        let img = image::load_from_memory(png)
            .ok()?
            .resize_exact(side, side, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (w, h) = img.dimensions();
        let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w as i32;
        bmi.bmiHeader.biHeight = -(h as i32);
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        let mut bits: *mut c_void = std::ptr::null_mut();
        unsafe {
            let dc = GetDC(None);
            let color_bmp =
                CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            let _ = ReleaseDC(None, dc);
            if bits.is_null() {
                let _ = DeleteObject(color_bmp.into());
                return None;
            }
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
            for (src, out) in img
                .as_raw()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(dst.as_chunks_mut::<4>().0.iter_mut())
            {
                let a = src[3];
                out[0] = tint.2;
                out[1] = tint.1;
                out[2] = tint.0;
                out[3] = a;
            }
            let mask_bmp = CreateBitmap(w as i32, h as i32, 1, 1, None);
            let info = ICONINFO {
                fIcon: true.into(),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask_bmp,
                hbmColor: color_bmp,
            };
            let icon = CreateIconIndirect(&info).ok();
            let _ = DeleteObject(color_bmp.into());
            let _ = DeleteObject(mask_bmp.into());
            icon
        }
    }

    fn resolve_icon(def: &'static StatusItemDef, prev: &mut Option<HICON>) -> HICON {
        let side = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) }.max(16) as u32;
        if !def.image.is_empty() {
            let best = def
                .image
                .iter()
                .min_by_key(|(s, _)| (*s - side as i32).abs())
                .map(|(_, bytes)| *bytes);
            if let Some(png) = best {
                let tint = if system_uses_light_theme() {
                    (0, 0, 0)
                } else {
                    (255, 255, 255)
                };
                if let Some(icon) = hicon_from_png(png, tint, side) {
                    if let Some(old) = prev.replace(icon) {
                        unsafe {
                            let _ = DestroyIcon(old);
                        }
                    }
                    return icon;
                }
            }
        }
        unsafe {
            let inst = GetModuleHandleW(None).unwrap_or_default();
            // `MAKEINTRESOURCEW(1)` encodes integer resource ID 1 in the low 16 bits.
            #[allow(clippy::manual_dangling_ptr)]
            let res_id = PCWSTR(1usize as *const u16);
            LoadImageW(
                Some(inst.into()),
                res_id,
                IMAGE_ICON,
                side as i32,
                side as i32,
                LR_SHARED,
            )
            .map(|h| HICON(h.0))
            .unwrap_or_default()
        }
    }

    fn notify_data(hwnd: HWND, flags: NOTIFY_ICON_DATA_FLAGS) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ICON_ID,
            uFlags: flags,
            ..Default::default()
        }
    }

    fn set_tip(data: &mut NOTIFYICONDATAW, tooltip: &str) {
        let w = wide(tooltip);
        let len = w.len().min(data.szTip.len().saturating_sub(1));
        data.szTip[..len].copy_from_slice(&w[..len]);
        data.szTip[len] = 0;
    }

    fn add_icon(state: &mut State, def: &'static StatusItemDef) {
        let mut data = notify_data(state.hwnd, NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP);
        data.uCallbackMessage = CALLBACK_MSG;
        data.hIcon = resolve_icon(def, &mut state.custom_icon);
        set_tip(&mut data, &state.tooltip);
        unsafe {
            if Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
            }
        }
    }

    fn fill_hmenu(hmenu: HMENU, entries: &[MenuEntry], table: &mut Vec<EntryKind>) {
        for entry in entries {
            if entry.separator {
                unsafe {
                    if GetMenuItemCount(Some(hmenu)) > 0 {
                        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                    }
                }
                continue;
            }
            let label = wide(&entry.label);
            if !entry.children.is_empty() {
                let Ok(sub) = (unsafe { CreatePopupMenu() }) else {
                    continue;
                };
                fill_hmenu(sub, &entry.children, table);
                unsafe {
                    if GetMenuItemCount(Some(sub)) == 0 {
                        let _ = DestroyMenu(sub);
                    } else {
                        let _ =
                            AppendMenuW(hmenu, MF_POPUP, sub.0 as usize, PCWSTR(label.as_ptr()));
                    }
                }
                continue;
            }
            table.push(entry.action.clone());
            let id = table.len();
            let mut flags = MF_STRING;
            if !entry.enabled {
                flags |= MF_GRAYED;
            }
            if entry.checked == Some(true) {
                flags |= MF_CHECKED;
            }
            unsafe {
                let _ = AppendMenuW(hmenu, flags, id, PCWSTR(label.as_ptr()));
            }
        }
    }

    fn popup_menu(hwnd: HWND, x: i32, y: i32) {
        let Some(def) = crate::host::host().def.status_item.as_ref() else {
            return;
        };
        let entries = build_full_menu(def);
        let Ok(hmenu) = (unsafe { CreatePopupMenu() }) else {
            return;
        };
        let mut table = Vec::new();
        fill_hmenu(hmenu, &entries, &mut table);
        ACTIONS.with(|actions| *actions.borrow_mut() = table);
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            let align = if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 {
                TPM_RIGHTALIGN
            } else {
                TPM_LEFTALIGN
            };
            let chosen = TrackPopupMenuEx(
                hmenu,
                (TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | align).0,
                x,
                y,
                hwnd,
                None,
            );
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(hmenu);
            if chosen.0 > 0 {
                crate::host::send_status_action(chosen.0 as usize);
            }
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == CALLBACK_MSG {
            let ev = (lparam.0 & 0xffff) as u32;
            if ev == NIN_SELECT || ev == NIN_KEYSELECT {
                crate::host::send_status_reopen();
            } else if ev == WM_CONTEXTMENU {
                let x = (wparam.0 & 0xffff) as i16 as i32;
                let y = ((wparam.0 >> 16) & 0xffff) as i16 as i32;
                popup_menu(hwnd, x, y);
            }
            return LRESULT(0);
        }
        let mut handled = false;
        STATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(state) = slot.as_mut() else {
                return;
            };
            if state.taskbar_created != 0 && msg == state.taskbar_created {
                if state.shown
                    && let Some(def) = crate::host::host().def.status_item.as_ref()
                {
                    add_icon(state, def);
                }
                handled = true;
            } else if msg == WM_SETTINGCHANGE
                && state.shown
                && let Some(def) = crate::host::host().def.status_item.as_ref()
                && !def.image.is_empty()
            {
                let mut data = notify_data(hwnd, NIF_ICON);
                data.hIcon = resolve_icon(def, &mut state.custom_icon);
                unsafe {
                    let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
                }
            }
        });
        if handled {
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    pub(super) fn sync(def: &'static StatusItemDef, visible: bool, tooltip: &str) {
        STATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                let class_name = wide("BernGpuiStatusArea");
                let empty = wide("");
                let inst = unsafe { GetModuleHandleW(None).unwrap_or_default() };
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: inst.into(),
                    lpszClassName: PCWSTR(class_name.as_ptr()),
                    ..Default::default()
                };
                unsafe {
                    let _ = RegisterClassW(&wc);
                }
                let Ok(hwnd) = (unsafe {
                    CreateWindowExW(
                        WS_EX_TOOLWINDOW,
                        PCWSTR(class_name.as_ptr()),
                        PCWSTR(empty.as_ptr()),
                        WS_POPUP,
                        0,
                        0,
                        0,
                        0,
                        None,
                        None,
                        Some(inst.into()),
                        None,
                    )
                }) else {
                    return;
                };
                let tb = wide("TaskbarCreated");
                let taskbar_created = unsafe { RegisterWindowMessageW(PCWSTR(tb.as_ptr())) };
                *slot = Some(State {
                    hwnd,
                    taskbar_created,
                    shown: false,
                    tooltip: String::new(),
                    custom_icon: None,
                });
            }
            let Some(state) = slot.as_mut() else {
                return;
            };
            let tip_changed = state.tooltip != tooltip;
            state.tooltip = tooltip.to_string();
            if visible && !state.shown {
                state.shown = true;
                add_icon(state, def);
            } else if !visible && state.shown {
                state.shown = false;
                let data = notify_data(state.hwnd, NOTIFY_ICON_DATA_FLAGS(0));
                unsafe {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                }
            } else if visible && tip_changed {
                let mut data = notify_data(state.hwnd, NIF_TIP | NIF_SHOWTIP);
                set_tip(&mut data, tooltip);
                unsafe {
                    if !Shell_NotifyIconW(NIM_MODIFY, &data).as_bool() {
                        add_icon(state, def);
                    }
                }
            }
        });
    }

    pub(super) fn remove() {
        STATE.with(|slot| {
            if let Some(state) = slot.borrow_mut().take() {
                if state.shown {
                    let data = notify_data(state.hwnd, NOTIFY_ICON_DATA_FLAGS(0));
                    unsafe {
                        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                    }
                }
                if let Some(icon) = state.custom_icon {
                    unsafe {
                        let _ = DestroyIcon(icon);
                    }
                }
                unsafe {
                    let _ = DestroyWindow(state.hwnd);
                }
            }
        });
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod platform {
    use super::{ACTIONS, EntryKind, MenuEntry, StatusItemDef, build_full_menu};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use zbus::interface;
    use zbus::object_server::SignalEmitter;
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

    #[derive(Clone)]
    struct DbusEntry {
        id: i32,
        label: String,
        enabled: bool,
        checked: Option<bool>,
        separator: bool,
        children: Vec<DbusEntry>,
    }

    #[derive(Clone)]
    struct Snapshot {
        id: String,
        title: String,
        shown: bool,
        pixmaps: Vec<(i32, i32, Vec<u8>)>,
        menu: Vec<DbusEntry>,
        revision: u32,
    }

    type MenuLayout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

    fn entry_props(entry: &DbusEntry) -> HashMap<String, OwnedValue> {
        let mut props = HashMap::new();
        if entry.separator {
            let _ = props.insert(
                "type".to_string(),
                Value::from("separator").try_into().unwrap(),
            );
            return props;
        }
        let _ = props.insert(
            "label".to_string(),
            Value::from(entry.label.clone()).try_into().unwrap(),
        );
        let _ = props.insert(
            "enabled".to_string(),
            Value::from(entry.enabled).try_into().unwrap(),
        );
        if let Some(checked) = entry.checked {
            let _ = props.insert(
                "toggle-type".to_string(),
                Value::from("checkmark").try_into().unwrap(),
            );
            let _ = props.insert(
                "toggle-state".to_string(),
                Value::from(if checked { 1i32 } else { 0i32 })
                    .try_into()
                    .unwrap(),
            );
        }
        if !entry.children.is_empty() {
            let _ = props.insert(
                "children-display".to_string(),
                Value::from("submenu").try_into().unwrap(),
            );
        }
        props
    }

    fn build_layout(entry: &DbusEntry, depth: i32) -> MenuLayout {
        let props = entry_props(entry);
        let children = if depth == 0 {
            Vec::new()
        } else {
            let next = if depth > 0 { depth - 1 } else { -1 };
            entry
                .children
                .iter()
                .filter_map(|child| {
                    let sub = build_layout(child, next);
                    OwnedValue::try_from(Value::from(sub)).ok()
                })
                .collect()
        };
        (entry.id, props, children)
    }

    fn find_entry<'a>(entries: &'a [DbusEntry], id: i32) -> Option<&'a DbusEntry> {
        for e in entries {
            if e.id == id {
                return Some(e);
            }
            if let Some(found) = find_entry(&e.children, id) {
                return Some(found);
            }
        }
        None
    }

    struct SniService {
        state: Arc<Mutex<Snapshot>>,
    }

    #[interface(name = "org.kde.StatusNotifierItem")]
    impl SniService {
        fn activate(&self, _x: i32, _y: i32) {
            crate::host::send_status_reopen();
        }
        fn secondary_activate(&self, _x: i32, _y: i32) {
            crate::host::send_status_reopen();
        }
        fn context_menu(&self, _x: i32, _y: i32) {}
        fn scroll(&self, _delta: i32, _orientation: &str) {}
        fn provide_xdg_activation_token(&self, _token: &str) {}

        #[zbus(property)]
        fn category(&self) -> String {
            "ApplicationStatus".into()
        }
        #[zbus(property)]
        fn id(&self) -> String {
            self.state.lock().map(|s| s.id.clone()).unwrap_or_default()
        }
        #[zbus(property)]
        fn title(&self) -> String {
            self.state
                .lock()
                .map(|s| s.title.clone())
                .unwrap_or_default()
        }
        #[zbus(property)]
        fn status(&self) -> String {
            let shown = self.state.lock().map(|s| s.shown).unwrap_or(false);
            if shown { "Active" } else { "Passive" }.into()
        }
        #[zbus(property)]
        fn window_id(&self) -> i32 {
            0
        }
        #[zbus(property)]
        fn icon_name(&self) -> String {
            String::new()
        }
        #[zbus(property)]
        fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            self.state
                .lock()
                .map(|s| s.pixmaps.clone())
                .unwrap_or_default()
        }
        #[zbus(property)]
        fn overlay_icon_name(&self) -> String {
            String::new()
        }
        #[zbus(property)]
        fn overlay_icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            Vec::new()
        }
        #[zbus(property)]
        fn attention_icon_name(&self) -> String {
            String::new()
        }
        #[zbus(property)]
        fn attention_icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
            Vec::new()
        }
        #[zbus(property)]
        fn attention_movie_name(&self) -> String {
            String::new()
        }
        #[zbus(property)]
        fn tool_tip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
            let title = self
                .state
                .lock()
                .map(|s| s.title.clone())
                .unwrap_or_default();
            (String::new(), Vec::new(), title, String::new())
        }
        #[zbus(property)]
        fn item_is_menu(&self) -> bool {
            false
        }
        #[zbus(property)]
        fn menu(&self) -> OwnedObjectPath {
            OwnedObjectPath::try_from("/MenuBar").expect("valid object path")
        }

        #[zbus(signal)]
        async fn new_title(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
        #[zbus(signal)]
        async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
        #[zbus(signal)]
        async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
        #[zbus(signal)]
        async fn new_status(emitter: &SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
    }

    struct DbusMenuService {
        state: Arc<Mutex<Snapshot>>,
    }

    #[interface(name = "com.canonical.dbusmenu")]
    impl DbusMenuService {
        fn get_layout(
            &self,
            parent_id: i32,
            recursion_depth: i32,
            _property_names: Vec<String>,
        ) -> (u32, MenuLayout) {
            let Ok(snap) = self.state.lock() else {
                return (1, (0, HashMap::new(), Vec::new()));
            };
            let root = DbusEntry {
                id: 0,
                label: String::new(),
                enabled: true,
                checked: None,
                separator: false,
                children: snap.menu.clone(),
            };
            let target = if parent_id == 0 {
                &root
            } else {
                find_entry(&snap.menu, parent_id).unwrap_or(&root)
            };
            (snap.revision, build_layout(target, recursion_depth))
        }

        fn get_group_properties(
            &self,
            ids: Vec<i32>,
            _property_names: Vec<String>,
        ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
            let Ok(snap) = self.state.lock() else {
                return Vec::new();
            };
            ids.into_iter()
                .filter_map(|id| find_entry(&snap.menu, id).map(|e| (id, entry_props(e))))
                .collect()
        }

        fn get_property(&self, id: i32, name: &str) -> OwnedValue {
            self.state
                .lock()
                .ok()
                .and_then(|snap| {
                    find_entry(&snap.menu, id).and_then(|e| entry_props(e).remove(name))
                })
                .unwrap_or_else(|| Value::from("").try_into().unwrap())
        }

        fn event(&self, id: i32, event_id: &str, _data: OwnedValue, _timestamp: u32) {
            if event_id == "clicked" && id > 0 {
                crate::host::send_status_action(id as usize);
            }
        }

        fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
            for (id, ev, _, _) in events {
                if ev == "clicked" && id > 0 {
                    crate::host::send_status_action(id as usize);
                }
            }
            Vec::new()
        }

        fn about_to_show(&self, _id: i32) -> bool {
            false
        }

        fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
            (Vec::new(), Vec::new())
        }

        #[zbus(property)]
        fn version(&self) -> u32 {
            3
        }
        #[zbus(property)]
        fn text_direction(&self) -> String {
            "ltr".into()
        }
        #[zbus(property)]
        fn status(&self) -> String {
            "normal".into()
        }
        #[zbus(property)]
        fn icon_theme_path(&self) -> Vec<String> {
            Vec::new()
        }

        #[zbus(signal)]
        async fn layout_updated(
            emitter: &SignalEmitter<'_>,
            revision: u32,
            parent: i32,
        ) -> zbus::Result<()>;
    }

    fn png_to_argb_pixmap(bytes: &[u8], recolor_white: bool) -> Option<(i32, i32, Vec<u8>)> {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let mut argb = Vec::with_capacity((w * h * 4) as usize);
        for px in img.as_raw().as_chunks::<4>().0 {
            let (r, g, b, a) = if recolor_white {
                (255u8, 255u8, 255u8, px[3])
            } else {
                (px[0], px[1], px[2], px[3])
            };
            argb.extend_from_slice(&[a, r, g, b]);
        }
        Some((w as i32, h as i32, argb))
    }

    fn build_pixmaps(def: &'static StatusItemDef) -> Vec<(i32, i32, Vec<u8>)> {
        if !def.image.is_empty() {
            return def
                .image
                .iter()
                .filter_map(|(_, bytes)| png_to_argb_pixmap(bytes, true))
                .collect();
        }
        if let Some(rel) = crate::host::host().def.app_icon {
            let path = std::path::Path::new(&crate::host::resources_dir()).join(rel);
            if let Ok(bytes) = std::fs::read(&path)
                && let Some(px) = png_to_argb_pixmap(&bytes, false)
            {
                return vec![px];
            }
        }
        Vec::new()
    }

    fn to_dbus_entries(entries: &[MenuEntry], table: &mut Vec<EntryKind>) -> Vec<DbusEntry> {
        let mut out = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.separator {
                out.push(DbusEntry {
                    id: -((table.len() as i32) + 10000),
                    label: String::new(),
                    enabled: true,
                    checked: None,
                    separator: true,
                    children: Vec::new(),
                });
                continue;
            }
            if !entry.children.is_empty() {
                let sub = to_dbus_entries(&entry.children, table);
                out.push(DbusEntry {
                    id: -((table.len() as i32) + 20000),
                    label: entry.label.clone(),
                    enabled: true,
                    checked: None,
                    separator: false,
                    children: sub,
                });
                continue;
            }
            table.push(entry.action.clone());
            out.push(DbusEntry {
                id: table.len() as i32,
                label: entry.label.clone(),
                enabled: entry.enabled,
                checked: entry.checked,
                separator: false,
                children: Vec::new(),
            });
        }
        out
    }

    struct Handle {
        state: Arc<Mutex<Snapshot>>,
        notify: futures::channel::mpsc::UnboundedSender<bool>,
    }

    thread_local! {
        static HANDLE: RefCell<Option<Handle>> = const { RefCell::new(None) };
    }

    fn start_dbus(initial: Snapshot) -> Handle {
        let state = Arc::new(Mutex::new(initial));
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<bool>();
        let shared = Arc::clone(&state);
        std::thread::spawn(move || {
            let _ = futures::executor::block_on(async move {
                use futures::StreamExt;
                let bus_name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
                let conn = zbus::connection::Builder::session()
                    .ok()?
                    .name(bus_name.as_str())
                    .ok()?
                    .serve_at(
                        "/StatusNotifierItem",
                        SniService {
                            state: Arc::clone(&shared),
                        },
                    )
                    .ok()?
                    .serve_at(
                        "/MenuBar",
                        DbusMenuService {
                            state: Arc::clone(&shared),
                        },
                    )
                    .ok()?
                    .build()
                    .await
                    .ok()?;
                let _ = conn
                    .call_method(
                        Some("org.kde.StatusNotifierWatcher"),
                        "/StatusNotifierWatcher",
                        Some("org.kde.StatusNotifierWatcher"),
                        "RegisterStatusNotifierItem",
                        &(bus_name.as_str(),),
                    )
                    .await;
                while let Some(keep) = rx.next().await {
                    if !keep {
                        break;
                    }
                    let (shown, rev) = shared
                        .lock()
                        .map(|s| (s.shown, s.revision))
                        .unwrap_or((false, 1));
                    if let Ok(iface) = conn
                        .object_server()
                        .interface::<_, SniService>("/StatusNotifierItem")
                        .await
                    {
                        let emitter = iface.signal_emitter();
                        let _ = SniService::new_title(emitter).await;
                        let _ = SniService::new_tool_tip(emitter).await;
                        let _ = SniService::new_status(
                            emitter,
                            if shown { "Active" } else { "Passive" },
                        )
                        .await;
                    }
                    if let Ok(iface) = conn
                        .object_server()
                        .interface::<_, DbusMenuService>("/MenuBar")
                        .await
                    {
                        let _ =
                            DbusMenuService::layout_updated(iface.signal_emitter(), rev, 0).await;
                    }
                }
                Some(())
            });
        });
        Handle { state, notify: tx }
    }

    pub(super) fn sync(def: &'static StatusItemDef, visible: bool, tooltip: &str) {
        let entries = build_full_menu(def);
        let mut table = Vec::new();
        let dbus_entries = to_dbus_entries(&entries, &mut table);
        ACTIONS.with(|actions| *actions.borrow_mut() = table);
        HANDLE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                let snap = Snapshot {
                    id: crate::host::host().def.bundle_id.to_string(),
                    title: tooltip.to_string(),
                    shown: visible,
                    pixmaps: build_pixmaps(def),
                    menu: dbus_entries,
                    revision: 1,
                };
                *slot = Some(start_dbus(snap));
                return;
            }
            if let Some(handle) = slot.as_ref()
                && let Ok(mut snap) = handle.state.lock()
            {
                snap.title = tooltip.to_string();
                snap.shown = visible;
                snap.menu = dbus_entries;
                snap.revision = snap.revision.wrapping_add(1).max(1);
                let _ = handle.notify.unbounded_send(true);
            }
        });
    }

    pub(super) fn remove() {
        HANDLE.with(|slot| {
            if let Some(handle) = slot.borrow_mut().take() {
                let _ = handle.notify.unbounded_send(false);
            }
        });
    }
}
