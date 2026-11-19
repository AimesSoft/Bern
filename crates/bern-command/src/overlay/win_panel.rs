//! Owned Win32 utility tool window anchored beside a Windows overlay.

use super::{Control, Panel, panel_changed, panel_dismissed};
use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontIndirectW, CreateSolidBrush, DEFAULT_GUI_FONT, DeleteObject,
    GetMonitorInfoW, GetStockObject, GetSysColorBrush, HBRUSH, HFONT, HGDIOBJ, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::Controls::{
    ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, SetScrollInfo, TB_BOTTOM,
    TB_ENDTRACK, TB_LINEDOWN, TB_LINEUP, TB_PAGEDOWN, TB_PAGEUP, TB_THUMBPOSITION, TB_THUMBTRACK,
    TB_TOP, TBM_SETLINESIZE, TBM_SETPAGESIZE, TBM_SETPOS, TBM_SETRANGEMAX, TBM_SETRANGEMIN,
    TBS_NOTICKS, TRACKBAR_CLASSW, WC_BUTTONW, WC_COMBOBOXW, WC_STATICW,
};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("BernOverlayTools");
const TBM_GETPOS: u32 = WM_USER;
const PANEL_WIDTH: i32 = 280;
const INSET: i32 = 12;
const ROW_GAP: i32 = 10;
const WINDOW_GAP: i32 = 8;

thread_local! {
    static CUSTOM_COLORS: RefCell<[COLORREF; 16]> = const { RefCell::new([COLORREF(0xffffff); 16]) };
}

fn style() -> WINDOW_STYLE {
    WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VSCROLL | WS_CLIPCHILDREN
}

fn ex_style() -> WINDOW_EX_STYLE {
    WS_EX_TOOLWINDOW | WS_EX_CONTROLPARENT
}

pub(super) fn register(instance: HINSTANCE) {
    unsafe {
        let _ = InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        });
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
            lpszClassName: CLASS,
            ..Default::default()
        });
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn set_text(hwnd: HWND, s: &str) {
    let s = wide(s);
    let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(s.as_ptr())) };
}

fn send_msg(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wp)), Some(LPARAM(lp))).0 }
}

fn format_number(v: f64, step: f64) -> String {
    if step >= 1.0 || (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn key(c: &Control) -> &str {
    match c {
        Control::Range { id, .. } | Control::Color { id, .. } | Control::Choice { id, .. } => id,
    }
}

#[derive(Clone, Copy)]
struct Scale {
    min: f64,
    max: f64,
    step: f64,
    ticks: i32,
}

impl Scale {
    fn new(min: f64, max: f64, step: f64) -> Self {
        let min = if min.is_finite() { min } else { 0.0 };
        let span = max - min;
        if !span.is_finite() || span <= 0.0 {
            return Self {
                min,
                max: min,
                step: 1.0,
                ticks: 0,
            };
        }
        let step = if step.is_finite() && step > 0.0 {
            step
        } else {
            span / 100.0
        };
        let n = (span / step).ceil().clamp(1.0, 100_000.0);
        Self {
            min,
            max,
            step,
            ticks: n as i32,
        }
    }

    fn to_tick(self, value: f64) -> i32 {
        if self.ticks <= 0 {
            return 0;
        }
        let clamped = value.clamp(self.min, self.max);
        ((clamped - self.min) / self.step)
            .round()
            .clamp(0.0, f64::from(self.ticks)) as i32
    }

    fn value_at(self, tick: i32) -> f64 {
        if self.ticks <= 0 {
            return self.min;
        }
        (self.min + f64::from(tick.clamp(0, self.ticks)) * self.step).clamp(self.min, self.max)
    }
}

enum Input {
    Range {
        scale: Scale,
        readout: HWND,
    },
    Color {
        rgb: u32,
        swatch: HWND,
        brush: Option<HBRUSH>,
    },
    Choice {
        options: Vec<String>,
    },
}

struct Row {
    id: String,
    caption: HWND,
    control: HWND,
    input: Input,
}

impl Row {
    fn destroy(self) {
        unsafe {
            let _ = DestroyWindow(self.caption);
            let _ = DestroyWindow(self.control);
            if let Input::Color { swatch, brush, .. } = self.input {
                let _ = DestroyWindow(swatch);
                if let Some(brush) = brush {
                    let _ = DeleteObject(HGDIOBJ(brush.0));
                }
            }
        }
    }
}

struct ToolState {
    id: String,
    owner: HWND,
    hwnd: HWND,
    instance: HINSTANCE,
    font: Option<HFONT>,
    dpi: u32,
    offset: i32,
    rows: Vec<Row>,
    wanted: bool,
    visible: bool,
    syncing: bool,
    /// Control id currently being dragged with a trackbar; `sync` does not
    /// fight the user's thumb while active.
    dragging: Option<String>,
    /// Whether a trackbar change since the last commit is waiting for
    /// `TB_ENDTRACK` so a drag release emits exactly one committed event.
    pending_commit: Option<String>,
}

impl ToolState {
    fn px(&self, logical: i32) -> i32 {
        (logical as f64 * f64::from(self.dpi) / 96.0).round() as i32
    }

    fn content_height(&self) -> i32 {
        let mut h = self.px(INSET);
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                h += self.px(ROW_GAP);
            }
            h += match row.input {
                Input::Range { .. } => self.px(46),
                Input::Color { .. } => self.px(26),
                Input::Choice { .. } => self.px(48),
            };
        }
        h + self.px(INSET)
    }

    fn collect_font_targets(&self) -> Vec<HWND> {
        let mut targets = Vec::with_capacity(self.rows.len() * 3);
        for row in &self.rows {
            targets.push(row.caption);
            targets.push(row.control);
            match row.input {
                Input::Color { swatch, .. } => targets.push(swatch),
                Input::Range { readout, .. } => targets.push(readout),
                Input::Choice { .. } => {}
            }
        }
        targets
    }

    fn swatch_brush(&self, target: HWND) -> Option<HBRUSH> {
        for row in &self.rows {
            if let Input::Color {
                swatch,
                brush: Some(brush),
                ..
            } = &row.input
                && *swatch == target
            {
                return Some(*brush);
            }
        }
        None
    }
}

/// Owned Win32 tool window whose state lives in `Rc<RefCell<ToolState>>` so
/// synchronous Win32 re-entrancy (`SendMessageW`, `ShowWindow`, `ChooseColorW`)
/// never creates aliased `&mut` references or dangling borrows.
pub(super) struct Tool {
    inner: Rc<RefCell<ToolState>>,
    hwnd: HWND,
}

impl Tool {
    pub(super) fn new(id: &str, owner: HWND, instance: HINSTANCE) -> Option<Box<Self>> {
        let inner = Rc::new(RefCell::new(ToolState {
            id: id.to_owned(),
            owner,
            hwnd: HWND::default(),
            instance,
            font: None,
            dpi: 96,
            offset: 0,
            rows: Vec::new(),
            wanted: false,
            visible: false,
            syncing: false,
            dragging: None,
            pending_commit: None,
        }));
        let raw = Rc::into_raw(Rc::clone(&inner));
        let created = unsafe {
            CreateWindowExW(
                ex_style(),
                CLASS,
                w!(""),
                style(),
                0,
                0,
                100,
                100,
                Some(owner),
                None,
                Some(instance),
                Some(raw.cast()),
            )
        };
        let Ok(hwnd) = created else {
            // If `WM_NCDESTROY` did not run (creation failed before `WM_NCCREATE`),
            // reclaim the `Rc` we passed in `lpCreateParams`.
            unsafe {
                let _ = Rc::from_raw(raw);
            }
            return None;
        };
        let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
        {
            let mut s = inner.borrow_mut();
            s.hwnd = hwnd;
            s.dpi = dpi;
        }
        update_font(&inner, dpi);
        Some(Box::new(Self { inner, hwnd }))
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn is_visible(&self) -> bool {
        self.inner.borrow().visible
    }

    pub(super) fn dismiss(&mut self) {
        dismiss_inner(&self.inner);
    }

    pub(super) fn present(&mut self, model: Option<Panel>, owner_visible: bool) {
        let Some(model) = model else {
            self.inner.borrow_mut().wanted = false;
            hide_inner(&self.inner);
            return;
        };
        let (hwnd, instance, font, old_rows, needs_rebuild) = {
            let mut s = self.inner.borrow_mut();
            s.wanted = true;
            s.syncing = true;
            let same_shape = s.rows.len() == model.controls.len()
                && s.rows
                    .iter()
                    .zip(&model.controls)
                    .all(|(r, c)| r.id == key(c) && same_kind(&r.input, c));
            let old = if same_shape {
                Vec::new()
            } else {
                s.dragging = None;
                s.pending_commit = None;
                std::mem::take(&mut s.rows)
            };
            (s.hwnd, s.instance, s.font, old, !same_shape)
        };

        set_text(hwnd, &model.title);

        if needs_rebuild {
            for row in old_rows {
                row.destroy();
            }
            let mut created = Vec::with_capacity(model.controls.len());
            for (index, spec) in model.controls.iter().enumerate() {
                if let Some(row) = create_row(hwnd, instance, font, index, spec) {
                    created.push(row);
                }
            }
            self.inner.borrow_mut().rows = created;
        }

        let ops = sync_rows(&self.inner, &model.controls);
        apply_sync_ops(ops);

        self.inner.borrow_mut().syncing = false;
        if owner_visible {
            position_inner(&self.inner, true);
            show_noactivate(&self.inner);
        } else {
            hide_inner(&self.inner);
        }
    }

    pub(super) fn owner_moved(&mut self, owner_visible: bool) {
        let wanted = self.inner.borrow().wanted;
        if !wanted || !owner_visible {
            hide_inner(&self.inner);
            return;
        }
        position_inner(&self.inner, false);
        show_noactivate(&self.inner);
    }
}

impl Drop for Tool {
    fn drop(&mut self) {
        let (font, rows) = {
            let mut s = self.inner.borrow_mut();
            (s.font.take(), std::mem::take(&mut s.rows))
        };
        for row in rows {
            row.destroy();
        }
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        if let Some(font) = font {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
    }
}

fn show_noactivate(inner: &Rc<RefCell<ToolState>>) {
    let hwnd = {
        let mut s = inner.borrow_mut();
        if s.visible {
            return;
        }
        s.visible = true;
        s.hwnd
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

fn hide_inner(inner: &Rc<RefCell<ToolState>>) {
    let hwnd = {
        let mut s = inner.borrow_mut();
        if !s.visible {
            return;
        }
        s.visible = false;
        s.dragging = None;
        s.pending_commit = None;
        s.hwnd
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

fn dismiss_inner(inner: &Rc<RefCell<ToolState>>) {
    let id = {
        let mut s = inner.borrow_mut();
        s.wanted = false;
        s.id.clone()
    };
    hide_inner(inner);
    panel_dismissed(&id);
}

fn update_font(inner: &Rc<RefCell<ToolState>>, dpi: u32) {
    let new_font = unsafe {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        if SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            size_of::<NONCLIENTMETRICSW>() as u32,
            Some((&raw mut metrics).cast()),
            0,
            dpi,
        )
        .is_ok()
        {
            CreateFontIndirectW(&metrics.lfMessageFont)
        } else {
            HFONT(GetStockObject(DEFAULT_GUI_FONT).0)
        }
    };
    if new_font.0.is_null() {
        return;
    }
    let (old_font, targets) = {
        let mut s = inner.borrow_mut();
        let old = s.font.replace(new_font);
        (old, s.collect_font_targets())
    };
    for target in targets {
        send_msg(target, WM_SETFONT, new_font.0 as usize, 1);
    }
    if let Some(old) = old_font {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(old.0));
        }
    }
}

fn create_row(
    parent: HWND,
    instance: HINSTANCE,
    font: Option<HFONT>,
    index: usize,
    spec: &Control,
) -> Option<Row> {
    let id_base = (index + 1) * 10;
    let caption = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WC_STATICW,
            w!(""),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            100,
            20,
            Some(parent),
            Some(HMENU(id_base as *mut _)),
            Some(instance),
            None,
        )
    }
    .ok()?;
    let (control, input) = match spec {
        Control::Range { min, max, step, .. } => {
            let readout = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_STATICW,
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x2),
                    0,
                    0,
                    48,
                    20,
                    Some(parent),
                    Some(HMENU((id_base + 1) as *mut _)),
                    Some(instance),
                    None,
                )
            }
            .ok()?;
            let track = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    TRACKBAR_CLASSW,
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(TBS_NOTICKS),
                    0,
                    0,
                    100,
                    24,
                    Some(parent),
                    Some(HMENU((id_base + 2) as *mut _)),
                    Some(instance),
                    None,
                )
            }
            .ok()?;
            let scale = Scale::new(*min, *max, *step);
            send_msg(track, TBM_SETRANGEMIN, 0, 0);
            send_msg(track, TBM_SETRANGEMAX, 1, scale.ticks as isize);
            send_msg(track, TBM_SETLINESIZE, 0, 1);
            send_msg(
                track,
                TBM_SETPAGESIZE,
                0,
                (scale.ticks / 10).max(1) as isize,
            );
            if let Some(font) = font {
                send_msg(readout, WM_SETFONT, font.0 as usize, 1);
            }
            (track, Input::Range { scale, readout })
        }
        Control::Color { rgb, .. } => {
            let swatch = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(WS_EX_STATICEDGE.0),
                    WC_STATICW,
                    w!(""),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    28,
                    22,
                    Some(parent),
                    Some(HMENU((id_base + 1) as *mut _)),
                    Some(instance),
                    None,
                )
            }
            .ok()?;
            let btn = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_BUTTONW,
                    w!("..."),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                    0,
                    0,
                    40,
                    24,
                    Some(parent),
                    Some(HMENU((id_base + 2) as *mut _)),
                    Some(instance),
                    None,
                )
            }
            .ok()?;
            if let Some(font) = font {
                send_msg(swatch, WM_SETFONT, font.0 as usize, 1);
            }
            (
                btn,
                Input::Color {
                    rgb: *rgb & 0xffffff,
                    swatch,
                    brush: Some(unsafe { CreateSolidBrush(colorref(*rgb)) }),
                },
            )
        }
        Control::Choice { .. } => {
            let combo = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_COMBOBOXW,
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    100,
                    180,
                    Some(parent),
                    Some(HMENU((id_base + 2) as *mut _)),
                    Some(instance),
                    None,
                )
            }
            .ok()?;
            (
                combo,
                Input::Choice {
                    options: Vec::new(),
                },
            )
        }
    };
    if let Some(font) = font {
        send_msg(caption, WM_SETFONT, font.0 as usize, 1);
        send_msg(control, WM_SETFONT, font.0 as usize, 1);
    }
    Some(Row {
        id: key(spec).to_owned(),
        caption,
        control,
        input,
    })
}

enum SyncOp {
    Text(HWND, String),
    Range(HWND, i32, i32, HWND, String),
    Invalidate(HWND),
    Choice(HWND, Option<Vec<String>>, Option<usize>),
}

fn sync_rows(inner: &Rc<RefCell<ToolState>>, controls: &[Control]) -> Vec<SyncOp> {
    let mut s = inner.borrow_mut();
    let dragging = s.dragging.clone();
    let mut ops = Vec::new();
    for (row, spec) in s.rows.iter_mut().zip(controls) {
        match (&mut row.input, spec) {
            (
                Input::Range { scale, readout },
                Control::Range {
                    label,
                    min,
                    max,
                    step,
                    value,
                    ..
                },
            ) => {
                ops.push(SyncOp::Text(row.caption, label.clone()));
                *scale = Scale::new(*min, *max, *step);
                if dragging.as_deref() != Some(row.id.as_str()) {
                    ops.push(SyncOp::Range(
                        row.control,
                        scale.ticks,
                        scale.to_tick(*value),
                        *readout,
                        format_number(*value, *step),
                    ));
                }
            }
            (
                Input::Color { rgb, swatch, brush },
                Control::Color {
                    label, rgb: next, ..
                },
            ) => {
                ops.push(SyncOp::Text(row.caption, label.clone()));
                let next = *next & 0xffffff;
                if *rgb != next {
                    *rgb = next;
                    if let Some(old) = brush.replace(unsafe { CreateSolidBrush(colorref(next)) }) {
                        unsafe {
                            let _ = DeleteObject(HGDIOBJ(old.0));
                        }
                    }
                    ops.push(SyncOp::Invalidate(*swatch));
                }
                ops.push(SyncOp::Text(row.control, format!("#{next:06X}")));
            }
            (
                Input::Choice { options: prev },
                Control::Choice {
                    label,
                    options,
                    selected,
                    ..
                },
            ) => {
                ops.push(SyncOp::Text(row.caption, label.clone()));
                let replace = if prev != options {
                    *prev = options.clone();
                    Some(options.clone())
                } else {
                    None
                };
                let sel = (*selected < options.len()).then_some(*selected);
                ops.push(SyncOp::Choice(row.control, replace, sel));
            }
            _ => {}
        }
    }
    ops
}

fn apply_sync_ops(ops: Vec<SyncOp>) {
    for op in ops {
        match op {
            SyncOp::Text(hwnd, text) => set_text(hwnd, &text),
            SyncOp::Range(track, ticks, tick, readout, formatted) => {
                send_msg(track, TBM_SETRANGEMAX, 0, ticks as isize);
                send_msg(track, TBM_SETPOS, 1, tick as isize);
                set_text(readout, &formatted);
            }
            SyncOp::Invalidate(hwnd) => unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            },
            SyncOp::Choice(combo, options, selected) => {
                if let Some(options) = options {
                    send_msg(combo, CB_RESETCONTENT, 0, 0);
                    for opt in &options {
                        let w = wide(opt);
                        send_msg(combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
                    }
                }
                if let Some(selected) = selected {
                    send_msg(combo, CB_SETCURSEL, selected, 0);
                }
            }
        }
    }
}

fn position_inner(inner: &Rc<RefCell<ToolState>>, force: bool) {
    let (owner, hwnd, old_dpi) = {
        let s = inner.borrow();
        (s.owner, s.hwnd, s.dpi)
    };
    unsafe {
        let monitor = MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mut anchor = RECT::default();
        if !GetMonitorInfoW(monitor, &mut info).as_bool()
            || GetWindowRect(owner, &mut anchor).is_err()
        {
            return;
        }
        let dpi = GetDpiForWindow(owner).max(96);
        let changed = old_dpi != dpi;
        if changed {
            inner.borrow_mut().dpi = dpi;
            update_font(inner, dpi);
        }
        let (panel_w, content_h, gap) = {
            let s = inner.borrow();
            (s.px(PANEL_WIDTH), s.content_height(), s.px(WINDOW_GAP))
        };
        let work = info.rcWork;
        let mut bounds = RECT {
            right: panel_w,
            bottom: content_h,
            ..Default::default()
        };
        let _ = AdjustWindowRectExForDpi(&mut bounds, style(), false, ex_style(), dpi);
        let w = (bounds.right - bounds.left)
            .min(work.right - work.left)
            .max(1);
        let h = (bounds.bottom - bounds.top)
            .min(work.bottom - work.top)
            .max(1);
        let right = anchor.right + gap;
        let x = if right <= work.right - w {
            right
        } else {
            anchor.left - gap - w
        };
        let x = x.clamp(work.left, (work.right - w).max(work.left));
        let y = anchor.top.clamp(work.top, (work.bottom - h).max(work.top));
        let mut actual = RECT::default();
        let sized = GetWindowRect(hwnd, &mut actual).is_err()
            || actual.right - actual.left != w
            || actual.bottom - actual.top != h;
        if sized || actual.left != x || actual.top != y {
            let _ = SetWindowPos(
                hwnd,
                None,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
            );
        }
        if force || sized || changed {
            layout_inner(inner);
        }
    }
}

fn layout_inner(inner: &Rc<RefCell<ToolState>>) {
    let hwnd = inner.borrow().hwnd;
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let page = client.bottom.max(1);
    let (content, offset, moves) = {
        let mut s = inner.borrow_mut();
        let content = s.content_height();
        s.offset = s.offset.clamp(0, (content - page).max(0));
        let pad = s.px(INSET);
        let width = (client.right - 2 * pad).max(40);
        let mut y = pad - s.offset;
        let mut moves = Vec::with_capacity(s.rows.len() * 3);
        for row in &s.rows {
            match &row.input {
                Input::Range { readout, .. } => {
                    let rw = s.px(44);
                    moves.push((row.caption, pad, y, (width - rw - 4).max(40), s.px(18)));
                    moves.push((*readout, pad + width - rw, y, rw, s.px(18)));
                    moves.push((row.control, pad, y + s.px(20), width, s.px(24)));
                    y += s.px(46 + ROW_GAP);
                }
                Input::Color { swatch, .. } => {
                    let sw = s.px(28);
                    let bw = s.px(74);
                    moves.push((
                        row.caption,
                        pad,
                        y + s.px(3),
                        (width - sw - bw - 12).max(40),
                        s.px(20),
                    ));
                    moves.push((
                        *swatch,
                        pad + width - sw - bw - 6,
                        y + s.px(2),
                        sw,
                        s.px(22),
                    ));
                    moves.push((row.control, pad + width - bw, y, bw, s.px(24)));
                    y += s.px(26 + ROW_GAP);
                }
                Input::Choice { .. } => {
                    moves.push((row.caption, pad, y, width, s.px(18)));
                    moves.push((row.control, pad, y + s.px(20), width, s.px(180)));
                    y += s.px(48 + ROW_GAP);
                }
            }
        }
        (content, s.offset, moves)
    };
    unsafe {
        SetScrollInfo(
            hwnd,
            SB_VERT,
            &SCROLLINFO {
                cbSize: size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: content.saturating_sub(1),
                nPage: page as u32,
                nPos: offset,
                ..Default::default()
            },
            true,
        );
        for (child, x, y, w, h) in moves {
            let _ = MoveWindow(child, x, y, w, h, true);
        }
    }
}

fn on_hscroll(inner: &Rc<RefCell<ToolState>>, code: u32, hwnd: HWND) {
    let info = {
        let s = inner.borrow();
        if s.syncing {
            return;
        }
        s.rows.iter().find_map(|r| {
            if r.control == hwnd
                && let Input::Range { scale, readout } = &r.input
            {
                Some((s.id.clone(), r.id.clone(), *scale, *readout))
            } else {
                None
            }
        })
    };
    let Some((overlay_id, row_id, scale, readout)) = info else {
        return;
    };
    let tick = send_msg(hwnd, TBM_GETPOS, 0, 0) as i32;
    let value = scale.value_at(tick);
    set_text(readout, &format_number(value, scale.step));

    match code {
        TB_THUMBTRACK => {
            {
                let mut s = inner.borrow_mut();
                s.dragging = Some(row_id.clone());
                s.pending_commit = Some(row_id.clone());
            }
            panel_changed(&overlay_id, &row_id, value, false);
        }
        // Trackbar always follows thumb release and keyboard navigation with
        // `TB_ENDTRACK`. Defer the commit until `TB_ENDTRACK` so a release or
        // arrow step emits one committed event instead of two.
        TB_THUMBPOSITION | TB_LINEUP | TB_LINEDOWN | TB_PAGEUP | TB_PAGEDOWN | TB_TOP
        | TB_BOTTOM => {
            inner.borrow_mut().pending_commit = Some(row_id);
        }
        TB_ENDTRACK => {
            let should_commit = {
                let mut s = inner.borrow_mut();
                let was_dragging = s.dragging.as_deref() == Some(row_id.as_str());
                if was_dragging {
                    s.dragging = None;
                }
                let had_pending = s.pending_commit.as_deref() == Some(row_id.as_str());
                if had_pending {
                    s.pending_commit = None;
                }
                was_dragging || had_pending
            };
            if should_commit {
                panel_changed(&overlay_id, &row_id, value, true);
            }
        }
        _ => {}
    }
}

fn on_command(inner: &Rc<RefCell<ToolState>>, notify: u32, hwnd: HWND) {
    enum Command {
        Color {
            owner: HWND,
            overlay_id: String,
            row_id: String,
            rgb: u32,
        },
        Choice {
            overlay_id: String,
            row_id: String,
        },
    }
    let command = {
        let s = inner.borrow();
        if s.syncing {
            return;
        }
        s.rows.iter().find_map(|r| {
            if r.control != hwnd {
                return None;
            }
            match &r.input {
                Input::Color { rgb, .. } if notify == BN_CLICKED => Some(Command::Color {
                    owner: s.hwnd,
                    overlay_id: s.id.clone(),
                    row_id: r.id.clone(),
                    rgb: *rgb,
                }),
                Input::Choice { .. } if notify == CBN_SELCHANGE => Some(Command::Choice {
                    overlay_id: s.id.clone(),
                    row_id: r.id.clone(),
                }),
                _ => None,
            }
        })
    };

    match command {
        Some(Command::Color {
            owner,
            overlay_id,
            row_id,
            rgb,
        }) => {
            let mut palette = CUSTOM_COLORS.with(|c| *c.borrow());
            let mut cc = CHOOSECOLORW {
                lStructSize: size_of::<CHOOSECOLORW>() as u32,
                hwndOwner: owner,
                rgbResult: colorref(rgb),
                lpCustColors: palette.as_mut_ptr(),
                Flags: CC_RGBINIT | CC_FULLOPEN,
                ..Default::default()
            };
            // No RefCell borrow is held while ChooseColorW runs its modal loop.
            let accepted = unsafe { ChooseColorW(&mut cc) }.as_bool();
            CUSTOM_COLORS.with(|c| *c.borrow_mut() = palette);
            if accepted {
                let next = from_colorref(cc.rgbResult);
                // Re-lookup the row by id in case the panel rebuilt during the dialog.
                let updated = {
                    let mut s = inner.borrow_mut();
                    s.rows.iter_mut().find_map(|r| {
                        if r.id != row_id {
                            return None;
                        }
                        if let Input::Color { rgb, swatch, brush } = &mut r.input {
                            *rgb = next;
                            if let Some(old) =
                                brush.replace(unsafe { CreateSolidBrush(cc.rgbResult) })
                            {
                                unsafe {
                                    let _ = DeleteObject(HGDIOBJ(old.0));
                                }
                            }
                            Some((r.control, *swatch))
                        } else {
                            None
                        }
                    })
                };
                if let Some((btn, swatch)) = updated {
                    set_text(btn, &format!("#{next:06X}"));
                    unsafe {
                        let _ = InvalidateRect(Some(swatch), None, true);
                    }
                }
                panel_changed(&overlay_id, &row_id, f64::from(next), true);
            }
        }
        Some(Command::Choice { overlay_id, row_id }) => {
            let sel = send_msg(hwnd, CB_GETCURSEL, 0, 0);
            if sel >= 0 {
                panel_changed(&overlay_id, &row_id, sel as f64, true);
            }
        }
        None => {}
    }
}

fn on_vscroll(inner: &Rc<RefCell<ToolState>>, code: u32) {
    let (hwnd, step) = {
        let s = inner.borrow();
        (s.hwnd, s.px(24))
    };
    let mut info = SCROLLINFO {
        cbSize: size_of::<SCROLLINFO>() as u32,
        fMask: SIF_ALL,
        ..Default::default()
    };
    if unsafe { GetScrollInfo(hwnd, SB_VERT, &mut info) }.is_err() {
        return;
    }
    let mut pos = info.nPos;
    match SCROLLBAR_COMMAND(code as i32) {
        SB_LINEUP => pos -= step,
        SB_LINEDOWN => pos += step,
        SB_PAGEUP => pos -= info.nPage as i32,
        SB_PAGEDOWN => pos += info.nPage as i32,
        SB_THUMBTRACK => pos = info.nTrackPos,
        _ => return,
    }
    let max_pos = (info.nMax - info.nPage as i32 + 1).max(0);
    inner.borrow_mut().offset = pos.clamp(0, max_pos);
    layout_inner(inner);
}

fn same_kind(input: &Input, spec: &Control) -> bool {
    matches!(
        (input, spec),
        (Input::Range { .. }, Control::Range { .. })
            | (Input::Color { .. }, Control::Color { .. })
            | (Input::Choice { .. }, Control::Choice { .. })
    )
}

fn colorref(rgb: u32) -> COLORREF {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    COLORREF(r | (g << 8) | (b << 16))
}

fn from_colorref(c: COLORREF) -> u32 {
    let r = c.0 & 0xff;
    let g = (c.0 >> 8) & 0xff;
    let b = (c.0 >> 16) & 0xff;
    (r << 16) | (g << 8) | b
}

/// Clone the `Rc<RefCell<ToolState>>` stored in `GWLP_USERDATA` without taking
/// ownership of the window's reference.
unsafe fn state_of(hwnd: HWND) -> Option<Rc<RefCell<ToolState>>> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const RefCell<ToolState>;
    if ptr.is_null() {
        return None;
    }
    unsafe {
        Rc::increment_strong_count(ptr);
        Some(Rc::from_raw(ptr))
    }
}

extern "system" fn proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        if message == WM_NCDESTROY {
            let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *const RefCell<ToolState>;
            if !ptr.is_null() {
                let _ = Rc::from_raw(ptr);
            }
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let Some(inner) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };
        match message {
            WM_CLOSE => {
                dismiss_inner(&inner);
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
                dismiss_inner(&inner);
                LRESULT(0)
            }
            WM_HSCROLL => {
                on_hscroll(&inner, (wparam.0 & 0xffff) as u32, HWND(lparam.0 as *mut _));
                LRESULT(0)
            }
            WM_COMMAND => {
                let notify = ((wparam.0 >> 16) & 0xffff) as u32;
                on_command(&inner, notify, HWND(lparam.0 as *mut _));
                LRESULT(0)
            }
            WM_VSCROLL => {
                on_vscroll(&inner, (wparam.0 & 0xffff) as u32);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xffff) as i16;
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                {
                    let mut s = inner.borrow_mut();
                    let max_pos = (s.content_height() - r.bottom.max(1)).max(0);
                    s.offset = (s.offset - (delta as i32 / 120) * s.px(32)).clamp(0, max_pos);
                }
                layout_inner(&inner);
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC => {
                let target = HWND(lparam.0 as *mut _);
                if let Some(brush) = inner.borrow().swatch_brush(target) {
                    return LRESULT(brush.0 as isize);
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_ACTIVATE if (wparam.0 & 0xffff) as u32 == WA_ACTIVE && GetFocus().0.is_null() => {
                let _ = SetFocus(Some(hwnd));
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
