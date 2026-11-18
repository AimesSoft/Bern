//! Owned Win32 utility tool window anchored beside a Windows overlay.

use super::{Control, Panel, panel_changed, panel_dismissed};
use std::mem::size_of;
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

static mut CUSTOM_COLORS: [COLORREF; 16] = [COLORREF(0xffffff); 16];

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

pub(super) struct Tool {
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
}

impl Tool {
    pub(super) fn new(id: &str, owner: HWND, instance: HINSTANCE) -> Option<Box<Self>> {
        let mut tool = Box::new(Self {
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
        });
        let ptr: *mut Tool = &mut *tool;
        let hwnd = unsafe {
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
                Some(ptr.cast()),
            )
        }
        .ok()?;
        tool.hwnd = hwnd;
        let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
        tool.dpi = dpi;
        tool.update_font(dpi);
        Some(tool)
    }

    fn px(&self, logical: i32) -> i32 {
        (logical as f64 * f64::from(self.dpi) / 96.0).round() as i32
    }

    fn update_font(&mut self, dpi: u32) {
        unsafe {
            if let Some(old) = self.font.take() {
                let _ = DeleteObject(HGDIOBJ(old.0));
            }
            let mut metrics = NONCLIENTMETRICSW {
                cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
                ..Default::default()
            };
            let font = if SystemParametersInfoForDpi(
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
            };
            if !font.0.is_null() {
                self.font = Some(font);
                for row in &self.rows {
                    send_msg(row.caption, WM_SETFONT, font.0 as usize, 1);
                    send_msg(row.control, WM_SETFONT, font.0 as usize, 1);
                    if let Input::Color { swatch, .. } = row.input {
                        send_msg(swatch, WM_SETFONT, font.0 as usize, 1);
                    }
                    if let Input::Range { readout, .. } = row.input {
                        send_msg(readout, WM_SETFONT, font.0 as usize, 1);
                    }
                }
            }
        }
    }

    pub(super) fn present(&mut self, model: Option<Panel>, owner_visible: bool) {
        let Some(model) = model else {
            self.wanted = false;
            self.hide();
            return;
        };
        self.wanted = true;
        self.syncing = true;
        set_text(self.hwnd, &model.title);

        let same_shape = self.rows.len() == model.controls.len()
            && self
                .rows
                .iter()
                .zip(&model.controls)
                .all(|(r, c)| r.id == key(c) && same_kind(&r.input, c));
        if !same_shape {
            for row in self.rows.drain(..) {
                row.destroy();
            }
            for (index, spec) in model.controls.iter().enumerate() {
                if let Some(row) = self.create_row(index, spec) {
                    self.rows.push(row);
                }
            }
        }
        for (row, spec) in self.rows.iter_mut().zip(&model.controls) {
            Self::sync_row(row, spec);
        }
        self.syncing = false;
        if owner_visible {
            self.position(true);
            if !self.visible {
                self.visible = true;
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                }
            }
        } else {
            self.hide();
        }
    }

    pub(super) fn owner_moved(&mut self, owner_visible: bool) {
        if !self.wanted || !owner_visible {
            self.hide();
            return;
        }
        self.position(false);
        if !self.visible {
            self.visible = true;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }
    }

    pub(super) fn hide(&mut self) {
        if self.visible {
            self.visible = false;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    fn create_row(&self, index: usize, spec: &Control) -> Option<Row> {
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
                Some(self.hwnd),
                Some(HMENU(id_base as *mut _)),
                Some(self.instance),
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
                        Some(self.hwnd),
                        Some(HMENU((id_base + 1) as *mut _)),
                        Some(self.instance),
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
                        Some(self.hwnd),
                        Some(HMENU((id_base + 2) as *mut _)),
                        Some(self.instance),
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
                if let Some(font) = self.font {
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
                        Some(self.hwnd),
                        Some(HMENU((id_base + 1) as *mut _)),
                        Some(self.instance),
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
                        Some(self.hwnd),
                        Some(HMENU((id_base + 2) as *mut _)),
                        Some(self.instance),
                        None,
                    )
                }
                .ok()?;
                if let Some(font) = self.font {
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
                        Some(self.hwnd),
                        Some(HMENU((id_base + 2) as *mut _)),
                        Some(self.instance),
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
        if let Some(font) = self.font {
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

    fn sync_row(row: &mut Row, spec: &Control) {
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
                set_text(row.caption, label);
                *scale = Scale::new(*min, *max, *step);
                send_msg(row.control, TBM_SETRANGEMAX, 0, scale.ticks as isize);
                send_msg(row.control, TBM_SETPOS, 1, scale.to_tick(*value) as isize);
                set_text(*readout, &format_number(*value, *step));
            }
            (
                Input::Color { rgb, swatch, brush },
                Control::Color {
                    label, rgb: next, ..
                },
            ) => {
                set_text(row.caption, label);
                let next = *next & 0xffffff;
                if *rgb != next {
                    *rgb = next;
                    if let Some(old) = brush.replace(unsafe { CreateSolidBrush(colorref(next)) }) {
                        unsafe {
                            let _ = DeleteObject(HGDIOBJ(old.0));
                        }
                    }
                    unsafe {
                        let _ = InvalidateRect(Some(*swatch), None, true);
                    }
                }
                set_text(row.control, &format!("#{next:06X}"));
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
                set_text(row.caption, label);
                if prev != options {
                    send_msg(row.control, CB_RESETCONTENT, 0, 0);
                    for opt in options {
                        let w = wide(opt);
                        send_msg(row.control, CB_ADDSTRING, 0, w.as_ptr() as isize);
                    }
                    *prev = options.clone();
                }
                if *selected < options.len() {
                    send_msg(row.control, CB_SETCURSEL, *selected, 0);
                }
            }
            _ => {}
        }
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

    fn position(&mut self, force: bool) {
        unsafe {
            let monitor = MonitorFromWindow(self.owner, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let mut anchor = RECT::default();
            if !GetMonitorInfoW(monitor, &mut info).as_bool()
                || GetWindowRect(self.owner, &mut anchor).is_err()
            {
                return;
            }
            let dpi = GetDpiForWindow(self.owner).max(96);
            let changed = self.dpi != dpi;
            if changed {
                self.dpi = dpi;
                self.update_font(dpi);
            }
            let work = info.rcWork;
            let mut bounds = RECT {
                right: self.px(PANEL_WIDTH),
                bottom: self.content_height(),
                ..Default::default()
            };
            let _ = AdjustWindowRectExForDpi(&mut bounds, style(), false, ex_style(), dpi);
            let w = (bounds.right - bounds.left)
                .min(work.right - work.left)
                .max(1);
            let h = (bounds.bottom - bounds.top)
                .min(work.bottom - work.top)
                .max(1);
            let gap = self.px(WINDOW_GAP);
            let right = anchor.right + gap;
            let x = if right <= work.right - w {
                right
            } else {
                anchor.left - gap - w
            };
            let x = x.clamp(work.left, (work.right - w).max(work.left));
            let y = anchor.top.clamp(work.top, (work.bottom - h).max(work.top));
            let mut actual = RECT::default();
            let sized = GetWindowRect(self.hwnd, &mut actual).is_err()
                || actual.right - actual.left != w
                || actual.bottom - actual.top != h;
            if sized || actual.left != x || actual.top != y {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
                );
            }
            if force || sized || changed {
                self.layout();
            }
        }
    }

    fn layout(&mut self) {
        let mut client = RECT::default();
        let _ = unsafe { GetClientRect(self.hwnd, &mut client) };
        let page = client.bottom.max(1);
        let content = self.content_height();
        self.offset = self.offset.clamp(0, (content - page).max(0));
        unsafe {
            SetScrollInfo(
                self.hwnd,
                SB_VERT,
                &SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                    nMin: 0,
                    nMax: content.saturating_sub(1),
                    nPage: page as u32,
                    nPos: self.offset,
                    ..Default::default()
                },
                true,
            );
        }
        let pad = self.px(INSET);
        let width = (client.right - 2 * pad).max(40);
        let mut y = pad - self.offset;
        for row in &self.rows {
            match &row.input {
                Input::Range { readout, .. } => {
                    let rw = self.px(44);
                    unsafe {
                        let _ = MoveWindow(
                            row.caption,
                            pad,
                            y,
                            (width - rw - 4).max(40),
                            self.px(18),
                            true,
                        );
                        let _ = MoveWindow(*readout, pad + width - rw, y, rw, self.px(18), true);
                        let _ =
                            MoveWindow(row.control, pad, y + self.px(20), width, self.px(24), true);
                    }
                    y += self.px(46 + ROW_GAP);
                }
                Input::Color { swatch, .. } => {
                    let sw = self.px(28);
                    let bw = self.px(74);
                    unsafe {
                        let _ = MoveWindow(
                            row.caption,
                            pad,
                            y + self.px(3),
                            (width - sw - bw - 12).max(40),
                            self.px(20),
                            true,
                        );
                        let _ = MoveWindow(
                            *swatch,
                            pad + width - sw - bw - 6,
                            y + self.px(2),
                            sw,
                            self.px(22),
                            true,
                        );
                        let _ = MoveWindow(row.control, pad + width - bw, y, bw, self.px(24), true);
                    }
                    y += self.px(26 + ROW_GAP);
                }
                Input::Choice { .. } => {
                    unsafe {
                        let _ = MoveWindow(row.caption, pad, y, width, self.px(18), true);
                        let _ = MoveWindow(
                            row.control,
                            pad,
                            y + self.px(20),
                            width,
                            self.px(180),
                            true,
                        );
                    }
                    y += self.px(48 + ROW_GAP);
                }
            }
        }
    }

    fn on_hscroll(&mut self, code: u32, hwnd: HWND) {
        if self.syncing {
            return;
        }
        let Some(row) = self.rows.iter().find(|r| r.control == hwnd) else {
            return;
        };
        let Input::Range { scale, readout } = &row.input else {
            return;
        };
        let tick = send_msg(hwnd, TBM_GETPOS, 0, 0) as i32;
        let value = scale.value_at(tick);
        set_text(*readout, &format_number(value, scale.step));
        let committed = matches!(
            code,
            TB_ENDTRACK
                | TB_THUMBPOSITION
                | TB_LINEUP
                | TB_LINEDOWN
                | TB_PAGEUP
                | TB_PAGEDOWN
                | TB_TOP
                | TB_BOTTOM
        );
        if code == TB_THUMBTRACK || committed {
            panel_changed(&self.id, &row.id, value, committed);
        }
    }

    fn on_command(&mut self, notify: u32, hwnd: HWND) {
        if self.syncing {
            return;
        }
        let Some(index) = self.rows.iter().position(|r| r.control == hwnd) else {
            return;
        };
        let id = self.rows[index].id.clone();
        match &mut self.rows[index].input {
            Input::Color { rgb, swatch, brush } if notify == BN_CLICKED => {
                let mut cc = CHOOSECOLORW {
                    lStructSize: size_of::<CHOOSECOLORW>() as u32,
                    hwndOwner: self.hwnd,
                    rgbResult: colorref(*rgb),
                    lpCustColors: (&raw mut CUSTOM_COLORS).cast(),
                    Flags: CC_RGBINIT | CC_FULLOPEN,
                    ..Default::default()
                };
                if unsafe { ChooseColorW(&mut cc) }.as_bool() {
                    let next = from_colorref(cc.rgbResult);
                    *rgb = next;
                    if let Some(old) = brush.replace(unsafe { CreateSolidBrush(cc.rgbResult) }) {
                        unsafe {
                            let _ = DeleteObject(HGDIOBJ(old.0));
                        }
                    }
                    set_text(hwnd, &format!("#{next:06X}"));
                    unsafe {
                        let _ = InvalidateRect(Some(*swatch), None, true);
                    }
                    panel_changed(&self.id, &id, f64::from(next), true);
                }
            }
            Input::Choice { .. } if notify == CBN_SELCHANGE => {
                let sel = send_msg(hwnd, CB_GETCURSEL, 0, 0);
                if sel >= 0 {
                    panel_changed(&self.id, &id, sel as f64, true);
                }
            }
            _ => {}
        }
    }

    fn on_vscroll(&mut self, code: u32) {
        let mut info = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_ALL,
            ..Default::default()
        };
        if unsafe { GetScrollInfo(self.hwnd, SB_VERT, &mut info) }.is_err() {
            return;
        }
        let step = self.px(24);
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
        self.offset = pos.clamp(0, max_pos);
        self.layout();
    }
}

impl Drop for Tool {
    fn drop(&mut self) {
        if let Some(font) = self.font.take() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
        for row in self.rows.drain(..) {
            row.destroy();
        }
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
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

extern "system" fn proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            return LRESULT(1);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Tool;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let tool = &mut *ptr;
        match message {
            WM_CLOSE => {
                tool.wanted = false;
                tool.hide();
                panel_dismissed(&tool.id);
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
                tool.wanted = false;
                tool.hide();
                panel_dismissed(&tool.id);
                LRESULT(0)
            }
            WM_HSCROLL => {
                tool.on_hscroll((wparam.0 & 0xffff) as u32, HWND(lparam.0 as *mut _));
                LRESULT(0)
            }
            WM_COMMAND => {
                let notify = ((wparam.0 >> 16) & 0xffff) as u32;
                tool.on_command(notify, HWND(lparam.0 as *mut _));
                LRESULT(0)
            }
            WM_VSCROLL => {
                tool.on_vscroll((wparam.0 & 0xffff) as u32);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xffff) as i16;
                let max_pos = (tool.content_height() - {
                    let mut r = RECT::default();
                    let _ = GetClientRect(hwnd, &mut r);
                    r.bottom.max(1)
                })
                .max(0);
                tool.offset = (tool.offset - (delta as i32 / 120) * tool.px(32)).clamp(0, max_pos);
                tool.layout();
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC => {
                let target = HWND(lparam.0 as *mut _);
                for row in &tool.rows {
                    if let Input::Color {
                        swatch,
                        brush: Some(brush),
                        ..
                    } = &row.input
                        && *swatch == target
                    {
                        return LRESULT(brush.0 as isize);
                    }
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
