//! Native nonactivating utility panel anchored beside a macOS overlay.

use super::{Control, Panel, panel_changed, panel_dismissed};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSApplicationDidHideNotification,
    NSBackingStoreType, NSColor, NSColorSpace, NSColorWell, NSControl, NSFont, NSPanel,
    NSPopUpButton, NSScrollView, NSSlider, NSTextAlignment, NSTextField, NSView,
    NSWindowCollectionBehavior, NSWindowDelegate, NSWindowDidChangeScreenNotification,
    NSWindowDidMoveNotification, NSWindowDidResizeNotification, NSWindowOrderingMode,
    NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

const FORM_WIDTH: f64 = 252.0;
const INSET: f64 = 12.0;
const ROW_GAP: f64 = 10.0;
const WINDOW_GAP: f64 = 6.0;

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

fn srgb(rgb: u32) -> Retained<NSColor> {
    let r = ((rgb >> 16) & 0xff) as f64 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f64 / 255.0;
    let b = (rgb & 0xff) as f64 / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0)
}

fn read_rgb(color: &NSColor) -> u32 {
    let srgb = color
        .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
        .unwrap_or_else(|| srgb(0xffffff));
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (byte(srgb.redComponent()) << 16)
        | (byte(srgb.greenComponent()) << 8)
        | byte(srgb.blueComponent())
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

define_class!(
    // SAFETY: Top-left coordinate system for vertical form layout; no custom drawing.
    #[unsafe(super(NSView))]
    #[name = "BernOverlayFormView"]
    struct FormView;

    impl FormView {
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool {
            true
        }
    }
);

define_class!(
    // SAFETY: Subclasses NSPanel to forward Escape to standard close handling.
    #[unsafe(super(NSPanel))]
    #[name = "BernOverlayInspectorPanel"]
    struct InspectorPanel;

    impl InspectorPanel {
        #[unsafe(method(cancelOperation:))]
        fn cancel(&self, sender: Option<&AnyObject>) {
            self.performClose(sender);
        }
    }
);

struct Row {
    id: String,
    caption: Retained<NSTextField>,
    kind: RowKind,
}

enum RowKind {
    Range {
        slider: Retained<NSSlider>,
        readout: Retained<NSTextField>,
        step: f64,
    },
    Color {
        well: Retained<NSColorWell>,
    },
    Choice {
        popup: Retained<NSPopUpButton>,
        options: Vec<String>,
    },
}

impl Row {
    fn new(
        id: &str,
        spec: &Control,
        target: &Adjustment,
        document: &FormView,
        mtm: MainThreadMarker,
    ) -> Self {
        let caption = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        caption.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        document.addSubview(&caption);

        let kind = match spec {
            Control::Range { min, max, .. } => {
                let readout = NSTextField::labelWithString(&NSString::from_str(""), mtm);
                readout.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
                    11.0, 0.0,
                )));
                readout.setAlignment(NSTextAlignment::Right);
                readout.setTextColor(Some(&NSColor::secondaryLabelColor()));
                document.addSubview(&readout);

                let slider =
                    NSSlider::initWithFrame(NSSlider::alloc(mtm), rect(0.0, 0.0, 100.0, 20.0));
                slider.setMinValue(*min);
                slider.setMaxValue(*max);
                slider.setContinuous(true);
                unsafe {
                    slider.setTarget(Some(target));
                    slider.setAction(Some(sel!(controlChanged:)));
                }
                document.addSubview(&slider);
                RowKind::Range {
                    slider,
                    readout,
                    step: 1.0,
                }
            }
            Control::Color { rgb, .. } => {
                let well =
                    NSColorWell::initWithFrame(NSColorWell::alloc(mtm), rect(0.0, 0.0, 44.0, 24.0));
                well.setColor(&srgb(*rgb));
                unsafe {
                    well.setTarget(Some(target));
                    well.setAction(Some(sel!(controlChanged:)));
                }
                document.addSubview(&well);
                RowKind::Color { well }
            }
            Control::Choice { .. } => {
                let popup = NSPopUpButton::initWithFrame_pullsDown(
                    NSPopUpButton::alloc(mtm),
                    rect(0.0, 0.0, 100.0, 24.0),
                    false,
                );
                unsafe {
                    popup.setTarget(Some(target));
                    popup.setAction(Some(sel!(controlChanged:)));
                }
                document.addSubview(&popup);
                RowKind::Choice {
                    popup,
                    options: Vec::new(),
                }
            }
        };

        Self {
            id: id.to_owned(),
            caption,
            kind,
        }
    }

    fn matches(&self, spec: &Control) -> bool {
        match (&self.kind, spec) {
            (RowKind::Range { .. }, Control::Range { id, .. })
            | (RowKind::Color { .. }, Control::Color { id, .. })
            | (RowKind::Choice { .. }, Control::Choice { id, .. }) => self.id == *id,
            _ => false,
        }
    }

    fn control(&self) -> &NSControl {
        match &self.kind {
            RowKind::Range { slider, .. } => slider,
            RowKind::Color { well } => well,
            RowKind::Choice { popup, .. } => popup,
        }
    }

    fn sync(&mut self, spec: &Control) {
        match (&mut self.kind, spec) {
            (
                RowKind::Range {
                    slider,
                    readout,
                    step: stored_step,
                },
                Control::Range {
                    label,
                    min,
                    max,
                    step,
                    value,
                    ..
                },
            ) => {
                self.caption.setStringValue(&NSString::from_str(label));
                *stored_step = *step;
                slider.setMinValue(*min);
                slider.setMaxValue(*max);
                slider.setDoubleValue(value.clamp(*min, *max));
                readout.setStringValue(&NSString::from_str(&format_number(*value, *step)));
            }
            (RowKind::Color { well }, Control::Color { label, rgb, .. }) => {
                self.caption.setStringValue(&NSString::from_str(label));
                if read_rgb(&well.color()) != (*rgb & 0xffffff) {
                    well.setColor(&srgb(*rgb));
                }
            }
            (
                RowKind::Choice {
                    popup,
                    options: prev,
                },
                Control::Choice {
                    label,
                    options,
                    selected,
                    ..
                },
            ) => {
                self.caption.setStringValue(&NSString::from_str(label));
                if prev != options {
                    popup.removeAllItems();
                    for opt in options {
                        popup.addItemWithTitle(&NSString::from_str(opt));
                    }
                    *prev = options.clone();
                }
                if *selected < options.len() {
                    popup.selectItemAtIndex(*selected as isize);
                }
            }
            _ => {}
        }
    }

    fn height(&self) -> f64 {
        match &self.kind {
            RowKind::Range { .. } => 42.0,
            RowKind::Color { .. } => 26.0,
            RowKind::Choice { .. } => 46.0,
        }
    }

    fn layout(&self, x: f64, y: f64, width: f64) {
        match &self.kind {
            RowKind::Range {
                slider, readout, ..
            } => {
                self.caption
                    .setFrame(rect(x, y, (width - 48.0).max(40.0), 16.0));
                readout.setFrame(rect(x + width - 46.0, y, 46.0, 16.0));
                slider.setFrame(rect(x, y + 20.0, width, 20.0));
            }
            RowKind::Color { well } => {
                self.caption
                    .setFrame(rect(x, y + 4.0, (width - 52.0).max(40.0), 18.0));
                well.setFrame(rect(x + width - 44.0, y, 44.0, 24.0));
            }
            RowKind::Choice { popup, .. } => {
                self.caption.setFrame(rect(x, y, width, 16.0));
                popup.setFrame(rect(x, y + 20.0, width, 24.0));
            }
        }
    }

    fn change(&self) -> Option<(f64, bool)> {
        match &self.kind {
            RowKind::Range {
                slider,
                readout,
                step,
            } => {
                let raw = slider.doubleValue();
                let value = if *step > 0.0 && step.is_finite() {
                    let min = slider.minValue();
                    let max = slider.maxValue();
                    (min + ((raw - min) / *step).round() * *step).clamp(min, max)
                } else {
                    raw
                };
                readout.setStringValue(&NSString::from_str(&format_number(value, *step)));
                let event = unsafe {
                    objc2_app_kit::NSApplication::sharedApplication(
                        MainThreadMarker::new_unchecked(),
                    )
                    .currentEvent()
                };
                let committed = !event.is_some_and(|e| {
                    matches!(
                        e.r#type(),
                        objc2_app_kit::NSEventType::LeftMouseDown
                            | objc2_app_kit::NSEventType::LeftMouseDragged
                    )
                });
                Some((value, committed))
            }
            RowKind::Color { well } => Some((f64::from(read_rgb(&well.color())), true)),
            RowKind::Choice { popup, .. } => {
                let idx = popup.indexOfSelectedItem();
                (idx >= 0).then_some((idx as f64, true))
            }
        }
    }

    fn remove(self) {
        self.caption.removeFromSuperview();
        match self.kind {
            RowKind::Range {
                slider, readout, ..
            } => {
                slider.removeFromSuperview();
                readout.removeFromSuperview();
            }
            RowKind::Color { well } => well.removeFromSuperview(),
            RowKind::Choice { popup, .. } => popup.removeFromSuperview(),
        }
    }
}

pub(super) struct State {
    id: String,
    parent: Retained<NSPanel>,
    window: Retained<NSPanel>,
    scroll: Retained<NSScrollView>,
    document: Retained<FormView>,
    rows: RefCell<Vec<Row>>,
    document_height: Cell<f64>,
    layout_dirty: Cell<bool>,
    wanted: Cell<bool>,
    attached: Cell<bool>,
    syncing: Cell<bool>,
}

define_class!(
    // SAFETY: All AppKit views, panels and notifications run on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "BernOverlayAdjustment"]
    #[ivars = State]
    pub(super) struct Adjustment;

    unsafe impl NSObjectProtocol for Adjustment {}

    unsafe impl NSWindowDelegate for Adjustment {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &AnyObject) -> bool {
            let id = self.ivars().id.clone();
            if self.fold_flags_only() {
                panel_dismissed(&id);
                super::mac::detach_panel(id);
            }
            false
        }
    }

    impl Adjustment {
        #[unsafe(method(parentFrameChanged:))]
        fn parent_frame_changed(&self, _: &NSNotification) {
            if !self.ivars().syncing.get() {
                self.refresh();
            }
        }

        #[unsafe(method(appHidden:))]
        fn app_hidden(&self, _: &NSNotification) {
            if self.fold() {
                panel_dismissed(&self.ivars().id);
            }
        }

        #[unsafe(method(controlChanged:))]
        fn control_changed(&self, sender: &NSControl) {
            let s = self.ivars();
            if !s.attached.get() || s.syncing.get() {
                return;
            }
            let change = s
                .rows
                .borrow()
                .iter()
                .find(|row| std::ptr::eq(row.control(), sender))
                .and_then(|row| row.change().map(|(v, commit)| (row.id.clone(), v, commit)));
            if let Some((control, value, committed)) = change {
                panel_changed(&s.id, &control, value, committed);
            }
        }
    }
);

impl Adjustment {
    pub(super) fn for_parent(
        id: &str,
        parent: Retained<NSPanel>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let frame = rect(0.0, 0.0, FORM_WIDTH, 1.0);
        let mask = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::UtilityWindow
            | NSWindowStyleMask::NonactivatingPanel;
        let this = InspectorPanel::alloc(mtm).set_ivars(());
        // SAFETY: NSPanel's designated initializer.
        let window: Retained<InspectorPanel> = unsafe {
            msg_send![super(this), initWithContentRect: frame, styleMask: mask,
                backing: NSBackingStoreType::Buffered, defer: false]
        };
        let window = window.into_super();
        window.setFloatingPanel(true);
        window.setHidesOnDeactivate(false);
        window.setBecomesKeyOnlyIfNeeded(false);
        window.setMovable(false);
        window.setExcludedFromWindowsMenu(true);
        window.setLevel(parent.level());
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe {
            window.setReleasedWhenClosed(false);
            let _: () = msg_send![&*window, setRestorable: false];
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setDrawsBackground(false);
        let this = FormView::alloc(mtm).set_ivars(());
        // SAFETY: NSView's designated initializer.
        let document: Retained<FormView> = unsafe { msg_send![super(this), initWithFrame: frame] };
        scroll.setDocumentView(Some(&document));
        window.setContentView(Some(&scroll));

        let this = Self::alloc(mtm).set_ivars(State {
            id: id.to_owned(),
            parent,
            window: window.clone(),
            scroll,
            document,
            rows: RefCell::new(Vec::new()),
            document_height: Cell::new(0.0),
            layout_dirty: Cell::new(true),
            wanted: Cell::new(false),
            attached: Cell::new(false),
            syncing: Cell::new(false),
        });
        // SAFETY: NSObject's designated initializer.
        let controller: Retained<Self> = unsafe { msg_send![super(this), init] };
        window.setDelegate(Some(ProtocolObject::from_ref(&*controller)));
        controller
    }

    pub(super) fn present(&self, model: Panel) {
        let s = self.ivars();
        let mtm = MainThreadMarker::from(self);
        s.syncing.set(true);
        let title = NSString::from_str(&model.title);
        if s.window.title() != title {
            s.window.setTitle(&title);
        }
        let mut old: HashMap<_, _> = s
            .rows
            .take()
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let mut seen = HashSet::new();
        let mut rows = Vec::with_capacity(model.controls.len());
        for spec in &model.controls {
            let id = match spec {
                Control::Range { id, .. }
                | Control::Color { id, .. }
                | Control::Choice { id, .. } => id,
            };
            if !seen.insert(id) {
                continue;
            }
            let mut row = match old.remove(id) {
                Some(row) if row.matches(spec) => row,
                other => {
                    if let Some(row) = other {
                        row.remove();
                    }
                    Row::new(id, spec, self, &s.document, mtm)
                }
            };
            row.sync(spec);
            rows.push(row);
        }
        for row in old.into_values() {
            row.remove();
        }
        s.document_height.set(
            2.0 * INSET
                + rows.iter().map(Row::height).sum::<f64>()
                + ROW_GAP * rows.len().saturating_sub(1) as f64,
        );
        s.rows.replace(rows);
        if !s.wanted.replace(true) {
            self.observe();
        }
        s.layout_dirty.set(true);
        self.refresh();
        s.syncing.set(false);
    }

    fn observe(&self) {
        let s = self.ivars();
        let center = NSNotificationCenter::defaultCenter();
        unsafe {
            for name in [
                NSWindowDidMoveNotification,
                NSWindowDidResizeNotification,
                NSWindowDidChangeScreenNotification,
            ] {
                center.addObserver_selector_name_object(
                    self,
                    sel!(parentFrameChanged:),
                    Some(name),
                    Some(&s.parent),
                );
            }
            center.addObserver_selector_name_object(
                self,
                sel!(parentFrameChanged:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
            );
            center.addObserver_selector_name_object(
                self,
                sel!(appHidden:),
                Some(NSApplicationDidHideNotification),
                None,
            );
        }
    }

    fn fold_flags_only(&self) -> bool {
        let s = self.ivars();
        if !s.wanted.replace(false) {
            return false;
        }
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(self) };
        true
    }

    pub(super) fn fold(&self) -> bool {
        if !self.fold_flags_only() {
            return false;
        }
        self.detach();
        true
    }

    pub(super) fn detach(&self) {
        let s = self.ivars();
        if !s.attached.replace(false) {
            return;
        }
        let was_syncing = s.syncing.replace(true);
        s.parent.removeChildWindow(&s.window);
        s.window.orderOut(None);
        s.syncing.set(was_syncing);
    }

    pub(super) fn refresh(&self) {
        let s = self.ivars();
        if !s.wanted.get() {
            return;
        }
        if !s.parent.isVisible() || !self.arrange() {
            self.detach();
        } else if !s.attached.replace(true) {
            unsafe {
                s.parent
                    .addChildWindow_ordered(&s.window, NSWindowOrderingMode::Above);
            }
            s.window.orderFrontRegardless();
        }
    }

    fn arrange(&self) -> bool {
        let s = self.ivars();
        let Some(screen) = s.parent.screen() else {
            return false;
        };
        let work = screen.visibleFrame();
        if work.size.width <= 0.0 || work.size.height <= 0.0 {
            return false;
        }
        let full =
            s.window
                .frameRectForContentRect(rect(0.0, 0.0, FORM_WIDTH, s.document_height.get()));
        let frame = attached_frame(s.parent.frame(), work, full.size);
        if s.window.frame() != frame {
            s.window.setFrame_display(frame, true);
        }
        let scrolls = full.size.height > frame.size.height;
        if s.scroll.hasVerticalScroller() != scrolls {
            s.scroll.setHasVerticalScroller(scrolls);
        }
        let clip = s.scroll.contentSize();
        let size = NSSize::new(clip.width, s.document_height.get().max(clip.height));
        let resized = s.document.frame().size != size;
        if resized {
            s.document.setFrameSize(size);
        }
        if s.layout_dirty.replace(false) || resized {
            let mut y = INSET;
            let width = (size.width - 2.0 * INSET).max(1.0);
            for row in s.rows.borrow().iter() {
                row.layout(INSET, y, width);
                y += row.height() + ROW_GAP;
            }
        }
        true
    }
}

fn attached_frame(parent: NSRect, work: NSRect, natural: NSSize) -> NSRect {
    let right = parent.origin.x + parent.size.width + WINDOW_GAP;
    let left = parent.origin.x - WINDOW_GAP;
    let max_x = work.origin.x + work.size.width;
    let room_right = max_x - right;
    let room_left = left - work.origin.x;
    let preferred = natural.width.min(work.size.width);
    let use_right = room_right >= preferred || (room_left < preferred && room_right >= room_left);
    let room = if use_right { room_right } else { room_left };
    let width = preferred.min(room.max(140.0));
    let height = natural.height.min(work.size.height);
    let x = if use_right { right } else { left - width };
    let y = parent.origin.y + parent.size.height - height;
    rect(
        work.origin.x + (x - work.origin.x).clamp(0.0, work.size.width - width),
        work.origin.y + (y - work.origin.y).clamp(0.0, work.size.height - height),
        width,
        height,
    )
}
